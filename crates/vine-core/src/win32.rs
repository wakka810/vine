use crate::runtime::TLS_OUT_OF_INDEXES;
use std::mem::size_of;

use encoding_rs::{EncoderResult, SHIFT_JIS};

use crate::api::{
    ApiCallFrame, ApiDeferredCall, ApiDispatcher, ApiEndpointIdentity, ApiFault,
    ApiGuestCallbackCall, ApiGuestWrite, ApiHandlerResult, ApiImmediate, ApiSymbol,
    DispatchOutcome, GuestCallbackArgument, GuestCallbackCompletionPolicy, GuestCallbackRequest,
    HostCompletion, commit_indirect_guest_writes,
};
use crate::memory::{GuestAddress, GuestMemory, MemoryAccess};
use crate::module_graph::{RuntimeModuleClass, RuntimeModuleLoadState};
use crate::pe32::ExportLookup;
use crate::runtime::{
    FLS_OUT_OF_INDEXES, RUNTIME_AUDIO_NOTIFICATION_LIMIT,
    RUNTIME_AUDIO_NOTIFICATION_PACING_SERVICE, RUNTIME_OPFS_READ_SERVICE,
    RUNTIME_PRIVATE_HEAP_ARENA_STRIDE, RUNTIME_SOURCE_READ_MAX_BYTES, RUNTIME_SOURCE_READ_SERVICE,
    RUNTIME_WAITABLE_TIMER_PACING_SERVICE, RUNTIME_WRITABLE_FILE_MAX_IO_BYTES,
    RuntimeApiBoundaryDisposition, RuntimeCreateThreadRequest, RuntimeCriticalSection,
    RuntimeFileBacking, RuntimeFindFirstFileOutcome, RuntimeHandle, RuntimeHeapInitialization,
    RuntimeMutexNameA, RuntimeObjectKind, RuntimePathResolution, RuntimeSourceReadHostResponse,
    RuntimeThreadPriority, RuntimeWaitDisposition, RuntimeWaitForSingleObjectExBegin,
    RuntimeWaitForSingleObjectExOutcome, RuntimeWaitState, X86_CRITICAL_SECTION_BYTE_LENGTH,
};

pub const RUNTIME_SLEEP_DELAY_SERVICE: &str = "browser.sleep";

const HEAP_ENABLE_TERMINATION_ON_CORRUPTION: u32 = 1;
const HEAP_NO_SERIALIZE: u32 = 0x0000_0001;
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
const ERROR_DISK_FULL: u32 = 112;
pub(crate) const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const ERROR_INVALID_NAME: u32 = 123;
const ERROR_MOD_NOT_FOUND: u32 = 126;
const ERROR_PROC_NOT_FOUND: u32 = 127;
const ERROR_ALREADY_EXISTS: u32 = 183;
pub(crate) const ERROR_NEGATIVE_SEEK: u32 = 131;
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
pub const MEM_RESERVE: u32 = 0x0000_2000;
pub const MEM_COMMIT: u32 = 0x0000_1000;
pub const MEM_DECOMMIT: u32 = 0x0000_4000;
pub const MEM_TOP_DOWN: u32 = 0x0010_0000;
pub const PAGE_READWRITE: u32 = 0x0000_0004;

pub(crate) fn kernel32_argument_words(endpoint: &ApiEndpointIdentity) -> Option<u8> {
    if !endpoint.module.eq_ignore_ascii_case("kernel32.dll") {
        return None;
    }
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return None;
    };
    Some(match name.as_str() {
        "GetCurrentProcessId"
        | "GetCurrentThreadId"
        | "GetCurrentThread"
        | "GetLastError"
        | "GetACP"
        | "GetSystemDefaultLangID"
        | "GetTickCount"
        | "GetCommandLineA"
        | "GetEnvironmentStringsW"
        | "TlsAlloc" => 0,
        "GetSystemTimeAsFileTime"
        | "GetSystemTime"
        | "GetLocalTime"
        | "ExitThread"
        | "ResumeThread"
        | "GetSystemInfo"
        | "GlobalMemoryStatus"
        | "GetVersionExA"
        | "SetLastError"
        | "GetDriveTypeA"
        | "IsProcessorFeaturePresent"
        | "SetUnhandledExceptionFilter"
        | "GetStdHandle"
        | "GetFileType"
        | "SetHandleCount"
        | "FreeEnvironmentStringsW"
        | "SetCurrentDirectoryA"
        | "GetFileAttributesA"
        | "FindClose"
        | "DeleteFileA"
        | "FlushFileBuffers"
        | "CloseHandle"
        | "SetEvent"
        | "ResetEvent"
        | "Sleep"
        | "QueryPerformanceFrequency"
        | "QueryPerformanceCounter"
        | "GetStartupInfoW"
        | "LoadLibraryA"
        | "GetModuleHandleW"
        | "TlsGetValue"
        | "EncodePointer"
        | "DecodePointer"
        | "FlsAlloc"
        | "FlsFree"
        | "FlsGetValue"
        | "InterlockedIncrement"
        | "InterlockedDecrement"
        | "InitializeCriticalSection"
        | "EnterCriticalSection"
        | "LeaveCriticalSection"
        | "DeleteCriticalSection" => 1,
        "GetExitCodeThread"
        | "SetThreadAffinityMask"
        | "SetThreadPriority"
        | "GetCPInfo"
        | "FindFirstFileA"
        | "FindNextFileA"
        | "CreateDirectoryA"
        | "GetFileSize"
        | "GetProcAddress"
        | "TlsSetValue"
        | "FlsSetValue"
        | "InitializeCriticalSectionAndSpinCount"
        | "WaitForSingleObject" => 2,
        "VirtualFree"
        | "CreateMutexA"
        | "CreateWaitableTimerA"
        | "WaitForSingleObjectEx"
        | "HeapCreate"
        | "HeapAlloc"
        | "HeapFree"
        | "HeapSize"
        | "GetModuleFileNameA" => 3,
        "RaiseException"
        | "RtlUnwind"
        | "VirtualAlloc"
        | "CreateEventA"
        | "SetFilePointer"
        | "WaitForMultipleObjects"
        | "GetStringTypeW"
        | "HeapSetInformation"
        | "HeapReAlloc" => 4,
        "ReadFile" | "WriteFile" => 5,
        "CreateThread" | "MultiByteToWideChar" | "SetWaitableTimer" | "LCMapStringW" => 6,
        "CreateFileA" => 7,
        "WideCharToMultiByte" => 8,
        _ => return None,
    })
}

fn handle_get_system_time_as_file_time(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let output = required_argument(endpoint, frame, 0, "FILETIME output")?;
    Ok(ApiImmediate::new(None).with_guest_write(
        GuestAddress(output),
        runtime.clock().wall_clock_file_time.to_le_bytes().to_vec(),
    ))
}

fn handle_raise_exception(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
    cpu: Option<(crate::api::X86CpuContext, GuestAddress)>,
) -> ApiHandlerResult {
    match prepare_raise_exception_callback(endpoint, frame, runtime, cpu) {
        Ok(request) => ApiHandlerResult::GuestCallback(Box::new(request)),
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn handle_rtl_unwind(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    match rtl_unwind_current_seh_frame(endpoint, frame, runtime) {
        Ok(immediate) => ApiHandlerResult::Immediate(immediate),
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn rtl_unwind_current_seh_frame(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let target_frame = GuestAddress(required_argument(endpoint, frame, 0, "target frame")?);
    let target_ip = GuestAddress(required_argument(endpoint, frame, 1, "target instruction")?);
    let exception_record = GuestAddress(required_argument(endpoint, frame, 2, "exception record")?);
    let return_value = required_argument(endpoint, frame, 3, "return value")?;
    if target_frame.0 == 0 {
        return Err(ApiFault::new("RtlUnwind exit unwind is not implemented"));
    }
    if target_ip != frame.return_address {
        return Err(ApiFault::new(
            "RtlUnwind currently supports the reached continuation at the API return address",
        ));
    }
    runtime
        .memory
        .validate_access(target_ip, 1, MemoryAccess::Execute)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    if exception_record.0 == 0 {
        return Err(ApiFault::new(
            "RtlUnwind with a generated exception record is not implemented",
        ));
    }
    let teb_address = runtime
        .current_thread_teb_address()
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let current_head = GuestAddress(read_guest_u32(&runtime.memory, teb_address, endpoint)?);
    if current_head != target_frame {
        return Err(ApiFault::new(
            "RtlUnwind currently supports a target equal to the current SEH chain head",
        ));
    }
    let registration = read_x86_exception_registration(runtime, target_frame, endpoint)?;
    let exception_bytes = runtime
        .memory
        .read(exception_record, X86_EXCEPTION_RECORD_BYTES)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    runtime
        .memory
        .validate_access(
            exception_record,
            X86_EXCEPTION_RECORD_BYTES,
            MemoryAccess::Write,
        )
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let exception_flags = u32::from_le_bytes(
        exception_bytes[4..8]
            .try_into()
            .expect("exception flags word"),
    );
    if exception_flags & !EXCEPTION_NONCONTINUABLE != 0 {
        return Err(ApiFault::new(
            "RtlUnwind currently supports the reached noncontinuable dispatch record",
        ));
    }
    let exception_flags_address = exception_record
        .0
        .checked_add(4)
        .map(GuestAddress)
        .ok_or_else(|| ApiFault::new("RtlUnwind exception flags cross the x86 address limit"))?;
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
    commit_indirect_guest_writes(&mut runtime.memory, &writes)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    Ok(ApiImmediate::new(Some(return_value)))
}

fn prepare_raise_exception_callback(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
    cpu: Option<(crate::api::X86CpuContext, GuestAddress)>,
) -> Result<GuestCallbackRequest, ApiFault> {
    let exception_code = required_argument(endpoint, frame, 0, "exception code")?;
    let exception_flags = required_argument(endpoint, frame, 1, "exception flags")?;
    let argument_count = required_argument(endpoint, frame, 2, "exception argument count")?;
    let arguments_address = required_argument(endpoint, frame, 3, "exception arguments")?;
    if exception_flags != EXCEPTION_NONCONTINUABLE {
        return Err(ApiFault::new(
            "RaiseException currently supports the reached noncontinuable exception variant",
        ));
    }
    let argument_count = usize::try_from(argument_count)
        .map_err(|_| ApiFault::new("RaiseException argument count does not fit host accounting"))?;
    if argument_count > EXCEPTION_MAXIMUM_PARAMETERS {
        return Err(ApiFault::new(
            "RaiseException accepts at most 15 exception arguments",
        ));
    }
    let argument_bytes = argument_count
        .checked_mul(4)
        .ok_or_else(|| ApiFault::new("RaiseException argument bytes overflowed host accounting"))?;
    let exception_arguments = if argument_bytes == 0 {
        Vec::new()
    } else {
        if arguments_address == 0 {
            return Err(ApiFault::new(
                "RaiseException requires a readable arguments array when the count is nonzero",
            ));
        }
        runtime
            .memory
            .read(GuestAddress(arguments_address), argument_bytes)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?
            .chunks_exact(4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("four-byte chunk")))
            .collect::<Vec<_>>()
    };

    let (context, callback_return_sentinel) = cpu.ok_or_else(|| {
        ApiFault::new("RaiseException dispatch requires the authoritative x86 register snapshot")
    })?;
    if context.esp != frame.stack_pointer.0 || context.eip != endpoint.address.0 {
        return Err(ApiFault::new(
            "RaiseException register snapshot does not identify the trapped API frame",
        ));
    }
    let teb_address = runtime
        .current_thread_teb_address()
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let exception_list = read_guest_u32(&runtime.memory, teb_address, endpoint)?;
    if exception_list == u32::MAX {
        return Err(unhandled_exception_fault(exception_code));
    }
    let registration =
        read_x86_exception_registration(runtime, GuestAddress(exception_list), endpoint)?;
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
    Ok(request)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct X86ExceptionRegistration {
    frame: GuestAddress,
    next: GuestAddress,
    handler: GuestAddress,
}

fn read_x86_exception_registration(
    runtime: &crate::runtime::Runtime,
    frame: GuestAddress,
    endpoint: &ApiEndpointIdentity,
) -> Result<X86ExceptionRegistration, ApiFault> {
    if frame.0 == u32::MAX || !frame.0.is_multiple_of(4) {
        return Err(ApiFault::new(
            "SEH registration frame must be aligned and may not be the chain terminator",
        ));
    }
    let stack = runtime
        .current_thread_state()
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
        .stack
        .as_ref()
        .ok_or_else(|| {
            ApiFault::new("SEH registration validation requires a registered x86 thread stack")
        })?;
    let frame_end = u64::from(frame.0)
        .checked_add(8)
        .ok_or_else(|| ApiFault::new("SEH registration frame crosses the x86 address limit"))?;
    if u64::from(frame.0) < u64::from(stack.committed_range.start().0)
        || frame_end > stack.committed_range.end()
    {
        return Err(ApiFault::new(
            "SEH registration frame is outside the current thread committed stack",
        ));
    }
    let bytes = runtime
        .memory
        .read(frame, 8)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let next = GuestAddress(u32::from_le_bytes(
        bytes[0..4].try_into().expect("next word"),
    ));
    let handler = GuestAddress(u32::from_le_bytes(
        bytes[4..8].try_into().expect("handler word"),
    ));
    if next.0 != u32::MAX {
        let next_end = u64::from(next.0)
            .checked_add(8)
            .ok_or_else(|| ApiFault::new("next SEH registration crosses the x86 address limit"))?;
        if !next.0.is_multiple_of(4)
            || next.0 <= frame.0
            || u64::from(next.0) < u64::from(stack.committed_range.start().0)
            || next_end > stack.committed_range.end()
        {
            return Err(ApiFault::new(
                "SEH registration chain must advance monotonically within the committed stack",
            ));
        }
    }
    runtime
        .memory
        .validate_access(handler, 1, MemoryAccess::Execute)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    Ok(X86ExceptionRegistration {
        frame,
        next,
        handler,
    })
}

fn read_guest_u32(
    memory: &GuestMemory,
    address: GuestAddress,
    endpoint: &ApiEndpointIdentity,
) -> Result<u32, ApiFault> {
    let bytes = memory
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
        .and_then(|value| {
            let argument_bytes = u32::try_from(frame.arguments.len()).ok()?.checked_mul(4)?;
            value.checked_add(argument_bytes)
        })
        .ok_or_else(|| {
            ApiFault::new("RaiseException continuation stack crosses the x86 address limit")
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
    ApiFault::new(format!(
        "guest exception {exception_code:#010x} reached the end of the SEH chain"
    ))
}

pub(crate) fn resolve_exception_dispatch_callback(
    call: &ApiGuestCallbackCall,
    result: u32,
    runtime: &crate::runtime::Runtime,
) -> ApiHandlerResult {
    let GuestCallbackCompletionPolicy::ExceptionDispatch {
        next_frame,
        exception_flags,
    } = call.request.completion_policy
    else {
        return ApiHandlerResult::Fault(ApiFault::new(
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
            match read_x86_exception_registration(runtime, next_frame, &call.frame.endpoint) {
                Ok(registration) => {
                    ApiHandlerResult::GuestCallback(Box::new(exception_dispatch_callback_request(
                        registration,
                        exception_flags,
                        call.request.scratch.clone(),
                        call.request.return_sentinel,
                    )))
                }
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        EXCEPTION_CONTINUE_EXECUTION if exception_flags & EXCEPTION_NONCONTINUABLE != 0 => {
            ApiHandlerResult::Fault(ApiFault::new(
                "guest SEH handler attempted to continue a noncontinuable exception",
            ))
        }
        EXCEPTION_CONTINUE_EXECUTION => ApiHandlerResult::Fault(ApiFault::new(
            "restoring a handler-modified x86 CONTEXT is not implemented",
        )),
        EXCEPTION_NESTED_EXCEPTION => ApiHandlerResult::Fault(ApiFault::new(
            "nested SEH exception disposition is not implemented",
        )),
        EXCEPTION_COLLIDED_UNWIND => ApiHandlerResult::Fault(ApiFault::new(
            "collided SEH unwind disposition is not implemented",
        )),
        _ => ApiHandlerResult::Fault(ApiFault::new(format!(
            "guest SEH handler returned invalid disposition {result}"
        ))),
    }
}

fn handle_get_system_time(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let output = required_argument(endpoint, frame, 0, "SYSTEMTIME output")?;
    Ok(ApiImmediate::new(None).with_guest_write(
        GuestAddress(output),
        system_time_bytes(runtime.clock().wall_clock_file_time),
    ))
}

fn handle_get_local_time(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let local_file_time = local_file_time(runtime.clock().wall_clock_file_time)?;
    let output = required_argument(endpoint, frame, 0, "SYSTEMTIME output")?;
    Ok(ApiImmediate::new(None)
        .with_guest_write(GuestAddress(output), system_time_bytes(local_file_time)))
}

fn local_file_time(file_time: u64) -> Result<u64, ApiFault> {
    // Fixed Japanese environment: UTC+09:00, without daylight saving.
    const JAPAN_OFFSET_TICKS: u64 = 540 * 600_000_000;
    file_time.checked_add(JAPAN_OFFSET_TICKS).ok_or_else(|| {
        ApiFault::new("runtime wall clock cannot be projected to Japanese local time")
    })
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

fn handle_get_current_thread(
    endpoint: &ApiEndpointIdentity,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let handle = runtime
        .current_thread_pseudo_handle()
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(handle)))
}

fn handle_get_exit_code_thread(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let handle = RuntimeHandle(required_argument(endpoint, frame, 0, "thread handle")?);
    let output = GuestAddress(required_argument(endpoint, frame, 1, "exit code output")?);
    let Some(exit_code) = runtime
        .thread_exit_code(handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
    else {
        return Ok(ApiImmediate::new(Some(0)).with_last_error(ERROR_INVALID_HANDLE));
    };
    Ok(ApiImmediate::new(Some(u32::from(true)))
        .with_guest_write(output, exit_code.to_le_bytes().to_vec()))
}

fn handle_exit_thread(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let exit_code = required_argument(endpoint, frame, 0, "exit code")?;
    let thread_id = runtime.current_thread_id();
    let thread = runtime
        .thread_state(thread_id)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    if thread.guest_bootstrap.is_none() {
        return Err(ApiFault::new(
            "bounded ExitThread currently supports dynamically created guest threads",
        ));
    }
    if !thread.callback_frames.is_empty() {
        return Err(ApiFault::new(
            "bounded ExitThread requires no active guest callback frames",
        ));
    }
    Ok(ApiImmediate::new(Some(exit_code))
        .with_execution_boundary(RuntimeApiBoundaryDisposition::TerminateCurrentThread))
}

fn handle_virtual_alloc_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let preferred = required_argument(endpoint, frame, 0, "preferred address")?;
    let size = required_argument(endpoint, frame, 1, "reservation size")?;
    let allocation_type = required_argument(endpoint, frame, 2, "allocation type")?;
    let protection = required_argument(endpoint, frame, 3, "protection")?;
    if protection != PAGE_READWRITE
        || (allocation_type != MEM_COMMIT && allocation_type != (MEM_RESERVE | MEM_TOP_DOWN))
    {
        return Err(ApiFault::new(
            "VirtualAlloc currently supports read/write top-down reserve and commit inside a reservation",
        ));
    }
    if size == 0
        || (allocation_type == (MEM_RESERVE | MEM_TOP_DOWN) && preferred != 0)
        || (allocation_type == MEM_COMMIT
            && (preferred == 0 || !preferred.is_multiple_of(crate::memory::GUEST_PAGE_SIZE as u32)))
    {
        return virtual_memory_failure(Some(0), ERROR_INVALID_PARAMETER);
    }
    let page = crate::memory::GUEST_PAGE_SIZE as u32;
    let rounded_size = size.checked_add(page - 1).map(|value| value & !(page - 1));
    let Some(rounded_size) = rounded_size else {
        return virtual_memory_failure(Some(0), ERROR_INVALID_PARAMETER);
    };
    let page_protection = crate::memory::PageProtection {
        read: true,
        write: true,
        execute: false,
    };
    let result = if allocation_type == MEM_COMMIT {
        runtime.commit_virtual_memory(GuestAddress(preferred), size, page_protection)
    } else {
        runtime.reserve_virtual_memory(rounded_size)
    };
    match result {
        Ok(Ok(address)) => Ok(ApiImmediate::new(Some(address.0))),
        Ok(Err(last_error)) => virtual_memory_failure(Some(0), last_error),
        Err(error) => Err(ApiFault::from_runtime(error, endpoint)),
    }
}

fn handle_virtual_free_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let address = required_argument(endpoint, frame, 0, "decommit address")?;
    let size = required_argument(endpoint, frame, 1, "decommit size")?;
    let free_type = required_argument(endpoint, frame, 2, "free type")?;
    if free_type != MEM_DECOMMIT {
        return Err(ApiFault::new(
            "VirtualFree currently supports nonempty decommit inside a reservation",
        ));
    }
    if address == 0 || size == 0 || !address.is_multiple_of(crate::memory::GUEST_PAGE_SIZE as u32) {
        return virtual_memory_failure(Some(u32::from(false)), ERROR_INVALID_PARAMETER);
    }
    match runtime.decommit_virtual_memory(GuestAddress(address), size) {
        Ok(Ok(())) => Ok(ApiImmediate::new(Some(u32::from(true)))),
        Ok(Err(last_error)) => virtual_memory_failure(Some(u32::from(false)), last_error),
        Err(error) => Err(ApiFault::from_runtime(error, endpoint)),
    }
}

fn virtual_memory_failure(value: Option<u32>, last_error: u32) -> Result<ApiImmediate, ApiFault> {
    Ok(ApiImmediate::new(value).with_last_error(last_error))
}

fn handle_set_thread_affinity_mask_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let handle = RuntimeHandle(required_argument(endpoint, frame, 0, "thread handle")?);
    let requested_mask = required_argument(endpoint, frame, 1, "thread affinity mask")?;
    match runtime
        .set_thread_affinity_mask(handle, requested_mask)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
    {
        Ok(previous_mask) => Ok(ApiImmediate::new(Some(previous_mask))),
        Err(last_error) => Ok(ApiImmediate::new(Some(0)).with_last_error(last_error)),
    }
}

fn set_thread_priority_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let thread_handle = required_argument(endpoint, frame, 0, "thread handle")?;
    let priority_value =
        i32::from_le_bytes(required_argument(endpoint, frame, 1, "thread priority")?.to_le_bytes());
    let Some(priority) = RuntimeThreadPriority::from_win32_value(priority_value) else {
        return Ok(
            ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_INVALID_PARAMETER)
        );
    };
    let success = runtime
        .set_thread_priority(RuntimeHandle(thread_handle), priority)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let immediate = ApiImmediate::new(Some(u32::from(success)));
    Ok(if success {
        immediate
    } else {
        immediate.with_last_error(ERROR_INVALID_HANDLE)
    })
}

fn handle_create_thread_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let stack = runtime
        .thread_stack(runtime.current_thread_id())
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let stack_reserve_bytes = stack.reserved_range.byte_length();
    let stack_commit_bytes = stack.initial_commit_bytes;
    const CREATE_SUSPENDED: u32 = 0x0000_0004;
    let security_attributes = required_argument(endpoint, frame, 0, "security attributes")?;
    let stack_size = required_argument(endpoint, frame, 1, "stack size")?;
    let start_address = required_argument(endpoint, frame, 2, "start address")?;
    let parameter = required_argument(endpoint, frame, 3, "thread parameter")?;
    let creation_flags = required_argument(endpoint, frame, 4, "creation flags")?;
    let thread_id_output = required_argument(endpoint, frame, 5, "thread ID output")?;
    let failed = |last_error| ApiImmediate::new(Some(0)).with_last_error(last_error);

    if thread_id_output != 0 {
        runtime
            .memory
            .validate_access(
                GuestAddress(thread_id_output),
                size_of::<u32>(),
                MemoryAccess::Write,
            )
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    }

    if security_attributes != 0
        || stack_size != 0
        || start_address == 0
        || !matches!(creation_flags, 0 | CREATE_SUSPENDED)
    {
        return Ok(failed(ERROR_INVALID_PARAMETER));
    }
    let (thread_id, handle) = match runtime.create_guest_thread(RuntimeCreateThreadRequest {
        stack_reserve_bytes,
        stack_commit_bytes,
        entry_point: GuestAddress(start_address),
        parameter,
        thread_id_output: (thread_id_output != 0).then_some(GuestAddress(thread_id_output)),
        initial_suspend_count: u32::from(creation_flags & CREATE_SUSPENDED != 0),
    }) {
        Ok(Some(result)) => result,
        Ok(None) => {
            return Ok(failed(ERROR_NOT_ENOUGH_MEMORY));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, endpoint)),
    };
    let mut immediate = ApiImmediate::new(Some(handle.0));
    immediate.created_thread = Some(thread_id);
    Ok(immediate)
}

fn handle_resume_thread(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let handle = RuntimeHandle(required_argument(endpoint, frame, 0, "thread handle")?);
    let previous_count = runtime
        .resume_thread(handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let immediate = ApiImmediate::new(Some(previous_count));
    Ok(if previous_count == u32::MAX {
        immediate.with_last_error(ERROR_INVALID_HANDLE)
    } else {
        immediate
    })
}

fn handle_get_system_info_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
) -> Result<ApiImmediate, ApiFault> {
    let output = GuestAddress(required_argument(endpoint, frame, 0, "SYSTEM_INFO output")?);
    let processor = vine_cpu_profile::WINDOWS_PROCESSOR;
    let mut info = Vec::with_capacity(36);
    info.extend_from_slice(&processor.architecture.to_le_bytes());
    info.extend_from_slice(&0_u16.to_le_bytes()); // reserved
    info.extend_from_slice(&4096_u32.to_le_bytes());
    info.extend_from_slice(&0x0001_0000_u32.to_le_bytes());
    info.extend_from_slice(&0x7ffe_ffff_u32.to_le_bytes());
    info.extend_from_slice(&processor.affinity_mask.to_le_bytes());
    info.extend_from_slice(&processor.logical_processors.to_le_bytes());
    info.extend_from_slice(&processor.processor_type.to_le_bytes());
    info.extend_from_slice(&65536_u32.to_le_bytes());
    info.extend_from_slice(&processor.level.to_le_bytes());
    info.extend_from_slice(&processor.revision.to_le_bytes());
    Ok(ApiImmediate::new(None).with_guest_write(output, info))
}

fn handle_global_memory_status_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    memory: &GuestMemory,
) -> Result<ApiImmediate, ApiFault> {
    let output = GuestAddress(required_argument(
        endpoint,
        frame,
        0,
        "MEMORYSTATUS output",
    )?);
    let limits = memory.limits();
    let stats = memory.stats();
    // The physical figures describe this process's bounded guest-memory budget,
    // not the browser host's memory or the v86 execution mirror.
    let total_physical = (limits.max_physical_pages as u64)
        .saturating_mul(4096)
        .min(u64::from(u32::MAX)) as u32;
    let available_physical = (limits
        .max_physical_pages
        .saturating_sub(stats.physical_pages) as u64)
        .saturating_mul(4096)
        .min(u64::from(total_physical)) as u32;
    // Match the x86 application span reported by GetSystemInfo. Reserved guest
    // pages cannot be allocated a second time even when not committed.
    const TOTAL_VIRTUAL: u32 = 0x7ffe_0000;
    let available_virtual = TOTAL_VIRTUAL.saturating_sub(
        (stats.reserved_pages as u64)
            .saturating_mul(4096)
            .min(u64::from(TOTAL_VIRTUAL)) as u32,
    );
    let memory_load = if total_physical == 0 {
        100
    } else {
        ((u64::from(total_physical - available_physical) * 100) / u64::from(total_physical)) as u32
    };
    let mut bytes = Vec::with_capacity(32);
    for word in [
        32,
        memory_load,
        total_physical,
        available_physical,
        total_physical,
        available_physical,
        TOTAL_VIRTUAL,
        available_virtual,
    ] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    Ok(ApiImmediate::new(None).with_guest_write(output, bytes))
}

fn handle_get_version_ex_a_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    memory: &GuestMemory,
) -> Result<ApiImmediate, ApiFault> {
    let output = GuestAddress(required_argument(
        endpoint,
        frame,
        0,
        "OSVERSIONINFOA output",
    )?);
    let declared_size_bytes = memory
        .read(output, size_of::<u32>())
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let declared_size = u32::from_le_bytes(
        declared_size_bytes
            .try_into()
            .expect("checked OSVERSIONINFOA size field has DWORD width"),
    );
    if !matches!(
        declared_size,
        X86_OS_VERSION_INFO_A_BASE_BYTES | X86_OS_VERSION_INFO_EX_A_BYTES
    ) {
        return Ok(
            ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_INSUFFICIENT_BUFFER)
        );
    }

    let mut bytes = memory
        .read(output, declared_size as usize)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    merge_os_version_info_a(&mut bytes);
    Ok(ApiImmediate::new(Some(u32::from(true))).with_guest_write(output, bytes))
}

fn merge_os_version_info_a(bytes: &mut [u8]) {
    // Fixed NT 10.0 / build 19045 workstation, independent of the browser host.
    bytes[4..8].copy_from_slice(&10_u32.to_le_bytes());
    bytes[8..12].copy_from_slice(&0_u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&19045_u32.to_le_bytes());
    bytes[16..20].copy_from_slice(&2_u32.to_le_bytes());
    bytes[20] = 0; // Empty service-pack string; preserve its unused tail.
    if bytes.len() == X86_OS_VERSION_INFO_EX_A_BYTES as usize {
        bytes[148..154].fill(0);
        bytes[154] = 1; // VER_NT_WORKSTATION
        bytes[155] = 0;
    }
}

fn handle_get_last_error(
    endpoint: &ApiEndpointIdentity,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    Ok(ApiImmediate::new(Some(
        runtime
            .current_last_error()
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?,
    )))
}

fn handle_set_last_error(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
) -> Result<ApiImmediate, ApiFault> {
    let error = required_argument(endpoint, frame, 0, "LastError value")?;
    Ok(ApiImmediate::new(None).with_last_error(error))
}

fn read_nul_terminated_drive_root_path(
    endpoint: &ApiEndpointIdentity,
    memory: &GuestMemory,
    source: GuestAddress,
    maximum: usize,
) -> Result<Vec<u8>, ApiFault> {
    let mut bytes = Vec::new();
    for offset in 0..maximum {
        let address = add_guest_offset(endpoint, source, offset)?;
        let byte = memory
            .read(address, 1)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?[0];
        bytes.push(byte);
        if byte == 0 {
            return Ok(bytes);
        }
    }
    Err(ApiFault::new(format!(
        "ANSI drive-root path exceeds the {} byte DOS path limit",
        maximum
    )))
}

fn handle_get_drive_type_a_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let address = required_argument(endpoint, frame, 0, "ANSI root path")?;
    let root = if address == 0 {
        None
    } else {
        Some(read_nul_terminated_drive_root_path(
            endpoint,
            &runtime.memory,
            GuestAddress(address),
            260,
        )?)
    };
    // Only the process image's mounted drive exists. Do not claim unmounted guest drives.
    let drive_type = if runtime.is_process_image_drive_root_a(root.as_deref()) {
        3 // DRIVE_FIXED
    } else {
        1 // DRIVE_NO_ROOT_DIR
    };
    Ok(ApiImmediate::new(Some(drive_type)))
}

fn handle_get_cp_info_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
) -> Result<ApiImmediate, ApiFault> {
    let code_page = required_argument(endpoint, frame, 0, "code page")?;
    let output = GuestAddress(required_argument(endpoint, frame, 1, "CPINFO output")?);
    if code_page != CP_ACP && code_page != WINDOWS_932_CODE_PAGE {
        return Err(ApiFault::new(format!(
            "GetCPInfo does not implement code page {code_page}"
        )));
    }
    Ok(ApiImmediate::new(Some(u32::from(true))).with_guest_write(
        output,
        vec![
            2, 0, 0, 0, b'?', 0, 0x81, 0x9f, 0xe0, 0xfc, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
    ))
}

fn handle_is_processor_feature_present_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
) -> Result<ApiImmediate, ApiFault> {
    let index = required_argument(endpoint, frame, 0, "processor feature index")?;
    // Preserve the existing HAL answers independently of backend instruction
    // coverage. The shared profile also records the guest CPUID capabilities.
    Ok(ApiImmediate::new(Some(u32::from(
        vine_cpu_profile::windows_processor_feature_present(index),
    ))))
}

fn handle_set_unhandled_exception_filter(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let filter_value = required_argument(endpoint, frame, 0, "unhandled exception filter")?;
    // Registration only records the filter; Windows validates nothing until an unhandled
    // exception actually dispatches to it, so an address that is not executable yet is legal.
    let filter = (filter_value != 0).then_some(GuestAddress(filter_value));
    let previous = runtime
        .set_unhandled_exception_filter(filter)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(
        previous.map_or(0, |address| address.0),
    )))
}

fn handle_get_tick_count(
    endpoint: &ApiEndpointIdentity,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let tick_count = runtime
        .sample_tick_count()
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(tick_count)))
}

fn handle_get_std_handle_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let selector = required_argument(endpoint, frame, 0, "standard handle selector")?;
    let handle = runtime
        .ensure_standard_handle(selector)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    if handle == u32::MAX {
        return Ok(ApiImmediate::new(Some(handle)).with_last_error(ERROR_INVALID_PARAMETER));
    }
    Ok(ApiImmediate::new(Some(handle)))
}

fn handle_get_file_type_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let handle = required_argument(endpoint, frame, 0, "file handle")?;
    let Some(file_type) = runtime
        .file_type(handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
    else {
        return Ok(ApiImmediate::new(Some(0)).with_last_error(ERROR_INVALID_HANDLE));
    };
    Ok(ApiImmediate::new(Some(file_type)))
}

fn handle_set_handle_count_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
) -> Result<ApiImmediate, ApiFault> {
    Ok(ApiImmediate::new(Some(required_argument(
        endpoint,
        frame,
        0,
        "handle count",
    )?)))
}

fn handle_get_command_line_a(
    endpoint: &ApiEndpointIdentity,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let command_line = runtime
        .process_command_line()
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let bytes = runtime
        .memory
        .read(
            command_line.ansi_buffer_address,
            usize::try_from(command_line.ansi_byte_length_with_nul).map_err(|_| {
                ApiFault::new("runtime ANSI command-line length does not fit host accounting")
            })?,
        )
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    if bytes.last() != Some(&0) || bytes[..bytes.len().saturating_sub(1)].contains(&0) {
        return Err(ApiFault::new(
            "the process-owned ANSI command-line view no longer has its bounded NUL-terminated shape",
        ));
    }
    Ok(ApiImmediate::new(Some(command_line.ansi_buffer_address.0)))
}

fn handle_get_module_file_name_a_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    const MAXIMUM_CAPACITY: u32 = 32768;
    let module_handle = required_argument(endpoint, frame, 0, "module handle")?;
    let output_address = required_argument(endpoint, frame, 1, "ANSI module file-name output")?;
    let capacity = required_argument(endpoint, frame, 2, "ANSI module file-name capacity")?;
    if module_handle != 0 {
        return Err(ApiFault::new(
            "GetModuleFileNameA currently supports only the current process image",
        ));
    }
    if capacity > MAXIMUM_CAPACITY {
        return Err(ApiFault::new(format!(
            "ANSI module file-name capacity {capacity} exceeds the limit of {MAXIMUM_CAPACITY} bytes"
        )));
    }
    if capacity == 0 {
        return Ok(ApiImmediate::new(Some(0)).with_last_error(ERROR_INSUFFICIENT_BUFFER));
    }
    if output_address == 0 {
        return Ok(ApiImmediate::new(Some(0)).with_last_error(ERROR_INVALID_PARAMETER));
    }

    let image_path = runtime.process_image_path_a();
    let capacity = usize::try_from(capacity).map_err(|_| {
        ApiFault::new("validated ANSI module file-name capacity does not fit host accounting")
    })?;
    let output = GuestAddress(output_address);
    runtime
        .memory
        .validate_access(output, capacity, MemoryAccess::Write)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let path_length = image_path.bytes_with_nul.len() - 1;
    if capacity <= path_length {
        let mut truncated = image_path.bytes_with_nul[..capacity].to_vec();
        *truncated
            .last_mut()
            .expect("positive GetModuleFileNameA capacity") = 0;
        return Ok(ApiImmediate::new(Some(capacity as u32))
            .with_last_error(ERROR_INSUFFICIENT_BUFFER)
            .with_guest_write(output, truncated));
    }
    Ok(ApiImmediate::new(Some(path_length as u32))
        .with_last_error(ERROR_SUCCESS)
        .with_guest_write(output, image_path.bytes_with_nul.clone()))
}

fn handle_get_environment_strings_w(
    endpoint: &ApiEndpointIdentity,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let environment = runtime
        .process_environment_strings()
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
        .clone();
    let byte_length = usize::try_from(environment.byte_length_with_double_nul).map_err(|_| {
        ApiFault::new("runtime wide environment length does not fit host accounting")
    })?;
    let authority = runtime
        .memory
        .read(environment.authority_buffer_address, byte_length)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let copy = runtime
        .memory
        .read(environment.copy_buffer_address, byte_length)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    if authority != [0, 0, 0, 0] || copy != authority {
        return Err(ApiFault::new(
            "the controlled wide environment authority or copy no longer has its empty double-NUL shape",
        ));
    }
    let copy = runtime
        .acquire_environment_strings_w()
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(copy.0)))
}

fn handle_free_environment_strings_w(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let buffer_address = GuestAddress(required_argument(
        endpoint,
        frame,
        0,
        "wide environment-string copy",
    )?);
    runtime
        .free_environment_strings_w(buffer_address)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(u32::from(true))))
}

#[derive(Debug)]
struct Windows932Unit {
    bytes: Vec<u8>,
    used_default: bool,
}

fn handle_wide_char_to_multi_byte_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    memory: &GuestMemory,
) -> Result<ApiImmediate, ApiFault> {
    const MAXIMUM_SOURCE_CODE_UNITS: usize = 65_536;
    let code_page = required_argument(endpoint, frame, 0, "code page")?;
    let flags = required_argument(endpoint, frame, 1, "conversion flags")?;
    let source_address = required_argument(endpoint, frame, 2, "UTF-16 source")?;
    let source_count = required_argument(endpoint, frame, 3, "UTF-16 source length")? as i32;
    let destination_address = required_argument(endpoint, frame, 4, "byte destination")?;
    let destination_bytes = required_argument(endpoint, frame, 5, "byte destination length")?;
    let default_char = required_argument(endpoint, frame, 6, "default character")?;
    let used_default_char = required_argument(endpoint, frame, 7, "used-default output")?;
    let effective_code_page = if code_page == CP_ACP {
        WINDOWS_932_CODE_PAGE
    } else {
        code_page
    };
    if effective_code_page != WINDOWS_932_CODE_PAGE || flags != 0 {
        return Err(ApiFault::new(format!(
            "Windows-932 conversion does not support code page {code_page} and flags {flags:#010x}"
        )));
    }
    if default_char != 0 {
        return Err(ApiFault::new(
            "the Windows-932 handler currently uses only the system default character",
        ));
    }

    if source_address == 0 || source_count == 0 || source_count < -1 {
        return Ok(wide_char_failure(ERROR_INVALID_PARAMETER));
    }
    let query = destination_address == 0 && destination_bytes == 0;
    let conversion_call = destination_address != 0 && destination_bytes != 0;
    if !query && !conversion_call {
        return Ok(wide_char_failure(ERROR_INVALID_PARAMETER));
    }
    if used_default_char != 0 {
        memory
            .validate_access(GuestAddress(used_default_char), 4, MemoryAccess::Write)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    }

    let source_units = read_wide_char_source(
        endpoint,
        memory,
        GuestAddress(source_address),
        source_count,
        MAXIMUM_SOURCE_CODE_UNITS,
    )?;
    let source_byte_length = source_units
        .len()
        .checked_mul(2)
        .ok_or_else(|| ApiFault::new("UTF-16 source length overflowed host accounting"))?;
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
        length
            .checked_add(unit.bytes.len())
            .ok_or_else(|| ApiFault::new("Windows-932 output length overflowed host accounting"))
    })?;
    let encoded_length_u32 = u32::try_from(encoded_length).map_err(|_| {
        ApiFault::new("Windows-932 output length does not fit the x86 return value")
    })?;
    let used_default = encoded.iter().any(|unit| unit.used_default);

    if query {
        let mut immediate = ApiImmediate::new(Some(encoded_length_u32));
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
            return Err(ApiFault::new(
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
        memory
            .validate_access(
                GuestAddress(destination_address),
                output.len(),
                MemoryAccess::Write,
            )
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    }
    let complete = output.len() == encoded_length;
    let mut immediate = ApiImmediate::new(Some(if complete { encoded_length_u32 } else { 0 }));
    if !complete {
        immediate = immediate.with_last_error(ERROR_INSUFFICIENT_BUFFER);
    }
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
    endpoint: &ApiEndpointIdentity,
    memory: &GuestMemory,
    source: GuestAddress,
    source_count: i32,
    maximum: usize,
) -> Result<Vec<u16>, ApiFault> {
    let unit_count = if source_count == -1 {
        let mut units = Vec::new();
        for offset in 0..maximum {
            let address = add_guest_offset(
                endpoint,
                source,
                offset.checked_mul(2).ok_or_else(|| {
                    ApiFault::new("UTF-16 source offset overflowed host accounting")
                })?,
            )?;
            let bytes = memory
                .read(address, 2)
                .map_err(|error| ApiFault::from_memory(error, endpoint))?;
            let unit = u16::from_le_bytes([bytes[0], bytes[1]]);
            units.push(unit);
            if unit == 0 {
                return Ok(units);
            }
        }
        return Err(ApiFault::new(format!(
            "UTF-16 source did not terminate within the limit of {} code units",
            maximum
        )));
    } else {
        usize::try_from(source_count).map_err(|_| {
            ApiFault::new("validated positive UTF-16 source count does not fit host accounting")
        })?
    };
    if unit_count > maximum {
        return Err(ApiFault::new(format!(
            "UTF-16 source count {unit_count} exceeds the limit of {}",
            maximum
        )));
    }
    let byte_length = unit_count
        .checked_mul(2)
        .ok_or_else(|| ApiFault::new("UTF-16 source length overflowed host accounting"))?;
    memory
        .validate_access(source, byte_length, MemoryAccess::Read)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let bytes = memory
        .read(source, byte_length)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
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
                return Err(ApiFault::new(
                    "Windows-932 encoder exceeded its bounded per-unit output buffer",
                ));
            }
            _ => {
                return Err(ApiFault::new(
                    "Windows-932 encoder returned an invalid per-unit progress result",
                ));
            }
        }
    }
    Ok(output)
}

fn handle_multi_byte_to_wide_char_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    memory: &GuestMemory,
) -> Result<ApiImmediate, ApiFault> {
    const MAXIMUM_SOURCE_BYTES: usize = 65_536;
    let code_page = required_argument(endpoint, frame, 0, "code page")?;
    let flags = required_argument(endpoint, frame, 1, "conversion flags")?;
    let source_address = required_argument(endpoint, frame, 2, "multibyte source")?;
    let source_count = required_argument(endpoint, frame, 3, "multibyte source length")? as i32;
    let destination_address = required_argument(endpoint, frame, 4, "UTF-16 destination")?;
    let destination_units =
        required_argument(endpoint, frame, 5, "UTF-16 destination length")? as i32;
    let effective_code_page = if code_page == CP_ACP {
        WINDOWS_932_CODE_PAGE
    } else {
        code_page
    };
    if effective_code_page != WINDOWS_932_CODE_PAGE || !matches!(flags, 0 | 1) {
        return Err(ApiFault::new(format!(
            "Windows-932 conversion does not support code page {code_page} and flags {flags:#010x}"
        )));
    }

    if source_address == 0 || source_count == 0 || source_count < -1 || destination_units < 0 {
        return Ok(multi_byte_failure(ERROR_INVALID_PARAMETER));
    }
    let query = destination_address == 0 && destination_units == 0;
    let conversion_call = destination_address != 0 && destination_units > 0;
    if !query && !conversion_call {
        return Ok(multi_byte_failure(ERROR_INVALID_PARAMETER));
    }

    let source = read_multi_byte_source(
        endpoint,
        memory,
        GuestAddress(source_address),
        source_count,
        MAXIMUM_SOURCE_BYTES,
    )?;
    let destination_byte_capacity = usize::try_from(destination_units)
        .map_err(|_| {
            ApiFault::new(
                "validated nonnegative UTF-16 destination count does not fit host accounting",
            )
        })?
        .checked_mul(2)
        .ok_or_else(|| ApiFault::new("UTF-16 destination length overflowed host accounting"))?;
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
        ApiFault::new("Windows-932 UTF-16 output length does not fit the x86 return value")
    })?;
    if query {
        return Ok(ApiImmediate::new(Some(decoded_length)));
    }

    let capacity = usize::try_from(destination_units).map_err(|_| {
        ApiFault::new("validated positive UTF-16 destination count does not fit host accounting")
    })?;
    let prefix = &decoded[..decoded.len().min(capacity)];
    let mut output = Vec::with_capacity(prefix.len() * 2);
    for &unit in prefix {
        output.extend_from_slice(&unit.to_le_bytes());
    }
    if !output.is_empty() {
        memory
            .validate_access(
                GuestAddress(destination_address),
                output.len(),
                MemoryAccess::Write,
            )
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    }
    let complete = prefix.len() == decoded.len();
    let mut immediate = ApiImmediate::new(Some(if complete { decoded_length } else { 0 }));
    if !complete {
        immediate = immediate.with_last_error(ERROR_INSUFFICIENT_BUFFER);
    }
    if !output.is_empty() {
        immediate = immediate.with_guest_write(GuestAddress(destination_address), output);
    }
    Ok(immediate)
}

fn handle_set_current_directory_a_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let address = required_argument(endpoint, frame, 0, "ANSI directory path")?;
    if address == 0 {
        return Ok(
            ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_INVALID_PARAMETER)
        );
    }
    let path =
        read_nul_terminated_ansi_path(endpoint, &runtime.memory, GuestAddress(address), 32768)?;
    if path == [0] {
        return Ok(ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_INVALID_NAME));
    }
    runtime
        .set_current_directory_a(&path)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(u32::from(true))))
}

fn read_nul_terminated_ansi_path(
    endpoint: &ApiEndpointIdentity,
    memory: &GuestMemory,
    source: GuestAddress,
    maximum: usize,
) -> Result<Vec<u8>, ApiFault> {
    let mut bytes = Vec::new();
    for offset in 0..maximum {
        let address = add_guest_offset(endpoint, source, offset)?;
        let byte = memory
            .read(address, 1)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?[0];
        bytes.push(byte);
        if byte == 0 {
            return Ok(bytes);
        }
    }
    Err(ApiFault::new(format!(
        "ANSI directory path did not terminate within the limit of {} bytes",
        maximum
    )))
}

// Browser reports only what OPFS observed. Windows results belong to the
// guest filesystem boundary, including the immutable-source parent fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpfsFileOperation {
    Attributes { source_missing: u8 },
    CreateDirectory,
    DeleteFile { source_missing: u8 },
}

pub fn complete_opfs_file_operation(
    operation: OpfsFileOperation,
    observation: u32,
    previous_last_error: u32,
) -> Result<ApiImmediate, ApiFault> {
    let invalid = || ApiFault::new("browser returned an invalid OPFS result");
    let (value, last_error) = match operation {
        OpfsFileOperation::Attributes { source_missing } => {
            if !matches!(source_missing, 2 | 3) {
                return Err(invalid());
            }
            let (attributes, error) = match observation {
                1 => (0x20, previous_last_error),
                2 => (0x30, previous_last_error),
                3 => (u32::MAX, ERROR_FILE_NOT_FOUND),
                4 => (u32::MAX, u32::from(source_missing)),
                5 => (u32::MAX, ERROR_ACCESS_DENIED),
                _ => return Err(invalid()),
            };
            return Ok(ApiImmediate::new(Some(attributes)).with_last_error(error));
        }
        OpfsFileOperation::CreateDirectory => match observation {
            0 => (true, previous_last_error),
            1 => (false, ERROR_ALREADY_EXISTS),
            2 => (false, ERROR_PATH_NOT_FOUND),
            3 => (false, ERROR_ACCESS_DENIED),
            4 => (false, ERROR_DISK_FULL),
            _ => return Err(invalid()),
        },
        OpfsFileOperation::DeleteFile { source_missing } => {
            if !matches!(source_missing, 2 | 3) {
                return Err(invalid());
            }
            match observation {
                0 => (true, previous_last_error),
                1 => (false, ERROR_FILE_NOT_FOUND),
                2 => (false, u32::from(source_missing)),
                3 => (false, ERROR_ACCESS_DENIED),
                4 => (false, ERROR_DISK_FULL),
                _ => return Err(invalid()),
            }
        }
    };
    Ok(ApiImmediate::new(Some(u32::from(value))).with_last_error(last_error))
}

// A file handle is published only after the browser has finished the OPFS
// operation and the suspended ABI frame has accepted its return value.
pub fn complete_kernel32_opfs_open(
    dispatcher: &ApiDispatcher,
    call: &ApiDeferredCall,
    observation: u32,
    byte_length: u64,
    runtime: &mut crate::runtime::Runtime,
) -> DispatchOutcome {
    let request_id = call.request.request_id;
    if call.request.service != "browser.opfs-open"
        || !call
            .frame
            .endpoint
            .module
            .eq_ignore_ascii_case("kernel32.dll")
        || !matches!(&call.frame.endpoint.symbol, ApiSymbol::Name { name } if name == "CreateFileA")
    {
        return DispatchOutcome::Fault(ApiFault::new(
            "OPFS file open does not identify CreateFileA",
        ));
    }
    let (handle, disposition, source_missing) =
        match runtime.preflight_opfs_file_open_completion(request_id, &call.request.payload) {
            Ok(prepared) => prepared,
            Err(error) => {
                return DispatchOutcome::Fault(ApiFault::from_runtime(error, &call.frame.endpoint));
            }
        };
    let invalid = || ApiFault::new("browser returned an invalid OPFS open observation");
    if byte_length > 9_007_199_254_740_991
        || (observation != 1 && byte_length != 0)
        || (observation == 1 && disposition == 2 && byte_length != 0)
        || (observation == 0 && disposition == 3)
        || observation > 6
    {
        return DispatchOutcome::Fault(invalid());
    }
    let (opened, last_error) = match observation {
        0 => (Some(0), Some(ERROR_SUCCESS)),
        1 => (
            Some(byte_length),
            if disposition == 3 {
                None
            } else {
                Some(ERROR_ALREADY_EXISTS)
            },
        ),
        2 => (None, Some(ERROR_FILE_NOT_FOUND)),
        3 => (None, Some(u32::from(source_missing))),
        4 | 6 => (None, Some(ERROR_ACCESS_DENIED)),
        5 => (None, Some(ERROR_DISK_FULL)),
        _ => unreachable!("validated OPFS observation"),
    };
    let mut immediate = ApiImmediate::new(Some(opened.map_or(INVALID_HANDLE_VALUE, |_| handle.0)));
    if let Some(error) = last_error {
        immediate = immediate.with_last_error(error);
    }
    let outcome = dispatcher.complete_deferred(
        call,
        HostCompletion {
            request_id,
            result: immediate,
        },
        runtime,
    );
    if matches!(outcome, DispatchOutcome::Immediate(_)) {
        runtime.commit_opfs_file_open(request_id, opened);
    }
    outcome
}

fn handle_get_file_attributes_a_opfs(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let result = (|| {
        let known = handle_get_file_attributes_a_basic(endpoint, frame, runtime)?;
        if known.eax != Some(u32::MAX) {
            return Ok(ApiHandlerResult::Immediate(known));
        }
        let address = required_argument(endpoint, frame, 0, "ANSI file path")?;
        let path =
            read_nul_terminated_source_path(endpoint, &runtime.memory, GuestAddress(address), 260)?;
        let relative = match runtime.opfs_relative_path(&path) {
            Ok(relative) => relative,
            Err(_) => return Ok(ApiHandlerResult::Immediate(known)),
        };
        // The source mount may contain the parent even when OPFS does not.
        // Retain its Windows missing-file/path distinction across the host stat.
        let source_missing = known
            .last_error
            .ok_or_else(|| ApiFault::new("missing source path has no Windows error"))?;
        if !matches!(source_missing, ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND) {
            return Err(ApiFault::new(
                "source lookup returned an unexpected Windows error",
            ));
        }
        let request = runtime
            .begin_host_request("browser.opfs-stat")
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        let mut payload = vec![source_missing as u8];
        payload.extend_from_slice(relative.as_bytes());
        Ok(ApiHandlerResult::Deferred(crate::api::HostRequest {
            request_id: request.request_id,
            service: request.service,
            payload,
        }))
    })();
    match result {
        Ok(outcome) => outcome,
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn handle_get_file_attributes_a_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let path_address = required_argument(endpoint, frame, 0, "ANSI file path")?;
    let path = read_nul_terminated_source_path(
        endpoint,
        &runtime.memory,
        GuestAddress(path_address),
        260,
    )?;
    let resolution = runtime
        .resolve_file_system_path_a(&path)
        .map_err(|error| ApiFault::new(error.message))?;
    Ok(match resolution {
        RuntimePathResolution::RegularFile => ApiImmediate::new(Some(0x20)),
        RuntimePathResolution::Directory => ApiImmediate::new(Some(0x30)),
        RuntimePathResolution::MissingFile => {
            ApiImmediate::new(Some(u32::MAX)).with_last_error(ERROR_FILE_NOT_FOUND)
        }
        RuntimePathResolution::MissingPath => {
            ApiImmediate::new(Some(u32::MAX)).with_last_error(ERROR_PATH_NOT_FOUND)
        }
    })
}

fn handle_delete_file_a_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let result = (|| {
        let address = required_argument(endpoint, frame, 0, "ANSI file path")?;
        if address == 0 {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_PATH_NOT_FOUND),
            ));
        }
        let path =
            read_nul_terminated_source_path(endpoint, &runtime.memory, GuestAddress(address), 260)?;
        let (relative, source_missing) = match runtime.opfs_delete_path(&path) {
            Ok(checked) => checked,
            Err(error) => {
                return Ok(ApiHandlerResult::Immediate(
                    ApiImmediate::new(Some(u32::from(false))).with_last_error(error),
                ));
            }
        };
        let request = runtime
            .begin_host_request("browser.opfs-delete-file")
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        Ok(ApiHandlerResult::Deferred(crate::api::HostRequest {
            request_id: request.request_id,
            service: request.service,
            payload: std::iter::once(source_missing)
                .chain(relative.into_bytes())
                .collect(),
        }))
    })();
    match result {
        Ok(outcome) => outcome,
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

// The selected-executable path uses the Rust VFS search directly. A search only
// enumerates paths already registered with the source mount; browser discovery
// remains a separate, read-only operation.
fn handle_find_first_file_a_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let address = required_argument(endpoint, frame, 0, "ANSI search pattern")?;
    if address == 0 {
        return Ok(
            ApiImmediate::new(Some(INVALID_HANDLE_VALUE)).with_last_error(ERROR_PATH_NOT_FOUND)
        );
    }
    let output = GuestAddress(required_argument(endpoint, frame, 1, "find data output")?);
    let pattern =
        read_nul_terminated_source_path(endpoint, &runtime.memory, GuestAddress(address), 260)?;
    match runtime.find_first_file_a(&pattern, output) {
        Ok(RuntimeFindFirstFileOutcome::Found(handle)) => Ok(ApiImmediate::new(Some(handle.0))),
        Ok(RuntimeFindFirstFileOutcome::NotFound(windows_error)) => {
            Ok(ApiImmediate::new(Some(INVALID_HANDLE_VALUE)).with_last_error(windows_error))
        }
        Err(error) => Err(ApiFault::from_runtime(error, endpoint)),
    }
}

fn handle_find_next_file_a_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let handle = RuntimeHandle(required_argument(endpoint, frame, 0, "file search handle")?);
    if !runtime
        .is_live_object_handle(handle, RuntimeObjectKind::FileSearch)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
    {
        return Ok(ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_INVALID_HANDLE));
    }
    let output = GuestAddress(required_argument(endpoint, frame, 1, "find data output")?);
    match runtime.find_next_file_a(handle, output) {
        Ok(true) => Ok(ApiImmediate::new(Some(u32::from(true)))),
        Ok(false) => {
            Ok(ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_NO_MORE_FILES))
        }
        Err(error) => Err(ApiFault::from_runtime(error, endpoint)),
    }
}

fn handle_find_close_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let handle = RuntimeHandle(required_argument(endpoint, frame, 0, "file search handle")?);
    if !runtime
        .is_live_object_handle(handle, RuntimeObjectKind::FileSearch)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
    {
        return Ok(ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_INVALID_HANDLE));
    }
    runtime
        .find_close(handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(u32::from(true))))
}

fn handle_create_directory_a_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let result = (|| {
        let address = required_argument(endpoint, frame, 0, "ANSI directory path")?;
        let security = required_argument(endpoint, frame, 1, "security attributes")?;
        if address == 0 {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_PATH_NOT_FOUND),
            ));
        }
        if security != 0 {
            return Err(ApiFault::new(
                "CreateDirectoryA supports NULL security attributes only",
            ));
        }
        let path =
            read_nul_terminated_source_path(endpoint, &runtime.memory, GuestAddress(address), 260)?;
        if matches!(
            runtime.resolve_file_system_path_a(&path),
            Ok(RuntimePathResolution::RegularFile | RuntimePathResolution::Directory)
        ) {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_ALREADY_EXISTS),
            ));
        }
        let (relative, source_parents) = match runtime.opfs_create_directory_path(&path) {
            Ok(checked) => checked,
            Err(error) => {
                return Ok(ApiHandlerResult::Immediate(
                    ApiImmediate::new(Some(u32::from(false))).with_last_error(error),
                ));
            }
        };
        let request = runtime
            .begin_host_request("browser.opfs-create-directory")
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        Ok(ApiHandlerResult::Deferred(crate::api::HostRequest {
            request_id: request.request_id,
            service: request.service,
            payload: std::iter::once(source_parents)
                .chain(relative.into_bytes())
                .collect(),
        }))
    })();
    match result {
        Ok(outcome) => outcome,
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn handle_create_file_a_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let result = (|| {
        let address = required_argument(endpoint, frame, 0, "ANSI file path")?;
        if address == 0 {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(Some(INVALID_HANDLE_VALUE))
                    .with_last_error(ERROR_INVALID_PARAMETER),
            ));
        }
        let access = required_argument(endpoint, frame, 1, "desired access")?;
        let sharing = required_argument(endpoint, frame, 2, "share mode")?;
        let security = required_argument(endpoint, frame, 3, "security attributes")?;
        let disposition = required_argument(endpoint, frame, 4, "creation disposition")?;
        let attributes = required_argument(endpoint, frame, 5, "file attributes")?;
        let template = required_argument(endpoint, frame, 6, "template file")?;
        let read_open = access == 0x8000_0000 && matches!(sharing, 0 | 1) && disposition == 3;
        let write_open = matches!(access, 0x4000_0000 | 0xc000_0000)
            && matches!(sharing, 0 | 1)
            && matches!(disposition, 2..=4);
        if security != 0
            || template != 0
            || attributes & !(0x0800_0000 | 0x80 | 0x20) != 0
            || !(read_open || write_open)
        {
            return handle_create_file_a_source_basic(endpoint, frame, runtime)
                .map(ApiHandlerResult::Immediate);
        }
        let path =
            read_nul_terminated_source_path(endpoint, &runtime.memory, GuestAddress(address), 260)?;
        if matches!(
            runtime.resolve_file_system_path_a(&path),
            Ok(RuntimePathResolution::RegularFile)
        ) && matches!(runtime.source_lookup_relative_path(&path), Ok(None))
        {
            return handle_create_file_a_source_basic(endpoint, frame, runtime)
                .map(ApiHandlerResult::Immediate);
        }
        let request = match runtime
            .begin_opfs_file_open(&path, access, sharing, disposition, attributes)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?
        {
            Ok(open) => open,
            Err(error) => {
                return Ok(ApiHandlerResult::Immediate(
                    ApiImmediate::new(Some(INVALID_HANDLE_VALUE)).with_last_error(error),
                ));
            }
        };
        Ok(ApiHandlerResult::Deferred(request))
    })();
    match result {
        Ok(outcome) => outcome,
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn handle_create_file_a_source_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let address = required_argument(endpoint, frame, 0, "ANSI file path")?;
    if address == 0 {
        return Ok(
            ApiImmediate::new(Some(INVALID_HANDLE_VALUE)).with_last_error(ERROR_INVALID_PARAMETER)
        );
    }
    let access = required_argument(endpoint, frame, 1, "desired access")?;
    let sharing = required_argument(endpoint, frame, 2, "share mode")?;
    let security = required_argument(endpoint, frame, 3, "security attributes")?;
    let disposition = required_argument(endpoint, frame, 4, "creation disposition")?;
    let attributes = required_argument(endpoint, frame, 5, "file attributes")?;
    let template = required_argument(endpoint, frame, 6, "template file")?;
    if access != 0x8000_0000
        || !matches!(sharing, 0 | 1)
        || security != 0
        || disposition != 3
        || attributes & !(0x0800_0000 | 0x80 | 0x20) != 0
        || template != 0
    {
        // An already registered immutable source file cannot be opened for
        // modification. Other unsupported variants must not claim success.
        if (access & 0x4000_0000 != 0 || disposition != 3)
            && let Ok(path) = read_nul_terminated_source_path(
                endpoint,
                &runtime.memory,
                GuestAddress(address),
                260,
            )
            && matches!(
                runtime.resolve_file_system_path_a(&path),
                Ok(RuntimePathResolution::RegularFile)
            )
            && matches!(runtime.source_lookup_relative_path(&path), Ok(None))
        {
            return Ok(
                ApiImmediate::new(Some(INVALID_HANDLE_VALUE)).with_last_error(ERROR_ACCESS_DENIED)
            );
        }
        return Err(ApiFault::new(
            "the requested file access mode requires writable overlay or unsupported flags",
        ));
    }
    let path =
        read_nul_terminated_source_path(endpoint, &runtime.memory, GuestAddress(address), 260)?;
    match runtime.open_file_a(&path, access, sharing, disposition, attributes) {
        Ok(Ok(Some(opened))) => Ok(ApiImmediate::new(Some(opened.0))),
        Ok(Ok(None)) => {
            Ok(ApiImmediate::new(Some(INVALID_HANDLE_VALUE)).with_last_error(ERROR_FILE_NOT_FOUND))
        }
        Ok(Err(win32_error)) => {
            Ok(ApiImmediate::new(Some(INVALID_HANDLE_VALUE)).with_last_error(win32_error))
        }
        Err(error) => Err(ApiFault::from_runtime(error, endpoint)),
    }
}

fn handle_get_file_size_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let handle = RuntimeHandle(required_argument(endpoint, frame, 0, "file handle")?);
    let size_high = required_argument(endpoint, frame, 1, "file size high output")?;
    if !runtime
        .is_live_object_handle(handle, RuntimeObjectKind::File)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
    {
        return Ok(ApiImmediate::new(Some(u32::MAX)).with_last_error(ERROR_INVALID_HANDLE));
    }
    let size = runtime
        .source_file_byte_length(handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let result = ApiImmediate::new(Some(size as u32));
    if size_high != 0 {
        commit_indirect_guest_writes(
            &mut runtime.memory,
            &[ApiGuestWrite {
                address: GuestAddress(size_high),
                bytes: ((size >> 32) as u32).to_le_bytes().to_vec(),
            }],
        )
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    }
    Ok(result)
}

fn handle_read_file_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let result = (|| {
        let handle = RuntimeHandle(required_argument(endpoint, frame, 0, "file handle")?);
        let buffer = GuestAddress(required_argument(endpoint, frame, 1, "read buffer")?);
        let requested = required_argument(endpoint, frame, 2, "requested bytes")?;
        let count = GuestAddress(required_argument(endpoint, frame, 3, "bytes read")?);
        let overlapped = required_argument(endpoint, frame, 4, "overlapped state")?;
        if !runtime
            .is_live_object_handle(handle, RuntimeObjectKind::File)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?
        {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_INVALID_HANDLE),
            ));
        }
        runtime
            .source_file_for_handle(handle)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        if count.0 == 0
            || (requested != 0 && buffer.0 == 0)
            || requested > RUNTIME_SOURCE_READ_MAX_BYTES
            || overlapped != 0
        {
            return Err(ApiFault::new(
                "ReadFile requires bounded synchronous file I/O",
            ));
        }
        if requested == 0 {
            commit_indirect_guest_writes(
                &mut runtime.memory,
                &[ApiGuestWrite {
                    address: count,
                    bytes: 0_u32.to_le_bytes().to_vec(),
                }],
            )
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;
            let result = ApiImmediate::new(Some(u32::from(true))).with_last_error(ERROR_SUCCESS);

            return Ok(ApiHandlerResult::Immediate(result));
        }
        runtime
            .memory
            .validate_access(buffer, requested as usize, MemoryAccess::Write)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        runtime
            .memory
            .validate_access(count, 4, MemoryAccess::Write)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        let request = runtime
            .begin_source_file_read(handle, buffer, count, requested)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        Ok(ApiHandlerResult::Deferred(request))
    })();
    match result {
        Ok(outcome) => outcome,
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn handle_write_file_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let result = (|| {
        let handle = RuntimeHandle(required_argument(endpoint, frame, 0, "file handle")?);
        if !runtime
            .is_live_object_handle(handle, RuntimeObjectKind::File)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?
        {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_INVALID_HANDLE),
            ));
        }
        let file = runtime
            .source_file_for_handle(handle)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        if matches!(file.backing, RuntimeFileBacking::ImmutableSource { .. }) {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_ACCESS_DENIED),
            ));
        }
        let buffer = GuestAddress(required_argument(endpoint, frame, 1, "write buffer")?);
        let requested = required_argument(endpoint, frame, 2, "write length")?;
        let count = GuestAddress(required_argument(endpoint, frame, 3, "bytes written")?);
        let overlapped = required_argument(endpoint, frame, 4, "overlapped")?;
        if count.0 == 0
            || buffer.0 == 0
            || requested == 0
            || requested > RUNTIME_WRITABLE_FILE_MAX_IO_BYTES
            || overlapped != 0
        {
            return Err(ApiFault::new(
                "WriteFile requires a nonempty bounded synchronous write and byte-count output",
            ));
        }
        runtime
            .memory
            .validate_access(count, 4, MemoryAccess::Write)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        let bytes = runtime
            .memory
            .read(buffer, requested as usize)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        let request = match runtime
            .begin_opfs_write(handle, count, requested)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?
        {
            Ok(prepared) => prepared,
            Err(error) => {
                return Ok(ApiHandlerResult::Immediate(
                    ApiImmediate::new(Some(u32::from(false))).with_last_error(error),
                ));
            }
        };
        Ok(ApiHandlerResult::Deferred(crate::api::HostRequest {
            request_id: request.request_id,
            service: request.service,
            payload: bytes,
        }))
    })();
    match result {
        Ok(outcome) => outcome,
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

pub fn complete_kernel32_opfs_write(
    dispatcher: &ApiDispatcher,
    call: &ApiDeferredCall,
    observation: u32,
    runtime: &mut crate::runtime::Runtime,
) -> DispatchOutcome {
    if call.request.service != "browser.opfs-write"
        || !call
            .frame
            .endpoint
            .module
            .eq_ignore_ascii_case("kernel32.dll")
        || !matches!(&call.frame.endpoint.symbol, ApiSymbol::Name { name } if name == "WriteFile")
        || observation > 3
    {
        return DispatchOutcome::Fault(ApiFault::new(
            "OPFS write completion does not match the suspended WriteFile",
        ));
    }
    let request_id = call.request.request_id;
    let (count, transferred) =
        match runtime.preflight_opfs_write_completion(request_id, call.request.payload.len()) {
            Ok(result) => result,
            Err(error) => {
                return DispatchOutcome::Fault(ApiFault::from_runtime(error, &call.frame.endpoint));
            }
        };
    let immediate = if observation == 0 {
        ApiImmediate::new(Some(u32::from(true)))
            .with_guest_write(count, transferred.to_le_bytes().to_vec())
            .with_last_error(ERROR_SUCCESS)
    } else {
        let error = match observation {
            1 => ERROR_FILE_NOT_FOUND,
            2 => ERROR_ACCESS_DENIED,
            3 => ERROR_DISK_FULL,
            _ => unreachable!("validated observation"),
        };
        ApiImmediate::new(Some(u32::from(false))).with_last_error(error)
    };
    let outcome = dispatcher.complete_deferred(
        call,
        HostCompletion {
            request_id,
            result: immediate,
        },
        runtime,
    );
    if matches!(outcome, DispatchOutcome::Immediate(_)) {
        runtime.commit_opfs_write(request_id, observation == 0);
    }
    outcome
}

fn handle_flush_file_buffers_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let handle = RuntimeHandle(required_argument(endpoint, frame, 0, "file handle")?);
    if !runtime
        .is_live_object_handle(handle, RuntimeObjectKind::File)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
    {
        return Ok(ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_INVALID_HANDLE));
    }
    let file = runtime
        .source_file_for_handle(handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    match file.backing {
        RuntimeFileBacking::ImmutableSource { .. } => {
            Ok(ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_ACCESS_DENIED))
        }
        RuntimeFileBacking::Opfs { .. } => {
            if runtime
                .opfs_write_pending(handle)
                .map_err(|error| ApiFault::from_runtime(error, endpoint))?
            {
                return Err(ApiFault::new(
                    "FlushFileBuffers requires the pending write to complete",
                ));
            }
            Ok(ApiImmediate::new(Some(u32::from(true))))
        }
    }
}

fn handle_set_file_pointer_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let handle = RuntimeHandle(required_argument(endpoint, frame, 0, "file handle")?);
    let distance_low = required_argument(endpoint, frame, 1, "distance low")?;
    let distance_high = required_argument(endpoint, frame, 2, "distance high")?;
    let origin = required_argument(endpoint, frame, 3, "move method")?;
    if !runtime
        .is_live_object_handle(handle, RuntimeObjectKind::File)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
    {
        return Ok(ApiImmediate::new(Some(u32::MAX)).with_last_error(ERROR_INVALID_HANDLE));
    }
    if origin > 2 {
        return Ok(ApiImmediate::new(Some(u32::MAX)).with_last_error(ERROR_INVALID_PARAMETER));
    }
    let distance_high = (distance_high != 0).then_some(GuestAddress(distance_high));
    let distance = if let Some(address) = distance_high {
        let high = runtime
            .memory
            .read(address, 4)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        runtime
            .memory
            .validate_access(address, 4, MemoryAccess::Write)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        ((u64::from(u32::from_le_bytes(high.try_into().expect("one DWORD"))) << 32)
            | u64::from(distance_low)) as i64
    } else {
        i64::from(distance_low as i32)
    };
    match runtime
        .seek_source_file(handle, distance, origin, distance_high)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
    {
        Ok(cursor) => {
            let result = ApiImmediate::new(Some(cursor as u32));
            Ok(if cursor as u32 == u32::MAX {
                result.with_last_error(0)
            } else {
                result
            })
        }
        Err(error) => Ok(ApiImmediate::new(Some(u32::MAX)).with_last_error(error)),
    }
}

fn handle_close_handle_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let handle = RuntimeHandle(required_argument(endpoint, frame, 0, "object handle")?);
    let Ok(kind) = handle.kind() else {
        return Ok(ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_INVALID_HANDLE));
    };
    if !matches!(
        kind,
        RuntimeObjectKind::Process
            | RuntimeObjectKind::Thread
            | RuntimeObjectKind::File
            | RuntimeObjectKind::WaitableTimer
            | RuntimeObjectKind::Event
            | RuntimeObjectKind::Mutex
    ) || !runtime
        .is_live_object_handle(handle, kind)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
    {
        return Ok(ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_INVALID_HANDLE));
    }
    runtime
        .close_handle(handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(u32::from(true))))
}

pub fn complete_kernel32_source_read(
    dispatcher: &ApiDispatcher,
    call: &ApiDeferredCall,
    response: RuntimeSourceReadHostResponse,
    runtime: &mut crate::runtime::Runtime,
) -> DispatchOutcome {
    if !call
        .frame
        .endpoint
        .module
        .eq_ignore_ascii_case("kernel32.dll")
        || !matches!(&call.frame.endpoint.symbol, ApiSymbol::Name { name } if name == "ReadFile")
        || call.request.request_id != response.request_id
        || !matches!(
            call.request.service.as_str(),
            RUNTIME_SOURCE_READ_SERVICE | RUNTIME_OPFS_READ_SERVICE
        )
    {
        return DispatchOutcome::Fault(ApiFault::new(
            "source read host completion does not match its suspended API call",
        ));
    }
    let plan = match runtime.plan_source_file_read_completion(&response) {
        Ok(plan) => plan,
        Err(error) => {
            return DispatchOutcome::Fault(ApiFault::from_runtime(error, &call.frame.endpoint));
        }
    };
    let mut immediate = ApiImmediate::new(Some(u32::from(response.win32_error == 0)))
        .with_guest_write(
            plan.bytes_read(),
            response.transferred_bytes.to_le_bytes().to_vec(),
        )
        .with_last_error(response.win32_error);
    if response.win32_error == 0 && !response.bytes.is_empty() {
        immediate = immediate.with_guest_write(plan.buffer(), response.bytes);
    }
    let request_id = call.request.request_id;
    let outcome = dispatcher.complete_deferred(
        call,
        HostCompletion {
            request_id,
            result: immediate,
        },
        runtime,
    );
    if matches!(outcome, DispatchOutcome::Immediate(_)) {
        runtime.commit_source_file_read_completion(plan);
    }
    outcome
}

pub fn complete_kernel32_waitable_timer_pacing(
    dispatcher: &ApiDispatcher,
    call: &ApiDeferredCall,
    acknowledgement: u32,
    runtime: &mut crate::runtime::Runtime,
) -> DispatchOutcome {
    if !call
        .frame
        .endpoint
        .module
        .eq_ignore_ascii_case("kernel32.dll")
        || !matches!(&call.frame.endpoint.symbol, ApiSymbol::Name { name } if name == "WaitForSingleObjectEx")
        || call.request.service != RUNTIME_WAITABLE_TIMER_PACING_SERVICE
        || acknowledgement != 1
    {
        return DispatchOutcome::Fault(ApiFault::new(
            "timer pacing host completion does not match its suspended API call",
        ));
    }
    let plan = match runtime.plan_waitable_timer_pacing_completion(call.request.request_id) {
        Ok(plan) => plan,
        Err(error) => {
            return DispatchOutcome::Fault(ApiFault::from_runtime(error, &call.frame.endpoint));
        }
    };
    let registration = plan.registration();
    let pacing = registration
        .waitable_timer_pacing
        .expect("timer pacing completion plan has a pacing endpoint");
    if call.request.payload != pacing.delay_milliseconds.to_le_bytes() {
        return DispatchOutcome::Fault(ApiFault::new(
            "timer pacing host payload differs from the Rust wait authority",
        ));
    }
    if let Some(pacing) = plan.repace() {
        if let Err(fault) =
            dispatcher.validate_deferred_completion(call, call.request.request_id, runtime.memory())
        {
            return DispatchOutcome::Fault(fault);
        }
        let retry = ApiDeferredCall {
            request: crate::api::HostRequest {
                request_id: pacing.request_id,
                service: RUNTIME_WAITABLE_TIMER_PACING_SERVICE.to_owned(),
                payload: pacing.delay_milliseconds.to_le_bytes().to_vec(),
            },
            frame: call.frame.clone(),
        };
        runtime.commit_waitable_timer_pacing_completion(plan);
        return DispatchOutcome::Deferred(retry);
    }
    let outcome = plan.outcome();
    let request_id = call.request.request_id;
    let outcome = dispatcher.complete_deferred(
        call,
        HostCompletion {
            request_id,
            result: wait_result_immediate(outcome),
        },
        runtime,
    );
    if matches!(outcome, DispatchOutcome::Immediate(_)) {
        runtime.commit_waitable_timer_pacing_completion(plan);
    }
    outcome
}

pub fn complete_kernel32_object_wait(
    dispatcher: &ApiDispatcher,
    call: &ApiDeferredCall,
    acknowledgement: u32,
    runtime: &mut crate::runtime::Runtime,
) -> DispatchOutcome {
    if !call
        .frame
        .endpoint
        .module
        .eq_ignore_ascii_case("kernel32.dll")
        || !matches!(&call.frame.endpoint.symbol, ApiSymbol::Name { name } if name == "WaitForSingleObject")
        || call.request.service != crate::RUNTIME_OBJECT_WAIT_SERVICE
    {
        return DispatchOutcome::Fault(ApiFault::new(
            "object wait completion does not match its suspended API",
        ));
    }
    let plan = match runtime.plan_object_wait_completion(call.request.request_id, acknowledgement) {
        Ok(plan) => plan,
        Err(error) => {
            return DispatchOutcome::Fault(ApiFault::from_runtime(error, &call.frame.endpoint));
        }
    };
    let pacing = plan
        .registration()
        .object_wait_pacing
        .expect("validated object wait pacing");
    if call.request.payload != pacing.delay_milliseconds.to_le_bytes() {
        return DispatchOutcome::Fault(ApiFault::new(
            "object wait delay payload differs from Rust authority",
        ));
    }
    if let Some(retry) = plan.retry() {
        if let Err(fault) =
            dispatcher.validate_deferred_completion(call, call.request.request_id, runtime.memory())
        {
            return DispatchOutcome::Fault(fault);
        }
        let call = ApiDeferredCall {
            frame: call.frame.clone(),
            request: crate::api::HostRequest {
                request_id: retry.request_id,
                service: crate::RUNTIME_OBJECT_WAIT_SERVICE.to_owned(),
                payload: retry.delay_milliseconds.to_le_bytes().to_vec(),
            },
        };
        runtime.commit_object_wait_completion(plan);
        return DispatchOutcome::Deferred(call);
    }
    let outcome = dispatcher.complete_deferred(
        call,
        HostCompletion {
            request_id: call.request.request_id,
            result: wait_result_immediate(plan.outcome()),
        },
        runtime,
    );
    if matches!(outcome, DispatchOutcome::Immediate(_)) {
        runtime.commit_object_wait_completion(plan);
    }
    outcome
}

pub fn complete_kernel32_audio_notification_wait(
    dispatcher: &ApiDispatcher,
    call: &ApiDeferredCall,
    acknowledgement: u32,
    runtime: &mut crate::runtime::Runtime,
) -> DispatchOutcome {
    if !call
        .frame
        .endpoint
        .module
        .eq_ignore_ascii_case("kernel32.dll")
        || !matches!(&call.frame.endpoint.symbol, ApiSymbol::Name { name } if name == "WaitForMultipleObjects")
        || call.request.service != RUNTIME_AUDIO_NOTIFICATION_PACING_SERVICE
        || acknowledgement != 1
    {
        return DispatchOutcome::Fault(ApiFault::new(
            "audio notification pacing completion does not match its suspended API call",
        ));
    }
    let timeout = call.frame.argument_u32(3).unwrap_or(0);
    let plan = match runtime.plan_audio_notification_wait_completion(
        call.request.request_id,
        timeout,
        &call.request.payload,
    ) {
        Ok(plan) => plan,
        Err(error) => {
            return DispatchOutcome::Fault(ApiFault::from_runtime(error, &call.frame.endpoint));
        }
    };
    let result = plan
        .result_index()
        .map_or(WAIT_TIMEOUT, |index| WAIT_OBJECT_0 + index);
    if let Some(payload) = plan.retry_payload() {
        if let Err(fault) =
            dispatcher.validate_deferred_completion(call, call.request.request_id, runtime.memory())
        {
            return DispatchOutcome::Fault(fault);
        }
        let retry = ApiDeferredCall {
            request: crate::api::HostRequest {
                request_id: call.request.request_id,
                service: RUNTIME_AUDIO_NOTIFICATION_PACING_SERVICE.to_owned(),
                payload: payload.to_vec(),
            },
            frame: call.frame.clone(),
        };
        runtime.commit_audio_notification_wait_completion(plan);
        return DispatchOutcome::Deferred(retry);
    }
    let request_id = call.request.request_id;
    let outcome = dispatcher.complete_deferred(
        call,
        HostCompletion {
            request_id,
            result: ApiImmediate::new(Some(result)),
        },
        runtime,
    );
    if matches!(outcome, DispatchOutcome::Immediate(_)) {
        runtime.commit_audio_notification_wait_completion(plan);
    }
    outcome
}

fn read_nul_terminated_source_path(
    endpoint: &ApiEndpointIdentity,
    memory: &GuestMemory,
    source: GuestAddress,
    maximum_path_bytes: u32,
) -> Result<Vec<u8>, ApiFault> {
    let maximum = usize::try_from(maximum_path_bytes).map_err(|_| {
        ApiFault::new("validated source file path limit does not fit host accounting")
    })?;
    let mut bytes = Vec::new();
    for offset in 0..maximum {
        let address = add_guest_offset(endpoint, source, offset)?;
        let byte = memory
            .read(address, 1)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?[0];
        bytes.push(byte);
        if byte == 0 {
            return Ok(bytes);
        }
    }
    Err(ApiFault::new(format!(
        "ANSI source file path did not terminate within {maximum_path_bytes} bytes"
    )))
}

fn handle_create_mutex_a_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    const MAXIMUM_NAME_BYTES: usize = 260;
    let security_attributes = required_argument(endpoint, frame, 0, "security attributes")?;
    let initial_owner = required_argument(endpoint, frame, 1, "initial owner")? != 0;
    let name_address = required_argument(endpoint, frame, 2, "ANSI mutex name")?;
    if security_attributes != 0 {
        return Err(ApiFault::new(
            "CreateMutexA currently supports only null security attributes",
        ));
    }
    let name_a = if name_address == 0 {
        None
    } else {
        let bytes_with_nul = read_nul_terminated_ansi_mutex_name(
            endpoint,
            &runtime.memory,
            GuestAddress(name_address),
            MAXIMUM_NAME_BYTES,
        )?;
        if bytes_with_nul == [0] {
            return Err(ApiFault::new(
                "empty object-manager names are not yet supported",
            ));
        }
        let utf16_units_with_nul = decode_windows_932(&bytes_with_nul);
        Some(
            RuntimeMutexNameA::new(bytes_with_nul, utf16_units_with_nul)
                .map_err(|error| ApiFault::from_runtime(error, endpoint))?,
        )
    };
    let (handle, already_exists) = runtime
        .create_mutex_a(name_a, initial_owner)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(
        ApiImmediate::new(Some(handle.0)).with_last_error(if already_exists {
            ERROR_ALREADY_EXISTS
        } else {
            ERROR_SUCCESS
        }),
    )
}

fn handle_create_event_a(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let security_attributes = required_argument(endpoint, frame, 0, "security attributes")?;
    let manual_reset = required_argument(endpoint, frame, 1, "manual reset")? != 0;
    let initial_state = required_argument(endpoint, frame, 2, "initial state")? != 0;
    let name = required_argument(endpoint, frame, 3, "ANSI event name")?;
    if security_attributes != 0 {
        return Err(ApiFault::new(
            "bounded CreateEventA supports null security attributes",
        ));
    }
    if name != 0 {
        return Err(ApiFault::new(
            "bounded CreateEventA supports the reached unnamed event variant",
        ));
    }
    let handle = runtime
        .create_event(manual_reset, initial_state)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(handle.0)).with_last_error(ERROR_SUCCESS))
}

fn handle_set_event(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
    set: bool,
) -> Result<ApiImmediate, ApiFault> {
    let handle = RuntimeHandle(required_argument(endpoint, frame, 0, "event handle")?);
    let result = if set {
        runtime.set_event(handle)
    } else {
        runtime.reset_event(handle)
    };
    let success = result.map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let immediate = ApiImmediate::new(Some(u32::from(success)));
    Ok(if success {
        immediate
    } else {
        immediate.with_last_error(ERROR_INVALID_HANDLE)
    })
}

fn handle_wait_for_single_object(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let result = (|| {
        let handle = RuntimeHandle(required_argument(endpoint, frame, 0, "wait handle")?);
        let timeout = required_argument(endpoint, frame, 1, "timeout")?;
        if timeout == 0 || timeout == u32::MAX {
            return handle_wait_for_single_object_immediate(endpoint, frame, runtime)
                .map(ApiHandlerResult::Immediate);
        }
        let kind = match handle.kind() {
            Ok(kind @ (RuntimeObjectKind::Event | RuntimeObjectKind::Thread)) => kind,
            _ => {
                return Ok(ApiHandlerResult::Immediate(
                    ApiImmediate::new(Some(WAIT_FAILED)).with_last_error(ERROR_INVALID_HANDLE),
                ));
            }
        };
        if !runtime
            .is_live_object_handle(handle, kind)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?
        {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(Some(WAIT_FAILED)).with_last_error(ERROR_INVALID_HANDLE),
            ));
        }
        match runtime
            .begin_finite_object_wait(handle, timeout)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?
        {
            RuntimeWaitForSingleObjectExBegin::Immediate { outcome } => {
                Ok(ApiHandlerResult::Immediate(wait_result_immediate(outcome)))
            }
            RuntimeWaitForSingleObjectExBegin::Deferred { registration } => {
                let pacing = registration
                    .object_wait_pacing
                    .expect("finite object wait has a pacing request");
                Ok(ApiHandlerResult::Deferred(crate::api::HostRequest {
                    request_id: pacing.request_id,
                    service: crate::RUNTIME_OBJECT_WAIT_SERVICE.to_owned(),
                    payload: pacing.delay_milliseconds.to_le_bytes().to_vec(),
                }))
            }
        }
    })();
    match result {
        Ok(outcome) => outcome,
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn handle_wait_for_single_object_immediate(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let handle = RuntimeHandle(required_argument(endpoint, frame, 0, "wait handle")?);
    let timeout = required_argument(endpoint, frame, 1, "timeout")?;
    let kind = match handle.kind() {
        Ok(kind @ (RuntimeObjectKind::Event | RuntimeObjectKind::Thread)) => kind,
        _ => {
            return Ok(ApiImmediate::new(Some(WAIT_FAILED)).with_last_error(ERROR_INVALID_HANDLE));
        }
    };
    if !runtime
        .is_live_object_handle(handle, kind)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
    {
        return Ok(ApiImmediate::new(Some(WAIT_FAILED)).with_last_error(ERROR_INVALID_HANDLE));
    }
    let wait_state = runtime
        .object_wait_state(handle, kind)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    if timeout == 0 {
        let signaled = if kind == RuntimeObjectKind::Event {
            runtime
                .try_wait_event(handle)
                .map_err(|error| ApiFault::from_runtime(error, endpoint))?
        } else {
            matches!(wait_state, RuntimeWaitState::Signaled { .. })
        };
        return Ok(ApiImmediate::new(Some(if signaled {
            WAIT_OBJECT_0
        } else {
            WAIT_TIMEOUT
        })));
    }
    let wait = runtime
        .wait_current_thread_on(handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    match wait {
        RuntimeWaitDisposition::AlreadySignaled { .. } => {
            Ok(ApiImmediate::new(Some(WAIT_OBJECT_0)))
        }
        RuntimeWaitDisposition::Pending { .. } => Ok(ApiImmediate::new(Some(WAIT_OBJECT_0))
            .with_execution_boundary(RuntimeApiBoundaryDisposition::BlockCurrentThread)),
        RuntimeWaitDisposition::MutexAcquired { .. } => Err(ApiFault::new(
            "event/thread wait unexpectedly returned a mutex acquisition",
        )),
    }
}

fn handle_wait_for_multiple_objects(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let result = (|| {
        let wait_all = required_argument(endpoint, frame, 2, "wait-all")? != 0;
        let timeout = required_argument(endpoint, frame, 3, "timeout")?;
        if wait_all {
            return Err(ApiFault::new(
                "reached DirectSound event wait requires wait-any semantics",
            ));
        }
        let handles = read_wait_handles(endpoint, frame, &runtime.memory)?;
        for &handle in &handles {
            if !runtime
                .is_live_object_handle(handle, RuntimeObjectKind::Event)
                .map_err(|error| ApiFault::from_runtime(error, endpoint))?
            {
                return Ok(ApiHandlerResult::Immediate(
                    ApiImmediate::new(Some(WAIT_FAILED)).with_last_error(ERROR_INVALID_HANDLE),
                ));
            }
        }
        runtime
            .refresh_audio_notifications_for_handles(&handles)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        for (index, &handle) in handles.iter().enumerate() {
            let event = runtime
                .event_for_handle(handle)
                .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
            if matches!(event.wait_state, RuntimeWaitState::Signaled { .. }) {
                runtime
                    .try_wait_event(handle)
                    .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
                return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(
                    WAIT_OBJECT_0 + index as u32,
                ))));
            }
        }
        if timeout == 0 {
            return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(
                WAIT_TIMEOUT,
            ))));
        }
        let Some((request, payload)) = runtime
            .begin_audio_notification_wait(&handles, timeout)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?
        else {
            return Err(ApiFault::new(
                "finite wait handle array is not owned by a DirectSound notification buffer",
            ));
        };
        Ok(ApiHandlerResult::Deferred(crate::api::HostRequest {
            request_id: request.request_id,
            service: request.service,
            payload,
        }))
    })();
    match result {
        Ok(outcome) => outcome,
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn read_wait_handles(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    memory: &GuestMemory,
) -> Result<Vec<RuntimeHandle>, ApiFault> {
    let count = usize::try_from(required_argument(endpoint, frame, 0, "handle count")?)
        .map_err(|_| ApiFault::new("wait handle count does not fit host accounting"))?;
    let pointer = required_argument(endpoint, frame, 1, "handle array")?;
    if count == 0 || count > RUNTIME_AUDIO_NOTIFICATION_LIMIT || pointer == 0 {
        return Err(ApiFault::new(
            "wait handle array must be bounded, nonempty, and nonnull",
        ));
    }
    let bytes = memory
        .read(GuestAddress(pointer), count * 4)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
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
    endpoint: &ApiEndpointIdentity,
    memory: &GuestMemory,
    source: GuestAddress,
    maximum: usize,
) -> Result<Vec<u8>, ApiFault> {
    let mut bytes = Vec::new();
    for offset in 0..maximum {
        let address = add_guest_offset(endpoint, source, offset)?;
        let byte = memory
            .read(address, 1)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?[0];
        bytes.push(byte);
        if byte == 0 {
            return Ok(bytes);
        }
    }
    Err(ApiFault::new(format!(
        "ANSI mutex name did not terminate within the limit of {} bytes",
        maximum
    )))
}

fn handle_create_waitable_timer_a_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let security_attributes = required_argument(endpoint, frame, 0, "security attributes")?;
    let manual_reset = required_argument(endpoint, frame, 1, "manual reset")? != 0;
    let name = required_argument(endpoint, frame, 2, "ANSI timer name")?;
    if security_attributes != 0 || name != 0 {
        return Err(ApiFault::new(
            "CreateWaitableTimerA supports unnamed timers with default security",
        ));
    }
    let timer = runtime
        .create_waitable_timer_a(manual_reset)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(timer.0)).with_last_error(ERROR_SUCCESS))
}

fn handle_set_waitable_timer_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let handle = RuntimeHandle(required_argument(
        endpoint,
        frame,
        0,
        "waitable timer handle",
    )?);
    let due = GuestAddress(required_argument(endpoint, frame, 1, "due time")?);
    let period = required_argument(endpoint, frame, 2, "period")?;
    let routine = required_argument(endpoint, frame, 3, "completion routine")?;
    let context = required_argument(endpoint, frame, 4, "completion context")?;
    let resume = required_argument(endpoint, frame, 5, "resume")?;
    if period != 0 || routine != 0 || context != 0 || resume != 0 {
        return Err(ApiFault::new(
            "only one-shot timers without completion routines or resume are supported",
        ));
    }
    if !runtime
        .is_live_object_handle(handle, RuntimeObjectKind::WaitableTimer)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
    {
        return Ok(ApiImmediate::new(Some(0)).with_last_error(ERROR_INVALID_HANDLE));
    }
    runtime
        .waitable_timer_for_handle(handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let bytes = runtime
        .memory
        .read(due, size_of::<i64>())
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let due_time_100ns = i64::from_le_bytes(bytes.try_into().expect("read exactly 8 bytes"));
    if due_time_100ns >= 0 {
        return Err(ApiFault::new(
            "absolute timer due times are not yet supported",
        ));
    }
    runtime
        .set_waitable_timer(handle, due_time_100ns.unsigned_abs())
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(u32::from(true))).with_last_error(ERROR_SUCCESS))
}

fn handle_wait_for_single_object_ex_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let result = (|| {
        let handle = RuntimeHandle(required_argument(endpoint, frame, 0, "wait handle")?);
        let timeout = required_argument(endpoint, frame, 1, "timeout")?;
        let alertable = required_argument(endpoint, frame, 2, "alertable")? != 0;
        if !runtime
            .is_live_object_handle(handle, RuntimeObjectKind::WaitableTimer)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?
        {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(Some(WAIT_FAILED)).with_last_error(ERROR_INVALID_HANDLE),
            ));
        }
        runtime
            .waitable_timer_for_handle(handle)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        // The browser timer is finite and cannot represent INFINITE or delays
        // beyond its signed 32-bit range. Do not report an early timeout.
        if timeout > i32::MAX as u32 {
            return Err(ApiFault::new(
                "WaitForSingleObjectEx requires a finite browser-representable timeout",
            ));
        }
        match runtime
            .begin_wait_for_single_object_ex(handle, timeout, alertable)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?
        {
            RuntimeWaitForSingleObjectExBegin::Immediate { outcome } => {
                Ok(ApiHandlerResult::Immediate(wait_result_immediate(outcome)))
            }
            RuntimeWaitForSingleObjectExBegin::Deferred { registration } => {
                let pacing = registration.waitable_timer_pacing.ok_or_else(|| {
                    ApiFault::new("timer wait registration has no browser pacing request")
                })?;
                Ok(ApiHandlerResult::Deferred(crate::api::HostRequest {
                    request_id: pacing.request_id,
                    service: RUNTIME_WAITABLE_TIMER_PACING_SERVICE.to_owned(),
                    payload: pacing.delay_milliseconds.to_le_bytes().to_vec(),
                }))
            }
        }
    })();
    match result {
        Ok(outcome) => outcome,
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn wait_result_immediate(outcome: RuntimeWaitForSingleObjectExOutcome) -> ApiImmediate {
    let value = match outcome {
        RuntimeWaitForSingleObjectExOutcome::ObjectSignaled => WAIT_OBJECT_0,
        RuntimeWaitForSingleObjectExOutcome::TimedOut => WAIT_TIMEOUT,
    };
    ApiImmediate::new(Some(value))
}

fn handle_sleep_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let result = (|| {
        let milliseconds = required_argument(endpoint, frame, 0, "sleep interval")?;
        if milliseconds == 0 {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(None)
                    .with_execution_boundary(RuntimeApiBoundaryDisposition::YieldCurrentThread),
            ));
        }
        // setTimeout cannot represent INFINITE or delays beyond its signed 32-bit range.
        if milliseconds > i32::MAX as u32 {
            return Err(ApiFault::new(
                "Sleep interval exceeds the browser timer range",
            ));
        }
        let request = runtime
            .begin_host_request(RUNTIME_SLEEP_DELAY_SERVICE)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
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

fn handle_get_string_type_w_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    memory: &GuestMemory,
) -> Result<ApiImmediate, ApiFault> {
    const MAXIMUM_SOURCE_CODE_UNITS: usize = 65_536;
    let info_type = required_argument(endpoint, frame, 0, "character information type")?;
    let source_address = required_argument(endpoint, frame, 1, "UTF-16 source")?;
    let source_count = required_argument(endpoint, frame, 2, "UTF-16 source length")? as i32;
    let output_address = required_argument(endpoint, frame, 3, "character-type output")?;
    if info_type != 1 {
        return Err(ApiFault::new(format!(
            "GetStringTypeW does not support information type {info_type:#010x}"
        )));
    }

    if source_count == 0 {
        return Ok(get_string_type_failure(ERROR_INVALID_PARAMETER));
    }
    if source_count < 0 {
        return Err(ApiFault::new(
            "GetStringTypeW supports only explicit positive source lengths",
        ));
    }
    let source_units = usize::try_from(source_count).map_err(|_| {
        ApiFault::new("validated positive UTF-16 source count does not fit host accounting")
    })?;
    if source_units > MAXIMUM_SOURCE_CODE_UNITS {
        return Err(ApiFault::new(format!(
            "UTF-16 source count {source_units} exceeds the limit of {MAXIMUM_SOURCE_CODE_UNITS}"
        )));
    }
    let byte_length = source_units
        .checked_mul(2)
        .ok_or_else(|| ApiFault::new("UTF-16 classification length overflowed host accounting"))?;
    let source = GuestAddress(source_address);
    let output = GuestAddress(output_address);
    memory
        .validate_access(source, byte_length, MemoryAccess::Read)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    memory
        .validate_access(output, byte_length, MemoryAccess::Write)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    if guest_ranges_overlap(source_address, byte_length, output_address, byte_length)? {
        return Err(ApiFault::new(
            "GetStringTypeW does not support overlapping input and output",
        ));
    }

    let source_bytes = memory
        .read(source, byte_length)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let mut output_bytes = Vec::with_capacity(byte_length);
    for (index, bytes) in source_bytes.chunks_exact(2).enumerate() {
        let unit = u16::from_le_bytes([bytes[0], bytes[1]]);
        let character_type = classify_windows_ctype1(unit).ok_or_else(|| {
            ApiFault::new(
                format!(
                    "the selected CT_CTYPE1 classifier does not cover UTF-16 unit {unit:#06x} at index {index}"
                ),
            )
        })?;
        output_bytes.extend_from_slice(&character_type.to_le_bytes());
    }
    Ok(ApiImmediate::new(Some(u32::from(true))).with_guest_write(output, output_bytes))
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
    ApiImmediate::new(Some(u32::from(false))).with_last_error(last_error)
}

fn handle_lc_map_string_w_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    memory: &GuestMemory,
) -> Result<ApiImmediate, ApiFault> {
    const MAXIMUM_SOURCE_CODE_UNITS: usize = 65_536;
    let locale = required_argument(endpoint, frame, 0, "locale")?;
    let flags = required_argument(endpoint, frame, 1, "mapping flags")?;
    let source_address = required_argument(endpoint, frame, 2, "UTF-16 source")?;
    let source_count = required_argument(endpoint, frame, 3, "UTF-16 source length")? as i32;
    let destination_address = required_argument(endpoint, frame, 4, "UTF-16 destination")?;
    let destination_count =
        required_argument(endpoint, frame, 5, "UTF-16 destination capacity")? as i32;
    if locale != 0x0411 {
        return Err(ApiFault::new(format!(
            "LCMapStringW does not support locale {locale:#010x}"
        )));
    }
    if !matches!(flags, 0x0100 | 0x0200) {
        return Err(ApiFault::new(format!(
            "LCMapStringW does not support mapping flags {flags:#010x}"
        )));
    }

    if source_count == 0 {
        return Ok(lc_map_string_failure(ERROR_INVALID_PARAMETER));
    }
    if source_count < 0 {
        return Err(ApiFault::new(
            "LCMapStringW supports only explicit positive source lengths",
        ));
    }
    if destination_count < 0 {
        return Ok(lc_map_string_failure(ERROR_INVALID_PARAMETER));
    }
    let source_units = usize::try_from(source_count).map_err(|_| {
        ApiFault::new("validated positive UTF-16 source count does not fit host accounting")
    })?;
    if source_units > MAXIMUM_SOURCE_CODE_UNITS {
        return Err(ApiFault::new(format!(
            "UTF-16 source count {source_units} exceeds the limit of {MAXIMUM_SOURCE_CODE_UNITS}"
        )));
    }
    let byte_length = source_units
        .checked_mul(2)
        .ok_or_else(|| ApiFault::new("UTF-16 mapping length overflowed host accounting"))?;
    let source = GuestAddress(source_address);
    memory
        .validate_access(source, byte_length, MemoryAccess::Read)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let source_bytes = memory
        .read(source, byte_length)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let mut output_bytes = Vec::with_capacity(byte_length);
    for (index, bytes) in source_bytes.chunks_exact(2).enumerate() {
        let unit = u16::from_le_bytes([bytes[0], bytes[1]]);
        let mapped = map_windows_lc_string_unit(flags, unit).ok_or_else(|| {
            ApiFault::new(
                format!(
                    "the selected locale mapper does not cover UTF-16 unit {unit:#06x} at index {index} for flags {flags:#010x}"
                ),
            )
        })?;
        output_bytes.extend_from_slice(&mapped.to_le_bytes());
    }

    if destination_address == 0 && destination_count == 0 {
        return Ok(ApiImmediate::new(Some(source_count as u32)));
    }
    if destination_address == 0 || destination_count == 0 {
        return Ok(lc_map_string_failure(ERROR_INVALID_PARAMETER));
    }
    let destination_units = usize::try_from(destination_count).map_err(|_| {
        ApiFault::new("validated positive UTF-16 destination count does not fit host accounting")
    })?;
    if destination_units < source_units {
        return Ok(lc_map_string_failure(ERROR_INSUFFICIENT_BUFFER));
    }
    let destination = GuestAddress(destination_address);
    memory
        .validate_access(destination, byte_length, MemoryAccess::Write)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    if guest_ranges_overlap(
        source_address,
        byte_length,
        destination_address,
        byte_length,
    )? {
        return Err(ApiFault::new(
            "LCMapStringW does not support overlapping input and output",
        ));
    }
    Ok(ApiImmediate::new(Some(source_count as u32)).with_guest_write(destination, output_bytes))
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
    ApiImmediate::new(Some(0)).with_last_error(last_error)
}

fn read_multi_byte_source(
    endpoint: &ApiEndpointIdentity,
    memory: &GuestMemory,
    source: GuestAddress,
    source_count: i32,
    maximum: usize,
) -> Result<Vec<u8>, ApiFault> {
    if source_count == -1 {
        let mut bytes = Vec::new();
        for offset in 0..maximum {
            let address = add_guest_offset(endpoint, source, offset)?;
            let byte = memory
                .read(address, 1)
                .map_err(|error| ApiFault::from_memory(error, endpoint))?[0];
            bytes.push(byte);
            if byte == 0 {
                return Ok(bytes);
            }
        }
        return Err(ApiFault::new(format!(
            "multibyte source did not terminate within the limit of {} bytes",
            maximum
        )));
    }
    let byte_length = usize::try_from(source_count).map_err(|_| {
        ApiFault::new("validated positive multibyte source count does not fit host accounting")
    })?;
    if byte_length > maximum {
        return Err(ApiFault::new(format!(
            "multibyte source count {byte_length} exceeds the limit of {}",
            maximum
        )));
    }
    memory
        .validate_access(source, byte_length, MemoryAccess::Read)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    memory
        .read(source, byte_length)
        .map_err(|error| ApiFault::from_memory(error, endpoint))
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
    ApiImmediate::new(Some(0)).with_last_error(last_error)
}

fn guest_ranges_overlap(
    left_address: u32,
    left_length: usize,
    right_address: u32,
    right_length: usize,
) -> Result<bool, ApiFault> {
    let left_end = u64::from(left_address)
        .checked_add(
            u64::try_from(left_length)
                .map_err(|_| ApiFault::new("guest range length does not fit x86 accounting"))?,
        )
        .ok_or_else(|| ApiFault::new("UTF-16 source range overflowed x86 accounting"))?;
    let right_end = u64::from(right_address)
        .checked_add(
            u64::try_from(right_length)
                .map_err(|_| ApiFault::new("guest range length does not fit x86 accounting"))?,
        )
        .ok_or_else(|| ApiFault::new("byte destination range overflowed x86 accounting"))?;
    if left_end > u64::from(u32::MAX) + 1 || right_end > u64::from(u32::MAX) + 1 {
        return Err(ApiFault::new(
            "conversion range exceeds the x86 address space",
        ));
    }
    Ok(u64::from(left_address) < right_end && u64::from(right_address) < left_end)
}

fn wide_char_failure(last_error: u32) -> ApiImmediate {
    ApiImmediate::new(Some(0)).with_last_error(last_error)
}

fn handle_query_performance_counter_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let output = required_argument(endpoint, frame, 0, "performance counter output")?;
    Ok(ApiImmediate::new(Some(u32::from(true))).with_guest_write(
        GuestAddress(output),
        runtime
            .sample_performance_counter()
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?
            .to_le_bytes()
            .to_vec(),
    ))
}

fn handle_query_performance_frequency(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let output = required_argument(endpoint, frame, 0, "performance frequency output")?;
    let frequency = runtime.clock().performance_frequency;
    Ok(ApiImmediate::new(Some(u32::from(true)))
        .with_guest_write(GuestAddress(output), frequency.to_le_bytes().to_vec()))
}

fn handle_get_startup_info_w(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let output = required_argument(endpoint, frame, 0, "STARTUPINFOW output")?;
    Ok(ApiImmediate::new(None)
        .with_guest_write(GuestAddress(output), runtime.startup_info().bytes.clone()))
}

fn handle_heap_set_information(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let heap = required_argument(endpoint, frame, 0, "heap handle")?;
    let information_class = required_argument(endpoint, frame, 1, "heap information class")?;
    let information = required_argument(endpoint, frame, 2, "heap information")?;
    let information_length = required_argument(endpoint, frame, 3, "heap information length")?;
    if heap != 0
        || information_class != HEAP_ENABLE_TERMINATION_ON_CORRUPTION
        || information != 0
        || information_length != 0
    {
        return Err(ApiFault::new(
            "only process-wide HeapEnableTerminationOnCorruption is implemented",
        ));
    }
    runtime.set_heap_termination_on_corruption(true);
    Ok(ApiImmediate::new(Some(u32::from(true))))
}

fn handle_heap_create(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let creation_flags = required_argument(endpoint, frame, 0, "heap creation flags")?;
    let initial_size = required_argument(endpoint, frame, 1, "heap initial size")?;
    let maximum_size = required_argument(endpoint, frame, 2, "heap maximum size")?;
    // Rust executes each heap operation synchronously; HEAP_NO_SERIALIZE
    // therefore needs no separate heap state or locking mode.
    if creation_flags & !HEAP_NO_SERIALIZE != 0 {
        return Err(ApiFault::new(
            "HeapCreate supports only no flags or HEAP_NO_SERIALIZE",
        ));
    }
    let heap = runtime
        .allocate_private_heap(initial_size, maximum_size)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(if heap == 0 {
        ApiImmediate::new(Some(0)).with_last_error(ERROR_NOT_ENOUGH_MEMORY)
    } else {
        ApiImmediate::new(Some(heap))
    })
}

fn heap_initialization(flags: u32) -> Result<RuntimeHeapInitialization, ApiFault> {
    if flags & !(HEAP_NO_SERIALIZE | HEAP_ZERO_MEMORY) != 0 {
        return Err(ApiFault::new(
            "heap allocation flags require an unsupported mode",
        ));
    }
    Ok(if flags & HEAP_ZERO_MEMORY != 0 {
        RuntimeHeapInitialization::Zeroed
    } else {
        RuntimeHeapInitialization::Uninitialized
    })
}

fn handle_heap_alloc_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let heap = required_argument(endpoint, frame, 0, "heap handle")?;
    let flags = required_argument(endpoint, frame, 1, "heap allocation flags")?;
    let byte_length = required_argument(endpoint, frame, 2, "heap allocation byte length")?;
    let initialization = heap_initialization(flags)?;
    let allocation = runtime
        .allocate_private_heap_block(heap, byte_length, initialization)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(
        allocation.map_or(0, |allocation| allocation.address.0),
    )))
}

fn handle_heap_realloc_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let heap = required_argument(endpoint, frame, 0, "heap handle")?;
    let flags = required_argument(endpoint, frame, 1, "heap reallocation flags")?;
    let address = GuestAddress(required_argument(endpoint, frame, 2, "heap allocation")?);
    let size = required_argument(endpoint, frame, 3, "heap reallocation byte length")?;
    let initialization = heap_initialization(flags)?;
    let Some(previous) = runtime
        .private_heap_allocation_option(heap, address)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
    else {
        return Ok(ApiImmediate::new(Some(0)).with_last_error(ERROR_INVALID_PARAMETER));
    };
    if previous.byte_length > RUNTIME_PRIVATE_HEAP_ARENA_STRIDE
        || size > RUNTIME_PRIVATE_HEAP_ARENA_STRIDE
    {
        return Err(ApiFault::new(
            "HeapReAlloc does not move lazy large allocations",
        ));
    }
    let allocation = runtime
        .reallocate_private_heap_block(heap, address, size, initialization)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(
        allocation.map_or(0, |allocation| allocation.address.0),
    )))
}

fn handle_heap_free_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let heap = required_argument(endpoint, frame, 0, "heap handle")?;
    let flags = required_argument(endpoint, frame, 1, "heap free flags")?;
    let allocation = GuestAddress(required_argument(endpoint, frame, 2, "heap allocation")?);
    if flags & !HEAP_NO_SERIALIZE != 0 {
        return Err(ApiFault::new(
            "HeapFree supports only no flags or HEAP_NO_SERIALIZE",
        ));
    }
    if allocation.0 == 0 {
        return Ok(ApiImmediate::new(Some(1)));
    }
    let Some(_) = runtime
        .private_heap_allocation_option(heap, allocation)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
    else {
        return Ok(
            ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_INVALID_PARAMETER)
        );
    };
    runtime
        .free_private_heap_block(heap, allocation)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(u32::from(true))))
}

fn handle_heap_size_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let heap = required_argument(endpoint, frame, 0, "heap handle")?;
    let flags = required_argument(endpoint, frame, 1, "heap size flags")?;
    let address = GuestAddress(required_argument(endpoint, frame, 2, "heap allocation")?);
    if flags & !HEAP_NO_SERIALIZE != 0 {
        return Err(ApiFault::new(
            "HeapSize supports only no flags or HEAP_NO_SERIALIZE",
        ));
    }
    let Some(allocation) = runtime
        .private_heap_allocation_option(heap, address)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
    else {
        return Ok(ApiImmediate::new(Some(u32::MAX)).with_last_error(ERROR_INVALID_PARAMETER));
    };
    Ok(ApiImmediate::new(Some(allocation.byte_length)))
}

fn handle_load_library_a_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let name_address = required_argument(endpoint, frame, 0, "module name")?;
    if name_address == 0 {
        return Ok(load_library_a_failure(ERROR_INVALID_PARAMETER));
    }
    let name = read_load_library_a_name(
        endpoint,
        &runtime.memory,
        GuestAddress(name_address),
        MAX_LOAD_LIBRARY_MODULE_NAME_BYTES,
    )?;
    if name.is_empty() {
        return Ok(load_library_a_failure(ERROR_INVALID_PARAMETER));
    }
    if !name.to_ascii_lowercase().ends_with(".dll")
        || name
            .bytes()
            .any(|byte| byte.is_ascii_control() || matches!(byte, b'/' | b'\\' | b':'))
    {
        return Err(ApiFault::new(
            "LoadLibraryA requires a DLL basename until the full Windows search path is implemented",
        ));
    }
    match runtime.module_by_name(&name) {
        Ok(Some(module)) if module.runtime_system_module => {
            let canonical_name = module.canonical_name.clone();
            let loaded = runtime
                .load_current_thread_runtime_system_module(name, canonical_name)
                .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
            Ok(ApiImmediate::new(Some(loaded)))
        }
        Ok(Some(module)) => {
            let handle = module.handle;
            let node = runtime
                .module_graph()
                .module_by_handle(handle)
                .map_err(|error| ApiFault::new(error.to_string()))?;
            if node.class != RuntimeModuleClass::GuestDynamicLibrary
                || !matches!(node.state, RuntimeModuleLoadState::ProcessAttached { .. })
                || runtime.module_images().image(node.id).is_err()
            {
                return Err(ApiFault::new(
                    "LoadLibraryA requires a materialized and attached guest DLL",
                ));
            }
            runtime
                .add_current_thread_module_reference(handle)
                .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
            Ok(ApiImmediate::new(Some(handle)))
        }
        Ok(None) => {
            let mut path = name.into_bytes();
            path.push(0);
            match runtime
                .resolve_file_system_path_a(&path)
                .map_err(|error| ApiFault::from_runtime(error, endpoint))?
            {
                RuntimePathResolution::RegularFile => Err(ApiFault::new(
                    "LoadLibraryA found an immutable DLL but guest module loading is not implemented",
                )),
                RuntimePathResolution::Directory
                | RuntimePathResolution::MissingFile
                | RuntimePathResolution::MissingPath => {
                    Ok(load_library_a_failure(ERROR_MOD_NOT_FOUND))
                }
            }
        }
        Err(error) => Err(ApiFault::from_runtime(error, endpoint)),
    }
}

fn load_library_a_failure(last_error: u32) -> ApiImmediate {
    ApiImmediate::new(Some(0)).with_last_error(last_error)
}

fn handle_get_module_handle_w(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let name_address = required_argument(endpoint, frame, 0, "module name")?;
    // A null module name is the documented request for the module handle of the file used to
    // create the calling process.
    let module = if name_address == 0 {
        runtime.main_module().map(Some)
    } else {
        let name = read_utf16_nul(
            endpoint,
            &runtime.memory,
            GuestAddress(name_address),
            MAX_MODULE_NAME_CODE_UNITS,
        )?;
        runtime.module_by_name(&name)
    };
    match module {
        Ok(Some(module)) => Ok(ApiImmediate::new(Some(module.handle))),
        Ok(None) => Ok(get_module_handle_w_failure(ERROR_MOD_NOT_FOUND)),
        Err(error) => Err(ApiFault::from_runtime(error, endpoint)),
    }
}

fn get_module_handle_w_failure(last_error: u32) -> ApiImmediate {
    ApiImmediate::new(Some(0)).with_last_error(last_error)
}

fn handle_get_proc_address(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let module_handle = required_argument(endpoint, frame, 0, "module handle")?;
    let name_address = required_argument(endpoint, frame, 1, "procedure name")?;
    let Some(module) = runtime
        .module_option(module_handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
    else {
        return Ok(get_proc_address_failure(ERROR_MOD_NOT_FOUND));
    };
    // A procedure name whose high-order word is zero is an ordinal, not a string address.
    let lookup = if name_address <= u32::from(u16::MAX) {
        ExportLookup::Ordinal {
            ordinal: name_address,
        }
    } else {
        let name = read_ascii_nul(
            endpoint,
            &runtime.memory,
            GuestAddress(name_address),
            MAX_EXPORT_NAME_BYTES,
        )?;
        if name.is_empty() || !name.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
            return Ok(get_proc_address_failure(ERROR_PROC_NOT_FOUND));
        }
        ExportLookup::Name { name }
    };
    if module.runtime_system_module {
        let export = match lookup {
            ExportLookup::Name { name } => {
                runtime.dynamic_endpoint_by_name(&module.canonical_name, &name)
            }
            ExportLookup::Ordinal { ordinal } => {
                runtime.dynamic_endpoint_by_ordinal(&module.canonical_name, ordinal)
            }
        };
        return Ok(match export {
            Some(export) => ApiImmediate::new(Some(export.address.0)),
            None => get_proc_address_failure(ERROR_PROC_NOT_FOUND),
        });
    }
    let export = runtime
        .resolve_guest_export(&module.canonical_name, lookup)
        .map_err(|error| ApiFault::new(error.to_string()))?;
    Ok(match export {
        Some(export) => ApiImmediate::new(Some(export.address.0)),
        None => get_proc_address_failure(ERROR_PROC_NOT_FOUND),
    })
}

fn get_proc_address_failure(last_error: u32) -> ApiImmediate {
    ApiImmediate::new(Some(0)).with_last_error(last_error)
}

fn handle_tls_alloc(
    endpoint: &ApiEndpointIdentity,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let index = runtime
        .allocate_tls_index()
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let immediate = ApiImmediate::new(Some(index));
    Ok(if index == TLS_OUT_OF_INDEXES {
        immediate.with_last_error(ERROR_NOT_ENOUGH_MEMORY)
    } else {
        immediate
    })
}

fn handle_tls_get_value(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let index = required_argument(endpoint, frame, 0, "TLS index")?;
    if !runtime.tls_index_available(index) {
        return Ok(ApiImmediate::new(Some(0)).with_last_error(ERROR_INVALID_PARAMETER));
    }
    let value = runtime
        .tls_value_or_unset(index)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(value.unwrap_or(0))).with_last_error(ERROR_SUCCESS))
}

fn handle_tls_set_value(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let index = required_argument(endpoint, frame, 0, "TLS index")?;
    let value = required_argument(endpoint, frame, 1, "TLS value")?;
    if !runtime.tls_index_available(index) {
        return Ok(
            ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_INVALID_PARAMETER)
        );
    }
    runtime
        .set_tls_value(index, value)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(u32::from(true))))
}

fn handle_encode_pointer(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let source = required_argument(endpoint, frame, 0, "pointer source")?;
    let token = runtime.encode_pointer(source);
    Ok(ApiImmediate::new(Some(token)))
}

fn handle_decode_pointer(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let token = required_argument(endpoint, frame, 0, "encoded pointer")?;
    let source = runtime.decode_pointer(token);
    Ok(ApiImmediate::new(Some(source)))
}

// Dispatch by imported symbol. The ABI boundary has already checked the call frame.
pub(crate) fn dispatch_kernel32_api(
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
    cpu: Option<(crate::api::X86CpuContext, GuestAddress)>,
) -> ApiHandlerResult {
    let endpoint = &frame.endpoint;
    let result = match &endpoint.symbol {
        crate::api::ApiSymbol::Name { name }
            if endpoint.module.eq_ignore_ascii_case("kernel32.dll") =>
        {
            if name == "RaiseException" {
                return handle_raise_exception(endpoint, frame, runtime, cpu);
            }
            if name == "RtlUnwind" {
                return handle_rtl_unwind(endpoint, frame, runtime);
            }
            if name == "WaitForMultipleObjects" {
                return handle_wait_for_multiple_objects(endpoint, frame, runtime);
            }
            if name == "ReadFile" {
                return handle_read_file_basic(endpoint, frame, runtime);
            }
            if name == "CreateFileA" {
                return handle_create_file_a_basic(endpoint, frame, runtime);
            }
            if name == "WriteFile" {
                return handle_write_file_basic(endpoint, frame, runtime);
            }
            if name == "CreateDirectoryA" {
                return handle_create_directory_a_basic(endpoint, frame, runtime);
            }
            if name == "DeleteFileA" {
                return handle_delete_file_a_basic(endpoint, frame, runtime);
            }
            if name == "GetFileAttributesA" {
                return handle_get_file_attributes_a_opfs(endpoint, frame, runtime);
            }
            match name.as_str() {
                "FlsAlloc" => handle_fls_alloc(endpoint, frame, runtime),
                "FlsFree" => handle_fls_free(endpoint, frame, runtime),
                "FlsGetValue" => handle_fls_get_value(endpoint, frame, runtime),
                "FlsSetValue" => handle_fls_set_value(endpoint, frame, runtime),
                "CreateThread" => handle_create_thread_basic(endpoint, frame, runtime),
                "ResumeThread" => handle_resume_thread(endpoint, frame, runtime),
                "SetThreadPriority" => set_thread_priority_basic(endpoint, frame, runtime),
                "ExitThread" => handle_exit_thread(endpoint, frame, runtime),
                "WaitForSingleObject" => {
                    return handle_wait_for_single_object(endpoint, frame, runtime);
                }
                "WaitForSingleObjectEx" => {
                    return handle_wait_for_single_object_ex_basic(endpoint, frame, runtime);
                }
                "GetExitCodeThread" => handle_get_exit_code_thread(endpoint, frame, runtime),
                "CloseHandle" => handle_close_handle_basic(endpoint, frame, runtime),
                "GetLastError" => handle_get_last_error(endpoint, runtime),
                "SetLastError" => handle_set_last_error(endpoint, frame),
                "GetCurrentProcessId" => Ok(ApiImmediate::new(Some(runtime.process_id().0))),
                "GetCurrentThreadId" => Ok(ApiImmediate::new(Some(runtime.current_thread_id().0))),
                "GetCurrentThread" => handle_get_current_thread(endpoint, runtime),
                "GetSystemTimeAsFileTime" => {
                    handle_get_system_time_as_file_time(endpoint, frame, runtime)
                }
                "GetSystemTime" => handle_get_system_time(endpoint, frame, runtime),
                "Sleep" => return handle_sleep_basic(endpoint, frame, runtime),
                "GetTickCount" => handle_get_tick_count(endpoint, runtime),
                "QueryPerformanceCounter" => {
                    handle_query_performance_counter_basic(endpoint, frame, runtime)
                }
                "QueryPerformanceFrequency" => {
                    handle_query_performance_frequency(endpoint, frame, runtime)
                }
                "GetStartupInfoW" => handle_get_startup_info_w(endpoint, frame, runtime),
                "HeapSetInformation" => handle_heap_set_information(endpoint, frame, runtime),
                "HeapCreate" => handle_heap_create(endpoint, frame, runtime),
                "HeapAlloc" => handle_heap_alloc_basic(endpoint, frame, runtime),
                "VirtualAlloc" => handle_virtual_alloc_basic(endpoint, frame, runtime),
                "VirtualFree" => handle_virtual_free_basic(endpoint, frame, runtime),
                "HeapReAlloc" => handle_heap_realloc_basic(endpoint, frame, runtime),
                "HeapFree" => handle_heap_free_basic(endpoint, frame, runtime),
                "HeapSize" => handle_heap_size_basic(endpoint, frame, runtime),
                "IsProcessorFeaturePresent" => {
                    handle_is_processor_feature_present_basic(endpoint, frame)
                }
                "SetUnhandledExceptionFilter" => {
                    handle_set_unhandled_exception_filter(endpoint, frame, runtime)
                }
                "GetStdHandle" => handle_get_std_handle_basic(endpoint, frame, runtime),
                "GetFileType" => handle_get_file_type_basic(endpoint, frame, runtime),
                "SetHandleCount" => handle_set_handle_count_basic(endpoint, frame),
                "GetCommandLineA" => handle_get_command_line_a(endpoint, runtime),
                "GetModuleFileNameA" => {
                    handle_get_module_file_name_a_basic(endpoint, frame, runtime)
                }
                "GetLocalTime" => handle_get_local_time(endpoint, frame, runtime),
                "GetSystemInfo" => handle_get_system_info_basic(endpoint, frame),
                "GlobalMemoryStatus" => {
                    handle_global_memory_status_basic(endpoint, frame, &runtime.memory)
                }
                "GetVersionExA" => handle_get_version_ex_a_basic(endpoint, frame, &runtime.memory),
                "GetDriveTypeA" => handle_get_drive_type_a_basic(endpoint, frame, runtime),
                "SetCurrentDirectoryA" => {
                    handle_set_current_directory_a_basic(endpoint, frame, runtime)
                }
                "CreateMutexA" => handle_create_mutex_a_basic(endpoint, frame, runtime),
                "CreateEventA" => handle_create_event_a(endpoint, frame, runtime),
                "SetEvent" => handle_set_event(endpoint, frame, runtime, true),
                "ResetEvent" => handle_set_event(endpoint, frame, runtime, false),
                "GetFileAttributesA" => {
                    handle_get_file_attributes_a_basic(endpoint, frame, runtime)
                }
                "FindFirstFileA" => handle_find_first_file_a_basic(endpoint, frame, runtime),
                "FindNextFileA" => handle_find_next_file_a_basic(endpoint, frame, runtime),
                "FindClose" => handle_find_close_basic(endpoint, frame, runtime),
                "GetFileSize" => handle_get_file_size_basic(endpoint, frame, runtime),
                "FlushFileBuffers" => handle_flush_file_buffers_basic(endpoint, frame, runtime),
                "SetFilePointer" => handle_set_file_pointer_basic(endpoint, frame, runtime),
                "CreateWaitableTimerA" => {
                    handle_create_waitable_timer_a_basic(endpoint, frame, runtime)
                }
                "SetWaitableTimer" => handle_set_waitable_timer_basic(endpoint, frame, runtime),
                "SetThreadAffinityMask" => {
                    handle_set_thread_affinity_mask_basic(endpoint, frame, runtime)
                }
                "GetEnvironmentStringsW" => handle_get_environment_strings_w(endpoint, runtime),
                "WideCharToMultiByte" => {
                    handle_wide_char_to_multi_byte_basic(endpoint, frame, &runtime.memory)
                }
                "MultiByteToWideChar" => {
                    handle_multi_byte_to_wide_char_basic(endpoint, frame, &runtime.memory)
                }
                "GetACP" => Ok(ApiImmediate::new(Some(WINDOWS_932_CODE_PAGE))),
                "GetSystemDefaultLangID" => Ok(ApiImmediate::new(Some(0x0411))),
                "GetCPInfo" => handle_get_cp_info_basic(endpoint, frame),
                "GetStringTypeW" => {
                    handle_get_string_type_w_basic(endpoint, frame, &runtime.memory)
                }
                "LCMapStringW" => handle_lc_map_string_w_basic(endpoint, frame, &runtime.memory),
                "FreeEnvironmentStringsW" => {
                    handle_free_environment_strings_w(endpoint, frame, runtime)
                }
                "GetModuleHandleW" => handle_get_module_handle_w(endpoint, frame, runtime),
                "LoadLibraryA" => handle_load_library_a_basic(endpoint, frame, runtime),
                "GetProcAddress" => handle_get_proc_address(endpoint, frame, runtime),
                "TlsAlloc" => handle_tls_alloc(endpoint, runtime),
                "TlsGetValue" => handle_tls_get_value(endpoint, frame, runtime),
                "TlsSetValue" => handle_tls_set_value(endpoint, frame, runtime),
                "EncodePointer" => handle_encode_pointer(endpoint, frame, runtime),
                "DecodePointer" => handle_decode_pointer(endpoint, frame, runtime),
                "InitializeCriticalSectionAndSpinCount" => {
                    handle_initialize_critical_section_and_spin_count(endpoint, frame, runtime)
                }
                "InitializeCriticalSection" => {
                    handle_initialize_critical_section_basic(endpoint, frame, runtime)
                }
                "EnterCriticalSection" => handle_enter_critical_section(endpoint, frame, runtime),
                "LeaveCriticalSection" => handle_leave_critical_section(endpoint, frame, runtime),
                "DeleteCriticalSection" => handle_delete_critical_section(endpoint, frame, runtime),
                "InterlockedIncrement" => handle_interlocked_long_update(
                    endpoint,
                    frame,
                    &runtime.memory,
                    1,
                    "InterlockedIncrement",
                ),
                "InterlockedDecrement" => handle_interlocked_long_update(
                    endpoint,
                    frame,
                    &runtime.memory,
                    -1,
                    "InterlockedDecrement",
                ),
                _ => Err(ApiFault::new(format!(
                    "unsupported imported function kernel32.dll!{name}"
                ))),
            }
        }
        _ => Err(ApiFault::new("unsupported imported function")),
    };
    match result {
        Ok(immediate) => ApiHandlerResult::Immediate(immediate),
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn handle_fls_alloc(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let callback_value = required_argument(endpoint, frame, 0, "FLS callback")?;
    let callback = (callback_value != 0).then_some(GuestAddress(callback_value));
    if let Some(address) = callback {
        runtime
            .memory
            .validate_access(address, 1, MemoryAccess::Execute)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    }
    let index = runtime
        .allocate_fls_slot(callback)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let immediate = ApiImmediate::new(Some(index));
    Ok(if index == FLS_OUT_OF_INDEXES {
        immediate.with_last_error(ERROR_NOT_ENOUGH_MEMORY)
    } else {
        immediate
    })
}

fn handle_fls_free(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let index = required_argument(endpoint, frame, 0, "FLS index")?;
    if !runtime.fls_index_allocated(index) {
        return Ok(
            ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_INVALID_PARAMETER)
        );
    }
    if !runtime
        .free_fls_slot(index)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
    {
        return Err(ApiFault::new(
            "FlsFree needs a guest callback for an active FLS value",
        ));
    }
    Ok(ApiImmediate::new(Some(u32::from(true))))
}

fn handle_fls_get_value(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let index = required_argument(endpoint, frame, 0, "FLS index")?;
    if !runtime.fls_index_allocated(index) {
        return Ok(ApiImmediate::new(Some(0)).with_last_error(ERROR_INVALID_PARAMETER));
    }
    let value = runtime
        .fls_value_or_unset(index)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(value.unwrap_or(0))).with_last_error(ERROR_SUCCESS))
}

fn handle_fls_set_value(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let index = required_argument(endpoint, frame, 0, "FLS index")?;
    let value = required_argument(endpoint, frame, 1, "FLS value")?;
    if !runtime.fls_index_allocated(index) {
        return Ok(
            ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_INVALID_PARAMETER)
        );
    }
    runtime
        .set_fls_value(index, value)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(u32::from(true))))
}

fn handle_interlocked_long_update(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    memory: &GuestMemory,
    delta: i32,
    operation: &str,
) -> Result<ApiImmediate, ApiFault> {
    let address = GuestAddress(required_argument(endpoint, frame, 0, "LONG target")?);
    if address.0 == 0 || !address.0.is_multiple_of(4) {
        return Err(ApiFault::new(
            "Interlocked LONG target requires a nonnull DWORD-aligned address",
        ));
    }
    memory
        .validate_access(address, size_of::<i32>(), MemoryAccess::Write)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let bytes = memory
        .read(address, size_of::<i32>())
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let value = i32::from_le_bytes(bytes.try_into().map_err(|_| {
        ApiFault::new(format!(
            "checked {operation} target did not contain one LONG"
        ))
    })?);
    let updated = value.wrapping_add(delta);
    Ok(ApiImmediate::new(Some(updated as u32))
        .with_guest_write(address, updated.to_le_bytes().to_vec()))
}

fn handle_initialize_critical_section_and_spin_count(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let address = GuestAddress(required_argument(
        endpoint,
        frame,
        0,
        "critical section storage",
    )?);
    let spin_count = required_argument(endpoint, frame, 1, "critical section spin count")?;
    runtime
        .memory
        .validate_access(
            address,
            X86_CRITICAL_SECTION_BYTE_LENGTH,
            MemoryAccess::Write,
        )
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let section = runtime
        .initialize_critical_section(address, spin_count)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(u32::from(true)))
        .with_guest_write(address, x86_critical_section_bytes(section)))
}

fn handle_initialize_critical_section_basic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let address = GuestAddress(required_argument(
        endpoint,
        frame,
        0,
        "critical section storage",
    )?);
    // Fixed RTL_CRITICAL_SECTION default spin state and initialization flag.
    runtime
        .memory
        .validate_access(
            address,
            X86_CRITICAL_SECTION_BYTE_LENGTH,
            MemoryAccess::Write,
        )
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let section = runtime
        .initialize_critical_section_with_spin_state(address, 0x0200_07d0)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(None).with_guest_write(address, x86_critical_section_bytes(section)))
}

fn handle_enter_critical_section(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let address = GuestAddress(required_argument(
        endpoint,
        frame,
        0,
        "critical section storage",
    )?);
    let section = runtime
        .critical_section(address)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    validate_x86_critical_section(endpoint, &runtime.memory, address, section)?;
    let current_thread_id = runtime.current_thread_id();
    if let Some(owner) = runtime
        .critical_section(address)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
        .owning_thread_id
        && owner != current_thread_id
    {
        runtime
            .wait_current_thread_for_critical_section(address)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        let section = runtime
            .critical_section(address)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        return Ok(ApiImmediate::new(None)
            .with_guest_write(address, x86_critical_section_bytes(section))
            .with_execution_boundary(RuntimeApiBoundaryDisposition::BlockCurrentThread));
    }
    let section = runtime
        .enter_critical_section(address)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(None).with_guest_write(address, x86_critical_section_bytes(section)))
}

fn handle_leave_critical_section(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let address = GuestAddress(required_argument(
        endpoint,
        frame,
        0,
        "critical section storage",
    )?);
    let section = runtime
        .critical_section(address)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    validate_x86_critical_section(endpoint, &runtime.memory, address, section)?;
    let section = runtime
        .leave_critical_section(address)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(None).with_guest_write(address, x86_critical_section_bytes(section)))
}

fn handle_delete_critical_section(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let address = GuestAddress(required_argument(
        endpoint,
        frame,
        0,
        "critical section storage",
    )?);
    let section = runtime
        .critical_section(address)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    validate_x86_critical_section(endpoint, &runtime.memory, address, section)?;
    runtime
        .delete_critical_section(address)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(
        ApiImmediate::new(None)
            .with_guest_write(address, vec![0; X86_CRITICAL_SECTION_BYTE_LENGTH]),
    )
}

fn validate_x86_critical_section(
    endpoint: &ApiEndpointIdentity,
    memory: &GuestMemory,
    address: GuestAddress,
    section: &RuntimeCriticalSection,
) -> Result<(), ApiFault> {
    memory
        .validate_access(
            address,
            X86_CRITICAL_SECTION_BYTE_LENGTH,
            MemoryAccess::Write,
        )
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let expected = x86_critical_section_bytes(section);
    let actual = memory
        .read(address, X86_CRITICAL_SECTION_BYTE_LENGTH)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    if actual != expected {
        return Err(ApiFault::new(
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
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    index: usize,
    label: &str,
) -> Result<u32, ApiFault> {
    frame.argument_u32(index).ok_or_else(|| {
        ApiFault::new(format!(
            "{:?} handler did not receive its {label}",
            endpoint.symbol
        ))
    })
}

fn read_ascii_nul(
    endpoint: &ApiEndpointIdentity,
    memory: &GuestMemory,
    address: GuestAddress,
    maximum_bytes: usize,
) -> Result<String, ApiFault> {
    let mut bytes = Vec::new();
    for offset in 0..maximum_bytes {
        let current = add_guest_offset(endpoint, address, offset)?;
        let byte = memory
            .read(current, 1)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?[0];
        if byte == 0 {
            return String::from_utf8(bytes)
                .map_err(|_| ApiFault::new("guest export name is not ASCII"));
        }
        if !byte.is_ascii() {
            return Err(ApiFault::new("guest export name is not ASCII"));
        }
        bytes.push(byte);
    }
    Err(ApiFault::new("guest export name exceeds its bounded read"))
}

fn read_load_library_a_name(
    endpoint: &ApiEndpointIdentity,
    memory: &GuestMemory,
    address: GuestAddress,
    maximum_bytes: usize,
) -> Result<String, ApiFault> {
    let mut bytes = Vec::new();
    for offset in 0..maximum_bytes {
        let current = add_guest_offset(endpoint, address, offset)?;
        let byte = memory
            .read(current, 1)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?[0];
        if byte == 0 {
            return String::from_utf8(bytes).map_err(|_| {
                ApiFault::new("LoadLibraryA currently accepts only ASCII module names")
            });
        }
        if !byte.is_ascii() {
            return Err(ApiFault::new(
                "LoadLibraryA currently accepts only ASCII module names",
            ));
        }
        bytes.push(byte);
    }
    Err(ApiFault::new(
        "LoadLibraryA module name exceeds its bounded read",
    ))
}

fn read_utf16_nul(
    endpoint: &ApiEndpointIdentity,
    memory: &GuestMemory,
    address: GuestAddress,
    maximum_code_units: usize,
) -> Result<String, ApiFault> {
    let mut code_units = Vec::new();
    for index in 0..maximum_code_units {
        let offset = index
            .checked_mul(2)
            .ok_or_else(|| ApiFault::new("UTF-16 string offset overflowed host accounting"))?;
        let current = add_guest_offset(endpoint, address, offset)?;
        let bytes = memory
            .read(current, 2)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        let code_unit = u16::from_le_bytes([bytes[0], bytes[1]]);
        if code_unit == 0 {
            return String::from_utf16(&code_units)
                .map_err(|_| ApiFault::new("guest module name is not valid UTF-16"));
        }
        code_units.push(code_unit);
    }
    Err(ApiFault::new("guest module name exceeds its bounded read"))
}

fn add_guest_offset(
    endpoint: &ApiEndpointIdentity,
    address: GuestAddress,
    offset: usize,
) -> Result<GuestAddress, ApiFault> {
    let offset = u32::try_from(offset)
        .map_err(|_| ApiFault::new("guest string offset does not fit x86 address accounting"))?;
    address
        .0
        .checked_add(offset)
        .map(GuestAddress)
        .ok_or_else(|| {
            ApiFault::new(format!(
                "{:?} string argument crosses the x86 address limit",
                endpoint.symbol
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::{
        OpfsFileOperation, complete_opfs_file_operation, local_file_time, system_time_bytes,
    };

    #[test]
    fn opfs_results_project_windows_failures_without_consuming_invalid_observations() {
        let directory = OpfsFileOperation::CreateDirectory;
        let delete = OpfsFileOperation::DeleteFile { source_missing: 2 };
        let attrs = OpfsFileOperation::Attributes { source_missing: 2 };
        for (operation, observation, value, error) in [
            (directory, 0, 1, 0x4321),
            (directory, 1, 0, 183),
            (directory, 2, 0, 3),
            (delete, 0, 1, 0x4321),
            (delete, 1, 0, 2),
            (delete, 2, 0, 2),
            (OpfsFileOperation::DeleteFile { source_missing: 3 }, 2, 0, 3),
            (attrs, 1, 0x20, 0x4321),
            (attrs, 2, 0x30, 0x4321),
            (attrs, 4, u32::MAX, 2),
            (
                OpfsFileOperation::Attributes { source_missing: 3 },
                4,
                u32::MAX,
                3,
            ),
        ] {
            let result = complete_opfs_file_operation(operation, observation, 0x4321).unwrap();
            assert_eq!(result.eax, Some(value));
            assert_eq!(result.last_error, Some(error));
        }
        for operation in [directory, delete, attrs] {
            assert!(complete_opfs_file_operation(operation, 99, 0x4321).is_err());
        }
        assert!(
            complete_opfs_file_operation(OpfsFileOperation::Attributes { source_missing: 9 }, 1, 0)
                .is_err()
        );
        assert!(
            complete_opfs_file_operation(OpfsFileOperation::DeleteFile { source_missing: 9 }, 0, 0)
                .is_err()
        );
    }

    #[test]
    fn local_time_projection_uses_japan_offset_and_checks_overflow() {
        let utc = 134_282_016_000_000_000;
        assert_eq!(local_file_time(utc).unwrap(), 134_282_340_000_000_000);
        assert_eq!(local_file_time(0).unwrap(), 324_000_000_000);
        assert!(local_file_time(u64::MAX).is_err());
    }

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

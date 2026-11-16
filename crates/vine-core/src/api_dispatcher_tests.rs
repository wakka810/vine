use crate as vine_core;
use crate::api::{
    ApiHandlerResult, GuestCallbackArgument, GuestCallbackCompletionPolicy, GuestCallbackRequest,
    HostRequest,
};
use crate::imports::{ImportThunk, ImportThunkDisposition, ImportThunkId};
use crate::module_graph::RuntimeModuleReferenceOwner;
use crate::runtime::{
    RuntimeDynamicEndpoint, RuntimeHandle, RuntimeObjectKind, RuntimeWaitState,
    RuntimeWaitableTimerArm, RuntimeWaitableTimerExpiration, RuntimeWaitableTimerId,
};
use crate::win32::dispatch_kernel32_api;

use std::cell::Cell;
use std::fs;
use std::path::PathBuf;

use vine_core::{
    ApiDispatcher, ApiEndpointIdentity, ApiImmediate, ApiOwnedTrap, DispatchOutcome,
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestRange, HostCompletion, ImportTarget,
    MemoryLimits, MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind, ModuleSnapshot, PageProtection,
    Pe32Limits, ProcessAddressAllocationPurpose, Runtime, RuntimeApiBoundaryDisposition,
    RuntimeBootstrap, RuntimeClockState, RuntimeModule, RuntimeModuleLoadState, RuntimeProcessId,
    RuntimeSchedulerEvent, RuntimeStartupInfo, RuntimeThreadId, Rva, SchedulerThreadState,
    X86CpuContext, build_pe32_load_plan, complete_kernel32_waitable_timer_pacing,
    install_initial_thread_stack, install_runtime_return_sentinels, materialize_pe32_image,
};

const MEMORY_BASE: GuestAddress = GuestAddress(0x2000_0000);
const STACK_BASE: u32 = 0x2000_0100;
const OUTPUT_BASE: u32 = 0x2000_1000;
const IAT_BASE: u32 = 0x2000_2000;
const ENDPOINT_BASE: u32 = 0x7f00_1000;
const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;
const INITIAL_LAST_ERROR: u32 = 0x1234;
/// `EncodePointer(NULL)` under the controlled process cookie 0xec01_9d3b.
const ENCODED_NULL_TOKEN: u32 = 0xd478_04a0;
/// `EncodePointer(0x100a_9608)` under the same cookie.
const ENCODED_FUNCTION_TOKEN: u32 = 0x06b9_c3a7;
const PERFORMANCE_COUNTER: u64 = 12_345_678_901;
const PERFORMANCE_FREQUENCY: u64 = 10_000_000;

#[test]
fn endpoint_lookup_keeps_owned_unsupported_imports_visible_to_cpu() {
    let endpoint = ApiEndpointIdentity::from_dynamic_endpoint(&RuntimeDynamicEndpoint {
        id: 7,
        address: GuestAddress(ENDPOINT_BASE),
        module: "example.dll".to_owned(),
        name: "Unsupported".to_owned(),
        ordinal: None,
    });
    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap();
    assert_eq!(dispatcher.endpoint(endpoint.address), Some(&endpoint));
    assert!(
        dispatcher
            .endpoint(GuestAddress(endpoint.address.0 + 16))
            .is_none()
    );
    let duplicate_id = ApiEndpointIdentity {
        address: GuestAddress(endpoint.address.0 + 16),
        ..endpoint.clone()
    };
    let _ = ApiDispatcher::from_endpoints([endpoint, duplicate_id]).unwrap_err();
}

#[test]
fn runtime_endpoint_registration_rejects_malformed_bindings_and_can_retry() {
    let endpoint = ApiEndpointIdentity::from_dynamic_endpoint(&RuntimeDynamicEndpoint {
        id: 7,
        address: GuestAddress(ENDPOINT_BASE),
        module: "kernel32.dll".to_owned(),
        name: "GetACP".to_owned(),
        ordinal: None,
    });
    let mut malformed = endpoint.clone();
    malformed.iat_address = Some(GuestAddress(IAT_BASE));
    assert!(ApiDispatcher::from_endpoints([malformed]).is_err());
    let mut malformed = endpoint.clone();
    malformed.binding = crate::api::ApiEndpointBinding::ImportAddressTable;
    malformed.disposition = Some(ImportThunkDisposition::DeclaredRuntimeExport);
    assert!(ApiDispatcher::from_endpoints([malformed]).is_err());
    let mut malformed = endpoint.clone();
    malformed.binding = crate::api::ApiEndpointBinding::ImportAddressTable;
    malformed.iat_address = Some(GuestAddress(IAT_BASE));
    assert!(ApiDispatcher::from_endpoints([malformed]).is_err());
    let alias = ApiEndpointIdentity {
        id: ImportThunkId(8),
        ..endpoint.clone()
    };
    assert!(ApiDispatcher::from_endpoints([endpoint.clone(), alias]).is_err());
    let alias = ApiEndpointIdentity {
        address: GuestAddress(ENDPOINT_BASE + 16),
        ..endpoint.clone()
    };
    assert!(ApiDispatcher::from_endpoints([endpoint.clone(), alias]).is_err());
    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap();
    assert_eq!(dispatcher.endpoint(endpoint.address), Some(&endpoint));
    assert!(
        dispatcher
            .endpoint(GuestAddress(ENDPOINT_BASE + 16))
            .is_none()
    );
}

#[test]
fn runtime_dispatch_uses_windows_module_case_and_exact_name_or_ordinal() {
    let (mut memory, owner) = initialized_memory();
    write_frame(
        &mut memory,
        owner,
        GuestAddress(STACK_BASE),
        &[0x1000_0100, 0, 0, 0],
    );
    memory.seal_owner(owner).unwrap();
    let reflection = memory.pending_reflection().unwrap();
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
    let mut runtime = controlled_runtime(memory, controlled_startup_info());
    let memory_before = runtime.memory.stats();
    let last_error = runtime.current_last_error().unwrap();
    let tick = runtime.clock().tick_count;
    for (module, name, ordinal, expected) in [
        ("KeRnEl32.dll", "GetACP", None, Some(932)),
        ("kernel32.dll", "GetVersion", None, Some(0x4a65_000a)),
        ("kernel32.dll", "getacp", None, None),
        ("example.dll", "GetACP", None, None),
        ("WINMM.dll", "timeGetTime", None, Some(tick)),
        ("dsound.dll", "DirectSoundCreate8", None, None),
        (
            "DSOUND.dll",
            "diagnostic-label",
            Some(crate::dsound::DSOUND_DIRECT_SOUND_CREATE_8_ORDINAL),
            Some(crate::dsound::DSOUND_E_INVALIDARG.bits()),
        ),
        ("DSOUND.dll", "diagnostic-label", Some(12), None),
    ] {
        let mut endpoint = ApiEndpointIdentity::from_dynamic_endpoint(&RuntimeDynamicEndpoint {
            id: 7,
            address: GuestAddress(ENDPOINT_BASE),
            module: module.to_owned(),
            name: name.to_owned(),
            ordinal: None,
        });
        if let Some(ordinal) = ordinal {
            endpoint.symbol = crate::api::ApiSymbol::Ordinal { ordinal };
        }
        let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap();
        assert_eq!(dispatcher.endpoint(endpoint.address), Some(&endpoint));
        let result = runtime.dispatch_api_call(
            &dispatcher,
            owned_trap(&endpoint, STACK_BASE, 0x1000_0100),
            crate::callbacks::CALLBACK_RETURN_ARENA_BASE,
            None,
        );
        match (expected, result) {
            (Some(value), DispatchOutcome::Immediate(completion)) => {
                assert_eq!(completion.eax, Some(value));
                assert_eq!(runtime.current_last_error().unwrap(), last_error);
            }
            (None, DispatchOutcome::Fault(fault)) => {
                assert!(!fault.message.is_empty())
            }
            (expected, result) => {
                panic!("unexpected Windows dispatch for {module}!{name}: {expected:?}, {result:?}")
            }
        }
        assert_eq!(runtime.memory.stats(), memory_before);
        assert_eq!(runtime.current_last_error().unwrap(), last_error);
        assert_eq!(runtime.clock().tick_count, tick);
        assert!(runtime.com().is_empty());
        assert!(runtime.audio_devices().is_empty());
    }
}

#[test]
fn cpu_backed_dispatch_rechecks_iat_and_return_address_before_calling_handler() {
    let (mut memory, owner) = initialized_memory();
    let (dispatcher, endpoints, frames) = dispatcher_fixture(&mut memory, owner);
    let mut runtime = controlled_runtime(memory, controlled_startup_info());
    let x86_context = |esp, eip| X86CpuContext {
        eax: 0,
        ecx: 0,
        edx: 0,
        ebx: 0,
        esp,
        ebp: 0,
        esi: 0,
        edi: 0,
        eip,
        eflags: 0x202,
        es: 0,
        cs: 0,
        ss: 0,
        ds: 0,
        fs: 0,
        gs: 0,
    };
    let handler_called = Cell::new(false);
    let dispatch = |runtime: &mut Runtime| {
        dispatcher.dispatch_owned(
            owned_trap(&endpoints[0], frames[0].0, frames[0].1[0]),
            runtime,
            Some((
                x86_context(frames[0].0, endpoints[0].address.0),
                GuestAddress(0),
            )),
            |_, _, _| {
                handler_called.set(true);
                ApiHandlerResult::Immediate(ApiImmediate::new(Some(123)))
            },
        )
    };
    assert!(matches!(
        dispatch(&mut runtime),
        DispatchOutcome::Immediate(_)
    ));
    handler_called.set(false);
    runtime
        .memory
        .write(endpoints[0].iat_address.unwrap(), &0_u32.to_le_bytes())
        .expect("corrupt bound IAT");
    let memory_before = runtime.memory.stats();
    assert!(matches!(dispatch(&mut runtime), DispatchOutcome::Fault(_)));
    assert!(!handler_called.get());
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    assert_eq!(runtime.current_thread_id(), RuntimeThreadId(THREAD_ID));
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.memory.stats(), memory_before);
    runtime
        .memory
        .write(
            endpoints[0].iat_address.unwrap(),
            &endpoints[0].address.0.to_le_bytes(),
        )
        .expect("restore bound IAT");
    runtime
        .memory
        .write(GuestAddress(frames[0].0), &0_u32.to_le_bytes())
        .expect("corrupt guest return address");
    let memory_before = runtime.memory.stats();
    assert!(matches!(dispatch(&mut runtime), DispatchOutcome::Fault(_)));
    assert!(!handler_called.get());
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    assert_eq!(runtime.current_thread_id(), RuntimeThreadId(THREAD_ID));
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.memory.stats(), memory_before);
}

#[test]
fn runtime_state_drives_five_api_shapes_and_nested_scheduler_state() {
    let (mut memory, owner) = initialized_memory();
    let (dispatcher, endpoints, frames) = dispatcher_fixture(&mut memory, owner);
    let startup_bytes = controlled_startup_info();
    let mut runtime = controlled_runtime(memory, startup_bytes.clone());

    runtime
        .register_dynamic_endpoint(RuntimeDynamicEndpoint {
            id: 2,
            address: GuestAddress(0x7f02_0010),
            module: "kernel32.dll".to_owned(),
            name: "FlsGetValue".to_owned(),
            ordinal: None,
        })
        .expect("register dynamic endpoint");
    let tls_index = runtime.allocate_tls_index().expect("allocate TLS index");
    assert_eq!(tls_index, 0);

    runtime.set_heap_termination_on_corruption(true);
    assert_eq!(
        runtime
            .allocate_private_heap(0x1000, 0)
            .expect("create private heap through the production allocator"),
        0x6801_0001
    );
    runtime
        .load_module(RuntimeModule {
            handle: 0x4d01_0001,
            requested_name: "KERNEL32.DLL".to_owned(),
            canonical_name: "kernel32.dll".to_owned(),
            runtime_system_module: true,
        })
        .expect("load logical module");

    let process = dispatch_owned(
        &dispatcher,
        &mut runtime,
        &endpoints[0],
        frames[0].0,
        frames[0].1[0],
    );
    assert_eq!(process.eax, Some(PROCESS_ID));
    assert_eq!(process.stack_pointer, GuestAddress(STACK_BASE + 4));
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);

    let counter = dispatch(
        &dispatcher,
        &mut runtime,
        &endpoints[1],
        frames[1].0,
        frames[1].1[0],
        4,
    );
    assert_eq!(counter.eax, Some(1));
    acknowledge_pending_delta(&mut runtime.memory);
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(OUTPUT_BASE), 8)
            .expect("counter output"),
        PERFORMANCE_COUNTER.to_le_bytes()
    );

    for name in ["GetStartupInfoA", "GetStartupInfoW"] {
        let mut endpoint = endpoints[2].clone();
        endpoint.symbol = crate::api::ApiSymbol::Name {
            name: name.to_owned(),
        };
        let startup_dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap();
        let startup = dispatch(
            &startup_dispatcher,
            &mut runtime,
            &endpoint,
            frames[2].0,
            frames[2].1[0],
            4,
        );
        assert_eq!(startup.eax, None);
        acknowledge_pending_delta(&mut runtime.memory);
        assert_eq!(
            runtime
                .memory
                .read(GuestAddress(OUTPUT_BASE + 0x100), startup_bytes.len())
                .expect("startup output"),
            startup_bytes
        );
    }

    let tls = dispatch(
        &dispatcher,
        &mut runtime,
        &endpoints[3],
        frames[3].0,
        frames[3].1[0],
        8,
    );
    assert_eq!(tls.stack_pointer, GuestAddress(STACK_BASE + 0x60 + 12));
    assert_eq!(
        runtime.tls_value(tls_index).expect("current TLS value"),
        0x7f02_0010
    );

    let encoded_null = dispatch(
        &dispatcher,
        &mut runtime,
        &endpoints[4],
        frames[4].0,
        frames[4].1[0],
        4,
    );
    assert_eq!(encoded_null.eax, Some(ENCODED_NULL_TOKEN));

    let encoded_function = dispatch(
        &dispatcher,
        &mut runtime,
        &endpoints[4],
        frames[6].0,
        frames[6].1[0],
        4,
    );
    assert_eq!(encoded_function.eax, Some(ENCODED_FUNCTION_TOKEN));
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    assert!(runtime.process().heap_termination_on_corruption());
    assert_eq!(runtime.heap(0x6801_0001).unwrap().initial_size, 0x1000);
    assert_eq!(
        runtime.module(0x4d01_0001).unwrap().canonical_name,
        "kernel32.dll"
    );

    let output_before = runtime
        .memory
        .read(GuestAddress(OUTPUT_BASE), 8)
        .expect("output before rejected write");
    let rejected = dispatch_with_handler(
        &dispatcher,
        &mut runtime,
        &endpoints[1],
        frames[1].0,
        frames[1].1[0],
        4,
        ApiImmediate::new(Some(u32::from(true)))
            .with_guest_write(GuestAddress(0xdead_0000), vec![0xff; 8]),
    );
    assert!(matches!(rejected, DispatchOutcome::Fault(_)));
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(OUTPUT_BASE), 8)
            .expect("output after rejected write"),
        output_before
    );

    let handler_called = Cell::new(false);
    let unsupported = {
        dispatcher.dispatch_owned(
            owned_trap(&endpoints[5], frames[5].0, frames[5].1[0]),
            &mut runtime,
            None,
            |_, _, _| {
                handler_called.set(true);
                ApiHandlerResult::Immediate(ApiImmediate::new(Some(0)))
            },
        )
    };
    assert!(!handler_called.get());
    assert!(matches!(unsupported, DispatchOutcome::Fault(_)));

    let outer = runtime
        .push_callback_frame(GuestAddress(0x1000_1000), GuestAddress(0x7f01_0000))
        .expect("outer callback");
    assert_eq!(outer.thread_id, RuntimeThreadId(THREAD_ID));
    assert_eq!(outer.entry_last_error, INITIAL_LAST_ERROR);
    runtime.set_current_last_error(0xabcd).unwrap();
    let inner = runtime
        .push_callback_frame(GuestAddress(0x1000_2000), GuestAddress(0x7f01_0010))
        .expect("nested callback");
    assert_eq!(inner.entry_last_error, 0xabcd);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::InCallback { depth: 2 }
    );
    runtime
        .pop_callback_frame(outer.id)
        .expect_err("callbacks return in LIFO order");

    let request = runtime
        .begin_host_request("browser.storage.read")
        .expect("defer host service from callback");
    assert_eq!(request.thread_id, RuntimeThreadId(THREAD_ID));
    assert_eq!(
        request.resume_state,
        SchedulerThreadState::InCallback { depth: 2 }
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::WaitingHost {
            request_id: request.request_id
        }
    );
    let completion = runtime
        .complete_host_request(request.request_id)
        .expect("complete host service");
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::InCallback { depth: 2 }
    );
    assert_eq!(
        runtime.pop_scheduler_event(),
        Some(RuntimeSchedulerEvent::HostCompleted(completion))
    );
    runtime
        .pop_callback_frame(inner.id)
        .expect("return nested callback");
    runtime
        .pop_callback_frame(outer.id)
        .expect("return outer callback");
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.current_thread_id(), RuntimeThreadId(THREAD_ID));
    assert_eq!(runtime.current_last_error().unwrap(), 0xabcd);

    runtime
        .push_callback_frame(GuestAddress(0x1000_3000), GuestAddress(0x7f01_0000))
        .expect("cancellation outer callback");
    runtime
        .push_callback_frame(GuestAddress(0x1000_4000), GuestAddress(0x7f01_0010))
        .expect("cancellation nested callback");
    let cancelled_request = runtime
        .begin_host_request("browser.storage.write")
        .expect("cancellation host request");
    runtime
        .request_current_thread_exit(0xc000_013a)
        .expect("request current thread exit");
    runtime
        .complete_current_thread_exit()
        .expect("complete current thread exit");
    assert!(runtime.scheduler().host_requests().is_empty());
    assert!(
        runtime
            .current_thread_state()
            .unwrap()
            .callback_frames
            .is_empty()
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Terminated {
            exit_code: 0xc000_013a
        }
    );
    runtime
        .complete_host_request(cancelled_request.request_id)
        .expect_err("cancelled host request is retired");
}

#[test]
fn checked_indirect_writes_complete_and_bad_output_rolls_back() {
    const INDIRECT_ADDRESS: GuestAddress = GuestAddress(OUTPUT_BASE + 0x200);
    const INDIRECT_BYTES: &[u8] = &[0x31, 0x41, 0x59, 0x26];

    let (mut memory, owner) = initialized_memory();
    let (_fixture_dispatcher, endpoints, frames) = dispatcher_fixture(&mut memory, owner);
    let endpoint = endpoints[1].clone();
    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).expect("dispatcher");
    let mut runtime = controlled_runtime(memory, controlled_startup_info());
    let pending = dispatch_with_handler(
        &dispatcher,
        &mut runtime,
        &endpoint,
        frames[1].0,
        frames[1].1[0],
        4,
        ApiImmediate::new(Some(u32::from(true)))
            .with_guest_write(
                GuestAddress(OUTPUT_BASE),
                PERFORMANCE_COUNTER.to_le_bytes().to_vec(),
            )
            .with_guest_write(INDIRECT_ADDRESS, INDIRECT_BYTES.to_vec()),
    );
    assert!(matches!(pending, DispatchOutcome::Immediate(_)));
    acknowledge_pending_delta(&mut runtime.memory);
    assert_eq!(
        runtime
            .memory
            .read(INDIRECT_ADDRESS, INDIRECT_BYTES.len())
            .unwrap(),
        INDIRECT_BYTES
    );

    let output_before_failure = runtime.memory.read(GuestAddress(OUTPUT_BASE), 8).unwrap();
    let memory_before_failure = runtime.memory.stats();
    let last_error_before_failure = runtime.current_last_error().unwrap();
    let failed = dispatch_with_handler(
        &dispatcher,
        &mut runtime,
        &endpoint,
        frames[1].0,
        frames[1].1[0],
        4,
        ApiImmediate::new(Some(u32::from(true)))
            .with_last_error(5)
            .with_guest_write(GuestAddress(OUTPUT_BASE), vec![0xff; 8])
            .with_guest_write(GuestAddress(0xffff_fffc), vec![0xff; 8]),
    );
    assert!(matches!(failed, DispatchOutcome::Fault(_)));
    assert_eq!(
        runtime.memory.read(GuestAddress(OUTPUT_BASE), 8).unwrap(),
        output_before_failure
    );
    assert_eq!(runtime.memory.stats(), memory_before_failure);
    assert!(!runtime.memory.has_pending_delta());
    assert_eq!(
        runtime.current_last_error().unwrap(),
        last_error_before_failure
    );
    assert_eq!(runtime.current_thread_id(), RuntimeThreadId(THREAD_ID));
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );

    runtime
        .memory
        .write(INDIRECT_ADDRESS, INDIRECT_BYTES)
        .expect("subsystem-owned indirect write");
    acknowledge_pending_delta(&mut runtime.memory);

    let committed = dispatch_with_handler(
        &dispatcher,
        &mut runtime,
        &endpoint,
        frames[1].0,
        frames[1].1[0],
        4,
        ApiImmediate::new(Some(u32::from(true))).with_guest_write(
            GuestAddress(OUTPUT_BASE),
            PERFORMANCE_COUNTER.to_le_bytes().to_vec(),
        ),
    );
    let DispatchOutcome::Immediate(completion) = committed else {
        panic!("expected committed indirect write completion, got {committed:?}");
    };
    assert_eq!(completion.eax, Some(1));
    acknowledge_pending_delta(&mut runtime.memory);
    assert_eq!(
        runtime
            .memory
            .read(INDIRECT_ADDRESS, INDIRECT_BYTES.len())
            .unwrap(),
        INDIRECT_BYTES
    );
}

#[test]
fn handler_sets_or_preserves_last_error_without_a_catalog_policy() {
    let (mut memory, owner) = initialized_memory();
    let (_, endpoints, frames) = dispatcher_fixture(&mut memory, owner);
    let endpoint = endpoints[1].clone();
    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap();
    let mut runtime = controlled_runtime(memory, controlled_startup_info());
    runtime.set_current_last_error(87).unwrap();

    for (last_error, expected) in [(None, 87), (Some(5), 5), (None, 5)] {
        let mut result = ApiImmediate::new(Some(u32::from(true))).with_guest_write(
            GuestAddress(OUTPUT_BASE),
            PERFORMANCE_COUNTER.to_le_bytes().to_vec(),
        );
        if let Some(value) = last_error {
            result = result.with_last_error(value);
        }
        let outcome = dispatch_with_handler(
            &dispatcher,
            &mut runtime,
            &endpoint,
            frames[1].0,
            frames[1].1[0],
            4,
            result,
        );
        let DispatchOutcome::Immediate(_) = outcome else {
            panic!("expected checked API completion: {outcome:?}");
        };
        assert_eq!(runtime.current_last_error().unwrap(), expected);
        assert_eq!(runtime.current_last_error().unwrap(), expected);
        acknowledge_pending_delta(&mut runtime.memory);
    }
}

#[test]
fn query_performance_frequency_uses_the_validated_clock_authority_atomically() {
    let (mut memory, owner) = initialized_memory();
    let endpoint = ApiEndpointIdentity::from_dynamic_endpoint(&RuntimeDynamicEndpoint {
        id: 99,
        address: GuestAddress(ENDPOINT_BASE + 0x800),
        module: "kernel32.dll".to_owned(),
        name: "QueryPerformanceFrequency".to_owned(),
        ordinal: None,
    });

    let valid_frame = (STACK_BASE, vec![0x1000_0081, OUTPUT_BASE]);
    let null_frame = (STACK_BASE + 0x20, vec![0x1000_0082, 0]);
    memory
        .write_initialization(owner, GuestAddress(OUTPUT_BASE), &[0xa5; 8])
        .expect("initialize frequency output");
    write_frame(
        &mut memory,
        owner,
        GuestAddress(valid_frame.0),
        &valid_frame.1,
    );
    write_frame(
        &mut memory,
        owner,
        GuestAddress(null_frame.0),
        &null_frame.1,
    );
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).expect("dispatcher");
    let mut runtime = controlled_runtime(memory, controlled_startup_info());
    runtime.set_current_last_error(87).unwrap();
    let clock_before = runtime.clock().clone();

    let completion = dispatch_owned(
        &dispatcher,
        &mut runtime,
        &endpoint,
        valid_frame.0,
        valid_frame.1[0],
    );
    assert_eq!(completion.eax, Some(1));
    assert_eq!(completion.stack_pointer, GuestAddress(valid_frame.0 + 8));
    acknowledge_pending_delta(&mut runtime.memory);
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(OUTPUT_BASE), 8)
            .expect("frequency output"),
        PERFORMANCE_FREQUENCY.to_le_bytes()
    );
    assert_eq!(runtime.clock(), &clock_before);
    assert_eq!(runtime.current_thread_id(), RuntimeThreadId(THREAD_ID));
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.current_last_error().unwrap(), 87);

    let memory_before_fault = runtime.memory.stats();
    let output_before_fault = runtime
        .memory
        .read(GuestAddress(OUTPUT_BASE), 8)
        .expect("frequency output before NULL rejection");
    let rejected = {
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, null_frame.0, null_frame.1[0]),
            &mut runtime,
            None,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(rejected, DispatchOutcome::Fault(_)));
    assert_eq!(runtime.clock(), &clock_before);
    assert_eq!(runtime.current_last_error().unwrap(), 87);
    assert_eq!(runtime.current_thread_id(), RuntimeThreadId(THREAD_ID));
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.memory.stats(), memory_before_fault);
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(OUTPUT_BASE), 8)
            .expect("frequency output after NULL rejection"),
        output_before_fault
    );
}

#[test]
fn query_performance_counter_reads_time_and_rejects_null_output_atomically() {
    let (mut memory, owner) = initialized_memory();
    let (dispatcher, endpoints, frames) = dispatcher_fixture(&mut memory, owner);
    let mut runtime = controlled_runtime(memory, controlled_startup_info());
    runtime.set_current_last_error(87).unwrap();

    let first = dispatch_owned(
        &dispatcher,
        &mut runtime,
        &endpoints[1],
        frames[1].0,
        frames[1].1[0],
    );
    assert_eq!(first.eax, Some(1));
    acknowledge_pending_delta(&mut runtime.memory);
    assert_eq!(
        runtime.memory.read(GuestAddress(OUTPUT_BASE), 8).unwrap(),
        PERFORMANCE_COUNTER.to_le_bytes()
    );
    assert_eq!(runtime.clock().performance_counter, PERFORMANCE_COUNTER);

    let second = dispatch_owned(
        &dispatcher,
        &mut runtime,
        &endpoints[1],
        frames[1].0,
        frames[1].1[0],
    );
    assert_eq!(second.eax, Some(1));
    acknowledge_pending_delta(&mut runtime.memory);
    assert_eq!(
        runtime.memory.read(GuestAddress(OUTPUT_BASE), 8).unwrap(),
        PERFORMANCE_COUNTER.to_le_bytes()
    );
    assert_eq!(runtime.clock().performance_counter, PERFORMANCE_COUNTER);
    assert_eq!(runtime.current_last_error().unwrap(), 87);

    let clock_before_fault = runtime.clock().clone();
    let memory_before_fault = runtime.memory.stats();
    let rejected = {
        dispatcher.dispatch_owned(
            owned_trap(&endpoints[1], frames[4].0, frames[4].1[0]),
            &mut runtime,
            None,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(rejected, DispatchOutcome::Fault(_)));
    assert_eq!(runtime.clock(), &clock_before_fault);
    assert_eq!(runtime.current_last_error().unwrap(), 87);
    assert_eq!(runtime.current_thread_id(), RuntimeThreadId(THREAD_ID));
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.memory.stats(), memory_before_fault);
}

#[test]
fn query_performance_counter_reads_scheduled_shared_clock_without_read_progression() {
    let (mut memory, owner) = initialized_memory();
    let (dispatcher, endpoints, frames) = dispatcher_fixture(&mut memory, owner);
    let mut runtime = controlled_runtime(memory, controlled_startup_info());

    for _ in 0..2 {
        let completion = dispatch_owned(
            &dispatcher,
            &mut runtime,
            &endpoints[1],
            frames[1].0,
            frames[1].1[0],
        );
        assert_eq!(completion.eax, Some(1));
        acknowledge_pending_delta(&mut runtime.memory);
        assert_eq!(
            runtime.memory.read(GuestAddress(OUTPUT_BASE), 8).unwrap(),
            PERFORMANCE_COUNTER.to_le_bytes()
        );
    }
    assert_eq!(runtime.timer_time_100ns(), 0);
    assert_eq!(runtime.clock().performance_counter, PERFORMANCE_COUNTER);

    runtime.advance_scheduled_clock_to(25_000).unwrap();
    let completion = dispatch_owned(
        &dispatcher,
        &mut runtime,
        &endpoints[1],
        frames[1].0,
        frames[1].1[0],
    );
    assert_eq!(completion.eax, Some(1));
    acknowledge_pending_delta(&mut runtime.memory);
    assert_eq!(
        runtime.memory.read(GuestAddress(OUTPUT_BASE), 8).unwrap(),
        (PERFORMANCE_COUNTER + 25_000).to_le_bytes()
    );
    assert_eq!(
        runtime.clock().performance_counter,
        PERFORMANCE_COUNTER + 25_000
    );
}

#[test]
fn get_local_time_projects_japan_time_on_both_dispatch_routes() {
    let (mut memory, owner) = initialized_memory();
    write_frame(
        &mut memory,
        owner,
        GuestAddress(STACK_BASE),
        &[0x1000_0580, OUTPUT_BASE],
    );
    let address = GuestAddress(ENDPOINT_BASE + 0x58);
    memory
        .write_initialization(owner, GuestAddress(IAT_BASE), &address.0.to_le_bytes())
        .unwrap();
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(1),
        address,
        requester_owner: owner,
        module: "kernel32.dll".to_owned(),
        iat_rva: Rva(0),
        iat_address: GuestAddress(IAT_BASE),
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "GetLocalTime".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap();
    memory.seal_owner(owner).unwrap();
    let reflection = memory.pending_reflection().unwrap();
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
    let mut runtime = controlled_runtime(memory, controlled_startup_info());
    runtime.set_current_last_error(87).unwrap();
    let clock = runtime.clock().clone();
    {
        let outcome = dispatcher.dispatch_owned(
            owned_trap(&endpoint, STACK_BASE, 0x1000_0580),
            &mut runtime,
            None,
            dispatch_kernel32_api,
        );
        let DispatchOutcome::Immediate(_) = outcome else {
            panic!("local time stopped: {outcome:?}");
        };
        assert_eq!(runtime.current_last_error().unwrap(), 87);
        acknowledge_pending_delta(&mut runtime.memory);
        let fields: Vec<u16> = runtime
            .memory
            .read(GuestAddress(OUTPUT_BASE), 16)
            .unwrap()
            .chunks_exact(2)
            .map(|bytes| u16::from_le_bytes(bytes.try_into().unwrap()))
            .collect();
        assert_eq!(fields, [2026, 7, 6, 11, 9, 0, 0, 0]);
        assert_eq!(runtime.clock(), &clock);
    }
}

#[test]
fn set_current_directory_a_owns_normalized_process_directory_transactionally() {
    const EXACT_PATH: u32 = OUTPUT_BASE + 0x700;
    const ALIAS_PATH: u32 = OUTPUT_BASE + 0x720;
    const UNSUPPORTED_PATH: u32 = OUTPUT_BASE + 0x740;
    const RELATIVE_PATH: u32 = OUTPUT_BASE + 0x760;
    const EMPTY_PATH: u32 = OUTPUT_BASE + 0x780;
    const UNTERMINATED_PATH: u32 = MEMORY_BASE.0 + 0x2fe0;

    let (mut memory, owner) = initialized_memory();
    for (address, bytes) in [
        (EXACT_PATH, b"C:\\Vine\\\0".as_slice()),
        (ALIAS_PATH, b"c:/vInE/\0".as_slice()),
        (UNSUPPORTED_PATH, b"C:\\Other\0".as_slice()),
        (RELATIVE_PATH, b".\0".as_slice()),
        (EMPTY_PATH, b"\0".as_slice()),
        (
            UNTERMINATED_PATH,
            b"XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX".as_slice(),
        ),
    ] {
        memory
            .write_initialization(owner, GuestAddress(address), bytes)
            .expect("initialize ANSI directory path");
    }

    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x108);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize SetCurrentDirectoryA IAT");
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(1),
        address: endpoint_address,
        requester_owner: owner,
        module: "KERNEL32.dll".to_owned(),
        iat_rva: Rva(0),
        iat_address: GuestAddress(IAT_BASE),
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "SetCurrentDirectoryA".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });

    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).expect("dispatcher");
    let frames = [
        vec![0x1000_0c01, EXACT_PATH],
        vec![0x1000_0c02, ALIAS_PATH],
        vec![0x1000_0c03, UNSUPPORTED_PATH],
        vec![0x1000_0c04, RELATIVE_PATH],
        vec![0x1000_0c05, EMPTY_PATH],
        vec![0x1000_0c06, 0],
        vec![0x1000_0c07, UNTERMINATED_PATH],
    ];
    for (index, words) in frames.iter().enumerate() {
        write_frame(
            &mut memory,
            owner,
            GuestAddress(STACK_BASE + index as u32 * 0x20),
            words,
        );
    }
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");

    let mut runtime = controlled_runtime(memory, controlled_startup_info());
    assert_eq!(runtime.current_directory_a().bytes_with_nul, b"C:\\Vine\0");

    let dispatch = |index: usize, runtime: &mut Runtime| -> DispatchOutcome {
        dispatcher.dispatch_owned(
            owned_trap(
                &endpoint,
                STACK_BASE + index as u32 * 0x20,
                frames[index][0],
            ),
            runtime,
            None,
            dispatch_kernel32_api,
        )
    };

    let exact_input_before = runtime.memory.read(GuestAddress(EXACT_PATH), 9).unwrap();
    let exact = dispatch(0, &mut runtime);
    let DispatchOutcome::Immediate(exact) = exact else {
        panic!("expected SetCurrentDirectoryA success, got {exact:?}");
    };
    assert_eq!(exact.eax, Some(1));
    assert_eq!(exact.stack_pointer, GuestAddress(STACK_BASE + 8));
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    assert_eq!(runtime.current_directory_a().bytes_with_nul, b"C:\\Vine\0");
    assert_eq!(
        runtime.memory.read(GuestAddress(EXACT_PATH), 9).unwrap(),
        exact_input_before
    );

    let alias = dispatch(1, &mut runtime);
    let DispatchOutcome::Immediate(alias) = alias else {
        panic!("expected normalized directory alias success, got {alias:?}");
    };
    assert_eq!(alias.eax, Some(1));
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    assert_eq!(runtime.current_directory_a().bytes_with_nul, b"C:\\Vine\0");

    for index in [2_usize, 3] {
        let directory_before = runtime.current_directory_a().clone();
        let clock_before = runtime.clock().clone();
        let memory_before = runtime.memory.stats();
        let unsupported = dispatch(index, &mut runtime);
        assert!(matches!(unsupported, DispatchOutcome::Fault(_)));
        assert_eq!(runtime.current_directory_a(), &directory_before);
        assert_eq!(runtime.clock(), &clock_before);
        assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
        assert_eq!(runtime.current_thread_id(), RuntimeThreadId(THREAD_ID));
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::Runnable
        );
        assert_eq!(runtime.memory.stats(), memory_before);
    }

    for (index, expected_error) in [(4_usize, 123), (5, 87)] {
        let directory_before = runtime.current_directory_a().clone();
        let memory_before = runtime.memory.stats();
        let failure = dispatch(index, &mut runtime);
        let DispatchOutcome::Immediate(failure) = failure else {
            panic!("expected Win32 directory failure, got {failure:?}");
        };
        assert_eq!(failure.eax, Some(0));
        assert_eq!(runtime.current_last_error().unwrap(), expected_error);
        assert_eq!(runtime.current_directory_a(), &directory_before);
        assert_eq!(runtime.memory.stats(), memory_before);
    }

    let directory_before = runtime.current_directory_a().clone();
    let clock_before = runtime.clock().clone();
    let memory_before_limit = runtime.memory.stats();
    let limit = dispatch(6, &mut runtime);
    assert!(matches!(limit, DispatchOutcome::Fault(_)));
    assert_eq!(runtime.current_directory_a(), &directory_before);
    assert_eq!(runtime.clock(), &clock_before);
    assert_eq!(runtime.current_last_error().unwrap(), 87);
    assert_eq!(runtime.current_thread_id(), RuntimeThreadId(THREAD_ID));
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.memory.stats(), memory_before_limit);

    let valid = dispatch(0, &mut runtime);
    assert!(
        matches!(valid, DispatchOutcome::Immediate(ref completion) if completion.eax == Some(1))
    );
    assert_eq!(runtime.current_directory_a().bytes_with_nul, b"C:\\Vine\0");
}

#[test]
fn create_waitable_timer_a_returns_typed_nonsignaled_manual_reset_object() {
    let (mut memory, owner) = initialized_memory();
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x110);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize CreateWaitableTimerA IAT");
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(1),
        address: endpoint_address,
        requester_owner: owner,
        module: "KERNEL32.dll".to_owned(),
        iat_rva: Rva(0),
        iat_address: GuestAddress(IAT_BASE),
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "CreateWaitableTimerA".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });

    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).expect("dispatcher");
    let frames = [
        vec![0x1000_0e01, 0, 1, 0],
        vec![0x1000_0e02, 4, 1, 0],
        vec![0x1000_0e03, 0, 1, OUTPUT_BASE],
        vec![0x1000_0e04, 0, 0, 0],
    ];
    for (index, words) in frames.iter().enumerate() {
        write_frame(
            &mut memory,
            owner,
            GuestAddress(STACK_BASE + index as u32 * 0x20),
            words,
        );
    }
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");

    let mut runtime = controlled_runtime(memory, controlled_startup_info());
    let dispatch = |index: usize, runtime: &mut Runtime| -> DispatchOutcome {
        dispatcher.dispatch_owned(
            owned_trap(
                &endpoint,
                STACK_BASE + index as u32 * 0x20,
                frames[index][0],
            ),
            runtime,
            None,
            dispatch_kernel32_api,
        )
    };

    let memory_before = runtime.memory.stats();
    let created = dispatch(0, &mut runtime);
    let DispatchOutcome::Immediate(created) = created else {
        panic!("expected waitable timer handle, got {created:?}");
    };
    assert_eq!(created.eax, Some(0x5701_0001));
    assert_eq!(created.stack_pointer, GuestAddress(STACK_BASE + 16));
    assert_eq!(runtime.current_last_error().unwrap(), 0);
    assert_eq!(runtime.memory.stats(), memory_before);
    let handle = crate::runtime::RuntimeHandle(0x5701_0001);
    let timer = runtime
        .waitable_timer_for_handle(handle)
        .expect("created timer payload");
    assert_eq!(timer.id, RuntimeWaitableTimerId(1));
    assert!(timer.manual_reset);
    assert_eq!(
        runtime
            .object_wait_state(handle, RuntimeObjectKind::WaitableTimer)
            .expect("created timer wait state"),
        RuntimeWaitState::Nonsignaled
    );

    for index in [1_usize, 2] {
        let timer_before = *runtime.waitable_timer_for_handle(handle).unwrap();
        let object_before = runtime
            .object_for_handle(handle, RuntimeObjectKind::WaitableTimer)
            .unwrap()
            .clone();
        let clock_before = runtime.clock().clone();
        let memory_before = runtime.memory.stats();
        let rejected = dispatch(index, &mut runtime);
        assert!(matches!(rejected, DispatchOutcome::Fault(_)));
        assert_eq!(
            runtime.waitable_timer_for_handle(handle).unwrap(),
            &timer_before
        );
        assert_eq!(
            runtime
                .object_for_handle(handle, RuntimeObjectKind::WaitableTimer)
                .unwrap(),
            &object_before
        );
        assert_eq!(runtime.clock(), &clock_before);
        assert_eq!(runtime.current_last_error().unwrap(), 0);
        assert_eq!(runtime.current_thread_id(), RuntimeThreadId(THREAD_ID));
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::Runnable
        );
        assert_eq!(runtime.memory.stats(), memory_before);
    }

    let auto_reset = {
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, STACK_BASE + 0x60, frames[3][0]),
            &mut runtime,
            None,
            dispatch_kernel32_api,
        )
    };
    let DispatchOutcome::Immediate(completion) = auto_reset else {
        panic!("generic auto-reset timer creation stopped: {auto_reset:?}");
    };
    assert_eq!(completion.eax, Some(0x5701_0002));
    assert_eq!(runtime.current_last_error().unwrap(), 0);
    assert!(
        !runtime
            .waitable_timer_for_handle(crate::runtime::RuntimeHandle(0x5701_0002))
            .unwrap()
            .manual_reset
    );
}

#[test]
fn set_waitable_timer_arms_bounded_relative_one_shot_atomically() {
    const DUE_TIME: u32 = OUTPUT_BASE + 0x180;
    const ABSOLUTE_DUE_TIME: u32 = OUTPUT_BASE + 0x188;
    const LONGER_DUE_TIME: u32 = OUTPUT_BASE + 0x190;

    let (mut memory, owner) = initialized_memory();
    for (address, value) in [
        (DUE_TIME, -5_000_i64),
        (ABSOLUTE_DUE_TIME, 5_000_i64),
        (LONGER_DUE_TIME, -100_001_i64),
    ] {
        memory
            .write_initialization(owner, GuestAddress(address), &value.to_le_bytes())
            .expect("initialize LARGE_INTEGER due time");
    }
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x114);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize SetWaitableTimer IAT");
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(1),
        address: endpoint_address,
        requester_owner: owner,
        module: "KERNEL32.dll".to_owned(),
        iat_rva: Rva(0),
        iat_address: GuestAddress(IAT_BASE),
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "SetWaitableTimer".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });

    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).expect("dispatcher");
    let frames = [
        vec![0x1000_0f01, 0x5701_0001, DUE_TIME, 0, 0, 0, 0],
        vec![0x1000_0f02, 0x5701_ffff, DUE_TIME, 0, 0, 0, 0],
        vec![0x1000_0f03, 0x5701_0001, ABSOLUTE_DUE_TIME, 0, 0, 0, 0],
        vec![0x1000_0f04, 0x5701_0001, LONGER_DUE_TIME, 0, 0, 0, 0],
        vec![0x1000_0f05, 0x5701_0001, DUE_TIME, 1, 0, 0, 0],
        vec![0x1000_0f06, 0x5701_0001, DUE_TIME, 0, 0x1009_9600, 0, 0],
        vec![0x1000_0f07, 0x5701_0001, DUE_TIME, 0, 0, 0x2000_3000, 0],
        vec![0x1000_0f08, 0x5701_0001, DUE_TIME, 0, 0, 0, 1],
    ];
    for (index, words) in frames.iter().enumerate() {
        write_frame(
            &mut memory,
            owner,
            GuestAddress(STACK_BASE + index as u32 * 0x20),
            words,
        );
    }
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");

    let mut runtime = controlled_runtime(memory, controlled_startup_info());
    let created = runtime
        .create_waitable_timer_a(true)
        .expect("create timer before arm");
    assert_eq!(created.0, 0x5701_0001);
    let dispatch = |index: usize, runtime: &mut Runtime| -> DispatchOutcome {
        dispatcher.dispatch_owned(
            owned_trap(
                &endpoint,
                STACK_BASE + index as u32 * 0x20,
                frames[index][0],
            ),
            runtime,
            None,
            dispatch_kernel32_api,
        )
    };

    let memory_before = runtime.memory.stats();
    let armed = dispatch(0, &mut runtime);
    let DispatchOutcome::Immediate(armed) = armed else {
        panic!("expected waitable timer arm success, got {armed:?}");
    };
    assert_eq!(armed.eax, Some(1));
    assert_eq!(armed.stack_pointer, GuestAddress(STACK_BASE + 28));
    assert_eq!(runtime.current_last_error().unwrap(), 0);
    assert_eq!(runtime.memory.stats(), memory_before);
    assert_eq!(
        runtime.waitable_timer_for_handle(created).unwrap().arm,
        Some(RuntimeWaitableTimerArm {
            deadline_time_100ns: 5_000,
            generation: 1,
        })
    );
    assert_eq!(
        runtime
            .object_wait_state(created, RuntimeObjectKind::WaitableTimer)
            .unwrap(),
        RuntimeWaitState::Nonsignaled
    );

    let rearmed = dispatch(0, &mut runtime);
    assert!(matches!(rearmed, DispatchOutcome::Immediate(_)));
    assert_eq!(
        runtime
            .waitable_timer_for_handle(created)
            .unwrap()
            .arm
            .unwrap()
            .generation,
        2
    );

    let timer_before_invalid_handle = *runtime
        .waitable_timer_for_handle(created)
        .expect("armed timer before invalid-handle call");
    let object_before_invalid_handle = runtime
        .object_for_handle(created, RuntimeObjectKind::WaitableTimer)
        .expect("timer object before invalid-handle call")
        .clone();
    let memory_before_invalid_handle = runtime.memory.stats();
    let invalid_handle = dispatch(1, &mut runtime);
    assert!(matches!(
        invalid_handle,
        DispatchOutcome::Immediate(ref completion)
            if completion.eax == Some(0) && runtime.current_last_error().unwrap() == 6
    ));
    assert_eq!(
        runtime
            .waitable_timer_for_handle(created)
            .expect("timer remains after invalid-handle call"),
        &timer_before_invalid_handle
    );
    assert_eq!(
        runtime
            .object_for_handle(created, RuntimeObjectKind::WaitableTimer)
            .expect("object remains after invalid-handle call"),
        &object_before_invalid_handle
    );
    assert_eq!(runtime.current_last_error().unwrap(), 6);
    assert_eq!(runtime.memory.stats(), memory_before_invalid_handle);

    for index in [2_usize, 4, 5, 6, 7] {
        let clock_before = runtime.clock().clone();
        let memory_before = runtime.memory.stats();
        let rejected = dispatch(index, &mut runtime);
        assert!(matches!(rejected, DispatchOutcome::Fault(_)));
        assert_eq!(
            runtime.waitable_timer_for_handle(created).unwrap(),
            &timer_before_invalid_handle
        );
        assert_eq!(
            runtime
                .object_for_handle(created, RuntimeObjectKind::WaitableTimer)
                .unwrap(),
            &object_before_invalid_handle
        );
        assert_eq!(runtime.clock(), &clock_before);
        assert_eq!(runtime.current_last_error().unwrap(), 6);
        assert_eq!(runtime.current_thread_id(), RuntimeThreadId(THREAD_ID));
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::Runnable
        );
        assert_eq!(runtime.memory.stats(), memory_before);
    }
    let clock_before = runtime.timer_time_100ns();
    assert!(matches!(dispatch(3, &mut runtime),
        DispatchOutcome::Immediate(ref completion) if completion.eax == Some(1)));
    assert_eq!(
        runtime
            .waitable_timer_for_handle(created)
            .unwrap()
            .arm
            .unwrap()
            .deadline_time_100ns,
        clock_before + 100_001
    );
}

#[test]
fn sleep_yields_or_defers_with_browser_delay_bounds() {
    let (mut memory, owner) = initialized_memory();
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x11c);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize Sleep IAT");
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(1),
        address: endpoint_address,
        requester_owner: owner,
        module: "KERNEL32.dll".to_owned(),
        iat_rva: Rva(0),
        iat_address: GuestAddress(IAT_BASE),
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "Sleep".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });

    let frames = [
        vec![0x1000_1010, 0],
        vec![0x1000_1011, 1],
        vec![0x1000_1012, 0],
        vec![0x1000_1013, 50],
        vec![0x1000_1014, 0x8000_0000],
    ];
    for (index, words) in frames.iter().enumerate() {
        write_frame(
            &mut memory,
            owner,
            GuestAddress(STACK_BASE + index as u32 * 0x20),
            words,
        );
    }
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).expect("dispatcher");

    let mut runtime = controlled_runtime(memory, controlled_startup_info());
    let dispatch = |index: usize, runtime: &mut Runtime| -> DispatchOutcome {
        dispatcher.dispatch_owned(
            owned_trap(
                &endpoint,
                STACK_BASE + index as u32 * 0x20,
                frames[index][0],
            ),
            runtime,
            None,
            dispatch_kernel32_api,
        )
    };

    let clock_runtime_before_success = runtime.clock().clone();
    let memory_before_success = runtime.memory.stats();
    let DispatchOutcome::Immediate(completion) = dispatch(0, &mut runtime) else {
        panic!("Sleep(0) did not complete immediately");
    };
    assert_eq!(completion.eax, None);
    assert_eq!(completion.stack_pointer, GuestAddress(STACK_BASE + 8));
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    assert_eq!(
        completion.execution_boundary,
        RuntimeApiBoundaryDisposition::YieldCurrentThread
    );
    assert_eq!(runtime.clock(), &clock_runtime_before_success);
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    assert_eq!(runtime.current_thread_id(), RuntimeThreadId(THREAD_ID));
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.memory.stats(), memory_before_success);

    let memory_before_finite = runtime.memory.stats();
    let finite = dispatch(3, &mut runtime);
    let DispatchOutcome::Deferred(finite) = finite else {
        panic!("Sleep(50) did not defer to browser pacing");
    };
    assert_eq!(
        finite.request.service,
        vine_core::RUNTIME_SLEEP_DELAY_SERVICE
    );
    assert_eq!(finite.request.payload, 50_u32.to_le_bytes());
    assert_eq!(finite.frame.argument_u32(0), Some(50));
    assert!(matches!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::WaitingHost { request_id }
            if request_id == finite.request.request_id
    ));
    assert_eq!(runtime.memory.stats(), memory_before_finite);

    let runtime_completion = runtime
        .complete_host_request(finite.request.request_id)
        .expect("complete finite Sleep owner wait");
    assert_eq!(
        runtime_completion.service,
        vine_core::RUNTIME_SLEEP_DELAY_SERVICE
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    let completed = {
        dispatcher.complete_deferred(
            &finite,
            HostCompletion {
                request_id: runtime_completion.request_id,
                result: ApiImmediate::new(None),
            },
            &mut runtime,
        )
    };
    let DispatchOutcome::Immediate(completed) = completed else {
        panic!("Sleep(50) completion did not restore the decoded ABI frame");
    };
    assert_eq!(completed.eax, None);
    assert_eq!(
        completed.stack_pointer,
        GuestAddress(STACK_BASE + 3 * 0x20 + 8)
    );
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);

    let basic = |index: usize, runtime: &mut Runtime| {
        dispatcher.dispatch_owned(
            owned_trap(
                &endpoint,
                STACK_BASE + index as u32 * 0x20,
                frames[index][0],
            ),
            runtime,
            None,
            dispatch_kernel32_api,
        )
    };
    let DispatchOutcome::Immediate(zero) = basic(0, &mut runtime) else {
        panic!("generic Sleep(0) did not yield");
    };
    assert_eq!(
        zero.execution_boundary,
        RuntimeApiBoundaryDisposition::YieldCurrentThread
    );
    let DispatchOutcome::Deferred(finite) = basic(1, &mut runtime) else {
        panic!("generic finite Sleep did not defer to browser pacing");
    };
    assert_eq!(
        finite.request.service,
        vine_core::RUNTIME_SLEEP_DELAY_SERVICE
    );
    assert_eq!(finite.request.payload, 1_u32.to_le_bytes());
    assert_eq!(finite.frame.argument_u32(0), Some(1));
    assert!(
        matches!(runtime.scheduler_state().unwrap(), SchedulerThreadState::WaitingHost { request_id } if request_id == finite.request.request_id)
    );
    let completion = runtime
        .complete_host_request(finite.request.request_id)
        .unwrap();
    let DispatchOutcome::Immediate(resumed) = dispatcher.complete_deferred(
        &finite,
        HostCompletion {
            request_id: completion.request_id,
            result: ApiImmediate::new(None),
        },
        &mut runtime,
    ) else {
        panic!("generic Sleep did not restore the guest frame");
    };
    assert_eq!(resumed.eax, None);
    assert_eq!(resumed.stack_pointer, GuestAddress(STACK_BASE + 0x20 + 8));
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    let clock_before = runtime.clock().clone();
    let memory_before = runtime.memory.stats();
    assert!(matches!(basic(4, &mut runtime), DispatchOutcome::Fault(_)));
    assert_eq!(runtime.clock(), &clock_before);
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    assert_eq!(runtime.current_thread_id(), RuntimeThreadId(THREAD_ID));
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.memory.stats(), memory_before);
}

#[test]
fn wait_for_single_object_ex_completes_bounded_timer_waits_and_preserves_last_error() {
    let (mut memory, owner) = initialized_memory();
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x118);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize WaitForSingleObjectEx IAT");
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(1),
        address: endpoint_address,
        requester_owner: owner,
        module: "KERNEL32.dll".to_owned(),
        iat_rva: Rva(0),
        iat_address: GuestAddress(IAT_BASE),
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "WaitForSingleObjectEx".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });

    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).expect("dispatcher");
    let mut runtime = controlled_runtime(memory, controlled_startup_info());
    let pacing_competitor = RuntimeThreadId(THREAD_ID + 1);
    runtime
        .register_thread(pacing_competitor, 0)
        .expect("register timer pacing competitor");
    let manual = runtime
        .create_waitable_timer_a(true)
        .expect("create manual-reset timer");
    runtime
        .set_waitable_timer(manual, 5_000)
        .expect("arm reached timer interval");
    let unarmed = runtime
        .create_waitable_timer_a(true)
        .expect("create unarmed timeout timer");
    let frames = [
        vec![0x1000_1001, manual.0, 10, 1],
        vec![0x1000_1002, manual.0, 0, 1],
        vec![0x1000_1003, unarmed.0, 0, 1],
        vec![0x1000_1004, unarmed.0, 10, 1],
        vec![0x1000_1005, u32::MAX, 10, 1],
        vec![0x1000_1006, manual.0, 11, 1],
        vec![0x1000_1007, manual.0, 10, 0],
    ];
    for (index, words) in frames.iter().enumerate() {
        write_frame(
            &mut runtime.memory,
            owner,
            GuestAddress(STACK_BASE + index as u32 * 0x20),
            words,
        );
    }
    runtime
        .memory
        .seal_owner(owner)
        .expect("seal runtime owner");
    let reflection = runtime
        .memory
        .pending_reflection()
        .expect("initial reflection");
    runtime
        .memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");

    let dispatch = |index: usize, runtime: &mut Runtime| -> DispatchOutcome {
        dispatcher.dispatch_owned(
            owned_trap(
                &endpoint,
                STACK_BASE + index as u32 * 0x20,
                frames[index][0],
            ),
            runtime,
            None,
            dispatch_kernel32_api,
        )
    };

    let memory_before_success = runtime.memory.stats();
    let satisfied = dispatch(0, &mut runtime);
    let DispatchOutcome::Deferred(mut satisfied_call) = satisfied else {
        panic!("expected paced signaled wait, got {satisfied:?}");
    };
    assert_eq!(satisfied_call.request.service, "browser.timer-delay");
    assert_eq!(satisfied_call.request.payload, 1_u32.to_le_bytes());
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::WaitingHost {
            request_id: satisfied_call.request.request_id
        }
    );
    assert_eq!(runtime.timer_time_100ns(), 0);
    runtime
        .select_thread(pacing_competitor)
        .expect("run another guest thread while the timer owner waits");
    runtime
        .advance_scheduled_clock_to(20_000)
        .expect("advance the shared clock past the pending timer deadline");
    runtime
        .select_thread(RuntimeThreadId(THREAD_ID))
        .expect("restore timer pacing owner for completion");
    let clock_before_bad_ack = runtime.clock().clone();
    let scheduler_before_bad_ack = runtime.scheduler_state().unwrap();
    let manual_before_bad_ack = *runtime.waitable_timer_for_handle(manual).unwrap();
    let unarmed_before_bad_ack = *runtime.waitable_timer_for_handle(unarmed).unwrap();
    let wait_before_bad_ack = *runtime
        .scheduler()
        .wait_registrations()
        .values()
        .find(|registration| registration.thread_id == RuntimeThreadId(THREAD_ID))
        .expect("live timer wait before browser acknowledgement");
    let memory_before_bad_ack = runtime.memory.stats();
    let bad_ack =
        complete_kernel32_waitable_timer_pacing(&dispatcher, &satisfied_call, 0, &mut runtime);
    assert!(matches!(bad_ack, DispatchOutcome::Fault(_)));
    assert_eq!(runtime.clock(), &clock_before_bad_ack);
    assert_eq!(runtime.scheduler_state().unwrap(), scheduler_before_bad_ack);
    assert_eq!(
        runtime.waitable_timer_for_handle(manual).unwrap(),
        &manual_before_bad_ack
    );
    assert_eq!(
        runtime.waitable_timer_for_handle(unarmed).unwrap(),
        &unarmed_before_bad_ack
    );
    assert_eq!(
        runtime
            .scheduler()
            .wait_registrations()
            .get(&wait_before_bad_ack.id),
        Some(&wait_before_bad_ack)
    );
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    assert_eq!(runtime.memory.stats(), memory_before_bad_ack);
    let original_delay = std::mem::replace(
        &mut satisfied_call.request.payload,
        2_u32.to_le_bytes().to_vec(),
    );
    let bad_delay =
        complete_kernel32_waitable_timer_pacing(&dispatcher, &satisfied_call, 1, &mut runtime);
    assert!(matches!(bad_delay, DispatchOutcome::Fault(_)));
    assert_eq!(runtime.clock(), &clock_before_bad_ack);
    assert_eq!(runtime.scheduler_state().unwrap(), scheduler_before_bad_ack);
    assert_eq!(
        runtime.waitable_timer_for_handle(manual).unwrap(),
        &manual_before_bad_ack
    );
    assert_eq!(
        runtime.waitable_timer_for_handle(unarmed).unwrap(),
        &unarmed_before_bad_ack
    );
    assert_eq!(
        runtime
            .scheduler()
            .wait_registrations()
            .get(&wait_before_bad_ack.id),
        Some(&wait_before_bad_ack)
    );
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    assert_eq!(runtime.memory.stats(), memory_before_bad_ack);
    satisfied_call.request.payload = original_delay;
    let satisfied =
        complete_kernel32_waitable_timer_pacing(&dispatcher, &satisfied_call, 1, &mut runtime);
    let DispatchOutcome::Immediate(satisfied) = satisfied else {
        panic!("expected paced signaled completion, got {satisfied:?}");
    };
    assert!(matches!(
        runtime.pop_scheduler_event(),
        Some(RuntimeSchedulerEvent::HostCompleted(_))
    ));
    assert_eq!(satisfied.eax, Some(0));
    assert_eq!(satisfied.stack_pointer, GuestAddress(STACK_BASE + 16));
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    assert_eq!(runtime.memory.stats(), memory_before_success);
    assert_eq!(runtime.timer_time_100ns(), 20_000);
    assert!(runtime.scheduler().wait_registrations().is_empty());
    assert!(runtime.scheduler().object_waiters().is_empty());
    assert_eq!(
        runtime
            .object_wait_state(manual, RuntimeObjectKind::WaitableTimer)
            .unwrap(),
        RuntimeWaitState::Signaled { exit_code: 0 }
    );
    let timer = runtime.waitable_timer_for_handle(manual).unwrap();
    assert_eq!(timer.arm, None);
    assert_eq!(
        timer.last_expiration,
        Some(RuntimeWaitableTimerExpiration {
            arm_generation: 1,
            deadline_time_100ns: 5_000,
        })
    );

    let repeated = dispatch(1, &mut runtime);
    assert!(matches!(
        repeated,
        DispatchOutcome::Immediate(ref completion)
            if completion.eax == Some(0)
                && runtime.current_last_error().unwrap() == INITIAL_LAST_ERROR
    ));
    assert_eq!(runtime.timer_time_100ns(), 20_000);

    let poll_timeout = dispatch(2, &mut runtime);
    assert!(matches!(
        poll_timeout,
        DispatchOutcome::Immediate(ref completion)
            if completion.eax == Some(258)
                && runtime.current_last_error().unwrap() == INITIAL_LAST_ERROR
    ));
    assert_eq!(runtime.timer_time_100ns(), 20_000);

    let finite_timeout = dispatch(3, &mut runtime);
    let DispatchOutcome::Deferred(mut finite_timeout_call) = finite_timeout else {
        panic!("expected paced timeout, got {finite_timeout:?}");
    };
    assert_eq!(runtime.timer_time_100ns(), 20_000);
    let clock_before_invalid_frame = runtime.clock().clone();
    let started_at_time_100ns_invalid_frame = runtime.timer_time_100ns();
    let memory_before_invalid_frame = runtime.memory.stats();
    let original_arguments = std::mem::take(&mut finite_timeout_call.frame.arguments);
    let rejected =
        complete_kernel32_waitable_timer_pacing(&dispatcher, &finite_timeout_call, 1, &mut runtime);
    assert!(matches!(rejected, DispatchOutcome::Fault(_)));
    assert_eq!(runtime.clock(), &clock_before_invalid_frame);
    assert_eq!(
        runtime.timer_time_100ns(),
        started_at_time_100ns_invalid_frame
    );
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    assert_eq!(runtime.memory.stats(), memory_before_invalid_frame);
    assert_eq!(runtime.scheduler().wait_registrations().len(), 1);
    assert_eq!(runtime.scheduler().host_requests().len(), 1);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::WaitingHost {
            request_id: finite_timeout_call.request.request_id,
        }
    );
    assert!(runtime.scheduler().events().is_empty());
    finite_timeout_call.frame.arguments = original_arguments;
    let finite_timeout =
        complete_kernel32_waitable_timer_pacing(&dispatcher, &finite_timeout_call, 1, &mut runtime);
    assert!(matches!(
        finite_timeout,
        DispatchOutcome::Immediate(ref completion)
            if completion.eax == Some(258)
                && runtime.current_last_error().unwrap() == INITIAL_LAST_ERROR
    ));
    assert!(matches!(
        runtime.pop_scheduler_event(),
        Some(RuntimeSchedulerEvent::HostCompleted(_))
    ));
    assert_eq!(runtime.timer_time_100ns(), 120_000);
    assert!(runtime.scheduler().wait_registrations().is_empty());
    assert!(runtime.scheduler().object_waiters().is_empty());
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );

    let time_before_invalid_handle = runtime.timer_time_100ns();
    let memory_before_invalid_handle = runtime.memory.stats();
    let invalid_handle = dispatch(4, &mut runtime);
    assert!(matches!(
        invalid_handle,
        DispatchOutcome::Immediate(ref completion)
            if completion.eax == Some(u32::MAX)
                && runtime.current_last_error().unwrap() == 6
    ));
    assert_eq!(runtime.timer_time_100ns(), time_before_invalid_handle);
    assert_eq!(runtime.current_last_error().unwrap(), 6);
    assert_eq!(runtime.memory.stats(), memory_before_invalid_handle);

    for index in [5_usize, 6] {
        let clock_before = runtime.clock().clone();
        let timer_before = *runtime.waitable_timer_for_handle(manual).unwrap();
        let memory_before = runtime.memory.stats();
        let satisfied = dispatch(index, &mut runtime);
        assert!(matches!(
            satisfied,
            DispatchOutcome::Immediate(ref completion)
                if completion.eax == Some(0) && runtime.current_last_error().unwrap() == 6
        ));
        assert_eq!(runtime.clock(), &clock_before);
        assert_eq!(
            runtime.waitable_timer_for_handle(manual).unwrap(),
            &timer_before
        );
        assert_eq!(runtime.current_last_error().unwrap(), 6);
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::Runnable
        );
        assert!(runtime.scheduler().wait_registrations().is_empty());
        assert!(runtime.scheduler().object_waiters().is_empty());
        assert_eq!(runtime.memory.stats(), memory_before);
    }
}

#[test]
fn get_string_type_w_models_measured_ctype1_and_fails_closed() {
    const SOURCE: u32 = OUTPUT_BASE + 0x100;
    const DESTINATION: u32 = OUTPUT_BASE + 0x500;
    const UNSUPPORTED_SOURCE: u32 = OUTPUT_BASE + 0x900;
    const UNSUPPORTED_DESTINATION: u32 = OUTPUT_BASE + 0xa00;

    let source = [
        0x0000_u16, 0x0009, 0x000a, 0x000d, 0x0020, 0x0021, 0x002b, 0x002c, 0x002e, 0x0030, 0x0039,
        0x0041, 0x005a, 0x005f, 0x0061, 0x007a, 0x007f, 0x0080, 0x00a0, 0x00a5, 0x3000, 0x3042,
        0x30a2, 0x30fb, 0x4e00, 0xff01, 0xff10, 0xff21, 0xff41, 0xff61, 0xff66, 0xff71, 0xff9e,
        0xf8f0,
    ];
    let expected = [
        544_u16, 616, 552, 552, 584, 528, 528, 528, 528, 644, 644, 897, 769, 528, 898, 770, 544,
        544, 584, 528, 584, 768, 768, 528, 768, 528, 644, 897, 898, 528, 768, 768, 768, 512,
    ];
    let source_bytes = source
        .iter()
        .flat_map(|unit| unit.to_le_bytes())
        .collect::<Vec<_>>();
    let expected_bytes = expected
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<_>>();

    let (mut memory, owner) = initialized_memory();
    memory
        .write_initialization(owner, GuestAddress(SOURCE), &source_bytes)
        .expect("representative UTF-16 source");
    memory
        .write_initialization(
            owner,
            GuestAddress(DESTINATION),
            &vec![0xa5; expected_bytes.len()],
        )
        .expect("CTYPE1 destination");
    memory
        .write_initialization(
            owner,
            GuestAddress(UNSUPPORTED_SOURCE),
            &0x2603_u16.to_le_bytes(),
        )
        .expect("unsupported UTF-16 source");
    memory
        .write_initialization(owner, GuestAddress(UNSUPPORTED_DESTINATION), &[0xa5, 0xa5])
        .expect("unsupported destination");

    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x108);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize GetStringTypeW IAT");
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(1),
        address: endpoint_address,
        requester_owner: owner,
        module: "KERNEL32.dll".to_owned(),
        iat_rva: Rva(0),
        iat_address: GuestAddress(IAT_BASE),
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "GetStringTypeW".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });

    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).expect("dispatcher");
    let frames = [
        vec![0x1000_0c01, 1, SOURCE, source.len() as u32, DESTINATION],
        vec![0x1000_0c02, 1, SOURCE, 0, DESTINATION],
        vec![0x1000_0c03, 2, SOURCE, source.len() as u32, DESTINATION],
        vec![0x1000_0c04, 1, SOURCE, u32::MAX, DESTINATION],
        vec![
            0x1000_0c05,
            1,
            UNSUPPORTED_SOURCE,
            1,
            UNSUPPORTED_DESTINATION,
        ],
        vec![0x1000_0c06, 1, SOURCE, source.len() as u32, SOURCE],
    ];
    for (index, words) in frames.iter().enumerate() {
        write_frame(
            &mut memory,
            owner,
            GuestAddress(STACK_BASE + index as u32 * 0x40),
            words,
        );
    }
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");

    let mut runtime = controlled_runtime(memory, controlled_startup_info());
    let dispatch = |index: usize, runtime: &mut Runtime| -> vine_core::ApiImmediateCompletion {
        dispatch_owned(
            &dispatcher,
            runtime,
            &endpoint,
            STACK_BASE + index as u32 * 0x40,
            frames[index][0],
        )
    };

    let success = dispatch(0, &mut runtime);
    assert_eq!(success.eax, Some(1));
    assert_eq!(success.stack_pointer, GuestAddress(STACK_BASE + 20));
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    acknowledge_pending_delta(&mut runtime.memory);
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(DESTINATION), expected_bytes.len())
            .expect("CTYPE1 output"),
        expected_bytes
    );

    let memory_before_zero = runtime.memory.stats();
    let zero_count = dispatch(1, &mut runtime);
    assert_eq!(zero_count.eax, Some(0));
    assert_eq!(runtime.current_last_error().unwrap(), 87);
    assert_eq!(runtime.memory.stats(), memory_before_zero);

    for index in [2_usize, 3, 4, 5] {
        let clock_before = runtime.clock().clone();
        let output_before = runtime
            .memory
            .read(GuestAddress(DESTINATION), expected_bytes.len())
            .unwrap();
        let unsupported_output_before = runtime
            .memory
            .read(GuestAddress(UNSUPPORTED_DESTINATION), 2)
            .unwrap();
        let memory_before = runtime.memory.stats();
        let unsupported = {
            dispatcher.dispatch_owned(
                owned_trap(
                    &endpoint,
                    STACK_BASE + index as u32 * 0x40,
                    frames[index][0],
                ),
                &mut runtime,
                None,
                dispatch_kernel32_api,
            )
        };
        assert!(matches!(unsupported, DispatchOutcome::Fault(_)));
        assert_eq!(runtime.clock(), &clock_before);
        assert_eq!(runtime.current_last_error().unwrap(), 87);
        assert_eq!(runtime.current_thread_id(), RuntimeThreadId(THREAD_ID));
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::Runnable
        );
        assert_eq!(
            runtime
                .memory
                .read(GuestAddress(DESTINATION), expected_bytes.len())
                .unwrap(),
            output_before
        );
        assert_eq!(
            runtime
                .memory
                .read(GuestAddress(UNSUPPORTED_DESTINATION), 2)
                .unwrap(),
            unsupported_output_before
        );
        assert_eq!(runtime.memory.stats(), memory_before);
    }
}

#[test]
fn lc_map_string_w_models_measured_japanese_case_mapping_and_fails_closed() {
    const LOWER_SOURCE: u32 = OUTPUT_BASE + 0x100;
    const LOWER_DESTINATION: u32 = OUTPUT_BASE + 0x300;
    const UPPER_SOURCE: u32 = OUTPUT_BASE + 0x500;
    const UPPER_DESTINATION: u32 = OUTPUT_BASE + 0x700;
    const UNSUPPORTED_SOURCE: u32 = OUTPUT_BASE + 0x900;
    const UNSUPPORTED_DESTINATION: u32 = OUTPUT_BASE + 0xa00;

    let lower_source = [0x0041_u16, 0x005a, 0x0061, 0x3042, 0x30a2, 0xff21, 0xff41];
    let lower_expected = [0x0061_u16, 0x007a, 0x0061, 0x3042, 0x30a2, 0xff41, 0xff41];
    let upper_source = [
        0x0061_u16, 0x007a, 0x0041, 0x3042, 0x30a2, 0xff21, 0xff61, 0xf8f0,
    ];
    let upper_expected = [
        0x0041_u16, 0x005a, 0x0041, 0x3042, 0x30a2, 0xff21, 0xff61, 0xf8f0,
    ];
    let lower_source_bytes = lower_source
        .iter()
        .flat_map(|unit| unit.to_le_bytes())
        .collect::<Vec<_>>();
    let lower_expected_bytes = lower_expected
        .iter()
        .flat_map(|unit| unit.to_le_bytes())
        .collect::<Vec<_>>();
    let upper_source_bytes = upper_source
        .iter()
        .flat_map(|unit| unit.to_le_bytes())
        .collect::<Vec<_>>();
    let upper_expected_bytes = upper_expected
        .iter()
        .flat_map(|unit| unit.to_le_bytes())
        .collect::<Vec<_>>();

    let (mut memory, owner) = initialized_memory();
    for (address, bytes) in [
        (LOWER_SOURCE, lower_source_bytes.as_slice()),
        (UPPER_SOURCE, upper_source_bytes.as_slice()),
    ] {
        memory
            .write_initialization(owner, GuestAddress(address), bytes)
            .expect("locale mapping source");
    }
    for (address, byte_length) in [
        (LOWER_DESTINATION, lower_expected_bytes.len()),
        (UPPER_DESTINATION, upper_expected_bytes.len()),
    ] {
        memory
            .write_initialization(owner, GuestAddress(address), &vec![0xa5; byte_length])
            .expect("locale mapping destination");
    }
    memory
        .write_initialization(
            owner,
            GuestAddress(UNSUPPORTED_SOURCE),
            &0x2603_u16.to_le_bytes(),
        )
        .expect("unsupported locale source");
    memory
        .write_initialization(owner, GuestAddress(UNSUPPORTED_DESTINATION), &[0xa5, 0xa5])
        .expect("unsupported locale destination");

    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x118);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize LCMapStringW IAT");
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(1),
        address: endpoint_address,
        requester_owner: owner,
        module: "KERNEL32.dll".to_owned(),
        iat_rva: Rva(0),
        iat_address: GuestAddress(IAT_BASE),
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "LCMapStringW".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });

    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).expect("dispatcher");
    let frames = [
        vec![
            0x1000_0d01,
            0,
            0x0100,
            LOWER_SOURCE,
            lower_source.len() as u32,
            0,
            0,
        ],
        vec![
            0x1000_0d02,
            0x0400,
            0x0100,
            LOWER_SOURCE,
            lower_source.len() as u32,
            LOWER_DESTINATION,
            lower_source.len() as u32,
        ],
        vec![
            0x1000_0d03,
            0x0800,
            0x0200,
            UPPER_SOURCE,
            upper_source.len() as u32,
            0,
            0,
        ],
        vec![
            0x1000_0d04,
            0x0411,
            0x0200,
            UPPER_SOURCE,
            upper_source.len() as u32,
            UPPER_DESTINATION,
            upper_source.len() as u32,
        ],
        vec![
            0x1000_0d05,
            0x0411,
            0x0100,
            LOWER_SOURCE,
            lower_source.len() as u32,
            LOWER_DESTINATION,
            2,
        ],
        vec![
            0x1000_0d06,
            0x0411,
            0x0100,
            LOWER_SOURCE,
            0,
            LOWER_DESTINATION,
            1,
        ],
        vec![
            0x1000_0d07,
            0x0409,
            0x0100,
            LOWER_SOURCE,
            lower_source.len() as u32,
            0,
            0,
        ],
        vec![
            0x1000_0d08,
            0x0411,
            0,
            LOWER_SOURCE,
            lower_source.len() as u32,
            0,
            0,
        ],
        vec![
            0x1000_0d09,
            0x0411,
            0x0100,
            LOWER_SOURCE,
            u32::MAX,
            LOWER_DESTINATION,
            lower_source.len() as u32,
        ],
        vec![
            0x1000_0d0a,
            0x0411,
            0x0100,
            UNSUPPORTED_SOURCE,
            1,
            UNSUPPORTED_DESTINATION,
            1,
        ],
        vec![
            0x1000_0d0b,
            0x0411,
            0x0100,
            LOWER_SOURCE,
            lower_source.len() as u32,
            LOWER_SOURCE,
            lower_source.len() as u32,
        ],
        vec![
            0x1000_0d0c,
            0x0411,
            0x0100,
            LOWER_SOURCE,
            lower_source.len() as u32,
            0,
            1,
        ],
    ];
    for (index, words) in frames.iter().enumerate() {
        write_frame(
            &mut memory,
            owner,
            GuestAddress(STACK_BASE + index as u32 * 0x40),
            words,
        );
    }
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");

    let mut runtime = controlled_runtime(memory, controlled_startup_info());
    let dispatch = |index: usize, runtime: &mut Runtime| -> vine_core::ApiImmediateCompletion {
        dispatch_owned(
            &dispatcher,
            runtime,
            &endpoint,
            STACK_BASE + index as u32 * 0x40,
            frames[index][0],
        )
    };

    for (index, expected) in [(0_usize, lower_source.len()), (2, upper_source.len())] {
        let query = dispatch(index, &mut runtime);
        assert_eq!(query.eax, Some(expected as u32));
        assert_eq!(
            query.stack_pointer,
            GuestAddress(STACK_BASE + index as u32 * 0x40 + 28)
        );
        assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    }

    let lower = dispatch(1, &mut runtime);
    assert_eq!(lower.eax, Some(lower_source.len() as u32));
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    acknowledge_pending_delta(&mut runtime.memory);
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(LOWER_DESTINATION), lower_expected_bytes.len())
            .expect("lowercase output"),
        lower_expected_bytes
    );

    let upper = dispatch(3, &mut runtime);
    assert_eq!(upper.eax, Some(upper_source.len() as u32));
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    acknowledge_pending_delta(&mut runtime.memory);
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(UPPER_DESTINATION), upper_expected_bytes.len())
            .expect("uppercase output"),
        upper_expected_bytes
    );

    let memory_before_insufficient = runtime.memory.stats();
    let insufficient = dispatch(4, &mut runtime);
    assert_eq!(insufficient.eax, Some(0));
    assert_eq!(runtime.current_last_error().unwrap(), 122);
    assert_eq!(runtime.memory.stats(), memory_before_insufficient);

    for (index, error) in [(5_usize, 87_u32), (11, 87)] {
        let memory_before = runtime.memory.stats();
        let failure = dispatch(index, &mut runtime);
        assert_eq!(failure.eax, Some(0));
        assert_eq!(runtime.current_last_error().unwrap(), error);
        assert_eq!(runtime.memory.stats(), memory_before);
    }

    for index in [6_usize, 7, 8, 9, 10] {
        let clock_before = runtime.clock().clone();
        let lower_before = runtime
            .memory
            .read(GuestAddress(LOWER_DESTINATION), lower_expected_bytes.len())
            .unwrap();
        let upper_before = runtime
            .memory
            .read(GuestAddress(UPPER_DESTINATION), upper_expected_bytes.len())
            .unwrap();
        let memory_before = runtime.memory.stats();
        let unsupported = {
            dispatcher.dispatch_owned(
                owned_trap(
                    &endpoint,
                    STACK_BASE + index as u32 * 0x40,
                    frames[index][0],
                ),
                &mut runtime,
                None,
                dispatch_kernel32_api,
            )
        };
        assert!(matches!(unsupported, DispatchOutcome::Fault(_)));
        assert_eq!(runtime.clock(), &clock_before);
        assert_eq!(runtime.current_last_error().unwrap(), 87);
        assert_eq!(runtime.current_thread_id(), RuntimeThreadId(THREAD_ID));
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::Runnable
        );
        assert_eq!(
            runtime
                .memory
                .read(GuestAddress(LOWER_DESTINATION), lower_expected_bytes.len())
                .unwrap(),
            lower_before
        );
        assert_eq!(
            runtime
                .memory
                .read(GuestAddress(UPPER_DESTINATION), upper_expected_bytes.len())
                .unwrap(),
            upper_before
        );
        assert_eq!(runtime.memory.stats(), memory_before);
    }
}

#[test]
fn system_time_and_tick_count_share_scheduled_clock_without_read_progression() {
    const MODULE_NAME_ADDRESS: u32 = OUTPUT_BASE + 0x200;
    const EXPORT_NAME_ADDRESS: u32 = OUTPUT_BASE + 0x300;

    let (mut memory, owner) = initialized_memory();
    let (dispatcher, endpoints, frames) =
        remaining_dispatcher_fixture(&mut memory, owner, MODULE_NAME_ADDRESS, EXPORT_NAME_ADDRESS);
    let mut runtime = controlled_runtime(memory, controlled_startup_info());

    for _ in 0..2 {
        let system_time = dispatch_owned(
            &dispatcher,
            &mut runtime,
            &endpoints[0],
            frames[0].0,
            frames[0].1[0],
        );
        assert_eq!(system_time.eax, None);
        acknowledge_pending_delta(&mut runtime.memory);
        assert_eq!(
            runtime.memory.read(GuestAddress(OUTPUT_BASE), 8).unwrap(),
            134_282_016_000_000_000_u64.to_le_bytes()
        );
        let tick_count = dispatch_owned(
            &dispatcher,
            &mut runtime,
            &endpoints[2],
            frames[2].0,
            frames[2].1[0],
        );
        assert_eq!(tick_count.eax, Some(3_600_000));
    }
    assert_eq!(runtime.timer_time_100ns(), 0);

    runtime.advance_scheduled_clock_to(25_000).unwrap();
    let system_time = dispatch_owned(
        &dispatcher,
        &mut runtime,
        &endpoints[0],
        frames[0].0,
        frames[0].1[0],
    );
    assert_eq!(system_time.eax, None);
    acknowledge_pending_delta(&mut runtime.memory);
    assert_eq!(
        runtime.memory.read(GuestAddress(OUTPUT_BASE), 8).unwrap(),
        134_282_016_000_025_000_u64.to_le_bytes()
    );
    let tick_count = dispatch_owned(
        &dispatcher,
        &mut runtime,
        &endpoints[2],
        frames[2].0,
        frames[2].1[0],
    );
    assert_eq!(tick_count.eax, Some(3_600_002));
    assert_eq!(runtime.clock().tick_count, 3_600_002);
}

#[test]
fn remaining_kernel32_calls_share_the_generic_dispatcher() {
    const MODULE_NAME_ADDRESS: u32 = OUTPUT_BASE + 0x200;
    const EXPORT_NAME_ADDRESS: u32 = OUTPUT_BASE + 0x300;

    let (mut memory, owner) = initialized_memory_with_limits(MemoryLimits::default());
    let (dispatcher, endpoints, frames) =
        remaining_dispatcher_fixture(&mut memory, owner, MODULE_NAME_ADDRESS, EXPORT_NAME_ADDRESS);
    let mut runtime = controlled_runtime(memory, controlled_startup_info());
    runtime
        .load_module(RuntimeModule {
            handle: 0x4d01_0001,
            requested_name: "KERNEL32.DLL".to_owned(),
            canonical_name: "kernel32.dll".to_owned(),
            runtime_system_module: true,
        })
        .expect("load logical module");
    runtime
        .register_dynamic_endpoint(RuntimeDynamicEndpoint {
            id: 1,
            address: GuestAddress(0x7f02_0000),
            module: "kernel32.dll".to_owned(),
            name: "FlsAlloc".to_owned(),
            ordinal: Some(7),
        })
        .expect("register dynamic endpoint");

    let system_time = dispatch(
        &dispatcher,
        &mut runtime,
        &endpoints[0],
        frames[0].0,
        frames[0].1[0],
        4,
    );
    assert_eq!(system_time.eax, None);
    acknowledge_pending_delta(&mut runtime.memory);
    assert_eq!(
        runtime.memory.read(GuestAddress(OUTPUT_BASE), 8).unwrap(),
        134_282_016_000_000_000_u64.to_le_bytes()
    );

    let thread = dispatch(
        &dispatcher,
        &mut runtime,
        &endpoints[1],
        frames[1].0,
        frames[1].1[0],
        0,
    );
    assert_eq!(thread.eax, Some(THREAD_ID));
    let tick = dispatch(
        &dispatcher,
        &mut runtime,
        &endpoints[2],
        frames[2].0,
        frames[2].1[0],
        0,
    );
    assert_eq!(tick.eax, Some(3_600_000));
    assert_eq!(runtime.clock().tick_count, 3_600_000);
    let heap_policy = dispatch(
        &dispatcher,
        &mut runtime,
        &endpoints[3],
        frames[3].0,
        frames[3].1[0],
        16,
    );
    assert_eq!(heap_policy.eax, Some(1));
    assert!(runtime.process().heap_termination_on_corruption());
    let heap = dispatch(
        &dispatcher,
        &mut runtime,
        &endpoints[4],
        frames[4].0,
        frames[4].1[0],
        12,
    );
    assert_eq!(heap.eax, Some(0x6801_0001));
    assert_eq!(runtime.heap(0x6801_0001).unwrap().initial_size, 0x1000);

    let module = dispatch(
        &dispatcher,
        &mut runtime,
        &endpoints[5],
        frames[5].0,
        frames[5].1[0],
        4,
    );
    assert_eq!(module.eax, Some(0x4d01_0001));
    let procedure = dispatch(
        &dispatcher,
        &mut runtime,
        &endpoints[6],
        frames[6].0,
        frames[6].1[0],
        8,
    );
    assert_eq!(procedure.eax, Some(0x7f02_0000));

    let tls = dispatch(
        &dispatcher,
        &mut runtime,
        &endpoints[7],
        frames[7].0,
        frames[7].1[0],
        0,
    );
    assert_eq!(tls.eax, Some(0));
    assert!(runtime.allocated_tls_indices().contains(&0));
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);

    // Synthetic modules resolve only the ordinal explicitly declared by their export table.
    let ordinal = dispatch(
        &dispatcher,
        &mut runtime,
        &endpoints[6],
        frames[8].0,
        frames[8].1[0],
        8,
    );
    assert_eq!(ordinal.eax, Some(0x7f02_0000));
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);

    // A missing export is the documented NULL plus ERROR_PROC_NOT_FOUND, not a fault.
    let missing_ordinal = dispatch(
        &dispatcher,
        &mut runtime,
        &endpoints[6],
        frames[9].0,
        frames[9].1[0],
        8,
    );
    assert_eq!(missing_ordinal.eax, Some(0));
    assert_eq!(runtime.current_last_error().unwrap(), 127);

    // A missing export named by string takes the same documented NULL + ERROR_PROC_NOT_FOUND path.
    let missing_name = dispatch(
        &dispatcher,
        &mut runtime,
        &endpoints[6],
        frames[12].0,
        frames[12].1[0],
        8,
    );
    assert_eq!(missing_name.eax, Some(0));
    assert_eq!(runtime.current_last_error().unwrap(), 127);

    // Both variants return the mapped image base, not the internal object handle.
    runtime
        .register_main_module(RuntimeModule {
            handle: 0x4d01_0002,
            requested_name: "fixture.exe".to_owned(),
            canonical_name: "fixture.exe".to_owned(),
            runtime_system_module: false,
        })
        .expect("register main executable module");
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.work/toolchain-spike/timer-wait.exe");
    let limits = Pe32Limits::default();
    let snapshot = ModuleSnapshot::from_bytes(fs::read(fixture).unwrap(), &limits).unwrap();
    let loader = build_pe32_load_plan(&snapshot, limits).unwrap();
    let image = materialize_pe32_image(
        &mut runtime.memory,
        &snapshot,
        &loader,
        GuestAddress(0x0040_0000),
    )
    .unwrap();
    runtime.memory.seal_owner(image.layout.owner).unwrap();
    runtime
        .register_module_image(0x4d01_0002, &loader, &image)
        .unwrap();
    acknowledge_pending_delta(&mut runtime.memory);
    runtime
        .set_current_last_error(INITIAL_LAST_ERROR)
        .expect("restore the LastError oracle after the not-found paths");
    let calling_process = dispatch(
        &dispatcher,
        &mut runtime,
        &endpoints[5],
        frames[10].0,
        frames[10].1[0],
        4,
    );
    assert_eq!(calling_process.eax, Some(0x0040_0000));
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);

    let mut ansi_module = endpoints[5].clone();
    ansi_module.symbol = crate::api::ApiSymbol::Name {
        name: "GetModuleHandleA".to_owned(),
    };
    let ansi_dispatcher = ApiDispatcher::from_endpoints([ansi_module.clone()]).unwrap();
    let calling_process = dispatch(
        &ansi_dispatcher,
        &mut runtime,
        &ansi_module,
        frames[10].0,
        frames[10].1[0],
        4,
    );
    assert_eq!(calling_process.eax, Some(0x0040_0000));
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);

    // An unloaded module is NULL plus ERROR_MOD_NOT_FOUND, not a Vine fault.
    let missing_module = dispatch(
        &dispatcher,
        &mut runtime,
        &endpoints[5],
        frames[11].0,
        frames[11].1[0],
        4,
    );
    assert_eq!(missing_module.eax, Some(0));
    assert_eq!(runtime.current_last_error().unwrap(), 126);

    // TlsSetValue deliberately performs only minimal validation for the guaranteed low-index
    // range, even if the caller did not obtain that index from TlsAlloc.
    runtime
        .set_current_last_error(INITIAL_LAST_ERROR)
        .expect("restore LastError before TLS validation");
    let low_unallocated_tls = dispatch(
        &dispatcher,
        &mut runtime,
        &endpoints[8],
        frames[13].0,
        frames[13].1[0],
        8,
    );
    assert_eq!(low_unallocated_tls.eax, Some(1));
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    assert_eq!(runtime.tls_value(7), Ok(0x1234_5678));

    // An unallocated index above TLS_MINIMUM_AVAILABLE is FALSE plus
    // ERROR_INVALID_PARAMETER, not a Vine fault.
    let tls_values_before = runtime.current_thread_state().unwrap().tls_values.clone();
    let invalid_tls = dispatch(
        &dispatcher,
        &mut runtime,
        &endpoints[8],
        frames[14].0,
        frames[14].1[0],
        8,
    );
    assert_eq!(invalid_tls.eax, Some(0));
    assert_eq!(runtime.current_last_error().unwrap(), 87);
    assert_eq!(
        runtime.current_thread_state().unwrap().tls_values,
        tls_values_before
    );
}

#[test]
fn load_library_a_reuses_an_attached_guest_dll_and_tracks_dynamic_references() {
    const MODULE_HANDLE: u32 = 0x4d01_0001;
    const MAIN_HANDLE: u32 = 0x4d01_0002;
    const DLL_NAME: u32 = OUTPUT_BASE + 0x400;
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 32,
        max_regions: 8,
        max_owners: 8,
        ..MemoryLimits::default()
    })
    .unwrap();
    let owner = memory.create_owner(MemoryOwnerKind::Runtime).unwrap();
    let range = memory
        .reserve_fixed(owner, MemoryRegionKind::Runtime, MEMORY_BASE, 0x3000)
        .unwrap();
    memory
        .commit(
            owner,
            range,
            PageProtection {
                read: true,
                write: true,
                execute: false,
            },
        )
        .unwrap();
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x180);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .unwrap();
    memory
        .write_initialization(owner, GuestAddress(DLL_NAME), b"LiFeCyClE_CoDeC.DlL\0")
        .unwrap();
    for (address, return_address) in [(STACK_BASE, 0x1000_0a01), (STACK_BASE + 0x20, 0x1000_0a02)] {
        write_frame(
            &mut memory,
            owner,
            GuestAddress(address),
            &[return_address, DLL_NAME],
        );
    }
    memory.seal_owner(owner).unwrap();
    let reflection = memory.pending_reflection().unwrap();
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();

    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(148),
        address: endpoint_address,
        requester_owner: owner,
        module: "KERNEL32.dll".to_owned(),
        iat_rva: Rva(0),
        iat_address: GuestAddress(IAT_BASE),
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "LoadLibraryA".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap();
    let mut runtime = controlled_runtime(memory, controlled_startup_info());
    for (handle, name, system) in [
        (MODULE_HANDLE, "lifecycle_codec.dll", false),
        (MAIN_HANDLE, "fixture.exe", false),
    ] {
        let module = RuntimeModule {
            handle,
            requested_name: name.to_owned(),
            canonical_name: name.to_owned(),
            runtime_system_module: system,
        };
        if handle == MAIN_HANDLE {
            runtime.register_main_module(module).unwrap();
        } else {
            runtime.load_module(module).unwrap();
        }
    }
    runtime
        .add_module_dependency(MAIN_HANDLE, MODULE_HANDLE)
        .unwrap();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".work/toolchain-spike/lifecycle_codec.dll");
    let bytes = fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}); run `npm run check:toolchain` first",
            path.display()
        )
    });
    let limits = Pe32Limits::default();
    let snapshot = ModuleSnapshot::from_bytes(bytes, &limits).unwrap();
    let loader = build_pe32_load_plan(&snapshot, limits).unwrap();
    let image = materialize_pe32_image(
        &mut runtime.memory,
        &snapshot,
        &loader,
        GuestAddress(0x3000_0000),
    )
    .unwrap();
    runtime.memory.seal_owner(image.layout.owner).unwrap();
    acknowledge_pending_delta(&mut runtime.memory);
    runtime
        .register_module_image(MODULE_HANDLE, &loader, &image)
        .unwrap();
    runtime.mark_module_ready(MODULE_HANDLE).unwrap();
    runtime.mark_module_ready(MAIN_HANDLE).unwrap();
    let dll_before_unattached = runtime
        .module_graph()
        .module_by_handle(MODULE_HANDLE)
        .unwrap()
        .clone();
    let main_before_unattached = runtime
        .module_graph()
        .module_by_handle(MAIN_HANDLE)
        .unwrap()
        .clone();
    let clock_before_unattached = runtime.clock().clone();
    let last_error_before_unattached = runtime.current_last_error().unwrap();
    let memory_before_unattached = runtime.memory.stats();
    let unattached = dispatcher.dispatch_owned(
        owned_trap(&endpoint, STACK_BASE, 0x1000_0a01),
        &mut runtime,
        None,
        dispatch_kernel32_api,
    );
    assert!(matches!(unattached, DispatchOutcome::Fault(_)));
    assert_eq!(
        runtime
            .module_graph()
            .module_by_handle(MODULE_HANDLE)
            .unwrap(),
        &dll_before_unattached
    );
    assert_eq!(
        runtime
            .module_graph()
            .module_by_handle(MAIN_HANDLE)
            .unwrap(),
        &main_before_unattached
    );
    assert_eq!(runtime.clock(), &clock_before_unattached);
    assert_eq!(
        runtime.current_last_error().unwrap(),
        last_error_before_unattached
    );
    assert_eq!(runtime.current_thread_id(), RuntimeThreadId(THREAD_ID));
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.memory.stats(), memory_before_unattached);

    runtime.attach_modules().unwrap();
    runtime.set_current_last_error(0x2468_1357).unwrap();
    let memory_before = runtime.memory.stats();

    for (address, return_address, expected_count) in [
        (STACK_BASE, 0x1000_0a01, 1),
        (STACK_BASE + 0x20, 0x1000_0a02, 2),
    ] {
        let outcome = dispatcher.dispatch_owned(
            owned_trap(&endpoint, address, return_address),
            &mut runtime,
            None,
            dispatch_kernel32_api,
        );
        let DispatchOutcome::Immediate(completion) = outcome else {
            panic!("expected attached guest DLL handle, got {outcome:?}");
        };
        assert_eq!(completion.eax, Some(0x3000_0000));
        assert_eq!(runtime.current_last_error().unwrap(), 0x2468_1357);
        let node = runtime
            .module_graph()
            .module_by_handle(MODULE_HANDLE)
            .unwrap();
        assert!(matches!(
            node.state,
            RuntimeModuleLoadState::ProcessAttached { .. }
        ));
        assert_eq!(
            node.reference_count_for(&RuntimeModuleReferenceOwner::DynamicLoad {
                thread_id: THREAD_ID,
            }),
            expected_count
        );
        assert_eq!(runtime.memory.stats(), memory_before);
    }

    let export = loader
        .exports
        .as_ref()
        .unwrap()
        .symbols
        .iter()
        .find(|symbol| matches!(symbol.target, crate::pe32::ExportTarget::Address { .. }))
        .unwrap();
    let crate::pe32::ExportTarget::Address { rva } = export.target else {
        unreachable!()
    };
    let mut procedure_endpoint = endpoint.clone();
    procedure_endpoint.symbol = crate::api::ApiSymbol::Name {
        name: "GetProcAddress".to_owned(),
    };
    let procedure_dispatcher = ApiDispatcher::from_endpoints([procedure_endpoint.clone()]).unwrap();
    runtime
        .memory
        .write(
            GuestAddress(STACK_BASE),
            &[0x1000_0a01, 0x3000_0000, export.ordinal]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        )
        .unwrap();
    acknowledge_pending_delta(&mut runtime.memory);
    let procedure = dispatch_owned(
        &procedure_dispatcher,
        &mut runtime,
        &procedure_endpoint,
        STACK_BASE,
        0x1000_0a01,
    );
    assert_eq!(procedure.eax, Some(0x3000_0000 + rva.0));
}

#[test]
fn load_library_a_reuses_registered_runtime_modules_without_export_configuration() {
    const D3DX_NAME: u32 = OUTPUT_BASE + 0x400;
    const EMPTY_NAME: u32 = OUTPUT_BASE + 0x440;
    const MISSING_NAME: u32 = OUTPUT_BASE + 0x450;
    const PATH_NAME: u32 = OUTPUT_BASE + 0x470;
    const NON_ASCII_NAME: u32 = OUTPUT_BASE + 0x4a0;

    let (mut memory, owner) = initialized_memory();
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x180);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .unwrap();
    for (address, bytes) in [
        (D3DX_NAME, b"D3DX9_43.DLL\0".as_slice()),
        (EMPTY_NAME, b"\0".as_slice()),
        (MISSING_NAME, b"missing.dll\0".as_slice()),
        (PATH_NAME, b"C:\\Windows\\d3dx9_43.dll\0".as_slice()),
        (NON_ASCII_NAME, &[0x82, 0xa0, 0]),
    ] {
        memory
            .write_initialization(owner, GuestAddress(address), bytes)
            .unwrap();
    }
    let frames = [
        (STACK_BASE, vec![0x1000_0a01, 0]),
        (STACK_BASE + 0x20, vec![0x1000_0a02, EMPTY_NAME]),
        (STACK_BASE + 0x40, vec![0x1000_0a03, MISSING_NAME]),
        (STACK_BASE + 0x60, vec![0x1000_0a04, PATH_NAME]),
        (STACK_BASE + 0x80, vec![0x1000_0a05, NON_ASCII_NAME]),
        (STACK_BASE + 0xa0, vec![0x1000_0a06, D3DX_NAME]),
        (STACK_BASE + 0xc0, vec![0x1000_0a07, D3DX_NAME]),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    memory.seal_owner(owner).unwrap();
    let reflection = memory.pending_reflection().unwrap();
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();

    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(148),
        address: endpoint_address,
        requester_owner: owner,
        module: "KERNEL32.dll".to_owned(),
        iat_rva: Rva(0),
        iat_address: GuestAddress(IAT_BASE),
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "LoadLibraryA".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });

    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap();

    let mut runtime = controlled_runtime(memory, controlled_startup_info());
    runtime
        .register_source_snapshot(vine_core::RuntimeSourceSnapshotManifest {
            mount: vine_core::RuntimeSourceMountManifest {
                source_id: 1,
                root_path_a: b"C:\\Vine\0".to_vec(),
            },
            files: vec![vine_core::RuntimeSourceFileManifest {
                object_id: 1,
                relative_path_a: b"fixture.exe".to_vec(),
                byte_length: 1,
            }],
        })
        .unwrap();
    runtime
        .load_module(RuntimeModule {
            handle: 0x4d01_0001,
            requested_name: "kernel32.dll".to_owned(),
            canonical_name: "kernel32.dll".to_owned(),
            runtime_system_module: true,
        })
        .unwrap();
    runtime
        .register_main_module(RuntimeModule {
            handle: 0x4d01_0002,
            requested_name: "fixture.exe".to_owned(),
            canonical_name: "fixture.exe".to_owned(),
            runtime_system_module: false,
        })
        .unwrap();
    runtime
        .add_module_dependency(0x4d01_0002, 0x4d01_0001)
        .unwrap();
    runtime.mark_module_ready(0x4d01_0001).unwrap();
    runtime.mark_module_ready(0x4d01_0002).unwrap();
    runtime
        .load_module(RuntimeModule {
            handle: 0x4d01_0003,
            requested_name: "d3dx9_43.dll".to_owned(),
            canonical_name: "d3dx9_43.dll".to_owned(),
            runtime_system_module: true,
        })
        .unwrap();
    runtime.mark_module_ready(0x4d01_0003).unwrap();
    runtime.attach_modules().unwrap();

    let kernel_before = runtime
        .module_graph()
        .module_by_handle(0x4d01_0001)
        .unwrap()
        .clone();
    let main_before = runtime
        .module_graph()
        .module_by_handle(0x4d01_0002)
        .unwrap()
        .clone();
    let d3dx_before = runtime
        .module_graph()
        .module_by_handle(0x4d01_0003)
        .unwrap()
        .clone();
    let clock_before = runtime.clock().clone();
    for (index, expected_error) in [(0, 87), (1, 87), (2, 126)] {
        let completion = dispatch_owned(
            &dispatcher,
            &mut runtime,
            &endpoint,
            frames[index].0,
            frames[index].1[0],
        );
        assert_eq!(completion.eax, Some(0));
        assert_eq!(runtime.current_last_error().unwrap(), expected_error);
        assert_eq!(
            runtime
                .module_graph()
                .module_by_handle(0x4d01_0001)
                .unwrap(),
            &kernel_before
        );
        assert_eq!(
            runtime
                .module_graph()
                .module_by_handle(0x4d01_0002)
                .unwrap(),
            &main_before
        );
        assert_eq!(
            runtime
                .module_graph()
                .module_by_handle(0x4d01_0003)
                .unwrap(),
            &d3dx_before
        );
        assert_eq!(runtime.clock(), &clock_before);
        assert_eq!(runtime.current_thread_id(), RuntimeThreadId(THREAD_ID));
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::Runnable
        );
        assert_eq!(runtime.current_last_error().unwrap(), expected_error);
    }

    for index in [3_usize, 4] {
        let memory_before = runtime.memory.stats();
        let outcome = {
            dispatcher.dispatch_owned(
                owned_trap(&endpoint, frames[index].0, frames[index].1[0]),
                &mut runtime,
                None,
                dispatch_kernel32_api,
            )
        };
        assert!(matches!(outcome, DispatchOutcome::Fault(_)));
        assert_eq!(
            runtime
                .module_graph()
                .module_by_handle(0x4d01_0001)
                .unwrap(),
            &kernel_before
        );
        assert_eq!(
            runtime
                .module_graph()
                .module_by_handle(0x4d01_0002)
                .unwrap(),
            &main_before
        );
        assert_eq!(
            runtime
                .module_graph()
                .module_by_handle(0x4d01_0003)
                .unwrap(),
            &d3dx_before
        );
        assert_eq!(runtime.clock(), &clock_before);
        assert_eq!(runtime.current_thread_id(), RuntimeThreadId(THREAD_ID));
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::Runnable
        );
        assert_eq!(runtime.current_last_error().unwrap(), 126);
        assert_eq!(runtime.memory.stats(), memory_before);
    }

    runtime.set_current_last_error(INITIAL_LAST_ERROR).unwrap();
    let memory_before_success = runtime.memory.stats();
    let first = dispatch_owned(
        &dispatcher,
        &mut runtime,
        &endpoint,
        frames[5].0,
        frames[5].1[0],
    );
    assert_eq!(first.eax, Some(0x4d01_0003));
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    assert_eq!(runtime.memory.stats(), memory_before_success);
    let loaded = runtime.module_by_name("d3dx9_43.dll").unwrap().unwrap();
    assert_eq!(loaded.handle, 0x4d01_0003);
    let node = runtime
        .module_graph()
        .module_by_handle(loaded.handle)
        .unwrap();
    assert!(matches!(
        node.state,
        RuntimeModuleLoadState::ProcessAttached { .. }
    ));
    assert_eq!(
        node.reference_count_for(&RuntimeModuleReferenceOwner::DynamicLoad {
            thread_id: THREAD_ID,
        }),
        1
    );

    let object_id = runtime
        .object_for_handle(RuntimeHandle(0x4d01_0003), RuntimeObjectKind::Module)
        .unwrap()
        .id;
    let second = dispatch_owned(
        &dispatcher,
        &mut runtime,
        &endpoint,
        frames[6].0,
        frames[6].1[0],
    );
    assert_eq!(second.eax, Some(0x4d01_0003));
    assert_eq!(
        runtime
            .object_for_handle(RuntimeHandle(0x4d01_0003), RuntimeObjectKind::Module)
            .unwrap()
            .id,
        object_id
    );
    assert_eq!(
        runtime
            .module_graph()
            .module_by_handle(0x4d01_0003)
            .unwrap()
            .reference_count_for(&RuntimeModuleReferenceOwner::DynamicLoad {
                thread_id: THREAD_ID,
            }),
        2
    );
}

#[test]
fn pe32_frames_reject_wrong_return_address_and_wrapping_stack_before_dispatch() {
    let (mut memory, owner) = initialized_memory();
    let (dispatcher, endpoints, frames) = suspended_dispatcher_fixture(&mut memory, owner);
    let mut runtime = controlled_runtime(memory, controlled_startup_info());
    let valid = owned_trap(&endpoints[0], frames[0].0, frames[0].1[0]);
    let clock_before = runtime.clock().clone();
    let memory_before = runtime.memory.stats();
    for invalid in [
        ApiOwnedTrap {
            return_address: GuestAddress(valid.return_address.0 ^ 1),
            ..valid
        },
        ApiOwnedTrap {
            stack_pointer: GuestAddress(0xffff_fffc),
            ..valid
        },
    ] {
        let handler_called = Cell::new(false);
        let result = {
            dispatcher.dispatch_owned(invalid, &mut runtime, None, |_, _, _| {
                handler_called.set(true);
                ApiHandlerResult::Immediate(ApiImmediate::new(Some(42)))
            })
        };
        assert!(matches!(result, DispatchOutcome::Fault(_)));
        assert!(!handler_called.get());
        assert_eq!(runtime.clock(), &clock_before);
        assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
        assert_eq!(runtime.current_thread_id(), RuntimeThreadId(THREAD_ID));
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::Runnable
        );
        assert_eq!(runtime.memory.stats(), memory_before);
    }
}

#[test]
fn mapped_final_stack_word_is_rejected_before_the_handler_can_mutate_state() {
    let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
    let owner = memory.create_owner(MemoryOwnerKind::Runtime).unwrap();
    let range = memory
        .reserve_fixed(
            owner,
            MemoryRegionKind::Runtime,
            GuestAddress(0xffff_f000),
            GUEST_PAGE_SIZE,
        )
        .unwrap();
    memory
        .commit(
            owner,
            range,
            PageProtection {
                read: true,
                write: true,
                execute: false,
            },
        )
        .unwrap();
    let return_address = GuestAddress(0x1000_0201);
    let stack_pointer = GuestAddress(0xffff_fffc);
    memory
        .write_initialization(owner, stack_pointer, &return_address.0.to_le_bytes())
        .unwrap();
    memory.seal_owner(owner).unwrap();
    let reflection = memory.pending_reflection().unwrap();
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
    assert_eq!(
        memory.read(stack_pointer, 4).unwrap(),
        return_address.0.to_le_bytes()
    );
    let endpoint = ApiEndpointIdentity::from_dynamic_endpoint(&RuntimeDynamicEndpoint {
        id: 1,
        address: GuestAddress(0x7f02_0000),
        module: "fixture.dll".to_owned(),
        name: "NoArguments".to_owned(),
        ordinal: None,
    });
    {
        let dispatcher = ApiDispatcher::with_argument_words([(endpoint.clone(), 0)]).unwrap();
        let mut runtime = controlled_runtime(memory, controlled_startup_info());
        let clock_before = runtime.clock().clone();
        let memory_before = runtime.memory.stats();
        let called = Cell::new(false);
        let result = dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: endpoint.id,
                endpoint_address: endpoint.address,
                stack_pointer,
                return_address,
            },
            &mut runtime,
            None,
            |_, _, _| {
                called.set(true);
                ApiHandlerResult::Immediate(ApiImmediate::new(Some(42)).with_last_error(99))
            },
        );
        assert!(matches!(result, DispatchOutcome::Fault(_)));
        assert!(!called.get());
        assert_eq!(runtime.clock(), &clock_before);
        assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
        assert_eq!(runtime.current_thread_id(), RuntimeThreadId(THREAD_ID));
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::Runnable
        );
        assert_eq!(runtime.memory.stats(), memory_before);
    }
}

#[test]
fn stdcall_cleanup_includes_return_address_and_argument_words() {
    {
        let cleanup_bytes = 12;
        let (mut memory, owner) = initialized_memory();
        let (_, endpoints, frames) = suspended_dispatcher_fixture(&mut memory, owner);
        let endpoint = endpoints[0].clone();
        let dispatcher = ApiDispatcher::with_argument_words([(endpoint.clone(), 2)]).unwrap();
        let mut runtime = controlled_runtime(memory, controlled_startup_info());
        let completion = dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[0].0, frames[0].1[0]),
            &mut runtime,
            None,
            |frame, _, _| {
                assert_eq!(frame.arguments, [19, 23]);
                ApiHandlerResult::Immediate(ApiImmediate::new(Some(42)))
            },
        );
        let DispatchOutcome::Immediate(completion) = completion else {
            panic!("expected immediate completion, got {completion:?}");
        };
        assert_eq!(completion.eax, Some(42));
        assert_eq!(completion.instruction_pointer, GuestAddress(frames[0].1[0]));
        assert_eq!(
            completion.stack_pointer,
            GuestAddress(frames[0].0 + cleanup_bytes)
        );
        assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    }
}

#[test]
fn deferred_and_guest_callback_completions_preserve_the_decoded_abi_frame() {
    let (mut memory, owner) = initialized_memory();
    let (dispatcher, endpoints, frames) = suspended_dispatcher_fixture(&mut memory, owner);
    let mut runtime = controlled_runtime(memory, controlled_startup_info());

    let deferred = {
        dispatcher.dispatch_owned(
            owned_trap(&endpoints[0], frames[0].0, frames[0].1[0]),
            &mut runtime,
            None,
            |frame, _, _| {
                assert_eq!(frame.argument_u32(0), Some(19));
                assert_eq!(frame.argument_u32(1), Some(23));
                ApiHandlerResult::Deferred(HostRequest {
                    request_id: 17,
                    service: "browser.deferred-add".to_owned(),
                    payload: vec![19, 23],
                })
            },
        )
    };
    let mut deferred = match deferred {
        DispatchOutcome::Deferred(call) => call,
        other => panic!("expected deferred dispatch, got {other:?}"),
    };
    assert_eq!(deferred.frame.arguments, [19, 23]);
    assert_eq!(deferred.frame.argument_u32(2), None);
    for arguments in [vec![19], vec![19, 23, 99]] {
        let original_arguments = std::mem::replace(&mut deferred.frame.arguments, arguments);
        let clock_before = runtime.clock().clone();
        let last_error_before = runtime.current_last_error().unwrap();
        let thread_before = runtime.current_thread_id();
        let scheduler_before = runtime.scheduler_state().unwrap();
        let memory_before = runtime.memory.stats();
        let result = {
            dispatcher.complete_deferred(
                &deferred,
                HostCompletion {
                    request_id: 17,
                    result: ApiImmediate::new(Some(42)),
                },
                &mut runtime,
            )
        };
        assert!(matches!(result, DispatchOutcome::Fault(_)));
        assert_eq!(runtime.clock(), &clock_before);
        assert_eq!(runtime.current_last_error().unwrap(), last_error_before);
        assert_eq!(runtime.current_thread_id(), thread_before);
        assert_eq!(runtime.scheduler_state().unwrap(), scheduler_before);
        assert_eq!(runtime.memory.stats(), memory_before);
        deferred.frame.arguments = original_arguments;
    }
    let deferred_completion = {
        dispatcher.complete_deferred(
            &deferred,
            HostCompletion {
                request_id: 17,
                result: ApiImmediate::new(Some(42)),
            },
            &mut runtime,
        )
    };
    let deferred_completion = match deferred_completion {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected deferred completion, got {other:?}"),
    };
    assert_eq!(deferred_completion.eax, Some(42));
    assert_eq!(
        deferred_completion.instruction_pointer,
        GuestAddress(frames[0].1[0])
    );
    assert_eq!(
        deferred_completion.stack_pointer,
        GuestAddress(frames[0].0 + 12)
    );

    let callback = {
        dispatcher.dispatch_owned(
            owned_trap(&endpoints[1], frames[1].0, frames[1].1[0]),
            &mut runtime,
            None,
            |frame, _, _| {
                ApiHandlerResult::GuestCallback(Box::new(GuestCallbackRequest {
                    target: GuestAddress(frame.argument_u32(0).expect("callback target")),
                    arguments: vec![GuestCallbackArgument::U32(
                        frame.argument_u32(1).expect("callback value"),
                    )],
                    scratch: Vec::new(),
                    scratch_pointer_relocations: Vec::new(),
                    return_sentinel: GuestAddress(0x7f01_0000),
                    completion_policy: GuestCallbackCompletionPolicy::ReturnU32,
                }))
            },
        )
    };
    let callback = match callback {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected guest callback dispatch, got {other:?}"),
    };
    assert_eq!(callback.request.arguments, [GuestCallbackArgument::U32(42)]);
    let callback_completion =
        { dispatcher.complete_guest_callback_result(callback, 44, &mut runtime) };
    let callback_completion = match callback_completion {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected callback completion, got {other:?}"),
    };
    assert_eq!(callback_completion.eax, Some(44));
    assert_eq!(
        callback_completion.instruction_pointer,
        GuestAddress(frames[1].1[0])
    );
    assert_eq!(
        callback_completion.stack_pointer,
        GuestAddress(frames[1].0 + 12)
    );
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
}

#[test]
fn raise_exception_dispatches_the_bounded_x86_seh_chain() {
    const API_STACK: GuestAddress = GuestAddress(0x006f_f800);
    const ARGUMENTS: GuestAddress = GuestAddress(0x006f_f840);
    const REGISTRATION_ONE: GuestAddress = GuestAddress(0x006f_ff00);
    const REGISTRATION_TWO: GuestAddress = GuestAddress(0x006f_ff20);
    const HANDLER_PAGE: GuestAddress = GuestAddress(0x3000_0000);
    const HANDLER_ONE: GuestAddress = GuestAddress(0x3000_0100);
    const HANDLER_TWO: GuestAddress = GuestAddress(0x3000_0200);
    const RETURN_ADDRESS: GuestAddress = GuestAddress(0x100a_3c74);
    const EXCEPTION_CODE: u32 = 0xe06d_7363;

    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 32,
        max_regions: 32,
        max_owners: 32,
        max_access_bytes: 0x1_0000,
    })
    .expect("SEH memory");
    let api_owner = memory
        .create_owner(MemoryOwnerKind::Runtime)
        .expect("API owner");
    let api_range = memory
        .reserve_fixed(api_owner, MemoryRegionKind::Runtime, MEMORY_BASE, 0x3000)
        .expect("API reservation");
    memory
        .commit(
            api_owner,
            api_range,
            PageProtection {
                read: true,
                write: true,
                execute: false,
            },
        )
        .expect("API commit");

    let raise_thunk = ImportThunk {
        id: ImportThunkId(1),
        address: GuestAddress(ENDPOINT_BASE),
        requester_owner: api_owner,
        module: "KERNEL32.dll".to_owned(),
        iat_rva: Rva(0),
        iat_address: GuestAddress(IAT_BASE),
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "RaiseException".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    };
    let unwind_thunk = ImportThunk {
        id: ImportThunkId(2),
        address: GuestAddress(ENDPOINT_BASE + 0x10),
        requester_owner: api_owner,
        module: "KERNEL32.dll".to_owned(),
        iat_rva: Rva(4),
        iat_address: GuestAddress(IAT_BASE + 4),
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "RtlUnwind".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    };
    memory
        .write_initialization(
            api_owner,
            raise_thunk.iat_address,
            &raise_thunk.address.0.to_le_bytes(),
        )
        .expect("RaiseException IAT");
    memory
        .write_initialization(
            api_owner,
            unwind_thunk.iat_address,
            &unwind_thunk.address.0.to_le_bytes(),
        )
        .expect("RtlUnwind IAT");
    memory.seal_owner(api_owner).expect("seal API owner");
    let endpoint = ApiEndpointIdentity::from_import_thunk(&raise_thunk);
    let unwind_endpoint = ApiEndpointIdentity::from_import_thunk(&unwind_thunk);
    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone(), unwind_endpoint.clone()])
        .expect("SEH dispatcher");

    let mut runtime = controlled_runtime(memory, controlled_startup_info());
    let callback_allocation = runtime
        .process_address_space_allocator()
        .allocation_for_purpose(&ProcessAddressAllocationPurpose::CallbackReturns)
        .expect("callback return allocation")
        .clone();
    let sentinels = install_runtime_return_sentinels(&mut runtime.memory, &callback_allocation)
        .expect("runtime sentinels");
    let initial_stack = install_initial_thread_stack(
        &mut runtime.memory,
        &sentinels.process_entry_return,
        0x0010_0000,
        GUEST_PAGE_SIZE,
    )
    .expect("initial stack");
    runtime
        .register_initial_thread_stack(&initial_stack)
        .expect("register initial stack");
    runtime
        .install_process_environment("seh-test.exe")
        .expect("process environment");

    let handler_owner = runtime
        .memory
        .create_owner(MemoryOwnerKind::Anonymous)
        .expect("handler owner");
    let handler_range = runtime
        .memory
        .reserve_fixed(
            handler_owner,
            MemoryRegionKind::Anonymous,
            HANDLER_PAGE,
            GUEST_PAGE_SIZE,
        )
        .expect("handler reservation");
    runtime
        .memory
        .commit(
            handler_owner,
            handler_range,
            PageProtection {
                read: true,
                write: false,
                execute: true,
            },
        )
        .expect("handler commit");
    runtime
        .memory
        .write_initialization(handler_owner, HANDLER_ONE, &[0x90])
        .expect("first handler byte");
    runtime
        .memory
        .write_initialization(handler_owner, HANDLER_TWO, &[0x90])
        .expect("second handler byte");
    runtime
        .memory
        .seal_owner(handler_owner)
        .expect("seal handler owner");

    let teb = runtime
        .current_thread_teb_address()
        .expect("current thread TEB");
    runtime
        .memory
        .write(teb, &REGISTRATION_ONE.0.to_le_bytes())
        .expect("SEH chain head");
    runtime
        .memory
        .write(
            REGISTRATION_ONE,
            &[
                REGISTRATION_TWO.0.to_le_bytes(),
                HANDLER_ONE.0.to_le_bytes(),
            ]
            .concat(),
        )
        .expect("first SEH registration");
    runtime
        .memory
        .write(
            REGISTRATION_TWO,
            &[u32::MAX.to_le_bytes(), HANDLER_TWO.0.to_le_bytes()].concat(),
        )
        .expect("second SEH registration");
    runtime
        .memory
        .write(
            ARGUMENTS,
            &[0x1993_0520_u32.to_le_bytes(), 0x006f_eb44_u32.to_le_bytes()].concat(),
        )
        .expect("exception arguments");
    let frame_words = [RETURN_ADDRESS.0, EXCEPTION_CODE, 1, 2, ARGUMENTS.0];
    runtime
        .memory
        .write(
            API_STACK,
            &frame_words
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        )
        .expect("RaiseException frame");
    let reflection = runtime.memory.pending_reflection().expect("SEH reflection");
    runtime
        .memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge SEH reflection");

    let cpu_context = X86CpuContext {
        eax: 0x1111_1111,
        ecx: 0x2222_2222,
        edx: 0x3333_3333,
        ebx: 0x4444_4444,
        esp: API_STACK.0,
        ebp: 0x006f_fa00,
        esi: 0x5555_5555,
        edi: 0x6666_6666,
        eip: endpoint.address.0,
        eflags: 0x0000_0202,
        es: 0x23,
        cs: 0x1b,
        ss: 0x23,
        ds: 0x23,
        fs: 0x3b,
        gs: 0,
    };
    let dispatched = {
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, API_STACK.0, RETURN_ADDRESS.0),
            &mut runtime,
            Some((cpu_context, sentinels.callback_return)),
            dispatch_kernel32_api,
        )
    };
    let call = match dispatched {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected SEH callback dispatch, got {other:?}"),
    };
    let unwind_call = call.clone();
    assert_eq!(call.request.target, HANDLER_ONE);
    assert_eq!(
        call.request.arguments,
        [
            GuestCallbackArgument::ScratchPointer { offset: 0 },
            GuestCallbackArgument::U32(REGISTRATION_ONE.0),
            GuestCallbackArgument::ScratchPointer { offset: 80 },
            GuestCallbackArgument::U32(0),
        ]
    );
    assert_eq!(read_slice_u32(&call.request.scratch, 0), EXCEPTION_CODE);
    assert_eq!(read_slice_u32(&call.request.scratch, 4), 1);
    assert_eq!(read_slice_u32(&call.request.scratch, 12), RETURN_ADDRESS.0);
    assert_eq!(read_slice_u32(&call.request.scratch, 16), 2);
    assert_eq!(read_slice_u32(&call.request.scratch, 20), 0x1993_0520);
    assert_eq!(read_slice_u32(&call.request.scratch, 24), 0x006f_eb44);
    assert_eq!(read_slice_u32(&call.request.scratch, 80), 0x0001_0007);
    assert_eq!(
        read_slice_u32(&call.request.scratch, 80 + 176),
        cpu_context.eax
    );
    assert_eq!(
        read_slice_u32(&call.request.scratch, 80 + 184),
        RETURN_ADDRESS.0
    );
    assert_eq!(
        read_slice_u32(&call.request.scratch, 80 + 196),
        API_STACK.0 + 20
    );
    assert!(matches!(
        call.request.completion_policy,
        GuestCallbackCompletionPolicy::ExceptionDispatch {
            next_frame: REGISTRATION_TWO,
            exception_flags: 1,
        }
    ));

    let continued = { dispatcher.complete_guest_callback_result(call, 1, &mut runtime) };
    let next = match continued {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected next SEH handler, got {other:?}"),
    };
    assert_eq!(next.request.target, HANDLER_TWO);
    assert_eq!(
        next.request.arguments[1],
        GuestCallbackArgument::U32(REGISTRATION_TWO.0)
    );
    assert!(matches!(
        next.request.completion_policy,
        GuestCallbackCompletionPolicy::ExceptionDispatch {
            next_frame: GuestAddress(u32::MAX),
            exception_flags: 1,
        }
    ));

    let prepared = runtime
        .begin_guest_callback(unwind_call.clone())
        .expect("materialize SEH callback");
    let callback_delta = runtime.memory.pending_delta().expect("SEH callback delta");
    runtime
        .memory
        .acknowledge_delta(callback_delta.generation)
        .expect("acknowledge SEH callback delta");
    const UNWIND_STACK: GuestAddress = GuestAddress(0x006f_f700);
    let unwind_words = [
        HANDLER_ONE.0,
        REGISTRATION_ONE.0,
        HANDLER_ONE.0,
        prepared.scratch_address.0,
        0x1234_5678,
    ];
    runtime
        .memory
        .write(
            UNWIND_STACK,
            &unwind_words
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        )
        .expect("RtlUnwind frame");
    let unwind_frame_delta = runtime
        .memory
        .pending_delta()
        .expect("RtlUnwind frame delta");
    runtime
        .memory
        .acknowledge_delta(unwind_frame_delta.generation)
        .expect("acknowledge RtlUnwind frame delta");
    let unwind_context = X86CpuContext {
        esp: UNWIND_STACK.0,
        eip: unwind_endpoint.address.0,
        ..cpu_context
    };
    let unwind = runtime.dispatch_api_call(
        &dispatcher,
        owned_trap(&unwind_endpoint, UNWIND_STACK.0, HANDLER_ONE.0),
        sentinels.callback_return,
        Some(unwind_context),
    );
    let unwind = match unwind {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected bounded RtlUnwind completion, got {other:?}"),
    };
    assert_eq!(unwind.eax, Some(0x1234_5678));
    assert_eq!(unwind.instruction_pointer, HANDLER_ONE);
    assert_eq!(unwind.stack_pointer, GuestAddress(UNWIND_STACK.0 + 20));
    assert!(runtime.pending_api_callbacks.is_empty());
    assert!(
        runtime
            .thread_state(runtime.current_thread_id())
            .unwrap()
            .callback_frames
            .is_empty()
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        vine_core::SchedulerThreadState::Runnable
    );
    let unwind_delta = runtime.memory.pending_delta().expect("RtlUnwind delta");
    runtime
        .memory
        .acknowledge_delta(unwind_delta.generation)
        .expect("acknowledge RtlUnwind delta");
    assert_eq!(
        u32::from_le_bytes(runtime.memory.read(teb, 4).unwrap().try_into().unwrap()),
        REGISTRATION_TWO.0
    );
    assert_eq!(
        u32::from_le_bytes(
            runtime
                .memory
                .read(GuestAddress(prepared.scratch_address.0 + 4), 4)
                .unwrap()
                .try_into()
                .unwrap()
        ),
        3
    );

    runtime
        .memory
        .write(teb, &REGISTRATION_ONE.0.to_le_bytes())
        .expect("restore SEH chain for negative case");
    runtime
        .memory
        .write(
            GuestAddress(prepared.scratch_address.0 + 4),
            &1_u32.to_le_bytes(),
        )
        .expect("restore exception flags for negative case");

    runtime
        .memory
        .write(GuestAddress(API_STACK.0 + 12), &16_u32.to_le_bytes())
        .expect("invalid exception count");
    let delta = runtime.memory.pending_delta().expect("invalid count delta");
    runtime
        .memory
        .acknowledge_delta(delta.generation)
        .expect("acknowledge invalid count delta");
    let clock_before = runtime.clock().clone();
    let last_error_before = runtime.current_last_error().unwrap();
    let thread_before = runtime.current_thread_id();
    let scheduler_before = runtime.scheduler_state().unwrap();
    let memory_before = runtime.memory.stats();
    let invalid = {
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, API_STACK.0, RETURN_ADDRESS.0),
            &mut runtime,
            Some((cpu_context, sentinels.callback_return)),
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(invalid, DispatchOutcome::Fault(_)));
    assert_eq!(runtime.clock(), &clock_before);
    assert_eq!(runtime.current_last_error().unwrap(), last_error_before);
    assert_eq!(runtime.current_thread_id(), thread_before);
    assert_eq!(runtime.scheduler_state().unwrap(), scheduler_before);
    assert_eq!(runtime.memory.stats(), memory_before);
}

fn read_slice_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32 slice"))
}

#[test]
fn wait_any_rejects_invalid_later_handle_before_consuming_a_signal_and_allows_repair() {
    let (memory, owner) = initialized_memory();
    let mut runtime = controlled_runtime(memory, controlled_startup_info());
    let first = runtime.create_event(false, true).unwrap();
    let second = runtime.create_event(false, true).unwrap();
    let endpoint = ApiEndpointIdentity::from_dynamic_endpoint(&RuntimeDynamicEndpoint {
        id: 1,
        address: GuestAddress(0x7f02_0000),
        module: "kernel32.dll".to_owned(),
        name: "WaitForMultipleObjects".to_owned(),
        ordinal: None,
    });
    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap();
    let return_address = 0x1000_0201;
    write_frame(
        &mut runtime.memory,
        owner,
        GuestAddress(STACK_BASE),
        &[return_address, 2, OUTPUT_BASE, 0, 0],
    );
    write_frame(
        &mut runtime.memory,
        owner,
        GuestAddress(OUTPUT_BASE),
        &[first.0, u32::MAX],
    );
    runtime.memory.seal_owner(owner).unwrap();
    let reflection = runtime.memory.pending_reflection().unwrap();
    runtime
        .memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
    let memory_before = runtime.memory.stats();

    let failed = dispatch_owned(
        &dispatcher,
        &mut runtime,
        &endpoint,
        STACK_BASE,
        return_address,
    );
    assert_eq!(failed.eax, Some(u32::MAX));
    assert_eq!(runtime.current_last_error().unwrap(), 6);
    assert_eq!(
        runtime.event_for_handle(first).unwrap().wait_state,
        RuntimeWaitState::Signaled { exit_code: 0 }
    );
    assert_eq!(
        runtime.event_for_handle(second).unwrap().wait_state,
        RuntimeWaitState::Signaled { exit_code: 0 }
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert!(runtime.scheduler().host_requests().is_empty());
    assert!(runtime.scheduler().wait_registrations().is_empty());
    assert_eq!(runtime.memory.stats(), memory_before);

    runtime
        .memory
        .write(GuestAddress(OUTPUT_BASE + 4), &second.0.to_le_bytes())
        .unwrap();
    acknowledge_pending_delta(&mut runtime.memory);
    let repaired = dispatch_owned(
        &dispatcher,
        &mut runtime,
        &endpoint,
        STACK_BASE,
        return_address,
    );
    assert_eq!(repaired.eax, Some(0));
    assert_eq!(runtime.current_last_error().unwrap(), 6);
    assert_eq!(
        runtime.event_for_handle(first).unwrap().wait_state,
        RuntimeWaitState::Nonsignaled
    );
    assert_eq!(
        runtime.event_for_handle(second).unwrap().wait_state,
        RuntimeWaitState::Signaled { exit_code: 0 }
    );
    let next = dispatch_owned(
        &dispatcher,
        &mut runtime,
        &endpoint,
        STACK_BASE,
        return_address,
    );
    assert_eq!(next.eax, Some(1));
    assert_eq!(
        runtime.event_for_handle(second).unwrap().wait_state,
        RuntimeWaitState::Nonsignaled
    );
}

#[test]
fn callback_preflight_and_encoding_share_pointer_and_relocation_checks_without_mutation() {
    let (mut memory, owner) = initialized_memory();
    memory
        .protect(
            owner,
            GuestRange::new(MEMORY_BASE, 0x3000).unwrap(),
            PageProtection {
                read: true,
                write: true,
                execute: true,
            },
        )
        .unwrap();
    memory.seal_owner(owner).unwrap();
    let reflection = memory.pending_reflection().unwrap();
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
    let runtime = controlled_runtime(memory, controlled_startup_info());
    let endpoint = ApiEndpointIdentity::from_dynamic_endpoint(&RuntimeDynamicEndpoint {
        id: 1,
        address: GuestAddress(0x7f02_0000),
        module: "fixture.dll".to_owned(),
        name: "Callback".to_owned(),
        ordinal: None,
    });
    let frame = super::ApiCallFrame {
        endpoint,
        stack_pointer: GuestAddress(STACK_BASE),
        return_address: GuestAddress(MEMORY_BASE.0 + 0x200),
        arguments: Vec::new(),
    };
    let request = GuestCallbackRequest {
        target: GuestAddress(MEMORY_BASE.0 + 0x200),
        arguments: vec![GuestCallbackArgument::ScratchPointer { offset: 4 }],
        scratch: vec![0; 8],
        scratch_pointer_relocations: vec![super::GuestCallbackScratchPointerRelocation {
            write_offset: 0,
            target_offset: 4,
        }],
        return_sentinel: GuestAddress(MEMORY_BASE.0 + 0x204),
        completion_policy: GuestCallbackCompletionPolicy::ReturnU32,
    };
    let scratch = runtime
        .prepare_callback_frame(None, request.target, request.return_sentinel, 8)
        .unwrap()
        .scratch;
    let before = runtime.memory.stats();
    runtime.preflight_guest_callback(&frame, &request).unwrap();
    let image =
        super::prepare_guest_callback_frame(&runtime.memory, &frame, &request, scratch).unwrap();
    let pointer = (image.scratch_address.0 + 4).to_le_bytes();
    assert_eq!(&image.stack[..4], &request.return_sentinel.0.to_le_bytes());
    assert_eq!(&image.stack[4..], &pointer);
    assert_eq!(&image.scratch[..4], &pointer);
    assert_eq!(&image.scratch[4..], &[0; 4]);
    assert_eq!(request.scratch, [0; 8]);
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(STACK_BASE - 8), 8)
            .unwrap(),
        [0; 8]
    );
    for (argument_offset, write_offset, target_offset) in [
        (2, 0, 4),
        (8, 0, 4),
        (4, 2, 4),
        (4, 8, 4),
        (4, 0, 8),
        (4, u32::MAX, 4),
    ] {
        let mut invalid = request.clone();
        invalid.arguments[0] = GuestCallbackArgument::ScratchPointer {
            offset: argument_offset,
        };
        invalid.scratch_pointer_relocations[0] = super::GuestCallbackScratchPointerRelocation {
            write_offset,
            target_offset,
        };
        assert!(runtime.preflight_guest_callback(&frame, &invalid).is_err());
        assert!(
            super::prepare_guest_callback_frame(&runtime.memory, &frame, &invalid, scratch)
                .is_err()
        );
        assert_eq!(runtime.memory.stats(), before);
        assert!(
            runtime
                .current_thread_state()
                .unwrap()
                .callback_frames
                .is_empty()
        );
        assert_eq!(request.scratch, [0; 8]);
    }
    runtime.preflight_guest_callback(&frame, &request).unwrap();
    assert_eq!(runtime.memory.stats(), before);
}

#[test]
fn heap_creation_rejects_unsupported_flags_and_returns_native_null_at_capacity() {
    let (memory, owner) = initialized_memory();
    let mut runtime = controlled_runtime(memory, controlled_startup_info());
    runtime.set_current_last_error(0x5678).unwrap();
    let endpoint = ApiEndpointIdentity::from_dynamic_endpoint(&RuntimeDynamicEndpoint {
        id: 1,
        address: GuestAddress(0x7f02_0000),
        module: "kernel32.dll".to_owned(),
        name: "HeapCreate".to_owned(),
        ordinal: None,
    });
    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap();
    let return_address = MEMORY_BASE.0 + 0x200;
    write_frame(
        &mut runtime.memory,
        owner,
        GuestAddress(STACK_BASE),
        &[return_address, 4, 0, 0],
    );
    runtime.memory.seal_owner(owner).unwrap();
    let reflection = runtime.memory.pending_reflection().unwrap();
    runtime
        .memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
    let before = runtime.memory.stats();
    assert!(matches!(
        runtime.dispatch_api_call(
            &dispatcher,
            owned_trap(&endpoint, STACK_BASE, return_address),
            crate::callbacks::CALLBACK_RETURN_ARENA_BASE,
            None
        ),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(runtime.current_last_error().unwrap(), 0x5678);
    assert!(runtime.heap(0x6801_0001).is_err());
    assert_eq!(runtime.memory.stats(), before);

    runtime
        .memory
        .write(GuestAddress(STACK_BASE + 4), &1_u32.to_le_bytes())
        .unwrap();
    acknowledge_pending_delta(&mut runtime.memory);
    let mut handles = Vec::new();
    for _ in 0..crate::runtime::RUNTIME_PRIVATE_HEAP_BACKING_LIMIT {
        let created = dispatch_owned(
            &dispatcher,
            &mut runtime,
            &endpoint,
            STACK_BASE,
            return_address,
        );
        let handle = created.eax.unwrap();
        assert_ne!(handle, 0);
        assert_eq!(runtime.heap(handle).unwrap().maximum_size, 0);
        assert_eq!(runtime.current_last_error().unwrap(), 0x5678);
        handles.push(handle);
    }
    let before = runtime.memory.stats();
    for _ in 0..2 {
        let exhausted = dispatch_owned(
            &dispatcher,
            &mut runtime,
            &endpoint,
            STACK_BASE,
            return_address,
        );
        assert_eq!(exhausted.eax, Some(0));
        assert_eq!(runtime.current_last_error().unwrap(), 8);
        assert_eq!(runtime.memory.stats(), before);
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::Runnable
        );
        assert!(runtime.scheduler().host_requests().is_empty());
        for &handle in &handles {
            assert_eq!(runtime.heap(handle).unwrap().live_allocations, 0);
            assert_eq!(
                runtime
                    .object_for_handle(RuntimeHandle(handle), RuntimeObjectKind::PrivateHeap)
                    .unwrap()
                    .open_handles,
                1
            );
        }
    }
}

fn initialized_memory() -> (GuestMemory, MemoryOwnerId) {
    initialized_memory_with_limits(MemoryLimits {
        max_physical_pages: 4,
        max_regions: 4,
        max_owners: 4,
        max_access_bytes: 0x4000,
    })
}

fn initialized_memory_with_limits(limits: MemoryLimits) -> (GuestMemory, MemoryOwnerId) {
    let mut memory = GuestMemory::new(limits).expect("memory");
    let owner = memory
        .create_owner(MemoryOwnerKind::Runtime)
        .expect("runtime owner");
    let range = memory
        .reserve_fixed(owner, MemoryRegionKind::Runtime, MEMORY_BASE, 0x3000)
        .expect("runtime reservation");
    memory
        .commit(
            owner,
            range,
            PageProtection {
                read: true,
                write: true,
                execute: false,
            },
        )
        .expect("runtime commit");
    (memory, owner)
}

type FrameWords = (u32, Vec<u32>);

fn dispatcher_fixture(
    memory: &mut GuestMemory,
    owner: MemoryOwnerId,
) -> (ApiDispatcher, Vec<ApiEndpointIdentity>, [FrameWords; 7]) {
    let specifications = [
        (
            "GetCurrentProcessId",
            ImportThunkDisposition::DeclaredRuntimeExport,
        ),
        (
            "QueryPerformanceCounter",
            ImportThunkDisposition::DeclaredRuntimeExport,
        ),
        (
            "GetStartupInfoW",
            ImportThunkDisposition::DeclaredRuntimeExport,
        ),
        ("TlsSetValue", ImportThunkDisposition::DeclaredRuntimeExport),
        (
            "EncodePointer",
            ImportThunkDisposition::DeclaredRuntimeExport,
        ),
        (
            "UnsupportedIdentity",
            ImportThunkDisposition::UnsupportedTrap,
        ),
    ];
    let thunks = specifications
        .into_iter()
        .enumerate()
        .map(|(index, (name, disposition))| {
            let id = ImportThunkId((index + 1) as u32);
            let address = GuestAddress(ENDPOINT_BASE + (index as u32 * 0x10));
            let iat_address = GuestAddress(IAT_BASE + (index as u32 * 4));
            memory
                .write_initialization(owner, iat_address, &address.0.to_le_bytes())
                .expect("initialize IAT");
            ImportThunk {
                id,
                address,
                requester_owner: owner,
                module: "KERNEL32.dll".to_owned(),
                iat_rva: Rva(index as u32 * 4),
                iat_address,
                original_iat_value: 0,
                disposition,
                target: ImportTarget::Name {
                    name: name.to_owned(),
                    hint: 0,
                    name_rva: Rva(0),
                },
            }
        })
        .collect::<Vec<_>>();
    let endpoints = thunks
        .iter()
        .map(ApiEndpointIdentity::from_import_thunk)
        .collect::<Vec<_>>();
    let frames = [
        (STACK_BASE, vec![0x1000_0001]),
        (STACK_BASE + 0x20, vec![0x1000_0002, OUTPUT_BASE]),
        (STACK_BASE + 0x40, vec![0x1000_0003, OUTPUT_BASE + 0x100]),
        (STACK_BASE + 0x60, vec![0x1000_0004, 0, 0x7f02_0010]),
        (STACK_BASE + 0x80, vec![0x1000_0005, 0]),
        (STACK_BASE + 0xa0, vec![0x1000_0006]),
        (STACK_BASE + 0xc0, vec![0x1000_0007, 0x100a_9608]),
    ];
    for (address, words) in &frames {
        write_frame(memory, owner, GuestAddress(*address), words);
    }
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    (
        ApiDispatcher::from_endpoints(endpoints.clone()).expect("dispatcher"),
        endpoints,
        frames,
    )
}

fn remaining_dispatcher_fixture(
    memory: &mut GuestMemory,
    owner: MemoryOwnerId,
    module_name_address: u32,
    export_name_address: u32,
) -> (ApiDispatcher, Vec<ApiEndpointIdentity>, [FrameWords; 15]) {
    const MISSING_MODULE_NAME_ADDRESS: u32 = OUTPUT_BASE + 0x280;
    const MISSING_EXPORT_NAME_ADDRESS: u32 = OUTPUT_BASE + 0x380;

    let names = [
        "GetSystemTimeAsFileTime",
        "GetCurrentThreadId",
        "GetTickCount",
        "HeapSetInformation",
        "HeapCreate",
        "GetModuleHandleW",
        "GetProcAddress",
        "TlsAlloc",
        "TlsSetValue",
    ];
    let thunks = names
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            let id = ImportThunkId((index + 1) as u32);
            let address = GuestAddress(ENDPOINT_BASE + (index as u32 * 0x10));
            let iat_address = GuestAddress(IAT_BASE + (index as u32 * 4));
            memory
                .write_initialization(owner, iat_address, &address.0.to_le_bytes())
                .expect("initialize IAT");
            ImportThunk {
                id,
                address,
                requester_owner: owner,
                module: "KERNEL32.dll".to_owned(),
                iat_rva: Rva(index as u32 * 4),
                iat_address,
                original_iat_value: 0,
                disposition: ImportThunkDisposition::DeclaredRuntimeExport,
                target: ImportTarget::Name {
                    name: name.to_owned(),
                    hint: 0,
                    name_rva: Rva(0),
                },
            }
        })
        .collect::<Vec<_>>();
    let endpoints = thunks
        .iter()
        .map(ApiEndpointIdentity::from_import_thunk)
        .collect::<Vec<_>>();
    let frames = [
        (STACK_BASE, vec![0x1000_0101, OUTPUT_BASE]),
        (STACK_BASE + 0x20, vec![0x1000_0102]),
        (STACK_BASE + 0x40, vec![0x1000_0103]),
        (STACK_BASE + 0x60, vec![0x1000_0104, 0, 1, 0, 0]),
        (STACK_BASE + 0x80, vec![0x1000_0105, 0, 0x1000, 0]),
        (STACK_BASE + 0xa0, vec![0x1000_0106, module_name_address]),
        (
            STACK_BASE + 0xc0,
            vec![0x1000_0107, 0x4d01_0001, export_name_address],
        ),
        (STACK_BASE + 0xe0, vec![0x1000_0108]),
        (STACK_BASE + 0x100, vec![0x1000_0109, 0x4d01_0001, 7]),
        (STACK_BASE + 0x120, vec![0x1000_010a, 0x4d01_0001, 2]),
        (STACK_BASE + 0x140, vec![0x1000_010b, 0]),
        (
            STACK_BASE + 0x160,
            vec![0x1000_010c, MISSING_MODULE_NAME_ADDRESS],
        ),
        (
            STACK_BASE + 0x180,
            vec![0x1000_010d, 0x4d01_0001, MISSING_EXPORT_NAME_ADDRESS],
        ),
        (STACK_BASE + 0x1a0, vec![0x1000_010e, 7, 0x1234_5678]),
        (STACK_BASE + 0x1c0, vec![0x1000_010f, 64, 0x8765_4321]),
    ];
    for (address, words) in &frames {
        write_frame(memory, owner, GuestAddress(*address), words);
    }
    for (address, name) in [
        (module_name_address, "KERNEL32.DLL"),
        (MISSING_MODULE_NAME_ADDRESS, "MISSING.DLL"),
    ] {
        let bytes = name
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        memory
            .write_initialization(owner, GuestAddress(address), &bytes)
            .expect("initialize module name");
    }
    memory
        .write_initialization(owner, GuestAddress(export_name_address), b"FlsAlloc\0")
        .expect("initialize export name");
    memory
        .write_initialization(
            owner,
            GuestAddress(MISSING_EXPORT_NAME_ADDRESS),
            b"NotAnExport\0",
        )
        .expect("initialize missing export name");
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    (
        ApiDispatcher::from_endpoints(endpoints.clone()).expect("dispatcher"),
        endpoints,
        frames,
    )
}

fn suspended_dispatcher_fixture(
    memory: &mut GuestMemory,
    owner: MemoryOwnerId,
) -> (ApiDispatcher, Vec<ApiEndpointIdentity>, [FrameWords; 2]) {
    let specifications = ["DeferredAdd", "CallCallback"];
    let thunks = specifications
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            let id = ImportThunkId((index + 1) as u32);
            let address = GuestAddress(ENDPOINT_BASE + (index as u32 * 0x10));
            let iat_address = GuestAddress(IAT_BASE + (index as u32 * 4));
            memory
                .write_initialization(owner, iat_address, &address.0.to_le_bytes())
                .expect("initialize suspended-call IAT");
            ImportThunk {
                id,
                address,
                requester_owner: owner,
                module: "fixture.dll".to_owned(),
                iat_rva: Rva(index as u32 * 4),
                iat_address,
                original_iat_value: 0,
                disposition: ImportThunkDisposition::DeclaredRuntimeExport,
                target: ImportTarget::Name {
                    name: name.to_owned(),
                    hint: 0,
                    name_rva: Rva(0),
                },
            }
        })
        .collect::<Vec<_>>();
    let endpoints = thunks
        .iter()
        .map(ApiEndpointIdentity::from_import_thunk)
        .collect::<Vec<_>>();
    let frames = [
        (STACK_BASE, vec![0x1000_0201, 19, 23]),
        (STACK_BASE + 0x20, vec![0x1000_0202, 0x1000_3000, 42]),
    ];
    for (address, words) in &frames {
        write_frame(memory, owner, GuestAddress(*address), words);
    }
    memory.seal_owner(owner).expect("seal suspended-call owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    (
        ApiDispatcher::with_argument_words(endpoints.iter().cloned().map(|endpoint| (endpoint, 2)))
            .expect("suspended dispatcher"),
        endpoints,
        frames,
    )
}

fn controlled_runtime(memory: GuestMemory, startup_bytes: Vec<u8>) -> Runtime {
    let mut runtime = Runtime::new(
        RuntimeBootstrap {
            process_id: RuntimeProcessId(PROCESS_ID),
            initial_thread_id: RuntimeThreadId(THREAD_ID),
            initial_last_error: INITIAL_LAST_ERROR,
            clock: RuntimeClockState {
                wall_clock_file_time: 134_282_016_000_000_000,
                tick_count: 3_600_000,
                performance_counter: PERFORMANCE_COUNTER,
                performance_frequency: PERFORMANCE_FREQUENCY,
            },
            startup_info: RuntimeStartupInfo {
                bytes: startup_bytes,
            },
            process_image_path_a: vine_core::RuntimeProcessImagePathA {
                bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            },
            initial_current_directory_a: vine_core::RuntimeCurrentDirectoryA {
                bytes_with_nul: b"C:\\Vine\0".to_vec(),
            },
            encoded_pointer_cookie: 0xec01_9d3b,
        },
        memory,
    )
    .expect("runtime state");
    runtime
        .start_initial_thread()
        .expect("start controlled initial thread");
    runtime
}

fn controlled_startup_info() -> Vec<u8> {
    let mut bytes = vec![0; 68];
    bytes[..4].copy_from_slice(&68_u32.to_le_bytes());
    bytes
}

fn dispatch(
    dispatcher: &ApiDispatcher,
    runtime: &mut Runtime,
    endpoint: &ApiEndpointIdentity,
    stack_pointer: u32,
    return_address: u32,
    _argument_bytes: u32,
) -> vine_core::ApiImmediateCompletion {
    match runtime.dispatch_api_call(
        dispatcher,
        owned_trap(endpoint, stack_pointer, return_address),
        crate::callbacks::CALLBACK_RETURN_ARENA_BASE,
        None,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected immediate dispatch, got {other:?}"),
    }
}

fn dispatch_owned(
    dispatcher: &ApiDispatcher,
    runtime: &mut Runtime,
    endpoint: &ApiEndpointIdentity,
    stack_pointer: u32,
    return_address: u32,
) -> vine_core::ApiImmediateCompletion {
    match runtime.dispatch_api_call(
        dispatcher,
        owned_trap(endpoint, stack_pointer, return_address),
        crate::callbacks::CALLBACK_RETURN_ARENA_BASE,
        None,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected immediate owned dispatch, got {other:?}"),
    }
}

#[allow(clippy::too_many_arguments)]
fn dispatch_with_handler(
    dispatcher: &ApiDispatcher,
    runtime: &mut Runtime,
    endpoint: &ApiEndpointIdentity,
    stack_pointer: u32,
    return_address: u32,
    _argument_bytes: u32,
    immediate: ApiImmediate,
) -> DispatchOutcome {
    dispatcher.dispatch_owned(
        owned_trap(endpoint, stack_pointer, return_address),
        runtime,
        None,
        |_, _, _| ApiHandlerResult::Immediate(immediate),
    )
}

fn owned_trap(
    endpoint: &ApiEndpointIdentity,
    stack_pointer: u32,
    return_address: u32,
) -> ApiOwnedTrap {
    ApiOwnedTrap {
        endpoint_id: endpoint.id,
        endpoint_address: endpoint.address,
        stack_pointer: GuestAddress(stack_pointer),
        return_address: GuestAddress(return_address),
    }
}

fn write_frame(
    memory: &mut GuestMemory,
    owner: MemoryOwnerId,
    address: GuestAddress,
    words: &[u32],
) {
    let bytes = words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<_>>();
    memory
        .write_initialization(owner, address, &bytes)
        .expect("initialize call frame");
}

fn acknowledge_pending_delta(memory: &mut GuestMemory) {
    let delta = memory.pending_delta().expect("pending API write delta");
    memory
        .acknowledge_delta(delta.generation)
        .expect("acknowledge API write delta");
}

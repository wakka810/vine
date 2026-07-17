use std::cell::Cell;

use vine_core::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiContract, ApiDirection, ApiDispatcher,
    ApiEndpointIdentity, ApiFaultClass, ApiHandlerResult, ApiImmediate, ApiOwnedTrap,
    ApiReturnSchema, ApiReturnValue, ApiTrap, ApiValueSchema, CREATE_MUTEX_A_IMPLEMENTATION,
    CallbackPolicy, CallingConvention, DYNAMIC_EXPORT_ENDPOINT_NAMESPACE, DispatchOutcome,
    ExecutionSession, FLS_GET_VALUE_IMPLEMENTATION, GET_ACP_IMPLEMENTATION,
    GET_CP_INFO_IMPLEMENTATION, GET_CURRENT_THREAD_IMPLEMENTATION, GET_DRIVE_TYPE_A_IMPLEMENTATION,
    GET_LAST_ERROR_IMPLEMENTATION, GET_STRING_TYPE_W_IMPLEMENTATION,
    GET_SYSTEM_DEFAULT_LANG_ID_IMPLEMENTATION, GET_SYSTEM_INFO_IMPLEMENTATION,
    GET_VERSION_EX_A_IMPLEMENTATION, GUEST_PAGE_SIZE, GuestAddress, GuestCallbackRequest,
    GuestMemory, GuestRange, HEAP_SIZE_IMPLEMENTATION, HostCompletion, HostRequest,
    INITIALIZE_CRITICAL_SECTION_IMPLEMENTATION, IS_PROCESSOR_FEATURE_PRESENT_IMPLEMENTATION,
    ImportTarget, ImportThunk, ImportThunkDisposition, ImportThunkId, Kernel32CodePageInfoPolicy,
    Kernel32CompatibilityPolicy, Kernel32CreateMutexAPolicy, Kernel32GetDriveTypeAPolicy,
    Kernel32GetStringTypeWPolicy, Kernel32HeapAllocationPolicy,
    Kernel32InitializeCriticalSectionPolicy, Kernel32LcMapStringWPolicy,
    Kernel32MultiByteToWideCharPolicy, Kernel32ProcessorFeaturePolicy,
    Kernel32SetCurrentDirectoryAPolicy, Kernel32StandardHandleFileType, Kernel32SystemInfoPolicy,
    Kernel32ThreadAffinityPolicy, Kernel32TickCountPolicy, Kernel32WideCharToMultiBytePolicy,
    LC_MAP_STRING_W_IMPLEMENTATION, LOGICAL_ENCODED_POINTER_NAMESPACE, LOGICAL_FLS_INDEX_NAMESPACE,
    LOGICAL_LOADED_MODULE_NAMESPACE, LOGICAL_PRIVATE_HEAP_NAMESPACE,
    LOGICAL_STANDARD_HANDLE_NAMESPACE, LastErrorPolicy, MULTI_BYTE_TO_WIDE_CHAR_IMPLEMENTATION,
    MemoryLimits, MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind,
    PROCESS_PARAMETERS_ENVIRONMENT_POINTER_OFFSET, PageProtection, ProcessAddressSpaceLayout,
    RUNTIME_CURRENT_THREAD_PSEUDO_HANDLE, RUNTIME_FLS_INDEX_LIMIT, RUNTIME_PRIVATE_HEAP_FREED_BYTE,
    RUNTIME_PRIVATE_HEAP_UNINITIALIZED_BYTE, RUNTIME_STANDARD_ERROR_HANDLE,
    RUNTIME_STANDARD_ERROR_SELECTOR, RUNTIME_STANDARD_INPUT_HANDLE,
    RUNTIME_STANDARD_INPUT_SELECTOR, RUNTIME_STANDARD_OUTPUT_HANDLE,
    RUNTIME_STANDARD_OUTPUT_SELECTOR, ReentrancyPolicy, RegisterUpdate, RuntimeBootstrap,
    RuntimeClockState, RuntimeDynamicEndpoint, RuntimeHeap, RuntimeHeapAllocationId,
    RuntimeHeapInitialization, RuntimeModule, RuntimeObjectKind, RuntimeProcessId,
    RuntimeSchedulerEvent, RuntimeStartupInfo, RuntimeState, RuntimeThreadId, RuntimeTlsValue, Rva,
    SET_CURRENT_DIRECTORY_A_IMPLEMENTATION, SET_LAST_ERROR_IMPLEMENTATION,
    SET_THREAD_AFFINITY_MASK_IMPLEMENTATION, SET_UNHANDLED_EXCEPTION_FILTER_IMPLEMENTATION,
    SchedulerThreadState, StackCleanup, TLS_GET_VALUE_IMPLEMENTATION, dispatch_kernel32_api,
    dispatch_kernel32_api_with_policy, kernel32_contract_for_endpoint,
};

const MEMORY_BASE: GuestAddress = GuestAddress(0x2000_0000);
const STACK_BASE: u32 = 0x2000_0100;
const OUTPUT_BASE: u32 = 0x2000_1000;
const IAT_BASE: u32 = 0x2000_2000;
const ENDPOINT_BASE: u32 = 0x7f00_1000;
const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;
const INITIAL_LAST_ERROR: u32 = 0x1234;
const PERFORMANCE_COUNTER: u64 = 12_345_678_901;

#[test]
fn runtime_state_drives_five_api_shapes_and_nested_scheduler_state() {
    let (mut memory, owner) = initialized_memory();
    let (dispatcher, endpoints, frames) = dispatcher_fixture(&mut memory, owner);
    let startup_bytes = controlled_startup_info();
    let mut runtime = controlled_runtime(startup_bytes.clone());

    runtime
        .register_dynamic_endpoint(RuntimeDynamicEndpoint {
            namespace: DYNAMIC_EXPORT_ENDPOINT_NAMESPACE.to_owned(),
            id: 2,
            address: GuestAddress(0x7f02_0010),
            module: "kernel32.dll".to_owned(),
            name: "FlsGetValue".to_owned(),
            lifetime: "process-runtime".to_owned(),
        })
        .expect("register dynamic endpoint");
    let tls_index = runtime.allocate_tls_index().expect("allocate TLS index");
    assert_eq!(tls_index, 0);

    runtime.set_heap_termination_on_corruption(true);
    runtime
        .create_heap(RuntimeHeap {
            owner: "test-runtime".to_owned(),
            variant_id: "growable".to_owned(),
            namespace: LOGICAL_PRIVATE_HEAP_NAMESPACE.to_owned(),
            handle: 0x6801_0001,
            creation_flags: 0,
            initial_size: 0x1000,
            maximum_size: 0,
            growable: true,
            live_allocations: 0,
        })
        .expect("create logical heap");
    runtime
        .load_module(RuntimeModule {
            owner: "test-runtime".to_owned(),
            variant_id: "runtime-kernel32".to_owned(),
            namespace: LOGICAL_LOADED_MODULE_NAMESPACE.to_owned(),
            handle: 0x4d01_0001,
            requested_name: "KERNEL32.DLL".to_owned(),
            canonical_name: "kernel32.dll".to_owned(),
            runtime_system_module: true,
        })
        .expect("load logical module");

    let process = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[0],
        frames[0].0,
        frames[0].1[0],
    );
    assert_eq!(process.eax, RegisterUpdate::Set(PROCESS_ID));
    assert_eq!(process.stack_pointer, GuestAddress(STACK_BASE + 4));
    assert_eq!(process.last_error, INITIAL_LAST_ERROR);

    let counter = dispatch(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[1],
        frames[1].0,
        frames[1].1[0],
        4,
    );
    assert_eq!(counter.eax, RegisterUpdate::Set(1));
    assert_eq!(counter.guest_writes.len(), 1);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(OUTPUT_BASE), 8)
            .expect("counter output"),
        PERFORMANCE_COUNTER.to_le_bytes()
    );

    let startup = dispatch(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[2],
        frames[2].0,
        frames[2].1[0],
        4,
    );
    assert_eq!(startup.eax, RegisterUpdate::Preserve);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(OUTPUT_BASE + 0x100), startup_bytes.len())
            .expect("startup output"),
        startup_bytes
    );

    let tls = dispatch(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[3],
        frames[3].0,
        frames[3].1[0],
        8,
    );
    assert_eq!(tls.frame.argument_u32(0), Some(tls_index));
    assert_eq!(tls.frame.argument_u32(1), Some(0x7f02_0010));
    assert_eq!(tls.stack_pointer, GuestAddress(STACK_BASE + 0x60 + 12));
    assert_eq!(
        runtime.tls_value(tls_index).expect("current TLS value"),
        &RuntimeTlsValue::DynamicEndpoint {
            endpoint_id: 2,
            address: GuestAddress(0x7f02_0010),
        }
    );

    let encoded_null = dispatch(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[4],
        frames[4].0,
        frames[4].1[0],
        4,
    );
    assert_eq!(encoded_null.frame.argument_u32(0), Some(0));
    assert_eq!(encoded_null.eax, RegisterUpdate::Set(0xec01_0001));

    let encoded_function = dispatch(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[4],
        frames[6].0,
        frames[6].1[0],
        4,
    );
    assert_eq!(encoded_function.eax, RegisterUpdate::Set(0xec01_0002));
    assert_eq!(
        runtime
            .encoded_pointer_for_source(0)
            .expect("encoded NULL")
            .namespace,
        LOGICAL_ENCODED_POINTER_NAMESPACE
    );
    assert_eq!(runtime.registries().encoded_pointers().len(), 2);
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    assert!(runtime.process().heap_termination_on_corruption());
    assert_eq!(runtime.heap(0x6801_0001).unwrap().initial_size, 0x1000);
    assert_eq!(
        runtime.module(0x4d01_0001).unwrap().canonical_name,
        "kernel32.dll"
    );
    assert_eq!(runtime.dynamic_endpoint(2).unwrap().name, "FlsGetValue");

    let output_before = memory
        .read(GuestAddress(OUTPUT_BASE), 8)
        .expect("output before rejected write");
    let rejected = dispatch_with_handler(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[1],
        frames[1].0,
        frames[1].1[0],
        4,
        ApiImmediate::new(ApiReturnValue::Bool32(true))
            .with_guest_write(GuestAddress(OUTPUT_BASE + 4), vec![0xff; 8]),
    );
    assert!(matches!(
        rejected,
        DispatchOutcome::Fault(ref fault)
            if fault.code == "handler-guest-write-outside-contract"
    ));
    assert_eq!(
        memory
            .read(GuestAddress(OUTPUT_BASE), 8)
            .expect("output after rejected write"),
        output_before
    );

    let handler_called = Cell::new(false);
    let unsupported = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoints[5], frames[5].0, frames[5].1[0]),
            &mut session,
            |_, _, _| {
                handler_called.set(true);
                ApiHandlerResult::Immediate(ApiImmediate::new(ApiReturnValue::U32(0)))
            },
        )
    };
    assert!(!handler_called.get());
    assert!(matches!(
        unsupported,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "endpoint-contract-missing"
                && fault.endpoint.as_deref() == Some(&endpoints[5])
    ));

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
    assert_eq!(
        runtime
            .pop_callback_frame(outer.id)
            .expect_err("callbacks return in LIFO order")
            .code,
        "callback-frame-return-order-mismatch"
    );

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
    let cancellation = runtime
        .cancel_current_thread(0xc000_013a)
        .expect("cancel current thread");
    assert_eq!(cancellation.thread_id, RuntimeThreadId(THREAD_ID));
    assert_eq!(cancellation.exit_code, 0xc000_013a);
    assert_eq!(cancellation.host_requests_cancelled, 1);
    assert_eq!(cancellation.callback_frames_cancelled, 2);
    assert_eq!(cancellation.scheduler_events_discarded, 0);
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
    assert_eq!(
        runtime
            .complete_host_request(cancelled_request.request_id)
            .expect_err("cancelled host request is retired")
            .code,
        "host-request-id-unknown"
    );
}

#[test]
fn fls_allocation_contract_owns_bounded_callback_slots() {
    const CALLBACK_ADDRESS: u32 = OUTPUT_BASE + 0x800;

    let (mut memory, owner) = initialized_memory();
    let (dispatcher, endpoint, frames) =
        fls_allocation_dispatcher_fixture(&mut memory, owner, CALLBACK_ADDRESS);
    let mut runtime = controlled_runtime(controlled_startup_info());
    let memory_before = memory.stats();

    let invalid_callback = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[0].0, frames[0].1[0]),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        invalid_callback,
        DispatchOutcome::Fault(ref fault) if fault.class == ApiFaultClass::InvalidCall
    ));
    assert!(runtime.fls_slots().is_empty());

    let callback_slot = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[1].0,
        frames[1].1[0],
    );
    assert_eq!(callback_slot.eax, RegisterUpdate::Set(0));
    assert_eq!(callback_slot.stack_pointer, GuestAddress(frames[1].0 + 8));
    assert_eq!(callback_slot.last_error, INITIAL_LAST_ERROR);
    assert_eq!(
        runtime.fls_slot(0).expect("callback FLS slot").namespace,
        LOGICAL_FLS_INDEX_NAMESPACE
    );
    assert_eq!(
        runtime.fls_slot(0).expect("callback FLS slot").callback,
        Some(GuestAddress(CALLBACK_ADDRESS))
    );

    let null_callback_slot = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[2].0,
        frames[2].1[0],
    );
    assert_eq!(null_callback_slot.eax, RegisterUpdate::Set(1));
    assert_eq!(
        runtime
            .fls_slot(1)
            .expect("null callback FLS slot")
            .callback,
        None
    );

    for expected_index in 2..RUNTIME_FLS_INDEX_LIMIT {
        assert_eq!(
            runtime
                .allocate_fls_slot(None)
                .expect("bounded FLS allocation")
                .index,
            expected_index
        );
    }
    assert_eq!(
        runtime
            .allocate_fls_slot(None)
            .expect_err("FLS capacity is fail-closed")
            .code,
        "fls-index-space-exhausted"
    );
    assert_eq!(runtime.fls_slots().len(), RUNTIME_FLS_INDEX_LIMIT as usize);
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    assert_eq!(memory.stats(), memory_before);
}

#[test]
fn fls_set_value_contract_owns_current_thread_values() {
    let (mut memory, owner) = initialized_memory();
    let (dispatcher, endpoint, frames) = fls_set_value_dispatcher_fixture(&mut memory, owner);
    let mut runtime = controlled_runtime(controlled_startup_info());
    assert_eq!(
        runtime
            .allocate_fls_slot(None)
            .expect("logical FLS slot")
            .index,
        0
    );
    let memory_before = memory.stats();

    let first = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[0].0,
        frames[0].1[0],
    );
    assert_eq!(first.eax, RegisterUpdate::Set(1));
    assert_eq!(first.stack_pointer, GuestAddress(frames[0].0 + 12));
    assert_eq!(first.last_error, INITIAL_LAST_ERROR);
    assert_eq!(
        runtime.fls_value(0).expect("initial-thread FLS value"),
        0x2000_0000
    );

    let second_thread = RuntimeThreadId(THREAD_ID + 1);
    runtime
        .register_thread(second_thread, 0x5678)
        .expect("register second FLS lane");
    runtime
        .select_thread(second_thread)
        .expect("select second FLS lane");
    assert_eq!(
        runtime
            .fls_value(0)
            .expect_err("FLS values are thread-local")
            .code,
        "fls-thread-storage-uninitialized"
    );
    let second = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[1].0,
        frames[1].1[0],
    );
    assert_eq!(second.eax, RegisterUpdate::Set(1));
    assert_eq!(second.last_error, 0x5678);
    assert_eq!(
        runtime.fls_value(0).expect("second-thread FLS value"),
        0x2000_0100
    );

    runtime
        .select_thread(RuntimeThreadId(THREAD_ID))
        .expect("restore initial FLS lane");
    assert_eq!(
        runtime.fls_value(0).expect("retained initial FLS value"),
        0x2000_0000
    );
    let runtime_before_rejection = runtime.clone();
    let rejected = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[2].0, frames[2].1[0]),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        rejected,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::InvalidCall
                && fault.code == "fls-index-unallocated"
    ));
    assert_eq!(runtime, runtime_before_rejection);
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    assert_eq!(memory.stats(), memory_before);
}

#[test]
fn fls_get_value_models_thread_storage_and_win32_last_error() {
    let (mut memory, owner) = initialized_memory();
    let frames = [
        (STACK_BASE, vec![0x1000_0491, 0]),
        (STACK_BASE + 0x20, vec![0x1000_0492, 1]),
        (STACK_BASE + 0x40, vec![0x1000_0493, u32::MAX]),
        (STACK_BASE + 0x60, vec![0x1000_0494, 0]),
        (STACK_BASE + 0x80, vec![0x1000_0495, 0]),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    let dynamic = RuntimeDynamicEndpoint {
        namespace: DYNAMIC_EXPORT_ENDPOINT_NAMESPACE.to_owned(),
        id: 2,
        address: GuestAddress(0x7f02_0010),
        module: "kernel32.dll".to_owned(),
        name: "FlsGetValue".to_owned(),
        lifetime: "process-runtime".to_owned(),
    };
    let endpoint = ApiEndpointIdentity::from_dynamic_endpoint(&dynamic);
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented FlsGetValue endpoint");
    assert_eq!(contract.implementation_id, FLS_GET_VALUE_IMPLEMENTATION);
    assert_eq!(contract.abi.arguments.len(), 1);
    assert_eq!(
        contract.abi.return_value,
        ApiReturnSchema::OpaquePointer { nullable: true }
    );
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let mut runtime = controlled_runtime(controlled_startup_info());
    assert_eq!(
        runtime
            .allocate_fls_slot(None)
            .expect("first logical FLS slot")
            .index,
        0
    );
    runtime
        .set_fls_value(0, 0x2000_0000)
        .expect("set main-thread FLS value");
    assert_eq!(
        runtime
            .allocate_fls_slot(None)
            .expect("second logical FLS slot")
            .index,
        1
    );
    let memory_before = memory.stats();

    let mut expected = runtime.clone();
    expected.set_current_last_error(0).unwrap();
    let initial = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[0].0,
        frames[0].1[0],
    );
    assert_eq!(initial.eax, RegisterUpdate::Set(0x2000_0000));
    assert_eq!(initial.stack_pointer, GuestAddress(frames[0].0 + 8));
    assert_eq!(initial.last_error, 0);
    assert!(initial.guest_writes.is_empty());
    assert_eq!(runtime, expected);
    assert_eq!(memory.stats(), memory_before);

    let mut expected = runtime.clone();
    expected.set_current_last_error(0).unwrap();
    let unset = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[1].0,
        frames[1].1[0],
    );
    assert_eq!(unset.eax, RegisterUpdate::Set(0));
    assert_eq!(unset.last_error, 0);
    assert_eq!(runtime, expected);
    assert_eq!(memory.stats(), memory_before);

    runtime.set_current_last_error(122).unwrap();
    let mut expected = runtime.clone();
    expected.set_current_last_error(87).unwrap();
    let invalid = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[2].0,
        frames[2].1[0],
    );
    assert_eq!(invalid.eax, RegisterUpdate::Set(0));
    assert_eq!(invalid.last_error, 87);
    assert_eq!(runtime, expected);
    assert_eq!(memory.stats(), memory_before);

    let second_thread = RuntimeThreadId(THREAD_ID + 1);
    runtime
        .register_thread(second_thread, 0x5678)
        .expect("register second FLS lane");
    runtime
        .select_thread(second_thread)
        .expect("select second FLS lane");
    let mut expected = runtime.clone();
    expected.set_current_last_error(87).unwrap();
    let uninitialized_thread = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[3].0,
        frames[3].1[0],
    );
    assert_eq!(uninitialized_thread.eax, RegisterUpdate::Set(0));
    assert_eq!(uninitialized_thread.last_error, 87);
    assert_eq!(runtime, expected);
    assert_eq!(memory.stats(), memory_before);

    runtime
        .set_fls_value(0, 0x2345_6789)
        .expect("set second-thread FLS value");
    runtime.set_current_last_error(122).unwrap();
    let mut expected = runtime.clone();
    expected.set_current_last_error(0).unwrap();
    let second = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[4].0,
        frames[4].1[0],
    );
    assert_eq!(second.eax, RegisterUpdate::Set(0x2345_6789));
    assert_eq!(second.last_error, 0);
    assert_eq!(runtime, expected);
    assert_eq!(memory.stats(), memory_before);

    runtime
        .select_thread(RuntimeThreadId(THREAD_ID))
        .expect("restore initial FLS lane");
    assert_eq!(runtime.fls_value(0).unwrap(), 0x2000_0000);
}

#[test]
fn get_last_error_reads_selected_thread_state_without_mutation() {
    let (mut memory, owner) = initialized_memory();
    let frames = [
        (STACK_BASE, vec![0x1000_0581]),
        (STACK_BASE + 0x20, vec![0x1000_0582]),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x58);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize GetLastError IAT");
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
            name: "GetLastError".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented GetLastError endpoint");
    assert_eq!(contract.implementation_id, GET_LAST_ERROR_IMPLEMENTATION);
    assert!(contract.abi.arguments.is_empty());
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::Preserve);
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let mut runtime = controlled_runtime(controlled_startup_info());
    let memory_before = memory.stats();
    let runtime_before = runtime.clone();

    let initial = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[0].0,
        frames[0].1[0],
    );
    assert_eq!(initial.eax, RegisterUpdate::Set(INITIAL_LAST_ERROR));
    assert_eq!(initial.stack_pointer, GuestAddress(frames[0].0 + 4));
    assert_eq!(initial.last_error, INITIAL_LAST_ERROR);
    assert!(initial.guest_writes.is_empty());
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.stats(), memory_before);

    let second_thread = RuntimeThreadId(THREAD_ID + 1);
    runtime
        .register_thread(second_thread, 0x5678)
        .expect("register second LastError lane");
    runtime
        .select_thread(second_thread)
        .expect("select second LastError lane");
    let second_before = runtime.clone();
    let second = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[1].0,
        frames[1].1[0],
    );
    assert_eq!(second.eax, RegisterUpdate::Set(0x5678));
    assert_eq!(second.stack_pointer, GuestAddress(frames[1].0 + 4));
    assert_eq!(second.last_error, 0x5678);
    assert!(second.guest_writes.is_empty());
    assert_eq!(runtime, second_before);
    assert_eq!(memory.stats(), memory_before);

    runtime
        .select_thread(RuntimeThreadId(THREAD_ID))
        .expect("restore initial LastError lane");
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
}

#[test]
fn get_current_thread_returns_a_stable_pseudo_handle_for_the_selected_typed_thread() {
    let (mut memory, owner) = initialized_memory();
    let frames = [
        (STACK_BASE, vec![0x1000_0588]),
        (STACK_BASE + 0x20, vec![0x1000_0589]),
        (STACK_BASE + 0x40, vec![0x1000_058a]),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x59);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize GetCurrentThread IAT");
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
            name: "GetCurrentThread".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented GetCurrentThread endpoint");
    assert_eq!(
        contract.implementation_id,
        GET_CURRENT_THREAD_IMPLEMENTATION
    );
    assert!(contract.abi.arguments.is_empty());
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::Preserve);
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let mut runtime = controlled_runtime(controlled_startup_info());
    let memory_before = memory.stats();

    let initial_thread_handle = runtime
        .current_thread_handle()
        .expect("initial thread handle");
    let initial_object_id = runtime
        .object_for_handle(initial_thread_handle, RuntimeObjectKind::Thread)
        .expect("initial typed thread object")
        .id;
    let initial_resolution = runtime
        .resolve_current_thread_pseudo_handle()
        .expect("initial pseudo-handle resolution");
    assert_eq!(
        initial_resolution.value,
        RUNTIME_CURRENT_THREAD_PSEUDO_HANDLE
    );
    assert_eq!(
        initial_resolution.selected_thread_id,
        RuntimeThreadId(THREAD_ID)
    );
    assert_eq!(initial_resolution.object_id, initial_object_id);
    let initial_handle_count = runtime.objects().handles().len();
    let initial_before = runtime.clone();
    let initial = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[0].0,
        frames[0].1[0],
    );
    assert_eq!(
        initial.eax,
        RegisterUpdate::Set(RUNTIME_CURRENT_THREAD_PSEUDO_HANDLE)
    );
    assert_eq!(initial.stack_pointer, GuestAddress(frames[0].0 + 4));
    assert_eq!(initial.last_error, INITIAL_LAST_ERROR);
    assert!(initial.guest_writes.is_empty());
    assert_eq!(runtime, initial_before);
    assert_eq!(runtime.objects().handles().len(), initial_handle_count);
    assert_eq!(memory.stats(), memory_before);

    runtime.set_current_last_error(87).unwrap();
    let repeated_before = runtime.clone();
    let repeated = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[1].0,
        frames[1].1[0],
    );
    assert_eq!(
        repeated.eax, initial.eax,
        "same-thread pseudo value must be stable"
    );
    assert_eq!(repeated.last_error, 87);
    assert_eq!(runtime, repeated_before);
    assert_eq!(runtime.objects().handles().len(), initial_handle_count);
    assert_eq!(memory.stats(), memory_before);

    let second_thread = RuntimeThreadId(THREAD_ID + 1);
    let second_handle = runtime
        .register_thread(second_thread, 122)
        .expect("register second thread lane");
    runtime
        .select_thread(second_thread)
        .expect("select second thread lane");
    let second_object_id = runtime
        .object_for_handle(second_handle, RuntimeObjectKind::Thread)
        .expect("second typed thread object")
        .id;
    let second_resolution = runtime
        .resolve_current_thread_pseudo_handle()
        .expect("second pseudo-handle resolution");
    assert_eq!(second_resolution.value, initial_resolution.value);
    assert_eq!(second_resolution.selected_thread_id, second_thread);
    assert_eq!(second_resolution.object_id, second_object_id);
    assert_ne!(second_resolution.object_id, initial_resolution.object_id);
    let second_handle_count = runtime.objects().handles().len();
    let second_before = runtime.clone();
    let second = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[2].0,
        frames[2].1[0],
    );
    assert_eq!(second.eax, initial.eax);
    assert_eq!(second.last_error, 122);
    assert_eq!(runtime, second_before);
    assert_eq!(runtime.objects().handles().len(), second_handle_count);
    assert_eq!(memory.stats(), memory_before);
}

#[test]
fn set_thread_affinity_mask_tracks_the_virtual_current_thread_and_fails_atomically() {
    let (mut memory, owner) = initialized_memory();
    let mut runtime = controlled_runtime(controlled_startup_info());
    let logical_thread_handle = runtime
        .current_thread_handle()
        .expect("initial logical thread handle")
        .0;
    let frames = [
        (
            STACK_BASE,
            vec![0x1000_0590, RUNTIME_CURRENT_THREAD_PSEUDO_HANDLE, 0b001],
        ),
        (
            STACK_BASE + 0x20,
            vec![0x1000_0591, RUNTIME_CURRENT_THREAD_PSEUDO_HANDLE, 0b010],
        ),
        (
            STACK_BASE + 0x40,
            vec![0x1000_0592, RUNTIME_CURRENT_THREAD_PSEUDO_HANDLE, 0b011],
        ),
        (
            STACK_BASE + 0x60,
            vec![0x1000_0593, RUNTIME_CURRENT_THREAD_PSEUDO_HANDLE, 0],
        ),
        (STACK_BASE + 0x80, vec![0x1000_0594, 0, 0b001]),
        (
            STACK_BASE + 0xa0,
            vec![0x1000_0595, RUNTIME_CURRENT_THREAD_PSEUDO_HANDLE, 0b100],
        ),
        (
            STACK_BASE + 0xc0,
            vec![0x1000_0596, logical_thread_handle, 0b001],
        ),
        (
            STACK_BASE + 0xe0,
            vec![0x1000_0597, RUNTIME_CURRENT_THREAD_PSEUDO_HANDLE, 0b001],
        ),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x5b);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize SetThreadAffinityMask IAT");
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
            name: "SetThreadAffinityMask".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented SetThreadAffinityMask endpoint");
    assert_eq!(
        contract.implementation_id,
        SET_THREAD_AFFINITY_MASK_IMPLEMENTATION
    );
    assert_eq!(contract.abi.arguments.len(), 2);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let mut policy = Kernel32CompatibilityPolicy::controlled_v1();
    policy.id = "kernel32-thread-affinity-test-v1".to_owned();
    policy.thread_affinity = Some(Kernel32ThreadAffinityPolicy {
        process_mask: 0b011,
    });
    policy.validate().expect("thread affinity policy");
    let memory_before = memory.stats();
    let handle_count = runtime.objects().handles().len();

    let first = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[0].0,
        frames[0].1[0],
        &policy,
    );
    assert_eq!(first.eax, RegisterUpdate::Set(0b011));
    assert_eq!(first.stack_pointer, GuestAddress(frames[0].0 + 12));
    assert_eq!(first.last_error, INITIAL_LAST_ERROR);
    assert!(first.guest_writes.is_empty());
    assert_eq!(runtime.current_thread_affinity_mask(0b011), Ok(0b001));

    runtime.set_current_last_error(0x5678).unwrap();
    let second = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[1].0,
        frames[1].1[0],
        &policy,
    );
    assert_eq!(second.eax, RegisterUpdate::Set(0b001));
    assert_eq!(second.last_error, 0x5678);
    assert_eq!(runtime.current_thread_affinity_mask(0b011), Ok(0b010));

    let restore = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[2].0,
        frames[2].1[0],
        &policy,
    );
    assert_eq!(restore.eax, RegisterUpdate::Set(0b010));
    assert_eq!(restore.last_error, 0x5678);
    assert_eq!(runtime.current_thread_affinity_mask(0b011), Ok(0b011));

    let zero = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[3].0,
        frames[3].1[0],
        &policy,
    );
    assert_eq!(zero.eax, RegisterUpdate::Set(0));
    assert_eq!(zero.last_error, 87);
    assert_eq!(runtime.current_thread_affinity_mask(0b011), Ok(0b011));

    runtime.set_current_last_error(0).unwrap();
    let null_handle = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[4].0,
        frames[4].1[0],
        &policy,
    );
    assert_eq!(null_handle.eax, RegisterUpdate::Set(0));
    assert_eq!(null_handle.last_error, 6);
    assert_eq!(runtime.current_thread_affinity_mask(0b011), Ok(0b011));

    let outside = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[5].0,
        frames[5].1[0],
        &policy,
    );
    assert_eq!(outside.eax, RegisterUpdate::Set(0));
    assert_eq!(outside.last_error, 87);
    assert_eq!(runtime.current_thread_affinity_mask(0b011), Ok(0b011));

    for (frame, selected_policy, expected_code) in [
        (
            &frames[6],
            &policy,
            "set-thread-affinity-mask-handle-unsupported",
        ),
        (
            &frames[7],
            &Kernel32CompatibilityPolicy::controlled_v1(),
            "set-thread-affinity-mask-policy-disabled",
        ),
    ] {
        let runtime_before_fault = runtime.clone();
        let memory_before_fault = memory.stats();
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        let outcome = dispatcher.dispatch_owned(
            owned_trap(&endpoint, frame.0, frame.1[0]),
            &mut session,
            |contract, frame, session| {
                dispatch_kernel32_api_with_policy(selected_policy, contract, frame, session)
            },
        );
        let DispatchOutcome::Fault(fault) = outcome else {
            panic!("expected affinity fault, got {outcome:?}");
        };
        assert_eq!(fault.class, ApiFaultClass::Unsupported);
        assert_eq!(fault.code, expected_code);
        assert_eq!(runtime, runtime_before_fault);
        assert_eq!(memory.stats(), memory_before_fault);
    }
    assert_eq!(runtime.objects().handles().len(), handle_count);
    assert_eq!(memory.stats(), memory_before);
}

#[test]
fn set_last_error_writes_only_the_selected_thread_without_guest_memory_mutation() {
    let (mut memory, owner) = initialized_memory();
    let frames = [
        (STACK_BASE, vec![0x1000_0585, 87]),
        (STACK_BASE + 0x20, vec![0x1000_0586, 0x1234_5678]),
        (STACK_BASE + 0x40, vec![0x1000_0587, 0]),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x5a);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize SetLastError IAT");
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
            name: "SetLastError".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented SetLastError endpoint");
    assert_eq!(contract.implementation_id, SET_LAST_ERROR_IMPLEMENTATION);
    assert_eq!(contract.abi.arguments.len(), 1);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::Void);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let mut runtime = controlled_runtime(controlled_startup_info());
    let memory_before = memory.stats();

    let initial = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[0].0,
        frames[0].1[0],
    );
    assert_eq!(initial.eax, RegisterUpdate::Preserve);
    assert_eq!(initial.stack_pointer, GuestAddress(frames[0].0 + 8));
    assert_eq!(initial.last_error, 87);
    assert!(initial.guest_writes.is_empty());
    assert_eq!(runtime.current_last_error().unwrap(), 87);
    assert_eq!(memory.stats(), memory_before);

    let initial_thread = RuntimeThreadId(THREAD_ID);
    let second_thread = RuntimeThreadId(THREAD_ID + 1);
    runtime
        .register_thread(second_thread, 122)
        .expect("register second LastError lane");
    runtime
        .select_thread(second_thread)
        .expect("select second LastError lane");
    let second = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[1].0,
        frames[1].1[0],
    );
    assert_eq!(second.eax, RegisterUpdate::Preserve);
    assert_eq!(second.last_error, 0x1234_5678);
    assert!(second.guest_writes.is_empty());
    assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
    assert_eq!(memory.stats(), memory_before);

    runtime
        .select_thread(initial_thread)
        .expect("restore initial LastError lane");
    assert_eq!(runtime.current_last_error().unwrap(), 87);
    let zero = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[2].0,
        frames[2].1[0],
    );
    assert_eq!(zero.eax, RegisterUpdate::Preserve);
    assert_eq!(zero.last_error, 0);
    assert!(zero.guest_writes.is_empty());
    assert_eq!(runtime.current_last_error().unwrap(), 0);
    assert_eq!(memory.stats(), memory_before);

    runtime
        .select_thread(second_thread)
        .expect("restore second LastError lane");
    assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
}

#[test]
fn get_system_info_writes_a_coherent_virtual_profile_and_fails_before_mutation() {
    const SYSTEM_INFO_OUTPUT: u32 = OUTPUT_BASE + 0x300;
    const CROSS_PAGE_OUTPUT: u32 = MEMORY_BASE.0 + 0x2ff0;
    const EXPECTED_SYSTEM_INFO: [u8; 36] = [
        0x00, 0x00, 0x00, 0x00, // architecture and reserved
        0x00, 0x10, 0x00, 0x00, // page size
        0x00, 0x00, 0x01, 0x00, // minimum application address
        0xff, 0xff, 0xfe, 0x7f, // maximum application address
        0x0b, 0x00, 0x00, 0x00, // active processor mask
        0x03, 0x00, 0x00, 0x00, // processor count derived from the mask
        0x4a, 0x02, 0x00, 0x00, // processor type
        0x00, 0x00, 0x01, 0x00, // allocation granularity
        0x05, 0x00, // processor level
        0x34, 0x12, // processor revision
    ];

    let (mut memory, owner) = initialized_memory();
    let frames = [
        (STACK_BASE, vec![0x1000_0589, SYSTEM_INFO_OUTPUT]),
        (STACK_BASE + 0x20, vec![0x1000_058a, SYSTEM_INFO_OUTPUT]),
        (STACK_BASE + 0x40, vec![0x1000_058b, 0]),
        (STACK_BASE + 0x60, vec![0x1000_058c, CROSS_PAGE_OUTPUT]),
        (STACK_BASE + 0x80, vec![0x1000_058d, SYSTEM_INFO_OUTPUT]),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    memory
        .write_initialization(owner, GuestAddress(SYSTEM_INFO_OUTPUT), &[0xa5; 48])
        .expect("initialize SYSTEM_INFO output and guard");
    memory
        .write_initialization(owner, GuestAddress(CROSS_PAGE_OUTPUT), &[0xa5; 16])
        .expect("initialize mapped prefix of cross-page output");
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x5c);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize GetSystemInfo IAT");
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
            name: "GetSystemInfo".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented GetSystemInfo endpoint");
    assert_eq!(contract.implementation_id, GET_SYSTEM_INFO_IMPLEMENTATION);
    assert_eq!(contract.abi.arguments.len(), 1);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::Void);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::Preserve);
    assert!(matches!(
        contract.abi.arguments[0].value,
        ApiValueSchema::Pointer { ref constraint }
            if constraint.byte_length == EXPECTED_SYSTEM_INFO.len() as u32
                && constraint.alignment == 4
    ));
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let mut policy: Kernel32CompatibilityPolicy = serde_json::from_str(include_str!(
        "../../../fixtures/runtime-policies/kernel32-get-system-info-v1.json"
    ))
    .expect("parse system-information policy fixture");
    policy.id = "public-get-system-info-test-v1".to_owned();
    policy.thread_affinity = Some(Kernel32ThreadAffinityPolicy {
        process_mask: 0b1011,
    });
    policy.system_info = Some(Kernel32SystemInfoPolicy {
        processor_level: 5,
        processor_revision: 0x1234,
        ..policy
            .system_info
            .expect("fixture system-information profile")
    });
    policy
        .validate()
        .expect("valid synthetic system-information policy");
    let mut runtime = controlled_runtime(controlled_startup_info());
    runtime.set_current_last_error(87).unwrap();
    let runtime_before = runtime.clone();

    let completion = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[0].0,
        frames[0].1[0],
        &policy,
    );
    assert_eq!(completion.eax, RegisterUpdate::Preserve);
    assert_eq!(completion.stack_pointer, GuestAddress(frames[0].0 + 8));
    assert_eq!(completion.last_error, 87);
    assert_eq!(completion.guest_writes.len(), 1);
    assert_eq!(
        completion.guest_writes[0].write.address.0,
        SYSTEM_INFO_OUTPUT
    );
    assert_eq!(completion.guest_writes[0].write.bytes, EXPECTED_SYSTEM_INFO);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(SYSTEM_INFO_OUTPUT), EXPECTED_SYSTEM_INFO.len())
            .expect("SYSTEM_INFO bytes"),
        EXPECTED_SYSTEM_INFO
    );
    assert_eq!(
        memory
            .read(
                GuestAddress(SYSTEM_INFO_OUTPUT + EXPECTED_SYSTEM_INFO.len() as u32),
                48 - EXPECTED_SYSTEM_INFO.len(),
            )
            .expect("SYSTEM_INFO trailing guard"),
        [0xa5; 12]
    );
    assert_eq!(runtime, runtime_before);

    let output_before_rejections = memory
        .read(GuestAddress(SYSTEM_INFO_OUTPUT), 48)
        .expect("output before rejected calls")
        .to_vec();
    let cross_page_prefix_before = memory
        .read(GuestAddress(CROSS_PAGE_OUTPUT), 16)
        .expect("cross-page prefix before rejected call")
        .to_vec();
    let memory_before_rejections = memory.stats();
    for (frame, selected_policy, expected_code) in [
        (
            &frames[1],
            &Kernel32CompatibilityPolicy::controlled_v1(),
            "get-system-info-policy-disabled",
        ),
        (&frames[2], &policy, "null-pointer-forbidden"),
        (&frames[3], &policy, "guest-page-fault"),
    ] {
        let rejected = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            dispatcher.dispatch_owned(
                owned_trap(&endpoint, frame.0, frame.1[0]),
                &mut session,
                |contract, frame, session| {
                    dispatch_kernel32_api_with_policy(selected_policy, contract, frame, session)
                },
            )
        };
        assert!(matches!(
            rejected,
            DispatchOutcome::Fault(ref fault)
                if fault.code == expected_code
                    && matches!(fault.class, ApiFaultClass::Unsupported | ApiFaultClass::InvalidCall)
        ));
        assert_eq!(runtime, runtime_before);
        assert_eq!(memory.stats(), memory_before_rejections);
        assert_eq!(
            memory.read(GuestAddress(SYSTEM_INFO_OUTPUT), 48).unwrap(),
            output_before_rejections
        );
        assert_eq!(
            memory.read(GuestAddress(CROSS_PAGE_OUTPUT), 16).unwrap(),
            cross_page_prefix_before
        );
    }

    memory
        .protect(
            owner,
            GuestRange::new(GuestAddress(OUTPUT_BASE), GUEST_PAGE_SIZE)
                .expect("SYSTEM_INFO output page range"),
            PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .expect("make SYSTEM_INFO output page read-only");
    acknowledge_pending_delta(&mut memory);
    let memory_before_read_only = memory.stats();
    let read_only = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[4].0, frames[4].1[0]),
            &mut session,
            |contract, frame, session| {
                dispatch_kernel32_api_with_policy(&policy, contract, frame, session)
            },
        )
    };
    assert!(matches!(
        read_only,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::InvalidCall && fault.code == "guest-page-fault"
    ));
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.stats(), memory_before_read_only);
    assert_eq!(
        memory.read(GuestAddress(SYSTEM_INFO_OUTPUT), 48).unwrap(),
        output_before_rejections
    );
}

#[test]
fn get_version_ex_a_honors_declared_layout_and_preserves_unwritten_bytes_atomically() {
    const BASE_OUTPUT: u32 = OUTPUT_BASE + 0x400;
    const EXTENDED_OUTPUT: u32 = OUTPUT_BASE + 0x500;
    const INVALID_SIZE_OUTPUT: u32 = OUTPUT_BASE + 0x600;
    const CROSS_PAGE_OUTPUT: u32 = MEMORY_BASE.0 + 0x2ff0;
    const BASE_BYTES: usize = 148;
    const EXTENDED_BYTES: usize = 156;

    let (mut memory, owner) = initialized_memory();
    let frames = [
        (STACK_BASE, vec![0x1000_0590, BASE_OUTPUT]),
        (STACK_BASE + 0x20, vec![0x1000_0591, EXTENDED_OUTPUT]),
        (STACK_BASE + 0x40, vec![0x1000_0592, INVALID_SIZE_OUTPUT]),
        (STACK_BASE + 0x60, vec![0x1000_0593, BASE_OUTPUT]),
        (STACK_BASE + 0x80, vec![0x1000_0594, 0]),
        (STACK_BASE + 0xa0, vec![0x1000_0595, CROSS_PAGE_OUTPUT]),
        (STACK_BASE + 0xc0, vec![0x1000_0596, BASE_OUTPUT]),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }

    let mut base_before = vec![0xa5; BASE_BYTES];
    base_before[..4].copy_from_slice(&(BASE_BYTES as u32).to_le_bytes());
    memory
        .write_initialization(owner, GuestAddress(BASE_OUTPUT), &base_before)
        .expect("initialize OSVERSIONINFOA output");
    let mut extended_before = vec![0xa5; EXTENDED_BYTES];
    extended_before[..4].copy_from_slice(&(EXTENDED_BYTES as u32).to_le_bytes());
    memory
        .write_initialization(owner, GuestAddress(EXTENDED_OUTPUT), &extended_before)
        .expect("initialize OSVERSIONINFOEXA output");
    let mut invalid_before = vec![0xa5; EXTENDED_BYTES];
    invalid_before[..4].copy_from_slice(&147_u32.to_le_bytes());
    memory
        .write_initialization(owner, GuestAddress(INVALID_SIZE_OUTPUT), &invalid_before)
        .expect("initialize invalid-size OSVERSIONINFOA output");
    let mut cross_page_prefix = [0xa5; 16];
    cross_page_prefix[..4].copy_from_slice(&(BASE_BYTES as u32).to_le_bytes());
    memory
        .write_initialization(owner, GuestAddress(CROSS_PAGE_OUTPUT), &cross_page_prefix)
        .expect("initialize mapped prefix of cross-page OSVERSIONINFOA output");

    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x64);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize GetVersionExA IAT");
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
            name: "GetVersionExA".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented GetVersionExA endpoint");
    assert_eq!(contract.implementation_id, GET_VERSION_EX_A_IMPLEMENTATION);
    assert_eq!(contract.abi.arguments.len(), 1);
    assert_eq!(contract.abi.arguments[0].direction, ApiDirection::InOut);
    assert!(matches!(
        contract.abi.arguments[0].value,
        ApiValueSchema::DynamicPointer { ref constraint }
            if constraint.minimum_byte_length == 4
                && constraint.maximum_byte_length == EXTENDED_BYTES as u32
                && constraint.alignment == 4
    ));
    assert_eq!(contract.abi.return_value, ApiReturnSchema::Bool32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);

    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let policy: Kernel32CompatibilityPolicy = serde_json::from_str(include_str!(
        "../../../fixtures/runtime-policies/kernel32-get-version-ex-a-v1.json"
    ))
    .expect("parse OS-version policy fixture");
    policy.validate().expect("valid OS-version policy fixture");
    let mut runtime = controlled_runtime(controlled_startup_info());
    runtime.set_current_last_error(87).unwrap();
    let runtime_before_success = runtime.clone();

    let base = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[0].0,
        frames[0].1[0],
        &policy,
    );
    assert_eq!(base.eax, RegisterUpdate::Set(1));
    assert_eq!(base.stack_pointer, GuestAddress(frames[0].0 + 8));
    assert_eq!(base.last_error, 87);
    assert_eq!(base.guest_writes.len(), 1);
    assert_eq!(base.guest_writes[0].write.bytes.len(), BASE_BYTES);
    acknowledge_pending_delta(&mut memory);
    let base_after = memory
        .read(GuestAddress(BASE_OUTPUT), BASE_BYTES)
        .expect("OSVERSIONINFOA bytes");
    assert_eq!(
        u32::from_le_bytes(base_after[0..4].try_into().unwrap()),
        148
    );
    assert_eq!(u32::from_le_bytes(base_after[4..8].try_into().unwrap()), 10);
    assert_eq!(u32::from_le_bytes(base_after[8..12].try_into().unwrap()), 0);
    assert_eq!(
        u32::from_le_bytes(base_after[12..16].try_into().unwrap()),
        19_045
    );
    assert_eq!(
        u32::from_le_bytes(base_after[16..20].try_into().unwrap()),
        2
    );
    assert_eq!(base_after[20], 0);
    assert_eq!(&base_after[21..], &[0xa5; BASE_BYTES - 21]);
    assert_eq!(runtime, runtime_before_success);

    let extended = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[1].0,
        frames[1].1[0],
        &policy,
    );
    assert_eq!(extended.eax, RegisterUpdate::Set(1));
    assert_eq!(extended.last_error, 87);
    assert_eq!(extended.guest_writes[0].write.bytes.len(), EXTENDED_BYTES);
    acknowledge_pending_delta(&mut memory);
    let extended_after = memory
        .read(GuestAddress(EXTENDED_OUTPUT), EXTENDED_BYTES)
        .expect("OSVERSIONINFOEXA bytes");
    assert_eq!(&extended_after[21..148], &[0xa5; 127]);
    assert_eq!(&extended_after[148..152], &[0, 0, 0, 0]);
    assert_eq!(&extended_after[152..156], &[0, 1, 1, 0]);
    assert_eq!(runtime, runtime_before_success);

    let memory_before_invalid_size = memory.stats();
    let invalid_size = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[2].0,
        frames[2].1[0],
        &policy,
    );
    assert_eq!(invalid_size.eax, RegisterUpdate::Set(0));
    assert_eq!(invalid_size.last_error, 122);
    assert!(invalid_size.guest_writes.is_empty());
    assert_eq!(memory.stats(), memory_before_invalid_size);
    assert_eq!(
        memory
            .read(GuestAddress(INVALID_SIZE_OUTPUT), EXTENDED_BYTES)
            .unwrap(),
        invalid_before
    );

    runtime.set_current_last_error(87).unwrap();
    let runtime_before_faults = runtime.clone();
    let memory_before_faults = memory.stats();
    let base_before_faults = memory
        .read(GuestAddress(BASE_OUTPUT), BASE_BYTES)
        .unwrap()
        .to_vec();
    for (frame, selected_policy, expected_code) in [
        (
            &frames[3],
            &Kernel32CompatibilityPolicy::controlled_v1(),
            "get-version-ex-a-policy-disabled",
        ),
        (&frames[4], &policy, "null-pointer-forbidden"),
        (&frames[5], &policy, "guest-page-fault"),
    ] {
        let rejected = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            dispatcher.dispatch_owned(
                owned_trap(&endpoint, frame.0, frame.1[0]),
                &mut session,
                |contract, frame, session| {
                    dispatch_kernel32_api_with_policy(selected_policy, contract, frame, session)
                },
            )
        };
        assert!(matches!(
            rejected,
            DispatchOutcome::Fault(ref fault)
                if fault.code == expected_code
                    && matches!(fault.class, ApiFaultClass::Unsupported | ApiFaultClass::InvalidCall)
        ));
        assert_eq!(runtime, runtime_before_faults);
        assert_eq!(memory.stats(), memory_before_faults);
        assert_eq!(
            memory.read(GuestAddress(BASE_OUTPUT), BASE_BYTES).unwrap(),
            base_before_faults
        );
        assert_eq!(
            memory.read(GuestAddress(CROSS_PAGE_OUTPUT), 16).unwrap(),
            cross_page_prefix
        );
    }

    memory
        .protect(
            owner,
            GuestRange::new(GuestAddress(OUTPUT_BASE), GUEST_PAGE_SIZE)
                .expect("OSVERSIONINFOA output page range"),
            PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .expect("make OSVERSIONINFOA output page read-only");
    acknowledge_pending_delta(&mut memory);
    let memory_before_read_only = memory.stats();
    let read_only = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[6].0, frames[6].1[0]),
            &mut session,
            |contract, frame, session| {
                dispatch_kernel32_api_with_policy(&policy, contract, frame, session)
            },
        )
    };
    assert!(matches!(
        read_only,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::InvalidCall && fault.code == "guest-page-fault"
    ));
    assert_eq!(runtime, runtime_before_faults);
    assert_eq!(memory.stats(), memory_before_read_only);
}

#[test]
fn get_acp_reads_only_the_policy_owned_active_code_page() {
    let (mut memory, owner) = initialized_memory();
    let frames = [
        (STACK_BASE, vec![0x1000_0589]),
        (STACK_BASE + 0x20, vec![0x1000_058a]),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x5c);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize GetACP IAT");
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
            name: "GetACP".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented GetACP endpoint");
    assert_eq!(contract.implementation_id, GET_ACP_IMPLEMENTATION);
    assert!(contract.abi.arguments.is_empty());
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::Preserve);
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let mut policy = Kernel32CompatibilityPolicy::controlled_v1();
    policy.id = "public-get-acp-v1".to_owned();
    policy.ansi_command_line = true;
    policy.wide_environment_strings = true;
    policy.wide_char_to_multi_byte = Some(Kernel32WideCharToMultiBytePolicy {
        active_code_page: 932,
        maximum_source_code_units: 65_536,
    });
    policy.validate().expect("valid active-code-page policy");
    let mut runtime = controlled_runtime(controlled_startup_info());
    runtime.set_current_last_error(87).unwrap();
    let runtime_before = runtime.clone();
    let memory_before = memory.stats();

    let completion = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[0].0,
        frames[0].1[0],
        &policy,
    );
    assert_eq!(completion.eax, RegisterUpdate::Set(932));
    assert_eq!(completion.stack_pointer, GuestAddress(frames[0].0 + 4));
    assert_eq!(completion.last_error, 87);
    assert!(completion.guest_writes.is_empty());
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.stats(), memory_before);

    let disabled = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[1].0, frames[1].1[0]),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        disabled,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "get-acp-policy-disabled"
    ));
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.stats(), memory_before);
}

#[test]
fn get_system_default_lang_id_zero_extends_policy_owned_u16_without_mutation() {
    let (mut memory, owner) = initialized_memory();
    let frames = [
        (STACK_BASE, vec![0x1000_058b]),
        (STACK_BASE + 0x20, vec![0x1000_058c]),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x60);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize GetSystemDefaultLangID IAT");
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
            name: "GetSystemDefaultLangID".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented GetSystemDefaultLangID endpoint");
    assert_eq!(
        contract.implementation_id,
        GET_SYSTEM_DEFAULT_LANG_ID_IMPLEMENTATION
    );
    assert!(contract.abi.arguments.is_empty());
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U16);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::Preserve);
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let policy: Kernel32CompatibilityPolicy = serde_json::from_str(include_str!(
        "../../../fixtures/runtime-policies/kernel32-get-system-default-lang-id-v1.json"
    ))
    .expect("parse system-default LANGID policy fixture");
    policy
        .validate()
        .expect("valid system-default LANGID policy");
    let mut runtime = controlled_runtime(controlled_startup_info());
    runtime.set_current_last_error(0x1234_5678).unwrap();
    let runtime_before = runtime.clone();
    let memory_before = memory.stats();

    let completion = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[0].0,
        frames[0].1[0],
        &policy,
    );
    assert_eq!(completion.eax, RegisterUpdate::Set(0x0000_0411));
    assert_eq!(completion.stack_pointer, GuestAddress(frames[0].0 + 4));
    assert_eq!(completion.last_error, 0x1234_5678);
    assert!(completion.guest_writes.is_empty());
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.stats(), memory_before);

    let disabled = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[1].0, frames[1].1[0]),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        disabled,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "get-system-default-lang-id-policy-disabled"
    ));
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.stats(), memory_before);
}

#[test]
fn get_drive_type_a_queries_process_image_drive_without_host_or_state_mutation() {
    const C_ROOT: u32 = OUTPUT_BASE + 0x180;
    const C_ALIAS: u32 = OUTPUT_BASE + 0x190;
    const OTHER_ROOT: u32 = OUTPUT_BASE + 0x1a0;
    const NON_ROOT: u32 = OUTPUT_BASE + 0x1b0;
    const RELATIVE: u32 = OUTPUT_BASE + 0x1c0;
    const EMPTY: u32 = OUTPUT_BASE + 0x1d0;
    const UNTERMINATED: u32 = OUTPUT_BASE + 0x1e0;

    let (mut memory, owner) = initialized_memory();
    for (address, bytes) in [
        (C_ROOT, b"C:\\\0".as_slice()),
        (C_ALIAS, b"c:/\0".as_slice()),
        (OTHER_ROOT, b"D:\\\0".as_slice()),
        (NON_ROOT, b"C:\\Vine\0".as_slice()),
        (RELATIVE, b".\0".as_slice()),
        (EMPTY, b"\0".as_slice()),
        (UNTERMINATED, b"ABCD".as_slice()),
    ] {
        memory
            .write_initialization(owner, GuestAddress(address), bytes)
            .expect("initialize ANSI drive path");
    }
    let frames = [
        (STACK_BASE, vec![0x1000_0590, 0]),
        (STACK_BASE + 0x20, vec![0x1000_0591, C_ROOT]),
        (STACK_BASE + 0x40, vec![0x1000_0592, C_ALIAS]),
        (STACK_BASE + 0x60, vec![0x1000_0593, OTHER_ROOT]),
        (STACK_BASE + 0x80, vec![0x1000_0594, NON_ROOT]),
        (STACK_BASE + 0xa0, vec![0x1000_0595, RELATIVE]),
        (STACK_BASE + 0xc0, vec![0x1000_0596, EMPTY]),
        (STACK_BASE + 0xe0, vec![0x1000_0597, UNTERMINATED]),
        (STACK_BASE + 0x100, vec![0x1000_0598, C_ROOT]),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x68);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize GetDriveTypeA IAT");
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
            name: "GetDriveTypeA".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented GetDriveTypeA endpoint");
    assert_eq!(contract.implementation_id, GET_DRIVE_TYPE_A_IMPLEMENTATION);
    assert_eq!(contract.abi.arguments.len(), 1);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let policy: Kernel32CompatibilityPolicy = serde_json::from_str(include_str!(
        "../../../fixtures/runtime-policies/kernel32-get-drive-type-a-v1.json"
    ))
    .expect("parse drive-type policy fixture");
    policy.validate().expect("valid drive-type policy");
    let mut runtime = controlled_runtime(controlled_startup_info());
    runtime.set_current_last_error(0x1234_5678).unwrap();
    let runtime_before = runtime.clone();
    let memory_before = memory.stats();

    for (index, expected) in [3_u32, 3, 3, 1, 1, 1, 1].into_iter().enumerate() {
        let completion = dispatch_owned_with_policy(
            &dispatcher,
            &mut memory,
            &mut runtime,
            &endpoint,
            frames[index].0,
            frames[index].1[0],
            &policy,
        );
        assert_eq!(completion.eax, RegisterUpdate::Set(expected));
        assert_eq!(completion.stack_pointer, GuestAddress(frames[index].0 + 8));
        assert_eq!(completion.last_error, 0x1234_5678);
        assert!(completion.guest_writes.is_empty());
        assert_eq!(runtime, runtime_before);
        assert_eq!(memory.stats(), memory_before);
    }

    let mut bounded_policy = policy.clone();
    bounded_policy.get_drive_type_a = Some(Kernel32GetDriveTypeAPolicy {
        process_image_drive_only: true,
        maximum_path_bytes: 4,
    });
    bounded_policy
        .validate()
        .expect("valid four-byte path bound");
    let unterminated = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[7].0, frames[7].1[0]),
            &mut session,
            |contract, frame, session| {
                dispatch_kernel32_api_with_policy(&bounded_policy, contract, frame, session)
            },
        )
    };
    assert!(matches!(
        unterminated,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::ResourceLimit
                && fault.code == "get-drive-type-a-path-not-terminated"
    ));
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.stats(), memory_before);

    let mut disabled_policy = policy.clone();
    disabled_policy.get_drive_type_a = None;
    disabled_policy.validate().expect("valid preceding policy");
    let disabled = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[8].0, frames[8].1[0]),
            &mut session,
            |contract, frame, session| {
                dispatch_kernel32_api_with_policy(&disabled_policy, contract, frame, session)
            },
        )
    };
    assert!(matches!(
        disabled,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "get-drive-type-a-policy-disabled"
    ));
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.stats(), memory_before);
}

#[test]
fn get_cp_info_writes_only_policy_owned_active_page_fields_and_rejects_before_mutation() {
    const CPINFO_OUTPUT: u32 = OUTPUT_BASE + 0x200;
    const CROSS_REGION_OUTPUT: u32 = MEMORY_BASE.0 + 0x2ff4;
    const EXPECTED_CPINFO: [u8; 18] = [
        2, 0, 0, 0, b'?', 0, 0x81, 0x9f, 0xe0, 0xfc, 0, 0, 0, 0, 0, 0, 0, 0,
    ];

    let (mut memory, owner) = initialized_memory();
    let frames = [
        (STACK_BASE, vec![0x1000_058b, 932, CPINFO_OUTPUT]),
        (STACK_BASE + 0x20, vec![0x1000_058c, 65001, CPINFO_OUTPUT]),
        (STACK_BASE + 0x40, vec![0x1000_058d, 932, CPINFO_OUTPUT]),
        (
            STACK_BASE + 0x60,
            vec![0x1000_058e, 932, CROSS_REGION_OUTPUT],
        ),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    memory
        .write_initialization(owner, GuestAddress(CPINFO_OUTPUT), &[0xa5; 32])
        .expect("initialize CPINFO output and trailing guard");
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x60);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize GetCPInfo IAT");
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
            name: "GetCPInfo".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented GetCPInfo endpoint");
    assert_eq!(contract.implementation_id, GET_CP_INFO_IMPLEMENTATION);
    assert_eq!(contract.abi.arguments.len(), 2);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::Bool32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::Preserve);
    assert!(matches!(
        contract.abi.arguments[1].value,
        ApiValueSchema::Pointer { ref constraint }
            if constraint.byte_length == EXPECTED_CPINFO.len() as u32
                && constraint.alignment == 4
    ));
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let mut policy = Kernel32CompatibilityPolicy::controlled_v1();
    policy.id = "public-get-cp-info-v1".to_owned();
    policy.ansi_command_line = true;
    policy.wide_environment_strings = true;
    policy.wide_char_to_multi_byte = Some(Kernel32WideCharToMultiBytePolicy {
        active_code_page: 932,
        maximum_source_code_units: 65_536,
    });
    policy.active_code_page_info = Some(Kernel32CodePageInfoPolicy {
        max_char_size: 2,
        default_char: [b'?', 0],
        lead_byte: [0x81, 0x9f, 0xe0, 0xfc, 0, 0, 0, 0, 0, 0, 0, 0],
    });
    policy
        .validate()
        .expect("valid active-code-page information policy");
    let mut runtime = controlled_runtime(controlled_startup_info());
    runtime.set_current_last_error(87).unwrap();
    let runtime_before = runtime.clone();

    let completion = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[0].0,
        frames[0].1[0],
        &policy,
    );
    assert_eq!(completion.eax, RegisterUpdate::Set(1));
    assert_eq!(completion.stack_pointer, GuestAddress(frames[0].0 + 12));
    assert_eq!(completion.last_error, 87);
    assert_eq!(completion.guest_writes.len(), 1);
    assert_eq!(completion.guest_writes[0].write.address.0, CPINFO_OUTPUT);
    assert_eq!(completion.guest_writes[0].write.bytes, EXPECTED_CPINFO);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(CPINFO_OUTPUT), EXPECTED_CPINFO.len())
            .expect("CPINFO fields"),
        EXPECTED_CPINFO
    );
    assert_eq!(
        memory
            .read(
                GuestAddress(CPINFO_OUTPUT + EXPECTED_CPINFO.len() as u32),
                32 - EXPECTED_CPINFO.len(),
            )
            .expect("CPINFO trailing guard"),
        [0xa5; 14]
    );
    assert_eq!(runtime, runtime_before);

    let output_before = memory
        .read(GuestAddress(CPINFO_OUTPUT), 32)
        .expect("output before unsupported code page")
        .to_vec();
    let runtime_before_unsupported = runtime.clone();
    let memory_before_unsupported = memory.stats();
    let unsupported = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[1].0, frames[1].1[0]),
            &mut session,
            |contract, frame, session| {
                dispatch_kernel32_api_with_policy(&policy, contract, frame, session)
            },
        )
    };
    assert!(matches!(
        unsupported,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "get-cp-info-code-page-unsupported"
    ));
    assert_eq!(runtime, runtime_before_unsupported);
    assert_eq!(memory.stats(), memory_before_unsupported);
    assert_eq!(
        memory.read(GuestAddress(CPINFO_OUTPUT), 32).unwrap(),
        output_before
    );

    let disabled = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[2].0, frames[2].1[0]),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        disabled,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "get-cp-info-policy-disabled"
    ));
    assert_eq!(runtime, runtime_before_unsupported);
    assert_eq!(memory.stats(), memory_before_unsupported);

    let invalid_output = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[3].0, frames[3].1[0]),
            &mut session,
            |contract, frame, session| {
                dispatch_kernel32_api_with_policy(&policy, contract, frame, session)
            },
        )
    };
    assert!(matches!(
        invalid_output,
        DispatchOutcome::Fault(ref fault) if fault.class == ApiFaultClass::InvalidCall
    ));
    assert_eq!(runtime, runtime_before_unsupported);
    assert_eq!(memory.stats(), memory_before_unsupported);
}

#[test]
fn processor_feature_query_returns_only_explicit_policy_values_without_mutation() {
    let (mut memory, owner) = initialized_memory();
    let frames = [
        (STACK_BASE, vec![0x1000_058f, 10]),
        (STACK_BASE + 0x20, vec![0x1000_0590, 6]),
        (STACK_BASE + 0x40, vec![0x1000_0591, 7]),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x64);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize IsProcessorFeaturePresent IAT");
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
            name: "IsProcessorFeaturePresent".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented IsProcessorFeaturePresent endpoint");
    assert_eq!(
        contract.implementation_id,
        IS_PROCESSOR_FEATURE_PRESENT_IMPLEMENTATION
    );
    assert_eq!(contract.abi.arguments.len(), 1);
    assert_eq!(contract.abi.arguments[0].value, ApiValueSchema::U32);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::Bool32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::Preserve);
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let mut policy = Kernel32CompatibilityPolicy::controlled_v1();
    policy.id = "public-processor-features-v1".to_owned();
    policy.processor_features = vec![
        Kernel32ProcessorFeaturePolicy {
            index: 10,
            present: true,
        },
        Kernel32ProcessorFeaturePolicy {
            index: 6,
            present: false,
        },
    ];
    policy.validate().expect("valid processor-feature policy");
    let mut runtime = controlled_runtime(controlled_startup_info());
    runtime.set_current_last_error(87).unwrap();
    let runtime_before = runtime.clone();
    let memory_before = memory.stats();

    let present = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[0].0,
        frames[0].1[0],
        &policy,
    );
    assert_eq!(present.eax, RegisterUpdate::Set(1));
    assert_eq!(present.stack_pointer, GuestAddress(frames[0].0 + 8));
    assert_eq!(present.last_error, 87);
    assert!(present.guest_writes.is_empty());
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.stats(), memory_before);

    let absent = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[1].0,
        frames[1].1[0],
        &policy,
    );
    assert_eq!(absent.eax, RegisterUpdate::Set(0));
    assert_eq!(absent.stack_pointer, GuestAddress(frames[1].0 + 8));
    assert_eq!(absent.last_error, 87);
    assert!(absent.guest_writes.is_empty());
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.stats(), memory_before);

    let unsupported = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[2].0, frames[2].1[0]),
            &mut session,
            |contract, frame, session| {
                dispatch_kernel32_api_with_policy(&policy, contract, frame, session)
            },
        )
    };
    assert!(matches!(
        unsupported,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "processor-feature-index-unsupported"
    ));
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.stats(), memory_before);
}

#[test]
fn unhandled_exception_filter_replacement_is_process_owned_and_fails_before_mutation() {
    const FILTER_A: u32 = OUTPUT_BASE + 0x100;
    const FILTER_B: u32 = OUTPUT_BASE + 0x200;

    let (mut memory, owner) = initialized_memory();
    memory
        .protect(
            owner,
            GuestRange::new(GuestAddress(OUTPUT_BASE), GUEST_PAGE_SIZE)
                .expect("filter code page range"),
            PageProtection {
                read: true,
                write: true,
                execute: true,
            },
        )
        .expect("make filter code page executable");
    let frames = [
        (STACK_BASE, vec![0x1000_05a1, FILTER_A]),
        (STACK_BASE + 0x20, vec![0x1000_05a2, FILTER_B]),
        (STACK_BASE + 0x40, vec![0x1000_05a3, 0]),
        (STACK_BASE + 0x60, vec![0x1000_05a4, 0x3000_0000]),
        (STACK_BASE + 0x80, vec![0x1000_05a5, FILTER_A]),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x68);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize SetUnhandledExceptionFilter IAT");
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
            name: "SetUnhandledExceptionFilter".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented SetUnhandledExceptionFilter endpoint");
    assert_eq!(
        contract.implementation_id,
        SET_UNHANDLED_EXCEPTION_FILTER_IMPLEMENTATION
    );
    assert_eq!(contract.abi.arguments.len(), 1);
    assert_eq!(
        contract.abi.arguments[0].value,
        ApiValueSchema::OpaquePointer {
            nullable: true,
            alignment: 1,
        }
    );
    assert_eq!(
        contract.abi.return_value,
        ApiReturnSchema::OpaquePointer { nullable: true }
    );
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::Preserve);
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let mut runtime = controlled_runtime(controlled_startup_info());
    runtime.set_current_last_error(87).unwrap();
    let memory_before = memory.stats();

    let install = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[0].0,
        frames[0].1[0],
    );
    assert_eq!(install.eax, RegisterUpdate::Set(0));
    assert_eq!(install.stack_pointer, GuestAddress(frames[0].0 + 8));
    assert_eq!(install.last_error, 87);
    assert!(install.guest_writes.is_empty());
    assert_eq!(
        runtime.unhandled_exception_filter(),
        Some(GuestAddress(FILTER_A))
    );

    let replace = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[1].0,
        frames[1].1[0],
    );
    assert_eq!(replace.eax, RegisterUpdate::Set(FILTER_A));
    assert_eq!(replace.last_error, 87);
    assert_eq!(
        runtime.unhandled_exception_filter(),
        Some(GuestAddress(FILTER_B))
    );

    let remove = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[2].0,
        frames[2].1[0],
    );
    assert_eq!(remove.eax, RegisterUpdate::Set(FILTER_B));
    assert_eq!(remove.last_error, 87);
    assert_eq!(runtime.unhandled_exception_filter(), None);
    assert_eq!(memory.stats(), memory_before);

    let runtime_before_rejection = runtime.clone();
    let rejected = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[3].0, frames[3].1[0]),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        rejected,
        DispatchOutcome::Fault(ref fault) if fault.class == ApiFaultClass::InvalidCall
    ));
    assert_eq!(runtime, runtime_before_rejection);
    assert_eq!(memory.stats(), memory_before);

    let reinstall = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[4].0,
        frames[4].1[0],
    );
    assert_eq!(reinstall.eax, RegisterUpdate::Set(0));
    assert_eq!(
        runtime.unhandled_exception_filter(),
        Some(GuestAddress(FILTER_A))
    );
    runtime
        .terminate_current_process(0)
        .expect("process termination retires the filter");
    assert_eq!(runtime.unhandled_exception_filter(), None);
    assert_eq!(memory.stats(), memory_before);
}

#[test]
fn tls_get_value_reads_only_the_selected_thread_and_sets_win32_last_error() {
    let (mut memory, owner) = initialized_memory();
    let frames = [
        (STACK_BASE, vec![0x1000_0591, 0]),
        (STACK_BASE + 0x20, vec![0x1000_0592, 0]),
        (STACK_BASE + 0x40, vec![0x1000_0593, 0]),
        (STACK_BASE + 0x60, vec![0x1000_0594, u32::MAX]),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x5c);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize TlsGetValue IAT");
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
            name: "TlsGetValue".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented TlsGetValue endpoint");
    assert_eq!(contract.implementation_id, TLS_GET_VALUE_IMPLEMENTATION);
    assert_eq!(contract.abi.arguments.len(), 1);
    assert_eq!(
        contract.abi.return_value,
        ApiReturnSchema::OpaquePointer { nullable: true }
    );
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let mut runtime = controlled_runtime(controlled_startup_info());
    runtime
        .register_dynamic_endpoint(RuntimeDynamicEndpoint {
            namespace: DYNAMIC_EXPORT_ENDPOINT_NAMESPACE.to_owned(),
            id: 2,
            address: GuestAddress(0x7f02_0010),
            module: "kernel32.dll".to_owned(),
            name: "FlsGetValue".to_owned(),
            lifetime: "process-runtime".to_owned(),
        })
        .expect("register controlled TLS endpoint value");
    let tls_index = runtime.allocate_tls_index().expect("allocate TLS index");
    runtime
        .set_tls_value(tls_index, 0x7f02_0010)
        .expect("set initial-thread TLS value");
    let memory_before = memory.stats();

    let mut expected = runtime.clone();
    expected.set_current_last_error(0).unwrap();
    let initial = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[0].0,
        frames[0].1[0],
    );
    assert_eq!(initial.eax, RegisterUpdate::Set(0x7f02_0010));
    assert_eq!(initial.stack_pointer, GuestAddress(frames[0].0 + 8));
    assert_eq!(initial.last_error, 0);
    assert!(initial.guest_writes.is_empty());
    assert_eq!(runtime, expected);
    assert_eq!(memory.stats(), memory_before);

    let second_thread = RuntimeThreadId(THREAD_ID + 1);
    runtime
        .register_thread(second_thread, 0x5678)
        .expect("register second TLS lane");
    runtime
        .select_thread(second_thread)
        .expect("select second TLS lane");
    let mut expected = runtime.clone();
    expected.set_current_last_error(0).unwrap();
    let unset = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[1].0,
        frames[1].1[0],
    );
    assert_eq!(unset.eax, RegisterUpdate::Set(0));
    assert_eq!(unset.last_error, 0);
    assert_eq!(runtime, expected);
    assert_eq!(memory.stats(), memory_before);

    runtime
        .set_tls_value(tls_index, 0x2345_6789)
        .expect("set second-thread raw TLS value");
    runtime.set_current_last_error(0x6789).unwrap();
    let mut expected = runtime.clone();
    expected.set_current_last_error(0).unwrap();
    let second = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[2].0,
        frames[2].1[0],
    );
    assert_eq!(second.eax, RegisterUpdate::Set(0x2345_6789));
    assert_eq!(second.last_error, 0);
    assert_eq!(runtime, expected);
    assert_eq!(memory.stats(), memory_before);

    runtime.set_current_last_error(122).unwrap();
    let mut expected = runtime.clone();
    expected.set_current_last_error(87).unwrap();
    let invalid = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[3].0,
        frames[3].1[0],
    );
    assert_eq!(invalid.eax, RegisterUpdate::Set(0));
    assert_eq!(invalid.last_error, 87);
    assert_eq!(runtime, expected);
    assert_eq!(memory.stats(), memory_before);

    runtime
        .select_thread(RuntimeThreadId(THREAD_ID))
        .expect("restore initial TLS lane");
    assert_eq!(
        runtime.tls_value(tls_index).unwrap(),
        &RuntimeTlsValue::DynamicEndpoint {
            endpoint_id: 2,
            address: GuestAddress(0x7f02_0010),
        }
    );
}

#[test]
fn heap_alloc_policy_enables_bounded_sizes_without_changing_semantics() {
    const FRAME_BASE: u32 = 0x3000_0100;
    const TEST_IAT_BASE: u32 = 0x3000_2000;

    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 8,
        max_regions: 4,
        max_owners: 4,
        max_access_bytes: 0x4000,
    })
    .expect("memory");
    let owner = memory
        .create_owner(MemoryOwnerKind::Runtime)
        .expect("runtime owner");
    let range = memory
        .reserve_fixed(
            owner,
            MemoryRegionKind::Runtime,
            GuestAddress(0x3000_0000),
            0x3000,
        )
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
    let endpoint_address = GuestAddress(ENDPOINT_BASE);
    memory
        .write_initialization(
            owner,
            GuestAddress(TEST_IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize HeapAlloc IAT");
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(1),
        address: endpoint_address,
        requester_owner: owner,
        module: "KERNEL32.dll".to_owned(),
        iat_rva: Rva(0),
        iat_address: GuestAddress(TEST_IAT_BASE),
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "HeapAlloc".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented HeapAlloc endpoint");
    let frames = [
        (FRAME_BASE, vec![0x1000_0601, 0x6801_0001, 0x8, 0x214]),
        (
            FRAME_BASE + 0x20,
            vec![0x1000_0602, 0x6801_0001, 0x8, 0x800],
        ),
        (FRAME_BASE + 0x40, vec![0x1000_0603, 0x6801_0001, 0, 1]),
        (FRAME_BASE + 0x60, vec![0x1000_0604, 0x6801_0001, 0, 0x220]),
        (FRAME_BASE + 0x80, vec![0x1000_0605, 0x6801_0001, 0, 16]),
        (FRAME_BASE + 0xa0, vec![0x1000_0606, 0x6801_0001, 0x8, 4]),
        (FRAME_BASE + 0xc0, vec![0x1000_0607, 0x6801_0001, 0x8, 128]),
        (FRAME_BASE + 0xe0, vec![0x1000_0608, 0x6801_0001, 0x8, 0xbc]),
        (
            FRAME_BASE + 0x100,
            vec![0x1000_0609, 0x6801_0001, 0, 0x0100_0001],
        ),
        (FRAME_BASE + 0x120, vec![0x1000_060a, 0x6801_0001, 1, 0xbc]),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let mut runtime = controlled_runtime(controlled_startup_info());
    let heap = runtime
        .allocate_private_heap(0, 0x1000, 0)
        .expect("logical private heap");
    assert_eq!(heap.handle, 0x6801_0001);

    let completion = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[0].0,
        frames[0].1[0],
    );
    assert_eq!(completion.eax, RegisterUpdate::Set(0x2000_0000));
    assert_eq!(completion.stack_pointer, GuestAddress(frames[0].0 + 16));
    assert_eq!(completion.last_error, INITIAL_LAST_ERROR);
    let allocation = runtime
        .heap_backing(0x6801_0001)
        .expect("heap backing lookup")
        .expect("materialized heap backing")
        .allocations
        .get(&0x2000_0000)
        .expect("first heap allocation");
    assert_eq!(allocation.id, RuntimeHeapAllocationId(1));
    assert_eq!(allocation.byte_length, 0x214);
    assert_eq!(allocation.reserved_byte_length, 0x218);
    let delta = memory.pending_delta().expect("heap allocation delta");
    assert_eq!(delta.mapping_operations.len(), 2);
    assert_eq!(delta.dirty_pages.len(), 1);
    assert_eq!(delta.dirty_pages[0].address, GuestAddress(0x2000_0000));
    assert_eq!(delta.dirty_pages[0].bytes, vec![0; 0x1000]);
    memory
        .acknowledge_delta(delta.generation)
        .expect("acknowledge heap allocation delta");
    assert_eq!(
        memory
            .read(GuestAddress(0x2000_0000), 0x218)
            .expect("zeroed heap allocation"),
        vec![0; 0x218]
    );

    let runtime_before_rejection = runtime.clone();
    let memory_before_rejection = memory.stats();
    let rejected = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[1].0, frames[1].1[0]),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        rejected,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "heap-alloc-variant-unsupported"
    ));
    assert_eq!(runtime, runtime_before_rejection);
    assert_eq!(memory.stats(), memory_before_rejection);

    let extended_policy = compatibility_policy(
        "heap-two-test",
        vec![RUNTIME_STANDARD_INPUT_SELECTOR],
        vec![(RUNTIME_STANDARD_INPUT_SELECTOR, 2)],
        vec![(0x8, vec![0x214, 0x800])],
    );
    let second = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[1].0,
        frames[1].1[0],
        &extended_policy,
    );
    assert_eq!(second.eax, RegisterUpdate::Set(0x2000_0218));
    assert_eq!(second.stack_pointer, GuestAddress(frames[1].0 + 16));
    assert_eq!(second.last_error, INITIAL_LAST_ERROR);
    let backing = runtime
        .heap_backing(0x6801_0001)
        .expect("heap backing lookup")
        .expect("materialized heap backing");
    assert_eq!(backing.allocations.len(), 2);
    let second_allocation = backing
        .allocations
        .get(&0x2000_0218)
        .expect("second heap allocation");
    assert_eq!(second_allocation.id, RuntimeHeapAllocationId(2));
    assert_eq!(second_allocation.byte_length, 0x800);
    assert_eq!(second_allocation.reserved_byte_length, 0x800);
    let second_delta = memory.pending_delta().expect("second allocation delta");
    assert!(second_delta.mapping_operations.is_empty());
    assert_eq!(second_delta.dirty_pages.len(), 1);
    assert_eq!(
        second_delta.dirty_pages[0].address,
        GuestAddress(0x2000_0000)
    );
    memory
        .acknowledge_delta(second_delta.generation)
        .expect("acknowledge second allocation delta");
    assert_eq!(
        memory
            .read(GuestAddress(0x2000_0000), 0xa18)
            .expect("first and second zeroed allocations"),
        vec![0; 0xa18]
    );

    let size_one_policy = compatibility_policy(
        "heap-size-one-test",
        vec![RUNTIME_STANDARD_INPUT_SELECTOR],
        vec![(RUNTIME_STANDARD_INPUT_SELECTOR, 2)],
        vec![(0x8, vec![0x214, 0x800]), (0, vec![1])],
    );
    let size_one = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[2].0,
        frames[2].1[0],
        &size_one_policy,
    );
    assert_eq!(size_one.eax, RegisterUpdate::Set(0x2000_0a18));
    assert_eq!(size_one.stack_pointer, GuestAddress(frames[2].0 + 16));
    assert_eq!(size_one.last_error, INITIAL_LAST_ERROR);
    let size_one_allocation = runtime
        .heap_backing(0x6801_0001)
        .unwrap()
        .unwrap()
        .allocations
        .get(&0x2000_0a18)
        .expect("one-byte uninitialized allocation");
    assert_eq!(size_one_allocation.id, RuntimeHeapAllocationId(3));
    assert_eq!(size_one_allocation.byte_length, 1);
    assert_eq!(size_one_allocation.reserved_byte_length, 8);
    assert_eq!(
        size_one_allocation.initialization,
        RuntimeHeapInitialization::DeterministicUninitialized
    );
    let size_one_delta = memory.pending_delta().expect("size-one allocation delta");
    assert!(size_one_delta.mapping_operations.is_empty());
    assert_eq!(size_one_delta.dirty_pages.len(), 1);
    memory
        .acknowledge_delta(size_one_delta.generation)
        .expect("acknowledge size-one allocation delta");
    assert_eq!(
        memory
            .read(GuestAddress(0x2000_0a18), 8)
            .expect("deterministic uninitialized allocation"),
        vec![RUNTIME_PRIVATE_HEAP_UNINITIALIZED_BYTE; 8]
    );

    let runtime_before_size_544_rejection = runtime.clone();
    let memory_before_size_544_rejection = memory.stats();
    let size_544_rejected = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[3].0, frames[3].1[0]),
            &mut session,
            |contract, frame, session| {
                dispatch_kernel32_api_with_policy(&size_one_policy, contract, frame, session)
            },
        )
    };
    assert!(matches!(
        size_544_rejected,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "heap-alloc-variant-unsupported"
    ));
    assert_eq!(runtime, runtime_before_size_544_rejection);
    assert_eq!(memory.stats(), memory_before_size_544_rejection);

    let size_544_policy = compatibility_policy(
        "heap-size-544-test",
        vec![RUNTIME_STANDARD_INPUT_SELECTOR],
        vec![(RUNTIME_STANDARD_INPUT_SELECTOR, 2)],
        vec![(0x8, vec![0x214, 0x800]), (0, vec![1, 0x220])],
    );
    let size_544 = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[3].0,
        frames[3].1[0],
        &size_544_policy,
    );
    assert_eq!(size_544.eax, RegisterUpdate::Set(0x2000_0a20));
    assert_eq!(size_544.stack_pointer, GuestAddress(frames[3].0 + 16));
    assert_eq!(size_544.last_error, INITIAL_LAST_ERROR);
    let size_544_allocation = runtime
        .heap_backing(0x6801_0001)
        .unwrap()
        .unwrap()
        .allocations
        .get(&0x2000_0a20)
        .expect("544-byte uninitialized allocation");
    assert_eq!(size_544_allocation.id, RuntimeHeapAllocationId(4));
    assert_eq!(size_544_allocation.byte_length, 0x220);
    assert_eq!(size_544_allocation.reserved_byte_length, 0x220);
    assert_eq!(
        size_544_allocation.initialization,
        RuntimeHeapInitialization::DeterministicUninitialized
    );
    let size_544_delta = memory.pending_delta().expect("size-544 allocation delta");
    assert!(size_544_delta.mapping_operations.is_empty());
    assert_eq!(size_544_delta.dirty_pages.len(), 1);
    memory
        .acknowledge_delta(size_544_delta.generation)
        .expect("acknowledge size-544 allocation delta");
    assert_eq!(
        memory
            .read(GuestAddress(0x2000_0a20), 0x220)
            .expect("deterministic uninitialized 544-byte allocation"),
        vec![RUNTIME_PRIVATE_HEAP_UNINITIALIZED_BYTE; 0x220]
    );

    let runtime_before_size_16_rejection = runtime.clone();
    let memory_before_size_16_rejection = memory.stats();
    let size_16_rejected = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[4].0, frames[4].1[0]),
            &mut session,
            |contract, frame, session| {
                dispatch_kernel32_api_with_policy(&size_544_policy, contract, frame, session)
            },
        )
    };
    assert!(matches!(
        size_16_rejected,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "heap-alloc-variant-unsupported"
    ));
    assert_eq!(runtime, runtime_before_size_16_rejection);
    assert_eq!(memory.stats(), memory_before_size_16_rejection);

    let size_16_policy = compatibility_policy(
        "heap-size-16-test",
        vec![RUNTIME_STANDARD_INPUT_SELECTOR],
        vec![(RUNTIME_STANDARD_INPUT_SELECTOR, 2)],
        vec![(0x8, vec![0x214, 0x800]), (0, vec![1, 16, 0x220])],
    );
    let size_16 = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[4].0,
        frames[4].1[0],
        &size_16_policy,
    );
    assert_eq!(size_16.eax, RegisterUpdate::Set(0x2000_0c40));
    assert_eq!(size_16.stack_pointer, GuestAddress(frames[4].0 + 16));
    assert_eq!(size_16.last_error, INITIAL_LAST_ERROR);
    let size_16_allocation = runtime
        .heap_backing(0x6801_0001)
        .unwrap()
        .unwrap()
        .allocations
        .get(&0x2000_0c40)
        .expect("16-byte uninitialized allocation");
    assert_eq!(size_16_allocation.id, RuntimeHeapAllocationId(5));
    assert_eq!(size_16_allocation.byte_length, 16);
    assert_eq!(size_16_allocation.reserved_byte_length, 16);
    assert_eq!(
        size_16_allocation.initialization,
        RuntimeHeapInitialization::DeterministicUninitialized
    );
    let size_16_delta = memory.pending_delta().expect("size-16 allocation delta");
    assert!(size_16_delta.mapping_operations.is_empty());
    assert_eq!(size_16_delta.dirty_pages.len(), 1);
    memory
        .acknowledge_delta(size_16_delta.generation)
        .expect("acknowledge size-16 allocation delta");
    assert_eq!(
        memory
            .read(GuestAddress(0x2000_0c40), 16)
            .expect("deterministic uninitialized 16-byte allocation"),
        vec![RUNTIME_PRIVATE_HEAP_UNINITIALIZED_BYTE; 16]
    );

    let runtime_before_size_4_rejection = runtime.clone();
    let memory_before_size_4_rejection = memory.stats();
    let size_4_rejected = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[5].0, frames[5].1[0]),
            &mut session,
            |contract, frame, session| {
                dispatch_kernel32_api_with_policy(&size_16_policy, contract, frame, session)
            },
        )
    };
    assert!(matches!(
        size_4_rejected,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "heap-alloc-variant-unsupported"
    ));
    assert_eq!(runtime, runtime_before_size_4_rejection);
    assert_eq!(memory.stats(), memory_before_size_4_rejection);

    let size_4_policy = compatibility_policy(
        "heap-size-4-test",
        vec![RUNTIME_STANDARD_INPUT_SELECTOR],
        vec![(RUNTIME_STANDARD_INPUT_SELECTOR, 2)],
        vec![(0x8, vec![4, 0x214, 0x800]), (0, vec![1, 16, 0x220])],
    );
    let size_4 = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[5].0,
        frames[5].1[0],
        &size_4_policy,
    );
    assert_eq!(size_4.eax, RegisterUpdate::Set(0x2000_0c50));
    assert_eq!(size_4.stack_pointer, GuestAddress(frames[5].0 + 16));
    assert_eq!(size_4.last_error, INITIAL_LAST_ERROR);
    let size_4_allocation = runtime
        .heap_backing(0x6801_0001)
        .unwrap()
        .unwrap()
        .allocations
        .get(&0x2000_0c50)
        .expect("four-byte zeroed allocation");
    assert_eq!(size_4_allocation.id, RuntimeHeapAllocationId(6));
    assert_eq!(size_4_allocation.byte_length, 4);
    assert_eq!(size_4_allocation.reserved_byte_length, 8);
    assert_eq!(
        size_4_allocation.initialization,
        RuntimeHeapInitialization::Zeroed
    );
    let size_4_delta = memory.pending_delta().expect("size-4 allocation delta");
    assert!(size_4_delta.mapping_operations.is_empty());
    assert_eq!(size_4_delta.dirty_pages.len(), 1);
    memory
        .acknowledge_delta(size_4_delta.generation)
        .expect("acknowledge size-4 allocation delta");
    assert_eq!(
        memory
            .read(GuestAddress(0x2000_0c50), 8)
            .expect("zeroed four-byte allocation"),
        vec![0; 8]
    );

    let runtime_before_later_rejection = runtime.clone();
    let memory_before_later_rejection = memory.stats();
    let later_rejected = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[6].0, frames[6].1[0]),
            &mut session,
            |contract, frame, session| {
                dispatch_kernel32_api_with_policy(&extended_policy, contract, frame, session)
            },
        )
    };
    assert!(matches!(
        later_rejected,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "heap-alloc-variant-unsupported"
    ));
    assert_eq!(runtime, runtime_before_later_rejection);
    assert_eq!(memory.stats(), memory_before_later_rejection);
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);

    let mut size_128_policy = compatibility_policy(
        "heap-size-128-test",
        vec![RUNTIME_STANDARD_INPUT_SELECTOR],
        vec![(RUNTIME_STANDARD_INPUT_SELECTOR, 2)],
        vec![(0x8, vec![4, 128, 0x214, 0x800]), (0, vec![1, 16, 0x220])],
    );
    size_128_policy.heap_free_flags = vec![0];
    size_128_policy.validate().expect("valid size-128 policy");
    let size_128 = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[6].0,
        frames[6].1[0],
        &size_128_policy,
    );
    assert_eq!(size_128.eax, RegisterUpdate::Set(0x2000_0c58));
    let size_128_allocation = runtime
        .heap_backing(0x6801_0001)
        .unwrap()
        .unwrap()
        .allocations
        .get(&0x2000_0c58)
        .expect("128-byte zeroed allocation");
    assert_eq!(size_128_allocation.id, RuntimeHeapAllocationId(7));
    assert_eq!(size_128_allocation.byte_length, 128);
    assert_eq!(size_128_allocation.reserved_byte_length, 128);
    assert_eq!(
        size_128_allocation.initialization,
        RuntimeHeapInitialization::Zeroed
    );
    let size_128_delta = memory.pending_delta().expect("size-128 allocation delta");
    assert!(size_128_delta.mapping_operations.is_empty());
    assert_eq!(size_128_delta.dirty_pages.len(), 1);
    memory
        .acknowledge_delta(size_128_delta.generation)
        .expect("acknowledge size-128 allocation delta");
    assert_eq!(
        memory
            .read(GuestAddress(0x2000_0c58), 128)
            .expect("zeroed 128-byte allocation"),
        vec![0; 128]
    );

    let bounded_heap_policy: Kernel32CompatibilityPolicy = serde_json::from_str(include_str!(
        "../../../fixtures/runtime-policies/kernel32-bounded-heap-v2.json"
    ))
    .expect("bounded heap policy JSON");
    bounded_heap_policy
        .validate()
        .expect("valid bounded heap policy");
    assert!(bounded_heap_policy.allows_heap_allocation(0, 37));
    assert!(bounded_heap_policy.allows_heap_allocation(0x8, 0xbc));
    let third = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[7].0,
        frames[7].1[0],
        &bounded_heap_policy,
    );
    assert_eq!(third.eax, RegisterUpdate::Set(0x2000_0cd8));
    assert_eq!(
        runtime
            .heap_backing(0x6801_0001)
            .unwrap()
            .unwrap()
            .allocations
            .len(),
        8
    );
    let bounded_delta = memory.pending_delta().expect("bounded allocation delta");
    memory
        .acknowledge_delta(bounded_delta.generation)
        .expect("acknowledge bounded allocation delta");

    for rejected_frame in [&frames[8], &frames[9]] {
        let runtime_before_rejection = runtime.clone();
        let memory_before_rejection = memory.stats();
        let rejected = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            dispatcher.dispatch_owned(
                owned_trap(&endpoint, rejected_frame.0, rejected_frame.1[0]),
                &mut session,
                |contract, frame, session| {
                    dispatch_kernel32_api_with_policy(
                        &bounded_heap_policy,
                        contract,
                        frame,
                        session,
                    )
                },
            )
        };
        assert!(matches!(
            rejected,
            DispatchOutcome::Fault(ref fault)
                if fault.class == ApiFaultClass::Unsupported
                    && fault.code == "heap-alloc-variant-unsupported"
        ));
        assert_eq!(runtime, runtime_before_rejection);
        assert_eq!(memory.stats(), memory_before_rejection);
    }
}

#[test]
fn heap_free_policy_retires_only_an_exact_live_allocation_transactionally() {
    const FRAME_BASE: u32 = 0x3000_0100;
    const TEST_IAT_BASE: u32 = 0x3000_2000;

    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 8,
        max_regions: 8,
        max_owners: 8,
        max_access_bytes: 0x4000,
    })
    .expect("memory");
    let owner = memory
        .create_owner(MemoryOwnerKind::Runtime)
        .expect("runtime owner");
    let range = memory
        .reserve_fixed(
            owner,
            MemoryRegionKind::Runtime,
            GuestAddress(0x3000_0000),
            0x3000,
        )
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
    let endpoint_address = GuestAddress(ENDPOINT_BASE);
    memory
        .write_initialization(
            owner,
            GuestAddress(TEST_IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize HeapFree IAT");
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(1),
        address: endpoint_address,
        requester_owner: owner,
        module: "KERNEL32.dll".to_owned(),
        iat_rva: Rva(0),
        iat_address: GuestAddress(TEST_IAT_BASE),
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "HeapFree".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented HeapFree endpoint");
    assert_eq!(contract.abi.return_value, ApiReturnSchema::Bool32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);

    let frames = [
        (FRAME_BASE, vec![0x1000_0701, 0x6801_0001, 0, 0x2000_0000]),
        (
            FRAME_BASE + 0x20,
            vec![0x1000_0702, 0x6801_0001, 1, 0x2000_0000],
        ),
        (FRAME_BASE + 0x40, vec![0x1000_0703, 0x6801_0001, 0, 0]),
        (
            FRAME_BASE + 0x60,
            vec![0x1000_0704, 0x6801_0001, 0, 0x2000_0010],
        ),
        (
            FRAME_BASE + 0x80,
            vec![0x1000_0705, 0x6801_0001, 0, 0x2100_0000],
        ),
        (
            FRAME_BASE + 0xa0,
            vec![0x1000_0706, 0x6801_0001, 0, 0x2000_0000],
        ),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");

    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let mut runtime = controlled_runtime(controlled_startup_info());
    let heap = runtime
        .allocate_private_heap(0, 0x1000, 0)
        .expect("first logical private heap")
        .handle;
    let foreign_heap = runtime
        .allocate_private_heap(0, 0x1000, 0)
        .expect("second logical private heap")
        .handle;
    assert_eq!(heap, 0x6801_0001);
    assert_eq!(foreign_heap, 0x6801_0002);

    let reached = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(
            heap,
            1,
            RuntimeHeapInitialization::DeterministicUninitialized,
        )
        .expect("reached one-byte allocation");
    let reached_delta = memory.pending_delta().expect("reached allocation delta");
    memory
        .acknowledge_delta(reached_delta.generation)
        .expect("acknowledge reached allocation");
    let interior_owner = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(
            heap,
            0x20,
            RuntimeHeapInitialization::DeterministicUninitialized,
        )
        .expect("interior-pointer allocation");
    let interior_delta = memory.pending_delta().expect("interior allocation delta");
    memory
        .acknowledge_delta(interior_delta.generation)
        .expect("acknowledge interior allocation");
    let foreign = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(
            foreign_heap,
            0x10,
            RuntimeHeapInitialization::DeterministicUninitialized,
        )
        .expect("foreign-heap allocation");
    let foreign_delta = memory.pending_delta().expect("foreign allocation delta");
    memory
        .acknowledge_delta(foreign_delta.generation)
        .expect("acknowledge foreign allocation");
    assert_eq!(reached.address, GuestAddress(0x2000_0000));
    assert_eq!(reached.reserved_byte_length, 8);
    assert_eq!(interior_owner.address, GuestAddress(0x2000_0008));
    assert_eq!(foreign.address, GuestAddress(0x2100_0000));

    let mut free_policy = compatibility_policy(
        "heap-free-test",
        vec![RUNTIME_STANDARD_INPUT_SELECTOR],
        vec![(RUNTIME_STANDARD_INPUT_SELECTOR, 2)],
        vec![(0x8, vec![0x214])],
    );
    free_policy.heap_free_flags = vec![0];
    free_policy.validate().expect("valid HeapFree policy");

    let assert_rejection_preserves_state =
        |outcome: &DispatchOutcome,
         runtime: &RuntimeState,
         expected_runtime: &RuntimeState,
         memory: &GuestMemory,
         expected_memory: vine_core::MemoryStats| {
            assert!(matches!(outcome, DispatchOutcome::Fault(_)));
            assert_eq!(runtime, expected_runtime);
            assert_eq!(memory.stats(), expected_memory);
        };

    let before_disabled = runtime.clone();
    let memory_before_disabled = memory.stats();
    let disabled = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[0].0, frames[0].1[0]),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        disabled,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "heap-free-variant-unsupported"
    ));
    assert_rejection_preserves_state(
        &disabled,
        &runtime,
        &before_disabled,
        &memory,
        memory_before_disabled,
    );

    for (frame_index, expected_code) in [
        (1, "heap-free-variant-unsupported"),
        (2, "null-pointer-forbidden"),
        (3, "heap-allocation-pointer-invalid"),
        (4, "heap-allocation-pointer-invalid"),
    ] {
        let expected_runtime = runtime.clone();
        let expected_memory = memory.stats();
        let outcome = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            dispatcher.dispatch_owned(
                owned_trap(&endpoint, frames[frame_index].0, frames[frame_index].1[0]),
                &mut session,
                |contract, frame, session| {
                    dispatch_kernel32_api_with_policy(&free_policy, contract, frame, session)
                },
            )
        };
        assert!(
            matches!(
                outcome,
                DispatchOutcome::Fault(ref fault) if fault.code == expected_code
            ),
            "unexpected rejection for frame {frame_index}: {outcome:?}"
        );
        assert_rejection_preserves_state(
            &outcome,
            &runtime,
            &expected_runtime,
            &memory,
            expected_memory,
        );
    }

    let completion = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[0].0,
        frames[0].1[0],
        &free_policy,
    );
    assert_eq!(completion.eax, RegisterUpdate::Set(1));
    assert_eq!(completion.stack_pointer, GuestAddress(frames[0].0 + 16));
    assert_eq!(completion.last_error, INITIAL_LAST_ERROR);
    assert_eq!(runtime.heap(heap).unwrap().live_allocations, 1);
    assert!(
        !runtime
            .heap_backing(heap)
            .unwrap()
            .unwrap()
            .allocations
            .contains_key(&reached.address.0)
    );
    let free_delta = memory.pending_delta().expect("HeapFree delta");
    assert!(free_delta.mapping_operations.is_empty());
    assert_eq!(free_delta.dirty_pages.len(), 1);
    memory
        .acknowledge_delta(free_delta.generation)
        .expect("acknowledge HeapFree delta");
    assert_eq!(
        memory
            .read(reached.address, reached.reserved_byte_length as usize)
            .expect("freed bytes"),
        vec![RUNTIME_PRIVATE_HEAP_FREED_BYTE; reached.reserved_byte_length as usize]
    );

    let before_double_free = runtime.clone();
    let memory_before_double_free = memory.stats();
    let double_free = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[5].0, frames[5].1[0]),
            &mut session,
            |contract, frame, session| {
                dispatch_kernel32_api_with_policy(&free_policy, contract, frame, session)
            },
        )
    };
    assert!(matches!(
        double_free,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::InvalidCall
                && fault.code == "heap-allocation-double-free"
    ));
    assert_rejection_preserves_state(
        &double_free,
        &runtime,
        &before_double_free,
        &memory,
        memory_before_double_free,
    );

    let reused = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(
            heap,
            1,
            RuntimeHeapInitialization::DeterministicUninitialized,
        )
        .expect("reuse freed one-byte slot");
    assert_eq!(reused.address, reached.address);
    assert!(reused.id.0 > reached.id.0);
    assert_ne!(reused.id, reached.id);
}

#[test]
fn heap_size_queries_process_owned_live_allocations_and_models_native_failures() {
    const FRAME_BASE: u32 = STACK_BASE;

    let (mut memory, owner) = initialized_memory_with_environment_capacity();
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x70);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize HeapSize IAT");
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
            name: "HeapSize".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented HeapSize endpoint");
    assert_eq!(contract.implementation_id, HEAP_SIZE_IMPLEMENTATION);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    assert_eq!(
        contract.abi.arguments[2].value,
        ApiValueSchema::OpaquePointer {
            nullable: true,
            alignment: 1,
        }
    );

    let frames = [
        (FRAME_BASE, vec![0x1000_0751, 0x6801_0002, 0, 0x2100_0000]),
        (
            FRAME_BASE + 0x20,
            vec![0x1000_0752, 0x6801_0002, 1, 0x2100_0000],
        ),
        (
            FRAME_BASE + 0x40,
            vec![0x1000_0753, 0x6801_0002, 0, 0x2100_0001],
        ),
        (FRAME_BASE + 0x60, vec![0x1000_0754, 0x6801_0002, 0, 0]),
        (
            FRAME_BASE + 0x80,
            vec![0x1000_0755, 0x6801_0002, 0, 0x2200_0000],
        ),
        (
            FRAME_BASE + 0xa0,
            vec![0x1000_0756, 0x6801_0002, 8, 0x2100_0000],
        ),
        (
            FRAME_BASE + 0xc0,
            vec![0x1000_0757, 0x6801_0004, 0, 0x2100_0000],
        ),
        (
            FRAME_BASE + 0xe0,
            vec![0x1000_0758, 0x6801_0002, 0, 0x2100_0000],
        ),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");

    let mut runtime = controlled_runtime(controlled_startup_info());
    runtime
        .allocate_private_heap(0, 0x1000, 0)
        .expect("reserved first logical heap slot");
    let heap = runtime
        .allocate_private_heap(0, 0x1000, 0)
        .expect("queried logical heap")
        .handle;
    let foreign_heap = runtime
        .allocate_private_heap(0, 0x1000, 0)
        .expect("foreign logical heap")
        .handle;
    assert_eq!(heap, 0x6801_0002);
    assert_eq!(foreign_heap, 0x6801_0003);
    let allocation = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(
            heap,
            129,
            RuntimeHeapInitialization::DeterministicUninitialized,
        )
        .expect("non-BGI-sized allocation");
    let allocation_delta = memory.pending_delta().expect("allocation delta");
    memory
        .acknowledge_delta(allocation_delta.generation)
        .expect("acknowledge allocation");
    let foreign = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(
            foreign_heap,
            17,
            RuntimeHeapInitialization::DeterministicUninitialized,
        )
        .expect("foreign allocation");
    let foreign_delta = memory.pending_delta().expect("foreign allocation delta");
    memory
        .acknowledge_delta(foreign_delta.generation)
        .expect("acknowledge foreign allocation");
    assert_eq!(allocation.address, GuestAddress(0x2100_0000));
    assert_eq!(allocation.byte_length, 129);
    assert_eq!(allocation.reserved_byte_length, 136);
    assert_eq!(foreign.address, GuestAddress(0x2200_0000));

    let mut policy = compatibility_policy(
        "heap-size-test",
        vec![RUNTIME_STANDARD_INPUT_SELECTOR],
        vec![(RUNTIME_STANDARD_INPUT_SELECTOR, 2)],
        vec![(0x8, vec![0x214])],
    );
    policy.heap_size_flags = vec![0, 1];
    policy.validate().expect("valid HeapSize policy");

    let before_disabled = runtime.clone();
    let memory_before_disabled = memory.stats();
    let disabled = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[0].0, frames[0].1[0]),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        disabled,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "heap-size-flags-unsupported"
    ));
    assert_eq!(runtime, before_disabled);
    assert_eq!(memory.stats(), memory_before_disabled);

    let memory_before_queries = memory.stats();
    let success = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[0].0,
        frames[0].1[0],
        &policy,
    );
    assert_eq!(success.eax, RegisterUpdate::Set(129));
    assert_eq!(success.stack_pointer, GuestAddress(frames[0].0 + 16));
    assert_eq!(success.last_error, INITIAL_LAST_ERROR);
    assert!(success.guest_writes.is_empty());

    runtime.set_current_last_error(87).unwrap();
    let no_serialize = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[1].0,
        frames[1].1[0],
        &policy,
    );
    assert_eq!(no_serialize.eax, RegisterUpdate::Set(129));
    assert_eq!(no_serialize.last_error, 87);

    for (frame_index, sentinel) in [(2, 122), (3, 5), (4, 0x1234_5678)] {
        runtime.set_current_last_error(sentinel).unwrap();
        let mut expected = runtime.clone();
        expected.set_current_last_error(87).unwrap();
        let failure = dispatch_owned_with_policy(
            &dispatcher,
            &mut memory,
            &mut runtime,
            &endpoint,
            frames[frame_index].0,
            frames[frame_index].1[0],
            &policy,
        );
        assert_eq!(failure.eax, RegisterUpdate::Set(u32::MAX));
        assert_eq!(failure.last_error, 87);
        assert!(failure.guest_writes.is_empty());
        assert_eq!(runtime, expected);
        assert_eq!(memory.stats(), memory_before_queries);
    }

    for frame_index in [5, 6] {
        let expected_runtime = runtime.clone();
        let expected_memory = memory.stats();
        let outcome = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            dispatcher.dispatch_owned(
                owned_trap(&endpoint, frames[frame_index].0, frames[frame_index].1[0]),
                &mut session,
                |contract, frame, session| {
                    dispatch_kernel32_api_with_policy(&policy, contract, frame, session)
                },
            )
        };
        assert!(matches!(outcome, DispatchOutcome::Fault(_)));
        assert_eq!(runtime, expected_runtime);
        assert_eq!(memory.stats(), expected_memory);
    }

    ExecutionSession::new(&mut memory, &mut runtime)
        .free_private_heap_block(heap, allocation.address)
        .expect("retire queried allocation");
    let free_delta = memory.pending_delta().expect("free delta");
    memory
        .acknowledge_delta(free_delta.generation)
        .expect("acknowledge free delta");
    runtime.set_current_last_error(5).unwrap();
    let mut expected = runtime.clone();
    expected.set_current_last_error(87).unwrap();
    let memory_after_free = memory.stats();
    let freed = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[7].0,
        frames[7].1[0],
        &policy,
    );
    assert_eq!(freed.eax, RegisterUpdate::Set(u32::MAX));
    assert_eq!(freed.last_error, 87);
    assert_eq!(runtime, expected);
    assert_eq!(memory.stats(), memory_after_free);
}

#[test]
fn get_std_handle_default_policy_owns_only_standard_input_identity() {
    const FRAME_BASE: u32 = STACK_BASE;
    const TEST_IAT_BASE: u32 = IAT_BASE;

    let (mut memory, owner) = initialized_memory();
    let frames = [
        (
            FRAME_BASE,
            vec![0x1000_0801, RUNTIME_STANDARD_INPUT_SELECTOR],
        ),
        (
            FRAME_BASE + 0x20,
            vec![0x1000_0802, RUNTIME_STANDARD_INPUT_SELECTOR],
        ),
        (FRAME_BASE + 0x40, vec![0x1000_0803, 0xffff_fff5]),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x80);
    memory
        .write_initialization(
            owner,
            GuestAddress(TEST_IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize GetStdHandle IAT");
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(1),
        address: endpoint_address,
        requester_owner: owner,
        module: "KERNEL32.dll".to_owned(),
        iat_rva: Rva(0),
        iat_address: GuestAddress(TEST_IAT_BASE),
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "GetStdHandle".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented GetStdHandle endpoint");
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let mut runtime = controlled_runtime(controlled_startup_info());
    let memory_before = memory.stats();

    for frame in &frames[..2] {
        let completion = dispatch_owned(
            &dispatcher,
            &mut memory,
            &mut runtime,
            &endpoint,
            frame.0,
            frame.1[0],
        );
        assert_eq!(
            completion.eax,
            RegisterUpdate::Set(RUNTIME_STANDARD_INPUT_HANDLE)
        );
        assert_eq!(completion.stack_pointer, GuestAddress(frame.0 + 8));
        assert_eq!(completion.last_error, INITIAL_LAST_ERROR);
        assert!(completion.guest_writes.is_empty());
    }
    assert_eq!(runtime.process().standard_handles().len(), 1);
    let standard_handle = runtime
        .standard_handle_by_value(RUNTIME_STANDARD_INPUT_HANDLE)
        .expect("logical standard input handle");
    assert_eq!(standard_handle.owner, "runtime-process-standard-handles");
    assert_eq!(standard_handle.variant_id, "standard-input-logical-v1");
    assert_eq!(standard_handle.namespace, LOGICAL_STANDARD_HANDLE_NAMESPACE);
    assert_eq!(standard_handle.selector, RUNTIME_STANDARD_INPUT_SELECTOR);
    assert_eq!(memory.stats(), memory_before);

    let runtime_before_rejection = runtime.clone();
    let rejected = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[2].0, frames[2].1[0]),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        rejected,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "standard-handle-selector-unsupported"
    ));
    assert_eq!(runtime, runtime_before_rejection);
    assert_eq!(memory.stats(), memory_before);
}

#[test]
fn get_std_handle_policy_adds_only_process_owned_standard_output_identity() {
    let (mut memory, owner) = initialized_memory();
    let frames = [
        (
            STACK_BASE,
            vec![0x1000_0881, RUNTIME_STANDARD_OUTPUT_SELECTOR],
        ),
        (
            STACK_BASE + 0x20,
            vec![0x1000_0882, RUNTIME_STANDARD_OUTPUT_SELECTOR],
        ),
        (STACK_BASE + 0x40, vec![0x1000_0883, 0xffff_fff4]),
        (
            STACK_BASE + 0x60,
            vec![0x1000_0884, RUNTIME_STANDARD_OUTPUT_HANDLE],
        ),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }

    let standard_handle_endpoint_address = GuestAddress(ENDPOINT_BASE + 0x80);
    let file_type_endpoint_address = GuestAddress(ENDPOINT_BASE + 0x90);
    for (iat_address, endpoint_address) in [
        (IAT_BASE, standard_handle_endpoint_address),
        (IAT_BASE + 4, file_type_endpoint_address),
    ] {
        memory
            .write_initialization(
                owner,
                GuestAddress(iat_address),
                &endpoint_address.0.to_le_bytes(),
            )
            .expect("initialize standard-handle endpoint IAT");
    }
    let standard_handle_endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(1),
        address: standard_handle_endpoint_address,
        requester_owner: owner,
        module: "KERNEL32.dll".to_owned(),
        iat_rva: Rva(0),
        iat_address: GuestAddress(IAT_BASE),
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "GetStdHandle".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let file_type_endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(2),
        address: file_type_endpoint_address,
        requester_owner: owner,
        module: "KERNEL32.dll".to_owned(),
        iat_rva: Rva(4),
        iat_address: GuestAddress(IAT_BASE + 4),
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "GetFileType".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let standard_handle_contract = kernel32_contract_for_endpoint(standard_handle_endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented GetStdHandle endpoint");
    let file_type_contract = kernel32_contract_for_endpoint(file_type_endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented GetFileType endpoint");
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new(
        [standard_handle_endpoint.clone(), file_type_endpoint.clone()],
        [standard_handle_contract, file_type_contract],
    )
    .expect("dispatcher");
    let output_policy = compatibility_policy(
        "standard-output-test",
        vec![
            RUNTIME_STANDARD_INPUT_SELECTOR,
            RUNTIME_STANDARD_OUTPUT_SELECTOR,
        ],
        vec![(RUNTIME_STANDARD_INPUT_SELECTOR, 2)],
        vec![(0x8, vec![0x214, 0x800])],
    );
    let mut runtime = controlled_runtime(controlled_startup_info());
    let memory_before = memory.stats();

    for frame in &frames[..2] {
        let completion = dispatch_owned_with_policy(
            &dispatcher,
            &mut memory,
            &mut runtime,
            &standard_handle_endpoint,
            frame.0,
            frame.1[0],
            &output_policy,
        );
        assert_eq!(
            completion.eax,
            RegisterUpdate::Set(RUNTIME_STANDARD_OUTPUT_HANDLE)
        );
        assert_eq!(completion.stack_pointer, GuestAddress(frame.0 + 8));
        assert_eq!(completion.last_error, INITIAL_LAST_ERROR);
        assert!(completion.guest_writes.is_empty());
    }
    assert_eq!(runtime.process().standard_handles().len(), 1);
    let standard_output = runtime
        .standard_handle_by_value(RUNTIME_STANDARD_OUTPUT_HANDLE)
        .expect("logical standard output handle");
    assert_eq!(standard_output.owner, "runtime-process-standard-handles");
    assert_eq!(standard_output.variant_id, "standard-output-logical-v1");
    assert_eq!(standard_output.namespace, LOGICAL_STANDARD_HANDLE_NAMESPACE);
    assert_eq!(standard_output.selector, RUNTIME_STANDARD_OUTPUT_SELECTOR);
    assert_eq!(memory.stats(), memory_before);

    let runtime_before_rejections = runtime.clone();
    let standard_error_rejected = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&standard_handle_endpoint, frames[2].0, frames[2].1[0]),
            &mut session,
            |contract, frame, session| {
                dispatch_kernel32_api_with_policy(&output_policy, contract, frame, session)
            },
        )
    };
    assert!(matches!(
        standard_error_rejected,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "standard-handle-selector-unsupported"
    ));
    let output_file_type_rejected = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&file_type_endpoint, frames[3].0, frames[3].1[0]),
            &mut session,
            |contract, frame, session| {
                dispatch_kernel32_api_with_policy(&output_policy, contract, frame, session)
            },
        )
    };
    assert!(matches!(
        output_file_type_rejected,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "file-type-standard-handle-unsupported"
    ));
    assert_eq!(runtime, runtime_before_rejections);
    assert_eq!(memory.stats(), memory_before);
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
}

#[test]
fn get_std_handle_policy_adds_only_process_owned_standard_error_identity() {
    let (mut memory, owner) = initialized_memory();
    let frames = [
        (
            STACK_BASE,
            vec![0x1000_08a1, RUNTIME_STANDARD_ERROR_SELECTOR],
        ),
        (
            STACK_BASE + 0x20,
            vec![0x1000_08a2, RUNTIME_STANDARD_ERROR_SELECTOR],
        ),
        (STACK_BASE + 0x40, vec![0x1000_08a3, 0xffff_fff3]),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }

    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0xa0);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize GetStdHandle IAT");
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
            name: "GetStdHandle".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented GetStdHandle endpoint");
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let error_policy = compatibility_policy(
        "standard-error-test",
        vec![
            RUNTIME_STANDARD_INPUT_SELECTOR,
            RUNTIME_STANDARD_OUTPUT_SELECTOR,
            RUNTIME_STANDARD_ERROR_SELECTOR,
        ],
        vec![
            (RUNTIME_STANDARD_INPUT_SELECTOR, 2),
            (RUNTIME_STANDARD_OUTPUT_SELECTOR, 3),
        ],
        vec![(0x8, vec![0x214, 0x800])],
    );
    let mut runtime = controlled_runtime(controlled_startup_info());
    let memory_before = memory.stats();

    for frame in &frames[..2] {
        let completion = dispatch_owned_with_policy(
            &dispatcher,
            &mut memory,
            &mut runtime,
            &endpoint,
            frame.0,
            frame.1[0],
            &error_policy,
        );
        assert_eq!(
            completion.eax,
            RegisterUpdate::Set(RUNTIME_STANDARD_ERROR_HANDLE)
        );
        assert_eq!(completion.stack_pointer, GuestAddress(frame.0 + 8));
        assert_eq!(completion.last_error, INITIAL_LAST_ERROR);
        assert!(completion.guest_writes.is_empty());
    }
    assert_eq!(runtime.process().standard_handles().len(), 1);
    let standard_error = runtime
        .standard_handle_by_value(RUNTIME_STANDARD_ERROR_HANDLE)
        .expect("logical standard error handle");
    assert_eq!(standard_error.owner, "runtime-process-standard-handles");
    assert_eq!(standard_error.variant_id, "standard-error-logical-v1");
    assert_eq!(standard_error.namespace, LOGICAL_STANDARD_HANDLE_NAMESPACE);
    assert_eq!(standard_error.selector, RUNTIME_STANDARD_ERROR_SELECTOR);
    assert_eq!(
        error_policy.standard_handle_file_type(RUNTIME_STANDARD_ERROR_SELECTOR),
        None
    );
    assert_eq!(memory.stats(), memory_before);

    let runtime_before_rejection = runtime.clone();
    let rejected = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[2].0, frames[2].1[0]),
            &mut session,
            |contract, frame, session| {
                dispatch_kernel32_api_with_policy(&error_policy, contract, frame, session)
            },
        )
    };
    assert!(matches!(
        rejected,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "standard-handle-selector-unsupported"
    ));
    assert_eq!(runtime, runtime_before_rejection);
    assert_eq!(memory.stats(), memory_before);
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
}

#[test]
fn get_file_type_contract_classifies_only_process_owned_standard_input() {
    let (mut memory, owner) = initialized_memory();
    let frames = [
        (STACK_BASE, vec![0x1000_0901, RUNTIME_STANDARD_INPUT_HANDLE]),
        (STACK_BASE + 0x20, vec![0x1000_0902, 0x5301_0002]),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x90);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize GetFileType IAT");
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
            name: "GetFileType".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented GetFileType endpoint");
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let mut runtime = controlled_runtime(controlled_startup_info());
    runtime
        .ensure_standard_handle(RUNTIME_STANDARD_INPUT_SELECTOR)
        .expect("register logical standard input");
    let runtime_before = runtime.clone();
    let memory_before = memory.stats();

    let completion = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[0].0,
        frames[0].1[0],
    );
    assert_eq!(completion.eax, RegisterUpdate::Set(2));
    assert_eq!(completion.stack_pointer, GuestAddress(frames[0].0 + 8));
    assert_eq!(completion.last_error, INITIAL_LAST_ERROR);
    assert!(completion.guest_writes.is_empty());
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.stats(), memory_before);

    let rejected = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[1].0, frames[1].1[0]),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        rejected,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::InvalidCall
                && fault.code == "standard-handle-unknown"
    ));
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.stats(), memory_before);
}

#[test]
fn get_file_type_policy_classifies_process_owned_standard_output_as_pipe() {
    let (mut memory, owner) = initialized_memory();
    let frames = [
        (
            STACK_BASE,
            vec![0x1000_0921, RUNTIME_STANDARD_OUTPUT_HANDLE],
        ),
        (
            STACK_BASE + 0x20,
            vec![0x1000_0922, RUNTIME_STANDARD_OUTPUT_HANDLE],
        ),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x90);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize GetFileType IAT");
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
            name: "GetFileType".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented GetFileType endpoint");
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let output_file_type_policy = compatibility_policy(
        "standard-output-file-type-test",
        vec![
            RUNTIME_STANDARD_INPUT_SELECTOR,
            RUNTIME_STANDARD_OUTPUT_SELECTOR,
        ],
        vec![
            (RUNTIME_STANDARD_INPUT_SELECTOR, 2),
            (RUNTIME_STANDARD_OUTPUT_SELECTOR, 3),
        ],
        vec![(0x8, vec![0x214, 0x800])],
    );
    let mut runtime = controlled_runtime(controlled_startup_info());
    runtime
        .ensure_standard_handle(RUNTIME_STANDARD_OUTPUT_SELECTOR)
        .expect("register logical standard output");
    let runtime_before = runtime.clone();
    let memory_before = memory.stats();

    let completion = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[0].0,
        frames[0].1[0],
        &output_file_type_policy,
    );
    assert_eq!(completion.eax, RegisterUpdate::Set(3));
    assert_eq!(completion.stack_pointer, GuestAddress(frames[0].0 + 8));
    assert_eq!(completion.last_error, INITIAL_LAST_ERROR);
    assert!(completion.guest_writes.is_empty());
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.stats(), memory_before);

    let default_policy_rejected = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[1].0, frames[1].1[0]),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        default_policy_rejected,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "file-type-standard-handle-unsupported"
    ));
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.stats(), memory_before);
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
}

#[test]
fn get_file_type_policy_classifies_process_owned_standard_error_as_pipe() {
    let (mut memory, owner) = initialized_memory();
    let frames = [
        (STACK_BASE, vec![0x1000_0941, RUNTIME_STANDARD_ERROR_HANDLE]),
        (
            STACK_BASE + 0x20,
            vec![0x1000_0942, RUNTIME_STANDARD_ERROR_HANDLE],
        ),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0xb0);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize GetFileType IAT");
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
            name: "GetFileType".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented GetFileType endpoint");
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let error_file_type_policy = compatibility_policy(
        "standard-error-file-type-test",
        vec![
            RUNTIME_STANDARD_INPUT_SELECTOR,
            RUNTIME_STANDARD_OUTPUT_SELECTOR,
            RUNTIME_STANDARD_ERROR_SELECTOR,
        ],
        vec![
            (RUNTIME_STANDARD_INPUT_SELECTOR, 2),
            (RUNTIME_STANDARD_OUTPUT_SELECTOR, 3),
            (RUNTIME_STANDARD_ERROR_SELECTOR, 3),
        ],
        vec![(0x8, vec![0x214, 0x800])],
    );
    let mut runtime = controlled_runtime(controlled_startup_info());
    runtime
        .ensure_standard_handle(RUNTIME_STANDARD_ERROR_SELECTOR)
        .expect("register logical standard error");
    let runtime_before = runtime.clone();
    let memory_before = memory.stats();

    let completion = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[0].0,
        frames[0].1[0],
        &error_file_type_policy,
    );
    assert_eq!(completion.eax, RegisterUpdate::Set(3));
    assert_eq!(completion.stack_pointer, GuestAddress(frames[0].0 + 8));
    assert_eq!(completion.last_error, INITIAL_LAST_ERROR);
    assert!(completion.guest_writes.is_empty());
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.stats(), memory_before);

    let default_policy_rejected = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[1].0, frames[1].1[0]),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        default_policy_rejected,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "file-type-standard-handle-unsupported"
    ));
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.stats(), memory_before);
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
}

#[test]
fn set_handle_count_policy_returns_reached_count_without_mutation() {
    let (mut memory, owner) = initialized_memory();
    let frames = [
        (STACK_BASE, vec![0x1000_0951, 32]),
        (STACK_BASE + 0x20, vec![0x1000_0952, 32]),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0xc0);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize SetHandleCount IAT");
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
            name: "SetHandleCount".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented SetHandleCount endpoint");
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let mut enabled_policy = compatibility_policy(
        "set-handle-count-test",
        vec![RUNTIME_STANDARD_INPUT_SELECTOR],
        vec![(RUNTIME_STANDARD_INPUT_SELECTOR, 2)],
        vec![(0x8, vec![0x214])],
    );
    enabled_policy.set_handle_counts = vec![32];
    enabled_policy
        .validate()
        .expect("valid SetHandleCount policy");
    let mut runtime = controlled_runtime(controlled_startup_info());
    let runtime_before = runtime.clone();
    let memory_before = memory.stats();

    let completion = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[0].0,
        frames[0].1[0],
        &enabled_policy,
    );
    assert_eq!(completion.eax, RegisterUpdate::Set(32));
    assert_eq!(completion.stack_pointer, GuestAddress(frames[0].0 + 8));
    assert_eq!(completion.last_error, INITIAL_LAST_ERROR);
    assert!(completion.guest_writes.is_empty());
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.stats(), memory_before);

    let default_policy_rejected = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[1].0, frames[1].1[0]),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        default_policy_rejected,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "set-handle-count-unsupported"
    ));
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.stats(), memory_before);
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
}

#[test]
fn get_command_line_a_returns_stable_process_owned_ascii_view_without_mutation() {
    let (mut memory, owner) = initialized_memory_with_environment_capacity();
    let frames = [
        (STACK_BASE, vec![0x1000_0961]),
        (STACK_BASE + 0x20, vec![0x1000_0962]),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0xd0);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize GetCommandLineA IAT");
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
            name: "GetCommandLineA".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented GetCommandLineA endpoint");
    let mut runtime = controlled_runtime(controlled_startup_info());
    let environment = ExecutionSession::new(&mut memory, &mut runtime)
        .install_controlled_process_environment_with_command_line_and_ansi("fixture.exe")
        .expect("process environment");
    let parameters = environment
        .process_parameters
        .as_ref()
        .expect("process parameters");
    let ansi = parameters.ansi.as_ref().expect("ASCII command-line view");
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let registered = runtime
        .process_command_line()
        .expect("registered process command line")
        .clone();
    assert_eq!(registered.ansi_buffer_address, ansi.buffer_address);
    assert_ne!(
        registered.ansi_buffer_address,
        registered.wide_buffer_address
    );
    assert_eq!(
        runtime
            .process_address_space_allocator()
            .allocation(environment.address_allocation_id)
            .expect("process environment allocation")
            .purpose,
        vine_core::ProcessAddressAllocationPurpose::ProcessEnvironment
    );
    let runtime_before = runtime.clone();
    let memory_before = memory.stats();

    for (stack_pointer, words) in &frames {
        let completion = dispatch_owned(
            &dispatcher,
            &mut memory,
            &mut runtime,
            &endpoint,
            *stack_pointer,
            words[0],
        );
        assert_eq!(completion.eax, RegisterUpdate::Set(ansi.buffer_address.0));
        assert_eq!(completion.stack_pointer, GuestAddress(*stack_pointer + 4));
        assert_eq!(completion.last_error, INITIAL_LAST_ERROR);
        assert!(completion.guest_writes.is_empty());
    }
    assert_eq!(
        memory
            .read(ansi.buffer_address, ansi.byte_length_with_nul as usize)
            .expect("read ANSI command line"),
        b"fixture.exe\0"
    );
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.stats(), memory_before);
}

#[test]
fn get_module_file_name_a_uses_process_path_and_bounds_every_output_before_mutation() {
    const MODULE_HANDLE: u32 = 0x4d01_0001;
    const PATH: &[u8] = b"C:\\Vine\\fixture.exe\0";
    const PATH_LENGTH: u32 = (PATH.len() - 1) as u32;
    const CROSS_UNMAPPED_OUTPUT: u32 = MEMORY_BASE.0 + 0x2ff0;

    let (mut memory, owner) = initialized_memory();
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0xd8);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize GetModuleFileNameA IAT");
    memory
        .write_initialization(owner, GuestAddress(OUTPUT_BASE), &vec![0xa5; 0x400])
        .expect("initialize module file-name outputs");
    let frames = [
        (STACK_BASE, vec![0x1000_0963, 0, OUTPUT_BASE, 64]),
        (
            STACK_BASE + 0x20,
            vec![0x1000_0964, 0, OUTPUT_BASE + 0x100, PATH.len() as u32],
        ),
        (
            STACK_BASE + 0x40,
            vec![0x1000_0965, 0, OUTPUT_BASE + 0x200, PATH_LENGTH],
        ),
        (
            STACK_BASE + 0x60,
            vec![0x1000_0966, 0, OUTPUT_BASE + 0x300, 1],
        ),
        (
            STACK_BASE + 0x80,
            vec![0x1000_0967, 0, OUTPUT_BASE + 0x310, 0],
        ),
        (STACK_BASE + 0xa0, vec![0x1000_0968, 0, 0, 0]),
        (STACK_BASE + 0xc0, vec![0x1000_0969, 0, 0, 1]),
        (
            STACK_BASE + 0xe0,
            vec![0x1000_096a, 0, OUTPUT_BASE + 0x320, 32_769],
        ),
        (
            STACK_BASE + 0x100,
            vec![0x1000_096b, MODULE_HANDLE, OUTPUT_BASE + 0x320, 64],
        ),
        (
            STACK_BASE + 0x120,
            vec![0x1000_096c, 0, CROSS_UNMAPPED_OUTPUT, 64],
        ),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
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
            name: "GetModuleFileNameA".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented GetModuleFileNameA endpoint");
    assert_eq!(
        contract.implementation_id,
        "kernel32.get-module-file-name-a.v1"
    );
    assert_eq!(contract.abi.arguments.len(), 3);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let mut runtime = controlled_runtime(controlled_startup_info());
    runtime
        .load_module(RuntimeModule {
            owner: "test-runtime".to_owned(),
            variant_id: "runtime-kernel32".to_owned(),
            namespace: LOGICAL_LOADED_MODULE_NAMESPACE.to_owned(),
            handle: MODULE_HANDLE,
            requested_name: "KERNEL32.DLL".to_owned(),
            canonical_name: "kernel32.dll".to_owned(),
            runtime_system_module: true,
        })
        .expect("load logical module");
    let policy: Kernel32CompatibilityPolicy = serde_json::from_str(include_str!(
        "../../../fixtures/runtime-policies/kernel32-get-module-file-name-a-v1.json"
    ))
    .expect("module file-name policy");

    let success = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[0].0,
        frames[0].1[0],
        &policy,
    );
    assert_eq!(success.eax, RegisterUpdate::Set(PATH_LENGTH));
    assert_eq!(success.stack_pointer, GuestAddress(frames[0].0 + 16));
    assert_eq!(success.last_error, 0);
    assert_eq!(success.guest_writes.len(), 1);
    assert_eq!(success.guest_writes[0].write.bytes, PATH);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(OUTPUT_BASE), PATH.len() + 1)
            .unwrap(),
        [PATH, &[0xa5]].concat()
    );

    let exact = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[1].0,
        frames[1].1[0],
        &policy,
    );
    assert_eq!(exact.eax, RegisterUpdate::Set(PATH_LENGTH));
    assert_eq!(exact.last_error, 0);
    assert_eq!(exact.guest_writes[0].write.bytes, PATH);
    acknowledge_pending_delta(&mut memory);

    let insufficient = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[2].0,
        frames[2].1[0],
        &policy,
    );
    assert_eq!(insufficient.eax, RegisterUpdate::Set(PATH_LENGTH));
    assert_eq!(insufficient.last_error, 122);
    let expected_truncated = [&PATH[..PATH.len() - 2], &[0]].concat();
    assert_eq!(insufficient.guest_writes[0].write.bytes, expected_truncated);
    acknowledge_pending_delta(&mut memory);

    let one_byte = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[3].0,
        frames[3].1[0],
        &policy,
    );
    assert_eq!(one_byte.eax, RegisterUpdate::Set(1));
    assert_eq!(one_byte.last_error, 122);
    assert_eq!(one_byte.guest_writes[0].write.bytes, [0]);
    acknowledge_pending_delta(&mut memory);

    for frame in [&frames[4], &frames[5]] {
        let zero_capacity = dispatch_owned_with_policy(
            &dispatcher,
            &mut memory,
            &mut runtime,
            &endpoint,
            frame.0,
            frame.1[0],
            &policy,
        );
        assert_eq!(zero_capacity.eax, RegisterUpdate::Set(0));
        assert_eq!(zero_capacity.last_error, 122);
        assert!(zero_capacity.guest_writes.is_empty());
    }
    let null_output = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[6].0,
        frames[6].1[0],
        &policy,
    );
    assert_eq!(null_output.eax, RegisterUpdate::Set(0));
    assert_eq!(null_output.last_error, 87);
    assert!(null_output.guest_writes.is_empty());

    for (frame, expected_class, expected_code) in [
        (
            &frames[7],
            ApiFaultClass::ResourceLimit,
            "get-module-file-name-a-capacity-limit-exceeded",
        ),
        (
            &frames[8],
            ApiFaultClass::Unsupported,
            "get-module-file-name-a-explicit-module-unsupported",
        ),
        (&frames[9], ApiFaultClass::InvalidCall, "guest-page-fault"),
    ] {
        let runtime_before = runtime.clone();
        let memory_before = memory.stats();
        let rejected = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            dispatcher.dispatch_owned(
                owned_trap(&endpoint, frame.0, frame.1[0]),
                &mut session,
                |contract, frame, session| {
                    dispatch_kernel32_api_with_policy(&policy, contract, frame, session)
                },
            )
        };
        assert!(matches!(
            rejected,
            DispatchOutcome::Fault(ref fault)
                if fault.class == expected_class && fault.code == expected_code
        ));
        assert_eq!(runtime, runtime_before);
        assert_eq!(memory.stats(), memory_before);
    }

    let runtime_before = runtime.clone();
    let memory_before = memory.stats();
    let default_policy_rejected = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[0].0, frames[0].1[0]),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        default_policy_rejected,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "get-module-file-name-a-policy-missing"
    ));
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.stats(), memory_before);
}

#[test]
fn get_and_free_environment_strings_w_manage_one_process_owned_copy() {
    let (mut memory, owner) = initialized_memory_with_environment_capacity();
    let mut runtime = controlled_runtime(controlled_startup_info());
    let environment = ExecutionSession::new(&mut memory, &mut runtime)
        .install_controlled_process_environment_with_command_line_ansi_and_environment_strings(
            "fixture.exe",
        )
        .expect("process environment");
    let parameters = environment
        .process_parameters
        .as_ref()
        .expect("process parameters");
    let environment_strings = parameters
        .environment_strings
        .as_ref()
        .expect("wide environment strings")
        .clone();
    assert_ne!(
        environment_strings.authority_buffer_address,
        environment_strings.copy_buffer_address
    );
    assert_eq!(environment_strings.byte_length_with_double_nul, 4);

    let get_frames = [
        (STACK_BASE, vec![0x1000_0971]),
        (STACK_BASE + 0x20, vec![0x1000_0972]),
    ];
    let free_frame = (
        STACK_BASE + 0x40,
        vec![0x1000_0973, environment_strings.copy_buffer_address.0],
    );
    let foreign_free_frame = (
        STACK_BASE + 0x60,
        vec![0x1000_0974, environment_strings.authority_buffer_address.0],
    );
    for (address, words) in get_frames.iter().chain([&free_frame, &foreign_free_frame]) {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }

    let mut endpoint = |id, offset, iat_offset, name: &str| {
        let endpoint_address = GuestAddress(ENDPOINT_BASE + offset);
        memory
            .write_initialization(
                owner,
                GuestAddress(IAT_BASE + iat_offset),
                &endpoint_address.0.to_le_bytes(),
            )
            .expect("initialize environment-string IAT");
        ApiEndpointIdentity::from_import_thunk(&ImportThunk {
            id: ImportThunkId(id),
            address: endpoint_address,
            requester_owner: owner,
            module: "KERNEL32.dll".to_owned(),
            iat_rva: Rva(iat_offset),
            iat_address: GuestAddress(IAT_BASE + iat_offset),
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: name.to_owned(),
                hint: 0,
                name_rva: Rva(0),
            },
        })
    };
    let get_endpoint = endpoint(1, 0xe0, 0, "GetEnvironmentStringsW");
    let free_endpoint = endpoint(2, 0xf0, 4, "FreeEnvironmentStringsW");
    let contracts = [&get_endpoint, &free_endpoint]
        .into_iter()
        .map(|endpoint| {
            kernel32_contract_for_endpoint(endpoint.clone())
                .expect("Kernel32 contract lookup")
                .expect("implemented environment-string endpoint")
        })
        .collect::<Vec<_>>();
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    assert_eq!(
        u32::from_le_bytes(
            memory
                .read(
                    GuestAddress(
                        parameters.process_parameters_address.0
                            + PROCESS_PARAMETERS_ENVIRONMENT_POINTER_OFFSET,
                    ),
                    4,
                )
                .expect("RTL_USER_PROCESS_PARAMETERS.Environment")
                .try_into()
                .expect("one x86 pointer"),
        ),
        environment_strings.authority_buffer_address.0
    );
    let dispatcher = ApiDispatcher::new([get_endpoint.clone(), free_endpoint.clone()], contracts)
        .expect("dispatcher");
    let memory_before = memory.stats();

    let acquired = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &get_endpoint,
        get_frames[0].0,
        get_frames[0].1[0],
    );
    assert_eq!(
        acquired.eax,
        RegisterUpdate::Set(environment_strings.copy_buffer_address.0)
    );
    assert_eq!(acquired.stack_pointer, GuestAddress(get_frames[0].0 + 4));
    assert_eq!(acquired.last_error, INITIAL_LAST_ERROR);
    assert!(acquired.guest_writes.is_empty());
    assert_eq!(
        memory
            .read(environment_strings.authority_buffer_address, 4)
            .expect("environment authority"),
        [0, 0, 0, 0]
    );
    assert_eq!(
        memory
            .read(environment_strings.copy_buffer_address, 4)
            .expect("environment copy"),
        [0, 0, 0, 0]
    );
    let active = runtime
        .process_environment_strings()
        .expect("registered environment strings")
        .active_copy
        .as_ref()
        .expect("active environment copy")
        .clone();
    assert_eq!(active.acquisition_id, 1);
    assert_eq!(
        active.buffer_address,
        environment_strings.copy_buffer_address
    );
    assert_eq!(memory.stats(), memory_before);

    let runtime_before_limit = runtime.clone();
    let concurrent = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&get_endpoint, get_frames[1].0, get_frames[1].1[0]),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        concurrent,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::ResourceLimit
                && fault.code == "wide-environment-copy-limit-reached"
    ));
    assert_eq!(runtime, runtime_before_limit);
    assert_eq!(memory.stats(), memory_before);

    let runtime_before_foreign_free = runtime.clone();
    let foreign_free = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(
                &free_endpoint,
                foreign_free_frame.0,
                foreign_free_frame.1[0],
            ),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        foreign_free,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::InvalidCall
                && fault.code == "wide-environment-copy-pointer-invalid"
    ));
    assert_eq!(runtime, runtime_before_foreign_free);
    assert_eq!(memory.stats(), memory_before);

    let freed = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &free_endpoint,
        free_frame.0,
        free_frame.1[0],
    );
    assert_eq!(freed.eax, RegisterUpdate::Set(1));
    assert_eq!(freed.stack_pointer, GuestAddress(free_frame.0 + 8));
    assert_eq!(freed.last_error, INITIAL_LAST_ERROR);
    assert!(freed.guest_writes.is_empty());
    assert!(
        runtime
            .process_environment_strings()
            .expect("registered environment strings")
            .active_copy
            .is_none()
    );
    assert_eq!(memory.stats(), memory_before);

    let runtime_before_double_free = runtime.clone();
    let double_free = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&free_endpoint, free_frame.0, free_frame.1[0]),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        double_free,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::InvalidCall
                && fault.code == "wide-environment-copy-not-active"
    ));
    assert_eq!(runtime, runtime_before_double_free);
    assert_eq!(memory.stats(), memory_before);
}

#[test]
fn wide_char_to_multi_byte_models_windows_932_query_conversion_and_failures() {
    const EMPTY_SOURCE: u32 = OUTPUT_BASE;
    const JAPANESE_SOURCE: u32 = OUTPUT_BASE + 0x10;
    const UNMAPPABLE_SOURCE: u32 = OUTPUT_BASE + 0x20;
    const EMPTY_DESTINATION: u32 = OUTPUT_BASE + 0x100;
    const JAPANESE_DESTINATION: u32 = OUTPUT_BASE + 0x110;
    const UNMAPPABLE_DESTINATION: u32 = OUTPUT_BASE + 0x120;
    const SHORT_DESTINATION: u32 = OUTPUT_BASE + 0x130;
    const USED_DEFAULT: u32 = OUTPUT_BASE + 0x140;

    let (mut memory, owner) = initialized_memory();
    memory
        .write_initialization(owner, GuestAddress(EMPTY_SOURCE), &[0, 0, 0, 0])
        .expect("empty UTF-16 source");
    memory
        .write_initialization(
            owner,
            GuestAddress(JAPANESE_SOURCE),
            &[0x41, 0, 0x42, 0x30, 0, 0],
        )
        .expect("Windows-932 UTF-16 source");
    memory
        .write_initialization(
            owner,
            GuestAddress(UNMAPPABLE_SOURCE),
            &[0x3d, 0xd8, 0, 0xde, 0, 0],
        )
        .expect("unmappable UTF-16 source");
    for (address, bytes) in [
        (EMPTY_DESTINATION, vec![0xa5]),
        (JAPANESE_DESTINATION, vec![0xa5; 4]),
        (UNMAPPABLE_DESTINATION, vec![0xa5; 8]),
        (SHORT_DESTINATION, vec![0xa5]),
        (USED_DEFAULT, vec![0xa5; 4]),
    ] {
        memory
            .write_initialization(owner, GuestAddress(address), &bytes)
            .expect("conversion output initialization");
    }

    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x100);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize WideCharToMultiByte IAT");
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
            name: "WideCharToMultiByte".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("WideCharToMultiByte contract");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let frames = [
        vec![0x1000_0a01, 0, 0, EMPTY_SOURCE, 1, 0, 0, 0, 0],
        vec![
            0x1000_0a02,
            0,
            0,
            EMPTY_SOURCE,
            1,
            EMPTY_DESTINATION,
            1,
            0,
            0,
        ],
        vec![0x1000_0a03, 0, 0, JAPANESE_SOURCE, 3, 0, 0, 0, USED_DEFAULT],
        vec![
            0x1000_0a04,
            0,
            0,
            JAPANESE_SOURCE,
            3,
            JAPANESE_DESTINATION,
            4,
            0,
            USED_DEFAULT,
        ],
        vec![
            0x1000_0a05,
            932,
            0,
            UNMAPPABLE_SOURCE,
            3,
            UNMAPPABLE_DESTINATION,
            8,
            0,
            USED_DEFAULT,
        ],
        vec![
            0x1000_0a06,
            0,
            0,
            JAPANESE_SOURCE,
            3,
            SHORT_DESTINATION,
            1,
            0,
            USED_DEFAULT,
        ],
        vec![0x1000_0a07, 0, 0, EMPTY_SOURCE, 0, 0, 0, 0, 0],
        vec![
            0x1000_0a08,
            0,
            0,
            JAPANESE_SOURCE,
            3,
            JAPANESE_SOURCE,
            4,
            0,
            0,
        ],
        vec![0x1000_0a09, 65001, 0, EMPTY_SOURCE, 1, 0, 0, 0, 0],
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

    let mut policy = Kernel32CompatibilityPolicy::controlled_v1();
    policy.id = "public-windows-932-v1".to_owned();
    policy.ansi_command_line = true;
    policy.wide_environment_strings = true;
    policy.wide_char_to_multi_byte = Some(Kernel32WideCharToMultiBytePolicy {
        active_code_page: 932,
        maximum_source_code_units: 65_536,
    });
    policy.validate().expect("valid conversion policy");
    let mut runtime = controlled_runtime(controlled_startup_info());

    let dispatch = |index: usize,
                    memory: &mut GuestMemory,
                    runtime: &mut RuntimeState|
     -> vine_core::ApiImmediateCompletion {
        dispatch_owned_with_policy(
            &dispatcher,
            memory,
            runtime,
            &endpoint,
            STACK_BASE + index as u32 * 0x40,
            frames[index][0],
            &policy,
        )
    };

    let empty_query = dispatch(0, &mut memory, &mut runtime);
    assert_eq!(empty_query.eax, RegisterUpdate::Set(1));
    assert_eq!(empty_query.last_error, INITIAL_LAST_ERROR);
    assert!(empty_query.guest_writes.is_empty());

    let empty_conversion = dispatch(1, &mut memory, &mut runtime);
    assert_eq!(empty_conversion.eax, RegisterUpdate::Set(1));
    assert_eq!(empty_conversion.last_error, INITIAL_LAST_ERROR);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(EMPTY_DESTINATION), 1)
            .expect("empty conversion"),
        [0]
    );

    let japanese_query = dispatch(2, &mut memory, &mut runtime);
    assert_eq!(japanese_query.eax, RegisterUpdate::Set(4));
    assert_eq!(japanese_query.last_error, INITIAL_LAST_ERROR);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(USED_DEFAULT), 4)
            .expect("query used-default output"),
        0_u32.to_le_bytes()
    );

    let japanese_conversion = dispatch(3, &mut memory, &mut runtime);
    assert_eq!(japanese_conversion.eax, RegisterUpdate::Set(4));
    assert_eq!(japanese_conversion.last_error, INITIAL_LAST_ERROR);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(JAPANESE_DESTINATION), 4)
            .expect("Japanese conversion"),
        [0x41, 0x82, 0xa0, 0]
    );

    let unmappable_conversion = dispatch(4, &mut memory, &mut runtime);
    assert_eq!(unmappable_conversion.eax, RegisterUpdate::Set(3));
    assert_eq!(unmappable_conversion.last_error, INITIAL_LAST_ERROR);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(UNMAPPABLE_DESTINATION), 3)
            .expect("default-character conversion"),
        [b'?', b'?', 0]
    );
    assert_eq!(
        memory
            .read(GuestAddress(USED_DEFAULT), 4)
            .expect("used-default output"),
        1_u32.to_le_bytes()
    );

    let insufficient = dispatch(5, &mut memory, &mut runtime);
    assert_eq!(insufficient.eax, RegisterUpdate::Set(0));
    assert_eq!(insufficient.last_error, 122);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(SHORT_DESTINATION), 1)
            .expect("partial conversion"),
        [0x41]
    );
    assert_eq!(
        memory
            .read(GuestAddress(USED_DEFAULT), 4)
            .expect("partial used-default output"),
        0_u32.to_le_bytes()
    );

    let zero_count = dispatch(6, &mut memory, &mut runtime);
    assert_eq!(zero_count.eax, RegisterUpdate::Set(0));
    assert_eq!(zero_count.last_error, 87);
    let source_before_overlap = memory
        .read(GuestAddress(JAPANESE_SOURCE), 6)
        .expect("source before overlap")
        .to_vec();
    let overlap = dispatch(7, &mut memory, &mut runtime);
    assert_eq!(overlap.eax, RegisterUpdate::Set(0));
    assert_eq!(overlap.last_error, 87);
    assert_eq!(
        memory
            .read(GuestAddress(JAPANESE_SOURCE), 6)
            .expect("source after overlap"),
        source_before_overlap
    );

    let runtime_before_unsupported = runtime.clone();
    let memory_before_unsupported = memory.stats();
    let unsupported = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, STACK_BASE + 8 * 0x40, frames[8][0]),
            &mut session,
            |contract, frame, session| {
                dispatch_kernel32_api_with_policy(&policy, contract, frame, session)
            },
        )
    };
    assert!(matches!(
        unsupported,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "wide-char-to-multi-byte-variant-unsupported"
    ));
    assert_eq!(runtime, runtime_before_unsupported);
    assert_eq!(memory.stats(), memory_before_unsupported);
}

#[test]
fn multi_byte_to_wide_char_models_windows_932_and_cp_acp_query_conversion() {
    const TARGET_SOURCE: u32 = OUTPUT_BASE + 0x200;
    const JAPANESE_SOURCE: u32 = OUTPUT_BASE + 0x300;
    const TARGET_DESTINATION: u32 = OUTPUT_BASE + 0x400;
    const JAPANESE_DESTINATION: u32 = OUTPUT_BASE + 0x600;
    const SHORT_DESTINATION: u32 = OUTPUT_BASE + 0x610;
    const ACTIVE_CODE_PAGE_DESTINATION: u32 = OUTPUT_BASE + 0x620;

    let mut target_source = (0_u16..=255).map(|byte| byte as u8).collect::<Vec<_>>();
    for byte in [0_u8]
        .into_iter()
        .chain(0x81_u8..=0x9f)
        .chain(0xe0_u8..=0xfc)
    {
        target_source[byte as usize] = b' ';
    }
    let mut target_units = (0_u16..=255).collect::<Vec<_>>();
    target_units[0] = u16::from(b' ');
    target_units[0x81..=0x9f].fill(u16::from(b' '));
    target_units[0xa0] = 0xf8f0;
    for (offset, unit) in target_units[0xa1..=0xdf].iter_mut().enumerate() {
        *unit = 0xff61 + offset as u16;
    }
    target_units[0xe0..=0xfc].fill(u16::from(b' '));
    target_units[0xfd] = 0xf8f1;
    target_units[0xfe] = 0xf8f2;
    target_units[0xff] = 0xf8f3;
    let target_output = target_units
        .iter()
        .flat_map(|unit| unit.to_le_bytes())
        .collect::<Vec<_>>();

    let (mut memory, owner) = initialized_memory();
    memory
        .write_initialization(owner, GuestAddress(TARGET_SOURCE), &target_source)
        .expect("BGI code-page table source");
    memory
        .write_initialization(owner, GuestAddress(JAPANESE_SOURCE), &[0x41, 0x82, 0xa0, 0])
        .expect("Windows-932 multibyte source");
    for (address, bytes) in [
        (TARGET_DESTINATION, vec![0xa5; 512]),
        (JAPANESE_DESTINATION, vec![0xa5; 6]),
        (SHORT_DESTINATION, vec![0xa5; 2]),
        (ACTIVE_CODE_PAGE_DESTINATION, vec![0xa5; 6]),
    ] {
        memory
            .write_initialization(owner, GuestAddress(address), &bytes)
            .expect("UTF-16 destination initialization");
    }

    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x104);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize MultiByteToWideChar IAT");
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
            name: "MultiByteToWideChar".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("MultiByteToWideChar contract");
    assert_eq!(
        contract.implementation_id,
        MULTI_BYTE_TO_WIDE_CHAR_IMPLEMENTATION
    );
    assert_eq!(contract.abi.arguments.len(), 6);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let frames = [
        vec![0x1000_0b01, 932, 1, TARGET_SOURCE, 256, 0, 0],
        vec![
            0x1000_0b02,
            932,
            1,
            TARGET_SOURCE,
            256,
            TARGET_DESTINATION,
            256,
        ],
        vec![0x1000_0b03, 932, 1, JAPANESE_SOURCE, 4, 0, 0],
        vec![
            0x1000_0b04,
            932,
            1,
            JAPANESE_SOURCE,
            4,
            JAPANESE_DESTINATION,
            3,
        ],
        vec![
            0x1000_0b05,
            932,
            1,
            JAPANESE_SOURCE,
            4,
            SHORT_DESTINATION,
            1,
        ],
        vec![0x1000_0b06, 932, 1, JAPANESE_SOURCE, 0, 0, 0],
        vec![0x1000_0b07, 0, 0, JAPANESE_SOURCE, u32::MAX, 0, 0],
        vec![
            0x1000_0b08,
            0,
            0,
            JAPANESE_SOURCE,
            u32::MAX,
            ACTIVE_CODE_PAGE_DESTINATION,
            3,
        ],
        vec![0x1000_0b09, 65_001, 0, JAPANESE_SOURCE, 4, 0, 0],
        vec![0x1000_0b0a, 932, 8, JAPANESE_SOURCE, 4, 0, 0],
        vec![
            0x1000_0b0b,
            932,
            1,
            JAPANESE_SOURCE,
            4,
            JAPANESE_DESTINATION,
            0,
        ],
        vec![
            0x1000_0b0c,
            932,
            1,
            JAPANESE_SOURCE,
            4,
            JAPANESE_SOURCE + 2,
            3,
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

    let mut policy = Kernel32CompatibilityPolicy::controlled_v1();
    policy.id = "public-multi-byte-windows-932-v1".to_owned();
    policy.ansi_command_line = true;
    policy.wide_environment_strings = true;
    policy.wide_char_to_multi_byte = Some(Kernel32WideCharToMultiBytePolicy {
        active_code_page: 932,
        maximum_source_code_units: 65_536,
    });
    policy.multi_byte_to_wide_char = Some(Kernel32MultiByteToWideCharPolicy {
        allow_active_code_page_selector: true,
        allowed_flags: vec![0, 1],
        maximum_source_bytes: 65_536,
    });
    policy
        .validate()
        .expect("valid multibyte conversion policy");
    let mut runtime = controlled_runtime(controlled_startup_info());
    let dispatch = |index: usize,
                    memory: &mut GuestMemory,
                    runtime: &mut RuntimeState|
     -> vine_core::ApiImmediateCompletion {
        dispatch_owned_with_policy(
            &dispatcher,
            memory,
            runtime,
            &endpoint,
            STACK_BASE + index as u32 * 0x40,
            frames[index][0],
            &policy,
        )
    };

    let target_query = dispatch(0, &mut memory, &mut runtime);
    assert_eq!(target_query.eax, RegisterUpdate::Set(256));
    assert_eq!(target_query.stack_pointer, GuestAddress(STACK_BASE + 28));
    assert_eq!(target_query.last_error, INITIAL_LAST_ERROR);
    assert!(target_query.guest_writes.is_empty());

    let target_conversion = dispatch(1, &mut memory, &mut runtime);
    assert_eq!(target_conversion.eax, RegisterUpdate::Set(256));
    assert_eq!(target_conversion.last_error, INITIAL_LAST_ERROR);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(TARGET_DESTINATION), target_output.len())
            .expect("BGI UTF-16 code-page table"),
        target_output
    );

    let japanese_query = dispatch(2, &mut memory, &mut runtime);
    assert_eq!(japanese_query.eax, RegisterUpdate::Set(3));
    assert_eq!(japanese_query.last_error, INITIAL_LAST_ERROR);

    let japanese_conversion = dispatch(3, &mut memory, &mut runtime);
    assert_eq!(japanese_conversion.eax, RegisterUpdate::Set(3));
    assert_eq!(japanese_conversion.last_error, INITIAL_LAST_ERROR);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(JAPANESE_DESTINATION), 6)
            .expect("Japanese UTF-16 conversion"),
        [0x41, 0, 0x42, 0x30, 0, 0]
    );

    let active_code_page_query = dispatch(6, &mut memory, &mut runtime);
    assert_eq!(active_code_page_query.eax, RegisterUpdate::Set(3));
    assert_eq!(active_code_page_query.last_error, INITIAL_LAST_ERROR);
    assert!(active_code_page_query.guest_writes.is_empty());

    let active_code_page_conversion = dispatch(7, &mut memory, &mut runtime);
    assert_eq!(active_code_page_conversion.eax, RegisterUpdate::Set(3));
    assert_eq!(active_code_page_conversion.last_error, INITIAL_LAST_ERROR);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(ACTIVE_CODE_PAGE_DESTINATION), 6)
            .expect("CP_ACP NUL-terminated UTF-16 conversion"),
        [0x41, 0, 0x42, 0x30, 0, 0]
    );

    let insufficient = dispatch(4, &mut memory, &mut runtime);
    assert_eq!(insufficient.eax, RegisterUpdate::Set(0));
    assert_eq!(insufficient.last_error, 122);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(SHORT_DESTINATION), 2)
            .expect("partial UTF-16 conversion"),
        [0x41, 0]
    );

    let zero_count = dispatch(5, &mut memory, &mut runtime);
    assert_eq!(zero_count.eax, RegisterUpdate::Set(0));
    assert_eq!(zero_count.last_error, 87);

    for index in [10_usize, 11] {
        let memory_before = memory.stats();
        let invalid = dispatch(index, &mut memory, &mut runtime);
        assert_eq!(invalid.eax, RegisterUpdate::Set(0));
        assert_eq!(invalid.last_error, 87);
        assert!(invalid.guest_writes.is_empty());
        assert_eq!(memory.stats(), memory_before);
    }

    for index in [8_usize, 9] {
        let runtime_before = runtime.clone();
        let memory_before = memory.stats();
        let unsupported = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            dispatcher.dispatch_owned(
                owned_trap(
                    &endpoint,
                    STACK_BASE + index as u32 * 0x40,
                    frames[index][0],
                ),
                &mut session,
                |contract, frame, session| {
                    dispatch_kernel32_api_with_policy(&policy, contract, frame, session)
                },
            )
        };
        assert!(matches!(
            unsupported,
            DispatchOutcome::Fault(ref fault)
                if fault.class == ApiFaultClass::Unsupported
                    && fault.code == "multi-byte-to-wide-char-variant-unsupported"
        ));
        assert_eq!(runtime, runtime_before);
        assert_eq!(memory.stats(), memory_before);
    }

    let mut explicit_only_policy = policy.clone();
    explicit_only_policy
        .multi_byte_to_wide_char
        .as_mut()
        .unwrap()
        .allow_active_code_page_selector = false;
    explicit_only_policy
        .validate()
        .expect("valid explicit-code-page-only policy");
    let runtime_before_selector = runtime.clone();
    let memory_before_selector = memory.stats();
    let disabled_selector = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, STACK_BASE + 6_u32 * 0x40, frames[6][0]),
            &mut session,
            |contract, frame, session| {
                dispatch_kernel32_api_with_policy(&explicit_only_policy, contract, frame, session)
            },
        )
    };
    assert!(matches!(
        disabled_selector,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "multi-byte-to-wide-char-variant-unsupported"
    ));
    assert_eq!(runtime, runtime_before_selector);
    assert_eq!(memory.stats(), memory_before_selector);

    let runtime_before_disabled = runtime.clone();
    let memory_before_disabled = memory.stats();
    let disabled = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, STACK_BASE, frames[0][0]),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        disabled,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "multi-byte-to-wide-char-policy-disabled"
    ));
    assert_eq!(runtime, runtime_before_disabled);
    assert_eq!(memory.stats(), memory_before_disabled);
}

#[test]
fn set_current_directory_a_owns_normalized_process_directory_transactionally() {
    const EXACT_PATH: u32 = OUTPUT_BASE + 0x700;
    const ALIAS_PATH: u32 = OUTPUT_BASE + 0x720;
    const UNSUPPORTED_PATH: u32 = OUTPUT_BASE + 0x740;
    const RELATIVE_PATH: u32 = OUTPUT_BASE + 0x760;
    const EMPTY_PATH: u32 = OUTPUT_BASE + 0x780;
    const UNTERMINATED_PATH: u32 = OUTPUT_BASE + 0x7a0;

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
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("SetCurrentDirectoryA contract");
    assert_eq!(
        contract.implementation_id,
        SET_CURRENT_DIRECTORY_A_IMPLEMENTATION
    );
    assert_eq!(contract.abi.arguments.len(), 1);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::Bool32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
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

    let mut policy = Kernel32CompatibilityPolicy::controlled_v1();
    policy.id = "public-process-current-directory-v1".to_owned();
    policy.ansi_command_line = true;
    policy.wide_environment_strings = true;
    policy.wide_char_to_multi_byte = Some(Kernel32WideCharToMultiBytePolicy {
        active_code_page: 932,
        maximum_source_code_units: 65_536,
    });
    policy.multi_byte_to_wide_char = Some(Kernel32MultiByteToWideCharPolicy {
        allow_active_code_page_selector: true,
        allowed_flags: vec![0, 1],
        maximum_source_bytes: 65_536,
    });
    policy.set_current_directory_a = Some(Kernel32SetCurrentDirectoryAPolicy {
        process_image_directory_only: true,
        maximum_path_bytes: 32,
    });
    policy
        .validate()
        .expect("valid process current-directory policy");
    let mut runtime = controlled_runtime(controlled_startup_info());
    assert_eq!(runtime.current_directory_a().bytes_with_nul, b"C:\\Vine\0");
    assert_eq!(runtime.current_directory_generation(), 0);

    let dispatch = |index: usize,
                    memory: &mut GuestMemory,
                    runtime: &mut RuntimeState,
                    selected_policy: &Kernel32CompatibilityPolicy|
     -> DispatchOutcome {
        let mut session = ExecutionSession::new(memory, runtime);
        dispatcher.dispatch_owned(
            owned_trap(
                &endpoint,
                STACK_BASE + index as u32 * 0x20,
                frames[index][0],
            ),
            &mut session,
            |contract, frame, session| {
                dispatch_kernel32_api_with_policy(selected_policy, contract, frame, session)
            },
        )
    };

    let exact_input_before = memory.read(GuestAddress(EXACT_PATH), 9).unwrap();
    let exact = dispatch(0, &mut memory, &mut runtime, &policy);
    let DispatchOutcome::Immediate(exact) = exact else {
        panic!("expected SetCurrentDirectoryA success, got {exact:?}");
    };
    assert_eq!(exact.eax, RegisterUpdate::Set(1));
    assert_eq!(exact.stack_pointer, GuestAddress(STACK_BASE + 8));
    assert_eq!(exact.last_error, INITIAL_LAST_ERROR);
    assert!(exact.guest_writes.is_empty());
    assert_eq!(runtime.current_directory_a().bytes_with_nul, b"C:\\Vine\0");
    assert_eq!(runtime.current_directory_generation(), 1);
    assert_eq!(
        memory.read(GuestAddress(EXACT_PATH), 9).unwrap(),
        exact_input_before
    );

    let alias = dispatch(1, &mut memory, &mut runtime, &policy);
    let DispatchOutcome::Immediate(alias) = alias else {
        panic!("expected normalized directory alias success, got {alias:?}");
    };
    assert_eq!(alias.eax, RegisterUpdate::Set(1));
    assert_eq!(alias.last_error, INITIAL_LAST_ERROR);
    assert_eq!(runtime.current_directory_a().bytes_with_nul, b"C:\\Vine\0");
    assert_eq!(runtime.current_directory_generation(), 2);

    for index in [2_usize, 3] {
        let runtime_before = runtime.clone();
        let memory_before = memory.stats();
        let unsupported = dispatch(index, &mut memory, &mut runtime, &policy);
        assert!(matches!(
            unsupported,
            DispatchOutcome::Fault(ref fault)
                if fault.class == ApiFaultClass::Unsupported
                    && fault.code == "set-current-directory-a-path-unsupported"
        ));
        assert_eq!(runtime, runtime_before);
        assert_eq!(memory.stats(), memory_before);
    }

    for (index, expected_error) in [(4_usize, 123), (5, 87)] {
        let directory_before = runtime.current_directory_a().clone();
        let generation_before = runtime.current_directory_generation();
        let memory_before = memory.stats();
        let failure = dispatch(index, &mut memory, &mut runtime, &policy);
        let DispatchOutcome::Immediate(failure) = failure else {
            panic!("expected Win32 directory failure, got {failure:?}");
        };
        assert_eq!(failure.eax, RegisterUpdate::Set(0));
        assert_eq!(failure.last_error, expected_error);
        assert!(failure.guest_writes.is_empty());
        assert_eq!(runtime.current_directory_a(), &directory_before);
        assert_eq!(runtime.current_directory_generation(), generation_before);
        assert_eq!(memory.stats(), memory_before);
    }

    let runtime_before_limit = runtime.clone();
    let memory_before_limit = memory.stats();
    let limit = dispatch(6, &mut memory, &mut runtime, &policy);
    assert!(matches!(
        limit,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::ResourceLimit
                && fault.code == "set-current-directory-a-path-not-terminated"
    ));
    assert_eq!(runtime, runtime_before_limit);
    assert_eq!(memory.stats(), memory_before_limit);

    let mut disabled_policy = policy.clone();
    disabled_policy.set_current_directory_a = None;
    disabled_policy.validate().expect("valid preceding policy");
    let runtime_before_disabled = runtime.clone();
    let memory_before_disabled = memory.stats();
    let disabled = dispatch(0, &mut memory, &mut runtime, &disabled_policy);
    assert!(matches!(
        disabled,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "set-current-directory-a-policy-disabled"
    ));
    assert_eq!(runtime, runtime_before_disabled);
    assert_eq!(memory.stats(), memory_before_disabled);
}

#[test]
fn create_mutex_a_copies_named_identity_and_returns_object_handles() {
    const FIRST_NAME: u32 = OUTPUT_BASE + 0x800;
    const DUPLICATE_NAME: u32 = OUTPUT_BASE + 0x840;
    const UNTERMINATED_NAME: u32 = OUTPUT_BASE + 0x880;
    const NAME: &[u8] = b"Vine public mutex\0";

    let (mut memory, owner) = initialized_memory();
    memory
        .write_initialization(owner, GuestAddress(FIRST_NAME), NAME)
        .expect("first ANSI mutex name");
    memory
        .write_initialization(owner, GuestAddress(DUPLICATE_NAME), NAME)
        .expect("duplicate ANSI mutex name");
    memory
        .write_initialization(owner, GuestAddress(UNTERMINATED_NAME), &[b'X'; 32])
        .expect("unterminated ANSI mutex name");

    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x10c);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize CreateMutexA IAT");
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
            name: "CreateMutexA".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("CreateMutexA contract");
    assert_eq!(contract.implementation_id, CREATE_MUTEX_A_IMPLEMENTATION);
    assert_eq!(contract.abi.arguments.len(), 3);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let frames = [
        vec![0x1000_0d01, 0, 1, FIRST_NAME],
        vec![0x1000_0d02, 0, 1, DUPLICATE_NAME],
        vec![0x1000_0d03, 4, 1, DUPLICATE_NAME],
        vec![0x1000_0d04, 0, 0, 0],
        vec![0x1000_0d05, 0, 0, UNTERMINATED_NAME],
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

    let mut policy: Kernel32CompatibilityPolicy = serde_json::from_str(include_str!(
        "../../../fixtures/runtime-policies/kernel32-create-mutex-a-v1.json"
    ))
    .expect("CreateMutexA policy fixture");
    policy.id = "public-create-mutex-a-v1".to_owned();
    policy.create_mutex_a = Some(Kernel32CreateMutexAPolicy {
        allow_named: true,
        allow_unnamed: false,
        maximum_name_bytes: 32,
    });
    policy.validate().expect("valid bounded named mutex policy");
    let mut runtime = controlled_runtime(controlled_startup_info());
    let dispatch = |index: usize,
                    memory: &mut GuestMemory,
                    runtime: &mut RuntimeState,
                    selected_policy: &Kernel32CompatibilityPolicy|
     -> DispatchOutcome {
        let mut session = ExecutionSession::new(memory, runtime);
        dispatcher.dispatch_owned(
            owned_trap(
                &endpoint,
                STACK_BASE + index as u32 * 0x20,
                frames[index][0],
            ),
            &mut session,
            |contract, frame, session| {
                dispatch_kernel32_api_with_policy(selected_policy, contract, frame, session)
            },
        )
    };

    let first = dispatch(0, &mut memory, &mut runtime, &policy);
    let DispatchOutcome::Immediate(first) = first else {
        panic!("expected new mutex handle, got {first:?}");
    };
    assert_eq!(first.eax, RegisterUpdate::Set(0x5801_0001));
    assert_eq!(first.stack_pointer, GuestAddress(STACK_BASE + 16));
    assert_eq!(first.last_error, 0);
    assert!(first.guest_writes.is_empty());
    let first_handle = vine_core::RuntimeHandle(0x5801_0001);
    assert_eq!(
        runtime
            .mutex_for_handle(first_handle)
            .unwrap()
            .name_a
            .as_ref()
            .unwrap()
            .bytes_with_nul,
        NAME
    );

    memory
        .write(GuestAddress(FIRST_NAME), b"X")
        .expect("mutate caller buffer after return");
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        runtime
            .mutex_for_handle(first_handle)
            .unwrap()
            .name_a
            .as_ref()
            .unwrap()
            .bytes_with_nul,
        NAME,
        "runtime mutex identity must not retain the guest pointer"
    );

    let duplicate = dispatch(1, &mut memory, &mut runtime, &policy);
    let DispatchOutcome::Immediate(duplicate) = duplicate else {
        panic!("expected duplicate mutex handle, got {duplicate:?}");
    };
    assert_eq!(duplicate.eax, RegisterUpdate::Set(0x5801_0002));
    assert_eq!(duplicate.last_error, 183);
    assert!(duplicate.guest_writes.is_empty());
    let duplicate_handle = vine_core::RuntimeHandle(0x5801_0002);
    assert_eq!(
        runtime.mutex_for_handle(first_handle).unwrap().id,
        runtime.mutex_for_handle(duplicate_handle).unwrap().id
    );
    assert_eq!(
        runtime
            .mutex_for_handle(duplicate_handle)
            .unwrap()
            .recursion_count,
        1
    );
    assert_eq!(
        runtime
            .object_for_handle(duplicate_handle, RuntimeObjectKind::Mutex)
            .unwrap()
            .open_handles,
        2
    );

    for (index, code) in [
        (2_usize, "create-mutex-a-security-attributes-unsupported"),
        (3, "create-mutex-a-unnamed-unsupported"),
        (4, "create-mutex-a-name-not-terminated"),
    ] {
        let runtime_before = runtime.clone();
        let memory_before = memory.stats();
        let rejected = dispatch(index, &mut memory, &mut runtime, &policy);
        assert!(matches!(
            rejected,
            DispatchOutcome::Fault(ref fault) if fault.code == code
        ));
        assert_eq!(runtime, runtime_before);
        assert_eq!(memory.stats(), memory_before);
    }

    let mut disabled_policy = policy.clone();
    disabled_policy.create_mutex_a = None;
    disabled_policy.validate().expect("valid preceding policy");
    let runtime_before_disabled = runtime.clone();
    let disabled = dispatch(1, &mut memory, &mut runtime, &disabled_policy);
    assert!(matches!(
        disabled,
        DispatchOutcome::Fault(ref fault)
            if fault.code == "create-mutex-a-policy-disabled"
    ));
    assert_eq!(runtime, runtime_before_disabled);
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
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("GetStringTypeW contract");
    assert_eq!(contract.implementation_id, GET_STRING_TYPE_W_IMPLEMENTATION);
    assert_eq!(contract.abi.arguments.len(), 4);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::Bool32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
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

    let mut policy = Kernel32CompatibilityPolicy::controlled_v1();
    policy.id = "public-string-type-v1".to_owned();
    policy.ansi_command_line = true;
    policy.wide_environment_strings = true;
    policy.wide_char_to_multi_byte = Some(Kernel32WideCharToMultiBytePolicy {
        active_code_page: 932,
        maximum_source_code_units: 65_536,
    });
    policy.multi_byte_to_wide_char = Some(Kernel32MultiByteToWideCharPolicy {
        allow_active_code_page_selector: false,
        allowed_flags: vec![1],
        maximum_source_bytes: 65_536,
    });
    policy.get_string_type_w = Some(Kernel32GetStringTypeWPolicy {
        allowed_info_types: vec![1],
        maximum_source_code_units: 65_536,
    });
    policy
        .validate()
        .expect("valid string classification policy");
    let mut runtime = controlled_runtime(controlled_startup_info());
    let dispatch = |index: usize,
                    memory: &mut GuestMemory,
                    runtime: &mut RuntimeState|
     -> vine_core::ApiImmediateCompletion {
        dispatch_owned_with_policy(
            &dispatcher,
            memory,
            runtime,
            &endpoint,
            STACK_BASE + index as u32 * 0x40,
            frames[index][0],
            &policy,
        )
    };

    let success = dispatch(0, &mut memory, &mut runtime);
    assert_eq!(success.eax, RegisterUpdate::Set(1));
    assert_eq!(success.stack_pointer, GuestAddress(STACK_BASE + 20));
    assert_eq!(success.last_error, INITIAL_LAST_ERROR);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(DESTINATION), expected_bytes.len())
            .expect("CTYPE1 output"),
        expected_bytes
    );

    let memory_before_zero = memory.stats();
    let zero_count = dispatch(1, &mut memory, &mut runtime);
    assert_eq!(zero_count.eax, RegisterUpdate::Set(0));
    assert_eq!(zero_count.last_error, 87);
    assert!(zero_count.guest_writes.is_empty());
    assert_eq!(memory.stats(), memory_before_zero);

    for (index, fault_code) in [
        (2_usize, "get-string-type-w-info-type-unsupported"),
        (3, "get-string-type-w-negative-count-unsupported"),
        (4, "get-string-type-w-code-unit-unsupported"),
        (5, "get-string-type-w-overlap-unsupported"),
    ] {
        let runtime_before = runtime.clone();
        let memory_before = memory.stats();
        let unsupported = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            dispatcher.dispatch_owned(
                owned_trap(
                    &endpoint,
                    STACK_BASE + index as u32 * 0x40,
                    frames[index][0],
                ),
                &mut session,
                |contract, frame, session| {
                    dispatch_kernel32_api_with_policy(&policy, contract, frame, session)
                },
            )
        };
        assert!(matches!(
            unsupported,
            DispatchOutcome::Fault(ref fault)
                if fault.class == ApiFaultClass::Unsupported && fault.code == fault_code
        ));
        assert_eq!(runtime, runtime_before);
        assert_eq!(memory.stats(), memory_before);
    }

    let runtime_before_disabled = runtime.clone();
    let memory_before_disabled = memory.stats();
    let disabled = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, STACK_BASE, frames[0][0]),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        disabled,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "get-string-type-w-policy-disabled"
    ));
    assert_eq!(runtime, runtime_before_disabled);
    assert_eq!(memory.stats(), memory_before_disabled);
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
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("LCMapStringW contract");
    assert_eq!(contract.implementation_id, LC_MAP_STRING_W_IMPLEMENTATION);
    assert_eq!(contract.abi.arguments.len(), 6);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let frames = [
        vec![
            0x1000_0d01,
            0x0411,
            0x0100,
            LOWER_SOURCE,
            lower_source.len() as u32,
            0,
            0,
        ],
        vec![
            0x1000_0d02,
            0x0411,
            0x0100,
            LOWER_SOURCE,
            lower_source.len() as u32,
            LOWER_DESTINATION,
            lower_source.len() as u32,
        ],
        vec![
            0x1000_0d03,
            0x0411,
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

    let mut policy = Kernel32CompatibilityPolicy::controlled_v1();
    policy.id = "public-lc-map-string-v1".to_owned();
    policy.ansi_command_line = true;
    policy.wide_environment_strings = true;
    policy.wide_char_to_multi_byte = Some(Kernel32WideCharToMultiBytePolicy {
        active_code_page: 932,
        maximum_source_code_units: 65_536,
    });
    policy.multi_byte_to_wide_char = Some(Kernel32MultiByteToWideCharPolicy {
        allow_active_code_page_selector: false,
        allowed_flags: vec![1],
        maximum_source_bytes: 65_536,
    });
    policy.get_string_type_w = Some(Kernel32GetStringTypeWPolicy {
        allowed_info_types: vec![1],
        maximum_source_code_units: 65_536,
    });
    policy.lc_map_string_w = Some(Kernel32LcMapStringWPolicy {
        allowed_locales: vec![0x0411],
        allowed_flags: vec![0x0100, 0x0200],
        maximum_source_code_units: 65_536,
    });
    policy.validate().expect("valid locale mapping policy");
    let mut runtime = controlled_runtime(controlled_startup_info());
    let dispatch = |index: usize,
                    memory: &mut GuestMemory,
                    runtime: &mut RuntimeState|
     -> vine_core::ApiImmediateCompletion {
        dispatch_owned_with_policy(
            &dispatcher,
            memory,
            runtime,
            &endpoint,
            STACK_BASE + index as u32 * 0x40,
            frames[index][0],
            &policy,
        )
    };

    for (index, expected) in [(0_usize, lower_source.len()), (2, upper_source.len())] {
        let query = dispatch(index, &mut memory, &mut runtime);
        assert_eq!(query.eax, RegisterUpdate::Set(expected as u32));
        assert_eq!(
            query.stack_pointer,
            GuestAddress(STACK_BASE + index as u32 * 0x40 + 28)
        );
        assert_eq!(query.last_error, INITIAL_LAST_ERROR);
        assert!(query.guest_writes.is_empty());
    }

    let lower = dispatch(1, &mut memory, &mut runtime);
    assert_eq!(lower.eax, RegisterUpdate::Set(lower_source.len() as u32));
    assert_eq!(lower.last_error, INITIAL_LAST_ERROR);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(LOWER_DESTINATION), lower_expected_bytes.len())
            .expect("lowercase output"),
        lower_expected_bytes
    );

    let upper = dispatch(3, &mut memory, &mut runtime);
    assert_eq!(upper.eax, RegisterUpdate::Set(upper_source.len() as u32));
    assert_eq!(upper.last_error, INITIAL_LAST_ERROR);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(UPPER_DESTINATION), upper_expected_bytes.len())
            .expect("uppercase output"),
        upper_expected_bytes
    );

    let memory_before_insufficient = memory.stats();
    let insufficient = dispatch(4, &mut memory, &mut runtime);
    assert_eq!(insufficient.eax, RegisterUpdate::Set(0));
    assert_eq!(insufficient.last_error, 122);
    assert!(insufficient.guest_writes.is_empty());
    assert_eq!(memory.stats(), memory_before_insufficient);

    for (index, error) in [(5_usize, 87_u32), (11, 87)] {
        let memory_before = memory.stats();
        let failure = dispatch(index, &mut memory, &mut runtime);
        assert_eq!(failure.eax, RegisterUpdate::Set(0));
        assert_eq!(failure.last_error, error);
        assert!(failure.guest_writes.is_empty());
        assert_eq!(memory.stats(), memory_before);
    }

    for (index, fault_code) in [
        (6_usize, "lc-map-string-w-locale-unsupported"),
        (7, "lc-map-string-w-flags-unsupported"),
        (8, "lc-map-string-w-negative-source-count-unsupported"),
        (9, "lc-map-string-w-code-unit-unsupported"),
        (10, "lc-map-string-w-overlap-unsupported"),
    ] {
        let runtime_before = runtime.clone();
        let memory_before = memory.stats();
        let unsupported = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            dispatcher.dispatch_owned(
                owned_trap(
                    &endpoint,
                    STACK_BASE + index as u32 * 0x40,
                    frames[index][0],
                ),
                &mut session,
                |contract, frame, session| {
                    dispatch_kernel32_api_with_policy(&policy, contract, frame, session)
                },
            )
        };
        assert!(matches!(
            unsupported,
            DispatchOutcome::Fault(ref fault)
                if fault.class == ApiFaultClass::Unsupported && fault.code == fault_code
        ));
        assert_eq!(runtime, runtime_before);
        assert_eq!(memory.stats(), memory_before);
    }

    let runtime_before_disabled = runtime.clone();
    let memory_before_disabled = memory.stats();
    let disabled = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, STACK_BASE, frames[0][0]),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        disabled,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "lc-map-string-w-policy-disabled"
    ));
    assert_eq!(runtime, runtime_before_disabled);
    assert_eq!(memory.stats(), memory_before_disabled);
}

#[test]
fn interlocked_increment_contract_wraps_and_rejects_invalid_targets_before_mutation() {
    const ZERO_TARGET: u32 = OUTPUT_BASE;
    const WRAP_TARGET: u32 = OUTPUT_BASE + 4;
    const UNMAPPED_TARGET: u32 = 0x4000_0000;
    const CROSS_PAGE_TARGET: u32 = OUTPUT_BASE + 0x0ffe;

    let (mut memory, owner) = initialized_memory();
    let endpoint_address = GuestAddress(ENDPOINT_BASE);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize InterlockedIncrement IAT");
    memory
        .write_initialization(owner, GuestAddress(ZERO_TARGET), &0_i32.to_le_bytes())
        .expect("initialize zero target");
    memory
        .write_initialization(owner, GuestAddress(WRAP_TARGET), &(-1_i32).to_le_bytes())
        .expect("initialize wrapping target");
    let frames = [
        (STACK_BASE, vec![0x1000_0701, ZERO_TARGET]),
        (STACK_BASE + 0x20, vec![0x1000_0702, WRAP_TARGET]),
        (STACK_BASE + 0x40, vec![0x1000_0703, ZERO_TARGET]),
        (STACK_BASE + 0x60, vec![0x1000_0704, UNMAPPED_TARGET]),
        (STACK_BASE + 0x80, vec![0x1000_0705, CROSS_PAGE_TARGET]),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
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
            name: "InterlockedIncrement".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented InterlockedIncrement endpoint");
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let mut runtime = controlled_runtime(controlled_startup_info());

    for (frame, expected) in [(&frames[0], 1_u32), (&frames[1], 0_u32)] {
        let completion = dispatch_owned(
            &dispatcher,
            &mut memory,
            &mut runtime,
            &endpoint,
            frame.0,
            frame.1[0],
        );
        assert_eq!(completion.eax, RegisterUpdate::Set(expected));
        assert_eq!(completion.stack_pointer, GuestAddress(frame.0 + 8));
        assert_eq!(completion.last_error, INITIAL_LAST_ERROR);
        assert_eq!(completion.guest_writes.len(), 1);
        assert_eq!(completion.guest_writes[0].write.address.0, frame.1[1]);
        assert_eq!(
            completion.guest_writes[0].write.bytes,
            (expected as i32).to_le_bytes()
        );
        acknowledge_pending_delta(&mut memory);
        assert_eq!(
            memory
                .read(GuestAddress(frame.1[1]), 4)
                .expect("read incremented LONG"),
            (expected as i32).to_le_bytes()
        );
    }

    memory
        .protect(
            owner,
            GuestRange::new(GuestAddress(OUTPUT_BASE), GUEST_PAGE_SIZE).expect("target page range"),
            PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .expect("make target page read-only");
    acknowledge_pending_delta(&mut memory);
    let runtime_before_rejections = runtime.clone();
    let memory_before_rejections = memory.stats();
    let rejection_cases = [
        (&frames[2], "guest-page-fault", "WriteDenied"),
        (&frames[3], "guest-page-fault", "Unmapped"),
        (
            &frames[4],
            "pointer-alignment-invalid",
            "not aligned to 4 bytes",
        ),
    ];
    for (frame, expected_code, expected_message) in rejection_cases {
        let rejected = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            dispatcher.dispatch_owned(
                owned_trap(&endpoint, frame.0, frame.1[0]),
                &mut session,
                dispatch_kernel32_api,
            )
        };
        assert!(matches!(
            rejected,
            DispatchOutcome::Fault(ref fault)
                if fault.class == ApiFaultClass::InvalidCall
                    && fault.code == expected_code
                    && fault.message.contains(expected_message)
        ));
        assert_eq!(runtime, runtime_before_rejections);
        assert_eq!(memory.stats(), memory_before_rejections);
    }
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    assert_eq!(
        memory
            .read(GuestAddress(ZERO_TARGET), 8)
            .expect("read unchanged valid targets"),
        [1_i32.to_le_bytes(), 0_i32.to_le_bytes()].concat()
    );
}

#[test]
fn interlocked_decrement_contract_models_reached_results_and_fails_before_mutation() {
    const ONE_TARGET: u32 = OUTPUT_BASE;
    const WRAP_TARGET: u32 = OUTPUT_BASE + 4;
    const UNMAPPED_TARGET: u32 = 0x4000_0000;
    const CROSS_PAGE_TARGET: u32 = OUTPUT_BASE + 0x0ffe;

    let (mut memory, owner) = initialized_memory();
    let endpoint_address = GuestAddress(ENDPOINT_BASE);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize InterlockedDecrement IAT");
    memory
        .write_initialization(owner, GuestAddress(ONE_TARGET), &1_i32.to_le_bytes())
        .expect("initialize reached target");
    memory
        .write_initialization(owner, GuestAddress(WRAP_TARGET), &i32::MIN.to_le_bytes())
        .expect("initialize wrapping target");
    let frames = [
        (STACK_BASE, vec![0x1000_0711, ONE_TARGET]),
        (STACK_BASE + 0x20, vec![0x1000_0712, ONE_TARGET]),
        (STACK_BASE + 0x40, vec![0x1000_0713, WRAP_TARGET]),
        (STACK_BASE + 0x60, vec![0x1000_0714, ONE_TARGET]),
        (STACK_BASE + 0x80, vec![0x1000_0715, 0]),
        (STACK_BASE + 0xa0, vec![0x1000_0716, UNMAPPED_TARGET]),
        (STACK_BASE + 0xc0, vec![0x1000_0717, CROSS_PAGE_TARGET]),
    ];
    for (address, words) in &frames {
        write_frame(&mut memory, owner, GuestAddress(*address), words);
    }
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
            name: "InterlockedDecrement".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented InterlockedDecrement endpoint");
    assert_eq!(
        contract.implementation_id,
        "kernel32.interlocked-decrement.v1"
    );
    assert_eq!(contract.abi.arguments.len(), 1);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let mut runtime = controlled_runtime(controlled_startup_info());

    for (frame, expected) in [
        (&frames[0], 0_i32),
        (&frames[1], -1_i32),
        (&frames[2], i32::MAX),
    ] {
        let completion = dispatch_owned(
            &dispatcher,
            &mut memory,
            &mut runtime,
            &endpoint,
            frame.0,
            frame.1[0],
        );
        assert_eq!(completion.eax, RegisterUpdate::Set(expected as u32));
        assert_eq!(completion.stack_pointer, GuestAddress(frame.0 + 8));
        assert_eq!(completion.last_error, INITIAL_LAST_ERROR);
        assert_eq!(completion.guest_writes.len(), 1);
        assert_eq!(completion.guest_writes[0].write.address.0, frame.1[1]);
        assert_eq!(
            completion.guest_writes[0].write.bytes,
            expected.to_le_bytes()
        );
        acknowledge_pending_delta(&mut memory);
        assert_eq!(
            memory
                .read(GuestAddress(frame.1[1]), 4)
                .expect("read decremented LONG"),
            expected.to_le_bytes()
        );
    }

    memory
        .protect(
            owner,
            GuestRange::new(GuestAddress(OUTPUT_BASE), GUEST_PAGE_SIZE).expect("target page range"),
            PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .expect("make target page read-only");
    acknowledge_pending_delta(&mut memory);
    let runtime_before_rejections = runtime.clone();
    let memory_before_rejections = memory.stats();
    let rejection_cases = [
        (&frames[3], "guest-page-fault", "WriteDenied"),
        (&frames[4], "null-pointer-forbidden", "cannot be NULL"),
        (&frames[5], "guest-page-fault", "Unmapped"),
        (
            &frames[6],
            "pointer-alignment-invalid",
            "not aligned to 4 bytes",
        ),
    ];
    for (frame, expected_code, expected_message) in rejection_cases {
        let rejected = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            dispatcher.dispatch_owned(
                owned_trap(&endpoint, frame.0, frame.1[0]),
                &mut session,
                dispatch_kernel32_api,
            )
        };
        assert!(
            matches!(
                rejected,
                DispatchOutcome::Fault(ref fault)
                    if fault.class == ApiFaultClass::InvalidCall
                        && fault.code == expected_code
                        && fault.message.contains(expected_message)
            ),
            "unexpected rejection: {rejected:?}"
        );
        assert_eq!(runtime, runtime_before_rejections);
        assert_eq!(memory.stats(), memory_before_rejections);
    }
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    assert_eq!(
        memory
            .read(GuestAddress(ONE_TARGET), 8)
            .expect("read unchanged valid targets"),
        [(-1_i32).to_le_bytes(), i32::MAX.to_le_bytes()].concat()
    );
}

#[test]
fn pointer_encoding_contract_round_trips_only_process_owned_tokens() {
    let (mut memory, owner) = initialized_memory();
    let (dispatcher, endpoints, frames) = pointer_encoding_dispatcher_fixture(&mut memory, owner);
    let mut runtime = controlled_runtime(controlled_startup_info());
    let memory_before = memory.stats();

    let encoded_null = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[0],
        frames[0].0,
        frames[0].1[0],
    );
    let encoded_endpoint = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[0],
        frames[1].0,
        frames[1].1[0],
    );
    assert_eq!(encoded_null.eax, RegisterUpdate::Set(0xec01_0001));
    assert_eq!(encoded_endpoint.eax, RegisterUpdate::Set(0xec01_0002));

    for frame in [&frames[2], &frames[3]] {
        let decoded = dispatch_owned(
            &dispatcher,
            &mut memory,
            &mut runtime,
            &endpoints[1],
            frame.0,
            frame.1[0],
        );
        assert_eq!(decoded.eax, RegisterUpdate::Set(0));
        assert_eq!(decoded.last_error, INITIAL_LAST_ERROR);
    }
    let decoded_endpoint = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[1],
        frames[4].0,
        frames[4].1[0],
    );
    assert_eq!(decoded_endpoint.eax, RegisterUpdate::Set(0x7f02_0000));
    assert_eq!(decoded_endpoint.last_error, INITIAL_LAST_ERROR);
    assert_eq!(
        runtime
            .decode_pointer(0xec01_0002)
            .expect("registered endpoint token")
            .source,
        0x7f02_0000
    );

    let encoded_pointers_before = runtime.registries().encoded_pointers().clone();
    for (frame, expected_code) in [
        (&frames[5], "encoded-pointer-token-unknown"),
        (&frames[6], "encoded-pointer-token-invalid"),
    ] {
        let rejected = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            dispatcher.dispatch_owned(
                owned_trap(&endpoints[1], frame.0, frame.1[0]),
                &mut session,
                dispatch_kernel32_api,
            )
        };
        assert!(matches!(
            rejected,
            DispatchOutcome::Fault(ref fault)
                if fault.class == ApiFaultClass::InvalidCall && fault.code == expected_code
        ));
    }
    assert_eq!(
        runtime.registries().encoded_pointers(),
        &encoded_pointers_before
    );
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    assert_eq!(memory.stats(), memory_before);
}

#[test]
fn critical_section_contract_owns_recursive_state_and_guest_bytes() {
    const CRITICAL_SECTION_ADDRESS: u32 = OUTPUT_BASE + 0x400;
    const SECOND_THREAD_ID: u32 = 0x3000;

    let (mut memory, owner) = initialized_memory();
    let (dispatcher, endpoints, frames) =
        critical_section_dispatcher_fixture(&mut memory, owner, CRITICAL_SECTION_ADDRESS);
    let mut runtime = controlled_runtime(controlled_startup_info());

    let initialized = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[0],
        frames[0].0,
        frames[0].1[0],
    );
    assert_eq!(initialized.eax, RegisterUpdate::Set(1));
    assert_eq!(initialized.stack_pointer, GuestAddress(frames[0].0 + 12));
    assert_eq!(initialized.last_error, INITIAL_LAST_ERROR);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(CRITICAL_SECTION_ADDRESS), 24)
            .expect("initialized critical section bytes"),
        expected_critical_section_bytes(4000, None, 0)
    );

    let entered = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[1],
        frames[1].0,
        frames[1].1[0],
    );
    assert_eq!(entered.eax, RegisterUpdate::Preserve);
    assert_eq!(entered.stack_pointer, GuestAddress(frames[1].0 + 8));
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(CRITICAL_SECTION_ADDRESS), 24)
            .expect("owned critical section bytes"),
        expected_critical_section_bytes(4000, Some(THREAD_ID), 1)
    );

    dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[1],
        frames[1].0,
        frames[1].1[0],
    );
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        runtime
            .critical_section(GuestAddress(CRITICAL_SECTION_ADDRESS))
            .expect("recursive critical section")
            .recursion_count,
        2
    );
    assert_eq!(
        memory
            .read(GuestAddress(CRITICAL_SECTION_ADDRESS), 24)
            .expect("recursive critical section bytes"),
        expected_critical_section_bytes(4000, Some(THREAD_ID), 2)
    );

    dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[2],
        frames[2].0,
        frames[2].1[0],
    );
    acknowledge_pending_delta(&mut memory);
    runtime
        .register_thread(RuntimeThreadId(SECOND_THREAD_ID), 0x5678)
        .expect("register contending thread");
    runtime
        .select_thread(RuntimeThreadId(SECOND_THREAD_ID))
        .expect("select contending thread");
    let contention = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoints[1], frames[1].0, frames[1].1[0]),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        contention,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "critical-section-contention-unimplemented"
    ));
    assert_eq!(runtime.current_last_error().unwrap(), 0x5678);
    assert_eq!(
        runtime
            .critical_section(GuestAddress(CRITICAL_SECTION_ADDRESS))
            .expect("contended critical section")
            .recursion_count,
        1
    );

    runtime
        .select_thread(RuntimeThreadId(THREAD_ID))
        .expect("restore owning thread");
    let left = dispatch_owned(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[2],
        frames[2].0,
        frames[2].1[0],
    );
    assert_eq!(left.eax, RegisterUpdate::Preserve);
    assert_eq!(left.last_error, INITIAL_LAST_ERROR);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(CRITICAL_SECTION_ADDRESS), 24)
            .expect("released critical section bytes"),
        expected_critical_section_bytes(4000, None, 0)
    );
}

#[test]
fn initialize_critical_section_uses_policy_owned_spin_state_and_void_abi() {
    const CRITICAL_SECTION_ADDRESS: u32 = OUTPUT_BASE + 0x500;
    const POLICY_SPIN_COUNT_FIELD: u32 = 0x0200_1234;

    let (mut memory, owner) = initialized_memory();
    let endpoint_address = GuestAddress(ENDPOINT_BASE + 0x90);
    memory
        .write_initialization(
            owner,
            GuestAddress(IAT_BASE),
            &endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize InitializeCriticalSection IAT");
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
            name: "InitializeCriticalSection".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented InitializeCriticalSection endpoint");
    assert_eq!(
        contract.implementation_id,
        INITIALIZE_CRITICAL_SECTION_IMPLEMENTATION
    );
    assert_eq!(contract.abi.return_value, ApiReturnSchema::Void);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::Preserve);
    assert_eq!(contract.abi.arguments.len(), 1);

    let frame = (STACK_BASE, vec![0x1000_0780, CRITICAL_SECTION_ADDRESS]);
    write_frame(&mut memory, owner, GuestAddress(frame.0), &frame.1);
    memory
        .write_initialization(owner, GuestAddress(CRITICAL_SECTION_ADDRESS), &[0xa5; 24])
        .expect("initialize critical-section output storage");
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let mut runtime = controlled_runtime(controlled_startup_info());

    let before_disabled = runtime.clone();
    let memory_before_disabled = memory.stats();
    let disabled = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frame.0, frame.1[0]),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        disabled,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "initialize-critical-section-policy-unsupported"
    ));
    assert_eq!(runtime, before_disabled);
    assert_eq!(memory.stats(), memory_before_disabled);

    let mut policy = compatibility_policy(
        "initialize-critical-section-test",
        vec![RUNTIME_STANDARD_INPUT_SELECTOR],
        vec![(RUNTIME_STANDARD_INPUT_SELECTOR, 2)],
        vec![(0x8, vec![0x214])],
    );
    policy.initialize_critical_section = Some(Kernel32InitializeCriticalSectionPolicy {
        spin_count_field: POLICY_SPIN_COUNT_FIELD,
    });
    policy
        .validate()
        .expect("valid InitializeCriticalSection policy");
    let initialized = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frame.0,
        frame.1[0],
        &policy,
    );
    assert_eq!(initialized.eax, RegisterUpdate::Preserve);
    assert_eq!(initialized.stack_pointer, GuestAddress(frame.0 + 8));
    assert_eq!(initialized.last_error, INITIAL_LAST_ERROR);
    assert_eq!(initialized.guest_writes.len(), 1);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(CRITICAL_SECTION_ADDRESS), 24)
            .expect("initialized one-argument critical section bytes"),
        expected_critical_section_bytes(POLICY_SPIN_COUNT_FIELD, None, 0)
    );
    assert_eq!(
        runtime
            .critical_section(GuestAddress(CRITICAL_SECTION_ADDRESS))
            .expect("process-owned one-argument critical section")
            .spin_count,
        POLICY_SPIN_COUNT_FIELD
    );

    let before_duplicate = runtime.clone();
    let memory_before_duplicate = memory.stats();
    let duplicate = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frame.0, frame.1[0]),
            &mut session,
            |contract, frame, session| {
                dispatch_kernel32_api_with_policy(&policy, contract, frame, session)
            },
        )
    };
    assert!(matches!(
        duplicate,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::InvalidCall
                && fault.code == "critical-section-already-initialized"
    ));
    assert_eq!(runtime, before_duplicate);
    assert_eq!(memory.stats(), memory_before_duplicate);
}

#[test]
fn remaining_kernel32_calls_share_the_generic_dispatcher() {
    const MODULE_NAME_ADDRESS: u32 = OUTPUT_BASE + 0x200;
    const EXPORT_NAME_ADDRESS: u32 = OUTPUT_BASE + 0x300;

    let (mut memory, owner) = initialized_memory();
    let (dispatcher, endpoints, frames) =
        remaining_dispatcher_fixture(&mut memory, owner, MODULE_NAME_ADDRESS, EXPORT_NAME_ADDRESS);
    let mut runtime = controlled_runtime(controlled_startup_info());
    runtime
        .load_module(RuntimeModule {
            owner: "test-runtime".to_owned(),
            variant_id: "runtime-kernel32".to_owned(),
            namespace: LOGICAL_LOADED_MODULE_NAMESPACE.to_owned(),
            handle: 0x4d01_0001,
            requested_name: "KERNEL32.DLL".to_owned(),
            canonical_name: "kernel32.dll".to_owned(),
            runtime_system_module: true,
        })
        .expect("load logical module");
    runtime
        .register_dynamic_endpoint(RuntimeDynamicEndpoint {
            namespace: DYNAMIC_EXPORT_ENDPOINT_NAMESPACE.to_owned(),
            id: 1,
            address: GuestAddress(0x7f02_0000),
            module: "kernel32.dll".to_owned(),
            name: "FlsAlloc".to_owned(),
            lifetime: "process-runtime".to_owned(),
        })
        .expect("register dynamic endpoint");

    let system_time = dispatch(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[0],
        frames[0].0,
        frames[0].1[0],
        4,
    );
    assert_eq!(system_time.eax, RegisterUpdate::Preserve);
    assert_eq!(system_time.guest_writes.len(), 1);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory.read(GuestAddress(OUTPUT_BASE), 8).unwrap(),
        134_282_016_000_000_000_u64.to_le_bytes()
    );

    let thread = dispatch(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[1],
        frames[1].0,
        frames[1].1[0],
        0,
    );
    assert_eq!(thread.eax, RegisterUpdate::Set(THREAD_ID));
    let tick = dispatch(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[2],
        frames[2].0,
        frames[2].1[0],
        0,
    );
    assert_eq!(tick.eax, RegisterUpdate::Set(3_600_000));
    assert_eq!(runtime.clock().tick_count, 3_600_000);
    let mut advancing_clock = Kernel32CompatibilityPolicy::controlled_v1();
    advancing_clock.id = "kernel32-advancing-tick-count-test-v1".to_owned();
    advancing_clock.tick_count = Some(Kernel32TickCountPolicy {
        increment_per_read: 1,
    });
    advancing_clock.validate().unwrap();
    let first_advancing_tick = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[2],
        frames[2].0,
        frames[2].1[0],
        &advancing_clock,
    );
    assert_eq!(first_advancing_tick.eax, RegisterUpdate::Set(3_600_000));
    assert_eq!(runtime.clock().tick_count, 3_600_001);
    let second_advancing_tick = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[2],
        frames[2].0,
        frames[2].1[0],
        &advancing_clock,
    );
    assert_eq!(second_advancing_tick.eax, RegisterUpdate::Set(3_600_001));
    assert_eq!(runtime.clock().tick_count, 3_600_002);

    let heap_policy = dispatch(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[3],
        frames[3].0,
        frames[3].1[0],
        16,
    );
    assert_eq!(heap_policy.eax, RegisterUpdate::Set(1));
    assert!(runtime.process().heap_termination_on_corruption());
    let heap = dispatch(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[4],
        frames[4].0,
        frames[4].1[0],
        12,
    );
    assert_eq!(heap.eax, RegisterUpdate::Set(0x6801_0001));
    assert_eq!(runtime.heap(0x6801_0001).unwrap().initial_size, 0x1000);

    let module = dispatch(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[5],
        frames[5].0,
        frames[5].1[0],
        4,
    );
    assert_eq!(module.eax, RegisterUpdate::Set(0x4d01_0001));
    let procedure = dispatch(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[6],
        frames[6].0,
        frames[6].1[0],
        8,
    );
    assert_eq!(procedure.eax, RegisterUpdate::Set(0x7f02_0000));

    let tls = dispatch(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoints[7],
        frames[7].0,
        frames[7].1[0],
        0,
    );
    assert_eq!(tls.eax, RegisterUpdate::Set(0));
    assert!(runtime.allocated_tls_indices().contains(&0));
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);

    let ordinal = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch(
            trap(&endpoints[6], frames[8].0, frames[8].1[0], 8),
            &mut session,
            dispatch_kernel32_api,
        )
    };
    assert!(matches!(
        ordinal,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "get-proc-address-ordinal-unsupported"
    ));
}

#[test]
fn deferred_and_guest_callback_completions_preserve_the_decoded_abi_frame() {
    let (mut memory, owner) = initialized_memory();
    let (dispatcher, endpoints, frames) = suspended_dispatcher_fixture(&mut memory, owner);
    let mut runtime = controlled_runtime(controlled_startup_info());

    let deferred = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch(
            trap(&endpoints[0], frames[0].0, frames[0].1[0], 8),
            &mut session,
            |_, frame, _| {
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
    let deferred = match deferred {
        DispatchOutcome::Deferred(call) => call,
        other => panic!("expected deferred dispatch, got {other:?}"),
    };
    let deferred_completion = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.complete_deferred(
            deferred,
            HostCompletion {
                request_id: 17,
                result: ApiImmediate::new(ApiReturnValue::U32(42)),
            },
            &mut session,
        )
    };
    let deferred_completion = match deferred_completion {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected deferred completion, got {other:?}"),
    };
    assert_eq!(
        deferred_completion.implementation_id,
        "test-deferred-add-v1"
    );
    assert_eq!(deferred_completion.eax, RegisterUpdate::Set(42));
    assert_eq!(
        deferred_completion.instruction_pointer,
        GuestAddress(frames[0].1[0])
    );
    assert_eq!(
        deferred_completion.stack_pointer,
        GuestAddress(frames[0].0 + 4)
    );

    let callback = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch(
            trap(&endpoints[1], frames[1].0, frames[1].1[0], 8),
            &mut session,
            |_, frame, _| {
                ApiHandlerResult::GuestCallback(GuestCallbackRequest {
                    target: GuestAddress(frame.argument_u32(0).expect("callback target")),
                    arguments: vec![vine_core::GuestCallbackArgument::U32(
                        frame.argument_u32(1).expect("callback value"),
                    )],
                    scratch: Vec::new(),
                    return_sentinel: GuestAddress(0x7f01_0000),
                    completion_policy: vine_core::GuestCallbackCompletionPolicy::ReturnU32,
                })
            },
        )
    };
    let callback = match callback {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected guest callback dispatch, got {other:?}"),
    };
    assert_eq!(
        callback.request.arguments,
        [vine_core::GuestCallbackArgument::U32(42)]
    );
    let callback_completion = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.complete_guest_callback(
            callback,
            ApiImmediate::new(ApiReturnValue::U32(44)),
            &mut session,
        )
    };
    let callback_completion = match callback_completion {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected callback completion, got {other:?}"),
    };
    assert_eq!(
        callback_completion.implementation_id,
        "test-call-callback-v1"
    );
    assert_eq!(callback_completion.eax, RegisterUpdate::Set(44));
    assert_eq!(
        callback_completion.instruction_pointer,
        GuestAddress(frames[1].1[0])
    );
    assert_eq!(
        callback_completion.stack_pointer,
        GuestAddress(frames[1].0 + 4)
    );
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
}

fn initialized_memory() -> (GuestMemory, MemoryOwnerId) {
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 4,
        max_regions: 4,
        max_owners: 4,
        max_access_bytes: 0x4000,
    })
    .expect("memory");
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

fn initialized_memory_with_environment_capacity() -> (GuestMemory, MemoryOwnerId) {
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 8,
        max_regions: 8,
        max_owners: 8,
        max_access_bytes: 0x4000,
    })
    .expect("memory");
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
    let contracts = endpoints[..5]
        .iter()
        .cloned()
        .map(|endpoint| {
            kernel32_contract_for_endpoint(endpoint)
                .expect("Kernel32 contract lookup")
                .expect("implemented Kernel32 endpoint")
        })
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
        ApiDispatcher::new(endpoints.clone(), contracts).expect("dispatcher"),
        endpoints,
        frames,
    )
}

fn remaining_dispatcher_fixture(
    memory: &mut GuestMemory,
    owner: MemoryOwnerId,
    module_name_address: u32,
    export_name_address: u32,
) -> (ApiDispatcher, Vec<ApiEndpointIdentity>, [FrameWords; 9]) {
    let names = [
        "GetSystemTimeAsFileTime",
        "GetCurrentThreadId",
        "GetTickCount",
        "HeapSetInformation",
        "HeapCreate",
        "GetModuleHandleW",
        "GetProcAddress",
        "TlsAlloc",
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
    let contracts = endpoints
        .iter()
        .cloned()
        .map(|endpoint| {
            kernel32_contract_for_endpoint(endpoint)
                .expect("Kernel32 contract lookup")
                .expect("implemented Kernel32 endpoint")
        })
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
        (STACK_BASE + 0x100, vec![0x1000_0109, 0x4d01_0001, 1]),
    ];
    for (address, words) in &frames {
        write_frame(memory, owner, GuestAddress(*address), words);
    }
    let module_name = "KERNEL32.DLL"
        .encode_utf16()
        .chain([0])
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    memory
        .write_initialization(owner, GuestAddress(module_name_address), &module_name)
        .expect("initialize module name");
    memory
        .write_initialization(owner, GuestAddress(export_name_address), b"FlsAlloc\0")
        .expect("initialize export name");
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    (
        ApiDispatcher::new(endpoints.clone(), contracts).expect("dispatcher"),
        endpoints,
        frames,
    )
}

fn critical_section_dispatcher_fixture(
    memory: &mut GuestMemory,
    owner: MemoryOwnerId,
    critical_section_address: u32,
) -> (ApiDispatcher, Vec<ApiEndpointIdentity>, [FrameWords; 3]) {
    let names = [
        "InitializeCriticalSectionAndSpinCount",
        "EnterCriticalSection",
        "LeaveCriticalSection",
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
                .expect("initialize critical-section IAT");
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
    let contracts = endpoints
        .iter()
        .cloned()
        .map(|endpoint| {
            kernel32_contract_for_endpoint(endpoint)
                .expect("Kernel32 contract lookup")
                .expect("implemented critical-section endpoint")
        })
        .collect::<Vec<_>>();
    let frames = [
        (
            STACK_BASE + 0x120,
            vec![0x1000_0301, critical_section_address, 4000],
        ),
        (
            STACK_BASE + 0x140,
            vec![0x1000_0302, critical_section_address],
        ),
        (
            STACK_BASE + 0x160,
            vec![0x1000_0303, critical_section_address],
        ),
    ];
    for (address, words) in &frames {
        write_frame(memory, owner, GuestAddress(*address), words);
    }
    memory
        .write_initialization(owner, GuestAddress(critical_section_address), &[0xa5; 24])
        .expect("initialize critical-section storage");
    memory.seal_owner(owner).expect("seal runtime owner");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial reflection");
    (
        ApiDispatcher::new(endpoints.clone(), contracts).expect("critical-section dispatcher"),
        endpoints,
        frames,
    )
}

fn pointer_encoding_dispatcher_fixture(
    memory: &mut GuestMemory,
    owner: MemoryOwnerId,
) -> (ApiDispatcher, Vec<ApiEndpointIdentity>, [FrameWords; 7]) {
    let names = ["EncodePointer", "DecodePointer"];
    let thunks = names
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            let id = ImportThunkId((index + 1) as u32);
            let address = GuestAddress(ENDPOINT_BASE + (index as u32 * 0x10));
            let iat_address = GuestAddress(IAT_BASE + (index as u32 * 4));
            memory
                .write_initialization(owner, iat_address, &address.0.to_le_bytes())
                .expect("initialize pointer-encoding IAT");
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
    let contracts = endpoints
        .iter()
        .cloned()
        .map(|endpoint| {
            kernel32_contract_for_endpoint(endpoint)
                .expect("Kernel32 contract lookup")
                .expect("implemented pointer-encoding endpoint")
        })
        .collect::<Vec<_>>();
    let frames = [
        (STACK_BASE + 0x180, vec![0x1000_0401, 0]),
        (STACK_BASE + 0x1a0, vec![0x1000_0402, 0x7f02_0000]),
        (STACK_BASE + 0x1c0, vec![0x1000_0403, 0xec01_0001]),
        (STACK_BASE + 0x1e0, vec![0x1000_0404, 0xec01_0001]),
        (STACK_BASE + 0x200, vec![0x1000_0405, 0xec01_0002]),
        (STACK_BASE + 0x220, vec![0x1000_0406, 0xec01_00ff]),
        (STACK_BASE + 0x240, vec![0x1000_0407, 0x1234_5678]),
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
        ApiDispatcher::new(endpoints.clone(), contracts).expect("pointer-encoding dispatcher"),
        endpoints,
        frames,
    )
}

fn fls_allocation_dispatcher_fixture(
    memory: &mut GuestMemory,
    owner: MemoryOwnerId,
    callback_address: u32,
) -> (ApiDispatcher, ApiEndpointIdentity, [FrameWords; 3]) {
    memory
        .protect(
            owner,
            GuestRange::new(GuestAddress(OUTPUT_BASE), GUEST_PAGE_SIZE)
                .expect("callback page range"),
            PageProtection {
                read: true,
                write: true,
                execute: true,
            },
        )
        .expect("make callback page executable");
    let dynamic = RuntimeDynamicEndpoint {
        namespace: DYNAMIC_EXPORT_ENDPOINT_NAMESPACE.to_owned(),
        id: 1,
        address: GuestAddress(0x7f02_0000),
        module: "kernel32.dll".to_owned(),
        name: "FlsAlloc".to_owned(),
        lifetime: "process-runtime".to_owned(),
    };
    let endpoint = ApiEndpointIdentity::from_dynamic_endpoint(&dynamic);
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented FLS allocation endpoint");
    let static_endpoint_address = GuestAddress(0x7f00_0000);
    let static_iat_address = GuestAddress(IAT_BASE);
    memory
        .write_initialization(
            owner,
            static_iat_address,
            &static_endpoint_address.0.to_le_bytes(),
        )
        .expect("initialize colliding static endpoint IAT");
    let static_endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(1),
        address: static_endpoint_address,
        requester_owner: owner,
        module: "fixture.dll".to_owned(),
        iat_rva: Rva(0),
        iat_address: static_iat_address,
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "StaticIdCollision".to_owned(),
            hint: 0,
            name_rva: Rva(0),
        },
    });
    let frames = [
        (STACK_BASE + 0x260, vec![0x1000_0501, 0x3000_0000]),
        (STACK_BASE + 0x280, vec![0x1000_0502, callback_address]),
        (STACK_BASE + 0x2a0, vec![0x1000_0503, 0]),
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
        ApiDispatcher::new([static_endpoint, endpoint.clone()], [contract])
            .expect("static/dynamic ID collision FLS allocation dispatcher"),
        endpoint,
        frames,
    )
}

fn fls_set_value_dispatcher_fixture(
    memory: &mut GuestMemory,
    owner: MemoryOwnerId,
) -> (ApiDispatcher, ApiEndpointIdentity, [FrameWords; 3]) {
    let dynamic = RuntimeDynamicEndpoint {
        namespace: DYNAMIC_EXPORT_ENDPOINT_NAMESPACE.to_owned(),
        id: 3,
        address: GuestAddress(0x7f02_0020),
        module: "kernel32.dll".to_owned(),
        name: "FlsSetValue".to_owned(),
        lifetime: "process-runtime".to_owned(),
    };
    let endpoint = ApiEndpointIdentity::from_dynamic_endpoint(&dynamic);
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("Kernel32 contract lookup")
        .expect("implemented FLS value endpoint");
    let frames = [
        (STACK_BASE + 0x2c0, vec![0x1000_0511, 0, 0x2000_0000]),
        (STACK_BASE + 0x2e0, vec![0x1000_0512, 0, 0x2000_0100]),
        (STACK_BASE + 0x300, vec![0x1000_0513, 1, 0x2000_0200]),
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
        ApiDispatcher::new([endpoint.clone()], [contract]).expect("FLS value dispatcher"),
        endpoint,
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
    let u32_argument = |name: &str| ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::U32,
    };
    let contract = |endpoint: ApiEndpointIdentity,
                    implementation_id: &str,
                    behavior: ApiBehavior,
                    callback_policy: CallbackPolicy,
                    arguments: Vec<ApiArgumentSchema>| ApiContract {
        endpoint,
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
        implementation_id: implementation_id.to_owned(),
    };
    let contracts = vec![
        contract(
            endpoints[0].clone(),
            "test-deferred-add-v1",
            ApiBehavior::Deferred,
            CallbackPolicy::Forbidden,
            vec![u32_argument("left"), u32_argument("right")],
        ),
        contract(
            endpoints[1].clone(),
            "test-call-callback-v1",
            ApiBehavior::Immediate,
            CallbackPolicy::Required,
            vec![
                ApiArgumentSchema {
                    name: "callback".to_owned(),
                    direction: ApiDirection::In,
                    value: ApiValueSchema::OpaquePointer {
                        nullable: false,
                        alignment: 1,
                    },
                },
                u32_argument("value"),
            ],
        ),
    ];
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
        ApiDispatcher::new(endpoints.clone(), contracts).expect("suspended dispatcher"),
        endpoints,
        frames,
    )
}

fn controlled_runtime(startup_bytes: Vec<u8>) -> RuntimeState {
    let mut runtime = RuntimeState::new(RuntimeBootstrap {
        process_id: RuntimeProcessId(PROCESS_ID),
        initial_thread_id: RuntimeThreadId(THREAD_ID),
        initial_last_error: INITIAL_LAST_ERROR,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: PERFORMANCE_COUNTER,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "test-startup-profile".to_owned(),
            bytes: startup_bytes,
        },
        process_image_path_a: vine_core::RuntimeProcessImagePathA {
            source_authority: "api-dispatcher-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: vine_core::RuntimeCurrentDirectoryA {
            source_authority: "api-dispatcher-process-bootstrap".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec01_0000,
    })
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

fn expected_critical_section_bytes(
    spin_count: u32,
    owning_thread_id: Option<u32>,
    recursion_count: u32,
) -> Vec<u8> {
    let lock_count = if recursion_count == 0 { -1_i32 } else { -2_i32 };
    [
        u32::MAX.to_le_bytes(),
        lock_count.to_le_bytes(),
        recursion_count.to_le_bytes(),
        owning_thread_id.unwrap_or(0).to_le_bytes(),
        0_u32.to_le_bytes(),
        spin_count.to_le_bytes(),
    ]
    .concat()
}

fn dispatch(
    dispatcher: &ApiDispatcher,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    endpoint: &ApiEndpointIdentity,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
) -> vine_core::ApiImmediateCompletion {
    let mut session = ExecutionSession::new(memory, runtime);
    match dispatcher.dispatch(
        trap(endpoint, stack_pointer, return_address, argument_bytes),
        &mut session,
        dispatch_kernel32_api,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected immediate dispatch, got {other:?}"),
    }
}

fn dispatch_owned(
    dispatcher: &ApiDispatcher,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    endpoint: &ApiEndpointIdentity,
    stack_pointer: u32,
    return_address: u32,
) -> vine_core::ApiImmediateCompletion {
    let mut session = ExecutionSession::new(memory, runtime);
    match dispatcher.dispatch_owned(
        owned_trap(endpoint, stack_pointer, return_address),
        &mut session,
        dispatch_kernel32_api,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected immediate owned dispatch, got {other:?}"),
    }
}

fn dispatch_owned_with_policy(
    dispatcher: &ApiDispatcher,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    endpoint: &ApiEndpointIdentity,
    stack_pointer: u32,
    return_address: u32,
    policy: &Kernel32CompatibilityPolicy,
) -> vine_core::ApiImmediateCompletion {
    let mut session = ExecutionSession::new(memory, runtime);
    match dispatcher.dispatch_owned(
        owned_trap(endpoint, stack_pointer, return_address),
        &mut session,
        |contract, frame, session| {
            dispatch_kernel32_api_with_policy(policy, contract, frame, session)
        },
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected policy dispatch, got {other:?}"),
    }
}

fn compatibility_policy(
    id: &str,
    standard_handle_selectors: Vec<u32>,
    standard_handle_file_types: Vec<(u32, u32)>,
    heap_allocations: Vec<(u32, Vec<u32>)>,
) -> Kernel32CompatibilityPolicy {
    let mut policy = Kernel32CompatibilityPolicy::controlled_v1();
    policy.id = id.to_owned();
    policy.standard_handle_selectors = standard_handle_selectors;
    policy.standard_handle_file_types = standard_handle_file_types
        .into_iter()
        .map(|(selector, file_type)| Kernel32StandardHandleFileType {
            selector,
            file_type,
        })
        .collect();
    policy.heap_allocations = heap_allocations
        .into_iter()
        .map(|(flags, byte_lengths)| Kernel32HeapAllocationPolicy {
            flags,
            byte_lengths,
        })
        .collect();
    policy.validate().expect("valid test compatibility policy");
    policy
}

#[allow(clippy::too_many_arguments)]
fn dispatch_with_handler(
    dispatcher: &ApiDispatcher,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    endpoint: &ApiEndpointIdentity,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
    immediate: ApiImmediate,
) -> DispatchOutcome {
    let mut session = ExecutionSession::new(memory, runtime);
    dispatcher.dispatch(
        trap(endpoint, stack_pointer, return_address, argument_bytes),
        &mut session,
        |_, _, _| ApiHandlerResult::Immediate(immediate),
    )
}

fn trap(
    endpoint: &ApiEndpointIdentity,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
) -> ApiTrap {
    ApiTrap {
        endpoint_id: endpoint.id,
        endpoint_address: endpoint.address,
        stack_pointer: GuestAddress(stack_pointer),
        return_address: GuestAddress(return_address),
        argument_bytes,
    }
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

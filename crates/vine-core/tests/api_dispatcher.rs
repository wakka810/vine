use std::cell::Cell;

use vine_core::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiContract, ApiDirection, ApiDispatcher,
    ApiEndpointIdentity, ApiFaultClass, ApiHandlerResult, ApiImmediate, ApiOwnedTrap,
    ApiReturnSchema, ApiReturnValue, ApiTrap, ApiValueSchema, CallbackPolicy, CallingConvention,
    DYNAMIC_EXPORT_ENDPOINT_NAMESPACE, DispatchOutcome, ExecutionSession, GUEST_PAGE_SIZE,
    GuestAddress, GuestCallbackRequest, GuestMemory, GuestRange, HostCompletion, HostRequest,
    ImportTarget, ImportThunk, ImportThunkDisposition, ImportThunkId, Kernel32CompatibilityPolicy,
    Kernel32HeapAllocationPolicy, Kernel32StandardHandleFileType,
    Kernel32WideCharToMultiBytePolicy, LOGICAL_ENCODED_POINTER_NAMESPACE,
    LOGICAL_FLS_INDEX_NAMESPACE, LOGICAL_LOADED_MODULE_NAMESPACE, LOGICAL_PRIVATE_HEAP_NAMESPACE,
    LOGICAL_STANDARD_HANDLE_NAMESPACE, LastErrorPolicy, MemoryLimits, MemoryOwnerId,
    MemoryOwnerKind, MemoryRegionKind, PROCESS_PARAMETERS_ENVIRONMENT_POINTER_OFFSET,
    PageProtection, ProcessAddressSpaceLayout, RUNTIME_FLS_INDEX_LIMIT,
    RUNTIME_PRIVATE_HEAP_UNINITIALIZED_BYTE, RUNTIME_STANDARD_ERROR_HANDLE,
    RUNTIME_STANDARD_ERROR_SELECTOR, RUNTIME_STANDARD_INPUT_HANDLE,
    RUNTIME_STANDARD_INPUT_SELECTOR, RUNTIME_STANDARD_OUTPUT_HANDLE,
    RUNTIME_STANDARD_OUTPUT_SELECTOR, ReentrancyPolicy, RegisterUpdate, RuntimeBootstrap,
    RuntimeClockState, RuntimeDynamicEndpoint, RuntimeHeap, RuntimeHeapAllocationId,
    RuntimeHeapInitialization, RuntimeModule, RuntimeProcessId, RuntimeSchedulerEvent,
    RuntimeStartupInfo, RuntimeState, RuntimeThreadId, RuntimeTlsValue, Rva, SchedulerThreadState,
    StackCleanup, dispatch_kernel32_api, dispatch_kernel32_api_with_policy,
    install_controlled_process_environment_with_command_line_and_ansi,
    install_controlled_process_environment_with_command_line_ansi_and_environment_strings,
    kernel32_contract_for_endpoint,
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
    assert_eq!(runtime.registries().encoded_pointers.len(), 2);
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
    assert!(runtime.process().heap_termination_on_corruption);
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
    assert!(runtime.registries().host_requests.is_empty());
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
    assert!(runtime.process().fls_slots.is_empty());

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
    assert_eq!(
        runtime.process().fls_slots.len(),
        RUNTIME_FLS_INDEX_LIMIT as usize
    );
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
        "fls-value-uninitialized"
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
        (FRAME_BASE + 0x60, vec![0x1000_0604, 0x6801_0001, 0x8, 0xbc]),
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

    let runtime_before_later_rejection = runtime.clone();
    let memory_before_later_rejection = memory.stats();
    let later_rejected = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        dispatcher.dispatch_owned(
            owned_trap(&endpoint, frames[3].0, frames[3].1[0]),
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

    let public_fixture_policy = compatibility_policy(
        "public-additional-size-test",
        vec![RUNTIME_STANDARD_INPUT_SELECTOR],
        vec![(RUNTIME_STANDARD_INPUT_SELECTOR, 2)],
        vec![(0x8, vec![0x214, 0x800, 0xbc])],
    );
    let third = dispatch_owned_with_policy(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &endpoint,
        frames[3].0,
        frames[3].1[0],
        &public_fixture_policy,
    );
    assert_eq!(third.eax, RegisterUpdate::Set(0x2000_0a20));
    assert_eq!(
        runtime
            .heap_backing(0x6801_0001)
            .unwrap()
            .unwrap()
            .allocations
            .len(),
        4
    );
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
    assert_eq!(runtime.process().standard_handles.len(), 1);
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
    assert_eq!(runtime.process().standard_handles.len(), 1);
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
    assert_eq!(runtime.process().standard_handles.len(), 1);
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
    let environment = install_controlled_process_environment_with_command_line_and_ansi(
        &mut memory,
        "fixture.exe",
    )
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
    let mut runtime = controlled_runtime(controlled_startup_info());
    let registered = runtime
        .register_initial_process_environment(&environment)
        .expect("register process environment")
        .clone();
    assert_eq!(
        registered.command_line.ansi_buffer_address,
        ansi.buffer_address
    );
    assert_ne!(
        registered.command_line.ansi_buffer_address,
        registered.command_line.wide_buffer_address
    );
    assert_eq!(
        runtime
            .process_address_space_allocator()
            .allocation(registered.address_allocation_id)
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
fn get_and_free_environment_strings_w_manage_one_process_owned_copy() {
    let (mut memory, owner) = initialized_memory_with_environment_capacity();
    let environment =
        install_controlled_process_environment_with_command_line_ansi_and_environment_strings(
            &mut memory,
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
    let mut runtime = controlled_runtime(controlled_startup_info());
    runtime
        .register_initial_process_environment(&environment)
        .expect("register process environment");
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

    let encoded_pointers_before = runtime.registries().encoded_pointers.clone();
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
        runtime.registries().encoded_pointers,
        encoded_pointers_before
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
    assert!(runtime.process().heap_termination_on_corruption);
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
    assert!(runtime.process().allocated_tls_indices.contains(&0));
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
                    arguments: vec![frame.argument_u32(1).expect("callback value")],
                    return_sentinel: GuestAddress(0x7f01_0000),
                })
            },
        )
    };
    let callback = match callback {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected guest callback dispatch, got {other:?}"),
    };
    assert_eq!(callback.request.arguments, [42]);
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

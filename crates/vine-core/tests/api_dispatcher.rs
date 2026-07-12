use std::cell::Cell;

use vine_core::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiContract, ApiDirection, ApiDispatcher,
    ApiEndpointIdentity, ApiFaultClass, ApiHandlerResult, ApiImmediate, ApiOwnedTrap,
    ApiReturnSchema, ApiReturnValue, ApiTrap, ApiValueSchema, CallbackPolicy, CallingConvention,
    DYNAMIC_EXPORT_ENDPOINT_NAMESPACE, DispatchOutcome, ExecutionSession, GUEST_PAGE_SIZE,
    GuestAddress, GuestCallbackRequest, GuestMemory, GuestRange, HostCompletion, HostRequest,
    ImportTarget, ImportThunk, ImportThunkDisposition, ImportThunkId,
    LOGICAL_ENCODED_POINTER_NAMESPACE, LOGICAL_FLS_INDEX_NAMESPACE,
    LOGICAL_LOADED_MODULE_NAMESPACE, LOGICAL_PRIVATE_HEAP_NAMESPACE, LastErrorPolicy, MemoryLimits,
    MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind, PageProtection, ProcessAddressSpaceLayout,
    RUNTIME_FLS_INDEX_LIMIT, ReentrancyPolicy, RegisterUpdate, RuntimeBootstrap, RuntimeClockState,
    RuntimeDynamicEndpoint, RuntimeHeap, RuntimeHeapAllocationId, RuntimeModule, RuntimeProcessId,
    RuntimeSchedulerEvent, RuntimeStartupInfo, RuntimeState, RuntimeThreadId, RuntimeTlsValue, Rva,
    SchedulerThreadState, StackCleanup, dispatch_kernel32_api, kernel32_contract_for_endpoint,
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
fn heap_alloc_contract_uses_transactional_guest_backing_and_rejects_other_variants() {
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
    assert_eq!(runtime.current_last_error().unwrap(), INITIAL_LAST_ERROR);
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

use crate::directshow::{
    CLSID_FILTER_GRAPH, IID_BASIC_AUDIO, IID_FILTER_GRAPH, IID_GRAPH_BUILDER, IID_MEDIA_CONTROL,
    IID_MEDIA_EVENT_EX, IID_MEDIA_EVENT_SINK, IID_MEDIA_SEEKING,
};
use crate::ole32::dispatch_ole32_api;
use crate::{
    ApiDispatcher, ApiEndpointIdentity, ApiOwnedTrap, ComCallCompletion, ComCallTrap,
    ComDispatchOutcome, DispatchOutcome, GuestAddress, GuestMemory, GuestRange, ImportTarget,
    ImportThunk, ImportThunkDisposition, ImportThunkId, MemoryLimits, MemoryOwnerId,
    MemoryOwnerKind, MemoryRegionKind, PageProtection, RUNTIME_COM_IUNKNOWN_IID, Runtime,
    RuntimeBootstrap, RuntimeClockState, RuntimeComApartmentModel, RuntimeComClass, RuntimeComIid,
    RuntimeComInterfaceRecord, RuntimeCurrentDirectoryA, RuntimeProcessId,
    RuntimeProcessImagePathA, RuntimeStartupInfo, RuntimeThreadId,
};

const BASE: GuestAddress = GuestAddress(0x2000_0000);
const STACK: GuestAddress = GuestAddress(BASE.0 + 0x100);
const CLASS: GuestAddress = GuestAddress(BASE.0 + 0x300);
const IID: GuestAddress = GuestAddress(BASE.0 + 0x310);
const OUTPUT: GuestAddress = GuestAddress(BASE.0 + 0x340);
const IAT: GuestAddress = GuestAddress(BASE.0 + 0x2000);
const ENDPOINT: GuestAddress = GuestAddress(0x7f00_12c0);
const RETURN: GuestAddress = GuestAddress(0x1008_7399);
const LAST_ERROR: u32 = 0x1234_5678;
const SENTINEL: u32 = 0xaabb_ccdd;
const UNKNOWN: RuntimeComIid = RuntimeComIid([0x5a; 16]);
const GRAPH_IIDS: [RuntimeComIid; 8] = [
    RUNTIME_COM_IUNKNOWN_IID,
    IID_GRAPH_BUILDER,
    IID_FILTER_GRAPH,
    IID_MEDIA_CONTROL,
    IID_MEDIA_EVENT_EX,
    IID_MEDIA_SEEKING,
    IID_BASIC_AUDIO,
    IID_MEDIA_EVENT_SINK,
];

#[test]
fn directshow_window_destroyed_is_consumed_without_application_event_or_reference_change() {
    let (mut fixture, sink, renderer, _, _) = window_destroyed_fixture();
    let event = fixture
        .runtime
        .com()
        .interface_for_object_iid(sink.object_id, IID_MEDIA_EVENT_EX)
        .unwrap()
        .clone();
    fixture
        .runtime
        .configure_input_profile(crate::RuntimeInputProfile {
            initial_mouse_buttons_swapped: false,
            initial_keyboard_source_focused: None,
            cursor_position: Some(crate::RuntimeCursorPositionProfile {
                initial_screen_x: 0,
                initial_screen_y: 0,
            }),
        })
        .unwrap();
    let thread = fixture.runtime.current_thread_id();
    fixture
        .runtime
        .post_thread_message(thread, 0, 0x0400, 17, 23)
        .unwrap();
    let messages = fixture.runtime.queued_thread_messages(thread).unwrap();
    let object = fixture
        .runtime
        .com()
        .object(sink.object_id)
        .unwrap()
        .clone();
    fixture.prepare_com(&sink, &[21, renderer.0, 0]);
    let memory = fixture.runtime.memory.current_reflection().unwrap();
    assert!(matches!(
        fixture.dispatch_com(&sink, 3),
        ComDispatchOutcome::Completed(completion) if completion.eax == 0
    ));
    assert_eq!(fixture.runtime.memory.current_reflection().unwrap(), memory);
    assert_eq!(
        fixture.runtime.com().object(sink.object_id).unwrap(),
        &object
    );
    assert_eq!(
        fixture.runtime.queued_thread_messages(thread).unwrap(),
        messages
    );
    let graph = &fixture.runtime.direct_show.graphs[&sink.object_id];
    assert_eq!(graph.filters.len(), 1);
    assert_eq!(graph.filters[0].pointer, renderer);
    assert_eq!(graph.state, crate::directshow::GraphState::Stopped);
    assert!(!graph.completed);
    assert!(fixture.runtime.direct_show.callbacks.is_empty());
    fixture.assert_last_error();

    let outputs = [OUTPUT.0, OUTPUT.0 + 4, OUTPUT.0 + 8];
    write_words(&mut fixture.runtime.memory, OUTPUT, &[SENTINEL; 3]);
    assert_eq!(
        fixture
            .call(&event, 8, &[outputs[0], outputs[1], outputs[2], 0])
            .eax,
        0x8000_4004
    );
    assert_eq!(
        fixture.runtime.memory.read(OUTPUT, 12).unwrap(),
        [SENTINEL; 3]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        fixture.runtime.queued_thread_messages(thread).unwrap(),
        messages
    );
    fixture.assert_last_error();
}

#[test]
fn directshow_window_destroyed_rejects_invalid_or_retired_renderer_atomically_then_retries() {
    let (mut fixture, sink, renderer, method, target) = window_destroyed_fixture();
    for (arguments, expected) in [
        ([21, renderer.0, 1], Some(0x8007_0057)),
        ([21, 0, 0], None),
        ([21, renderer.0 + 4, 0], None),
    ] {
        assert_window_destroyed_rejected(&mut fixture, &sink, arguments, expected);
    }
    write_words(&mut fixture.runtime.memory, method, &[OUTPUT.0]);
    acknowledge(&mut fixture.runtime.memory);
    assert_window_destroyed_rejected(&mut fixture, &sink, [21, renderer.0, 0], None);
    write_words(&mut fixture.runtime.memory, method, &[target.0]);
    acknowledge(&mut fixture.runtime.memory);
    assert_eq!(fixture.call(&sink, 3, &[21, renderer.0, 0]).eax, 0);

    let renderer_page = GuestRange::new(GuestAddress(BASE.0 + 0x1000), 0x1000).unwrap();
    fixture
        .runtime
        .memory
        .decommit(fixture.owner, renderer_page)
        .unwrap();
    acknowledge(&mut fixture.runtime.memory);
    assert_window_destroyed_rejected(&mut fixture, &sink, [21, renderer.0, 0], None);
    fixture
        .runtime
        .memory
        .commit(
            fixture.owner,
            renderer_page,
            PageProtection {
                read: true,
                write: true,
                execute: false,
            },
        )
        .unwrap();
    write_words(&mut fixture.runtime.memory, renderer, &[BASE.0 + 0x1200]);
    write_words(&mut fixture.runtime.memory, method, &[target.0]);
    acknowledge(&mut fixture.runtime.memory);
    assert_eq!(fixture.call(&sink, 3, &[21, renderer.0, 0]).eax, 0);
    fixture.assert_last_error();
}

fn window_destroyed_fixture() -> (
    Fixture,
    RuntimeComInterfaceRecord,
    GuestAddress,
    GuestAddress,
    GuestAddress,
) {
    let mut fixture = Fixture::new(MemoryLimits::default());
    assert_eq!(fixture.create(CLSID_FILTER_GRAPH, IID_MEDIA_EVENT_SINK), 0);
    let sink = fixture.interface(read_u32(&fixture.runtime.memory, OUTPUT));
    let allocation = fixture
        .runtime
        .process_address_space_allocator()
        .allocation_for_purpose(&crate::ProcessAddressAllocationPurpose::CallbackReturns)
        .unwrap()
        .clone();
    let target = crate::install_runtime_return_sentinels(&mut fixture.runtime.memory, &allocation)
        .unwrap()
        .process_entry_return
        .address;
    let renderer = GuestAddress(BASE.0 + 0x1100);
    let vtable = GuestAddress(BASE.0 + 0x1200);
    let method = GuestAddress(vtable.0 + 13 * 4);
    write_words(&mut fixture.runtime.memory, renderer, &[vtable.0]);
    write_words(&mut fixture.runtime.memory, method, &[target.0]);
    acknowledge(&mut fixture.runtime.memory);
    fixture
        .runtime
        .direct_show
        .graphs
        .get_mut(&sink.object_id)
        .unwrap()
        .filters
        .push(crate::directshow::GuestFilter { pointer: renderer });
    (fixture, sink, renderer, method, target)
}

fn assert_window_destroyed_rejected(
    fixture: &mut Fixture,
    sink: &RuntimeComInterfaceRecord,
    arguments: [u32; 3],
    expected: Option<u32>,
) {
    fixture.prepare_com(sink, &arguments);
    let memory = fixture.runtime.memory.current_reflection().unwrap();
    let statistics = fixture.runtime.memory.stats();
    let object = fixture
        .runtime
        .com()
        .object(sink.object_id)
        .unwrap()
        .clone();
    let allocator = fixture.runtime.process_address_space_allocator().clone();
    let scheduler = fixture.runtime.scheduler_state().unwrap();
    let thread = fixture.runtime.current_thread_id();
    let messages = fixture.runtime.queued_thread_messages(thread).unwrap();
    match (fixture.dispatch_com(sink, 3), expected) {
        (ComDispatchOutcome::Completed(completion), Some(hresult)) => {
            assert_eq!(completion.eax, hresult)
        }
        (ComDispatchOutcome::Fault(_), None) => {}
        (other, _) => panic!("unexpected rejected renderer notification: {other:?}"),
    }
    assert_eq!(fixture.runtime.memory.current_reflection().unwrap(), memory);
    assert_eq!(fixture.runtime.memory.stats(), statistics);
    assert_eq!(
        fixture.runtime.com().object(sink.object_id).unwrap(),
        &object
    );
    assert_eq!(
        fixture.runtime.process_address_space_allocator(),
        &allocator
    );
    assert_eq!(fixture.runtime.scheduler_state().unwrap(), scheduler);
    assert_eq!(
        fixture.runtime.queued_thread_messages(thread).unwrap(),
        messages
    );
    assert_eq!(
        fixture.runtime.direct_show.graphs[&sink.object_id]
            .filters
            .len(),
        1
    );
    assert!(fixture.runtime.direct_show.callbacks.is_empty());
    fixture.assert_last_error();
}

#[test]
fn directshow_render_file_empty_path_fails_without_publication() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    assert_eq!(fixture.create(CLSID_FILTER_GRAPH, IID_GRAPH_BUILDER), 0);
    let graph = fixture.interface(read_u32(&fixture.runtime.memory, OUTPUT));
    let empty_path = GuestAddress(BASE.0 + 0x600);
    fixture.runtime.memory.write(empty_path, &[0, 0]).unwrap();
    fixture.prepare_com(&graph, &[empty_path.0, 0]);
    let bytes = fixture.runtime.memory.read(BASE, 0x1000).unwrap();
    let memory = fixture.runtime.memory.stats();
    let allocator = fixture.runtime.process_address_space_allocator().clone();
    let scheduler = fixture.runtime.scheduler_state().unwrap();
    let ComDispatchOutcome::Completed(completion) = fixture.dispatch_com(&graph, 13) else {
        panic!("empty RenderFile must complete with a failing HRESULT");
    };
    assert_eq!(completion.eax, crate::directshow::VFW_E_NOT_FOUND);
    assert_eq!(fixture.runtime.memory.read(BASE, 0x1000).unwrap(), bytes);
    assert_eq!(fixture.runtime.memory.stats(), memory);
    assert_eq!(
        fixture.runtime.process_address_space_allocator(),
        &allocator
    );
    assert_eq!(fixture.runtime.scheduler_state().unwrap(), scheduler);
    assert_eq!(fixture.runtime.com().objects().len(), 1);
    assert_eq!(
        fixture
            .runtime
            .com()
            .object(graph.object_id)
            .unwrap()
            .reference_count,
        1
    );
    let state = &fixture.runtime.direct_show.graphs[&graph.object_id];
    assert_eq!(state.state, crate::directshow::GraphState::Stopped);
    assert_eq!(state.position_100ns, 0);
    assert!(state.duration_100ns.is_none());
    assert!(state.filters.is_empty());
    assert!(fixture.runtime.direct_show.callbacks.is_empty());
    fixture.assert_last_error();
    assert_eq!(fixture.call(&graph, 2, &[]).eax, 0);
    assert!(fixture.runtime.direct_show.graphs.is_empty());
}

#[test]
fn directshow_guest_read_rejects_bad_callee_without_consuming_wait_and_retries() {
    let (mut fixture, pending, slot, target) = pending_guest_reader();
    write_words(&mut fixture.runtime.memory, slot, &[0]);
    acknowledge(&mut fixture.runtime.memory);
    assert_read_rejected_without_mutation(&mut fixture, pending.request_id);
    write_words(&mut fixture.runtime.memory, slot, &[target.0]);
    acknowledge(&mut fixture.runtime.memory);
    assert!(matches!(
        fixture
            .runtime
            .read_com_media(pending.request_id, 0, 16)
            .unwrap(),
        ComDispatchOutcome::GuestCallback(_)
    ));
}

fn assert_read_rejected_without_mutation(fixture: &mut Fixture, request_id: u64) {
    let before = fixture.runtime.memory.stats();
    let allocator = fixture.runtime.process_address_space_allocator().clone();
    let scheduler = fixture.runtime.scheduler_state().unwrap();
    let thread = fixture.runtime.current_thread_id();
    let callback_depth = fixture
        .runtime
        .thread_state(thread)
        .unwrap()
        .callback_frames
        .len();
    let scratch = fixture.runtime.memory.read(BASE, 0x1000).unwrap();
    assert!(fixture.runtime.read_com_media(request_id, 0, 16).is_err());
    assert_eq!(fixture.runtime.memory.stats(), before);
    assert_eq!(
        fixture.runtime.process_address_space_allocator(),
        &allocator
    );
    assert_eq!(fixture.runtime.scheduler_state().unwrap(), scheduler);
    assert_eq!(
        fixture
            .runtime
            .thread_state(thread)
            .unwrap()
            .callback_frames
            .len(),
        callback_depth
    );
    assert_eq!(fixture.runtime.memory.read(BASE, 0x1000).unwrap(), scratch);
    fixture.assert_last_error();
}

fn pending_guest_reader() -> (
    Fixture,
    crate::directshow::ComMediaDeferred,
    GuestAddress,
    GuestAddress,
) {
    let mut fixture = Fixture::new(MemoryLimits::default());
    assert_eq!(fixture.create(CLSID_FILTER_GRAPH, IID_GRAPH_BUILDER), 0);
    let graph = fixture.interface(read_u32(&fixture.runtime.memory, OUTPUT));
    let callback_allocation = fixture
        .runtime
        .process_address_space_allocator()
        .allocation_for_purpose(&crate::ProcessAddressAllocationPurpose::CallbackReturns)
        .unwrap()
        .clone();
    let sentinels =
        crate::install_runtime_return_sentinels(&mut fixture.runtime.memory, &callback_allocation)
            .unwrap();
    let stack = crate::install_initial_thread_stack(
        &mut fixture.runtime.memory,
        &sentinels.process_entry_return,
        1024 * 1024,
        64 * 1024,
    )
    .unwrap();
    fixture
        .runtime
        .register_initial_thread_stack(&stack)
        .unwrap();
    let source = GuestAddress(BASE.0 + 0x600);
    let vtable = GuestAddress(BASE.0 + 0x700);
    let target = sentinels.process_entry_return.address;
    let slot = GuestAddress(vtable.0 + 7 * 4);
    write_words(&mut fixture.runtime.memory, source, &[vtable.0]);
    for index in [0, 7, 8] {
        write_words(
            &mut fixture.runtime.memory,
            GuestAddress(vtable.0 + index * 4),
            &[target.0],
        );
    }
    let trap_stack = GuestAddress(stack.initial_esp.0 - 128);
    write_words(
        &mut fixture.runtime.memory,
        trap_stack,
        &[RETURN.0, graph.guest_pointer.0, source.0],
    );
    acknowledge(&mut fixture.runtime.memory);
    let outcome = fixture.runtime.dispatch_com_call(ComCallTrap {
        endpoint_address: graph.methods[&12].endpoint,
        stack_pointer: trap_stack,
        return_address: RETURN,
        call_site: RETURN,
    });
    let ComDispatchOutcome::GuestCallback(query) = outcome else {
        panic!("Render: {outcome:?}");
    };
    acknowledge(&mut fixture.runtime.memory);
    assert_eq!(
        read_u32(
            &fixture.runtime.memory,
            GuestAddress(query.stack_pointer.0 + 4)
        ),
        source.0
    );
    let queried_iid = GuestAddress(read_u32(
        &fixture.runtime.memory,
        GuestAddress(query.stack_pointer.0 + 8),
    ));
    assert_eq!(
        fixture.runtime.memory.read(queried_iid, 16).unwrap(),
        crate::directshow::IID_ASYNC_READER.0
    );
    let output = GuestAddress(read_u32(
        &fixture.runtime.memory,
        GuestAddress(query.stack_pointer.0 + 12),
    ));
    write_words(&mut fixture.runtime.memory, output, &[source.0]);
    acknowledge(&mut fixture.runtime.memory);
    let outcome = fixture
        .runtime
        .complete_guest_callback(&fixture.dispatcher, sentinels.callback_return, 0)
        .unwrap();
    let crate::GuestCallbackReturn::Com(ComDispatchOutcome::GuestCallback(length)) = outcome else {
        panic!("QI: {outcome:?}");
    };
    acknowledge(&mut fixture.runtime.memory);
    let length_outputs = [8, 12].map(|offset| {
        GuestAddress(read_u32(
            &fixture.runtime.memory,
            GuestAddress(length.stack_pointer.0 + offset),
        ))
    });
    for output in length_outputs {
        fixture
            .runtime
            .memory
            .write(output, &4096_i64.to_le_bytes())
            .unwrap();
    }
    acknowledge(&mut fixture.runtime.memory);
    let outcome = fixture
        .runtime
        .complete_guest_callback(&fixture.dispatcher, sentinels.callback_return, 0)
        .unwrap();
    let crate::GuestCallbackReturn::Com(ComDispatchOutcome::MediaPending(pending)) = outcome else {
        panic!("Length: {outcome:?}");
    };
    acknowledge(&mut fixture.runtime.memory);
    (fixture, pending, slot, target)
}

#[test]
fn directshow_factory_and_query_interface_share_one_live_identity() {
    for requested in GRAPH_IIDS {
        let mut fixture = Fixture::new(MemoryLimits::default());
        assert_eq!(fixture.create(CLSID_FILTER_GRAPH, requested), 0);
        let interface = fixture.interface(read_u32(&fixture.runtime.memory, OUTPUT));
        let object = fixture.runtime.com().object(interface.object_id).unwrap();
        assert_eq!(object.class, RuntimeComClass::DirectShowGraph);
        assert_eq!(object.reference_count, 1);
        let backing = object.backing.clone();
        let unknown = fixture
            .runtime
            .com()
            .interface_for_object_iid(interface.object_id, RUNTIME_COM_IUNKNOWN_IID)
            .unwrap()
            .clone();
        for iid in GRAPH_IIDS {
            fixture.write_iid(iid);
            assert_eq!(fixture.call(&interface, 0, &[IID.0, OUTPUT.0]).eax, 0);
            let queried = fixture.interface(read_u32(&fixture.runtime.memory, OUTPUT));
            assert_eq!(queried.object_id, interface.object_id);
            assert_eq!(queried.iid, iid);
            fixture.write_iid(RUNTIME_COM_IUNKNOWN_IID);
            assert_eq!(fixture.call(&queried, 0, &[IID.0, OUTPUT.0]).eax, 0);
            assert_eq!(
                read_u32(&fixture.runtime.memory, OUTPUT),
                unknown.guest_pointer.0
            );
            assert_eq!(fixture.call(&unknown, 2, &[]).eax, 2);
            assert_eq!(fixture.call(&queried, 2, &[]).eax, 1);
        }
        assert_eq!(fixture.call(&interface, 2, &[]).eax, 0);
        assert!(fixture.runtime.com().is_empty());
        assert!(fixture.runtime.com().interfaces().is_empty());
        assert!(
            fixture
                .runtime
                .memory
                .read(interface.guest_pointer, 4)
                .is_err()
        );
        assert!(
            fixture
                .runtime
                .process_address_space_allocator()
                .allocation(backing.address_allocation_id)
                .is_err()
        );
        assert!(
            fixture
                .runtime
                .memory
                .owner_is_sealed(backing.memory_owner)
                .is_err()
        );
        fixture.assert_last_error();
    }
}

#[test]
fn directshow_factory_rejects_unknown_class_or_interface_without_publication() {
    for (class, iid, result) in [
        (UNKNOWN, IID_GRAPH_BUILDER, 0x8004_0154),
        (CLSID_FILTER_GRAPH, UNKNOWN, 0x8000_4002),
    ] {
        let mut fixture = Fixture::new(MemoryLimits::default());
        fixture.prepare_create(class, iid);
        let memory_before = fixture.runtime.memory.stats();
        let allocations_before = fixture.runtime.process_address_space_allocator().clone();
        assert_eq!(immediate(fixture.dispatch_create()).eax, Some(result));
        acknowledge(&mut fixture.runtime.memory);
        assert_eq!(read_u32(&fixture.runtime.memory, OUTPUT), 0);
        assert!(fixture.runtime.com().is_empty());
        assert_eq!(
            fixture.runtime.process_address_space_allocator(),
            &allocations_before
        );
        // A completed HRESULT changes only the caller's output bytes.
        assert_eq!(
            fixture.runtime.memory.stats().physical_pages,
            memory_before.physical_pages
        );
        assert_eq!(
            fixture.runtime.memory.stats().regions,
            memory_before.regions
        );
        assert_eq!(fixture.runtime.memory.stats().owners, memory_before.owners);
        fixture.assert_last_error();
    }
}

#[test]
fn directshow_unknown_query_clears_output_without_acquiring_a_reference() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    assert_eq!(fixture.create(CLSID_FILTER_GRAPH, IID_GRAPH_BUILDER), 0);
    let graph = fixture.interface(read_u32(&fixture.runtime.memory, OUTPUT));
    fixture.write_iid(UNKNOWN);
    fixture.write_output();
    assert_eq!(fixture.call(&graph, 0, &[IID.0, OUTPUT.0]).eax, 0x8000_4002);
    assert_eq!(read_u32(&fixture.runtime.memory, OUTPUT), 0);
    assert_eq!(
        fixture
            .runtime
            .com()
            .object(graph.object_id)
            .unwrap()
            .reference_count,
        1
    );
    fixture.write_output();
    fixture.prepare_com(&graph, &[BASE.0 + 0x2ff8, OUTPUT.0]);
    let before = fixture.runtime.memory.stats();
    assert!(matches!(
        fixture.dispatch_com(&graph, 0),
        ComDispatchOutcome::Fault(_)
    ));
    assert_eq!(read_u32(&fixture.runtime.memory, OUTPUT), SENTINEL);
    assert_eq!(fixture.runtime.memory.stats(), before);
    assert_eq!(
        fixture
            .runtime
            .com()
            .object(graph.object_id)
            .unwrap()
            .reference_count,
        1
    );
    assert_eq!(fixture.call(&graph, 2, &[]).eax, 0);
    fixture.assert_last_error();
}

#[test]
fn directshow_factory_bad_pointer_or_read_only_output_is_atomic() {
    for arguments in [
        [BASE.0 + 0x2ff8, 0, 1, IID.0, OUTPUT.0],
        [CLASS.0, 0, 1, BASE.0 + 0x2ff8, OUTPUT.0],
        [CLASS.0, 0, 1, IID.0, BASE.0 + 0x2fff],
    ] {
        let mut fixture = Fixture::new(MemoryLimits::default());
        fixture.prepare_create(CLSID_FILTER_GRAPH, IID_GRAPH_BUILDER);
        write_words(
            &mut fixture.runtime.memory,
            STACK,
            &[
                RETURN.0,
                arguments[0],
                arguments[1],
                arguments[2],
                arguments[3],
                arguments[4],
            ],
        );
        acknowledge(&mut fixture.runtime.memory);
        fixture.assert_factory_fault_is_atomic();
    }
    let mut fixture = Fixture::new(MemoryLimits::default());
    fixture.prepare_create(CLSID_FILTER_GRAPH, IID_GRAPH_BUILDER);
    fixture
        .runtime
        .memory
        .protect(
            fixture.owner,
            GuestRange::new(BASE, 0x1000).unwrap(),
            PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .unwrap();
    acknowledge(&mut fixture.runtime.memory);
    fixture.assert_factory_fault_is_atomic();
}

#[test]
fn directshow_factory_backing_exhaustion_does_not_publish_or_leak() {
    for limits in [
        MemoryLimits {
            max_physical_pages: 3,
            ..MemoryLimits::default()
        },
        MemoryLimits {
            max_regions: 1,
            ..MemoryLimits::default()
        },
        MemoryLimits {
            max_owners: 1,
            ..MemoryLimits::default()
        },
    ] {
        let mut fixture = Fixture::new(limits);
        fixture.prepare_create(CLSID_FILTER_GRAPH, IID_GRAPH_BUILDER);
        fixture.assert_factory_fault_is_atomic();
    }
}

struct Fixture {
    runtime: Runtime,
    owner: MemoryOwnerId,
    dispatcher: ApiDispatcher,
    endpoint: ApiEndpointIdentity,
}

impl Fixture {
    fn new(limits: MemoryLimits) -> Self {
        let mut memory = GuestMemory::new(limits).unwrap();
        let owner = memory.create_owner(MemoryOwnerKind::Runtime).unwrap();
        let range = memory
            .reserve_fixed(owner, MemoryRegionKind::Runtime, BASE, 0x3000)
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
        memory
            .write_initialization(owner, IAT, &ENDPOINT.0.to_le_bytes())
            .unwrap();
        memory.seal_owner(owner).unwrap();
        let reflection = memory.pending_reflection().unwrap();
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .unwrap();
        let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
            id: ImportThunkId(301),
            address: ENDPOINT,
            requester_owner: owner,
            module: "ole32.dll".to_owned(),
            iat_rva: crate::Rva(0x2000),
            iat_address: IAT,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "CoCreateInstance".to_owned(),
                hint: 0,
                name_rva: crate::Rva(0),
            },
        });
        let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap();
        let mut runtime = Runtime::new(
            RuntimeBootstrap {
                process_id: RuntimeProcessId(4096),
                initial_thread_id: RuntimeThreadId(8192),
                initial_last_error: LAST_ERROR,
                clock: RuntimeClockState {
                    wall_clock_file_time: 134_282_016_000_000_000,
                    tick_count: 3_600_000,
                    performance_counter: 12_345_678_901,
                    performance_frequency: 10_000_000,
                },
                startup_info: RuntimeStartupInfo { bytes: vec![0; 68] },
                process_image_path_a: RuntimeProcessImagePathA {
                    bytes_with_nul: b"C:\\Vine\\directshow.exe\0".to_vec(),
                },
                initial_current_directory_a: RuntimeCurrentDirectoryA {
                    bytes_with_nul: b"C:\\Vine\0".to_vec(),
                },
                encoded_pointer_cookie: 0xec01_9d3b,
            },
            memory,
        )
        .unwrap();
        runtime.start_initial_thread().unwrap();
        runtime
            .initialize_current_com_apartment(RuntimeComApartmentModel::SingleThreaded)
            .unwrap();
        Self {
            runtime,
            owner,
            dispatcher,
            endpoint,
        }
    }

    fn prepare_create(&mut self, class: RuntimeComIid, iid: RuntimeComIid) {
        self.runtime
            .memory
            .write(CLASS, &class.windows_bytes())
            .unwrap();
        self.write_iid(iid);
        self.write_output();
        write_words(
            &mut self.runtime.memory,
            STACK,
            &[RETURN.0, CLASS.0, 0, 1, IID.0, OUTPUT.0],
        );
        acknowledge(&mut self.runtime.memory);
    }

    fn create(&mut self, class: RuntimeComIid, iid: RuntimeComIid) -> u32 {
        self.prepare_create(class, iid);
        let completion = immediate(self.dispatch_create());
        acknowledge(&mut self.runtime.memory);
        completion.eax.expect("CoCreateInstance HRESULT")
    }

    fn dispatch_create(&mut self) -> DispatchOutcome {
        self.dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: self.endpoint.id,
                endpoint_address: self.endpoint.address,
                stack_pointer: STACK,
                return_address: RETURN,
            },
            &mut self.runtime,
            None,
            |frame, runtime, _| dispatch_ole32_api(frame, runtime),
        )
    }

    fn interface(&self, pointer: u32) -> RuntimeComInterfaceRecord {
        self.runtime
            .com()
            .interface_for_pointer(GuestAddress(pointer))
            .unwrap()
            .unwrap()
            .clone()
    }

    fn write_iid(&mut self, iid: RuntimeComIid) {
        self.runtime
            .memory
            .write(IID, &iid.windows_bytes())
            .unwrap();
    }

    fn write_output(&mut self) {
        self.runtime
            .memory
            .write(OUTPUT, &SENTINEL.to_le_bytes())
            .unwrap();
    }

    fn prepare_com(&mut self, interface: &RuntimeComInterfaceRecord, arguments: &[u32]) {
        let mut words = vec![RETURN.0, interface.guest_pointer.0];
        words.extend_from_slice(arguments);
        write_words(&mut self.runtime.memory, STACK, &words);
        acknowledge(&mut self.runtime.memory);
    }

    fn dispatch_com(
        &mut self,
        interface: &RuntimeComInterfaceRecord,
        slot: u16,
    ) -> ComDispatchOutcome {
        self.runtime.dispatch_com_call(ComCallTrap {
            endpoint_address: interface.methods[&slot].endpoint,
            stack_pointer: STACK,
            return_address: RETURN,
            call_site: RETURN,
        })
    }

    fn call(
        &mut self,
        interface: &RuntimeComInterfaceRecord,
        slot: u16,
        arguments: &[u32],
    ) -> ComCallCompletion {
        self.prepare_com(interface, arguments);
        match self.dispatch_com(interface, slot) {
            ComDispatchOutcome::Completed(completion) => {
                acknowledge(&mut self.runtime.memory);
                completion
            }
            other => panic!("unexpected DirectShow COM outcome: {other:?}"),
        }
    }

    fn assert_factory_fault_is_atomic(&mut self) {
        let memory_before = self.runtime.memory.stats();
        let allocations_before = self.runtime.process_address_space_allocator().clone();
        assert!(matches!(self.dispatch_create(), DispatchOutcome::Fault(_)));
        assert_eq!(read_u32(&self.runtime.memory, OUTPUT), SENTINEL);
        assert!(self.runtime.com().is_empty());
        assert!(self.runtime.com().interfaces().is_empty());
        assert_eq!(self.runtime.memory.stats(), memory_before);
        assert_eq!(
            self.runtime.process_address_space_allocator(),
            &allocations_before
        );
        self.assert_last_error();
    }

    fn assert_last_error(&self) {
        assert_eq!(self.runtime.current_last_error().unwrap(), LAST_ERROR);
    }
}

fn immediate(outcome: DispatchOutcome) -> crate::ApiImmediateCompletion {
    match outcome {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("unexpected DirectShow factory outcome: {other:?}"),
    }
}

fn write_words(memory: &mut GuestMemory, address: GuestAddress, words: &[u32]) {
    let bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
    memory.write(address, &bytes).unwrap();
}

fn read_u32(memory: &GuestMemory, address: GuestAddress) -> u32 {
    u32::from_le_bytes(memory.read(address, 4).unwrap().try_into().unwrap())
}

fn acknowledge(memory: &mut GuestMemory) {
    if memory.stats().pending_invalidation.is_some() {
        let delta = memory.pending_delta().unwrap();
        memory.acknowledge_delta(delta.generation).unwrap();
    }
}

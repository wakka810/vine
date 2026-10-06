use super::*;
use crate::{
    GuestMemory, MemoryLimits, ProcessAddressAllocationPurpose, RuntimeBootstrap,
    RuntimeClockState, RuntimeCurrentDirectoryA, RuntimeProcessId, RuntimeProcessImagePathA,
    RuntimeStartupInfo,
};

const DATA: GuestAddress = GuestAddress(0x2000_0000);
const RENDERER: GuestAddress = GuestAddress(DATA.0 + 0x100);
const VTABLE: GuestAddress = GuestAddress(DATA.0 + 0x200);
const LAST_ERROR: u32 = 0x1234_5678;

#[derive(Clone, Copy)]
enum Completion {
    Opened,
    Frame,
    Ended,
}

#[test]
fn media_callback_read_only_stack_retains_open_frame_and_end_then_retries() {
    for completion in [Completion::Opened, Completion::Frame, Completion::Ended] {
        let (mut runtime, pending) = pending_completion(completion, 0);
        let stack = runtime.thread_stack(pending.thread_id).unwrap().clone();
        let address = runtime.direct_show.active[&pending.request_id]
            .origin
            .trap
            .stack_pointer;
        let page = GuestRange::new(GuestAddress(address.0 & !0xfff), 4096).unwrap();
        runtime
            .memory
            .protect(stack.memory_owner, page, read_only())
            .unwrap();
        acknowledge(&mut runtime);
        assert_completion_rejected(&mut runtime, &pending, response(completion, &pending));
        runtime
            .memory
            .protect(stack.memory_owner, page, writable())
            .unwrap();
        acknowledge(&mut runtime);
        assert_retry(&mut runtime, &pending, completion, 1);
    }
}

#[test]
fn media_callback_id_exhaustion_retains_open_frame_and_end_without_publication() {
    for completion in [Completion::Opened, Completion::Frame, Completion::Ended] {
        let (mut runtime, pending) = pending_completion(completion, 0);
        runtime.exhaust_callback_ids_for_test();
        let error =
            assert_completion_rejected(&mut runtime, &pending, response(completion, &pending));
        assert!(
            error
                .message
                .contains("callback frame ID space is exhausted")
        );
    }
}

#[test]
fn media_opened_read_only_scratch_retains_wait_and_format_then_retries() {
    let (mut runtime, pending) = pending_completion(Completion::Opened, 0);
    let scratch = runtime.callback_scratch_allocation().unwrap().clone();
    let owner = runtime
        .memory
        .create_owner(MemoryOwnerKind::Runtime)
        .unwrap();
    runtime
        .memory
        .reserve_fixed_for_address_allocation(
            scratch.id.0,
            owner,
            MemoryRegionKind::Runtime,
            scratch.range.base,
            scratch.range.byte_length,
        )
        .unwrap();
    let slot = GuestRange::new(
        scratch.range.base,
        u64::from(crate::callbacks::CALLBACK_SCRATCH_SLOT_SIZE),
    )
    .unwrap();
    runtime.memory.commit(owner, slot, writable()).unwrap();
    runtime
        .memory
        .write(scratch.range.base, &[0xa5; 4])
        .unwrap();
    runtime.memory.protect(owner, slot, read_only()).unwrap();
    acknowledge(&mut runtime);
    let error = assert_completion_rejected(
        &mut runtime,
        &pending,
        response(Completion::Opened, &pending),
    );
    assert!(
        error
            .message
            .contains("scratch page has wrong owner or protection")
    );
    runtime.memory.protect(owner, slot, writable()).unwrap();
    acknowledge(&mut runtime);
    assert_retry(&mut runtime, &pending, Completion::Opened, 1);
}

#[test]
fn media_opened_scratch_depth_exhaustion_retains_held_nested_wait() {
    let slots = crate::callbacks::CALLBACK_SCRATCH_ARENA_SIZE
        / u64::from(crate::callbacks::CALLBACK_SCRATCH_SLOT_SIZE);
    let (mut runtime, pending) = pending_completion(Completion::Opened, slots as usize);
    assert_eq!(
        runtime
            .current_thread_state()
            .unwrap()
            .callback_frames
            .len(),
        slots as usize
    );
    let error = assert_completion_rejected(
        &mut runtime,
        &pending,
        response(Completion::Opened, &pending),
    );
    assert!(error.message.contains("scratch depth"));
}

#[test]
fn media_wrong_held_token_and_native_callee_retain_open_frame_and_end_then_retry() {
    for completion in [Completion::Opened, Completion::Frame, Completion::Ended] {
        let (mut runtime, pending) = pending_completion(completion, 0);
        let wrong = ComMediaDeferred {
            request_id: pending.request_id + 1,
            graph_id: pending.graph_id,
            thread_id: pending.thread_id,
            operation: pending.operation,
            resource_id: pending.resource_id,
            source: pending.source.clone(),
            volume: pending.volume,
            delay_milliseconds: pending.delay_milliseconds,
            audio: pending.audio.clone(),
        };
        assert_completion_rejected(&mut runtime, &wrong, response(completion, &pending));
        let slot = match completion {
            Completion::Opened => 10,
            Completion::Frame => 6,
            Completion::Ended => 14,
        };
        let address = GuestAddress(VTABLE.0 + slot * 4);
        let target = runtime.memory.read(address, 4).unwrap();
        runtime.memory.write(address, &0_u32.to_le_bytes()).unwrap();
        acknowledge(&mut runtime);
        assert_completion_rejected(&mut runtime, &pending, response(completion, &pending));
        runtime.memory.write(address, &target).unwrap();
        acknowledge(&mut runtime);
        assert_retry(&mut runtime, &pending, completion, 1);
    }
}

fn assert_completion_rejected(
    runtime: &mut Runtime,
    pending: &ComMediaDeferred,
    response: ComMediaResponse,
) -> RuntimeStateError {
    let memory = runtime.memory.current_reflection().unwrap();
    let stats = runtime.memory.stats();
    let allocations = runtime.process_address_space_allocator().clone();
    let scheduler = runtime.scheduler().clone();
    let thread = runtime.current_thread_state().unwrap().clone();
    let direct_show = format!("{:?}", runtime.direct_show);
    let com = format!("{:?}", runtime.com());
    let error = runtime
        .complete_com_media(pending, response)
        .expect_err("invalid media callback must retain its actual host wait");
    assert_eq!(runtime.memory.current_reflection().unwrap(), memory);
    assert_eq!(runtime.memory.stats(), stats);
    assert_eq!(runtime.process_address_space_allocator(), &allocations);
    assert_eq!(runtime.scheduler(), &scheduler);
    assert_eq!(runtime.current_thread_state().unwrap(), &thread);
    assert_eq!(format!("{:?}", runtime.direct_show), direct_show);
    assert_eq!(format!("{:?}", runtime.com()), com);
    assert_eq!(runtime.current_last_error().unwrap(), LAST_ERROR);
    error
}

fn assert_retry(
    runtime: &mut Runtime,
    pending: &ComMediaDeferred,
    completion: Completion,
    expected_id: u64,
) {
    let outcome = runtime
        .complete_com_media(pending, response(completion, pending))
        .unwrap();
    let ComDispatchOutcome::GuestCallback(callback) = outcome else {
        panic!("repaired held completion must enter its guest callback: {outcome:?}");
    };
    assert_eq!(callback.frame_id.0, expected_id);
    assert!(!runtime.direct_show.active.contains_key(&pending.request_id));
    assert_eq!(runtime.direct_show.callbacks.len(), 1);
    assert_eq!(runtime.current_last_error().unwrap(), LAST_ERROR);
    match completion {
        Completion::Opened => {
            assert_eq!(
                runtime.direct_show.graphs[&pending.graph_id].format,
                Some(format(pending.request_id))
            );
            assert!(runtime.direct_show.samples.is_empty());
        }
        Completion::Frame => {
            assert_eq!(runtime.direct_show.samples.len(), 1);
            let sample = runtime.direct_show.samples.values().next().unwrap();
            assert_eq!(
                runtime
                    .memory
                    .read_owned_host(sample.backing.range.start(), 16, sample.backing.owner)
                    .unwrap(),
                vec![0x55; 16]
            );
        }
        Completion::Ended => assert!(runtime.direct_show.samples.is_empty()),
    }
}

fn response(completion: Completion, pending: &ComMediaDeferred) -> ComMediaResponse {
    match completion {
        Completion::Opened => ComMediaResponse::Opened(format(pending.request_id)),
        Completion::Frame => ComMediaResponse::Frame(ComMediaFrame {
            resource_id: 99,
            start_100ns: 0,
            end_100ns: 333_333,
            bgra: vec![0x55; 16],
        }),
        Completion::Ended => ComMediaResponse::Ended { resource_id: 99 },
    }
}

fn format(resource_id: u64) -> ComMediaFormat {
    ComMediaFormat {
        resource_id,
        width: 2,
        height: 2,
        duration_100ns: Some(1_000_000),
        frame_duration_100ns: 333_333,
        audio_channels: 0,
        audio_samples_per_second: 0,
    }
}

fn pending_completion(completion: Completion, nested: usize) -> (Runtime, ComMediaDeferred) {
    let mut runtime = runtime();
    let graph = runtime
        .install_com_object_with_guest_writes(None, filter_graph_spec(), |_| Ok(Vec::new()))
        .unwrap();
    let interface = runtime
        .com()
        .interface_for_object_iid(graph.object_id, IID_GRAPH_BUILDER)
        .unwrap()
        .clone();
    let target = runtime
        .resolve_com_method(interface.methods[&12].endpoint)
        .unwrap()
        .unwrap();
    let stack = runtime
        .thread_stack(runtime.current_thread_id())
        .unwrap()
        .clone();
    let stack_pointer = GuestAddress(stack.initial_stack_pointer.0 - 128);
    let return_address = stack.return_sentinel;
    let words = [return_address.0, interface.guest_pointer.0, RENDERER.0];
    runtime
        .memory
        .write(
            stack_pointer,
            &words
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        )
        .unwrap();
    let origin = ComOrigin {
        trap: ComCallTrap {
            endpoint_address: interface.methods[&12].endpoint,
            stack_pointer,
            return_address,
            call_site: return_address,
        },
        target,
        frame: ComCallFrame {
            object_id: graph.object_id,
            interface_id: interface.id,
            iid: interface.iid,
            vtable_slot: 12,
            return_stack_pointer: GuestAddress(stack_pointer.0 + 12),
            return_address,
            explicit_arguments: vec![RENDERER.0],
        },
    };
    runtime
        .direct_show
        .graphs
        .get_mut(&graph.object_id)
        .unwrap()
        .filters
        .push(GuestFilter { pointer: RENDERER });
    if !matches!(completion, Completion::Opened) {
        let allocator = runtime
            .install_com_object_with_guest_writes(Some(interface.id), allocator_spec(), |_| {
                Ok(Vec::new())
            })
            .unwrap();
        runtime.direct_show.allocators.insert(
            allocator.object_id,
            MediaAllocator {
                graph_id: graph.object_id,
                properties: [2, 16, 1, 0],
                committed: false,
                free: Vec::new(),
            },
        );
        runtime.commit_media_allocator(allocator.object_id).unwrap();
        let state = runtime
            .direct_show
            .graphs
            .get_mut(&graph.object_id)
            .unwrap();
        state.allocator = Some(allocator.object_id);
        state.input_pin = Some(RENDERER);
        state.mem_input = Some(RENDERER);
        state.format = Some(format(99));
        state.state = GraphState::Running;
    }
    for _ in 0..nested {
        runtime
            .push_callback_frame(return_address, crate::callbacks::CALLBACK_RETURN_ARENA_BASE)
            .unwrap();
    }
    acknowledge(&mut runtime);
    let operation = if matches!(completion, Completion::Opened) {
        ComMediaOperation::Open
    } else {
        ComMediaOperation::NextFrame
    };
    let ComDispatchOutcome::MediaPending(pending) = runtime
        .begin_media_call(
            origin,
            operation,
            Some(ComMediaSource::GuestAsyncReader {
                pointer: RENDERER,
                byte_length: 4096,
            }),
        )
        .unwrap()
    else {
        unreachable!()
    };
    (runtime, pending)
}

fn runtime() -> Runtime {
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
                bytes_with_nul: b"C:\\Vine\\media-test.exe\0".to_vec(),
            },
            initial_current_directory_a: RuntimeCurrentDirectoryA {
                bytes_with_nul: b"C:\\Vine\0".to_vec(),
            },
            encoded_pointer_cookie: 0xec01_9d3b,
        },
        GuestMemory::new(MemoryLimits::default()).unwrap(),
    )
    .unwrap();
    let allocation = runtime
        .process_address_space_allocator()
        .allocation_for_purpose(&ProcessAddressAllocationPurpose::CallbackReturns)
        .unwrap()
        .clone();
    let sentinels =
        crate::install_runtime_return_sentinels(&mut runtime.memory, &allocation).unwrap();
    let stack = crate::install_initial_thread_stack(
        &mut runtime.memory,
        &sentinels.process_entry_return,
        1024 * 1024,
        64 * 1024,
    )
    .unwrap();
    runtime.register_initial_thread_stack(&stack).unwrap();
    runtime.start_initial_thread().unwrap();
    let owner = runtime
        .memory
        .create_owner(MemoryOwnerKind::Runtime)
        .unwrap();
    let range = runtime
        .memory
        .reserve_fixed(owner, MemoryRegionKind::Runtime, DATA, 4096)
        .unwrap();
    runtime.memory.commit(owner, range, writable()).unwrap();
    runtime
        .memory
        .write(RENDERER, &VTABLE.0.to_le_bytes())
        .unwrap();
    for slot in [6, 10, 14] {
        runtime
            .memory
            .write(
                GuestAddress(VTABLE.0 + slot * 4),
                &sentinels.process_entry_return.address.0.to_le_bytes(),
            )
            .unwrap();
    }
    let reflection = runtime.memory.pending_reflection().unwrap();
    runtime
        .memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
    runtime
}

fn writable() -> PageProtection {
    PageProtection {
        read: true,
        write: true,
        execute: false,
    }
}
fn read_only() -> PageProtection {
    PageProtection {
        read: true,
        write: false,
        execute: false,
    }
}
fn acknowledge(runtime: &mut Runtime) {
    if runtime.memory.stats().pending_invalidation.is_some() {
        let delta = runtime.memory.pending_delta().unwrap();
        runtime.memory.acknowledge_delta(delta.generation).unwrap();
    }
}

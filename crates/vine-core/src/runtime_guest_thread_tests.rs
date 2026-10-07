use crate as vine_core;
use crate::runtime::RuntimeCreateThreadRequest;
use crate::win32::dispatch_kernel32_api;

use vine_core::{
    ApiDispatcher, ApiEndpointBinding, ApiEndpointIdentity, ApiOwnedTrap, ApiSymbol,
    DispatchOutcome, GUEST_PAGE_SIZE, GuestAddress, GuestMemory, ImportThunkDisposition,
    ImportThunkId, MappingOperation, MemoryLimits, MemoryOwnerKind, MemoryRegionKind,
    PageProtection, ProcessAddressAllocationPurpose, Runtime, RuntimeApiBoundaryDisposition,
    RuntimeBootstrap, RuntimeClockState, RuntimeCurrentDirectoryA, RuntimeCursorPositionProfile,
    RuntimeHandle, RuntimeInputProfile, RuntimeObjectKind, RuntimeProcessId,
    RuntimeProcessImagePathA, RuntimeStartupInfo, RuntimeThreadId, RuntimeThreadPriority,
    RuntimeWaitState, SchedulerThreadState, X86_STARTUP_INFO_W_BYTE_LENGTH,
    install_initial_thread_stack, install_runtime_return_sentinels,
};

const PROCESS_ID: u32 = 4096;
const INITIAL_THREAD_ID: RuntimeThreadId = RuntimeThreadId(8192);
const ENTRY: GuestAddress = GuestAddress(0x1000_0000);
const OUTPUT: GuestAddress = GuestAddress(0x2000_0000);

#[test]
fn exit_thread_api_terminates_dynamic_guest_thread() {
    const EXIT_CODE: u32 = 0x1357_9bdf;
    const RETURN_ADDRESS: GuestAddress = GuestAddress(0x1000_0040);
    const ENDPOINT: GuestAddress = GuestAddress(0x7f00_1010);
    const FRAME: GuestAddress = GuestAddress(0x2000_0100);
    const IAT: GuestAddress = GuestAddress(0x2000_0200);

    let mut runtime = prepared_runtime();
    let created = runtime
        .create_guest_thread(RuntimeCreateThreadRequest {
            stack_reserve_bytes: 0x0010_0000,
            stack_commit_bytes: GUEST_PAGE_SIZE,
            entry_point: ENTRY,
            parameter: OUTPUT.0,
            thread_id_output: None,
            initial_suspend_count: 0,
        })
        .expect("create runnable guest thread")
        .expect("guest thread capacity");
    let created_stack = runtime.thread_stack(created.0).unwrap().clone();
    acknowledge_pending_delta(&mut runtime.memory);
    while runtime.pop_scheduler_event().is_some() {}
    runtime
        .select_thread(created.0)
        .expect("select dynamic guest thread");

    let frame_bytes = [RETURN_ADDRESS.0, EXIT_CODE]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    let mut transaction = runtime.memory.begin_transaction();
    transaction.write(FRAME, &frame_bytes).unwrap();
    transaction.write(IAT, &ENDPOINT.0.to_le_bytes()).unwrap();
    transaction.commit_transaction();
    acknowledge_pending_delta(&mut runtime.memory);

    let endpoint = ApiEndpointIdentity {
        id: ImportThunkId(1),
        address: ENDPOINT,
        module: "kernel32.dll".to_owned(),
        symbol: ApiSymbol::Name {
            name: "ExitThread".to_owned(),
        },
        iat_address: Some(IAT),
        disposition: Some(ImportThunkDisposition::DeclaredRuntimeExport),
        binding: ApiEndpointBinding::ImportAddressTable,
    };
    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap();
    let last_error_before_dispatch = runtime.current_last_error().unwrap();
    let outcome = dispatcher.dispatch_owned(
        ApiOwnedTrap {
            endpoint_id: endpoint.id,
            endpoint_address: endpoint.address,
            stack_pointer: FRAME,
            return_address: RETURN_ADDRESS,
        },
        &mut runtime,
        None,
        dispatch_kernel32_api,
    );
    let DispatchOutcome::Immediate(completion) = outcome else {
        panic!("ExitThread did not produce a terminating completion: {outcome:?}");
    };
    assert_eq!(completion.eax, Some(EXIT_CODE));
    assert_eq!(completion.stack_pointer, GuestAddress(FRAME.0 + 8));
    assert_eq!(
        completion.execution_boundary,
        RuntimeApiBoundaryDisposition::TerminateCurrentThread
    );
    assert_eq!(runtime.current_thread_id(), created.0);
    assert_eq!(
        runtime.current_last_error().unwrap(),
        last_error_before_dispatch
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.thread_stack(created.0).unwrap(), &created_stack);
    assert_eq!(
        runtime
            .object_wait_state(created.1, RuntimeObjectKind::Thread)
            .unwrap(),
        RuntimeWaitState::Nonsignaled
    );
    assert!(runtime.pop_scheduler_event().is_none());

    runtime
        .configure_input_profile(RuntimeInputProfile {
            initial_mouse_buttons_swapped: false,
            initial_keyboard_source_focused: None,
            cursor_position: Some(RuntimeCursorPositionProfile {
                initial_screen_x: 960,
                initial_screen_y: 540,
            }),
        })
        .unwrap();
    for message in [0x0401, 0x0402] {
        runtime
            .post_thread_message(created.0, 0, message, 0, 0)
            .expect("queue a message before ExitThread");
    }
    runtime.request_current_thread_exit(EXIT_CODE).unwrap();
    assert!(
        runtime
            .queued_thread_messages(created.0)
            .unwrap()
            .is_empty()
    );
    runtime.complete_current_guest_thread_exit().unwrap();
    assert_eq!(
        runtime.scheduler_state_for_thread(created.0).unwrap(),
        SchedulerThreadState::Terminated {
            exit_code: EXIT_CODE
        }
    );
    assert_eq!(
        runtime
            .object_wait_state(created.1, RuntimeObjectKind::Thread)
            .unwrap(),
        RuntimeWaitState::Signaled {
            exit_code: EXIT_CODE
        }
    );
}

#[test]
fn suspended_guest_thread_materializes_stack_teb_handle_and_resumes() {
    let mut runtime = prepared_runtime();
    let result = runtime
        .create_guest_thread(RuntimeCreateThreadRequest {
            stack_reserve_bytes: 0x0010_0000,
            stack_commit_bytes: GUEST_PAGE_SIZE,
            entry_point: ENTRY,
            parameter: OUTPUT.0,
            thread_id_output: Some(OUTPUT),
            initial_suspend_count: 1,
        })
        .expect("create suspended guest thread")
        .expect("guest thread capacity");
    let result_stack = runtime.thread_stack(result.0).unwrap().clone();
    let result_environment = runtime
        .thread_state(result.0)
        .unwrap()
        .environment
        .as_ref()
        .unwrap()
        .clone();
    let delta = runtime
        .memory
        .pending_delta()
        .expect("guest thread creation delta");
    assert_eq!(delta.mapping_operations.len(), 4);
    runtime.memory.acknowledge_delta(delta.generation).unwrap();

    assert_eq!(result.0, RuntimeThreadId(8193));
    assert_eq!(result.1, RuntimeHandle(0x5401_0002));
    assert_eq!(read_u32(&runtime.memory, OUTPUT), result.0.0);
    assert_eq!(
        result_stack.reserved_range.start(),
        GuestAddress(0x5000_0000)
    );
    assert_eq!(result_stack.reserved_range.byte_length(), 0x0010_0000);
    assert_eq!(result_stack.committed_range.byte_length(), GUEST_PAGE_SIZE);
    assert_eq!(
        result_stack.initial_stack_pointer.0,
        result_stack.stack_top.0 - 8
    );
    assert_eq!(
        read_u32(&runtime.memory, result_stack.initial_stack_pointer),
        result_stack.return_sentinel.0
    );
    assert_eq!(
        read_u32(
            &runtime.memory,
            GuestAddress(result_stack.initial_stack_pointer.0 + 4)
        ),
        OUTPUT.0
    );
    assert_eq!(result_environment.teb_address, GuestAddress(0x7f03_0000));
    assert_eq!(
        read_u32(&runtime.memory, result_environment.teb_address),
        u32::MAX
    );
    assert_eq!(
        read_u32(
            &runtime.memory,
            GuestAddress(result_environment.teb_address.0 + 0x04)
        ),
        result_stack.stack_top.0
    );
    assert_eq!(
        read_u32(
            &runtime.memory,
            GuestAddress(result_environment.teb_address.0 + 0x08)
        ),
        result_stack.committed_range.start().0
    );
    assert_eq!(
        read_u32(
            &runtime.memory,
            GuestAddress(result_environment.teb_address.0 + 0x18)
        ),
        result_environment.teb_address.0
    );
    assert_eq!(
        read_u32(
            &runtime.memory,
            GuestAddress(result_environment.teb_address.0 + 0x20)
        ),
        PROCESS_ID
    );
    assert_eq!(
        read_u32(
            &runtime.memory,
            GuestAddress(result_environment.teb_address.0 + 0x24)
        ),
        result.0.0
    );
    assert_eq!(
        read_u32(
            &runtime.memory,
            GuestAddress(result_environment.teb_address.0 + 0x30)
        ),
        result_environment.peb_address.0
    );
    assert_eq!(
        runtime.scheduler_state_for_thread(result.0).unwrap(),
        SchedulerThreadState::Suspended { count: 1 }
    );
    assert_eq!(
        runtime
            .object_for_handle(result.1, RuntimeObjectKind::Thread)
            .unwrap()
            .open_handles,
        1
    );

    let resumed = runtime.resume_thread(result.1).expect("resume thread");
    assert_eq!(resumed, 1);
    assert_eq!(
        runtime.scheduler_state_for_thread(result.0).unwrap(),
        SchedulerThreadState::Runnable
    );
    let already_runnable = runtime
        .resume_thread(result.1)
        .expect("resume runnable thread");
    assert_eq!(already_runnable, 0);
    assert_eq!(
        runtime.scheduler_state_for_thread(result.0).unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(
        runtime.thread_priority(result.0).unwrap(),
        RuntimeThreadPriority::Normal
    );
    assert!(
        runtime
            .set_thread_priority(result.1, RuntimeThreadPriority::AboveNormal)
            .unwrap()
    );
    assert_eq!(
        runtime.thread_priority(result.0).unwrap(),
        RuntimeThreadPriority::AboveNormal
    );
    assert_eq!(
        runtime.thread_priority(INITIAL_THREAD_ID).unwrap(),
        RuntimeThreadPriority::Normal
    );
    for invalid_handle in [RuntimeHandle(u32::MAX), runtime.process_handle().unwrap()] {
        assert_eq!(runtime.resume_thread(invalid_handle).unwrap(), u32::MAX);
        assert!(
            !runtime
                .set_thread_priority(invalid_handle, RuntimeThreadPriority::Highest)
                .unwrap()
        );
        assert_eq!(
            runtime.thread_priority(result.0).unwrap(),
            RuntimeThreadPriority::AboveNormal
        );
        assert_eq!(
            runtime.thread_priority(INITIAL_THREAD_ID).unwrap(),
            RuntimeThreadPriority::Normal
        );
        assert_eq!(
            runtime.scheduler_state_for_thread(result.0).unwrap(),
            SchedulerThreadState::Runnable
        );
        assert_eq!(
            runtime
                .object_for_handle(result.1, RuntimeObjectKind::Thread)
                .unwrap()
                .open_handles,
            1
        );
    }

    let retired_stack_address = result_stack.reserved_range.start();
    let retired_stack_allocation = result_stack.address_allocation_id;
    let retired_teb_address = result_environment.teb_address;
    let retired_teb_allocation = result_environment.address_allocation_id;
    runtime
        .select_thread(result.0)
        .expect("select dynamically created thread");
    runtime
        .request_current_thread_exit(0x1357_9bdf)
        .expect("request dynamically created thread exit");
    runtime
        .complete_current_guest_thread_exit()
        .expect("retire dynamically created thread backing");
    assert_eq!(
        runtime.scheduler_state_for_thread(result.0).unwrap(),
        SchedulerThreadState::Terminated {
            exit_code: 0x1357_9bdf
        }
    );
    assert_eq!(
        runtime
            .object_wait_state(result.1, RuntimeObjectKind::Thread)
            .unwrap(),
        RuntimeWaitState::Signaled {
            exit_code: 0x1357_9bdf
        }
    );
    assert!(runtime.memory.read(retired_stack_address, 1).is_err());
    assert!(runtime.memory.read(retired_teb_address, 1).is_err());
    assert!(
        runtime
            .process_address_space_allocator()
            .allocation(retired_stack_allocation)
            .is_err()
    );
    assert!(
        runtime
            .process_address_space_allocator()
            .allocation(retired_teb_allocation)
            .is_err()
    );
    let retirement_delta = runtime
        .memory
        .pending_delta()
        .expect("guest thread retirement delta");
    assert_eq!(retirement_delta.mapping_operations.len(), 2);
    assert!(
        retirement_delta
            .mapping_operations
            .iter()
            .all(|operation| matches!(operation, MappingOperation::Release { .. }))
    );
    runtime
        .memory
        .acknowledge_delta(retirement_delta.generation)
        .expect("acknowledge guest thread retirement delta");

    runtime.close_handle(result.1).expect("close thread handle");
    assert_eq!(runtime.resume_thread(result.1).unwrap(), u32::MAX);
    assert!(
        runtime
            .object_for_handle(result.1, RuntimeObjectKind::Thread)
            .is_err()
    );
    assert!(
        !runtime
            .set_thread_priority(result.1, RuntimeThreadPriority::AboveNormal)
            .unwrap()
    );
    runtime
        .select_thread(INITIAL_THREAD_ID)
        .expect("restore surviving initial thread");
    let replacement = runtime
        .create_guest_thread(RuntimeCreateThreadRequest {
            stack_reserve_bytes: 0x0010_0000,
            stack_commit_bytes: GUEST_PAGE_SIZE,
            entry_point: ENTRY,
            parameter: 0,
            thread_id_output: None,
            initial_suspend_count: 0,
        })
        .expect("reuse retired guest thread address ranges")
        .expect("guest thread capacity");
    assert_eq!(replacement.0, RuntimeThreadId(8194));
    assert_eq!(
        runtime
            .thread_stack(replacement.0)
            .unwrap()
            .reserved_range
            .start(),
        retired_stack_address
    );
    assert_eq!(
        runtime
            .thread_state(replacement.0)
            .unwrap()
            .environment
            .as_ref()
            .unwrap()
            .teb_address,
        retired_teb_address
    );
}

#[test]
fn current_api_created_thread_grows_only_its_own_stack_guard_atomically() {
    let mut runtime = prepared_runtime();
    let tls = runtime.allocate_tls_index().unwrap();
    let fls = runtime.allocate_fls_slot(None).unwrap();
    runtime.set_tls_value(tls, 17).unwrap();
    runtime.set_fls_value(fls, 23).unwrap();
    runtime.set_current_last_error(0x1111).unwrap();
    let initial_stack = runtime.thread_stack(INITIAL_THREAD_ID).unwrap().clone();
    let created = runtime
        .create_guest_thread(RuntimeCreateThreadRequest {
            stack_reserve_bytes: 0x0010_0000,
            stack_commit_bytes: GUEST_PAGE_SIZE,
            entry_point: ENTRY,
            parameter: 0,
            thread_id_output: None,
            initial_suspend_count: 0,
        })
        .expect("create runnable guest thread")
        .expect("guest thread capacity");
    let created_stack = runtime.thread_stack(created.0).unwrap().clone();
    let creation_generation = runtime
        .memory
        .pending_delta()
        .expect("guest thread creation delta")
        .generation;
    runtime
        .memory
        .acknowledge_delta(creation_generation)
        .unwrap();
    let guard = created_stack.guard_page.expect("created thread guard page");

    let memory_before_wrong_thread = runtime.memory.current_reflection().unwrap();
    runtime
        .service_page_fault(guard.start(), 4)
        .expect_err("another thread may not consume the created stack guard");
    assert_eq!(runtime.current_thread_id(), INITIAL_THREAD_ID);
    assert_eq!(
        runtime.thread_stack(INITIAL_THREAD_ID).unwrap(),
        &initial_stack
    );
    assert_eq!(runtime.thread_stack(created.0).unwrap(), &created_stack);
    assert_eq!(
        runtime.memory.current_reflection().unwrap(),
        memory_before_wrong_thread
    );

    runtime
        .select_thread(created.0)
        .expect("select API-created thread");
    runtime.set_tls_value(tls, 19).unwrap();
    runtime.set_fls_value(fls, 29).unwrap();
    runtime.set_current_last_error(0x2222).unwrap();
    let memory_before_protection_fault = runtime.memory.current_reflection().unwrap();
    runtime
        .service_page_fault(guard.start(), 5)
        .expect_err("present-page fault must not grow the stack");
    assert_eq!(runtime.current_thread_id(), created.0);
    assert_eq!(
        runtime.thread_stack(INITIAL_THREAD_ID).unwrap(),
        &initial_stack
    );
    assert_eq!(runtime.thread_stack(created.0).unwrap(), &created_stack);
    assert_eq!(
        runtime.memory.current_reflection().unwrap(),
        memory_before_protection_fault
    );

    runtime
        .service_page_fault(GuestAddress(guard.start().0 + 0xd94), 4)
        .expect("grow the selected API-created thread stack");
    assert_eq!(
        runtime.thread_stack(INITIAL_THREAD_ID).unwrap(),
        &initial_stack
    );
    let grown = runtime.thread_stack(created.0).unwrap();
    assert_eq!(grown.committed_range.start(), guard.start());
    assert_eq!(grown.committed_range.byte_length(), 2 * GUEST_PAGE_SIZE);
    assert_eq!(grown.initial_commit_bytes, GUEST_PAGE_SIZE);
    assert_eq!(
        grown.address_allocation_id,
        created_stack.address_allocation_id
    );
    assert_eq!(grown.memory_owner, created_stack.memory_owner);
    assert_eq!(grown.committed_range.page_count(), 2);
    assert_eq!(runtime.tls_value(tls).unwrap(), 19);
    assert_eq!(runtime.fls_value(fls).unwrap(), 29);
    assert_eq!(runtime.current_last_error().unwrap(), 0x2222);
    let initial = runtime.thread_state(INITIAL_THREAD_ID).unwrap();
    assert_eq!(initial.tls_values.get(&tls), Some(&17));
    assert_eq!(initial.fls_values.get(&fls), Some(&23));
    assert_eq!(initial.last_error, 0x1111);
    assert_eq!(
        grown.guard_page.unwrap().start(),
        GuestAddress(guard.start().0 - GUEST_PAGE_SIZE as u32)
    );

    let delta = runtime
        .memory
        .pending_delta()
        .expect("created thread growth delta");
    assert!(matches!(delta.mapping_operations.as_slice(),
        [MappingOperation::Commit { range, owner, .. }]
            if *range == guard && *owner == created_stack.memory_owner));
    assert_eq!(delta.dirty_pages.len(), 1);
    assert_eq!(delta.dirty_pages[0].address, guard.start());
    assert_eq!(delta.dirty_pages[0].owner, created_stack.memory_owner);
    assert!(delta.dirty_pages[0].bytes.iter().all(|byte| *byte == 0));
    let growth_generation = delta.generation;

    let grown_stack = runtime.thread_stack(created.0).unwrap().clone();
    let memory_before_unsynchronized = runtime.memory.current_reflection().unwrap();
    let next_guard = grown_stack.guard_page.expect("remaining stack guard");
    runtime
        .service_page_fault(next_guard.start(), 4)
        .expect_err("unacknowledged memory must reject another growth");
    assert_eq!(runtime.thread_stack(created.0).unwrap(), &grown_stack);
    assert_eq!(
        runtime.thread_stack(INITIAL_THREAD_ID).unwrap(),
        &initial_stack
    );
    assert_eq!(
        runtime.memory.pending_delta().unwrap().generation,
        growth_generation
    );
    assert_eq!(
        runtime.memory.current_reflection().unwrap(),
        memory_before_unsynchronized
    );
    runtime.memory.acknowledge_delta(growth_generation).unwrap();

    let memory_before_stale_guard = runtime.memory.current_reflection().unwrap();
    runtime
        .service_page_fault(guard.start(), 4)
        .expect_err("a committed former guard must be stale");
    assert_eq!(runtime.thread_stack(created.0).unwrap(), &grown_stack);
    assert_eq!(
        runtime.thread_stack(INITIAL_THREAD_ID).unwrap(),
        &initial_stack
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(
        runtime
            .object_wait_state(created.1, RuntimeObjectKind::Thread)
            .unwrap(),
        RuntimeWaitState::Nonsignaled
    );
    assert_eq!(
        runtime.memory.current_reflection().unwrap(),
        memory_before_stale_guard
    );
}

#[test]
fn guest_thread_creation_failures_preserve_memory_and_guest_output_then_retry() {
    let mut runtime = prepared_runtime();
    let initial_stack = runtime.thread_stack(INITIAL_THREAD_ID).unwrap().clone();
    runtime
        .memory
        .write(OUTPUT, &0xdead_beef_u32.to_le_bytes())
        .unwrap();
    acknowledge_pending_delta(&mut runtime.memory);
    let last_error = runtime.current_last_error().unwrap();
    let memory_before = runtime.memory.current_reflection().unwrap();
    runtime
        .create_guest_thread(RuntimeCreateThreadRequest {
            stack_reserve_bytes: 0x0010_0000,
            stack_commit_bytes: GUEST_PAGE_SIZE,
            entry_point: ENTRY,
            parameter: 0,
            thread_id_output: Some(GuestAddress(0x3000_0000)),
            initial_suspend_count: 1,
        })
        .expect_err("unmapped thread ID output must fail");
    assert_eq!(
        runtime.thread_stack(INITIAL_THREAD_ID).unwrap(),
        &initial_stack
    );
    assert_eq!(runtime.current_last_error().unwrap(), last_error);
    assert_eq!(runtime.memory.current_reflection().unwrap(), memory_before);
    assert_eq!(read_u32(&runtime.memory, OUTPUT), 0xdead_beef);

    let unavailable = runtime
        .create_guest_thread(RuntimeCreateThreadRequest {
            stack_reserve_bytes: 0x1000_0000,
            stack_commit_bytes: 0x1000_0000,
            entry_point: ENTRY,
            parameter: 0,
            thread_id_output: Some(OUTPUT),
            initial_suspend_count: 1,
        })
        .expect("valid guest thread request");
    assert!(
        unavailable.is_none(),
        "physical page quota must fail before commit"
    );
    assert_eq!(
        runtime.thread_stack(INITIAL_THREAD_ID).unwrap(),
        &initial_stack
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.current_thread_id(), INITIAL_THREAD_ID);
    assert_eq!(runtime.current_last_error().unwrap(), last_error);
    assert_eq!(runtime.memory.current_reflection().unwrap(), memory_before);
    assert_eq!(read_u32(&runtime.memory, OUTPUT), 0xdead_beef);

    let (thread_id, handle) = runtime
        .create_guest_thread(RuntimeCreateThreadRequest {
            stack_reserve_bytes: 0x0010_0000,
            stack_commit_bytes: GUEST_PAGE_SIZE,
            entry_point: ENTRY,
            parameter: 0,
            thread_id_output: Some(OUTPUT),
            initial_suspend_count: 1,
        })
        .expect("retry after rejected creation")
        .expect("guest thread capacity");
    acknowledge_pending_delta(&mut runtime.memory);
    assert_ne!(thread_id, INITIAL_THREAD_ID);
    assert_eq!(read_u32(&runtime.memory, OUTPUT), thread_id.0);
    assert_eq!(
        runtime
            .object_wait_state(handle, RuntimeObjectKind::Thread)
            .unwrap(),
        RuntimeWaitState::Nonsignaled
    );
    let stack = runtime.thread_stack(thread_id).unwrap();
    assert_eq!(
        read_u32(&runtime.memory, stack.initial_stack_pointer),
        stack.return_sentinel.0
    );
    let teb = runtime
        .thread_state(thread_id)
        .unwrap()
        .environment
        .as_ref()
        .unwrap()
        .teb_address;
    assert_eq!(
        read_u32(&runtime.memory, GuestAddress(teb.0 + 0x24)),
        thread_id.0
    );
}

fn prepared_runtime() -> Runtime {
    let memory = GuestMemory::new(MemoryLimits::default()).unwrap();
    let mut runtime = Runtime::new(runtime_bootstrap(), memory).unwrap();
    let callback_allocation = runtime
        .process_address_space_allocator()
        .allocation_for_purpose(&ProcessAddressAllocationPurpose::CallbackReturns)
        .unwrap()
        .clone();
    let sentinels =
        install_runtime_return_sentinels(&mut runtime.memory, &callback_allocation).unwrap();
    let initial_stack = install_initial_thread_stack(
        &mut runtime.memory,
        &sentinels.process_entry_return,
        0x0010_0000,
        GUEST_PAGE_SIZE,
    )
    .unwrap();
    runtime
        .register_initial_thread_stack(&initial_stack)
        .unwrap();
    runtime
        .install_process_environment("thread-test.exe")
        .unwrap();
    install_page(
        &mut runtime.memory,
        ENTRY,
        PageProtection {
            read: true,
            write: false,
            execute: true,
        },
    );
    install_page(
        &mut runtime.memory,
        OUTPUT,
        PageProtection {
            read: true,
            write: true,
            execute: false,
        },
    );
    runtime.start_initial_thread().unwrap();
    let reflection = runtime.memory.pending_reflection().unwrap();
    runtime
        .memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
    runtime
}

fn install_page(memory: &mut GuestMemory, address: GuestAddress, protection: PageProtection) {
    let owner = memory.create_owner(MemoryOwnerKind::Anonymous).unwrap();
    let range = memory
        .reserve_fixed(owner, MemoryRegionKind::Anonymous, address, GUEST_PAGE_SIZE)
        .unwrap();
    memory.commit(owner, range, protection).unwrap();
    if protection.execute {
        memory
            .write_initialization(owner, address, &[0x90])
            .unwrap();
    }
    memory.seal_owner(owner).unwrap();
}

fn read_u32(memory: &GuestMemory, address: GuestAddress) -> u32 {
    u32::from_le_bytes(memory.read(address, 4).unwrap().try_into().unwrap())
}

fn acknowledge_pending_delta(memory: &mut GuestMemory) {
    let generation = memory.pending_delta().unwrap().generation;
    memory.acknowledge_delta(generation).unwrap();
}

fn runtime_bootstrap() -> RuntimeBootstrap {
    let mut startup_info = vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH];
    startup_info[..4].copy_from_slice(&(X86_STARTUP_INFO_W_BYTE_LENGTH as u32).to_le_bytes());
    RuntimeBootstrap {
        process_id: RuntimeProcessId(PROCESS_ID),
        initial_thread_id: INITIAL_THREAD_ID,
        initial_last_error: 0x5a5a,
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
            performance_frequency: 10_000_000,
        },
        startup_info: RuntimeStartupInfo {
            bytes: startup_info,
        },
        process_image_path_a: RuntimeProcessImagePathA {
            bytes_with_nul: b"C:\\Vine\\thread-test.exe\0".to_vec(),
        },
        initial_current_directory_a: RuntimeCurrentDirectoryA {
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
        },
        encoded_pointer_cookie: 0xec01_9d3b,
    }
}

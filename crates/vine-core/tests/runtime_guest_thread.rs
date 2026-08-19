use vine_core::{
    ApiBehavior, ApiDispatcher, ApiEndpointBinding, ApiEndpointIdentity, ApiOwnedTrap,
    ApiReturnSchema, ApiSymbol, DispatchOutcome, EXIT_THREAD_IMPLEMENTATION, ExecutionSession,
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, ImportThunkDisposition, ImportThunkId,
    MappingOperation, MemoryLimits, MemoryOwnerKind, MemoryRegionKind, PageProtection,
    ProcessAddressAllocationPurpose, ProcessAddressSpaceLayout, RegisterUpdate,
    RuntimeApiBoundaryDisposition, RuntimeBootstrap, RuntimeClockState, RuntimeCreateThreadRequest,
    RuntimeCurrentDirectoryA, RuntimeHandle, RuntimeObjectKind, RuntimePageFaultResolution,
    RuntimeProcessId, RuntimeProcessImagePathA, RuntimeStartupInfo, RuntimeState, RuntimeThreadId,
    RuntimeThreadPriority, RuntimeWaitState, SchedulerThreadState, StackCleanup,
    X86_STARTUP_INFO_W_BYTE_LENGTH, dispatch_kernel32_api, install_initial_thread_stack,
    install_runtime_return_sentinels, kernel32_contract_for_endpoint,
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

    let (mut memory, mut runtime) = prepared_runtime();
    let created = runtime
        .create_guest_thread(
            &mut memory,
            RuntimeCreateThreadRequest {
                stack_reserve_bytes: 0x0010_0000,
                stack_commit_bytes: GUEST_PAGE_SIZE,
                entry_point: ENTRY,
                parameter: OUTPUT.0,
                thread_id_output: None,
                initial_suspend_count: 0,
            },
        )
        .expect("create runnable guest thread");
    acknowledge_pending_delta(&mut memory);
    while runtime.pop_scheduler_event().is_some() {}
    runtime
        .select_thread(created.thread_id)
        .expect("select dynamic guest thread");

    let frame_bytes = [RETURN_ADDRESS.0, EXIT_CODE]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    let mut transaction = memory.begin_transaction();
    transaction.write(FRAME, &frame_bytes).unwrap();
    transaction.write(IAT, &ENDPOINT.0.to_le_bytes()).unwrap();
    transaction.commit_transaction();
    acknowledge_pending_delta(&mut memory);

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
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .unwrap()
        .expect("ExitThread contract");
    assert_eq!(contract.implementation_id, EXIT_THREAD_IMPLEMENTATION);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::Void);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.behavior, ApiBehavior::Immediate);
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).unwrap();
    let before_dispatch = runtime.clone();
    let outcome = dispatcher.dispatch_owned(
        ApiOwnedTrap {
            endpoint_id: endpoint.id,
            endpoint_address: endpoint.address,
            stack_pointer: FRAME,
            return_address: RETURN_ADDRESS,
        },
        &mut ExecutionSession::new(&mut memory, &mut runtime),
        dispatch_kernel32_api,
    );
    let DispatchOutcome::Immediate(completion) = outcome else {
        panic!("ExitThread did not produce a terminating completion: {outcome:?}");
    };
    assert_eq!(completion.eax, RegisterUpdate::Set(EXIT_CODE));
    assert_eq!(completion.stack_pointer, GuestAddress(FRAME.0 + 8));
    assert_eq!(
        completion.execution_boundary,
        RuntimeApiBoundaryDisposition::TerminateCurrentThread
    );
    assert_eq!(runtime, before_dispatch);

    let cancellation = runtime.request_current_thread_exit(EXIT_CODE).unwrap();
    assert_eq!(cancellation.host_requests_cancelled, 0);
    assert_eq!(cancellation.callback_frames_cancelled, 0);
    assert_eq!(cancellation.wait_registrations_cancelled, 0);
    runtime
        .complete_current_guest_thread_exit(&mut memory)
        .unwrap();
    assert_eq!(
        runtime
            .scheduler_state_for_thread(created.thread_id)
            .unwrap(),
        SchedulerThreadState::Terminated {
            exit_code: EXIT_CODE
        }
    );
    assert_eq!(
        runtime
            .object_wait_state(created.handle, RuntimeObjectKind::Thread)
            .unwrap(),
        RuntimeWaitState::Signaled {
            exit_code: EXIT_CODE
        }
    );
}

#[test]
fn suspended_guest_thread_materializes_stack_teb_handle_and_resumes() {
    let (mut memory, mut runtime) = prepared_runtime();
    let result = runtime
        .create_guest_thread(
            &mut memory,
            RuntimeCreateThreadRequest {
                stack_reserve_bytes: 0x0010_0000,
                stack_commit_bytes: GUEST_PAGE_SIZE,
                entry_point: ENTRY,
                parameter: OUTPUT.0,
                thread_id_output: Some(OUTPUT),
                initial_suspend_count: 1,
            },
        )
        .expect("create suspended guest thread");
    let delta = memory.pending_delta().expect("guest thread creation delta");
    assert_eq!(delta.mapping_operations.len(), 4);
    memory.acknowledge_delta(delta.generation).unwrap();

    assert_eq!(result.thread_id, RuntimeThreadId(8193));
    assert_eq!(result.handle, RuntimeHandle(0x5401_0002));
    assert_eq!(result.thread_id_output, Some(OUTPUT));
    assert_eq!(result.thread_id_write.as_ref().unwrap().bytes_written, 4);
    assert_eq!(read_u32(&memory, OUTPUT), result.thread_id.0);
    assert_eq!(
        result.stack.reserved_range.start(),
        GuestAddress(0x5000_0000)
    );
    assert_eq!(result.stack.reserved_range.byte_length(), 0x0010_0000);
    assert_eq!(result.stack.committed_range.byte_length(), GUEST_PAGE_SIZE);
    assert_eq!(
        result.stack.initial_stack_pointer.0,
        result.stack.stack_top.0 - 8
    );
    assert_eq!(
        read_u32(&memory, result.stack.initial_stack_pointer),
        result.stack.return_sentinel.0
    );
    assert_eq!(
        read_u32(
            &memory,
            GuestAddress(result.stack.initial_stack_pointer.0 + 4)
        ),
        OUTPUT.0
    );
    assert_eq!(result.environment.teb_address, GuestAddress(0x7f03_0000));
    assert_eq!(read_u32(&memory, result.environment.teb_address), u32::MAX);
    assert_eq!(
        read_u32(
            &memory,
            GuestAddress(result.environment.teb_address.0 + 0x04)
        ),
        result.stack.stack_top.0
    );
    assert_eq!(
        read_u32(
            &memory,
            GuestAddress(result.environment.teb_address.0 + 0x08)
        ),
        result.stack.committed_range.start().0
    );
    assert_eq!(
        read_u32(
            &memory,
            GuestAddress(result.environment.teb_address.0 + 0x18)
        ),
        result.environment.teb_address.0
    );
    assert_eq!(
        read_u32(
            &memory,
            GuestAddress(result.environment.teb_address.0 + 0x20)
        ),
        PROCESS_ID
    );
    assert_eq!(
        read_u32(
            &memory,
            GuestAddress(result.environment.teb_address.0 + 0x24)
        ),
        result.thread_id.0
    );
    assert_eq!(
        read_u32(
            &memory,
            GuestAddress(result.environment.teb_address.0 + 0x30)
        ),
        result.environment.peb_address.0
    );
    assert_eq!(
        runtime
            .scheduler_state_for_thread(result.thread_id)
            .unwrap(),
        SchedulerThreadState::Suspended { count: 1 }
    );
    assert_eq!(
        runtime
            .object_for_handle(result.handle, RuntimeObjectKind::Thread)
            .unwrap()
            .open_handles,
        1
    );

    let resumed = runtime.resume_thread(result.handle).expect("resume thread");
    assert_eq!(resumed.previous_suspend_count, 1);
    assert_eq!(resumed.remaining_suspend_count, 0);
    assert!(resumed.became_runnable);
    assert_eq!(
        runtime
            .scheduler_state_for_thread(result.thread_id)
            .unwrap(),
        SchedulerThreadState::Runnable
    );
    let already_runnable = runtime
        .resume_thread(result.handle)
        .expect("resume runnable thread");
    assert_eq!(already_runnable.previous_suspend_count, 0);
    assert!(!already_runnable.became_runnable);
    assert_eq!(
        runtime.thread_priority(result.thread_id).unwrap(),
        RuntimeThreadPriority::Normal
    );
    assert_eq!(
        runtime
            .set_thread_priority(result.handle, RuntimeThreadPriority::AboveNormal)
            .unwrap(),
        RuntimeThreadPriority::Normal
    );
    assert_eq!(
        runtime.thread_priority(result.thread_id).unwrap(),
        RuntimeThreadPriority::AboveNormal
    );
    assert_eq!(
        runtime.thread_priority(INITIAL_THREAD_ID).unwrap(),
        RuntimeThreadPriority::Normal
    );
    let before_invalid_handle = runtime.clone();
    assert!(
        runtime
            .set_thread_priority(RuntimeHandle(u32::MAX), RuntimeThreadPriority::Highest)
            .is_err()
    );
    assert_eq!(runtime, before_invalid_handle);
    let process_handle = runtime.process_handle().unwrap();
    let before_wrong_kind = runtime.clone();
    assert!(
        runtime
            .set_thread_priority(process_handle, RuntimeThreadPriority::Highest)
            .is_err()
    );
    assert_eq!(runtime, before_wrong_kind);

    let retired_stack_address = result.stack.reserved_range.start();
    let retired_stack_allocation = result.stack.address_allocation_id;
    let retired_teb_address = result.environment.teb_address;
    let retired_teb_allocation = result.environment.address_allocation_id;
    runtime
        .select_thread(result.thread_id)
        .expect("select dynamically created thread");
    runtime
        .request_current_thread_exit(0x1357_9bdf)
        .expect("request dynamically created thread exit");
    runtime
        .complete_current_guest_thread_exit(&mut memory)
        .expect("retire dynamically created thread backing");
    assert_eq!(
        runtime
            .scheduler_state_for_thread(result.thread_id)
            .unwrap(),
        SchedulerThreadState::Terminated {
            exit_code: 0x1357_9bdf
        }
    );
    assert_eq!(
        runtime
            .object_wait_state(result.handle, RuntimeObjectKind::Thread)
            .unwrap(),
        RuntimeWaitState::Signaled {
            exit_code: 0x1357_9bdf
        }
    );
    assert!(memory.read(retired_stack_address, 1).is_err());
    assert!(memory.read(retired_teb_address, 1).is_err());
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
    let retirement_delta = memory
        .pending_delta()
        .expect("guest thread retirement delta");
    assert_eq!(retirement_delta.mapping_operations.len(), 2);
    assert!(
        retirement_delta
            .mapping_operations
            .iter()
            .all(|operation| matches!(operation, MappingOperation::Release { .. }))
    );
    memory
        .acknowledge_delta(retirement_delta.generation)
        .expect("acknowledge guest thread retirement delta");

    runtime
        .close_handle(result.handle)
        .expect("close thread handle");
    assert!(
        runtime
            .object_for_handle(result.handle, RuntimeObjectKind::Thread)
            .is_err()
    );
    assert!(
        runtime
            .set_thread_priority(result.handle, RuntimeThreadPriority::AboveNormal)
            .is_err()
    );
    runtime
        .select_thread(INITIAL_THREAD_ID)
        .expect("restore surviving initial thread");
    let replacement = runtime
        .create_guest_thread(
            &mut memory,
            RuntimeCreateThreadRequest {
                stack_reserve_bytes: 0x0010_0000,
                stack_commit_bytes: GUEST_PAGE_SIZE,
                entry_point: ENTRY,
                parameter: 0,
                thread_id_output: None,
                initial_suspend_count: 0,
            },
        )
        .expect("reuse retired guest thread address ranges");
    assert_eq!(replacement.thread_id, RuntimeThreadId(8194));
    assert_eq!(
        replacement.stack.reserved_range.start(),
        retired_stack_address
    );
    assert_eq!(replacement.environment.teb_address, retired_teb_address);
}

#[test]
fn current_api_created_thread_grows_only_its_own_stack_guard_atomically() {
    let (mut memory, mut runtime) = prepared_runtime();
    let initial_stack = runtime.thread_stack(INITIAL_THREAD_ID).unwrap().clone();
    let created = runtime
        .create_guest_thread(
            &mut memory,
            RuntimeCreateThreadRequest {
                stack_reserve_bytes: 0x0010_0000,
                stack_commit_bytes: GUEST_PAGE_SIZE,
                entry_point: ENTRY,
                parameter: 0,
                thread_id_output: None,
                initial_suspend_count: 0,
            },
        )
        .expect("create runnable guest thread");
    let creation_generation = memory
        .pending_delta()
        .expect("guest thread creation delta")
        .generation;
    memory.acknowledge_delta(creation_generation).unwrap();
    let guard = created.stack.guard_page.expect("created thread guard page");

    let runtime_before_wrong_thread = runtime.clone();
    let memory_before_wrong_thread = memory.current_reflection().unwrap();
    let wrong_thread = runtime
        .service_page_fault(&mut memory, guard.start(), 4)
        .expect_err("another thread may not consume the created stack guard");
    assert_eq!(wrong_thread.code, "guest-page-fault-unhandled");
    assert_eq!(runtime, runtime_before_wrong_thread);
    assert_eq!(
        memory.current_reflection().unwrap(),
        memory_before_wrong_thread
    );

    runtime
        .select_thread(created.thread_id)
        .expect("select API-created thread");
    let runtime_before_protection_fault = runtime.clone();
    let memory_before_protection_fault = memory.current_reflection().unwrap();
    let protection_fault = runtime
        .service_page_fault(&mut memory, guard.start(), 5)
        .expect_err("present-page fault must not grow the stack");
    assert_eq!(protection_fault.code, "thread-stack-protection-fault");
    assert_eq!(runtime, runtime_before_protection_fault);
    assert_eq!(
        memory.current_reflection().unwrap(),
        memory_before_protection_fault
    );

    let growth = match runtime
        .service_page_fault(&mut memory, GuestAddress(guard.start().0 + 0xd94), 4)
        .expect("grow the selected API-created thread stack")
    {
        RuntimePageFaultResolution::ThreadStackGrowth { growth } => growth,
        other => panic!("unexpected page-fault resolution: {other:?}"),
    };
    assert_eq!(growth.thread_id, created.thread_id);
    assert_eq!(
        growth.address_allocation_id,
        created.stack.address_allocation_id
    );
    assert_eq!(growth.memory_owner, created.stack.memory_owner);
    assert_eq!(growth.committed_page, guard);
    assert_eq!(growth.previous_guard_page, guard);
    assert_eq!(growth.committed_pages, 2);
    assert_eq!(
        runtime.thread_stack(INITIAL_THREAD_ID).unwrap(),
        &initial_stack
    );
    let grown = runtime.thread_stack(created.thread_id).unwrap();
    assert_eq!(grown.committed_range.start(), guard.start());
    assert_eq!(grown.committed_range.byte_length(), 2 * GUEST_PAGE_SIZE);
    assert_eq!(grown.guard_page, growth.next_guard_page);

    let delta = memory.pending_delta().expect("created thread growth delta");
    assert_eq!(delta.mapping_operations.len(), 1);
    assert_eq!(delta.dirty_pages.len(), 1);
    assert_eq!(delta.dirty_pages[0].address, guard.start());
    assert_eq!(delta.dirty_pages[0].owner, created.stack.memory_owner);
    assert!(delta.dirty_pages[0].bytes.iter().all(|byte| *byte == 0));
    let growth_generation = delta.generation;

    let runtime_before_unsynchronized = runtime.clone();
    let memory_before_unsynchronized = memory.current_reflection().unwrap();
    let next_guard = growth.next_guard_page.expect("remaining stack guard");
    let unsynchronized = runtime
        .grow_current_thread_stack(&mut memory, next_guard.start(), 4)
        .expect_err("unacknowledged memory must reject another growth");
    assert_eq!(
        unsynchronized.code,
        "thread-stack-growth-memory-unsynchronized"
    );
    assert_eq!(runtime, runtime_before_unsynchronized);
    assert_eq!(
        memory.current_reflection().unwrap(),
        memory_before_unsynchronized
    );
    memory.acknowledge_delta(growth_generation).unwrap();

    let runtime_before_stale_guard = runtime.clone();
    let memory_before_stale_guard = memory.current_reflection().unwrap();
    let stale_guard = runtime
        .service_page_fault(&mut memory, guard.start(), 4)
        .expect_err("a committed former guard must be stale");
    assert_eq!(stale_guard.code, "guest-page-fault-unhandled");
    assert_eq!(runtime, runtime_before_stale_guard);
    assert_eq!(
        memory.current_reflection().unwrap(),
        memory_before_stale_guard
    );
}

#[test]
fn guest_thread_creation_failures_preserve_memory_allocators_and_handle_generation() {
    let (mut memory, mut runtime) = prepared_runtime();
    let runtime_before = runtime.clone();
    let memory_before = memory.current_reflection().unwrap();
    let invalid_output = runtime
        .create_guest_thread(
            &mut memory,
            RuntimeCreateThreadRequest {
                stack_reserve_bytes: 0x0010_0000,
                stack_commit_bytes: GUEST_PAGE_SIZE,
                entry_point: ENTRY,
                parameter: 0,
                thread_id_output: Some(GuestAddress(0x3000_0000)),
                initial_suspend_count: 1,
            },
        )
        .expect_err("unmapped thread ID output must fail");
    assert_eq!(invalid_output.code, "guest-page-fault");
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.current_reflection().unwrap(), memory_before);

    let runtime_before = runtime.clone();
    let memory_before = memory.current_reflection().unwrap();
    let quota = runtime
        .create_guest_thread(
            &mut memory,
            RuntimeCreateThreadRequest {
                stack_reserve_bytes: 0x1000_0000,
                stack_commit_bytes: 0x1000_0000,
                entry_point: ENTRY,
                parameter: 0,
                thread_id_output: Some(OUTPUT),
                initial_suspend_count: 1,
            },
        )
        .expect_err("physical page quota must fail before commit");
    assert_eq!(quota.code, "physical-page-quota-exceeded");
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.current_reflection().unwrap(), memory_before);
    assert_eq!(read_u32(&memory, OUTPUT), 0);

    let result = runtime
        .create_guest_thread(
            &mut memory,
            RuntimeCreateThreadRequest {
                stack_reserve_bytes: 0x0010_0000,
                stack_commit_bytes: GUEST_PAGE_SIZE,
                entry_point: ENTRY,
                parameter: 0,
                thread_id_output: Some(OUTPUT),
                initial_suspend_count: 1,
            },
        )
        .expect("retry after rejected plans");
    assert_eq!(result.thread_id, RuntimeThreadId(8193));
    assert_eq!(result.handle, RuntimeHandle(0x5401_0002));
    assert_eq!(
        result.stack.reserved_range.start(),
        GuestAddress(0x5000_0000)
    );
    assert_eq!(result.environment.teb_address, GuestAddress(0x7f03_0000));
}

fn prepared_runtime() -> (GuestMemory, RuntimeState) {
    let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
    let mut runtime = RuntimeState::new(runtime_bootstrap()).unwrap();
    let callback_allocation = runtime
        .process_address_space_allocator()
        .allocation_for_purpose(&ProcessAddressAllocationPurpose::CallbackReturns)
        .unwrap()
        .clone();
    let sentinels = install_runtime_return_sentinels(&mut memory, &callback_allocation).unwrap();
    let initial_stack = install_initial_thread_stack(
        &mut memory,
        &sentinels.process_entry_return,
        0x0010_0000,
        GUEST_PAGE_SIZE,
    )
    .unwrap();
    runtime
        .register_initial_thread_stack(&initial_stack)
        .unwrap();
    ExecutionSession::new(&mut memory, &mut runtime)
        .install_controlled_process_environment_with_command_line_and_ansi("thread-test.exe")
        .unwrap();
    install_page(
        &mut memory,
        ENTRY,
        PageProtection {
            read: true,
            write: false,
            execute: true,
        },
    );
    install_page(
        &mut memory,
        OUTPUT,
        PageProtection {
            read: true,
            write: true,
            execute: false,
        },
    );
    runtime.start_initial_thread().unwrap();
    let reflection = memory.pending_reflection().unwrap();
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
    (memory, runtime)
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
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
            performance_frequency: 10_000_000,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "runtime-guest-thread-test-v1".to_owned(),
            bytes: startup_info,
        },
        process_image_path_a: RuntimeProcessImagePathA {
            source_authority: "runtime-guest-thread-test".to_owned(),
            bytes_with_nul: b"C:\\Vine\\thread-test.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: RuntimeCurrentDirectoryA {
            source_authority: "runtime-guest-thread-process-bootstrap".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: vine_core::LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_cookie: 0xec01_9d3b,
    }
}

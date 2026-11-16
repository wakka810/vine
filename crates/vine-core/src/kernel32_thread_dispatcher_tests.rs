use crate as vine_core;
use crate::api::ApiSymbol;
use crate::imports::{ImportThunk, ImportThunkDisposition, ImportThunkId};
use crate::runtime::{RuntimeObjectKind, RuntimeThreadPriority, X86_STARTUP_INFO_W_BYTE_LENGTH};
use crate::win32::dispatch_kernel32_api;
use vine_core::{
    ApiDispatcher, ApiEndpointIdentity, ApiOwnedTrap, DispatchOutcome, GUEST_PAGE_SIZE,
    GuestAddress, GuestMemory, ImportTarget, MemoryLimits, MemoryOwnerId, MemoryOwnerKind,
    MemoryRegionKind, PageProtection, ProcessAddressAllocationPurpose, Runtime, RuntimeBootstrap,
    RuntimeClockState, RuntimeCurrentDirectoryA, RuntimeProcessId, RuntimeProcessImagePathA,
    RuntimeStartupInfo, RuntimeThreadId, SchedulerThreadState, install_initial_thread_stack,
    install_runtime_return_sentinels,
};

const PROCESS_ID: u32 = 4096;
const INITIAL_THREAD_ID: RuntimeThreadId = RuntimeThreadId(8192);
const ENTRY: GuestAddress = GuestAddress(0x1000_0000);
const DATA: GuestAddress = GuestAddress(0x2000_0000);
const CREATE_STACK: GuestAddress = GuestAddress(0x2000_0100);
const RESUME_STACK: GuestAddress = GuestAddress(0x2000_0140);
const SET_PRIORITY_STACK: GuestAddress = GuestAddress(0x2000_0160);
const INVALID_PRIORITY_STACK: GuestAddress = GuestAddress(0x2000_0180);
const CLOSE_STACK: GuestAddress = GuestAddress(0x2000_01a0);
const CLOSED_PRIORITY_STACK: GuestAddress = GuestAddress(0x2000_01c0);
const INVALID_RESUME_STACK: GuestAddress = GuestAddress(0x2000_01e0);
const PSEUDO_PRIORITY_STACK: GuestAddress = GuestAddress(0x2000_0200);
const GET_EXIT_CODE_STACK: GuestAddress = GuestAddress(0x2000_0220);
/// The value `GetCurrentThread()` returns; it always designates the calling thread.
const CURRENT_THREAD_PSEUDO_HANDLE: u32 = 0xffff_fffe;

#[test]
fn exit_code_query_accepts_current_pseudo_handle_and_rejects_invalid_handles_without_output() {
    let (mut runtime, owner) = prepared_runtime();
    let get_exit = endpoint(owner, 1, 0x7f00_0310, DATA.0 + 0x800, "GetExitCodeThread");
    runtime
        .memory
        .write_initialization(
            owner,
            GuestAddress(DATA.0 + 0x800),
            &get_exit.address.0.to_le_bytes(),
        )
        .unwrap();
    let output = GuestAddress(DATA.0 + 0x900);
    write_words(
        &mut runtime.memory,
        owner,
        GET_EXIT_CODE_STACK,
        &[0x401800, CURRENT_THREAD_PSEUDO_HANDLE, output.0],
    );
    runtime.memory.seal_owner(owner).unwrap();
    let reflection = runtime.memory.pending_reflection().unwrap();
    runtime
        .memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
    let dispatcher = ApiDispatcher::from_endpoints([get_exit.clone()]).unwrap();
    let live = runtime.open_current_thread_handle().unwrap();
    let closed = runtime.open_current_thread_handle().unwrap();
    runtime.close_handle(closed).unwrap();
    let wrong_kind = runtime.create_event(false, false).unwrap();
    let open_handles_before = runtime
        .object_for_handle(live, RuntimeObjectKind::Thread)
        .unwrap()
        .open_handles;
    for (handle, succeeds) in [
        (CURRENT_THREAD_PSEUDO_HANDLE, true),
        (live.0, true),
        (0, false),
        (u32::MAX, false),
        (wrong_kind.0, false),
        (closed.0, false),
    ] {
        runtime
            .memory
            .write(
                GuestAddress(GET_EXIT_CODE_STACK.0 + 4),
                &handle.to_le_bytes(),
            )
            .unwrap();
        runtime
            .memory
            .write(output, &0xdead_beef_u32.to_le_bytes())
            .unwrap();
        acknowledge(&mut runtime.memory);
        runtime.set_current_last_error(0x5678).unwrap();
        let before = runtime.memory.stats();
        let queried = dispatch(&dispatcher, &mut runtime, &get_exit, GET_EXIT_CODE_STACK);
        assert_eq!(queried.eax, Some(u32::from(succeeds)));
        if succeeds {
            assert_eq!(runtime.current_last_error().unwrap(), 0x5678);
            acknowledge(&mut runtime.memory);
            assert_eq!(
                runtime.memory.read(output, 4).unwrap(),
                259_u32.to_le_bytes()
            );
        } else {
            assert_eq!(runtime.current_last_error().unwrap(), 6);
            assert_eq!(runtime.memory.stats(), before);
            assert_eq!(
                runtime.memory.read(output, 4).unwrap(),
                0xdead_beef_u32.to_le_bytes()
            );
        }
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::Runnable
        );
        assert_eq!(
            runtime
                .object_for_handle(live, RuntimeObjectKind::Thread)
                .unwrap()
                .open_handles,
            open_handles_before
        );
    }
}

#[test]
fn resume_current_thread_accepts_pseudo_and_real_handles_without_mutation() {
    let (mut runtime, owner) = prepared_runtime();
    let resume = endpoint(owner, 1, 0x7f00_0310, DATA.0 + 0x800, "ResumeThread");
    runtime
        .memory
        .write_initialization(
            owner,
            GuestAddress(DATA.0 + 0x800),
            &resume.address.0.to_le_bytes(),
        )
        .unwrap();
    write_words(
        &mut runtime.memory,
        owner,
        RESUME_STACK,
        &[0x401100, CURRENT_THREAD_PSEUDO_HANDLE],
    );
    runtime.memory.seal_owner(owner).unwrap();
    let reflection = runtime.memory.pending_reflection().unwrap();
    runtime
        .memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
    let dispatcher = ApiDispatcher::from_endpoints([resume.clone()]).unwrap();
    let real = runtime.open_current_thread_handle().unwrap();
    let open_handles_before = runtime
        .object_for_handle(real, RuntimeObjectKind::Thread)
        .unwrap()
        .open_handles;
    for handle in [CURRENT_THREAD_PSEUDO_HANDLE, real.0] {
        runtime
            .memory
            .write(GuestAddress(RESUME_STACK.0 + 4), &handle.to_le_bytes())
            .unwrap();
        acknowledge(&mut runtime.memory);
        runtime.set_current_last_error(0x5678).unwrap();
        let before = runtime.memory.stats();
        let completion = dispatch(&dispatcher, &mut runtime, &resume, RESUME_STACK);
        assert_eq!(completion.eax, Some(0));
        assert_eq!(runtime.current_last_error().unwrap(), 0x5678);
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::Runnable
        );
        assert_eq!(runtime.current_thread_id(), INITIAL_THREAD_ID);
        assert_eq!(
            runtime
                .object_for_handle(real, RuntimeObjectKind::Thread)
                .unwrap()
                .open_handles,
            open_handles_before
        );
        assert_eq!(runtime.memory.stats(), before);
        assert!(runtime.scheduler().host_requests().is_empty());
    }
}

#[test]
fn create_resume_set_priority_and_close_follow_x86_contract_and_last_error_oracle() {
    let (mut runtime, owner) = prepared_runtime();
    let create = endpoint(owner, 1, 0x7f00_0310, 0x2000_0800, "CreateThread");
    let resume = endpoint(owner, 2, 0x7f00_0320, 0x2000_0804, "ResumeThread");
    let set_priority = endpoint(owner, 3, 0x7f00_0330, 0x2000_0808, "SetThreadPriority");
    let close = endpoint(owner, 4, 0x7f00_0340, 0x2000_080c, "CloseHandle");
    let get_exit_code = endpoint(owner, 5, 0x7f00_0350, 0x2000_0810, "GetExitCodeThread");
    for (iat, address) in [
        (0x2000_0800, create.address.0),
        (0x2000_0804, resume.address.0),
        (0x2000_0808, set_priority.address.0),
        (0x2000_080c, close.address.0),
        (0x2000_0810, get_exit_code.address.0),
    ] {
        runtime
            .memory
            .write_initialization(owner, GuestAddress(iat), &address.to_le_bytes())
            .unwrap();
    }
    write_words(
        &mut runtime.memory,
        owner,
        CREATE_STACK,
        &[0x401000, 0, 0, ENTRY.0, 0x1234_5678, 4, DATA.0],
    );
    write_words(&mut runtime.memory, owner, RESUME_STACK, &[0x401100, 0]);
    write_words(
        &mut runtime.memory,
        owner,
        SET_PRIORITY_STACK,
        &[0x401200, 0, 2],
    );
    write_words(
        &mut runtime.memory,
        owner,
        INVALID_PRIORITY_STACK,
        &[0x401300, 0, 3],
    );
    write_words(&mut runtime.memory, owner, CLOSE_STACK, &[0x401400, 0]);
    write_words(
        &mut runtime.memory,
        owner,
        CLOSED_PRIORITY_STACK,
        &[0x401500, 0, 1],
    );
    write_words(
        &mut runtime.memory,
        owner,
        INVALID_RESUME_STACK,
        &[0x401600, u32::MAX],
    );
    write_words(
        &mut runtime.memory,
        owner,
        GET_EXIT_CODE_STACK,
        &[0x401800, 0, DATA.0 + 4],
    );
    runtime.memory.seal_owner(owner).unwrap();
    let reflection = runtime.memory.pending_reflection().unwrap();
    runtime
        .memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();

    let dispatcher = ApiDispatcher::from_endpoints([
        create.clone(),
        resume.clone(),
        set_priority.clone(),
        close.clone(),
        get_exit_code.clone(),
    ])
    .unwrap();

    let creation = dispatch(&dispatcher, &mut runtime, &create, CREATE_STACK);
    let Some(handle) = creation.eax else {
        panic!("CreateThread did not return a handle");
    };
    assert_eq!(handle, 0x5401_0002);
    assert_eq!(creation.created_thread, Some(RuntimeThreadId(8193)));
    assert_eq!(creation.instruction_pointer, GuestAddress(0x401000));
    assert_eq!(creation.stack_pointer, GuestAddress(CREATE_STACK.0 + 28));
    assert_eq!(runtime.current_last_error().unwrap(), 0x5a5a);
    acknowledge(&mut runtime.memory);
    assert_eq!(read_u32(&runtime.memory, DATA), 8193);

    write_runtime_u32(
        &mut runtime.memory,
        GuestAddress(GET_EXIT_CODE_STACK.0 + 4),
        handle,
    );
    let running = dispatch(
        &dispatcher,
        &mut runtime,
        &get_exit_code,
        GET_EXIT_CODE_STACK,
    );
    assert_eq!(running.eax, Some(1));
    assert!(running.created_thread.is_none());
    assert_eq!(runtime.current_last_error().unwrap(), 0x5a5a);
    acknowledge(&mut runtime.memory);
    assert_eq!(read_u32(&runtime.memory, GuestAddress(DATA.0 + 4)), 259);
    assert_eq!(
        runtime
            .scheduler_state_for_thread(RuntimeThreadId(8193))
            .unwrap(),
        SchedulerThreadState::Suspended { count: 1 }
    );

    write_runtime_u32(
        &mut runtime.memory,
        GuestAddress(RESUME_STACK.0 + 4),
        handle,
    );
    let resumed = dispatch(&dispatcher, &mut runtime, &resume, RESUME_STACK);
    assert_eq!(resumed.eax, Some(1));
    assert_eq!(resumed.stack_pointer, GuestAddress(RESUME_STACK.0 + 8));
    assert_eq!(runtime.current_last_error().unwrap(), 0x5a5a);
    assert_eq!(
        runtime
            .scheduler_state_for_thread(RuntimeThreadId(8193))
            .unwrap(),
        SchedulerThreadState::Runnable
    );

    write_runtime_u32(
        &mut runtime.memory,
        GuestAddress(SET_PRIORITY_STACK.0 + 4),
        handle,
    );
    let priority = dispatch(&dispatcher, &mut runtime, &set_priority, SET_PRIORITY_STACK);
    assert_eq!(priority.eax, Some(1));
    assert_eq!(
        priority.stack_pointer,
        GuestAddress(SET_PRIORITY_STACK.0 + 12)
    );
    assert_eq!(runtime.current_last_error().unwrap(), 0x5a5a);
    assert_eq!(
        runtime.thread_priority(RuntimeThreadId(8193)).unwrap(),
        RuntimeThreadPriority::Highest
    );

    write_runtime_u32(
        &mut runtime.memory,
        GuestAddress(SET_PRIORITY_STACK.0 + 8),
        15,
    );
    let time_critical = dispatch(&dispatcher, &mut runtime, &set_priority, SET_PRIORITY_STACK);
    assert_eq!(time_critical.eax, Some(1));
    assert_eq!(runtime.current_last_error().unwrap(), 0x5a5a);
    assert_eq!(
        runtime.thread_priority(RuntimeThreadId(8193)).unwrap(),
        RuntimeThreadPriority::TimeCritical
    );

    write_runtime_u32(
        &mut runtime.memory,
        GuestAddress(INVALID_PRIORITY_STACK.0 + 4),
        handle,
    );
    let invalid_priority = dispatch(
        &dispatcher,
        &mut runtime,
        &set_priority,
        INVALID_PRIORITY_STACK,
    );
    assert_eq!(invalid_priority.eax, Some(0));
    assert_eq!(runtime.current_last_error().unwrap(), 87);
    assert_eq!(
        runtime.thread_priority(RuntimeThreadId(8193)).unwrap(),
        RuntimeThreadPriority::TimeCritical
    );
    runtime.set_current_last_error(0x5a5a).unwrap();

    let process_handle = runtime.process_handle().unwrap().0;
    write_runtime_u32(
        &mut runtime.memory,
        GuestAddress(INVALID_PRIORITY_STACK.0 + 4),
        process_handle,
    );
    write_runtime_u32(
        &mut runtime.memory,
        GuestAddress(INVALID_PRIORITY_STACK.0 + 8),
        1,
    );
    let wrong_kind_priority = dispatch(
        &dispatcher,
        &mut runtime,
        &set_priority,
        INVALID_PRIORITY_STACK,
    );
    assert_eq!(wrong_kind_priority.eax, Some(0));
    assert_eq!(runtime.current_last_error().unwrap(), 6);
    assert_eq!(
        runtime.thread_priority(RuntimeThreadId(8193)).unwrap(),
        RuntimeThreadPriority::TimeCritical
    );
    runtime.set_current_last_error(0x5a5a).unwrap();

    write_runtime_u32(&mut runtime.memory, GuestAddress(CLOSE_STACK.0 + 4), handle);
    let closed = dispatch(&dispatcher, &mut runtime, &close, CLOSE_STACK);
    assert_eq!(closed.eax, Some(1));
    assert_eq!(runtime.current_last_error().unwrap(), 0x5a5a);
    assert!(
        runtime
            .object_for_handle(
                crate::runtime::RuntimeHandle(handle),
                RuntimeObjectKind::Thread
            )
            .is_err()
    );

    let invalid_exit_code = dispatch(
        &dispatcher,
        &mut runtime,
        &get_exit_code,
        GET_EXIT_CODE_STACK,
    );
    assert_eq!(invalid_exit_code.eax, Some(0));
    assert_eq!(runtime.current_last_error().unwrap(), 6);
    assert_eq!(read_u32(&runtime.memory, GuestAddress(DATA.0 + 4)), 259);
    for invalid_handle in [0, runtime.process_handle().unwrap().0] {
        write_runtime_u32(
            &mut runtime.memory,
            GuestAddress(GET_EXIT_CODE_STACK.0 + 4),
            invalid_handle,
        );
        let invalid = dispatch(
            &dispatcher,
            &mut runtime,
            &get_exit_code,
            GET_EXIT_CODE_STACK,
        );
        assert_eq!(invalid.eax, Some(0));
        assert_eq!(runtime.current_last_error().unwrap(), 6);
        assert_eq!(read_u32(&runtime.memory, GuestAddress(DATA.0 + 4)), 259);
    }

    write_runtime_u32(
        &mut runtime.memory,
        GuestAddress(CLOSED_PRIORITY_STACK.0 + 4),
        handle,
    );
    let closed_priority = dispatch(
        &dispatcher,
        &mut runtime,
        &set_priority,
        CLOSED_PRIORITY_STACK,
    );
    assert_eq!(closed_priority.eax, Some(0));
    assert_eq!(runtime.current_last_error().unwrap(), 6);
    assert_eq!(
        runtime.thread_priority(RuntimeThreadId(8193)).unwrap(),
        RuntimeThreadPriority::TimeCritical
    );

    let invalid = dispatch(&dispatcher, &mut runtime, &resume, INVALID_RESUME_STACK);
    assert_eq!(invalid.eax, Some(u32::MAX));
    assert_eq!(runtime.current_last_error().unwrap(), 6);
}

#[test]
fn create_thread_capacity_failure_preserves_output_and_allows_retry_after_releasing_resources() {
    for quota in 0..3 {
        let mut limits = MemoryLimits::default();
        match quota {
            0 => limits.max_owners = 10,
            1 => limits.max_regions = 10,
            _ => limits.max_physical_pages = 10,
        }
        let (mut runtime, owner) = prepared_runtime_with_limits(limits);
        let create = endpoint(owner, 1, 0x7f00_0310, DATA.0 + 0x800, "CreateThread");
        let output = GuestAddress(DATA.0 + 0x900);
        runtime
            .memory
            .write_initialization(
                owner,
                GuestAddress(DATA.0 + 0x800),
                &create.address.0.to_le_bytes(),
            )
            .unwrap();
        runtime
            .memory
            .write_initialization(owner, output, &0xdead_beef_u32.to_le_bytes())
            .unwrap();
        write_words(
            &mut runtime.memory,
            owner,
            CREATE_STACK,
            &[0x401000, 0, 0, ENTRY.0, 0x1234, 0, output.0],
        );
        runtime.memory.seal_owner(owner).unwrap();
        let extra_owner = runtime
            .memory
            .create_owner(MemoryOwnerKind::Anonymous)
            .unwrap();
        let extra_range = runtime
            .memory
            .reserve_fixed(
                extra_owner,
                MemoryRegionKind::Anonymous,
                GuestAddress(0x3000_0000),
                GUEST_PAGE_SIZE,
            )
            .unwrap();
        runtime
            .memory
            .commit(
                extra_owner,
                extra_range,
                PageProtection {
                    read: true,
                    write: true,
                    execute: false,
                },
            )
            .unwrap();
        let reflection = runtime.memory.pending_reflection().unwrap();
        runtime
            .memory
            .acknowledge_delta(reflection.invalidation.generation)
            .unwrap();
        let dispatcher = ApiDispatcher::from_endpoints([create.clone()]).unwrap();
        let memory = runtime.memory.stats();
        let allocations = runtime.process_address_space_allocator().clone();
        let result = dispatch(&dispatcher, &mut runtime, &create, CREATE_STACK);
        assert_eq!(result.eax, Some(0));
        assert_eq!(runtime.current_last_error().unwrap(), 8);
        assert_eq!(runtime.memory.stats(), memory);
        assert_eq!(runtime.process_address_space_allocator(), &allocations);
        assert_eq!(read_u32(&runtime.memory, output), 0xdead_beef);
        assert_eq!(runtime.scheduler().thread_states().len(), 1);
        assert!(runtime.thread_handle(RuntimeThreadId(8193)).is_err());
        runtime
            .memory
            .release_region(extra_owner, extra_range.start())
            .unwrap();
        runtime.memory.destroy_owner(extra_owner).unwrap();
        acknowledge(&mut runtime.memory);
        let retry = dispatch(&dispatcher, &mut runtime, &create, CREATE_STACK);
        assert_eq!(retry.eax, Some(0x5401_0002));
        assert_eq!(runtime.current_last_error().unwrap(), 8);
        acknowledge(&mut runtime.memory);
        assert_eq!(read_u32(&runtime.memory, output), 8193);
    }
}

#[test]
fn create_thread_faults_for_a_pending_caller_without_consuming_output_or_identity() {
    let (mut runtime, owner) = prepared_runtime();
    let create = endpoint(owner, 1, 0x7f00_0310, DATA.0 + 0x800, "CreateThread");
    let output = GuestAddress(DATA.0 + 0x900);
    runtime
        .memory
        .write_initialization(
            owner,
            GuestAddress(DATA.0 + 0x800),
            &create.address.0.to_le_bytes(),
        )
        .unwrap();
    runtime
        .memory
        .write_initialization(owner, output, &0xdead_beef_u32.to_le_bytes())
        .unwrap();
    write_words(
        &mut runtime.memory,
        owner,
        CREATE_STACK,
        &[0x401000, 0, 0, ENTRY.0, 0x1234, 0, output.0],
    );
    runtime.memory.seal_owner(owner).unwrap();
    let reflection = runtime.memory.pending_reflection().unwrap();
    runtime
        .memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
    let dispatcher = ApiDispatcher::from_endpoints([create.clone()]).unwrap();
    let pending = runtime.begin_host_request("fixture.pending").unwrap();
    let memory = runtime.memory.stats();
    let allocations = runtime.process_address_space_allocator().clone();
    let scheduler = runtime.scheduler_state().unwrap();
    let last_error = runtime.current_last_error().unwrap();
    let faulted = dispatcher.dispatch_owned(
        ApiOwnedTrap {
            endpoint_id: create.id,
            endpoint_address: create.address,
            stack_pointer: CREATE_STACK,
            return_address: GuestAddress(0x401000),
        },
        &mut runtime,
        None,
        crate::win32::dispatch_kernel32_api,
    );
    assert!(matches!(faulted, DispatchOutcome::Fault(_)));
    assert_eq!(runtime.memory.stats(), memory);
    assert_eq!(runtime.process_address_space_allocator(), &allocations);
    assert_eq!(runtime.scheduler_state().unwrap(), scheduler);
    assert_eq!(runtime.current_last_error().unwrap(), last_error);
    assert_eq!(read_u32(&runtime.memory, output), 0xdead_beef);
    assert!(runtime.thread_handle(RuntimeThreadId(8193)).is_err());
    runtime.complete_host_request(pending.request_id).unwrap();
    let retry = dispatch(&dispatcher, &mut runtime, &create, CREATE_STACK);
    assert_eq!(retry.eax, Some(0x5401_0002));
    acknowledge(&mut runtime.memory);
    assert_eq!(read_u32(&runtime.memory, output), 8193);
}

#[test]
fn create_thread_rejects_invalid_arguments_before_memory_or_lifetime_publication() {
    let (mut runtime, owner) = prepared_runtime();
    let create = endpoint(owner, 1, 0x7f00_0310, 0x2000_0800, "CreateThread");
    runtime
        .memory
        .write_initialization(
            owner,
            GuestAddress(0x2000_0800),
            &create.address.0.to_le_bytes(),
        )
        .unwrap();
    write_words(
        &mut runtime.memory,
        owner,
        CREATE_STACK,
        &[0x401000, 0, 0, ENTRY.0, 0x1234, 0, DATA.0],
    );
    runtime.memory.seal_owner(owner).unwrap();
    let reflection = runtime.memory.pending_reflection().unwrap();
    runtime
        .memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
    let dispatcher = ApiDispatcher::from_endpoints([create.clone()]).unwrap();
    let valid = [0, 0, ENTRY.0, 0x1234, 0, DATA.0];
    for (argument, invalid_value, native_failure) in [
        (0, DATA.0, true),       // unsupported security attributes
        (0, DATA.0 + 1, true),   // opaque argument words need no alignment policy
        (1, 4096, true),         // non-default stack size is not implemented
        (2, 0, true),            // NULL entry uses the handler's native failure
        (2, DATA.0, false),      // non-executable entry is a memory fault
        (2, 0xdead_beef, false), // unmapped entry is a memory fault
        (4, 1, true),            // unknown creation flag
        (5, ENTRY.0, false),     // read-only thread ID output is a memory fault
        (5, 0xffff_fffe, false), // output crosses the x86 limit
    ] {
        let mut arguments = valid;
        arguments[argument] = invalid_value;
        for (index, value) in arguments.into_iter().enumerate() {
            write_runtime_u32(
                &mut runtime.memory,
                GuestAddress(CREATE_STACK.0 + 4 + index as u32 * 4),
                value,
            );
        }
        runtime.set_current_last_error(0x1234).unwrap();
        let stack_before = runtime.thread_stack(INITIAL_THREAD_ID).unwrap().clone();
        let memory_before = runtime.memory.stats();
        let output_before = runtime.memory.read(DATA, 4).unwrap();
        let failure = dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: create.id,
                endpoint_address: create.address,
                stack_pointer: CREATE_STACK,
                return_address: GuestAddress(0x401000),
            },
            &mut runtime,
            None,
            crate::win32::dispatch_kernel32_api,
        );
        match failure {
            DispatchOutcome::Immediate(completion) if native_failure => {
                assert_eq!(completion.eax, Some(0));
                assert_eq!(runtime.current_last_error().unwrap(), 87);
            }
            DispatchOutcome::Fault(_) if !native_failure => {
                assert_eq!(runtime.current_last_error().unwrap(), 0x1234);
            }
            other => panic!("creation failure used the wrong Windows/fault path: {other:?}"),
        }
        assert_eq!(runtime.current_thread_id(), INITIAL_THREAD_ID);
        assert_eq!(
            runtime.current_last_error().unwrap(),
            if native_failure { 87 } else { 0x1234 }
        );
        assert_eq!(runtime.scheduler().thread_states().len(), 1);
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::Runnable
        );
        assert_eq!(
            runtime.thread_stack(INITIAL_THREAD_ID).unwrap(),
            &stack_before
        );
        assert!(runtime.thread_state(RuntimeThreadId(8193)).is_err());
        assert!(runtime.thread_handle(RuntimeThreadId(8193)).is_err());
        assert_eq!(runtime.memory.stats(), memory_before);
        assert_eq!(runtime.memory.read(DATA, 4).unwrap(), output_before);
    }
    for (index, value) in valid.into_iter().enumerate() {
        write_runtime_u32(
            &mut runtime.memory,
            GuestAddress(CREATE_STACK.0 + 4 + index as u32 * 4),
            value,
        );
    }
    write_runtime_u32(&mut runtime.memory, GuestAddress(CREATE_STACK.0 + 24), 0);
    runtime.set_current_last_error(87).unwrap();
    let creation = dispatch(&dispatcher, &mut runtime, &create, CREATE_STACK);
    assert!(matches!(creation.eax, Some(handle) if handle != 0));
    assert_eq!(runtime.current_last_error().unwrap(), 87);
    assert_eq!(
        runtime
            .scheduler_state_for_thread(RuntimeThreadId(8193))
            .unwrap(),
        SchedulerThreadState::Runnable
    );
    let parent = runtime.thread_stack(INITIAL_THREAD_ID).unwrap();
    let child = runtime.thread_stack(RuntimeThreadId(8193)).unwrap();
    assert_eq!(
        child.reserved_range.byte_length(),
        parent.reserved_range.byte_length()
    );
    assert_eq!(child.initial_commit_bytes, parent.initial_commit_bytes);
}

#[test]
fn set_thread_priority_accepts_the_current_thread_pseudo_handle() {
    let (mut runtime, owner) = prepared_runtime();
    let set_priority = endpoint(owner, 3, 0x7f00_0330, 0x2000_0808, "SetThreadPriority");
    runtime
        .memory
        .write_initialization(
            owner,
            GuestAddress(0x2000_0808),
            &set_priority.address.0.to_le_bytes(),
        )
        .unwrap();
    write_words(
        &mut runtime.memory,
        owner,
        PSEUDO_PRIORITY_STACK,
        &[0x401700, CURRENT_THREAD_PSEUDO_HANDLE, 2],
    );
    runtime.memory.seal_owner(owner).unwrap();
    let reflection = runtime.memory.pending_reflection().unwrap();
    runtime
        .memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();

    let dispatcher = ApiDispatcher::from_endpoints([set_priority.clone()]).unwrap();

    // GetCurrentThread() hands the guest 0xfffffffe, not a real handle; SetThreadPriority must
    // resolve it to the calling thread rather than rejecting it as an unknown handle.
    let raised = dispatch(
        &dispatcher,
        &mut runtime,
        &set_priority,
        PSEUDO_PRIORITY_STACK,
    );
    assert_eq!(raised.eax, Some(1));
    assert_eq!(
        raised.stack_pointer,
        GuestAddress(PSEUDO_PRIORITY_STACK.0 + 12)
    );
    assert_eq!(runtime.current_last_error().unwrap(), 0x5a5a);
    assert_eq!(
        runtime.thread_priority(INITIAL_THREAD_ID).unwrap(),
        RuntimeThreadPriority::Highest
    );

    write_runtime_u32(
        &mut runtime.memory,
        GuestAddress(PSEUDO_PRIORITY_STACK.0 + 8),
        1,
    );
    let lowered = dispatch(
        &dispatcher,
        &mut runtime,
        &set_priority,
        PSEUDO_PRIORITY_STACK,
    );
    assert_eq!(lowered.eax, Some(1));
    assert_eq!(runtime.current_last_error().unwrap(), 0x5a5a);
    assert_eq!(
        runtime.thread_priority(INITIAL_THREAD_ID).unwrap(),
        RuntimeThreadPriority::AboveNormal
    );
    for value in [-15_i32, -2, -1, 0, 1, 2, 15] {
        write_runtime_u32(
            &mut runtime.memory,
            GuestAddress(PSEUDO_PRIORITY_STACK.0 + 8),
            value as u32,
        );
        let completion = dispatch(
            &dispatcher,
            &mut runtime,
            &set_priority,
            PSEUDO_PRIORITY_STACK,
        );
        assert_eq!(completion.eax, Some(1));
        assert_eq!(runtime.current_last_error().unwrap(), 0x5a5a);
        assert_eq!(
            runtime
                .thread_priority(INITIAL_THREAD_ID)
                .unwrap()
                .win32_value(),
            value
        );
    }
}

#[test]
fn wait_for_single_object_joins_live_threads_and_keeps_event_reset_semantics() {
    let (mut runtime, owner) = prepared_runtime();
    let wait = endpoint(owner, 1, 0x7f00_0310, 0x2000_0800, "WaitForSingleObject");
    let dispatcher = ApiDispatcher::from_endpoints(vec![wait.clone()]).unwrap();

    let second = RuntimeThreadId(8193);
    let thread = runtime.register_thread(second, 0).unwrap();
    write_words(
        &mut runtime.memory,
        owner,
        GuestAddress(0x2000_0800),
        &[wait.address.0],
    );
    write_words(
        &mut runtime.memory,
        owner,
        CREATE_STACK,
        &[0x401000, thread.0, 0],
    );
    write_words(
        &mut runtime.memory,
        owner,
        RESUME_STACK,
        &[0x401100, thread.0, u32::MAX],
    );
    runtime.memory.seal_owner(owner).unwrap();
    let reflection = runtime.memory.pending_reflection().unwrap();
    runtime
        .memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();

    let poll = dispatch(&dispatcher, &mut runtime, &wait, CREATE_STACK);
    assert_eq!(poll.eax, Some(258));
    assert_eq!(runtime.current_last_error().unwrap(), 0x5a5a);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    write_runtime_u32(&mut runtime.memory, GuestAddress(CREATE_STACK.0 + 8), 5000);
    let clock_before_finite_wait = runtime.clock().clone();
    let memory_before_finite_wait = runtime.memory.stats();
    let return_address = GuestAddress(read_u32(&runtime.memory, CREATE_STACK));
    let pending_finite_wait = dispatcher.dispatch_owned(
        ApiOwnedTrap {
            endpoint_id: wait.id,
            endpoint_address: wait.address,
            stack_pointer: CREATE_STACK,
            return_address,
        },
        &mut runtime,
        None,
        dispatch_kernel32_api,
    );
    let DispatchOutcome::Deferred(pending_finite_wait) = pending_finite_wait else {
        panic!("finite thread wait must defer: {pending_finite_wait:?}");
    };
    assert_eq!(
        pending_finite_wait.request.service,
        crate::RUNTIME_OBJECT_WAIT_SERVICE
    );
    assert_eq!(pending_finite_wait.request.payload, 5000_u32.to_le_bytes());
    assert_eq!(runtime.current_last_error().unwrap(), 0x5a5a);
    assert_eq!(runtime.current_thread_id(), INITIAL_THREAD_ID);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::WaitingHost {
            request_id: pending_finite_wait.request.request_id
        }
    );
    assert_eq!(runtime.clock(), &clock_before_finite_wait);
    assert_eq!(runtime.memory.stats(), memory_before_finite_wait);
    let reject_invalid_completion = |runtime: &mut Runtime, pending: &crate::ApiDeferredCall| {
        let malformed = crate::ApiDeferredCall {
            frame: pending.frame.clone(),
            request: crate::api::HostRequest {
                request_id: pending.request.request_id,
                service: pending.request.service.clone(),
                payload: 1_u32.to_le_bytes().to_vec(),
            },
        };
        let before = runtime.scheduler().clone();
        assert!(matches!(
            crate::complete_kernel32_object_wait(&dispatcher, &malformed, 1, runtime),
            DispatchOutcome::Fault(_)
        ));
        assert_eq!(runtime.scheduler(), &before);
        {
            let address = wait.iat_address.unwrap();
            let original = read_u32(&runtime.memory, address);
            write_runtime_u32(&mut runtime.memory, address, 0xdead_beef);
            let before = runtime.scheduler().clone();
            let memory_before = runtime.memory.stats();
            assert!(matches!(
                crate::complete_kernel32_object_wait(&dispatcher, pending, 1, runtime),
                DispatchOutcome::Fault(_)
            ));
            assert_eq!(runtime.scheduler(), &before);
            assert_eq!(runtime.memory.stats(), memory_before);
            assert_eq!(runtime.current_last_error().unwrap(), 0x5a5a);
            write_runtime_u32(&mut runtime.memory, address, original);
        }
    };
    reject_invalid_completion(&mut runtime, &pending_finite_wait);
    for ack in [0, 2] {
        let before = runtime.scheduler().clone();
        assert!(matches!(
            crate::complete_kernel32_object_wait(
                &dispatcher,
                &pending_finite_wait,
                ack,
                &mut runtime
            ),
            DispatchOutcome::Fault(_)
        ));
        assert_eq!(runtime.scheduler(), &before);
    }
    let DispatchOutcome::Deferred(retry) =
        crate::complete_kernel32_object_wait(&dispatcher, &pending_finite_wait, 1, &mut runtime)
    else {
        panic!("early retry");
    };
    assert_eq!(
        retry.request.request_id,
        pending_finite_wait.request.request_id
    );
    assert_eq!(retry.request.service, pending_finite_wait.request.service);
    assert_eq!(retry.request.payload, pending_finite_wait.request.payload);
    assert_eq!(
        retry.frame.stack_pointer,
        pending_finite_wait.frame.stack_pointer
    );
    runtime
        .synchronize_scheduled_clock_for_host_execution(0, 50_000_000)
        .unwrap();
    reject_invalid_completion(&mut runtime, &retry);
    let DispatchOutcome::Immediate(timeout) =
        crate::complete_kernel32_object_wait(&dispatcher, &retry, 1, &mut runtime)
    else {
        panic!("completed timeout");
    };
    assert_eq!(timeout.eax, Some(258));
    assert_eq!(timeout.stack_pointer, GuestAddress(CREATE_STACK.0 + 12));
    assert_eq!(runtime.current_last_error().unwrap(), 0x5a5a);
    assert!(runtime.scheduler().wait_registrations().is_empty());
    assert!(runtime.scheduler().host_requests().is_empty());
    let before = runtime.scheduler().clone();
    assert!(matches!(
        crate::complete_kernel32_object_wait(&dispatcher, &retry, 1, &mut runtime),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(runtime.scheduler(), &before);
    write_runtime_u32(&mut runtime.memory, GuestAddress(CREATE_STACK.0 + 8), 0);
    let join = dispatch(&dispatcher, &mut runtime, &wait, RESUME_STACK);
    assert_eq!(join.eax, Some(0));
    assert_eq!(runtime.current_last_error().unwrap(), 0x5a5a);
    assert_eq!(
        join.execution_boundary,
        vine_core::RuntimeApiBoundaryDisposition::BlockCurrentThread
    );
    assert!(matches!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::WaitingObject { .. }
    ));

    runtime.select_thread(second).unwrap();
    runtime.request_current_thread_exit(23).unwrap();
    assert!(matches!(
        runtime
            .scheduler_state_for_thread(INITIAL_THREAD_ID)
            .unwrap(),
        SchedulerThreadState::WaitingObject { .. }
    ));
    runtime.complete_current_thread_exit().unwrap();
    runtime.select_thread(INITIAL_THREAD_ID).unwrap();
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert!(runtime.scheduler().wait_registrations().is_empty());
    for stack in [RESUME_STACK, CREATE_STACK, CREATE_STACK] {
        let joined = dispatch(&dispatcher, &mut runtime, &wait, stack);
        assert_eq!(joined.eax, Some(0));
        assert_eq!(runtime.current_last_error().unwrap(), 0x5a5a);
        assert_eq!(
            joined.execution_boundary,
            vine_core::RuntimeApiBoundaryDisposition::ContinueQuantum
        );
    }

    write_runtime_u32(&mut runtime.memory, GuestAddress(RESUME_STACK.0 + 8), 5000);
    let finite_signaled = dispatch(&dispatcher, &mut runtime, &wait, RESUME_STACK);
    assert_eq!(finite_signaled.eax, Some(0));
    assert_eq!(runtime.current_last_error().unwrap(), 0x5a5a);
    runtime.close_handle(thread).unwrap();
    let invalid = dispatch(&dispatcher, &mut runtime, &wait, CREATE_STACK);
    assert_eq!(invalid.eax, Some(u32::MAX));
    assert_eq!(runtime.current_last_error().unwrap(), 6);
    let event = runtime.create_event(false, true).unwrap();
    write_runtime_u32(
        &mut runtime.memory,
        GuestAddress(CREATE_STACK.0 + 4),
        event.0,
    );
    for expected in [0, 258] {
        let result = dispatch(&dispatcher, &mut runtime, &wait, CREATE_STACK);
        assert_eq!(result.eax, Some(expected));
        assert_eq!(runtime.current_last_error().unwrap(), 6);
    }
}

fn dispatch(
    dispatcher: &ApiDispatcher,
    runtime: &mut Runtime,
    endpoint: &ApiEndpointIdentity,
    stack_pointer: GuestAddress,
) -> vine_core::ApiImmediateCompletion {
    let return_address = GuestAddress(
        runtime
            .memory
            .read(stack_pointer, 4)
            .unwrap()
            .try_into()
            .map(u32::from_le_bytes)
            .unwrap(),
    );
    match dispatcher.dispatch_owned(
        ApiOwnedTrap {
            endpoint_id: endpoint.id,
            endpoint_address: endpoint.address,
            stack_pointer,
            return_address,
        },
        runtime,
        None,
        dispatch_kernel32_api,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected immediate completion, got {other:?}"),
    }
}

fn endpoint(
    owner: MemoryOwnerId,
    id: u32,
    address: u32,
    iat_address: u32,
    name: &str,
) -> ApiEndpointIdentity {
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(id),
        address: GuestAddress(address),
        requester_owner: owner,
        module: "KERNEL32.dll".to_owned(),
        iat_rva: vine_core::Rva(iat_address - DATA.0),
        iat_address: GuestAddress(iat_address),
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: name.to_owned(),
            hint: 0,
            name_rva: vine_core::Rva(0),
        },
    });
    assert_eq!(
        endpoint.symbol,
        ApiSymbol::Name {
            name: name.to_owned()
        }
    );
    endpoint
}

fn prepared_runtime() -> (Runtime, MemoryOwnerId) {
    prepared_runtime_with_limits(MemoryLimits::default())
}

fn prepared_runtime_with_limits(limits: MemoryLimits) -> (Runtime, MemoryOwnerId) {
    let memory = GuestMemory::new(limits).unwrap();
    let mut runtime = Runtime::new(runtime_bootstrap(), memory).unwrap();
    let allocation = runtime
        .process_address_space_allocator()
        .allocation_for_purpose(&ProcessAddressAllocationPurpose::CallbackReturns)
        .unwrap()
        .clone();
    let sentinels = install_runtime_return_sentinels(&mut runtime.memory, &allocation).unwrap();
    let stack = install_initial_thread_stack(
        &mut runtime.memory,
        &sentinels.process_entry_return,
        0x0010_0000,
        GUEST_PAGE_SIZE,
    )
    .unwrap();
    runtime.register_initial_thread_stack(&stack).unwrap();
    runtime
        .install_process_environment("thread-api.exe")
        .unwrap();

    let entry_owner = runtime
        .memory
        .create_owner(MemoryOwnerKind::Anonymous)
        .unwrap();
    let entry_range = runtime
        .memory
        .reserve_fixed(
            entry_owner,
            MemoryRegionKind::Anonymous,
            ENTRY,
            GUEST_PAGE_SIZE,
        )
        .unwrap();
    runtime
        .memory
        .commit(
            entry_owner,
            entry_range,
            PageProtection {
                read: true,
                write: false,
                execute: true,
            },
        )
        .unwrap();
    runtime
        .memory
        .write_initialization(entry_owner, ENTRY, &[0x90])
        .unwrap();
    runtime.memory.seal_owner(entry_owner).unwrap();

    let data_owner = runtime
        .memory
        .create_owner(MemoryOwnerKind::Anonymous)
        .unwrap();
    let data_range = runtime
        .memory
        .reserve_fixed(
            data_owner,
            MemoryRegionKind::Anonymous,
            DATA,
            GUEST_PAGE_SIZE,
        )
        .unwrap();
    runtime
        .memory
        .commit(
            data_owner,
            data_range,
            PageProtection {
                read: true,
                write: true,
                execute: false,
            },
        )
        .unwrap();
    runtime.start_initial_thread().unwrap();
    (runtime, data_owner)
}

fn write_words(
    memory: &mut GuestMemory,
    owner: MemoryOwnerId,
    address: GuestAddress,
    words: &[u32],
) {
    let bytes = words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<_>>();
    memory.write_initialization(owner, address, &bytes).unwrap();
}

fn write_runtime_u32(memory: &mut GuestMemory, address: GuestAddress, value: u32) {
    memory.write(address, &value.to_le_bytes()).unwrap();
    acknowledge(memory);
}

fn acknowledge(memory: &mut GuestMemory) {
    let delta = memory.pending_delta().unwrap();
    memory.acknowledge_delta(delta.generation).unwrap();
}

fn read_u32(memory: &GuestMemory, address: GuestAddress) -> u32 {
    u32::from_le_bytes(memory.read(address, 4).unwrap().try_into().unwrap())
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
            bytes_with_nul: b"C:\\Vine\\thread-api.exe\0".to_vec(),
        },
        initial_current_directory_a: RuntimeCurrentDirectoryA {
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
        },
        encoded_pointer_cookie: 0xec01_9d3b,
    }
}

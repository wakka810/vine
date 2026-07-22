use vine_core::{
    ApiDispatcher, ApiEndpointIdentity, ApiOwnedTrap, ApiSymbol, DispatchOutcome, ExecutionSession,
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, ImportTarget, ImportThunk, ImportThunkDisposition,
    ImportThunkId, Kernel32CompatibilityPolicy, Kernel32CreateThreadPolicy, MemoryLimits,
    MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind, PageProtection,
    ProcessAddressAllocationPurpose, ProcessAddressSpaceLayout, RegisterUpdate, RuntimeBootstrap,
    RuntimeClockState, RuntimeCurrentDirectoryA, RuntimeObjectKind, RuntimeProcessId,
    RuntimeProcessImagePathA, RuntimeStartupInfo, RuntimeState, RuntimeThreadId,
    SchedulerThreadState, X86_STARTUP_INFO_W_BYTE_LENGTH, dispatch_kernel32_api_with_policy,
    install_initial_thread_stack, install_runtime_return_sentinels, kernel32_contract_for_endpoint,
};

const PROCESS_ID: u32 = 4096;
const INITIAL_THREAD_ID: RuntimeThreadId = RuntimeThreadId(8192);
const ENTRY: GuestAddress = GuestAddress(0x1000_0000);
const DATA: GuestAddress = GuestAddress(0x2000_0000);
const CREATE_STACK: GuestAddress = GuestAddress(0x2000_0100);
const RESUME_STACK: GuestAddress = GuestAddress(0x2000_0140);
const CLOSE_STACK: GuestAddress = GuestAddress(0x2000_0160);
const INVALID_RESUME_STACK: GuestAddress = GuestAddress(0x2000_0180);

#[test]
fn create_resume_and_close_thread_follow_x86_contract_and_last_error_oracle() {
    let (mut memory, mut runtime, owner) = prepared_runtime();
    let create = endpoint(owner, 1, 0x7f00_0310, 0x2000_0800, "CreateThread");
    let resume = endpoint(owner, 2, 0x7f00_0320, 0x2000_0804, "ResumeThread");
    let close = endpoint(owner, 3, 0x7f00_0330, 0x2000_0808, "CloseHandle");
    for (iat, address) in [
        (0x2000_0800, create.address.0),
        (0x2000_0804, resume.address.0),
        (0x2000_0808, close.address.0),
    ] {
        memory
            .write_initialization(owner, GuestAddress(iat), &address.to_le_bytes())
            .unwrap();
    }
    write_words(
        &mut memory,
        owner,
        CREATE_STACK,
        &[0x401000, 0, 0, ENTRY.0, 0x1234_5678, 4, DATA.0],
    );
    write_words(&mut memory, owner, RESUME_STACK, &[0x401100, 0]);
    write_words(&mut memory, owner, CLOSE_STACK, &[0x401200, 0]);
    write_words(
        &mut memory,
        owner,
        INVALID_RESUME_STACK,
        &[0x401300, u32::MAX],
    );
    memory.seal_owner(owner).unwrap();
    let reflection = memory.pending_reflection().unwrap();
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();

    let contracts = [&create, &resume, &close]
        .into_iter()
        .map(|endpoint| {
            kernel32_contract_for_endpoint(endpoint.clone())
                .unwrap()
                .unwrap()
        })
        .collect::<Vec<_>>();
    let dispatcher =
        ApiDispatcher::new([create.clone(), resume.clone(), close.clone()], contracts).unwrap();
    let policy = thread_policy();

    let creation = dispatch(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &create,
        CREATE_STACK,
        &policy,
    );
    let RegisterUpdate::Set(handle) = creation.eax else {
        panic!("CreateThread did not return a handle");
    };
    assert_eq!(handle, 0x5401_0002);
    assert_eq!(creation.instruction_pointer, GuestAddress(0x401000));
    assert_eq!(creation.stack_pointer, GuestAddress(CREATE_STACK.0 + 28));
    assert_eq!(creation.last_error, 0x5a5a);
    assert_eq!(creation.guest_writes.len(), 1);
    assert_eq!(creation.guest_writes[0].write.address, DATA);
    assert_eq!(creation.guest_writes[0].write.bytes, 8193_u32.to_le_bytes());
    acknowledge(&mut memory);
    assert_eq!(read_u32(&memory, DATA), 8193);
    assert_eq!(
        runtime
            .scheduler_state_for_thread(RuntimeThreadId(8193))
            .unwrap(),
        SchedulerThreadState::Suspended { count: 1 }
    );

    write_runtime_u32(&mut memory, GuestAddress(RESUME_STACK.0 + 4), handle);
    let resumed = dispatch(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &resume,
        RESUME_STACK,
        &policy,
    );
    assert_eq!(resumed.eax, RegisterUpdate::Set(1));
    assert_eq!(resumed.stack_pointer, GuestAddress(RESUME_STACK.0 + 8));
    assert_eq!(resumed.last_error, 0x5a5a);
    assert_eq!(
        runtime
            .scheduler_state_for_thread(RuntimeThreadId(8193))
            .unwrap(),
        SchedulerThreadState::Runnable
    );

    write_runtime_u32(&mut memory, GuestAddress(CLOSE_STACK.0 + 4), handle);
    let closed = dispatch(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &close,
        CLOSE_STACK,
        &policy,
    );
    assert_eq!(closed.eax, RegisterUpdate::Set(1));
    assert_eq!(closed.last_error, 0x5a5a);
    assert!(
        runtime
            .object_for_handle(vine_core::RuntimeHandle(handle), RuntimeObjectKind::Thread)
            .is_err()
    );

    let invalid = dispatch(
        &dispatcher,
        &mut memory,
        &mut runtime,
        &resume,
        INVALID_RESUME_STACK,
        &policy,
    );
    assert_eq!(invalid.eax, RegisterUpdate::Set(u32::MAX));
    assert_eq!(invalid.last_error, 6);
}

fn dispatch(
    dispatcher: &ApiDispatcher,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    endpoint: &ApiEndpointIdentity,
    stack_pointer: GuestAddress,
    policy: &Kernel32CompatibilityPolicy,
) -> vine_core::ApiImmediateCompletion {
    let return_address = GuestAddress(
        memory
            .read(stack_pointer, 4)
            .unwrap()
            .try_into()
            .map(u32::from_le_bytes)
            .unwrap(),
    );
    let mut session = ExecutionSession::new(memory, runtime);
    match dispatcher.dispatch_owned(
        ApiOwnedTrap {
            endpoint_id: endpoint.id,
            endpoint_address: endpoint.address,
            stack_pointer,
            return_address,
        },
        &mut session,
        |contract, frame, session| {
            dispatch_kernel32_api_with_policy(policy, contract, frame, session)
        },
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

fn thread_policy() -> Kernel32CompatibilityPolicy {
    let mut policy: Kernel32CompatibilityPolicy = serde_json::from_str(include_str!(
        "../../../fixtures/runtime-policies/kernel32-d3dx-runtime-module-v1.json"
    ))
    .unwrap();
    policy.id = "kernel32-create-thread-test-v1".to_owned();
    policy.create_thread = Some(Kernel32CreateThreadPolicy {
        default_stack_reserve_bytes: 0x0010_0000,
        default_stack_commit_bytes: GUEST_PAGE_SIZE as u32,
        allowed_creation_flags: vec![0, 4],
        maximum_threads: 8,
    });
    policy.validate().unwrap();
    policy
}

fn prepared_runtime() -> (GuestMemory, RuntimeState, MemoryOwnerId) {
    let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
    let mut runtime = RuntimeState::new(runtime_bootstrap()).unwrap();
    let allocation = runtime
        .process_address_space_allocator()
        .allocation_for_purpose(&ProcessAddressAllocationPurpose::CallbackReturns)
        .unwrap()
        .clone();
    let sentinels = install_runtime_return_sentinels(&mut memory, &allocation).unwrap();
    let stack = install_initial_thread_stack(
        &mut memory,
        &sentinels.process_entry_return,
        0x0010_0000,
        GUEST_PAGE_SIZE,
    )
    .unwrap();
    runtime.register_initial_thread_stack(&stack).unwrap();
    ExecutionSession::new(&mut memory, &mut runtime)
        .install_controlled_process_environment_with_command_line_and_ansi("thread-api.exe")
        .unwrap();

    let entry_owner = memory.create_owner(MemoryOwnerKind::Anonymous).unwrap();
    let entry_range = memory
        .reserve_fixed(
            entry_owner,
            MemoryRegionKind::Anonymous,
            ENTRY,
            GUEST_PAGE_SIZE,
        )
        .unwrap();
    memory
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
    memory
        .write_initialization(entry_owner, ENTRY, &[0x90])
        .unwrap();
    memory.seal_owner(entry_owner).unwrap();

    let data_owner = memory.create_owner(MemoryOwnerKind::Anonymous).unwrap();
    let data_range = memory
        .reserve_fixed(
            data_owner,
            MemoryRegionKind::Anonymous,
            DATA,
            GUEST_PAGE_SIZE,
        )
        .unwrap();
    memory
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
    (memory, runtime, data_owner)
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
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
            performance_frequency: 10_000_000,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "kernel32-thread-dispatcher-test-v1".to_owned(),
            bytes: startup_info,
        },
        process_image_path_a: RuntimeProcessImagePathA {
            source_authority: "kernel32-thread-dispatcher-test".to_owned(),
            bytes_with_nul: b"C:\\Vine\\thread-api.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: RuntimeCurrentDirectoryA {
            source_authority: "kernel32-thread-dispatcher-bootstrap".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: vine_core::LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec01_0000,
    }
}

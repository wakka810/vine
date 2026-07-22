use vine_core::{
    ApiDeferredCall, ApiDispatcher, ApiEndpointIdentity, DispatchOutcome, ExecutionSession,
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestRange, Kernel32CompatibilityPolicy,
    MemoryLimits, MemoryOwnerKind, MemoryRegionKind, PageProtection, ProcessAddressSpaceLayout,
    RegisterUpdate, RuntimeBootstrap, RuntimeClockState, RuntimeDynamicEndpoint, RuntimeHandle,
    RuntimeObjectKind, RuntimeProcessId, RuntimeProcessLifecycleState, RuntimeSchedulerEvent,
    RuntimeSourceReadHostResponse, RuntimeSourceSnapshotManifest, RuntimeStartupInfo, RuntimeState,
    RuntimeThreadId, SchedulerThreadState, complete_kernel32_source_read,
    dispatch_kernel32_api_with_policy, kernel32_contract_for_endpoint,
};

const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;
const MEMORY_BASE: GuestAddress = GuestAddress(0x2000_0000);
const PATH_ADDRESS: u32 = 0x2000_1000;
const BUFFER_ADDRESS: u32 = 0x2000_2000;
const BYTES_READ_ADDRESS: u32 = 0x2000_2f00;
const FIRST_FILE_HANDLE: u32 = 0x4601_0001;

#[test]
fn source_file_api_slice_opens_reads_seeks_and_closes_atomically() {
    let fixture = include_bytes!("../../../fixtures/storage/source-read-v1.dat");
    let policy = source_policy();
    let manifest = source_manifest();
    let (mut memory, owner) = initialized_memory();
    let endpoints = [
        endpoint(1, "CreateFileA"),
        endpoint(2, "GetFileSize"),
        endpoint(3, "ReadFile"),
        endpoint(4, "SetFilePointer"),
        endpoint(5, "CloseHandle"),
    ];
    let contracts = endpoints
        .iter()
        .cloned()
        .map(|endpoint| {
            kernel32_contract_for_endpoint(endpoint)
                .expect("valid file API contract")
                .expect("implemented file API contract")
        })
        .collect::<Vec<_>>();
    let dispatcher = ApiDispatcher::new(endpoints.clone(), contracts).expect("file dispatcher");

    write_initialization(
        &mut memory,
        owner,
        GuestAddress(PATH_ADDRESS),
        b"fixtures\\storage\\source-read-v1.dat\0",
    );
    let frames = [
        (
            0x2000_0000,
            vec![
                0x1000_0010,
                PATH_ADDRESS,
                0x8000_0000,
                1,
                0,
                3,
                0x0800_0020,
                0,
            ],
        ),
        (0x2000_0040, vec![0x1000_0020, FIRST_FILE_HANDLE, 0]),
        (
            0x2000_0080,
            vec![
                0x1000_0030,
                FIRST_FILE_HANDLE,
                BUFFER_ADDRESS,
                16,
                BYTES_READ_ADDRESS,
                0,
            ],
        ),
        (0x2000_00c0, vec![0x1000_0040, FIRST_FILE_HANDLE, 5, 0, 0]),
        (
            0x2000_0100,
            vec![
                0x1000_0050,
                FIRST_FILE_HANDLE,
                BUFFER_ADDRESS,
                8,
                BYTES_READ_ADDRESS,
                0,
            ],
        ),
        (0x2000_0140, vec![0x1000_0060, FIRST_FILE_HANDLE]),
        (0x2000_0180, vec![0x1000_0070, FIRST_FILE_HANDLE, 0]),
    ];
    for (address, words) in &frames {
        let bytes = words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>();
        write_initialization(&mut memory, owner, GuestAddress(*address), &bytes);
    }
    memory.seal_owner(owner).expect("seal fixture memory");
    acknowledge_initial_reflection(&mut memory);

    let mut runtime = controlled_runtime();
    runtime
        .register_source_snapshot(manifest)
        .expect("register controlled source snapshot");

    let create = dispatch(
        &dispatcher,
        &endpoints[0],
        frames[0].0,
        &policy,
        &mut memory,
        &mut runtime,
    );
    let create = expect_immediate(create);
    assert_eq!(create.eax, RegisterUpdate::Set(FIRST_FILE_HANDLE));
    assert_eq!(create.last_error, 0x1234);
    let handle = RuntimeHandle(FIRST_FILE_HANDLE);
    assert_eq!(
        runtime
            .object_for_handle(handle, RuntimeObjectKind::File)
            .expect("file object")
            .open_handles,
        1
    );

    let size = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[1],
        frames[1].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(size.eax, RegisterUpdate::Set(fixture.len() as u32));

    let first_call = expect_deferred(dispatch(
        &dispatcher,
        &endpoints[2],
        frames[2].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(
        decode_words(&first_call.request.payload),
        vec![1, 1, 1, 0, 0, 16]
    );
    assert_eq!(
        runtime.scheduler_state().expect("waiting scheduler"),
        SchedulerThreadState::WaitingHost { request_id: 1 }
    );

    let runtime_before_wrong = runtime.clone();
    let buffer_before_wrong = memory
        .read(GuestAddress(BUFFER_ADDRESS), 16)
        .expect("buffer before wrong completion")
        .to_vec();
    let (wrong, completion) = complete_kernel32_source_read(
        &dispatcher,
        first_call.clone(),
        RuntimeSourceReadHostResponse {
            request_id: 1,
            source_id: 1,
            object_id: 99,
            offset: 0,
            requested_bytes: 16,
            transferred_bytes: 16,
            win32_error: 0,
            bytes: fixture[..16].to_vec(),
        },
        &mut ExecutionSession::new(&mut memory, &mut runtime),
    );
    assert!(matches!(wrong, DispatchOutcome::Fault(_)));
    assert!(completion.is_none());
    assert_eq!(runtime, runtime_before_wrong);
    assert_eq!(
        memory
            .read(GuestAddress(BUFFER_ADDRESS), 16)
            .expect("buffer after wrong completion"),
        buffer_before_wrong
    );

    memory
        .protect(
            owner,
            GuestRange::new(GuestAddress(BUFFER_ADDRESS), GUEST_PAGE_SIZE)
                .expect("source output page range"),
            PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .expect("make source output page read-only");
    acknowledge_pending_delta(&mut memory);
    let runtime_before_memory_fault = runtime.clone();
    let memory_before_memory_fault = memory.stats();
    let (memory_fault, memory_fault_completion) = complete_kernel32_source_read(
        &dispatcher,
        first_call.clone(),
        RuntimeSourceReadHostResponse {
            request_id: 1,
            source_id: 1,
            object_id: 1,
            offset: 0,
            requested_bytes: 16,
            transferred_bytes: 16,
            win32_error: 0,
            bytes: fixture[..16].to_vec(),
        },
        &mut ExecutionSession::new(&mut memory, &mut runtime),
    );
    assert!(matches!(memory_fault, DispatchOutcome::Fault(_)));
    assert!(memory_fault_completion.is_none());
    assert_eq!(runtime, runtime_before_memory_fault);
    assert_eq!(memory.stats(), memory_before_memory_fault);
    assert_eq!(
        memory
            .read(GuestAddress(BUFFER_ADDRESS), 16)
            .expect("buffer after failed guest-memory completion"),
        buffer_before_wrong
    );
    memory
        .protect(
            owner,
            GuestRange::new(GuestAddress(BUFFER_ADDRESS), GUEST_PAGE_SIZE)
                .expect("source output page range"),
            PageProtection {
                read: true,
                write: true,
                execute: false,
            },
        )
        .expect("restore source output page writes");
    acknowledge_pending_delta(&mut memory);

    let (first_outcome, first_completion) = complete_kernel32_source_read(
        &dispatcher,
        first_call.clone(),
        RuntimeSourceReadHostResponse {
            request_id: 1,
            source_id: 1,
            object_id: 1,
            offset: 0,
            requested_bytes: 16,
            transferred_bytes: 16,
            win32_error: 0,
            bytes: fixture[..16].to_vec(),
        },
        &mut ExecutionSession::new(&mut memory, &mut runtime),
    );
    let first_outcome = expect_immediate(first_outcome);
    assert_eq!(first_outcome.eax, RegisterUpdate::Set(1));
    assert_eq!(
        first_completion.expect("source completion").cursor_after,
        16
    );
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(BUFFER_ADDRESS), 16)
            .expect("first source bytes"),
        &fixture[..16]
    );
    assert_eq!(
        memory
            .read(GuestAddress(BYTES_READ_ADDRESS), 4)
            .expect("first byte count"),
        16_u32.to_le_bytes()
    );
    assert!(matches!(
        runtime.pop_scheduler_event(),
        Some(RuntimeSchedulerEvent::HostCompleted(_))
    ));

    let runtime_before_duplicate = runtime.clone();
    let (duplicate, duplicate_completion) = complete_kernel32_source_read(
        &dispatcher,
        first_call,
        RuntimeSourceReadHostResponse {
            request_id: 1,
            source_id: 1,
            object_id: 1,
            offset: 0,
            requested_bytes: 16,
            transferred_bytes: 16,
            win32_error: 0,
            bytes: fixture[..16].to_vec(),
        },
        &mut ExecutionSession::new(&mut memory, &mut runtime),
    );
    assert!(matches!(duplicate, DispatchOutcome::Fault(_)));
    assert!(duplicate_completion.is_none());
    assert_eq!(runtime, runtime_before_duplicate);

    let seek = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[3],
        frames[3].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(seek.eax, RegisterUpdate::Set(5));
    let second_call = expect_deferred(dispatch(
        &dispatcher,
        &endpoints[2],
        frames[4].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(
        decode_words(&second_call.request.payload),
        vec![1, 1, 1, 5, 0, 8]
    );
    let (second_outcome, second_completion) = complete_kernel32_source_read(
        &dispatcher,
        second_call,
        RuntimeSourceReadHostResponse {
            request_id: 2,
            source_id: 1,
            object_id: 1,
            offset: 5,
            requested_bytes: 8,
            transferred_bytes: 8,
            win32_error: 0,
            bytes: fixture[5..13].to_vec(),
        },
        &mut ExecutionSession::new(&mut memory, &mut runtime),
    );
    assert_eq!(expect_immediate(second_outcome).eax, RegisterUpdate::Set(1));
    assert_eq!(
        second_completion
            .expect("second source completion")
            .cursor_after,
        13
    );
    assert!(matches!(
        runtime.pop_scheduler_event(),
        Some(RuntimeSchedulerEvent::HostCompleted(_))
    ));
    acknowledge_pending_delta(&mut memory);

    let close = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[4],
        frames[5].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(close.eax, RegisterUpdate::Set(1));
    assert!(runtime.source_file_for_handle(handle).is_err());
    let invalid_size = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[1],
        frames[6].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(invalid_size.eax, RegisterUpdate::Set(u32::MAX));
    assert_eq!(invalid_size.last_error, 6);
}

#[test]
fn source_file_sharing_mount_and_pending_exit_paths_preserve_authority() {
    let mut runtime = controlled_runtime();
    runtime
        .register_source_snapshot(source_manifest())
        .expect("register controlled source snapshot");
    let first = runtime
        .open_source_file_a(
            b"C:\\Vine\\fixtures\\storage\\source-read-v1.dat\0",
            0x8000_0000,
            0,
            0x0800_0020,
        )
        .expect("first source open")
        .expect("known source path");
    let before_share_rejection = runtime.clone();
    let sharing = runtime
        .open_source_file_a(
            b"c:/vine/FIXTURES/STORAGE/SOURCE-READ-V1.DAT\0",
            0x8000_0000,
            1,
            0x0800_0020,
        )
        .expect_err("read share must conflict");
    assert_eq!(sharing.code, "source-file-sharing-violation");
    assert_eq!(runtime, before_share_rejection);
    let outside = runtime
        .open_source_file_a(b"..\\outside.dat\0", 0x8000_0000, 1, 0x0800_0020)
        .expect_err("mount escape must fail");
    assert_eq!(outside.code, "source-path-outside-mount");
    assert_eq!(runtime, before_share_rejection);

    let request = runtime
        .begin_source_file_read(
            first.handle,
            GuestAddress(BUFFER_ADDRESS),
            GuestAddress(BYTES_READ_ADDRESS),
            8,
        )
        .expect("begin source read");
    assert_eq!(request.request_id, 1);
    let cancellation = runtime
        .terminate_current_process(9)
        .expect("direct termination cancels pending source read");
    assert_eq!(cancellation.host_requests_cancelled, 1);
    assert_eq!(
        runtime.process().lifecycle(),
        RuntimeProcessLifecycleState::Exited { exit_code: 9 }
    );
    assert!(runtime.source_file_for_handle(first.handle).is_err());
    assert_eq!(
        runtime
            .scheduler_state_for_thread(RuntimeThreadId(THREAD_ID))
            .expect("terminated scheduler"),
        SchedulerThreadState::Terminated { exit_code: 9 }
    );
}

fn dispatch(
    dispatcher: &ApiDispatcher,
    endpoint: &ApiEndpointIdentity,
    stack_pointer: u32,
    policy: &Kernel32CompatibilityPolicy,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
) -> DispatchOutcome {
    let return_address = u32::from_le_bytes(
        memory
            .read(GuestAddress(stack_pointer), 4)
            .expect("call return address")
            .try_into()
            .expect("x86 word"),
    );
    dispatcher.dispatch_owned(
        vine_core::ApiOwnedTrap {
            endpoint_id: endpoint.id,
            endpoint_address: endpoint.address,
            stack_pointer: GuestAddress(stack_pointer),
            return_address: GuestAddress(return_address),
        },
        &mut ExecutionSession::new(memory, runtime),
        |contract, frame, session| {
            dispatch_kernel32_api_with_policy(policy, contract, frame, session)
        },
    )
}

fn expect_immediate(outcome: DispatchOutcome) -> vine_core::ApiImmediateCompletion {
    match outcome {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected immediate completion, got {other:?}"),
    }
}

fn expect_deferred(outcome: DispatchOutcome) -> ApiDeferredCall {
    match outcome {
        DispatchOutcome::Deferred(call) => call,
        other => panic!("expected deferred call, got {other:?}"),
    }
}

fn endpoint(id: u32, name: &str) -> ApiEndpointIdentity {
    ApiEndpointIdentity::from_dynamic_endpoint(&RuntimeDynamicEndpoint {
        namespace: "vine.test-source-endpoint.v1".to_owned(),
        id,
        address: GuestAddress(0x7f00_1000 + id * 0x10),
        module: "kernel32.dll".to_owned(),
        name: name.to_owned(),
        lifetime: "test-process".to_owned(),
    })
}

fn initialized_memory() -> (GuestMemory, vine_core::MemoryOwnerId) {
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 4,
        max_regions: 4,
        max_owners: 4,
        max_access_bytes: 1024 * 1024,
    })
    .expect("guest memory");
    let owner = memory
        .create_owner(MemoryOwnerKind::Runtime)
        .expect("memory owner");
    let range = memory
        .reserve_fixed(owner, MemoryRegionKind::Runtime, MEMORY_BASE, 0x3000)
        .expect("memory reservation");
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
        .expect("memory commit");
    (memory, owner)
}

fn controlled_runtime() -> RuntimeState {
    let mut startup = vec![0; 68];
    startup[..4].copy_from_slice(&68_u32.to_le_bytes());
    let mut runtime = RuntimeState::new(RuntimeBootstrap {
        process_id: RuntimeProcessId(PROCESS_ID),
        initial_thread_id: RuntimeThreadId(THREAD_ID),
        initial_last_error: 0x1234,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
            performance_frequency: 10_000_000,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "source-file-gateway-v1".to_owned(),
            bytes: startup,
        },
        process_image_path_a: vine_core::RuntimeProcessImagePathA {
            source_authority: "source-file-gateway-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: vine_core::RuntimeCurrentDirectoryA {
            source_authority: "source-file-gateway-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: vine_core::LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec01_0000,
    })
    .expect("runtime state");
    runtime
        .start_initial_thread()
        .expect("start initial thread");
    runtime
}

fn source_policy() -> Kernel32CompatibilityPolicy {
    let policy: Kernel32CompatibilityPolicy = serde_json::from_str(include_str!(
        "../../../fixtures/runtime-policies/kernel32-source-file-read-v1.json"
    ))
    .expect("source file policy JSON");
    policy.validate().expect("valid source file policy");
    policy
}

fn source_manifest() -> RuntimeSourceSnapshotManifest {
    serde_json::from_str(include_str!(
        "../../../fixtures/source-snapshots/controlled-source-read-v1.json"
    ))
    .expect("source snapshot manifest")
}

fn write_initialization(
    memory: &mut GuestMemory,
    owner: vine_core::MemoryOwnerId,
    address: GuestAddress,
    bytes: &[u8],
) {
    memory
        .write_initialization(owner, address, bytes)
        .expect("fixture initialization");
}

fn acknowledge_initial_reflection(memory: &mut GuestMemory) {
    let generation = memory
        .pending_reflection()
        .expect("initial reflection")
        .invalidation
        .generation;
    memory
        .acknowledge_delta(generation)
        .expect("acknowledge initial reflection");
}

fn acknowledge_pending_delta(memory: &mut GuestMemory) {
    let generation = memory.pending_delta().expect("pending delta").generation;
    memory
        .acknowledge_delta(generation)
        .expect("acknowledge pending delta");
}

fn decode_words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().expect("x86 word")))
        .collect()
}

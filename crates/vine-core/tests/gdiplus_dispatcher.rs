use vine_core::{
    ApiBehavior, ApiDirection, ApiDispatcher, ApiEndpointIdentity, ApiFaultClass, ApiOwnedTrap,
    ApiReturnSchema, ApiValueSchema, ApiWriteRequirement, CallbackPolicy, CallingConvention,
    DispatchOutcome, ExecutionSession, GDIPLUS_SHUTDOWN_IMPLEMENTATION,
    GDIPLUS_STARTUP_IMPLEMENTATION, GDIPLUS_STATUS_OK, GDIPLUS_STATUS_UNSUPPORTED_GDIPLUS_VERSION,
    GdiplusCompatibilityPolicy, GuestAddress, GuestMemory, ImportTarget, ImportThunk,
    ImportThunkDisposition, ImportThunkId, LastErrorPolicy, MemoryLimits, MemoryOwnerId,
    MemoryOwnerKind, MemoryRegionKind, PageProtection, ProcessAddressSpaceLayout,
    RUNTIME_GDIPLUS_SESSION_TOKEN_PREFIX, ReentrancyPolicy, RegisterUpdate, RuntimeBootstrap,
    RuntimeClockState, RuntimeCurrentDirectoryA, RuntimeProcessId, RuntimeProcessImagePathA,
    RuntimeStartupInfo, RuntimeState, RuntimeThreadId, StackCleanup,
    X86_GDIPLUS_STARTUP_INPUT_BYTE_LENGTH, X86_GDIPLUS_STARTUP_OUTPUT_BYTE_LENGTH,
    dispatch_gdiplus_api_with_policy, gdiplus_contract_for_endpoint,
};

const MEMORY_BASE: GuestAddress = GuestAddress(0x2000_0000);
const MEMORY_BYTES: u64 = 0x4000;
const STACK: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x100);
const TOKEN_OUTPUT: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x800);
const SECOND_TOKEN_OUTPUT: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x804);
const STARTUP_INPUT: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x900);
const STARTUP_OUTPUT: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0xa00);
const STARTUP_IAT: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2000);
const SHUTDOWN_IAT: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2004);
const STARTUP_ENDPOINT: GuestAddress = GuestAddress(0x7f00_1430);
const SHUTDOWN_ENDPOINT: GuestAddress = GuestAddress(0x7f00_1420);
const RETURN_ADDRESS: GuestAddress = GuestAddress(0x1006_52c8);
const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;
const LAST_ERROR_SENTINEL: u32 = 0x1234_5678;
const TOKEN_SENTINEL: u32 = 0xcccc_cccc;

struct Fixture {
    memory: GuestMemory,
    runtime: RuntimeState,
    dispatcher: ApiDispatcher,
    startup_endpoint: ApiEndpointIdentity,
    shutdown_endpoint: ApiEndpointIdentity,
    policy: GdiplusCompatibilityPolicy,
}

impl Fixture {
    fn new() -> Self {
        let (mut memory, owner) = initialized_memory();
        let startup_endpoint =
            endpoint(owner, 324, STARTUP_ENDPOINT, STARTUP_IAT, "GdiplusStartup");
        let shutdown_endpoint = endpoint(
            owner,
            323,
            SHUTDOWN_ENDPOINT,
            SHUTDOWN_IAT,
            "GdiplusShutdown",
        );
        memory
            .write_initialization(owner, STARTUP_IAT, &STARTUP_ENDPOINT.0.to_le_bytes())
            .unwrap();
        memory
            .write_initialization(owner, SHUTDOWN_IAT, &SHUTDOWN_ENDPOINT.0.to_le_bytes())
            .unwrap();
        memory.seal_owner(owner).unwrap();
        let reflection = memory.pending_reflection().unwrap();
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .unwrap();
        let contracts = [&startup_endpoint, &shutdown_endpoint]
            .into_iter()
            .map(|endpoint| {
                gdiplus_contract_for_endpoint(endpoint.clone())
                    .unwrap()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let dispatcher = ApiDispatcher::new(
            [startup_endpoint.clone(), shutdown_endpoint.clone()],
            contracts,
        )
        .unwrap();
        Self {
            memory,
            runtime: controlled_runtime(),
            dispatcher,
            startup_endpoint,
            shutdown_endpoint,
            policy: GdiplusCompatibilityPolicy::controlled_v1(),
        }
    }

    fn prepare_startup(
        &mut self,
        stack: GuestAddress,
        token_output: u32,
        startup_input: [u32; 4],
        output: u32,
    ) {
        write_words(&mut self.memory, STARTUP_INPUT, &startup_input);
        if token_output != 0 {
            write_words(
                &mut self.memory,
                GuestAddress(token_output),
                &[TOKEN_SENTINEL],
            );
        }
        write_words(
            &mut self.memory,
            stack,
            &[RETURN_ADDRESS.0, token_output, STARTUP_INPUT.0, output],
        );
        acknowledge_pending(&mut self.memory);
    }

    fn dispatch_startup(&mut self, stack: GuestAddress) -> DispatchOutcome {
        let policy = self.policy.clone();
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        self.dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: self.startup_endpoint.id,
                endpoint_address: self.startup_endpoint.address,
                stack_pointer: stack,
                return_address: RETURN_ADDRESS,
            },
            &mut session,
            |contract, frame, session| {
                dispatch_gdiplus_api_with_policy(&policy, contract, frame, session)
            },
        )
    }

    fn dispatch_shutdown(&mut self, stack: GuestAddress, token: u32) -> DispatchOutcome {
        write_words(&mut self.memory, stack, &[RETURN_ADDRESS.0, token]);
        acknowledge_pending(&mut self.memory);
        let policy = self.policy.clone();
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        self.dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: self.shutdown_endpoint.id,
                endpoint_address: self.shutdown_endpoint.address,
                stack_pointer: stack,
                return_address: RETURN_ADDRESS,
            },
            &mut session,
            |contract, frame, session| {
                dispatch_gdiplus_api_with_policy(&policy, contract, frame, session)
            },
        )
    }
}

#[test]
fn catalog_exposes_x86_startup_and_shutdown_lifetime_contracts() {
    let fixture = Fixture::new();
    let startup = gdiplus_contract_for_endpoint(fixture.startup_endpoint)
        .unwrap()
        .unwrap();
    assert_eq!(startup.implementation_id, GDIPLUS_STARTUP_IMPLEMENTATION);
    assert_eq!(startup.abi.calling_convention, CallingConvention::Stdcall);
    assert_eq!(startup.abi.arguments.len(), 3);
    assert_eq!(startup.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(startup.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(startup.last_error_policy, LastErrorPolicy::Preserve);
    assert_eq!(startup.reentrancy_policy, ReentrancyPolicy::Disallowed);
    assert_eq!(startup.behavior, ApiBehavior::Immediate);
    assert_eq!(startup.callback_policy, CallbackPolicy::Forbidden);
    assert_eq!(startup.abi.arguments[0].direction, ApiDirection::Out);
    let ApiValueSchema::Pointer { constraint: token } = &startup.abi.arguments[0].value else {
        panic!("startup token is not an output pointer")
    };
    assert!(!token.nullable);
    assert_eq!(token.byte_length, 4);
    assert_eq!(token.write_requirement, ApiWriteRequirement::Optional);
    let ApiValueSchema::Pointer { constraint: input } = &startup.abi.arguments[1].value else {
        panic!("startup input is not a pointer")
    };
    assert!(!input.nullable);
    assert_eq!(input.byte_length, X86_GDIPLUS_STARTUP_INPUT_BYTE_LENGTH);
    assert_eq!(input.write_requirement, ApiWriteRequirement::None);
    let ApiValueSchema::Pointer { constraint: output } = &startup.abi.arguments[2].value else {
        panic!("startup output is not a pointer")
    };
    assert!(output.nullable);
    assert_eq!(output.byte_length, X86_GDIPLUS_STARTUP_OUTPUT_BYTE_LENGTH);

    let shutdown = gdiplus_contract_for_endpoint(fixture.shutdown_endpoint)
        .unwrap()
        .unwrap();
    assert_eq!(shutdown.implementation_id, GDIPLUS_SHUTDOWN_IMPLEMENTATION);
    assert_eq!(shutdown.abi.arguments.len(), 1);
    assert_eq!(shutdown.abi.return_value, ApiReturnSchema::Void);
    assert_eq!(shutdown.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(shutdown.last_error_policy, LastErrorPolicy::Preserve);
}

#[test]
fn startup_writes_distinct_logical_tokens_and_shutdown_retires_exact_session() {
    let mut fixture = Fixture::new();
    fixture.prepare_startup(STACK, TOKEN_OUTPUT.0, [1, 0, 0, 0], 0);
    let first = immediate(fixture.dispatch_startup(STACK));
    assert_eq!(first.eax, RegisterUpdate::Set(GDIPLUS_STATUS_OK));
    assert_eq!(first.instruction_pointer, RETURN_ADDRESS);
    assert_eq!(first.stack_pointer, GuestAddress(STACK.0 + 16));
    assert_eq!(first.last_error, LAST_ERROR_SENTINEL);
    assert_eq!(first.guest_writes.len(), 1);
    acknowledge_pending(&mut fixture.memory);
    let first_token = read_u32(&fixture.memory, TOKEN_OUTPUT);
    assert_eq!(first_token, RUNTIME_GDIPLUS_SESSION_TOKEN_PREFIX | 1);
    assert_eq!(fixture.runtime.gdiplus_sessions().len(), 1);

    let second_stack = GuestAddress(STACK.0 + 0x40);
    fixture.prepare_startup(second_stack, SECOND_TOKEN_OUTPUT.0, [1, 0, 0, 0], 0);
    let second = immediate(fixture.dispatch_startup(second_stack));
    assert_eq!(second.eax, RegisterUpdate::Set(GDIPLUS_STATUS_OK));
    acknowledge_pending(&mut fixture.memory);
    let second_token = read_u32(&fixture.memory, SECOND_TOKEN_OUTPUT);
    assert_eq!(second_token, RUNTIME_GDIPLUS_SESSION_TOKEN_PREFIX | 2);
    assert_ne!(first_token, second_token);
    assert_eq!(fixture.runtime.gdiplus_sessions().len(), 2);

    let shutdown_stack = GuestAddress(STACK.0 + 0x80);
    let shutdown = immediate(fixture.dispatch_shutdown(shutdown_stack, first_token));
    assert_eq!(shutdown.eax, RegisterUpdate::Preserve);
    assert_eq!(shutdown.stack_pointer, GuestAddress(shutdown_stack.0 + 8));
    assert_eq!(shutdown.last_error, LAST_ERROR_SENTINEL);
    assert!(shutdown.guest_writes.is_empty());
    assert_eq!(fixture.runtime.gdiplus_sessions().len(), 1);
    assert_eq!(
        fixture
            .runtime
            .gdiplus_sessions()
            .values()
            .next()
            .unwrap()
            .token,
        second_token
    );

    let runtime_before = fixture.runtime.clone();
    let duplicate = fault(fixture.dispatch_shutdown(GuestAddress(STACK.0 + 0xc0), first_token));
    assert_eq!(duplicate.class, ApiFaultClass::InvalidCall);
    assert_eq!(duplicate.code, "gdiplus-session-token-unknown");
    assert_eq!(fixture.runtime, runtime_before);

    fixture.runtime.terminate_current_process(0).unwrap();
    assert!(fixture.runtime.gdiplus_sessions().is_empty());
}

#[test]
fn unsupported_version_returns_status_without_writing_or_allocating() {
    let mut fixture = Fixture::new();
    fixture.prepare_startup(STACK, TOKEN_OUTPUT.0, [2, 0, 0, 0], 0);
    let runtime_before = fixture.runtime.clone();
    let memory_before = fixture.memory.stats();
    let completion = immediate(fixture.dispatch_startup(STACK));
    assert_eq!(
        completion.eax,
        RegisterUpdate::Set(GDIPLUS_STATUS_UNSUPPORTED_GDIPLUS_VERSION)
    );
    assert_eq!(completion.last_error, LAST_ERROR_SENTINEL);
    assert!(completion.guest_writes.is_empty());
    assert_eq!(read_u32(&fixture.memory, TOKEN_OUTPUT), TOKEN_SENTINEL);
    assert_eq!(fixture.runtime, runtime_before);
    assert_eq!(fixture.memory.stats(), memory_before);
}

#[test]
fn unreached_variants_and_bad_pointers_fail_before_authority_mutation() {
    for (debug_callback, suppress_background, suppress_external, output) in [
        (0x1000_0000, 0, 0, 0),
        (0, 1, 0, 0),
        (0, 0, 1, 0),
        (0, 0, 0, STARTUP_OUTPUT.0),
    ] {
        let mut fixture = Fixture::new();
        fixture.prepare_startup(
            STACK,
            TOKEN_OUTPUT.0,
            [1, debug_callback, suppress_background, suppress_external],
            output,
        );
        let runtime_before = fixture.runtime.clone();
        let memory_before = fixture.memory.stats();
        let rejected = fault(fixture.dispatch_startup(STACK));
        assert_eq!(rejected.class, ApiFaultClass::Unsupported);
        assert_eq!(rejected.code, "gdiplus-startup-variant-unsupported");
        assert_eq!(read_u32(&fixture.memory, TOKEN_OUTPUT), TOKEN_SENTINEL);
        assert_eq!(fixture.runtime, runtime_before);
        assert_eq!(fixture.memory.stats(), memory_before);
    }

    for (token_output, input) in [(0, STARTUP_INPUT.0), (TOKEN_OUTPUT.0, 0x1000)] {
        let mut fixture = Fixture::new();
        fixture.prepare_startup(STACK, token_output, [1, 0, 0, 0], 0);
        if input != STARTUP_INPUT.0 {
            write_words(
                &mut fixture.memory,
                STACK,
                &[RETURN_ADDRESS.0, token_output, input, 0],
            );
            acknowledge_pending(&mut fixture.memory);
        }
        let runtime_before = fixture.runtime.clone();
        let rejected = fault(fixture.dispatch_startup(STACK));
        assert_eq!(rejected.class, ApiFaultClass::InvalidCall);
        assert_eq!(fixture.runtime, runtime_before);
        assert!(fixture.runtime.gdiplus_sessions().is_empty());
    }
}

fn endpoint(
    owner: MemoryOwnerId,
    id: u32,
    address: GuestAddress,
    iat_address: GuestAddress,
    name: &str,
) -> ApiEndpointIdentity {
    ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(id),
        address,
        requester_owner: owner,
        module: "gdiplus.dll".to_owned(),
        iat_rva: vine_core::Rva(iat_address.0 - MEMORY_BASE.0),
        iat_address,
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: name.to_owned(),
            hint: 0,
            name_rva: vine_core::Rva(0),
        },
    })
}

fn initialized_memory() -> (GuestMemory, MemoryOwnerId) {
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 4,
        max_regions: 1,
        max_owners: 1,
        max_access_bytes: MEMORY_BYTES as usize,
    })
    .unwrap();
    let owner = memory.create_owner(MemoryOwnerKind::Runtime).unwrap();
    let range = memory
        .reserve_fixed(owner, MemoryRegionKind::Runtime, MEMORY_BASE, MEMORY_BYTES)
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
    (memory, owner)
}

fn controlled_runtime() -> RuntimeState {
    let mut startup = vec![0; 68];
    startup[..4].copy_from_slice(&68_u32.to_le_bytes());
    let mut runtime = RuntimeState::new(RuntimeBootstrap {
        process_id: RuntimeProcessId(PROCESS_ID),
        initial_thread_id: RuntimeThreadId(THREAD_ID),
        initial_last_error: LAST_ERROR_SENTINEL,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
            performance_frequency: 10_000_000,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "gdiplus-test-startup".to_owned(),
            bytes: startup,
        },
        process_image_path_a: RuntimeProcessImagePathA {
            source_authority: "gdiplus-test".to_owned(),
            bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: RuntimeCurrentDirectoryA {
            source_authority: "gdiplus-test".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: vine_core::LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec01_0000,
    })
    .unwrap();
    runtime.start_initial_thread().unwrap();
    runtime
}

fn write_words(memory: &mut GuestMemory, address: GuestAddress, words: &[u32]) {
    let bytes = words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<_>>();
    memory.write(address, &bytes).unwrap();
}

fn read_u32(memory: &GuestMemory, address: GuestAddress) -> u32 {
    u32::from_le_bytes(memory.read(address, 4).unwrap().try_into().unwrap())
}

fn acknowledge_pending(memory: &mut GuestMemory) {
    if memory.stats().pending_invalidation.is_some() {
        let delta = memory.pending_delta().unwrap();
        memory.acknowledge_delta(delta.generation).unwrap();
    }
}

fn immediate(outcome: DispatchOutcome) -> vine_core::ApiImmediateCompletion {
    match outcome {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected immediate completion, got {other:?}"),
    }
}

fn fault(outcome: DispatchOutcome) -> vine_core::ApiFault {
    match outcome {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected API fault, got {other:?}"),
    }
}

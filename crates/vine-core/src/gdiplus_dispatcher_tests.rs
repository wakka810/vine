use crate as vine_core;
use crate::gdiplus::{
    GDIPLUS_STATUS_OK, GDIPLUS_STATUS_UNSUPPORTED_GDIPLUS_VERSION, dispatch_gdiplus_api,
};
use crate::imports::{ImportThunk, ImportThunkDisposition, ImportThunkId};
use crate::runtime::RUNTIME_GDIPLUS_SESSION_TOKEN_PREFIX;
use vine_core::{
    ApiDispatcher, ApiEndpointIdentity, ApiOwnedTrap, DispatchOutcome, GuestAddress, GuestMemory,
    ImportTarget, MemoryLimits, MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind, PageProtection,
    Runtime, RuntimeBootstrap, RuntimeClockState, RuntimeCurrentDirectoryA, RuntimeProcessId,
    RuntimeProcessImagePathA, RuntimeStartupInfo, RuntimeThreadId,
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
    runtime: Runtime,
    dispatcher: ApiDispatcher,
    startup_endpoint: ApiEndpointIdentity,
    shutdown_endpoint: ApiEndpointIdentity,
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
        let dispatcher =
            ApiDispatcher::from_endpoints([startup_endpoint.clone(), shutdown_endpoint.clone()])
                .unwrap();
        Self {
            runtime: controlled_runtime(memory),
            dispatcher,
            startup_endpoint,
            shutdown_endpoint,
        }
    }

    fn prepare_startup(
        &mut self,
        stack: GuestAddress,
        token_output: u32,
        startup_input: [u32; 4],
        output: u32,
    ) {
        write_words(&mut self.runtime.memory, STARTUP_INPUT, &startup_input);
        if token_output != 0 {
            write_words(
                &mut self.runtime.memory,
                GuestAddress(token_output),
                &[TOKEN_SENTINEL],
            );
        }
        write_words(
            &mut self.runtime.memory,
            stack,
            &[RETURN_ADDRESS.0, token_output, STARTUP_INPUT.0, output],
        );
        acknowledge_pending(&mut self.runtime.memory);
    }

    fn dispatch_startup(&mut self, stack: GuestAddress) -> DispatchOutcome {
        self.dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: self.startup_endpoint.id,
                endpoint_address: self.startup_endpoint.address,
                stack_pointer: stack,
                return_address: RETURN_ADDRESS,
            },
            &mut self.runtime,
            None,
            |frame, runtime, _| dispatch_gdiplus_api(frame, runtime),
        )
    }

    fn dispatch_shutdown(&mut self, stack: GuestAddress, token: u32) -> DispatchOutcome {
        write_words(&mut self.runtime.memory, stack, &[RETURN_ADDRESS.0, token]);
        acknowledge_pending(&mut self.runtime.memory);
        self.dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: self.shutdown_endpoint.id,
                endpoint_address: self.shutdown_endpoint.address,
                stack_pointer: stack,
                return_address: RETURN_ADDRESS,
            },
            &mut self.runtime,
            None,
            |frame, runtime, _| dispatch_gdiplus_api(frame, runtime),
        )
    }
}

#[test]
fn startup_writes_distinct_logical_tokens_and_shutdown_retires_exact_session() {
    let mut fixture = Fixture::new();
    fixture.prepare_startup(STACK, TOKEN_OUTPUT.0, [1, 0, 0, 0], 0);
    let first = immediate(fixture.dispatch_startup(STACK));
    assert_eq!(first.eax, Some(GDIPLUS_STATUS_OK));
    assert_eq!(first.instruction_pointer, RETURN_ADDRESS);
    assert_eq!(first.stack_pointer, GuestAddress(STACK.0 + 16));
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
    acknowledge_pending(&mut fixture.runtime.memory);
    let first_token = read_u32(&fixture.runtime.memory, TOKEN_OUTPUT);
    assert_eq!(first_token, RUNTIME_GDIPLUS_SESSION_TOKEN_PREFIX | 1);

    let second_stack = GuestAddress(STACK.0 + 0x40);
    fixture.prepare_startup(second_stack, SECOND_TOKEN_OUTPUT.0, [1, 0, 0, 0], 0);
    let second = immediate(fixture.dispatch_startup(second_stack));
    assert_eq!(second.eax, Some(GDIPLUS_STATUS_OK));
    acknowledge_pending(&mut fixture.runtime.memory);
    let second_token = read_u32(&fixture.runtime.memory, SECOND_TOKEN_OUTPUT);
    assert_eq!(second_token, RUNTIME_GDIPLUS_SESSION_TOKEN_PREFIX | 2);
    assert_ne!(first_token, second_token);

    let shutdown_stack = GuestAddress(STACK.0 + 0x80);
    let shutdown = immediate(fixture.dispatch_shutdown(shutdown_stack, first_token));
    assert_eq!(shutdown.eax, None);
    assert_eq!(shutdown.stack_pointer, GuestAddress(shutdown_stack.0 + 8));
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );

    fault(fixture.dispatch_shutdown(GuestAddress(STACK.0 + 0xc0), first_token));

    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );

    let second_shutdown = immediate(fixture.dispatch_shutdown(STACK, second_token));
    assert_eq!(second_shutdown.eax, None);
    fault(fixture.dispatch_shutdown(STACK, second_token));
}

#[test]
fn unsupported_version_returns_status_without_writing_or_allocating() {
    let mut fixture = Fixture::new();
    fixture.prepare_startup(STACK, TOKEN_OUTPUT.0, [2, 0, 0, 0], 0);
    let clock_before = fixture.runtime.clock().clone();
    let memory_before = fixture.runtime.memory.stats();
    let completion = immediate(fixture.dispatch_startup(STACK));
    assert_eq!(
        completion.eax,
        Some(GDIPLUS_STATUS_UNSUPPORTED_GDIPLUS_VERSION)
    );
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
    assert_eq!(
        read_u32(&fixture.runtime.memory, TOKEN_OUTPUT),
        TOKEN_SENTINEL
    );

    assert_eq!(fixture.runtime.clock(), &clock_before);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
    assert_eq!(fixture.runtime.memory.stats(), memory_before);
    assert_first_startup_token(&mut fixture);
}

#[test]
fn capacity_failure_returns_status_without_publishing_a_token() {
    let mut fixture = Fixture::new();
    for _ in 0..crate::runtime::RUNTIME_GDIPLUS_SESSION_LIMIT {
        fixture.prepare_startup(STACK, TOKEN_OUTPUT.0, [1, 0, 0, 0], 0);
        assert_eq!(
            immediate(fixture.dispatch_startup(STACK)).eax,
            Some(GDIPLUS_STATUS_OK)
        );
    }
    fixture.prepare_startup(STACK, TOKEN_OUTPUT.0, [1, 0, 0, 0], 0);
    let clock_before = fixture.runtime.clock().clone();
    let memory_before = fixture.runtime.memory.stats();
    let completion = immediate(fixture.dispatch_startup(STACK));
    assert_eq!(
        completion.eax,
        Some(crate::gdiplus::GDIPLUS_STATUS_OUT_OF_MEMORY)
    );
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
    assert_eq!(
        read_u32(&fixture.runtime.memory, TOKEN_OUTPUT),
        TOKEN_SENTINEL
    );

    assert_eq!(fixture.runtime.clock(), &clock_before);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
    assert_eq!(fixture.runtime.memory.stats(), memory_before);
    let first_token = RUNTIME_GDIPLUS_SESSION_TOKEN_PREFIX | 1;
    immediate(fixture.dispatch_shutdown(STACK, first_token));
    fixture.prepare_startup(STACK, TOKEN_OUTPUT.0, [1, 0, 0, 0], 0);
    assert_eq!(
        immediate(fixture.dispatch_startup(STACK)).eax,
        Some(GDIPLUS_STATUS_OK)
    );
    acknowledge_pending(&mut fixture.runtime.memory);
    assert_eq!(
        read_u32(&fixture.runtime.memory, TOKEN_OUTPUT),
        RUNTIME_GDIPLUS_SESSION_TOKEN_PREFIX
            | (crate::runtime::RUNTIME_GDIPLUS_SESSION_LIMIT as u32 + 1)
    );
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
        let clock_before = fixture.runtime.clock().clone();
        let memory_before = fixture.runtime.memory.stats();
        fault(fixture.dispatch_startup(STACK));
        assert_eq!(
            read_u32(&fixture.runtime.memory, TOKEN_OUTPUT),
            TOKEN_SENTINEL
        );

        assert_eq!(fixture.runtime.clock(), &clock_before);
        assert_eq!(
            fixture.runtime.current_last_error().unwrap(),
            LAST_ERROR_SENTINEL
        );
        assert_eq!(fixture.runtime.memory.stats(), memory_before);
        assert_first_startup_token(&mut fixture);
    }

    for (token_output, input) in [
        (0, STARTUP_INPUT.0),
        (TOKEN_OUTPUT.0, 0x1000),
        (TOKEN_OUTPUT.0, MEMORY_BASE.0 + MEMORY_BYTES as u32 - 4),
    ] {
        let mut fixture = Fixture::new();
        fixture.prepare_startup(STACK, token_output, [1, 0, 0, 0], 0);
        if input != STARTUP_INPUT.0 {
            write_words(
                &mut fixture.runtime.memory,
                STACK,
                &[RETURN_ADDRESS.0, token_output, input, 0],
            );
            acknowledge_pending(&mut fixture.runtime.memory);
        }
        let clock_before = fixture.runtime.clock().clone();
        fault(fixture.dispatch_startup(STACK));

        assert_eq!(fixture.runtime.clock(), &clock_before);
        assert_eq!(
            fixture.runtime.current_last_error().unwrap(),
            LAST_ERROR_SENTINEL
        );
        assert_first_startup_token(&mut fixture);
    }
}

fn assert_first_startup_token(fixture: &mut Fixture) {
    fixture.prepare_startup(STACK, TOKEN_OUTPUT.0, [1, 0, 0, 0], 0);
    assert_eq!(
        immediate(fixture.dispatch_startup(STACK)).eax,
        Some(GDIPLUS_STATUS_OK)
    );
    acknowledge_pending(&mut fixture.runtime.memory);
    assert_eq!(
        read_u32(&fixture.runtime.memory, TOKEN_OUTPUT),
        RUNTIME_GDIPLUS_SESSION_TOKEN_PREFIX | 1
    );
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

fn controlled_runtime(memory: GuestMemory) -> Runtime {
    let mut startup = vec![0; 68];
    startup[..4].copy_from_slice(&68_u32.to_le_bytes());
    let mut runtime = Runtime::new(
        RuntimeBootstrap {
            process_id: RuntimeProcessId(PROCESS_ID),
            initial_thread_id: RuntimeThreadId(THREAD_ID),
            initial_last_error: LAST_ERROR_SENTINEL,
            clock: RuntimeClockState {
                wall_clock_file_time: 134_282_016_000_000_000,
                tick_count: 3_600_000,
                performance_counter: 12_345_678_901,
                performance_frequency: 10_000_000,
            },
            startup_info: RuntimeStartupInfo { bytes: startup },
            process_image_path_a: RuntimeProcessImagePathA {
                bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
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

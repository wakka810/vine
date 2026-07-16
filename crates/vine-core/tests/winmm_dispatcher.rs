use vine_core::{
    ApiDispatcher, ApiDynamicPointerConstraint, ApiEndpointIdentity, ApiFaultClass, ApiOwnedTrap,
    ApiPointeeSchema, ApiReturnSchema, ApiValueSchema, ApiWriteRequirement, CallbackPolicy,
    CallingConvention, DispatchOutcome, ExecutionSession, GuestAddress, GuestMemory, ImportTarget,
    ImportThunk, ImportThunkDisposition, ImportThunkId, LastErrorPolicy, MemoryLimits,
    MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind, PageProtection, ProcessAddressSpaceLayout,
    ReentrancyPolicy, RegisterUpdate, RuntimeBootstrap, RuntimeClockState,
    RuntimeCurrentDirectoryA, RuntimeProcessId, RuntimeProcessImagePathA, RuntimeStartupInfo,
    RuntimeState, RuntimeThreadId, StackCleanup, TIME_GET_DEV_CAPS_IMPLEMENTATION, TIMERR_STRUCT,
    WinmmCompatibilityPolicy, dispatch_winmm_api_with_policy, winmm_contract_for_endpoint,
};

const MEMORY_BASE: GuestAddress = GuestAddress(0x2000_0000);
const STACK_POINTER: u32 = 0x2000_0100;
const SHORT_STACK_POINTER: u32 = STACK_POINTER + 0x20;
const OVERSIZED_STACK_POINTER: u32 = STACK_POINTER + 0x40;
const NULL_STACK_POINTER: u32 = STACK_POINTER + 0x60;
const FAULT_STACK_POINTER: u32 = STACK_POINTER + 0x80;
const OUTPUT_ADDRESS: GuestAddress = GuestAddress(0x2000_1000);
const SHORT_OUTPUT_ADDRESS: GuestAddress = GuestAddress(0x2000_1020);
const OVERSIZED_OUTPUT_ADDRESS: GuestAddress = GuestAddress(0x2000_1040);
const FAULT_OUTPUT_ADDRESS: GuestAddress = GuestAddress(0x2000_2ffc);
const IAT_ADDRESS: GuestAddress = GuestAddress(0x2000_2000);
const ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0020);
const RETURN_ADDRESS: GuestAddress = GuestAddress(0x1009_1e6d);
const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;
const INITIAL_LAST_ERROR: u32 = 0x1234_5678;

struct TimeGetDevCapsFixture {
    memory: GuestMemory,
    runtime: RuntimeState,
    dispatcher: ApiDispatcher,
    endpoint: ApiEndpointIdentity,
    policy: WinmmCompatibilityPolicy,
}

impl TimeGetDevCapsFixture {
    fn new() -> Self {
        let (mut memory, owner) = initialized_memory();
        let thunk = ImportThunk {
            id: ImportThunkId(3),
            address: ENDPOINT_ADDRESS,
            requester_owner: owner,
            module: "WINMM.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_14d0),
            iat_address: IAT_ADDRESS,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "timeGetDevCaps".to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        };
        memory
            .write_initialization(owner, IAT_ADDRESS, &ENDPOINT_ADDRESS.0.to_le_bytes())
            .expect("initialize timeGetDevCaps IAT");
        write_words(
            &mut memory,
            owner,
            GuestAddress(STACK_POINTER),
            &[RETURN_ADDRESS.0, OUTPUT_ADDRESS.0, 8],
        );
        write_words(
            &mut memory,
            owner,
            GuestAddress(SHORT_STACK_POINTER),
            &[RETURN_ADDRESS.0, SHORT_OUTPUT_ADDRESS.0, 7],
        );
        write_words(
            &mut memory,
            owner,
            GuestAddress(OVERSIZED_STACK_POINTER),
            &[RETURN_ADDRESS.0, OVERSIZED_OUTPUT_ADDRESS.0, 12],
        );
        write_words(
            &mut memory,
            owner,
            GuestAddress(NULL_STACK_POINTER),
            &[RETURN_ADDRESS.0, 0, 8],
        );
        write_words(
            &mut memory,
            owner,
            GuestAddress(FAULT_STACK_POINTER),
            &[RETURN_ADDRESS.0, FAULT_OUTPUT_ADDRESS.0, 8],
        );
        for output in [
            OUTPUT_ADDRESS,
            SHORT_OUTPUT_ADDRESS,
            OVERSIZED_OUTPUT_ADDRESS,
        ] {
            memory
                .write_initialization(owner, output, &[0xa5; 16])
                .expect("initialize guarded TIMECAPS output");
        }
        memory
            .write_initialization(owner, FAULT_OUTPUT_ADDRESS, &[0xa5; 4])
            .expect("initialize truncated TIMECAPS output");
        memory.seal_owner(owner).expect("seal WinMM fixture owner");
        let reflection = memory.pending_reflection().expect("initial reflection");
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .expect("acknowledge initial reflection");

        let endpoint = ApiEndpointIdentity::from_import_thunk(&thunk);
        let contract = winmm_contract_for_endpoint(endpoint.clone())
            .expect("WinMM contract lookup")
            .expect("implemented timeGetDevCaps contract");
        let dispatcher =
            ApiDispatcher::new([endpoint.clone()], [contract]).expect("WinMM dispatcher");
        Self {
            memory,
            runtime: controlled_runtime(),
            dispatcher,
            endpoint,
            policy: WinmmCompatibilityPolicy::controlled_v1(),
        }
    }

    fn dispatch(&mut self, stack_pointer: u32) -> DispatchOutcome {
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        self.dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: self.endpoint.id,
                endpoint_address: self.endpoint.address,
                stack_pointer: GuestAddress(stack_pointer),
                return_address: RETURN_ADDRESS,
            },
            &mut session,
            |contract, frame, session| {
                dispatch_winmm_api_with_policy(&self.policy, contract, frame, session)
            },
        )
    }
}

#[test]
fn time_get_dev_caps_contract_is_two_argument_stdcall_mmresult() {
    let fixture = TimeGetDevCapsFixture::new();
    let contract = winmm_contract_for_endpoint(fixture.endpoint)
        .expect("WinMM contract lookup")
        .expect("timeGetDevCaps contract");
    assert_eq!(contract.implementation_id, TIME_GET_DEV_CAPS_IMPLEMENTATION);
    assert_eq!(contract.abi.calling_convention, CallingConvention::Stdcall);
    assert_eq!(contract.abi.arguments.len(), 2);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::Preserve);
    assert_eq!(contract.reentrancy_policy, ReentrancyPolicy::Disallowed);
    assert_eq!(contract.callback_policy, CallbackPolicy::Forbidden);
    let ApiValueSchema::DynamicPointer { constraint } = &contract.abi.arguments[0].value else {
        panic!("timeGetDevCaps output must be a bounded dynamic pointer");
    };
    assert_eq!(
        constraint,
        &ApiDynamicPointerConstraint {
            nullable: false,
            alignment: 4,
            minimum_byte_length: 1,
            maximum_byte_length: 8,
            pointee: ApiPointeeSchema::Struct {
                type_id: "TIMECAPS".to_owned(),
            },
            write_requirement: ApiWriteRequirement::Optional,
        }
    );
}

#[test]
fn time_get_dev_caps_writes_policy_profile_and_preserves_last_error() {
    let mut fixture = TimeGetDevCapsFixture::new();
    let before_generation = fixture.memory.stats().generation;
    let completion = match fixture.dispatch(STACK_POINTER) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected timeGetDevCaps completion, got {other:?}"),
    };
    assert_eq!(completion.eax, RegisterUpdate::Set(0));
    assert_eq!(completion.instruction_pointer, RETURN_ADDRESS);
    assert_eq!(completion.stack_pointer, GuestAddress(STACK_POINTER + 12));
    assert_eq!(completion.last_error, INITIAL_LAST_ERROR);
    assert_eq!(completion.guest_writes.len(), 1);
    assert_eq!(completion.guest_writes[0].write.address, OUTPUT_ADDRESS);
    assert_eq!(completion.guest_writes[0].write.bytes.len(), 8);
    assert_eq!(completion.guest_writes[0].result.bytes_written, 8);
    assert_eq!(fixture.memory.stats().generation, before_generation + 1);
    acknowledge_pending_delta(&mut fixture.memory);
    assert_eq!(
        fixture.memory.read(OUTPUT_ADDRESS, 16).unwrap(),
        [
            1, 0, 0, 0, 0x40, 0x42, 0x0f, 0, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5
        ]
    );
}

#[test]
fn time_get_dev_caps_size_variants_match_native_failure_ordering() {
    let mut fixture = TimeGetDevCapsFixture::new();
    let before = fixture.memory.read(SHORT_OUTPUT_ADDRESS, 16).unwrap();
    let before_stats = fixture.memory.stats();
    let short = match fixture.dispatch(SHORT_STACK_POINTER) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected short-size MMRESULT, got {other:?}"),
    };
    assert_eq!(short.eax, RegisterUpdate::Set(TIMERR_STRUCT));
    assert_eq!(short.stack_pointer, GuestAddress(SHORT_STACK_POINTER + 12));
    assert_eq!(short.last_error, INITIAL_LAST_ERROR);
    assert!(short.guest_writes.is_empty());
    assert_eq!(
        fixture.memory.read(SHORT_OUTPUT_ADDRESS, 16).unwrap(),
        before
    );
    assert_eq!(fixture.memory.stats(), before_stats);

    let oversized = match fixture.dispatch(OVERSIZED_STACK_POINTER) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected oversized timeGetDevCaps completion, got {other:?}"),
    };
    assert_eq!(oversized.eax, RegisterUpdate::Set(0));
    assert_eq!(oversized.last_error, INITIAL_LAST_ERROR);
    acknowledge_pending_delta(&mut fixture.memory);
    assert_eq!(
        fixture.memory.read(OVERSIZED_OUTPUT_ADDRESS, 16).unwrap(),
        [
            1, 0, 0, 0, 0x40, 0x42, 0x0f, 0, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5
        ]
    );
}

#[test]
fn time_get_dev_caps_policy_pointer_and_memory_failures_are_atomic() {
    let mut disabled = TimeGetDevCapsFixture::new();
    disabled.policy.time_get_dev_caps = None;
    let disabled_runtime = disabled.runtime.clone();
    let disabled_memory = disabled.memory.stats();
    let disabled_output = disabled.memory.read(OUTPUT_ADDRESS, 16).unwrap();
    let fault = match disabled.dispatch(STACK_POINTER) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected disabled-policy fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "time-get-dev-caps-policy-disabled");
    assert_eq!(disabled.runtime, disabled_runtime);
    assert_eq!(disabled.memory.stats(), disabled_memory);
    assert_eq!(
        disabled.memory.read(OUTPUT_ADDRESS, 16).unwrap(),
        disabled_output
    );

    let mut null = TimeGetDevCapsFixture::new();
    let null_runtime = null.runtime.clone();
    let null_memory = null.memory.stats();
    let fault = match null.dispatch(NULL_STACK_POINTER) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected NULL output fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::InvalidCall);
    assert_eq!(null.runtime, null_runtime);
    assert_eq!(null.memory.stats(), null_memory);

    let mut truncated = TimeGetDevCapsFixture::new();
    let truncated_runtime = truncated.runtime.clone();
    let truncated_memory = truncated.memory.stats();
    let truncated_output = truncated.memory.read(FAULT_OUTPUT_ADDRESS, 4).unwrap();
    let fault = match truncated.dispatch(FAULT_STACK_POINTER) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected truncated output fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::InvalidCall);
    assert_eq!(fault.code, "guest-page-fault");
    assert_eq!(truncated.runtime, truncated_runtime);
    assert_eq!(truncated.memory.stats(), truncated_memory);
    assert_eq!(
        truncated.memory.read(FAULT_OUTPUT_ADDRESS, 4).unwrap(),
        truncated_output
    );
}

fn initialized_memory() -> (GuestMemory, MemoryOwnerId) {
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 3,
        max_regions: 1,
        max_owners: 1,
        max_access_bytes: 0x3000,
    })
    .expect("memory");
    let owner = memory
        .create_owner(MemoryOwnerKind::Runtime)
        .expect("memory owner");
    let range = memory
        .reserve_fixed(owner, MemoryRegionKind::Runtime, MEMORY_BASE, 0x3000)
        .expect("memory range");
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
        .expect("commit memory");
    (memory, owner)
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
    memory
        .write_initialization(owner, address, &bytes)
        .expect("initialize words");
}

fn acknowledge_pending_delta(memory: &mut GuestMemory) {
    let delta = memory.pending_delta().expect("pending WinMM write delta");
    memory
        .acknowledge_delta(delta.generation)
        .expect("acknowledge WinMM write delta");
}

fn controlled_runtime() -> RuntimeState {
    let mut startup = vec![0; 68];
    startup[..4].copy_from_slice(&68_u32.to_le_bytes());
    RuntimeState::new(RuntimeBootstrap {
        process_id: RuntimeProcessId(PROCESS_ID),
        initial_thread_id: RuntimeThreadId(THREAD_ID),
        initial_last_error: INITIAL_LAST_ERROR,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "test-startup-profile".to_owned(),
            bytes: startup,
        },
        process_image_path_a: RuntimeProcessImagePathA {
            source_authority: "winmm-test".to_owned(),
            bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: RuntimeCurrentDirectoryA {
            source_authority: "winmm-test".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: vine_core::LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec01_0000,
    })
    .expect("runtime state")
}

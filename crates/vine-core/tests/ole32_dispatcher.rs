use vine_core::{
    ApiDispatcher, ApiEndpointIdentity, ApiFaultClass, ApiOwnedTrap, ApiReturnSchema,
    CO_INITIALIZE_IMPLEMENTATION, CallbackPolicy, CallingConvention, DispatchOutcome,
    ExecutionSession, GuestAddress, GuestMemory, ImportTarget, ImportThunk, ImportThunkDisposition,
    ImportThunkId, LastErrorPolicy, MemoryLimits, MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind,
    PageProtection, ProcessAddressSpaceLayout, ReentrancyPolicy, RegisterUpdate, RuntimeBootstrap,
    RuntimeClockState, RuntimeComApartmentModel, RuntimeComInitializationDisposition,
    RuntimeCurrentDirectoryA, RuntimeProcessId, RuntimeProcessImagePathA, RuntimeStartupInfo,
    RuntimeState, RuntimeThreadId, StackCleanup, dispatch_ole32_api, ole32_contract_for_endpoint,
};

const MEMORY_BASE: GuestAddress = GuestAddress(0x2000_0000);
const STACK_POINTER: u32 = 0x2000_0100;
const IAT_ADDRESS: GuestAddress = GuestAddress(0x2000_2000);
const ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_12c0);
const RETURN_ADDRESS: GuestAddress = GuestAddress(0x1008_7399);
const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;
const INITIAL_LAST_ERROR: u32 = 0x1234_5678;

struct CoInitializeFixture {
    memory: GuestMemory,
    runtime: RuntimeState,
    dispatcher: ApiDispatcher,
    endpoint: ApiEndpointIdentity,
}

impl CoInitializeFixture {
    fn new() -> Self {
        let (mut memory, owner) = initialized_memory();
        let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
            id: ImportThunkId(301),
            address: ENDPOINT_ADDRESS,
            requester_owner: owner,
            module: "OLE32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_1544),
            iat_address: IAT_ADDRESS,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "CoInitialize".to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        });
        memory
            .write_initialization(owner, IAT_ADDRESS, &ENDPOINT_ADDRESS.0.to_le_bytes())
            .expect("initialize CoInitialize IAT");
        for (index, reserved) in [0_u32, 0, 1].into_iter().enumerate() {
            write_words(
                &mut memory,
                owner,
                GuestAddress(STACK_POINTER + index as u32 * 0x20),
                &[RETURN_ADDRESS.0, reserved],
            );
        }
        memory.seal_owner(owner).expect("seal Ole32 fixture owner");
        let reflection = memory.pending_reflection().expect("initial reflection");
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .expect("acknowledge initial reflection");
        let contract = ole32_contract_for_endpoint(endpoint.clone())
            .expect("Ole32 contract lookup")
            .expect("implemented CoInitialize contract");
        let dispatcher =
            ApiDispatcher::new([endpoint.clone()], [contract]).expect("Ole32 dispatcher");
        Self {
            memory,
            runtime: controlled_runtime(),
            dispatcher,
            endpoint,
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
            dispatch_ole32_api,
        )
    }
}

#[test]
fn co_initialize_contract_is_one_argument_stdcall_hresult() {
    let fixture = CoInitializeFixture::new();
    let contract = ole32_contract_for_endpoint(fixture.endpoint)
        .expect("Ole32 contract lookup")
        .expect("CoInitialize contract");
    assert_eq!(contract.implementation_id, CO_INITIALIZE_IMPLEMENTATION);
    assert_eq!(contract.abi.calling_convention, CallingConvention::Stdcall);
    assert_eq!(contract.abi.arguments.len(), 1);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::HResult);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::Preserve);
    assert_eq!(contract.reentrancy_policy, ReentrancyPolicy::Disallowed);
    assert_eq!(contract.callback_policy, CallbackPolicy::Forbidden);
}

#[test]
fn co_initialize_returns_s_ok_then_s_false_and_preserves_last_error() {
    let mut fixture = CoInitializeFixture::new();
    let memory_before = fixture.memory.stats();

    let first = match fixture.dispatch(STACK_POINTER) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected CoInitialize completion, got {other:?}"),
    };
    assert_eq!(first.eax, RegisterUpdate::Set(0));
    assert_eq!(first.instruction_pointer, RETURN_ADDRESS);
    assert_eq!(first.stack_pointer, GuestAddress(STACK_POINTER + 8));
    assert_eq!(first.last_error, INITIAL_LAST_ERROR);
    assert!(first.guest_writes.is_empty());
    let apartment = fixture
        .runtime
        .current_com_apartment()
        .unwrap()
        .copied()
        .unwrap();
    assert_eq!(apartment.model, RuntimeComApartmentModel::SingleThreaded);
    assert_eq!(apartment.initialization_count, 1);

    let repeat = match fixture.dispatch(STACK_POINTER + 0x20) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected repeat CoInitialize completion, got {other:?}"),
    };
    assert_eq!(repeat.eax, RegisterUpdate::Set(1));
    assert_eq!(repeat.stack_pointer, GuestAddress(STACK_POINTER + 0x28));
    assert_eq!(repeat.last_error, INITIAL_LAST_ERROR);
    assert!(repeat.guest_writes.is_empty());
    assert_eq!(
        fixture
            .runtime
            .current_com_apartment()
            .unwrap()
            .unwrap()
            .initialization_count,
        2
    );
    assert_eq!(fixture.memory.stats(), memory_before);
}

#[test]
fn co_initialize_reports_changed_mode_without_mutation() {
    let mut fixture = CoInitializeFixture::new();
    let initialized = fixture
        .runtime
        .initialize_current_com_apartment(RuntimeComApartmentModel::MultiThreaded)
        .expect("initialize MTA model");
    assert_eq!(
        initialized.disposition,
        RuntimeComInitializationDisposition::Initialized
    );
    let runtime_before = fixture.runtime.clone();
    let memory_before = fixture.memory.stats();

    let conflict = match fixture.dispatch(STACK_POINTER) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected changed-mode completion, got {other:?}"),
    };
    assert_eq!(conflict.eax, RegisterUpdate::Set(0x8001_0106));
    assert_eq!(conflict.last_error, INITIAL_LAST_ERROR);
    assert!(conflict.guest_writes.is_empty());
    assert_eq!(fixture.runtime, runtime_before);
    assert_eq!(fixture.memory.stats(), memory_before);
}

#[test]
fn co_initialize_rejects_nonnull_reserved_before_mutation() {
    let mut fixture = CoInitializeFixture::new();
    let runtime_before = fixture.runtime.clone();
    let memory_before = fixture.memory.stats();

    let fault = match fixture.dispatch(STACK_POINTER + 0x40) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected reserved-pointer fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "co-initialize-reserved-unsupported");
    assert_eq!(fixture.runtime, runtime_before);
    assert_eq!(fixture.memory.stats(), memory_before);
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

fn controlled_runtime() -> RuntimeState {
    let mut startup = vec![0; 68];
    startup[..4].copy_from_slice(&68_u32.to_le_bytes());
    let mut runtime = RuntimeState::new(RuntimeBootstrap {
        process_id: RuntimeProcessId(PROCESS_ID),
        initial_thread_id: RuntimeThreadId(THREAD_ID),
        initial_last_error: INITIAL_LAST_ERROR,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
            performance_frequency: 10_000_000,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "test-startup-profile".to_owned(),
            bytes: startup,
        },
        process_image_path_a: RuntimeProcessImagePathA {
            source_authority: "ole32-test".to_owned(),
            bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: RuntimeCurrentDirectoryA {
            source_authority: "ole32-test".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: vine_core::LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_cookie: 0xec01_9d3b,
    })
    .expect("runtime");
    runtime
        .start_initial_thread()
        .expect("start initial thread");
    runtime
}

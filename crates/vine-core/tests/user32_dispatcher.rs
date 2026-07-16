use vine_core::{
    ApiDispatcher, ApiEndpointIdentity, ApiFaultClass, ApiOwnedTrap, ApiReturnSchema,
    CallbackPolicy, CallingConvention, DispatchOutcome, ExecutionSession, GUEST_PAGE_SIZE,
    GuestAddress, GuestCallbackArgument, GuestMemory, GuestRange, ImportTarget, ImportThunk,
    ImportThunkDisposition, ImportThunkId, LastErrorPolicy, MemoryLimits, MemoryOwnerId,
    MemoryOwnerKind, MemoryRegionKind, PageProtection, ProcessAddressSpaceLayout,
    RUNTIME_PRIMARY_MONITOR_HANDLE, ReentrancyPolicy, RegisterUpdate, RuntimeBootstrap,
    RuntimeCallbackFrameId, RuntimeClockState, RuntimeCurrentDirectoryA, RuntimeDisplayProfile,
    RuntimeInputProfile, RuntimeProcessId, RuntimeProcessImagePathA, RuntimeRect,
    RuntimeStartupInfo, RuntimeState, RuntimeThreadId, StackCleanup, User32CompatibilityPolicy,
    dispatch_user32_api_with_policy, enum_display_monitors_contract, swap_mouse_button_contract,
};

const MEMORY_BASE: GuestAddress = GuestAddress(0x2000_0000);
const STACK_POINTER: u32 = 0x2000_1040;
const LOW_STACK_POINTER: u32 = 0x2000_0008;
const IAT_ADDRESS: GuestAddress = GuestAddress(0x2000_2000);
const CALLBACK_TARGET: GuestAddress = GuestAddress(0x2000_2100);
const CALLBACK_SENTINEL: GuestAddress = GuestAddress(0x2000_2110);
const ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0bc0);
const RETURN_ADDRESS: GuestAddress = GuestAddress(0x1006_a781);
const SWAP_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0a50);
const SWAP_FIRST_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1008_8a3b);
const SWAP_SECOND_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1008_8a43);
const SWAP_SECOND_STACK_POINTER: u32 = STACK_POINTER + 0x20;
const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;
const INITIAL_LAST_ERROR: u32 = 0x1234_5678;

struct EnumFixture {
    memory: GuestMemory,
    runtime: RuntimeState,
    dispatcher: ApiDispatcher,
    endpoint: ApiEndpointIdentity,
    policy: User32CompatibilityPolicy,
    stack_pointer: u32,
}

impl EnumFixture {
    fn new(hdc: u32, clip: u32, callback: u32, data: u32) -> Self {
        Self::with_stack(STACK_POINTER, hdc, clip, callback, data)
    }

    fn with_stack(stack_pointer: u32, hdc: u32, clip: u32, callback: u32, data: u32) -> Self {
        let (mut memory, owner) = initialized_memory();
        let thunk = ImportThunk {
            id: ImportThunkId(301),
            address: ENDPOINT_ADDRESS,
            requester_owner: owner,
            module: "USER32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_1140),
            iat_address: IAT_ADDRESS,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "EnumDisplayMonitors".to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        };
        memory
            .write_initialization(owner, IAT_ADDRESS, &ENDPOINT_ADDRESS.0.to_le_bytes())
            .expect("initialize EnumDisplayMonitors IAT");
        write_words(
            &mut memory,
            owner,
            GuestAddress(stack_pointer),
            &[RETURN_ADDRESS.0, hdc, clip, callback, data],
        );
        memory
            .protect(
                owner,
                GuestRange::new(GuestAddress(0x2000_2000), GUEST_PAGE_SIZE)
                    .expect("callback executable page"),
                PageProtection {
                    read: true,
                    write: true,
                    execute: true,
                },
            )
            .expect("make callback page executable");
        memory.seal_owner(owner).expect("seal User32 fixture owner");
        let reflection = memory.pending_reflection().expect("initial reflection");
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .expect("acknowledge initial reflection");

        let endpoint = ApiEndpointIdentity::from_import_thunk(&thunk);
        let contract =
            enum_display_monitors_contract(endpoint.clone()).expect("EnumDisplayMonitors contract");
        let dispatcher =
            ApiDispatcher::new([endpoint.clone()], [contract]).expect("User32 dispatcher");
        let policy = User32CompatibilityPolicy::controlled_v2();
        let mut runtime = controlled_runtime();
        runtime
            .configure_display_profile(policy.display_profile().expect("display profile"))
            .expect("configure logical display");

        Self {
            memory,
            runtime,
            dispatcher,
            endpoint,
            policy,
            stack_pointer,
        }
    }

    fn dispatch(&mut self) -> DispatchOutcome {
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        self.dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: self.endpoint.id,
                endpoint_address: self.endpoint.address,
                stack_pointer: GuestAddress(self.stack_pointer),
                return_address: RETURN_ADDRESS,
            },
            &mut session,
            |contract, frame, session| {
                dispatch_user32_api_with_policy(
                    &self.policy,
                    CALLBACK_SENTINEL,
                    contract,
                    frame,
                    session,
                )
            },
        )
    }
}

struct SwapFixture {
    memory: GuestMemory,
    runtime: RuntimeState,
    dispatcher: ApiDispatcher,
    endpoint: ApiEndpointIdentity,
    policy: User32CompatibilityPolicy,
}

impl SwapFixture {
    fn new(initial_swapped: bool) -> Self {
        let (mut memory, owner) = initialized_memory();
        let thunk = ImportThunk {
            id: ImportThunkId(166),
            address: SWAP_ENDPOINT_ADDRESS,
            requester_owner: owner,
            module: "USER32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_1370),
            iat_address: IAT_ADDRESS,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "SwapMouseButton".to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        };
        memory
            .write_initialization(owner, IAT_ADDRESS, &SWAP_ENDPOINT_ADDRESS.0.to_le_bytes())
            .expect("initialize SwapMouseButton IAT");
        write_words(
            &mut memory,
            owner,
            GuestAddress(STACK_POINTER),
            &[SWAP_FIRST_RETURN_ADDRESS.0, 0],
        );
        write_words(
            &mut memory,
            owner,
            GuestAddress(SWAP_SECOND_STACK_POINTER),
            &[SWAP_SECOND_RETURN_ADDRESS.0, u32::from(initial_swapped)],
        );
        memory.seal_owner(owner).expect("seal User32 fixture owner");
        let reflection = memory.pending_reflection().expect("initial reflection");
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .expect("acknowledge initial reflection");

        let endpoint = ApiEndpointIdentity::from_import_thunk(&thunk);
        let contract =
            swap_mouse_button_contract(endpoint.clone()).expect("SwapMouseButton contract");
        let dispatcher =
            ApiDispatcher::new([endpoint.clone()], [contract]).expect("User32 dispatcher");
        let mut policy = User32CompatibilityPolicy::controlled_v2();
        policy
            .swap_mouse_button
            .as_mut()
            .expect("input profile")
            .initial_mouse_buttons_swapped = initial_swapped;
        let mut runtime = controlled_runtime();
        runtime
            .configure_input_profile(policy.input_profile().expect("input profile"))
            .expect("configure logical input");

        Self {
            memory,
            runtime,
            dispatcher,
            endpoint,
            policy,
        }
    }

    fn dispatch(&mut self, stack_pointer: u32, return_address: GuestAddress) -> DispatchOutcome {
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        self.dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: self.endpoint.id,
                endpoint_address: self.endpoint.address,
                stack_pointer: GuestAddress(stack_pointer),
                return_address,
            },
            &mut session,
            |contract, frame, session| {
                dispatch_user32_api_with_policy(
                    &self.policy,
                    CALLBACK_SENTINEL,
                    contract,
                    frame,
                    session,
                )
            },
        )
    }
}

#[test]
fn enum_display_monitors_contract_is_bounded_stdcall_callback() {
    let (_, owner) = initialized_memory();
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(301),
        address: ENDPOINT_ADDRESS,
        requester_owner: owner,
        module: "USER32.dll".to_owned(),
        iat_rva: vine_core::Rva(0x0d_1140),
        iat_address: IAT_ADDRESS,
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "EnumDisplayMonitors".to_owned(),
            hint: 0,
            name_rva: vine_core::Rva(0),
        },
    });
    let contract = enum_display_monitors_contract(endpoint).expect("contract");
    assert_eq!(contract.abi.calling_convention, CallingConvention::Stdcall);
    assert_eq!(contract.abi.arguments.len(), 4);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::Bool32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::Preserve);
    assert_eq!(contract.reentrancy_policy, ReentrancyPolicy::Allowed);
    assert_eq!(contract.callback_policy, CallbackPolicy::Required);
}

#[test]
fn swap_mouse_button_contract_is_immediate_bool_stdcall() {
    let (_, owner) = initialized_memory();
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(166),
        address: SWAP_ENDPOINT_ADDRESS,
        requester_owner: owner,
        module: "USER32.dll".to_owned(),
        iat_rva: vine_core::Rva(0x0d_1370),
        iat_address: IAT_ADDRESS,
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "SwapMouseButton".to_owned(),
            hint: 0,
            name_rva: vine_core::Rva(0),
        },
    });
    let contract = swap_mouse_button_contract(endpoint).expect("contract");
    assert_eq!(contract.abi.calling_convention, CallingConvention::Stdcall);
    assert_eq!(contract.abi.arguments.len(), 1);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::Bool32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::Preserve);
    assert_eq!(contract.reentrancy_policy, ReentrancyPolicy::Allowed);
    assert_eq!(contract.callback_policy, CallbackPolicy::Forbidden);
}

#[test]
fn swap_mouse_button_returns_previous_mapping_and_bgi_pair_restores_initial_state() {
    for initial_swapped in [false, true] {
        let mut fixture = SwapFixture::new(initial_swapped);
        let memory_generation = fixture.memory.stats().generation;
        let first = match fixture.dispatch(STACK_POINTER, SWAP_FIRST_RETURN_ADDRESS) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected first immediate completion, got {other:?}"),
        };
        assert_eq!(first.eax, RegisterUpdate::Set(u32::from(initial_swapped)));
        assert_eq!(first.stack_pointer, GuestAddress(STACK_POINTER + 8));
        assert_eq!(first.instruction_pointer, SWAP_FIRST_RETURN_ADDRESS);
        assert_eq!(first.last_error, INITIAL_LAST_ERROR);
        assert!(first.guest_writes.is_empty());
        let after_first = fixture
            .runtime
            .mouse_button_mapping()
            .expect("configured mapping");
        assert!(!after_first.swapped);
        assert_eq!(after_first.generation, u64::from(initial_swapped));

        let second = match fixture.dispatch(SWAP_SECOND_STACK_POINTER, SWAP_SECOND_RETURN_ADDRESS) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected second immediate completion, got {other:?}"),
        };
        assert_eq!(second.eax, RegisterUpdate::Set(0));
        assert_eq!(
            second.stack_pointer,
            GuestAddress(SWAP_SECOND_STACK_POINTER + 8)
        );
        assert_eq!(second.instruction_pointer, SWAP_SECOND_RETURN_ADDRESS);
        assert_eq!(second.last_error, INITIAL_LAST_ERROR);
        assert!(second.guest_writes.is_empty());
        let final_mapping = fixture
            .runtime
            .mouse_button_mapping()
            .expect("configured mapping");
        assert_eq!(final_mapping.swapped, initial_swapped);
        assert_eq!(
            final_mapping.generation,
            if initial_swapped { 2 } else { 0 }
        );
        assert_eq!(fixture.memory.stats().generation, memory_generation);
    }
}

#[test]
fn swap_mouse_button_policy_and_authority_failures_are_atomic() {
    let mut disabled = SwapFixture::new(false);
    disabled.policy.swap_mouse_button = None;
    let mapping_before = disabled.runtime.mouse_button_mapping().cloned();
    let memory_generation = disabled.memory.stats().generation;
    let fault = match disabled.dispatch(STACK_POINTER, SWAP_FIRST_RETURN_ADDRESS) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected disabled-policy fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "swap-mouse-button-policy-disabled");
    assert_eq!(
        disabled.runtime.mouse_button_mapping(),
        mapping_before.as_ref()
    );
    assert_eq!(disabled.memory.stats().generation, memory_generation);

    let mut missing = SwapFixture::new(false);
    missing.runtime = controlled_runtime();
    let fault = match missing.dispatch(STACK_POINTER, SWAP_FIRST_RETURN_ADDRESS) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected missing-authority fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::RuntimeDefect);
    assert_eq!(fault.code, "logical-mouse-button-profile-missing");
    assert!(missing.runtime.mouse_button_mapping().is_none());

    let mut mismatch = SwapFixture::new(false);
    mismatch.runtime = controlled_runtime();
    mismatch
        .runtime
        .configure_input_profile(RuntimeInputProfile {
            id: "different-input-profile-v1".to_owned(),
            initial_mouse_buttons_swapped: false,
        })
        .expect("configure mismatched input");
    let mapping_before = mismatch.runtime.mouse_button_mapping().cloned();
    let fault = match mismatch.dispatch(STACK_POINTER, SWAP_FIRST_RETURN_ADDRESS) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected profile-mismatch fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::RuntimeDefect);
    assert_eq!(fault.code, "logical-mouse-button-profile-mismatch");
    assert_eq!(
        mismatch.runtime.mouse_button_mapping(),
        mapping_before.as_ref()
    );
}

#[test]
fn enum_display_monitors_builds_atomic_scratch_frame_and_normalizes_bool_result() {
    for (callback_result, expected_bool) in [(0, 0), (7, 1)] {
        let data = 0x5566_7788;
        let mut fixture = EnumFixture::new(0, 0, CALLBACK_TARGET.0, data);
        let call = match fixture.dispatch() {
            DispatchOutcome::GuestCallback(call) => call,
            other => panic!("expected guest callback, got {other:?}"),
        };
        assert_eq!(call.request.target, CALLBACK_TARGET);
        assert_eq!(
            call.request.arguments,
            [
                GuestCallbackArgument::U32(RUNTIME_PRIMARY_MONITOR_HANDLE),
                GuestCallbackArgument::U32(0),
                GuestCallbackArgument::ScratchPointer { offset: 0 },
                GuestCallbackArgument::U32(data),
            ]
        );
        assert_eq!(
            call.request.scratch,
            [0_i32, 0, 1920, 1080]
                .into_iter()
                .flat_map(i32::to_le_bytes)
                .collect::<Vec<_>>()
        );

        let generation_before = fixture.memory.stats().generation;
        let prepared = ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime)
            .begin_guest_callback(&call)
            .expect("prepare callback frame");
        assert_eq!(prepared.frame_id, RuntimeCallbackFrameId(1));
        assert_eq!(prepared.allocation_start, GuestAddress(STACK_POINTER - 36));
        assert_eq!(prepared.scratch_address, prepared.allocation_start);
        assert_eq!(prepared.stack_pointer, GuestAddress(STACK_POINTER - 20));
        assert_eq!(prepared.scratch_bytes, 16);
        assert_eq!(prepared.frame_bytes, 36);
        assert_eq!(fixture.memory.stats().generation, generation_before + 1);
        let delta = fixture
            .memory
            .pending_delta()
            .expect("callback frame delta");
        fixture
            .memory
            .acknowledge_delta(delta.generation)
            .expect("acknowledge callback frame delta");

        let mut expected = call.request.scratch.clone();
        expected.extend_from_slice(&CALLBACK_SENTINEL.0.to_le_bytes());
        expected.extend_from_slice(&RUNTIME_PRIMARY_MONITOR_HANDLE.to_le_bytes());
        expected.extend_from_slice(&0_u32.to_le_bytes());
        expected.extend_from_slice(&prepared.scratch_address.0.to_le_bytes());
        expected.extend_from_slice(&data.to_le_bytes());
        assert_eq!(
            fixture
                .memory
                .read(prepared.allocation_start, prepared.frame_bytes as usize)
                .expect("callback frame bytes"),
            expected
        );

        fixture
            .runtime
            .pop_callback_frame(prepared.frame_id)
            .expect("retire callback frame");
        let completion = {
            let mut session = ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime);
            fixture
                .dispatcher
                .complete_guest_callback_result(call, callback_result, &mut session)
        };
        let completion = match completion {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected callback completion, got {other:?}"),
        };
        assert_eq!(completion.eax, RegisterUpdate::Set(expected_bool));
        assert_eq!(completion.instruction_pointer, RETURN_ADDRESS);
        assert_eq!(
            completion.stack_pointer,
            GuestAddress(STACK_POINTER + 5 * 4)
        );
        assert_eq!(completion.last_error, INITIAL_LAST_ERROR);
        assert!(completion.guest_writes.is_empty());
    }
}

#[test]
fn enum_display_monitors_rejects_unreached_variants_and_invalid_callback_before_mutation() {
    for (hdc, clip, callback, expected_class, expected_code) in [
        (
            1,
            0,
            CALLBACK_TARGET.0,
            ApiFaultClass::Unsupported,
            "enum-display-monitors-variant-unsupported",
        ),
        (
            0,
            STACK_POINTER + 0x100,
            CALLBACK_TARGET.0,
            ApiFaultClass::Unsupported,
            "enum-display-monitors-variant-unsupported",
        ),
        (
            0,
            0,
            STACK_POINTER,
            ApiFaultClass::InvalidCall,
            "guest-page-fault",
        ),
    ] {
        let mut fixture = EnumFixture::new(hdc, clip, callback, 0);
        let generation_before = fixture.memory.stats().generation;
        let fault = match fixture.dispatch() {
            DispatchOutcome::Fault(fault) => fault,
            other => panic!("expected fault, got {other:?}"),
        };
        assert_eq!(fault.class, expected_class);
        assert_eq!(fault.code, expected_code);
        assert_eq!(fixture.memory.stats().generation, generation_before);
        let frame = fixture
            .runtime
            .push_callback_frame(CALLBACK_TARGET, CALLBACK_SENTINEL)
            .expect("failure left callback stack empty");
        assert_eq!(frame.id, RuntimeCallbackFrameId(1));
    }
}

#[test]
fn callback_preparation_failures_preserve_memory_and_frame_allocator() {
    let mut unmapped = EnumFixture::with_stack(LOW_STACK_POINTER, 0, 0, CALLBACK_TARGET.0, 0);
    let call = match unmapped.dispatch() {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected callback request, got {other:?}"),
    };
    let generation_before = unmapped.memory.stats().generation;
    let fault = ExecutionSession::new(&mut unmapped.memory, &mut unmapped.runtime)
        .begin_guest_callback(&call)
        .expect_err("unmapped callback scratch must fail");
    assert_eq!(fault.class, ApiFaultClass::InvalidCall);
    assert_eq!(fault.code, "guest-page-fault");
    assert_eq!(unmapped.memory.stats().generation, generation_before);
    let frame = unmapped
        .runtime
        .push_callback_frame(CALLBACK_TARGET, CALLBACK_SENTINEL)
        .expect("unmapped failure did not allocate a frame ID");
    assert_eq!(frame.id, RuntimeCallbackFrameId(1));

    let mut invalid_scratch = EnumFixture::new(0, 0, CALLBACK_TARGET.0, 0);
    let mut call = match invalid_scratch.dispatch() {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected callback request, got {other:?}"),
    };
    call.request.arguments[2] = GuestCallbackArgument::ScratchPointer { offset: 16 };
    let generation_before = invalid_scratch.memory.stats().generation;
    let fault = ExecutionSession::new(&mut invalid_scratch.memory, &mut invalid_scratch.runtime)
        .begin_guest_callback(&call)
        .expect_err("out-of-range scratch pointer must fail");
    assert_eq!(fault.class, ApiFaultClass::InvalidCall);
    assert_eq!(fault.code, "callback-scratch-pointer-invalid");
    assert_eq!(invalid_scratch.memory.stats().generation, generation_before);
    let frame = invalid_scratch
        .runtime
        .push_callback_frame(CALLBACK_TARGET, CALLBACK_SENTINEL)
        .expect("invalid scratch failure did not allocate a frame ID");
    assert_eq!(frame.id, RuntimeCallbackFrameId(1));
}

#[test]
fn callback_frame_identity_rejects_stale_and_duplicate_completion() {
    let mut fixture = EnumFixture::new(0, 0, CALLBACK_TARGET.0, 0);
    let call = match fixture.dispatch() {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected callback request, got {other:?}"),
    };
    let prepared = ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime)
        .begin_guest_callback(&call)
        .expect("prepare callback");
    assert_eq!(
        fixture
            .runtime
            .validate_callback_frame_return(RuntimeCallbackFrameId(prepared.frame_id.0 + 1))
            .expect_err("stale frame preflight must fail")
            .code,
        "callback-frame-return-order-mismatch"
    );
    fixture
        .runtime
        .validate_callback_frame_return(prepared.frame_id)
        .expect("matching frame preflight is mutation-free");
    assert_eq!(
        fixture
            .runtime
            .pop_callback_frame(RuntimeCallbackFrameId(prepared.frame_id.0 + 1))
            .expect_err("stale frame ID must fail")
            .code,
        "callback-frame-return-order-mismatch"
    );
    fixture
        .runtime
        .pop_callback_frame(prepared.frame_id)
        .expect("matching frame completes");
    assert_eq!(
        fixture
            .runtime
            .pop_callback_frame(prepared.frame_id)
            .expect_err("duplicate frame completion must fail")
            .code,
        "callback-return-thread-state-invalid"
    );
}

#[test]
fn enabled_policy_requires_matching_rust_display_authority() {
    let mut missing = EnumFixture::new(0, 0, CALLBACK_TARGET.0, 0);
    missing.runtime = controlled_runtime();
    let fault = match missing.dispatch() {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected missing display fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::RuntimeDefect);
    assert_eq!(fault.code, "logical-monitor-profile-missing");

    let mut mismatch = EnumFixture::new(0, 0, CALLBACK_TARGET.0, 0);
    mismatch.runtime = controlled_runtime();
    mismatch
        .runtime
        .configure_display_profile(RuntimeDisplayProfile {
            id: "different-profile-v1".to_owned(),
            primary_monitor_rect: RuntimeRect {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            },
        })
        .expect("configure mismatched display");
    let fault = match mismatch.dispatch() {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected display mismatch fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::RuntimeDefect);
    assert_eq!(fault.code, "logical-monitor-profile-mismatch");
}

fn initialized_memory() -> (GuestMemory, MemoryOwnerId) {
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 3,
        max_regions: 1,
        max_owners: 1,
        max_access_bytes: 0x4000,
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
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "test-startup-profile".to_owned(),
            bytes: startup,
        },
        process_image_path_a: RuntimeProcessImagePathA {
            source_authority: "user32-test".to_owned(),
            bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: RuntimeCurrentDirectoryA {
            source_authority: "user32-test".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: vine_core::LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec01_0000,
    })
    .expect("runtime");
    runtime
        .start_initial_thread()
        .expect("start initial thread");
    runtime
}

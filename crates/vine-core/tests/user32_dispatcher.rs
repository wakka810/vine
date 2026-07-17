use std::fs;
use std::path::PathBuf;

use vine_core::{
    ApiDispatcher, ApiEndpointIdentity, ApiFaultClass, ApiOwnedTrap, ApiReturnSchema,
    CallbackPolicy, CallingConvention, DispatchOutcome, ExecutionSession, GUEST_PAGE_SIZE,
    GuestAddress, GuestCallbackArgument, GuestMemory, GuestRange, ImportTarget, ImportThunk,
    ImportThunkDisposition, ImportThunkId, LOGICAL_LOADED_MODULE_NAMESPACE, LastErrorPolicy,
    LoaderIngressReport, MemoryLimits, MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind,
    ModuleSnapshot, PageProtection, Pe32Limits, ProcessAddressSpaceLayout,
    RUNTIME_PRIMARY_MONITOR_HANDLE, ReentrancyPolicy, RegisterUpdate, RuntimeBootstrap,
    RuntimeCallbackFrameId, RuntimeClockState, RuntimeCurrentDirectoryA, RuntimeCursorSource,
    RuntimeDisplayProfile, RuntimeInputProfile, RuntimeModule, RuntimeProcessId,
    RuntimeProcessImagePathA, RuntimeRect, RuntimeStartupInfo, RuntimeState,
    RuntimeSystemCursorKind, RuntimeThreadId, StackCleanup, User32CompatibilityPolicy,
    build_pe32_load_plan, dispatch_user32_api_with_policy, enum_display_monitors_contract,
    load_cursor_a_contract, load_icon_a_contract, materialize_pe32_image,
    swap_mouse_button_contract,
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
const ICON_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0bf0);
const CURSOR_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0f30);
const CURSOR_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1006_7d09);
const ICON_IMAGE_BASE: u32 = 0x0040_0000;
const ICON_MODULE_HANDLE: u32 = 0x4d01_000f;
const ICON_FIXTURE_SHA256: &str =
    "52587db69f1ea9f39f8604bf7ccb8c50ee783f7378228cf5f20229ac54803dac";
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
        let policy = User32CompatibilityPolicy::controlled_v4();
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
        let mut policy = User32CompatibilityPolicy::controlled_v4();
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

struct IconFixture {
    memory: GuestMemory,
    runtime: RuntimeState,
    dispatcher: ApiDispatcher,
    endpoint: ApiEndpointIdentity,
    policy: User32CompatibilityPolicy,
}

impl IconFixture {
    fn new() -> Self {
        let bytes = icon_fixture_bytes();
        let limits = Pe32Limits::default();
        ModuleSnapshot::from_bytes(bytes.clone(), &limits)
            .expect("fixed icon fixture snapshot")
            .verify_expected_sha256(ICON_FIXTURE_SHA256)
            .expect("fixed icon fixture fingerprint");
        Self::from_bytes(bytes)
    }

    fn from_bytes(bytes: Vec<u8>) -> Self {
        let mut memory = GuestMemory::new(MemoryLimits {
            max_physical_pages: 9,
            max_regions: 2,
            max_owners: 2,
            max_access_bytes: 0x1_0000,
        })
        .expect("icon fixture memory");
        let runtime_owner = memory
            .create_owner(MemoryOwnerKind::Runtime)
            .expect("runtime owner");
        let runtime_range = memory
            .reserve_fixed(
                runtime_owner,
                MemoryRegionKind::Runtime,
                MEMORY_BASE,
                0x3000,
            )
            .expect("runtime range");
        memory
            .commit(
                runtime_owner,
                runtime_range,
                PageProtection {
                    read: true,
                    write: true,
                    execute: false,
                },
            )
            .expect("commit runtime range");
        memory
            .write_initialization(
                runtime_owner,
                IAT_ADDRESS,
                &ICON_ENDPOINT_ADDRESS.0.to_le_bytes(),
            )
            .expect("initialize LoadIconA IAT");
        for (index, (instance, icon_id)) in [
            (ICON_IMAGE_BASE, 101_u32),
            (ICON_IMAGE_BASE, 101),
            (ICON_IMAGE_BASE, 0xffff),
            (ICON_IMAGE_BASE, 101),
            (ICON_IMAGE_BASE + 0x1000, 101),
            (ICON_IMAGE_BASE, MEMORY_BASE.0),
            (0, 0x7f00),
        ]
        .into_iter()
        .enumerate()
        {
            write_words(
                &mut memory,
                runtime_owner,
                GuestAddress(STACK_POINTER + index as u32 * 0x20),
                &[RETURN_ADDRESS.0 + index as u32, instance, icon_id],
            );
        }

        let limits = Pe32Limits::default();
        let snapshot = ModuleSnapshot::from_bytes(bytes, &limits).expect("icon fixture snapshot");
        let loader = build_pe32_load_plan(&snapshot, limits).expect("icon fixture load plan");
        assert_eq!(loader.load_plan.image.preferred_image_base, ICON_IMAGE_BASE);
        let materialization = materialize_pe32_image(
            &mut memory,
            &snapshot,
            &loader,
            GuestAddress(ICON_IMAGE_BASE),
        )
        .expect("materialize icon fixture");
        memory
            .seal_owner(materialization.layout.owner)
            .expect("seal icon image owner");
        memory
            .seal_owner(runtime_owner)
            .expect("seal runtime owner");
        let reflection = memory
            .pending_reflection()
            .expect("icon initial reflection");
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .expect("acknowledge icon initial reflection");

        let mut runtime = controlled_runtime();
        runtime
            .register_main_module(RuntimeModule {
                owner: "user32-icon-test".to_owned(),
                variant_id: "mapped-icon-resource-fixture".to_owned(),
                namespace: LOGICAL_LOADED_MODULE_NAMESPACE.to_owned(),
                handle: ICON_MODULE_HANDLE,
                requested_name: "icon-resource.exe".to_owned(),
                canonical_name: "icon-resource.exe".to_owned(),
                runtime_system_module: false,
            })
            .expect("register icon main module");
        ExecutionSession::new(&mut memory, &mut runtime)
            .register_module_image(ICON_MODULE_HANDLE, &loader, &materialization)
            .expect("register icon module image");

        let thunk = ImportThunk {
            id: ImportThunkId(192),
            address: ICON_ENDPOINT_ADDRESS,
            requester_owner: runtime_owner,
            module: "USER32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_13d8),
            iat_address: IAT_ADDRESS,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "LoadIconA".to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        };
        let endpoint = ApiEndpointIdentity::from_import_thunk(&thunk);
        let contract = load_icon_a_contract(endpoint.clone()).expect("LoadIconA contract");
        let dispatcher =
            ApiDispatcher::new([endpoint.clone()], [contract]).expect("LoadIconA dispatcher");
        Self {
            memory,
            runtime,
            dispatcher,
            endpoint,
            policy: User32CompatibilityPolicy::controlled_v4(),
        }
    }

    fn dispatch(&mut self, index: u32) -> DispatchOutcome {
        let stack_pointer = GuestAddress(STACK_POINTER + index * 0x20);
        let return_address = GuestAddress(RETURN_ADDRESS.0 + index);
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        self.dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: self.endpoint.id,
                endpoint_address: self.endpoint.address,
                stack_pointer,
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

struct CursorFixture {
    memory: GuestMemory,
    runtime: RuntimeState,
    dispatcher: ApiDispatcher,
    endpoint: ApiEndpointIdentity,
    policy: User32CompatibilityPolicy,
}

impl CursorFixture {
    fn new() -> Self {
        let (mut memory, owner) = initialized_memory();
        let thunk = ImportThunk {
            id: ImportThunkId(244),
            address: CURSOR_ENDPOINT_ADDRESS,
            requester_owner: owner,
            module: "USER32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_14a8),
            iat_address: IAT_ADDRESS,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "LoadCursorA".to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        };
        memory
            .write_initialization(owner, IAT_ADDRESS, &CURSOR_ENDPOINT_ADDRESS.0.to_le_bytes())
            .expect("initialize LoadCursorA IAT");
        for (index, (instance, cursor_name)) in [
            (0_u32, 0x7f00_u32),
            (0, 0x7f00),
            (0, 0xffff),
            (0, 0x7f01),
            (ICON_IMAGE_BASE, 106),
            (0, MEMORY_BASE.0),
        ]
        .into_iter()
        .enumerate()
        {
            write_words(
                &mut memory,
                owner,
                GuestAddress(STACK_POINTER + index as u32 * 0x20),
                &[
                    CURSOR_RETURN_ADDRESS.0 + index as u32,
                    instance,
                    cursor_name,
                ],
            );
        }
        memory.seal_owner(owner).expect("seal cursor fixture owner");
        let reflection = memory.pending_reflection().expect("cursor reflection");
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .expect("acknowledge cursor reflection");

        let endpoint = ApiEndpointIdentity::from_import_thunk(&thunk);
        let contract = load_cursor_a_contract(endpoint.clone()).expect("LoadCursorA contract");
        let dispatcher =
            ApiDispatcher::new([endpoint.clone()], [contract]).expect("LoadCursorA dispatcher");
        Self {
            memory,
            runtime: controlled_runtime(),
            dispatcher,
            endpoint,
            policy: User32CompatibilityPolicy::controlled_v4(),
        }
    }

    fn dispatch(&mut self, index: u32) -> DispatchOutcome {
        let stack_pointer = GuestAddress(STACK_POINTER + index * 0x20);
        let return_address = GuestAddress(CURSOR_RETURN_ADDRESS.0 + index);
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        self.dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: self.endpoint.id,
                endpoint_address: self.endpoint.address,
                stack_pointer,
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
fn load_icon_a_contract_is_bounded_stdcall_with_handler_last_error() {
    let (_, owner) = initialized_memory();
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(192),
        address: ICON_ENDPOINT_ADDRESS,
        requester_owner: owner,
        module: "USER32.dll".to_owned(),
        iat_rva: vine_core::Rva(0x0d_13d8),
        iat_address: IAT_ADDRESS,
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "LoadIconA".to_owned(),
            hint: 0,
            name_rva: vine_core::Rva(0),
        },
    });
    let contract = load_icon_a_contract(endpoint).expect("LoadIconA contract");
    assert_eq!(contract.abi.calling_convention, CallingConvention::Stdcall);
    assert_eq!(contract.abi.arguments.len(), 2);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    assert_eq!(contract.reentrancy_policy, ReentrancyPolicy::Allowed);
    assert_eq!(contract.callback_policy, CallbackPolicy::Forbidden);
}

#[test]
fn load_cursor_a_contract_is_bounded_stdcall_with_handler_last_error() {
    let (_, owner) = initialized_memory();
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(244),
        address: CURSOR_ENDPOINT_ADDRESS,
        requester_owner: owner,
        module: "USER32.dll".to_owned(),
        iat_rva: vine_core::Rva(0x0d_14a8),
        iat_address: IAT_ADDRESS,
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "LoadCursorA".to_owned(),
            hint: 0,
            name_rva: vine_core::Rva(0),
        },
    });
    let contract = load_cursor_a_contract(endpoint).expect("LoadCursorA contract");
    assert_eq!(contract.abi.calling_convention, CallingConvention::Stdcall);
    assert_eq!(contract.abi.arguments.len(), 2);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    assert_eq!(contract.reentrancy_policy, ReentrancyPolicy::Allowed);
    assert_eq!(contract.callback_policy, CallbackPolicy::Forbidden);
}

#[test]
fn load_icon_a_selects_the_exact_dib_and_reuses_process_shared_identity() {
    let mut fixture = IconFixture::new();
    let memory_generation = fixture.memory.stats().generation;
    let first = match fixture.dispatch(0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected first LoadIconA completion, got {other:?}"),
    };
    assert_eq!(first.eax, RegisterUpdate::Set(0x4901_0001));
    assert_eq!(first.stack_pointer, GuestAddress(STACK_POINTER + 12));
    assert_eq!(first.instruction_pointer, RETURN_ADDRESS);
    assert_eq!(first.last_error, 0);
    assert!(first.guest_writes.is_empty());
    assert_eq!(fixture.memory.stats().generation, memory_generation);
    assert_eq!(fixture.runtime.icons().len(), 1);
    let icon = fixture
        .runtime
        .icon(0x4901_0001)
        .expect("logical shared icon")
        .clone();
    assert_eq!(icon.id.0, 1);
    assert_eq!(icon.namespace, "vine.logical-icon.v1");
    assert!(icon.shared);
    assert_eq!(
        icon.resource.module_load_base,
        GuestAddress(ICON_IMAGE_BASE)
    );
    assert_eq!(icon.resource.module_source_sha256, ICON_FIXTURE_SHA256);
    assert_eq!(icon.resource.group_resource_id, 101);
    assert_eq!(icon.resource.language_id, 1041);
    assert_eq!(icon.resource.icon_resource_id, 2);
    assert_eq!(icon.resource.group_data_size, 48);
    assert_eq!(icon.resource.icon_data_size, 4_264);
    assert_eq!(icon.resource.width, 32);
    assert_eq!(icon.resource.height, 32);
    assert_eq!(icon.resource.planes, 1);
    assert_eq!(icon.resource.bits_per_pixel, 32);

    let second = match fixture.dispatch(1) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected repeated LoadIconA completion, got {other:?}"),
    };
    assert_eq!(second.eax, RegisterUpdate::Set(icon.handle));
    assert_eq!(second.last_error, 0);
    assert_eq!(fixture.runtime.icons().len(), 1);
    assert_eq!(fixture.runtime.icon(icon.handle).unwrap(), &icon);
    assert_eq!(fixture.memory.stats().generation, memory_generation);

    fixture
        .runtime
        .terminate_current_process(0)
        .expect("terminate icon process");
    assert!(fixture.runtime.icons().is_empty());
    assert_eq!(
        fixture
            .runtime
            .icon(icon.handle)
            .expect_err("process teardown retires HICON")
            .code,
        "icon-handle-unknown"
    );
}

#[test]
fn load_icon_a_negative_paths_are_failure_atomic_and_do_not_consume_identity() {
    let mut missing = IconFixture::new();
    let generation = missing.memory.stats().generation;
    let completion = match missing.dispatch(2) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected missing resource completion, got {other:?}"),
    };
    assert_eq!(completion.eax, RegisterUpdate::Set(0));
    assert_eq!(completion.last_error, 1813);
    assert!(missing.runtime.icons().is_empty());
    assert_eq!(missing.memory.stats().generation, generation);
    let valid = match missing.dispatch(3) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected valid retry completion, got {other:?}"),
    };
    assert_eq!(valid.eax, RegisterUpdate::Set(0x4901_0001));

    let mut unknown_module = IconFixture::new();
    let completion = match unknown_module.dispatch(4) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid module completion, got {other:?}"),
    };
    assert_eq!(completion.eax, RegisterUpdate::Set(0));
    assert_eq!(completion.last_error, 6);
    assert!(unknown_module.runtime.icons().is_empty());

    let mut string_name = IconFixture::new();
    let fault = match string_name.dispatch(5) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected string resource fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "load-icon-a-string-resource-unsupported");
    assert!(string_name.runtime.icons().is_empty());

    let mut disabled = IconFixture::new();
    disabled.policy.load_icon_a = None;
    let fault = match disabled.dispatch(0) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected disabled icon policy fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "load-icon-a-policy-disabled");
    assert!(disabled.runtime.icons().is_empty());

    let mut malformed = IconFixture::from_bytes(malformed_group_icon_bytes());
    let completion = match malformed.dispatch(0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected malformed group failure, got {other:?}"),
    };
    assert_eq!(completion.eax, RegisterUpdate::Set(0));
    assert_eq!(completion.last_error, 1812);
    assert!(malformed.runtime.icons().is_empty());

    let mut system = IconFixture::new();
    let fault = match system.dispatch(6) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected system icon fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "load-icon-a-system-resource-unsupported");
    assert!(system.runtime.icons().is_empty());

    let mut png = IconFixture::from_bytes(png_selected_icon_bytes());
    let fault = match png.dispatch(0) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected PNG icon fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "load-icon-a-selected-png-unsupported");
    assert!(png.runtime.icons().is_empty());

    let mut alternate_size = IconFixture::new();
    let icon_policy = alternate_size
        .policy
        .load_icon_a
        .as_mut()
        .expect("icon policy");
    icon_policy.target_width = 24;
    icon_policy.target_height = 24;
    let fault = match alternate_size.dispatch(0) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected alternate-size icon fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "load-icon-a-size-selection-unsupported");
    assert!(alternate_size.runtime.icons().is_empty());
}

#[test]
fn load_cursor_a_preserves_last_error_and_reuses_shared_arrow_identity() {
    let mut fixture = CursorFixture::new();
    let memory_generation = fixture.memory.stats().generation;
    let first = match fixture.dispatch(0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected first LoadCursorA completion, got {other:?}"),
    };
    assert_eq!(first.eax, RegisterUpdate::Set(0x4301_0001));
    assert_eq!(first.stack_pointer, GuestAddress(STACK_POINTER + 12));
    assert_eq!(first.instruction_pointer, CURSOR_RETURN_ADDRESS);
    assert_eq!(first.last_error, INITIAL_LAST_ERROR);
    assert!(first.guest_writes.is_empty());
    assert_eq!(fixture.memory.stats().generation, memory_generation);
    assert_eq!(fixture.runtime.cursors().len(), 1);
    let cursor = fixture
        .runtime
        .cursor(0x4301_0001)
        .expect("logical shared cursor")
        .clone();
    assert_eq!(cursor.id.0, 1);
    assert_eq!(cursor.namespace, "vine.logical-cursor.v1");
    assert!(cursor.shared);
    assert_eq!(
        cursor.source,
        RuntimeCursorSource::SystemPredefined {
            resource_id: 0x7f00,
            cursor: RuntimeSystemCursorKind::Arrow,
        }
    );

    let repeated = match fixture.dispatch(1) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected repeated LoadCursorA completion, got {other:?}"),
    };
    assert_eq!(repeated.eax, RegisterUpdate::Set(cursor.handle));
    assert_eq!(repeated.last_error, INITIAL_LAST_ERROR);
    assert_eq!(fixture.runtime.cursors().len(), 1);
    assert_eq!(fixture.runtime.cursor(cursor.handle).unwrap(), &cursor);
    assert_eq!(fixture.memory.stats().generation, memory_generation);

    fixture
        .runtime
        .terminate_current_process(0)
        .expect("terminate cursor process");
    assert!(fixture.runtime.cursors().is_empty());
    assert_eq!(
        fixture
            .runtime
            .cursor(cursor.handle)
            .expect_err("process teardown retires HCURSOR")
            .code,
        "cursor-handle-unknown"
    );
}

#[test]
fn load_cursor_a_negative_variants_are_failure_atomic() {
    let mut missing = CursorFixture::new();
    let memory_generation = missing.memory.stats().generation;
    let completion = match missing.dispatch(2) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected missing cursor completion, got {other:?}"),
    };
    assert_eq!(completion.eax, RegisterUpdate::Set(0));
    assert_eq!(completion.last_error, 1814);
    assert!(missing.runtime.cursors().is_empty());
    assert_eq!(missing.memory.stats().generation, memory_generation);
    let valid = match missing.dispatch(0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected valid cursor retry, got {other:?}"),
    };
    assert_eq!(valid.eax, RegisterUpdate::Set(0x4301_0001));

    for (index, expected_code) in [
        (3, "load-cursor-a-predefined-variant-unsupported"),
        (4, "load-cursor-a-module-resource-unsupported"),
        (5, "load-cursor-a-string-resource-unsupported"),
    ] {
        let mut fixture = CursorFixture::new();
        let fault = match fixture.dispatch(index) {
            DispatchOutcome::Fault(fault) => fault,
            other => panic!("expected cursor variant fault, got {other:?}"),
        };
        assert_eq!(fault.class, ApiFaultClass::Unsupported);
        assert_eq!(fault.code, expected_code);
        assert!(fixture.runtime.cursors().is_empty());
    }

    let mut disabled = CursorFixture::new();
    disabled.policy.load_cursor_a = None;
    let fault = match disabled.dispatch(0) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected disabled cursor policy fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "load-cursor-a-policy-disabled");
    assert!(disabled.runtime.cursors().is_empty());
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

fn icon_fixture_bytes() -> Vec<u8> {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".work/toolchain-spike/icon-resource.exe");
    fs::read(&fixture).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}); run `npm run check:toolchain` first",
            fixture.display()
        )
    })
}

fn malformed_group_icon_bytes() -> Vec<u8> {
    let mut bytes = icon_fixture_bytes();
    let limits = Pe32Limits::default();
    let snapshot = ModuleSnapshot::from_bytes(bytes.clone(), &limits).unwrap();
    let loader = build_pe32_load_plan(&snapshot, limits).unwrap();
    let group_rva = loader
        .load_plan
        .resources
        .as_ref()
        .and_then(|resources| resources.integer_entries(14, 101).next())
        .expect("group icon resource")
        .data_rva
        .0;
    let offset = rva_file_offset(&loader, group_rva).expect("group icon file offset");
    bytes[offset + 2..offset + 4].copy_from_slice(&2_u16.to_le_bytes());
    bytes
}

fn png_selected_icon_bytes() -> Vec<u8> {
    let mut bytes = icon_fixture_bytes();
    let limits = Pe32Limits::default();
    let snapshot = ModuleSnapshot::from_bytes(bytes.clone(), &limits).unwrap();
    let loader = build_pe32_load_plan(&snapshot, limits).unwrap();
    let icon_rva = loader
        .load_plan
        .resources
        .as_ref()
        .and_then(|resources| resources.integer_entries(3, 2).next())
        .expect("selected 32-pixel icon resource")
        .data_rva
        .0;
    let offset = rva_file_offset(&loader, icon_rva).expect("selected icon file offset");
    bytes[offset..offset + 8].copy_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    bytes
}

fn rva_file_offset(loader: &LoaderIngressReport, rva: u32) -> Option<usize> {
    loader.load_plan.sections.iter().find_map(|section| {
        let offset = rva.checked_sub(section.destination_rva.0)?;
        (offset < section.copy_size)
            .then(|| usize::try_from(section.source_offset + offset).ok())
            .flatten()
    })
}

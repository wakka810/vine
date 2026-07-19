use std::fs;
use std::path::PathBuf;

use vine_core::{
    ApiDirection, ApiDispatcher, ApiEndpointIdentity, ApiFaultClass, ApiOwnedTrap,
    ApiPointeeSchema, ApiReturnSchema, ApiValueSchema, ApiWriteRequirement,
    CALLBACK_SCRATCH_ARENA_BASE, CallbackPolicy, CallingConvention, DispatchOutcome,
    ExecutionSession, GUEST_PAGE_SIZE, GuestAddress, GuestCallbackArgument,
    GuestCallbackCompletionPolicy, GuestMemory, GuestRange, ImportTarget, ImportThunk,
    ImportThunkDisposition, ImportThunkId, LOGICAL_LOADED_MODULE_NAMESPACE, LastErrorPolicy,
    LoaderIngressReport, MemoryLimits, MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind,
    ModuleSnapshot, PageProtection, Pe32Limits, ProcessAddressSpaceLayout,
    RUNTIME_PRIMARY_MONITOR_HANDLE, ReentrancyPolicy, RegisterUpdate, RuntimeBootstrap,
    RuntimeCallbackFrameId, RuntimeClockState, RuntimeCurrentDirectoryA, RuntimeCursorSource,
    RuntimeDisplayProfile, RuntimeGuestCallbackSemanticAction, RuntimeInputProfile, RuntimeModule,
    RuntimeProcessId, RuntimeProcessImagePathA, RuntimeRect, RuntimeSetWindowPosCallbackAction,
    RuntimeSetWindowPosCallbackMessage, RuntimeStartupInfo, RuntimeState, RuntimeSystemCursorKind,
    RuntimeThreadId, RuntimeWindowCreationCallbackAction, RuntimeWindowCreationMessage,
    RuntimeWindowGeometry, RuntimeWindowLifecycle, StackCleanup, User32CompatibilityPolicy,
    build_pe32_load_plan, create_window_ex_a_contract, def_window_proc_a_contract,
    dispatch_gdi32_api, dispatch_user32_api_with_policy, enable_menu_item_contract,
    enum_display_monitors_contract, fill_rect_contract, gdi32_contract_for_endpoint,
    get_client_rect_contract, get_dc_contract, get_stock_object_contract, get_system_menu_contract,
    get_system_metrics_contract, load_cursor_a_contract, load_icon_a_contract,
    load_image_a_contract, materialize_pe32_image, register_class_ex_a_contract,
    release_dc_contract, send_message_a_contract, set_window_long_a_contract,
    set_window_pos_contract, swap_mouse_button_contract, user32_contract_for_endpoint,
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
const IMAGE_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0c00);
const IMAGE_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 4);
const IMAGE_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x1200;
const IMAGE_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1006_7f43);
const CURSOR_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0f30);
const CURSOR_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1006_7d09);
const CLASS_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0be0);
const CLASS_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1006_7d28);
const METRICS_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0ca0);
const METRICS_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1006_7ddb);
const ICON_IMAGE_BASE: u32 = 0x0040_0000;
const ICON_MODULE_HANDLE: u32 = 0x4d01_000f;
const ICON_FIXTURE_SHA256: &str =
    "2ddcca724644e16405cc03d71e3b0d5460ddb72ac60072a1781f128fd5e4da4c";
const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;
const INITIAL_LAST_ERROR: u32 = 0x1234_5678;
const CLASS_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 8);
const CLASS_STRUCT_BASE: u32 = MEMORY_BASE.0 + 0x2300;
const CLASS_NAME_BASE: u32 = MEMORY_BASE.0 + 0x2700;
const CREATE_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0e70);
const DEF_WINDOW_PROC_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_13e0);
const CREATE_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 12);
const DEF_WINDOW_PROC_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 16);
const CREATE_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x1800;
const DEF_WINDOW_PROC_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x1900;
const CREATE_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1006_7efa);
const DEF_WINDOW_PROC_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1009_33bf);
const SEND_MESSAGE_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0e40);
const WINDOW_IMAGE_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 20);
const SEND_MESSAGE_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 24);
const WINDOW_IMAGE_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x1c00;
const SEND_MESSAGE_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x1d00;
const SEND_MESSAGE_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1006_7f58);
const SET_WINDOW_POS_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0c90);
const SET_WINDOW_POS_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 28);
const SET_WINDOW_POS_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x1e00;
const SET_WINDOW_POS_SHOW_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1006_7f88);
const SET_WINDOW_POS_HIDE_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1006_7f9f);
const GET_CLIENT_RECT_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0c60);
const GET_CLIENT_RECT_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 32);
const GET_CLIENT_RECT_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x1f00;
const GET_CLIENT_RECT_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1005_c997);
const CLIENT_RECT_OUTPUT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2e00);
const GET_DC_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0ee0);
const GET_STOCK_OBJECT_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_3100);
const FILL_RECT_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0e90);
const RELEASE_DC_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0ef0);
const GET_DC_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 36);
const GET_STOCK_OBJECT_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 40);
const FILL_RECT_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 44);
const RELEASE_DC_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 48);
const GET_DC_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x1f40;
const GET_STOCK_OBJECT_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x1f60;
const FILL_RECT_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x1f80;
const RELEASE_DC_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x1fa0;
const GET_DC_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1005_c9a4);
const GET_STOCK_OBJECT_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1005_c9ae);
const FILL_RECT_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1005_c9bb);
const RELEASE_DC_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1005_c9cb);
const SET_WINDOW_LONG_A_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0d30);
const SET_WINDOW_LONG_A_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 52);
const SET_WINDOW_LONG_A_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x1fc0;
const SET_WINDOW_LONG_A_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1005_cae3);
const WINDOW_TITLE_BASE: u32 = MEMORY_BASE.0 + 0x2b00;
const GET_SYSTEM_MENU_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0b10);
const ENABLE_MENU_ITEM_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0b20);
const GET_SYSTEM_MENU_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 20);
const ENABLE_MENU_ITEM_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 24);
const GET_SYSTEM_MENU_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x1a00;
const ENABLE_MENU_ITEM_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x1b00;
const GET_SYSTEM_MENU_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1009_274c);
const ENABLE_MENU_ITEM_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1009_2753);

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
        let policy = User32CompatibilityPolicy::controlled_v14();
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
        let mut policy = User32CompatibilityPolicy::controlled_v14();
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

struct MetricsFixture {
    memory: GuestMemory,
    runtime: RuntimeState,
    dispatcher: ApiDispatcher,
    endpoint: ApiEndpointIdentity,
    policy: User32CompatibilityPolicy,
}

impl MetricsFixture {
    fn new(configure_display: bool) -> Self {
        let (mut memory, owner) = initialized_memory();
        let thunk = ImportThunk {
            id: ImportThunkId(203),
            address: METRICS_ENDPOINT_ADDRESS,
            requester_owner: owner,
            module: "USER32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_1404),
            iat_address: IAT_ADDRESS,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "GetSystemMetrics".to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        };
        memory
            .write_initialization(
                owner,
                IAT_ADDRESS,
                &METRICS_ENDPOINT_ADDRESS.0.to_le_bytes(),
            )
            .expect("initialize GetSystemMetrics IAT");
        for (index, metric) in [7_u32, 8, 4, 0, 1, 50, 49, 2].into_iter().enumerate() {
            write_words(
                &mut memory,
                owner,
                GuestAddress(STACK_POINTER + index as u32 * 0x20),
                &[METRICS_RETURN_ADDRESS.0 + index as u32, metric],
            );
        }
        memory
            .seal_owner(owner)
            .expect("seal metrics fixture owner");
        let reflection = memory.pending_reflection().expect("metrics reflection");
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .expect("acknowledge metrics reflection");

        let endpoint = ApiEndpointIdentity::from_import_thunk(&thunk);
        let contract =
            get_system_metrics_contract(endpoint.clone()).expect("GetSystemMetrics contract");
        let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract])
            .expect("GetSystemMetrics dispatcher");
        let policy = User32CompatibilityPolicy::controlled_v14();
        let mut runtime = controlled_runtime();
        if configure_display {
            runtime
                .configure_display_profile(policy.display_profile().expect("display profile"))
                .expect("configure metrics display");
        }
        Self {
            memory,
            runtime,
            dispatcher,
            endpoint,
            policy,
        }
    }

    fn dispatch(&mut self, index: u32) -> DispatchOutcome {
        let stack_pointer = GuestAddress(STACK_POINTER + index * 0x20);
        let return_address = GuestAddress(METRICS_RETURN_ADDRESS.0 + index);
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

struct InvalidMenuFixture {
    memory: GuestMemory,
    runtime: RuntimeState,
    dispatcher: ApiDispatcher,
    get_system_menu_endpoint: ApiEndpointIdentity,
    enable_menu_item_endpoint: ApiEndpointIdentity,
    policy: User32CompatibilityPolicy,
}

impl InvalidMenuFixture {
    fn new() -> Self {
        let (mut memory, owner) = initialized_memory();
        for (iat, endpoint) in [
            (
                GET_SYSTEM_MENU_IAT_ADDRESS,
                GET_SYSTEM_MENU_ENDPOINT_ADDRESS,
            ),
            (
                ENABLE_MENU_ITEM_IAT_ADDRESS,
                ENABLE_MENU_ITEM_ENDPOINT_ADDRESS,
            ),
        ] {
            memory
                .write_initialization(owner, iat, &endpoint.0.to_le_bytes())
                .expect("initialize menu fixture IAT");
        }
        for (index, (window, revert)) in [(0_u32, 0_u32), (1, 0), (0, 1)].into_iter().enumerate() {
            write_words(
                &mut memory,
                owner,
                GuestAddress(GET_SYSTEM_MENU_STACK_POINTER + index as u32 * 0x20),
                &[
                    GET_SYSTEM_MENU_RETURN_ADDRESS.0 + index as u32,
                    window,
                    revert,
                ],
            );
        }
        for (index, (menu, item, flags)) in [
            (0_u32, 0x0000_f060_u32, 0_u32),
            (1, 0x0000_f060, 0),
            (0, 0x0000_f061, 0),
            (0, 0x0000_f060, 1),
        ]
        .into_iter()
        .enumerate()
        {
            write_words(
                &mut memory,
                owner,
                GuestAddress(ENABLE_MENU_ITEM_STACK_POINTER + index as u32 * 0x20),
                &[
                    ENABLE_MENU_ITEM_RETURN_ADDRESS.0 + index as u32,
                    menu,
                    item,
                    flags,
                ],
            );
        }
        memory.seal_owner(owner).expect("seal menu fixture owner");
        let reflection = memory.pending_reflection().expect("menu reflection");
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .expect("acknowledge menu reflection");

        let get_system_menu_endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
            id: ImportThunkId(178),
            address: GET_SYSTEM_MENU_ENDPOINT_ADDRESS,
            requester_owner: owner,
            module: "USER32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_13a0),
            iat_address: GET_SYSTEM_MENU_IAT_ADDRESS,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "GetSystemMenu".to_owned(),
                hint: 381,
                name_rva: vine_core::Rva(0),
            },
        });
        let enable_menu_item_endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
            id: ImportThunkId(179),
            address: ENABLE_MENU_ITEM_ENDPOINT_ADDRESS,
            requester_owner: owner,
            module: "USER32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_13a4),
            iat_address: ENABLE_MENU_ITEM_IAT_ADDRESS,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "EnableMenuItem".to_owned(),
                hint: 214,
                name_rva: vine_core::Rva(0),
            },
        });
        let dispatcher = ApiDispatcher::new(
            [
                get_system_menu_endpoint.clone(),
                enable_menu_item_endpoint.clone(),
            ],
            [
                get_system_menu_contract(get_system_menu_endpoint.clone())
                    .expect("GetSystemMenu contract"),
                enable_menu_item_contract(enable_menu_item_endpoint.clone())
                    .expect("EnableMenuItem contract"),
            ],
        )
        .expect("invalid menu dispatcher");
        Self {
            memory,
            runtime: controlled_runtime(),
            dispatcher,
            get_system_menu_endpoint,
            enable_menu_item_endpoint,
            policy: User32CompatibilityPolicy::controlled_v14(),
        }
    }

    fn dispatch_get_system_menu(&mut self, index: u32) -> DispatchOutcome {
        self.dispatch(
            self.get_system_menu_endpoint.clone(),
            GuestAddress(GET_SYSTEM_MENU_STACK_POINTER + index * 0x20),
            GuestAddress(GET_SYSTEM_MENU_RETURN_ADDRESS.0 + index),
        )
    }

    fn dispatch_enable_menu_item(&mut self, index: u32) -> DispatchOutcome {
        self.dispatch(
            self.enable_menu_item_endpoint.clone(),
            GuestAddress(ENABLE_MENU_ITEM_STACK_POINTER + index * 0x20),
            GuestAddress(ENABLE_MENU_ITEM_RETURN_ADDRESS.0 + index),
        )
    }

    fn dispatch(
        &mut self,
        endpoint: ApiEndpointIdentity,
        stack_pointer: GuestAddress,
        return_address: GuestAddress,
    ) -> DispatchOutcome {
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        self.dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: endpoint.id,
                endpoint_address: endpoint.address,
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

struct IconFixture {
    memory: GuestMemory,
    runtime: RuntimeState,
    dispatcher: ApiDispatcher,
    endpoint: ApiEndpointIdentity,
    image_endpoint: ApiEndpointIdentity,
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
            max_physical_pages: 10,
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
        memory
            .write_initialization(
                runtime_owner,
                IMAGE_IAT_ADDRESS,
                &IMAGE_ENDPOINT_ADDRESS.0.to_le_bytes(),
            )
            .expect("initialize LoadImageA IAT");
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

        for (index, (instance, name, image_type, width, height, flags)) in [
            (ICON_IMAGE_BASE, 101_u32, 1_u32, 16_u32, 16_u32, 0x8000_u32),
            (ICON_IMAGE_BASE, 101, 1, 16, 16, 0x8000),
            (ICON_IMAGE_BASE, 0xffff, 1, 16, 16, 0x8000),
            (ICON_IMAGE_BASE, 101, 2, 16, 16, 0x8000),
            (ICON_IMAGE_BASE, 101, 1, 32, 16, 0x8000),
            (ICON_IMAGE_BASE, 101, 1, 16, 16, 0),
            (ICON_IMAGE_BASE, MEMORY_BASE.0, 1, 16, 16, 0x8000),
            (ICON_IMAGE_BASE + 0x1000, 101, 1, 16, 16, 0x8000),
        ]
        .into_iter()
        .enumerate()
        {
            write_words(
                &mut memory,
                runtime_owner,
                GuestAddress(IMAGE_STACK_POINTER + index as u32 * 0x20),
                &[
                    IMAGE_RETURN_ADDRESS.0 + index as u32,
                    instance,
                    name,
                    image_type,
                    width,
                    height,
                    flags,
                ],
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
        let image_endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
            id: ImportThunkId(193),
            address: IMAGE_ENDPOINT_ADDRESS,
            requester_owner: runtime_owner,
            module: "USER32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_13dc),
            iat_address: IMAGE_IAT_ADDRESS,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "LoadImageA".to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        });
        let contract = load_icon_a_contract(endpoint.clone()).expect("LoadIconA contract");
        let image_contract =
            load_image_a_contract(image_endpoint.clone()).expect("LoadImageA contract");
        let dispatcher = ApiDispatcher::new(
            [endpoint.clone(), image_endpoint.clone()],
            [contract, image_contract],
        )
        .expect("module icon dispatcher");
        Self {
            memory,
            runtime,
            dispatcher,
            endpoint,
            image_endpoint,
            policy: User32CompatibilityPolicy::controlled_v14(),
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

    fn dispatch_image(&mut self, index: u32) -> DispatchOutcome {
        let stack_pointer = GuestAddress(IMAGE_STACK_POINTER + index * 0x20);
        let return_address = GuestAddress(IMAGE_RETURN_ADDRESS.0 + index);
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        self.dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: self.image_endpoint.id,
                endpoint_address: self.image_endpoint.address,
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
        let bytes = icon_fixture_bytes();
        let limits = Pe32Limits::default();
        ModuleSnapshot::from_bytes(bytes.clone(), &limits)
            .expect("fixed cursor fixture snapshot")
            .verify_expected_sha256(ICON_FIXTURE_SHA256)
            .expect("fixed cursor fixture fingerprint");
        Self::from_bytes(bytes)
    }

    fn from_bytes(bytes: Vec<u8>) -> Self {
        let mut memory = GuestMemory::new(MemoryLimits {
            max_physical_pages: 10,
            max_regions: 2,
            max_owners: 2,
            max_access_bytes: 0x1_0000,
        })
        .expect("cursor fixture memory");
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
        let thunk = ImportThunk {
            id: ImportThunkId(244),
            address: CURSOR_ENDPOINT_ADDRESS,
            requester_owner: runtime_owner,
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
            .write_initialization(
                runtime_owner,
                IAT_ADDRESS,
                &CURSOR_ENDPOINT_ADDRESS.0.to_le_bytes(),
            )
            .expect("initialize LoadCursorA IAT");
        for (index, (instance, cursor_name)) in [
            (0_u32, 0x7f00_u32),
            (0, 0x7f00),
            (0, 0xffff),
            (0, 0x7f01),
            (ICON_IMAGE_BASE, 106),
            (0, MEMORY_BASE.0),
            (ICON_IMAGE_BASE, 106),
            (ICON_IMAGE_BASE, 0xffff),
            (ICON_IMAGE_BASE, 101),
            (ICON_IMAGE_BASE + 0x1000, 106),
            (ICON_IMAGE_BASE, MEMORY_BASE.0),
        ]
        .into_iter()
        .enumerate()
        {
            write_words(
                &mut memory,
                runtime_owner,
                GuestAddress(STACK_POINTER + index as u32 * 0x20),
                &[
                    CURSOR_RETURN_ADDRESS.0 + index as u32,
                    instance,
                    cursor_name,
                ],
            );
        }

        let limits = Pe32Limits::default();
        let snapshot = ModuleSnapshot::from_bytes(bytes, &limits).expect("cursor fixture snapshot");
        let loader = build_pe32_load_plan(&snapshot, limits).expect("cursor fixture load plan");
        assert_eq!(loader.load_plan.image.preferred_image_base, ICON_IMAGE_BASE);
        let materialization = materialize_pe32_image(
            &mut memory,
            &snapshot,
            &loader,
            GuestAddress(ICON_IMAGE_BASE),
        )
        .expect("materialize cursor fixture");
        memory
            .seal_owner(materialization.layout.owner)
            .expect("seal cursor image owner");
        memory
            .seal_owner(runtime_owner)
            .expect("seal cursor runtime owner");
        let reflection = memory
            .pending_reflection()
            .expect("cursor initial reflection");
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .expect("acknowledge cursor initial reflection");

        let mut runtime = controlled_runtime();
        runtime
            .register_main_module(RuntimeModule {
                owner: "user32-cursor-test".to_owned(),
                variant_id: "mapped-cursor-resource-fixture".to_owned(),
                namespace: LOGICAL_LOADED_MODULE_NAMESPACE.to_owned(),
                handle: ICON_MODULE_HANDLE,
                requested_name: "icon-resource.exe".to_owned(),
                canonical_name: "icon-resource.exe".to_owned(),
                runtime_system_module: false,
            })
            .expect("register cursor main module");
        ExecutionSession::new(&mut memory, &mut runtime)
            .register_module_image(ICON_MODULE_HANDLE, &loader, &materialization)
            .expect("register cursor module image");

        let endpoint = ApiEndpointIdentity::from_import_thunk(&thunk);
        let contract = load_cursor_a_contract(endpoint.clone()).expect("LoadCursorA contract");
        let dispatcher =
            ApiDispatcher::new([endpoint.clone()], [contract]).expect("LoadCursorA dispatcher");
        Self {
            memory,
            runtime,
            dispatcher,
            endpoint,
            policy: User32CompatibilityPolicy::controlled_v14(),
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

struct RegisterClassFixture {
    memory: GuestMemory,
    runtime: RuntimeState,
    dispatcher: ApiDispatcher,
    icon_endpoint: ApiEndpointIdentity,
    cursor_endpoint: ApiEndpointIdentity,
    class_endpoint: ApiEndpointIdentity,
    create_endpoint: ApiEndpointIdentity,
    def_window_proc_endpoint: ApiEndpointIdentity,
    image_endpoint: ApiEndpointIdentity,
    send_message_endpoint: ApiEndpointIdentity,
    set_window_pos_endpoint: ApiEndpointIdentity,
    get_client_rect_endpoint: ApiEndpointIdentity,
    get_dc_endpoint: ApiEndpointIdentity,
    get_stock_object_endpoint: ApiEndpointIdentity,
    fill_rect_endpoint: ApiEndpointIdentity,
    release_dc_endpoint: ApiEndpointIdentity,
    set_window_long_a_endpoint: ApiEndpointIdentity,
    window_procedure: GuestAddress,
    callback_sentinel: GuestAddress,
    policy: User32CompatibilityPolicy,
}

impl RegisterClassFixture {
    fn new() -> Self {
        Self::with_display(true)
    }

    fn without_display() -> Self {
        Self::with_display(false)
    }

    fn with_display(configure_display: bool) -> Self {
        let bytes = icon_fixture_bytes();
        let mut memory = GuestMemory::new(MemoryLimits {
            max_physical_pages: 11,
            max_regions: 3,
            max_owners: 3,
            max_access_bytes: 0x1_0000,
        })
        .expect("class fixture memory");
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

        let limits = Pe32Limits::default();
        let snapshot = ModuleSnapshot::from_bytes(bytes, &limits).expect("class fixture snapshot");
        snapshot
            .verify_expected_sha256(ICON_FIXTURE_SHA256)
            .expect("class fixture fingerprint");
        let loader = build_pe32_load_plan(&snapshot, limits).expect("class fixture load plan");
        let materialization = materialize_pe32_image(
            &mut memory,
            &snapshot,
            &loader,
            GuestAddress(ICON_IMAGE_BASE),
        )
        .expect("materialize class fixture");
        let window_procedure = materialization.entry_point_address;
        let callback_sentinel = GuestAddress(window_procedure.0 + 0x20);

        let icon_iat = IAT_ADDRESS;
        let cursor_iat = GuestAddress(IAT_ADDRESS.0 + 4);
        for (address, endpoint) in [
            (icon_iat, ICON_ENDPOINT_ADDRESS),
            (cursor_iat, CURSOR_ENDPOINT_ADDRESS),
            (CLASS_IAT_ADDRESS, CLASS_ENDPOINT_ADDRESS),
            (CREATE_IAT_ADDRESS, CREATE_ENDPOINT_ADDRESS),
            (
                DEF_WINDOW_PROC_IAT_ADDRESS,
                DEF_WINDOW_PROC_ENDPOINT_ADDRESS,
            ),
            (WINDOW_IMAGE_IAT_ADDRESS, IMAGE_ENDPOINT_ADDRESS),
            (SEND_MESSAGE_IAT_ADDRESS, SEND_MESSAGE_ENDPOINT_ADDRESS),
            (SET_WINDOW_POS_IAT_ADDRESS, SET_WINDOW_POS_ENDPOINT_ADDRESS),
            (
                GET_CLIENT_RECT_IAT_ADDRESS,
                GET_CLIENT_RECT_ENDPOINT_ADDRESS,
            ),
            (GET_DC_IAT_ADDRESS, GET_DC_ENDPOINT_ADDRESS),
            (
                GET_STOCK_OBJECT_IAT_ADDRESS,
                GET_STOCK_OBJECT_ENDPOINT_ADDRESS,
            ),
            (FILL_RECT_IAT_ADDRESS, FILL_RECT_ENDPOINT_ADDRESS),
            (RELEASE_DC_IAT_ADDRESS, RELEASE_DC_ENDPOINT_ADDRESS),
            (
                SET_WINDOW_LONG_A_IAT_ADDRESS,
                SET_WINDOW_LONG_A_ENDPOINT_ADDRESS,
            ),
        ] {
            memory
                .write_initialization(runtime_owner, address, &endpoint.0.to_le_bytes())
                .expect("initialize class fixture IAT");
        }
        write_words(
            &mut memory,
            runtime_owner,
            GuestAddress(STACK_POINTER),
            &[RETURN_ADDRESS.0, ICON_IMAGE_BASE, 101],
        );
        write_words(
            &mut memory,
            runtime_owner,
            GuestAddress(STACK_POINTER + 0x20),
            &[CURSOR_RETURN_ADDRESS.0, 0, 0x7f00],
        );

        let class_names: [&[u8]; 11] = [
            b"Vine Main Class\0",
            b"Vine Child Class\0",
            b"Vine Main Class\0",
            b"Invalid Size Class\0",
            b"Wrong Module Class\0",
            b"Non Executable Class\0",
            b"Wrong Icon Class\0",
            b"Wrong Cursor Class\0",
            b"Unsupported Style Class\0",
            &[0x82, 0xa0, 0],
            b"Unsupported Optional Class\0",
        ];
        for (index, name) in class_names.into_iter().enumerate() {
            memory
                .write_initialization(
                    runtime_owner,
                    GuestAddress(CLASS_NAME_BASE + u32::try_from(index).unwrap() * 0x40),
                    name,
                )
                .expect("initialize class name");
        }
        memory
            .write_initialization(
                runtime_owner,
                GuestAddress(CLASS_NAME_BASE + 11 * 0x40),
                &[b'A'; 64],
            )
            .expect("initialize unterminated bounded class name");
        memory
            .write_initialization(
                runtime_owner,
                GuestAddress(WINDOW_TITLE_BASE),
                b"Ethornell - controlled CreateWindowExA fixture\0",
            )
            .expect("initialize logical window title");
        write_words(
            &mut memory,
            runtime_owner,
            GuestAddress(CREATE_STACK_POINTER),
            &[
                CREATE_RETURN_ADDRESS.0,
                0x0004_0000,
                CLASS_NAME_BASE,
                WINDOW_TITLE_BASE,
                0x80ca_0000,
                557,
                214,
                806,
                629,
                0,
                0,
                ICON_IMAGE_BASE,
                0,
            ],
        );
        write_words(
            &mut memory,
            runtime_owner,
            GuestAddress(GET_CLIENT_RECT_STACK_POINTER),
            &[
                GET_CLIENT_RECT_RETURN_ADDRESS.0,
                0x4801_0001,
                CLIENT_RECT_OUTPUT_ADDRESS.0,
            ],
        );
        write_words(
            &mut memory,
            runtime_owner,
            GuestAddress(WINDOW_IMAGE_STACK_POINTER),
            &[
                IMAGE_RETURN_ADDRESS.0,
                ICON_IMAGE_BASE,
                101,
                1,
                16,
                16,
                0x0000_8000,
            ],
        );
        write_words(
            &mut memory,
            runtime_owner,
            GuestAddress(SEND_MESSAGE_STACK_POINTER),
            &[
                SEND_MESSAGE_RETURN_ADDRESS.0,
                0x4801_0001,
                0x0080,
                0,
                0x4901_0002,
            ],
        );
        write_words(
            &mut memory,
            runtime_owner,
            GuestAddress(SET_WINDOW_POS_STACK_POINTER),
            &[
                SET_WINDOW_POS_SHOW_RETURN_ADDRESS.0,
                0x4801_0001,
                0xffff_fffe,
                15_360,
                8_640,
                0,
                0,
                0x0000_0559,
            ],
        );

        let class_words = |index: u32| {
            let mut words = [
                48,
                if index == 1 { 0x3020 } else { 0x28 },
                window_procedure.0,
                0,
                0,
                ICON_IMAGE_BASE,
                0x4901_0001,
                0x4301_0001,
                0,
                0,
                CLASS_NAME_BASE + index * 0x40,
                0,
            ];
            match index {
                2 => words[10] = CLASS_NAME_BASE,
                3 => words[0] = 47,
                4 => words[5] = ICON_IMAGE_BASE + 0x1000,
                5 => words[2] = CLASS_NAME_BASE,
                6 => words[6] = 0x4901_ffff,
                7 => words[7] = 0x4301_ffff,
                8 => words[1] = 1,
                10 => words[8] = 1,
                _ => {}
            }
            words
        };
        for index in 0..12_u32 {
            let class_address = CLASS_STRUCT_BASE + index * 0x40;
            write_words(
                &mut memory,
                runtime_owner,
                GuestAddress(class_address),
                &class_words(index),
            );
            write_words(
                &mut memory,
                runtime_owner,
                GuestAddress(STACK_POINTER + (index + 2) * 0x20),
                &[CLASS_RETURN_ADDRESS.0 + index, class_address],
            );
        }
        write_words(
            &mut memory,
            runtime_owner,
            GuestAddress(STACK_POINTER + 14 * 0x20),
            &[CLASS_RETURN_ADDRESS.0 + 12, MEMORY_BASE.0 + 0x3000],
        );

        memory
            .seal_owner(materialization.layout.owner)
            .expect("seal class image owner");
        memory
            .seal_owner(runtime_owner)
            .expect("seal class runtime owner");
        let reflection = memory.pending_reflection().expect("class reflection");
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .expect("acknowledge class reflection");

        let mut runtime = controlled_runtime();
        runtime
            .register_main_module(RuntimeModule {
                owner: "user32-class-test".to_owned(),
                variant_id: "mapped-class-fixture".to_owned(),
                namespace: LOGICAL_LOADED_MODULE_NAMESPACE.to_owned(),
                handle: ICON_MODULE_HANDLE,
                requested_name: "icon-resource.exe".to_owned(),
                canonical_name: "icon-resource.exe".to_owned(),
                runtime_system_module: false,
            })
            .expect("register class main module");
        ExecutionSession::new(&mut memory, &mut runtime)
            .register_module_image(ICON_MODULE_HANDLE, &loader, &materialization)
            .expect("register class module image");

        let icon_endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
            id: ImportThunkId(192),
            address: ICON_ENDPOINT_ADDRESS,
            requester_owner: runtime_owner,
            module: "USER32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_13d8),
            iat_address: icon_iat,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "LoadIconA".to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        });
        let cursor_endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
            id: ImportThunkId(244),
            address: CURSOR_ENDPOINT_ADDRESS,
            requester_owner: runtime_owner,
            module: "USER32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_14a8),
            iat_address: cursor_iat,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "LoadCursorA".to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        });
        let class_endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
            id: ImportThunkId(191),
            address: CLASS_ENDPOINT_ADDRESS,
            requester_owner: runtime_owner,
            module: "USER32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_13d4),
            iat_address: CLASS_IAT_ADDRESS,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "RegisterClassExA".to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        });
        let create_endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
            id: ImportThunkId(232),
            address: CREATE_ENDPOINT_ADDRESS,
            requester_owner: runtime_owner,
            module: "USER32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_1478),
            iat_address: CREATE_IAT_ADDRESS,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "CreateWindowExA".to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        });
        let def_window_proc_endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
            id: ImportThunkId(155),
            address: DEF_WINDOW_PROC_ENDPOINT_ADDRESS,
            requester_owner: runtime_owner,
            module: "USER32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_147c),
            iat_address: DEF_WINDOW_PROC_IAT_ADDRESS,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "DefWindowProcA".to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        });
        let image_endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
            id: ImportThunkId(193),
            address: IMAGE_ENDPOINT_ADDRESS,
            requester_owner: runtime_owner,
            module: "USER32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_13dc),
            iat_address: WINDOW_IMAGE_IAT_ADDRESS,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "LoadImageA".to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        });
        let send_message_endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
            id: ImportThunkId(229),
            address: SEND_MESSAGE_ENDPOINT_ADDRESS,
            requester_owner: runtime_owner,
            module: "USER32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_146c),
            iat_address: SEND_MESSAGE_IAT_ADDRESS,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "SendMessageA".to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        });
        let set_window_pos_endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
            id: ImportThunkId(202),
            address: SET_WINDOW_POS_ENDPOINT_ADDRESS,
            requester_owner: runtime_owner,
            module: "USER32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_1400),
            iat_address: SET_WINDOW_POS_IAT_ADDRESS,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "SetWindowPos".to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        });
        let get_client_rect_endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
            id: ImportThunkId(199),
            address: GET_CLIENT_RECT_ENDPOINT_ADDRESS,
            requester_owner: runtime_owner,
            module: "USER32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_13f4),
            iat_address: GET_CLIENT_RECT_IAT_ADDRESS,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "GetClientRect".to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        });
        let get_dc_endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
            id: ImportThunkId(239),
            address: GET_DC_ENDPOINT_ADDRESS,
            requester_owner: runtime_owner,
            module: "USER32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_1494),
            iat_address: GET_DC_IAT_ADDRESS,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "GetDC".to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        });
        let get_stock_object_endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
            id: ImportThunkId(401),
            address: GET_STOCK_OBJECT_ENDPOINT_ADDRESS,
            requester_owner: runtime_owner,
            module: "GDI32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_10b0),
            iat_address: GET_STOCK_OBJECT_IAT_ADDRESS,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "GetStockObject".to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        });
        let fill_rect_endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
            id: ImportThunkId(234),
            address: FILL_RECT_ENDPOINT_ADDRESS,
            requester_owner: runtime_owner,
            module: "USER32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_140c),
            iat_address: FILL_RECT_IAT_ADDRESS,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "FillRect".to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        });
        let release_dc_endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
            id: ImportThunkId(240),
            address: RELEASE_DC_ENDPOINT_ADDRESS,
            requester_owner: runtime_owner,
            module: "USER32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_1498),
            iat_address: RELEASE_DC_IAT_ADDRESS,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "ReleaseDC".to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        });
        let set_window_long_a_endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
            id: ImportThunkId(212),
            address: SET_WINDOW_LONG_A_ENDPOINT_ADDRESS,
            requester_owner: runtime_owner,
            module: "USER32.dll".to_owned(),
            iat_rva: vine_core::Rva(0x0d_1428),
            iat_address: SET_WINDOW_LONG_A_IAT_ADDRESS,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: "SetWindowLongA".to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        });
        let dispatcher = ApiDispatcher::new(
            [
                icon_endpoint.clone(),
                cursor_endpoint.clone(),
                class_endpoint.clone(),
                create_endpoint.clone(),
                def_window_proc_endpoint.clone(),
                image_endpoint.clone(),
                send_message_endpoint.clone(),
                set_window_pos_endpoint.clone(),
                get_client_rect_endpoint.clone(),
                get_dc_endpoint.clone(),
                get_stock_object_endpoint.clone(),
                fill_rect_endpoint.clone(),
                release_dc_endpoint.clone(),
                set_window_long_a_endpoint.clone(),
            ],
            [
                load_icon_a_contract(icon_endpoint.clone()).expect("icon contract"),
                load_cursor_a_contract(cursor_endpoint.clone()).expect("cursor contract"),
                register_class_ex_a_contract(class_endpoint.clone()).expect("class contract"),
                create_window_ex_a_contract(create_endpoint.clone()).expect("window contract"),
                def_window_proc_a_contract(def_window_proc_endpoint.clone())
                    .expect("default window procedure contract"),
                load_image_a_contract(image_endpoint.clone()).expect("window image contract"),
                send_message_a_contract(send_message_endpoint.clone())
                    .expect("synchronous message contract"),
                set_window_pos_contract(set_window_pos_endpoint.clone())
                    .expect("window position contract"),
                get_client_rect_contract(get_client_rect_endpoint.clone())
                    .expect("client rectangle contract"),
                get_dc_contract(get_dc_endpoint.clone()).expect("GetDC contract"),
                get_stock_object_contract(get_stock_object_endpoint.clone())
                    .expect("GetStockObject contract"),
                fill_rect_contract(fill_rect_endpoint.clone()).expect("FillRect contract"),
                release_dc_contract(release_dc_endpoint.clone()).expect("ReleaseDC contract"),
                set_window_long_a_contract(set_window_long_a_endpoint.clone())
                    .expect("SetWindowLongA contract"),
            ],
        )
        .expect("class dispatcher");
        let policy = User32CompatibilityPolicy::controlled_v14();
        if configure_display {
            runtime
                .configure_display_profile(policy.display_profile().expect("display profile"))
                .expect("configure logical display for window placement");
        }
        Self {
            memory,
            runtime,
            dispatcher,
            icon_endpoint,
            cursor_endpoint,
            class_endpoint,
            create_endpoint,
            def_window_proc_endpoint,
            image_endpoint,
            send_message_endpoint,
            set_window_pos_endpoint,
            get_client_rect_endpoint,
            get_dc_endpoint,
            get_stock_object_endpoint,
            fill_rect_endpoint,
            release_dc_endpoint,
            set_window_long_a_endpoint,
            window_procedure,
            callback_sentinel,
            policy,
        }
    }

    fn prepare_resources(&mut self) {
        let icon_endpoint = self.icon_endpoint.clone();
        let icon =
            self.dispatch_endpoint(icon_endpoint, GuestAddress(STACK_POINTER), RETURN_ADDRESS);
        assert!(matches!(
            icon,
            DispatchOutcome::Immediate(ref completion)
                if completion.eax == RegisterUpdate::Set(0x4901_0001)
        ));
        let cursor_endpoint = self.cursor_endpoint.clone();
        let cursor = self.dispatch_endpoint(
            cursor_endpoint,
            GuestAddress(STACK_POINTER + 0x20),
            CURSOR_RETURN_ADDRESS,
        );
        assert!(matches!(
            cursor,
            DispatchOutcome::Immediate(ref completion)
                if completion.eax == RegisterUpdate::Set(0x4301_0001)
        ));
    }

    fn dispatch_class(&mut self, index: u32) -> DispatchOutcome {
        self.dispatch_endpoint(
            self.class_endpoint.clone(),
            GuestAddress(STACK_POINTER + (index + 2) * 0x20),
            GuestAddress(CLASS_RETURN_ADDRESS.0 + index),
        )
    }

    fn dispatch_create(&mut self) -> DispatchOutcome {
        self.dispatch_endpoint(
            self.create_endpoint.clone(),
            GuestAddress(CREATE_STACK_POINTER),
            CREATE_RETURN_ADDRESS,
        )
    }

    fn prepare_created_window_and_small_icon(&mut self) -> (u32, u32) {
        self.prepare_resources();
        assert!(matches!(
            self.dispatch_class(0),
            DispatchOutcome::Immediate(ref completion)
                if completion.eax == RegisterUpdate::Set(0xc000)
        ));
        let image_endpoint = self.image_endpoint.clone();
        let small_icon = match self.dispatch_endpoint(
            image_endpoint,
            GuestAddress(WINDOW_IMAGE_STACK_POINTER),
            IMAGE_RETURN_ADDRESS,
        ) {
            DispatchOutcome::Immediate(completion) => match completion.eax {
                RegisterUpdate::Set(handle) => handle,
                other => panic!("expected small HICON result, got {other:?}"),
            },
            other => panic!("expected small icon completion, got {other:?}"),
        };
        assert_eq!(small_icon, 0x4901_0002);

        let mut call = match self.dispatch_create() {
            DispatchOutcome::GuestCallback(call) => call,
            other => panic!("expected logical window creation callback, got {other:?}"),
        };
        let window_handle = self.runtime.windows().values().next().unwrap().handle;
        let mut frame_id = ExecutionSession::new(&mut self.memory, &mut self.runtime)
            .begin_guest_callback(&call)
            .expect("enter first creation callback")
            .frame_id;
        self.acknowledge_pending_delta();
        for (index, result) in [1_u32, 0, 0, 0, 0].into_iter().enumerate() {
            let completion_policy = call.request.completion_policy;
            let endpoint = call.frame.endpoint.clone();
            let outcome = {
                let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
                self.dispatcher
                    .complete_guest_callback_result(call.clone(), result, &mut session)
            };
            if index < 4 {
                let next = match outcome {
                    DispatchOutcome::GuestCallback(next) => next,
                    other => panic!("expected replacement creation callback, got {other:?}"),
                };
                let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
                let prepared = session
                    .replace_guest_callback(&next, frame_id)
                    .expect("replace creation callback");
                assert!(matches!(
                    session
                        .commit_guest_callback_semantics(completion_policy, result, &endpoint)
                        .expect("commit creation callback semantics"),
                    Some(RuntimeGuestCallbackSemanticAction::WindowCreation(
                        RuntimeWindowCreationCallbackAction::Continue { .. }
                    ))
                ));
                frame_id = prepared.frame_id;
                call = next;
                self.acknowledge_pending_delta();
            } else {
                assert!(matches!(
                    outcome,
                    DispatchOutcome::Immediate(ref completion)
                        if completion.eax == RegisterUpdate::Set(window_handle)
                ));
                self.runtime
                    .pop_callback_frame(frame_id)
                    .expect("retire final creation callback");
                assert!(matches!(
                    ExecutionSession::new(&mut self.memory, &mut self.runtime)
                        .commit_guest_callback_semantics(completion_policy, result, &endpoint,)
                        .expect("commit final creation callback semantics"),
                    Some(RuntimeGuestCallbackSemanticAction::WindowCreation(
                        RuntimeWindowCreationCallbackAction::Created { .. }
                    ))
                ));
            }
        }
        assert_eq!(
            self.runtime.window(window_handle).unwrap().lifecycle,
            RuntimeWindowLifecycle::Created
        );
        (window_handle, small_icon)
    }

    fn prepare_created_window_with_small_icon(&mut self) -> (u32, u32) {
        let (window_handle, icon_handle) = self.prepare_created_window_and_small_icon();
        let completion = match self.dispatch_def_window_proc(window_handle, 0x0080, icon_handle) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected WM_SETICON default completion, got {other:?}"),
        };
        assert_eq!(completion.eax, RegisterUpdate::Set(0));
        assert_eq!(
            self.runtime
                .window(window_handle)
                .expect("window after WM_SETICON")
                .small_icon
                .map(|icon| icon.handle),
            Some(icon_handle)
        );
        (window_handle, icon_handle)
    }

    fn configure_send_message(
        &mut self,
        window_handle: u32,
        message: u32,
        w_param: u32,
        l_param: u32,
    ) {
        let words = [
            SEND_MESSAGE_RETURN_ADDRESS.0,
            window_handle,
            message,
            w_param,
            l_param,
        ];
        self.memory
            .write(
                GuestAddress(SEND_MESSAGE_STACK_POINTER),
                &words
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>(),
            )
            .expect("write SendMessageA frame");
        self.acknowledge_pending_delta();
    }

    fn dispatch_send_message(&mut self) -> DispatchOutcome {
        self.dispatch_endpoint(
            self.send_message_endpoint.clone(),
            GuestAddress(SEND_MESSAGE_STACK_POINTER),
            SEND_MESSAGE_RETURN_ADDRESS,
        )
    }

    fn configure_set_window_pos(&mut self, arguments: [u32; 7], return_address: GuestAddress) {
        let words = [
            return_address.0,
            arguments[0],
            arguments[1],
            arguments[2],
            arguments[3],
            arguments[4],
            arguments[5],
            arguments[6],
        ];
        self.memory
            .write(
                GuestAddress(SET_WINDOW_POS_STACK_POINTER),
                &words
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>(),
            )
            .expect("write SetWindowPos frame");
        self.acknowledge_pending_delta();
    }

    fn dispatch_set_window_pos(&mut self, return_address: GuestAddress) -> DispatchOutcome {
        self.dispatch_endpoint(
            self.set_window_pos_endpoint.clone(),
            GuestAddress(SET_WINDOW_POS_STACK_POINTER),
            return_address,
        )
    }

    fn configure_get_client_rect(&mut self, window_handle: u32, output: GuestAddress) {
        let words = [GET_CLIENT_RECT_RETURN_ADDRESS.0, window_handle, output.0];
        self.memory
            .write(
                GuestAddress(GET_CLIENT_RECT_STACK_POINTER),
                &words
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>(),
            )
            .expect("write GetClientRect frame");
        self.acknowledge_pending_delta();
    }

    fn dispatch_get_client_rect(&mut self) -> DispatchOutcome {
        self.dispatch_endpoint(
            self.get_client_rect_endpoint.clone(),
            GuestAddress(GET_CLIENT_RECT_STACK_POINTER),
            GET_CLIENT_RECT_RETURN_ADDRESS,
        )
    }

    fn dispatch_get_dc(&mut self, window_handle: u32) -> DispatchOutcome {
        self.write_live_words(
            GuestAddress(GET_DC_STACK_POINTER),
            &[GET_DC_RETURN_ADDRESS.0, window_handle],
        );
        self.acknowledge_pending_delta();
        self.dispatch_endpoint(
            self.get_dc_endpoint.clone(),
            GuestAddress(GET_DC_STACK_POINTER),
            GET_DC_RETURN_ADDRESS,
        )
    }

    fn dispatch_get_stock_object(&mut self, index: i32) -> DispatchOutcome {
        self.write_live_words(
            GuestAddress(GET_STOCK_OBJECT_STACK_POINTER),
            &[GET_STOCK_OBJECT_RETURN_ADDRESS.0, index as u32],
        );
        self.acknowledge_pending_delta();
        self.dispatch_endpoint(
            self.get_stock_object_endpoint.clone(),
            GuestAddress(GET_STOCK_OBJECT_STACK_POINTER),
            GET_STOCK_OBJECT_RETURN_ADDRESS,
        )
    }

    fn dispatch_fill_rect(
        &mut self,
        device_context: u32,
        rect_address: GuestAddress,
        brush: u32,
    ) -> DispatchOutcome {
        self.write_live_words(
            GuestAddress(FILL_RECT_STACK_POINTER),
            &[
                FILL_RECT_RETURN_ADDRESS.0,
                device_context,
                rect_address.0,
                brush,
            ],
        );
        self.acknowledge_pending_delta();
        self.dispatch_endpoint(
            self.fill_rect_endpoint.clone(),
            GuestAddress(FILL_RECT_STACK_POINTER),
            FILL_RECT_RETURN_ADDRESS,
        )
    }

    fn dispatch_release_dc(&mut self, window_handle: u32, device_context: u32) -> DispatchOutcome {
        self.write_live_words(
            GuestAddress(RELEASE_DC_STACK_POINTER),
            &[RELEASE_DC_RETURN_ADDRESS.0, window_handle, device_context],
        );
        self.acknowledge_pending_delta();
        self.dispatch_endpoint(
            self.release_dc_endpoint.clone(),
            GuestAddress(RELEASE_DC_STACK_POINTER),
            RELEASE_DC_RETURN_ADDRESS,
        )
    }

    fn dispatch_set_window_long_a(
        &mut self,
        window_handle: u32,
        index: i32,
        new_value: u32,
    ) -> DispatchOutcome {
        self.write_live_words(
            GuestAddress(SET_WINDOW_LONG_A_STACK_POINTER),
            &[
                SET_WINDOW_LONG_A_RETURN_ADDRESS.0,
                window_handle,
                index as u32,
                new_value,
            ],
        );
        self.acknowledge_pending_delta();
        self.dispatch_endpoint(
            self.set_window_long_a_endpoint.clone(),
            GuestAddress(SET_WINDOW_LONG_A_STACK_POINTER),
            SET_WINDOW_LONG_A_RETURN_ADDRESS,
        )
    }

    fn write_live_words(&mut self, address: GuestAddress, words: &[u32]) {
        self.memory
            .write(
                address,
                &words
                    .iter()
                    .copied()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>(),
            )
            .expect("write live GDI call frame");
    }

    fn dispatch_def_window_proc(
        &mut self,
        handle: u32,
        message: u32,
        l_param: u32,
    ) -> DispatchOutcome {
        let words = [
            DEF_WINDOW_PROC_RETURN_ADDRESS.0,
            handle,
            message,
            0,
            l_param,
        ];
        let bytes = words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        self.memory
            .write(GuestAddress(DEF_WINDOW_PROC_STACK_POINTER), &bytes)
            .expect("write nested DefWindowProcA frame");
        self.acknowledge_pending_delta();
        self.dispatch_endpoint(
            self.def_window_proc_endpoint.clone(),
            GuestAddress(DEF_WINDOW_PROC_STACK_POINTER),
            DEF_WINDOW_PROC_RETURN_ADDRESS,
        )
    }

    fn acknowledge_pending_delta(&mut self) {
        if self.memory.stats().pending_invalidation.is_none() {
            return;
        }
        let delta = self.memory.pending_delta().expect("pending User32 delta");
        self.memory
            .acknowledge_delta(delta.generation)
            .expect("acknowledge User32 delta");
    }

    fn dispatch_endpoint(
        &mut self,
        endpoint: ApiEndpointIdentity,
        stack_pointer: GuestAddress,
        return_address: GuestAddress,
    ) -> DispatchOutcome {
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        self.dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: endpoint.id,
                endpoint_address: endpoint.address,
                stack_pointer,
                return_address,
            },
            &mut session,
            |contract, frame, session| {
                if contract.endpoint.module.eq_ignore_ascii_case("gdi32.dll") {
                    dispatch_gdi32_api(contract, frame, session)
                } else {
                    dispatch_user32_api_with_policy(
                        &self.policy,
                        self.callback_sentinel,
                        contract,
                        frame,
                        session,
                    )
                }
            },
        )
    }
}

fn assert_send_message_fault_is_atomic(
    fixture: &mut RegisterClassFixture,
    expected_class: ApiFaultClass,
    expected_code: &str,
) {
    let runtime_before = fixture.runtime.clone();
    let memory_before = fixture.memory.stats();
    let memory_hash_before = fixture
        .memory
        .current_reflection()
        .expect("current SendMessageA reflection")
        .content_sha256;
    let fault = match fixture.dispatch_send_message() {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected SendMessageA fault, got {other:?}"),
    };
    assert_eq!(fault.class, expected_class);
    assert_eq!(fault.code, expected_code);
    assert_eq!(fixture.runtime, runtime_before);
    assert_eq!(fixture.memory.stats(), memory_before);
    assert_eq!(
        fixture
            .memory
            .current_reflection()
            .expect("unchanged SendMessageA reflection")
            .content_sha256,
        memory_hash_before
    );
}

fn assert_set_window_pos_fault_is_atomic(
    fixture: &mut RegisterClassFixture,
    return_address: GuestAddress,
    expected_class: ApiFaultClass,
    expected_code: &str,
) {
    let runtime_before = fixture.runtime.clone();
    let memory_before = fixture.memory.stats();
    let memory_hash_before = fixture
        .memory
        .current_reflection()
        .expect("current SetWindowPos reflection")
        .content_sha256;
    let fault = match fixture.dispatch_set_window_pos(return_address) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected SetWindowPos fault, got {other:?}"),
    };
    assert_eq!(fault.class, expected_class);
    assert_eq!(fault.code, expected_code);
    assert_eq!(fixture.runtime, runtime_before);
    assert_eq!(fixture.memory.stats(), memory_before);
    assert_eq!(
        fixture
            .memory
            .current_reflection()
            .expect("unchanged SetWindowPos reflection")
            .content_sha256,
        memory_hash_before
    );
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
fn load_image_a_contract_is_six_argument_stdcall_with_handler_last_error() {
    let (_, owner) = initialized_memory();
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(193),
        address: IMAGE_ENDPOINT_ADDRESS,
        requester_owner: owner,
        module: "USER32.dll".to_owned(),
        iat_rva: vine_core::Rva(0x0d_13dc),
        iat_address: IMAGE_IAT_ADDRESS,
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "LoadImageA".to_owned(),
            hint: 0,
            name_rva: vine_core::Rva(0),
        },
    });
    let contract = load_image_a_contract(endpoint).expect("LoadImageA contract");
    assert_eq!(contract.abi.calling_convention, CallingConvention::Stdcall);
    assert_eq!(contract.abi.arguments.len(), 6);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    assert_eq!(contract.reentrancy_policy, ReentrancyPolicy::Allowed);
    assert_eq!(contract.callback_policy, CallbackPolicy::Forbidden);
}

#[test]
fn send_message_a_contract_is_four_argument_stdcall_callback_with_lresult() {
    let (_, owner) = initialized_memory();
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(229),
        address: SEND_MESSAGE_ENDPOINT_ADDRESS,
        requester_owner: owner,
        module: "USER32.dll".to_owned(),
        iat_rva: vine_core::Rva(0x0d_146c),
        iat_address: SEND_MESSAGE_IAT_ADDRESS,
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "SendMessageA".to_owned(),
            hint: 0,
            name_rva: vine_core::Rva(0),
        },
    });
    let contract = send_message_a_contract(endpoint).expect("SendMessageA contract");
    assert_eq!(contract.abi.calling_convention, CallingConvention::Stdcall);
    assert_eq!(contract.abi.arguments.len(), 4);
    assert!(matches!(
        contract.abi.arguments[3].value,
        ApiValueSchema::ScalarOrPointer { .. }
    ));
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::Preserve);
    assert_eq!(contract.reentrancy_policy, ReentrancyPolicy::Allowed);
    assert_eq!(contract.callback_policy, CallbackPolicy::Required);
}

#[test]
fn set_window_pos_contract_is_seven_argument_stdcall_callback_with_bool_result() {
    let (_, owner) = initialized_memory();
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(202),
        address: SET_WINDOW_POS_ENDPOINT_ADDRESS,
        requester_owner: owner,
        module: "USER32.dll".to_owned(),
        iat_rva: vine_core::Rva(0x0d_1400),
        iat_address: SET_WINDOW_POS_IAT_ADDRESS,
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "SetWindowPos".to_owned(),
            hint: 0,
            name_rva: vine_core::Rva(0),
        },
    });
    let contract = set_window_pos_contract(endpoint).expect("SetWindowPos contract");
    assert_eq!(contract.abi.calling_convention, CallingConvention::Stdcall);
    assert_eq!(contract.abi.arguments.len(), 7);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::Bool32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::Preserve);
    assert_eq!(contract.reentrancy_policy, ReentrancyPolicy::Allowed);
    assert_eq!(contract.callback_policy, CallbackPolicy::Required);
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
fn register_class_ex_a_contract_is_one_struct_stdcall_with_u16_result() {
    let (_, owner) = initialized_memory();
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(191),
        address: CLASS_ENDPOINT_ADDRESS,
        requester_owner: owner,
        module: "USER32.dll".to_owned(),
        iat_rva: vine_core::Rva(0x0d_13d4),
        iat_address: CLASS_IAT_ADDRESS,
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "RegisterClassExA".to_owned(),
            hint: 0,
            name_rva: vine_core::Rva(0),
        },
    });
    let contract = register_class_ex_a_contract(endpoint).expect("RegisterClassExA contract");
    assert_eq!(contract.abi.calling_convention, CallingConvention::Stdcall);
    assert_eq!(contract.abi.arguments.len(), 1);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U16);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    assert_eq!(contract.reentrancy_policy, ReentrancyPolicy::Allowed);
    assert_eq!(contract.callback_policy, CallbackPolicy::Forbidden);
}

#[test]
fn get_system_metrics_contract_is_one_index_stdcall_and_preserves_last_error() {
    let (_, owner) = initialized_memory();
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(203),
        address: METRICS_ENDPOINT_ADDRESS,
        requester_owner: owner,
        module: "USER32.dll".to_owned(),
        iat_rva: vine_core::Rva(0x0d_1404),
        iat_address: IAT_ADDRESS,
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "GetSystemMetrics".to_owned(),
            hint: 0,
            name_rva: vine_core::Rva(0),
        },
    });
    let contract = get_system_metrics_contract(endpoint).expect("GetSystemMetrics contract");
    assert_eq!(contract.abi.calling_convention, CallingConvention::Stdcall);
    assert_eq!(contract.abi.arguments.len(), 1);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::Preserve);
    assert_eq!(contract.reentrancy_policy, ReentrancyPolicy::Allowed);
    assert_eq!(contract.callback_policy, CallbackPolicy::Forbidden);
}

#[test]
fn get_client_rect_contract_is_two_argument_stdcall_with_conditional_rect_output() {
    let (_, owner) = initialized_memory();
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(199),
        address: GET_CLIENT_RECT_ENDPOINT_ADDRESS,
        requester_owner: owner,
        module: "USER32.dll".to_owned(),
        iat_rva: vine_core::Rva(0x0d_13f4),
        iat_address: GET_CLIENT_RECT_IAT_ADDRESS,
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "GetClientRect".to_owned(),
            hint: 0,
            name_rva: vine_core::Rva(0),
        },
    });
    let contract = get_client_rect_contract(endpoint).expect("GetClientRect contract");
    assert_eq!(contract.abi.calling_convention, CallingConvention::Stdcall);
    assert_eq!(contract.abi.arguments.len(), 2);
    assert!(matches!(
        &contract.abi.arguments[0].value,
        ApiValueSchema::LogicalHandle { namespace, nullable }
            if namespace == "vine.logical-window.v1" && *nullable
    ));
    assert_eq!(contract.abi.arguments[1].direction, ApiDirection::Out);
    assert!(matches!(
        &contract.abi.arguments[1].value,
        ApiValueSchema::Pointer { constraint }
            if !constraint.nullable
                && constraint.alignment == 4
                && constraint.byte_length == 16
                && constraint.write_requirement == ApiWriteRequirement::Optional
                && matches!(
                    &constraint.pointee,
                    ApiPointeeSchema::Struct { type_id } if type_id == "RECT"
                )
    ));
    assert_eq!(contract.abi.return_value, ApiReturnSchema::Bool32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    assert_eq!(contract.reentrancy_policy, ReentrancyPolicy::Allowed);
    assert_eq!(contract.callback_policy, CallbackPolicy::Forbidden);
}

#[test]
fn window_dc_slice_contracts_keep_user32_and_gdi32_frontends_separate() {
    let fixture = RegisterClassFixture::new();
    let get_dc = user32_contract_for_endpoint(fixture.get_dc_endpoint.clone())
        .expect("GetDC catalog lookup")
        .expect("GetDC contract");
    assert_eq!(get_dc.abi.arguments.len(), 1);
    assert!(matches!(
        &get_dc.abi.return_value,
        ApiReturnSchema::LogicalHandle { namespace, nullable }
            if namespace == "vine.logical-gdi-dc.v1" && *nullable
    ));

    let fill_rect = user32_contract_for_endpoint(fixture.fill_rect_endpoint.clone())
        .expect("FillRect catalog lookup")
        .expect("FillRect contract");
    assert_eq!(fill_rect.abi.arguments.len(), 3);
    assert!(matches!(
        &fill_rect.abi.arguments[0].value,
        ApiValueSchema::LogicalHandle { namespace, nullable }
            if namespace == "vine.logical-gdi-dc.v1" && !*nullable
    ));
    assert!(matches!(
        &fill_rect.abi.arguments[2].value,
        ApiValueSchema::LogicalHandle { namespace, nullable }
            if namespace == "vine.logical-gdi-object.v1" && !*nullable
    ));

    let release_dc = user32_contract_for_endpoint(fixture.release_dc_endpoint.clone())
        .expect("ReleaseDC catalog lookup")
        .expect("ReleaseDC contract");
    assert_eq!(release_dc.abi.arguments.len(), 2);
    assert_eq!(release_dc.abi.return_value, ApiReturnSchema::Bool32);

    assert!(
        user32_contract_for_endpoint(fixture.get_stock_object_endpoint.clone())
            .expect("User32 rejects GDI32 endpoint")
            .is_none()
    );
    let stock_object = gdi32_contract_for_endpoint(fixture.get_stock_object_endpoint.clone())
        .expect("GDI32 catalog lookup")
        .expect("GetStockObject contract");
    assert_eq!(stock_object.abi.arguments.len(), 1);
    assert!(matches!(
        &stock_object.abi.return_value,
        ApiReturnSchema::LogicalHandle { namespace, nullable }
            if namespace == "vine.logical-gdi-object.v1" && *nullable
    ));
    assert!(
        gdi32_contract_for_endpoint(fixture.get_dc_endpoint.clone())
            .expect("GDI32 rejects User32 endpoint")
            .is_none()
    );
}

#[test]
fn set_window_long_a_contract_is_immediate_three_word_stdcall_long() {
    let fixture = RegisterClassFixture::new();
    let contract = user32_contract_for_endpoint(fixture.set_window_long_a_endpoint.clone())
        .expect("SetWindowLongA catalog lookup")
        .expect("SetWindowLongA contract");
    assert_eq!(contract.abi.calling_convention, CallingConvention::Stdcall);
    assert_eq!(contract.abi.arguments.len(), 3);
    assert!(matches!(
        &contract.abi.arguments[0].value,
        ApiValueSchema::LogicalHandle { namespace, nullable }
            if namespace == "vine.logical-window.v1" && !*nullable
    ));
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    assert_eq!(contract.reentrancy_policy, ReentrancyPolicy::Allowed);
    assert_eq!(contract.callback_policy, CallbackPolicy::Forbidden);
}

#[test]
fn invalid_menu_chain_contracts_are_cataloged_immediate_stdcall_leaves() {
    let fixture = InvalidMenuFixture::new();
    let get_system_menu = user32_contract_for_endpoint(fixture.get_system_menu_endpoint.clone())
        .expect("GetSystemMenu catalog lookup")
        .expect("GetSystemMenu implemented contract");
    assert_eq!(
        get_system_menu.abi.calling_convention,
        CallingConvention::Stdcall
    );
    assert_eq!(get_system_menu.abi.arguments.len(), 2);
    assert_eq!(get_system_menu.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(get_system_menu.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(
        get_system_menu.last_error_policy,
        LastErrorPolicy::FromHandler
    );
    assert_eq!(get_system_menu.reentrancy_policy, ReentrancyPolicy::Allowed);
    assert_eq!(get_system_menu.callback_policy, CallbackPolicy::Forbidden);

    let enable_menu_item = user32_contract_for_endpoint(fixture.enable_menu_item_endpoint.clone())
        .expect("EnableMenuItem catalog lookup")
        .expect("EnableMenuItem implemented contract");
    assert_eq!(
        enable_menu_item.abi.calling_convention,
        CallingConvention::Stdcall
    );
    assert_eq!(enable_menu_item.abi.arguments.len(), 3);
    assert_eq!(enable_menu_item.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(enable_menu_item.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(
        enable_menu_item.last_error_policy,
        LastErrorPolicy::FromHandler
    );
    assert_eq!(
        enable_menu_item.reentrancy_policy,
        ReentrancyPolicy::Allowed
    );
    assert_eq!(enable_menu_item.callback_policy, CallbackPolicy::Forbidden);
}

#[test]
fn def_window_proc_a_contract_keeps_message_dependent_lparam_as_scalar() {
    let (_, owner) = initialized_memory();
    let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(233),
        address: DEF_WINDOW_PROC_ENDPOINT_ADDRESS,
        requester_owner: owner,
        module: "USER32.dll".to_owned(),
        iat_rva: vine_core::Rva(0x0d_147c),
        iat_address: DEF_WINDOW_PROC_IAT_ADDRESS,
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "DefWindowProcA".to_owned(),
            hint: 0,
            name_rva: vine_core::Rva(0),
        },
    });
    let contract = def_window_proc_a_contract(endpoint).expect("DefWindowProcA contract");
    assert_eq!(contract.abi.calling_convention, CallingConvention::Stdcall);
    assert_eq!(contract.abi.arguments.len(), 4);
    let ApiValueSchema::ScalarOrPointer { constraint } = &contract.abi.arguments[3].value else {
        panic!("DefWindowProcA lParam must be a scalar-or-pointer union");
    };
    assert_eq!(constraint.alignment, 4);
    assert_eq!(constraint.minimum_byte_length, 16);
    assert_eq!(constraint.maximum_byte_length, 48);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::Preserve);
    assert_eq!(contract.callback_policy, CallbackPolicy::Optional);
}

#[test]
fn invalid_menu_chain_matches_native_failures_without_allocating_menu_identity() {
    let mut fixture = InvalidMenuFixture::new();
    let generation = fixture.memory.stats().generation;

    let get_system_menu = match fixture.dispatch_get_system_menu(0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected GetSystemMenu completion, got {other:?}"),
    };
    assert_eq!(get_system_menu.eax, RegisterUpdate::Set(0));
    assert_eq!(
        get_system_menu.stack_pointer,
        GuestAddress(GET_SYSTEM_MENU_STACK_POINTER + 3 * 4)
    );
    assert_eq!(
        get_system_menu.instruction_pointer,
        GET_SYSTEM_MENU_RETURN_ADDRESS
    );
    assert_eq!(get_system_menu.last_error, 1400);
    assert!(get_system_menu.guest_writes.is_empty());
    assert_eq!(
        fixture
            .runtime
            .current_last_error()
            .expect("GetSystemMenu LastError"),
        1400
    );

    let enable_menu_item = match fixture.dispatch_enable_menu_item(0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected EnableMenuItem completion, got {other:?}"),
    };
    assert_eq!(enable_menu_item.eax, RegisterUpdate::Set(u32::MAX));
    assert_eq!(
        enable_menu_item.stack_pointer,
        GuestAddress(ENABLE_MENU_ITEM_STACK_POINTER + 4 * 4)
    );
    assert_eq!(
        enable_menu_item.instruction_pointer,
        ENABLE_MENU_ITEM_RETURN_ADDRESS
    );
    assert_eq!(enable_menu_item.last_error, 1401);
    assert!(enable_menu_item.guest_writes.is_empty());
    assert_eq!(
        fixture
            .runtime
            .current_last_error()
            .expect("EnableMenuItem LastError"),
        1401
    );
    assert_eq!(fixture.memory.stats().generation, generation);
}

#[test]
fn invalid_menu_chain_rejects_unreached_variants_before_mutation() {
    for index in 1..=2 {
        let mut fixture = InvalidMenuFixture::new();
        let generation = fixture.memory.stats().generation;
        let last_error = fixture
            .runtime
            .current_last_error()
            .expect("entry LastError");
        let fault = match fixture.dispatch_get_system_menu(index) {
            DispatchOutcome::Fault(fault) => fault,
            other => panic!("expected GetSystemMenu variant fault, got {other:?}"),
        };
        assert_eq!(fault.class, ApiFaultClass::Unsupported);
        assert_eq!(fault.code, "get-system-menu-variant-unsupported");
        assert_eq!(fixture.memory.stats().generation, generation);
        assert_eq!(
            fixture
                .runtime
                .current_last_error()
                .expect("unchanged LastError"),
            last_error
        );
    }

    for index in 1..=3 {
        let mut fixture = InvalidMenuFixture::new();
        let generation = fixture.memory.stats().generation;
        let last_error = fixture
            .runtime
            .current_last_error()
            .expect("entry LastError");
        let fault = match fixture.dispatch_enable_menu_item(index) {
            DispatchOutcome::Fault(fault) => fault,
            other => panic!("expected EnableMenuItem variant fault, got {other:?}"),
        };
        assert_eq!(fault.class, ApiFaultClass::Unsupported);
        assert_eq!(fault.code, "enable-menu-item-variant-unsupported");
        assert_eq!(fixture.memory.stats().generation, generation);
        assert_eq!(
            fixture
                .runtime
                .current_last_error()
                .expect("unchanged LastError"),
            last_error
        );
    }

    for enable_menu_item in [false, true] {
        let mut fixture = InvalidMenuFixture::new();
        fixture.policy.invalid_menu_chain = None;
        let generation = fixture.memory.stats().generation;
        let last_error = fixture
            .runtime
            .current_last_error()
            .expect("entry LastError");
        let fault = match if enable_menu_item {
            fixture.dispatch_enable_menu_item(0)
        } else {
            fixture.dispatch_get_system_menu(0)
        } {
            DispatchOutcome::Fault(fault) => fault,
            other => panic!("expected disabled menu-policy fault, got {other:?}"),
        };
        assert_eq!(fault.class, ApiFaultClass::Unsupported);
        assert_eq!(
            fault.code,
            if enable_menu_item {
                "enable-menu-item-policy-disabled"
            } else {
                "get-system-menu-policy-disabled"
            }
        );
        assert_eq!(fixture.memory.stats().generation, generation);
        assert_eq!(
            fixture
                .runtime
                .current_last_error()
                .expect("unchanged LastError"),
            last_error
        );
    }
}

#[test]
fn get_system_metrics_reads_only_the_controlled_display_profile() {
    let mut fixture = MetricsFixture::new(true);
    let generation = fixture.memory.stats().generation;
    let expected = [3_u32, 3, 23, 1920, 1080];
    for (index, expected_value) in expected.into_iter().enumerate() {
        let sentinel = 0x1122_3300 + index as u32;
        fixture
            .runtime
            .set_current_last_error(sentinel)
            .expect("seed metrics LastError");
        let completion = match fixture.dispatch(index as u32) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected GetSystemMetrics completion {index}, got {other:?}"),
        };
        assert_eq!(completion.eax, RegisterUpdate::Set(expected_value));
        assert_eq!(
            completion.stack_pointer,
            GuestAddress(STACK_POINTER + index as u32 * 0x20 + 8)
        );
        assert_eq!(
            completion.instruction_pointer,
            GuestAddress(METRICS_RETURN_ADDRESS.0 + index as u32)
        );
        assert_eq!(completion.last_error, sentinel);
        assert!(completion.guest_writes.is_empty());
        assert_eq!(fixture.memory.stats().generation, generation);
    }
    assert_eq!(
        fixture.runtime.display_profile_id(),
        Some("single-monitor-1920x1080-v1")
    );
    assert_eq!(
        fixture.runtime.primary_monitor().unwrap().rect,
        RuntimeRect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        }
    );

    let sentinel = 0x5566_7788;
    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed small-icon-height LastError");
    let completion = match fixture.dispatch(5) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected small-icon-height completion, got {other:?}"),
    };
    assert_eq!(completion.eax, RegisterUpdate::Set(16));
    assert_eq!(completion.last_error, sentinel);
    assert!(completion.guest_writes.is_empty());
    assert_eq!(fixture.memory.stats().generation, generation);

    let sentinel = 0x6677_8899;
    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed small-icon-width LastError");
    let completion = match fixture.dispatch(6) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected small-icon-width completion, got {other:?}"),
    };
    assert_eq!(completion.eax, RegisterUpdate::Set(16));
    assert_eq!(completion.last_error, sentinel);
    assert!(completion.guest_writes.is_empty());
    assert_eq!(fixture.memory.stats().generation, generation);
}

#[test]
fn get_client_rect_reads_live_window_geometry_and_preserves_failure_atomicity() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, _) = fixture.prepare_created_window_and_small_icon();
    let expected_bytes = [0_i32, 0, 800, 600]
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect::<Vec<_>>();
    fixture
        .memory
        .write(CLIENT_RECT_OUTPUT_ADDRESS, &[0xa5; 16])
        .expect("seed client RECT output");
    fixture.acknowledge_pending_delta();
    fixture.configure_get_client_rect(window_handle, CLIENT_RECT_OUTPUT_ADDRESS);
    let windows_before = fixture.runtime.windows().clone();
    let sentinel = 0x1122_3344;
    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed GetClientRect LastError");
    let completion = match fixture.dispatch_get_client_rect() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected GetClientRect completion, got {other:?}"),
    };
    assert_eq!(completion.eax, RegisterUpdate::Set(1));
    assert_eq!(completion.last_error, sentinel);
    assert_eq!(completion.guest_writes.len(), 1);
    assert_eq!(
        completion.guest_writes[0].write.address,
        CLIENT_RECT_OUTPUT_ADDRESS
    );
    assert_eq!(completion.guest_writes[0].write.bytes, expected_bytes);
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture
            .memory
            .read(CLIENT_RECT_OUTPUT_ADDRESS, 16)
            .expect("read completed client RECT"),
        expected_bytes
    );
    assert_eq!(fixture.runtime.windows(), &windows_before);

    fixture
        .memory
        .write(CLIENT_RECT_OUTPUT_ADDRESS, &[0xa5; 16])
        .expect("reset failed client RECT output");
    fixture.acknowledge_pending_delta();
    fixture.configure_get_client_rect(0, CLIENT_RECT_OUTPUT_ADDRESS);
    let memory_before_invalid_handle = fixture.memory.stats();
    let windows_before_invalid_handle = fixture.runtime.windows().clone();
    let invalid_handle = match fixture.dispatch_get_client_rect() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid-HWND completion, got {other:?}"),
    };
    assert_eq!(invalid_handle.eax, RegisterUpdate::Set(0));
    assert_eq!(invalid_handle.last_error, 1400);
    assert!(invalid_handle.guest_writes.is_empty());
    assert_eq!(fixture.memory.stats(), memory_before_invalid_handle);
    assert_eq!(
        fixture
            .memory
            .read(CLIENT_RECT_OUTPUT_ADDRESS, 16)
            .expect("read preserved invalid-HWND output"),
        [0xa5; 16]
    );
    assert_eq!(fixture.runtime.windows(), &windows_before_invalid_handle);

    let unmapped_output = GuestAddress(MEMORY_BASE.0 + 0x3000);
    fixture.configure_get_client_rect(window_handle, unmapped_output);
    let pointer_fault_runtime = fixture.runtime.clone();
    let pointer_fault_memory = fixture.memory.stats();
    let pointer_fault = match fixture.dispatch_get_client_rect() {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected client-RECT pointer fault, got {other:?}"),
    };
    assert_eq!(pointer_fault.class, ApiFaultClass::InvalidCall);
    assert_eq!(pointer_fault.code, "guest-page-fault");
    assert_eq!(fixture.runtime, pointer_fault_runtime);
    assert_eq!(fixture.memory.stats(), pointer_fault_memory);
}

#[test]
fn window_dc_fill_slice_preserves_identity_lifetime_and_failure_atomicity() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, _) = fixture.prepare_created_window_and_small_icon();
    let sentinel = 0x1122_3344;

    let windows_before_invalid_window = fixture.runtime.windows().clone();
    let contexts_before_invalid_window = fixture.runtime.gdi_device_contexts().clone();
    let surfaces_before_invalid_window = fixture.runtime.gdi_surfaces().clone();
    let invalid_window = match fixture.dispatch_get_dc(0x4801_ffff) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid-window GetDC completion, got {other:?}"),
    };
    assert_eq!(invalid_window.eax, RegisterUpdate::Set(0));
    assert_eq!(invalid_window.last_error, 1400);
    assert_eq!(fixture.runtime.windows(), &windows_before_invalid_window);
    assert_eq!(
        fixture.runtime.gdi_device_contexts(),
        &contexts_before_invalid_window
    );
    assert_eq!(
        fixture.runtime.gdi_surfaces(),
        &surfaces_before_invalid_window
    );

    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed GetDC LastError");
    let first_dc = match fixture.dispatch_get_dc(window_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected first GetDC completion, got {other:?}"),
    };
    let first_dc_handle = match first_dc.eax {
        RegisterUpdate::Set(handle) => handle,
        other => panic!("GetDC did not set EAX: {other:?}"),
    };
    assert_eq!(first_dc_handle, 0x4701_0001);
    assert_eq!(first_dc.last_error, sentinel);
    assert!(first_dc.guest_writes.is_empty());

    let second_dc = match fixture.dispatch_get_dc(window_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected second GetDC completion, got {other:?}"),
    };
    let second_dc_handle = match second_dc.eax {
        RegisterUpdate::Set(handle) => handle,
        other => panic!("second GetDC did not set EAX: {other:?}"),
    };
    assert_eq!(second_dc_handle, 0x4701_0002);
    assert_ne!(first_dc_handle, second_dc_handle);
    assert_eq!(
        fixture
            .runtime
            .gdi_device_context(first_dc_handle)
            .unwrap()
            .surface_id,
        fixture
            .runtime
            .gdi_device_context(second_dc_handle)
            .unwrap()
            .surface_id
    );

    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed GetStockObject LastError");
    let objects_before_invalid = fixture.runtime.gdi_objects().clone();
    let invalid_object = match fixture.dispatch_get_stock_object(-1) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid stock-object completion, got {other:?}"),
    };
    assert_eq!(invalid_object.eax, RegisterUpdate::Set(0));
    assert_eq!(invalid_object.last_error, sentinel);
    assert_eq!(fixture.runtime.gdi_objects(), &objects_before_invalid);

    let black_brush = match fixture.dispatch_get_stock_object(4) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected BLACK_BRUSH completion, got {other:?}"),
    };
    let black_brush_handle = match black_brush.eax {
        RegisterUpdate::Set(handle) => handle,
        other => panic!("GetStockObject did not set EAX: {other:?}"),
    };
    assert_eq!(black_brush_handle, 0x4702_0001);
    assert_eq!(black_brush.last_error, sentinel);
    let repeated_brush = match fixture.dispatch_get_stock_object(4) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected repeated BLACK_BRUSH completion, got {other:?}"),
    };
    assert_eq!(repeated_brush.eax, RegisterUpdate::Set(black_brush_handle));
    assert_eq!(fixture.runtime.gdi_objects().len(), 1);

    let rect_bytes = [0_i32, 0, 800, 600]
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect::<Vec<_>>();
    fixture
        .memory
        .write(CLIENT_RECT_OUTPUT_ADDRESS, &rect_bytes)
        .expect("write FillRect input");
    fixture.acknowledge_pending_delta();
    let surfaces_before_invalid_hdc = fixture.runtime.gdi_surfaces().clone();
    let invalid_hdc = match fixture.dispatch_fill_rect(
        0x4701_ffff,
        CLIENT_RECT_OUTPUT_ADDRESS,
        black_brush_handle,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid-HDC FillRect completion, got {other:?}"),
    };
    assert_eq!(invalid_hdc.eax, RegisterUpdate::Set(0));
    assert_eq!(invalid_hdc.last_error, 6);
    assert_eq!(fixture.runtime.gdi_surfaces(), &surfaces_before_invalid_hdc);

    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed FillRect LastError");
    let fill = match fixture.dispatch_fill_rect(
        first_dc_handle,
        CLIENT_RECT_OUTPUT_ADDRESS,
        black_brush_handle,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected FillRect completion, got {other:?}"),
    };
    assert_eq!(fill.eax, RegisterUpdate::Set(1));
    assert_eq!(fill.last_error, sentinel);
    assert!(fill.guest_writes.is_empty());
    let surface = fixture.runtime.gdi_surfaces().values().next().unwrap();
    assert_eq!((surface.width, surface.height), (800, 600));
    assert_eq!(surface.fills.len(), 1);
    assert_eq!(
        surface.fills[0].rect,
        RuntimeRect {
            left: 0,
            top: 0,
            right: 800,
            bottom: 600,
        }
    );
    assert_eq!(surface.fills[0].color_ref, 0);

    let runtime_before_pointer_fault = fixture.runtime.clone();
    let pointer_fault = match fixture.dispatch_fill_rect(
        first_dc_handle,
        GuestAddress(MEMORY_BASE.0 + 0x3000),
        black_brush_handle,
    ) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected FillRect pointer fault, got {other:?}"),
    };
    assert_eq!(pointer_fault.class, ApiFaultClass::InvalidCall);
    assert_eq!(pointer_fault.code, "guest-page-fault");
    assert_eq!(fixture.runtime, runtime_before_pointer_fault);

    let contexts_before_invalid_release = fixture.runtime.gdi_device_contexts().clone();
    let invalid_release = match fixture.dispatch_release_dc(window_handle, 0x4701_ffff) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid-HDC ReleaseDC completion, got {other:?}"),
    };
    assert_eq!(invalid_release.eax, RegisterUpdate::Set(0));
    assert_eq!(invalid_release.last_error, 1425);
    assert_eq!(
        fixture.runtime.gdi_device_contexts(),
        &contexts_before_invalid_release
    );

    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed ReleaseDC LastError");
    let released = match fixture.dispatch_release_dc(window_handle, first_dc_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected ReleaseDC completion, got {other:?}"),
    };
    assert_eq!(released.eax, RegisterUpdate::Set(1));
    assert_eq!(released.last_error, sentinel);
    assert!(fixture.runtime.gdi_device_context(first_dc_handle).is_err());
    assert!(fixture.runtime.gdi_device_context(second_dc_handle).is_ok());

    let double_release = match fixture.dispatch_release_dc(window_handle, first_dc_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected double ReleaseDC completion, got {other:?}"),
    };
    assert_eq!(double_release.eax, RegisterUpdate::Set(0));
    assert_eq!(double_release.last_error, 1425);

    fixture
        .runtime
        .terminate_current_process(0)
        .expect("terminate process with a live window DC");
    assert!(fixture.runtime.gdi_device_contexts().is_empty());
    assert!(fixture.runtime.gdi_objects().is_empty());
    assert!(fixture.runtime.gdi_surfaces().is_empty());
    assert!(fixture.runtime.windows().is_empty());
    assert_eq!(
        fixture
            .runtime
            .gdi_device_context(second_dc_handle)
            .expect_err("process teardown retires live HDC")
            .code,
        "device-context-handle-unknown"
    );
}

#[test]
fn set_window_long_a_updates_effective_style_visibility_and_preserves_failures() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, _) = fixture.prepare_created_window_and_small_icon();
    let initial = fixture.runtime.window(window_handle).unwrap().clone();
    assert_eq!(initial.creation.style, 0x80ca_0000);
    assert_eq!(initial.creation.effective_style, 0x84ca_0000);
    assert!(!initial.visible);

    let before_invalid_window = fixture.runtime.windows().clone();
    let invalid_window = match fixture.dispatch_set_window_long_a(0x4801_ffff, -16, 0x90ca_0000) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid-window completion, got {other:?}"),
    };
    assert_eq!(invalid_window.eax, RegisterUpdate::Set(0));
    assert_eq!(invalid_window.last_error, 1400);
    assert_eq!(fixture.runtime.windows(), &before_invalid_window);

    let before_invalid_index = fixture.runtime.windows().clone();
    let invalid_index = match fixture.dispatch_set_window_long_a(window_handle, -100, 0x90ca_0000) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid-index completion, got {other:?}"),
    };
    assert_eq!(invalid_index.eax, RegisterUpdate::Set(0));
    assert_eq!(invalid_index.last_error, 1413);
    assert_eq!(fixture.runtime.windows(), &before_invalid_index);

    let before_unsupported = fixture.runtime.clone();
    let unsupported = match fixture.dispatch_set_window_long_a(window_handle, -16, 0x80ca_0000) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected unsupported-style fault, got {other:?}"),
    };
    assert_eq!(unsupported.class, ApiFaultClass::Unsupported);
    assert_eq!(unsupported.code, "set-window-long-a-variant-unsupported");
    assert_eq!(fixture.runtime, before_unsupported);

    let sentinel = 0x1122_3344;
    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed SetWindowLongA LastError");
    let completion = match fixture.dispatch_set_window_long_a(window_handle, -16, 0x90ca_0000) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected SetWindowLongA completion, got {other:?}"),
    };
    assert_eq!(completion.eax, RegisterUpdate::Set(0x84ca_0000));
    assert_eq!(completion.last_error, sentinel);
    assert!(completion.guest_writes.is_empty());
    assert_eq!(
        completion.stack_pointer.0,
        SET_WINDOW_LONG_A_STACK_POINTER + 16
    );
    let updated = fixture.runtime.window(window_handle).unwrap();
    assert_eq!(updated.creation.style, 0x80ca_0000);
    assert_eq!(updated.creation.effective_style, 0x94ca_0000);
    assert!(updated.visible);

    let before_repeated = fixture.runtime.clone();
    let repeated = match fixture.dispatch_set_window_long_a(window_handle, -16, 0x90ca_0000) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected repeated SetWindowLongA completion, got {other:?}"),
    };
    assert_eq!(repeated.eax, RegisterUpdate::Set(0x94ca_0000));
    assert_eq!(repeated.last_error, sentinel);
    assert_eq!(fixture.runtime, before_repeated);
}

#[test]
fn get_system_metrics_rejects_unreached_or_inconsistent_profiles_without_mutation() {
    let mut unsupported = MetricsFixture::new(true);
    let generation = unsupported.memory.stats().generation;
    let fault = match unsupported.dispatch(7) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected unsupported system metric fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "get-system-metrics-index-unsupported");
    assert_eq!(unsupported.memory.stats().generation, generation);

    let mut small_icon_disabled = MetricsFixture::new(true);
    small_icon_disabled
        .policy
        .get_system_metrics
        .as_mut()
        .expect("metrics policy")
        .small_icon_height = None;
    small_icon_disabled.policy.load_image_a = None;
    small_icon_disabled.policy.send_message_a = None;
    small_icon_disabled.policy.set_window_pos = None;
    let generation = small_icon_disabled.memory.stats().generation;
    let last_error = small_icon_disabled
        .runtime
        .current_last_error()
        .expect("entry LastError");
    let fault = match small_icon_disabled.dispatch(5) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected disabled small-icon-height fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "get-system-metrics-index-unsupported");
    assert_eq!(small_icon_disabled.memory.stats().generation, generation);
    assert_eq!(
        small_icon_disabled
            .runtime
            .current_last_error()
            .expect("unchanged LastError"),
        last_error
    );

    let mut small_icon_width_disabled = MetricsFixture::new(true);
    small_icon_width_disabled
        .policy
        .get_system_metrics
        .as_mut()
        .expect("metrics policy")
        .small_icon_width = None;
    small_icon_width_disabled.policy.load_image_a = None;
    small_icon_width_disabled.policy.send_message_a = None;
    small_icon_width_disabled.policy.set_window_pos = None;
    let generation = small_icon_width_disabled.memory.stats().generation;
    let last_error = small_icon_width_disabled
        .runtime
        .current_last_error()
        .expect("entry LastError");
    let fault = match small_icon_width_disabled.dispatch(6) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected disabled small-icon-width fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "get-system-metrics-index-unsupported");
    assert_eq!(
        small_icon_width_disabled.memory.stats().generation,
        generation
    );
    assert_eq!(
        small_icon_width_disabled
            .runtime
            .current_last_error()
            .expect("unchanged LastError"),
        last_error
    );

    let mut disabled = MetricsFixture::new(true);
    disabled.policy.get_system_metrics = None;
    disabled.policy.create_window_ex_a = None;
    disabled.policy.invalid_menu_chain = None;
    disabled.policy.load_image_a = None;
    disabled.policy.send_message_a = None;
    disabled.policy.set_window_pos = None;
    disabled.policy.set_window_long_a = None;
    let fault = match disabled.dispatch(0) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected disabled metrics policy fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "get-system-metrics-policy-disabled");

    let mut missing = MetricsFixture::new(false);
    let fault = match missing.dispatch(0) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected missing display authority fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::RuntimeDefect);
    assert_eq!(fault.code, "logical-display-profile-missing");

    let mut mismatch = MetricsFixture::new(false);
    mismatch
        .runtime
        .configure_display_profile(RuntimeDisplayProfile {
            id: "mismatched-1280x720-v1".to_owned(),
            primary_monitor_rect: RuntimeRect {
                left: 0,
                top: 0,
                right: 1280,
                bottom: 720,
            },
        })
        .expect("configure mismatched metrics display");
    let fault = match mismatch.dispatch(0) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected mismatched display authority fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::RuntimeDefect);
    assert_eq!(fault.code, "logical-display-profile-mismatch");
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
    disabled.policy.load_image_a = None;
    disabled.policy.send_message_a = None;
    disabled.policy.set_window_pos = None;
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
fn load_image_a_selects_small_dib_and_reuses_shared_identity_across_apis() {
    let mut fixture = IconFixture::new();
    let memory_generation = fixture.memory.stats().generation;
    let large = match fixture.dispatch(0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected large LoadIconA completion, got {other:?}"),
    };
    assert_eq!(large.eax, RegisterUpdate::Set(0x4901_0001));

    let first = match fixture.dispatch_image(0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected first LoadImageA completion, got {other:?}"),
    };
    assert_eq!(first.eax, RegisterUpdate::Set(0x4901_0002));
    assert_eq!(first.stack_pointer, GuestAddress(IMAGE_STACK_POINTER + 28));
    assert_eq!(first.instruction_pointer, IMAGE_RETURN_ADDRESS);
    assert_eq!(first.last_error, 0);
    assert!(first.guest_writes.is_empty());
    assert_eq!(fixture.memory.stats().generation, memory_generation);
    assert_eq!(fixture.runtime.icons().len(), 2);

    let small = fixture
        .runtime
        .icon(0x4901_0002)
        .expect("logical shared small icon")
        .clone();
    assert_eq!(small.id.0, 2);
    assert_eq!(small.namespace, "vine.logical-icon.v1");
    assert!(small.shared);
    assert_eq!(
        small.resource.module_load_base,
        GuestAddress(ICON_IMAGE_BASE)
    );
    assert_eq!(small.resource.module_source_sha256, ICON_FIXTURE_SHA256);
    assert_eq!(small.resource.group_resource_id, 101);
    assert_eq!(small.resource.language_id, 1041);
    assert_eq!(small.resource.icon_resource_id, 3);
    assert_eq!(small.resource.group_data_size, 48);
    assert_eq!(small.resource.icon_data_size, 1_128);
    assert_eq!(small.resource.width, 16);
    assert_eq!(small.resource.height, 16);
    assert_eq!(small.resource.planes, 1);
    assert_eq!(small.resource.bits_per_pixel, 32);

    let repeated = match fixture.dispatch_image(1) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected repeated LoadImageA completion, got {other:?}"),
    };
    assert_eq!(repeated.eax, RegisterUpdate::Set(small.handle));
    assert_eq!(repeated.last_error, 0);
    assert_eq!(fixture.runtime.icons().len(), 2);
    assert_eq!(fixture.runtime.icon(small.handle).unwrap(), &small);
    assert_eq!(fixture.memory.stats().generation, memory_generation);

    let repeated_large = match fixture.dispatch(1) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected repeated LoadIconA completion, got {other:?}"),
    };
    assert_eq!(repeated_large.eax, large.eax);
    assert_eq!(fixture.runtime.icons().len(), 2);
}

#[test]
fn load_image_a_negative_variants_are_failure_atomic() {
    let mut missing = IconFixture::new();
    let generation = missing.memory.stats().generation;
    let completion = match missing.dispatch_image(2) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected missing LoadImageA resource completion, got {other:?}"),
    };
    assert_eq!(completion.eax, RegisterUpdate::Set(0));
    assert_eq!(completion.last_error, 1813);
    assert!(missing.runtime.icons().is_empty());
    assert_eq!(missing.memory.stats().generation, generation);
    let valid = match missing.dispatch_image(0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected valid LoadImageA retry, got {other:?}"),
    };
    assert_eq!(valid.eax, RegisterUpdate::Set(0x4901_0001));

    for (index, code) in [
        (3, "load-image-a-type-unsupported"),
        (4, "load-image-a-size-unsupported"),
        (5, "load-image-a-flags-unsupported"),
        (6, "load-image-a-string-resource-unsupported"),
    ] {
        let mut fixture = IconFixture::new();
        let generation = fixture.memory.stats().generation;
        let fault = match fixture.dispatch_image(index) {
            DispatchOutcome::Fault(fault) => fault,
            other => panic!("expected LoadImageA variant fault, got {other:?}"),
        };
        assert_eq!(fault.class, ApiFaultClass::Unsupported);
        assert_eq!(fault.code, code);
        assert!(fixture.runtime.icons().is_empty());
        assert_eq!(fixture.memory.stats().generation, generation);
    }

    let mut unknown_module = IconFixture::new();
    let completion = match unknown_module.dispatch_image(7) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid LoadImageA module completion, got {other:?}"),
    };
    assert_eq!(completion.eax, RegisterUpdate::Set(0));
    assert_eq!(completion.last_error, 6);
    assert!(unknown_module.runtime.icons().is_empty());

    let mut disabled = IconFixture::new();
    disabled.policy.load_image_a = None;
    disabled.policy.send_message_a = None;
    disabled.policy.set_window_pos = None;
    let fault = match disabled.dispatch_image(0) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected disabled LoadImageA policy fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "load-image-a-policy-disabled");
    assert!(disabled.runtime.icons().is_empty());
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
        .set_current_last_error(0x8765_4321)
        .expect("seed module cursor LastError");
    let module = match fixture.dispatch(4) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected module LoadCursorA completion, got {other:?}"),
    };
    assert_eq!(module.eax, RegisterUpdate::Set(0x4301_0002));
    assert_eq!(module.last_error, 0);
    assert!(module.guest_writes.is_empty());
    assert_eq!(fixture.memory.stats().generation, memory_generation);
    assert_eq!(fixture.runtime.cursors().len(), 2);
    let module_cursor = fixture
        .runtime
        .cursor(0x4301_0002)
        .expect("logical module cursor")
        .clone();
    let RuntimeCursorSource::ModuleResource { resource } = &module_cursor.source else {
        panic!("expected module resource cursor source");
    };
    assert_eq!(resource.module_id.0, 1);
    assert_eq!(resource.module_load_base, GuestAddress(ICON_IMAGE_BASE));
    assert_eq!(resource.module_source_sha256, ICON_FIXTURE_SHA256);
    assert_eq!(resource.group_resource_id, 106);
    assert_eq!(resource.language_id, 1041);
    assert_eq!(resource.cursor_resource_id, 4);
    assert_eq!(resource.group_data_rva.0, 0x6d28);
    assert_eq!(resource.group_data_size, 20);
    assert_eq!(resource.cursor_data_rva.0, 0x5c78);
    assert_eq!(resource.cursor_data_size, 4_268);
    assert_eq!((resource.width, resource.height), (32, 32));
    assert_eq!((resource.planes, resource.bits_per_pixel), (1, 32));
    assert_eq!((resource.hotspot_x, resource.hotspot_y), (3, 4));

    let repeated_module = match fixture.dispatch(6) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected repeated module LoadCursorA completion, got {other:?}"),
    };
    assert_eq!(
        repeated_module.eax,
        RegisterUpdate::Set(module_cursor.handle)
    );
    assert_eq!(repeated_module.last_error, 0);
    assert_eq!(fixture.runtime.cursors().len(), 2);
    assert_eq!(
        fixture.runtime.cursor(module_cursor.handle).unwrap(),
        &module_cursor
    );

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
        (5, "load-cursor-a-string-resource-unsupported"),
        (10, "load-cursor-a-string-resource-unsupported"),
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

    for (index, last_error) in [(7, 1813), (8, 1813), (9, 6)] {
        let mut fixture = CursorFixture::new();
        let generation = fixture.memory.stats().generation;
        let completion = match fixture.dispatch(index) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected module cursor failure completion, got {other:?}"),
        };
        assert_eq!(completion.eax, RegisterUpdate::Set(0));
        assert_eq!(completion.last_error, last_error);
        assert!(fixture.runtime.cursors().is_empty());
        assert_eq!(fixture.memory.stats().generation, generation);
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
fn load_cursor_a_rejects_malformed_module_resources_before_identity_mutation() {
    let mut malformed_group = CursorFixture::from_bytes(malformed_group_cursor_bytes());
    let generation = malformed_group.memory.stats().generation;
    let completion = match malformed_group.dispatch(4) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected malformed group cursor completion, got {other:?}"),
    };
    assert_eq!(completion.eax, RegisterUpdate::Set(0));
    assert_eq!(completion.last_error, 1812);
    assert!(malformed_group.runtime.cursors().is_empty());
    assert_eq!(malformed_group.memory.stats().generation, generation);

    let mut wrong_size = CursorFixture::from_bytes(wrong_size_group_cursor_bytes());
    let generation = wrong_size.memory.stats().generation;
    let fault = match wrong_size.dispatch(4) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected unsupported cursor size fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(
        fault.code,
        "load-cursor-a-module-size-selection-unsupported"
    );
    assert!(wrong_size.runtime.cursors().is_empty());
    assert_eq!(wrong_size.memory.stats().generation, generation);

    let mut invalid_hotspot = CursorFixture::from_bytes(invalid_cursor_hotspot_bytes());
    let generation = invalid_hotspot.memory.stats().generation;
    let completion = match invalid_hotspot.dispatch(4) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid cursor hotspot completion, got {other:?}"),
    };
    assert_eq!(completion.eax, RegisterUpdate::Set(0));
    assert_eq!(completion.last_error, 1812);
    assert!(invalid_hotspot.runtime.cursors().is_empty());
    assert_eq!(invalid_hotspot.memory.stats().generation, generation);
}

#[test]
fn register_class_ex_a_copies_two_local_classes_and_rejects_duplicate_name() {
    let mut fixture = RegisterClassFixture::new();
    fixture.prepare_resources();
    let class_bytes = fixture
        .memory
        .read(GuestAddress(CLASS_STRUCT_BASE), 48)
        .expect("main class bytes");
    let memory_generation = fixture.memory.stats().generation;

    fixture
        .runtime
        .set_current_last_error(0x1122_3344)
        .expect("seed class LastError");
    let main = match fixture.dispatch_class(0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected main class completion, got {other:?}"),
    };
    assert_eq!(main.eax, RegisterUpdate::Set(0xc000));
    assert_eq!(main.stack_pointer, GuestAddress(STACK_POINTER + 0x48));
    assert_eq!(main.instruction_pointer, CLASS_RETURN_ADDRESS);
    assert_eq!(main.last_error, 0x1122_3344);
    assert!(main.guest_writes.is_empty());
    assert_eq!(fixture.memory.stats().generation, memory_generation);
    assert_eq!(
        fixture
            .memory
            .read(GuestAddress(CLASS_STRUCT_BASE), 48)
            .unwrap(),
        class_bytes
    );

    let main_class = fixture
        .runtime
        .window_class(0xc000)
        .expect("main logical class")
        .clone();
    assert_eq!(main_class.id.0, 1);
    assert_eq!(
        main_class.atom_namespace,
        "vine.logical-window-class-atom.v1"
    );
    assert!(main_class.local);
    assert_eq!(main_class.registration.style, 0x28);
    assert_eq!(main_class.registration.class_name.value, "Vine Main Class");
    assert_eq!(
        main_class.registration.class_name.encoding,
        "windows-932-v1"
    );
    assert_eq!(
        main_class.registration.module.load_base,
        GuestAddress(ICON_IMAGE_BASE)
    );
    assert_eq!(
        main_class.registration.module.source_sha256,
        ICON_FIXTURE_SHA256
    );
    assert_eq!(main_class.registration.window_procedure.argument_bytes, 16);
    assert_eq!(main_class.registration.icon.unwrap().handle, 0x4901_0001);
    assert_eq!(main_class.registration.cursor.unwrap().handle, 0x4301_0001);
    assert_eq!(
        fixture
            .runtime
            .window_class_by_name(main_class.registration.module.module_id, "vine MAIN class",)
            .unwrap()
            .id,
        main_class.id
    );

    fixture
        .runtime
        .set_current_last_error(0x5566_7788)
        .expect("seed child LastError");
    let child = match fixture.dispatch_class(1) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected child class completion, got {other:?}"),
    };
    assert_eq!(child.eax, RegisterUpdate::Set(0xc001));
    assert_eq!(child.last_error, 0x5566_7788);
    assert_eq!(fixture.runtime.window_classes().len(), 2);
    assert_eq!(
        fixture
            .runtime
            .window_class(0xc001)
            .unwrap()
            .registration
            .style,
        0x3020
    );

    let duplicate = match fixture.dispatch_class(2) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected duplicate class failure, got {other:?}"),
    };
    assert_eq!(duplicate.eax, RegisterUpdate::Set(0));
    assert_eq!(duplicate.last_error, 1410);
    assert!(duplicate.guest_writes.is_empty());
    assert_eq!(fixture.runtime.window_classes().len(), 2);
    assert_eq!(fixture.memory.stats().generation, memory_generation);

    fixture
        .runtime
        .terminate_current_process(0)
        .expect("terminate class process");
    assert!(fixture.runtime.window_classes().is_empty());
    assert!(fixture.runtime.icons().is_empty());
    assert!(fixture.runtime.cursors().is_empty());
    assert_eq!(
        fixture
            .runtime
            .window_class(0xc000)
            .expect_err("process teardown retires class ATOM")
            .code,
        "window-class-atom-unknown"
    );
}

#[test]
fn create_window_ex_a_runs_ordered_callback_replacement_and_default_nonclient_geometry() {
    let mut fixture = RegisterClassFixture::new();
    fixture.prepare_resources();
    assert!(matches!(
        fixture.dispatch_class(0),
        DispatchOutcome::Immediate(ref completion)
            if completion.eax == RegisterUpdate::Set(0xc000)
    ));
    fixture
        .runtime
        .set_current_last_error(0x2468_1357)
        .expect("seed CreateWindowExA LastError");
    let mut call = match fixture.dispatch_create() {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected first creation callback, got {other:?}"),
    };
    assert_eq!(fixture.runtime.windows().len(), 1);
    let window = fixture.runtime.windows().values().next().unwrap().clone();
    assert_eq!(window.handle, 0x4801_0001);
    assert_eq!(window.creation.geometry.client_width, 800);
    assert_eq!(window.creation.geometry.client_height, 600);
    assert_eq!(
        window.lifecycle,
        RuntimeWindowLifecycle::Creating {
            message: RuntimeWindowCreationMessage::NonClientCreate,
        }
    );

    let mut prepared = ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime)
        .begin_guest_callback(&call)
        .expect("enter WM_NCCREATE callback");
    fixture.acknowledge_pending_delta();
    assert_eq!(prepared.frame_id, RuntimeCallbackFrameId(1));
    assert_eq!(
        fixture
            .runtime
            .current_thread_state()
            .unwrap()
            .callback_frames
            .len(),
        1
    );
    let read_word = |bytes: &[u8], offset: usize| {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
    };
    let create_struct = fixture
        .memory
        .read(prepared.scratch_address, 48)
        .expect("read callback CREATESTRUCTA");
    assert_eq!(read_word(&create_struct, 0), 0);
    assert_eq!(read_word(&create_struct, 4), ICON_IMAGE_BASE);
    assert_eq!(read_word(&create_struct, 16), 629);
    assert_eq!(read_word(&create_struct, 20), 806);
    assert_eq!(read_word(&create_struct, 24), 214);
    assert_eq!(read_word(&create_struct, 28), 557);
    assert_eq!(read_word(&create_struct, 32), 0x80ca_0000);
    assert_eq!(read_word(&create_struct, 44), 0x0004_0100);
    let title_pointer = read_word(&create_struct, 36);
    let class_pointer = read_word(&create_struct, 40);
    assert_eq!(
        fixture
            .memory
            .read(GuestAddress(title_pointer), 9)
            .expect("read relocated window title"),
        b"Ethornell"
    );
    assert_eq!(
        fixture
            .memory
            .read(GuestAddress(class_pointer), 15)
            .expect("read relocated class name"),
        b"Vine Main Class"
    );

    let expected_messages = [
        RuntimeWindowCreationMessage::NonClientCreate,
        RuntimeWindowCreationMessage::NonClientCalculate,
        RuntimeWindowCreationMessage::Create,
        RuntimeWindowCreationMessage::Size,
        RuntimeWindowCreationMessage::Move,
    ];
    let expected_message_ids = [0x0081, 0x0083, 0x0001, 0x0005, 0x0003];
    let mut final_completion = None;
    for (index, (message, message_id)) in expected_messages
        .into_iter()
        .zip(expected_message_ids)
        .enumerate()
    {
        assert_eq!(
            call.request.arguments[1],
            GuestCallbackArgument::U32(message_id)
        );
        let callback_result = match message {
            RuntimeWindowCreationMessage::NonClientCreate => {
                let completion = match fixture.dispatch_def_window_proc(
                    window.handle,
                    message_id,
                    prepared.scratch_address.0,
                ) {
                    DispatchOutcome::Immediate(completion) => completion,
                    other => panic!("expected WM_NCCREATE default result, got {other:?}"),
                };
                fixture.acknowledge_pending_delta();
                assert_eq!(completion.eax, RegisterUpdate::Set(1));
                1
            }
            RuntimeWindowCreationMessage::NonClientCalculate => {
                assert_eq!(
                    fixture
                        .memory
                        .read(prepared.scratch_address, 16)
                        .expect("read outer NCCALCSIZE rect"),
                    [557_i32, 214, 1363, 843]
                        .into_iter()
                        .flat_map(i32::to_le_bytes)
                        .collect::<Vec<_>>()
                );
                let completion = match fixture.dispatch_def_window_proc(
                    window.handle,
                    message_id,
                    prepared.scratch_address.0,
                ) {
                    DispatchOutcome::Immediate(completion) => completion,
                    other => panic!("expected WM_NCCALCSIZE default result, got {other:?}"),
                };
                fixture.acknowledge_pending_delta();
                assert_eq!(completion.eax, RegisterUpdate::Set(0));
                assert_eq!(
                    fixture
                        .memory
                        .read(prepared.scratch_address, 16)
                        .expect("read client NCCALCSIZE rect"),
                    [560_i32, 240, 1360, 840]
                        .into_iter()
                        .flat_map(i32::to_le_bytes)
                        .collect::<Vec<_>>()
                );
                0
            }
            RuntimeWindowCreationMessage::Size | RuntimeWindowCreationMessage::Move => {
                let l_param = match call.request.arguments[3] {
                    GuestCallbackArgument::U32(value) => value,
                    other => panic!("expected packed callback lParam, got {other:?}"),
                };
                let expected_code = if message == RuntimeWindowCreationMessage::Size {
                    "def-window-proc-size-lparam-mismatch"
                } else {
                    "def-window-proc-move-lparam-mismatch"
                };
                let fault = match fixture.dispatch_def_window_proc(
                    window.handle,
                    message_id,
                    l_param ^ 1,
                ) {
                    DispatchOutcome::Fault(fault) => fault,
                    other => panic!("expected packed lParam mismatch, got {other:?}"),
                };
                assert_eq!(fault.class, ApiFaultClass::InvalidCall);
                assert_eq!(fault.code, expected_code);
                assert_eq!(
                    fixture.runtime.current_last_error().unwrap(),
                    0x2468_1357,
                    "failed default processing must preserve LastError"
                );
                let completion =
                    match fixture.dispatch_def_window_proc(window.handle, message_id, l_param) {
                        DispatchOutcome::Immediate(completion) => completion,
                        other => panic!("expected packed default result, got {other:?}"),
                    };
                assert_eq!(completion.eax, RegisterUpdate::Set(0));
                assert_eq!(completion.last_error, 0x2468_1357);
                0
            }
            RuntimeWindowCreationMessage::Create => 0,
        };
        let completion_policy = call.request.completion_policy;
        let endpoint = call.frame.endpoint.clone();
        let outcome = {
            let mut session = ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime);
            fixture.dispatcher.complete_guest_callback_result(
                call.clone(),
                callback_result,
                &mut session,
            )
        };
        if index + 1 < expected_messages.len() {
            let next = match outcome {
                DispatchOutcome::GuestCallback(next) => next,
                other => panic!("expected replacement callback after {message:?}, got {other:?}"),
            };
            let action = {
                let mut session = ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime);
                let next_prepared = session
                    .replace_guest_callback(&next, prepared.frame_id)
                    .expect("replace creation callback frame");
                let action = session
                    .commit_guest_callback_semantics(completion_policy, callback_result, &endpoint)
                    .expect("commit creation callback semantics")
                    .expect("CreateWindowExA has semantic continuation");
                prepared = next_prepared;
                action
            };
            fixture.acknowledge_pending_delta();
            assert!(matches!(
                action,
                RuntimeGuestCallbackSemanticAction::WindowCreation(
                    RuntimeWindowCreationCallbackAction::Continue { .. }
                )
            ));
            assert_eq!(
                fixture
                    .runtime
                    .current_thread_state()
                    .unwrap()
                    .callback_frames
                    .len(),
                1,
                "callback replacement must preserve depth"
            );
            assert_eq!(prepared.frame_id.0, u64::try_from(index + 2).unwrap());
            call = next;
        } else {
            let completion = match outcome {
                DispatchOutcome::Immediate(completion) => completion,
                other => panic!("expected completed CreateWindowExA, got {other:?}"),
            };
            fixture
                .runtime
                .validate_callback_frame_return(prepared.frame_id)
                .expect("preflight final callback pop");
            fixture
                .runtime
                .pop_callback_frame(prepared.frame_id)
                .expect("pop final callback frame");
            let action = ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime)
                .commit_guest_callback_semantics(completion_policy, callback_result, &endpoint)
                .expect("commit final creation semantics")
                .expect("CreateWindowExA has final semantic action");
            assert!(matches!(
                action,
                RuntimeGuestCallbackSemanticAction::WindowCreation(
                    RuntimeWindowCreationCallbackAction::Created { .. }
                )
            ));
            final_completion = Some(completion);
        }
    }
    let completion = final_completion.expect("creation completed");
    assert_eq!(completion.eax, RegisterUpdate::Set(window.handle));
    assert_eq!(completion.last_error, 0x2468_1357);
    assert_eq!(completion.instruction_pointer, CREATE_RETURN_ADDRESS);
    assert_eq!(
        completion.stack_pointer,
        GuestAddress(CREATE_STACK_POINTER + 13 * 4)
    );
    assert_eq!(
        fixture.runtime.window(window.handle).unwrap().lifecycle,
        RuntimeWindowLifecycle::Created
    );
    assert!(
        fixture
            .runtime
            .current_thread_state()
            .unwrap()
            .callback_frames
            .is_empty()
    );
}

#[test]
fn send_message_a_dispatches_same_thread_small_icon_to_registered_window_procedure() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, icon_handle) = fixture.prepare_created_window_and_small_icon();
    fixture.configure_send_message(window_handle, 0x0080, 0, icon_handle);
    fixture
        .runtime
        .set_current_last_error(0x2468_1357)
        .expect("seed SendMessageA LastError");
    let windows_before = fixture.runtime.windows().clone();
    let icons_before = fixture.runtime.icons().clone();
    let memory_generation_before = fixture.memory.stats().generation;

    let call = match fixture.dispatch_send_message() {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected SendMessageA window callback, got {other:?}"),
    };
    assert_eq!(call.request.target, fixture.window_procedure);
    assert_eq!(
        call.request.arguments,
        [window_handle, 0x0080, 0, icon_handle]
            .into_iter()
            .map(GuestCallbackArgument::U32)
            .collect::<Vec<_>>()
    );
    assert!(call.request.scratch.is_empty());
    assert!(call.request.scratch_pointer_relocations.is_empty());
    assert_eq!(call.request.return_sentinel, fixture.callback_sentinel);
    assert_eq!(
        call.request.completion_policy,
        GuestCallbackCompletionPolicy::ReturnU32
    );
    assert_eq!(fixture.runtime.windows(), &windows_before);
    assert_eq!(fixture.runtime.icons(), &icons_before);
    assert_eq!(fixture.memory.stats().generation, memory_generation_before);
    assert!(
        fixture
            .runtime
            .current_thread_state()
            .unwrap()
            .callback_frames
            .is_empty()
    );

    let prepared = ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime)
        .begin_guest_callback(&call)
        .expect("enter SendMessageA callback");
    fixture.acknowledge_pending_delta();
    assert_eq!(prepared.scratch_address, GuestAddress(0));
    assert_eq!(prepared.scratch_bytes, 0);
    assert_eq!(prepared.frame_bytes, 20);
    assert_eq!(
        prepared.allocation_start,
        GuestAddress(SEND_MESSAGE_STACK_POINTER - 20)
    );
    assert_eq!(prepared.stack_pointer, prepared.allocation_start);
    let callback_frame = fixture
        .memory
        .read(prepared.stack_pointer, 20)
        .expect("read SendMessageA callback frame");
    assert_eq!(
        callback_frame,
        [
            fixture.callback_sentinel.0,
            window_handle,
            0x0080,
            0,
            icon_handle,
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>()
    );

    let default_completion =
        match fixture.dispatch_def_window_proc(window_handle, 0x0080, icon_handle) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected nested WM_SETICON default completion, got {other:?}"),
        };
    assert_eq!(default_completion.eax, RegisterUpdate::Set(0));
    assert_eq!(default_completion.last_error, 0x2468_1357);
    assert!(default_completion.guest_writes.is_empty());
    assert_eq!(
        fixture
            .runtime
            .window(window_handle)
            .expect("window after WM_SETICON")
            .small_icon
            .expect("small icon reference")
            .handle,
        icon_handle
    );
    let windows_with_icon = fixture.runtime.windows().clone();

    let repeated_default =
        match fixture.dispatch_def_window_proc(window_handle, 0x0080, icon_handle) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected repeated WM_SETICON completion, got {other:?}"),
        };
    assert_eq!(repeated_default.eax, RegisterUpdate::Set(icon_handle));
    assert_eq!(repeated_default.last_error, 0x2468_1357);
    assert_eq!(fixture.runtime.windows(), &windows_with_icon);

    let callback_result = 0x89ab_cdef;
    let completion = {
        let mut session = ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime);
        match fixture
            .dispatcher
            .complete_guest_callback_result(call, callback_result, &mut session)
        {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected completed SendMessageA, got {other:?}"),
        }
    };
    fixture
        .runtime
        .pop_callback_frame(prepared.frame_id)
        .expect("retire SendMessageA callback frame");
    assert_eq!(completion.eax, RegisterUpdate::Set(callback_result));
    assert_eq!(completion.last_error, 0x2468_1357);
    assert_eq!(completion.instruction_pointer, SEND_MESSAGE_RETURN_ADDRESS);
    assert_eq!(
        completion.stack_pointer,
        GuestAddress(SEND_MESSAGE_STACK_POINTER + 20)
    );
    assert!(completion.guest_writes.is_empty());
    assert_eq!(fixture.runtime.windows(), &windows_with_icon);
    assert_eq!(fixture.runtime.icons(), &icons_before);
    assert!(
        fixture
            .runtime
            .current_thread_state()
            .unwrap()
            .callback_frames
            .is_empty()
    );
}

#[test]
fn set_window_pos_commits_show_and_hide_after_ordered_nested_callbacks() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, icon_handle) = fixture.prepare_created_window_with_small_icon();
    let initial_window = fixture
        .runtime
        .window(window_handle)
        .expect("created window")
        .clone();
    assert_eq!(
        initial_window.creation.geometry,
        RuntimeWindowGeometry {
            x: 557,
            y: 214,
            width: 806,
            height: 629,
            client_x: 560,
            client_y: 240,
            client_width: 800,
            client_height: 600,
        }
    );
    assert!(!initial_window.visible);
    assert_eq!(
        initial_window.small_icon.map(|icon| icon.handle),
        Some(icon_handle)
    );

    fixture
        .runtime
        .set_current_last_error(0x2468_1357)
        .expect("seed SetWindowPos LastError");
    fixture.configure_set_window_pos(
        [window_handle, 0xffff_fffe, 15_360, 8_640, 0, 0, 0x0000_0559],
        SET_WINDOW_POS_SHOW_RETURN_ADDRESS,
    );
    let show_call = match fixture.dispatch_set_window_pos(SET_WINDOW_POS_SHOW_RETURN_ADDRESS) {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected SetWindowPos show callback, got {other:?}"),
    };
    assert_eq!(show_call.request.target, fixture.window_procedure);
    assert_eq!(
        show_call.request.arguments,
        vec![
            GuestCallbackArgument::U32(window_handle),
            GuestCallbackArgument::U32(0x0047),
            GuestCallbackArgument::U32(0),
            GuestCallbackArgument::ScratchPointer { offset: 0 },
        ]
    );
    assert_eq!(show_call.request.scratch.len(), 28);
    assert_eq!(
        show_call.request.scratch,
        [
            window_handle,
            0xffff_fffe,
            15_360,
            8_640,
            806,
            629,
            0x1000_0d59,
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>()
    );
    let GuestCallbackCompletionPolicy::SetWindowPos {
        operation: show_operation,
        message: RuntimeSetWindowPosCallbackMessage::WindowPositionChanged,
    } = show_call.request.completion_policy
    else {
        panic!("show callback must carry SetWindowPos semantics");
    };
    assert_eq!(
        show_operation.previous_geometry,
        initial_window.creation.geometry
    );
    assert_eq!(show_operation.next_geometry.x, 15_360);
    assert_eq!(show_operation.next_geometry.y, 8_640);
    assert_eq!(show_operation.next_geometry.client_x, 15_363);
    assert_eq!(show_operation.next_geometry.client_y, 8_666);
    assert_eq!(
        fixture.runtime.window(window_handle).unwrap(),
        &initial_window
    );

    let show_prepared = ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime)
        .begin_guest_callback(&show_call)
        .expect("enter WM_WINDOWPOSCHANGED callback");
    fixture.acknowledge_pending_delta();
    assert_eq!(show_prepared.scratch_bytes, 28);
    assert_eq!(
        fixture
            .memory
            .read(show_prepared.scratch_address, 28)
            .expect("read WINDOWPOS scratch"),
        show_call.request.scratch
    );
    assert_eq!(
        fixture
            .memory
            .read(show_prepared.stack_pointer, 20)
            .expect("read WM_WINDOWPOSCHANGED callback frame"),
        [
            fixture.callback_sentinel.0,
            window_handle,
            0x0047,
            0,
            show_prepared.scratch_address.0,
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>()
    );

    let move_call = match fixture.dispatch_def_window_proc(
        window_handle,
        0x0047,
        show_prepared.scratch_address.0,
    ) {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected nested WM_MOVE callback, got {other:?}"),
    };
    assert_eq!(
        move_call.request.arguments,
        [window_handle, 0x0003, 0, 0x21da_3c03]
            .into_iter()
            .map(GuestCallbackArgument::U32)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        move_call.request.completion_policy,
        GuestCallbackCompletionPolicy::ReturnConstantU32 { value: 0 }
    );
    let move_prepared = ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime)
        .begin_guest_callback(&move_call)
        .expect("enter nested WM_MOVE callback");
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture
            .runtime
            .current_thread_state()
            .expect("owner thread")
            .callback_frames
            .len(),
        2
    );
    let move_policy = move_call.request.completion_policy;
    let move_endpoint = move_call.frame.endpoint.clone();
    let move_completion = {
        let mut session = ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime);
        match fixture.dispatcher.complete_guest_callback_result(
            move_call,
            0xdead_beef,
            &mut session,
        ) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected completed DefWindowProcA, got {other:?}"),
        }
    };
    assert_eq!(move_completion.eax, RegisterUpdate::Set(0));
    fixture
        .runtime
        .pop_callback_frame(move_prepared.frame_id)
        .expect("retire nested WM_MOVE callback");
    assert_eq!(
        ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime)
            .commit_guest_callback_semantics(move_policy, 0xdead_beef, &move_endpoint)
            .expect("commit constant callback semantics"),
        None
    );
    assert_eq!(
        fixture.runtime.window(window_handle).unwrap(),
        &initial_window
    );

    let show_policy = show_call.request.completion_policy;
    let show_endpoint = show_call.frame.endpoint.clone();
    let icon_call = {
        let mut session = ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime);
        match fixture
            .dispatcher
            .complete_guest_callback_result(show_call, 0, &mut session)
        {
            DispatchOutcome::GuestCallback(call) => call,
            other => panic!("expected replacement WM_GETICON callback, got {other:?}"),
        }
    };
    assert_eq!(
        icon_call.request.arguments,
        [window_handle, 0x007f, 2, 0]
            .into_iter()
            .map(GuestCallbackArgument::U32)
            .collect::<Vec<_>>()
    );
    let icon_prepared = {
        let mut session = ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime);
        let prepared = session
            .replace_guest_callback(&icon_call, show_prepared.frame_id)
            .expect("replace WM_WINDOWPOSCHANGED with WM_GETICON");
        let action = session
            .commit_guest_callback_semantics(show_policy, 0, &show_endpoint)
            .expect("commit SetWindowPos continuation")
            .expect("show callback has semantic continuation");
        assert!(matches!(
            action,
            RuntimeGuestCallbackSemanticAction::SetWindowPos(
                RuntimeSetWindowPosCallbackAction::Continue {
                    next_message: RuntimeSetWindowPosCallbackMessage::GetSmallIcon,
                    ..
                }
            )
        ));
        prepared
    };
    fixture.acknowledge_pending_delta();
    assert_eq!(icon_prepared.scratch_bytes, 0);
    assert_eq!(
        fixture
            .runtime
            .current_thread_state()
            .expect("owner thread")
            .callback_frames
            .len(),
        1,
        "replacement must preserve the outer callback depth"
    );
    assert_eq!(
        fixture.runtime.window(window_handle).unwrap(),
        &initial_window
    );

    let icon_policy = icon_call.request.completion_policy;
    let icon_endpoint = icon_call.frame.endpoint.clone();
    let show_completion = {
        let mut session = ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime);
        match fixture.dispatcher.complete_guest_callback_result(
            icon_call,
            icon_handle,
            &mut session,
        ) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected completed SetWindowPos show, got {other:?}"),
        }
    };
    assert_eq!(show_completion.eax, RegisterUpdate::Set(1));
    assert_eq!(show_completion.last_error, 0x2468_1357);
    assert_eq!(
        show_completion.instruction_pointer,
        SET_WINDOW_POS_SHOW_RETURN_ADDRESS
    );
    assert_eq!(
        show_completion.stack_pointer,
        GuestAddress(SET_WINDOW_POS_STACK_POINTER + 32)
    );
    assert_eq!(
        fixture.runtime.window(window_handle).unwrap(),
        &initial_window
    );
    fixture
        .runtime
        .pop_callback_frame(icon_prepared.frame_id)
        .expect("retire WM_GETICON callback");
    let show_action = ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime)
        .commit_guest_callback_semantics(icon_policy, icon_handle, &icon_endpoint)
        .expect("commit SetWindowPos show")
        .expect("show completion has semantic action");
    let RuntimeGuestCallbackSemanticAction::SetWindowPos(
        RuntimeSetWindowPosCallbackAction::Completed {
            window: shown_window,
        },
    ) = show_action
    else {
        panic!("expected completed show action");
    };
    assert_eq!(shown_window.creation.geometry, show_operation.next_geometry);
    assert!(shown_window.visible);
    assert!(!shown_window.topmost);
    assert_eq!(
        fixture.runtime.window(window_handle).unwrap(),
        &shown_window
    );

    fixture.configure_set_window_pos(
        [window_handle, 0xffff_fffe, 0, 0, 0, 0, 0x0000_059b],
        SET_WINDOW_POS_HIDE_RETURN_ADDRESS,
    );
    let hide_call = match fixture.dispatch_set_window_pos(SET_WINDOW_POS_HIDE_RETURN_ADDRESS) {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected SetWindowPos hide callback, got {other:?}"),
    };
    assert_eq!(
        hide_call.request.scratch,
        [
            window_handle,
            0xffff_fffe,
            15_360,
            8_640,
            806,
            629,
            0x0000_1d9f,
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>()
    );
    let hide_prepared = ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime)
        .begin_guest_callback(&hide_call)
        .expect("enter hide WM_WINDOWPOSCHANGED callback");
    fixture.acknowledge_pending_delta();
    let hide_default = match fixture.dispatch_def_window_proc(
        window_handle,
        0x0047,
        hide_prepared.scratch_address.0,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("hide default processing must not emit WM_MOVE, got {other:?}"),
    };
    assert_eq!(hide_default.eax, RegisterUpdate::Set(0));
    assert_eq!(
        fixture
            .runtime
            .current_thread_state()
            .expect("owner thread")
            .callback_frames
            .len(),
        1
    );
    let hide_policy = hide_call.request.completion_policy;
    let hide_endpoint = hide_call.frame.endpoint.clone();
    let hide_completion = {
        let mut session = ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime);
        match fixture
            .dispatcher
            .complete_guest_callback_result(hide_call, 0, &mut session)
        {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected completed SetWindowPos hide, got {other:?}"),
        }
    };
    assert_eq!(hide_completion.eax, RegisterUpdate::Set(1));
    assert_eq!(hide_completion.last_error, 0x2468_1357);
    assert_eq!(
        hide_completion.instruction_pointer,
        SET_WINDOW_POS_HIDE_RETURN_ADDRESS
    );
    assert_eq!(
        fixture.runtime.window(window_handle).unwrap(),
        &shown_window
    );
    fixture
        .runtime
        .pop_callback_frame(hide_prepared.frame_id)
        .expect("retire hide callback");
    let hide_action = ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime)
        .commit_guest_callback_semantics(hide_policy, 0, &hide_endpoint)
        .expect("commit SetWindowPos hide")
        .expect("hide completion has semantic action");
    let RuntimeGuestCallbackSemanticAction::SetWindowPos(
        RuntimeSetWindowPosCallbackAction::Completed {
            window: hidden_window,
        },
    ) = hide_action
    else {
        panic!("expected completed hide action");
    };
    assert_eq!(
        hidden_window.creation.geometry,
        shown_window.creation.geometry
    );
    assert!(!hidden_window.visible);
    assert!(!hidden_window.topmost);
    assert_eq!(hidden_window.small_icon, shown_window.small_icon);
    assert_eq!(
        fixture.runtime.window(window_handle).unwrap(),
        &hidden_window
    );
    assert!(
        fixture
            .runtime
            .current_thread_state()
            .expect("owner thread")
            .callback_frames
            .is_empty()
    );
}

#[test]
fn set_window_pos_rejects_unreached_variants_and_invalid_state_atomically() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, _) = fixture.prepare_created_window_with_small_icon();

    for (insert_after, x, y, width, height, flags, class, code) in [
        (
            0,
            15_360,
            8_640,
            0,
            0,
            0x0000_0559,
            ApiFaultClass::Unsupported,
            "set-window-pos-insert-after-unsupported",
        ),
        (
            0xffff_fffe,
            15_360,
            8_640,
            1,
            0,
            0x0000_0559,
            ApiFaultClass::Unsupported,
            "set-window-pos-size-unsupported",
        ),
        (
            0xffff_fffe,
            15_361,
            8_640,
            0,
            0,
            0x0000_0559,
            ApiFaultClass::Unsupported,
            "set-window-pos-variant-unsupported",
        ),
        (
            0xffff_fffe,
            0,
            0,
            0,
            0,
            0x0000_059b,
            ApiFaultClass::InvalidCall,
            "set-window-pos-hide-state-invalid",
        ),
    ] {
        fixture.configure_set_window_pos(
            [window_handle, insert_after, x, y, width, height, flags],
            SET_WINDOW_POS_SHOW_RETURN_ADDRESS,
        );
        assert_set_window_pos_fault_is_atomic(
            &mut fixture,
            SET_WINDOW_POS_SHOW_RETURN_ADDRESS,
            class,
            code,
        );
    }

    for (invalid_window, code) in [
        (0, "null-logical-handle-forbidden"),
        (0x4301_0001, "window-handle-unknown"),
        (0x4801_ffff, "window-handle-unknown"),
    ] {
        fixture.configure_set_window_pos(
            [
                invalid_window,
                0xffff_fffe,
                15_360,
                8_640,
                0,
                0,
                0x0000_0559,
            ],
            SET_WINDOW_POS_SHOW_RETURN_ADDRESS,
        );
        assert_set_window_pos_fault_is_atomic(
            &mut fixture,
            SET_WINDOW_POS_SHOW_RETURN_ADDRESS,
            ApiFaultClass::InvalidCall,
            code,
        );
    }

    fixture.policy.set_window_pos = None;
    fixture.configure_set_window_pos(
        [window_handle, 0xffff_fffe, 15_360, 8_640, 0, 0, 0x0000_0559],
        SET_WINDOW_POS_SHOW_RETURN_ADDRESS,
    );
    assert_set_window_pos_fault_is_atomic(
        &mut fixture,
        SET_WINDOW_POS_SHOW_RETURN_ADDRESS,
        ApiFaultClass::Unsupported,
        "set-window-pos-policy-disabled",
    );

    let mut no_icon = RegisterClassFixture::new();
    let (window_handle, _) = no_icon.prepare_created_window_and_small_icon();
    no_icon.configure_set_window_pos(
        [window_handle, 0xffff_fffe, 15_360, 8_640, 0, 0, 0x0000_0559],
        SET_WINDOW_POS_SHOW_RETURN_ADDRESS,
    );
    assert_set_window_pos_fault_is_atomic(
        &mut no_icon,
        SET_WINDOW_POS_SHOW_RETURN_ADDRESS,
        ApiFaultClass::InvalidCall,
        "set-window-pos-show-state-invalid",
    );

    let mut no_display = RegisterClassFixture::without_display();
    let (window_handle, _) = no_display.prepare_created_window_with_small_icon();
    no_display.configure_set_window_pos(
        [window_handle, 0xffff_fffe, 15_360, 8_640, 0, 0, 0x0000_0559],
        SET_WINDOW_POS_SHOW_RETURN_ADDRESS,
    );
    assert_set_window_pos_fault_is_atomic(
        &mut no_display,
        SET_WINDOW_POS_SHOW_RETURN_ADDRESS,
        ApiFaultClass::InvalidCall,
        "set-window-pos-display-authority-missing",
    );

    let mut cross_thread = RegisterClassFixture::new();
    let (window_handle, _) = cross_thread.prepare_created_window_with_small_icon();
    cross_thread
        .runtime
        .register_thread(RuntimeThreadId(0x2001), 0x1357_2468)
        .expect("register non-owner thread");
    cross_thread
        .runtime
        .select_thread(RuntimeThreadId(0x2001))
        .expect("select non-owner thread");
    cross_thread.configure_set_window_pos(
        [window_handle, 0xffff_fffe, 15_360, 8_640, 0, 0, 0x0000_0559],
        SET_WINDOW_POS_SHOW_RETURN_ADDRESS,
    );
    assert_set_window_pos_fault_is_atomic(
        &mut cross_thread,
        SET_WINDOW_POS_SHOW_RETURN_ADDRESS,
        ApiFaultClass::InvalidCall,
        "window-message-owner-thread-mismatch",
    );

    let mut creating = RegisterClassFixture::new();
    creating.prepare_resources();
    assert!(matches!(
        creating.dispatch_class(0),
        DispatchOutcome::Immediate(_)
    ));
    let image_endpoint = creating.image_endpoint.clone();
    assert!(matches!(
        creating.dispatch_endpoint(
            image_endpoint,
            GuestAddress(WINDOW_IMAGE_STACK_POINTER),
            IMAGE_RETURN_ADDRESS,
        ),
        DispatchOutcome::Immediate(_)
    ));
    assert!(matches!(
        creating.dispatch_create(),
        DispatchOutcome::GuestCallback(_)
    ));
    let creating_handle = creating.runtime.windows().values().next().unwrap().handle;
    creating.configure_set_window_pos(
        [
            creating_handle,
            0xffff_fffe,
            15_360,
            8_640,
            0,
            0,
            0x0000_0559,
        ],
        SET_WINDOW_POS_SHOW_RETURN_ADDRESS,
    );
    assert_set_window_pos_fault_is_atomic(
        &mut creating,
        SET_WINDOW_POS_SHOW_RETURN_ADDRESS,
        ApiFaultClass::InvalidCall,
        "window-message-lifecycle-state-invalid",
    );

    let mut callback_preflight = RegisterClassFixture::new();
    let (window_handle, _) = callback_preflight.prepare_created_window_with_small_icon();
    callback_preflight.callback_sentinel = MEMORY_BASE;
    callback_preflight.configure_set_window_pos(
        [window_handle, 0xffff_fffe, 15_360, 8_640, 0, 0, 0x0000_0559],
        SET_WINDOW_POS_SHOW_RETURN_ADDRESS,
    );
    assert_set_window_pos_fault_is_atomic(
        &mut callback_preflight,
        SET_WINDOW_POS_SHOW_RETURN_ADDRESS,
        ApiFaultClass::InvalidCall,
        "guest-page-fault",
    );
}

#[test]
fn set_window_pos_callback_result_failures_do_not_publish_window_state() {
    let mut wrong_position_result = RegisterClassFixture::new();
    let (window_handle, _) = wrong_position_result.prepare_created_window_with_small_icon();
    wrong_position_result.configure_set_window_pos(
        [window_handle, 0xffff_fffe, 15_360, 8_640, 0, 0, 0x0000_0559],
        SET_WINDOW_POS_SHOW_RETURN_ADDRESS,
    );
    let call =
        match wrong_position_result.dispatch_set_window_pos(SET_WINDOW_POS_SHOW_RETURN_ADDRESS) {
            DispatchOutcome::GuestCallback(call) => call,
            other => panic!("expected SetWindowPos callback, got {other:?}"),
        };
    let prepared = ExecutionSession::new(
        &mut wrong_position_result.memory,
        &mut wrong_position_result.runtime,
    )
    .begin_guest_callback(&call)
    .expect("enter SetWindowPos callback");
    wrong_position_result.acknowledge_pending_delta();
    let runtime_before = wrong_position_result.runtime.clone();
    let memory_before = wrong_position_result.memory.stats();
    let memory_hash_before = wrong_position_result
        .memory
        .current_reflection()
        .expect("callback reflection")
        .content_sha256;
    let fault = {
        let mut session = ExecutionSession::new(
            &mut wrong_position_result.memory,
            &mut wrong_position_result.runtime,
        );
        match wrong_position_result
            .dispatcher
            .complete_guest_callback_result(call, 1, &mut session)
        {
            DispatchOutcome::Fault(fault) => fault,
            other => panic!("expected invalid position callback result, got {other:?}"),
        }
    };
    assert_eq!(fault.class, ApiFaultClass::InvalidCall);
    assert_eq!(fault.code, "set-window-pos-window-procedure-result-invalid");
    assert_eq!(wrong_position_result.runtime, runtime_before);
    assert_eq!(wrong_position_result.memory.stats(), memory_before);
    assert_eq!(
        wrong_position_result
            .memory
            .current_reflection()
            .expect("unchanged callback reflection")
            .content_sha256,
        memory_hash_before
    );
    wrong_position_result
        .runtime
        .pop_callback_frame(prepared.frame_id)
        .expect("retire rejected position callback");

    let mut wrong_icon_result = RegisterClassFixture::new();
    let (window_handle, icon_handle) = wrong_icon_result.prepare_created_window_with_small_icon();
    wrong_icon_result.configure_set_window_pos(
        [window_handle, 0xffff_fffe, 15_360, 8_640, 0, 0, 0x0000_0559],
        SET_WINDOW_POS_SHOW_RETURN_ADDRESS,
    );
    let position_call =
        match wrong_icon_result.dispatch_set_window_pos(SET_WINDOW_POS_SHOW_RETURN_ADDRESS) {
            DispatchOutcome::GuestCallback(call) => call,
            other => panic!("expected SetWindowPos callback, got {other:?}"),
        };
    let position_prepared = ExecutionSession::new(
        &mut wrong_icon_result.memory,
        &mut wrong_icon_result.runtime,
    )
    .begin_guest_callback(&position_call)
    .expect("enter position callback");
    wrong_icon_result.acknowledge_pending_delta();
    let position_policy = position_call.request.completion_policy;
    let position_endpoint = position_call.frame.endpoint.clone();
    let icon_call = {
        let mut session = ExecutionSession::new(
            &mut wrong_icon_result.memory,
            &mut wrong_icon_result.runtime,
        );
        match wrong_icon_result.dispatcher.complete_guest_callback_result(
            position_call,
            0,
            &mut session,
        ) {
            DispatchOutcome::GuestCallback(call) => call,
            other => panic!("expected WM_GETICON replacement, got {other:?}"),
        }
    };
    let icon_prepared = {
        let mut session = ExecutionSession::new(
            &mut wrong_icon_result.memory,
            &mut wrong_icon_result.runtime,
        );
        let prepared = session
            .replace_guest_callback(&icon_call, position_prepared.frame_id)
            .expect("replace position callback");
        assert!(matches!(
            session
                .commit_guest_callback_semantics(position_policy, 0, &position_endpoint)
                .expect("commit position continuation"),
            Some(RuntimeGuestCallbackSemanticAction::SetWindowPos(
                RuntimeSetWindowPosCallbackAction::Continue { .. }
            ))
        ));
        prepared
    };
    wrong_icon_result.acknowledge_pending_delta();
    let runtime_before = wrong_icon_result.runtime.clone();
    let memory_before = wrong_icon_result.memory.stats();
    let memory_hash_before = wrong_icon_result
        .memory
        .current_reflection()
        .expect("replacement reflection")
        .content_sha256;
    let fault = {
        let mut session = ExecutionSession::new(
            &mut wrong_icon_result.memory,
            &mut wrong_icon_result.runtime,
        );
        match wrong_icon_result.dispatcher.complete_guest_callback_result(
            icon_call,
            icon_handle.wrapping_add(1),
            &mut session,
        ) {
            DispatchOutcome::Fault(fault) => fault,
            other => panic!("expected invalid WM_GETICON result, got {other:?}"),
        }
    };
    assert_eq!(fault.class, ApiFaultClass::InvalidCall);
    assert_eq!(fault.code, "set-window-pos-small-icon-result-mismatch");
    assert_eq!(wrong_icon_result.runtime, runtime_before);
    assert_eq!(wrong_icon_result.memory.stats(), memory_before);
    assert_eq!(
        wrong_icon_result
            .memory
            .current_reflection()
            .expect("unchanged replacement reflection")
            .content_sha256,
        memory_hash_before
    );
    assert!(
        !wrong_icon_result
            .runtime
            .window(window_handle)
            .unwrap()
            .visible
    );
    wrong_icon_result
        .runtime
        .pop_callback_frame(icon_prepared.frame_id)
        .expect("retire rejected icon callback");
}

#[test]
fn send_message_a_rejects_wrong_identity_lifecycle_thread_and_variant_atomically() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, icon_handle) = fixture.prepare_created_window_and_small_icon();

    for (message, w_param, code) in [
        (0x0081, 0, "send-message-a-message-unsupported"),
        (0x0080, 1, "send-message-a-wparam-unsupported"),
    ] {
        fixture.configure_send_message(window_handle, message, w_param, icon_handle);
        assert_send_message_fault_is_atomic(&mut fixture, ApiFaultClass::Unsupported, code);
    }

    fixture.configure_send_message(0, 0x0080, 0, icon_handle);
    assert_send_message_fault_is_atomic(
        &mut fixture,
        ApiFaultClass::InvalidCall,
        "null-logical-handle-forbidden",
    );
    for invalid_window in [0x4301_0001, 0x4801_ffff] {
        fixture.configure_send_message(invalid_window, 0x0080, 0, icon_handle);
        assert_send_message_fault_is_atomic(
            &mut fixture,
            ApiFaultClass::InvalidCall,
            "window-handle-unknown",
        );
    }

    fixture.configure_send_message(window_handle, 0x0080, 0, 0x4901_0001);
    assert_send_message_fault_is_atomic(
        &mut fixture,
        ApiFaultClass::InvalidCall,
        "window-message-icon-variant-invalid",
    );
    for invalid_icon in [0, 0x4301_0001, 0x4901_ffff] {
        fixture.configure_send_message(window_handle, 0x0080, 0, invalid_icon);
        assert_send_message_fault_is_atomic(
            &mut fixture,
            ApiFaultClass::InvalidCall,
            "icon-handle-unknown",
        );
    }

    fixture.policy.send_message_a = None;
    fixture.policy.set_window_pos = None;
    fixture.configure_send_message(window_handle, 0x0080, 0, icon_handle);
    assert_send_message_fault_is_atomic(
        &mut fixture,
        ApiFaultClass::Unsupported,
        "send-message-a-policy-disabled",
    );
}

#[test]
fn send_message_a_rejects_creating_cross_thread_and_unexecutable_callback_atomically() {
    let mut creating = RegisterClassFixture::new();
    creating.prepare_resources();
    assert!(matches!(
        creating.dispatch_class(0),
        DispatchOutcome::Immediate(_)
    ));
    let image_endpoint = creating.image_endpoint.clone();
    assert!(matches!(
        creating.dispatch_endpoint(
            image_endpoint,
            GuestAddress(WINDOW_IMAGE_STACK_POINTER),
            IMAGE_RETURN_ADDRESS,
        ),
        DispatchOutcome::Immediate(ref completion)
            if completion.eax == RegisterUpdate::Set(0x4901_0002)
    ));
    assert!(matches!(
        creating.dispatch_create(),
        DispatchOutcome::GuestCallback(_)
    ));
    let creating_handle = creating.runtime.windows().values().next().unwrap().handle;
    creating.configure_send_message(creating_handle, 0x0080, 0, 0x4901_0002);
    assert_send_message_fault_is_atomic(
        &mut creating,
        ApiFaultClass::InvalidCall,
        "window-message-lifecycle-state-invalid",
    );

    let mut cross_thread = RegisterClassFixture::new();
    let (window_handle, icon_handle) = cross_thread.prepare_created_window_and_small_icon();
    cross_thread
        .runtime
        .register_thread(RuntimeThreadId(0x2001), 0x1357_2468)
        .expect("register non-owner thread");
    cross_thread
        .runtime
        .select_thread(RuntimeThreadId(0x2001))
        .expect("select non-owner thread");
    cross_thread.configure_send_message(window_handle, 0x0080, 0, icon_handle);
    assert_send_message_fault_is_atomic(
        &mut cross_thread,
        ApiFaultClass::InvalidCall,
        "window-message-owner-thread-mismatch",
    );

    let mut callback_preflight = RegisterClassFixture::new();
    let (window_handle, icon_handle) = callback_preflight.prepare_created_window_and_small_icon();
    callback_preflight.callback_sentinel = MEMORY_BASE;
    callback_preflight.configure_send_message(window_handle, 0x0080, 0, icon_handle);
    assert_send_message_fault_is_atomic(
        &mut callback_preflight,
        ApiFaultClass::InvalidCall,
        "guest-page-fault",
    );
}

#[test]
fn create_window_ex_a_preflight_and_callback_rejection_do_not_leave_live_windows() {
    let mut preflight = RegisterClassFixture::new();
    preflight.prepare_resources();
    assert!(matches!(
        preflight.dispatch_class(0),
        DispatchOutcome::Immediate(_)
    ));
    let valid_sentinel = preflight.callback_sentinel;
    preflight.callback_sentinel = MEMORY_BASE;
    let fault = match preflight.dispatch_create() {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected callback preflight fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::InvalidCall);
    assert!(preflight.runtime.windows().is_empty());
    preflight.callback_sentinel = valid_sentinel;
    let call = match preflight.dispatch_create() {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected valid retry callback, got {other:?}"),
    };
    assert_eq!(
        preflight.runtime.windows().values().next().unwrap().handle,
        0x4801_0001
    );
    let prepared = ExecutionSession::new(&mut preflight.memory, &mut preflight.runtime)
        .begin_guest_callback(&call)
        .expect("enter rejecting callback");
    preflight.acknowledge_pending_delta();
    let completion_policy = call.request.completion_policy;
    let endpoint = call.frame.endpoint.clone();
    let completion = {
        let mut session = ExecutionSession::new(&mut preflight.memory, &mut preflight.runtime);
        preflight
            .dispatcher
            .complete_guest_callback_result(call, 0, &mut session)
    };
    assert!(matches!(
        completion,
        DispatchOutcome::Immediate(ref completion)
            if completion.eax == RegisterUpdate::Set(0) && completion.last_error == 87
    ));
    preflight
        .runtime
        .pop_callback_frame(prepared.frame_id)
        .expect("pop rejected callback");
    let action = ExecutionSession::new(&mut preflight.memory, &mut preflight.runtime)
        .commit_guest_callback_semantics(completion_policy, 0, &endpoint)
        .expect("commit rejected creation")
        .expect("CreateWindowExA rejection has semantic action");
    assert!(matches!(
        action,
        RuntimeGuestCallbackSemanticAction::WindowCreation(
            RuntimeWindowCreationCallbackAction::Aborted { .. }
        )
    ));
    assert!(preflight.runtime.windows().is_empty());
    assert_eq!(
        preflight
            .runtime
            .window(0x4801_0001)
            .expect_err("rejected HWND must be retired")
            .code,
        "window-handle-unknown"
    );
}

#[test]
fn register_class_ex_a_negative_paths_are_failure_atomic_and_do_not_consume_atom() {
    for index in [3_u32, 4, 5, 6, 7] {
        let mut fixture = RegisterClassFixture::new();
        fixture.prepare_resources();
        let generation = fixture.memory.stats().generation;
        let completion = match fixture.dispatch_class(index) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected invalid class completion for {index}, got {other:?}"),
        };
        assert_eq!(completion.eax, RegisterUpdate::Set(0));
        assert_eq!(completion.last_error, 87);
        assert!(completion.guest_writes.is_empty());
        assert!(fixture.runtime.window_classes().is_empty());
        assert_eq!(fixture.memory.stats().generation, generation);
        let valid = match fixture.dispatch_class(0) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected valid class retry after {index}, got {other:?}"),
        };
        assert_eq!(valid.eax, RegisterUpdate::Set(0xc000));
    }

    for (index, expected_code) in [
        (8_u32, "register-class-ex-a-style-unsupported"),
        (9, "register-class-ex-a-non-ascii-name-unsupported"),
        (10, "register-class-ex-a-optional-field-unsupported"),
        (11, "register-class-ex-a-name-bound-exceeded"),
    ] {
        let mut fixture = RegisterClassFixture::new();
        fixture.prepare_resources();
        let generation = fixture.memory.stats().generation;
        let fault = match fixture.dispatch_class(index) {
            DispatchOutcome::Fault(fault) => fault,
            other => panic!("expected unsupported class fault for {index}, got {other:?}"),
        };
        assert_eq!(fault.class, ApiFaultClass::Unsupported);
        assert_eq!(fault.code, expected_code);
        assert!(fixture.runtime.window_classes().is_empty());
        assert_eq!(fixture.memory.stats().generation, generation);
    }

    let mut invalid_pointer = RegisterClassFixture::new();
    invalid_pointer.prepare_resources();
    let generation = invalid_pointer.memory.stats().generation;
    let fault = match invalid_pointer.dispatch_class(12) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected invalid WNDCLASSEXA pointer fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::InvalidCall);
    assert_eq!(fault.code, "guest-page-fault");
    assert!(invalid_pointer.runtime.window_classes().is_empty());
    assert_eq!(invalid_pointer.memory.stats().generation, generation);

    let mut disabled = RegisterClassFixture::new();
    disabled.prepare_resources();
    disabled.policy.register_class_ex_a = None;
    let fault = match disabled.dispatch_class(0) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected disabled class policy fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "register-class-ex-a-policy-disabled");
    assert!(disabled.runtime.window_classes().is_empty());
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
        assert_eq!(prepared.allocation_start, GuestAddress(STACK_POINTER - 20));
        assert_eq!(prepared.scratch_address, CALLBACK_SCRATCH_ARENA_BASE);
        assert_eq!(prepared.stack_pointer, GuestAddress(STACK_POINTER - 20));
        assert_eq!(prepared.scratch_bytes, 16);
        assert_eq!(prepared.frame_bytes, 20);
        assert_eq!(fixture.memory.stats().generation, generation_before + 4);
        let delta = fixture
            .memory
            .pending_delta()
            .expect("callback frame delta");
        fixture
            .memory
            .acknowledge_delta(delta.generation)
            .expect("acknowledge callback frame delta");

        assert_eq!(
            fixture
                .memory
                .read(prepared.scratch_address, prepared.scratch_bytes as usize)
                .expect("callback scratch bytes"),
            call.request.scratch
        );
        let mut expected = CALLBACK_SENTINEL.0.to_le_bytes().to_vec();
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
        max_physical_pages: 4,
        max_regions: 2,
        max_owners: 2,
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

fn malformed_group_cursor_bytes() -> Vec<u8> {
    let mut bytes = icon_fixture_bytes();
    let limits = Pe32Limits::default();
    let snapshot = ModuleSnapshot::from_bytes(bytes.clone(), &limits).unwrap();
    let loader = build_pe32_load_plan(&snapshot, limits).unwrap();
    let group_rva = loader
        .load_plan
        .resources
        .as_ref()
        .and_then(|resources| resources.integer_entries(12, 106).next())
        .expect("group cursor resource")
        .data_rva
        .0;
    let offset = rva_file_offset(&loader, group_rva).expect("group cursor file offset");
    bytes[offset + 2..offset + 4].copy_from_slice(&1_u16.to_le_bytes());
    bytes
}

fn wrong_size_group_cursor_bytes() -> Vec<u8> {
    let mut bytes = icon_fixture_bytes();
    let limits = Pe32Limits::default();
    let snapshot = ModuleSnapshot::from_bytes(bytes.clone(), &limits).unwrap();
    let loader = build_pe32_load_plan(&snapshot, limits).unwrap();
    let group_rva = loader
        .load_plan
        .resources
        .as_ref()
        .and_then(|resources| resources.integer_entries(12, 106).next())
        .expect("group cursor resource")
        .data_rva
        .0;
    let offset = rva_file_offset(&loader, group_rva).expect("group cursor file offset");
    bytes[offset + 6..offset + 8].copy_from_slice(&16_u16.to_le_bytes());
    bytes
}

fn invalid_cursor_hotspot_bytes() -> Vec<u8> {
    let mut bytes = icon_fixture_bytes();
    let limits = Pe32Limits::default();
    let snapshot = ModuleSnapshot::from_bytes(bytes.clone(), &limits).unwrap();
    let loader = build_pe32_load_plan(&snapshot, limits).unwrap();
    let cursor_rva = loader
        .load_plan
        .resources
        .as_ref()
        .and_then(|resources| resources.integer_entries(1, 4).next())
        .expect("cursor resource")
        .data_rva
        .0;
    let offset = rva_file_offset(&loader, cursor_rva).expect("cursor file offset");
    bytes[offset..offset + 2].copy_from_slice(&32_u16.to_le_bytes());
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

use crate as vine_core;
use crate::api::{GuestCallbackArgument, GuestCallbackCompletionPolicy};
use crate::api::{PreparedGuestCallback, RuntimeGuestCallbackSemanticAction};
use crate::imports::{ImportThunk, ImportThunkDisposition, ImportThunkId};
use crate::runtime::{
    RUNTIME_PRIMARY_MONITOR_HANDLE, RuntimeCallbackFrameId, RuntimeDeviceContextTarget,
    RuntimeImeStatusWindowRequest, RuntimeSetWindowPosCallbackAction,
    RuntimeSetWindowPosCallbackMessage, RuntimeSetWindowPosDefaultCallbackAction,
    RuntimeSetWindowPosDefaultCallbackMessage, RuntimeSetWindowPosOperation,
    RuntimeSetWindowPosVariant, RuntimeWindowCreationCallbackAction, RuntimeWindowCreationMessage,
    RuntimeWindowGeometry, RuntimeWindowLifecycle,
};
use crate::user32::dispatch_user32_api;
use std::fs;
use std::path::PathBuf;

use vine_core::{
    ApiDispatcher, ApiEndpointIdentity, ApiGuestCallbackCall, ApiImmediateCompletion, ApiOwnedTrap,
    DispatchOutcome, GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestRange, ImportTarget,
    MemoryLimits, MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind, ModuleSnapshot, PageProtection,
    Pe32Limits, Pe32LoadPlan, Runtime, RuntimeBootstrap, RuntimeClockState,
    RuntimeCurrentDirectoryA, RuntimeCursorPositionProfile, RuntimeDisplayProfile,
    RuntimeInputProfile, RuntimeKeyboardSourceEvent, RuntimeKeyboardSourceEventKind, RuntimeModule,
    RuntimePointerButton, RuntimePointerButtonSourceEvent, RuntimePointerButtonSourceEventKind,
    RuntimePointerPositionSourceEvent, RuntimeProcessId, RuntimeProcessImagePathA, RuntimeRect,
    RuntimeStartupInfo, RuntimeThreadId, SchedulerThreadState, build_pe32_load_plan,
    materialize_pe32_image,
};

const MEMORY_BASE: GuestAddress = GuestAddress(0x2000_0000);
const STACK_POINTER: u32 = 0x2000_1040;
const IAT_ADDRESS: GuestAddress = GuestAddress(0x2000_2000);
const CALLBACK_TARGET: GuestAddress = GuestAddress(0x2000_2100);
const CALLBACK_SENTINEL: GuestAddress = GuestAddress(0x2000_2110);
const ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0bc0);
const RETURN_ADDRESS: GuestAddress = GuestAddress(0x1006_a781);
const SWAP_FIRST_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1008_8a3b);
const SHOW_CURSOR_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0a60);
const SHOW_CURSOR_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x2380;
const SHOW_CURSOR_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1008_8c08);
const ICON_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0bf0);
const IMAGE_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0c00);
const IMAGE_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 4);
const IMAGE_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x1200;
const IMAGE_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1006_7f43);
const CURSOR_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0f30);
const CURSOR_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1006_7d09);
const CLASS_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0be0);
const CLASS_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1006_7d28);
const ICON_IMAGE_BASE: u32 = 0x0040_0000;
const ICON_MODULE_HANDLE: u32 = 0x4d01_000f;

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
const SET_WINDOW_TEXT_A_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0ec0);
const SET_WINDOW_TEXT_A_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 136);
const SET_WINDOW_TEXT_A_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x2fe0;
const SET_WINDOW_TEXT_A_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1008_343b);
const UPDATED_WINDOW_TITLE_BASE: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2b82);
const IMM_GET_DEFAULT_IME_WND_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_00a0);
const IMM_ASSOCIATE_CONTEXT_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_00b0);
const IMM_GET_DEFAULT_IME_WND_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 68);
const IMM_ASSOCIATE_CONTEXT_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 72);
const IMM_GET_DEFAULT_IME_WND_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x21a0;
const IMM_ASSOCIATE_CONTEXT_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x21c0;
const IMM_GET_DEFAULT_IME_WND_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1008_73da);
const IMM_ASSOCIATE_CONTEXT_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1008_73fe);
const SET_WINDOW_POS_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0c90);
const SET_WINDOW_POS_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 28);
const SET_WINDOW_POS_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x1e00;
const SET_WINDOW_POS_SHOW_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1006_7f88);
const SET_WINDOW_POS_HIDE_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1006_7f9f);
const SET_WINDOW_POS_ACTIVATE_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1005_cb4b);
const SHOW_WINDOW_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0ea0);
const SHOW_WINDOW_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 84);
const SHOW_WINDOW_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x21e0;
const SHOW_WINDOW_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1009_38da);
const UPDATE_WINDOW_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0cb0);
const UPDATE_WINDOW_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 88);
const UPDATE_WINDOW_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x2200;
const UPDATE_WINDOW_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1009_38e7);
const BEGIN_PAINT_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0e50);
const END_PAINT_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0e20);
const BEGIN_PAINT_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 92);
const END_PAINT_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 96);
const BEGIN_PAINT_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x2240;
const END_PAINT_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x2260;
const BEGIN_PAINT_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1009_28d9);
const END_PAINT_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1009_28e5);
const PAINT_STRUCT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2f00);
const GET_FOREGROUND_WINDOW_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0af0);
const GET_FOREGROUND_WINDOW_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 100);
const GET_FOREGROUND_WINDOW_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x2280;
const GET_FOREGROUND_WINDOW_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1009_390e);
const SET_FOCUS_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0d00);
const SET_FOCUS_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 132);
const SET_FOCUS_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x2f60;
const SET_FOCUS_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1009_2dcb);
const GET_CLIENT_RECT_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0c60);
const GET_CLIENT_RECT_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 32);
const GET_CLIENT_RECT_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x1f00;
const GET_CLIENT_RECT_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1005_c997);
const CLIENT_RECT_OUTPUT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2e00);
const GET_WINDOW_RECT_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0e30);
const GET_WINDOW_RECT_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 108);
const GET_WINDOW_RECT_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x22c0;
const GET_WINDOW_RECT_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1008_8558);
const WINDOW_RECT_OUTPUT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2e20);
const GET_CURSOR_INFO_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0a70);
const GET_CURSOR_INFO_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 140);
const GET_CURSOR_INFO_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x2340;
const GET_CURSOR_INFO_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1008_8c98);
const CURSOR_INFO_OUTPUT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2e40);
const SHOW_CURSOR_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 148);
const PEEK_MESSAGE_A_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0a30);
const PEEK_MESSAGE_A_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 116);
const PEEK_MESSAGE_A_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x2f80;
const PEEK_MESSAGE_A_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1009_378b);
const PEEK_MESSAGE_A_OUTPUT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2e60);
const TRANSLATE_MESSAGE_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0a10);
const TRANSLATE_MESSAGE_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 124);
const TRANSLATE_MESSAGE_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x2f20;
const TRANSLATE_MESSAGE_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1009_37a2);
const DISPATCH_MESSAGE_A_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0b50);
const DISPATCH_MESSAGE_A_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 128);
const DISPATCH_MESSAGE_A_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x2f40;
const DISPATCH_MESSAGE_A_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1009_37ac);
const IS_ICONIC_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0b60);
const IS_ICONIC_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 120);
const IS_ICONIC_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x2fc0;
const IS_ICONIC_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1008_89c4);
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
const SET_TIMER_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0b40);
const SET_TIMER_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 64);
const SET_TIMER_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x2160;
const SET_TIMER_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1009_9d4b);
const MONITOR_FROM_WINDOW_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0cd0);
const MONITOR_FROM_WINDOW_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 56);
const MONITOR_FROM_WINDOW_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x1fe0;
const MONITOR_FROM_WINDOW_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1005_9d19);
const GET_MONITOR_INFO_A_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0c70);
const GET_MONITOR_INFO_A_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 60);
const GET_MONITOR_INFO_A_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x2120;
const GET_MONITOR_INFO_A_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1005_c913);
const MONITOR_INFO_OUTPUT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2e40);
const WINDOW_TITLE_BASE: u32 = MEMORY_BASE.0 + 0x2b00;
const GET_SYSTEM_MENU_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0b10);
const ENABLE_MENU_ITEM_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0b20);
const LIVE_GET_SYSTEM_MENU_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 76);
const LIVE_ENABLE_MENU_ITEM_IAT_ADDRESS: GuestAddress = GuestAddress(IAT_ADDRESS.0 + 80);
const GET_SYSTEM_MENU_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x1a00;
const ENABLE_MENU_ITEM_STACK_POINTER: u32 = MEMORY_BASE.0 + 0x1b00;
const GET_SYSTEM_MENU_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1009_274c);
const ENABLE_MENU_ITEM_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1009_2753);

fn fixture_endpoint(
    id: u32,
    address: GuestAddress,
    requester_owner: MemoryOwnerId,
    module: &str,
    iat_address: GuestAddress,
    name: &str,
) -> ApiEndpointIdentity {
    ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(id),
        address,
        requester_owner,
        module: module.to_owned(),
        iat_rva: vine_core::Rva(0),
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

struct EnumFixture {
    runtime: Runtime,
    dispatcher: ApiDispatcher,
    endpoint: ApiEndpointIdentity,
    stack_pointer: u32,
}

impl EnumFixture {
    fn new(hdc: u32, clip: u32, callback: u32, data: u32) -> Self {
        Self::with_stack(STACK_POINTER, hdc, clip, callback, data)
    }

    fn with_stack(stack_pointer: u32, hdc: u32, clip: u32, callback: u32, data: u32) -> Self {
        let mut fixture = Self::without_display(stack_pointer, hdc, clip, callback, data);
        fixture
            .runtime
            .configure_display_profile(fixture_display_profile())
            .unwrap();
        fixture
    }

    fn without_display(stack_pointer: u32, hdc: u32, clip: u32, callback: u32, data: u32) -> Self {
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
        let dispatcher =
            ApiDispatcher::from_endpoints([endpoint.clone()]).expect("User32 dispatcher");
        let runtime = controlled_runtime(memory);

        Self {
            runtime,
            dispatcher,
            endpoint,
            stack_pointer,
        }
    }

    fn dispatch(&mut self) -> DispatchOutcome {
        self.runtime.dispatch_api_call(
            &self.dispatcher,
            ApiOwnedTrap {
                endpoint_id: self.endpoint.id,
                endpoint_address: self.endpoint.address,
                stack_pointer: GuestAddress(self.stack_pointer),
                return_address: RETURN_ADDRESS,
            },
            CALLBACK_SENTINEL,
            None,
        )
    }
}

struct IconFixture {
    runtime: Runtime,
    dispatcher: ApiDispatcher,
    endpoint: ApiEndpointIdentity,
    image_endpoint: ApiEndpointIdentity,
}

impl IconFixture {
    fn new() -> Self {
        let bytes = icon_fixture_bytes();
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
        assert_eq!(loader.image.preferred_image_base, ICON_IMAGE_BASE);
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

        let mut runtime = controlled_runtime(memory);
        runtime
            .register_main_module(RuntimeModule {
                handle: ICON_MODULE_HANDLE,
                requested_name: "icon-resource.exe".to_owned(),
                canonical_name: "icon-resource.exe".to_owned(),
                runtime_system_module: false,
            })
            .expect("register icon main module");
        runtime
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
        let image_endpoint = fixture_endpoint(
            193,
            IMAGE_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            IMAGE_IAT_ADDRESS,
            "LoadImageA",
        );
        let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone(), image_endpoint.clone()])
            .expect("module icon dispatcher");
        Self {
            runtime,
            dispatcher,
            endpoint,
            image_endpoint,
        }
    }

    fn dispatch(&mut self, index: u32) -> DispatchOutcome {
        let stack_pointer = GuestAddress(STACK_POINTER + index * 0x20);
        let return_address = GuestAddress(RETURN_ADDRESS.0 + index);
        self.runtime.dispatch_api_call(
            &self.dispatcher,
            ApiOwnedTrap {
                endpoint_id: self.endpoint.id,
                endpoint_address: self.endpoint.address,
                stack_pointer,
                return_address,
            },
            CALLBACK_SENTINEL,
            None,
        )
    }

    fn dispatch_image(&mut self, index: u32) -> DispatchOutcome {
        let stack_pointer = GuestAddress(IMAGE_STACK_POINTER + index * 0x20);
        let return_address = GuestAddress(IMAGE_RETURN_ADDRESS.0 + index);
        self.runtime.dispatch_api_call(
            &self.dispatcher,
            ApiOwnedTrap {
                endpoint_id: self.image_endpoint.id,
                endpoint_address: self.image_endpoint.address,
                stack_pointer,
                return_address,
            },
            CALLBACK_SENTINEL,
            None,
        )
    }
}

struct CursorFixture {
    runtime: Runtime,
    dispatcher: ApiDispatcher,
    endpoint: ApiEndpointIdentity,
}

impl CursorFixture {
    fn new() -> Self {
        let bytes = icon_fixture_bytes();
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
        assert_eq!(loader.image.preferred_image_base, ICON_IMAGE_BASE);
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

        let mut runtime = controlled_runtime(memory);
        runtime
            .register_main_module(RuntimeModule {
                handle: ICON_MODULE_HANDLE,
                requested_name: "icon-resource.exe".to_owned(),
                canonical_name: "icon-resource.exe".to_owned(),
                runtime_system_module: false,
            })
            .expect("register cursor main module");
        runtime
            .register_module_image(ICON_MODULE_HANDLE, &loader, &materialization)
            .expect("register cursor module image");

        let endpoint = ApiEndpointIdentity::from_import_thunk(&thunk);
        let dispatcher =
            ApiDispatcher::from_endpoints([endpoint.clone()]).expect("LoadCursorA dispatcher");
        Self {
            runtime,
            dispatcher,
            endpoint,
        }
    }

    fn dispatch(&mut self, index: u32) -> DispatchOutcome {
        let stack_pointer = GuestAddress(STACK_POINTER + index * 0x20);
        let return_address = GuestAddress(CURSOR_RETURN_ADDRESS.0 + index);
        self.runtime.dispatch_api_call(
            &self.dispatcher,
            ApiOwnedTrap {
                endpoint_id: self.endpoint.id,
                endpoint_address: self.endpoint.address,
                stack_pointer,
                return_address,
            },
            CALLBACK_SENTINEL,
            None,
        )
    }
}

struct RegisterClassFixture {
    runtime: Runtime,
    dispatcher: ApiDispatcher,
    icon_endpoint: ApiEndpointIdentity,
    cursor_endpoint: ApiEndpointIdentity,
    class_endpoint: ApiEndpointIdentity,
    create_endpoint: ApiEndpointIdentity,
    def_window_proc_endpoint: ApiEndpointIdentity,
    image_endpoint: ApiEndpointIdentity,
    send_message_endpoint: ApiEndpointIdentity,
    set_window_text_a_endpoint: ApiEndpointIdentity,
    imm_get_default_ime_wnd_endpoint: ApiEndpointIdentity,
    imm_associate_context_endpoint: ApiEndpointIdentity,
    set_window_pos_endpoint: ApiEndpointIdentity,
    show_window_endpoint: ApiEndpointIdentity,
    update_window_endpoint: ApiEndpointIdentity,
    begin_paint_endpoint: ApiEndpointIdentity,
    end_paint_endpoint: ApiEndpointIdentity,
    get_foreground_window_endpoint: ApiEndpointIdentity,
    set_focus_endpoint: ApiEndpointIdentity,
    get_client_rect_endpoint: ApiEndpointIdentity,
    get_window_rect_endpoint: ApiEndpointIdentity,
    get_cursor_info_endpoint: ApiEndpointIdentity,
    show_cursor_endpoint: ApiEndpointIdentity,
    peek_message_a_endpoint: ApiEndpointIdentity,
    translate_message_endpoint: ApiEndpointIdentity,
    dispatch_message_a_endpoint: ApiEndpointIdentity,
    is_iconic_endpoint: ApiEndpointIdentity,
    get_dc_endpoint: ApiEndpointIdentity,
    get_stock_object_endpoint: ApiEndpointIdentity,
    fill_rect_endpoint: ApiEndpointIdentity,
    release_dc_endpoint: ApiEndpointIdentity,
    set_window_long_a_endpoint: ApiEndpointIdentity,
    set_timer_endpoint: ApiEndpointIdentity,
    monitor_from_window_endpoint: ApiEndpointIdentity,
    get_monitor_info_a_endpoint: ApiEndpointIdentity,
    get_system_menu_endpoint: ApiEndpointIdentity,
    enable_menu_item_endpoint: ApiEndpointIdentity,
    window_procedure: GuestAddress,
    callback_sentinel: GuestAddress,
}

impl RegisterClassFixture {
    fn new() -> Self {
        Self::with_display(true)
    }

    fn without_display() -> Self {
        Self::with_display(false)
    }

    fn with_display(configure_display: bool) -> Self {
        let mut fixture = Self::with_display_without_input(configure_display);
        fixture
            .runtime
            .configure_input_profile(fixture_input_profile(false))
            .unwrap();
        fixture
    }

    fn with_display_without_input(configure_display: bool) -> Self {
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
            (
                SET_WINDOW_TEXT_A_IAT_ADDRESS,
                SET_WINDOW_TEXT_A_ENDPOINT_ADDRESS,
            ),
            (
                IMM_GET_DEFAULT_IME_WND_IAT_ADDRESS,
                IMM_GET_DEFAULT_IME_WND_ENDPOINT_ADDRESS,
            ),
            (
                IMM_ASSOCIATE_CONTEXT_IAT_ADDRESS,
                IMM_ASSOCIATE_CONTEXT_ENDPOINT_ADDRESS,
            ),
            (SET_WINDOW_POS_IAT_ADDRESS, SET_WINDOW_POS_ENDPOINT_ADDRESS),
            (SHOW_WINDOW_IAT_ADDRESS, SHOW_WINDOW_ENDPOINT_ADDRESS),
            (UPDATE_WINDOW_IAT_ADDRESS, UPDATE_WINDOW_ENDPOINT_ADDRESS),
            (BEGIN_PAINT_IAT_ADDRESS, BEGIN_PAINT_ENDPOINT_ADDRESS),
            (END_PAINT_IAT_ADDRESS, END_PAINT_ENDPOINT_ADDRESS),
            (
                GET_FOREGROUND_WINDOW_IAT_ADDRESS,
                GET_FOREGROUND_WINDOW_ENDPOINT_ADDRESS,
            ),
            (SET_FOCUS_IAT_ADDRESS, SET_FOCUS_ENDPOINT_ADDRESS),
            (
                GET_CLIENT_RECT_IAT_ADDRESS,
                GET_CLIENT_RECT_ENDPOINT_ADDRESS,
            ),
            (
                GET_WINDOW_RECT_IAT_ADDRESS,
                GET_WINDOW_RECT_ENDPOINT_ADDRESS,
            ),
            (
                GET_CURSOR_INFO_IAT_ADDRESS,
                GET_CURSOR_INFO_ENDPOINT_ADDRESS,
            ),
            (SHOW_CURSOR_IAT_ADDRESS, SHOW_CURSOR_ENDPOINT_ADDRESS),
            (PEEK_MESSAGE_A_IAT_ADDRESS, PEEK_MESSAGE_A_ENDPOINT_ADDRESS),
            (
                TRANSLATE_MESSAGE_IAT_ADDRESS,
                TRANSLATE_MESSAGE_ENDPOINT_ADDRESS,
            ),
            (
                DISPATCH_MESSAGE_A_IAT_ADDRESS,
                DISPATCH_MESSAGE_A_ENDPOINT_ADDRESS,
            ),
            (IS_ICONIC_IAT_ADDRESS, IS_ICONIC_ENDPOINT_ADDRESS),
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
            (
                MONITOR_FROM_WINDOW_IAT_ADDRESS,
                MONITOR_FROM_WINDOW_ENDPOINT_ADDRESS,
            ),
            (
                GET_MONITOR_INFO_A_IAT_ADDRESS,
                GET_MONITOR_INFO_A_ENDPOINT_ADDRESS,
            ),
            (SET_TIMER_IAT_ADDRESS, SET_TIMER_ENDPOINT_ADDRESS),
            (
                LIVE_GET_SYSTEM_MENU_IAT_ADDRESS,
                GET_SYSTEM_MENU_ENDPOINT_ADDRESS,
            ),
            (
                LIVE_ENABLE_MENU_ITEM_IAT_ADDRESS,
                ENABLE_MENU_ITEM_ENDPOINT_ADDRESS,
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
                &[b'A'; 256],
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
        write_words(
            &mut memory,
            runtime_owner,
            GuestAddress(MONITOR_FROM_WINDOW_STACK_POINTER),
            &[MONITOR_FROM_WINDOW_RETURN_ADDRESS.0, 0x4801_0001, 1],
        );
        write_words(
            &mut memory,
            runtime_owner,
            GuestAddress(GET_MONITOR_INFO_A_STACK_POINTER),
            &[
                GET_MONITOR_INFO_A_RETURN_ADDRESS.0,
                RUNTIME_PRIMARY_MONITOR_HANDLE,
                MONITOR_INFO_OUTPUT_ADDRESS.0,
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

        let mut runtime = controlled_runtime(memory);
        runtime
            .register_main_module(RuntimeModule {
                handle: ICON_MODULE_HANDLE,
                requested_name: "icon-resource.exe".to_owned(),
                canonical_name: "icon-resource.exe".to_owned(),
                runtime_system_module: false,
            })
            .expect("register class main module");
        runtime
            .register_module_image(ICON_MODULE_HANDLE, &loader, &materialization)
            .expect("register class module image");

        let icon_endpoint = fixture_endpoint(
            192,
            ICON_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            icon_iat,
            "LoadIconA",
        );
        let cursor_endpoint = fixture_endpoint(
            244,
            CURSOR_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            cursor_iat,
            "LoadCursorA",
        );
        let class_endpoint = fixture_endpoint(
            191,
            CLASS_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            CLASS_IAT_ADDRESS,
            "RegisterClassExA",
        );
        let create_endpoint = fixture_endpoint(
            232,
            CREATE_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            CREATE_IAT_ADDRESS,
            "CreateWindowExA",
        );
        let def_window_proc_endpoint = fixture_endpoint(
            155,
            DEF_WINDOW_PROC_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            DEF_WINDOW_PROC_IAT_ADDRESS,
            "DefWindowProcA",
        );
        let image_endpoint = fixture_endpoint(
            193,
            IMAGE_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            WINDOW_IMAGE_IAT_ADDRESS,
            "LoadImageA",
        );
        let send_message_endpoint = fixture_endpoint(
            229,
            SEND_MESSAGE_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            SEND_MESSAGE_IAT_ADDRESS,
            "SendMessageA",
        );
        let set_window_text_a_endpoint = fixture_endpoint(
            237,
            SET_WINDOW_TEXT_A_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            SET_WINDOW_TEXT_A_IAT_ADDRESS,
            "SetWindowTextA",
        );
        let imm_get_default_ime_wnd_endpoint = fixture_endpoint(
            311,
            IMM_GET_DEFAULT_IME_WND_ENDPOINT_ADDRESS,
            runtime_owner,
            "IMM32.dll",
            IMM_GET_DEFAULT_IME_WND_IAT_ADDRESS,
            "ImmGetDefaultIMEWnd",
        );
        let imm_associate_context_endpoint = fixture_endpoint(
            312,
            IMM_ASSOCIATE_CONTEXT_ENDPOINT_ADDRESS,
            runtime_owner,
            "IMM32.dll",
            IMM_ASSOCIATE_CONTEXT_IAT_ADDRESS,
            "ImmAssociateContext",
        );
        let set_window_pos_endpoint = fixture_endpoint(
            202,
            SET_WINDOW_POS_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            SET_WINDOW_POS_IAT_ADDRESS,
            "SetWindowPos",
        );
        let show_window_endpoint = fixture_endpoint(
            235,
            SHOW_WINDOW_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            SHOW_WINDOW_IAT_ADDRESS,
            "ShowWindow",
        );
        let update_window_endpoint = fixture_endpoint(
            204,
            UPDATE_WINDOW_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            UPDATE_WINDOW_IAT_ADDRESS,
            "UpdateWindow",
        );
        let begin_paint_endpoint = fixture_endpoint(
            230,
            BEGIN_PAINT_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            BEGIN_PAINT_IAT_ADDRESS,
            "BeginPaint",
        );
        let end_paint_endpoint = fixture_endpoint(
            227,
            END_PAINT_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            END_PAINT_IAT_ADDRESS,
            "EndPaint",
        );
        let get_foreground_window_endpoint = fixture_endpoint(
            176,
            GET_FOREGROUND_WINDOW_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            GET_FOREGROUND_WINDOW_IAT_ADDRESS,
            "GetForegroundWindow",
        );
        let set_focus_endpoint = fixture_endpoint(
            209,
            SET_FOCUS_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            SET_FOCUS_IAT_ADDRESS,
            "SetFocus",
        );
        let get_client_rect_endpoint = fixture_endpoint(
            199,
            GET_CLIENT_RECT_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            GET_CLIENT_RECT_IAT_ADDRESS,
            "GetClientRect",
        );
        let get_window_rect_endpoint = fixture_endpoint(
            228,
            GET_WINDOW_RECT_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            GET_WINDOW_RECT_IAT_ADDRESS,
            "GetWindowRect",
        );
        let get_cursor_info_endpoint = fixture_endpoint(
            236,
            GET_CURSOR_INFO_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            GET_CURSOR_INFO_IAT_ADDRESS,
            "GetCursorInfo",
        );
        let show_cursor_endpoint = fixture_endpoint(
            238,
            SHOW_CURSOR_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            SHOW_CURSOR_IAT_ADDRESS,
            "ShowCursor",
        );
        let peek_message_a_endpoint = fixture_endpoint(
            164,
            PEEK_MESSAGE_A_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            PEEK_MESSAGE_A_IAT_ADDRESS,
            "PeekMessageA",
        );
        let translate_message_endpoint = fixture_endpoint(
            162,
            TRANSLATE_MESSAGE_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            TRANSLATE_MESSAGE_IAT_ADDRESS,
            "TranslateMessage",
        );
        let dispatch_message_a_endpoint = fixture_endpoint(
            182,
            DISPATCH_MESSAGE_A_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            DISPATCH_MESSAGE_A_IAT_ADDRESS,
            "DispatchMessageA",
        );
        let is_iconic_endpoint = fixture_endpoint(
            183,
            IS_ICONIC_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            IS_ICONIC_IAT_ADDRESS,
            "IsIconic",
        );
        let get_dc_endpoint = fixture_endpoint(
            239,
            GET_DC_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            GET_DC_IAT_ADDRESS,
            "GetDC",
        );
        let get_stock_object_endpoint = fixture_endpoint(
            401,
            GET_STOCK_OBJECT_ENDPOINT_ADDRESS,
            runtime_owner,
            "GDI32.dll",
            GET_STOCK_OBJECT_IAT_ADDRESS,
            "GetStockObject",
        );
        let fill_rect_endpoint = fixture_endpoint(
            234,
            FILL_RECT_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            FILL_RECT_IAT_ADDRESS,
            "FillRect",
        );
        let release_dc_endpoint = fixture_endpoint(
            240,
            RELEASE_DC_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            RELEASE_DC_IAT_ADDRESS,
            "ReleaseDC",
        );
        let set_window_long_a_endpoint = fixture_endpoint(
            212,
            SET_WINDOW_LONG_A_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            SET_WINDOW_LONG_A_IAT_ADDRESS,
            "SetWindowLongA",
        );
        let set_timer_endpoint = fixture_endpoint(
            181,
            SET_TIMER_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            SET_TIMER_IAT_ADDRESS,
            "SetTimer",
        );
        let monitor_from_window_endpoint = fixture_endpoint(
            206,
            MONITOR_FROM_WINDOW_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            MONITOR_FROM_WINDOW_IAT_ADDRESS,
            "MonitorFromWindow",
        );
        let get_monitor_info_a_endpoint = fixture_endpoint(
            200,
            GET_MONITOR_INFO_A_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            GET_MONITOR_INFO_A_IAT_ADDRESS,
            "GetMonitorInfoA",
        );
        let get_system_menu_endpoint = fixture_endpoint(
            280,
            GET_SYSTEM_MENU_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            LIVE_GET_SYSTEM_MENU_IAT_ADDRESS,
            "GetSystemMenu",
        );
        let enable_menu_item_endpoint = fixture_endpoint(
            281,
            ENABLE_MENU_ITEM_ENDPOINT_ADDRESS,
            runtime_owner,
            "USER32.dll",
            LIVE_ENABLE_MENU_ITEM_IAT_ADDRESS,
            "EnableMenuItem",
        );
        let dispatcher = ApiDispatcher::from_endpoints([
            icon_endpoint.clone(),
            cursor_endpoint.clone(),
            class_endpoint.clone(),
            create_endpoint.clone(),
            def_window_proc_endpoint.clone(),
            image_endpoint.clone(),
            send_message_endpoint.clone(),
            set_window_text_a_endpoint.clone(),
            imm_get_default_ime_wnd_endpoint.clone(),
            imm_associate_context_endpoint.clone(),
            set_window_pos_endpoint.clone(),
            show_window_endpoint.clone(),
            update_window_endpoint.clone(),
            begin_paint_endpoint.clone(),
            end_paint_endpoint.clone(),
            get_foreground_window_endpoint.clone(),
            set_focus_endpoint.clone(),
            get_client_rect_endpoint.clone(),
            get_window_rect_endpoint.clone(),
            get_cursor_info_endpoint.clone(),
            show_cursor_endpoint.clone(),
            peek_message_a_endpoint.clone(),
            translate_message_endpoint.clone(),
            dispatch_message_a_endpoint.clone(),
            is_iconic_endpoint.clone(),
            get_dc_endpoint.clone(),
            get_stock_object_endpoint.clone(),
            fill_rect_endpoint.clone(),
            release_dc_endpoint.clone(),
            set_window_long_a_endpoint.clone(),
            set_timer_endpoint.clone(),
            monitor_from_window_endpoint.clone(),
            get_monitor_info_a_endpoint.clone(),
            get_system_menu_endpoint.clone(),
            enable_menu_item_endpoint.clone(),
        ])
        .expect("class dispatcher");
        if configure_display {
            runtime
                .configure_display_profile(fixture_display_profile())
                .expect("configure logical display for window placement");
        }
        Self {
            runtime,
            dispatcher,
            icon_endpoint,
            cursor_endpoint,
            class_endpoint,
            create_endpoint,
            def_window_proc_endpoint,
            image_endpoint,
            send_message_endpoint,
            set_window_text_a_endpoint,
            imm_get_default_ime_wnd_endpoint,
            imm_associate_context_endpoint,
            set_window_pos_endpoint,
            show_window_endpoint,
            update_window_endpoint,
            begin_paint_endpoint,
            end_paint_endpoint,
            get_foreground_window_endpoint,
            set_focus_endpoint,
            get_client_rect_endpoint,
            get_window_rect_endpoint,
            get_cursor_info_endpoint,
            show_cursor_endpoint,
            peek_message_a_endpoint,
            translate_message_endpoint,
            dispatch_message_a_endpoint,
            is_iconic_endpoint,
            get_dc_endpoint,
            get_stock_object_endpoint,
            fill_rect_endpoint,
            release_dc_endpoint,
            set_window_long_a_endpoint,
            set_timer_endpoint,
            monitor_from_window_endpoint,
            get_monitor_info_a_endpoint,
            get_system_menu_endpoint,
            enable_menu_item_endpoint,
            window_procedure,
            callback_sentinel,
        }
    }

    fn prepare_resources(&mut self) {
        let icon_endpoint = self.icon_endpoint.clone();
        let icon =
            self.dispatch_endpoint(icon_endpoint, GuestAddress(STACK_POINTER), RETURN_ADDRESS);
        assert!(matches!(
            icon,
            DispatchOutcome::Immediate(ref completion)
                if completion.eax == Some(0x4901_0001)
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
                if completion.eax == Some(0x4301_0001)
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
                if completion.eax == Some(0xc000)
        ));
        let image_endpoint = self.image_endpoint.clone();
        let small_icon = match self.dispatch_endpoint(
            image_endpoint,
            GuestAddress(WINDOW_IMAGE_STACK_POINTER),
            IMAGE_RETURN_ADDRESS,
        ) {
            DispatchOutcome::Immediate(completion) => match completion.eax {
                Some(handle) => handle,
                other => panic!("expected small HICON result, got {other:?}"),
            },
            other => panic!("expected small icon completion, got {other:?}"),
        };
        assert_eq!(small_icon, 0x4901_0002);

        let call = match self.dispatch_create() {
            DispatchOutcome::GuestCallback(call) => call,
            other => panic!("expected logical window creation callback, got {other:?}"),
        };
        let window_handle = self.runtime.windows().values().next().unwrap().handle;
        let mut frame_id = self
            .runtime
            .begin_guest_callback(call.clone())
            .expect("enter first creation callback")
            .frame_id;
        self.acknowledge_pending_delta();
        for (index, result) in [1_u32, 0, 0, 0, 0].into_iter().enumerate() {
            let outcome = self
                .runtime
                .complete_guest_callback(&self.dispatcher, self.callback_sentinel, result)
                .expect("complete creation callback through Runtime");
            match outcome {
                vine_core::GuestCallbackReturn::Continue(prepared) => {
                    assert!(index < 4);
                    assert_ne!(prepared.frame_id, frame_id);
                    frame_id = prepared.frame_id;
                    self.acknowledge_pending_delta();
                }
                vine_core::GuestCallbackReturn::Completed(completion) => {
                    assert_eq!(index, 4);
                    assert_eq!(completion.eax, Some(window_handle));
                    assert!(
                        self.runtime
                            .thread_state(self.runtime.current_thread_id())
                            .unwrap()
                            .callback_frames
                            .is_empty()
                    );
                    break;
                }
                other => panic!("expected window callback continuation, got {other:?}"),
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
        assert_eq!(completion.eax, Some(0));
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

    fn prepare_active_window_with_small_icon(&mut self) -> (u32, u32) {
        let (window_handle, icon_handle) = self.prepare_created_window_with_small_icon();
        assert!(matches!(
            self.dispatch_set_window_long_a(window_handle, -16, 0x90ca_0000),
            DispatchOutcome::Immediate(_)
        ));
        self.configure_set_window_pos(
            [window_handle, 0xffff_fffe, 0, 0, 806, 629, 0x0000_010a],
            SET_WINDOW_POS_ACTIVATE_RETURN_ADDRESS,
        );
        let first = match self.dispatch_set_window_pos(SET_WINDOW_POS_ACTIVATE_RETURN_ADDRESS) {
            DispatchOutcome::GuestCallback(call) => call,
            other => panic!("expected activation callback, got {other:?}"),
        };
        let first_prepared = begin_fixture_callback(self, &first);
        assert!(matches!(
            self.dispatch_def_window_proc(window_handle, 0x0046, first_prepared.scratch_address.0),
            DispatchOutcome::Immediate(_)
        ));
        let (second, second_prepared, _) =
            replace_fixture_callback(self, first, 0, first_prepared.frame_id);
        assert!(matches!(
            self.dispatch_def_window_proc(window_handle, 0x0046, second_prepared.scratch_address.0),
            DispatchOutcome::Immediate(_)
        ));
        let (activate_app, activate_app_prepared, _) =
            replace_fixture_callback(self, second, 0, second_prepared.frame_id);
        assert!(matches!(
            self.dispatch_def_window_proc_args(window_handle, 0x001c, 1, 0),
            DispatchOutcome::Immediate(_)
        ));
        let (nc_activate, nc_activate_prepared, _) =
            replace_fixture_callback(self, activate_app, 0, activate_app_prepared.frame_id);
        assert!(matches!(
            self.dispatch_def_window_proc_args(window_handle, 0x0086, 1, 0),
            DispatchOutcome::Immediate(_)
        ));
        let (activate, activate_prepared, _) =
            replace_fixture_callback(self, nc_activate, 1, nc_activate_prepared.frame_id);
        let ime = match self.dispatch_def_window_proc_args(window_handle, 0x0006, 1, 0) {
            DispatchOutcome::GuestCallback(call) => call,
            other => panic!("expected IME activation callback, got {other:?}"),
        };
        let ime_prepared = begin_fixture_callback(self, &ime);
        let notify = match self.dispatch_def_window_proc_args(window_handle, 0x0281, 1, 0xc000_000f)
        {
            DispatchOutcome::GuestCallback(call) => call,
            other => panic!("expected IME notification callback, got {other:?}"),
        };
        let notify_prepared = begin_fixture_callback(self, &notify);
        assert!(matches!(
            self.dispatch_def_window_proc_args(window_handle, 0x0282, 2, 0),
            DispatchOutcome::Immediate(_)
        ));
        complete_fixture_callback(self, notify, 0, notify_prepared.frame_id);
        let (focus, focus_prepared, _) =
            replace_fixture_callback(self, ime, 0, ime_prepared.frame_id);
        assert!(matches!(
            self.dispatch_def_window_proc_args(window_handle, 0x0007, 0, 0),
            DispatchOutcome::Immediate(_)
        ));
        complete_fixture_callback(self, focus, 0, focus_prepared.frame_id);
        complete_fixture_callback(self, activate, 0, activate_prepared.frame_id);
        assert_eq!(
            self.runtime.active_window_handle().unwrap(),
            Some(window_handle)
        );
        assert_eq!(
            self.runtime.focused_window_handle().unwrap(),
            Some(window_handle)
        );
        assert_eq!(
            self.runtime.foreground_window_handle().unwrap(),
            Some(window_handle)
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
        self.runtime
            .memory
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

    fn dispatch_send_message_basic(&mut self) -> DispatchOutcome {
        self.runtime.dispatch_api_call(
            &self.dispatcher,
            ApiOwnedTrap {
                endpoint_id: self.send_message_endpoint.id,
                endpoint_address: self.send_message_endpoint.address,
                stack_pointer: GuestAddress(SEND_MESSAGE_STACK_POINTER),
                return_address: SEND_MESSAGE_RETURN_ADDRESS,
            },
            self.callback_sentinel,
            None,
        )
    }

    fn dispatch_send_message(&mut self) -> DispatchOutcome {
        self.dispatch_endpoint(
            self.send_message_endpoint.clone(),
            GuestAddress(SEND_MESSAGE_STACK_POINTER),
            SEND_MESSAGE_RETURN_ADDRESS,
        )
    }

    fn configure_set_window_text_a(
        &mut self,
        window_handle: u32,
        title_address: GuestAddress,
        title: &[u8],
    ) {
        self.runtime
            .memory
            .write(title_address, title)
            .expect("write SetWindowTextA title");
        self.write_live_words(
            GuestAddress(SET_WINDOW_TEXT_A_STACK_POINTER),
            &[
                SET_WINDOW_TEXT_A_RETURN_ADDRESS.0,
                window_handle,
                title_address.0,
            ],
        );
        self.acknowledge_pending_delta();
    }

    fn dispatch_set_window_text_a(&mut self) -> DispatchOutcome {
        self.dispatch_endpoint(
            self.set_window_text_a_endpoint.clone(),
            GuestAddress(SET_WINDOW_TEXT_A_STACK_POINTER),
            SET_WINDOW_TEXT_A_RETURN_ADDRESS,
        )
    }

    fn dispatch_imm_get_default_ime_wnd(&mut self, window_handle: u32) -> DispatchOutcome {
        self.runtime
            .memory
            .write(
                GuestAddress(IMM_GET_DEFAULT_IME_WND_STACK_POINTER),
                &[IMM_GET_DEFAULT_IME_WND_RETURN_ADDRESS.0, window_handle]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>(),
            )
            .expect("write ImmGetDefaultIMEWnd frame");
        self.acknowledge_pending_delta();
        self.dispatch_endpoint(
            self.imm_get_default_ime_wnd_endpoint.clone(),
            GuestAddress(IMM_GET_DEFAULT_IME_WND_STACK_POINTER),
            IMM_GET_DEFAULT_IME_WND_RETURN_ADDRESS,
        )
    }

    fn dispatch_imm_associate_context(
        &mut self,
        window_handle: u32,
        context_handle: u32,
    ) -> DispatchOutcome {
        self.runtime
            .memory
            .write(
                GuestAddress(IMM_ASSOCIATE_CONTEXT_STACK_POINTER),
                &[
                    IMM_ASSOCIATE_CONTEXT_RETURN_ADDRESS.0,
                    window_handle,
                    context_handle,
                ]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
            )
            .expect("write ImmAssociateContext frame");
        self.acknowledge_pending_delta();
        self.dispatch_endpoint(
            self.imm_associate_context_endpoint.clone(),
            GuestAddress(IMM_ASSOCIATE_CONTEXT_STACK_POINTER),
            IMM_ASSOCIATE_CONTEXT_RETURN_ADDRESS,
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
        self.runtime
            .memory
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

    fn dispatch_show_window(&mut self, window_handle: u32, command: u32) -> DispatchOutcome {
        self.runtime
            .memory
            .write(
                GuestAddress(SHOW_WINDOW_STACK_POINTER),
                &[SHOW_WINDOW_RETURN_ADDRESS.0, window_handle, command]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>(),
            )
            .expect("write ShowWindow frame");
        self.acknowledge_pending_delta();
        self.dispatch_endpoint(
            self.show_window_endpoint.clone(),
            GuestAddress(SHOW_WINDOW_STACK_POINTER),
            SHOW_WINDOW_RETURN_ADDRESS,
        )
    }

    fn dispatch_update_window(&mut self, window_handle: u32) -> DispatchOutcome {
        self.runtime
            .memory
            .write(
                GuestAddress(UPDATE_WINDOW_STACK_POINTER),
                &[UPDATE_WINDOW_RETURN_ADDRESS.0, window_handle]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>(),
            )
            .expect("write UpdateWindow frame");
        self.acknowledge_pending_delta();
        self.dispatch_endpoint(
            self.update_window_endpoint.clone(),
            GuestAddress(UPDATE_WINDOW_STACK_POINTER),
            UPDATE_WINDOW_RETURN_ADDRESS,
        )
    }

    fn dispatch_begin_paint(
        &mut self,
        window_handle: u32,
        paint_struct: GuestAddress,
    ) -> DispatchOutcome {
        self.runtime
            .memory
            .write(
                GuestAddress(BEGIN_PAINT_STACK_POINTER),
                &[BEGIN_PAINT_RETURN_ADDRESS.0, window_handle, paint_struct.0]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>(),
            )
            .expect("write BeginPaint frame");
        self.acknowledge_pending_delta();
        self.dispatch_endpoint(
            self.begin_paint_endpoint.clone(),
            GuestAddress(BEGIN_PAINT_STACK_POINTER),
            BEGIN_PAINT_RETURN_ADDRESS,
        )
    }

    fn dispatch_end_paint(
        &mut self,
        window_handle: u32,
        paint_struct: GuestAddress,
    ) -> DispatchOutcome {
        self.runtime
            .memory
            .write(
                GuestAddress(END_PAINT_STACK_POINTER),
                &[END_PAINT_RETURN_ADDRESS.0, window_handle, paint_struct.0]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>(),
            )
            .expect("write EndPaint frame");
        self.acknowledge_pending_delta();
        self.dispatch_endpoint(
            self.end_paint_endpoint.clone(),
            GuestAddress(END_PAINT_STACK_POINTER),
            END_PAINT_RETURN_ADDRESS,
        )
    }

    fn dispatch_get_foreground_window(&mut self) -> DispatchOutcome {
        self.write_live_words(
            GuestAddress(GET_FOREGROUND_WINDOW_STACK_POINTER),
            &[GET_FOREGROUND_WINDOW_RETURN_ADDRESS.0],
        );
        self.acknowledge_pending_delta();
        self.dispatch_endpoint(
            self.get_foreground_window_endpoint.clone(),
            GuestAddress(GET_FOREGROUND_WINDOW_STACK_POINTER),
            GET_FOREGROUND_WINDOW_RETURN_ADDRESS,
        )
    }

    fn dispatch_get_foreground_window_basic(&mut self) -> DispatchOutcome {
        self.write_live_words(
            GuestAddress(GET_FOREGROUND_WINDOW_STACK_POINTER),
            &[GET_FOREGROUND_WINDOW_RETURN_ADDRESS.0],
        );
        self.acknowledge_pending_delta();
        self.runtime.dispatch_api_call(
            &self.dispatcher,
            ApiOwnedTrap {
                endpoint_id: self.get_foreground_window_endpoint.id,
                endpoint_address: self.get_foreground_window_endpoint.address,
                stack_pointer: GuestAddress(GET_FOREGROUND_WINDOW_STACK_POINTER),
                return_address: GET_FOREGROUND_WINDOW_RETURN_ADDRESS,
            },
            self.callback_sentinel,
            None,
        )
    }

    fn configure_set_focus(&mut self, window_handle: u32) {
        self.write_live_words(
            GuestAddress(SET_FOCUS_STACK_POINTER),
            &[SET_FOCUS_RETURN_ADDRESS.0, window_handle],
        );
        self.acknowledge_pending_delta();
    }

    fn dispatch_set_focus(&mut self) -> DispatchOutcome {
        self.runtime.dispatch_api_call(
            &self.dispatcher,
            ApiOwnedTrap {
                endpoint_id: self.set_focus_endpoint.id,
                endpoint_address: self.set_focus_endpoint.address,
                stack_pointer: GuestAddress(SET_FOCUS_STACK_POINTER),
                return_address: SET_FOCUS_RETURN_ADDRESS,
            },
            self.callback_sentinel,
            None,
        )
    }

    fn configure_get_client_rect(&mut self, window_handle: u32, output: GuestAddress) {
        let words = [GET_CLIENT_RECT_RETURN_ADDRESS.0, window_handle, output.0];
        self.runtime
            .memory
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

    fn configure_get_window_rect(&mut self, window_handle: u32, output: GuestAddress) {
        let words = [GET_WINDOW_RECT_RETURN_ADDRESS.0, window_handle, output.0];
        self.runtime
            .memory
            .write(
                GuestAddress(GET_WINDOW_RECT_STACK_POINTER),
                &words
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>(),
            )
            .expect("write GetWindowRect frame");
        self.acknowledge_pending_delta();
    }

    fn dispatch_get_window_rect(&mut self) -> DispatchOutcome {
        self.dispatch_endpoint(
            self.get_window_rect_endpoint.clone(),
            GuestAddress(GET_WINDOW_RECT_STACK_POINTER),
            GET_WINDOW_RECT_RETURN_ADDRESS,
        )
    }

    fn configure_get_cursor_info(&mut self, output: GuestAddress) {
        self.write_live_words(
            GuestAddress(GET_CURSOR_INFO_STACK_POINTER),
            &[GET_CURSOR_INFO_RETURN_ADDRESS.0, output.0],
        );
        self.acknowledge_pending_delta();
    }

    fn dispatch_show_cursor(&mut self, show: bool) -> DispatchOutcome {
        self.write_live_words(
            GuestAddress(SHOW_CURSOR_STACK_POINTER),
            &[SHOW_CURSOR_RETURN_ADDRESS.0, u32::from(show)],
        );
        self.acknowledge_pending_delta();
        self.dispatch_endpoint(
            self.show_cursor_endpoint.clone(),
            GuestAddress(SHOW_CURSOR_STACK_POINTER),
            SHOW_CURSOR_RETURN_ADDRESS,
        )
    }

    fn dispatch_get_cursor_info(&mut self) -> DispatchOutcome {
        self.dispatch_endpoint(
            self.get_cursor_info_endpoint.clone(),
            GuestAddress(GET_CURSOR_INFO_STACK_POINTER),
            GET_CURSOR_INFO_RETURN_ADDRESS,
        )
    }

    fn configure_peek_message_a(
        &mut self,
        output: GuestAddress,
        window: u32,
        minimum_message: u32,
        maximum_message: u32,
        remove_message: u32,
    ) {
        self.write_live_words(
            GuestAddress(PEEK_MESSAGE_A_STACK_POINTER),
            &[
                PEEK_MESSAGE_A_RETURN_ADDRESS.0,
                output.0,
                window,
                minimum_message,
                maximum_message,
                remove_message,
            ],
        );
        self.acknowledge_pending_delta();
    }

    fn dispatch_peek_message_a(&mut self) -> DispatchOutcome {
        self.dispatch_endpoint(
            self.peek_message_a_endpoint.clone(),
            GuestAddress(PEEK_MESSAGE_A_STACK_POINTER),
            PEEK_MESSAGE_A_RETURN_ADDRESS,
        )
    }

    fn dispatch_peek_message_a_basic(&mut self) -> DispatchOutcome {
        self.runtime.dispatch_api_call(
            &self.dispatcher,
            ApiOwnedTrap {
                endpoint_id: self.peek_message_a_endpoint.id,
                endpoint_address: self.peek_message_a_endpoint.address,
                stack_pointer: GuestAddress(PEEK_MESSAGE_A_STACK_POINTER),
                return_address: PEEK_MESSAGE_A_RETURN_ADDRESS,
            },
            self.callback_sentinel,
            None,
        )
    }

    fn configure_translate_message(&mut self, message: GuestAddress) {
        self.write_live_words(
            GuestAddress(TRANSLATE_MESSAGE_STACK_POINTER),
            &[TRANSLATE_MESSAGE_RETURN_ADDRESS.0, message.0],
        );
        self.acknowledge_pending_delta();
    }

    fn dispatch_translate_message(&mut self) -> DispatchOutcome {
        self.dispatch_endpoint(
            self.translate_message_endpoint.clone(),
            GuestAddress(TRANSLATE_MESSAGE_STACK_POINTER),
            TRANSLATE_MESSAGE_RETURN_ADDRESS,
        )
    }

    fn dispatch_translate_message_basic(&mut self) -> DispatchOutcome {
        self.runtime.dispatch_api_call(
            &self.dispatcher,
            ApiOwnedTrap {
                endpoint_id: self.translate_message_endpoint.id,
                endpoint_address: self.translate_message_endpoint.address,
                stack_pointer: GuestAddress(TRANSLATE_MESSAGE_STACK_POINTER),
                return_address: TRANSLATE_MESSAGE_RETURN_ADDRESS,
            },
            self.callback_sentinel,
            None,
        )
    }

    fn configure_dispatch_message_a(&mut self, message: GuestAddress) {
        self.write_live_words(
            GuestAddress(DISPATCH_MESSAGE_A_STACK_POINTER),
            &[DISPATCH_MESSAGE_A_RETURN_ADDRESS.0, message.0],
        );
        self.acknowledge_pending_delta();
    }

    fn dispatch_dispatch_message_a_basic(&mut self) -> DispatchOutcome {
        self.runtime.dispatch_api_call(
            &self.dispatcher,
            ApiOwnedTrap {
                endpoint_id: self.dispatch_message_a_endpoint.id,
                endpoint_address: self.dispatch_message_a_endpoint.address,
                stack_pointer: GuestAddress(DISPATCH_MESSAGE_A_STACK_POINTER),
                return_address: DISPATCH_MESSAGE_A_RETURN_ADDRESS,
            },
            self.callback_sentinel,
            None,
        )
    }

    fn dispatch_dispatch_message_a(&mut self) -> DispatchOutcome {
        self.dispatch_endpoint(
            self.dispatch_message_a_endpoint.clone(),
            GuestAddress(DISPATCH_MESSAGE_A_STACK_POINTER),
            DISPATCH_MESSAGE_A_RETURN_ADDRESS,
        )
    }

    fn configure_is_iconic(&mut self, window_handle: u32) {
        self.write_live_words(
            GuestAddress(IS_ICONIC_STACK_POINTER),
            &[IS_ICONIC_RETURN_ADDRESS.0, window_handle],
        );
        self.acknowledge_pending_delta();
    }

    fn dispatch_is_iconic(&mut self) -> DispatchOutcome {
        self.dispatch_endpoint(
            self.is_iconic_endpoint.clone(),
            GuestAddress(IS_ICONIC_STACK_POINTER),
            IS_ICONIC_RETURN_ADDRESS,
        )
    }

    fn dispatch_is_iconic_basic(&mut self) -> DispatchOutcome {
        self.runtime.dispatch_api_call(
            &self.dispatcher,
            ApiOwnedTrap {
                endpoint_id: self.is_iconic_endpoint.id,
                endpoint_address: self.is_iconic_endpoint.address,
                stack_pointer: GuestAddress(IS_ICONIC_STACK_POINTER),
                return_address: IS_ICONIC_RETURN_ADDRESS,
            },
            self.callback_sentinel,
            None,
        )
    }

    fn configure_monitor_from_window(&mut self, window_handle: u32, flags: u32) {
        self.write_live_words(
            GuestAddress(MONITOR_FROM_WINDOW_STACK_POINTER),
            &[MONITOR_FROM_WINDOW_RETURN_ADDRESS.0, window_handle, flags],
        );
        self.acknowledge_pending_delta();
    }

    fn dispatch_monitor_from_window(&mut self) -> DispatchOutcome {
        self.dispatch_endpoint(
            self.monitor_from_window_endpoint.clone(),
            GuestAddress(MONITOR_FROM_WINDOW_STACK_POINTER),
            MONITOR_FROM_WINDOW_RETURN_ADDRESS,
        )
    }

    fn configure_get_monitor_info_a(&mut self, monitor_handle: u32, output: GuestAddress) {
        self.write_live_words(
            GuestAddress(GET_MONITOR_INFO_A_STACK_POINTER),
            &[
                GET_MONITOR_INFO_A_RETURN_ADDRESS.0,
                monitor_handle,
                output.0,
            ],
        );
        self.acknowledge_pending_delta();
    }

    fn initialize_monitor_info_output(
        &mut self,
        output: GuestAddress,
        byte_length: usize,
        structure_byte_length: u32,
    ) -> Vec<u8> {
        let mut bytes = vec![0xa5; byte_length];
        bytes[..4].copy_from_slice(&structure_byte_length.to_le_bytes());
        self.runtime
            .memory
            .write(output, &bytes)
            .expect("initialize GetMonitorInfoA output");
        self.acknowledge_pending_delta();
        bytes
    }

    fn dispatch_get_monitor_info_a(&mut self) -> DispatchOutcome {
        self.dispatch_endpoint(
            self.get_monitor_info_a_endpoint.clone(),
            GuestAddress(GET_MONITOR_INFO_A_STACK_POINTER),
            GET_MONITOR_INFO_A_RETURN_ADDRESS,
        )
    }

    fn dispatch_get_system_menu(&mut self, window_handle: u32, revert: u32) -> DispatchOutcome {
        self.write_live_words(
            GuestAddress(GET_SYSTEM_MENU_STACK_POINTER),
            &[GET_SYSTEM_MENU_RETURN_ADDRESS.0, window_handle, revert],
        );
        self.acknowledge_pending_delta();
        self.dispatch_endpoint(
            self.get_system_menu_endpoint.clone(),
            GuestAddress(GET_SYSTEM_MENU_STACK_POINTER),
            GET_SYSTEM_MENU_RETURN_ADDRESS,
        )
    }

    fn dispatch_enable_menu_item(
        &mut self,
        menu_handle: u32,
        item: u32,
        flags: u32,
    ) -> DispatchOutcome {
        self.write_live_words(
            GuestAddress(ENABLE_MENU_ITEM_STACK_POINTER),
            &[ENABLE_MENU_ITEM_RETURN_ADDRESS.0, menu_handle, item, flags],
        );
        self.acknowledge_pending_delta();
        self.dispatch_endpoint(
            self.enable_menu_item_endpoint.clone(),
            GuestAddress(ENABLE_MENU_ITEM_STACK_POINTER),
            ENABLE_MENU_ITEM_RETURN_ADDRESS,
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

    fn dispatch_set_timer_basic(
        &mut self,
        window_handle: u32,
        timer_id: u32,
        interval_milliseconds: u32,
        callback: GuestAddress,
    ) -> DispatchOutcome {
        self.write_live_words(
            GuestAddress(SET_TIMER_STACK_POINTER),
            &[
                SET_TIMER_RETURN_ADDRESS.0,
                window_handle,
                timer_id,
                interval_milliseconds,
                callback.0,
            ],
        );
        self.acknowledge_pending_delta();
        self.runtime.dispatch_api_call(
            &self.dispatcher,
            ApiOwnedTrap {
                endpoint_id: self.set_timer_endpoint.id,
                endpoint_address: self.set_timer_endpoint.address,
                stack_pointer: GuestAddress(SET_TIMER_STACK_POINTER),
                return_address: SET_TIMER_RETURN_ADDRESS,
            },
            self.callback_sentinel,
            None,
        )
    }

    fn dispatch_set_timer(
        &mut self,
        window_handle: u32,
        timer_id: u32,
        interval_milliseconds: u32,
        callback: GuestAddress,
    ) -> DispatchOutcome {
        self.write_live_words(
            GuestAddress(SET_TIMER_STACK_POINTER),
            &[
                SET_TIMER_RETURN_ADDRESS.0,
                window_handle,
                timer_id,
                interval_milliseconds,
                callback.0,
            ],
        );
        self.acknowledge_pending_delta();
        self.dispatch_endpoint(
            self.set_timer_endpoint.clone(),
            GuestAddress(SET_TIMER_STACK_POINTER),
            SET_TIMER_RETURN_ADDRESS,
        )
    }

    fn write_live_words(&mut self, address: GuestAddress, words: &[u32]) {
        self.runtime
            .memory
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
        self.dispatch_def_window_proc_args(handle, message, 0, l_param)
    }

    fn dispatch_def_window_proc_args(
        &mut self,
        handle: u32,
        message: u32,
        w_param: u32,
        l_param: u32,
    ) -> DispatchOutcome {
        self.configure_def_window_proc_args(handle, message, w_param, l_param);
        self.dispatch_configured_def_window_proc()
    }

    fn configure_def_window_proc_args(
        &mut self,
        handle: u32,
        message: u32,
        w_param: u32,
        l_param: u32,
    ) {
        let words = [
            DEF_WINDOW_PROC_RETURN_ADDRESS.0,
            handle,
            message,
            w_param,
            l_param,
        ];
        let bytes = words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        self.runtime
            .memory
            .write(GuestAddress(DEF_WINDOW_PROC_STACK_POINTER), &bytes)
            .expect("write nested DefWindowProcA frame");
        self.acknowledge_pending_delta();
    }

    fn dispatch_configured_def_window_proc(&mut self) -> DispatchOutcome {
        self.dispatch_endpoint(
            self.def_window_proc_endpoint.clone(),
            GuestAddress(DEF_WINDOW_PROC_STACK_POINTER),
            DEF_WINDOW_PROC_RETURN_ADDRESS,
        )
    }

    fn acknowledge_pending_delta(&mut self) {
        if self.runtime.memory.stats().pending_invalidation.is_none() {
            return;
        }
        let delta = self
            .runtime
            .memory
            .pending_delta()
            .expect("pending User32 delta");
        self.runtime
            .memory
            .acknowledge_delta(delta.generation)
            .expect("acknowledge User32 delta");
    }

    fn dispatch_endpoint(
        &mut self,
        endpoint: ApiEndpointIdentity,
        stack_pointer: GuestAddress,
        return_address: GuestAddress,
    ) -> DispatchOutcome {
        self.runtime.dispatch_api_call(
            &self.dispatcher,
            ApiOwnedTrap {
                endpoint_id: endpoint.id,
                endpoint_address: endpoint.address,
                stack_pointer,
                return_address,
            },
            self.callback_sentinel,
            None,
        )
    }
}

fn begin_fixture_callback(
    fixture: &mut RegisterClassFixture,
    call: &ApiGuestCallbackCall,
) -> PreparedGuestCallback {
    let prepared = fixture
        .runtime
        .begin_guest_callback(call.clone())
        .expect("enter fixture callback");
    fixture.acknowledge_pending_delta();
    prepared
}

fn replace_fixture_callback(
    fixture: &mut RegisterClassFixture,
    call: ApiGuestCallbackCall,
    result: u32,
    frame_id: RuntimeCallbackFrameId,
) -> (
    ApiGuestCallbackCall,
    PreparedGuestCallback,
    RuntimeGuestCallbackSemanticAction,
) {
    let completion_policy = call.request.completion_policy;
    let endpoint = call.frame.endpoint.clone();
    let next = {
        match fixture
            .dispatcher
            .complete_guest_callback_result(call, result, &mut fixture.runtime)
        {
            DispatchOutcome::GuestCallback(next) => next,
            other => panic!("expected replacement callback, got {other:?}"),
        }
    };
    let (prepared, action) = {
        let callback_runtime = &mut fixture.runtime;
        let prepared = callback_runtime
            .replace_guest_callback(next.clone(), frame_id)
            .expect("replace fixture callback");
        let action = callback_runtime
            .commit_guest_callback_semantics(completion_policy, result, &endpoint)
            .expect("commit replacement callback semantics")
            .expect("replacement callback has semantic continuation");
        (prepared, action)
    };
    fixture.acknowledge_pending_delta();
    (next, prepared, action)
}

fn complete_fixture_callback(
    fixture: &mut RegisterClassFixture,
    call: ApiGuestCallbackCall,
    result: u32,
    frame_id: RuntimeCallbackFrameId,
) -> (
    ApiImmediateCompletion,
    Option<RuntimeGuestCallbackSemanticAction>,
) {
    let completion_policy = call.request.completion_policy;
    let endpoint = call.frame.endpoint.clone();
    let completion = {
        match fixture
            .dispatcher
            .complete_guest_callback_result(call, result, &mut fixture.runtime)
        {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected completed callback, got {other:?}"),
        }
    };
    let action = {
        let callback_runtime = &mut fixture.runtime;
        callback_runtime
            .pop_callback_frame(frame_id)
            .expect("retire fixture callback");
        callback_runtime
            .commit_guest_callback_semantics(completion_policy, result, &endpoint)
            .expect("commit completed callback semantics")
    };
    fixture.acknowledge_pending_delta();
    (completion, action)
}

fn assert_send_message_fault_is_atomic(fixture: &mut RegisterClassFixture) {
    let window_before = fixture.runtime.windows().values().next().cloned();
    let owner = fixture.runtime.current_thread_id();
    let queue_before = fixture.runtime.queued_thread_messages(owner).unwrap();
    let scheduler_before = fixture.runtime.scheduler_state().unwrap();
    let last_error_before = fixture.runtime.current_last_error().unwrap();
    let memory_before = fixture.runtime.memory.stats();
    let reflection_before = fixture
        .runtime
        .memory
        .current_reflection()
        .expect("current SendMessageA reflection");
    match fixture.dispatch_send_message() {
        DispatchOutcome::Fault(fault) => fault,
        DispatchOutcome::GuestCallback(call) => fixture
            .runtime
            .begin_guest_callback(call)
            .expect_err("SendMessageA callback installation fault"),
        other => panic!("expected SendMessageA fault, got {other:?}"),
    };

    if let Some(window) = window_before {
        assert_eq!(fixture.runtime.window(window.handle).unwrap(), &window);
        assert_eq!(fixture.runtime.windows().len(), 1);
    } else {
        assert!(fixture.runtime.windows().is_empty());
    }
    assert_eq!(fixture.runtime.current_thread_id(), owner);
    assert_eq!(
        fixture.runtime.queued_thread_messages(owner).unwrap(),
        queue_before
    );
    assert_eq!(fixture.runtime.scheduler_state().unwrap(), scheduler_before);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );
    assert_eq!(fixture.runtime.memory.stats(), memory_before);
    assert_eq!(
        fixture
            .runtime
            .memory
            .current_reflection()
            .expect("unchanged SendMessageA reflection"),
        reflection_before
    );
}

#[test]
fn screen_dc_acquisition_and_release_preserve_null_window_native_lifetime() {
    let mut fixture = RegisterClassFixture::new();
    let sentinel = 0x5566_7788;
    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed screen GetDC LastError");

    let first = match fixture.dispatch_get_dc(0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected first screen GetDC completion, got {other:?}"),
    };
    let first_handle = match first.eax {
        Some(handle) => handle,
        other => panic!("screen GetDC did not set EAX: {other:?}"),
    };
    assert_eq!(first_handle, 0x4701_0001);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert!(matches!(
        fixture
            .runtime
            .gdi_device_context(first_handle)
            .unwrap()
            .target,
        RuntimeDeviceContextTarget::Screen {
            monitor_handle: RUNTIME_PRIMARY_MONITOR_HANDLE,
            rect: RuntimeRect {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            },
        }
    ));
    assert!(fixture.runtime.gdi_surfaces().is_empty());

    let second = match fixture.dispatch_get_dc(0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected second screen GetDC completion, got {other:?}"),
    };
    let second_handle = match second.eax {
        Some(handle) => handle,
        other => panic!("second screen GetDC did not set EAX: {other:?}"),
    };
    assert_eq!(second_handle, 0x4701_0002);
    assert_ne!(first_handle, second_handle);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert!(fixture.runtime.gdi_surfaces().is_empty());

    let released = match fixture.dispatch_release_dc(0, first_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected screen ReleaseDC completion, got {other:?}"),
    };
    assert_eq!(released.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert!(fixture.runtime.gdi_device_context(first_handle).is_err());
    assert!(fixture.runtime.gdi_device_context(second_handle).is_ok());

    let contexts_before_repeat = fixture.runtime.gdi_device_contexts().clone();
    let repeated = match fixture.dispatch_release_dc(0, first_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected repeated screen ReleaseDC completion, got {other:?}"),
    };
    assert_eq!(repeated.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(
        fixture.runtime.gdi_device_contexts(),
        &contexts_before_repeat
    );

    let released_second = match fixture.dispatch_release_dc(0, second_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected second screen ReleaseDC completion, got {other:?}"),
    };
    assert_eq!(released_second.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert!(fixture.runtime.gdi_device_contexts().is_empty());
    assert!(fixture.runtime.gdi_surfaces().is_empty());
}

#[test]
fn synchronous_user32_calls_preserve_last_error_from_prior_windows_failures() {
    let mut fixture = RegisterClassFixture::new();
    let (window, _) = fixture.prepare_created_window_and_small_icon();
    fixture.runtime.set_current_last_error(0x1357_2468).unwrap();
    let DispatchOutcome::Immediate(invalid) = fixture.dispatch_get_dc(0x4801_ffff) else {
        panic!("invalid HWND did not return a Windows failure")
    };
    assert_eq!(invalid.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
    let DispatchOutcome::Immediate(acquired) = fixture.dispatch_get_dc(0) else {
        panic!("screen GetDC failed")
    };
    let Some(hdc) = acquired.eax else {
        panic!("GetDC did not return HDC")
    };
    assert_ne!(hdc, 0);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
    let DispatchOutcome::Immediate(mismatch) = fixture.dispatch_release_dc(window, hdc) else {
        panic!("mismatched ReleaseDC did not return a Windows failure")
    };
    assert_eq!(mismatch.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1425);
    let DispatchOutcome::Immediate(released) = fixture.dispatch_release_dc(0, hdc) else {
        panic!("screen ReleaseDC failed")
    };
    assert_eq!(released.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1425);
    let DispatchOutcome::Immediate(repeated) = fixture.dispatch_release_dc(0, hdc) else {
        panic!("repeated screen ReleaseDC stopped")
    };
    assert_eq!(repeated.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1425);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1425);
}

#[test]
fn release_dc_validates_handle_target_and_thread_without_consuming_another_dc() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, _) = fixture.prepare_created_window_and_small_icon();
    let window_dc = match fixture.dispatch_get_dc(window_handle) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            Some(handle) => handle,
            other => panic!("window GetDC did not return an HDC: {other:?}"),
        },
        other => panic!("window GetDC stopped: {other:?}"),
    };
    let screen_dc = match fixture.dispatch_get_dc(0) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            Some(handle) => handle,
            other => panic!("screen GetDC did not return an HDC: {other:?}"),
        },
        other => panic!("screen GetDC stopped: {other:?}"),
    };
    let window_dc_before = fixture
        .runtime
        .gdi_device_context(window_dc)
        .unwrap()
        .clone();
    let screen_dc_before = fixture
        .runtime
        .gdi_device_context(screen_dc)
        .unwrap()
        .clone();
    let sentinel = 0x2938_4756;
    for (hwnd, hdc, expected_error) in [
        (window_handle, screen_dc, 1425),
        (0, window_dc, sentinel),
        (window_handle, 0x4701_ffff, 1425),
    ] {
        fixture.runtime.set_current_last_error(sentinel).unwrap();
        let result = fixture.dispatch_release_dc(hwnd, hdc);
        let DispatchOutcome::Immediate(completion) = result else {
            panic!("mismatched ReleaseDC stopped: {result:?}");
        };
        assert_eq!(completion.eax, Some(0));
        assert_eq!(
            fixture.runtime.current_last_error().unwrap(),
            expected_error
        );
        assert_eq!(
            fixture.runtime.current_last_error().unwrap(),
            expected_error
        );
        assert_eq!(
            fixture.runtime.current_thread_id(),
            RuntimeThreadId(THREAD_ID)
        );
        assert_eq!(
            fixture.runtime.gdi_device_context(window_dc).unwrap(),
            &window_dc_before
        );
        assert_eq!(
            fixture.runtime.gdi_device_context(screen_dc).unwrap(),
            &screen_dc_before
        );
        assert_eq!(fixture.runtime.gdi_device_contexts().len(), 2);
    }
    fixture
        .runtime
        .register_thread(RuntimeThreadId(THREAD_ID + 1), sentinel)
        .unwrap();
    fixture
        .runtime
        .select_thread(RuntimeThreadId(THREAD_ID + 1))
        .unwrap();
    for (hwnd, hdc, expected_error) in [(window_handle, window_dc, 1425), (0, screen_dc, sentinel)]
    {
        fixture.runtime.set_current_last_error(sentinel).unwrap();
        let result = fixture.dispatch_release_dc(hwnd, hdc);
        let DispatchOutcome::Immediate(completion) = result else {
            panic!("cross-thread ReleaseDC stopped: {result:?}");
        };
        assert_eq!(completion.eax, Some(0));
        assert_eq!(
            fixture.runtime.current_last_error().unwrap(),
            expected_error
        );
        assert_eq!(
            fixture.runtime.current_last_error().unwrap(),
            expected_error
        );
        assert_eq!(
            fixture.runtime.current_thread_id(),
            RuntimeThreadId(THREAD_ID + 1)
        );
        assert_eq!(
            fixture.runtime.gdi_device_context(window_dc).unwrap(),
            &window_dc_before
        );
        assert_eq!(
            fixture.runtime.gdi_device_context(screen_dc).unwrap(),
            &screen_dc_before
        );
        assert_eq!(fixture.runtime.gdi_device_contexts().len(), 2);
    }
    fixture
        .runtime
        .select_thread(RuntimeThreadId(THREAD_ID))
        .unwrap();
    for (hwnd, hdc) in [(window_handle, window_dc), (0, screen_dc)] {
        assert!(
            matches!(fixture.dispatch_release_dc(hwnd, hdc), DispatchOutcome::Immediate(ref completion)
            if completion.eax == Some(1))
        );
    }
}

#[test]
fn screen_dc_uses_live_monitor_without_profile_identity_matching() {
    let mut fixture = RegisterClassFixture::without_display();
    let last_error_before = fixture.runtime.current_last_error().unwrap();
    assert!(matches!(
        fixture.dispatch_get_dc(0),
        DispatchOutcome::Fault(_)
    ));
    assert!(fixture.runtime.primary_monitor().is_none());
    assert!(fixture.runtime.gdi_device_contexts().is_empty());
    assert!(fixture.runtime.gdi_surfaces().is_empty());
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );

    fixture
        .runtime
        .configure_display_profile(RuntimeDisplayProfile {
            primary_monitor_rect: RuntimeRect {
                left: 0,
                top: 0,
                right: 1280,
                bottom: 720,
            },
            primary_work_rect: RuntimeRect {
                left: 0,
                top: 0,
                right: 1280,
                bottom: 680,
            },
        })
        .unwrap();
    fixture.runtime.set_current_last_error(0x1234).unwrap();
    let acquired = match fixture.dispatch_get_dc(0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected screen DC for live alternate monitor, got {other:?}"),
    };
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x1234);
    let Some(handle) = acquired.eax else {
        panic!("missing HDC")
    };
    assert_eq!(handle, 0x4701_0001);
    assert!(matches!(
        fixture.runtime.gdi_device_context(handle).unwrap().target,
        RuntimeDeviceContextTarget::Screen {
            rect: RuntimeRect {
                right: 1280,
                bottom: 720,
                ..
            },
            ..
        }
    ));
    let released = match fixture.dispatch_release_dc(0, handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected screen DC release, got {other:?}"),
    };
    assert_eq!(released.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x1234);
}

#[test]
fn live_system_menu_identity_is_window_owned_reused_and_failure_atomic() {
    let mut fixture = RegisterClassFixture::new();
    let (window, _) = fixture.prepare_created_window_and_small_icon();
    let sentinel = 0xa5a5_5a5a;
    fixture.runtime.set_current_last_error(sentinel).unwrap();
    let DispatchOutcome::Immediate(first) = fixture.dispatch_get_system_menu(window, 0) else {
        panic!("GetSystemMenu failed")
    };
    let menu = first.eax.unwrap();
    assert_ne!(menu, 0);
    let DispatchOutcome::Immediate(repeated) = fixture.dispatch_get_system_menu(window, 0) else {
        panic!("repeat GetSystemMenu failed")
    };
    assert_eq!(repeated.eax, Some(menu));
    let DispatchOutcome::Immediate(enabled) = fixture.dispatch_enable_menu_item(menu, 0xf060, 0)
    else {
        panic!("EnableMenuItem failed")
    };
    assert_eq!(enabled.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    for (item, flags) in [(0xf061, 0), (0xf060, 1)] {
        assert!(matches!(
            fixture.dispatch_enable_menu_item(menu, item, flags),
            DispatchOutcome::Fault(_)
        ));
        assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    }
    let DispatchOutcome::Immediate(invalid) =
        fixture.dispatch_get_system_menu(window.wrapping_add(1), 0)
    else {
        panic!("invalid HWND did not return")
    };
    assert_eq!(invalid.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
    let DispatchOutcome::Immediate(enabled) = fixture.dispatch_enable_menu_item(menu, 0xf060, 0)
    else {
        panic!("failed call consumed the live menu")
    };
    assert_eq!(enabled.eax, Some(0));
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
        .runtime
        .memory
        .write(CLIENT_RECT_OUTPUT_ADDRESS, &[0xa5; 16])
        .expect("seed client RECT output");
    fixture.acknowledge_pending_delta();
    fixture.configure_get_client_rect(window_handle, CLIENT_RECT_OUTPUT_ADDRESS);
    let window_id = fixture.runtime.window(window_handle).unwrap().id;
    let geometry_before = fixture
        .runtime
        .window(window_handle)
        .unwrap()
        .creation
        .geometry;
    let sentinel = 0x1122_3344;
    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed GetClientRect LastError");
    let completion = match fixture.dispatch_get_client_rect() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected GetClientRect completion, got {other:?}"),
    };
    assert_eq!(completion.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(CLIENT_RECT_OUTPUT_ADDRESS, 16)
            .expect("read completed client RECT"),
        expected_bytes
    );
    assert_eq!(fixture.runtime.window(window_handle).unwrap().id, window_id);
    assert_eq!(
        fixture
            .runtime
            .window(window_handle)
            .unwrap()
            .creation
            .geometry,
        geometry_before
    );

    fixture
        .runtime
        .memory
        .write(CLIENT_RECT_OUTPUT_ADDRESS, &[0xa5; 16])
        .expect("reset failed client RECT output");
    fixture.acknowledge_pending_delta();
    fixture.configure_get_client_rect(0, CLIENT_RECT_OUTPUT_ADDRESS);
    let memory_before_invalid_handle = fixture.runtime.memory.stats();

    let invalid_handle = match fixture.dispatch_get_client_rect() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid-HWND completion, got {other:?}"),
    };
    assert_eq!(invalid_handle.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
    assert_eq!(fixture.runtime.memory.stats(), memory_before_invalid_handle);
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(CLIENT_RECT_OUTPUT_ADDRESS, 16)
            .expect("read preserved invalid-HWND output"),
        [0xa5; 16]
    );
    assert_eq!(fixture.runtime.window(window_handle).unwrap().id, window_id);
    assert_eq!(
        fixture
            .runtime
            .window(window_handle)
            .unwrap()
            .creation
            .geometry,
        geometry_before
    );

    let unmapped_output = GuestAddress(MEMORY_BASE.0 + 0x3000);
    fixture.configure_get_client_rect(window_handle, unmapped_output);
    let last_error_before = fixture.runtime.current_last_error().unwrap();
    let pointer_fault_memory = fixture.runtime.memory.stats();
    match fixture.dispatch_get_client_rect() {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected client-RECT pointer fault, got {other:?}"),
    };

    assert_eq!(fixture.runtime.window(window_handle).unwrap().id, window_id);
    assert_eq!(
        fixture
            .runtime
            .window(window_handle)
            .unwrap()
            .creation
            .geometry,
        geometry_before
    );
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );
    assert_eq!(fixture.runtime.memory.stats(), pointer_fault_memory);
    fixture.configure_get_client_rect(window_handle, CLIENT_RECT_OUTPUT_ADDRESS);
    let retried = match fixture.dispatch_get_client_rect() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected valid client RECT retry, got {other:?}"),
    };
    assert_eq!(retried.eax, Some(1));
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(CLIENT_RECT_OUTPUT_ADDRESS, 16)
            .unwrap(),
        expected_bytes
    );
    assert_eq!(fixture.runtime.window(window_handle).unwrap().id, window_id);
    assert_eq!(
        fixture
            .runtime
            .window(window_handle)
            .unwrap()
            .creation
            .geometry,
        geometry_before
    );
}

#[test]
fn get_window_placement_reads_normal_geometry_and_checks_output_length() {
    let mut fixture = RegisterClassFixture::new();
    let (window, _) = fixture.prepare_created_window_and_small_icon();
    fixture.get_window_rect_endpoint.symbol = crate::api::ApiSymbol::Name {
        name: "GetWindowPlacement".to_owned(),
    };
    fixture.dispatcher =
        ApiDispatcher::from_endpoints([fixture.get_window_rect_endpoint.clone()]).unwrap();
    fixture.configure_get_window_rect(window, WINDOW_RECT_OUTPUT_ADDRESS);
    let mut output = [0xa5; 44];
    output[..4].copy_from_slice(&44_u32.to_le_bytes());
    fixture
        .runtime
        .memory
        .write(WINDOW_RECT_OUTPUT_ADDRESS, &output)
        .unwrap();
    fixture.acknowledge_pending_delta();
    let last_error = fixture.runtime.current_last_error().unwrap();
    assert!(matches!(fixture.dispatch_get_window_rect(),
        DispatchOutcome::Immediate(ref completion) if completion.eax == Some(1)));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), last_error);
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(WINDOW_RECT_OUTPUT_ADDRESS, 44)
            .unwrap(),
        [44_i32, 0, 1, -1, -1, -1, -1, 557, 214, 1363, 843]
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    fixture
        .runtime
        .memory
        .write(WINDOW_RECT_OUTPUT_ADDRESS, &43_u32.to_le_bytes())
        .unwrap();
    fixture.acknowledge_pending_delta();
    let before = fixture
        .runtime
        .memory
        .read(WINDOW_RECT_OUTPUT_ADDRESS, 44)
        .unwrap();
    assert!(matches!(fixture.dispatch_get_window_rect(),
        DispatchOutcome::Immediate(ref completion) if completion.eax == Some(0)));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 87);
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(WINDOW_RECT_OUTPUT_ADDRESS, 44)
            .unwrap(),
        before
    );
}

#[test]
fn get_window_rect_reads_live_outer_geometry_and_preserves_failure_atomicity() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, _) = fixture.prepare_created_window_and_small_icon();
    let expected_bytes = [557_i32, 214, 1_363, 843]
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect::<Vec<_>>();
    fixture
        .runtime
        .memory
        .write(WINDOW_RECT_OUTPUT_ADDRESS, &[0xa5; 16])
        .expect("seed window RECT output");
    fixture.acknowledge_pending_delta();
    fixture.configure_get_window_rect(window_handle, WINDOW_RECT_OUTPUT_ADDRESS);
    let window_id = fixture.runtime.window(window_handle).unwrap().id;
    let geometry_before = fixture
        .runtime
        .window(window_handle)
        .unwrap()
        .creation
        .geometry;
    let sentinel = 0x1122_3344;
    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed GetWindowRect LastError");
    let completion = match fixture.dispatch_get_window_rect() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected GetWindowRect completion, got {other:?}"),
    };
    assert_eq!(completion.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(WINDOW_RECT_OUTPUT_ADDRESS, 16)
            .expect("read completed window RECT"),
        expected_bytes
    );
    assert_eq!(fixture.runtime.window(window_handle).unwrap().id, window_id);
    assert_eq!(
        fixture
            .runtime
            .window(window_handle)
            .unwrap()
            .creation
            .geometry,
        geometry_before
    );

    fixture
        .runtime
        .memory
        .write(WINDOW_RECT_OUTPUT_ADDRESS, &[0xa5; 16])
        .expect("reset failed window RECT output");
    fixture.acknowledge_pending_delta();
    fixture.configure_get_window_rect(0, WINDOW_RECT_OUTPUT_ADDRESS);
    let memory_before_invalid_handle = fixture.runtime.memory.stats();

    let invalid_handle = match fixture.dispatch_get_window_rect() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid-HWND completion, got {other:?}"),
    };
    assert_eq!(invalid_handle.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
    assert_eq!(fixture.runtime.memory.stats(), memory_before_invalid_handle);
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(WINDOW_RECT_OUTPUT_ADDRESS, 16)
            .expect("read preserved invalid-HWND output"),
        [0xa5; 16]
    );
    assert_eq!(fixture.runtime.window(window_handle).unwrap().id, window_id);
    assert_eq!(
        fixture
            .runtime
            .window(window_handle)
            .unwrap()
            .creation
            .geometry,
        geometry_before
    );

    let unmapped_output = GuestAddress(MEMORY_BASE.0 + 0x3000);
    fixture.configure_get_window_rect(window_handle, unmapped_output);
    let last_error_before = fixture.runtime.current_last_error().unwrap();
    let pointer_fault_memory = fixture.runtime.memory.stats();
    match fixture.dispatch_get_window_rect() {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected window-RECT pointer fault, got {other:?}"),
    };

    assert_eq!(fixture.runtime.window(window_handle).unwrap().id, window_id);
    assert_eq!(
        fixture
            .runtime
            .window(window_handle)
            .unwrap()
            .creation
            .geometry,
        geometry_before
    );
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );
    assert_eq!(fixture.runtime.memory.stats(), pointer_fault_memory);
    fixture.configure_get_window_rect(window_handle, WINDOW_RECT_OUTPUT_ADDRESS);
    let retried = match fixture.dispatch_get_window_rect() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected valid window RECT retry, got {other:?}"),
    };
    assert_eq!(retried.eax, Some(1));
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(WINDOW_RECT_OUTPUT_ADDRESS, 16)
            .unwrap(),
        expected_bytes
    );
    assert_eq!(fixture.runtime.window(window_handle).unwrap().id, window_id);
    assert_eq!(
        fixture
            .runtime
            .window(window_handle)
            .unwrap()
            .creation
            .geometry,
        geometry_before
    );
}

#[test]
fn get_cursor_info_projects_loaded_arrow_and_logical_position() {
    let mut fixture = RegisterClassFixture::new();
    fixture.prepare_resources();
    let mut input = vec![0xa5; 20];
    input[0..4].copy_from_slice(&20_u32.to_le_bytes());
    fixture
        .runtime
        .memory
        .write(CURSOR_INFO_OUTPUT_ADDRESS, &input)
        .expect("seed CURSORINFO output");
    fixture.acknowledge_pending_delta();
    fixture.configure_get_cursor_info(CURSOR_INFO_OUTPUT_ADDRESS);
    let sentinel = 0x4455_6677;
    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed GetCursorInfo LastError");
    let cursor_before = fixture.runtime.cursor_position().cloned();
    let handle_before = fixture
        .runtime
        .current_cursor()
        .unwrap()
        .map(|cursor| cursor.handle);
    let display_count_before = fixture
        .runtime
        .current_thread_cursor_display_count()
        .unwrap();
    let completion = match fixture.dispatch_get_cursor_info() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected GetCursorInfo completion, got {other:?}"),
    };
    // cbSize is caller-owned input that the API echoes, flags follow the ShowCursor display count
    // (>= 0 on a freshly bootstrapped thread, so CURSOR_SHOWING).
    let expected_bytes = [20_u32, 1, 0x4301_0001]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .chain([960_i32, 540].into_iter().flat_map(i32::to_le_bytes))
        .collect::<Vec<_>>();
    assert_eq!(completion.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(
        completion.stack_pointer,
        GuestAddress(GET_CURSOR_INFO_STACK_POINTER + 8)
    );
    assert_eq!(
        completion.instruction_pointer,
        GET_CURSOR_INFO_RETURN_ADDRESS
    );
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(CURSOR_INFO_OUTPUT_ADDRESS, 20)
            .expect("read completed CURSORINFO"),
        expected_bytes
    );
    assert_eq!(fixture.runtime.cursor_position(), cursor_before.as_ref());
    assert_eq!(
        fixture
            .runtime
            .current_cursor()
            .unwrap()
            .map(|cursor| cursor.handle),
        handle_before
    );
    assert_eq!(
        fixture
            .runtime
            .current_thread_cursor_display_count()
            .unwrap(),
        display_count_before
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
}

#[test]
fn set_cursor_drives_the_cursor_identity_reported_by_get_cursor_info() {
    const SET_CURSOR_ENDPOINT: GuestAddress = GuestAddress(0x7f00_0ab0);
    const SET_CURSOR_STACK: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x23a0);
    const SET_CURSOR_RETURN: GuestAddress = GuestAddress(0x1008_8ca0);

    let mut fixture = RegisterClassFixture::new();
    fixture.prepare_resources();
    assert_eq!(
        fixture
            .runtime
            .current_cursor()
            .expect("initial current cursor")
            .map(|cursor| cursor.handle),
        Some(0x4301_0001)
    );

    let endpoint = fixture_endpoint(
        239,
        SET_CURSOR_ENDPOINT,
        MemoryOwnerId(1),
        "USER32.dll",
        GuestAddress(IAT_ADDRESS.0 + 152),
        "SetCursor",
    );

    fixture
        .runtime
        .memory
        .write(
            GuestAddress(IAT_ADDRESS.0 + 152),
            &SET_CURSOR_ENDPOINT.0.to_le_bytes(),
        )
        .expect("initialize SetCursor IAT");
    fixture.acknowledge_pending_delta();
    let dispatcher =
        ApiDispatcher::from_endpoints([endpoint.clone()]).expect("SetCursor dispatcher");
    let dispatch_set_cursor = |fixture: &mut RegisterClassFixture, handle: u32| {
        fixture.write_live_words(SET_CURSOR_STACK, &[SET_CURSOR_RETURN.0, handle]);
        fixture.acknowledge_pending_delta();
        dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: endpoint.id,
                endpoint_address: endpoint.address,
                stack_pointer: SET_CURSOR_STACK,
                return_address: SET_CURSOR_RETURN,
            },
            &mut fixture.runtime,
            None,
            |frame, runtime, _cpu| dispatch_user32_api(CALLBACK_SENTINEL, frame, runtime),
        )
    };
    let sentinel = 0x4455_6677;
    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed SetCursor LastError");

    let cleared = match dispatch_set_cursor(&mut fixture, 0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected SetCursor(NULL) completion, got {other:?}"),
    };
    assert_eq!(cleared.eax, Some(0x4301_0001));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(fixture.runtime.current_cursor().unwrap(), None);

    let mut cursor_info = vec![0xa5; 20];
    cursor_info[0..4].copy_from_slice(&20_u32.to_le_bytes());
    fixture
        .runtime
        .memory
        .write(CURSOR_INFO_OUTPUT_ADDRESS, &cursor_info)
        .expect("seed hidden CURSORINFO");
    fixture.acknowledge_pending_delta();
    fixture.configure_get_cursor_info(CURSOR_INFO_OUTPUT_ADDRESS);
    let hidden = match fixture.dispatch_get_cursor_info() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected hidden current-cursor info, got {other:?}"),
    };
    assert_eq!(hidden.eax, Some(1));
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(GuestAddress(CURSOR_INFO_OUTPUT_ADDRESS.0 + 4), 8)
            .unwrap(),
        [0; 8]
    );

    let selected = match dispatch_set_cursor(&mut fixture, 0x4301_0001) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected live SetCursor completion, got {other:?}"),
    };
    assert_eq!(selected.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(
        fixture
            .runtime
            .current_cursor()
            .unwrap()
            .map(|cursor| cursor.handle),
        Some(0x4301_0001)
    );

    let position_before = fixture.runtime.cursor_position().cloned();
    let display_count_before = fixture
        .runtime
        .current_thread_cursor_display_count()
        .unwrap();
    let invalid = match dispatch_set_cursor(&mut fixture, 0x4301_ffff) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid SetCursor completion, got {other:?}"),
    };
    assert_eq!(invalid.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(
        fixture
            .runtime
            .current_cursor()
            .unwrap()
            .map(|cursor| cursor.handle),
        Some(0x4301_0001)
    );
    assert_eq!(fixture.runtime.cursor_position(), position_before.as_ref());
    assert_eq!(
        fixture
            .runtime
            .current_thread_cursor_display_count()
            .unwrap(),
        display_count_before
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
}

#[test]
fn get_cursor_info_flags_follow_the_show_cursor_display_count() {
    fn seed_cursor_info(fixture: &mut RegisterClassFixture) {
        let mut input = vec![0xa5; 20];
        input[0..4].copy_from_slice(&20_u32.to_le_bytes());
        fixture
            .runtime
            .memory
            .write(CURSOR_INFO_OUTPUT_ADDRESS, &input)
            .expect("seed CURSORINFO output");
        fixture.acknowledge_pending_delta();
        fixture.configure_get_cursor_info(CURSOR_INFO_OUTPUT_ADDRESS);
    }

    let mut fixture = RegisterClassFixture::new();
    fixture.prepare_resources();

    // ShowCursor(FALSE) drives the per-thread display count negative, which hides the cursor and
    // must clear CURSOR_SHOWING in the reported flags.
    match fixture.dispatch_show_cursor(false) {
        DispatchOutcome::Immediate(completion) => {
            assert_eq!(completion.eax, Some((-1_i32) as u32));
        }
        other => panic!("expected ShowCursor completion, got {other:?}"),
    }
    seed_cursor_info(&mut fixture);
    let hidden = match fixture.dispatch_get_cursor_info() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected GetCursorInfo completion, got {other:?}"),
    };
    assert_eq!(hidden.eax, Some(1));
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(GuestAddress(CURSOR_INFO_OUTPUT_ADDRESS.0 + 4), 4)
            .unwrap(),
        [0, 0, 0, 0]
    );

    // Restoring the count to zero makes the cursor visible again.
    match fixture.dispatch_show_cursor(true) {
        DispatchOutcome::Immediate(completion) => {
            assert_eq!(completion.eax, Some(0));
        }
        other => panic!("expected ShowCursor completion, got {other:?}"),
    }
    seed_cursor_info(&mut fixture);
    let shown = match fixture.dispatch_get_cursor_info() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected GetCursorInfo completion, got {other:?}"),
    };
    assert_eq!(shown.eax, Some(1));
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(GuestAddress(CURSOR_INFO_OUTPUT_ADDRESS.0 + 4), 4)
            .unwrap(),
        [1, 0, 0, 0]
    );
}

#[test]
fn get_cursor_info_failures_preserve_output_and_cursor_authorities() {
    let mut fixture = RegisterClassFixture::new();
    fixture.prepare_resources();

    fixture.configure_get_cursor_info(GuestAddress(0));
    let memory_before_null = fixture.runtime.memory.stats();
    let cursors_before_null = fixture.runtime.cursors().clone();
    let cursor_position_before_null = fixture.runtime.cursor_position().cloned();
    let null_completion = match fixture.dispatch_get_cursor_info() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected null GetCursorInfo completion, got {other:?}"),
    };
    assert_eq!(null_completion.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 998);
    assert_eq!(fixture.runtime.memory.stats(), memory_before_null);
    assert_eq!(fixture.runtime.cursors(), &cursors_before_null);
    assert_eq!(
        fixture.runtime.cursor_position(),
        cursor_position_before_null.as_ref()
    );

    for cb_size in [0_u32, 19, 21] {
        let mut bytes = vec![0xa5; 20];
        bytes[0..4].copy_from_slice(&cb_size.to_le_bytes());
        fixture
            .runtime
            .memory
            .write(CURSOR_INFO_OUTPUT_ADDRESS, &bytes)
            .expect("seed invalid CURSORINFO size");
        fixture.acknowledge_pending_delta();
        fixture.configure_get_cursor_info(CURSOR_INFO_OUTPUT_ADDRESS);
        let cursors_before = fixture.runtime.cursors().clone();
        let cursor_position_before = fixture.runtime.cursor_position().cloned();
        let completion = match fixture.dispatch_get_cursor_info() {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected invalid-size GetCursorInfo completion, got {other:?}"),
        };
        assert_eq!(completion.eax, Some(0));
        assert_eq!(fixture.runtime.current_last_error().unwrap(), 87);
        assert_eq!(
            fixture
                .runtime
                .memory
                .read(CURSOR_INFO_OUTPUT_ADDRESS, 20)
                .expect("read preserved invalid CURSORINFO"),
            bytes
        );
        assert_eq!(fixture.runtime.cursors(), &cursors_before);
        assert_eq!(
            fixture.runtime.cursor_position(),
            cursor_position_before.as_ref()
        );
    }

    let partial_output = GuestAddress(MEMORY_BASE.0 + 0x2ffc);
    fixture
        .runtime
        .memory
        .write(partial_output, &19_u32.to_le_bytes())
        .expect("seed partial invalid CURSORINFO");
    fixture.acknowledge_pending_delta();
    fixture.configure_get_cursor_info(partial_output);
    let partial_size_completion = match fixture.dispatch_get_cursor_info() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected partial invalid-size completion, got {other:?}"),
    };
    assert_eq!(partial_size_completion.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 87);
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(partial_output, 4)
            .expect("read preserved partial size"),
        19_u32.to_le_bytes()
    );

    fixture
        .runtime
        .memory
        .write(partial_output, &20_u32.to_le_bytes())
        .expect("seed partial valid CURSORINFO");
    fixture.acknowledge_pending_delta();
    fixture.configure_get_cursor_info(partial_output);
    let cursors_before_fault = fixture.runtime.cursors().clone();
    let cursor_position_before_fault = fixture.runtime.cursor_position().cloned();
    match fixture.dispatch_get_cursor_info() {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected CURSORINFO write-span fault, got {other:?}"),
    };

    assert_eq!(
        fixture
            .runtime
            .memory
            .read(partial_output, 4)
            .expect("read preserved partial valid size"),
        20_u32.to_le_bytes()
    );
    assert_eq!(fixture.runtime.cursors(), &cursors_before_fault);
    assert_eq!(
        fixture.runtime.cursor_position(),
        cursor_position_before_fault.as_ref()
    );

    let mut missing = RegisterClassFixture::new();
    let mut valid = vec![0xa5; 20];
    valid[0..4].copy_from_slice(&20_u32.to_le_bytes());
    missing
        .runtime
        .memory
        .write(CURSOR_INFO_OUTPUT_ADDRESS, &valid)
        .expect("seed missing-cursor CURSORINFO");
    missing.acknowledge_pending_delta();
    missing.configure_get_cursor_info(CURSOR_INFO_OUTPUT_ADDRESS);
    let no_current_cursor = match missing.dispatch_get_cursor_info() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected no-current-cursor completion, got {other:?}"),
    };
    assert_eq!(no_current_cursor.eax, Some(1));
    missing.acknowledge_pending_delta();
    assert_eq!(
        missing
            .runtime
            .memory
            .read(CURSOR_INFO_OUTPUT_ADDRESS, 12)
            .unwrap(),
        &[20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    assert!(missing.runtime.cursors().is_empty());

    let mut disabled = RegisterClassFixture::new();
    disabled.prepare_resources();
    disabled
        .runtime
        .memory
        .write(CURSOR_INFO_OUTPUT_ADDRESS, &valid)
        .expect("seed disabled-policy CURSORINFO");
    disabled.acknowledge_pending_delta();
    disabled.configure_get_cursor_info(CURSOR_INFO_OUTPUT_ADDRESS);
    let disabled_completion = match disabled.dispatch_get_cursor_info() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected policy-independent GetCursorInfo completion, got {other:?}"),
    };
    assert_eq!(disabled_completion.eax, Some(1));
    disabled.acknowledge_pending_delta();
    assert_eq!(
        disabled
            .runtime
            .memory
            .read(GuestAddress(CURSOR_INFO_OUTPUT_ADDRESS.0 + 4), 4)
            .unwrap(),
        [1, 0, 0, 0]
    );
    assert_eq!(
        disabled
            .runtime
            .memory
            .read(GuestAddress(CURSOR_INFO_OUTPUT_ADDRESS.0 + 8), 4)
            .unwrap(),
        0x4301_0001_u32.to_le_bytes()
    );
}

#[test]
fn peek_message_a_preserves_empty_output_and_commits_remove_with_guest_write_atomically() {
    let mut fixture = RegisterClassFixture::new();
    fixture
        .runtime
        .memory
        .write(PEEK_MESSAGE_A_OUTPUT_ADDRESS, &[0xa5; 28])
        .expect("seed PeekMessageA output");
    fixture.acknowledge_pending_delta();
    fixture.configure_peek_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 0, 0, 0, 1);
    let sentinel = 0x1122_3344;
    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed PeekMessageA LastError");
    let empty = match fixture.dispatch_peek_message_a() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected empty PeekMessageA completion, got {other:?}"),
    };
    assert_eq!(empty.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
            .expect("read preserved empty PeekMessageA output"),
        [0xa5; 28]
    );

    let callback = fixture
        .runtime
        .push_callback_frame(CALLBACK_TARGET, CALLBACK_SENTINEL)
        .expect("enter callback for reentrant PeekMessageA");
    fixture.configure_peek_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 0, 0, 0, 1);
    let callback_empty = match fixture.dispatch_peek_message_a() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected callback PeekMessageA completion, got {other:?}"),
    };
    assert_eq!(callback_empty.eax, Some(0));
    assert_eq!(
        fixture.runtime.scheduler_state().unwrap(),
        SchedulerThreadState::InCallback { depth: 1 }
    );
    fixture
        .runtime
        .pop_callback_frame(callback.id)
        .expect("leave callback after reentrant PeekMessageA");

    let posted = fixture
        .runtime
        .post_thread_message(
            RuntimeThreadId(THREAD_ID),
            0,
            0x8001,
            0x2233_4455,
            0x6677_8899,
        )
        .expect("post controlled thread message");
    let expected_bytes = posted.to_x86_msg_bytes();
    fixture.configure_peek_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 0, 0, 0, 0);
    let no_remove = match fixture.dispatch_peek_message_a() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected PM_NOREMOVE completion, got {other:?}"),
    };
    assert_eq!(no_remove.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
            .unwrap(),
        expected_bytes
    );
    assert_eq!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .expect("inspect retained message"),
        vec![posted]
    );
    fixture.acknowledge_pending_delta();

    fixture.configure_peek_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 0, 0, 0, 1);
    let remove = match fixture.dispatch_peek_message_a() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected PM_REMOVE completion, got {other:?}"),
    };
    assert_eq!(remove.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
            .expect("read committed MSG output"),
        expected_bytes
    );
    assert!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .expect("inspect consumed message")
            .is_empty()
    );

    let rejected_message = fixture
        .runtime
        .post_thread_message(RuntimeThreadId(THREAD_ID), 0, 0x8002, 1, 2)
        .expect("post message for rejected variants");
    let output_before_rejection = fixture
        .runtime
        .memory
        .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
        .expect("read output before rejection");
    fixture.configure_peek_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 1, 0, 0, 1);
    match fixture.dispatch_peek_message_a() {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected PeekMessageA filter fault, got {other:?}"),
    };

    assert_eq!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .expect("inspect message after filter rejection"),
        vec![rejected_message]
    );
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
            .expect("read output after filter rejection"),
        output_before_rejection
    );

    let unmapped_output = GuestAddress(MEMORY_BASE.0 + 0x3000);
    fixture.configure_peek_message_a(unmapped_output, 0, 0, 0, 1);
    let memory_before_pointer_fault = fixture.runtime.memory.stats();
    match fixture.dispatch_peek_message_a() {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected PeekMessageA pointer fault, got {other:?}"),
    };

    assert_eq!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .unwrap(),
        vec![rejected_message]
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(
        fixture.runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(fixture.runtime.memory.stats(), memory_before_pointer_fault);
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
            .unwrap(),
        output_before_rejection
    );
    fixture.configure_peek_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 0, 0, 0, 1);
    let retried = match fixture.dispatch_peek_message_a() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected queued MSG retry, got {other:?}"),
    };
    assert_eq!(retried.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .unwrap()
            .is_empty()
    );
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
            .unwrap(),
        rejected_message.to_x86_msg_bytes()
    );
}

#[test]
fn due_window_timer_materializes_once_and_dispatches_timerproc_on_the_owner_thread() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, _) = fixture.prepare_created_window_and_small_icon();
    let callback = fixture.window_procedure;
    let sentinel = 0x55aa_1234;
    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed timer delivery LastError");
    let registration = match fixture.dispatch_set_timer(window_handle, 1, 20, callback) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected SetTimer completion, got {other:?}"),
    };
    assert_eq!(registration.eax, Some(1));
    let timer = *fixture.runtime.window_timers().values().next().unwrap();
    assert_eq!(timer.next_deadline_time_100ns, 200_000);

    fixture
        .runtime
        .advance_scheduled_clock_to(200_000)
        .expect("advance logical clock to window timer deadline");
    assert_eq!(fixture.runtime.timer_time_100ns(), 200_000);

    fixture.configure_peek_message_a(GuestAddress(MEMORY_BASE.0 + 0x3000), 0, 0, 0, 1);
    let timer_before_pointer_fault = *fixture.runtime.window_timers().values().next().unwrap();
    let memory_before_pointer_fault = fixture.runtime.memory.stats();
    match fixture.dispatch_peek_message_a() {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected due timer MSG pointer fault, got {other:?}"),
    };

    assert_eq!(
        fixture.runtime.window_timers().values().next(),
        Some(&timer_before_pointer_fault)
    );
    assert_eq!(fixture.runtime.timer_time_100ns(), 200_000);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .unwrap()
            .is_empty()
    );
    assert_eq!(fixture.runtime.memory.stats(), memory_before_pointer_fault);

    fixture.configure_peek_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 0, 0, 0, 1);
    let peek = match fixture.dispatch_peek_message_a() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected due window timer message, got {other:?}"),
    };
    assert_eq!(peek.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    fixture.acknowledge_pending_delta();
    let message_bytes = fixture
        .runtime
        .memory
        .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
        .unwrap();
    let message_words = message_bytes
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(message_words.len(), 7);
    assert_eq!(&message_words[..4], &[window_handle, 0x0113, 1, callback.0]);
    assert_eq!(message_words[4], 3_600_020);
    assert_eq!(message_words[5], 960);
    assert_eq!(message_words[6], 540);
    assert!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .expect("inspect removed timer message")
            .is_empty()
    );
    let delivered = *fixture.runtime.window_timers().values().next().unwrap();
    assert_eq!(delivered.delivery_count, 1);
    assert_eq!(delivered.next_deadline_time_100ns, 400_000);
    assert_eq!(
        delivered.last_delivery.unwrap().delivered_at_time_100ns,
        200_000
    );
    fixture.acknowledge_pending_delta();

    fixture.configure_translate_message(PEEK_MESSAGE_A_OUTPUT_ADDRESS);
    let translated = match fixture.dispatch_translate_message() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected timer TranslateMessage completion, got {other:?}"),
    };
    assert_eq!(translated.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);

    let translated_basic = fixture.dispatch_translate_message_basic();
    assert!(
        matches!(translated_basic, DispatchOutcome::Immediate(ref completion)
        if completion.eax == Some(0) && fixture.runtime.current_last_error().unwrap() == sentinel)
    );
    fixture.configure_dispatch_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS);
    let basic_call = fixture.dispatch_dispatch_message_a_basic();
    assert!(matches!(basic_call, DispatchOutcome::GuestCallback(_)));
    let scheduler_before_dispatch_entry = fixture.runtime.scheduler_state().unwrap();
    let memory_before_dispatch_entry = fixture.runtime.memory.stats();
    let message_before_dispatch_entry = fixture
        .runtime
        .memory
        .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
        .expect("read TIMERPROC MSG before DispatchMessageA entry");
    let call = match fixture.dispatch_dispatch_message_a() {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected TIMERPROC callback, got {other:?}"),
    };
    assert_eq!(
        fixture.runtime.scheduler_state().unwrap(),
        scheduler_before_dispatch_entry
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(
        *fixture.runtime.window_timers().values().next().unwrap(),
        delivered
    );
    assert!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .unwrap()
            .is_empty()
    );
    assert_eq!(fixture.runtime.memory.stats(), memory_before_dispatch_entry);
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
            .expect("read TIMERPROC MSG after DispatchMessageA entry"),
        message_before_dispatch_entry
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .expect("inspect timer queue at DispatchMessageA entry")
            .is_empty()
    );
    assert_eq!(call.request.target, callback);
    assert_eq!(
        call.request.arguments,
        [window_handle, 0x0113, 1, message_words[4]]
            .into_iter()
            .map(GuestCallbackArgument::U32)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        call.request.completion_policy,
        GuestCallbackCompletionPolicy::ReturnConstantU32 { value: 0 }
    );
    assert_eq!(call.request.return_sentinel, fixture.callback_sentinel);
    let prepared = begin_fixture_callback(&mut fixture, &call);
    assert_eq!(prepared.frame_bytes, 20);
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(prepared.stack_pointer, 20)
            .expect("read TIMERPROC frame"),
        [
            fixture.callback_sentinel.0,
            window_handle,
            0x0113,
            1,
            message_words[4],
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>()
    );
    let (completion, semantic_action) =
        complete_fixture_callback(&mut fixture, call, 0xdead_beef, prepared.frame_id);
    assert_eq!(semantic_action, None);
    assert_eq!(completion.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(
        completion.instruction_pointer,
        DISPATCH_MESSAGE_A_RETURN_ADDRESS
    );
    assert_eq!(
        completion.stack_pointer,
        GuestAddress(DISPATCH_MESSAGE_A_STACK_POINTER + 8)
    );

    fixture.configure_peek_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 0, 0, 0, 1);
    let coalesced = match fixture.dispatch_peek_message_a() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected coalesced timer queue to be empty, got {other:?}"),
    };
    assert_eq!(coalesced.eax, Some(0));
    assert_eq!(
        fixture
            .runtime
            .window_timers()
            .values()
            .next()
            .unwrap()
            .delivery_count,
        1
    );
}

#[test]
fn pointer_input_before_a_foreground_window_has_no_guest_message_and_cannot_skip_a_live_target() {
    let mut fixture = RegisterClassFixture::new();
    fixture.runtime.skip_unroutable_pointer_event(1).unwrap();
    assert!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        fixture
            .runtime
            .keyboard_state()
            .unwrap()
            .last_event_sequence,
        1
    );
    let (window_handle, _) = fixture.prepare_active_window_with_small_icon();
    let owner = fixture.runtime.current_thread_id();
    let cursor_before = fixture.runtime.cursor_position().cloned();
    let messages_before = fixture.runtime.queued_thread_messages(owner).unwrap();
    let last_error_before = fixture.runtime.current_last_error().unwrap();
    assert!(fixture.runtime.skip_unroutable_pointer_event(2).is_err());
    assert_eq!(
        fixture.runtime.foreground_window_handle().unwrap(),
        Some(window_handle)
    );
    assert_eq!(fixture.runtime.cursor_position(), cursor_before.as_ref());
    assert_eq!(
        fixture
            .runtime
            .keyboard_state()
            .unwrap()
            .last_event_sequence,
        1
    );
    assert_eq!(
        fixture.runtime.queued_thread_messages(owner).unwrap(),
        messages_before
    );
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );
    let accepted = fixture
        .runtime
        .ingest_pointer_position_event(RuntimePointerPositionSourceEvent {
            sequence: 2,
            normalized_x: 32_768,
            normalized_y: 32_768,
        })
        .expect("rejected skip must leave the same sequence available for real input");
    assert_eq!(accepted.hwnd, window_handle);
    assert_eq!(
        fixture
            .runtime
            .keyboard_state()
            .unwrap()
            .last_event_sequence,
        2
    );
    assert_eq!(
        fixture
            .runtime
            .queued_thread_messages(owner)
            .unwrap()
            .last(),
        Some(&accepted)
    );
}

#[test]
fn pointer_position_ingress_dispatches_mouse_move_and_preserves_shared_sequence() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, _) = fixture.prepare_active_window_with_small_icon();
    let sentinel = 0x41aa_7263;
    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed pointer move LastError");

    let position = fixture
        .runtime
        .ingest_pointer_position_event(RuntimePointerPositionSourceEvent {
            sequence: 1,
            normalized_x: 16_384,
            normalized_y: 49_152,
        })
        .expect("ingest pointer position");
    assert_eq!(position.hwnd, window_handle);
    assert_eq!(position.message, 0x0200);
    assert_eq!(position.w_param, 0);
    assert_eq!(position.l_param, 0x01c2_00c8);
    assert_eq!((position.point_x, position.point_y), (760, 690));
    assert_eq!(
        fixture
            .runtime
            .keyboard_state()
            .unwrap()
            .last_event_sequence,
        1
    );

    let cursor_before = fixture.runtime.cursor_position().unwrap().clone();
    let queue_before = fixture
        .runtime
        .queued_thread_messages(RuntimeThreadId(THREAD_ID))
        .unwrap();
    let selected_before = fixture.runtime.current_thread_id();
    let stale = fixture
        .runtime
        .ingest_pointer_position_event(RuntimePointerPositionSourceEvent {
            sequence: 1,
            normalized_x: 0,
            normalized_y: 0,
        })
        .expect_err("duplicate pointer position sequence must be rejected");
    let _ = stale;
    assert_eq!(fixture.runtime.cursor_position().unwrap(), &cursor_before);
    assert_eq!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .unwrap(),
        queue_before
    );
    assert_eq!(
        fixture
            .runtime
            .keyboard_state()
            .unwrap()
            .last_event_sequence,
        1
    );
    assert_eq!(fixture.runtime.current_thread_id(), selected_before);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);

    fixture.configure_peek_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 0, 0, 0, 1);
    let peek = match fixture.dispatch_peek_message_a() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected pointer-move PeekMessageA completion, got {other:?}"),
    };
    assert_eq!(peek.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
            .unwrap(),
        position.to_x86_msg_bytes()
    );
    fixture.acknowledge_pending_delta();

    fixture.configure_translate_message(PEEK_MESSAGE_A_OUTPUT_ADDRESS);
    let translated = match fixture.dispatch_translate_message() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected pointer-move TranslateMessage completion, got {other:?}"),
    };
    assert_eq!(translated.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert!(
        matches!(fixture.dispatch_translate_message_basic(), DispatchOutcome::Immediate(ref completion)
        if completion.eax == Some(0) && fixture.runtime.current_last_error().unwrap() == sentinel)
    );

    fixture.configure_dispatch_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS);
    assert!(matches!(
        fixture.dispatch_dispatch_message_a_basic(),
        DispatchOutcome::GuestCallback(_)
    ));
    let call = match fixture.dispatch_dispatch_message_a() {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected pointer-move window-procedure callback, got {other:?}"),
    };
    assert_eq!(
        call.request.arguments,
        [window_handle, 0x0200, 0, position.l_param]
            .into_iter()
            .map(GuestCallbackArgument::U32)
            .collect::<Vec<_>>()
    );
    let prepared = begin_fixture_callback(&mut fixture, &call);
    let (completion, semantic_action) =
        complete_fixture_callback(&mut fixture, call, 0x1020_3040, prepared.frame_id);
    assert_eq!(semantic_action, None);
    assert_eq!(completion.eax, Some(0x1020_3040));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);

    let down = fixture
        .runtime
        .ingest_pointer_button_event(RuntimePointerButtonSourceEvent {
            sequence: 2,
            kind: RuntimePointerButtonSourceEventKind::ButtonDown,
            button: RuntimePointerButton::Primary,
            normalized_x: 16_384,
            normalized_y: 49_152,
        })
        .expect("button event must continue the position event sequence");
    assert_eq!(down.message, 0x0201);
    assert_eq!(down.w_param, 0x0001);
}

#[test]
fn pointer_button_ingress_commits_input_queue_and_window_proc_dispatch_atomically() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, _) = fixture.prepare_active_window_with_small_icon();
    let sentinel = 0x55aa_4321;
    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed pointer delivery LastError");

    let down = fixture
        .runtime
        .ingest_pointer_button_event(RuntimePointerButtonSourceEvent {
            sequence: 1,
            kind: RuntimePointerButtonSourceEventKind::ButtonDown,
            button: RuntimePointerButton::Primary,
            normalized_x: 32_768,
            normalized_y: 32_768,
        })
        .expect("ingest primary pointer down");
    assert_eq!(down.hwnd, window_handle);
    assert_eq!(down.target_thread_id, RuntimeThreadId(THREAD_ID));
    assert_eq!(down.message, 0x0201);
    assert_eq!(down.w_param, 0x0001);
    assert_eq!(down.l_param, 0x012c_0190);
    assert_eq!((down.point_x, down.point_y), (960, 540));
    assert_eq!(
        fixture
            .runtime
            .query_async_key_state(0x01)
            .unwrap()
            .unwrap(),
        0x8001_u16 as i16
    );

    fixture.configure_peek_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 0, 0, 0, 1);
    let peek = match fixture.dispatch_peek_message_a() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected pointer PeekMessageA completion, got {other:?}"),
    };
    assert_eq!(peek.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
            .unwrap(),
        down.to_x86_msg_bytes()
    );
    fixture.acknowledge_pending_delta();

    fixture.configure_translate_message(PEEK_MESSAGE_A_OUTPUT_ADDRESS);
    let translated = match fixture.dispatch_translate_message() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected pointer TranslateMessage completion, got {other:?}"),
    };
    assert_eq!(translated.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);

    fixture.configure_dispatch_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS);
    let call = match fixture.dispatch_dispatch_message_a() {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected pointer window-procedure callback, got {other:?}"),
    };
    assert_eq!(call.request.target, fixture.window_procedure);
    assert_eq!(
        call.request.arguments,
        [window_handle, 0x0201, 0x0001, 0x012c_0190]
            .into_iter()
            .map(GuestCallbackArgument::U32)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        call.request.completion_policy,
        GuestCallbackCompletionPolicy::ReturnU32
    );
    let prepared = begin_fixture_callback(&mut fixture, &call);
    let (completion, semantic_action) =
        complete_fixture_callback(&mut fixture, call, 0x1234_5678, prepared.frame_id);
    assert_eq!(semantic_action, None);
    assert_eq!(completion.eax, Some(0x1234_5678));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);

    let up = fixture
        .runtime
        .ingest_pointer_button_event(RuntimePointerButtonSourceEvent {
            sequence: 2,
            kind: RuntimePointerButtonSourceEventKind::ButtonUp,
            button: RuntimePointerButton::Primary,
            normalized_x: 32_768,
            normalized_y: 32_768,
        })
        .expect("ingest primary pointer up");
    assert_eq!(up.message, 0x0202);
    assert_eq!(up.w_param, 0);
    assert_eq!(up.l_param, 0x012c_0190);
    assert_eq!(
        fixture
            .runtime
            .query_async_key_state(0x01)
            .unwrap()
            .unwrap(),
        0
    );

    fixture.configure_peek_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 0, 0, 0, 1);
    let release_peek = match fixture.dispatch_peek_message_a() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected pointer-release PeekMessageA completion, got {other:?}"),
    };
    assert_eq!(release_peek.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
            .unwrap(),
        up.to_x86_msg_bytes()
    );
    fixture.acknowledge_pending_delta();
    assert!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .unwrap()
            .is_empty()
    );
    fixture.configure_translate_message(PEEK_MESSAGE_A_OUTPUT_ADDRESS);
    let window_before_release = fixture.runtime.window(window_handle).unwrap().clone();
    let selected_before_release = fixture.runtime.current_thread_id();
    let sequence_before_release = fixture
        .runtime
        .keyboard_state()
        .unwrap()
        .last_event_sequence;
    let message_before_release_translate = fixture
        .runtime
        .memory
        .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
        .unwrap()
        .to_vec();
    let release_translated = match fixture.dispatch_translate_message() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected pointer-release TranslateMessage completion, got {other:?}"),
    };
    assert_eq!(release_translated.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(
        fixture.runtime.window(window_handle).unwrap(),
        &window_before_release
    );
    assert_eq!(fixture.runtime.current_thread_id(), selected_before_release);
    assert_eq!(
        fixture
            .runtime
            .keyboard_state()
            .unwrap()
            .last_event_sequence,
        sequence_before_release
    );
    assert!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .unwrap()
            .is_empty()
    );
    assert!(
        fixture
            .runtime
            .thread_state(selected_before_release)
            .unwrap()
            .callback_frames
            .is_empty()
    );
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
            .unwrap(),
        message_before_release_translate
    );

    fixture.configure_dispatch_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS);
    let message_before_release_dispatch = fixture
        .runtime
        .memory
        .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
        .unwrap()
        .to_vec();
    let release_call = match fixture.dispatch_dispatch_message_a() {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected pointer-release window-procedure callback, got {other:?}"),
    };
    assert_eq!(
        fixture.runtime.window(window_handle).unwrap(),
        &window_before_release
    );
    assert_eq!(fixture.runtime.current_thread_id(), selected_before_release);
    assert_eq!(
        fixture
            .runtime
            .keyboard_state()
            .unwrap()
            .last_event_sequence,
        sequence_before_release
    );
    assert!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .unwrap()
            .is_empty()
    );
    assert!(
        fixture
            .runtime
            .thread_state(selected_before_release)
            .unwrap()
            .callback_frames
            .is_empty()
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
            .unwrap(),
        message_before_release_dispatch
    );
    assert_eq!(release_call.request.target, fixture.window_procedure);
    assert_eq!(
        release_call.request.arguments,
        [window_handle, 0x0202, 0, up.l_param]
            .into_iter()
            .map(GuestCallbackArgument::U32)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        release_call.request.completion_policy,
        GuestCallbackCompletionPolicy::ReturnU32
    );
    assert_eq!(
        release_call.request.return_sentinel,
        fixture.callback_sentinel
    );

    let release_prepared = begin_fixture_callback(&mut fixture, &release_call);
    assert_eq!(
        release_prepared.allocation_start,
        release_prepared.stack_pointer
    );
    assert_eq!(release_prepared.scratch_bytes, 0);
    assert_eq!(release_prepared.frame_bytes, 20);
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(release_prepared.stack_pointer, 20)
            .expect("read pointer-release window-procedure frame"),
        [
            fixture.callback_sentinel.0,
            window_handle,
            0x0202,
            0,
            up.l_param,
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>()
    );
    assert_eq!(
        fixture
            .runtime
            .current_thread_state()
            .unwrap()
            .callback_frames
            .len(),
        1
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
            .unwrap(),
        message_before_release_dispatch
    );

    let (release_completion, release_semantic_action) = complete_fixture_callback(
        &mut fixture,
        release_call,
        0x89ab_cdef,
        release_prepared.frame_id,
    );
    assert_eq!(release_semantic_action, None);
    assert_eq!(release_completion.eax, Some(0x89ab_cdef));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(
        release_completion.instruction_pointer,
        DISPATCH_MESSAGE_A_RETURN_ADDRESS
    );
    assert_eq!(
        release_completion.stack_pointer,
        GuestAddress(DISPATCH_MESSAGE_A_STACK_POINTER + 8)
    );
    assert!(
        fixture
            .runtime
            .current_thread_state()
            .unwrap()
            .callback_frames
            .is_empty()
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
            .unwrap(),
        message_before_release_dispatch
    );

    for message in 0..4_096 {
        fixture
            .runtime
            .post_thread_message(RuntimeThreadId(THREAD_ID), 0, 0x8000 + message, 0, 0)
            .expect("fill pointer target queue");
    }
    let queue_before = fixture
        .runtime
        .queued_thread_messages(RuntimeThreadId(THREAD_ID))
        .unwrap();
    let cursor_before = fixture.runtime.cursor_position().unwrap().clone();
    let sequence_before = fixture
        .runtime
        .keyboard_state()
        .unwrap()
        .last_event_sequence;
    let selected_before = fixture.runtime.current_thread_id();
    let fault = fixture
        .runtime
        .ingest_pointer_button_event(RuntimePointerButtonSourceEvent {
            sequence: 3,
            kind: RuntimePointerButtonSourceEventKind::ButtonDown,
            button: RuntimePointerButton::Primary,
            normalized_x: 0,
            normalized_y: 0,
        })
        .expect_err("full queue must reject pointer input before mutation");
    let _ = fault;
    assert_eq!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .unwrap(),
        queue_before
    );
    assert_eq!(fixture.runtime.cursor_position().unwrap(), &cursor_before);
    assert_eq!(
        fixture
            .runtime
            .keyboard_state()
            .unwrap()
            .last_event_sequence,
        sequence_before
    );
    assert_eq!(fixture.runtime.current_thread_id(), selected_before);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    fixture.runtime.receive_current_thread_message().unwrap();
    let retry = fixture
        .runtime
        .ingest_pointer_button_event(RuntimePointerButtonSourceEvent {
            sequence: 3,
            kind: RuntimePointerButtonSourceEventKind::ButtonDown,
            button: RuntimePointerButton::Primary,
            normalized_x: 0,
            normalized_y: 0,
        })
        .expect("rejected sequence remains available after freeing queue capacity");
    let queue = fixture
        .runtime
        .queued_thread_messages(RuntimeThreadId(THREAD_ID))
        .unwrap();
    assert_eq!(&queue[..4095], &queue_before[1..]);
    assert_eq!(queue[4095], retry);
    assert_eq!(retry.message, 0x0201);
    assert_eq!(retry.w_param, 1);
    assert_eq!(retry.l_param, 0);
    assert_eq!(
        fixture
            .runtime
            .keyboard_state()
            .unwrap()
            .last_event_sequence,
        3
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
}

#[test]
fn pointer_button_ingress_emits_double_click_for_registered_class_style() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, _) = fixture.prepare_active_window_with_small_icon();
    let sentinel = 0x62aa_3517;
    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed double-click LastError");
    let window = fixture.runtime.window(window_handle).unwrap();
    assert_ne!(
        fixture
            .runtime
            .window_class(window.creation.class.atom)
            .unwrap()
            .registration
            .style
            & 0x0008,
        0,
        "fixture must exercise a CS_DBLCLKS window class"
    );

    let mut ingest = |sequence, kind| {
        fixture
            .runtime
            .ingest_pointer_button_event(RuntimePointerButtonSourceEvent {
                sequence,
                kind,
                button: RuntimePointerButton::Primary,
                normalized_x: 32_768,
                normalized_y: 32_768,
            })
            .unwrap()
            .message
    };
    assert_eq!(
        ingest(1, RuntimePointerButtonSourceEventKind::ButtonDown),
        0x0201
    );
    assert_eq!(
        ingest(2, RuntimePointerButtonSourceEventKind::ButtonUp),
        0x0202
    );
    assert_eq!(
        ingest(3, RuntimePointerButtonSourceEventKind::ButtonDown),
        0x0203
    );
    assert_eq!(
        ingest(4, RuntimePointerButtonSourceEventKind::ButtonUp),
        0x0202
    );
    assert_eq!(
        ingest(5, RuntimePointerButtonSourceEventKind::ButtonDown),
        0x0201,
        "the click after a completed double-click starts a fresh sequence"
    );

    let mut double_click_l_param = 0;
    for expected_message in [0x0201, 0x0202, 0x0203] {
        fixture.configure_peek_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 0, 0, 0, 1);
        let peek = match fixture.dispatch_peek_message_a() {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected queued click message, got {other:?}"),
        };
        assert_eq!(peek.eax, Some(1));
        assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
        fixture.acknowledge_pending_delta();
        let words = fixture
            .runtime
            .memory
            .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
            .unwrap()
            .chunks_exact(4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(words[1], expected_message);
        if expected_message == 0x0203 {
            assert_eq!(words[2], 0x0001);
            double_click_l_param = words[3];
        }
        fixture.acknowledge_pending_delta();
    }

    fixture.configure_translate_message(PEEK_MESSAGE_A_OUTPUT_ADDRESS);
    let selected_before_translate = fixture.runtime.current_thread_id();
    let memory_before_translate = fixture.runtime.memory.stats();
    let translated = match fixture.dispatch_translate_message() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected double-click TranslateMessage completion, got {other:?}"),
    };
    assert_eq!(translated.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(
        fixture.runtime.current_thread_id(),
        selected_before_translate
    );
    assert_eq!(
        fixture
            .runtime
            .keyboard_state()
            .unwrap()
            .last_event_sequence,
        5
    );
    assert_eq!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .unwrap()
            .iter()
            .map(|message| message.message)
            .collect::<Vec<_>>(),
        [0x0202, 0x0201]
    );
    assert_eq!(fixture.runtime.memory.stats(), memory_before_translate);

    fixture.configure_dispatch_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS);
    let call = match fixture.dispatch_dispatch_message_a() {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected double-click window-procedure callback, got {other:?}"),
    };
    assert_eq!(
        call.request.arguments,
        [window_handle, 0x0203, 0x0001, double_click_l_param]
            .into_iter()
            .map(GuestCallbackArgument::U32)
            .collect::<Vec<_>>()
    );
    let prepared = begin_fixture_callback(&mut fixture, &call);
    let (completion, semantic_action) =
        complete_fixture_callback(&mut fixture, call, 0x7152_3498, prepared.frame_id);
    assert_eq!(semantic_action, None);
    assert_eq!(completion.eax, Some(0x7152_3498));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
}

#[test]
fn is_iconic_queries_normal_window_and_rejects_unknown_handle_atomically() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, _) = fixture.prepare_active_window_with_small_icon();
    let sentinel = 0x1122_3344;
    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed IsIconic LastError");
    fixture.configure_is_iconic(window_handle);
    let window_before = fixture.runtime.window(window_handle).unwrap().clone();
    let selected_before = fixture.runtime.current_thread_id();
    let memory_before = fixture.runtime.memory.stats();
    let normal = match fixture.dispatch_is_iconic() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected normal IsIconic completion, got {other:?}"),
    };
    assert_eq!(normal.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(normal.instruction_pointer, IS_ICONIC_RETURN_ADDRESS);
    assert_eq!(
        normal.stack_pointer,
        GuestAddress(IS_ICONIC_STACK_POINTER + 8)
    );
    assert_eq!(
        fixture.runtime.window(window_handle).unwrap(),
        &window_before
    );
    assert_eq!(fixture.runtime.current_thread_id(), selected_before);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(fixture.runtime.memory.stats(), memory_before);
    let basic_normal = fixture.dispatch_is_iconic_basic();
    assert!(
        matches!(basic_normal, DispatchOutcome::Immediate(ref completion)
        if completion.eax == Some(0) && fixture.runtime.current_last_error().unwrap() == sentinel)
    );

    fixture.configure_is_iconic(0x1234);
    let invalid_memory_before = fixture.runtime.memory.stats();
    let invalid = match fixture.dispatch_is_iconic() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid-HWND IsIconic completion, got {other:?}"),
    };
    assert_eq!(invalid.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
    assert_eq!(fixture.runtime.current_thread_id(), selected_before);
    assert_eq!(
        fixture.runtime.window(window_handle).unwrap(),
        &window_before
    );
    assert_eq!(fixture.runtime.memory.stats(), invalid_memory_before);
    let basic_invalid = fixture.dispatch_is_iconic_basic();
    assert!(
        matches!(basic_invalid, DispatchOutcome::Immediate(ref completion)
        if completion.eax == Some(0) && fixture.runtime.current_last_error().unwrap() == 1400)
    );
}

#[test]
fn monitor_from_window_returns_rust_logical_primary_and_preserves_failures() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, _) = fixture.prepare_created_window_and_small_icon();
    let sentinel = 0x1122_3344;
    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed MonitorFromWindow LastError");
    fixture.configure_monitor_from_window(window_handle, 1);
    let window_id = fixture.runtime.window(window_handle).unwrap().id;
    let geometry_before = fixture
        .runtime
        .window(window_handle)
        .unwrap()
        .creation
        .geometry;
    let monitor_before = fixture.runtime.primary_monitor().cloned();
    let memory_before_success = fixture.runtime.memory.stats();
    let completion = match fixture.dispatch_monitor_from_window() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected MonitorFromWindow completion, got {other:?}"),
    };
    assert_eq!(completion.eax, Some(RUNTIME_PRIMARY_MONITOR_HANDLE));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(
        completion.stack_pointer.0,
        MONITOR_FROM_WINDOW_STACK_POINTER + 12
    );
    assert_eq!(fixture.runtime.primary_monitor().cloned(), monitor_before);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(fixture.runtime.memory.stats(), memory_before_success);

    fixture.configure_monitor_from_window(0x4801_ffff, 1);

    let monitor_before_invalid_window = fixture.runtime.primary_monitor().cloned();
    let memory_before_invalid_window = fixture.runtime.memory.stats();
    let invalid_window = match fixture.dispatch_monitor_from_window() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid-HWND MonitorFromWindow completion, got {other:?}"),
    };
    assert_eq!(invalid_window.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
    assert_eq!(fixture.runtime.window(window_handle).unwrap().id, window_id);
    assert_eq!(
        fixture
            .runtime
            .window(window_handle)
            .unwrap()
            .creation
            .geometry,
        geometry_before
    );
    assert_eq!(
        fixture.runtime.primary_monitor().cloned(),
        monitor_before_invalid_window
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
    assert_eq!(fixture.runtime.memory.stats(), memory_before_invalid_window);

    fixture.configure_monitor_from_window(window_handle, 2);
    let last_error_before = fixture.runtime.current_last_error().unwrap();
    let memory_before_flags = fixture.runtime.memory.stats();
    match fixture.dispatch_monitor_from_window() {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected unsupported MonitorFromWindow flags, got {other:?}"),
    };

    assert_eq!(fixture.runtime.primary_monitor().cloned(), monitor_before);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );
    assert_eq!(fixture.runtime.memory.stats(), memory_before_flags);
}

#[test]
fn get_monitor_info_a_writes_owned_geometry_and_preserves_all_failure_buffers() {
    let mut fixture = RegisterClassFixture::new();
    let expected_words = [40_u32, 0, 0, 1920, 1080, 0, 0, 1920, 1032, 1];
    let expected_bytes = expected_words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    let initial = fixture.initialize_monitor_info_output(MONITOR_INFO_OUTPUT_ADDRESS, 80, 40);
    fixture
        .configure_get_monitor_info_a(RUNTIME_PRIMARY_MONITOR_HANDLE, MONITOR_INFO_OUTPUT_ADDRESS);
    let sentinel = 0x1122_3344;
    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed GetMonitorInfoA LastError");
    let monitor_before = fixture.runtime.primary_monitor().cloned();
    let completion = match fixture.dispatch_get_monitor_info_a() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected GetMonitorInfoA completion, got {other:?}"),
    };
    assert_eq!(completion.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(
        completion.stack_pointer.0,
        GET_MONITOR_INFO_A_STACK_POINTER + 12
    );
    fixture.acknowledge_pending_delta();
    let written = fixture
        .runtime
        .memory
        .read(MONITOR_INFO_OUTPUT_ADDRESS, 80)
        .expect("read completed MONITORINFO");
    assert_eq!(&written[..40], expected_bytes.as_slice());
    assert_eq!(&written[40..], &initial[40..]);
    assert_eq!(fixture.runtime.primary_monitor().cloned(), monitor_before);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);

    let invalid_handle_before =
        fixture.initialize_monitor_info_output(MONITOR_INFO_OUTPUT_ADDRESS, 80, 0);
    fixture.configure_get_monitor_info_a(0, MONITOR_INFO_OUTPUT_ADDRESS);
    let invalid_handle = match fixture.dispatch_get_monitor_info_a() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid-HMONITOR completion, got {other:?}"),
    };
    assert_eq!(invalid_handle.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1461);
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(MONITOR_INFO_OUTPUT_ADDRESS, 80)
            .expect("read invalid-HMONITOR output"),
        invalid_handle_before
    );

    let invalid_size_before =
        fixture.initialize_monitor_info_output(MONITOR_INFO_OUTPUT_ADDRESS, 80, 39);
    fixture
        .configure_get_monitor_info_a(RUNTIME_PRIMARY_MONITOR_HANDLE, MONITOR_INFO_OUTPUT_ADDRESS);
    let invalid_size = match fixture.dispatch_get_monitor_info_a() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid MONITORINFO size completion, got {other:?}"),
    };
    assert_eq!(invalid_size.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 87);
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(MONITOR_INFO_OUTPUT_ADDRESS, 80)
            .expect("read invalid-size output"),
        invalid_size_before
    );

    let monitor_info_ex_before =
        fixture.initialize_monitor_info_output(MONITOR_INFO_OUTPUT_ADDRESS, 80, 72);
    fixture
        .configure_get_monitor_info_a(RUNTIME_PRIMARY_MONITOR_HANDLE, MONITOR_INFO_OUTPUT_ADDRESS);
    let last_error_before = fixture.runtime.current_last_error().unwrap();
    match fixture.dispatch_get_monitor_info_a() {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected MONITORINFOEXA unsupported fault, got {other:?}"),
    };

    assert_eq!(fixture.runtime.primary_monitor().cloned(), monitor_before);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(MONITOR_INFO_OUTPUT_ADDRESS, 80)
            .expect("read unsupported MONITORINFOEXA output"),
        monitor_info_ex_before
    );

    let boundary_output = GuestAddress(MEMORY_BASE.0 + 0x2ffc);
    fixture.initialize_monitor_info_output(boundary_output, 4, 40);
    fixture.configure_get_monitor_info_a(RUNTIME_PRIMARY_MONITOR_HANDLE, boundary_output);
    let last_error_before = fixture.runtime.current_last_error().unwrap();
    match fixture.dispatch_get_monitor_info_a() {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected full MONITORINFO write fault, got {other:?}"),
    };

    assert_eq!(fixture.runtime.primary_monitor().cloned(), monitor_before);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(boundary_output, 4)
            .expect("read boundary size field"),
        40_u32.to_le_bytes()
    );
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
    assert_eq!(invalid_window.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
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
        Some(handle) => handle,
        other => panic!("GetDC did not set EAX: {other:?}"),
    };
    assert_eq!(first_dc_handle, 0x4701_0001);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);

    let second_dc = match fixture.dispatch_get_dc(window_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected second GetDC completion, got {other:?}"),
    };
    let second_dc_handle = match second_dc.eax {
        Some(handle) => handle,
        other => panic!("second GetDC did not set EAX: {other:?}"),
    };
    assert_eq!(second_dc_handle, 0x4701_0002);
    assert_ne!(first_dc_handle, second_dc_handle);
    assert_eq!(
        fixture
            .runtime
            .gdi_device_context(first_dc_handle)
            .unwrap()
            .window_surface_id(),
        fixture
            .runtime
            .gdi_device_context(second_dc_handle)
            .unwrap()
            .window_surface_id()
    );

    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed GetStockObject LastError");
    assert!(fixture.runtime.gdi_objects().is_empty());
    let invalid_object = match fixture.dispatch_get_stock_object(-1) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid stock-object completion, got {other:?}"),
    };
    assert_eq!(invalid_object.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert!(fixture.runtime.gdi_objects().is_empty());

    let black_brush = match fixture.dispatch_get_stock_object(4) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected BLACK_BRUSH completion, got {other:?}"),
    };
    let black_brush_handle = match black_brush.eax {
        Some(handle) => handle,
        other => panic!("GetStockObject did not set EAX: {other:?}"),
    };
    assert_eq!(black_brush_handle, 0x4702_0001);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    let repeated_brush = match fixture.dispatch_get_stock_object(4) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected repeated BLACK_BRUSH completion, got {other:?}"),
    };
    assert_eq!(repeated_brush.eax, Some(black_brush_handle));
    assert_eq!(fixture.runtime.gdi_objects().len(), 1);

    let rect_bytes = [0_i32, 0, 800, 600]
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect::<Vec<_>>();
    fixture
        .runtime
        .memory
        .write(CLIENT_RECT_OUTPUT_ADDRESS, &rect_bytes)
        .expect("write FillRect input");
    fixture.acknowledge_pending_delta();
    let surface_before = fixture
        .runtime
        .gdi_surfaces()
        .values()
        .next()
        .unwrap()
        .clone();
    let invalid_hdc = match fixture.dispatch_fill_rect(
        0x4701_ffff,
        CLIENT_RECT_OUTPUT_ADDRESS,
        black_brush_handle,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid-HDC FillRect completion, got {other:?}"),
    };
    assert_eq!(invalid_hdc.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 6);
    assert_eq!(fixture.runtime.gdi_surfaces().len(), 1);
    assert_eq!(
        fixture.runtime.gdi_surfaces()[&surface_before.id],
        surface_before
    );
    let invalid_brush = match fixture.dispatch_fill_rect(
        first_dc_handle,
        CLIENT_RECT_OUTPUT_ADDRESS,
        0x4702_ffff,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid-brush FillRect completion, got {other:?}"),
    };
    assert_eq!(invalid_brush.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 6);
    assert_eq!(
        fixture.runtime.gdi_surfaces()[&surface_before.id],
        surface_before
    );
    assert_eq!(fixture.runtime.gdi_device_contexts().len(), 2);

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
    assert_eq!(fill.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
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

    let filled_surface = surface.clone();
    let first_dc_id = fixture
        .runtime
        .gdi_device_context(first_dc_handle)
        .unwrap()
        .id;
    let second_dc_id = fixture
        .runtime
        .gdi_device_context(second_dc_handle)
        .unwrap()
        .id;
    match fixture.dispatch_fill_rect(
        first_dc_handle,
        GuestAddress(MEMORY_BASE.0 + 0x3000),
        black_brush_handle,
    ) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected FillRect pointer fault, got {other:?}"),
    };

    assert_eq!(
        fixture.runtime.gdi_surfaces()[&filled_surface.id],
        filled_surface
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(
        fixture
            .runtime
            .gdi_device_context(first_dc_handle)
            .unwrap()
            .id,
        first_dc_id
    );
    assert_eq!(
        fixture
            .runtime
            .gdi_device_context(second_dc_handle)
            .unwrap()
            .id,
        second_dc_id
    );

    let invalid_release = match fixture.dispatch_release_dc(window_handle, 0x4701_ffff) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid-HDC ReleaseDC completion, got {other:?}"),
    };
    assert_eq!(invalid_release.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1425);
    assert_eq!(fixture.runtime.gdi_device_contexts().len(), 2);
    assert_eq!(
        fixture
            .runtime
            .gdi_device_context(first_dc_handle)
            .unwrap()
            .id,
        first_dc_id
    );
    assert_eq!(
        fixture
            .runtime
            .gdi_device_context(second_dc_handle)
            .unwrap()
            .id,
        second_dc_id
    );
    assert_eq!(
        fixture.runtime.gdi_surfaces()[&filled_surface.id],
        filled_surface
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1425);

    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed ReleaseDC LastError");
    let released = match fixture.dispatch_release_dc(window_handle, first_dc_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected ReleaseDC completion, got {other:?}"),
    };
    assert_eq!(released.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert!(fixture.runtime.gdi_device_context(first_dc_handle).is_err());
    assert!(fixture.runtime.gdi_device_context(second_dc_handle).is_ok());

    let double_release = match fixture.dispatch_release_dc(window_handle, first_dc_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected double ReleaseDC completion, got {other:?}"),
    };
    assert_eq!(double_release.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1425);
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
    assert_eq!(invalid_window.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
    assert_eq!(fixture.runtime.windows(), &before_invalid_window);

    let before_invalid_index = fixture.runtime.windows().clone();
    let invalid_index = match fixture.dispatch_set_window_long_a(window_handle, -100, 0x90ca_0000) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid-index completion, got {other:?}"),
    };
    assert_eq!(invalid_index.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1413);
    assert_eq!(fixture.runtime.windows(), &before_invalid_index);

    // Successful and repeated style updates preserve the prior invalid-index failure.
    let sentinel = 1413;
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    let completion = match fixture.dispatch_set_window_long_a(window_handle, -16, 0x90ca_0000) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected SetWindowLongA completion, got {other:?}"),
    };
    assert_eq!(completion.eax, Some(0x84ca_0000));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(
        completion.stack_pointer.0,
        SET_WINDOW_LONG_A_STACK_POINTER + 16
    );
    let updated = fixture.runtime.window(window_handle).unwrap();
    assert_eq!(updated.creation.style, 0x80ca_0000);
    assert_eq!(updated.creation.effective_style, 0x90ca_0000);
    assert!(updated.visible);

    let window_id = updated.id;
    let owner = fixture.runtime.current_thread_id();
    let messages_before = fixture.runtime.queued_thread_messages(owner).unwrap();
    let repeated = match fixture.dispatch_set_window_long_a(window_handle, -16, 0x90ca_0000) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected repeated SetWindowLongA completion, got {other:?}"),
    };
    assert_eq!(repeated.eax, Some(0x90ca_0000));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    let repeated_window = fixture.runtime.window(window_handle).unwrap();
    assert_eq!(repeated_window.id, window_id);
    assert_eq!(repeated_window.creation.style, 0x80ca_0000);
    assert_eq!(repeated_window.creation.effective_style, 0x90ca_0000);
    assert!(repeated_window.visible);
    assert_eq!(
        fixture.runtime.queued_thread_messages(owner).unwrap(),
        messages_before
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
}

#[test]
fn basic_set_timer_accepts_bounded_window_callback_timers_and_rejects_bad_targets() {
    let mut fixture = RegisterClassFixture::new();
    let (window, _) = fixture.prepare_created_window_and_small_icon();
    let callback = fixture.window_procedure;
    let window_id = fixture.runtime.window(window).unwrap().id;
    assert!(fixture.runtime.window_timers().is_empty());
    let invalid = fixture.dispatch_set_timer_basic(0x4801_ffff, 7, 23, callback);
    let DispatchOutcome::Immediate(invalid) = invalid else {
        panic!("invalid window did not return");
    };
    assert_eq!(invalid.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
    assert!(fixture.runtime.window_timers().is_empty());
    assert_eq!(fixture.runtime.window(window).unwrap().id, window_id);
    for (id, callback_address) in [
        (0, callback),
        (7, GuestAddress(0)),
        (7, GuestAddress(CLASS_NAME_BASE)),
    ] {
        assert!(matches!(
            fixture.dispatch_set_timer_basic(window, id, 23, callback_address),
            DispatchOutcome::Fault(_)
        ));
        assert!(fixture.runtime.window_timers().is_empty());
        assert_eq!(fixture.runtime.window(window).unwrap().id, window_id);
        assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
    }
    let completion = fixture.dispatch_set_timer_basic(window, 7, 23, callback);
    let DispatchOutcome::Immediate(completion) = completion else {
        panic!("timer registration failed");
    };
    assert_eq!(completion.eax, Some(7));
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        fixture.runtime.current_last_error().unwrap()
    );
    let timer = fixture.runtime.window_timers().values().next().unwrap();
    assert_eq!(timer.timer_id, 7);
    assert_eq!(timer.requested_interval_milliseconds, 23);
    assert_eq!(timer.callback_address, callback);
}

#[test]
fn set_timer_registers_and_replaces_owned_window_timer_atomically() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, _) = fixture.prepare_created_window_and_small_icon();
    let callback = fixture.window_procedure;
    assert!(fixture.runtime.window_timers().is_empty());

    let window_id = fixture.runtime.window(window_handle).unwrap().id;
    let invalid_window = match fixture.dispatch_set_timer(0x4801_ffff, 1, 20, callback) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid-window SetTimer completion, got {other:?}"),
    };
    assert_eq!(invalid_window.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
    assert!(fixture.runtime.window_timers().is_empty());
    assert_eq!(fixture.runtime.window(window_handle).unwrap().id, window_id);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);

    for (timer_id, interval) in [(2, 20), (1, 21), (7, 1)] {
        let mut arbitrary = RegisterClassFixture::new();
        let (window, _) = arbitrary.prepare_created_window_and_small_icon();
        let completion = match arbitrary.dispatch_set_timer(window, timer_id, interval, callback) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected caller-selected SetTimer completion, got {other:?}"),
        };
        assert_eq!(completion.eax, Some(timer_id));
        let timer = arbitrary.runtime.window_timers().values().next().unwrap();
        assert_eq!(timer.timer_id, timer_id);
        assert_eq!(timer.requested_interval_milliseconds, interval);
        assert_eq!(timer.effective_interval_milliseconds, interval.max(10));
    }

    let last_error_before = fixture.runtime.current_last_error().unwrap();
    match fixture.dispatch_set_timer(window_handle, 1, 20, GuestAddress(CLASS_NAME_BASE)) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected non-executable TIMERPROC fault, got {other:?}"),
    };

    assert!(fixture.runtime.window_timers().is_empty());
    assert_eq!(fixture.runtime.window(window_handle).unwrap().id, window_id);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );

    let sentinel = 0x55aa_1234;
    fixture
        .runtime
        .set_current_last_error(sentinel)
        .expect("seed SetTimer LastError");
    let completion = match fixture.dispatch_set_timer(window_handle, 1, 20, callback) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected SetTimer completion, got {other:?}"),
    };
    assert_eq!(completion.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(completion.stack_pointer.0, SET_TIMER_STACK_POINTER + 20);
    assert_eq!(fixture.runtime.window_timers().len(), 1);
    let first = *fixture.runtime.window_timers().values().next().unwrap();

    assert_eq!(first.window_handle, window_handle);
    assert_eq!(first.timer_id, 1);
    assert_eq!(first.owner_thread_id, RuntimeThreadId(THREAD_ID));
    assert_eq!(first.requested_interval_milliseconds, 20);
    assert_eq!(first.effective_interval_milliseconds, 20);
    assert_eq!(first.callback_address, callback);

    let replacement_callback = GuestAddress(callback.0 + 1);
    let replacement = match fixture.dispatch_set_timer(window_handle, 1, 20, replacement_callback) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected replacement SetTimer completion, got {other:?}"),
    };
    assert_eq!(replacement.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), sentinel);
    assert_eq!(fixture.runtime.window_timers().len(), 1);
    let replaced = fixture.runtime.window_timers().values().next().unwrap();
    assert_eq!(replaced.id, first.id);
    assert_eq!(replaced.callback_address, replacement_callback);
}

#[test]
fn load_icon_a_reuses_process_shared_identity() {
    let mut fixture = IconFixture::new();
    let DispatchOutcome::Immediate(first) = fixture.dispatch(0) else {
        panic!("LoadIconA failed")
    };
    let icon = first.eax.unwrap();
    assert_ne!(icon, 0);
    assert_eq!(first.stack_pointer, GuestAddress(STACK_POINTER + 12));
    assert_eq!(first.instruction_pointer, RETURN_ADDRESS);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0);
    let DispatchOutcome::Immediate(second) = fixture.dispatch(1) else {
        panic!("repeat LoadIconA failed")
    };
    assert_eq!(second.eax, Some(icon));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0);
}

#[test]
fn load_icon_a_negative_paths_are_failure_atomic_and_do_not_consume_identity() {
    let mut missing = IconFixture::new();
    let generation = missing.runtime.memory.stats().generation;
    let completion = match missing.dispatch(2) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected missing resource completion, got {other:?}"),
    };
    assert_eq!(completion.eax, Some(0));
    assert_eq!(missing.runtime.current_last_error().unwrap(), 1813);
    assert!(missing.runtime.icons().is_empty());
    assert_eq!(missing.runtime.memory.stats().generation, generation);
    let valid = match missing.dispatch(3) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected valid retry completion, got {other:?}"),
    };
    assert_eq!(valid.eax, Some(0x4901_0001));

    let mut unknown_module = IconFixture::new();
    let completion = match unknown_module.dispatch(4) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid module completion, got {other:?}"),
    };
    assert_eq!(completion.eax, Some(0));
    assert_eq!(unknown_module.runtime.current_last_error().unwrap(), 6);
    assert!(unknown_module.runtime.icons().is_empty());

    let mut string_name = IconFixture::new();
    match string_name.dispatch(5) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected string resource fault, got {other:?}"),
    };
    assert!(string_name.runtime.icons().is_empty());

    let mut malformed = IconFixture::from_bytes(malformed_group_icon_bytes());
    let completion = match malformed.dispatch(0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected malformed group failure, got {other:?}"),
    };
    assert_eq!(completion.eax, Some(0));
    assert_eq!(malformed.runtime.current_last_error().unwrap(), 1812);
    assert!(malformed.runtime.icons().is_empty());

    let mut system = IconFixture::new();
    match system.dispatch(6) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected system icon fault, got {other:?}"),
    };
    assert!(system.runtime.icons().is_empty());

    let mut png = IconFixture::from_bytes(png_selected_icon_bytes());
    match png.dispatch(0) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected PNG icon fault, got {other:?}"),
    };
    assert!(png.runtime.icons().is_empty());
}

#[test]
fn load_image_a_shares_equal_sizes_and_distinguishes_small_icons() {
    let mut fixture = IconFixture::new();
    let DispatchOutcome::Immediate(large) = fixture.dispatch(0) else {
        panic!("LoadIconA failed")
    };
    let DispatchOutcome::Immediate(first) = fixture.dispatch_image(0) else {
        panic!("LoadImageA failed")
    };
    let DispatchOutcome::Immediate(second) = fixture.dispatch_image(1) else {
        panic!("repeat LoadImageA failed")
    };
    assert_ne!(large.eax, Some(0));
    assert_ne!(first.eax, Some(0));
    assert_ne!(first.eax, large.eax);
    assert_eq!(second.eax, first.eax);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0);
}

#[test]
fn load_image_a_negative_variants_are_failure_atomic() {
    let mut missing = IconFixture::new();
    let generation = missing.runtime.memory.stats().generation;
    let completion = match missing.dispatch_image(2) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected missing LoadImageA resource completion, got {other:?}"),
    };
    assert_eq!(completion.eax, Some(0));
    assert_eq!(missing.runtime.current_last_error().unwrap(), 1813);
    assert!(missing.runtime.icons().is_empty());
    assert_eq!(missing.runtime.memory.stats().generation, generation);
    let valid = match missing.dispatch_image(0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected valid LoadImageA retry, got {other:?}"),
    };
    assert_eq!(valid.eax, Some(0x4901_0001));

    for index in [3, 4, 5, 6] {
        let mut fixture = IconFixture::new();
        let generation = fixture.runtime.memory.stats().generation;
        match fixture.dispatch_image(index) {
            DispatchOutcome::Fault(fault) => fault,
            other => panic!("expected LoadImageA variant fault, got {other:?}"),
        };
        assert!(fixture.runtime.icons().is_empty());
        assert_eq!(fixture.runtime.memory.stats().generation, generation);
    }

    let mut unknown_module = IconFixture::new();
    let completion = match unknown_module.dispatch_image(7) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid LoadImageA module completion, got {other:?}"),
    };
    assert_eq!(completion.eax, Some(0));
    assert_eq!(unknown_module.runtime.current_last_error().unwrap(), 6);
    assert!(unknown_module.runtime.icons().is_empty());
}

#[test]
fn load_cursor_a_preserves_last_error_and_reuses_shared_arrow_identity() {
    let mut fixture = CursorFixture::new();
    let DispatchOutcome::Immediate(first) = fixture.dispatch(0) else {
        panic!("LoadCursorA failed")
    };
    let cursor = first.eax.unwrap();
    assert_ne!(cursor, 0);
    assert_eq!(first.stack_pointer, GuestAddress(STACK_POINTER + 12));
    assert_eq!(first.instruction_pointer, CURSOR_RETURN_ADDRESS);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
    let DispatchOutcome::Immediate(repeated) = fixture.dispatch(1) else {
        panic!("repeat LoadCursorA failed")
    };
    assert_eq!(repeated.eax, Some(cursor));
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
    fixture.runtime.set_current_last_error(0x8765_4321).unwrap();
    let DispatchOutcome::Immediate(module) = fixture.dispatch(4) else {
        panic!("module cursor failed")
    };
    assert_ne!(module.eax, Some(0));
    assert_ne!(module.eax, Some(cursor));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0);
    let DispatchOutcome::Immediate(repeated_module) = fixture.dispatch(6) else {
        panic!("repeat module cursor failed")
    };
    assert_eq!(repeated_module.eax, module.eax);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0);
}

#[test]
fn load_cursor_a_negative_variants_are_failure_atomic() {
    let mut missing = CursorFixture::new();
    let memory_generation = missing.runtime.memory.stats().generation;
    let completion = match missing.dispatch(2) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected missing cursor completion, got {other:?}"),
    };
    assert_eq!(completion.eax, Some(0));
    assert_eq!(missing.runtime.current_last_error().unwrap(), 1814);
    assert!(missing.runtime.cursors().is_empty());
    assert_eq!(missing.runtime.memory.stats().generation, memory_generation);
    let valid = match missing.dispatch(0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected valid cursor retry, got {other:?}"),
    };
    assert_eq!(valid.eax, Some(0x4301_0001));

    for index in [3, 5, 10] {
        let mut fixture = CursorFixture::new();
        match fixture.dispatch(index) {
            DispatchOutcome::Fault(fault) => fault,
            other => panic!("expected cursor variant fault, got {other:?}"),
        };
        assert!(fixture.runtime.cursors().is_empty());
    }

    for (index, last_error) in [(7, 1813), (8, 1813), (9, 6)] {
        let mut fixture = CursorFixture::new();
        let generation = fixture.runtime.memory.stats().generation;
        let completion = match fixture.dispatch(index) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected module cursor failure completion, got {other:?}"),
        };
        assert_eq!(completion.eax, Some(0));
        assert_eq!(fixture.runtime.current_last_error().unwrap(), last_error);
        assert!(fixture.runtime.cursors().is_empty());
        assert_eq!(fixture.runtime.memory.stats().generation, generation);
    }
}

#[test]
fn load_cursor_a_rejects_malformed_module_resources_before_identity_mutation() {
    let mut malformed_group = CursorFixture::from_bytes(malformed_group_cursor_bytes());
    let generation = malformed_group.runtime.memory.stats().generation;
    let completion = match malformed_group.dispatch(4) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected malformed group cursor completion, got {other:?}"),
    };
    assert_eq!(completion.eax, Some(0));
    assert_eq!(malformed_group.runtime.current_last_error().unwrap(), 1812);
    assert!(malformed_group.runtime.cursors().is_empty());
    assert_eq!(
        malformed_group.runtime.memory.stats().generation,
        generation
    );

    let mut wrong_size = CursorFixture::from_bytes(wrong_size_group_cursor_bytes());
    let generation = wrong_size.runtime.memory.stats().generation;
    match wrong_size.dispatch(4) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected unsupported cursor size fault, got {other:?}"),
    };

    assert!(wrong_size.runtime.cursors().is_empty());
    assert_eq!(wrong_size.runtime.memory.stats().generation, generation);

    let mut invalid_hotspot = CursorFixture::from_bytes(invalid_cursor_hotspot_bytes());
    let generation = invalid_hotspot.runtime.memory.stats().generation;
    let completion = match invalid_hotspot.dispatch(4) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid cursor hotspot completion, got {other:?}"),
    };
    assert_eq!(completion.eax, Some(0));
    assert_eq!(invalid_hotspot.runtime.current_last_error().unwrap(), 1812);
    assert!(invalid_hotspot.runtime.cursors().is_empty());
    assert_eq!(
        invalid_hotspot.runtime.memory.stats().generation,
        generation
    );
}

#[test]
fn generic_create_window_uses_requested_bounded_geometry_and_copies_class_state() {
    let mut fixture = RegisterClassFixture::new();
    fixture.prepare_resources();
    assert!(matches!(
        fixture.dispatch_class(0),
        DispatchOutcome::Immediate(_)
    ));
    // Width and height are guest arguments, not a fixed 800x600 title profile.
    fixture
        .runtime
        .memory
        .write(
            GuestAddress(CREATE_STACK_POINTER + 7 * 4),
            &1026_u32.to_le_bytes(),
        )
        .expect("write outer width");
    fixture
        .runtime
        .memory
        .write(
            GuestAddress(CREATE_STACK_POINTER + 8 * 4),
            &797_u32.to_le_bytes(),
        )
        .expect("write outer height");
    fixture.acknowledge_pending_delta();
    let endpoint = fixture.create_endpoint.clone();
    let outcome = {
        fixture.dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: endpoint.id,
                endpoint_address: endpoint.address,
                stack_pointer: GuestAddress(CREATE_STACK_POINTER),
                return_address: CREATE_RETURN_ADDRESS,
            },
            &mut fixture.runtime,
            None,
            |frame, runtime, _cpu| dispatch_user32_api(fixture.callback_sentinel, frame, runtime),
        )
    };
    let DispatchOutcome::GuestCallback(call) = outcome else {
        panic!(
            "generic window creation did not enter the registered window procedure: {outcome:?}"
        );
    };
    let window = fixture
        .runtime
        .windows()
        .values()
        .next()
        .expect("pending window");
    assert_eq!(window.creation.geometry.width, 1026);
    assert_eq!(window.creation.geometry.height, 797);
    assert_eq!(window.creation.geometry.client_width, 1020);
    assert_eq!(window.creation.geometry.client_height, 768);
    assert_eq!(
        call.request.arguments[1],
        GuestCallbackArgument::U32(0x0081)
    );
    let prepared = begin_fixture_callback(&mut fixture, &call);
    assert!(prepared.scratch_bytes >= 48);
    let mut invalid = RegisterClassFixture::new();
    invalid.prepare_resources();
    assert!(matches!(
        invalid.dispatch_class(0),
        DispatchOutcome::Immediate(_)
    ));
    invalid
        .runtime
        .memory
        .write(
            GuestAddress(CREATE_STACK_POINTER + 9 * 4),
            &1_u32.to_le_bytes(),
        )
        .expect("set unsupported parent");
    invalid.acknowledge_pending_delta();
    let previous_generation = invalid.runtime.memory.stats().generation;
    let endpoint = invalid.create_endpoint.clone();
    let outcome = {
        invalid.dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: endpoint.id,
                endpoint_address: endpoint.address,
                stack_pointer: GuestAddress(CREATE_STACK_POINTER),
                return_address: CREATE_RETURN_ADDRESS,
            },
            &mut invalid.runtime,
            None,
            |frame, runtime, _cpu| dispatch_user32_api(invalid.callback_sentinel, frame, runtime),
        )
    };
    assert!(matches!(outcome, DispatchOutcome::Fault(_)));
    assert!(invalid.runtime.windows().is_empty());
    assert_eq!(
        invalid.runtime.memory.stats().generation,
        previous_generation
    );
}

#[test]
fn register_class_ex_a_preserves_guest_input_and_rejects_duplicate_name() {
    let mut fixture = RegisterClassFixture::new();
    fixture.prepare_resources();
    let class_bytes = fixture
        .runtime
        .memory
        .read(GuestAddress(CLASS_STRUCT_BASE), 48)
        .unwrap();
    fixture.runtime.set_current_last_error(0x1122_3344).unwrap();
    let DispatchOutcome::Immediate(main) = fixture.dispatch_class(0) else {
        panic!("class registration failed")
    };
    let atom = main.eax.unwrap();
    assert_ne!(atom, 0);
    assert!(atom <= u32::from(u16::MAX));
    assert_eq!(main.stack_pointer, GuestAddress(STACK_POINTER + 0x48));
    assert_eq!(main.instruction_pointer, CLASS_RETURN_ADDRESS);
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(GuestAddress(CLASS_STRUCT_BASE), 48)
            .unwrap(),
        class_bytes
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x1122_3344);
    let DispatchOutcome::Immediate(child) = fixture.dispatch_class(1) else {
        panic!("second registration failed")
    };
    assert_ne!(child.eax, Some(0));
    assert_ne!(child.eax, Some(atom));
    let DispatchOutcome::Immediate(duplicate) = fixture.dispatch_class(2) else {
        panic!("duplicate class did not return")
    };
    assert_eq!(duplicate.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1410);
}

#[test]
fn create_window_ex_a_runs_ordered_callback_replacement_and_default_nonclient_geometry() {
    let mut fixture = RegisterClassFixture::new();
    fixture.prepare_resources();
    assert!(matches!(
        fixture.dispatch_class(0),
        DispatchOutcome::Immediate(ref completion)
            if completion.eax == Some(0xc000)
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

    let mut prepared = fixture
        .runtime
        .begin_guest_callback(call.clone())
        .expect("enter WM_NCCREATE callback");
    fixture.acknowledge_pending_delta();
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
        .runtime
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
            .runtime
            .memory
            .read(GuestAddress(title_pointer), 9)
            .expect("read relocated window title"),
        b"Ethornell"
    );
    assert_eq!(
        fixture
            .runtime
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
                assert_eq!(completion.eax, Some(1));
                1
            }
            RuntimeWindowCreationMessage::NonClientCalculate => {
                assert_eq!(
                    fixture
                        .runtime
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
                assert_eq!(completion.eax, Some(0));
                assert_eq!(
                    fixture
                        .runtime
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

                match fixture.dispatch_def_window_proc(window.handle, message_id, l_param ^ 1) {
                    DispatchOutcome::Fault(fault) => fault,
                    other => panic!("expected packed lParam mismatch, got {other:?}"),
                };

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
                assert_eq!(completion.eax, Some(0));
                assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x2468_1357);
                0
            }
            RuntimeWindowCreationMessage::Create => 0,
        };
        let completion_policy = call.request.completion_policy;
        let endpoint = call.frame.endpoint.clone();
        let outcome = {
            fixture.dispatcher.complete_guest_callback_result(
                call.clone(),
                callback_result,
                &mut fixture.runtime,
            )
        };
        if index + 1 < expected_messages.len() {
            let next = match outcome {
                DispatchOutcome::GuestCallback(next) => next,
                other => panic!("expected replacement callback after {message:?}, got {other:?}"),
            };
            let action = {
                let callback_runtime = &mut fixture.runtime;
                let next_prepared = callback_runtime
                    .replace_guest_callback(next.clone(), prepared.frame_id)
                    .expect("replace creation callback frame");
                let action = callback_runtime
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
            let action = fixture
                .runtime
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
    assert_eq!(completion.eax, Some(window.handle));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x2468_1357);
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
    let memory_generation_before = fixture.runtime.memory.stats().generation;

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
    assert_eq!(
        fixture.runtime.memory.stats().generation,
        memory_generation_before
    );
    assert!(
        fixture
            .runtime
            .current_thread_state()
            .unwrap()
            .callback_frames
            .is_empty()
    );

    let prepared = fixture
        .runtime
        .begin_guest_callback(call.clone())
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
        .runtime
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
    assert_eq!(default_completion.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x2468_1357);
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
    assert_eq!(repeated_default.eax, Some(icon_handle));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x2468_1357);
    assert_eq!(fixture.runtime.windows(), &windows_with_icon);

    let callback_result = 0x89ab_cdef;
    let completion = {
        match fixture.dispatcher.complete_guest_callback_result(
            call,
            callback_result,
            &mut fixture.runtime,
        ) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected completed SendMessageA, got {other:?}"),
        }
    };
    fixture
        .runtime
        .pop_callback_frame(prepared.frame_id)
        .expect("retire SendMessageA callback frame");
    assert_eq!(completion.eax, Some(callback_result));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x2468_1357);
    assert_eq!(completion.instruction_pointer, SEND_MESSAGE_RETURN_ADDRESS);
    assert_eq!(
        completion.stack_pointer,
        GuestAddress(SEND_MESSAGE_STACK_POINTER + 20)
    );
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
fn set_window_text_a_dispatches_wm_settext_and_def_window_proc_commits_copied_windows_932_title() {
    const TITLE: &[u8] = &[0x93, 0xfa, 0x96, 0x7b, 0x8c, 0xea, 0]; // 日本語

    {
        let mut fixture = RegisterClassFixture::new();
        let (window_handle, _) = fixture.prepare_created_window_and_small_icon();
        fixture.configure_set_window_text_a(window_handle, UPDATED_WINDOW_TITLE_BASE, TITLE);
        fixture
            .runtime
            .set_current_last_error(0x2468_1357)
            .expect("seed SetWindowTextA LastError");
        let windows_before = fixture.runtime.windows().clone();
        let memory_generation_before = fixture.runtime.memory.stats().generation;

        let call = match fixture.dispatch_set_window_text_a() {
            DispatchOutcome::GuestCallback(call) => call,
            other => panic!("expected SetWindowTextA window callback, got {other:?}"),
        };
        assert_eq!(call.request.target, fixture.window_procedure);
        assert_eq!(
            call.request.arguments,
            [window_handle, 0x000c, 0, UPDATED_WINDOW_TITLE_BASE.0]
                .into_iter()
                .map(GuestCallbackArgument::U32)
                .collect::<Vec<_>>()
        );
        assert!(call.request.scratch.is_empty());
        assert!(call.request.scratch_pointer_relocations.is_empty());
        assert_eq!(call.request.return_sentinel, fixture.callback_sentinel);
        assert_eq!(
            call.request.completion_policy,
            GuestCallbackCompletionPolicy::ReturnBool32WithLastError {
                last_error: 0x2468_1357,
            }
        );
        assert_eq!(fixture.runtime.windows(), &windows_before);
        assert_eq!(
            fixture.runtime.memory.stats().generation,
            memory_generation_before
        );

        let prepared = begin_fixture_callback(&mut fixture, &call);
        let callback_frame = fixture
            .runtime
            .memory
            .read(prepared.stack_pointer, 20)
            .expect("read SetWindowTextA callback frame");
        assert_eq!(
            callback_frame,
            [
                fixture.callback_sentinel.0,
                window_handle,
                0x000c,
                0,
                UPDATED_WINDOW_TITLE_BASE.0,
            ]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
        );

        let default_completion = match fixture.dispatch_def_window_proc_args(
            window_handle,
            0x000c,
            0,
            UPDATED_WINDOW_TITLE_BASE.0,
        ) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected nested WM_SETTEXT default completion, got {other:?}"),
        };
        assert_eq!(default_completion.eax, Some(1));
        assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x2468_1357);
        let window = fixture
            .runtime
            .window(window_handle)
            .expect("updated window");
        assert_eq!(window.creation.name.value, "日本語");
        assert_eq!(
            window.creation.name.windows_932_bytes().unwrap(),
            &TITLE[..TITLE.len() - 1]
        );
        let windows_with_title = fixture.runtime.windows().clone();

        let repeated_default = match fixture.dispatch_def_window_proc_args(
            window_handle,
            0x000c,
            0,
            UPDATED_WINDOW_TITLE_BASE.0,
        ) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected repeated WM_SETTEXT default completion, got {other:?}"),
        };
        assert_eq!(repeated_default.eax, Some(1));
        assert_eq!(fixture.runtime.windows(), &windows_with_title);

        let completion = {
            match fixture
                .dispatcher
                .complete_guest_callback_result(call, 1, &mut fixture.runtime)
            {
                DispatchOutcome::Immediate(completion) => completion,
                other => panic!("expected completed SetWindowTextA, got {other:?}"),
            }
        };
        fixture
            .runtime
            .pop_callback_frame(prepared.frame_id)
            .expect("retire SetWindowTextA callback frame");
        assert_eq!(completion.eax, Some(1));
        assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x2468_1357);
        assert_eq!(
            completion.instruction_pointer,
            SET_WINDOW_TEXT_A_RETURN_ADDRESS
        );
        assert_eq!(
            completion.stack_pointer,
            GuestAddress(SET_WINDOW_TEXT_A_STACK_POINTER + 12)
        );
    }
}

#[test]
fn set_window_text_a_does_not_mutate_title_when_wndproc_skips_default_processing() {
    const TITLE: &[u8] = &[0x93, 0xfa, 0x96, 0x7b, 0x8c, 0xea, 0]; // 日本語

    let mut fixture = RegisterClassFixture::new();
    let (window_handle, _) = fixture.prepare_created_window_and_small_icon();
    fixture.configure_set_window_text_a(window_handle, UPDATED_WINDOW_TITLE_BASE, TITLE);
    let windows_before = fixture.runtime.windows().clone();
    let call = match fixture.dispatch_set_window_text_a() {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected SetWindowTextA callback, got {other:?}"),
    };
    let prepared = begin_fixture_callback(&mut fixture, &call);
    let completion = {
        match fixture
            .dispatcher
            .complete_guest_callback_result(call, 0, &mut fixture.runtime)
        {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected rejected SetWindowTextA completion, got {other:?}"),
        }
    };
    fixture
        .runtime
        .pop_callback_frame(prepared.frame_id)
        .expect("retire rejected SetWindowTextA callback frame");
    assert_eq!(completion.eax, Some(0));
    assert_eq!(fixture.runtime.windows(), &windows_before);
}

#[test]
fn set_window_text_a_treats_empty_and_absent_titles_as_ordinary_wm_settext_requests() {
    // Windows documents lpString as optional and places no lower bound on its length, so both an
    // empty string and NULL are ordinary calls that reach the window procedure as WM_SETTEXT.
    for title_address in [UPDATED_WINDOW_TITLE_BASE, GuestAddress(0)] {
        let mut fixture = RegisterClassFixture::new();
        let (window_handle, _) = fixture.prepare_created_window_and_small_icon();
        fixture.configure_set_window_text_a(window_handle, UPDATED_WINDOW_TITLE_BASE, b"\0");
        if title_address.0 == 0 {
            fixture.write_live_words(
                GuestAddress(SET_WINDOW_TEXT_A_STACK_POINTER),
                &[SET_WINDOW_TEXT_A_RETURN_ADDRESS.0, window_handle, 0],
            );
            fixture.acknowledge_pending_delta();
        }
        fixture
            .runtime
            .set_current_last_error(0x2468_1357)
            .expect("seed SetWindowTextA LastError");
        let windows_before = fixture.runtime.windows().clone();

        let call = match fixture.dispatch_set_window_text_a() {
            DispatchOutcome::GuestCallback(call) => call,
            other => panic!("expected SetWindowTextA window callback, got {other:?}"),
        };
        assert_eq!(call.request.target, fixture.window_procedure);
        assert_eq!(
            call.request.arguments,
            [window_handle, 0x000c, 0, title_address.0]
                .into_iter()
                .map(GuestCallbackArgument::U32)
                .collect::<Vec<_>>()
        );
        assert_eq!(fixture.runtime.windows(), &windows_before);

        let prepared = begin_fixture_callback(&mut fixture, &call);
        let default_completion = match fixture.dispatch_def_window_proc_args(
            window_handle,
            0x000c,
            0,
            title_address.0,
        ) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected empty WM_SETTEXT default completion, got {other:?}"),
        };
        assert_eq!(default_completion.eax, Some(1));
        assert_eq!(
            fixture
                .runtime
                .window(window_handle)
                .expect("window after empty WM_SETTEXT")
                .creation
                .name
                .value,
            ""
        );
        let completion = {
            match fixture
                .dispatcher
                .complete_guest_callback_result(call, 1, &mut fixture.runtime)
            {
                DispatchOutcome::Immediate(completion) => completion,
                other => panic!("expected completed SetWindowTextA, got {other:?}"),
            }
        };
        fixture
            .runtime
            .pop_callback_frame(prepared.frame_id)
            .expect("retire SetWindowTextA callback frame");
        assert_eq!(completion.eax, Some(1));
        assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x2468_1357);
    }
}

#[test]
fn set_window_text_a_rejects_invalid_handle_and_malformed_titles_without_partial_mutation() {
    let mut invalid_handle = RegisterClassFixture::new();
    invalid_handle.prepare_created_window_and_small_icon();
    invalid_handle.configure_set_window_text_a(
        0x4801_ffff,
        UPDATED_WINDOW_TITLE_BASE,
        b"invalid handle\0",
    );
    invalid_handle
        .runtime
        .set_current_last_error(0x2468_1357)
        .unwrap();
    let invalid_windows = invalid_handle.runtime.windows().clone();
    let invalid_completion = match invalid_handle.dispatch_set_window_text_a() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid-HWND SetWindowTextA completion, got {other:?}"),
    };
    assert_eq!(invalid_completion.eax, Some(0));
    assert_eq!(invalid_handle.runtime.current_last_error().unwrap(), 1400);
    assert_eq!(invalid_handle.runtime.windows(), &invalid_windows);

    for (title, _) in [
        (vec![0x82, 0], "window-title-a-windows-932-invalid"),
        (vec![b'A'; 256], "window-title-a-bound-exceeded"),
    ] {
        let mut malformed = RegisterClassFixture::new();
        let (window_handle, _) = malformed.prepare_created_window_and_small_icon();
        malformed.configure_set_window_text_a(window_handle, UPDATED_WINDOW_TITLE_BASE, &title);
        malformed
            .runtime
            .set_current_last_error(0x2468_1357)
            .unwrap();
        let windows_before = malformed.runtime.windows().clone();
        match malformed.dispatch_set_window_text_a() {
            DispatchOutcome::Fault(fault) => fault,
            other => panic!("expected malformed SetWindowTextA fault, got {other:?}"),
        };

        assert_eq!(malformed.runtime.windows(), &windows_before);
        assert_eq!(malformed.runtime.current_last_error().unwrap(), 0x2468_1357);
        assert!(
            malformed
                .runtime
                .current_thread_state()
                .unwrap()
                .callback_frames
                .is_empty()
        );
    }
}

#[test]
fn imm32_default_window_control_detach_and_restore_share_logical_authority() {
    let mut fixture = RegisterClassFixture::new();
    let (app_window_handle, _) = fixture.prepare_created_window_and_small_icon();
    let app_window_id = fixture.runtime.window(app_window_handle).unwrap().id;
    fixture.runtime.set_current_last_error(0xa5a5_5a5a).unwrap();

    let first = match fixture.dispatch_imm_get_default_ime_wnd(app_window_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected ImmGetDefaultIMEWnd completion, got {other:?}"),
    };
    let ime_window_handle = match first.eax {
        Some(handle) => handle,
        other => panic!("expected logical IME HWND, got {other:?}"),
    };
    assert_eq!(ime_window_handle, app_window_handle + 1);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0);
    assert_eq!(
        first.stack_pointer,
        GuestAddress(IMM_GET_DEFAULT_IME_WND_STACK_POINTER + 8)
    );
    let ime_window = fixture.runtime.ime_window(ime_window_handle).unwrap();
    assert_eq!(ime_window.class_name, "IME");
    assert_eq!(ime_window.owner_thread_id, RuntimeThreadId(THREAD_ID));
    assert_eq!(ime_window.status_window_request, None);
    let initial_association = fixture.runtime.ime_context_associations()[&app_window_id].clone();
    let context_handle = initial_association.context.unwrap().handle;
    assert_eq!(
        fixture
            .runtime
            .ime_context(context_handle)
            .unwrap()
            .owner_thread_id,
        RuntimeThreadId(THREAD_ID)
    );
    assert_ne!(context_handle, ime_window_handle);

    let state_before_repeat = (
        fixture.runtime.ime_windows().clone(),
        fixture.runtime.ime_contexts().clone(),
        fixture.runtime.ime_context_associations().clone(),
    );
    let repeated = match fixture.dispatch_imm_get_default_ime_wnd(app_window_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected repeated IME window completion, got {other:?}"),
    };
    assert_eq!(repeated.eax, Some(ime_window_handle));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0);
    assert_eq!(
        (
            fixture.runtime.ime_windows().clone(),
            fixture.runtime.ime_contexts().clone(),
            fixture.runtime.ime_context_associations().clone(),
        ),
        state_before_repeat
    );

    fixture.configure_send_message(ime_window_handle, 0x0283, 0x21, 0);
    let close = match fixture.dispatch_send_message() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected immediate IME status close, got {other:?}"),
    };
    assert_eq!(close.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0);
    assert_eq!(
        fixture
            .runtime
            .ime_window(ime_window_handle)
            .unwrap()
            .status_window_request,
        Some(RuntimeImeStatusWindowRequest::Close)
    );

    fixture.runtime.set_current_last_error(0x1234_5678).unwrap();
    let detach = match fixture.dispatch_imm_associate_context(app_window_handle, 0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected IME context detach completion, got {other:?}"),
    };
    assert_eq!(detach.eax, Some(context_handle));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x1234_5678);
    assert_eq!(
        fixture.runtime.ime_context_associations()[&app_window_id].context,
        None
    );

    let repeated_after_detach = match fixture.dispatch_imm_get_default_ime_wnd(app_window_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected detached repeat query, got {other:?}"),
    };
    assert_eq!(repeated_after_detach.eax, Some(ime_window_handle));
    assert_eq!(
        fixture.runtime.ime_context_associations()[&app_window_id].context,
        None
    );

    fixture.runtime.set_current_last_error(0x8765_4321).unwrap();
    let restore = match fixture.dispatch_imm_associate_context(app_window_handle, context_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected IME context restore completion, got {other:?}"),
    };
    assert_eq!(restore.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x8765_4321);
    assert_eq!(
        fixture.runtime.ime_context_associations()[&app_window_id]
            .context
            .unwrap()
            .handle,
        context_handle
    );

    fixture.configure_send_message(ime_window_handle, 0x0283, 0x22, 0);
    assert!(matches!(
        fixture.dispatch_send_message(),
        DispatchOutcome::Immediate(ref completion) if completion.eax == Some(0)
    ));
    assert_eq!(
        fixture
            .runtime
            .ime_window(ime_window_handle)
            .unwrap()
            .status_window_request,
        Some(RuntimeImeStatusWindowRequest::Open)
    );
}

#[test]
fn default_ime_window_query_from_another_thread_uses_the_window_owner_identity() {
    let mut fixture = RegisterClassFixture::new();
    let (app_window_handle, _) = fixture.prepare_created_window_and_small_icon();
    let other_thread = RuntimeThreadId(0x2001);
    fixture
        .runtime
        .register_thread(other_thread, 0x1357_2468)
        .unwrap();
    fixture.runtime.select_thread(other_thread).unwrap();
    let DispatchOutcome::Immediate(first) =
        fixture.dispatch_imm_get_default_ime_wnd(app_window_handle)
    else {
        panic!("cross-thread default IME query failed")
    };
    let Some(handle) = first.eax else {
        panic!("missing IME HWND")
    };
    assert_ne!(handle, 0);
    assert_eq!(
        fixture.runtime.ime_window(handle).unwrap().owner_thread_id,
        RuntimeThreadId(THREAD_ID)
    );
    assert_eq!(fixture.runtime.current_thread_id(), other_thread);
    let context_before = fixture
        .runtime
        .ime_contexts()
        .values()
        .next()
        .unwrap()
        .clone();
    fixture.configure_send_message(handle, 0x0283, 0x21, 0);
    assert!(matches!(
        fixture.dispatch_send_message_basic(),
        DispatchOutcome::Immediate(_)
    ));
    assert_eq!(
        fixture
            .runtime
            .ime_window(handle)
            .unwrap()
            .status_window_request,
        Some(RuntimeImeStatusWindowRequest::Close)
    );
    let ime_id = fixture.runtime.ime_window(handle).unwrap().id;
    fixture.configure_send_message(handle, 0x0283, 0x21, 0);
    assert!(matches!(
        fixture.dispatch_send_message_basic(),
        DispatchOutcome::Immediate(_)
    ));
    assert_eq!(fixture.runtime.ime_window(handle).unwrap().id, ime_id);
    assert_eq!(
        fixture
            .runtime
            .ime_window(handle)
            .unwrap()
            .status_window_request,
        Some(RuntimeImeStatusWindowRequest::Close)
    );
    let app_id = fixture.runtime.window(app_window_handle).unwrap().id;
    let context = fixture.runtime.ime_context_associations()[&app_id]
        .context
        .unwrap()
        .handle;
    let DispatchOutcome::Immediate(detached) =
        fixture.dispatch_imm_associate_context(app_window_handle, 0)
    else {
        panic!("cross-thread context detach failed")
    };
    assert_eq!(detached.eax, Some(context));
    assert_eq!(
        fixture.runtime.ime_context_associations()[&app_id].context,
        None
    );
    let last_error_before = fixture.runtime.current_last_error().unwrap();
    let DispatchOutcome::Immediate(rejected) =
        fixture.dispatch_imm_associate_context(app_window_handle, context)
    else {
        panic!("cross-thread attach did not return a Windows failure")
    };
    assert_eq!(rejected.eax, Some(0));
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );
    assert_eq!(fixture.runtime.current_thread_id(), other_thread);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );
    assert_eq!(
        fixture.runtime.ime_context(context).unwrap(),
        &context_before
    );
    assert_eq!(
        fixture.runtime.ime_context_associations()[&app_id].context,
        None
    );
    assert_eq!(fixture.runtime.ime_window(handle).unwrap().id, ime_id);
    assert_eq!(fixture.runtime.ime_contexts().len(), 1);
    assert_eq!(fixture.runtime.ime_windows().len(), 1);
    fixture
        .runtime
        .select_thread(RuntimeThreadId(THREAD_ID))
        .unwrap();
    let DispatchOutcome::Immediate(repeated) =
        fixture.dispatch_imm_get_default_ime_wnd(app_window_handle)
    else {
        panic!("owner-thread default IME query failed")
    };
    assert_eq!(repeated.eax, Some(handle));
    let DispatchOutcome::Immediate(restored) =
        fixture.dispatch_imm_associate_context(app_window_handle, context)
    else {
        panic!("owner-thread context restore failed")
    };
    assert_eq!(restored.eax, Some(0));
    assert_eq!(
        fixture.runtime.ime_context(context).unwrap(),
        &context_before
    );
    assert_eq!(fixture.runtime.ime_contexts().len(), 1);
    assert_eq!(
        fixture.runtime.ime_context_associations()[&app_id]
            .context
            .unwrap()
            .handle,
        context
    );
}

#[test]
fn imm32_null_invalid_and_wrong_identity_fail_without_partial_resources() {
    let mut fixture = RegisterClassFixture::new();
    let (app_window_handle, _) = fixture.prepare_created_window_and_small_icon();

    fixture.runtime.set_current_last_error(0xa5a5_5a5a).unwrap();
    let null_query = match fixture.dispatch_imm_get_default_ime_wnd(0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected NULL IME window query completion, got {other:?}"),
    };
    assert_eq!(null_query.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0);
    assert!(fixture.runtime.ime_windows().is_empty());
    assert!(fixture.runtime.ime_contexts().is_empty());

    fixture.runtime.set_current_last_error(0xa5a5_5a5a).unwrap();
    let invalid_query = match fixture.dispatch_imm_get_default_ime_wnd(0x1234) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid IME window query completion, got {other:?}"),
    };
    assert_eq!(invalid_query.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
    assert!(fixture.runtime.ime_windows().is_empty());
    assert!(fixture.runtime.ime_contexts().is_empty());

    let ime_window_handle = match fixture.dispatch_imm_get_default_ime_wnd(app_window_handle) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            Some(handle) => handle,
            other => panic!("expected logical IME HWND, got {other:?}"),
        },
        other => panic!("expected valid IME window query, got {other:?}"),
    };
    let app_id = fixture.runtime.window(app_window_handle).unwrap().id;
    let ime_before = fixture
        .runtime
        .ime_window(ime_window_handle)
        .unwrap()
        .clone();
    let association_before = fixture.runtime.ime_context_associations()[&app_id].clone();
    let context_reference = association_before.context.unwrap();
    let last_error_before = fixture.runtime.current_last_error().unwrap();
    fixture.configure_send_message(app_window_handle, 0x0283, 0x21, 0);
    match fixture.dispatch_send_message() {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected wrong IME HWND fault, got {other:?}"),
    };

    assert_eq!(
        fixture.runtime.ime_window(ime_window_handle).unwrap(),
        &ime_before
    );
    assert_eq!(
        fixture.runtime.ime_context_associations()[&app_id],
        association_before
    );
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );

    match fixture.dispatch_imm_associate_context(app_window_handle, 0x494d_ffff) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected unknown logical HIMC fault, got {other:?}"),
    };

    assert_eq!(
        fixture.runtime.ime_window(ime_window_handle).unwrap(),
        &ime_before
    );
    assert_eq!(
        fixture.runtime.ime_context_associations()[&app_id],
        association_before
    );
    assert_eq!(
        fixture
            .runtime
            .ime_context(context_reference.handle)
            .unwrap()
            .id,
        context_reference.context_id
    );
    assert_eq!(fixture.runtime.ime_windows().len(), 1);
    assert_eq!(fixture.runtime.ime_contexts().len(), 1);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );
    let repeated = match fixture.dispatch_imm_get_default_ime_wnd(app_window_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected live IME window retry, got {other:?}"),
    };
    assert_eq!(repeated.eax, Some(ime_window_handle));
    assert_eq!(
        fixture.runtime.ime_context_associations()[&app_id],
        association_before
    );
}

#[test]
fn set_window_pos_shows_a_window_without_an_explicit_small_icon() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, _) = fixture.prepare_created_window_and_small_icon();
    assert!(
        fixture
            .runtime
            .window(window_handle)
            .unwrap()
            .small_icon
            .is_none()
    );
    fixture.configure_set_window_pos(
        [window_handle, 0xffff_fffe, 15_360, 8_640, 0, 0, 0x0559],
        SET_WINDOW_POS_SHOW_RETURN_ADDRESS,
    );
    let DispatchOutcome::GuestCallback(call) =
        fixture.dispatch_set_window_pos(SET_WINDOW_POS_SHOW_RETURN_ADDRESS)
    else {
        panic!("expected position notification");
    };
    fixture.runtime.begin_guest_callback(call).unwrap();
    fixture.acknowledge_pending_delta();
    assert!(matches!(
        fixture
            .runtime
            .complete_guest_callback(&fixture.dispatcher, fixture.callback_sentinel, 0)
            .unwrap(),
        vine_core::GuestCallbackReturn::Continue(_)
    ));
    fixture.acknowledge_pending_delta();
    let result = fixture
        .runtime
        .complete_guest_callback(&fixture.dispatcher, fixture.callback_sentinel, 0)
        .unwrap();
    assert!(
        matches!(result, vine_core::GuestCallbackReturn::Completed(ref completion) if completion.eax == Some(1))
    );
    let window = fixture.runtime.window(window_handle).unwrap();
    assert!(window.visible);
    assert!(window.small_icon.is_none());
    assert_eq!(
        (window.creation.geometry.x, window.creation.geometry.y),
        (15_360, 8_640)
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

    let show_prepared = fixture
        .runtime
        .begin_guest_callback(show_call.clone())
        .expect("enter WM_WINDOWPOSCHANGED callback");
    fixture.acknowledge_pending_delta();
    assert_eq!(show_prepared.scratch_bytes, 28);
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(show_prepared.scratch_address, 28)
            .expect("read WINDOWPOS scratch"),
        show_call.request.scratch
    );
    assert_eq!(
        fixture
            .runtime
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
    let move_prepared = fixture
        .runtime
        .begin_guest_callback(move_call.clone())
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
        match fixture.dispatcher.complete_guest_callback_result(
            move_call,
            0xdead_beef,
            &mut fixture.runtime,
        ) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected completed DefWindowProcA, got {other:?}"),
        }
    };
    assert_eq!(move_completion.eax, Some(0));
    fixture
        .runtime
        .pop_callback_frame(move_prepared.frame_id)
        .expect("retire nested WM_MOVE callback");
    assert_eq!(
        fixture
            .runtime
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
        match fixture
            .dispatcher
            .complete_guest_callback_result(show_call, 0, &mut fixture.runtime)
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
        let callback_runtime = &mut fixture.runtime;
        let prepared = callback_runtime
            .replace_guest_callback(icon_call.clone(), show_prepared.frame_id)
            .expect("replace WM_WINDOWPOSCHANGED with WM_GETICON");
        let action = callback_runtime
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
        match fixture.dispatcher.complete_guest_callback_result(
            icon_call,
            icon_handle,
            &mut fixture.runtime,
        ) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected completed SetWindowPos show, got {other:?}"),
        }
    };
    assert_eq!(show_completion.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x2468_1357);
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
    let show_action = fixture
        .runtime
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
    let hide_prepared = fixture
        .runtime
        .begin_guest_callback(hide_call.clone())
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
    assert_eq!(hide_default.eax, Some(0));
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
        match fixture
            .dispatcher
            .complete_guest_callback_result(hide_call, 0, &mut fixture.runtime)
        {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected completed SetWindowPos hide, got {other:?}"),
        }
    };
    assert_eq!(hide_completion.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x2468_1357);
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
    let hide_action = fixture
        .runtime
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
fn get_foreground_window_reads_process_relation_without_mutation() {
    let mut fixture = RegisterClassFixture::new();
    fixture
        .runtime
        .set_current_last_error(0x1357_2468)
        .expect("seed empty foreground LastError");
    let owner = fixture.runtime.current_thread_id();
    let messages_before = fixture.runtime.queued_thread_messages(owner).unwrap();
    let empty = match fixture.dispatch_get_foreground_window() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected empty foreground completion, got {other:?}"),
    };
    assert_eq!(empty.eax, Some(0));
    assert_eq!(
        empty.stack_pointer,
        GuestAddress(GET_FOREGROUND_WINDOW_STACK_POINTER + 4)
    );
    assert_eq!(fixture.runtime.foreground_window_handle().unwrap(), None);
    assert_eq!(fixture.runtime.active_window_handle().unwrap(), None);
    assert_eq!(fixture.runtime.focused_window_handle().unwrap(), None);
    assert_eq!(
        fixture.runtime.queued_thread_messages(owner).unwrap(),
        messages_before
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x1357_2468);
    let empty_basic = fixture.dispatch_get_foreground_window_basic();
    assert!(
        matches!(empty_basic, DispatchOutcome::Immediate(ref completion)
        if completion.eax == Some(0))
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x1357_2468);

    let (window_handle, _) = fixture.prepare_active_window_with_small_icon();
    fixture
        .runtime
        .set_current_last_error(0x2468_1357)
        .expect("seed active foreground LastError");
    let messages_before = fixture.runtime.queued_thread_messages(owner).unwrap();
    let active = match fixture.dispatch_get_foreground_window() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected active foreground completion, got {other:?}"),
    };
    assert_eq!(active.eax, Some(window_handle));
    assert_eq!(
        active.stack_pointer,
        GuestAddress(GET_FOREGROUND_WINDOW_STACK_POINTER + 4)
    );
    assert_eq!(
        fixture.runtime.foreground_window_handle().unwrap(),
        Some(window_handle)
    );
    assert_eq!(
        fixture.runtime.active_window_handle().unwrap(),
        Some(window_handle)
    );
    assert_eq!(
        fixture.runtime.focused_window_handle().unwrap(),
        Some(window_handle)
    );
    assert_eq!(
        fixture.runtime.queued_thread_messages(owner).unwrap(),
        messages_before
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x2468_1357);
    let active_basic = fixture.dispatch_get_foreground_window_basic();
    assert!(
        matches!(active_basic, DispatchOutcome::Immediate(ref completion)
        if completion.eax == Some(window_handle))
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x2468_1357);
}

#[test]
fn set_focus_same_current_window_returns_previous_handle_without_state_or_message_mutation() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, _) = fixture.prepare_active_window_with_small_icon();
    fixture
        .runtime
        .set_current_last_error(0x2468_1357)
        .expect("seed SetFocus LastError");
    fixture.configure_set_focus(window_handle);
    let owner = fixture.runtime.current_thread_id();
    let messages_before = fixture.runtime.queued_thread_messages(owner).unwrap();
    let memory_before = fixture.runtime.memory.stats();
    let completion = match fixture.dispatch_set_focus() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected same-window SetFocus completion, got {other:?}"),
    };
    assert_eq!(completion.eax, Some(window_handle));
    assert_eq!(
        completion.stack_pointer,
        GuestAddress(SET_FOCUS_STACK_POINTER + 8)
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x2468_1357);
    assert_eq!(
        fixture.runtime.queued_thread_messages(owner).unwrap(),
        messages_before
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x2468_1357);
    assert_eq!(fixture.runtime.memory.stats(), memory_before);
    assert_eq!(
        fixture.runtime.active_window_handle().unwrap(),
        Some(window_handle)
    );
    assert_eq!(
        fixture.runtime.focused_window_handle().unwrap(),
        Some(window_handle)
    );
    assert_eq!(
        fixture.runtime.foreground_window_handle().unwrap(),
        Some(window_handle)
    );
}

#[test]
fn set_focus_rejects_unreached_variants_before_mutation() {
    let mut unfocused = RegisterClassFixture::new();
    let (unfocused_handle, _) = unfocused.prepare_created_window_with_small_icon();
    unfocused.configure_set_focus(unfocused_handle);
    let owner = unfocused.runtime.current_thread_id();
    let messages_before = unfocused.runtime.queued_thread_messages(owner).unwrap();
    let last_error_before = unfocused.runtime.current_last_error().unwrap();
    let memory_before = unfocused.runtime.memory.stats();
    let _ = match unfocused.dispatch_set_focus() {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected bounded relation fault, got {other:?}"),
    };

    assert_eq!(unfocused.runtime.active_window_handle().unwrap(), None);
    assert_eq!(unfocused.runtime.focused_window_handle().unwrap(), None);
    assert_eq!(unfocused.runtime.foreground_window_handle().unwrap(), None);
    assert_eq!(
        unfocused.runtime.queued_thread_messages(owner).unwrap(),
        messages_before
    );
    assert_eq!(
        unfocused.runtime.current_last_error().unwrap(),
        last_error_before
    );
    assert_eq!(unfocused.runtime.memory.stats(), memory_before);

    let mut null_window = RegisterClassFixture::new();
    null_window.configure_set_focus(0);
    let owner = null_window.runtime.current_thread_id();
    let messages_before = null_window.runtime.queued_thread_messages(owner).unwrap();
    let last_error_before = null_window.runtime.current_last_error().unwrap();
    let memory_before = null_window.runtime.memory.stats();
    let _ = match null_window.dispatch_set_focus() {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected NULL SetFocus fault, got {other:?}"),
    };

    assert_eq!(null_window.runtime.active_window_handle().unwrap(), None);
    assert_eq!(null_window.runtime.focused_window_handle().unwrap(), None);
    assert_eq!(
        null_window.runtime.foreground_window_handle().unwrap(),
        None
    );
    assert_eq!(
        null_window.runtime.queued_thread_messages(owner).unwrap(),
        messages_before
    );
    assert_eq!(
        null_window.runtime.current_last_error().unwrap(),
        last_error_before
    );
    assert_eq!(null_window.runtime.memory.stats(), memory_before);
}

#[test]
fn set_window_pos_same_geometry_activation_commits_ordered_focus_and_ime_callbacks() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, _) = fixture.prepare_created_window_with_small_icon();
    let style = match fixture.dispatch_set_window_long_a(window_handle, -16, 0x90ca_0000) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected visible style transition, got {other:?}"),
    };
    assert_eq!(style.eax, Some(0x84ca_0000));
    let initial_window = fixture.runtime.window(window_handle).unwrap().clone();
    assert!(initial_window.visible);
    assert_eq!(fixture.runtime.active_window_handle().unwrap(), None);
    assert_eq!(fixture.runtime.focused_window_handle().unwrap(), None);
    assert_eq!(fixture.runtime.foreground_window_handle().unwrap(), None);

    fixture
        .runtime
        .set_current_last_error(0x1357_2468)
        .expect("seed activation LastError");
    fixture.configure_set_window_pos(
        [window_handle, 0xffff_fffe, 0, 0, 806, 629, 0x0000_010a],
        SET_WINDOW_POS_ACTIVATE_RETURN_ADDRESS,
    );
    let first_position =
        match fixture.dispatch_set_window_pos(SET_WINDOW_POS_ACTIVATE_RETURN_ADDRESS) {
            DispatchOutcome::GuestCallback(call) => call,
            other => panic!("expected first activation WINDOWPOS callback, got {other:?}"),
        };
    assert_eq!(
        first_position.request.arguments,
        vec![
            GuestCallbackArgument::U32(window_handle),
            GuestCallbackArgument::U32(0x0046),
            GuestCallbackArgument::U32(0),
            GuestCallbackArgument::ScratchPointer { offset: 0 },
        ]
    );
    assert_eq!(
        first_position.request.scratch,
        [window_handle, 0xffff_fffe, 0, 0, 806, 629, 0x0000_010a]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    let first_prepared = begin_fixture_callback(&mut fixture, &first_position);
    assert!(matches!(
        fixture.dispatch_def_window_proc(window_handle, 0x0046, first_prepared.scratch_address.0),
        DispatchOutcome::Immediate(_)
    ));
    let (second_position, second_prepared, first_action) =
        replace_fixture_callback(&mut fixture, first_position, 0, first_prepared.frame_id);
    assert!(matches!(
        first_action,
        RuntimeGuestCallbackSemanticAction::SetWindowPos(
            RuntimeSetWindowPosCallbackAction::Continue {
                next_message: RuntimeSetWindowPosCallbackMessage::ActivationWindowPositionChanging,
                ..
            }
        )
    ));
    assert_eq!(
        fixture.runtime.active_window_handle().unwrap(),
        Some(window_handle)
    );
    assert_eq!(fixture.runtime.focused_window_handle().unwrap(), None);
    assert_eq!(
        fixture.runtime.foreground_window_handle().unwrap(),
        Some(window_handle)
    );
    assert_eq!(
        second_position.request.scratch,
        [window_handle, 0, 0, 0, 0, 0, 3]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    assert!(matches!(
        fixture.dispatch_def_window_proc(window_handle, 0x0046, second_prepared.scratch_address.0),
        DispatchOutcome::Immediate(_)
    ));

    let (activate_app, activate_app_prepared, _) =
        replace_fixture_callback(&mut fixture, second_position, 0, second_prepared.frame_id);
    assert_eq!(
        activate_app.request.arguments,
        [window_handle, 0x001c, 1, 0]
            .into_iter()
            .map(GuestCallbackArgument::U32)
            .collect::<Vec<_>>()
    );
    assert!(matches!(
        fixture.dispatch_def_window_proc_args(window_handle, 0x001c, 1, 0),
        DispatchOutcome::Immediate(_)
    ));

    let (nc_activate, nc_activate_prepared, _) = replace_fixture_callback(
        &mut fixture,
        activate_app,
        0,
        activate_app_prepared.frame_id,
    );
    assert_eq!(
        nc_activate.request.arguments,
        [window_handle, 0x0086, 1, 0]
            .into_iter()
            .map(GuestCallbackArgument::U32)
            .collect::<Vec<_>>()
    );
    let nc_default = fixture.dispatch_def_window_proc_args(window_handle, 0x0086, 1, 0);
    assert!(matches!(
        nc_default,
        DispatchOutcome::Immediate(ref completion) if completion.eax == Some(1)
    ));

    let (activate, activate_prepared, _) =
        replace_fixture_callback(&mut fixture, nc_activate, 0, nc_activate_prepared.frame_id);
    assert_eq!(
        activate.request.arguments,
        [window_handle, 0x0006, 1, 0]
            .into_iter()
            .map(GuestCallbackArgument::U32)
            .collect::<Vec<_>>()
    );
    let ime_context = match fixture.dispatch_def_window_proc_args(window_handle, 0x0006, 1, 0) {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected nested WM_IME_SETCONTEXT, got {other:?}"),
    };
    assert_eq!(
        fixture.runtime.focused_window_handle().unwrap(),
        Some(window_handle)
    );
    assert_eq!(
        ime_context.request.arguments,
        [window_handle, 0x0281, 1, 0xc000_000f]
            .into_iter()
            .map(GuestCallbackArgument::U32)
            .collect::<Vec<_>>()
    );
    let ime_prepared = begin_fixture_callback(&mut fixture, &ime_context);
    assert_eq!(
        fixture
            .runtime
            .current_thread_state()
            .unwrap()
            .callback_frames
            .len(),
        2
    );

    let ime_notify =
        match fixture.dispatch_def_window_proc_args(window_handle, 0x0281, 1, 0xc000_000f) {
            DispatchOutcome::GuestCallback(call) => call,
            other => panic!("expected nested WM_IME_NOTIFY, got {other:?}"),
        };
    assert_eq!(
        ime_notify.request.arguments,
        [window_handle, 0x0282, 2, 0]
            .into_iter()
            .map(GuestCallbackArgument::U32)
            .collect::<Vec<_>>()
    );
    let notify_prepared = begin_fixture_callback(&mut fixture, &ime_notify);
    assert_eq!(
        fixture
            .runtime
            .current_thread_state()
            .unwrap()
            .callback_frames
            .len(),
        3
    );
    assert!(matches!(
        fixture.dispatch_def_window_proc_args(window_handle, 0x0282, 2, 0),
        DispatchOutcome::Immediate(_)
    ));
    let (_, notify_action) =
        complete_fixture_callback(&mut fixture, ime_notify, 0, notify_prepared.frame_id);
    assert_eq!(notify_action, None);

    let (set_focus, focus_prepared, ime_action) =
        replace_fixture_callback(&mut fixture, ime_context, 0, ime_prepared.frame_id);
    assert!(matches!(
        ime_action,
        RuntimeGuestCallbackSemanticAction::SetWindowPosDefault(
            RuntimeSetWindowPosDefaultCallbackAction::Continue {
                next_message: RuntimeSetWindowPosDefaultCallbackMessage::SetFocus,
                ..
            }
        )
    ));
    assert_eq!(
        set_focus.request.arguments,
        [window_handle, 0x0007, 0, 0]
            .into_iter()
            .map(GuestCallbackArgument::U32)
            .collect::<Vec<_>>()
    );
    assert!(matches!(
        fixture.dispatch_def_window_proc_args(window_handle, 0x0007, 0, 0),
        DispatchOutcome::Immediate(_)
    ));
    let (activate_default, focus_action) =
        complete_fixture_callback(&mut fixture, set_focus, 0, focus_prepared.frame_id);
    assert_eq!(activate_default.eax, Some(0));
    assert!(matches!(
        focus_action,
        Some(RuntimeGuestCallbackSemanticAction::SetWindowPosDefault(
            RuntimeSetWindowPosDefaultCallbackAction::Completed
        ))
    ));

    let (completion, activation_action) =
        complete_fixture_callback(&mut fixture, activate, 0, activate_prepared.frame_id);
    assert_eq!(completion.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x1357_2468);
    assert_eq!(
        completion.instruction_pointer,
        SET_WINDOW_POS_ACTIVATE_RETURN_ADDRESS
    );
    let Some(RuntimeGuestCallbackSemanticAction::SetWindowPos(
        RuntimeSetWindowPosCallbackAction::Completed { window: activated },
    )) = activation_action
    else {
        panic!("expected completed activation action");
    };
    assert_eq!(activated, initial_window);
    assert_eq!(
        fixture.runtime.window(window_handle).unwrap(),
        &initial_window
    );
    assert_eq!(
        fixture.runtime.active_window_handle().unwrap(),
        Some(window_handle)
    );
    assert_eq!(
        fixture.runtime.focused_window_handle().unwrap(),
        Some(window_handle)
    );
    assert_eq!(
        fixture.runtime.foreground_window_handle().unwrap(),
        Some(window_handle)
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
fn set_window_pos_resize_atomically_updates_window_and_gdi_surface_after_nccalc() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, _) = fixture.prepare_active_window_with_small_icon();
    let previous_window = fixture.runtime.window(window_handle).unwrap().clone();
    assert_eq!(
        previous_window.creation.geometry,
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

    let dc = match fixture.dispatch_get_dc(window_handle) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            Some(handle) => handle,
            other => panic!("expected window HDC, got {other:?}"),
        },
        other => panic!("expected GetDC completion, got {other:?}"),
    };
    let surface_id = fixture
        .runtime
        .gdi_device_context(dc)
        .unwrap()
        .window_surface_id()
        .unwrap();
    let brush = match fixture.dispatch_get_stock_object(4) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            Some(handle) => handle,
            other => panic!("expected BLACK_BRUSH, got {other:?}"),
        },
        other => panic!("expected GetStockObject completion, got {other:?}"),
    };
    fixture
        .runtime
        .memory
        .write(
            CLIENT_RECT_OUTPUT_ADDRESS,
            &[0_i32, 0, 800, 600]
                .into_iter()
                .flat_map(i32::to_le_bytes)
                .collect::<Vec<_>>(),
        )
        .unwrap();
    fixture.acknowledge_pending_delta();
    assert!(matches!(
        fixture.dispatch_fill_rect(dc, CLIENT_RECT_OUTPUT_ADDRESS, brush),
        DispatchOutcome::Immediate(ref completion)
            if completion.eax == Some(1)
    ));
    assert!(matches!(
        fixture.dispatch_release_dc(window_handle, dc),
        DispatchOutcome::Immediate(ref completion)
            if completion.eax == Some(1)
    ));
    let previous_surface = fixture.runtime.gdi_surfaces()[&surface_id].clone();
    assert_eq!(
        (previous_surface.width, previous_surface.height),
        (800, 600)
    );
    assert_eq!(previous_surface.fills.len(), 1);

    fixture.runtime.set_current_last_error(0x2468_ace0).unwrap();
    fixture.configure_set_window_pos(
        [window_handle, 0xffff_fffe, 0, 0, 1_286, 749, 0x0000_010a],
        SET_WINDOW_POS_ACTIVATE_RETURN_ADDRESS,
    );
    let changing = match fixture.dispatch_set_window_pos(SET_WINDOW_POS_ACTIVATE_RETURN_ADDRESS) {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected resize WM_WINDOWPOSCHANGING, got {other:?}"),
    };
    assert_eq!(
        changing.request.scratch,
        [window_handle, 0xffff_fffe, 0, 0, 1_286, 749, 0x0000_010a,]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    let changing_prepared = begin_fixture_callback(&mut fixture, &changing);
    assert!(matches!(
        fixture.dispatch_def_window_proc(
            window_handle,
            0x0046,
            changing_prepared.scratch_address.0,
        ),
        DispatchOutcome::Immediate(_)
    ));
    let (nccalc, nccalc_prepared, changing_action) =
        replace_fixture_callback(&mut fixture, changing, 0, changing_prepared.frame_id);
    assert!(matches!(
        changing_action,
        RuntimeGuestCallbackSemanticAction::SetWindowPos(
            RuntimeSetWindowPosCallbackAction::Continue {
                next_message: RuntimeSetWindowPosCallbackMessage::NonClientCalculate,
                ..
            }
        )
    ));
    assert_eq!(
        nccalc.request.arguments[1],
        GuestCallbackArgument::U32(0x0083)
    );
    assert_eq!(nccalc.request.arguments[2], GuestCallbackArgument::U32(1));
    assert_eq!(nccalc.request.scratch.len(), 80);
    assert_eq!(nccalc.request.scratch_pointer_relocations.len(), 1);
    assert_eq!(
        nccalc.request.scratch_pointer_relocations[0].write_offset,
        48
    );
    assert_eq!(
        nccalc.request.scratch_pointer_relocations[0].target_offset,
        52
    );
    let mut expected_nccalc = [
        557_u32,
        214,
        1_843,
        963,
        557,
        214,
        1_363,
        843,
        560,
        240,
        1_360,
        840,
        0,
        window_handle,
        0xffff_fffe,
        557,
        214,
        1_286,
        749,
        0x0000_190e,
    ]
    .into_iter()
    .flat_map(u32::to_le_bytes)
    .collect::<Vec<_>>();
    expected_nccalc[48..52]
        .copy_from_slice(&(nccalc_prepared.scratch_address.0 + 52).to_le_bytes());
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(nccalc_prepared.scratch_address, 80)
            .unwrap(),
        expected_nccalc
    );
    assert_eq!(
        fixture.runtime.window(window_handle).unwrap(),
        &previous_window
    );
    assert_eq!(
        fixture.runtime.gdi_surfaces()[&surface_id],
        previous_surface
    );

    fixture
        .runtime
        .memory
        .write(
            GuestAddress(nccalc_prepared.scratch_address.0 + 48),
            &0_u32.to_le_bytes(),
        )
        .unwrap();
    fixture.acknowledge_pending_delta();
    let malformed = fixture.dispatch_def_window_proc_args(
        window_handle,
        0x0083,
        1,
        nccalc_prepared.scratch_address.0,
    );
    assert!(matches!(malformed, DispatchOutcome::Fault(_)));
    assert_eq!(
        fixture.runtime.window(window_handle).unwrap(),
        &previous_window
    );
    assert_eq!(
        fixture.runtime.gdi_surfaces()[&surface_id],
        previous_surface
    );
    fixture
        .runtime
        .memory
        .write(
            GuestAddress(nccalc_prepared.scratch_address.0 + 48),
            &(nccalc_prepared.scratch_address.0 + 52).to_le_bytes(),
        )
        .unwrap();
    fixture.acknowledge_pending_delta();
    let nccalc_default = match fixture.dispatch_def_window_proc_args(
        window_handle,
        0x0083,
        1,
        nccalc_prepared.scratch_address.0,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected resize NCCALCSIZE default result, got {other:?}"),
    };
    assert_eq!(nccalc_default.eax, Some(0));
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(nccalc_prepared.scratch_address, 16)
            .unwrap(),
        [560_i32, 240, 1_840, 960]
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        fixture.runtime.window(window_handle).unwrap(),
        &previous_window
    );
    assert_eq!(
        fixture.runtime.gdi_surfaces()[&surface_id],
        previous_surface
    );

    let (size, size_prepared, nccalc_action) =
        replace_fixture_callback(&mut fixture, nccalc, 0, nccalc_prepared.frame_id);
    assert!(matches!(
        nccalc_action,
        RuntimeGuestCallbackSemanticAction::SetWindowPos(
            RuntimeSetWindowPosCallbackAction::Continue {
                next_message: RuntimeSetWindowPosCallbackMessage::Size,
                ..
            }
        )
    ));
    let resized_window = fixture.runtime.window(window_handle).unwrap().clone();
    assert_eq!(
        resized_window.creation.geometry,
        RuntimeWindowGeometry {
            width: 1_286,
            height: 749,
            client_width: 1_280,
            client_height: 720,
            ..previous_window.creation.geometry
        }
    );
    let resized_surface = fixture.runtime.gdi_surfaces()[&surface_id].clone();
    assert_eq!(resized_surface.id, previous_surface.id);
    assert_eq!(
        (resized_surface.width, resized_surface.height),
        (1_280, 720)
    );
    assert!(resized_surface.fills.is_empty());
    assert_eq!(
        size.request.arguments,
        [window_handle, 0x0005, 0, 0x02d0_0500]
            .into_iter()
            .map(GuestCallbackArgument::U32)
            .collect::<Vec<_>>()
    );
    assert!(matches!(
        fixture.dispatch_def_window_proc_args(window_handle, 0x0005, 0, 0x02d0_0500),
        DispatchOutcome::Immediate(_)
    ));

    let (changed, changed_prepared, _) =
        replace_fixture_callback(&mut fixture, size, 0, size_prepared.frame_id);
    assert_eq!(
        changed.request.scratch,
        [
            window_handle,
            0xffff_fffe,
            557,
            214,
            1_286,
            749,
            0x0000_110e,
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>()
    );
    assert!(matches!(
        fixture.dispatch_def_window_proc(
            window_handle,
            0x0047,
            changed_prepared.scratch_address.0,
        ),
        DispatchOutcome::Immediate(ref completion)
            if completion.eax == Some(0)
    ));
    let (completion, action) =
        complete_fixture_callback(&mut fixture, changed, 0, changed_prepared.frame_id);
    assert_eq!(completion.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x2468_ace0);
    assert!(matches!(
        action,
        Some(RuntimeGuestCallbackSemanticAction::SetWindowPos(
            RuntimeSetWindowPosCallbackAction::Completed { .. }
        ))
    ));
    assert_eq!(
        fixture.runtime.window(window_handle).unwrap(),
        &resized_window
    );
    assert_eq!(fixture.runtime.gdi_surfaces()[&surface_id], resized_surface);

    fixture.runtime.set_current_last_error(0x1357_9bdf).unwrap();
    fixture.configure_set_window_pos(
        [window_handle, 0xffff_fffe, 0, 0, 1_286, 749, 0x0000_010a],
        SET_WINDOW_POS_ACTIVATE_RETURN_ADDRESS,
    );
    let repeat = match fixture.dispatch_set_window_pos(SET_WINDOW_POS_ACTIVATE_RETURN_ADDRESS) {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected repeat WM_WINDOWPOSCHANGING, got {other:?}"),
    };
    assert!(matches!(
        &repeat.request.completion_policy,
        GuestCallbackCompletionPolicy::SetWindowPos {
            operation,
            message: RuntimeSetWindowPosCallbackMessage::WindowPositionChanging,
        } if operation.variant == RuntimeSetWindowPosVariant::RepeatActivation
    ));
    assert_eq!(
        repeat.request.scratch,
        [window_handle, 0xffff_fffe, 0, 0, 1_286, 749, 0x0000_010a]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    let repeat_prepared = begin_fixture_callback(&mut fixture, &repeat);
    assert!(matches!(
        fixture.dispatch_def_window_proc(
            window_handle,
            0x0046,
            repeat_prepared.scratch_address.0,
        ),
        DispatchOutcome::Immediate(ref completion)
            if completion.eax == Some(0)
    ));
    let (repeat_completion, repeat_action) =
        complete_fixture_callback(&mut fixture, repeat, 0, repeat_prepared.frame_id);
    assert_eq!(repeat_completion.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x1357_9bdf);
    assert!(matches!(
        repeat_action,
        Some(RuntimeGuestCallbackSemanticAction::SetWindowPos(
            RuntimeSetWindowPosCallbackAction::Completed { ref window }
        )) if window == &resized_window
    ));
    assert_eq!(
        fixture.runtime.window(window_handle).unwrap(),
        &resized_window
    );
    assert_eq!(fixture.runtime.gdi_surfaces()[&surface_id], resized_surface);
    assert_eq!(
        fixture.runtime.active_window_handle().unwrap(),
        Some(window_handle)
    );
    assert_eq!(
        fixture.runtime.focused_window_handle().unwrap(),
        Some(window_handle)
    );
    assert_eq!(
        fixture.runtime.foreground_window_handle().unwrap(),
        Some(window_handle)
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
fn set_window_pos_frame_change_move_drives_show_and_update_paint_lifecycle_atomically() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, _) = fixture.prepare_active_window_with_small_icon();

    fixture.configure_set_window_pos(
        [window_handle, 0xffff_fffe, 0, 0, 1_286, 749, 0x0000_010a],
        SET_WINDOW_POS_ACTIVATE_RETURN_ADDRESS,
    );
    let resize_changing =
        match fixture.dispatch_set_window_pos(SET_WINDOW_POS_ACTIVATE_RETURN_ADDRESS) {
            DispatchOutcome::GuestCallback(call) => call,
            other => panic!("expected resize WM_WINDOWPOSCHANGING, got {other:?}"),
        };
    let resize_changing_prepared = begin_fixture_callback(&mut fixture, &resize_changing);
    assert!(matches!(
        fixture.dispatch_def_window_proc(
            window_handle,
            0x0046,
            resize_changing_prepared.scratch_address.0,
        ),
        DispatchOutcome::Immediate(_)
    ));
    let (resize_nccalc, resize_nccalc_prepared, _) = replace_fixture_callback(
        &mut fixture,
        resize_changing,
        0,
        resize_changing_prepared.frame_id,
    );
    assert!(matches!(
        fixture.dispatch_def_window_proc_args(
            window_handle,
            0x0083,
            1,
            resize_nccalc_prepared.scratch_address.0,
        ),
        DispatchOutcome::Immediate(_)
    ));
    fixture.acknowledge_pending_delta();
    let (resize_size, resize_size_prepared, _) = replace_fixture_callback(
        &mut fixture,
        resize_nccalc,
        0,
        resize_nccalc_prepared.frame_id,
    );
    assert!(matches!(
        fixture.dispatch_def_window_proc_args(window_handle, 0x0005, 0, 0x02d0_0500),
        DispatchOutcome::Immediate(_)
    ));
    let (resize_changed, resize_changed_prepared, _) =
        replace_fixture_callback(&mut fixture, resize_size, 0, resize_size_prepared.frame_id);
    assert!(matches!(
        fixture.dispatch_def_window_proc(
            window_handle,
            0x0047,
            resize_changed_prepared.scratch_address.0,
        ),
        DispatchOutcome::Immediate(_)
    ));
    complete_fixture_callback(
        &mut fixture,
        resize_changed,
        0,
        resize_changed_prepared.frame_id,
    );

    let previous_window = fixture.runtime.window(window_handle).unwrap().clone();
    assert_eq!(
        previous_window.creation.geometry,
        RuntimeWindowGeometry {
            x: 557,
            y: 214,
            width: 1_286,
            height: 749,
            client_x: 560,
            client_y: 240,
            client_width: 1_280,
            client_height: 720,
        }
    );
    let dc = match fixture.dispatch_get_dc(window_handle) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            Some(handle) => handle,
            other => panic!("expected window HDC, got {other:?}"),
        },
        other => panic!("expected GetDC completion, got {other:?}"),
    };
    let surface_id = fixture
        .runtime
        .gdi_device_context(dc)
        .unwrap()
        .window_surface_id()
        .unwrap();
    assert!(matches!(
        fixture.dispatch_release_dc(window_handle, dc),
        DispatchOutcome::Immediate(ref completion)
            if completion.eax == Some(1)
    ));
    let previous_surface = fixture.runtime.gdi_surfaces()[&surface_id].clone();

    fixture.runtime.set_current_last_error(0x1357_9bdf).unwrap();
    fixture.configure_set_window_pos(
        [window_handle, 0, 365, 155, 0, 0, 0x0000_0025],
        SET_WINDOW_POS_ACTIVATE_RETURN_ADDRESS,
    );
    let changing = match fixture.dispatch_set_window_pos(SET_WINDOW_POS_ACTIVATE_RETURN_ADDRESS) {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected frame-change WM_WINDOWPOSCHANGING, got {other:?}"),
    };
    assert!(matches!(
        changing.request.completion_policy,
        GuestCallbackCompletionPolicy::SetWindowPos {
            operation: RuntimeSetWindowPosOperation {
                variant: RuntimeSetWindowPosVariant::FrameChangeMove,
                ..
            },
            message: RuntimeSetWindowPosCallbackMessage::WindowPositionChanging,
        }
    ));
    assert_eq!(
        changing.request.scratch,
        [window_handle, 0, 365, 155, 0, 0, 0x0000_0025]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    let changing_prepared = begin_fixture_callback(&mut fixture, &changing);
    assert!(matches!(
        fixture.dispatch_def_window_proc(
            window_handle,
            0x0046,
            changing_prepared.scratch_address.0,
        ),
        DispatchOutcome::Immediate(ref completion)
            if completion.eax == Some(0)
    ));
    let (nccalc, nccalc_prepared, changing_action) =
        replace_fixture_callback(&mut fixture, changing, 0, changing_prepared.frame_id);
    assert!(matches!(
        changing_action,
        RuntimeGuestCallbackSemanticAction::SetWindowPos(
            RuntimeSetWindowPosCallbackAction::Continue {
                next_message: RuntimeSetWindowPosCallbackMessage::NonClientCalculate,
                ..
            }
        )
    ));
    let mut expected_nccalc = [
        365_u32,
        155,
        1_651,
        904,
        557,
        214,
        1_843,
        963,
        560,
        240,
        1_840,
        960,
        0,
        window_handle,
        0,
        365,
        155,
        1_286,
        749,
        0x0000_1825,
    ]
    .into_iter()
    .flat_map(u32::to_le_bytes)
    .collect::<Vec<_>>();
    expected_nccalc[48..52]
        .copy_from_slice(&(nccalc_prepared.scratch_address.0 + 52).to_le_bytes());
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(nccalc_prepared.scratch_address, 80)
            .unwrap(),
        expected_nccalc
    );
    assert_eq!(
        fixture.runtime.window(window_handle).unwrap(),
        &previous_window
    );
    assert_eq!(
        fixture.runtime.gdi_surfaces()[&surface_id],
        previous_surface
    );

    let default_nccalc = match fixture.dispatch_def_window_proc_args(
        window_handle,
        0x0083,
        1,
        nccalc_prepared.scratch_address.0,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected frame-change NCCALCSIZE default result, got {other:?}"),
    };
    assert_eq!(default_nccalc.eax, Some(0));
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(nccalc_prepared.scratch_address, 16)
            .unwrap(),
        [368_i32, 181, 1_648, 901]
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect::<Vec<_>>()
    );

    let (changed, changed_prepared, nccalc_action) =
        replace_fixture_callback(&mut fixture, nccalc, 0, nccalc_prepared.frame_id);
    assert!(matches!(
        nccalc_action,
        RuntimeGuestCallbackSemanticAction::SetWindowPos(
            RuntimeSetWindowPosCallbackAction::Continue {
                next_message: RuntimeSetWindowPosCallbackMessage::WindowPositionChanged,
                ..
            }
        )
    ));
    let moved_window = fixture.runtime.window(window_handle).unwrap().clone();
    assert_eq!(
        moved_window.creation.geometry,
        RuntimeWindowGeometry {
            x: 365,
            y: 155,
            client_x: 368,
            client_y: 181,
            ..previous_window.creation.geometry
        }
    );
    assert_eq!(
        fixture.runtime.gdi_surfaces()[&surface_id],
        previous_surface
    );
    assert_eq!(
        changed.request.scratch,
        [window_handle, 0, 365, 155, 1_286, 749, 0x0000_0825,]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );

    let move_call = match fixture.dispatch_def_window_proc(
        window_handle,
        0x0047,
        changed_prepared.scratch_address.0,
    ) {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected nested frame-change WM_MOVE, got {other:?}"),
    };
    assert_eq!(
        move_call.request.arguments,
        [window_handle, 0x0003, 0, 0x00b5_0170]
            .into_iter()
            .map(GuestCallbackArgument::U32)
            .collect::<Vec<_>>()
    );
    let move_prepared = begin_fixture_callback(&mut fixture, &move_call);
    assert!(matches!(
        fixture.dispatch_def_window_proc_args(window_handle, 0x0003, 0, 0x00b5_0170),
        DispatchOutcome::Immediate(ref completion)
            if completion.eax == Some(0)
    ));
    let (move_completion, move_action) =
        complete_fixture_callback(&mut fixture, move_call, 0, move_prepared.frame_id);
    assert_eq!(move_completion.eax, Some(0));
    assert_eq!(move_action, None);

    let (completion, action) =
        complete_fixture_callback(&mut fixture, changed, 0, changed_prepared.frame_id);
    assert_eq!(completion.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x1357_9bdf);
    assert!(matches!(
        action,
        Some(RuntimeGuestCallbackSemanticAction::SetWindowPos(
            RuntimeSetWindowPosCallbackAction::Completed { ref window }
        )) if window == &moved_window
    ));
    assert_eq!(
        fixture.runtime.window(window_handle).unwrap(),
        &moved_window
    );
    assert_eq!(
        fixture.runtime.gdi_surfaces()[&surface_id],
        previous_surface
    );
    assert_eq!(
        fixture.runtime.active_window_handle().unwrap(),
        Some(window_handle)
    );
    assert_eq!(
        fixture.runtime.focused_window_handle().unwrap(),
        Some(window_handle)
    );
    assert_eq!(
        fixture.runtime.foreground_window_handle().unwrap(),
        Some(window_handle)
    );
    fixture.runtime.set_current_last_error(0x2468_ace0).unwrap();

    let show = match fixture.dispatch_show_window(window_handle, 1) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected immediate ShowWindow completion, got {other:?}"),
    };
    assert_eq!(show.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x2468_ace0);
    assert_eq!(show.instruction_pointer, SHOW_WINDOW_RETURN_ADDRESS);
    assert_eq!(
        show.stack_pointer,
        GuestAddress(SHOW_WINDOW_STACK_POINTER + 12)
    );
    assert_eq!(
        fixture.runtime.window(window_handle).unwrap(),
        &moved_window
    );
    assert_eq!(
        fixture.runtime.gdi_surfaces()[&surface_id],
        previous_surface
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x2468_ace0);
    assert_eq!(
        fixture
            .runtime
            .window(window_handle)
            .unwrap()
            .pending_update_rect,
        Some(RuntimeRect {
            left: 0,
            top: 0,
            right: 1_280,
            bottom: 720,
        })
    );

    let paint = match fixture.dispatch_update_window(window_handle) {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected synchronous UpdateWindow callback, got {other:?}"),
    };
    assert_eq!(paint.request.target, fixture.window_procedure);
    assert_eq!(
        paint.request.arguments,
        [window_handle, 0x000f, 0, 0]
            .into_iter()
            .map(GuestCallbackArgument::U32)
            .collect::<Vec<_>>()
    );
    let paint_prepared = begin_fixture_callback(&mut fixture, &paint);
    assert_eq!(
        fixture
            .runtime
            .window(window_handle)
            .unwrap()
            .pending_update_rect,
        Some(RuntimeRect {
            left: 0,
            top: 0,
            right: 1_280,
            bottom: 720,
        }),
        "WM_PAINT entry must not consume the update region"
    );

    // Preserve a real preceding Windows failure through the synchronous paint cycle.
    let DispatchOutcome::Immediate(_) = fixture.dispatch_get_dc(0x4801_ffff) else {
        panic!("invalid HWND did not return a Windows failure inside WM_PAINT")
    };
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
    fixture
        .runtime
        .memory
        .write(PAINT_STRUCT_ADDRESS, &[0xa5; 64])
        .expect("seed PAINTSTRUCT storage");
    fixture.acknowledge_pending_delta();
    let begin = match fixture.dispatch_begin_paint(window_handle, PAINT_STRUCT_ADDRESS) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected BeginPaint completion, got {other:?}"),
    };
    let paint_dc = match begin.eax {
        Some(handle) if handle != 0 => handle,
        other => panic!("expected logical paint HDC, got {other:?}"),
    };
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
    assert_eq!(begin.instruction_pointer, BEGIN_PAINT_RETURN_ADDRESS);
    assert_eq!(
        begin.stack_pointer,
        GuestAddress(BEGIN_PAINT_STACK_POINTER + 12)
    );
    let mut expected_paint_struct = Vec::with_capacity(64);
    expected_paint_struct.extend_from_slice(&paint_dc.to_le_bytes());
    expected_paint_struct.extend_from_slice(&1_u32.to_le_bytes());
    for value in [0_i32, 0, 1_280, 720] {
        expected_paint_struct.extend_from_slice(&value.to_le_bytes());
    }
    expected_paint_struct.extend_from_slice(&[0; 40]);
    assert_eq!(expected_paint_struct.len(), 64);
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(PAINT_STRUCT_ADDRESS, 64)
            .unwrap(),
        expected_paint_struct
    );
    let window_during_paint = fixture.runtime.window(window_handle).unwrap();
    assert!(window_during_paint.pending_update_rect.is_none());
    assert_eq!(window_during_paint.paint_generation, 0);
    let paint_session = fixture.runtime.paint_sessions()[&window_during_paint.id].clone();
    assert_eq!(paint_session.device_context_handle, paint_dc);
    assert_eq!(paint_session.paint_struct_address, PAINT_STRUCT_ADDRESS);
    assert_eq!(paint_session.paint_struct_bytes, expected_paint_struct);
    assert_eq!(paint_session.paint_generation, 1);
    assert!(matches!(
        fixture.runtime.gdi_device_contexts()[&paint_session.device_context_id].target,
        RuntimeDeviceContextTarget::Window {
            window_id,
            window_handle: target_handle,
            surface_id: target_surface,
        } if window_id == window_during_paint.id
            && target_handle == window_handle
            && target_surface == surface_id
    ));

    let paint_window_id = window_during_paint.id;
    let _ = match fixture.dispatch_release_dc(window_handle, paint_dc) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected paint-HDC ReleaseDC rejection, got {other:?}"),
    };

    assert_eq!(
        fixture.runtime.paint_sessions().get(&paint_window_id),
        Some(&paint_session)
    );
    assert_eq!(
        fixture.runtime.gdi_device_context(paint_dc).unwrap().id,
        paint_session.device_context_id
    );
    assert_eq!(
        fixture
            .runtime
            .window(window_handle)
            .unwrap()
            .paint_generation,
        0
    );
    assert!(
        fixture
            .runtime
            .window(window_handle)
            .unwrap()
            .pending_update_rect
            .is_none()
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
    assert_eq!(
        fixture.runtime.scheduler_state().unwrap(),
        SchedulerThreadState::InCallback { depth: 1 }
    );
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(PAINT_STRUCT_ADDRESS, 64)
            .unwrap(),
        expected_paint_struct
    );

    let mut corrupted_paint_struct = expected_paint_struct.clone();
    corrupted_paint_struct[4] = 0;
    fixture
        .runtime
        .memory
        .write(PAINT_STRUCT_ADDRESS, &corrupted_paint_struct)
        .expect("corrupt PAINTSTRUCT for negative case");
    fixture.acknowledge_pending_delta();

    let _ = match fixture.dispatch_end_paint(window_handle, PAINT_STRUCT_ADDRESS) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected mismatched EndPaint rejection, got {other:?}"),
    };

    assert_eq!(
        fixture.runtime.paint_sessions().get(&paint_window_id),
        Some(&paint_session)
    );
    assert_eq!(
        fixture.runtime.gdi_device_context(paint_dc).unwrap().id,
        paint_session.device_context_id
    );
    assert_eq!(
        fixture
            .runtime
            .window(window_handle)
            .unwrap()
            .paint_generation,
        0
    );
    assert!(
        fixture
            .runtime
            .window(window_handle)
            .unwrap()
            .pending_update_rect
            .is_none()
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
    assert_eq!(
        fixture.runtime.scheduler_state().unwrap(),
        SchedulerThreadState::InCallback { depth: 1 }
    );
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(PAINT_STRUCT_ADDRESS, 64)
            .unwrap(),
        corrupted_paint_struct
    );
    fixture
        .runtime
        .memory
        .write(PAINT_STRUCT_ADDRESS, &expected_paint_struct)
        .expect("restore PAINTSTRUCT");
    fixture.acknowledge_pending_delta();

    let end = match fixture.dispatch_end_paint(window_handle, PAINT_STRUCT_ADDRESS) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected EndPaint completion, got {other:?}"),
    };
    assert_eq!(end.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
    assert_eq!(end.instruction_pointer, END_PAINT_RETURN_ADDRESS);
    assert_eq!(
        end.stack_pointer,
        GuestAddress(END_PAINT_STACK_POINTER + 12)
    );
    assert!(fixture.runtime.paint_sessions().is_empty());
    assert!(
        !fixture
            .runtime
            .gdi_device_contexts()
            .contains_key(&paint_session.device_context_id)
    );
    assert_eq!(
        fixture
            .runtime
            .window(window_handle)
            .unwrap()
            .paint_generation,
        1
    );

    let (paint_completion, paint_action) =
        complete_fixture_callback(&mut fixture, paint, 0x1357_9bdf, paint_prepared.frame_id);
    assert_eq!(paint_completion.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x2468_ace0);
    assert_eq!(
        paint_completion.instruction_pointer,
        UPDATE_WINDOW_RETURN_ADDRESS
    );
    assert_eq!(
        paint_completion.stack_pointer,
        GuestAddress(UPDATE_WINDOW_STACK_POINTER + 8)
    );
    assert_eq!(paint_action, None);
    assert!(
        fixture
            .runtime
            .window(window_handle)
            .unwrap()
            .pending_update_rect
            .is_none()
    );
    assert_eq!(
        fixture.runtime.gdi_surfaces()[&surface_id],
        previous_surface
    );
}

#[test]
fn show_window_rejects_unreached_command_or_handle_and_accepts_visible_window_noop() {
    let mut invalid_handle = RegisterClassFixture::new();
    assert!(invalid_handle.runtime.windows().is_empty());
    let completion = match invalid_handle.dispatch_show_window(0x1234, 1) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected invalid-HWND ShowWindow completion, got {other:?}"),
    };
    assert_eq!(completion.eax, Some(0));
    assert_eq!(invalid_handle.runtime.current_last_error().unwrap(), 1400);
    assert!(invalid_handle.runtime.windows().is_empty());

    let last_error_before = invalid_handle.runtime.current_last_error().unwrap();
    match invalid_handle.dispatch_show_window(0x1234, 5) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected unsupported ShowWindow command, got {other:?}"),
    };

    assert!(invalid_handle.runtime.windows().is_empty());
    assert_eq!(
        invalid_handle.runtime.current_last_error().unwrap(),
        last_error_before
    );

    let mut wrong_state = RegisterClassFixture::new();
    let (window_handle, _) = wrong_state.prepare_active_window_with_small_icon();
    let window_before = wrong_state.runtime.window(window_handle).unwrap().clone();
    let owner = wrong_state.runtime.current_thread_id();
    let queue_before = wrong_state.runtime.queued_thread_messages(owner).unwrap();
    let last_error_before = wrong_state.runtime.current_last_error().unwrap();
    let no_op = match wrong_state.dispatch_show_window(window_handle, 1) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected visible ShowWindow completion, got {other:?}"),
    };
    assert_eq!(no_op.eax, Some(1));
    assert_eq!(
        wrong_state.runtime.window(window_handle).unwrap(),
        &window_before
    );
    assert_eq!(
        wrong_state.runtime.queued_thread_messages(owner).unwrap(),
        queue_before
    );
    assert_eq!(
        wrong_state.runtime.current_last_error().unwrap(),
        last_error_before
    );
    assert_eq!(
        wrong_state.runtime.active_window_handle().unwrap(),
        Some(window_handle)
    );
    assert_eq!(
        wrong_state.runtime.focused_window_handle().unwrap(),
        Some(window_handle)
    );
    assert_eq!(
        wrong_state.runtime.foreground_window_handle().unwrap(),
        Some(window_handle)
    );
}

#[test]
fn set_window_pos_resize_rejects_live_window_dc_before_window_or_surface_commit() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, _) = fixture.prepare_active_window_with_small_icon();
    let dc = match fixture.dispatch_get_dc(window_handle) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            Some(handle) => handle,
            other => panic!("expected live window HDC, got {other:?}"),
        },
        other => panic!("expected GetDC completion, got {other:?}"),
    };
    fixture.configure_set_window_pos(
        [window_handle, 0xffff_fffe, 0, 0, 1_286, 749, 0x0000_010a],
        SET_WINDOW_POS_ACTIVATE_RETURN_ADDRESS,
    );
    let changing = match fixture.dispatch_set_window_pos(SET_WINDOW_POS_ACTIVATE_RETURN_ADDRESS) {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected resize WM_WINDOWPOSCHANGING, got {other:?}"),
    };
    let changing_prepared = begin_fixture_callback(&mut fixture, &changing);
    assert!(matches!(
        fixture.dispatch_def_window_proc(
            window_handle,
            0x0046,
            changing_prepared.scratch_address.0,
        ),
        DispatchOutcome::Immediate(_)
    ));
    let (nccalc, nccalc_prepared, _) =
        replace_fixture_callback(&mut fixture, changing, 0, changing_prepared.frame_id);
    assert!(matches!(
        fixture.dispatch_def_window_proc_args(
            window_handle,
            0x0083,
            1,
            nccalc_prepared.scratch_address.0,
        ),
        DispatchOutcome::Immediate(_)
    ));
    fixture.acknowledge_pending_delta();
    let window_before = fixture.runtime.window(window_handle).unwrap().clone();
    let surface_before = fixture
        .runtime
        .gdi_surfaces()
        .values()
        .next()
        .unwrap()
        .clone();
    let dc_id = fixture.runtime.gdi_device_context(dc).unwrap().id;
    let last_error_before = fixture.runtime.current_last_error().unwrap();
    match fixture
        .dispatcher
        .complete_guest_callback_result(nccalc, 0, &mut fixture.runtime)
    {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected live-HDC resize fault, got {other:?}"),
    };

    assert_eq!(
        fixture.runtime.window(window_handle).unwrap(),
        &window_before
    );
    assert_eq!(
        fixture.runtime.gdi_surfaces()[&surface_before.id],
        surface_before
    );
    assert_eq!(fixture.runtime.gdi_device_context(dc).unwrap().id, dc_id);
    assert_eq!(
        fixture.runtime.scheduler_state().unwrap(),
        SchedulerThreadState::InCallback { depth: 1 }
    );
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );
    assert_eq!(
        fixture
            .runtime
            .window(window_handle)
            .unwrap()
            .creation
            .geometry,
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
    assert_eq!(
        fixture
            .runtime
            .gdi_surfaces()
            .values()
            .map(|surface| (surface.width, surface.height))
            .collect::<Vec<_>>(),
        vec![(800, 600)]
    );
    fixture
        .runtime
        .pop_callback_frame(nccalc_prepared.frame_id)
        .unwrap();
    assert!(matches!(
        fixture.dispatch_release_dc(window_handle, dc),
        DispatchOutcome::Immediate(ref completion)
            if completion.eax == Some(1)
    ));
}

#[test]
fn set_window_pos_callback_result_failures_do_not_publish_window_state() {
    let mut wrong_activation_result = RegisterClassFixture::new();
    let (activation_window, _) = wrong_activation_result.prepare_created_window_with_small_icon();
    assert!(matches!(
        wrong_activation_result.dispatch_set_window_long_a(activation_window, -16, 0x90ca_0000,),
        DispatchOutcome::Immediate(_)
    ));
    wrong_activation_result.configure_set_window_pos(
        [activation_window, 0xffff_fffe, 0, 0, 806, 629, 0x0000_010a],
        SET_WINDOW_POS_ACTIVATE_RETURN_ADDRESS,
    );
    let activation_call = match wrong_activation_result
        .dispatch_set_window_pos(SET_WINDOW_POS_ACTIVATE_RETURN_ADDRESS)
    {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected activation callback, got {other:?}"),
    };
    let activation_prepared =
        begin_fixture_callback(&mut wrong_activation_result, &activation_call);
    let window_before = wrong_activation_result
        .runtime
        .window(activation_window)
        .unwrap()
        .clone();
    let last_error_before = wrong_activation_result
        .runtime
        .current_last_error()
        .unwrap();
    match wrong_activation_result
        .dispatcher
        .complete_guest_callback_result(activation_call, 1, &mut wrong_activation_result.runtime)
    {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected invalid activation callback result, got {other:?}"),
    };

    assert_eq!(
        wrong_activation_result
            .runtime
            .window(activation_window)
            .unwrap(),
        &window_before
    );
    assert_eq!(
        wrong_activation_result
            .runtime
            .current_last_error()
            .unwrap(),
        last_error_before
    );
    assert_eq!(
        wrong_activation_result.runtime.scheduler_state().unwrap(),
        SchedulerThreadState::InCallback { depth: 1 }
    );
    assert_eq!(
        wrong_activation_result
            .runtime
            .active_window_handle()
            .unwrap(),
        None
    );
    assert_eq!(
        wrong_activation_result
            .runtime
            .focused_window_handle()
            .unwrap(),
        None
    );
    assert_eq!(
        wrong_activation_result
            .runtime
            .foreground_window_handle()
            .unwrap(),
        None
    );
    wrong_activation_result
        .runtime
        .pop_callback_frame(activation_prepared.frame_id)
        .expect("retire rejected activation callback");

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
    let prepared = wrong_position_result
        .runtime
        .begin_guest_callback(call.clone())
        .expect("enter SetWindowPos callback");
    wrong_position_result.acknowledge_pending_delta();
    let window_before = wrong_position_result
        .runtime
        .window(window_handle)
        .unwrap()
        .clone();
    let last_error_before = wrong_position_result.runtime.current_last_error().unwrap();
    let memory_before = wrong_position_result.runtime.memory.stats();
    let reflection_before = wrong_position_result
        .runtime
        .memory
        .current_reflection()
        .expect("callback reflection");
    match wrong_position_result
        .dispatcher
        .complete_guest_callback_result(call, 1, &mut wrong_position_result.runtime)
    {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected invalid position callback result, got {other:?}"),
    };

    assert_eq!(
        wrong_position_result.runtime.window(window_handle).unwrap(),
        &window_before
    );
    assert_eq!(
        wrong_position_result.runtime.current_last_error().unwrap(),
        last_error_before
    );
    assert_eq!(
        wrong_position_result.runtime.scheduler_state().unwrap(),
        SchedulerThreadState::InCallback { depth: 1 }
    );
    assert_eq!(wrong_position_result.runtime.memory.stats(), memory_before);
    assert_eq!(
        wrong_position_result
            .runtime
            .memory
            .current_reflection()
            .expect("unchanged callback reflection"),
        reflection_before
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
    let position_prepared = wrong_icon_result
        .runtime
        .begin_guest_callback(position_call.clone())
        .expect("enter position callback");
    wrong_icon_result.acknowledge_pending_delta();
    let position_policy = position_call.request.completion_policy;
    let position_endpoint = position_call.frame.endpoint.clone();
    let icon_call = {
        match wrong_icon_result.dispatcher.complete_guest_callback_result(
            position_call,
            0,
            &mut wrong_icon_result.runtime,
        ) {
            DispatchOutcome::GuestCallback(call) => call,
            other => panic!("expected WM_GETICON replacement, got {other:?}"),
        }
    };
    let icon_prepared = {
        let callback_runtime = &mut wrong_icon_result.runtime;
        let prepared = callback_runtime
            .replace_guest_callback(icon_call.clone(), position_prepared.frame_id)
            .expect("replace position callback");
        assert!(matches!(
            callback_runtime
                .commit_guest_callback_semantics(position_policy, 0, &position_endpoint)
                .expect("commit position continuation"),
            Some(RuntimeGuestCallbackSemanticAction::SetWindowPos(
                RuntimeSetWindowPosCallbackAction::Continue { .. }
            ))
        ));
        prepared
    };
    wrong_icon_result.acknowledge_pending_delta();
    let window_before = wrong_icon_result
        .runtime
        .window(window_handle)
        .unwrap()
        .clone();
    let last_error_before = wrong_icon_result.runtime.current_last_error().unwrap();
    let memory_before = wrong_icon_result.runtime.memory.stats();
    let reflection_before = wrong_icon_result
        .runtime
        .memory
        .current_reflection()
        .expect("replacement reflection");
    match wrong_icon_result.dispatcher.complete_guest_callback_result(
        icon_call,
        icon_handle.wrapping_add(1),
        &mut wrong_icon_result.runtime,
    ) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected invalid WM_GETICON result, got {other:?}"),
    };

    assert_eq!(
        wrong_icon_result.runtime.window(window_handle).unwrap(),
        &window_before
    );
    assert_eq!(
        wrong_icon_result.runtime.current_last_error().unwrap(),
        last_error_before
    );
    assert_eq!(
        wrong_icon_result.runtime.scheduler_state().unwrap(),
        SchedulerThreadState::InCallback { depth: 1 }
    );
    assert_eq!(wrong_icon_result.runtime.memory.stats(), memory_before);
    assert_eq!(
        wrong_icon_result
            .runtime
            .memory
            .current_reflection()
            .expect("unchanged replacement reflection"),
        reflection_before
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
fn send_message_a_accepts_live_shared_icon_without_a_fixed_size_profile() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, small_icon) = fixture.prepare_created_window_and_small_icon();
    assert_ne!(small_icon, 0x4901_0001);
    let windows_before = fixture.runtime.windows().clone();
    fixture.configure_send_message(window_handle, 0x0080, 0, 0x4901_0001);
    let call = match fixture.dispatch_send_message() {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected live shared icon callback, got {other:?}"),
    };
    assert_eq!(call.request.target, fixture.window_procedure);
    assert_eq!(
        call.request.arguments,
        [window_handle, 0x0080, 0, 0x4901_0001]
            .into_iter()
            .map(GuestCallbackArgument::U32)
            .collect::<Vec<_>>()
    );
    assert_eq!(fixture.runtime.windows(), &windows_before);
}

#[test]
fn send_message_a_rejects_wrong_identity_and_variant_atomically() {
    let mut fixture = RegisterClassFixture::new();
    let (window_handle, icon_handle) = fixture.prepare_created_window_and_small_icon();

    for (message, w_param, _) in [
        (0x0081, 0, "send-message-variant-unsupported"),
        (0x0080, 1, "send-message-variant-unsupported"),
    ] {
        fixture.configure_send_message(window_handle, message, w_param, icon_handle);
        assert_send_message_fault_is_atomic(&mut fixture);
    }

    fixture.configure_send_message(0, 0x0080, 0, icon_handle);
    assert_send_message_fault_is_atomic(&mut fixture);
    for invalid_window in [0x4301_0001, 0x4801_ffff] {
        fixture.configure_send_message(invalid_window, 0x0080, 0, icon_handle);
        assert_send_message_fault_is_atomic(&mut fixture);
    }

    for invalid_icon in [0, 0x4301_0001, 0x4901_ffff] {
        fixture.configure_send_message(window_handle, 0x0080, 0, invalid_icon);
        assert_send_message_fault_is_atomic(&mut fixture);
    }
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
            if completion.eax == Some(0x4901_0002)
    ));
    assert!(matches!(
        creating.dispatch_create(),
        DispatchOutcome::GuestCallback(_)
    ));
    let creating_handle = creating.runtime.windows().values().next().unwrap().handle;
    creating.configure_send_message(creating_handle, 0x0080, 0, 0x4901_0002);
    assert_send_message_fault_is_atomic(&mut creating);

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
    assert_send_message_fault_is_atomic(&mut cross_thread);

    let mut callback_preflight = RegisterClassFixture::new();
    let (window_handle, icon_handle) = callback_preflight.prepare_created_window_and_small_icon();
    callback_preflight.callback_sentinel = MEMORY_BASE;
    callback_preflight.configure_send_message(window_handle, 0x0080, 0, icon_handle);
    assert_send_message_fault_is_atomic(&mut callback_preflight);
}

#[test]
fn create_window_ex_a_invalid_instance_words_return_native_failure_without_publication() {
    let mut fixture = RegisterClassFixture::new();
    fixture.prepare_resources();
    assert!(matches!(
        fixture.dispatch_class(0),
        DispatchOutcome::Immediate(_)
    ));
    for instance in [0, ICON_IMAGE_BASE + 1] {
        fixture
            .runtime
            .memory
            .write(
                GuestAddress(CREATE_STACK_POINTER + 11 * 4),
                &instance.to_le_bytes(),
            )
            .unwrap();
        fixture.acknowledge_pending_delta();
        fixture.runtime.set_current_last_error(0x1234).unwrap();
        let memory_before = fixture.runtime.memory.stats();
        let outcome = fixture.dispatch_create();
        assert!(matches!(outcome, DispatchOutcome::Immediate(ref result) if result.eax == Some(0)));
        assert_eq!(fixture.runtime.current_last_error().unwrap(), 87);
        assert!(fixture.runtime.windows().is_empty());
        assert!(
            fixture
                .runtime
                .current_thread_state()
                .unwrap()
                .callback_frames
                .is_empty()
        );
        assert_eq!(fixture.runtime.memory.stats(), memory_before);
    }
    fixture
        .runtime
        .memory
        .write(
            GuestAddress(CREATE_STACK_POINTER + 11 * 4),
            &ICON_IMAGE_BASE.to_le_bytes(),
        )
        .unwrap();
    fixture.acknowledge_pending_delta();
    assert!(matches!(
        fixture.dispatch_create(),
        DispatchOutcome::GuestCallback(_)
    ));
    assert_eq!(
        fixture.runtime.windows().values().next().unwrap().handle,
        0x4801_0001
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
    match preflight.dispatch_create() {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected callback preflight fault, got {other:?}"),
    };
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
    let prepared = preflight
        .runtime
        .begin_guest_callback(call.clone())
        .expect("enter rejecting callback");
    preflight.acknowledge_pending_delta();
    let completion_policy = call.request.completion_policy;
    let endpoint = call.frame.endpoint.clone();
    let completion = {
        preflight
            .dispatcher
            .complete_guest_callback_result(call, 0, &mut preflight.runtime)
    };
    assert!(matches!(
        completion,
        DispatchOutcome::Immediate(ref completion)
            if completion.eax == Some(0) && preflight.runtime.current_last_error().unwrap() == 87
    ));
    preflight
        .runtime
        .pop_callback_frame(prepared.frame_id)
        .expect("pop rejected callback");
    let action = preflight
        .runtime
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
    preflight
        .runtime
        .window(0x4801_0001)
        .expect_err("rejected HWND must be retired");
}

#[test]
fn windows_932_class_and_title_survive_create_window_callback_preparation() {
    let mut fixture = RegisterClassFixture::new();
    fixture.prepare_resources();
    let registered = match fixture.dispatch_class(9) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("Windows-932 class registration stopped: {other:?}"),
    };
    assert_eq!(registered.eax, Some(0xc000));
    let class = fixture.runtime.window_class(0xc000).unwrap();
    assert_eq!(class.registration.class_name, "あ");
    let module_id = class.registration.module.module_id;
    let class_id = class.id;
    assert_eq!(
        fixture
            .runtime
            .window_class_by_name(module_id, "あ")
            .unwrap()
            .id,
        class_id
    );

    fixture
        .runtime
        .memory
        .write(
            GuestAddress(CREATE_STACK_POINTER + 8),
            &(CLASS_NAME_BASE + 9 * 0x40).to_le_bytes(),
        )
        .unwrap();
    fixture
        .runtime
        .memory
        .write(GuestAddress(WINDOW_TITLE_BASE), &[0x82, 0x20, 0])
        .unwrap();
    fixture.acknowledge_pending_delta();
    let last_error_before = fixture.runtime.current_last_error().unwrap();
    let memory_before = fixture.runtime.memory.stats();
    assert!(matches!(
        fixture.dispatch_create(),
        DispatchOutcome::Fault(_)
    ));
    assert!(fixture.runtime.windows().is_empty());
    assert_eq!(fixture.runtime.window_class(0xc000).unwrap().id, class_id);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );
    assert_eq!(
        fixture.runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(fixture.runtime.memory.stats(), memory_before);
    fixture
        .runtime
        .memory
        .write(
            GuestAddress(WINDOW_TITLE_BASE),
            &[0x93, 0xfa, 0x96, 0x7b, 0x8c, 0xea, 0],
        )
        .unwrap();
    fixture.acknowledge_pending_delta();
    assert!(matches!(
        fixture.dispatch_create(),
        DispatchOutcome::GuestCallback(_)
    ));
    let window = fixture.runtime.windows().values().next().unwrap();
    assert_eq!(window.handle, 0x4801_0001);
    assert_eq!(window.creation.name.value, "日本語");
    assert_eq!(window.creation.class.class_id, class_id);
}

#[test]
fn register_class_ex_a_negative_paths_are_failure_atomic_and_do_not_consume_atom() {
    for index in [3_u32, 4, 5, 6, 7] {
        let mut fixture = RegisterClassFixture::new();
        fixture.prepare_resources();
        let generation = fixture.runtime.memory.stats().generation;
        let completion = match fixture.dispatch_class(index) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected invalid class completion for {index}, got {other:?}"),
        };
        assert_eq!(completion.eax, Some(0));
        assert_eq!(fixture.runtime.current_last_error().unwrap(), 87);
        assert!(fixture.runtime.window_classes().is_empty());
        assert_eq!(fixture.runtime.memory.stats().generation, generation);
        let valid = match fixture.dispatch_class(0) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected valid class retry after {index}, got {other:?}"),
        };
        assert_eq!(valid.eax, Some(0xc000));
    }

    for (index, _) in [
        (9_u32, "register-class-ex-a-windows-932-invalid"),
        (10, "class-optional-resource-unsupported"),
        (11, "register-class-ex-a-name-bound-exceeded"),
    ] {
        let mut fixture = RegisterClassFixture::new();
        fixture.prepare_resources();
        if index == 9 {
            fixture
                .runtime
                .memory
                .write(GuestAddress(CLASS_NAME_BASE + 9 * 0x40), &[0x82, 0x20, 0])
                .expect("malformed Windows-932 class name");
            fixture.acknowledge_pending_delta();
        }
        let generation = fixture.runtime.memory.stats().generation;
        match fixture.dispatch_class(index) {
            DispatchOutcome::Fault(fault) => fault,
            other => panic!("expected unsupported class fault for {index}, got {other:?}"),
        };

        assert!(fixture.runtime.window_classes().is_empty());
        assert_eq!(fixture.runtime.memory.stats().generation, generation);
    }

    let mut invalid_pointer = RegisterClassFixture::new();
    invalid_pointer.prepare_resources();
    let generation = invalid_pointer.runtime.memory.stats().generation;
    match invalid_pointer.dispatch_class(12) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected invalid WNDCLASSEXA pointer fault, got {other:?}"),
    };

    assert!(invalid_pointer.runtime.window_classes().is_empty());
    assert_eq!(
        invalid_pointer.runtime.memory.stats().generation,
        generation
    );

    let mut style = RegisterClassFixture::new();
    style.prepare_resources();
    let completion = match style.dispatch_class(8) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected ordinary CS_VREDRAW registration, got {other:?}"),
    };
    assert_eq!(completion.eax, Some(0xc000));
    assert_eq!(
        style
            .runtime
            .window_class(0xc000)
            .unwrap()
            .registration
            .style,
        1
    );
}

#[test]
fn thread_exit_cancels_owned_callback_calls_before_a_late_return_can_complete() {
    let mut fixture = EnumFixture::new(0, 0, CALLBACK_TARGET.0, 0);
    let DispatchOutcome::GuestCallback(call) = fixture.dispatch() else {
        panic!("expected callback request");
    };
    let prepared = fixture.runtime.begin_guest_callback(call).unwrap();
    let delta = fixture.runtime.memory.pending_delta().unwrap();
    fixture
        .runtime
        .memory
        .acknowledge_delta(delta.generation)
        .unwrap();
    let before = fixture
        .runtime
        .memory
        .read(prepared.stack_pointer, prepared.frame_bytes as usize)
        .unwrap();
    fixture.runtime.request_current_thread_exit(23).unwrap();
    assert!(
        fixture
            .runtime
            .complete_guest_callback(&fixture.dispatcher, CALLBACK_SENTINEL, 7)
            .is_err()
    );
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(prepared.stack_pointer, prepared.frame_bytes as usize)
            .unwrap(),
        before
    );
    assert_eq!(
        fixture.runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Terminating { exit_code: 23 }
    );
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
}

fn initialized_memory() -> (GuestMemory, MemoryOwnerId) {
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 5,
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

fn controlled_runtime(memory: GuestMemory) -> Runtime {
    let mut startup = vec![0; 68];
    startup[..4].copy_from_slice(&68_u32.to_le_bytes());
    let mut runtime = Runtime::new(
        RuntimeBootstrap {
            process_id: RuntimeProcessId(PROCESS_ID),
            initial_thread_id: RuntimeThreadId(THREAD_ID),
            initial_last_error: INITIAL_LAST_ERROR,
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

fn rva_file_offset(loader: &Pe32LoadPlan, rva: u32) -> Option<usize> {
    loader.sections.iter().find_map(|section| {
        let offset = rva.checked_sub(section.destination_rva.0)?;
        (offset < section.copy_size)
            .then(|| usize::try_from(section.source_offset + offset).ok())
            .flatten()
    })
}

#[test]
fn def_window_proc_application_messages_return_zero_without_observable_changes() {
    let mut fixture = RegisterClassFixture::new();
    let (handle, _) = fixture.prepare_active_window_with_small_icon();
    fixture
        .runtime
        .post_thread_message(RuntimeThreadId(THREAD_ID), handle, 0x8001, 17, 23)
        .unwrap();
    fixture
        .runtime
        .set_current_last_error(INITIAL_LAST_ERROR)
        .unwrap();
    let window = fixture.runtime.window(handle).unwrap().clone();
    let messages = fixture
        .runtime
        .queued_thread_messages(RuntimeThreadId(THREAD_ID))
        .unwrap();
    let focus = (
        fixture.runtime.active_window_handle().unwrap(),
        fixture.runtime.focused_window_handle().unwrap(),
        fixture.runtime.foreground_window_handle().unwrap(),
    );
    for message in [0x0400, 0x7fff, 0x8000, 0xbfff] {
        fixture.configure_def_window_proc_args(handle, message, 0xa5a5_ff00, 0xffff_ffff);
        let memory = fixture.runtime.memory.current_reflection().unwrap();
        let outcome = fixture.dispatch_configured_def_window_proc();
        assert!(
            matches!(outcome, DispatchOutcome::Immediate(ref completion) if completion.eax == Some(0)),
            "{outcome:?}"
        );
        assert_eq!(fixture.runtime.memory.current_reflection().unwrap(), memory);
        assert_eq!(fixture.runtime.window(handle).unwrap(), &window);
        assert_eq!(
            fixture
                .runtime
                .queued_thread_messages(RuntimeThreadId(THREAD_ID))
                .unwrap(),
            messages
        );
        assert_eq!(
            (
                fixture.runtime.active_window_handle().unwrap(),
                fixture.runtime.focused_window_handle().unwrap(),
                fixture.runtime.foreground_window_handle().unwrap(),
            ),
            focus
        );
        assert_eq!(
            fixture.runtime.current_last_error().unwrap(),
            INITIAL_LAST_ERROR
        );
    }
}

#[test]
fn def_window_proc_application_messages_reject_invalid_window_without_mutation() {
    let mut fixture = RegisterClassFixture::new();
    let (handle, _) = fixture.prepare_active_window_with_small_icon();
    fixture
        .runtime
        .post_thread_message(RuntimeThreadId(THREAD_ID), handle, 0x8001, 17, 23)
        .unwrap();
    fixture
        .runtime
        .set_current_last_error(INITIAL_LAST_ERROR)
        .unwrap();
    let window = fixture.runtime.window(handle).unwrap().clone();
    let messages = fixture
        .runtime
        .queued_thread_messages(RuntimeThreadId(THREAD_ID))
        .unwrap();
    let allocator = fixture.runtime.process_address_space_allocator().clone();
    for (invalid, message) in [(0, 0x0400), (0x4801_ffff, 0x8000)] {
        fixture.configure_def_window_proc_args(invalid, message, 0xa5a5_ff00, 0xffff_ffff);
        let memory = fixture.runtime.memory.current_reflection().unwrap();
        let statistics = fixture.runtime.memory.stats();
        let scheduler = fixture.runtime.scheduler_state().unwrap();
        assert!(matches!(
            fixture.dispatch_configured_def_window_proc(),
            DispatchOutcome::Fault(_)
        ));
        assert_eq!(fixture.runtime.memory.current_reflection().unwrap(), memory);
        assert_eq!(fixture.runtime.memory.stats(), statistics);
        assert_eq!(fixture.runtime.window(handle).unwrap(), &window);
        assert_eq!(
            fixture
                .runtime
                .queued_thread_messages(RuntimeThreadId(THREAD_ID))
                .unwrap(),
            messages
        );
        assert_eq!(
            fixture.runtime.process_address_space_allocator(),
            &allocator
        );
        assert_eq!(fixture.runtime.scheduler_state().unwrap(), scheduler);
        assert_eq!(
            fixture.runtime.current_last_error().unwrap(),
            INITIAL_LAST_ERROR
        );
    }
}

#[test]
fn application_messages_translate_without_changes_and_dispatch_original_parameters() {
    for message_id in [0x0400, 0x8000] {
        let mut fixture = RegisterClassFixture::new();
        let (window, _) = fixture.prepare_active_window_with_small_icon();
        let expected = fixture
            .runtime
            .post_thread_message(
                RuntimeThreadId(THREAD_ID),
                window,
                message_id,
                0xa5a5_ff00,
                0xffff_ffff,
            )
            .unwrap();
        fixture.configure_peek_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 0, 0, 0, 1);
        assert!(
            matches!(fixture.dispatch_peek_message_a(), DispatchOutcome::Immediate(ref completion) if completion.eax == Some(1))
        );
        fixture.acknowledge_pending_delta();
        fixture
            .runtime
            .post_thread_message(RuntimeThreadId(THREAD_ID), 0, 0x8001, 17, 23)
            .unwrap();
        fixture
            .runtime
            .set_current_last_error(INITIAL_LAST_ERROR)
            .unwrap();
        fixture.configure_translate_message(PEEK_MESSAGE_A_OUTPUT_ADDRESS);
        let queue = fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .unwrap()
            .to_vec();
        let keyboard = fixture.runtime.keyboard_state().cloned().unwrap();
        let memory = fixture.runtime.memory.current_reflection().unwrap();
        let translated = fixture.dispatch_translate_message_basic();
        assert!(
            matches!(translated, DispatchOutcome::Immediate(ref completion) if completion.eax == Some(0)),
            "{translated:?}"
        );
        assert_eq!(fixture.runtime.memory.current_reflection().unwrap(), memory);
        assert_eq!(
            fixture
                .runtime
                .queued_thread_messages(RuntimeThreadId(THREAD_ID))
                .unwrap(),
            queue
        );
        assert_eq!(fixture.runtime.keyboard_state().unwrap(), &keyboard);
        assert_eq!(
            fixture.runtime.current_last_error().unwrap(),
            INITIAL_LAST_ERROR
        );

        fixture.configure_dispatch_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS);
        let before = fixture
            .runtime
            .memory
            .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
            .unwrap();
        assert_eq!(before, expected.to_x86_msg_bytes());
        let callback_target = fixture
            .runtime
            .resolve_window_procedure(window)
            .unwrap()
            .address;
        let call = match fixture.dispatch_dispatch_message_a_basic() {
            DispatchOutcome::GuestCallback(call) => call,
            other => panic!("application message must dispatch to its WndProc: {other:?}"),
        };
        assert_eq!(call.request.target, callback_target);
        assert_eq!(
            call.request.arguments,
            [window, message_id, expected.w_param, expected.l_param]
                .into_iter()
                .map(GuestCallbackArgument::U32)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            call.request.completion_policy,
            GuestCallbackCompletionPolicy::ReturnU32
        );
        let prepared = begin_fixture_callback(&mut fixture, &call);
        let (completion, action) =
            complete_fixture_callback(&mut fixture, call, 0x89ab_cdef, prepared.frame_id);
        fixture.acknowledge_pending_delta();
        assert_eq!(completion.eax, Some(0x89ab_cdef));
        assert_eq!(action, None);
        assert_eq!(
            fixture
                .runtime
                .memory
                .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
                .unwrap(),
            before
        );
        assert_eq!(
            fixture
                .runtime
                .queued_thread_messages(RuntimeThreadId(THREAD_ID))
                .unwrap(),
            queue
        );
        assert_eq!(
            fixture.runtime.current_last_error().unwrap(),
            INITIAL_LAST_ERROR
        );
    }
}

#[test]
fn application_messages_malformed_pointer_rejects_before_translation_or_dispatch() {
    let mut fixture = RegisterClassFixture::new();
    fixture.prepare_active_window_with_small_icon();
    fixture
        .runtime
        .post_thread_message(RuntimeThreadId(THREAD_ID), 0, 0x8000, 17, 23)
        .unwrap();
    for pointer in [GuestAddress(0), GuestAddress(MEMORY_BASE.0 + 0x2ff0)] {
        for dispatch in [false, true] {
            if dispatch {
                fixture.configure_dispatch_message_a(pointer);
            } else {
                fixture.configure_translate_message(pointer);
            }
            let memory = fixture.runtime.memory.current_reflection().unwrap();
            let stats = fixture.runtime.memory.stats();
            let queue = fixture
                .runtime
                .queued_thread_messages(RuntimeThreadId(THREAD_ID))
                .unwrap()
                .to_vec();
            let keyboard = fixture.runtime.keyboard_state().cloned().unwrap();
            let scheduler = fixture.runtime.scheduler_state().unwrap();
            let last_error = fixture.runtime.current_last_error().unwrap();
            let outcome = if dispatch {
                fixture.dispatch_dispatch_message_a_basic()
            } else {
                fixture.dispatch_translate_message_basic()
            };
            assert!(matches!(outcome, DispatchOutcome::Fault(_)), "{outcome:?}");
            assert_eq!(fixture.runtime.memory.current_reflection().unwrap(), memory);
            assert_eq!(fixture.runtime.memory.stats(), stats);
            assert_eq!(
                fixture
                    .runtime
                    .queued_thread_messages(RuntimeThreadId(THREAD_ID))
                    .unwrap(),
                queue
            );
            assert_eq!(fixture.runtime.keyboard_state().unwrap(), &keyboard);
            assert_eq!(fixture.runtime.scheduler_state().unwrap(), scheduler);
            assert_eq!(fixture.runtime.current_last_error().unwrap(), last_error);
        }
    }
}

fn dispatch_queue_status_fixture(
    fixture: &mut RegisterClassFixture,
    flags: u32,
) -> DispatchOutcome {
    let mut endpoint = fixture.class_endpoint.clone();
    endpoint.symbol = crate::api::ApiSymbol::Name {
        name: "GetQueueStatus".to_owned(),
    };
    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap();
    fixture.write_live_words(GuestAddress(STACK_POINTER), &[RETURN_ADDRESS.0, flags]);
    fixture.acknowledge_pending_delta();
    dispatcher.dispatch_owned(
        ApiOwnedTrap {
            endpoint_id: endpoint.id,
            endpoint_address: endpoint.address,
            stack_pointer: GuestAddress(STACK_POINTER),
            return_address: RETURN_ADDRESS,
        },
        &mut fixture.runtime,
        None,
        |frame, runtime, _| dispatch_user32_api(CALLBACK_SENTINEL, frame, runtime),
    )
}

#[test]
fn queue_status_native_key_and_timer_classes_survive_current_but_clear_new_on_peek() {
    let mut fixture = RegisterClassFixture::new();
    let (window, _) = fixture.prepare_active_window_with_small_icon();
    fixture
        .runtime
        .post_thread_message(RuntimeThreadId(THREAD_ID), window, 0x0100, 13, 1)
        .unwrap();
    fixture
        .runtime
        .ingest_keyboard_event(RuntimeKeyboardSourceEvent {
            sequence: 1,
            kind: RuntimeKeyboardSourceEventKind::KeyDown,
            virtual_key: Some(0x0d),
            repeat: false,
        })
        .unwrap();
    assert!(
        matches!(dispatch_queue_status_fixture(&mut fixture, 0x108), DispatchOutcome::Immediate(ref value) if value.eax == Some(0x0108_0108))
    );
    assert!(
        matches!(dispatch_queue_status_fixture(&mut fixture, 1), DispatchOutcome::Immediate(ref value) if value.eax == Some(0x0001_0001))
    );
    fixture.configure_peek_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 0, 0, 0, 0);
    assert!(
        matches!(fixture.dispatch_peek_message_a_basic(), DispatchOutcome::Immediate(ref value) if value.eax == Some(1))
    );
    assert!(
        matches!(dispatch_queue_status_fixture(&mut fixture, 0x109), DispatchOutcome::Immediate(ref value) if value.eax == Some(0x0109_0000))
    );
    for _ in 0..2 {
        fixture.configure_peek_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 0, 0, 0, 1);
        assert!(
            matches!(fixture.dispatch_peek_message_a_basic(), DispatchOutcome::Immediate(ref value) if value.eax == Some(1))
        );
    }
    assert!(
        matches!(dispatch_queue_status_fixture(&mut fixture, 0x109), DispatchOutcome::Immediate(ref value) if value.eax == Some(0))
    );
    let timer_callback = fixture.window_procedure;
    assert!(
        matches!(fixture.dispatch_set_timer_basic(window, 7, 10, timer_callback), DispatchOutcome::Immediate(ref value) if value.eax == Some(7))
    );
    fixture
        .runtime
        .set_current_last_error(INITIAL_LAST_ERROR)
        .unwrap();
    fixture.runtime.advance_scheduled_clock_to(100_000).unwrap();
    fixture.configure_peek_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 0, 0, 0, 0);
    assert!(
        matches!(fixture.dispatch_peek_message_a_basic(), DispatchOutcome::Immediate(ref value) if value.eax == Some(1))
    );
    assert!(
        matches!(dispatch_queue_status_fixture(&mut fixture, 0x10), DispatchOutcome::Immediate(ref value) if value.eax == Some(0x0010_0000))
    );
    fixture.configure_peek_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 0, 0, 0, 1);
    assert!(
        matches!(fixture.dispatch_peek_message_a_basic(), DispatchOutcome::Immediate(ref value) if value.eax == Some(1))
    );
    assert!(
        matches!(dispatch_queue_status_fixture(&mut fixture, 0x10), DispatchOutcome::Immediate(ref value) if value.eax == Some(0))
    );
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
}

#[test]
fn queue_status_invalid_flags_and_failed_peek_preserve_unobserved_posts() {
    let mut fixture = RegisterClassFixture::new();
    fixture.prepare_active_window_with_small_icon();
    fixture
        .runtime
        .post_thread_message(RuntimeThreadId(THREAD_ID), 0, 0x8000, 17, 23)
        .unwrap();
    let queue = fixture
        .runtime
        .queued_thread_messages(RuntimeThreadId(THREAD_ID))
        .unwrap()
        .to_vec();
    assert!(
        matches!(dispatch_queue_status_fixture(&mut fixture, 0x200), DispatchOutcome::Immediate(ref value) if value.eax == Some(0))
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1004);
    assert_eq!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .unwrap(),
        queue
    );
    for (output, filter) in [
        (GuestAddress(MEMORY_BASE.0 + 0x3000), 0),
        (PEEK_MESSAGE_A_OUTPUT_ADDRESS, 1),
    ] {
        fixture.configure_peek_message_a(output, filter, 0, 0, 0);
        let memory = fixture.runtime.memory.current_reflection().unwrap();
        assert!(matches!(
            fixture.dispatch_peek_message_a_basic(),
            DispatchOutcome::Fault(_)
        ));
        assert_eq!(fixture.runtime.memory.current_reflection().unwrap(), memory);
        assert_eq!(
            fixture
                .runtime
                .queued_thread_messages(RuntimeThreadId(THREAD_ID))
                .unwrap(),
            queue
        );
        assert_eq!(fixture.runtime.current_last_error().unwrap(), 1004);
    }
    fixture
        .runtime
        .set_current_last_error(INITIAL_LAST_ERROR)
        .unwrap();
    assert!(
        matches!(dispatch_queue_status_fixture(&mut fixture, 0x108), DispatchOutcome::Immediate(ref value) if value.eax == Some(0x0108_0108))
    );
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
}

fn fixture_display_profile() -> RuntimeDisplayProfile {
    RuntimeDisplayProfile {
        primary_monitor_rect: RuntimeRect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        },
        primary_work_rect: RuntimeRect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1032,
        },
    }
}

fn fixture_input_profile(initial_mouse_buttons_swapped: bool) -> RuntimeInputProfile {
    RuntimeInputProfile {
        initial_mouse_buttons_swapped,
        initial_keyboard_source_focused: Some(true),
        cursor_position: Some(RuntimeCursorPositionProfile {
            initial_screen_x: 960,
            initial_screen_y: 540,
        }),
    }
}

fn remove_keyboard_fixture_message(
    fixture: &mut RegisterClassFixture,
) -> crate::runtime::RuntimeThreadMessage {
    let expected = fixture
        .runtime
        .queued_thread_messages(RuntimeThreadId(THREAD_ID))
        .unwrap()[0];
    fixture.configure_peek_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 0, 0, 0, 1);
    assert!(
        matches!(fixture.dispatch_peek_message_a(), DispatchOutcome::Immediate(ref completion) if completion.eax == Some(1))
    );
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
            .unwrap(),
        expected.to_x86_msg_bytes()
    );
    expected
}

#[test]
fn keyboard_ingress_routes_owner_repeat_and_release_to_window_proc() {
    let mut fixture = RegisterClassFixture::new();
    let (window, _) = fixture.prepare_active_window_with_small_icon();
    for (sequence, kind, repeat) in [
        (1, RuntimeKeyboardSourceEventKind::KeyDown, false),
        (2, RuntimeKeyboardSourceEventKind::KeyDown, true),
        (3, RuntimeKeyboardSourceEventKind::KeyUp, false),
    ] {
        fixture
            .runtime
            .ingest_keyboard_event(RuntimeKeyboardSourceEvent {
                sequence,
                kind,
                virtual_key: Some(0x0d),
                repeat,
            })
            .unwrap();
    }
    let messages = fixture
        .runtime
        .queued_thread_messages(RuntimeThreadId(THREAD_ID))
        .unwrap();
    assert_eq!(
        messages
            .iter()
            .map(|message| (
                message.hwnd,
                message.target_thread_id,
                message.message,
                message.w_param,
                message.l_param
            ))
            .collect::<Vec<_>>(),
        vec![
            (window, RuntimeThreadId(THREAD_ID), 0x100, 0x0d, 0x001c0001),
            (window, RuntimeThreadId(THREAD_ID), 0x100, 0x0d, 0x401c0001),
            (window, RuntimeThreadId(THREAD_ID), 0x101, 0x0d, 0xc01c0001)
        ]
    );
    assert_eq!(
        fixture.runtime.query_async_key_state(0x0d).unwrap(),
        Some(1)
    );
    assert_eq!(
        fixture.runtime.query_async_key_state(0x0d).unwrap(),
        Some(0)
    );
    let down = remove_keyboard_fixture_message(&mut fixture);
    fixture.configure_dispatch_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS);
    let call = match fixture.dispatch_dispatch_message_a() {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected actual keyboard callback, got {other:?}"),
    };
    assert_eq!(
        call.request.arguments,
        [window, 0x100, 0x0d, down.l_param].map(GuestCallbackArgument::U32)
    );
    let prepared = begin_fixture_callback(&mut fixture, &call);
    let (completion, action) =
        complete_fixture_callback(&mut fixture, call, 0x12345678, prepared.frame_id);
    assert_eq!(completion.eax, Some(0x12345678));
    assert_eq!(action, None);
    fixture.configure_translate_message(PEEK_MESSAGE_A_OUTPUT_ADDRESS);
    assert!(
        matches!(fixture.dispatch_translate_message(), DispatchOutcome::Immediate(ref completion) if completion.eax == Some(1))
    );
    let character = remove_keyboard_fixture_message(&mut fixture);
    assert_eq!(
        (
            character.hwnd,
            character.message,
            character.w_param,
            character.l_param
        ),
        (window, 0x102, 0x0d, down.l_param)
    );
}

#[test]
fn keyboard_ingress_capacity_and_focus_rejections_preserve_state_and_sequence() {
    let mut fixture = RegisterClassFixture::new();
    fixture.prepare_active_window_with_small_icon();
    for _ in 0..4096 {
        fixture
            .runtime
            .post_thread_message(RuntimeThreadId(THREAD_ID), 0, 0x400, 0, 0)
            .unwrap();
    }
    let event = RuntimeKeyboardSourceEvent {
        sequence: 1,
        kind: RuntimeKeyboardSourceEventKind::KeyDown,
        virtual_key: Some(0x0d),
        repeat: false,
    };
    let before = fixture.runtime.keyboard_state().cloned().unwrap();
    let queue = fixture
        .runtime
        .queued_thread_messages(RuntimeThreadId(THREAD_ID))
        .unwrap();
    let scheduler = fixture.runtime.scheduler_state().unwrap();
    assert!(fixture.runtime.ingest_keyboard_event(event).is_err());
    assert_eq!(fixture.runtime.keyboard_state().unwrap(), &before);
    assert_eq!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .unwrap(),
        queue
    );
    assert_eq!(fixture.runtime.scheduler_state().unwrap(), scheduler);
    fixture.runtime.receive_current_thread_message().unwrap();
    fixture.runtime.ingest_keyboard_event(event).unwrap();
    fixture
        .runtime
        .ingest_keyboard_event(RuntimeKeyboardSourceEvent {
            sequence: 2,
            kind: RuntimeKeyboardSourceEventKind::FocusLost,
            virtual_key: None,
            repeat: false,
        })
        .unwrap();
    let before = fixture.runtime.keyboard_state().cloned().unwrap();
    let queue = fixture
        .runtime
        .queued_thread_messages(RuntimeThreadId(THREAD_ID))
        .unwrap();
    assert!(
        fixture
            .runtime
            .ingest_keyboard_event(RuntimeKeyboardSourceEvent {
                sequence: 3,
                kind: RuntimeKeyboardSourceEventKind::KeyDown,
                virtual_key: Some(0x0d),
                repeat: false,
            })
            .is_err()
    );
    assert_eq!(fixture.runtime.keyboard_state().unwrap(), &before);
    assert_eq!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .unwrap(),
        queue
    );
}

#[test]
fn keyboard_ingress_wakes_window_owner_without_changing_selected_thread() {
    let mut fixture = RegisterClassFixture::new();
    let (window, _) = fixture.prepare_active_window_with_small_icon();
    fixture.runtime.receive_current_thread_message().unwrap();
    assert_eq!(
        fixture.runtime.scheduler_state().unwrap(),
        SchedulerThreadState::WaitingMessage
    );
    let other = RuntimeThreadId(8193);
    fixture.runtime.register_thread(other, 0).unwrap();
    fixture.runtime.select_thread(other).unwrap();
    fixture
        .runtime
        .ingest_keyboard_event(RuntimeKeyboardSourceEvent {
            sequence: 1,
            kind: RuntimeKeyboardSourceEventKind::KeyDown,
            virtual_key: Some(0x0d),
            repeat: false,
        })
        .unwrap();
    assert_eq!(fixture.runtime.current_thread_id(), other);
    assert_eq!(
        fixture
            .runtime
            .scheduler_state_for_thread(RuntimeThreadId(THREAD_ID))
            .unwrap(),
        SchedulerThreadState::Runnable
    );
    let messages = fixture
        .runtime
        .queued_thread_messages(RuntimeThreadId(THREAD_ID))
        .unwrap();
    assert_eq!((messages.len(), messages[0].hwnd), (1, window));
    assert!(
        fixture
            .runtime
            .queued_thread_messages(other)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn keyboard_translation_uses_removed_modifier_state_and_posts_one_character() {
    let mut fixture = RegisterClassFixture::new();
    let (window, _) = fixture.prepare_active_window_with_small_icon();
    for (sequence, kind, key) in [
        (1, RuntimeKeyboardSourceEventKind::KeyDown, 0xa0),
        (2, RuntimeKeyboardSourceEventKind::KeyDown, 0x41),
        (3, RuntimeKeyboardSourceEventKind::KeyUp, 0x41),
        (4, RuntimeKeyboardSourceEventKind::KeyUp, 0xa0),
    ] {
        fixture
            .runtime
            .ingest_keyboard_event(RuntimeKeyboardSourceEvent {
                sequence,
                kind,
                virtual_key: Some(key),
                repeat: false,
            })
            .unwrap();
    }
    assert_eq!(
        fixture.runtime.query_async_key_state(0x10).unwrap(),
        Some(1)
    );
    remove_keyboard_fixture_message(&mut fixture); // Shift down becomes this thread's state.
    let down = remove_keyboard_fixture_message(&mut fixture);
    let before_msg = fixture
        .runtime
        .memory
        .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
        .unwrap();
    fixture.configure_translate_message(PEEK_MESSAGE_A_OUTPUT_ADDRESS);
    assert!(
        matches!(fixture.dispatch_translate_message(), DispatchOutcome::Immediate(ref completion) if completion.eax == Some(1))
    );
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
            .unwrap(),
        before_msg
    );
    let character = remove_keyboard_fixture_message(&mut fixture);
    assert_eq!(
        (
            character.hwnd,
            character.message,
            character.w_param,
            character.l_param
        ),
        (window, 0x102, 0x41, down.l_param)
    );
    fixture.configure_dispatch_message_a(PEEK_MESSAGE_A_OUTPUT_ADDRESS);
    assert!(matches!(
        fixture.dispatch_dispatch_message_a_basic(),
        DispatchOutcome::GuestCallback(_)
    ));
    let up = remove_keyboard_fixture_message(&mut fixture);
    assert_eq!(up.message, 0x101);
    fixture.configure_translate_message(PEEK_MESSAGE_A_OUTPUT_ADDRESS);
    assert!(
        matches!(fixture.dispatch_translate_message(), DispatchOutcome::Immediate(ref completion) if completion.eax == Some(1))
    );
    assert_eq!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .unwrap()
            .len(),
        1
    );
    remove_keyboard_fixture_message(&mut fixture); // Shift up.
    assert!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn keyboard_translation_rejects_unimplemented_text_without_publication() {
    for (key, shift) in [(0xba, false), (0x31, true), (0x60, false)] {
        let mut fixture = RegisterClassFixture::new();
        fixture.prepare_active_window_with_small_icon();
        let mut sequence = 1;
        if shift {
            fixture
                .runtime
                .ingest_keyboard_event(RuntimeKeyboardSourceEvent {
                    sequence,
                    kind: RuntimeKeyboardSourceEventKind::KeyDown,
                    virtual_key: Some(0xa0),
                    repeat: false,
                })
                .unwrap();
            remove_keyboard_fixture_message(&mut fixture);
            fixture.configure_translate_message(PEEK_MESSAGE_A_OUTPUT_ADDRESS);
            assert!(
                matches!(fixture.dispatch_translate_message(), DispatchOutcome::Immediate(ref completion) if completion.eax == Some(1))
            );
            assert!(
                fixture
                    .runtime
                    .queued_thread_messages(RuntimeThreadId(THREAD_ID))
                    .unwrap()
                    .is_empty()
            );
            sequence += 1;
        }
        fixture
            .runtime
            .ingest_keyboard_event(RuntimeKeyboardSourceEvent {
                sequence,
                kind: RuntimeKeyboardSourceEventKind::KeyDown,
                virtual_key: Some(key),
                repeat: false,
            })
            .unwrap();
        remove_keyboard_fixture_message(&mut fixture);
        fixture.configure_translate_message(PEEK_MESSAGE_A_OUTPUT_ADDRESS);
        let before = fixture
            .runtime
            .memory
            .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
            .unwrap();
        let keyboard = fixture.runtime.keyboard_state().cloned().unwrap();
        assert!(matches!(
            fixture.dispatch_translate_message(),
            DispatchOutcome::Fault(_)
        ));
        assert_eq!(
            fixture
                .runtime
                .memory
                .read(PEEK_MESSAGE_A_OUTPUT_ADDRESS, 28)
                .unwrap(),
            before
        );
        assert_eq!(fixture.runtime.keyboard_state().unwrap(), &keyboard);
        assert!(
            fixture
                .runtime
                .queued_thread_messages(RuntimeThreadId(THREAD_ID))
                .unwrap()
                .is_empty()
        );
    }
}

fn dispatch_message_box_fixture(
    fixture: &mut RegisterClassFixture,
    owner: u32,
    flags: u32,
    text: Option<&[u8]>,
    title: Option<&[u8]>,
) -> (ApiDispatcher, DispatchOutcome) {
    let mut endpoint = fixture.class_endpoint.clone();
    endpoint.symbol = crate::api::ApiSymbol::Name {
        name: "MessageBoxA".to_owned(),
    };
    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap();
    let text_address = GuestAddress(MEMORY_BASE.0 + 0xb00);
    let title_address = GuestAddress(MEMORY_BASE.0 + 0xc00);
    for (address, bytes) in [(text_address, text), (title_address, title)] {
        if let Some(bytes) = bytes {
            fixture.runtime.memory.write(address, bytes).unwrap();
        }
    }
    fixture.write_live_words(
        GuestAddress(STACK_POINTER),
        &[
            SWAP_FIRST_RETURN_ADDRESS.0,
            owner,
            if text.is_some() { text_address.0 } else { 0 },
            if title.is_some() { title_address.0 } else { 0 },
            flags,
        ],
    );
    fixture.acknowledge_pending_delta();
    let outcome = dispatcher.dispatch_owned(
        ApiOwnedTrap {
            endpoint_id: endpoint.id,
            endpoint_address: endpoint.address,
            stack_pointer: GuestAddress(STACK_POINTER),
            return_address: SWAP_FIRST_RETURN_ADDRESS,
        },
        &mut fixture.runtime,
        None,
        |frame, runtime, _| dispatch_user32_api(CALLBACK_SENTINEL, frame, runtime),
    );
    (dispatcher, outcome)
}

#[test]
fn message_box_focus_loss_releases_removed_control_before_return_translation() {
    let mut fixture = RegisterClassFixture::new();
    let (window, _) = fixture.prepare_active_window_with_small_icon();
    fixture
        .runtime
        .ingest_keyboard_event(RuntimeKeyboardSourceEvent {
            sequence: 1,
            kind: RuntimeKeyboardSourceEventKind::KeyDown,
            virtual_key: Some(0xa2),
            repeat: false,
        })
        .unwrap();
    let control = remove_keyboard_fixture_message(&mut fixture);
    assert_eq!((control.message, control.w_param), (0x100, 0x11));
    assert_eq!(
        fixture.runtime.query_async_key_state(0x11).unwrap(),
        Some(0x8001_u16 as i16)
    );
    fixture
        .runtime
        .post_thread_message(RuntimeThreadId(THREAD_ID), 0, 0x400, 17, 23)
        .unwrap();
    let queued = fixture
        .runtime
        .queued_thread_messages(RuntimeThreadId(THREAD_ID))
        .unwrap();
    let (dispatcher, outcome) = dispatch_message_box_fixture(&mut fixture, window, 0, None, None);
    let call = match outcome {
        DispatchOutcome::Deferred(call) => call,
        other => panic!("{other:?}"),
    };
    fixture
        .runtime
        .ingest_keyboard_event(RuntimeKeyboardSourceEvent {
            sequence: 2,
            kind: RuntimeKeyboardSourceEventKind::FocusLost,
            virtual_key: None,
            repeat: false,
        })
        .unwrap();
    assert_eq!(
        fixture
            .runtime
            .queued_thread_messages(RuntimeThreadId(THREAD_ID))
            .unwrap(),
        queued
    );
    assert_eq!(
        fixture.runtime.query_async_key_state(0x11).unwrap(),
        Some(0)
    );
    assert_eq!(
        fixture.runtime.query_async_key_state(0xa2).unwrap(),
        Some(0)
    );
    assert!(
        matches!(crate::complete_user32_message_box(&dispatcher, &call, 1, &mut fixture.runtime), DispatchOutcome::Immediate(ref completion) if completion.eax == Some(1))
    );
    fixture
        .runtime
        .ingest_keyboard_event(RuntimeKeyboardSourceEvent {
            sequence: 3,
            kind: RuntimeKeyboardSourceEventKind::FocusGained,
            virtual_key: None,
            repeat: false,
        })
        .unwrap();
    assert_eq!(
        fixture.runtime.query_async_key_state(0x11).unwrap(),
        Some(0)
    );
    assert_eq!(remove_keyboard_fixture_message(&mut fixture), queued[0]);
    fixture
        .runtime
        .ingest_keyboard_event(RuntimeKeyboardSourceEvent {
            sequence: 4,
            kind: RuntimeKeyboardSourceEventKind::KeyDown,
            virtual_key: Some(0x0d),
            repeat: false,
        })
        .unwrap();
    let down = remove_keyboard_fixture_message(&mut fixture);
    fixture.configure_translate_message(PEEK_MESSAGE_A_OUTPUT_ADDRESS);
    assert!(
        matches!(fixture.dispatch_translate_message(), DispatchOutcome::Immediate(ref completion) if completion.eax == Some(1))
    );
    let character = remove_keyboard_fixture_message(&mut fixture);
    assert_eq!(
        (
            character.hwnd,
            character.message,
            character.w_param,
            character.l_param
        ),
        (window, 0x102, 0x0d, down.l_param)
    );
}

#[test]
fn message_box_copies_windows_932_and_defers_until_an_allowed_real_button() {
    let mut fixture = RegisterClassFixture::new();
    let (window, _) = fixture.prepare_active_window_with_small_icon();
    fixture.runtime.set_current_last_error(0x13572468).unwrap();
    let (dispatcher, outcome) = dispatch_message_box_fixture(
        &mut fixture,
        window,
        0x1010,
        Some(&[0x82, 0xa0, 0x0a, b'A', 0]),
        Some(b"Caption\0"),
    );
    let call = match outcome {
        DispatchOutcome::Deferred(call) => call,
        other => panic!("{other:?}"),
    };
    let request_id = call.request.request_id;
    assert!(call.request.payload.is_empty());
    let dialog = fixture.runtime.message_box(request_id).unwrap();
    assert_eq!(
        (dialog.title.as_str(), dialog.text.as_str()),
        ("Caption", "あ\nA")
    );
    assert_eq!(dialog.icon, crate::RuntimeMessageBoxIcon::Error);
    assert_eq!((dialog.default_button, dialog.cancel_button), (1, Some(1)));
    assert_eq!(
        dialog
            .buttons
            .iter()
            .map(|button| button.id)
            .collect::<Vec<_>>(),
        vec![1]
    );
    assert_eq!(
        fixture.runtime.scheduler_state().unwrap(),
        SchedulerThreadState::WaitingHost { request_id }
    );
    let keyboard = fixture.runtime.keyboard_state().cloned().unwrap();
    let key = RuntimeKeyboardSourceEvent {
        sequence: 1,
        kind: RuntimeKeyboardSourceEventKind::KeyDown,
        virtual_key: Some(0x0d),
        repeat: false,
    };
    assert!(fixture.runtime.ingest_keyboard_event(key).is_err());
    assert_eq!(fixture.runtime.keyboard_state().unwrap(), &keyboard);
    assert!(fixture.runtime.complete_host_request(request_id).is_err());
    assert!(matches!(
        crate::complete_user32_message_box(&dispatcher, &call, 2, &mut fixture.runtime),
        DispatchOutcome::Fault(_)
    ));
    assert!(fixture.runtime.message_box(request_id).is_ok());
    let completion =
        match crate::complete_user32_message_box(&dispatcher, &call, 1, &mut fixture.runtime) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("{other:?}"),
        };
    assert_eq!(
        (
            completion.eax,
            completion.instruction_pointer,
            completion.stack_pointer
        ),
        (
            Some(1),
            SWAP_FIRST_RETURN_ADDRESS,
            GuestAddress(STACK_POINTER + 20)
        )
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x13572468);
    assert_eq!(
        fixture.runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert!(fixture.runtime.message_box(request_id).is_err());
    fixture.runtime.ingest_keyboard_event(key).unwrap();
    assert!(matches!(
        crate::complete_user32_message_box(&dispatcher, &call, 1, &mut fixture.runtime),
        DispatchOutcome::Fault(_)
    ));
}

#[test]
fn message_box_default_cancel_and_null_owner_do_not_disable_guest_windows() {
    let mut fixture = RegisterClassFixture::new();
    fixture.prepare_active_window_with_small_icon();
    let (dispatcher, outcome) = dispatch_message_box_fixture(&mut fixture, 0, 0x103, None, None);
    let call = match outcome {
        DispatchOutcome::Deferred(call) => call,
        other => panic!("{other:?}"),
    };
    let dialog = fixture
        .runtime
        .message_box(call.request.request_id)
        .unwrap();
    assert_eq!((dialog.default_button, dialog.cancel_button), (7, Some(2)));
    assert_eq!(
        dialog
            .buttons
            .iter()
            .map(|button| button.id)
            .collect::<Vec<_>>(),
        vec![6, 7, 2]
    );
    fixture
        .runtime
        .ingest_keyboard_event(RuntimeKeyboardSourceEvent {
            sequence: 1,
            kind: RuntimeKeyboardSourceEventKind::KeyDown,
            virtual_key: Some(0x0d),
            repeat: false,
        })
        .unwrap();
    assert!(matches!(
        crate::complete_user32_message_box(&dispatcher, &call, 1, &mut fixture.runtime),
        DispatchOutcome::Fault(_)
    ));
    assert!(
        matches!(crate::complete_user32_message_box(&dispatcher,&call,2,&mut fixture.runtime),DispatchOutcome::Immediate(ref result) if result.eax==Some(2))
    );
}

#[test]
fn message_box_rejections_do_not_publish_and_thread_exit_cancels_pending_dialog() {
    let mut invalid_owner = RegisterClassFixture::new();
    let (_, outcome) = dispatch_message_box_fixture(&mut invalid_owner, 0xffff, 0, None, None);
    assert!(matches!(outcome, DispatchOutcome::Immediate(ref result) if result.eax == Some(0)));
    assert_eq!(invalid_owner.runtime.current_last_error().unwrap(), 1400);
    assert!(invalid_owner.runtime.scheduler().host_requests().is_empty());
    for (owner, flags, text) in [
        (0, 0x4000, &b"Text\0"[..]),
        (0, 0x2000, &b"Text\0"[..]),
        (0, 0x200, &b"Text\0"[..]),
        (0, 0, &[0x81, 0][..]),
    ] {
        let mut fixture = RegisterClassFixture::new();
        let (_, outcome) =
            dispatch_message_box_fixture(&mut fixture, owner, flags, Some(text), None);
        assert!(matches!(outcome, DispatchOutcome::Fault(_)));
        assert!(fixture.runtime.scheduler().host_requests().is_empty());
        assert_eq!(
            fixture.runtime.scheduler_state().unwrap(),
            SchedulerThreadState::Runnable
        );
        let (dispatcher, outcome) = dispatch_message_box_fixture(&mut fixture, 0, 0, None, None);
        let call = match outcome {
            DispatchOutcome::Deferred(call) => call,
            other => panic!("{other:?}"),
        };
        assert_eq!(call.request.request_id, 1);
        fixture.runtime.request_current_thread_exit(7).unwrap();
        assert!(
            fixture
                .runtime
                .message_box(call.request.request_id)
                .is_err()
        );
        assert!(matches!(
            crate::complete_user32_message_box(&dispatcher, &call, 1, &mut fixture.runtime),
            DispatchOutcome::Fault(_)
        ));
    }
}

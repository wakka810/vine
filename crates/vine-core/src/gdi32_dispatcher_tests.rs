use crate as vine_core;
use crate::api::{GuestCallbackArgument, GuestCallbackCompletionPolicy};
use crate::gdi32::dispatch_gdi32_api;
use crate::gdi32::{
    X86_BITMAPINFOHEADER_BYTE_LENGTH, X86_DIB_BITMAPINFO_BYTE_LENGTH,
    X86_ENUMLOGFONTEXA_BYTE_LENGTH, X86_LOGFONTA_BYTE_LENGTH, X86_NEWTEXTMETRICEXA_BYTE_LENGTH,
    X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH,
};
use crate::imports::{ImportThunk, ImportThunkDisposition, ImportThunkId};
use crate::pe32::ImportTarget;
use crate::runtime::{RuntimeDeviceContextTarget, RuntimeGdiObjectKind, RuntimeHostCompletion};
use crate::user32::dispatch_user32_api;
use vine_core::{
    ApiDeferredCall, ApiDispatcher, ApiEndpointIdentity, ApiOwnedTrap, DispatchOutcome,
    GUEST_PAGE_SIZE, Gdi32TextRasterHostResponse, GuestAddress, GuestCallbackReturn, GuestMemory,
    GuestRange, MemoryLimits, MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind, PageProtection,
    ProcessAddressAllocationPurpose, Runtime, RuntimeBootstrap, RuntimeClockState,
    RuntimeCurrentDirectoryA, RuntimeDisplayProfile, RuntimeProcessId, RuntimeProcessImagePathA,
    RuntimeRect, RuntimeStartupInfo, RuntimeThreadId, complete_gdi32_text_raster,
};

const MEMORY_BASE: GuestAddress = GuestAddress(0x2000_0000);
const GET_DC_STACK_POINTER: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x1040);
const RELEASE_DC_STACK_POINTER: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x1060);
const ENUM_STACK_POINTER: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x1080);
const CREATE_FONT_STACK_POINTER: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x10c0);
const CREATE_COMPATIBLE_DC_STACK_POINTER: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x1100);
const DELETE_DC_STACK_POINTER: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x1120);
const CREATE_DIB_STACK_POINTER: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x1140);
const GET_OBJECT_STACK_POINTER: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x1180);
const SELECT_OBJECT_STACK_POINTER: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x11a0);
const DELETE_OBJECT_STACK_POINTER: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x11c0);
const OUTLINE_TEXT_METRICS_STACK_POINTER: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x11e0);
const TEXT_OUT_STACK_POINTER: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x1200);
const LOW_ENUM_STACK_POINTER: GuestAddress = GuestAddress(MEMORY_BASE.0 + 8);
const GET_DC_IAT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2000);
const RELEASE_DC_IAT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2004);
const ENUM_IAT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2008);
const CREATE_FONT_IAT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x200c);
const CREATE_COMPATIBLE_DC_IAT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2010);
const DELETE_DC_IAT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2014);
const CREATE_DIB_IAT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2018);
const GET_OBJECT_IAT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x201c);
const SELECT_OBJECT_IAT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2020);
const DELETE_OBJECT_IAT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2024);
const OUTLINE_TEXT_METRICS_IAT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2028);
const TEXT_OUT_IAT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x202c);
const CALLBACK_TARGET: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2100);
const CALLBACK_SENTINEL: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2110);
const LOG_FONT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2200);
const FACE_NAME_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2300);
const BITMAP_INFO_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2400);
const BITS_OUTPUT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2900);
const OBJECT_OUTPUT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2920);
const OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2a00);
const TEXT_OUT_BYTES_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2b20);
const GET_DC_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0ee0);
const RELEASE_DC_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0ef0);
const ENUM_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0fc0);
const CREATE_FONT_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0ff0);
const CREATE_COMPATIBLE_DC_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0fb0);
const DELETE_DC_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_1000);
const CREATE_DIB_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_1010);
const GET_OBJECT_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_1020);
const SELECT_OBJECT_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_1030);
const DELETE_OBJECT_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_1040);
const OUTLINE_TEXT_METRICS_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_1050);
const TEXT_OUT_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_1060);
const GET_DC_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1002_c149);
const RELEASE_DC_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1002_c16a);
const ENUM_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1002_c16a);
const CREATE_FONT_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1002_c838);
const CREATE_COMPATIBLE_DC_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1002_c8ad);
const DELETE_DC_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1002_c472);
const CREATE_DIB_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1002_c8f0);
const GET_OBJECT_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1002_c910);
const SELECT_OBJECT_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1002_c930);
const DELETE_OBJECT_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1002_c950);
const OUTLINE_TEXT_METRICS_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1002_c51f);
const TEXT_OUT_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1002_cab1);
const CALLBACK_PARAMETER: u32 = 0x100d_ce7c;
const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;
const INITIAL_LAST_ERROR: u32 = 0x1234_5678;
const MS_GOTHIC_WINDOWS_932: &[u8] = &[
    0x82, 0x6c, 0x82, 0x72, 0x20, 0x83, 0x53, 0x83, 0x56, 0x83, 0x62, 0x83, 0x4e,
];

struct FontFixture {
    runtime: Runtime,
    dispatcher: ApiDispatcher,
    get_dc_endpoint: ApiEndpointIdentity,
    release_dc_endpoint: ApiEndpointIdentity,
    enum_endpoint: ApiEndpointIdentity,
    create_font_endpoint: ApiEndpointIdentity,
    create_compatible_dc_endpoint: ApiEndpointIdentity,
    delete_dc_endpoint: ApiEndpointIdentity,
    create_dib_endpoint: ApiEndpointIdentity,
    get_object_endpoint: ApiEndpointIdentity,
    select_object_endpoint: ApiEndpointIdentity,
    delete_object_endpoint: ApiEndpointIdentity,
    outline_text_metrics_endpoint: ApiEndpointIdentity,
    text_out_endpoint: ApiEndpointIdentity,
    callback_sentinel: GuestAddress,
    enum_stack_pointer: GuestAddress,
}

impl FontFixture {
    fn new() -> Self {
        Self::with_enum_stack(ENUM_STACK_POINTER)
    }

    fn with_enum_stack(enum_stack_pointer: GuestAddress) -> Self {
        let (mut memory, owner) = initialized_memory();
        let get_dc_endpoint = endpoint(
            owner,
            230,
            GET_DC_ENDPOINT_ADDRESS,
            GET_DC_IAT_ADDRESS,
            "USER32.dll",
            "GetDC",
        );
        let release_dc_endpoint = endpoint(
            owner,
            240,
            RELEASE_DC_ENDPOINT_ADDRESS,
            RELEASE_DC_IAT_ADDRESS,
            "USER32.dll",
            "ReleaseDC",
        );
        let enum_endpoint = endpoint(
            owner,
            253,
            ENUM_ENDPOINT_ADDRESS,
            ENUM_IAT_ADDRESS,
            "GDI32.dll",
            "EnumFontFamiliesExA",
        );
        let create_font_endpoint = endpoint(
            owner,
            256,
            CREATE_FONT_ENDPOINT_ADDRESS,
            CREATE_FONT_IAT_ADDRESS,
            "GDI32.dll",
            "CreateFontA",
        );
        let create_compatible_dc_endpoint = endpoint(
            owner,
            252,
            CREATE_COMPATIBLE_DC_ENDPOINT_ADDRESS,
            CREATE_COMPATIBLE_DC_IAT_ADDRESS,
            "GDI32.dll",
            "CreateCompatibleDC",
        );
        let delete_dc_endpoint = endpoint(
            owner,
            257,
            DELETE_DC_ENDPOINT_ADDRESS,
            DELETE_DC_IAT_ADDRESS,
            "GDI32.dll",
            "DeleteDC",
        );
        let create_dib_endpoint = endpoint(
            owner,
            258,
            CREATE_DIB_ENDPOINT_ADDRESS,
            CREATE_DIB_IAT_ADDRESS,
            "GDI32.dll",
            "CreateDIBSection",
        );
        let get_object_endpoint = endpoint(
            owner,
            259,
            GET_OBJECT_ENDPOINT_ADDRESS,
            GET_OBJECT_IAT_ADDRESS,
            "GDI32.dll",
            "GetObjectA",
        );
        let select_object_endpoint = endpoint(
            owner,
            260,
            SELECT_OBJECT_ENDPOINT_ADDRESS,
            SELECT_OBJECT_IAT_ADDRESS,
            "GDI32.dll",
            "SelectObject",
        );
        let delete_object_endpoint = endpoint(
            owner,
            261,
            DELETE_OBJECT_ENDPOINT_ADDRESS,
            DELETE_OBJECT_IAT_ADDRESS,
            "GDI32.dll",
            "DeleteObject",
        );
        let outline_text_metrics_endpoint = endpoint(
            owner,
            262,
            OUTLINE_TEXT_METRICS_ENDPOINT_ADDRESS,
            OUTLINE_TEXT_METRICS_IAT_ADDRESS,
            "GDI32.dll",
            "GetOutlineTextMetricsA",
        );
        let text_out_endpoint = endpoint(
            owner,
            263,
            TEXT_OUT_ENDPOINT_ADDRESS,
            TEXT_OUT_IAT_ADDRESS,
            "GDI32.dll",
            "TextOutA",
        );
        for (iat, target) in [
            (GET_DC_IAT_ADDRESS, GET_DC_ENDPOINT_ADDRESS),
            (RELEASE_DC_IAT_ADDRESS, RELEASE_DC_ENDPOINT_ADDRESS),
            (ENUM_IAT_ADDRESS, ENUM_ENDPOINT_ADDRESS),
            (CREATE_FONT_IAT_ADDRESS, CREATE_FONT_ENDPOINT_ADDRESS),
            (
                CREATE_COMPATIBLE_DC_IAT_ADDRESS,
                CREATE_COMPATIBLE_DC_ENDPOINT_ADDRESS,
            ),
            (DELETE_DC_IAT_ADDRESS, DELETE_DC_ENDPOINT_ADDRESS),
            (CREATE_DIB_IAT_ADDRESS, CREATE_DIB_ENDPOINT_ADDRESS),
            (GET_OBJECT_IAT_ADDRESS, GET_OBJECT_ENDPOINT_ADDRESS),
            (SELECT_OBJECT_IAT_ADDRESS, SELECT_OBJECT_ENDPOINT_ADDRESS),
            (DELETE_OBJECT_IAT_ADDRESS, DELETE_OBJECT_ENDPOINT_ADDRESS),
            (
                OUTLINE_TEXT_METRICS_IAT_ADDRESS,
                OUTLINE_TEXT_METRICS_ENDPOINT_ADDRESS,
            ),
            (TEXT_OUT_IAT_ADDRESS, TEXT_OUT_ENDPOINT_ADDRESS),
        ] {
            memory
                .write_initialization(owner, iat, &target.0.to_le_bytes())
                .expect("initialize GDI fixture IAT");
        }
        memory
            .protect(
                owner,
                GuestRange::new(GET_DC_IAT_ADDRESS, GUEST_PAGE_SIZE)
                    .expect("callback executable page"),
                PageProtection {
                    read: true,
                    write: true,
                    execute: true,
                },
            )
            .expect("make callback page executable");
        memory.seal_owner(owner).expect("seal GDI fixture owner");
        let reflection = memory.pending_reflection().expect("initial reflection");
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .expect("acknowledge initial reflection");

        let dispatcher = ApiDispatcher::from_endpoints([
            get_dc_endpoint.clone(),
            release_dc_endpoint.clone(),
            enum_endpoint.clone(),
            create_font_endpoint.clone(),
            create_compatible_dc_endpoint.clone(),
            delete_dc_endpoint.clone(),
            create_dib_endpoint.clone(),
            get_object_endpoint.clone(),
            select_object_endpoint.clone(),
            delete_object_endpoint.clone(),
            outline_text_metrics_endpoint.clone(),
            text_out_endpoint.clone(),
        ])
        .expect("GDI fixture dispatcher");
        let mut runtime = controlled_runtime(memory);
        runtime
            .configure_display_profile(RuntimeDisplayProfile {
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
            })
            .expect("configure logical display");

        Self {
            runtime,
            dispatcher,
            get_dc_endpoint,
            release_dc_endpoint,
            enum_endpoint,
            create_font_endpoint,
            create_compatible_dc_endpoint,
            delete_dc_endpoint,
            create_dib_endpoint,
            get_object_endpoint,
            select_object_endpoint,
            delete_object_endpoint,
            outline_text_metrics_endpoint,
            text_out_endpoint,
            callback_sentinel: CALLBACK_SENTINEL,
            enum_stack_pointer,
        }
    }

    fn acquire_screen_dc(&mut self) -> u32 {
        self.write_words(GET_DC_STACK_POINTER, &[GET_DC_RETURN_ADDRESS.0, 0]);
        let endpoint = self.get_dc_endpoint.clone();
        match self.dispatch_endpoint(endpoint, GET_DC_STACK_POINTER, GET_DC_RETURN_ADDRESS) {
            DispatchOutcome::Immediate(completion) => match completion.eax {
                Some(handle) => handle,
                other => panic!("GetDC did not set EAX: {other:?}"),
            },
            other => panic!("expected screen GetDC completion, got {other:?}"),
        }
    }

    fn release_screen_dc(&mut self, device_context: u32) {
        self.write_words(
            RELEASE_DC_STACK_POINTER,
            &[RELEASE_DC_RETURN_ADDRESS.0, 0, device_context],
        );
        let endpoint = self.release_dc_endpoint.clone();
        let completion = match self.dispatch_endpoint(
            endpoint,
            RELEASE_DC_STACK_POINTER,
            RELEASE_DC_RETURN_ADDRESS,
        ) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected screen ReleaseDC completion, got {other:?}"),
        };
        assert_eq!(completion.eax, Some(1));
    }

    fn install_filter(&mut self, filter: &[u8]) {
        assert_eq!(filter.len(), X86_LOGFONTA_BYTE_LENGTH as usize);
        self.runtime
            .memory
            .write(LOG_FONT_ADDRESS, filter)
            .expect("write LOGFONTA filter");
        self.acknowledge_pending_delta();
    }

    fn configure_enum(
        &mut self,
        device_context: u32,
        log_font: GuestAddress,
        callback: GuestAddress,
        flags: u32,
    ) {
        self.write_words(
            self.enum_stack_pointer,
            &[
                ENUM_RETURN_ADDRESS.0,
                device_context,
                log_font.0,
                callback.0,
                CALLBACK_PARAMETER,
                flags,
            ],
        );
    }

    fn dispatch_enum(&mut self) -> DispatchOutcome {
        let endpoint = self.enum_endpoint.clone();
        self.dispatch_endpoint(endpoint, self.enum_stack_pointer, ENUM_RETURN_ADDRESS)
    }

    fn install_face_name(&mut self, bytes: &[u8]) {
        self.runtime
            .memory
            .write(FACE_NAME_ADDRESS, bytes)
            .expect("write CreateFontA face name");
        self.acknowledge_pending_delta();
    }

    fn configure_create_font(&mut self, arguments: [u32; 13], face_name: GuestAddress) {
        let mut words = vec![CREATE_FONT_RETURN_ADDRESS.0];
        words.extend_from_slice(&arguments);
        words.push(face_name.0);
        self.write_words(CREATE_FONT_STACK_POINTER, &words);
    }

    fn dispatch_create_font(&mut self) -> DispatchOutcome {
        let endpoint = self.create_font_endpoint.clone();
        self.dispatch_endpoint(
            endpoint,
            CREATE_FONT_STACK_POINTER,
            CREATE_FONT_RETURN_ADDRESS,
        )
    }

    fn dispatch_create_compatible_dc(&mut self, source_device_context: u32) -> DispatchOutcome {
        self.write_words(
            CREATE_COMPATIBLE_DC_STACK_POINTER,
            &[CREATE_COMPATIBLE_DC_RETURN_ADDRESS.0, source_device_context],
        );
        let endpoint = self.create_compatible_dc_endpoint.clone();
        self.dispatch_endpoint(
            endpoint,
            CREATE_COMPATIBLE_DC_STACK_POINTER,
            CREATE_COMPATIBLE_DC_RETURN_ADDRESS,
        )
    }

    fn dispatch_delete_dc(&mut self, device_context: u32) -> DispatchOutcome {
        self.write_words(
            DELETE_DC_STACK_POINTER,
            &[DELETE_DC_RETURN_ADDRESS.0, device_context],
        );
        let endpoint = self.delete_dc_endpoint.clone();
        self.dispatch_endpoint(endpoint, DELETE_DC_STACK_POINTER, DELETE_DC_RETURN_ADDRESS)
    }

    fn install_bitmap_info(&mut self, bytes: &[u8]) {
        assert_eq!(bytes.len(), X86_DIB_BITMAPINFO_BYTE_LENGTH);
        self.runtime
            .memory
            .write(BITMAP_INFO_ADDRESS, bytes)
            .expect("write BITMAPINFO");
        self.acknowledge_pending_delta();
    }

    fn dispatch_create_dib_section(
        &mut self,
        device_context: u32,
        bitmap_info: GuestAddress,
        bits_output: GuestAddress,
    ) -> DispatchOutcome {
        self.write_words(
            CREATE_DIB_STACK_POINTER,
            &[
                CREATE_DIB_RETURN_ADDRESS.0,
                device_context,
                bitmap_info.0,
                0,
                bits_output.0,
                0,
                0,
            ],
        );
        let endpoint = self.create_dib_endpoint.clone();
        self.dispatch_endpoint(
            endpoint,
            CREATE_DIB_STACK_POINTER,
            CREATE_DIB_RETURN_ADDRESS,
        )
    }

    fn dispatch_get_object(
        &mut self,
        object: u32,
        byte_length: u32,
        output: GuestAddress,
    ) -> DispatchOutcome {
        self.write_words(
            GET_OBJECT_STACK_POINTER,
            &[GET_OBJECT_RETURN_ADDRESS.0, object, byte_length, output.0],
        );
        let endpoint = self.get_object_endpoint.clone();
        self.dispatch_endpoint(
            endpoint,
            GET_OBJECT_STACK_POINTER,
            GET_OBJECT_RETURN_ADDRESS,
        )
    }

    fn dispatch_select_object(&mut self, device_context: u32, object: u32) -> DispatchOutcome {
        self.write_words(
            SELECT_OBJECT_STACK_POINTER,
            &[SELECT_OBJECT_RETURN_ADDRESS.0, device_context, object],
        );
        let endpoint = self.select_object_endpoint.clone();
        self.dispatch_endpoint(
            endpoint,
            SELECT_OBJECT_STACK_POINTER,
            SELECT_OBJECT_RETURN_ADDRESS,
        )
    }

    fn create_primary_font_memory_dc(&mut self) -> (u32, u32, u32) {
        self.create_font_memory_dc(primary_create_font_arguments())
    }

    fn create_font_memory_dc(&mut self, arguments: [u32; 13]) -> (u32, u32, u32) {
        self.install_face_name(&primary_face_name());
        self.configure_create_font(arguments, FACE_NAME_ADDRESS);
        let font = match self.dispatch_create_font() {
            DispatchOutcome::Immediate(completion) => match completion.eax {
                Some(handle) => handle,
                other => panic!("CreateFontA did not set EAX: {other:?}"),
            },
            other => panic!("expected CreateFontA completion, got {other:?}"),
        };
        let device_context = match self.dispatch_create_compatible_dc(0) {
            DispatchOutcome::Immediate(completion) => match completion.eax {
                Some(handle) => handle,
                other => panic!("CreateCompatibleDC did not set EAX: {other:?}"),
            },
            other => panic!("expected CreateCompatibleDC completion, got {other:?}"),
        };
        let previous_font = match self.dispatch_select_object(device_context, font) {
            DispatchOutcome::Immediate(completion) => match completion.eax {
                Some(handle) => handle,
                other => panic!("SelectObject did not set EAX: {other:?}"),
            },
            other => panic!("expected SelectObject completion, got {other:?}"),
        };
        (font, device_context, previous_font)
    }

    fn dispatch_delete_object(&mut self, object: u32) -> DispatchOutcome {
        self.write_words(
            DELETE_OBJECT_STACK_POINTER,
            &[DELETE_OBJECT_RETURN_ADDRESS.0, object],
        );
        let endpoint = self.delete_object_endpoint.clone();
        self.dispatch_endpoint(
            endpoint,
            DELETE_OBJECT_STACK_POINTER,
            DELETE_OBJECT_RETURN_ADDRESS,
        )
    }

    fn dispatch_outline_text_metrics(
        &mut self,
        device_context: u32,
        byte_length: u32,
        output: GuestAddress,
    ) -> DispatchOutcome {
        self.write_words(
            OUTLINE_TEXT_METRICS_STACK_POINTER,
            &[
                OUTLINE_TEXT_METRICS_RETURN_ADDRESS.0,
                device_context,
                byte_length,
                output.0,
            ],
        );
        self.dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: self.outline_text_metrics_endpoint.id,
                endpoint_address: self.outline_text_metrics_endpoint.address,
                stack_pointer: OUTLINE_TEXT_METRICS_STACK_POINTER,
                return_address: OUTLINE_TEXT_METRICS_RETURN_ADDRESS,
            },
            &mut self.runtime,
            None,
            |frame, runtime, _cpu| dispatch_gdi32_api(self.callback_sentinel, frame, runtime),
        )
    }

    fn dispatch_text_out(
        &mut self,
        device_context: u32,
        text: GuestAddress,
        count: u32,
    ) -> DispatchOutcome {
        self.write_words(
            TEXT_OUT_STACK_POINTER,
            &[
                TEXT_OUT_RETURN_ADDRESS.0,
                device_context,
                0,
                0,
                text.0,
                count,
            ],
        );
        self.dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: self.text_out_endpoint.id,
                endpoint_address: self.text_out_endpoint.address,
                stack_pointer: TEXT_OUT_STACK_POINTER,
                return_address: TEXT_OUT_RETURN_ADDRESS,
            },
            &mut self.runtime,
            None,
            |frame, runtime, _cpu| dispatch_gdi32_api(self.callback_sentinel, frame, runtime),
        )
    }

    fn complete_text_raster(
        &mut self,
        call: &ApiDeferredCall,
        pixels: Vec<u8>,
    ) -> (DispatchOutcome, Option<RuntimeHostCompletion>) {
        let request_id = call.request.request_id;
        complete_gdi32_text_raster(
            &self.dispatcher,
            call,
            Gdi32TextRasterHostResponse { request_id, pixels },
            &mut self.runtime,
        )
    }

    fn write_words(&mut self, address: GuestAddress, words: &[u32]) {
        let bytes = words
            .iter()
            .copied()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        self.runtime
            .memory
            .write(address, &bytes)
            .expect("write call frame");
        self.acknowledge_pending_delta();
    }

    fn acknowledge_pending_delta(&mut self) {
        if self.runtime.memory.stats().pending_invalidation.is_some() {
            let delta = self
                .runtime
                .memory
                .pending_delta()
                .expect("pending GDI fixture delta");
            self.runtime
                .memory
                .acknowledge_delta(delta.generation)
                .expect("acknowledge GDI fixture delta");
        }
    }

    fn dispatch_endpoint(
        &mut self,
        endpoint: ApiEndpointIdentity,
        stack_pointer: GuestAddress,
        return_address: GuestAddress,
    ) -> DispatchOutcome {
        let callback_sentinel = self.callback_sentinel;
        self.dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: endpoint.id,
                endpoint_address: endpoint.address,
                stack_pointer,
                return_address,
            },
            &mut self.runtime,
            None,
            |frame, runtime, _cpu| {
                if frame.endpoint.module.eq_ignore_ascii_case("gdi32.dll") {
                    dispatch_gdi32_api(callback_sentinel, frame, runtime)
                } else {
                    dispatch_user32_api(callback_sentinel, frame, runtime)
                }
            },
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct PreparedTextSurface {
    device_context: u32,
    bits: GuestAddress,
}

fn prepare_primary_text_surface(fixture: &mut FontFixture) -> PreparedTextSurface {
    let (_font, device_context, _default_font) = fixture.create_primary_font_memory_dc();
    fixture.install_bitmap_info(&primary_bitmap_info());
    fixture
        .runtime
        .memory
        .write(BITS_OUTPUT_ADDRESS, &0xa5a5_a5a5_u32.to_le_bytes())
        .expect("initialize text DIB bits output");
    fixture.acknowledge_pending_delta();
    let creation = match fixture.dispatch_create_dib_section(
        device_context,
        BITMAP_INFO_ADDRESS,
        BITS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected text DIB creation, got {other:?}"),
    };
    let dib = match creation.eax {
        Some(handle) => handle,
        other => panic!("CreateDIBSection did not set EAX: {other:?}"),
    };
    fixture.acknowledge_pending_delta();
    let bits = GuestAddress(read_u32(
        &fixture
            .runtime
            .memory
            .read(BITS_OUTPUT_ADDRESS, 4)
            .expect("read text DIB bits output"),
        0,
    ));
    let _default_bitmap = match fixture.dispatch_select_object(device_context, dib) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            Some(handle) => handle,
            other => panic!("DIB SelectObject did not set EAX: {other:?}"),
        },
        other => panic!("expected text DIB selection, got {other:?}"),
    };
    let backing_range = fixture
        .runtime
        .gdi_objects()
        .values()
        .find_map(|object| {
            if object.handle != dib {
                return None;
            }
            match object.object {
                RuntimeGdiObjectKind::DibSection { backing, .. } => Some(backing.range),
                _ => None,
            }
        })
        .expect("text DIB backing");
    assert_eq!(backing_range.start(), bits);
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(bits, 12_288)
            .expect("fresh text DIB pixels"),
        vec![0; 12_288]
    );
    fixture
        .runtime
        .memory
        .write(TEXT_OUT_BYTES_ADDRESS, b"a")
        .expect("install TextOutA bytes");
    fixture.acknowledge_pending_delta();
    PreparedTextSurface {
        device_context,
        bits,
    }
}

#[test]
fn compatible_dc_creation_shares_defaults_and_delete_retires_only_the_exact_dc() {
    let mut fixture = FontFixture::new();
    let first_completion = match fixture.dispatch_create_compatible_dc(0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected CreateCompatibleDC completion, got {other:?}"),
    };
    let Some(first_handle) = first_completion.eax else {
        panic!("CreateCompatibleDC did not set EAX")
    };
    assert_eq!(first_handle, 0x4701_0001);
    assert_eq!(
        first_completion.instruction_pointer,
        CREATE_COMPATIBLE_DC_RETURN_ADDRESS
    );
    assert_eq!(
        first_completion.stack_pointer,
        GuestAddress(CREATE_COMPATIBLE_DC_STACK_POINTER.0 + 8)
    );
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );

    let first = fixture
        .runtime
        .gdi_device_context(first_handle)
        .expect("first compatible memory DC");
    assert!(matches!(
        first.target,
        RuntimeDeviceContextTarget::Memory {
            compatible_monitor_handle: 0x4f01_0001,
            ..
        }
    ));
    assert_eq!(first.selected_font_id.map(|id| id.0), Some(1));
    assert_eq!(first.selected_bitmap_id.map(|id| id.0), Some(2));
    let first_selected_font_id = first.selected_font_id;
    let first_selected_bitmap_id = first.selected_bitmap_id;
    assert_eq!(fixture.runtime.gdi_objects().len(), 2);
    assert!(fixture.runtime.gdi_objects().values().any(|object| {
        object.shared && matches!(object.object, RuntimeGdiObjectKind::StockFont { index: 13 })
    }));
    assert!(fixture.runtime.gdi_objects().values().any(|object| {
        object.shared
            && matches!(
                object.object,
                RuntimeGdiObjectKind::StockBitmap {
                    index: 21,
                    width: 1,
                    height: 1,
                    bits_per_pixel: 1,
                }
            )
    }));

    let second_completion = match fixture.dispatch_create_compatible_dc(0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected repeated CreateCompatibleDC completion, got {other:?}"),
    };
    let Some(second_handle) = second_completion.eax else {
        panic!("repeated CreateCompatibleDC did not set EAX")
    };
    assert_eq!(second_handle, first_handle + 1);
    let second = fixture
        .runtime
        .gdi_device_context(second_handle)
        .expect("second compatible memory DC");
    assert_eq!(second.selected_font_id, first_selected_font_id);
    assert_eq!(second.selected_bitmap_id, first_selected_bitmap_id);
    assert_eq!(fixture.runtime.gdi_objects().len(), 2);

    let delete_first = match fixture.dispatch_delete_dc(first_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected DeleteDC completion, got {other:?}"),
    };
    assert_eq!(delete_first.eax, Some(1));
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
    assert!(fixture.runtime.gdi_device_context(first_handle).is_err());
    assert!(fixture.runtime.gdi_device_context(second_handle).is_ok());
    assert_eq!(fixture.runtime.gdi_objects().len(), 2);

    let second_before_repeat = fixture
        .runtime
        .gdi_device_context(second_handle)
        .unwrap()
        .clone();
    let clock_before_repeat = fixture.runtime.clock().clone();
    let thread_before_repeat = fixture.runtime.current_thread_id();
    let repeated_delete = match fixture.dispatch_delete_dc(first_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected repeated DeleteDC completion, got {other:?}"),
    };
    assert_eq!(repeated_delete.eax, Some(0));
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
    assert!(fixture.runtime.gdi_device_context(first_handle).is_err());
    assert_eq!(
        fixture.runtime.gdi_device_context(second_handle).unwrap(),
        &second_before_repeat
    );
    assert_eq!(fixture.runtime.gdi_objects().len(), 2);
    assert_eq!(fixture.runtime.clock(), &clock_before_repeat);
    assert_eq!(fixture.runtime.current_thread_id(), thread_before_repeat);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
    assert!(matches!(fixture.dispatch_delete_dc(second_handle),
        DispatchOutcome::Immediate(ref completion) if completion.eax == Some(1)));
    assert!(fixture.runtime.gdi_device_context(second_handle).is_err());
    assert_eq!(fixture.runtime.gdi_objects().len(), 2);
}

#[test]
fn dib_backing_and_gdi_objects_follow_primary_restore_before_delete_lifecycle() {
    let mut fixture = FontFixture::new();
    fixture.install_face_name(&primary_face_name());
    fixture.configure_create_font(primary_create_font_arguments(), FACE_NAME_ADDRESS);
    let font_handle = match fixture.dispatch_create_font() {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            Some(handle) => handle,
            other => panic!("CreateFontA did not set EAX: {other:?}"),
        },
        other => panic!("expected CreateFontA completion, got {other:?}"),
    };
    let memory_dc = match fixture.dispatch_create_compatible_dc(0) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            Some(handle) => handle,
            other => panic!("CreateCompatibleDC did not set EAX: {other:?}"),
        },
        other => panic!("expected CreateCompatibleDC completion, got {other:?}"),
    };
    let (default_font_handle, default_bitmap_handle) = {
        let dc = fixture
            .runtime
            .gdi_device_context(memory_dc)
            .expect("compatible memory DC");
        let objects = fixture.runtime.gdi_objects();
        (
            objects
                .get(&dc.selected_font_id.expect("default font identity"))
                .expect("default font object")
                .handle,
            objects
                .get(&dc.selected_bitmap_id.expect("default bitmap identity"))
                .expect("default bitmap object")
                .handle,
        )
    };

    fixture.install_bitmap_info(&primary_bitmap_info());
    fixture
        .runtime
        .memory
        .write(BITS_OUTPUT_ADDRESS, &0xa5a5_a5a5_u32.to_le_bytes())
        .expect("initialize bits output");
    fixture.acknowledge_pending_delta();
    let memory_before_create = fixture.runtime.memory.stats();
    let address_records_before_create = fixture
        .runtime
        .process_address_space_allocator()
        .records()
        .len();
    let creation = match fixture.dispatch_create_dib_section(
        memory_dc,
        BITMAP_INFO_ADDRESS,
        BITS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected CreateDIBSection completion, got {other:?}"),
    };
    let Some(dib_handle) = creation.eax else {
        panic!("CreateDIBSection did not set EAX")
    };
    assert_ne!(dib_handle, 0);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
    assert_eq!(
        creation.stack_pointer,
        GuestAddress(CREATE_DIB_STACK_POINTER.0 + 28)
    );
    fixture.acknowledge_pending_delta();
    let bits_address = GuestAddress(read_u32(
        &fixture
            .runtime
            .memory
            .read(BITS_OUTPUT_ADDRESS, 4)
            .expect("DIB bits output"),
        0,
    ));
    assert_ne!(bits_address, GuestAddress(0));

    let (descriptor, backing) = {
        let object = fixture
            .runtime
            .gdi_objects()
            .values()
            .find(|object| object.handle == dib_handle)
            .expect("logical DIB object");
        assert!(!object.shared);
        let RuntimeGdiObjectKind::DibSection {
            descriptor,
            backing,
        } = &object.object
        else {
            panic!("created object is not a DIB section")
        };
        (descriptor.clone(), *backing)
    };
    assert_eq!(descriptor.width, 128);
    assert_eq!(descriptor.height, -96);
    assert_eq!(descriptor.stride_bytes, 128);
    assert_eq!(descriptor.pixel_byte_length, 12_288);
    assert_eq!(descriptor.palette.len(), 256);
    assert_eq!(backing.range.start(), bits_address);
    assert_eq!(backing.range.byte_length(), 12_288);
    assert_eq!(
        fixture
            .runtime
            .process_address_space_allocator()
            .allocation(backing.address_allocation_id)
            .expect("DIB address allocation")
            .purpose,
        ProcessAddressAllocationPurpose::GdiResource
    );
    assert!(fixture.runtime.memory.regions().any(|region| {
        region.address_allocation_id == Some(backing.address_allocation_id.0)
            && region.owner == backing.memory_owner
            && region.kind == MemoryRegionKind::GdiResource
            && region.range == backing.range
    }));
    assert_eq!(
        fixture.runtime.memory.stats().owners,
        memory_before_create.owners + 1
    );
    assert_eq!(
        fixture.runtime.memory.stats().regions,
        memory_before_create.regions + 1
    );
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(bits_address, 12_288)
            .expect("DIB pixels"),
        vec![0; 12_288]
    );
    assert_eq!(
        fixture
            .runtime
            .process_address_space_allocator()
            .records()
            .len(),
        address_records_before_create + 1
    );

    fixture
        .runtime
        .memory
        .write(OBJECT_OUTPUT_ADDRESS, &[0x5a; 32])
        .expect("initialize BITMAP output guards");
    fixture.acknowledge_pending_delta();
    let get_object = match fixture.dispatch_get_object(dib_handle, 24, OBJECT_OUTPUT_ADDRESS) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected GetObjectA completion, got {other:?}"),
    };
    assert_eq!(get_object.eax, Some(24));
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
    fixture.acknowledge_pending_delta();
    let bitmap = fixture
        .runtime
        .memory
        .read(OBJECT_OUTPUT_ADDRESS, 32)
        .expect("x86 BITMAP and guards");
    assert_eq!(read_u32(&bitmap, 0), 0);
    assert_eq!(read_i32(&bitmap, 4), 128);
    assert_eq!(read_i32(&bitmap, 8), 96);
    assert_eq!(read_u32(&bitmap, 12), 128);
    assert_eq!(u16::from_le_bytes(bitmap[16..18].try_into().unwrap()), 1);
    assert_eq!(u16::from_le_bytes(bitmap[18..20].try_into().unwrap()), 8);
    assert_eq!(read_u32(&bitmap, 20), bits_address.0);
    assert_eq!(&bitmap[24..], &[0x5a; 8]);

    let select_bitmap = match fixture.dispatch_select_object(memory_dc, dib_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected bitmap SelectObject completion, got {other:?}"),
    };
    assert_eq!(select_bitmap.eax, Some(default_bitmap_handle));
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
    let select_font = match fixture.dispatch_select_object(memory_dc, font_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected font SelectObject completion, got {other:?}"),
    };
    assert_eq!(select_font.eax, Some(default_font_handle));
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );

    let dc_before_selected_delete = fixture
        .runtime
        .gdi_device_context(memory_dc)
        .unwrap()
        .clone();
    let bitmap_before_selected_delete = fixture
        .runtime
        .gdi_objects()
        .values()
        .find(|object| object.handle == dib_handle)
        .unwrap()
        .clone();
    let object_count_before_selected_delete = fixture.runtime.gdi_objects().len();
    let clock_before_selected_delete = fixture.runtime.clock().clone();
    let thread_before_selected_delete = fixture.runtime.current_thread_id();
    let address_records_before_selected_delete = fixture
        .runtime
        .process_address_space_allocator()
        .records()
        .cloned()
        .collect::<Vec<_>>();
    let pixels_before_selected_delete = fixture
        .runtime
        .memory
        .read(bits_address, 12_288)
        .expect("selected DIB pixels");
    let selected_delete = match fixture.dispatch_delete_object(dib_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected selected DeleteObject completion, got {other:?}"),
    };
    assert_eq!(selected_delete.eax, Some(0));
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
    assert_eq!(
        fixture.runtime.gdi_device_context(memory_dc).unwrap(),
        &dc_before_selected_delete
    );
    assert_eq!(
        fixture
            .runtime
            .gdi_objects()
            .values()
            .find(|object| object.handle == dib_handle)
            .unwrap(),
        &bitmap_before_selected_delete
    );
    assert_eq!(
        fixture.runtime.gdi_objects().len(),
        object_count_before_selected_delete
    );
    assert_eq!(fixture.runtime.clock(), &clock_before_selected_delete);
    assert_eq!(
        fixture.runtime.current_thread_id(),
        thread_before_selected_delete
    );
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
    assert_eq!(
        fixture
            .runtime
            .process_address_space_allocator()
            .records()
            .cloned()
            .collect::<Vec<_>>(),
        address_records_before_selected_delete
    );
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(bits_address, 12_288)
            .expect("preserved selected DIB pixels"),
        pixels_before_selected_delete
    );

    let restore_bitmap = match fixture.dispatch_select_object(memory_dc, default_bitmap_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected bitmap restore completion, got {other:?}"),
    };
    assert_eq!(restore_bitmap.eax, Some(dib_handle));
    let restore_font = match fixture.dispatch_select_object(memory_dc, default_font_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected font restore completion, got {other:?}"),
    };
    assert_eq!(restore_font.eax, Some(font_handle));

    let delete_dib = match fixture.dispatch_delete_object(dib_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected DIB DeleteObject completion, got {other:?}"),
    };
    assert_eq!(delete_dib.eax, Some(1));
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
    fixture.acknowledge_pending_delta();
    assert!(
        fixture
            .runtime
            .gdi_objects()
            .values()
            .all(|object| object.handle != dib_handle)
    );
    assert!(fixture.runtime.memory.read(bits_address, 1).is_err());
    assert!(
        fixture
            .runtime
            .process_address_space_allocator()
            .allocation(backing.address_allocation_id)
            .is_err()
    );
    assert!(
        fixture
            .runtime
            .memory
            .regions()
            .all(|region| region.kind != MemoryRegionKind::GdiResource)
    );
    assert_eq!(
        fixture.runtime.memory.stats().owners,
        memory_before_create.owners
    );
    assert_eq!(
        fixture.runtime.memory.stats().regions,
        memory_before_create.regions
    );

    fixture
        .runtime
        .memory
        .write(OBJECT_OUTPUT_ADDRESS, &[0x6b; 32])
        .expect("initialize stale GetObjectA guard");
    fixture.acknowledge_pending_delta();
    let object_count_before_stale_get = fixture.runtime.gdi_objects().len();
    let clock_before_stale_get = fixture.runtime.clock().clone();
    let stale_get = match fixture.dispatch_get_object(dib_handle, 24, OBJECT_OUTPUT_ADDRESS) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected stale GetObjectA completion, got {other:?}"),
    };
    assert_eq!(stale_get.eax, Some(0));
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
    assert_eq!(
        fixture.runtime.gdi_objects().len(),
        object_count_before_stale_get
    );
    assert!(
        !fixture
            .runtime
            .gdi_objects()
            .values()
            .any(|object| object.handle == dib_handle)
    );
    assert_eq!(fixture.runtime.clock(), &clock_before_stale_get);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(OBJECT_OUTPUT_ADDRESS, 32)
            .expect("preserved stale GetObjectA guard"),
        vec![0x6b; 32]
    );

    let delete_font = match fixture.dispatch_delete_object(font_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected font DeleteObject completion, got {other:?}"),
    };
    assert_eq!(delete_font.eax, Some(1));
    let delete_dc = match fixture.dispatch_delete_dc(memory_dc) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected memory DeleteDC completion, got {other:?}"),
    };
    assert_eq!(delete_dc.eax, Some(1));
}

#[test]
fn dib_deletion_keeps_the_object_and_allocation_when_guest_backing_release_fails() {
    let mut fixture = FontFixture::new();
    let memory_dc = match fixture.dispatch_create_compatible_dc(0) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            Some(handle) => handle,
            other => panic!("CreateCompatibleDC did not return an HDC: {other:?}"),
        },
        other => panic!("CreateCompatibleDC stopped: {other:?}"),
    };
    fixture.install_bitmap_info(&primary_bitmap_info());
    fixture.acknowledge_pending_delta();
    let creation = match fixture.dispatch_create_dib_section(
        memory_dc,
        BITMAP_INFO_ADDRESS,
        BITS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("CreateDIBSection stopped: {other:?}"),
    };
    let Some(dib_handle) = creation.eax else {
        panic!("CreateDIBSection did not return a bitmap")
    };
    fixture.acknowledge_pending_delta();
    let backing = match &fixture
        .runtime
        .gdi_objects()
        .values()
        .find(|object| object.handle == dib_handle)
        .expect("created bitmap")
        .object
    {
        RuntimeGdiObjectKind::DibSection { backing, .. } => *backing,
        other => panic!("created object is not a DIB: {other:?}"),
    };
    fixture
        .runtime
        .memory
        .release_region(backing.memory_owner, backing.range.start())
        .expect("remove guest backing to exercise failed release");
    fixture.acknowledge_pending_delta();
    let object_before = fixture
        .runtime
        .gdi_objects()
        .values()
        .find(|object| object.handle == dib_handle)
        .unwrap()
        .clone();
    let dc_before = fixture
        .runtime
        .gdi_device_context(memory_dc)
        .unwrap()
        .clone();
    let clock_before = fixture.runtime.clock().clone();
    let object_count = fixture.runtime.gdi_objects().len();
    let memory_before = fixture.runtime.memory.regions().collect::<Vec<_>>();
    let result = fixture.dispatch_delete_object(dib_handle);
    assert!(
        matches!(result, DispatchOutcome::Fault(_)),
        "missing authoritative DIB backing must fault: {result:?}"
    );
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
    assert_eq!(
        fixture
            .runtime
            .gdi_objects()
            .values()
            .find(|object| object.handle == dib_handle)
            .unwrap(),
        &object_before
    );
    assert_eq!(
        fixture.runtime.gdi_device_context(memory_dc).unwrap(),
        &dc_before
    );
    assert_eq!(fixture.runtime.gdi_objects().len(), object_count);
    assert_eq!(fixture.runtime.clock(), &clock_before);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
    assert_eq!(
        fixture.runtime.memory.regions().collect::<Vec<_>>(),
        memory_before
    );
    assert!(
        fixture
            .runtime
            .process_address_space_allocator()
            .allocation(backing.address_allocation_id)
            .is_ok()
    );
}

#[test]
fn delete_dc_rejects_screen_and_cross_thread_handles_without_retiring_them() {
    let mut fixture = FontFixture::new();
    let screen = fixture.acquire_screen_dc();
    let memory_dc = match fixture.dispatch_create_compatible_dc(0) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            Some(handle) => handle,
            other => panic!("CreateCompatibleDC did not return an HDC: {other:?}"),
        },
        other => panic!("CreateCompatibleDC stopped: {other:?}"),
    };
    let screen_before = fixture.runtime.gdi_device_context(screen).unwrap().clone();
    let dc_before = fixture
        .runtime
        .gdi_device_context(memory_dc)
        .unwrap()
        .clone();
    let clock_before = fixture.runtime.clock().clone();
    let selected_thread_before = fixture.runtime.current_thread_id();
    let result = fixture.dispatch_delete_dc(screen);
    let DispatchOutcome::Immediate(completion) = result else {
        panic!("screen DeleteDC did not return FALSE: {result:?}");
    };
    assert_eq!(completion.eax, Some(0));
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
    assert_eq!(
        fixture.runtime.gdi_device_context(screen).unwrap(),
        &screen_before
    );
    assert_eq!(
        fixture.runtime.gdi_device_context(memory_dc).unwrap(),
        &dc_before
    );
    assert_eq!(fixture.runtime.clock(), &clock_before);
    assert_eq!(fixture.runtime.current_thread_id(), selected_thread_before);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );

    let result = fixture.dispatch_delete_dc(0x4701_ffff);
    let DispatchOutcome::Immediate(completion) = result else {
        panic!("unknown DeleteDC did not return FALSE: {result:?}");
    };
    assert_eq!(completion.eax, Some(0));
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
    assert_eq!(
        fixture.runtime.gdi_device_context(screen).unwrap(),
        &screen_before
    );
    assert_eq!(
        fixture.runtime.gdi_device_context(memory_dc).unwrap(),
        &dc_before
    );
    assert_eq!(fixture.runtime.clock(), &clock_before);
    assert_eq!(fixture.runtime.current_thread_id(), selected_thread_before);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );

    fixture
        .runtime
        .register_thread(RuntimeThreadId(THREAD_ID + 1), INITIAL_LAST_ERROR)
        .unwrap();
    fixture
        .runtime
        .select_thread(RuntimeThreadId(THREAD_ID + 1))
        .unwrap();
    let screen_before = fixture.runtime.gdi_device_context(screen).unwrap().clone();
    let dc_before = fixture
        .runtime
        .gdi_device_context(memory_dc)
        .unwrap()
        .clone();
    let clock_before = fixture.runtime.clock().clone();
    let selected_thread_before = fixture.runtime.current_thread_id();
    let result = fixture.dispatch_delete_dc(memory_dc);
    let DispatchOutcome::Immediate(completion) = result else {
        panic!("cross-thread DeleteDC did not return FALSE: {result:?}");
    };
    assert_eq!(completion.eax, Some(0));
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
    assert_eq!(
        fixture.runtime.gdi_device_context(screen).unwrap(),
        &screen_before
    );
    assert_eq!(
        fixture.runtime.gdi_device_context(memory_dc).unwrap(),
        &dc_before
    );
    assert_eq!(fixture.runtime.clock(), &clock_before);
    assert_eq!(fixture.runtime.current_thread_id(), selected_thread_before);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
    fixture
        .runtime
        .select_thread(RuntimeThreadId(THREAD_ID))
        .unwrap();
    assert!(matches!(fixture.dispatch_delete_dc(memory_dc),
        DispatchOutcome::Immediate(ref completion) if completion.eax == Some(1)));
    assert!(fixture.runtime.gdi_device_context(memory_dc).is_err());
    assert_eq!(
        fixture.runtime.gdi_device_context(screen).unwrap(),
        &screen_before
    );
}

#[test]
fn create_font_a_accepts_arbitrary_bounded_sizes_and_preserves_state_on_bad_inputs() {
    let mut fixture = FontFixture::new();
    fixture.install_face_name(&primary_face_name());
    let mut arguments = primary_create_font_arguments();
    arguments[0] = 37;
    arguments[1] = 19;
    fixture.configure_create_font(arguments, FACE_NAME_ADDRESS);
    let completion = match fixture.dispatch_create_font() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("bounded font was not created: {other:?}"),
    };
    let Some(handle) = completion.eax else {
        panic!("missing font handle")
    };
    let object = fixture
        .runtime
        .gdi_objects()
        .values()
        .find(|object| object.handle == handle)
        .unwrap();
    let crate::runtime::RuntimeGdiObjectKind::FontA { log_font } = &object.object else {
        panic!("expected font")
    };
    assert_eq!((log_font.height, log_font.width), (37, 19));
    let font_before = object.clone();
    let object_count = fixture.runtime.gdi_objects().len();
    for invalid_height in [0_u32, 4097, i32::MIN as u32] {
        arguments[0] = invalid_height;
        fixture.configure_create_font(arguments, FACE_NAME_ADDRESS);
        let clock_before = fixture.runtime.clock().clone();
        let last_error_before = fixture.runtime.current_last_error().unwrap();
        let thread_before = fixture.runtime.current_thread_id();
        let scheduler_before = fixture.runtime.scheduler_state().unwrap();
        let memory_before = fixture.runtime.memory.stats();
        assert!(matches!(
            fixture.dispatch_create_font(),
            DispatchOutcome::Fault(_)
        ));
        assert_eq!(fixture.runtime.gdi_objects().len(), object_count);
        assert_eq!(
            fixture
                .runtime
                .gdi_objects()
                .values()
                .find(|object| object.handle == handle)
                .unwrap(),
            &font_before
        );
        assert_eq!(fixture.runtime.clock(), &clock_before);
        assert_eq!(
            fixture.runtime.current_last_error().unwrap(),
            last_error_before
        );
        assert_eq!(fixture.runtime.current_thread_id(), thread_before);
        assert_eq!(fixture.runtime.scheduler_state().unwrap(), scheduler_before);
        assert_eq!(fixture.runtime.memory.stats(), memory_before);
    }
}

#[test]
fn text_raster_defers_only_after_validating_a_selected_cleared_dib() {
    let mut fixture = FontFixture::new();
    let surface = prepare_primary_text_surface(&mut fixture);
    let request = match fixture.dispatch_text_out(surface.device_context, TEXT_OUT_BYTES_ADDRESS, 1)
    {
        DispatchOutcome::Deferred(call) => call,
        other => panic!("generic TextOutA did not request a browser glyph: {other:?}"),
    };
    assert_eq!(read_u32(&request.request.payload, 36), 64);
    assert_eq!(read_u32(&request.request.payload, 60), 12_288);
    assert_eq!(read_u32(&request.request.payload, 68), u32::from(b'a'));
    let dc_before = fixture
        .runtime
        .gdi_device_context(surface.device_context)
        .unwrap()
        .clone();
    let clock_before = fixture.runtime.clock().clone();
    let last_error_before = fixture.runtime.current_last_error().unwrap();
    let scheduler_before = fixture.runtime.scheduler_state().unwrap();
    let thread_before = fixture.runtime.current_thread_id();
    let (invalid, no_completion) = fixture.complete_text_raster(&request, vec![255]);
    assert!(matches!(invalid, DispatchOutcome::Fault(_)));
    assert!(no_completion.is_none());
    assert_eq!(
        fixture
            .runtime
            .gdi_device_context(surface.device_context)
            .unwrap(),
        &dc_before
    );
    assert_eq!(fixture.runtime.clock(), &clock_before);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );
    assert_eq!(fixture.runtime.scheduler_state().unwrap(), scheduler_before);
    assert_eq!(fixture.runtime.current_thread_id(), thread_before);
    assert_eq!(
        fixture.runtime.memory.read(surface.bits, 12_288).unwrap(),
        vec![0; 12_288]
    );
    let mut pixels = vec![0; 12_288];
    pixels[32 * 128 + 5] = 255;
    let (result, completion) = fixture.complete_text_raster(&request, pixels.clone());
    assert!(matches!(result, DispatchOutcome::Immediate(_)));
    assert!(completion.is_some());
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture.runtime.memory.read(surface.bits, 12_288).unwrap(),
        pixels
    );

    let dc_before = fixture
        .runtime
        .gdi_device_context(surface.device_context)
        .unwrap()
        .clone();
    let clock_before = fixture.runtime.clock().clone();
    let last_error_before = fixture.runtime.current_last_error().unwrap();
    let scheduler_before = fixture.runtime.scheduler_state().unwrap();
    let thread_before = fixture.runtime.current_thread_id();
    assert!(matches!(
        fixture.dispatch_text_out(surface.device_context, TEXT_OUT_BYTES_ADDRESS, 1),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(
        fixture
            .runtime
            .gdi_device_context(surface.device_context)
            .unwrap(),
        &dc_before
    );
    assert_eq!(fixture.runtime.clock(), &clock_before);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );
    assert_eq!(fixture.runtime.scheduler_state().unwrap(), scheduler_before);
    assert_eq!(fixture.runtime.current_thread_id(), thread_before);
    assert_eq!(
        fixture.runtime.memory.read(surface.bits, 12_288).unwrap(),
        pixels
    );
}

#[test]
fn outline_metrics_invalid_or_cross_thread_dc_preserves_guest_output() {
    let mut fixture = FontFixture::new();
    let output = OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS;
    fixture.runtime.memory.write(output, &[0xa5; 269]).unwrap();
    fixture.acknowledge_pending_delta();
    let before_objects = fixture.runtime.gdi_objects().clone();
    for invalid_dc in [0, 0x4701_ffff] {
        let result = fixture.dispatch_outline_text_metrics(invalid_dc, 269, output);
        let DispatchOutcome::Immediate(completion) = result else {
            panic!("invalid HDC did not return a Windows failure: {result:?}");
        };
        assert_eq!(completion.eax, Some(0));
        assert_eq!(fixture.runtime.current_last_error().unwrap(), 6);
        assert_eq!(
            fixture.runtime.memory.read(output, 269).unwrap(),
            [0xa5; 269]
        );
        assert_eq!(fixture.runtime.gdi_objects(), &before_objects);
    }
    let (_font, memory_dc, _) = fixture.create_primary_font_memory_dc();
    fixture
        .runtime
        .register_thread(RuntimeThreadId(THREAD_ID + 1), INITIAL_LAST_ERROR)
        .unwrap();
    fixture
        .runtime
        .select_thread(RuntimeThreadId(THREAD_ID + 1))
        .unwrap();
    let result = fixture.dispatch_outline_text_metrics(memory_dc, 269, output);
    let DispatchOutcome::Immediate(completion) = result else {
        panic!("cross-thread HDC did not return a Windows failure: {result:?}");
    };
    assert_eq!(completion.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 6);
    assert_eq!(
        fixture.runtime.memory.read(output, 269).unwrap(),
        [0xa5; 269]
    );
}

#[test]
fn synchronous_gdi_results_preserve_live_last_error_after_windows_failures() {
    let mut fixture = FontFixture::new();
    let (_, memory_dc, _) = fixture.create_primary_font_memory_dc();
    fixture.runtime.set_current_last_error(0x1357_2468).unwrap();
    let DispatchOutcome::Immediate(invalid) = fixture.dispatch_outline_text_metrics(
        0x4701_ffff,
        269,
        OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS,
    ) else {
        panic!("invalid HDC did not produce a Windows failure")
    };
    assert_eq!(invalid.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 6);
    let DispatchOutcome::Immediate(query) =
        fixture.dispatch_outline_text_metrics(memory_dc, 0, GuestAddress(0))
    else {
        panic!("metrics query failed")
    };
    assert_eq!(query.eax, Some(269));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 6);

    let DispatchOutcome::Immediate(partial) =
        fixture.dispatch_outline_text_metrics(memory_dc, 213, OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS)
    else {
        panic!("partial metrics did not produce a Windows failure")
    };
    assert_eq!(partial.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1003);
    fixture.acknowledge_pending_delta();
    let DispatchOutcome::Immediate(created) = fixture.dispatch_create_compatible_dc(0) else {
        panic!("compatible DC creation failed")
    };
    assert!(matches!(created.eax, Some(handle) if handle != 0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1003);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1003);
}

#[test]
fn outline_metrics_scale_with_selected_font_and_preserve_bounds() {
    let mut fixture = FontFixture::new();
    let (_font, memory_dc, _) = fixture.create_primary_font_memory_dc();
    let dc_before = fixture
        .runtime
        .gdi_device_context(memory_dc)
        .unwrap()
        .clone();
    let clock_before = fixture.runtime.clock().clone();
    let last_error_before = fixture.runtime.current_last_error().unwrap();
    let thread_before = fixture.runtime.current_thread_id();
    let object_count = fixture.runtime.gdi_objects().len();
    let result = fixture.dispatch_outline_text_metrics(memory_dc, 0, GuestAddress(0));
    let DispatchOutcome::Immediate(result) = result else {
        panic!("outline size query failed")
    };
    assert_eq!(
        result.eax,
        Some(X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as u32)
    );
    assert_eq!(
        fixture.runtime.gdi_device_context(memory_dc).unwrap(),
        &dc_before
    );
    assert_eq!(fixture.runtime.clock(), &clock_before);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );
    assert_eq!(fixture.runtime.current_thread_id(), thread_before);
    assert_eq!(fixture.runtime.gdi_objects().len(), object_count);
    let dc_before = fixture
        .runtime
        .gdi_device_context(memory_dc)
        .unwrap()
        .clone();
    let clock_before = fixture.runtime.clock().clone();
    let last_error_before = fixture.runtime.current_last_error().unwrap();
    let thread_before = fixture.runtime.current_thread_id();
    let object_count = fixture.runtime.gdi_objects().len();
    let result = fixture.dispatch_outline_text_metrics(
        memory_dc,
        X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as u32,
        OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS,
    );
    let DispatchOutcome::Immediate(result) = result else {
        panic!("full outline read failed")
    };
    assert_eq!(
        result.eax,
        Some(X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as u32)
    );
    assert_eq!(
        fixture.runtime.gdi_device_context(memory_dc).unwrap(),
        &dc_before
    );
    assert_eq!(fixture.runtime.clock(), &clock_before);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );
    assert_eq!(fixture.runtime.current_thread_id(), thread_before);
    assert_eq!(fixture.runtime.gdi_objects().len(), object_count);
    fixture.acknowledge_pending_delta();
    let bytes = fixture
        .runtime
        .memory
        .read(
            OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS,
            X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH,
        )
        .unwrap();
    assert_eq!(read_u32(&bytes, 0), 269);
    assert_eq!(read_i32(&bytes, 4), 64);
    assert_eq!(read_i32(&bytes, 24), 32);
    assert_eq!(read_u32(&bytes, 196), 212);
    assert_eq!(&bytes[212..225], MS_GOTHIC_WINDOWS_932);

    fixture.install_face_name(&primary_face_name());
    let mut arguments = primary_create_font_arguments();
    arguments[0] = 37;
    arguments[1] = 19;
    fixture.configure_create_font(arguments, FACE_NAME_ADDRESS);
    let DispatchOutcome::Immediate(font) = fixture.dispatch_create_font() else {
        panic!("unprofiled font creation failed")
    };
    let Some(font) = font.eax else {
        panic!("missing font handle")
    };
    assert!(matches!(
        fixture.dispatch_select_object(memory_dc, font),
        DispatchOutcome::Immediate(_)
    ));
    fixture.acknowledge_pending_delta();
    assert!(matches!(
        fixture.dispatch_outline_text_metrics(memory_dc, 212, OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS),
        DispatchOutcome::Immediate(_)
    ));
    fixture.acknowledge_pending_delta();
    let fixed = fixture
        .runtime
        .memory
        .read(OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS, 216)
        .unwrap();
    assert_eq!(read_u32(&fixed, 0), 212);
    assert_eq!(read_i32(&fixed, 4), 37);
    assert_eq!(read_i32(&fixed, 24), 19);
    assert_eq!(read_u32(&fixed, 196), 0);
    assert_eq!(&fixed[212..216], &bytes[212..216]); // untouched tail from prior full read

    arguments[0] = 144;
    arguments[1] = 144;
    fixture.configure_create_font(arguments, FACE_NAME_ADDRESS);
    let DispatchOutcome::Immediate(font) = fixture.dispatch_create_font() else {
        panic!("wide font creation failed")
    };
    let Some(font) = font.eax else {
        panic!("missing wide font handle")
    };
    assert!(matches!(
        fixture.dispatch_select_object(memory_dc, font),
        DispatchOutcome::Immediate(_)
    ));
    fixture.acknowledge_pending_delta();
    assert!(matches!(
        fixture.dispatch_outline_text_metrics(memory_dc, 269, OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS),
        DispatchOutcome::Immediate(_)
    ));
    fixture.acknowledge_pending_delta();
    let wide = fixture
        .runtime
        .memory
        .read(OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS, 269)
        .unwrap();
    assert_eq!(read_i32(&wide, 24), 144);
    assert_eq!(read_i32(&wide, 108), 72);
    assert_eq!(read_i32(&wide, 116), -281);
    assert_eq!(read_i32(&wide, 124), 288);
}

#[test]
fn dib_uses_guest_dimensions_and_preserves_state_on_overflow() {
    let mut fixture = FontFixture::new();
    let memory_dc = match fixture.dispatch_create_compatible_dc(0) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            Some(handle) => handle,
            other => panic!("missing memory DC: {other:?}"),
        },
        other => panic!("memory DC was not created: {other:?}"),
    };
    let bitmap_info = bitmap_info_bytes(37, -19);
    fixture.install_bitmap_info(&bitmap_info);
    let result =
        fixture.dispatch_create_dib_section(memory_dc, BITMAP_INFO_ADDRESS, BITS_OUTPUT_ADDRESS);
    let completion = match result {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("bounded DIB failed: {other:?}"),
    };

    let Some(bitmap) = completion.eax else {
        panic!("missing bitmap handle")
    };
    assert_ne!(bitmap, 0);
    fixture.acknowledge_pending_delta();
    let bits = read_u32(
        &fixture.runtime.memory.read(BITS_OUTPUT_ADDRESS, 4).unwrap(),
        0,
    );
    assert_ne!(bits, 0);
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(GuestAddress(bits), 37 * 19 * 4)
            .unwrap(),
        vec![0; 37 * 19 * 4]
    );
    fixture.install_bitmap_info(&bitmap_info_bytes(i32::MAX, i32::MAX));
    let dc_before = fixture
        .runtime
        .gdi_device_context(memory_dc)
        .unwrap()
        .clone();
    let bitmap_before = fixture
        .runtime
        .gdi_objects()
        .values()
        .find(|object| object.handle == bitmap)
        .unwrap()
        .clone();
    let object_count = fixture.runtime.gdi_objects().len();
    let last_error_before = fixture.runtime.current_last_error().unwrap();
    assert!(matches!(
        fixture.dispatch_create_dib_section(memory_dc, BITMAP_INFO_ADDRESS, BITS_OUTPUT_ADDRESS),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(
        fixture.runtime.gdi_device_context(memory_dc).unwrap(),
        &dc_before
    );
    assert_eq!(
        fixture
            .runtime
            .gdi_objects()
            .values()
            .find(|object| object.handle == bitmap)
            .unwrap(),
        &bitmap_before
    );
    assert_eq!(fixture.runtime.gdi_objects().len(), object_count);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture.runtime.memory.read(BITS_OUTPUT_ADDRESS, 4).unwrap(),
        bits.to_le_bytes()
    );
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(GuestAddress(bits), 37 * 19 * 4)
            .unwrap(),
        vec![0; 37 * 19 * 4]
    );
}

#[test]
fn gdi_creation_capacity_returns_null_without_publishing_partial_defaults() {
    let mut fixture = FontFixture::new();
    fixture.install_face_name(&primary_face_name());
    fixture.configure_create_font(primary_create_font_arguments(), FACE_NAME_ADDRESS);
    let DispatchOutcome::Immediate(first) = fixture.dispatch_create_font() else {
        panic!("initial font creation failed")
    };
    let first = first.eax.unwrap();
    let RuntimeGdiObjectKind::FontA { log_font } = &fixture
        .runtime
        .gdi_objects()
        .values()
        .find(|object| object.handle == first)
        .unwrap()
        .object
    else {
        panic!("created object is not a font")
    };
    let font = log_font.clone();
    let mut last_created = first;
    while let Some(handle) = fixture.runtime.create_gdi_font_a(font.clone()).unwrap() {
        last_created = handle;
    }
    assert!(fixture.runtime.delete_gdi_object(last_created).unwrap());
    fixture.write_words(
        CREATE_COMPATIBLE_DC_STACK_POINTER,
        &[CREATE_COMPATIBLE_DC_RETURN_ADDRESS.0, 0],
    );
    fixture.runtime.set_current_last_error(0x1234).unwrap();
    let objects_before = fixture.runtime.gdi_objects().clone();
    let memory_before = fixture.runtime.memory.stats();
    let clock_before = fixture.runtime.clock().clone();
    let dc_endpoint = fixture.create_compatible_dc_endpoint.clone();
    let DispatchOutcome::Immediate(no_dc) = fixture.dispatch_endpoint(
        dc_endpoint,
        CREATE_COMPATIBLE_DC_STACK_POINTER,
        CREATE_COMPATIBLE_DC_RETURN_ADDRESS,
    ) else {
        panic!("one remaining object slot must complete CreateCompatibleDC with NULL")
    };
    assert_eq!(no_dc.eax, Some(0));
    assert_eq!(fixture.runtime.gdi_objects(), &objects_before);
    assert!(fixture.runtime.gdi_device_contexts().is_empty());
    assert_eq!(fixture.runtime.memory.stats(), memory_before);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x1234);
    assert_eq!(fixture.runtime.clock(), &clock_before);

    assert!(fixture.runtime.create_gdi_font_a(font).unwrap().is_some());
    let objects_before = fixture.runtime.gdi_objects().clone();
    let DispatchOutcome::Immediate(no_font) = fixture.dispatch_create_font() else {
        panic!("full object capacity must complete CreateFontA with NULL")
    };
    assert_eq!(no_font.eax, Some(0));
    assert_eq!(fixture.runtime.gdi_objects(), &objects_before);
    assert_eq!(fixture.runtime.memory.stats(), memory_before);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x1234);

    let mut endpoint = fixture.create_compatible_dc_endpoint.clone();
    endpoint.symbol = crate::api::ApiSymbol::Name {
        name: "GetStockObject".to_owned(),
    };
    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap();
    fixture.write_words(
        CREATE_COMPATIBLE_DC_STACK_POINTER,
        &[CREATE_COMPATIBLE_DC_RETURN_ADDRESS.0, 4],
    );
    let memory_before = fixture.runtime.memory.stats();
    let outcome = dispatcher.dispatch_owned(
        ApiOwnedTrap {
            endpoint_id: endpoint.id,
            endpoint_address: endpoint.address,
            stack_pointer: CREATE_COMPATIBLE_DC_STACK_POINTER,
            return_address: CREATE_COMPATIBLE_DC_RETURN_ADDRESS,
        },
        &mut fixture.runtime,
        None,
        |frame, runtime, _| dispatch_gdi32_api(CALLBACK_SENTINEL, frame, runtime),
    );
    let DispatchOutcome::Immediate(no_stock) = outcome else {
        panic!("full object capacity must complete GetStockObject with NULL")
    };
    assert_eq!(no_stock.eax, Some(0));
    assert_eq!(fixture.runtime.gdi_objects(), &objects_before);
    assert_eq!(fixture.runtime.memory.stats(), memory_before);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 0x1234);
}

#[test]
fn create_font_a_creates_distinct_rust_owned_objects_without_guest_writes() {
    let mut fixture = FontFixture::new();
    fixture.install_face_name(&primary_face_name());
    let mut handles = Vec::new();
    for _ in 0..2 {
        fixture.configure_create_font(primary_create_font_arguments(), FACE_NAME_ADDRESS);
        let memory_before = fixture.runtime.memory.stats();
        let DispatchOutcome::Immediate(completion) = fixture.dispatch_create_font() else {
            panic!("font creation failed")
        };
        let Some(handle) = completion.eax else {
            panic!("missing font handle")
        };
        assert_ne!(handle, 0);
        assert_eq!(fixture.runtime.memory.stats(), memory_before);
        handles.push(handle);
    }
    assert_ne!(handles[0], handles[1]);
    assert_eq!(fixture.runtime.gdi_objects().len(), 2);
}

#[test]
fn enum_font_families_ex_a_materializes_primary_record_and_propagates_callback_result() {
    for callback_result in [0_u32, 1] {
        let mut fixture = FontFixture::new();
        let device_context = fixture.acquire_screen_dc();
        assert!(matches!(
            fixture
                .runtime
                .gdi_device_context(device_context)
                .expect("live screen HDC")
                .target,
            RuntimeDeviceContextTarget::Screen { .. }
        ));
        let filter = primary_filter();
        fixture.install_filter(&filter);
        fixture.configure_enum(device_context, LOG_FONT_ADDRESS, CALLBACK_TARGET, 0);
        let dc_before = fixture
            .runtime
            .gdi_device_context(device_context)
            .unwrap()
            .clone();
        let clock_before = fixture.runtime.clock().clone();
        let last_error_before = fixture.runtime.current_last_error().unwrap();
        let thread_before = fixture.runtime.current_thread_id();
        let object_count = fixture.runtime.gdi_objects().len();
        let memory_generation_before = fixture.runtime.memory.stats().generation;

        let call = match fixture.dispatch_enum() {
            DispatchOutcome::GuestCallback(call) => call,
            other => panic!("expected font callback, got {other:?}"),
        };
        assert_eq!(
            fixture.runtime.gdi_device_context(device_context).unwrap(),
            &dc_before
        );
        assert_eq!(fixture.runtime.clock(), &clock_before);
        assert_eq!(
            fixture.runtime.current_last_error().unwrap(),
            last_error_before
        );
        assert_eq!(fixture.runtime.current_thread_id(), thread_before);
        assert_eq!(fixture.runtime.gdi_objects().len(), object_count);
        assert_eq!(
            fixture.runtime.memory.stats().generation,
            memory_generation_before
        );
        assert_eq!(call.request.target, CALLBACK_TARGET);
        assert_eq!(
            call.request.arguments,
            [
                GuestCallbackArgument::ScratchPointer { offset: 0 },
                GuestCallbackArgument::ScratchPointer {
                    offset: X86_ENUMLOGFONTEXA_BYTE_LENGTH as u32,
                },
                GuestCallbackArgument::U32(4),
                GuestCallbackArgument::U32(CALLBACK_PARAMETER),
            ]
        );
        assert_eq!(
            call.request.completion_policy,
            GuestCallbackCompletionPolicy::ReturnU32
        );
        assert_primary_record(&call.request.scratch);

        let prepared = fixture
            .runtime
            .begin_guest_callback(call.clone())
            .expect("prepare font callback");
        fixture.acknowledge_pending_delta();
        assert_eq!(
            prepared.scratch_bytes,
            (X86_ENUMLOGFONTEXA_BYTE_LENGTH + X86_NEWTEXTMETRICEXA_BYTE_LENGTH) as u32
        );
        assert_eq!(prepared.frame_bytes, 20);
        assert_eq!(
            fixture
                .runtime
                .memory
                .read(prepared.scratch_address, prepared.scratch_bytes as usize)
                .expect("callback scratch"),
            call.request.scratch
        );
        let mut expected_frame = CALLBACK_SENTINEL.0.to_le_bytes().to_vec();
        expected_frame.extend_from_slice(&prepared.scratch_address.0.to_le_bytes());
        expected_frame.extend_from_slice(
            &prepared
                .scratch_address
                .0
                .wrapping_add(X86_ENUMLOGFONTEXA_BYTE_LENGTH as u32)
                .to_le_bytes(),
        );
        expected_frame.extend_from_slice(&4_u32.to_le_bytes());
        expected_frame.extend_from_slice(&CALLBACK_PARAMETER.to_le_bytes());
        assert_eq!(
            fixture
                .runtime
                .memory
                .read(prepared.allocation_start, prepared.frame_bytes as usize)
                .expect("callback argument frame"),
            expected_frame
        );
        assert_eq!(
            fixture
                .runtime
                .memory
                .read(LOG_FONT_ADDRESS, filter.len())
                .expect("preserved LOGFONTA"),
            filter
        );

        let completion = fixture
            .runtime
            .complete_guest_callback(&fixture.dispatcher, CALLBACK_SENTINEL, callback_result)
            .expect("complete live font callback");
        let GuestCallbackReturn::Completed(completion) = completion else {
            panic!("expected completed font enumeration")
        };
        assert_eq!(completion.eax, Some(callback_result));
        assert_eq!(completion.instruction_pointer, ENUM_RETURN_ADDRESS);
        assert_eq!(
            completion.stack_pointer,
            GuestAddress(ENUM_STACK_POINTER.0 + 24)
        );
        assert_eq!(
            fixture.runtime.current_last_error().unwrap(),
            INITIAL_LAST_ERROR
        );
    }
}

#[test]
fn enum_font_families_ex_a_rejects_unreached_filters_flags_and_callbacks_atomically() {
    for (filter, callback, flags) in [
        (filter_with_charset(0), CALLBACK_TARGET, 0),
        (unsupported_face_filter(), CALLBACK_TARGET, 0),
        (unterminated_face_filter(), CALLBACK_TARGET, 0),
        (primary_filter(), CALLBACK_TARGET, 1),
        (primary_filter(), GET_DC_STACK_POINTER, 0),
    ] {
        let mut fixture = FontFixture::new();
        let device_context = fixture.acquire_screen_dc();
        fixture.install_filter(&filter);
        fixture.configure_enum(device_context, LOG_FONT_ADDRESS, callback, flags);
        assert_enum_fault_atomic(&mut fixture);
    }
}

#[test]
fn enum_font_families_ex_a_requires_live_same_thread_screen_dc_atomically() {
    let mut unknown = FontFixture::new();
    unknown.install_filter(&primary_filter());
    unknown.configure_enum(0x4701_ffff, LOG_FONT_ADDRESS, CALLBACK_TARGET, 0);
    assert_enum_fault_atomic(&mut unknown);

    let mut stale = FontFixture::new();
    let stale_handle = stale.acquire_screen_dc();
    stale.release_screen_dc(stale_handle);
    stale.install_filter(&primary_filter());
    stale.configure_enum(stale_handle, LOG_FONT_ADDRESS, CALLBACK_TARGET, 0);
    assert_enum_fault_atomic(&mut stale);

    let mut cross_thread = FontFixture::new();
    let device_context = cross_thread.acquire_screen_dc();
    cross_thread
        .runtime
        .register_thread(RuntimeThreadId(THREAD_ID + 1), INITIAL_LAST_ERROR)
        .expect("register second thread");
    cross_thread
        .runtime
        .select_thread(RuntimeThreadId(THREAD_ID + 1))
        .expect("select second thread");
    cross_thread.install_filter(&primary_filter());
    cross_thread.configure_enum(device_context, LOG_FONT_ADDRESS, CALLBACK_TARGET, 0);
    assert_enum_fault_atomic(&mut cross_thread);
}

#[test]
fn font_callback_stack_failure_preserves_memory_and_can_retry() {
    let mut fixture = FontFixture::with_enum_stack(LOW_ENUM_STACK_POINTER);
    let device_context = fixture.acquire_screen_dc();
    fixture.install_filter(&primary_filter());
    fixture.configure_enum(device_context, LOG_FONT_ADDRESS, CALLBACK_TARGET, 0);
    let call = match fixture.dispatch_enum() {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected font callback, got {other:?}"),
    };
    let clock_before = fixture.runtime.clock().clone();
    let last_error_before = fixture.runtime.current_last_error().unwrap();
    let thread_before = fixture.runtime.current_thread_id();
    let object_count = fixture.runtime.gdi_objects().len();
    let memory_before = fixture.runtime.memory.stats();
    let reflection_before = fixture
        .runtime
        .memory
        .current_reflection()
        .expect("current reflection");
    fixture
        .runtime
        .begin_guest_callback(call.clone())
        .expect_err("low callback stack must fail");

    assert_eq!(fixture.runtime.clock(), &clock_before);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );
    assert_eq!(fixture.runtime.current_thread_id(), thread_before);
    assert_eq!(fixture.runtime.gdi_objects().len(), object_count);
    assert_eq!(fixture.runtime.memory.stats(), memory_before);
    assert_eq!(
        fixture
            .runtime
            .memory
            .current_reflection()
            .expect("unchanged reflection"),
        reflection_before
    );
    fixture.enum_stack_pointer = ENUM_STACK_POINTER;
    fixture.configure_enum(device_context, LOG_FONT_ADDRESS, CALLBACK_TARGET, 0);
    let DispatchOutcome::GuestCallback(call) = fixture.dispatch_enum() else {
        panic!("expected font callback after stack correction")
    };
    fixture
        .runtime
        .begin_guest_callback(call)
        .expect("valid stack permits retry after failed preparation");
    fixture.acknowledge_pending_delta();
    let completion = fixture
        .runtime
        .complete_guest_callback(&fixture.dispatcher, CALLBACK_SENTINEL, 7)
        .expect("complete retried font callback");
    let GuestCallbackReturn::Completed(completion) = completion else {
        panic!("expected completed retried font enumeration")
    };
    assert_eq!(completion.eax, Some(7));
    assert_eq!(completion.instruction_pointer, ENUM_RETURN_ADDRESS);
    assert_eq!(
        completion.stack_pointer,
        GuestAddress(ENUM_STACK_POINTER.0 + 24)
    );
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        INITIAL_LAST_ERROR
    );
}

fn assert_enum_fault_atomic(fixture: &mut FontFixture) {
    let clock_before = fixture.runtime.clock().clone();
    let last_error_before = fixture.runtime.current_last_error().unwrap();
    let thread_before = fixture.runtime.current_thread_id();
    let object_count = fixture.runtime.gdi_objects().len();
    let memory_before = fixture.runtime.memory.stats();
    let reflection_before = fixture
        .runtime
        .memory
        .current_reflection()
        .expect("current reflection");
    match fixture.dispatch_enum() {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected enumeration fault, got {other:?}"),
    };

    assert_eq!(fixture.runtime.clock(), &clock_before);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        last_error_before
    );
    assert_eq!(fixture.runtime.current_thread_id(), thread_before);
    assert_eq!(fixture.runtime.gdi_objects().len(), object_count);
    assert_eq!(fixture.runtime.memory.stats(), memory_before);
    assert_eq!(
        fixture
            .runtime
            .memory
            .current_reflection()
            .expect("unchanged reflection"),
        reflection_before
    );
}

fn primary_bitmap_info() -> Vec<u8> {
    bitmap_info_bytes(128, -96)
}

fn bitmap_info_bytes(width: i32, height: i32) -> Vec<u8> {
    let mut bytes = vec![0; X86_DIB_BITMAPINFO_BYTE_LENGTH];
    bytes[0..4].copy_from_slice(&X86_BITMAPINFOHEADER_BYTE_LENGTH.to_le_bytes());
    bytes[4..8].copy_from_slice(&width.to_le_bytes());
    bytes[8..12].copy_from_slice(&height.to_le_bytes());
    bytes[12..14].copy_from_slice(&1_u16.to_le_bytes());
    bytes[14..16].copy_from_slice(&8_u16.to_le_bytes());
    bytes[32..36].copy_from_slice(&256_u32.to_le_bytes());
    for (index, color) in bytes[X86_BITMAPINFOHEADER_BYTE_LENGTH as usize..]
        .chunks_exact_mut(4)
        .enumerate()
    {
        let intensity = u8::try_from(255 - index).expect("256-entry DIB palette");
        color[0] = intensity;
        color[1] = intensity;
        color[2] = intensity;
    }
    bytes
}

fn primary_create_font_arguments() -> [u32; 13] {
    [64, 32, 0, 0, 100, 0, 0, 0, 128, 4, 0, 2, 0]
}

fn primary_face_name() -> Vec<u8> {
    let mut bytes = MS_GOTHIC_WINDOWS_932.to_vec();
    bytes.push(0);
    bytes
}

fn primary_filter() -> Vec<u8> {
    filter_with_charset(128)
}

fn filter_with_charset(character_set: u8) -> Vec<u8> {
    let mut filter = vec![0xa5; X86_LOGFONTA_BYTE_LENGTH as usize];
    filter[23] = character_set;
    filter[28..28 + MS_GOTHIC_WINDOWS_932.len()].copy_from_slice(MS_GOTHIC_WINDOWS_932);
    filter[28 + MS_GOTHIC_WINDOWS_932.len()] = 0;
    filter
}

fn unsupported_face_filter() -> Vec<u8> {
    let mut filter = primary_filter();
    filter[28] ^= 1;
    filter
}

fn unterminated_face_filter() -> Vec<u8> {
    let mut filter = primary_filter();
    filter[28..60].fill(0xa5);
    filter
}

fn assert_primary_record(scratch: &[u8]) {
    assert_eq!(
        scratch.len(),
        X86_ENUMLOGFONTEXA_BYTE_LENGTH + X86_NEWTEXTMETRICEXA_BYTE_LENGTH
    );
    let font = &scratch[..X86_ENUMLOGFONTEXA_BYTE_LENGTH];
    let metric = &scratch[X86_ENUMLOGFONTEXA_BYTE_LENGTH..];
    assert_eq!(read_i32(font, 0), 32);
    assert_eq!(read_i32(font, 4), 16);
    assert_eq!(read_i32(font, 16), 400);
    assert_eq!(&font[23..28], &[128, 3, 2, 1, 49]);
    assert_eq!(&font[28..41], MS_GOTHIC_WINDOWS_932);
    assert_eq!(&font[60..73], MS_GOTHIC_WINDOWS_932);
    assert_eq!(&font[124..128], &[0x95, 0x57, 0x8f, 0x80]);
    assert_eq!(&font[156..162], &[0x93, 0xfa, 0x96, 0x7b, 0x8c, 0xea]);
    assert_eq!(read_i32(metric, 0), 32);
    assert_eq!(read_i32(metric, 4), 28);
    assert_eq!(read_i32(metric, 8), 4);
    assert_eq!(read_i32(metric, 20), 16);
    assert_eq!(read_i32(metric, 24), 63);
    assert_eq!(read_i32(metric, 28), 400);
    assert_eq!(&metric[44..48], &[0, 255, 165, 2]);
    assert_eq!(&metric[51..53], &[54, 128]);
    assert_eq!(read_u32(metric, 56), 2_359_360);
    assert_eq!(read_u32(metric, 60), 256);
    assert_eq!(read_u32(metric, 64), 256);
    assert_eq!(read_u32(metric, 68), 128);
    assert_eq!(
        &metric[72..96],
        &[
            0xff, 0x02, 0x00, 0xe0, 0xfb, 0xfd, 0xc7, 0x6a, 0x12, 0x00, 0x00, 0x08, 0x00, 0x00,
            0x00, 0x00, 0x9f, 0x00, 0x02, 0x40, 0x00, 0x00, 0xd7, 0xdf,
        ]
    );
}

#[test]
fn bitmapv4_16bpp_bitfields_dib_and_general_shift_jis_font_render_text_out_a() {
    let mut fixture = FontFixture::new();
    let memory_dc = match fixture.dispatch_create_compatible_dc(0) {
        DispatchOutcome::Immediate(done) => done.eax.expect("memory DC"),
        other => panic!("CreateCompatibleDC stopped: {other:?}"),
    };
    // General upright Windows-932 font with default width=0 and ANTIALIASED_QUALITY (4).
    let mut ms_mincho = b"\x82\x6c\x82\x72\x20\x96\xbe\x92\xa9\0".to_vec();
    ms_mincho.resize(32, 0);
    fixture.install_face_name(&ms_mincho);
    let font_args = [28_u32, 0, 0, 0, 700, 0, 0, 0, 0x80, 0, 0, 4, 0x11];
    fixture.configure_create_font(font_args, FACE_NAME_ADDRESS);
    let font = match fixture.dispatch_create_font() {
        DispatchOutcome::Immediate(done) => done.eax.expect("font handle"),
        other => panic!("CreateFontA stopped: {other:?}"),
    };
    assert_ne!(font, 0);

    // 108-byte BITMAPV4HEADER with 16bpp BI_BITFIELDS (0x7c00 / 0x03e0 / 0x001f).
    let mut v4 = vec![0_u8; 108];
    v4[0..4].copy_from_slice(&108_u32.to_le_bytes());
    v4[4..8].copy_from_slice(&16_i32.to_le_bytes());
    v4[8..12].copy_from_slice(&(-16_i32).to_le_bytes());
    v4[12..14].copy_from_slice(&1_u16.to_le_bytes());
    v4[14..16].copy_from_slice(&16_u16.to_le_bytes());
    v4[16..20].copy_from_slice(&3_u32.to_le_bytes()); // BI_BITFIELDS
    v4[40..44].copy_from_slice(&0x7c00_u32.to_le_bytes());
    v4[44..48].copy_from_slice(&0x03e0_u32.to_le_bytes());
    v4[48..52].copy_from_slice(&0x001f_u32.to_le_bytes());
    fixture
        .runtime
        .memory
        .write(BITMAP_INFO_ADDRESS, &v4)
        .unwrap();
    fixture.acknowledge_pending_delta();
    let dib = match fixture.dispatch_create_dib_section(
        memory_dc,
        BITMAP_INFO_ADDRESS,
        BITS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(done) => done.eax.expect("DIB handle"),
        other => panic!("CreateDIBSection BITMAPV4HEADER stopped: {other:?}"),
    };
    assert_ne!(dib, 0);
    fixture.acknowledge_pending_delta();
    let bits_addr = GuestAddress(read_u32(
        &fixture.runtime.memory.read(BITS_OUTPUT_ADDRESS, 4).unwrap(),
        0,
    ));

    assert!(matches!(
        fixture.dispatch_select_object(memory_dc, dib),
        DispatchOutcome::Immediate(_)
    ));
    assert!(matches!(
        fixture.dispatch_select_object(memory_dc, font),
        DispatchOutcome::Immediate(_)
    ));
    assert_eq!(
        fixture
            .runtime
            .set_gdi_text_attribute(memory_dc, 0, 0x00ff_ffff)
            .unwrap(),
        Some(0)
    );
    assert_eq!(
        fixture
            .runtime
            .set_gdi_text_attribute(memory_dc, 2, 1)
            .unwrap(),
        Some(2)
    );
    fixture
        .runtime
        .memory
        .write(TEXT_OUT_BYTES_ADDRESS, b"A")
        .unwrap();
    fixture.acknowledge_pending_delta();

    let deferred = match fixture.dispatch_text_out(memory_dc, TEXT_OUT_BYTES_ADDRESS, 1) {
        DispatchOutcome::Deferred(call) => call,
        other => panic!("16bpp TextOutA did not defer TXT2 request: {other:?}"),
    };
    assert_eq!(read_u32(&deferred.request.payload, 0), 0x3254_5854); // "TXT2"

    let mut coverage = vec![0_u8; 16 * 16];
    coverage[3 * 16 + 2] = 255;
    let (done, host) = fixture.complete_text_raster(&deferred, coverage);
    assert!(matches!(done, DispatchOutcome::Immediate(ref c) if c.eax == Some(1)));
    assert!(host.is_some());
    fixture.acknowledge_pending_delta();
    let out = fixture.runtime.memory.read(bits_addr, 16 * 32).unwrap();
    let word = u16::from_le_bytes([out[3 * 32 + 4], out[3 * 32 + 5]]);
    assert_eq!(word, 0x7fff);
}

fn read_i32(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("i32 bytes"))
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32 bytes"))
}

fn endpoint(
    owner: MemoryOwnerId,
    id: u32,
    address: GuestAddress,
    iat_address: GuestAddress,
    module: &str,
    name: &str,
) -> ApiEndpointIdentity {
    ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(id),
        address,
        requester_owner: owner,
        module: module.to_owned(),
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
        max_physical_pages: 64,
        max_regions: 3,
        max_owners: 3,
        max_access_bytes: 0x40000,
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

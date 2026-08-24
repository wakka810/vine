use vine_core::{
    ApiBehavior, ApiDeferredCall, ApiDispatcher, ApiEndpointIdentity, ApiFaultClass, ApiOwnedTrap,
    ApiPointeeSchema, ApiReturnSchema, ApiStringEncoding, ApiValueSchema,
    CALLBACK_SCRATCH_ARENA_BASE, CallbackPolicy, DispatchOutcome, ExecutionSession,
    GUEST_PAGE_SIZE, Gdi32CompatibilityPolicy, Gdi32TextRasterHostResponse, GuestAddress,
    GuestCallbackArgument, GuestCallbackCompletionPolicy, GuestMemory, GuestRange, ImportTarget,
    ImportThunk, ImportThunkDisposition, ImportThunkId, LastErrorPolicy, MemoryLimits,
    MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind, PageProtection,
    ProcessAddressAllocationPurpose, ProcessAddressSpaceLayout, RUNTIME_GDI_TEXT_RASTER_SERVICE,
    RegisterUpdate, RuntimeBootstrap, RuntimeCallbackFrameId, RuntimeClockState,
    RuntimeCurrentDirectoryA, RuntimeDeviceContextTarget, RuntimeGdiObjectKind,
    RuntimeHostCompletion, RuntimeProcessId, RuntimeProcessImagePathA, RuntimeSchedulerEvent,
    RuntimeStartupInfo, RuntimeState, RuntimeThreadId, User32CompatibilityPolicy,
    X86_BITMAP_BYTE_LENGTH, X86_BITMAPINFOHEADER_BYTE_LENGTH, X86_DIB_BITMAPINFO_BYTE_LENGTH,
    X86_ENUMLOGFONTEXA_BYTE_LENGTH, X86_LOGFONTA_BYTE_LENGTH, X86_NEWTEXTMETRICEXA_BYTE_LENGTH,
    X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH, X86_OUTLINE_TEXT_METRIC_A_FIXED_BYTE_LENGTH,
    complete_gdi32_text_raster, create_compatible_dc_contract, create_dib_section_contract,
    create_font_a_contract, delete_dc_contract, delete_object_contract,
    dispatch_gdi32_api_with_policy, dispatch_user32_api_with_policy,
    enum_font_families_ex_a_contract, gdi32_contract_for_endpoint, get_dc_contract,
    get_object_a_contract, get_outline_text_metrics_a_contract, release_dc_contract,
    select_object_contract, text_out_a_contract,
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
    memory: GuestMemory,
    runtime: RuntimeState,
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
    user32_policy: User32CompatibilityPolicy,
    gdi32_policy: Gdi32CompatibilityPolicy,
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

        let dispatcher = ApiDispatcher::new(
            [
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
            ],
            [
                get_dc_contract(get_dc_endpoint.clone()).expect("GetDC contract"),
                release_dc_contract(release_dc_endpoint.clone()).expect("ReleaseDC contract"),
                enum_font_families_ex_a_contract(enum_endpoint.clone())
                    .expect("EnumFontFamiliesExA contract"),
                create_font_a_contract(create_font_endpoint.clone()).expect("CreateFontA contract"),
                create_compatible_dc_contract(create_compatible_dc_endpoint.clone())
                    .expect("CreateCompatibleDC contract"),
                delete_dc_contract(delete_dc_endpoint.clone()).expect("DeleteDC contract"),
                create_dib_section_contract(create_dib_endpoint.clone())
                    .expect("CreateDIBSection contract"),
                get_object_a_contract(get_object_endpoint.clone()).expect("GetObjectA contract"),
                select_object_contract(select_object_endpoint.clone())
                    .expect("SelectObject contract"),
                delete_object_contract(delete_object_endpoint.clone())
                    .expect("DeleteObject contract"),
                get_outline_text_metrics_a_contract(outline_text_metrics_endpoint.clone())
                    .expect("GetOutlineTextMetricsA contract"),
                text_out_a_contract(text_out_endpoint.clone()).expect("TextOutA contract"),
            ],
        )
        .expect("GDI fixture dispatcher");
        let user32_policy = User32CompatibilityPolicy::controlled_v30();
        let gdi32_policy = Gdi32CompatibilityPolicy::controlled_v9();
        user32_policy.validate().expect("valid User32 policy");
        gdi32_policy.validate().expect("valid GDI32 policy");
        let mut runtime = controlled_runtime();
        runtime
            .configure_display_profile(user32_policy.display_profile().expect("display profile"))
            .expect("configure logical display");

        Self {
            memory,
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
            user32_policy,
            gdi32_policy,
            callback_sentinel: CALLBACK_SENTINEL,
            enum_stack_pointer,
        }
    }

    fn acquire_screen_dc(&mut self) -> u32 {
        self.write_words(GET_DC_STACK_POINTER, &[GET_DC_RETURN_ADDRESS.0, 0]);
        let endpoint = self.get_dc_endpoint.clone();
        match self.dispatch_endpoint(endpoint, GET_DC_STACK_POINTER, GET_DC_RETURN_ADDRESS) {
            DispatchOutcome::Immediate(completion) => match completion.eax {
                RegisterUpdate::Set(handle) => handle,
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
        assert_eq!(completion.eax, RegisterUpdate::Set(1));
    }

    fn install_filter(&mut self, filter: &[u8]) {
        assert_eq!(filter.len(), X86_LOGFONTA_BYTE_LENGTH as usize);
        self.memory
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
        self.memory
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
        self.memory
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

    fn create_post_heap_growth_font_memory_dc(&mut self) -> (u32, u32, u32) {
        self.create_font_memory_dc(post_heap_growth_create_font_arguments())
    }

    fn create_font_memory_dc(&mut self, arguments: [u32; 13]) -> (u32, u32, u32) {
        self.install_face_name(&primary_face_name());
        self.configure_create_font(arguments, FACE_NAME_ADDRESS);
        let font = match self.dispatch_create_font() {
            DispatchOutcome::Immediate(completion) => match completion.eax {
                RegisterUpdate::Set(handle) => handle,
                other => panic!("CreateFontA did not set EAX: {other:?}"),
            },
            other => panic!("expected CreateFontA completion, got {other:?}"),
        };
        let device_context = match self.dispatch_create_compatible_dc(0) {
            DispatchOutcome::Immediate(completion) => match completion.eax {
                RegisterUpdate::Set(handle) => handle,
                other => panic!("CreateCompatibleDC did not set EAX: {other:?}"),
            },
            other => panic!("expected CreateCompatibleDC completion, got {other:?}"),
        };
        let previous_font = match self.dispatch_select_object(device_context, font) {
            DispatchOutcome::Immediate(completion) => match completion.eax {
                RegisterUpdate::Set(handle) => handle,
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
        let endpoint = self.outline_text_metrics_endpoint.clone();
        self.dispatch_endpoint(
            endpoint,
            OUTLINE_TEXT_METRICS_STACK_POINTER,
            OUTLINE_TEXT_METRICS_RETURN_ADDRESS,
        )
    }

    fn dispatch_text_out(
        &mut self,
        device_context: u32,
        x: i32,
        y: i32,
        text: GuestAddress,
        character_count: u32,
    ) -> DispatchOutcome {
        self.write_words(
            TEXT_OUT_STACK_POINTER,
            &[
                TEXT_OUT_RETURN_ADDRESS.0,
                device_context,
                x as u32,
                y as u32,
                text.0,
                character_count,
            ],
        );
        let endpoint = self.text_out_endpoint.clone();
        self.dispatch_endpoint(endpoint, TEXT_OUT_STACK_POINTER, TEXT_OUT_RETURN_ADDRESS)
    }

    fn complete_text_raster(
        &mut self,
        call: ApiDeferredCall,
        pixels: Vec<u8>,
    ) -> (DispatchOutcome, Option<RuntimeHostCompletion>) {
        let request_id = call.request.request_id;
        complete_gdi32_text_raster(
            &self.dispatcher,
            call,
            Gdi32TextRasterHostResponse { request_id, pixels },
            &mut ExecutionSession::new(&mut self.memory, &mut self.runtime),
        )
    }

    fn write_words(&mut self, address: GuestAddress, words: &[u32]) {
        let bytes = words
            .iter()
            .copied()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        self.memory
            .write(address, &bytes)
            .expect("write call frame");
        self.acknowledge_pending_delta();
    }

    fn acknowledge_pending_delta(&mut self) {
        if self.memory.stats().pending_invalidation.is_some() {
            let delta = self
                .memory
                .pending_delta()
                .expect("pending GDI fixture delta");
            self.memory
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
        let user32_policy = &self.user32_policy;
        let gdi32_policy = &self.gdi32_policy;
        let callback_sentinel = self.callback_sentinel;
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
                    dispatch_gdi32_api_with_policy(
                        gdi32_policy,
                        callback_sentinel,
                        contract,
                        frame,
                        session,
                    )
                } else {
                    dispatch_user32_api_with_policy(
                        user32_policy,
                        callback_sentinel,
                        contract,
                        frame,
                        session,
                    )
                }
            },
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct PreparedTextSurface {
    device_context: u32,
    dib: u32,
    bits: GuestAddress,
    backing_owner: MemoryOwnerId,
    backing_range: GuestRange,
}

fn prepare_primary_text_surface(fixture: &mut FontFixture) -> PreparedTextSurface {
    let (_font, device_context, _default_font) = fixture.create_primary_font_memory_dc();
    fixture.install_bitmap_info(&primary_bitmap_info());
    fixture
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
        RegisterUpdate::Set(handle) => handle,
        other => panic!("CreateDIBSection did not set EAX: {other:?}"),
    };
    fixture.acknowledge_pending_delta();
    let bits = GuestAddress(read_u32(
        &fixture
            .memory
            .read(BITS_OUTPUT_ADDRESS, 4)
            .expect("read text DIB bits output"),
        0,
    ));
    let _default_bitmap = match fixture.dispatch_select_object(device_context, dib) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            RegisterUpdate::Set(handle) => handle,
            other => panic!("DIB SelectObject did not set EAX: {other:?}"),
        },
        other => panic!("expected text DIB selection, got {other:?}"),
    };
    let (backing_owner, backing_range) = fixture
        .runtime
        .gdi_objects()
        .values()
        .find_map(|object| {
            if object.handle != dib {
                return None;
            }
            match object.object {
                RuntimeGdiObjectKind::DibSection { backing, .. } => {
                    Some((backing.memory_owner, backing.range))
                }
                _ => None,
            }
        })
        .expect("text DIB backing");
    assert_eq!(backing_range.start(), bits);
    assert_eq!(
        fixture
            .memory
            .read(bits, 12_288)
            .expect("fresh text DIB pixels"),
        vec![0; 12_288]
    );
    fixture
        .memory
        .write(TEXT_OUT_BYTES_ADDRESS, b"a")
        .expect("install TextOutA bytes");
    fixture.acknowledge_pending_delta();
    PreparedTextSurface {
        device_context,
        dib,
        bits,
        backing_owner,
        backing_range,
    }
}

fn prepare_scenario_text_surface(fixture: &mut FontFixture) -> PreparedTextSurface {
    let (_font, device_context, _default_font) = fixture.create_post_heap_growth_font_memory_dc();
    fixture.install_bitmap_info(&post_create_font_bitmap_info());
    fixture
        .memory
        .write(BITS_OUTPUT_ADDRESS, &0xa5a5_a5a5_u32.to_le_bytes())
        .expect("initialize scenario text DIB bits output");
    fixture.acknowledge_pending_delta();
    let creation = match fixture.dispatch_create_dib_section(
        device_context,
        BITMAP_INFO_ADDRESS,
        BITS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected scenario text DIB creation, got {other:?}"),
    };
    let dib = match creation.eax {
        RegisterUpdate::Set(handle) => handle,
        other => panic!("scenario CreateDIBSection did not set EAX: {other:?}"),
    };
    fixture.acknowledge_pending_delta();
    let bits = GuestAddress(read_u32(
        &fixture
            .memory
            .read(BITS_OUTPUT_ADDRESS, 4)
            .expect("read scenario text DIB bits output"),
        0,
    ));
    assert!(matches!(
        fixture.dispatch_select_object(device_context, dib),
        DispatchOutcome::Immediate(_)
    ));
    let (backing_owner, backing_range) = fixture
        .runtime
        .gdi_objects()
        .values()
        .find_map(|object| {
            if object.handle != dib {
                return None;
            }
            match object.object {
                RuntimeGdiObjectKind::DibSection { backing, .. } => {
                    Some((backing.memory_owner, backing.range))
                }
                _ => None,
            }
        })
        .expect("scenario text DIB backing");
    fixture
        .memory
        .write(TEXT_OUT_BYTES_ADDRESS, &[0x81, 0x40])
        .expect("install scenario full-width space bytes");
    fixture.acknowledge_pending_delta();
    PreparedTextSurface {
        device_context,
        dib,
        bits,
        backing_owner,
        backing_range,
    }
}

#[test]
fn compatible_dc_catalog_exposes_nullable_source_and_owned_delete_contracts() {
    let fixture = FontFixture::new();
    let create = gdi32_contract_for_endpoint(fixture.create_compatible_dc_endpoint.clone())
        .expect("GDI32 catalog lookup")
        .expect("CreateCompatibleDC contract");
    assert_eq!(create.abi.arguments.len(), 1);
    assert!(matches!(
        &create.abi.arguments[0].value,
        ApiValueSchema::LogicalHandle { namespace, nullable }
            if namespace == "vine.logical-gdi-dc.v1" && *nullable
    ));
    assert!(matches!(
        &create.abi.return_value,
        ApiReturnSchema::LogicalHandle { namespace, nullable }
            if namespace == "vine.logical-gdi-dc.v1" && *nullable
    ));
    assert_eq!(create.last_error_policy, LastErrorPolicy::FromHandler);
    assert_eq!(create.callback_policy, CallbackPolicy::Forbidden);

    let delete = gdi32_contract_for_endpoint(fixture.delete_dc_endpoint.clone())
        .expect("GDI32 catalog lookup")
        .expect("DeleteDC contract");
    assert_eq!(delete.abi.arguments.len(), 1);
    assert!(matches!(
        &delete.abi.arguments[0].value,
        ApiValueSchema::LogicalHandle { namespace, nullable }
            if namespace == "vine.logical-gdi-dc.v1" && *nullable
    ));
    assert_eq!(delete.abi.return_value, ApiReturnSchema::Bool32);
    assert_eq!(delete.last_error_policy, LastErrorPolicy::FromHandler);
    assert_eq!(delete.callback_policy, CallbackPolicy::Forbidden);
}

#[test]
fn compatible_dc_creation_shares_defaults_and_delete_retires_only_the_exact_dc() {
    let mut fixture = FontFixture::new();
    let first_completion = match fixture.dispatch_create_compatible_dc(0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected CreateCompatibleDC completion, got {other:?}"),
    };
    let RegisterUpdate::Set(first_handle) = first_completion.eax else {
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
    assert_eq!(first_completion.last_error, INITIAL_LAST_ERROR);
    assert!(first_completion.guest_writes.is_empty());

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
    let RegisterUpdate::Set(second_handle) = second_completion.eax else {
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
    assert_eq!(delete_first.eax, RegisterUpdate::Set(1));
    assert_eq!(delete_first.last_error, INITIAL_LAST_ERROR);
    assert!(delete_first.guest_writes.is_empty());
    assert!(fixture.runtime.gdi_device_context(first_handle).is_err());
    assert!(fixture.runtime.gdi_device_context(second_handle).is_ok());
    assert_eq!(fixture.runtime.gdi_objects().len(), 2);

    let runtime_before_repeat = fixture.runtime.clone();
    let repeated_delete = match fixture.dispatch_delete_dc(first_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected repeated DeleteDC completion, got {other:?}"),
    };
    assert_eq!(repeated_delete.eax, RegisterUpdate::Set(0));
    assert_eq!(repeated_delete.last_error, INITIAL_LAST_ERROR);
    assert_eq!(fixture.runtime, runtime_before_repeat);
}

#[test]
fn compatible_dc_rejects_nonnull_or_disabled_creation_and_nonmemory_delete_atomically() {
    let mut nonnull = FontFixture::new();
    let runtime_before_nonnull = nonnull.runtime.clone();
    let fault = match nonnull.dispatch_create_compatible_dc(0x4701_0001) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected non-NULL CreateCompatibleDC fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "create-compatible-dc-source-unsupported");
    assert_eq!(nonnull.runtime, runtime_before_nonnull);

    let mut disabled = FontFixture::new();
    disabled.gdi32_policy.create_compatible_dc = None;
    let runtime_before_disabled = disabled.runtime.clone();
    let fault = match disabled.dispatch_create_compatible_dc(0) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected disabled CreateCompatibleDC fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "create-compatible-dc-policy-disabled");
    assert_eq!(disabled.runtime, runtime_before_disabled);

    let mut wrong_target = FontFixture::new();
    let screen_dc = wrong_target.acquire_screen_dc();
    let runtime_before_delete = wrong_target.runtime.clone();
    let completion = match wrong_target.dispatch_delete_dc(screen_dc) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected non-memory DeleteDC completion, got {other:?}"),
    };
    assert_eq!(completion.eax, RegisterUpdate::Set(0));
    assert_eq!(completion.last_error, INITIAL_LAST_ERROR);
    assert_eq!(wrong_target.runtime, runtime_before_delete);

    let runtime_before_null = wrong_target.runtime.clone();
    let completion = match wrong_target.dispatch_delete_dc(0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected NULL DeleteDC completion, got {other:?}"),
    };
    assert_eq!(completion.eax, RegisterUpdate::Set(0));
    assert_eq!(completion.last_error, INITIAL_LAST_ERROR);
    assert_eq!(wrong_target.runtime, runtime_before_null);
}

#[test]
fn dib_object_catalog_exposes_bounded_x86_lifecycle_contracts() {
    let fixture = FontFixture::new();
    let create = gdi32_contract_for_endpoint(fixture.create_dib_endpoint.clone())
        .expect("GDI32 catalog lookup")
        .expect("CreateDIBSection contract");
    assert_eq!(create.abi.arguments.len(), 6);
    assert!(matches!(
        &create.abi.arguments[1].value,
        ApiValueSchema::Pointer { constraint }
            if !constraint.nullable
                && constraint.alignment == 4
                && constraint.byte_length == X86_BITMAPINFOHEADER_BYTE_LENGTH
    ));
    assert!(matches!(
        &create.abi.arguments[3].value,
        ApiValueSchema::Pointer { constraint }
            if !constraint.nullable && constraint.byte_length == 4
    ));
    assert!(matches!(
        &create.abi.return_value,
        ApiReturnSchema::LogicalHandle { namespace, nullable }
            if namespace == "vine.logical-gdi-object.v1" && *nullable
    ));

    let get_object = gdi32_contract_for_endpoint(fixture.get_object_endpoint.clone())
        .expect("GDI32 catalog lookup")
        .expect("GetObjectA contract");
    assert_eq!(get_object.abi.arguments.len(), 3);
    assert!(matches!(
        &get_object.abi.arguments[2].value,
        ApiValueSchema::DynamicPointer { constraint }
            if !constraint.nullable
                && constraint.minimum_byte_length == 1
                && constraint.maximum_byte_length == X86_LOGFONTA_BYTE_LENGTH
    ));
    assert_eq!(get_object.abi.return_value, ApiReturnSchema::U32);

    let select = gdi32_contract_for_endpoint(fixture.select_object_endpoint.clone())
        .expect("GDI32 catalog lookup")
        .expect("SelectObject contract");
    assert_eq!(select.abi.arguments.len(), 2);
    assert!(matches!(
        &select.abi.return_value,
        ApiReturnSchema::LogicalHandle { namespace, nullable }
            if namespace == "vine.logical-gdi-object.v1" && *nullable
    ));

    let delete = gdi32_contract_for_endpoint(fixture.delete_object_endpoint.clone())
        .expect("GDI32 catalog lookup")
        .expect("DeleteObject contract");
    assert_eq!(delete.abi.arguments.len(), 1);
    assert_eq!(delete.abi.return_value, ApiReturnSchema::Bool32);
    for contract in [&create, &get_object, &select, &delete] {
        assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
        assert_eq!(contract.callback_policy, CallbackPolicy::Forbidden);
    }
}

#[test]
fn outline_text_metrics_catalog_exposes_nullable_size_query_and_bounded_output() {
    let fixture = FontFixture::new();
    let contract = gdi32_contract_for_endpoint(fixture.outline_text_metrics_endpoint.clone())
        .expect("GDI32 catalog lookup")
        .expect("GetOutlineTextMetricsA contract");
    assert_eq!(contract.abi.arguments.len(), 3);
    assert!(matches!(
        &contract.abi.arguments[0].value,
        ApiValueSchema::LogicalHandle { namespace, nullable }
            if namespace == "vine.logical-gdi-dc.v1" && *nullable
    ));
    assert!(matches!(
        &contract.abi.arguments[2].value,
        ApiValueSchema::DynamicPointer { constraint }
            if constraint.nullable
                && constraint.alignment == 4
                && constraint.minimum_byte_length == 1
                && constraint.maximum_byte_length
                    == X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as u32
    ));
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    assert_eq!(contract.callback_policy, CallbackPolicy::Forbidden);
}

#[test]
fn outline_text_metrics_matches_native_size_partial_failure_and_full_buffer_contracts() {
    let mut fixture = FontFixture::new();
    let (font, device_context, _default_font) = fixture.create_primary_font_memory_dc();
    let runtime_before_queries = fixture.runtime.clone();

    let size = match fixture.dispatch_outline_text_metrics(device_context, 0, GuestAddress(0)) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected outline size completion, got {other:?}"),
    };
    assert_eq!(
        size.eax,
        RegisterUpdate::Set(X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as u32)
    );
    assert_eq!(size.last_error, INITIAL_LAST_ERROR);
    assert_eq!(
        size.stack_pointer,
        GuestAddress(OUTLINE_TEXT_METRICS_STACK_POINTER.0 + 16)
    );
    assert!(size.guest_writes.is_empty());
    assert_eq!(fixture.runtime, runtime_before_queries);

    fixture
        .memory
        .write(OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS, &[0xa5; 320])
        .expect("initialize exact outline output");
    fixture.acknowledge_pending_delta();
    let exact = match fixture.dispatch_outline_text_metrics(
        device_context,
        X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as u32,
        OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected exact outline completion, got {other:?}"),
    };
    assert_eq!(
        exact.eax,
        RegisterUpdate::Set(X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as u32)
    );
    assert_eq!(exact.last_error, INITIAL_LAST_ERROR);
    assert_eq!(exact.guest_writes.len(), 1);
    assert_eq!(fixture.runtime, runtime_before_queries);
    fixture.acknowledge_pending_delta();
    let exact_bytes = fixture
        .memory
        .read(OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS, 320)
        .expect("exact outline output and guard");
    assert_eq!(read_u32(&exact_bytes, 0), 269);
    assert_eq!(read_i32(&exact_bytes, 4), 64);
    assert_eq!(read_i32(&exact_bytes, 8), 55);
    assert_eq!(read_i32(&exact_bytes, 12), 9);
    assert_eq!(&exact_bytes[57..60], &[0, 0, 0]);
    assert_eq!(exact_bytes[71], 0);
    assert_eq!(read_u32(&exact_bytes, 196), 212);
    assert_eq!(read_u32(&exact_bytes, 200), 226);
    assert_eq!(read_u32(&exact_bytes, 204), 240);
    assert_eq!(read_u32(&exact_bytes, 208), 245);
    assert_eq!(&exact_bytes[212..225], MS_GOTHIC_WINDOWS_932);
    assert_eq!(exact_bytes[225], 0);
    assert_eq!(&exact_bytes[226..239], MS_GOTHIC_WINDOWS_932);
    assert_eq!(exact_bytes[239], 0);
    assert_eq!(&exact_bytes[240..245], &[0x95, 0x57, 0x8f, 0x80, 0]);
    assert_eq!(&exact_bytes[245..255], b"Microsoft:");
    assert_eq!(&exact_bytes[255..268], MS_GOTHIC_WINDOWS_932);
    assert_eq!(exact_bytes[268], 0);
    assert_eq!(&exact_bytes[269..], &[0xa5; 51]);
    fixture.acknowledge_pending_delta();

    fixture
        .memory
        .write(OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS, &[0xa5; 320])
        .expect("initialize three-byte outline output");
    fixture.acknowledge_pending_delta();
    let three = match fixture.dispatch_outline_text_metrics(
        device_context,
        3,
        OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected three-byte outline completion, got {other:?}"),
    };
    assert_eq!(three.eax, RegisterUpdate::Set(3));
    assert_eq!(three.last_error, INITIAL_LAST_ERROR);
    fixture.acknowledge_pending_delta();
    let three_bytes = fixture
        .memory
        .read(OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS, 8)
        .expect("three-byte outline output and guard");
    assert_eq!(&three_bytes[..3], &[0x22, 0x01, 0x00]);
    assert_eq!(&three_bytes[3..], &[0xa5; 5]);
    fixture.acknowledge_pending_delta();

    fixture
        .memory
        .write(OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS, &[0xa5; 320])
        .expect("initialize fixed outline output");
    fixture.acknowledge_pending_delta();
    let fixed = match fixture.dispatch_outline_text_metrics(
        device_context,
        X86_OUTLINE_TEXT_METRIC_A_FIXED_BYTE_LENGTH as u32,
        OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected fixed outline completion, got {other:?}"),
    };
    assert_eq!(fixed.eax, RegisterUpdate::Set(212));
    assert_eq!(fixed.last_error, INITIAL_LAST_ERROR);
    fixture.acknowledge_pending_delta();
    let fixed_bytes = fixture
        .memory
        .read(OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS, 216)
        .expect("fixed outline output and guard");
    assert_eq!(read_u32(&fixed_bytes, 0), 212);
    for offset in [196, 200, 204, 208] {
        assert_eq!(read_u32(&fixed_bytes, offset), 0);
    }
    assert_eq!(&fixed_bytes[212..], &[0xa5; 4]);
    fixture.acknowledge_pending_delta();

    fixture
        .memory
        .write(OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS, &[0xa5; 320])
        .expect("initialize insufficient outline output");
    fixture.acknowledge_pending_delta();
    let insufficient = match fixture.dispatch_outline_text_metrics(
        device_context,
        268,
        OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected insufficient outline completion, got {other:?}"),
    };
    assert_eq!(insufficient.eax, RegisterUpdate::Set(0));
    assert_eq!(insufficient.last_error, 1003);
    fixture.acknowledge_pending_delta();
    let insufficient_bytes = fixture
        .memory
        .read(OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS, 272)
        .expect("insufficient outline output and guard");
    assert_eq!(read_u32(&insufficient_bytes, 0), 0);
    for offset in [196, 200, 204, 208] {
        assert_eq!(read_u32(&insufficient_bytes, offset), 0);
    }
    assert_eq!(&insufficient_bytes[212..], &[0xa5; 60]);
    fixture.acknowledge_pending_delta();
    fixture
        .runtime
        .set_current_last_error(INITIAL_LAST_ERROR)
        .expect("restore outline LastError sentinel");

    fixture
        .memory
        .write(OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS, &[0xa5; 320])
        .expect("initialize oversized outline output");
    fixture.acknowledge_pending_delta();
    let oversized = match fixture.dispatch_outline_text_metrics(
        device_context,
        285,
        OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected oversized outline completion, got {other:?}"),
    };
    assert_eq!(oversized.eax, RegisterUpdate::Set(269));
    assert_eq!(oversized.last_error, INITIAL_LAST_ERROR);
    fixture.acknowledge_pending_delta();
    let oversized_bytes = fixture
        .memory
        .read(OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS, 285)
        .expect("oversized outline output and tail");
    assert_eq!(read_u32(&oversized_bytes, 0), 269);
    assert_eq!(&oversized_bytes[269..], &[0xa5; 16]);
    assert_eq!(fixture.runtime, runtime_before_queries);
    fixture.acknowledge_pending_delta();

    let runtime_before_null = fixture.runtime.clone();
    let fault = match fixture.dispatch_outline_text_metrics(device_context, 269, GuestAddress(0)) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected NULL outline output fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::InvalidCall);
    assert_eq!(fault.code, "outline-text-metrics-output-null");
    assert_eq!(fixture.runtime, runtime_before_null);

    let deleted = match fixture.dispatch_delete_dc(device_context) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected memory DC deletion, got {other:?}"),
    };
    assert_eq!(deleted.eax, RegisterUpdate::Set(1));
    let runtime_after_delete = fixture.runtime.clone();
    let stale = match fixture.dispatch_outline_text_metrics(device_context, 0, GuestAddress(0)) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected stale outline completion, got {other:?}"),
    };
    assert_eq!(stale.eax, RegisterUpdate::Set(0));
    assert_eq!(stale.last_error, 6);
    assert!(stale.guest_writes.is_empty());
    fixture
        .runtime
        .set_current_last_error(INITIAL_LAST_ERROR)
        .expect("restore stale-HDC LastError sentinel");
    assert_eq!(fixture.runtime, runtime_after_delete);
    let delete_font = match fixture.dispatch_delete_object(font) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected font deletion, got {other:?}"),
    };
    assert_eq!(delete_font.eax, RegisterUpdate::Set(1));
}

#[test]
fn post_heap_growth_font_outline_metrics_match_native_selected_font_profile() {
    let mut fixture = FontFixture::new();
    let (font, device_context, default_font) = fixture.create_post_heap_growth_font_memory_dc();
    let runtime_before_queries = fixture.runtime.clone();

    let size = match fixture.dispatch_outline_text_metrics(device_context, 0, GuestAddress(0)) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected post-font outline size completion, got {other:?}"),
    };
    assert_eq!(
        size.eax,
        RegisterUpdate::Set(X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as u32)
    );
    assert_eq!(size.last_error, INITIAL_LAST_ERROR);
    assert!(size.guest_writes.is_empty());
    assert_eq!(fixture.runtime, runtime_before_queries);

    fixture
        .memory
        .write(OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS, &[0xa5; 320])
        .expect("initialize post-font outline output");
    fixture.acknowledge_pending_delta();
    let full = match fixture.dispatch_outline_text_metrics(
        device_context,
        X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as u32,
        OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected post-font outline full completion, got {other:?}"),
    };
    assert_eq!(
        full.eax,
        RegisterUpdate::Set(X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as u32)
    );
    assert_eq!(full.last_error, INITIAL_LAST_ERROR);
    assert_eq!(full.guest_writes.len(), 1);
    fixture.acknowledge_pending_delta();
    let bytes = fixture
        .memory
        .read(OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS, 320)
        .expect("post-font outline output and guard");
    for (offset, value) in [
        (0, 269),
        (4, 225),
        (8, 193),
        (12, 32),
        (16, 1),
        (20, 0),
        (24, 112),
        (28, 442),
        (32, 400),
        (36, 0),
        (40, 96),
        (44, 96),
        (80, 1),
        (84, 0),
        (88, 0),
        (92, 256),
        (96, 193),
        (100, -32),
        (104, 0),
        (108, 112),
        (112, 56),
        (116, -219),
        (120, 193),
        (124, 223),
        (128, -31),
        (132, 193),
        (136, -32),
        (140, 0),
        (144, 25),
        (148, 112),
        (152, 112),
        (156, 0),
        (160, 81),
        (164, 112),
        (168, 112),
        (172, 0),
        (176, 193),
        (180, 11),
        (184, 58),
        (188, 17),
        (192, -15),
        (196, 212),
        (200, 226),
        (204, 240),
        (208, 245),
    ] {
        assert_eq!(read_i32(&bytes, offset), value, "field at offset {offset}");
    }
    assert_eq!(&bytes[57..60], &[0, 0, 0]);
    assert_eq!(bytes[71], 0);
    assert_eq!(&bytes[212..225], MS_GOTHIC_WINDOWS_932);
    assert_eq!(bytes[225], 0);
    assert_eq!(&bytes[226..239], MS_GOTHIC_WINDOWS_932);
    assert_eq!(bytes[239], 0);
    assert_eq!(&bytes[240..245], &[0x95, 0x57, 0x8f, 0x80, 0]);
    assert_eq!(&bytes[245..255], b"Microsoft:");
    assert_eq!(&bytes[255..268], MS_GOTHIC_WINDOWS_932);
    assert_eq!(bytes[268], 0);
    assert_eq!(&bytes[269..], &[0xa5; 51]);
    assert_eq!(fixture.runtime, runtime_before_queries);

    let restore = match fixture.dispatch_select_object(device_context, default_font) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected post-font restore completion, got {other:?}"),
    };
    assert_eq!(restore.eax, RegisterUpdate::Set(font));
    let delete_font = match fixture.dispatch_delete_object(font) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected post-font deletion, got {other:?}"),
    };
    assert_eq!(delete_font.eax, RegisterUpdate::Set(1));
    let delete_dc = match fixture.dispatch_delete_dc(device_context) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected post-font DC deletion, got {other:?}"),
    };
    assert_eq!(delete_dc.eax, RegisterUpdate::Set(1));
}

#[test]
fn dialogue_font_dib_metrics_and_text_raster_form_one_reached_vertical_slice() {
    let mut fixture = FontFixture::new();
    let (_font, device_context, _default_font) =
        fixture.create_font_memory_dc(dialogue_create_font_arguments());
    fixture
        .memory
        .write(OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS, &[0xa5; 320])
        .expect("initialize dialogue outline output");
    fixture.acknowledge_pending_delta();
    let outline = match fixture.dispatch_outline_text_metrics(
        device_context,
        X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as u32,
        OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected dialogue outline metrics, got {other:?}"),
    };
    assert_eq!(
        outline.eax,
        RegisterUpdate::Set(X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as u32)
    );
    fixture.acknowledge_pending_delta();
    let metrics = fixture
        .memory
        .read(
            OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS,
            X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH,
        )
        .expect("dialogue outline metrics bytes");
    for (offset, value) in [
        (4, 80),
        (8, 69),
        (12, 11),
        (24, 40),
        (28, 158),
        (96, 69),
        (100, -11),
        (192, -5),
    ] {
        assert_eq!(read_i32(&metrics, offset), value);
    }

    fixture.install_bitmap_info(&dialogue_bitmap_info());
    fixture
        .memory
        .write(BITS_OUTPUT_ADDRESS, &0xa5a5_a5a5_u32.to_le_bytes())
        .expect("initialize dialogue DIB output");
    fixture.acknowledge_pending_delta();
    let dib = match fixture.dispatch_create_dib_section(
        device_context,
        BITMAP_INFO_ADDRESS,
        BITS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            RegisterUpdate::Set(handle) => handle,
            other => panic!("dialogue CreateDIBSection did not set EAX: {other:?}"),
        },
        other => panic!("expected dialogue DIB creation, got {other:?}"),
    };
    fixture.acknowledge_pending_delta();
    let bits = GuestAddress(read_u32(
        &fixture.memory.read(BITS_OUTPUT_ADDRESS, 4).unwrap(),
        0,
    ));
    assert!(matches!(
        fixture.dispatch_select_object(device_context, dib),
        DispatchOutcome::Immediate(_)
    ));
    fixture
        .memory
        .write(TEXT_OUT_BYTES_ADDRESS, &[0x81, 0x40])
        .expect("install dialogue CP932 glyph");
    fixture.acknowledge_pending_delta();
    let call = match fixture.dispatch_text_out(device_context, 0, 0, TEXT_OUT_BYTES_ADDRESS, 2) {
        DispatchOutcome::Deferred(call) => call,
        other => panic!("expected dialogue text raster request, got {other:?}"),
    };
    assert_eq!(read_u32(&call.request.payload, 52), 160);
    assert_eq!(read_u32(&call.request.payload, 56) as i32, -120);
    assert_eq!(read_u32(&call.request.payload, 64), 19_200);
    let (outcome, runtime_completion) = fixture.complete_text_raster(call, vec![0; 19_200]);
    assert!(matches!(outcome, DispatchOutcome::Immediate(_)));
    assert!(runtime_completion.is_some());
    fixture.acknowledge_pending_delta();
    assert_eq!(fixture.memory.read(bits, 19_200).unwrap(), vec![0; 19_200]);
}

#[test]
fn large_scenario_font_dib_metrics_and_text_raster_form_one_reached_vertical_slice() {
    let mut fixture = FontFixture::new();
    let (_font, device_context, _default_font) =
        fixture.create_font_memory_dc(large_scenario_create_font_arguments());
    fixture
        .memory
        .write(OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS, &[0xa5; 320])
        .expect("initialize large scenario outline output");
    fixture.acknowledge_pending_delta();
    let outline = match fixture.dispatch_outline_text_metrics(
        device_context,
        X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as u32,
        OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected large scenario outline metrics, got {other:?}"),
    };
    assert_eq!(
        outline.eax,
        RegisterUpdate::Set(X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as u32)
    );
    fixture.acknowledge_pending_delta();
    let metrics = fixture
        .memory
        .read(
            OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS,
            X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH,
        )
        .expect("large scenario outline metrics bytes");
    for (offset, value) in [
        (4, 192),
        (8, 165),
        (12, 27),
        (24, 96),
        (28, 379),
        (96, 165),
        (100, -27),
        (192, -13),
    ] {
        assert_eq!(read_i32(&metrics, offset), value);
    }

    fixture.install_bitmap_info(&large_scenario_bitmap_info());
    fixture
        .memory
        .write(BITS_OUTPUT_ADDRESS, &0xa5a5_a5a5_u32.to_le_bytes())
        .expect("initialize large scenario DIB output");
    fixture.acknowledge_pending_delta();
    let dib = match fixture.dispatch_create_dib_section(
        device_context,
        BITMAP_INFO_ADDRESS,
        BITS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            RegisterUpdate::Set(handle) => handle,
            other => panic!("large scenario CreateDIBSection did not set EAX: {other:?}"),
        },
        other => panic!("expected large scenario DIB creation, got {other:?}"),
    };
    fixture.acknowledge_pending_delta();
    let bits = GuestAddress(read_u32(
        &fixture.memory.read(BITS_OUTPUT_ADDRESS, 4).unwrap(),
        0,
    ));
    assert!(matches!(
        fixture.dispatch_select_object(device_context, dib),
        DispatchOutcome::Immediate(_)
    ));
    fixture
        .memory
        .write(TEXT_OUT_BYTES_ADDRESS, &[0x81, 0x40])
        .expect("install large scenario CP932 glyph");
    fixture.acknowledge_pending_delta();
    let call = match fixture.dispatch_text_out(device_context, 0, 0, TEXT_OUT_BYTES_ADDRESS, 2) {
        DispatchOutcome::Deferred(call) => call,
        other => panic!("expected large scenario text raster request, got {other:?}"),
    };
    assert_eq!(read_u32(&call.request.payload, 40), 192);
    assert_eq!(read_u32(&call.request.payload, 44), 96);
    assert_eq!(read_u32(&call.request.payload, 52), 384);
    assert_eq!(read_u32(&call.request.payload, 56) as i32, -288);
    assert_eq!(read_u32(&call.request.payload, 60), 384);
    assert_eq!(read_u32(&call.request.payload, 64), 110_592);
    let (outcome, runtime_completion) = fixture.complete_text_raster(call, vec![0; 110_592]);
    assert!(matches!(outcome, DispatchOutcome::Immediate(_)));
    assert!(runtime_completion.is_some());
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture.memory.read(bits, 110_592).unwrap(),
        vec![0; 110_592]
    );
}

#[test]
fn save_catalog_font_and_dib_form_the_reached_surface() {
    let mut fixture = FontFixture::new();
    let (_font, device_context, _default_font) =
        fixture.create_font_memory_dc(save_catalog_create_font_arguments());
    fixture.install_bitmap_info(&save_catalog_bitmap_info());
    fixture
        .memory
        .write(BITS_OUTPUT_ADDRESS, &0xa5a5_a5a5_u32.to_le_bytes())
        .expect("initialize save-catalog DIB output");
    fixture.acknowledge_pending_delta();

    let creation = match fixture.dispatch_create_dib_section(
        device_context,
        BITMAP_INFO_ADDRESS,
        BITS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected save-catalog DIB creation, got {other:?}"),
    };
    let dib = match creation.eax {
        RegisterUpdate::Set(handle) => handle,
        other => panic!("save-catalog CreateDIBSection did not set EAX: {other:?}"),
    };
    assert_ne!(dib, 0);
    assert_eq!(creation.last_error, INITIAL_LAST_ERROR);
    fixture.acknowledge_pending_delta();

    let bits = GuestAddress(read_u32(
        &fixture
            .memory
            .read(BITS_OUTPUT_ADDRESS, 4)
            .expect("save-catalog DIB bits output"),
        0,
    ));
    let (descriptor, backing) = {
        let object = fixture
            .runtime
            .gdi_objects()
            .values()
            .find(|object| object.handle == dib)
            .expect("save-catalog logical DIB");
        let RuntimeGdiObjectKind::DibSection {
            descriptor,
            backing,
        } = &object.object
        else {
            panic!("created save-catalog object is not a DIB section")
        };
        (descriptor.clone(), *backing)
    };
    assert_eq!(descriptor.width, 576);
    assert_eq!(descriptor.height, -216);
    assert_eq!(descriptor.planes, 1);
    assert_eq!(descriptor.bits_per_pixel, 8);
    assert_eq!(descriptor.stride_bytes, 576);
    assert_eq!(descriptor.pixel_byte_length, 124_416);
    assert_eq!(descriptor.palette[0].red, 255);
    assert_eq!(descriptor.palette[255].red, 0);
    assert_eq!(backing.range.start(), bits);
    let backing_allocation = fixture
        .runtime
        .process_address_space_allocator()
        .allocation(backing.address_allocation_id)
        .expect("save-catalog DIB address allocation");
    assert_eq!(backing_allocation.usable_byte_length, 124_416);
    assert_eq!(
        fixture
            .memory
            .read(bits, 124_416)
            .expect("fresh save-catalog DIB pixels"),
        vec![0; 124_416]
    );

    assert!(matches!(
        fixture.dispatch_select_object(device_context, dib),
        DispatchOutcome::Immediate(_)
    ));
    let size_query = match fixture.dispatch_outline_text_metrics(device_context, 0, GuestAddress(0))
    {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected save-catalog outline size query, got {other:?}"),
    };
    assert_eq!(
        size_query.eax,
        RegisterUpdate::Set(X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as u32)
    );
    assert!(size_query.guest_writes.is_empty());

    fixture
        .memory
        .write(OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS, &[0xa5; 320])
        .expect("initialize save-catalog outline output");
    fixture.acknowledge_pending_delta();
    let outline = match fixture.dispatch_outline_text_metrics(
        device_context,
        X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as u32,
        OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected save-catalog outline metrics, got {other:?}"),
    };
    assert_eq!(
        outline.eax,
        RegisterUpdate::Set(X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as u32)
    );
    fixture.acknowledge_pending_delta();
    let metrics = fixture
        .memory
        .read(
            OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS,
            X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH,
        )
        .expect("save-catalog outline metrics bytes");
    for (offset, value) in [
        (4, 144),
        (8, 124),
        (12, 20),
        (24, 144),
        (28, 568),
        (96, 124),
        (100, -20),
        (116, -281),
        (124, 287),
        (192, -10),
    ] {
        assert_eq!(read_i32(&metrics, offset), value);
    }

    fixture
        .memory
        .write(TEXT_OUT_BYTES_ADDRESS, &[0xb6])
        .expect("install save-catalog CP932 glyph");
    fixture.acknowledge_pending_delta();
    let call = match fixture.dispatch_text_out(device_context, 0, 0, TEXT_OUT_BYTES_ADDRESS, 1) {
        DispatchOutcome::Deferred(call) => call,
        other => panic!("expected save-catalog text raster request, got {other:?}"),
    };
    assert_eq!(read_u32(&call.request.payload, 40), 144);
    assert_eq!(read_u32(&call.request.payload, 44), 144);
    assert_eq!(read_u32(&call.request.payload, 52), 576);
    assert_eq!(read_u32(&call.request.payload, 56) as i32, -216);
    assert_eq!(read_u32(&call.request.payload, 60), 576);
    assert_eq!(read_u32(&call.request.payload, 64), 124_416);
    assert_eq!(read_u32(&call.request.payload, 68), 1);
    assert_eq!(read_u32(&call.request.payload, 72), 0xb6);
    let mut rasterized = vec![0; 124_416];
    rasterized[72 * 576 + 72] = 255;
    let (outcome, runtime_completion) = fixture.complete_text_raster(call, rasterized.clone());
    assert!(matches!(outcome, DispatchOutcome::Immediate(_)));
    assert!(runtime_completion.is_some());
    fixture.acknowledge_pending_delta();
    assert_eq!(fixture.memory.read(bits, 124_416).unwrap(), rasterized);
}

#[test]
fn scenario_128x64_font_dib_metrics_and_raster_form_one_reached_surface() {
    let mut fixture = FontFixture::new();
    let (_font, device_context, _default_font) =
        fixture.create_font_memory_dc(scenario_128x64_create_font_arguments());
    fixture.install_bitmap_info(&scenario_256x192_bitmap_info());
    fixture
        .memory
        .write(BITS_OUTPUT_ADDRESS, &0xa5a5_a5a5_u32.to_le_bytes())
        .expect("initialize 256x192 DIB output");
    fixture.acknowledge_pending_delta();
    let dib = match fixture.dispatch_create_dib_section(
        device_context,
        BITMAP_INFO_ADDRESS,
        BITS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            RegisterUpdate::Set(handle) => handle,
            other => panic!("256x192 CreateDIBSection did not set EAX: {other:?}"),
        },
        other => panic!("expected 256x192 DIB creation, got {other:?}"),
    };
    fixture.acknowledge_pending_delta();
    let bits = GuestAddress(read_u32(
        &fixture.memory.read(BITS_OUTPUT_ADDRESS, 4).unwrap(),
        0,
    ));
    assert!(matches!(
        fixture.dispatch_select_object(device_context, dib),
        DispatchOutcome::Immediate(_)
    ));

    fixture
        .memory
        .write(OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS, &[0xa5; 320])
        .expect("initialize 128x64 outline output");
    fixture.acknowledge_pending_delta();
    let outline = match fixture.dispatch_outline_text_metrics(
        device_context,
        X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as u32,
        OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected 128x64 outline metrics, got {other:?}"),
    };
    assert_eq!(
        outline.eax,
        RegisterUpdate::Set(X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as u32)
    );
    fixture.acknowledge_pending_delta();
    let metrics = fixture
        .memory
        .read(
            OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS,
            X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH,
        )
        .expect("128x64 outline metrics bytes");
    for (offset, value) in [
        (4, 128),
        (8, 110),
        (12, 18),
        (24, 64),
        (28, 253),
        (116, -125),
        (124, 128),
        (192, -9),
    ] {
        assert_eq!(read_i32(&metrics, offset), value);
    }

    fixture
        .memory
        .write(TEXT_OUT_BYTES_ADDRESS, &[0xb6])
        .expect("install 128x64 CP932 glyph");
    fixture.acknowledge_pending_delta();
    let call = match fixture.dispatch_text_out(device_context, 0, 0, TEXT_OUT_BYTES_ADDRESS, 1) {
        DispatchOutcome::Deferred(call) => call,
        other => panic!("expected 128x64 text raster request, got {other:?}"),
    };
    assert_eq!(read_u32(&call.request.payload, 40), 128);
    assert_eq!(read_u32(&call.request.payload, 44), 64);
    assert_eq!(read_u32(&call.request.payload, 52), 256);
    assert_eq!(read_u32(&call.request.payload, 56) as i32, -192);
    assert_eq!(read_u32(&call.request.payload, 60), 256);
    assert_eq!(read_u32(&call.request.payload, 64), 49_152);
    let (outcome, runtime_completion) = fixture.complete_text_raster(call, vec![0; 49_152]);
    assert!(matches!(outcome, DispatchOutcome::Immediate(_)));
    assert!(runtime_completion.is_some());
    fixture.acknowledge_pending_delta();
    assert_eq!(fixture.memory.read(bits, 49_152).unwrap(), vec![0; 49_152]);
}

#[test]
fn scenario_144x72_font_dib_metrics_and_raster_form_one_reached_surface() {
    let mut fixture = FontFixture::new();
    let (_font, device_context, _default_font) =
        fixture.create_font_memory_dc(scenario_144x72_create_font_arguments());
    fixture.install_bitmap_info(&scenario_288x216_bitmap_info());
    fixture
        .memory
        .write(BITS_OUTPUT_ADDRESS, &0xa5a5_a5a5_u32.to_le_bytes())
        .expect("initialize 288x216 DIB output");
    fixture.acknowledge_pending_delta();
    let dib = match fixture.dispatch_create_dib_section(
        device_context,
        BITMAP_INFO_ADDRESS,
        BITS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            RegisterUpdate::Set(handle) => handle,
            other => panic!("288x216 CreateDIBSection did not set EAX: {other:?}"),
        },
        other => panic!("expected 288x216 DIB creation, got {other:?}"),
    };
    fixture.acknowledge_pending_delta();
    assert!(matches!(
        fixture.dispatch_select_object(device_context, dib),
        DispatchOutcome::Immediate(_)
    ));

    fixture
        .memory
        .write(OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS, &[0xa5; 320])
        .expect("initialize 144x72 outline output");
    fixture.acknowledge_pending_delta();
    assert!(matches!(
        fixture.dispatch_outline_text_metrics(
            device_context,
            X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as u32,
            OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS,
        ),
        DispatchOutcome::Immediate(_)
    ));
    fixture.acknowledge_pending_delta();
    let metrics = fixture
        .memory
        .read(
            OUTLINE_TEXT_METRICS_OUTPUT_ADDRESS,
            X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH,
        )
        .expect("144x72 outline metrics bytes");
    for (offset, value) in [
        (4, 144),
        (8, 124),
        (12, 20),
        (24, 72),
        (28, 284),
        (116, -141),
        (124, 143),
        (192, -10),
    ] {
        assert_eq!(read_i32(&metrics, offset), value);
    }

    fixture
        .memory
        .write(TEXT_OUT_BYTES_ADDRESS, &[0xb6])
        .expect("install 144x72 CP932 glyph");
    fixture.acknowledge_pending_delta();
    let call = match fixture.dispatch_text_out(device_context, 0, 0, TEXT_OUT_BYTES_ADDRESS, 1) {
        DispatchOutcome::Deferred(call) => call,
        other => panic!("expected 144x72 text raster request, got {other:?}"),
    };
    assert_eq!(read_u32(&call.request.payload, 40), 144);
    assert_eq!(read_u32(&call.request.payload, 44), 72);
    assert_eq!(read_u32(&call.request.payload, 52), 288);
    assert_eq!(read_u32(&call.request.payload, 56) as i32, -216);
    assert_eq!(read_u32(&call.request.payload, 60), 288);
    assert_eq!(read_u32(&call.request.payload, 64), 62_208);
    let (outcome, runtime_completion) = fixture.complete_text_raster(call, vec![0; 62_208]);
    assert!(matches!(outcome, DispatchOutcome::Immediate(_)));
    assert!(runtime_completion.is_some());
}

#[test]
fn outline_text_metrics_write_failure_preserves_guest_bytes_and_gdi_state() {
    let mut fixture = FontFixture::new();
    let (_font, device_context, _default_font) = fixture.create_primary_font_memory_dc();
    let boundary_output = GuestAddress(MEMORY_BASE.0 + 0x2ffc);
    fixture
        .memory
        .write(boundary_output, &[0xa5; 4])
        .expect("initialize outline boundary guard");
    fixture.acknowledge_pending_delta();
    let runtime_before = fixture.runtime.clone();

    let fault = match fixture.dispatch_outline_text_metrics(
        device_context,
        X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as u32,
        boundary_output,
    ) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected outline boundary write fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::InvalidCall);
    assert_eq!(fault.code, "guest-page-fault");
    assert_eq!(fixture.runtime, runtime_before);
    assert_eq!(
        fixture
            .memory
            .read(boundary_output, 4)
            .expect("read outline boundary guard"),
        [0xa5; 4]
    );
}

#[test]
fn outline_text_metrics_rejects_unprofiled_selection_or_disabled_policy_atomically() {
    let mut stock = FontFixture::new();
    let stock_dc = match stock.dispatch_create_compatible_dc(0) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            RegisterUpdate::Set(handle) => handle,
            other => panic!("CreateCompatibleDC did not set EAX: {other:?}"),
        },
        other => panic!("expected CreateCompatibleDC completion, got {other:?}"),
    };
    let runtime_before_stock = stock.runtime.clone();
    let fault = match stock.dispatch_outline_text_metrics(stock_dc, 0, GuestAddress(0)) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected stock-font outline fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "outline-text-metrics-selected-font-unsupported");
    assert_eq!(stock.runtime, runtime_before_stock);

    let mut disabled = FontFixture::new();
    let (_font, device_context, _default_font) = disabled.create_primary_font_memory_dc();
    disabled.gdi32_policy.get_outline_text_metrics_a = None;
    let runtime_before_disabled = disabled.runtime.clone();
    let fault = match disabled.dispatch_outline_text_metrics(device_context, 0, GuestAddress(0)) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected disabled outline fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "outline-text-metrics-policy-disabled");
    assert_eq!(disabled.runtime, runtime_before_disabled);
}

#[test]
fn text_out_catalog_exposes_nullable_counted_windows_932_input() {
    let fixture = FontFixture::new();
    let contract = gdi32_contract_for_endpoint(fixture.text_out_endpoint.clone())
        .expect("GDI32 catalog lookup")
        .expect("TextOutA contract");
    assert_eq!(contract.abi.arguments.len(), 5);
    assert!(matches!(
        &contract.abi.arguments[0].value,
        ApiValueSchema::LogicalHandle { namespace, nullable }
            if namespace == "vine.logical-gdi-dc.v1" && *nullable
    ));
    assert!(matches!(
        &contract.abi.arguments[3].value,
        ApiValueSchema::DynamicPointer { constraint }
            if constraint.nullable
                && constraint.alignment == 1
                && constraint.minimum_byte_length == 1
                && constraint.maximum_byte_length == 2
                && constraint.pointee == ApiPointeeSchema::Bytes
    ));
    assert_eq!(contract.abi.return_value, ApiReturnSchema::Bool32);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    assert_eq!(
        contract.behavior,
        ApiBehavior::ImmediateOrDeferredWithCommittedIndirectGuestWrites
    );
    assert_eq!(contract.callback_policy, CallbackPolicy::Forbidden);
}

#[test]
fn text_out_rasterizes_native_a_mask_into_selected_dib_and_zero_length_is_noop() {
    let mut fixture = FontFixture::new();
    let surface = prepare_primary_text_surface(&mut fixture);
    let generation_before = fixture.memory.stats().generation;
    let completion =
        match fixture.dispatch_text_out(surface.device_context, 0, 0, TEXT_OUT_BYTES_ADDRESS, 1) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected TextOutA completion, got {other:?}"),
        };
    assert_eq!(completion.eax, RegisterUpdate::Set(1));
    assert_eq!(completion.last_error, 0);
    assert_eq!(
        completion.stack_pointer,
        GuestAddress(TEXT_OUT_STACK_POINTER.0 + 24)
    );
    assert_eq!(completion.guest_writes.len(), 1);
    assert_eq!(
        completion.guest_writes[0].write.address,
        GuestAddress(surface.bits.0 + 20 * 128)
    );
    assert_eq!(completion.guest_writes[0].write.bytes.len(), 4_384);
    assert_eq!(completion.guest_writes[0].result.bytes_written, 4_384);
    assert_eq!(fixture.memory.stats().generation, generation_before + 2);
    assert_eq!(
        completion.guest_writes[0]
            .result
            .invalidation
            .expect("TextOutA raster invalidation")
            .generation,
        fixture.memory.stats().generation
    );
    fixture.acknowledge_pending_delta();

    let pixels = fixture
        .memory
        .read(surface.bits, 12_288)
        .expect("read TextOutA DIB pixels");
    assert_eq!(fnv1a32(&pixels), 0xe6fc_8461);
    let changed = pixels
        .iter()
        .enumerate()
        .filter_map(|(offset, &pixel)| (pixel != 0).then_some((offset, pixel)))
        .collect::<Vec<_>>();
    assert_eq!(changed.len(), 476);
    assert!(changed.iter().all(|(_, pixel)| *pixel == 255));
    assert_eq!(
        changed
            .iter()
            .map(|(offset, _)| offset % 128)
            .min()
            .unwrap(),
        3
    );
    assert_eq!(
        changed
            .iter()
            .map(|(offset, _)| offset % 128)
            .max()
            .unwrap(),
        28
    );
    assert_eq!(changed.first().unwrap().0 / 128, 20);
    assert_eq!(changed.last().unwrap().0 / 128, 54);
    let content_generation = fixture
        .runtime
        .gdi_objects()
        .values()
        .find_map(|object| {
            if object.handle != surface.dib {
                return None;
            }
            match &object.object {
                RuntimeGdiObjectKind::DibSection { backing, .. } => {
                    Some(backing.content_generation)
                }
                _ => None,
            }
        })
        .expect("rendered DIB generation");
    assert_eq!(content_generation, 1);

    fixture
        .runtime
        .set_current_last_error(INITIAL_LAST_ERROR)
        .expect("seed zero-length TextOutA LastError");
    let runtime_before_zero = fixture.runtime.clone();
    let pixels_before_zero = pixels;
    let zero = match fixture.dispatch_text_out(surface.device_context, 0, 0, GuestAddress(0), 0) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected zero-length TextOutA completion, got {other:?}"),
    };
    assert_eq!(zero.eax, RegisterUpdate::Set(1));
    assert_eq!(zero.last_error, INITIAL_LAST_ERROR);
    assert!(zero.guest_writes.is_empty());
    assert_eq!(fixture.runtime, runtime_before_zero);
    assert_eq!(
        fixture
            .memory
            .read(surface.bits, 12_288)
            .expect("read zero-length TextOutA pixels"),
        pixels_before_zero
    );
}

#[test]
fn text_out_defers_reached_cp932_glyph_and_commits_browser_pixels_atomically() {
    let mut fixture = FontFixture::new();
    let surface = prepare_scenario_text_surface(&mut fixture);
    let call =
        match fixture.dispatch_text_out(surface.device_context, 0, 0, TEXT_OUT_BYTES_ADDRESS, 2) {
            DispatchOutcome::Deferred(call) => call,
            other => panic!("expected scenario TextOutA host request, got {other:?}"),
        };
    assert_eq!(call.request.service, RUNTIME_GDI_TEXT_RASTER_SERVICE);
    assert_eq!(call.request.payload.len(), 21 * 4);
    assert_eq!(read_u32(&call.request.payload, 0), 1);
    assert_eq!(read_u32(&call.request.payload, 4), surface.device_context);
    assert_eq!(read_u32(&call.request.payload, 64), 150_528);
    assert_eq!(read_u32(&call.request.payload, 68), 2);
    assert_eq!(read_u32(&call.request.payload, 72), 0x0000_4081);

    let mut pixels = vec![0; 150_528];
    pixels[0] = 255;
    pixels[449] = 127;
    let (outcome, runtime_completion) = fixture.complete_text_raster(call, pixels.clone());
    let completion = match outcome {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected scenario TextOutA completion, got {other:?}"),
    };
    assert_eq!(completion.eax, RegisterUpdate::Set(1));
    assert_eq!(completion.last_error, INITIAL_LAST_ERROR);
    assert_eq!(completion.guest_writes.len(), 1);
    assert_eq!(completion.guest_writes[0].write.address, surface.bits);
    assert_eq!(completion.guest_writes[0].write.bytes, pixels);
    let runtime_completion = runtime_completion.expect("runtime host completion");
    assert!(matches!(
        fixture.runtime.pop_scheduler_event(),
        Some(RuntimeSchedulerEvent::HostCompleted(event)) if event == runtime_completion
    ));
    fixture.acknowledge_pending_delta();
    assert_eq!(
        fixture
            .memory
            .read(surface.bits, 150_528)
            .expect("committed scenario text pixels"),
        pixels
    );
    let content_generation = fixture
        .runtime
        .gdi_objects()
        .values()
        .find_map(|object| {
            if object.handle != surface.dib {
                return None;
            }
            match object.object {
                RuntimeGdiObjectKind::DibSection { backing, .. } => {
                    Some(backing.content_generation)
                }
                _ => None,
            }
        })
        .expect("scenario text content generation");
    assert_eq!(content_generation, 1);
}

#[test]
fn text_out_rejects_malformed_browser_pixels_without_partial_commit() {
    let mut fixture = FontFixture::new();
    let surface = prepare_scenario_text_surface(&mut fixture);
    let call =
        match fixture.dispatch_text_out(surface.device_context, 0, 0, TEXT_OUT_BYTES_ADDRESS, 2) {
            DispatchOutcome::Deferred(call) => call,
            other => panic!("expected scenario TextOutA host request, got {other:?}"),
        };
    let runtime_waiting = fixture.runtime.clone();
    let memory_waiting = fixture.memory.stats();
    let pixels_waiting = fixture
        .memory
        .read(surface.bits, 150_528)
        .expect("scenario pixels before rejected completion");
    let (outcome, runtime_completion) = fixture.complete_text_raster(call, vec![0; 1]);
    let fault = match outcome {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected malformed text raster response fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::InvalidCall);
    assert_eq!(fault.code, "text-raster-host-response-mismatch");
    assert!(runtime_completion.is_none());
    assert_eq!(fixture.runtime, runtime_waiting);
    assert_eq!(fixture.memory.stats(), memory_waiting);
    assert_eq!(
        fixture
            .memory
            .read(surface.bits, 150_528)
            .expect("scenario pixels after rejected completion"),
        pixels_waiting
    );
}

#[test]
fn text_out_rejects_stale_selection_content_and_unwritable_backing_atomically() {
    let mut stale = FontFixture::new();
    let stale_dc = match stale.dispatch_create_compatible_dc(0) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            RegisterUpdate::Set(handle) => handle,
            other => panic!("CreateCompatibleDC did not set EAX: {other:?}"),
        },
        other => panic!("expected CreateCompatibleDC completion, got {other:?}"),
    };
    assert!(matches!(
        stale.dispatch_delete_dc(stale_dc),
        DispatchOutcome::Immediate(_)
    ));
    let runtime_after_delete = stale.runtime.clone();
    let completion = match stale.dispatch_text_out(stale_dc, 0, 0, TEXT_OUT_BYTES_ADDRESS, 1) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected stale-HDC TextOutA completion, got {other:?}"),
    };
    assert_eq!(completion.eax, RegisterUpdate::Set(0));
    assert_eq!(completion.last_error, 0);
    assert!(completion.guest_writes.is_empty());
    assert_ne!(stale.runtime, runtime_after_delete);
    stale
        .runtime
        .set_current_last_error(INITIAL_LAST_ERROR)
        .expect("restore stale TextOutA LastError");
    assert_eq!(stale.runtime, runtime_after_delete);

    let mut stock = FontFixture::new();
    let stock_dc = match stock.dispatch_create_compatible_dc(0) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            RegisterUpdate::Set(handle) => handle,
            other => panic!("CreateCompatibleDC did not set EAX: {other:?}"),
        },
        other => panic!("expected CreateCompatibleDC completion, got {other:?}"),
    };
    let stock_runtime = stock.runtime.clone();
    let fault = match stock.dispatch_text_out(stock_dc, 0, 0, TEXT_OUT_BYTES_ADDRESS, 1) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected stock-selection TextOutA fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "text-raster-selected-font-unsupported");
    assert_eq!(stock.runtime, stock_runtime);

    let mut no_dib = FontFixture::new();
    let (_font, no_dib_dc, _default_font) = no_dib.create_primary_font_memory_dc();
    let no_dib_runtime = no_dib.runtime.clone();
    let fault = match no_dib.dispatch_text_out(no_dib_dc, 0, 0, TEXT_OUT_BYTES_ADDRESS, 1) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected missing-DIB TextOutA fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "text-raster-selected-bitmap-unsupported");
    assert_eq!(no_dib.runtime, no_dib_runtime);

    let mut wrong_text = FontFixture::new();
    let wrong_text_surface = prepare_primary_text_surface(&mut wrong_text);
    wrong_text
        .memory
        .write(TEXT_OUT_BYTES_ADDRESS, b"b")
        .expect("install unsupported TextOutA bytes");
    wrong_text.acknowledge_pending_delta();
    let wrong_text_runtime = wrong_text.runtime.clone();
    let pixels_before_wrong_text = wrong_text
        .memory
        .read(wrong_text_surface.bits, 12_288)
        .unwrap();
    let deferred = match wrong_text.dispatch_text_out(
        wrong_text_surface.device_context,
        0,
        0,
        TEXT_OUT_BYTES_ADDRESS,
        1,
    ) {
        DispatchOutcome::Deferred(call) => call,
        other => panic!("expected generic TextOutA host request, got {other:?}"),
    };
    assert_eq!(deferred.request.service, RUNTIME_GDI_TEXT_RASTER_SERVICE);
    assert_ne!(wrong_text.runtime, wrong_text_runtime);
    assert_eq!(
        wrong_text
            .memory
            .read(wrong_text_surface.bits, 12_288)
            .unwrap(),
        pixels_before_wrong_text
    );

    let mut nonfresh = FontFixture::new();
    let nonfresh_surface = prepare_primary_text_surface(&mut nonfresh);
    nonfresh
        .memory
        .write(nonfresh_surface.bits, &[1])
        .expect("dirty text DIB before raster");
    nonfresh.acknowledge_pending_delta();
    let nonfresh_runtime = nonfresh.runtime.clone();
    let nonfresh_pixels = nonfresh.memory.read(nonfresh_surface.bits, 12_288).unwrap();
    let fault = match nonfresh.dispatch_text_out(
        nonfresh_surface.device_context,
        0,
        0,
        TEXT_OUT_BYTES_ADDRESS,
        1,
    ) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected nonfresh TextOutA fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "text-out-a-initial-pixels-unsupported");
    assert_eq!(nonfresh.runtime, nonfresh_runtime);
    assert_eq!(
        nonfresh.memory.read(nonfresh_surface.bits, 12_288).unwrap(),
        nonfresh_pixels
    );

    let mut read_only = FontFixture::new();
    let read_only_surface = prepare_primary_text_surface(&mut read_only);
    read_only
        .memory
        .protect(
            read_only_surface.backing_owner,
            read_only_surface.backing_range,
            PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .expect("protect text DIB read-only");
    read_only.acknowledge_pending_delta();
    let read_only_runtime = read_only.runtime.clone();
    let read_only_pixels = read_only
        .memory
        .read(read_only_surface.bits, 12_288)
        .unwrap();
    let fault = match read_only.dispatch_text_out(
        read_only_surface.device_context,
        0,
        0,
        TEXT_OUT_BYTES_ADDRESS,
        1,
    ) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected read-only TextOutA fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::InvalidCall);
    assert_eq!(fault.code, "guest-page-fault");
    assert_eq!(read_only.runtime, read_only_runtime);
    assert_eq!(
        read_only
            .memory
            .read(read_only_surface.bits, 12_288)
            .unwrap(),
        read_only_pixels
    );

    let mut disabled = FontFixture::new();
    let disabled_surface = prepare_primary_text_surface(&mut disabled);
    disabled.gdi32_policy.text_out_a = None;
    let disabled_runtime = disabled.runtime.clone();
    let fault = match disabled.dispatch_text_out(
        disabled_surface.device_context,
        0,
        0,
        TEXT_OUT_BYTES_ADDRESS,
        1,
    ) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected disabled TextOutA fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "text-out-a-policy-disabled");
    assert_eq!(disabled.runtime, disabled_runtime);
}

#[test]
fn dib_backing_and_gdi_objects_follow_primary_restore_before_delete_lifecycle() {
    let mut fixture = FontFixture::new();
    fixture.install_face_name(&primary_face_name());
    fixture.configure_create_font(primary_create_font_arguments(), FACE_NAME_ADDRESS);
    let font_handle = match fixture.dispatch_create_font() {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            RegisterUpdate::Set(handle) => handle,
            other => panic!("CreateFontA did not set EAX: {other:?}"),
        },
        other => panic!("expected CreateFontA completion, got {other:?}"),
    };
    let memory_dc = match fixture.dispatch_create_compatible_dc(0) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            RegisterUpdate::Set(handle) => handle,
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
        .memory
        .write(BITS_OUTPUT_ADDRESS, &0xa5a5_a5a5_u32.to_le_bytes())
        .expect("initialize bits output");
    fixture.acknowledge_pending_delta();
    let memory_before_create = fixture.memory.stats();
    let address_records_before_create = fixture.runtime.process_address_space_allocator().records();
    let creation = match fixture.dispatch_create_dib_section(
        memory_dc,
        BITMAP_INFO_ADDRESS,
        BITS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected CreateDIBSection completion, got {other:?}"),
    };
    let RegisterUpdate::Set(dib_handle) = creation.eax else {
        panic!("CreateDIBSection did not set EAX")
    };
    assert_ne!(dib_handle, 0);
    assert_eq!(creation.last_error, INITIAL_LAST_ERROR);
    assert_eq!(creation.guest_writes.len(), 1);
    assert_eq!(
        creation.stack_pointer,
        GuestAddress(CREATE_DIB_STACK_POINTER.0 + 28)
    );
    fixture.acknowledge_pending_delta();
    let bits_address = GuestAddress(read_u32(
        &fixture
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
    assert!(fixture.memory.region_snapshots().iter().any(|region| {
        region.address_allocation_id == Some(backing.address_allocation_id.0)
            && region.owner == backing.memory_owner
            && region.kind == MemoryRegionKind::GdiResource
            && region.range == backing.range
    }));
    assert_eq!(
        fixture.memory.stats().owners,
        memory_before_create.owners + 1
    );
    assert_eq!(
        fixture.memory.stats().regions,
        memory_before_create.regions + 1
    );
    assert_eq!(
        fixture
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
        address_records_before_create.len() + 1
    );

    fixture
        .memory
        .write(OBJECT_OUTPUT_ADDRESS, &[0x5a; 32])
        .expect("initialize BITMAP output guards");
    fixture.acknowledge_pending_delta();
    let get_object = match fixture.dispatch_get_object(
        dib_handle,
        X86_BITMAP_BYTE_LENGTH,
        OBJECT_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected GetObjectA completion, got {other:?}"),
    };
    assert_eq!(get_object.eax, RegisterUpdate::Set(X86_BITMAP_BYTE_LENGTH));
    assert_eq!(get_object.last_error, INITIAL_LAST_ERROR);
    fixture.acknowledge_pending_delta();
    let bitmap = fixture
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
    assert_eq!(
        select_bitmap.eax,
        RegisterUpdate::Set(default_bitmap_handle)
    );
    assert_eq!(select_bitmap.last_error, INITIAL_LAST_ERROR);
    let select_font = match fixture.dispatch_select_object(memory_dc, font_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected font SelectObject completion, got {other:?}"),
    };
    assert_eq!(select_font.eax, RegisterUpdate::Set(default_font_handle));
    assert_eq!(select_font.last_error, INITIAL_LAST_ERROR);

    let runtime_before_selected_delete = fixture.runtime.clone();
    let address_records_before_selected_delete =
        fixture.runtime.process_address_space_allocator().records();
    let pixels_before_selected_delete = fixture
        .memory
        .read(bits_address, 12_288)
        .expect("selected DIB pixels");
    let selected_delete = match fixture.dispatch_delete_object(dib_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected selected DeleteObject completion, got {other:?}"),
    };
    assert_eq!(selected_delete.eax, RegisterUpdate::Set(0));
    assert_eq!(selected_delete.last_error, INITIAL_LAST_ERROR);
    assert_eq!(fixture.runtime, runtime_before_selected_delete);
    assert_eq!(
        fixture.runtime.process_address_space_allocator().records(),
        address_records_before_selected_delete
    );
    assert_eq!(
        fixture
            .memory
            .read(bits_address, 12_288)
            .expect("preserved selected DIB pixels"),
        pixels_before_selected_delete
    );

    let restore_bitmap = match fixture.dispatch_select_object(memory_dc, default_bitmap_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected bitmap restore completion, got {other:?}"),
    };
    assert_eq!(restore_bitmap.eax, RegisterUpdate::Set(dib_handle));
    let restore_font = match fixture.dispatch_select_object(memory_dc, default_font_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected font restore completion, got {other:?}"),
    };
    assert_eq!(restore_font.eax, RegisterUpdate::Set(font_handle));

    let delete_dib = match fixture.dispatch_delete_object(dib_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected DIB DeleteObject completion, got {other:?}"),
    };
    assert_eq!(delete_dib.eax, RegisterUpdate::Set(1));
    assert_eq!(delete_dib.last_error, INITIAL_LAST_ERROR);
    fixture.acknowledge_pending_delta();
    assert!(
        fixture
            .runtime
            .gdi_objects()
            .values()
            .all(|object| object.handle != dib_handle)
    );
    assert!(fixture.memory.read(bits_address, 1).is_err());
    assert!(
        fixture
            .runtime
            .process_address_space_allocator()
            .allocation(backing.address_allocation_id)
            .is_err()
    );
    assert!(
        fixture
            .memory
            .region_snapshots()
            .iter()
            .all(|region| region.kind != MemoryRegionKind::GdiResource)
    );
    assert_eq!(fixture.memory.stats().owners, memory_before_create.owners);
    assert_eq!(fixture.memory.stats().regions, memory_before_create.regions);

    fixture
        .memory
        .write(OBJECT_OUTPUT_ADDRESS, &[0x6b; 32])
        .expect("initialize stale GetObjectA guard");
    fixture.acknowledge_pending_delta();
    let runtime_before_stale_get = fixture.runtime.clone();
    let stale_get = match fixture.dispatch_get_object(
        dib_handle,
        X86_BITMAP_BYTE_LENGTH,
        OBJECT_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected stale GetObjectA completion, got {other:?}"),
    };
    assert_eq!(stale_get.eax, RegisterUpdate::Set(0));
    assert_eq!(stale_get.last_error, INITIAL_LAST_ERROR);
    assert_eq!(fixture.runtime, runtime_before_stale_get);
    assert_eq!(
        fixture
            .memory
            .read(OBJECT_OUTPUT_ADDRESS, 32)
            .expect("preserved stale GetObjectA guard"),
        vec![0x6b; 32]
    );

    let delete_font = match fixture.dispatch_delete_object(font_handle) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected font DeleteObject completion, got {other:?}"),
    };
    assert_eq!(delete_font.eax, RegisterUpdate::Set(1));
    let delete_dc = match fixture.dispatch_delete_dc(memory_dc) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected memory DeleteDC completion, got {other:?}"),
    };
    assert_eq!(delete_dc.eax, RegisterUpdate::Set(1));
}

#[test]
fn post_create_font_dib_profile_creates_exact_backing_and_bitmap_shape() {
    let mut fixture = FontFixture::new();
    fixture.install_face_name(&primary_face_name());
    fixture.configure_create_font(post_heap_growth_create_font_arguments(), FACE_NAME_ADDRESS);
    let font = match fixture.dispatch_create_font() {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            RegisterUpdate::Set(handle) => handle,
            other => panic!("CreateFontA did not set EAX: {other:?}"),
        },
        other => panic!("expected CreateFontA completion, got {other:?}"),
    };
    let memory_dc = match fixture.dispatch_create_compatible_dc(0) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            RegisterUpdate::Set(handle) => handle,
            other => panic!("CreateCompatibleDC did not set EAX: {other:?}"),
        },
        other => panic!("expected CreateCompatibleDC completion, got {other:?}"),
    };
    let (default_font, default_bitmap) = {
        let dc = fixture
            .runtime
            .gdi_device_context(memory_dc)
            .expect("post-font memory DC");
        let objects = fixture.runtime.gdi_objects();
        (
            objects[&dc.selected_font_id.expect("default font")].handle,
            objects[&dc.selected_bitmap_id.expect("default bitmap")].handle,
        )
    };

    fixture.install_bitmap_info(&post_create_font_bitmap_info());
    fixture
        .memory
        .write(BITS_OUTPUT_ADDRESS, &0xa5a5_a5a5_u32.to_le_bytes())
        .expect("initialize post-font bits output");
    fixture.acknowledge_pending_delta();
    let memory_before = fixture.memory.stats();
    let address_records_before = fixture.runtime.process_address_space_allocator().records();
    let creation = match fixture.dispatch_create_dib_section(
        memory_dc,
        BITMAP_INFO_ADDRESS,
        BITS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected post-font CreateDIBSection completion, got {other:?}"),
    };
    let dib = match creation.eax {
        RegisterUpdate::Set(handle) => handle,
        other => panic!("CreateDIBSection did not set EAX: {other:?}"),
    };
    assert_ne!(dib, 0);
    assert_eq!(creation.last_error, INITIAL_LAST_ERROR);
    assert_eq!(creation.guest_writes.len(), 1);
    fixture.acknowledge_pending_delta();
    let bits = GuestAddress(read_u32(
        &fixture
            .memory
            .read(BITS_OUTPUT_ADDRESS, 4)
            .expect("post-font DIB bits output"),
        0,
    ));
    let (descriptor, backing) = {
        let object = fixture
            .runtime
            .gdi_objects()
            .values()
            .find(|object| object.handle == dib)
            .expect("post-font logical DIB");
        let RuntimeGdiObjectKind::DibSection {
            descriptor,
            backing,
        } = &object.object
        else {
            panic!("created object is not a DIB section")
        };
        (descriptor.clone(), *backing)
    };
    assert_eq!(descriptor.width, 448);
    assert_eq!(descriptor.height, -336);
    assert_eq!(descriptor.planes, 1);
    assert_eq!(descriptor.bits_per_pixel, 8);
    assert_eq!(descriptor.stride_bytes, 448);
    assert_eq!(descriptor.pixel_byte_length, 150_528);
    assert_eq!(descriptor.palette.len(), 256);
    assert_eq!(descriptor.palette[0].red, 255);
    assert_eq!(descriptor.palette[255].red, 0);
    assert_eq!(backing.range.start(), bits);
    assert_eq!(backing.range.byte_length(), 151_552);
    let backing_allocation = fixture
        .runtime
        .process_address_space_allocator()
        .allocation(backing.address_allocation_id)
        .expect("post-font DIB address allocation");
    assert_eq!(backing_allocation.range.byte_length, 151_552);
    assert_eq!(backing_allocation.usable_byte_length, 150_528);
    assert_eq!(
        backing_allocation.purpose,
        ProcessAddressAllocationPurpose::GdiResource
    );
    assert_eq!(fixture.memory.stats().owners, memory_before.owners + 1);
    assert_eq!(fixture.memory.stats().regions, memory_before.regions + 1);
    assert_eq!(
        fixture
            .runtime
            .process_address_space_allocator()
            .records()
            .len(),
        address_records_before.len() + 1
    );
    assert_eq!(
        fixture
            .memory
            .read(bits, 150_528)
            .expect("post-font DIB pixels"),
        vec![0; 150_528]
    );

    fixture
        .memory
        .write(OBJECT_OUTPUT_ADDRESS, &[0x5a; 32])
        .expect("initialize post-font BITMAP output");
    fixture.acknowledge_pending_delta();
    let get_object =
        match fixture.dispatch_get_object(dib, X86_BITMAP_BYTE_LENGTH, OBJECT_OUTPUT_ADDRESS) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected post-font GetObjectA completion, got {other:?}"),
        };
    assert_eq!(get_object.eax, RegisterUpdate::Set(X86_BITMAP_BYTE_LENGTH));
    fixture.acknowledge_pending_delta();
    let bitmap = fixture
        .memory
        .read(OBJECT_OUTPUT_ADDRESS, 24)
        .expect("post-font BITMAP output");
    assert_eq!(read_u32(&bitmap, 0), 0);
    assert_eq!(read_i32(&bitmap, 4), 448);
    assert_eq!(read_i32(&bitmap, 8), 336);
    assert_eq!(read_u32(&bitmap, 12), 448);
    assert_eq!(u16::from_le_bytes(bitmap[16..18].try_into().unwrap()), 1);
    assert_eq!(u16::from_le_bytes(bitmap[18..20].try_into().unwrap()), 8);
    assert_eq!(read_u32(&bitmap, 20), bits.0);

    let selected_bitmap = match fixture.dispatch_select_object(memory_dc, dib) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected post-font bitmap SelectObject, got {other:?}"),
    };
    assert_eq!(selected_bitmap.eax, RegisterUpdate::Set(default_bitmap));
    let selected_font = match fixture.dispatch_select_object(memory_dc, font) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected post-font font SelectObject, got {other:?}"),
    };
    assert_eq!(selected_font.eax, RegisterUpdate::Set(default_font));
    assert!(matches!(
        fixture.dispatch_select_object(memory_dc, default_bitmap),
        DispatchOutcome::Immediate(_)
    ));
    assert!(matches!(
        fixture.dispatch_select_object(memory_dc, default_font),
        DispatchOutcome::Immediate(_)
    ));
    assert!(matches!(
        fixture.dispatch_delete_object(dib),
        DispatchOutcome::Immediate(_)
    ));
    fixture.acknowledge_pending_delta();
    assert!(fixture.memory.read(bits, 1).is_err());
}

#[test]
fn create_dib_section_rejects_malformed_unreached_and_stale_inputs_atomically() {
    let mut malformed = FontFixture::new();
    let memory_dc = match malformed.dispatch_create_compatible_dc(0) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            RegisterUpdate::Set(handle) => handle,
            other => panic!("CreateCompatibleDC did not set EAX: {other:?}"),
        },
        other => panic!("expected CreateCompatibleDC completion, got {other:?}"),
    };
    let mut malformed_info = primary_bitmap_info();
    malformed_info[0..4].copy_from_slice(&0_u32.to_le_bytes());
    malformed.install_bitmap_info(&malformed_info);
    malformed
        .memory
        .write(BITS_OUTPUT_ADDRESS, &0xa5a5_a5a5_u32.to_le_bytes())
        .expect("initialize malformed bits output");
    malformed.acknowledge_pending_delta();
    let runtime_before_malformed = malformed.runtime.clone();
    let records_before_malformed = malformed
        .runtime
        .process_address_space_allocator()
        .records();
    let memory_before_malformed = malformed.memory.stats();
    let completion = match malformed.dispatch_create_dib_section(
        memory_dc,
        BITMAP_INFO_ADDRESS,
        BITS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected malformed CreateDIBSection completion, got {other:?}"),
    };
    assert_eq!(completion.eax, RegisterUpdate::Set(0));
    assert_eq!(completion.last_error, INITIAL_LAST_ERROR);
    malformed.acknowledge_pending_delta();
    assert_eq!(
        malformed
            .memory
            .read(BITS_OUTPUT_ADDRESS, 4)
            .expect("cleared malformed bits output"),
        0_u32.to_le_bytes()
    );
    assert_eq!(malformed.runtime, runtime_before_malformed);
    assert_eq!(
        malformed
            .runtime
            .process_address_space_allocator()
            .records(),
        records_before_malformed
    );
    assert_eq!(
        malformed.memory.stats().owners,
        memory_before_malformed.owners
    );
    assert_eq!(
        malformed.memory.stats().regions,
        memory_before_malformed.regions
    );

    let mut wrong_palette = FontFixture::new();
    let memory_dc = match wrong_palette.dispatch_create_compatible_dc(0) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            RegisterUpdate::Set(handle) => handle,
            other => panic!("CreateCompatibleDC did not set EAX: {other:?}"),
        },
        other => panic!("expected CreateCompatibleDC completion, got {other:?}"),
    };
    let mut bitmap_info = primary_bitmap_info();
    bitmap_info[X86_BITMAPINFOHEADER_BYTE_LENGTH as usize] ^= 1;
    wrong_palette.install_bitmap_info(&bitmap_info);
    wrong_palette
        .memory
        .write(BITS_OUTPUT_ADDRESS, &0xfeed_beef_u32.to_le_bytes())
        .expect("initialize wrong-palette bits output");
    wrong_palette.acknowledge_pending_delta();
    let runtime_before_palette = wrong_palette.runtime.clone();
    let records_before_palette = wrong_palette
        .runtime
        .process_address_space_allocator()
        .records();
    let fault = match wrong_palette.dispatch_create_dib_section(
        memory_dc,
        BITMAP_INFO_ADDRESS,
        BITS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected wrong-palette CreateDIBSection fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "create-dib-section-variant-unsupported");
    assert_eq!(wrong_palette.runtime, runtime_before_palette);
    assert_eq!(
        wrong_palette
            .runtime
            .process_address_space_allocator()
            .records(),
        records_before_palette
    );
    assert_eq!(
        wrong_palette
            .memory
            .read(BITS_OUTPUT_ADDRESS, 4)
            .expect("preserved wrong-palette bits output"),
        0xfeed_beef_u32.to_le_bytes()
    );

    let mut wrong_dimensions = FontFixture::new();
    let memory_dc = match wrong_dimensions.dispatch_create_compatible_dc(0) {
        DispatchOutcome::Immediate(completion) => match completion.eax {
            RegisterUpdate::Set(handle) => handle,
            other => panic!("CreateCompatibleDC did not set EAX: {other:?}"),
        },
        other => panic!("expected CreateCompatibleDC completion, got {other:?}"),
    };
    wrong_dimensions.install_bitmap_info(&bitmap_info_bytes(448, -335));
    wrong_dimensions
        .memory
        .write(BITS_OUTPUT_ADDRESS, &0x2468_1357_u32.to_le_bytes())
        .expect("initialize wrong-dimensions bits output");
    wrong_dimensions.acknowledge_pending_delta();
    let runtime_before_dimensions = wrong_dimensions.runtime.clone();
    let memory_before_dimensions = wrong_dimensions.memory.stats();
    let records_before_dimensions = wrong_dimensions
        .runtime
        .process_address_space_allocator()
        .records();
    let fault = match wrong_dimensions.dispatch_create_dib_section(
        memory_dc,
        BITMAP_INFO_ADDRESS,
        BITS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected wrong-dimensions CreateDIBSection fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::Unsupported);
    assert_eq!(fault.code, "create-dib-section-variant-unsupported");
    assert_eq!(wrong_dimensions.runtime, runtime_before_dimensions);
    let memory_after_dimensions = wrong_dimensions.memory.stats();
    assert_eq!(
        memory_after_dimensions.owners,
        memory_before_dimensions.owners
    );
    assert_eq!(
        memory_after_dimensions.regions,
        memory_before_dimensions.regions
    );
    assert_eq!(
        memory_after_dimensions.physical_pages,
        memory_before_dimensions.physical_pages
    );
    assert_eq!(
        wrong_dimensions
            .runtime
            .process_address_space_allocator()
            .records(),
        records_before_dimensions
    );
    assert_eq!(
        wrong_dimensions
            .memory
            .read(BITS_OUTPUT_ADDRESS, 4)
            .expect("preserved wrong-dimensions bits output"),
        0x2468_1357_u32.to_le_bytes()
    );

    let mut stale_dc = FontFixture::new();
    stale_dc.install_bitmap_info(&primary_bitmap_info());
    stale_dc
        .memory
        .write(BITS_OUTPUT_ADDRESS, &0xcafe_babe_u32.to_le_bytes())
        .expect("initialize stale-DC bits output");
    stale_dc.acknowledge_pending_delta();
    let runtime_before_stale = stale_dc.runtime.clone();
    let fault = match stale_dc.dispatch_create_dib_section(
        0x4701_ffff,
        BITMAP_INFO_ADDRESS,
        BITS_OUTPUT_ADDRESS,
    ) {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected stale-DC CreateDIBSection fault, got {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::InvalidCall);
    assert_eq!(fault.code, "device-context-handle-unknown");
    assert_eq!(stale_dc.runtime, runtime_before_stale);
    assert_eq!(
        stale_dc
            .memory
            .read(BITS_OUTPUT_ADDRESS, 4)
            .expect("preserved stale-DC bits output"),
        0xcafe_babe_u32.to_le_bytes()
    );
}

#[test]
fn create_font_a_catalog_exposes_bounded_windows_932_contract() {
    let fixture = FontFixture::new();
    let contract = gdi32_contract_for_endpoint(fixture.create_font_endpoint.clone())
        .expect("GDI32 catalog lookup")
        .expect("CreateFontA contract");
    assert_eq!(contract.abi.arguments.len(), 14);
    assert!(
        contract.abi.arguments[..13]
            .iter()
            .all(|argument| argument.value == ApiValueSchema::U32)
    );
    assert!(matches!(
        &contract.abi.arguments[13].value,
        ApiValueSchema::Pointer { constraint }
            if !constraint.nullable
                && constraint.alignment == 1
                && constraint.byte_length == 1
                && matches!(
                    constraint.pointee,
                    ApiPointeeSchema::String {
                        encoding: ApiStringEncoding::Windows932,
                        maximum_code_units: 32,
                        nul_terminated: true,
                    }
                )
    ));
    assert!(matches!(
        &contract.abi.return_value,
        ApiReturnSchema::LogicalHandle { namespace, nullable }
            if namespace == "vine.logical-gdi-object.v1" && *nullable
    ));
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    assert_eq!(contract.callback_policy, CallbackPolicy::Forbidden);
}

#[test]
fn create_font_a_creates_distinct_rust_owned_objects_without_guest_writes() {
    let mut fixture = FontFixture::new();
    fixture.install_face_name(&primary_face_name());
    fixture.configure_create_font(primary_create_font_arguments(), FACE_NAME_ADDRESS);
    let memory_before = fixture.memory.stats();

    let completion = match fixture.dispatch_create_font() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected CreateFontA completion, got {other:?}"),
    };
    let RegisterUpdate::Set(first_handle) = completion.eax else {
        panic!("CreateFontA did not set EAX")
    };
    assert_eq!(first_handle, 0x4702_0001);
    assert_eq!(completion.instruction_pointer, CREATE_FONT_RETURN_ADDRESS);
    assert_eq!(
        completion.stack_pointer,
        GuestAddress(CREATE_FONT_STACK_POINTER.0 + 60)
    );
    assert_eq!(completion.last_error, INITIAL_LAST_ERROR);
    assert!(completion.guest_writes.is_empty());
    assert_eq!(fixture.memory.stats(), memory_before);

    let first = fixture
        .runtime
        .gdi_objects()
        .values()
        .find(|object| object.handle == first_handle)
        .expect("created logical font");
    assert!(!first.shared);
    let RuntimeGdiObjectKind::FontA { log_font } = &first.object else {
        panic!("created object is not a font")
    };
    assert_eq!(log_font.height, 64);
    assert_eq!(log_font.width, 32);
    assert_eq!(log_font.weight, 100);
    assert_eq!(log_font.character_set, 128);
    assert_eq!(log_font.output_precision, 4);
    assert_eq!(log_font.clip_precision, 0);
    assert_eq!(log_font.quality, 2);
    assert_eq!(log_font.pitch_and_family, 0);
    assert_eq!(log_font.face_name_windows_932, MS_GOTHIC_WINDOWS_932);
    assert_eq!(
        &log_font.to_bytes().expect("canonical LOGFONTA")[..46],
        &[
            64, 0, 0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100, 0, 0, 0, 0, 0, 0, 128, 4, 0, 2,
            0, 130, 108, 130, 114, 32, 131, 83, 131, 86, 131, 98, 131, 78, 0, 0, 0, 0, 0,
        ]
    );

    fixture.configure_create_font(primary_create_font_arguments(), FACE_NAME_ADDRESS);
    let completion = match fixture.dispatch_create_font() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected repeated CreateFontA completion, got {other:?}"),
    };
    assert_eq!(completion.eax, RegisterUpdate::Set(first_handle + 1));
    assert_eq!(fixture.runtime.gdi_objects().len(), 2);

    fixture.configure_create_font(post_heap_growth_create_font_arguments(), FACE_NAME_ADDRESS);
    let completion = match fixture.dispatch_create_font() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected post-heap-growth CreateFontA completion, got {other:?}"),
    };
    assert_eq!(completion.eax, RegisterUpdate::Set(first_handle + 2));
    let post_heap_growth = fixture
        .runtime
        .gdi_objects()
        .values()
        .find(|object| object.handle == first_handle + 2)
        .expect("created post-heap-growth logical font");
    let RuntimeGdiObjectKind::FontA { log_font } = &post_heap_growth.object else {
        panic!("post-heap-growth object is not a font")
    };
    assert_eq!(log_font.height, 224);
    assert_eq!(log_font.width, 112);
    assert_eq!(log_font.weight, 100);
    assert_eq!(log_font.character_set, 128);
    assert_eq!(log_font.output_precision, 4);
    assert_eq!(log_font.clip_precision, 0);
    assert_eq!(log_font.quality, 2);
    assert_eq!(log_font.pitch_and_family, 0);
    assert_eq!(log_font.face_name_windows_932, MS_GOTHIC_WINDOWS_932);

    fixture.configure_create_font(save_catalog_create_font_arguments(), FACE_NAME_ADDRESS);
    let completion = match fixture.dispatch_create_font() {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected save-catalog CreateFontA completion, got {other:?}"),
    };
    assert_eq!(completion.eax, RegisterUpdate::Set(first_handle + 3));
    let save_catalog = fixture
        .runtime
        .gdi_objects()
        .values()
        .find(|object| object.handle == first_handle + 3)
        .expect("created save-catalog logical font");
    let RuntimeGdiObjectKind::FontA { log_font } = &save_catalog.object else {
        panic!("save-catalog object is not a font")
    };
    assert_eq!(log_font.height, 144);
    assert_eq!(log_font.width, 144);
    assert_eq!(log_font.face_name_windows_932, MS_GOTHIC_WINDOWS_932);
    assert_eq!(fixture.runtime.gdi_objects().len(), 4);
}

#[test]
fn create_font_a_rejects_unreached_or_malformed_variants_atomically() {
    let mut wrong_weight = FontFixture::new();
    wrong_weight.install_face_name(&primary_face_name());
    let mut arguments = primary_create_font_arguments();
    arguments[4] = 400;
    wrong_weight.configure_create_font(arguments, FACE_NAME_ADDRESS);
    assert_create_font_fault_atomic(
        &mut wrong_weight,
        ApiFaultClass::Unsupported,
        "create-font-a-variant-unsupported",
    );

    let mut oversized_byte = FontFixture::new();
    oversized_byte.install_face_name(&primary_face_name());
    let mut arguments = primary_create_font_arguments();
    arguments[8] = 256;
    oversized_byte.configure_create_font(arguments, FACE_NAME_ADDRESS);
    assert_create_font_fault_atomic(
        &mut oversized_byte,
        ApiFaultClass::Unsupported,
        "create-font-a-variant-unsupported",
    );

    let mut wrong_face = FontFixture::new();
    let mut face = primary_face_name();
    face[0] ^= 1;
    wrong_face.install_face_name(&face);
    wrong_face.configure_create_font(primary_create_font_arguments(), FACE_NAME_ADDRESS);
    assert_create_font_fault_atomic(
        &mut wrong_face,
        ApiFaultClass::Unsupported,
        "create-font-a-variant-unsupported",
    );

    let mut unterminated = FontFixture::new();
    unterminated.install_face_name(&[0xa5; 32]);
    unterminated.configure_create_font(primary_create_font_arguments(), FACE_NAME_ADDRESS);
    assert_create_font_fault_atomic(
        &mut unterminated,
        ApiFaultClass::Unsupported,
        "create-font-a-face-name-unterminated",
    );

    let mut null_face = FontFixture::new();
    null_face.configure_create_font(primary_create_font_arguments(), GuestAddress(0));
    assert_create_font_fault_atomic(
        &mut null_face,
        ApiFaultClass::InvalidCall,
        "null-pointer-forbidden",
    );

    let mut cross_page = FontFixture::new();
    let final_byte = GuestAddress(MEMORY_BASE.0 + 0x2fff);
    cross_page
        .memory
        .write(final_byte, b"x")
        .expect("write final mapped face byte");
    cross_page.acknowledge_pending_delta();
    cross_page.configure_create_font(primary_create_font_arguments(), final_byte);
    assert_create_font_fault_atomic(
        &mut cross_page,
        ApiFaultClass::InvalidCall,
        "guest-page-fault",
    );

    let mut disabled = FontFixture::new();
    disabled.install_face_name(&primary_face_name());
    disabled.configure_create_font(primary_create_font_arguments(), FACE_NAME_ADDRESS);
    disabled.gdi32_policy.create_font_a = None;
    assert_create_font_fault_atomic(
        &mut disabled,
        ApiFaultClass::Unsupported,
        "create-font-a-policy-disabled",
    );
}

#[test]
fn enum_font_families_ex_a_catalog_exposes_bounded_callback_contract() {
    let fixture = FontFixture::new();
    let contract = gdi32_contract_for_endpoint(fixture.enum_endpoint.clone())
        .expect("GDI32 catalog lookup")
        .expect("EnumFontFamiliesExA contract");
    assert_eq!(contract.abi.arguments.len(), 5);
    assert!(matches!(
        &contract.abi.arguments[0].value,
        ApiValueSchema::LogicalHandle { namespace, nullable }
            if namespace == "vine.logical-gdi-dc.v1" && !nullable
    ));
    assert!(matches!(
        &contract.abi.arguments[1].value,
        ApiValueSchema::Pointer { constraint }
            if constraint.byte_length == X86_LOGFONTA_BYTE_LENGTH
    ));
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::Preserve);
    assert_eq!(contract.callback_policy, CallbackPolicy::Required);
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
        let runtime_before = fixture.runtime.clone();
        let memory_generation_before = fixture.memory.stats().generation;

        let call = match fixture.dispatch_enum() {
            DispatchOutcome::GuestCallback(call) => call,
            other => panic!("expected font callback, got {other:?}"),
        };
        assert_eq!(fixture.runtime, runtime_before);
        assert_eq!(fixture.memory.stats().generation, memory_generation_before);
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

        let prepared = ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime)
            .begin_guest_callback(&call)
            .expect("prepare font callback");
        fixture.acknowledge_pending_delta();
        assert_eq!(prepared.frame_id, RuntimeCallbackFrameId(1));
        assert_eq!(prepared.scratch_address, CALLBACK_SCRATCH_ARENA_BASE);
        assert_eq!(
            prepared.scratch_bytes,
            (X86_ENUMLOGFONTEXA_BYTE_LENGTH + X86_NEWTEXTMETRICEXA_BYTE_LENGTH) as u32
        );
        assert_eq!(prepared.frame_bytes, 20);
        assert_eq!(
            fixture
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
                .memory
                .read(prepared.allocation_start, prepared.frame_bytes as usize)
                .expect("callback argument frame"),
            expected_frame
        );
        assert_eq!(
            fixture
                .memory
                .read(LOG_FONT_ADDRESS, filter.len())
                .expect("preserved LOGFONTA"),
            filter
        );

        fixture
            .runtime
            .pop_callback_frame(prepared.frame_id)
            .expect("retire font callback");
        let completion = {
            let mut session = ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime);
            fixture
                .dispatcher
                .complete_guest_callback_result(call, callback_result, &mut session)
        };
        let completion = match completion {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected completed enumeration, got {other:?}"),
        };
        assert_eq!(completion.eax, RegisterUpdate::Set(callback_result));
        assert_eq!(completion.instruction_pointer, ENUM_RETURN_ADDRESS);
        assert_eq!(
            completion.stack_pointer,
            GuestAddress(ENUM_STACK_POINTER.0 + 24)
        );
        assert_eq!(completion.last_error, INITIAL_LAST_ERROR);
        assert!(completion.guest_writes.is_empty());
    }
}

#[test]
fn enum_font_families_ex_a_rejects_unreached_filters_flags_and_callbacks_atomically() {
    for (filter, callback, flags, expected_class, expected_code) in [
        (
            filter_with_charset(0),
            CALLBACK_TARGET,
            0,
            ApiFaultClass::Unsupported,
            "enum-font-families-ex-a-filter-unsupported",
        ),
        (
            unsupported_face_filter(),
            CALLBACK_TARGET,
            0,
            ApiFaultClass::Unsupported,
            "enum-font-families-ex-a-filter-unsupported",
        ),
        (
            unterminated_face_filter(),
            CALLBACK_TARGET,
            0,
            ApiFaultClass::Unsupported,
            "enum-font-families-ex-a-face-name-unterminated",
        ),
        (
            primary_filter(),
            CALLBACK_TARGET,
            1,
            ApiFaultClass::Unsupported,
            "enum-font-families-ex-a-flags-unsupported",
        ),
        (
            primary_filter(),
            GET_DC_STACK_POINTER,
            0,
            ApiFaultClass::InvalidCall,
            "guest-page-fault",
        ),
    ] {
        let mut fixture = FontFixture::new();
        let device_context = fixture.acquire_screen_dc();
        fixture.install_filter(&filter);
        fixture.configure_enum(device_context, LOG_FONT_ADDRESS, callback, flags);
        assert_enum_fault_atomic(&mut fixture, expected_class, expected_code);
    }

    let mut disabled = FontFixture::new();
    let device_context = disabled.acquire_screen_dc();
    disabled.install_filter(&primary_filter());
    disabled.configure_enum(device_context, LOG_FONT_ADDRESS, CALLBACK_TARGET, 0);
    disabled.gdi32_policy.enum_font_families_ex_a = None;
    assert_enum_fault_atomic(
        &mut disabled,
        ApiFaultClass::Unsupported,
        "enum-font-families-ex-a-policy-disabled",
    );
}

#[test]
fn enum_font_families_ex_a_requires_live_same_thread_screen_dc_atomically() {
    let mut unknown = FontFixture::new();
    unknown.install_filter(&primary_filter());
    unknown.configure_enum(0x4701_ffff, LOG_FONT_ADDRESS, CALLBACK_TARGET, 0);
    assert_enum_fault_atomic(
        &mut unknown,
        ApiFaultClass::InvalidCall,
        "device-context-handle-unknown",
    );

    let mut stale = FontFixture::new();
    let stale_handle = stale.acquire_screen_dc();
    stale.release_screen_dc(stale_handle);
    stale.install_filter(&primary_filter());
    stale.configure_enum(stale_handle, LOG_FONT_ADDRESS, CALLBACK_TARGET, 0);
    assert_enum_fault_atomic(
        &mut stale,
        ApiFaultClass::InvalidCall,
        "device-context-handle-unknown",
    );

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
    assert_enum_fault_atomic(
        &mut cross_thread,
        ApiFaultClass::InvalidCall,
        "screen-device-context-relation-mismatch",
    );
}

#[test]
fn font_callback_preparation_failure_preserves_memory_and_frame_identity() {
    let mut fixture = FontFixture::with_enum_stack(LOW_ENUM_STACK_POINTER);
    let device_context = fixture.acquire_screen_dc();
    fixture.install_filter(&primary_filter());
    fixture.configure_enum(device_context, LOG_FONT_ADDRESS, CALLBACK_TARGET, 0);
    let call = match fixture.dispatch_enum() {
        DispatchOutcome::GuestCallback(call) => call,
        other => panic!("expected font callback, got {other:?}"),
    };
    let runtime_before = fixture.runtime.clone();
    let memory_before = fixture.memory.stats();
    let memory_hash_before = fixture
        .memory
        .current_reflection()
        .expect("current reflection")
        .content_sha256;
    let fault = ExecutionSession::new(&mut fixture.memory, &mut fixture.runtime)
        .begin_guest_callback(&call)
        .expect_err("low callback stack must fail");
    assert_eq!(fault.class, ApiFaultClass::InvalidCall);
    assert_eq!(fault.code, "guest-page-fault");
    assert_eq!(fixture.runtime, runtime_before);
    assert_eq!(fixture.memory.stats(), memory_before);
    assert_eq!(
        fixture
            .memory
            .current_reflection()
            .expect("unchanged reflection")
            .content_sha256,
        memory_hash_before
    );
    let frame = fixture
        .runtime
        .push_callback_frame(CALLBACK_TARGET, CALLBACK_SENTINEL)
        .expect("failed preparation did not consume callback identity");
    assert_eq!(frame.id, RuntimeCallbackFrameId(1));
}

fn assert_enum_fault_atomic(
    fixture: &mut FontFixture,
    expected_class: ApiFaultClass,
    expected_code: &str,
) {
    let runtime_before = fixture.runtime.clone();
    let memory_before = fixture.memory.stats();
    let memory_hash_before = fixture
        .memory
        .current_reflection()
        .expect("current reflection")
        .content_sha256;
    let fault = match fixture.dispatch_enum() {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected enumeration fault, got {other:?}"),
    };
    assert_eq!(fault.class, expected_class);
    assert_eq!(fault.code, expected_code);
    assert_eq!(fixture.runtime, runtime_before);
    assert_eq!(fixture.memory.stats(), memory_before);
    assert_eq!(
        fixture
            .memory
            .current_reflection()
            .expect("unchanged reflection")
            .content_sha256,
        memory_hash_before
    );
}

fn assert_create_font_fault_atomic(
    fixture: &mut FontFixture,
    expected_class: ApiFaultClass,
    expected_code: &str,
) {
    let runtime_before = fixture.runtime.clone();
    let memory_before = fixture.memory.stats();
    let memory_hash_before = fixture
        .memory
        .current_reflection()
        .expect("current reflection")
        .content_sha256;
    let fault = match fixture.dispatch_create_font() {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected CreateFontA fault, got {other:?}"),
    };
    assert_eq!(fault.class, expected_class);
    assert_eq!(fault.code, expected_code);
    assert_eq!(fixture.runtime, runtime_before);
    assert_eq!(fixture.memory.stats(), memory_before);
    assert_eq!(
        fixture
            .memory
            .current_reflection()
            .expect("unchanged reflection")
            .content_sha256,
        memory_hash_before
    );
}

fn primary_bitmap_info() -> Vec<u8> {
    bitmap_info_bytes(128, -96)
}

fn post_create_font_bitmap_info() -> Vec<u8> {
    bitmap_info_bytes(448, -336)
}

fn dialogue_bitmap_info() -> Vec<u8> {
    bitmap_info_bytes(160, -120)
}

fn large_scenario_bitmap_info() -> Vec<u8> {
    bitmap_info_bytes(384, -288)
}

fn save_catalog_bitmap_info() -> Vec<u8> {
    bitmap_info_bytes(576, -216)
}

fn scenario_256x192_bitmap_info() -> Vec<u8> {
    bitmap_info_bytes(256, -192)
}

fn scenario_288x216_bitmap_info() -> Vec<u8> {
    bitmap_info_bytes(288, -216)
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

fn post_heap_growth_create_font_arguments() -> [u32; 13] {
    [224, 112, 0, 0, 100, 0, 0, 0, 128, 4, 0, 2, 0]
}

fn dialogue_create_font_arguments() -> [u32; 13] {
    [80, 40, 0, 0, 100, 0, 0, 0, 128, 4, 0, 2, 0]
}

fn large_scenario_create_font_arguments() -> [u32; 13] {
    [192, 96, 0, 0, 100, 0, 0, 0, 128, 4, 0, 2, 0]
}

fn save_catalog_create_font_arguments() -> [u32; 13] {
    [144, 144, 0, 0, 100, 0, 0, 0, 128, 4, 0, 2, 0]
}

fn scenario_128x64_create_font_arguments() -> [u32; 13] {
    [128, 64, 0, 0, 100, 0, 0, 0, 128, 4, 0, 2, 0]
}

fn scenario_144x72_create_font_arguments() -> [u32; 13] {
    [144, 72, 0, 0, 100, 0, 0, 0, 128, 4, 0, 2, 0]
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

fn read_i32(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("i32 bytes"))
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32 bytes"))
}

fn fnv1a32(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5_u32, |digest, byte| {
        (digest ^ u32::from(*byte)).wrapping_mul(0x0100_0193)
    })
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
            profile_id: "gdi32-test-startup-profile".to_owned(),
            bytes: startup,
        },
        process_image_path_a: RuntimeProcessImagePathA {
            source_authority: "gdi32-test".to_owned(),
            bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: RuntimeCurrentDirectoryA {
            source_authority: "gdi32-test".to_owned(),
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

use vine_core::{
    ApiDispatcher, ApiEndpointIdentity, ApiFaultClass, ApiOwnedTrap, ApiReturnSchema,
    ApiValueSchema, CALLBACK_SCRATCH_ARENA_BASE, CallbackPolicy, DispatchOutcome, ExecutionSession,
    GUEST_PAGE_SIZE, Gdi32CompatibilityPolicy, GuestAddress, GuestCallbackArgument,
    GuestCallbackCompletionPolicy, GuestMemory, GuestRange, ImportTarget, ImportThunk,
    ImportThunkDisposition, ImportThunkId, LastErrorPolicy, MemoryLimits, MemoryOwnerId,
    MemoryOwnerKind, MemoryRegionKind, PageProtection, ProcessAddressSpaceLayout, RegisterUpdate,
    RuntimeBootstrap, RuntimeCallbackFrameId, RuntimeClockState, RuntimeCurrentDirectoryA,
    RuntimeDeviceContextTarget, RuntimeProcessId, RuntimeProcessImagePathA, RuntimeStartupInfo,
    RuntimeState, RuntimeThreadId, User32CompatibilityPolicy, X86_ENUMLOGFONTEXA_BYTE_LENGTH,
    X86_LOGFONTA_BYTE_LENGTH, X86_NEWTEXTMETRICEXA_BYTE_LENGTH, dispatch_gdi32_api_with_policy,
    dispatch_user32_api_with_policy, enum_font_families_ex_a_contract, gdi32_contract_for_endpoint,
    get_dc_contract, release_dc_contract,
};

const MEMORY_BASE: GuestAddress = GuestAddress(0x2000_0000);
const GET_DC_STACK_POINTER: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x1040);
const RELEASE_DC_STACK_POINTER: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x1060);
const ENUM_STACK_POINTER: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x1080);
const LOW_ENUM_STACK_POINTER: GuestAddress = GuestAddress(MEMORY_BASE.0 + 8);
const GET_DC_IAT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2000);
const RELEASE_DC_IAT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2004);
const ENUM_IAT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2008);
const CALLBACK_TARGET: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2100);
const CALLBACK_SENTINEL: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2110);
const LOG_FONT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2200);
const GET_DC_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0ee0);
const RELEASE_DC_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0ef0);
const ENUM_ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0fc0);
const GET_DC_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1002_c149);
const RELEASE_DC_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1002_c16a);
const ENUM_RETURN_ADDRESS: GuestAddress = GuestAddress(0x1002_c16a);
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
        for (iat, target) in [
            (GET_DC_IAT_ADDRESS, GET_DC_ENDPOINT_ADDRESS),
            (RELEASE_DC_IAT_ADDRESS, RELEASE_DC_ENDPOINT_ADDRESS),
            (ENUM_IAT_ADDRESS, ENUM_ENDPOINT_ADDRESS),
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
            ],
            [
                get_dc_contract(get_dc_endpoint.clone()).expect("GetDC contract"),
                release_dc_contract(release_dc_endpoint.clone()).expect("ReleaseDC contract"),
                enum_font_families_ex_a_contract(enum_endpoint.clone())
                    .expect("EnumFontFamiliesExA contract"),
            ],
        )
        .expect("GDI fixture dispatcher");
        let user32_policy = User32CompatibilityPolicy::controlled_v19();
        let gdi32_policy = Gdi32CompatibilityPolicy::controlled_v1();
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
        encoded_pointer_token_prefix: 0xec01_0000,
    })
    .expect("runtime");
    runtime
        .start_initial_thread()
        .expect("start initial thread");
    runtime
}

use crate::dinput8::{
    DINPUT8_CLASS_E_NOAGGREGATION, DINPUT8_DI8DEVCLASS_GAMECTRL, DINPUT8_DIEDFL_ATTACHEDONLY,
    DINPUT8_DIERR_BETADIRECTINPUTVERSION, DINPUT8_DIERR_OLDDIRECTINPUTVERSION,
    DINPUT8_E_INVALIDARG, DINPUT8_IDIRECTINPUT8A_IID, DINPUT8_VERSION,
};
use crate::dinput8::{dinput8_argument_words, dispatch_dinput8_api};
use crate::{
    ApiDispatcher, ApiEndpointIdentity, ApiOwnedTrap, COM_E_NOINTERFACE, ComCallTrap,
    ComDispatchOutcome, DispatchOutcome, GuestAddress, GuestMemory, ImportTarget, ImportThunk,
    ImportThunkDisposition, ImportThunkId, MemoryLimits, MemoryOwnerId, MemoryOwnerKind,
    MemoryRegionKind, PageProtection, RUNTIME_COM_IUNKNOWN_IID, Runtime, RuntimeBootstrap,
    RuntimeClockState, RuntimeComClass, RuntimeCurrentDirectoryA, RuntimeProcessId,
    RuntimeProcessImagePathA, RuntimeStartupInfo, RuntimeThreadId, X86_STARTUP_INFO_W_BYTE_LENGTH,
};

const MEMORY_BASE: GuestAddress = GuestAddress(0x2000_0000);
const MEMORY_BYTES: u64 = 0x3000;
const STACK: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x100);
const COM_STACK: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x180);
const IID_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x400);
const OUTPUT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x420);
const IAT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2000);
const ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0010);
const RETURN_ADDRESS: GuestAddress = GuestAddress(0x1005_c234);
const ENUM_DEVICES_CALL_SITE: GuestAddress = GuestAddress(0x1005_c1ce);
const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;
const LAST_ERROR_SENTINEL: u32 = 0x1234_5678;
const OUTPUT_SENTINEL: u32 = 0xcccc_cccc;
const INSTANCE: u32 = 0x1000_0000;
const UNKNOWN_IID: [u8; 16] = [
    0xef, 0xbe, 0xad, 0xde, 0x11, 0x11, 0x22, 0x22, 0x33, 0x33, 0x44, 0x44, 0x55, 0x55, 0x66, 0x66,
];

#[test]
fn native_com_slot_routing_does_not_depend_on_the_diagnostic_method_name() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    let mut methods = Vec::new();
    for (_slot, name, words, implemented) in [
        (0, "query-diagnostic", 2, true),
        (1, "add-reference-diagnostic", 0, true),
        (2, "release-diagnostic", 0, true),
        (3, "unimplemented", 0, false),
        (4, "diagnostic-label", 4, true),
    ] {
        methods.push(crate::com::ComMethodSpec {
            name: name.to_owned(),
            explicit_argument_words: implemented.then_some(words),
        });
    }
    let installed = fixture
        .runtime
        .install_com_object(crate::com::ComObjectSpec {
            class: RuntimeComClass::DirectInput8A,
            interfaces: vec![
                crate::com::ComInterfaceSpec {
                    iid: RUNTIME_COM_IUNKNOWN_IID,
                    name: "IUnknown".to_owned(),
                    methods: methods[..3].to_vec(),
                },
                crate::com::ComInterfaceSpec {
                    iid: DINPUT8_IDIRECTINPUT8A_IID,
                    name: "IDirectInput8A".to_owned(),
                    methods,
                },
            ],
        })
        .unwrap();
    let interface = fixture
        .runtime
        .com()
        .interface_for_object_iid(installed.object_id, DINPUT8_IDIRECTINPUT8A_IID)
        .unwrap()
        .clone();
    write_words(
        &mut fixture.runtime.memory,
        COM_STACK,
        &[
            ENUM_DEVICES_CALL_SITE.0,
            interface.guest_pointer.0,
            DINPUT8_DI8DEVCLASS_GAMECTRL,
            0,
            0,
            DINPUT8_DIEDFL_ATTACHEDONLY,
        ],
    );
    acknowledge_pending(&mut fixture.runtime.memory);
    let outcome = fixture.dispatch_dinput_com(&interface, 4, COM_STACK);
    let ComDispatchOutcome::Completed(completion) = outcome else {
        panic!("diagnostic method name changed native slot routing: {outcome:?}");
    };
    assert_eq!(completion.eax, DINPUT8_E_INVALIDARG.bits());
    assert!(completion.presented_device.is_none());
    write_words(
        &mut fixture.runtime.memory,
        COM_STACK,
        &[ENUM_DEVICES_CALL_SITE.0, interface.guest_pointer.0],
    );
    acknowledge_pending(&mut fixture.runtime.memory);
    for (slot, expected_count) in [(1, 2), (2, 1)] {
        let outcome = fixture.dispatch_com(&interface, slot, COM_STACK);
        let ComDispatchOutcome::Completed(completion) = outcome else {
            panic!("diagnostic label changed IUnknown slot routing: {outcome:?}");
        };
        assert_eq!(completion.eax, expected_count);
        assert!(completion.presented_device.is_none());
    }
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
}

#[test]
fn direct_input_8_create_installs_owned_identity_and_stops_at_reached_enum_devices() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    let completion = immediate(fixture.dispatch_factory(
        DINPUT8_VERSION,
        DINPUT8_IDIRECTINPUT8A_IID.windows_bytes(),
        0,
    ));
    assert_eq!(completion.eax, Some(0));
    assert_eq!(completion.stack_pointer, GuestAddress(STACK.0 + 24));
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
    fixture.acknowledge();

    let pointer = read_u32(&fixture.runtime.memory, OUTPUT_ADDRESS);
    assert_ne!(pointer, 0);
    assert_ne!(pointer, OUTPUT_SENTINEL);
    let object = fixture
        .runtime
        .com()
        .objects()
        .values()
        .next()
        .expect("DirectInput root object")
        .clone();
    assert_eq!(object.class, RuntimeComClass::DirectInput8A);
    assert_eq!(object.reference_count, 1);
    let interface = fixture
        .runtime
        .com_interface_for_pointer(GuestAddress(pointer))
        .unwrap()
        .expect("IDirectInput8A pointer")
        .clone();
    assert_eq!(interface.iid, DINPUT8_IDIRECTINPUT8A_IID);
    assert_eq!(interface.name, "IDirectInput8A");
    assert_eq!(interface.methods.len(), 11);
    assert_eq!(interface.methods[&3].name, "CreateDevice");

    assert_eq!(interface.methods[&4].name, "EnumDevices");

    assert_eq!(interface.methods[&10].name, "ConfigureDevices");

    write_words(
        &mut fixture.runtime.memory,
        COM_STACK,
        &[ENUM_DEVICES_CALL_SITE.0, pointer, 4, 0x1005_bef0, 0, 1],
    );
    acknowledge_pending(&mut fixture.runtime.memory);
    let before_memory = fixture.runtime.memory.stats();
    let fault = faulted(fixture.dispatch_com(&interface, 4, COM_STACK));
    assert_eq!(fault.call_site, ENUM_DEVICES_CALL_SITE);
    let target = fault.target.expect("typed EnumDevices target");
    assert_eq!(target.interface, "IDirectInput8A");
    assert_eq!(target.vtable_slot, 4);
    assert_eq!(target.method, "EnumDevices");
    assert_eq!(fixture.runtime.memory.stats(), before_memory);
    assert_live_direct_input(&fixture, &interface);

    write_words(
        &mut fixture.runtime.memory,
        COM_STACK,
        &[ENUM_DEVICES_CALL_SITE.0, pointer],
    );
    acknowledge_pending(&mut fixture.runtime.memory);
    let release = match fixture.dispatch_com(&interface, 2, COM_STACK) {
        ComDispatchOutcome::Completed(completion) => completion,
        other => panic!("Release did not complete: {other:?}"),
    };
    assert_eq!(release.eax, 0);
    assert!(fixture.runtime.com().is_empty());
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
}

#[test]
fn enum_devices_returns_an_empty_game_controller_catalog_without_callbacks() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    immediate(fixture.dispatch_factory(
        DINPUT8_VERSION,
        DINPUT8_IDIRECTINPUT8A_IID.windows_bytes(),
        0,
    ));
    fixture.acknowledge();
    let pointer = read_u32(&fixture.runtime.memory, OUTPUT_ADDRESS);
    let interface = fixture
        .runtime
        .com_interface_for_pointer(GuestAddress(pointer))
        .unwrap()
        .expect("IDirectInput8A pointer")
        .clone();
    let callback = interface.methods[&0].endpoint.0;

    write_words(
        &mut fixture.runtime.memory,
        COM_STACK,
        &[
            ENUM_DEVICES_CALL_SITE.0,
            pointer,
            DINPUT8_DI8DEVCLASS_GAMECTRL,
            callback,
            0,
            DINPUT8_DIEDFL_ATTACHEDONLY,
        ],
    );
    acknowledge_pending(&mut fixture.runtime.memory);
    let before_memory = fixture.runtime.memory.stats();
    let completion = match fixture.dispatch_dinput_com(&interface, 4, COM_STACK) {
        ComDispatchOutcome::Completed(completion) => completion,
        other => panic!("EnumDevices call did not complete: {other:?}"),
    };
    assert_eq!(completion.eax, 0);
    assert_eq!(completion.stack_pointer, GuestAddress(COM_STACK.0 + 24));
    assert_eq!(
        fixture
            .runtime
            .com()
            .object(interface.object_id)
            .unwrap()
            .reference_count,
        1
    );
    assert_eq!(fixture.runtime.memory.stats(), before_memory);
    assert_live_direct_input(&fixture, &interface);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
}

#[test]
fn enum_devices_rejects_invalid_arguments_and_stops_on_valid_unreached_variants_atomically() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    immediate(fixture.dispatch_factory(
        DINPUT8_VERSION,
        DINPUT8_IDIRECTINPUT8A_IID.windows_bytes(),
        0,
    ));
    fixture.acknowledge();
    let pointer = read_u32(&fixture.runtime.memory, OUTPUT_ADDRESS);
    let interface = fixture
        .runtime
        .com_interface_for_pointer(GuestAddress(pointer))
        .unwrap()
        .expect("IDirectInput8A pointer")
        .clone();
    let callback = interface.methods[&0].endpoint.0;

    for (device_class, callback, flags) in [
        (DINPUT8_DI8DEVCLASS_GAMECTRL, 0, DINPUT8_DIEDFL_ATTACHEDONLY),
        (99, callback, DINPUT8_DIEDFL_ATTACHEDONLY),
        (DINPUT8_DI8DEVCLASS_GAMECTRL, callback, 0x8000_0000),
    ] {
        write_words(
            &mut fixture.runtime.memory,
            COM_STACK,
            &[
                ENUM_DEVICES_CALL_SITE.0,
                pointer,
                device_class,
                callback,
                0x1234_5678,
                flags,
            ],
        );
        acknowledge_pending(&mut fixture.runtime.memory);
        let before_memory = fixture.runtime.memory.stats();
        let completion = match fixture.dispatch_dinput_com(&interface, 4, COM_STACK) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("invalid EnumDevices arguments did not return HRESULT: {other:?}"),
        };
        assert_eq!(completion.eax, DINPUT8_E_INVALIDARG.bits());
        assert_eq!(fixture.runtime.memory.stats(), before_memory);
        assert_live_direct_input(&fixture, &interface);
    }

    for (device_class, callback) in [(3, callback), (DINPUT8_DI8DEVCLASS_GAMECTRL, MEMORY_BASE.0)] {
        write_words(
            &mut fixture.runtime.memory,
            COM_STACK,
            &[
                ENUM_DEVICES_CALL_SITE.0,
                pointer,
                device_class,
                callback,
                0,
                DINPUT8_DIEDFL_ATTACHEDONLY,
            ],
        );
        acknowledge_pending(&mut fixture.runtime.memory);
        let before_memory = fixture.runtime.memory.stats();
        faulted(fixture.dispatch_dinput_com(&interface, 4, COM_STACK));
        assert_eq!(fixture.runtime.memory.stats(), before_memory);
        assert_live_direct_input(&fixture, &interface);
    }
}

#[test]
fn factory_variants_clear_output_without_allocating_com_state() {
    for (version, iid, outer, expected) in [
        (DINPUT8_VERSION, UNKNOWN_IID, 0, COM_E_NOINTERFACE),
        (
            DINPUT8_VERSION - 0x100,
            DINPUT8_IDIRECTINPUT8A_IID.windows_bytes(),
            0,
            DINPUT8_DIERR_BETADIRECTINPUTVERSION,
        ),
        (
            DINPUT8_VERSION + 0x100,
            DINPUT8_IDIRECTINPUT8A_IID.windows_bytes(),
            0,
            DINPUT8_DIERR_OLDDIRECTINPUTVERSION,
        ),
        (
            DINPUT8_VERSION,
            DINPUT8_IDIRECTINPUT8A_IID.windows_bytes(),
            0x2000_0100,
            DINPUT8_CLASS_E_NOAGGREGATION,
        ),
        (
            DINPUT8_VERSION,
            DINPUT8_IDIRECTINPUT8A_IID.windows_bytes(),
            1,
            DINPUT8_CLASS_E_NOAGGREGATION,
        ),
        (
            DINPUT8_VERSION,
            DINPUT8_IDIRECTINPUT8A_IID.windows_bytes(),
            u32::MAX,
            DINPUT8_CLASS_E_NOAGGREGATION,
        ),
    ] {
        let mut fixture = Fixture::new(MemoryLimits::default());
        let clock_before = fixture.runtime.clock().clone();
        let completion = immediate(fixture.dispatch_factory(version, iid, outer));
        assert_eq!(completion.eax, Some(expected.0 as u32));
        assert_eq!(
            fixture.runtime.current_last_error().unwrap(),
            LAST_ERROR_SENTINEL
        );
        fixture.acknowledge();
        assert_eq!(read_u32(&fixture.runtime.memory, OUTPUT_ADDRESS), 0);
        assert_eq!(fixture.runtime.clock(), &clock_before);
        assert_eq!(
            fixture.runtime.current_last_error().unwrap(),
            LAST_ERROR_SENTINEL
        );
        assert!(fixture.runtime.com().is_empty());
    }
}

#[test]
fn factory_can_publish_iunknown_and_preserves_failure_atomicity() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    let completion = immediate(fixture.dispatch_factory(
        DINPUT8_VERSION,
        RUNTIME_COM_IUNKNOWN_IID.windows_bytes(),
        0,
    ));
    assert_eq!(completion.eax, Some(0));
    fixture.acknowledge();
    let pointer = GuestAddress(read_u32(&fixture.runtime.memory, OUTPUT_ADDRESS));
    let interface = fixture
        .runtime
        .com_interface_for_pointer(pointer)
        .unwrap()
        .expect("factory IUnknown pointer");
    assert_eq!(interface.iid, RUNTIME_COM_IUNKNOWN_IID);
    assert_eq!(interface.name, "IUnknown");

    let limits = MemoryLimits {
        max_physical_pages: 3,
        ..MemoryLimits::default()
    };
    let mut exhausted = Fixture::new(limits);
    exhausted.prepare_factory_call(
        DINPUT8_VERSION,
        DINPUT8_IDIRECTINPUT8A_IID.windows_bytes(),
        0,
    );
    let before_memory = exhausted.runtime.memory.stats();
    let allocations_before = exhausted.runtime.process_address_space_allocator().clone();
    let clock_before = exhausted.runtime.clock().clone();
    let outcome = exhausted.dispatch_prepared();
    let DispatchOutcome::Fault(_) = outcome else {
        panic!("exhausted COM factory did not fault: {outcome:?}");
    };
    assert_eq!(
        read_u32(&exhausted.runtime.memory, OUTPUT_ADDRESS),
        OUTPUT_SENTINEL
    );
    assert_eq!(exhausted.runtime.memory.stats(), before_memory);
    assert_eq!(
        exhausted.runtime.process_address_space_allocator(),
        &allocations_before
    );
    assert_eq!(exhausted.runtime.clock(), &clock_before);
    assert_eq!(
        exhausted.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
    assert!(exhausted.runtime.com().is_empty());
}

#[test]
fn factory_catalog_matches_only_named_direct_input_exports() {
    let fixture = Fixture::new(MemoryLimits::default());
    assert!(dinput8_argument_words(&fixture.endpoint).is_some());

    let mut other = fixture.endpoint;
    other.module = "d3d9.dll".to_owned();
    assert!(dinput8_argument_words(&other).is_none());
}

fn assert_live_direct_input(fixture: &Fixture, interface: &crate::RuntimeComInterfaceRecord) {
    let live = fixture
        .runtime
        .com_interface_for_pointer(interface.guest_pointer)
        .unwrap()
        .expect("live DirectInput interface");
    assert_eq!(live.id, interface.id);
    assert_eq!(live.object_id, interface.object_id);
    assert_eq!(live.iid, DINPUT8_IDIRECTINPUT8A_IID);
    assert_eq!(live.vtable, interface.vtable);
    assert_eq!(fixture.runtime.com().objects().len(), 1);
    let object = &fixture.runtime.com().objects()[&interface.object_id];
    assert_eq!(object.class, RuntimeComClass::DirectInput8A);
    assert_eq!(object.reference_count, 1);
    assert_eq!(
        fixture.runtime.current_thread_id(),
        RuntimeThreadId(THREAD_ID)
    );
    assert!(
        fixture
            .runtime
            .thread_state(RuntimeThreadId(THREAD_ID))
            .unwrap()
            .callback_frames
            .is_empty()
    );
    assert_eq!(
        fixture.runtime.scheduler_state().unwrap(),
        crate::SchedulerThreadState::Runnable
    );
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
}

struct Fixture {
    runtime: Runtime,
    dispatcher: ApiDispatcher,
    endpoint: ApiEndpointIdentity,
}

impl Fixture {
    fn new(limits: MemoryLimits) -> Self {
        let mut memory = GuestMemory::new(limits).expect("guest memory");
        let owner = memory
            .create_owner(MemoryOwnerKind::Runtime)
            .expect("fixture owner");
        let range = memory
            .reserve_fixed(owner, MemoryRegionKind::Runtime, MEMORY_BASE, MEMORY_BYTES)
            .expect("fixture reservation");
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
            .expect("fixture commit");
        memory
            .write_initialization(owner, IAT_ADDRESS, &ENDPOINT_ADDRESS.0.to_le_bytes())
            .expect("DirectInput IAT initialization");
        memory.seal_owner(owner).expect("seal fixture owner");
        let reflection = memory.pending_reflection().expect("initial reflection");
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .expect("acknowledge initial reflection");
        let endpoint = endpoint(owner);
        let dispatcher =
            ApiDispatcher::from_endpoints([endpoint.clone()]).expect("DirectInput dispatcher");
        Self {
            runtime: controlled_runtime(memory),
            dispatcher,
            endpoint,
        }
    }

    fn prepare_factory_call(&mut self, version: u32, iid: [u8; 16], outer_unknown: u32) {
        self.runtime
            .memory
            .write(IID_ADDRESS, &iid)
            .expect("write IID");
        self.runtime
            .memory
            .write(OUTPUT_ADDRESS, &OUTPUT_SENTINEL.to_le_bytes())
            .expect("write output sentinel");
        write_words(
            &mut self.runtime.memory,
            STACK,
            &[
                RETURN_ADDRESS.0,
                INSTANCE,
                version,
                IID_ADDRESS.0,
                OUTPUT_ADDRESS.0,
                outer_unknown,
            ],
        );
        acknowledge_pending(&mut self.runtime.memory);
    }

    fn dispatch_factory(
        &mut self,
        version: u32,
        iid: [u8; 16],
        outer_unknown: u32,
    ) -> DispatchOutcome {
        self.prepare_factory_call(version, iid, outer_unknown);
        self.dispatch_prepared()
    }

    fn dispatch_prepared(&mut self) -> DispatchOutcome {
        let trap = ApiOwnedTrap {
            endpoint_id: self.endpoint.id,
            endpoint_address: self.endpoint.address,
            stack_pointer: STACK,
            return_address: RETURN_ADDRESS,
        };
        self.dispatcher
            .dispatch_owned(trap, &mut self.runtime, None, |frame, runtime, _| {
                dispatch_dinput8_api(frame, runtime)
            })
    }

    fn dispatch_com(
        &mut self,
        interface: &crate::RuntimeComInterfaceRecord,
        slot: u16,
        stack_pointer: GuestAddress,
    ) -> ComDispatchOutcome {
        self.runtime.dispatch_com_call(ComCallTrap {
            endpoint_address: interface.methods[&slot].endpoint,
            stack_pointer,
            return_address: ENUM_DEVICES_CALL_SITE,
            call_site: ENUM_DEVICES_CALL_SITE,
        })
    }

    fn dispatch_dinput_com(
        &mut self,
        interface: &crate::RuntimeComInterfaceRecord,
        slot: u16,
        stack_pointer: GuestAddress,
    ) -> ComDispatchOutcome {
        self.runtime.dispatch_com_call(ComCallTrap {
            endpoint_address: interface.methods[&slot].endpoint,
            stack_pointer,
            return_address: ENUM_DEVICES_CALL_SITE,
            call_site: ENUM_DEVICES_CALL_SITE,
        })
    }

    fn acknowledge(&mut self) {
        acknowledge_pending(&mut self.runtime.memory);
    }
}

fn endpoint(owner: MemoryOwnerId) -> ApiEndpointIdentity {
    ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(2),
        address: ENDPOINT_ADDRESS,
        requester_owner: owner,
        module: "dinput8.dll".to_owned(),
        iat_rva: crate::Rva(0x000d_1050),
        iat_address: IAT_ADDRESS,
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "DirectInput8Create".to_owned(),
            hint: 0,
            name_rva: crate::Rva(0),
        },
    })
}

fn immediate(outcome: DispatchOutcome) -> crate::ApiImmediateCompletion {
    match outcome {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("factory did not complete immediately: {other:?}"),
    }
}

fn faulted(outcome: ComDispatchOutcome) -> crate::ComFault {
    match outcome {
        ComDispatchOutcome::Fault(fault) => fault,
        other => panic!("COM call did not fault: {other:?}"),
    }
}

fn write_words(memory: &mut GuestMemory, address: GuestAddress, words: &[u32]) {
    let bytes = words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<_>>();
    memory.write(address, &bytes).expect("write fixture words");
}

fn read_u32(memory: &GuestMemory, address: GuestAddress) -> u32 {
    u32::from_le_bytes(
        memory
            .read(address, 4)
            .expect("read fixture word")
            .try_into()
            .unwrap(),
    )
}

fn acknowledge_pending(memory: &mut GuestMemory) {
    if memory.stats().pending_invalidation.is_some() {
        let delta = memory.pending_delta().expect("pending fixture delta");
        memory
            .acknowledge_delta(delta.generation)
            .expect("acknowledge fixture delta");
    }
}

fn controlled_runtime(memory: GuestMemory) -> Runtime {
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
            startup_info: RuntimeStartupInfo {
                bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
            },
            process_image_path_a: RuntimeProcessImagePathA {
                bytes_with_nul: b"C:\\Vine\\dinput8-root.exe\0".to_vec(),
            },
            initial_current_directory_a: RuntimeCurrentDirectoryA {
                bytes_with_nul: b"C:\\Vine\0".to_vec(),
            },
            encoded_pointer_cookie: 0xec01_9d3b,
        },
        memory,
    )
    .expect("controlled DirectInput runtime");
    runtime
        .start_initial_thread()
        .expect("start DirectInput fixture thread");
    runtime
}

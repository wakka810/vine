use vine_core::{
    ApiDispatcher, ApiEndpointIdentity, ApiFaultClass, ApiReturnSchema, ApiTrap, ApiValueSchema,
    COM_E_NOINTERFACE, ComCallTrap, ComDispatchOutcome, ComFaultClass,
    DINPUT8_CLASS_E_NOAGGREGATION, DINPUT8_DI8DEVCLASS_GAMECTRL, DINPUT8_DI8DEVCLASS_KEYBOARD,
    DINPUT8_DIEDFL_ATTACHEDONLY, DINPUT8_DIERR_BETADIRECTINPUTVERSION,
    DINPUT8_DIERR_OLDDIRECTINPUTVERSION, DINPUT8_E_INVALIDARG,
    DINPUT8_IDIRECTINPUT8A_CLASS_IDENTITY, DINPUT8_IDIRECTINPUT8A_IID, DINPUT8_VERSION,
    Dinput8CompatibilityPolicy, DispatchOutcome, ExecutionSession, GuestAddress, GuestMemory,
    ImportTarget, ImportThunk, ImportThunkDisposition, ImportThunkId,
    LOGICAL_ENCODED_POINTER_NAMESPACE, MemoryLimits, MemoryOwnerId, MemoryOwnerKind,
    MemoryRegionKind, PageProtection, ProcessAddressSpaceLayout, RUNTIME_COM_IUNKNOWN_IID,
    RegisterUpdate, RuntimeBootstrap, RuntimeClockState, RuntimeComMethodImplementation,
    RuntimeComReturnKind, RuntimeCurrentDirectoryA, RuntimeProcessId, RuntimeProcessImagePathA,
    RuntimeStartupInfo, RuntimeState, RuntimeThreadId, X86_STARTUP_INFO_W_BYTE_LENGTH,
    dinput8_contract_for_endpoint, direct_input_8_create_contract, dispatch_dinput8_api,
    dispatch_dinput8_com_call,
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
fn direct_input_8_create_installs_owned_identity_and_stops_at_reached_enum_devices() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    let completion = immediate(fixture.dispatch_factory(
        DINPUT8_VERSION,
        DINPUT8_IDIRECTINPUT8A_IID.windows_bytes(),
        0,
    ));
    assert_eq!(completion.eax, RegisterUpdate::Set(0));
    assert_eq!(completion.stack_pointer, GuestAddress(STACK.0 + 24));
    assert_eq!(completion.last_error, LAST_ERROR_SENTINEL);
    assert_eq!(completion.guest_writes.len(), 1);
    assert_eq!(completion.guest_writes[0].write.address, OUTPUT_ADDRESS);
    fixture.acknowledge();

    let pointer = read_u32(&fixture.memory, OUTPUT_ADDRESS);
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
    assert_eq!(object.class_identity, DINPUT8_IDIRECTINPUT8A_CLASS_IDENTITY);
    assert_eq!(object.reference_count, 1);
    let interface = fixture
        .runtime
        .com_interface_for_pointer(GuestAddress(pointer))
        .expect("IDirectInput8A pointer")
        .clone();
    assert_eq!(interface.iid, DINPUT8_IDIRECTINPUT8A_IID);
    assert_eq!(interface.name, "IDirectInput8A");
    assert_eq!(interface.methods.len(), 11);
    assert_eq!(interface.methods[&3].name, "CreateDevice");
    assert_eq!(interface.methods[&3].explicit_argument_words, 3);
    assert_eq!(interface.methods[&4].name, "EnumDevices");
    assert_eq!(interface.methods[&4].explicit_argument_words, 4);
    assert_eq!(
        interface.methods[&4].return_kind,
        RuntimeComReturnKind::HResult
    );
    assert_eq!(
        interface.methods[&4].implementation,
        RuntimeComMethodImplementation::Frontend
    );
    assert_eq!(interface.methods[&10].name, "ConfigureDevices");
    assert_eq!(interface.methods[&10].explicit_argument_words, 4);

    write_words(
        &mut fixture.memory,
        COM_STACK,
        &[ENUM_DEVICES_CALL_SITE.0, pointer, 4, 0x1005_bef0, 0, 1],
    );
    acknowledge_pending(&mut fixture.memory);
    let before_memory = fixture.memory.stats();
    let before_runtime = fixture.runtime.clone();
    let fault = faulted(fixture.dispatch_com(&interface, 4, COM_STACK));
    assert_eq!(fault.class, ComFaultClass::Unsupported);
    assert_eq!(fault.code, "com-method-unsupported");
    assert_eq!(fault.call_site, ENUM_DEVICES_CALL_SITE);
    let target = fault.target.expect("typed EnumDevices target");
    assert_eq!(target.object_id, object.id);
    assert_eq!(target.interface_id, interface.id);
    assert_eq!(target.interface, "IDirectInput8A");
    assert_eq!(target.iid, DINPUT8_IDIRECTINPUT8A_IID);
    assert_eq!(target.vtable_slot, 4);
    assert_eq!(target.method, "EnumDevices");
    assert_eq!(fixture.memory.stats(), before_memory);
    assert_eq!(fixture.runtime, before_runtime);

    write_words(
        &mut fixture.memory,
        COM_STACK,
        &[ENUM_DEVICES_CALL_SITE.0, pointer],
    );
    acknowledge_pending(&mut fixture.memory);
    let release = match fixture.dispatch_com(&interface, 2, COM_STACK) {
        ComDispatchOutcome::Completed(completion) => completion,
        other => panic!("Release did not complete: {other:?}"),
    };
    assert_eq!(release.return_value.eax(), 0);
    assert!(release.object_retired);
    assert!(fixture.runtime.com().is_empty());
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
}

#[test]
fn enum_devices_returns_the_evidenced_empty_game_controller_catalog_without_callback_state() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    immediate(fixture.dispatch_factory(
        DINPUT8_VERSION,
        DINPUT8_IDIRECTINPUT8A_IID.windows_bytes(),
        0,
    ));
    fixture.acknowledge();
    let pointer = read_u32(&fixture.memory, OUTPUT_ADDRESS);
    let interface = fixture
        .runtime
        .com_interface_for_pointer(GuestAddress(pointer))
        .expect("IDirectInput8A pointer")
        .clone();
    let callback = interface.methods[&0].endpoint.0;
    let policy = Dinput8CompatibilityPolicy::controlled_v1();
    policy.validate().expect("controlled DirectInput policy");

    write_words(
        &mut fixture.memory,
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
    acknowledge_pending(&mut fixture.memory);
    let before_memory = fixture.memory.stats();
    let before_runtime = fixture.runtime.clone();
    let completion = match fixture.dispatch_com_with_policy(&policy, &interface, 4, COM_STACK) {
        ComDispatchOutcome::Completed(completion) => completion,
        other => panic!("evidenced EnumDevices call did not complete: {other:?}"),
    };
    assert_eq!(completion.return_value.eax(), 0);
    assert_eq!(completion.stack_pointer, GuestAddress(COM_STACK.0 + 24));
    assert_eq!(completion.reference_count, 1);
    assert!(!completion.object_retired);
    assert!(completion.guest_writes.is_empty());
    assert_eq!(fixture.memory.stats(), before_memory);
    assert_eq!(fixture.runtime, before_runtime);
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
    let pointer = read_u32(&fixture.memory, OUTPUT_ADDRESS);
    let interface = fixture
        .runtime
        .com_interface_for_pointer(GuestAddress(pointer))
        .expect("IDirectInput8A pointer")
        .clone();
    let callback = interface.methods[&0].endpoint.0;
    let policy = Dinput8CompatibilityPolicy::controlled_v1();

    for (device_class, callback, flags) in [
        (DINPUT8_DI8DEVCLASS_GAMECTRL, 0, DINPUT8_DIEDFL_ATTACHEDONLY),
        (99, callback, DINPUT8_DIEDFL_ATTACHEDONLY),
        (DINPUT8_DI8DEVCLASS_GAMECTRL, callback, 0x8000_0000),
    ] {
        write_words(
            &mut fixture.memory,
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
        acknowledge_pending(&mut fixture.memory);
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let completion = match fixture.dispatch_com_with_policy(&policy, &interface, 4, COM_STACK) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("invalid EnumDevices arguments did not return HRESULT: {other:?}"),
        };
        assert_eq!(completion.return_value.eax(), DINPUT8_E_INVALIDARG.bits());
        assert!(completion.guest_writes.is_empty());
        assert_eq!(fixture.memory.stats(), before_memory);
        assert_eq!(fixture.runtime, before_runtime);
    }

    for (policy, device_class, callback, expected_code) in [
        (
            Dinput8CompatibilityPolicy::controlled_v1(),
            DINPUT8_DI8DEVCLASS_KEYBOARD,
            callback,
            "dinput8-enum-devices-variant-unsupported",
        ),
        (
            {
                let mut disabled = Dinput8CompatibilityPolicy::controlled_v1();
                disabled.enum_devices = None;
                disabled
            },
            DINPUT8_DI8DEVCLASS_GAMECTRL,
            callback,
            "dinput8-enum-devices-policy-disabled",
        ),
        (
            Dinput8CompatibilityPolicy::controlled_v1(),
            DINPUT8_DI8DEVCLASS_GAMECTRL,
            MEMORY_BASE.0,
            "guest-page-fault",
        ),
    ] {
        write_words(
            &mut fixture.memory,
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
        acknowledge_pending(&mut fixture.memory);
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let fault = faulted(fixture.dispatch_com_with_policy(&policy, &interface, 4, COM_STACK));
        assert_eq!(fault.code, expected_code);
        assert_eq!(fixture.memory.stats(), before_memory);
        assert_eq!(fixture.runtime, before_runtime);
    }
}

#[test]
fn enum_devices_policy_rejects_device_claims_before_dispatch() {
    let mut policy = Dinput8CompatibilityPolicy::controlled_v1();
    policy
        .enum_devices
        .as_mut()
        .expect("enumeration policy")
        .attached_device_count = 1;
    let fault = policy
        .validate()
        .expect_err("callback-producing policy must be rejected");
    assert_eq!(fault.code, "dinput8-enum-devices-profile-invalid");

    let mut header = Dinput8CompatibilityPolicy::controlled_v1();
    header.schema_version += 1;
    let fault = header
        .validate()
        .expect_err("future policy version must be rejected");
    assert_eq!(fault.code, "dinput8-compatibility-policy-header-invalid");
}

#[test]
fn factory_oracle_variants_clear_output_without_allocating_com_state() {
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
    ] {
        let mut fixture = Fixture::new(MemoryLimits::default());
        let before_runtime = fixture.runtime.clone();
        let completion = immediate(fixture.dispatch_factory(version, iid, outer));
        assert_eq!(completion.eax, RegisterUpdate::Set(expected.0 as u32));
        assert_eq!(completion.last_error, LAST_ERROR_SENTINEL);
        fixture.acknowledge();
        assert_eq!(read_u32(&fixture.memory, OUTPUT_ADDRESS), 0);
        assert_eq!(fixture.runtime, before_runtime);
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
    assert_eq!(completion.eax, RegisterUpdate::Set(0));
    fixture.acknowledge();
    let pointer = GuestAddress(read_u32(&fixture.memory, OUTPUT_ADDRESS));
    let interface = fixture
        .runtime
        .com_interface_for_pointer(pointer)
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
    let before_memory = exhausted.memory.stats();
    let before_runtime = exhausted.runtime.clone();
    let outcome = exhausted.dispatch_prepared();
    let DispatchOutcome::Fault(fault) = outcome else {
        panic!("exhausted COM factory did not fault: {outcome:?}");
    };
    assert_eq!(fault.class, ApiFaultClass::ResourceLimit);
    assert_eq!(read_u32(&exhausted.memory, OUTPUT_ADDRESS), OUTPUT_SENTINEL);
    assert_eq!(exhausted.memory.stats(), before_memory);
    assert_eq!(exhausted.runtime, before_runtime);
    assert!(exhausted.runtime.com().is_empty());
}

#[test]
fn factory_catalog_fixes_the_five_word_hresult_contract() {
    let fixture = Fixture::new(MemoryLimits::default());
    let contract = direct_input_8_create_contract(fixture.endpoint.clone()).unwrap();
    assert_eq!(contract.abi.argument_bytes().unwrap(), 20);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::HResult);
    assert_eq!(contract.abi.arguments.len(), 5);
    assert!(matches!(
        contract.abi.arguments[0].value,
        ApiValueSchema::OpaquePointer {
            nullable: true,
            alignment: 4
        }
    ));
    assert!(matches!(
        contract.abi.arguments[1].value,
        ApiValueSchema::U32
    ));
    assert!(matches!(
        contract.abi.arguments[2].value,
        ApiValueSchema::Pointer { .. }
    ));
    assert!(matches!(
        contract.abi.arguments[3].value,
        ApiValueSchema::Pointer { .. }
    ));
    assert!(matches!(
        contract.abi.arguments[4].value,
        ApiValueSchema::OpaquePointer {
            nullable: true,
            alignment: 4
        }
    ));
    assert!(
        dinput8_contract_for_endpoint(fixture.endpoint.clone())
            .unwrap()
            .is_some()
    );

    let mut other = fixture.endpoint;
    other.module = "d3d9.dll".to_owned();
    assert!(dinput8_contract_for_endpoint(other).unwrap().is_none());
}

struct Fixture {
    memory: GuestMemory,
    runtime: RuntimeState,
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
        let contract = direct_input_8_create_contract(endpoint.clone()).expect("factory contract");
        let dispatcher =
            ApiDispatcher::new([endpoint.clone()], [contract]).expect("DirectInput dispatcher");
        Self {
            memory,
            runtime: controlled_runtime(),
            dispatcher,
            endpoint,
        }
    }

    fn prepare_factory_call(&mut self, version: u32, iid: [u8; 16], outer_unknown: u32) {
        self.memory.write(IID_ADDRESS, &iid).expect("write IID");
        self.memory
            .write(OUTPUT_ADDRESS, &OUTPUT_SENTINEL.to_le_bytes())
            .expect("write output sentinel");
        write_words(
            &mut self.memory,
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
        acknowledge_pending(&mut self.memory);
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
        let trap = ApiTrap {
            endpoint_id: self.endpoint.id,
            endpoint_address: self.endpoint.address,
            stack_pointer: STACK,
            return_address: RETURN_ADDRESS,
            argument_bytes: 20,
        };
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        self.dispatcher
            .dispatch(trap, &mut session, dispatch_dinput8_api)
    }

    fn dispatch_com(
        &mut self,
        interface: &vine_core::RuntimeComInterfaceRecord,
        slot: u16,
        stack_pointer: GuestAddress,
    ) -> ComDispatchOutcome {
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        session.dispatch_com_call(ComCallTrap {
            endpoint_address: interface.methods[&slot].endpoint,
            stack_pointer,
            return_address: ENUM_DEVICES_CALL_SITE,
            call_site: ENUM_DEVICES_CALL_SITE,
        })
    }

    fn dispatch_com_with_policy(
        &mut self,
        policy: &Dinput8CompatibilityPolicy,
        interface: &vine_core::RuntimeComInterfaceRecord,
        slot: u16,
        stack_pointer: GuestAddress,
    ) -> ComDispatchOutcome {
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        dispatch_dinput8_com_call(
            policy,
            ComCallTrap {
                endpoint_address: interface.methods[&slot].endpoint,
                stack_pointer,
                return_address: ENUM_DEVICES_CALL_SITE,
                call_site: ENUM_DEVICES_CALL_SITE,
            },
            &mut session,
        )
    }

    fn acknowledge(&mut self) {
        acknowledge_pending(&mut self.memory);
    }
}

fn endpoint(owner: MemoryOwnerId) -> ApiEndpointIdentity {
    ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(2),
        address: ENDPOINT_ADDRESS,
        requester_owner: owner,
        module: "dinput8.dll".to_owned(),
        iat_rva: vine_core::Rva(0x000d_1050),
        iat_address: IAT_ADDRESS,
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "DirectInput8Create".to_owned(),
            hint: 0,
            name_rva: vine_core::Rva(0),
        },
    })
}

fn immediate(outcome: DispatchOutcome) -> vine_core::ApiImmediateCompletion {
    match outcome {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("factory did not complete immediately: {other:?}"),
    }
}

fn faulted(outcome: ComDispatchOutcome) -> vine_core::ComFault {
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

fn controlled_runtime() -> RuntimeState {
    let mut runtime = RuntimeState::new(RuntimeBootstrap {
        process_id: RuntimeProcessId(PROCESS_ID),
        initial_thread_id: RuntimeThreadId(THREAD_ID),
        initial_last_error: LAST_ERROR_SENTINEL,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
            performance_frequency: 10_000_000,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "dinput8-root-fixture".to_owned(),
            bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
        },
        process_image_path_a: RuntimeProcessImagePathA {
            source_authority: "dinput8-root-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\\dinput8-root.exe\0".to_vec(),
            encoding: "windows-932-v1".to_owned(),
        },
        initial_current_directory_a: RuntimeCurrentDirectoryA {
            source_authority: "dinput8-root-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: "windows-932-v1".to_owned(),
        },
        encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec01_0000,
    })
    .expect("controlled DirectInput runtime");
    runtime
        .start_initial_thread()
        .expect("start DirectInput fixture thread");
    runtime
}

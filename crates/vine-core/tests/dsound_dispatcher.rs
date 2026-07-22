use vine_core::{
    ApiDispatcher, ApiEndpointIdentity, ApiFaultClass, ApiReturnSchema, ApiTrap, ApiValueSchema,
    ComCallTrap, ComDispatchOutcome, ComFaultClass, DSOUND_CLASS_E_NOAGGREGATION,
    DSOUND_DIRECT_SOUND_CREATE_8_ORDINAL, DSOUND_DSBCAPS_PRIMARYBUFFER, DSOUND_DSBUFFERDESC_BYTES,
    DSOUND_DSSCL_NORMAL, DSOUND_DSSCL_PRIORITY, DSOUND_E_INVALIDARG,
    DSOUND_IDIRECTSOUND8_CLASS_IDENTITY, DSOUND_IDIRECTSOUND8_IID, DispatchOutcome,
    DsoundCompatibilityPolicy, ExecutionSession, GuestAddress, GuestMemory, ImportTarget,
    ImportThunk, ImportThunkDisposition, ImportThunkId, LOGICAL_ENCODED_POINTER_NAMESPACE,
    MemoryLimits, MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind, PageProtection,
    ProcessAddressSpaceLayout, RegisterUpdate, RuntimeBootstrap, RuntimeClockState,
    RuntimeComMethodImplementation, RuntimeComReturnKind, RuntimeCurrentDirectoryA,
    RuntimeProcessId, RuntimeProcessImagePathA, RuntimeStartupInfo, RuntimeState, RuntimeThreadId,
    X86_STARTUP_INFO_W_BYTE_LENGTH, direct_sound_create_8_contract, dispatch_dsound_api,
    dispatch_dsound_com_call, dsound_contract_for_endpoint,
};

const MEMORY_BASE: GuestAddress = GuestAddress(0x2000_0000);
const MEMORY_BYTES: u64 = 0x3000;
const STACK: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x100);
const COM_STACK: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x180);
const DEVICE_GUID: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x400);
const OUTPUT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x420);
const BUFFER_DESCRIPTOR: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x500);
const BUFFER_FORMAT: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x540);
const IAT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2000);
const ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0010);
const RETURN_ADDRESS: GuestAddress = GuestAddress(0x1009_ccd8);
const SET_COOPERATIVE_CALL_SITE: GuestAddress = GuestAddress(0x1009_cd0e);
const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;
const LAST_ERROR_SENTINEL: u32 = 0x1234_5678;
const OUTPUT_SENTINEL: u32 = 0xcccc_cccc;

#[test]
fn direct_sound_create_8_installs_one_owned_root_and_audio_lifetime() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    let completion = immediate(fixture.dispatch_factory(0, OUTPUT_ADDRESS.0, 0));
    assert_eq!(completion.eax, RegisterUpdate::Set(0));
    assert_eq!(completion.stack_pointer, GuestAddress(STACK.0 + 16));
    assert_eq!(completion.last_error, LAST_ERROR_SENTINEL);
    assert_eq!(completion.guest_writes.len(), 1);
    fixture.acknowledge();

    let pointer = GuestAddress(read_u32(&fixture.memory, OUTPUT_ADDRESS));
    assert_ne!(pointer.0, 0);
    let object = fixture
        .runtime
        .com()
        .objects()
        .values()
        .next()
        .expect("DirectSound root object")
        .clone();
    assert_eq!(object.class_identity, DSOUND_IDIRECTSOUND8_CLASS_IDENTITY);
    assert_eq!(object.reference_count, 1);
    let interface = fixture
        .runtime
        .com_interface_for_pointer(pointer)
        .expect("IDirectSound8 pointer")
        .clone();
    assert_eq!(interface.iid, DSOUND_IDIRECTSOUND8_IID);
    assert_eq!(interface.name, "IDirectSound8");
    assert_eq!(interface.methods.len(), 12);
    assert_eq!(interface.methods[&3].name, "CreateSoundBuffer");
    assert_eq!(interface.methods[&3].explicit_argument_words, 3);
    assert_eq!(
        interface.methods[&3].implementation,
        RuntimeComMethodImplementation::Frontend
    );
    assert_eq!(interface.methods[&6].name, "SetCooperativeLevel");
    assert_eq!(interface.methods[&6].explicit_argument_words, 2);
    assert_eq!(
        interface.methods[&6].return_kind,
        RuntimeComReturnKind::HResult
    );
    assert_eq!(
        interface.methods[&6].implementation,
        RuntimeComMethodImplementation::Frontend
    );
    let device = fixture
        .runtime
        .audio_devices()
        .values()
        .next()
        .expect("logical DirectSound device");
    assert_eq!(fixture.runtime.audio_devices().len(), 1);
    assert_eq!(device.com_object_id, object.id);
    assert_eq!(device.cooperative, None);

    write_words(
        &mut fixture.memory,
        COM_STACK,
        &[RETURN_ADDRESS.0, pointer.0],
    );
    acknowledge_pending(&mut fixture.memory);
    let backing = object.backing.range;
    let release = fixture.dispatch_com(&interface, 2);
    let ComDispatchOutcome::Completed(release) = release else {
        panic!("Release did not complete: {release:?}");
    };
    assert_eq!(release.return_value.eax(), 0);
    assert!(release.object_retired);
    assert!(fixture.runtime.com().is_empty());
    assert!(fixture.runtime.audio_devices().is_empty());
    assert!(fixture.memory.read(backing.base, 4).is_err());
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
}

#[test]
fn buffer_oracle_argument_failures_preserve_output_and_authorities() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    immediate(fixture.dispatch_factory(0, OUTPUT_ADDRESS.0, 0));
    fixture.acknowledge();
    let root_pointer = GuestAddress(read_u32(&fixture.memory, OUTPUT_ADDRESS));
    let root_interface = fixture
        .runtime
        .com_interface_for_pointer(root_pointer)
        .expect("IDirectSound8 pointer")
        .clone();
    let policy = DsoundCompatibilityPolicy::controlled_v1();

    let invalid_descriptors = [
        [
            DSOUND_DSBUFFERDESC_BYTES - 1,
            DSOUND_DSBCAPS_PRIMARYBUFFER,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ],
        [
            DSOUND_DSBUFFERDESC_BYTES,
            DSOUND_DSBCAPS_PRIMARYBUFFER,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
        ],
        [
            DSOUND_DSBUFFERDESC_BYTES,
            DSOUND_DSBCAPS_PRIMARYBUFFER,
            0,
            0,
            BUFFER_FORMAT.0,
            0,
            0,
            0,
            0,
        ],
        [DSOUND_DSBUFFERDESC_BYTES, 0x80a8, 0, 0, 0, 0, 0, 0, 0],
    ];
    for descriptor in invalid_descriptors {
        write_words(&mut fixture.memory, BUFFER_DESCRIPTOR, &descriptor);
        write_words(&mut fixture.memory, OUTPUT_ADDRESS, &[OUTPUT_SENTINEL]);
        write_words(
            &mut fixture.memory,
            COM_STACK,
            &[
                SET_COOPERATIVE_CALL_SITE.0,
                root_pointer.0,
                BUFFER_DESCRIPTOR.0,
                OUTPUT_ADDRESS.0,
                0,
            ],
        );
        fixture.acknowledge();
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let completion = match fixture.dispatch_com_with_policy(&policy, &root_interface, 3) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("invalid descriptor did not return E_INVALIDARG: {other:?}"),
        };
        assert_eq!(completion.return_value.eax(), DSOUND_E_INVALIDARG.bits());
        assert!(completion.guest_writes.is_empty());
        assert_eq!(read_u32(&fixture.memory, OUTPUT_ADDRESS), OUTPUT_SENTINEL);
        assert_eq!(fixture.memory.stats(), before_memory);
        assert_eq!(fixture.runtime, before_runtime);
    }

    for (descriptor, output, outer, expected) in [
        (0, OUTPUT_ADDRESS.0, 0, DSOUND_E_INVALIDARG.bits()),
        (BUFFER_DESCRIPTOR.0, 0, 0, DSOUND_E_INVALIDARG.bits()),
        (
            BUFFER_DESCRIPTOR.0,
            OUTPUT_ADDRESS.0,
            0x2000_0100,
            DSOUND_CLASS_E_NOAGGREGATION.bits(),
        ),
    ] {
        write_words(&mut fixture.memory, OUTPUT_ADDRESS, &[OUTPUT_SENTINEL]);
        write_words(
            &mut fixture.memory,
            COM_STACK,
            &[
                SET_COOPERATIVE_CALL_SITE.0,
                root_pointer.0,
                descriptor,
                output,
                outer,
            ],
        );
        fixture.acknowledge();
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let completion = match fixture.dispatch_com_with_policy(&policy, &root_interface, 3) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("invalid CreateSoundBuffer arguments did not complete: {other:?}"),
        };
        assert_eq!(completion.return_value.eax(), expected);
        assert_eq!(read_u32(&fixture.memory, OUTPUT_ADDRESS), OUTPUT_SENTINEL);
        assert_eq!(fixture.memory.stats(), before_memory);
        assert_eq!(fixture.runtime, before_runtime);
    }
}

#[test]
fn factory_oracle_failures_preserve_output_and_all_authorities() {
    for (device_guid, output, outer, expected) in [
        (0, 0, 0, Some(DSOUND_E_INVALIDARG.bits())),
        (
            0,
            OUTPUT_ADDRESS.0,
            0x2000_0100,
            Some(DSOUND_CLASS_E_NOAGGREGATION.bits()),
        ),
        (DEVICE_GUID.0, OUTPUT_ADDRESS.0, 0, None),
    ] {
        let mut fixture = Fixture::new(MemoryLimits::default());
        let before_runtime = fixture.runtime.clone();
        let before_memory = fixture.memory.stats();
        let outcome = fixture.dispatch_factory(device_guid, output, outer);
        match expected {
            Some(expected) => {
                let completion = immediate(outcome);
                assert_eq!(completion.eax, RegisterUpdate::Set(expected));
                assert!(completion.guest_writes.is_empty());
            }
            None => {
                let DispatchOutcome::Fault(fault) = outcome else {
                    panic!("non-default device selection did not fault: {outcome:?}");
                };
                assert_eq!(fault.class, ApiFaultClass::Unsupported);
                assert_eq!(fault.code, "dsound-device-guid-unsupported");
            }
        }
        fixture.acknowledge();
        assert_eq!(read_u32(&fixture.memory, OUTPUT_ADDRESS), OUTPUT_SENTINEL);
        assert_eq!(fixture.runtime, before_runtime);
        assert_eq!(
            fixture.memory.stats().committed_pages,
            before_memory.committed_pages
        );
        assert!(fixture.runtime.com().is_empty());
        assert!(fixture.runtime.audio_devices().is_empty());
    }
}

#[test]
fn factory_resource_exhaustion_rolls_back_guest_com_and_audio_state() {
    let limits = MemoryLimits {
        max_physical_pages: 3,
        ..MemoryLimits::default()
    };
    let mut fixture = Fixture::new(limits);
    fixture.prepare_factory_call(0, OUTPUT_ADDRESS.0, 0);
    let before_memory = fixture.memory.stats();
    let before_runtime = fixture.runtime.clone();
    let outcome = fixture.dispatch_prepared();
    let DispatchOutcome::Fault(fault) = outcome else {
        panic!("exhausted DirectSound factory did not fault: {outcome:?}");
    };
    assert_eq!(fault.class, ApiFaultClass::ResourceLimit);
    assert_eq!(read_u32(&fixture.memory, OUTPUT_ADDRESS), OUTPUT_SENTINEL);
    assert_eq!(fixture.memory.stats(), before_memory);
    assert_eq!(fixture.runtime, before_runtime);
    assert!(fixture.runtime.com().is_empty());
    assert!(fixture.runtime.audio_devices().is_empty());
}

#[test]
fn cooperative_level_rejects_invalid_or_unreached_relations_atomically() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    immediate(fixture.dispatch_factory(0, OUTPUT_ADDRESS.0, 0));
    fixture.acknowledge();
    let pointer = GuestAddress(read_u32(&fixture.memory, OUTPUT_ADDRESS));
    let interface = fixture
        .runtime
        .com_interface_for_pointer(pointer)
        .expect("IDirectSound8 pointer")
        .clone();
    let policy = DsoundCompatibilityPolicy::controlled_v1();

    for (window, level) in [(0, DSOUND_DSSCL_PRIORITY), (0x4801_0001, 0)] {
        fixture.prepare_cooperative_call(pointer, window, level);
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let completion = match fixture.dispatch_com_with_policy(&policy, &interface, 6) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("invalid cooperative arguments did not return HRESULT: {other:?}"),
        };
        assert_eq!(completion.return_value.eax(), DSOUND_E_INVALIDARG.bits());
        assert!(completion.guest_writes.is_empty());
        assert_eq!(fixture.memory.stats(), before_memory);
        assert_eq!(fixture.runtime, before_runtime);
    }

    fixture.prepare_cooperative_call(pointer, 0x4801_0001, DSOUND_DSSCL_PRIORITY);
    let before_memory = fixture.memory.stats();
    let before_runtime = fixture.runtime.clone();
    let completion = match fixture.dispatch_com_with_policy(&policy, &interface, 6) {
        ComDispatchOutcome::Completed(completion) => completion,
        other => panic!("unknown logical HWND did not return HRESULT: {other:?}"),
    };
    assert_eq!(completion.return_value.eax(), DSOUND_E_INVALIDARG.bits());
    assert_eq!(fixture.memory.stats(), before_memory);
    assert_eq!(fixture.runtime, before_runtime);

    fixture.prepare_cooperative_call(pointer, 0x4801_0001, DSOUND_DSSCL_NORMAL);
    let before_memory = fixture.memory.stats();
    let before_runtime = fixture.runtime.clone();
    let fault = match fixture.dispatch_com_with_policy(&policy, &interface, 6) {
        ComDispatchOutcome::Fault(fault) => fault,
        other => panic!("unreached cooperative variant did not fault: {other:?}"),
    };
    assert_eq!(fault.class, ComFaultClass::Unsupported);
    assert_eq!(fault.code, "dsound-cooperative-level-variant-unsupported");
    assert_eq!(fixture.memory.stats(), before_memory);
    assert_eq!(fixture.runtime, before_runtime);
}

#[test]
fn factory_catalog_fixes_the_ordinal_three_word_hresult_contract() {
    let fixture = Fixture::new(MemoryLimits::default());
    let contract = direct_sound_create_8_contract(fixture.endpoint.clone()).unwrap();
    assert_eq!(contract.abi.argument_bytes().unwrap(), 12);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::HResult);
    assert_eq!(contract.abi.arguments.len(), 3);
    assert!(matches!(
        contract.abi.arguments[0].value,
        ApiValueSchema::Pointer { .. }
    ));
    assert!(matches!(
        contract.abi.arguments[1].value,
        ApiValueSchema::Pointer { .. }
    ));
    assert!(matches!(
        contract.abi.arguments[2].value,
        ApiValueSchema::OpaquePointer {
            nullable: true,
            alignment: 4
        }
    ));
    assert!(
        dsound_contract_for_endpoint(fixture.endpoint.clone())
            .unwrap()
            .is_some()
    );

    let mut named = fixture.endpoint.clone();
    named.symbol = vine_core::ApiSymbol::Name {
        name: "DirectSoundCreate8".to_owned(),
    };
    assert!(dsound_contract_for_endpoint(named).unwrap().is_none());
    let mut other_ordinal = fixture.endpoint;
    other_ordinal.symbol = vine_core::ApiSymbol::Ordinal { ordinal: 12 };
    assert!(
        dsound_contract_for_endpoint(other_ordinal)
            .unwrap()
            .is_none()
    );
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
            .expect("DirectSound IAT initialization");
        memory.seal_owner(owner).expect("seal fixture owner");
        let reflection = memory.pending_reflection().expect("initial reflection");
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .expect("acknowledge initial reflection");
        let endpoint = endpoint(owner);
        let contract = direct_sound_create_8_contract(endpoint.clone()).expect("factory contract");
        let dispatcher =
            ApiDispatcher::new([endpoint.clone()], [contract]).expect("DirectSound dispatcher");
        Self {
            memory,
            runtime: controlled_runtime(),
            dispatcher,
            endpoint,
        }
    }

    fn prepare_factory_call(&mut self, device_guid: u32, output: u32, outer_unknown: u32) {
        self.memory
            .write(OUTPUT_ADDRESS, &OUTPUT_SENTINEL.to_le_bytes())
            .expect("write output sentinel");
        write_words(
            &mut self.memory,
            STACK,
            &[RETURN_ADDRESS.0, device_guid, output, outer_unknown],
        );
        acknowledge_pending(&mut self.memory);
    }

    fn dispatch_factory(
        &mut self,
        device_guid: u32,
        output: u32,
        outer_unknown: u32,
    ) -> DispatchOutcome {
        self.prepare_factory_call(device_guid, output, outer_unknown);
        self.dispatch_prepared()
    }

    fn dispatch_prepared(&mut self) -> DispatchOutcome {
        let trap = ApiTrap {
            endpoint_id: self.endpoint.id,
            endpoint_address: self.endpoint.address,
            stack_pointer: STACK,
            return_address: RETURN_ADDRESS,
            argument_bytes: 12,
        };
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        self.dispatcher
            .dispatch(trap, &mut session, dispatch_dsound_api)
    }

    fn prepare_cooperative_call(&mut self, this: GuestAddress, window: u32, level: u32) {
        write_words(
            &mut self.memory,
            COM_STACK,
            &[SET_COOPERATIVE_CALL_SITE.0, this.0, window, level],
        );
        acknowledge_pending(&mut self.memory);
    }

    fn dispatch_com(
        &mut self,
        interface: &vine_core::RuntimeComInterfaceRecord,
        slot: u16,
    ) -> ComDispatchOutcome {
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        session.dispatch_com_call(ComCallTrap {
            endpoint_address: interface.methods[&slot].endpoint,
            stack_pointer: COM_STACK,
            return_address: RETURN_ADDRESS,
            call_site: RETURN_ADDRESS,
        })
    }

    fn dispatch_com_with_policy(
        &mut self,
        policy: &DsoundCompatibilityPolicy,
        interface: &vine_core::RuntimeComInterfaceRecord,
        slot: u16,
    ) -> ComDispatchOutcome {
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        dispatch_dsound_com_call(
            policy,
            ComCallTrap {
                endpoint_address: interface.methods[&slot].endpoint,
                stack_pointer: COM_STACK,
                return_address: SET_COOPERATIVE_CALL_SITE,
                call_site: SET_COOPERATIVE_CALL_SITE,
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
        module: "dsound.dll".to_owned(),
        iat_rva: vine_core::Rva(0x000d_1064),
        iat_address: IAT_ADDRESS,
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Ordinal {
            ordinal: DSOUND_DIRECT_SOUND_CREATE_8_ORDINAL,
        },
    })
}

fn immediate(outcome: DispatchOutcome) -> vine_core::ApiImmediateCompletion {
    match outcome {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("factory did not complete immediately: {other:?}"),
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
            profile_id: "dsound-root-fixture".to_owned(),
            bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
        },
        process_image_path_a: RuntimeProcessImagePathA {
            source_authority: "dsound-root-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\\dsound-root.exe\0".to_vec(),
            encoding: "windows-932-v1".to_owned(),
        },
        initial_current_directory_a: RuntimeCurrentDirectoryA {
            source_authority: "dsound-root-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: "windows-932-v1".to_owned(),
        },
        encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec01_0000,
    })
    .expect("controlled DirectSound runtime");
    runtime
        .start_initial_thread()
        .expect("start DirectSound fixture thread");
    runtime
}

use vine_core::{
    ApiDispatcher, ApiEndpointIdentity, ApiFaultClass, ApiTrap, COM_S_OK, ComCallCompletion,
    ComCallTrap, ComDispatchOutcome, ComFaultClass, ComReturnValue,
    D3D9_D3DDISPLAYMODE_BYTE_LENGTH, D3D9_D3DERR_INVALIDCALL, D3D9_D3DFMT_X8R8G8B8,
    D3D9_IDIRECT3D9_CLASS_IDENTITY, D3D9_IDIRECT3D9_IID, D3D9_SDK_VERSION, D3d9CompatibilityPolicy,
    DispatchOutcome, ExecutionSession, GuestAddress, GuestMemory, ImportTarget, ImportThunk,
    ImportThunkDisposition, ImportThunkId, LOGICAL_ENCODED_POINTER_NAMESPACE, MemoryLimits,
    MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind, PageProtection, ProcessAddressSpaceLayout,
    RegisterUpdate, RuntimeBootstrap, RuntimeClockState, RuntimeComMethodImplementation,
    RuntimeComReturnKind, RuntimeCurrentDirectoryA, RuntimeDisplayProfile, RuntimeProcessId,
    RuntimeProcessImagePathA, RuntimeRect, RuntimeStartupInfo, RuntimeState, RuntimeThreadId,
    X86_STARTUP_INFO_W_BYTE_LENGTH, d3d9_contract_for_endpoint, direct3d_create9_contract,
    dispatch_d3d9_api, dispatch_d3d9_com_call,
};

const MEMORY_BASE: GuestAddress = GuestAddress(0x2000_0000);
const MEMORY_BYTES: u64 = 0x3000;
const IAT_ADDRESS: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2000);
const ENDPOINT_ADDRESS: GuestAddress = GuestAddress(0x7f00_0000);
const RETURN_ADDRESS: GuestAddress = GuestAddress(0x1005_b892);
const STACK_INVALID: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x100);
const STACK_VALID: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x120);
const STACK_COM: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x180);
const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;
const LAST_ERROR_SENTINEL: u32 = 0x1234_5678;

#[test]
fn direct3d_create9_installs_bounded_root_identity_and_retires_atomically() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    write_words(&mut fixture.memory, STACK_INVALID, &[RETURN_ADDRESS.0, 0]);
    acknowledge_pending(&mut fixture.memory);
    let before_invalid_memory = fixture.memory.stats();
    let before_invalid_runtime = fixture.runtime.clone();
    let invalid = match fixture.dispatch(STACK_INVALID) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("invalid SDK did not return NULL: {other:?}"),
    };
    assert_eq!(invalid.eax, RegisterUpdate::Set(0));
    assert_eq!(invalid.last_error, LAST_ERROR_SENTINEL);
    assert_eq!(fixture.memory.stats(), before_invalid_memory);
    assert_eq!(fixture.runtime, before_invalid_runtime);

    let first = fixture.dispatch_factory(STACK_VALID, D3D9_SDK_VERSION);
    let pointer = match first.eax {
        RegisterUpdate::Set(pointer) => pointer,
        other => panic!("Direct3DCreate9 did not set EAX: {other:?}"),
    };
    assert_ne!(pointer, 0);
    assert_eq!(first.stack_pointer, GuestAddress(STACK_VALID.0 + 8));
    assert_eq!(first.last_error, LAST_ERROR_SENTINEL);
    assert!(first.guest_writes.is_empty());
    let object = fixture
        .runtime
        .com()
        .objects()
        .values()
        .next()
        .expect("D3D9 root COM object")
        .clone();
    assert_eq!(object.class_identity, D3D9_IDIRECT3D9_CLASS_IDENTITY);
    assert_eq!(object.reference_count, 1);
    let interface = fixture
        .runtime
        .com_interface_for_pointer(GuestAddress(pointer))
        .expect("IDirect3D9 pointer")
        .clone();
    assert_eq!(interface.iid, D3D9_IDIRECT3D9_IID);
    assert_eq!(interface.name, "IDirect3D9");
    assert_eq!(interface.methods.len(), 17);
    assert_eq!(interface.methods[&8].name, "GetAdapterDisplayMode");
    assert_eq!(interface.methods[&8].explicit_argument_words, 2);
    assert_eq!(
        interface.methods[&8].return_kind,
        RuntimeComReturnKind::HResult
    );
    assert_eq!(
        interface.methods[&8].implementation,
        RuntimeComMethodImplementation::Frontend
    );
    assert_eq!(interface.methods[&16].name, "CreateDevice");
    assert_eq!(interface.methods[&16].explicit_argument_words, 6);
    fixture.acknowledge();
    assert_eq!(
        u32::from_le_bytes(
            fixture
                .memory
                .read(interface.guest_pointer, 4)
                .expect("IDirect3D9 interface pointer")
                .try_into()
                .unwrap()
        ),
        interface.vtable.0
    );
    assert_eq!(
        fixture
            .memory
            .read(interface.methods[&8].endpoint, 10)
            .expect("IDirect3D9 slot 8 endpoint"),
        [0x0f, 0x0b, b'V', b'C', b'O', b'M', 1, 0, 8, 0]
    );
    write_words(
        &mut fixture.memory,
        STACK_COM,
        &[0x1005_b8d6, pointer, 0, MEMORY_BASE.0 + 0x280],
    );
    acknowledge_pending(&mut fixture.memory);
    let before_unsupported_memory = fixture.memory.stats();
    let before_unsupported_runtime = fixture.runtime.clone();
    let unsupported = faulted(fixture.dispatch_com(&interface, 8, STACK_COM));
    assert_eq!(unsupported.class, ComFaultClass::Unsupported);
    assert_eq!(unsupported.code, "com-method-unsupported");
    assert_eq!(unsupported.call_site, GuestAddress(0x1005_b8d6));
    let target = unsupported.target.expect("typed IDirect3D9 fault target");
    assert_eq!(target.object_id, object.id);
    assert_eq!(target.interface_id, interface.id);
    assert_eq!(target.interface, "IDirect3D9");
    assert_eq!(target.vtable_slot, 8);
    assert_eq!(target.method, "GetAdapterDisplayMode");
    assert_eq!(fixture.memory.stats(), before_unsupported_memory);
    assert_eq!(fixture.runtime, before_unsupported_runtime);

    write_words(&mut fixture.memory, STACK_COM, &[0x1000_1000, pointer]);
    acknowledge_pending(&mut fixture.memory);
    let add_ref = completed(fixture.dispatch_com(&interface, 1, STACK_COM));
    assert_eq!(add_ref.return_value, ComReturnValue::U32(2));
    let first_release = completed(fixture.dispatch_com(&interface, 2, STACK_COM));
    assert_eq!(first_release.return_value, ComReturnValue::U32(1));
    assert!(!first_release.object_retired);
    let final_release = completed(fixture.dispatch_com(&interface, 2, STACK_COM));
    assert_eq!(final_release.return_value, ComReturnValue::U32(0));
    assert!(final_release.object_retired);
    assert!(fixture.runtime.com().is_empty());
    assert!(
        fixture
            .runtime
            .process_address_space_allocator()
            .allocation(object.backing.address_allocation_id)
            .is_err()
    );
    assert!(fixture.memory.read(interface.guest_pointer, 4).is_err());
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
    fixture.acknowledge();

    let second = fixture.dispatch_factory(STACK_VALID, D3D9_SDK_VERSION);
    assert_eq!(second.eax, RegisterUpdate::Set(pointer));
    let second_object = fixture
        .runtime
        .com()
        .objects()
        .values()
        .next()
        .expect("replacement D3D9 root");
    assert_ne!(second_object.id, object.id);
    assert_eq!(second_object.reference_count, 1);
}

#[test]
fn get_adapter_display_mode_reads_logical_geometry_and_writes_one_atomic_mode() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    let factory = fixture.dispatch_factory(STACK_VALID, D3D9_SDK_VERSION);
    let pointer = match factory.eax {
        RegisterUpdate::Set(pointer) => pointer,
        other => panic!("Direct3DCreate9 did not return an interface: {other:?}"),
    };
    let interface = fixture
        .runtime
        .com_interface_for_pointer(GuestAddress(pointer))
        .expect("IDirect3D9 interface")
        .clone();
    fixture.acknowledge();

    let output = GuestAddress(MEMORY_BASE.0 + 0x280);
    fixture
        .memory
        .write(output, &[0xcc; D3D9_D3DDISPLAYMODE_BYTE_LENGTH as usize])
        .expect("display-mode sentinel");
    write_words(
        &mut fixture.memory,
        STACK_COM,
        &[0x1005_b8d6, pointer, 0, output.0],
    );
    acknowledge_pending(&mut fixture.memory);
    let before_runtime = fixture.runtime.clone();
    let before_reference_count = fixture
        .runtime
        .com()
        .objects()
        .values()
        .next()
        .expect("D3D9 root object")
        .reference_count;
    let completion = completed(fixture.dispatch_d3d9_com(
        &D3d9CompatibilityPolicy::controlled_v1(),
        &interface,
        8,
        STACK_COM,
    ));
    let expected = [
        1920_u32.to_le_bytes(),
        1080_u32.to_le_bytes(),
        60_u32.to_le_bytes(),
        D3D9_D3DFMT_X8R8G8B8.to_le_bytes(),
    ]
    .concat();
    assert_eq!(completion.return_value, ComReturnValue::HResult(COM_S_OK));
    assert_eq!(completion.stack_pointer, GuestAddress(STACK_COM.0 + 16));
    assert_eq!(completion.reference_count, before_reference_count);
    assert!(!completion.object_retired);
    assert_eq!(completion.guest_writes.len(), 1);
    assert_eq!(completion.guest_writes[0].address, output);
    assert_eq!(completion.guest_writes[0].bytes, expected);
    fixture.acknowledge();
    assert_eq!(fixture.memory.read(output, 16).unwrap(), expected);
    assert_eq!(fixture.runtime, before_runtime);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
}

#[test]
fn get_adapter_display_mode_negative_paths_preserve_memory_and_authority() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    let factory = fixture.dispatch_factory(STACK_VALID, D3D9_SDK_VERSION);
    let pointer = match factory.eax {
        RegisterUpdate::Set(pointer) => pointer,
        other => panic!("Direct3DCreate9 did not return an interface: {other:?}"),
    };
    let interface = fixture
        .runtime
        .com_interface_for_pointer(GuestAddress(pointer))
        .expect("IDirect3D9 interface")
        .clone();
    fixture.acknowledge();
    let output = GuestAddress(MEMORY_BASE.0 + 0x280);
    let policy = D3d9CompatibilityPolicy::controlled_v1();
    assert!(policy.validate().is_ok());

    for (adapter, output_pointer) in [(1, output.0), (0, 0)] {
        fixture
            .memory
            .write(output, &[0xcc; D3D9_D3DDISPLAYMODE_BYTE_LENGTH as usize])
            .expect("negative display-mode sentinel");
        write_words(
            &mut fixture.memory,
            STACK_COM,
            &[0x1005_b8d6, pointer, adapter, output_pointer],
        );
        acknowledge_pending(&mut fixture.memory);
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let completion = completed(fixture.dispatch_d3d9_com(&policy, &interface, 8, STACK_COM));
        assert_eq!(
            completion.return_value,
            ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL)
        );
        assert!(completion.guest_writes.is_empty());
        assert_eq!(fixture.memory.stats(), before_memory);
        assert_eq!(fixture.runtime, before_runtime);
        assert_eq!(
            fixture.memory.read(output, 16).unwrap(),
            [0xcc; D3D9_D3DDISPLAYMODE_BYTE_LENGTH as usize]
        );
    }

    write_words(
        &mut fixture.memory,
        STACK_COM,
        &[0x1005_b8d6, pointer, 0, interface.guest_pointer.0],
    );
    acknowledge_pending(&mut fixture.memory);
    let before_memory = fixture.memory.stats();
    let before_runtime = fixture.runtime.clone();
    let unwritable = faulted(fixture.dispatch_d3d9_com(&policy, &interface, 8, STACK_COM));
    assert_eq!(unwritable.class, ComFaultClass::InvalidCall);
    assert_eq!(fixture.memory.stats(), before_memory);
    assert_eq!(fixture.runtime, before_runtime);

    for rejected_policy in {
        let mut disabled = policy.clone();
        disabled.get_adapter_display_mode = None;
        let mut mismatched = policy.clone();
        mismatched
            .get_adapter_display_mode
            .as_mut()
            .unwrap()
            .display_profile_id = "single-monitor-mismatch-v1".to_owned();
        let mut malformed = policy.clone();
        malformed.schema_version += 1;
        [disabled, mismatched, malformed]
    } {
        write_words(
            &mut fixture.memory,
            STACK_COM,
            &[0x1005_b8d6, pointer, 0, output.0],
        );
        acknowledge_pending(&mut fixture.memory);
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let fault = faulted(fixture.dispatch_d3d9_com(&rejected_policy, &interface, 8, STACK_COM));
        assert!(matches!(
            fault.class,
            ComFaultClass::Unsupported | ComFaultClass::RuntimeDefect
        ));
        assert_eq!(fixture.memory.stats(), before_memory);
        assert_eq!(fixture.runtime, before_runtime);
    }

    let mut missing_display = Fixture::new_without_display(MemoryLimits::default());
    let factory = missing_display.dispatch_factory(STACK_VALID, D3D9_SDK_VERSION);
    let missing_pointer = match factory.eax {
        RegisterUpdate::Set(pointer) => pointer,
        other => panic!("Direct3DCreate9 did not return an interface: {other:?}"),
    };
    let missing_interface = missing_display
        .runtime
        .com_interface_for_pointer(GuestAddress(missing_pointer))
        .unwrap()
        .clone();
    missing_display.acknowledge();
    write_words(
        &mut missing_display.memory,
        STACK_COM,
        &[0x1005_b8d6, missing_pointer, 0, output.0],
    );
    acknowledge_pending(&mut missing_display.memory);
    let before_memory = missing_display.memory.stats();
    let before_runtime = missing_display.runtime.clone();
    let missing =
        faulted(missing_display.dispatch_d3d9_com(&policy, &missing_interface, 8, STACK_COM));
    assert_eq!(missing.class, ComFaultClass::RuntimeDefect);
    assert_eq!(missing.code, "logical-primary-monitor-missing");
    assert_eq!(missing_display.memory.stats(), before_memory);
    assert_eq!(missing_display.runtime, before_runtime);
}

#[test]
fn direct3d_create9_contract_and_resource_exhaustion_fail_before_publication() {
    let endpoint = endpoint(MemoryOwnerId(1));
    let contract = direct3d_create9_contract(endpoint.clone()).expect("D3D9 contract");
    assert_eq!(contract.abi.arguments.len(), 1);
    assert_eq!(contract.abi.argument_bytes().unwrap(), 4);
    assert_eq!(
        contract.abi.return_value,
        vine_core::ApiReturnSchema::OpaquePointer { nullable: true }
    );
    assert_eq!(
        d3d9_contract_for_endpoint(endpoint.clone())
            .unwrap()
            .unwrap(),
        contract
    );

    let mut fixture = Fixture::new(MemoryLimits {
        max_physical_pages: 3,
        ..MemoryLimits::default()
    });
    write_words(
        &mut fixture.memory,
        STACK_VALID,
        &[RETURN_ADDRESS.0, D3D9_SDK_VERSION],
    );
    acknowledge_pending(&mut fixture.memory);
    let before_memory = fixture.memory.stats();
    let before_runtime = fixture.runtime.clone();
    let outcome = fixture.dispatch(STACK_VALID);
    let fault = match outcome {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("resource exhaustion unexpectedly completed: {other:?}"),
    };
    assert_eq!(fault.class, ApiFaultClass::ResourceLimit);
    assert_eq!(fault.stage, "physical-allocator");
    assert_eq!(fixture.memory.stats(), before_memory);
    assert_eq!(fixture.runtime, before_runtime);
}

struct Fixture {
    memory: GuestMemory,
    runtime: RuntimeState,
    dispatcher: ApiDispatcher,
    endpoint: ApiEndpointIdentity,
}

impl Fixture {
    fn new(limits: MemoryLimits) -> Self {
        Self::new_with_display(limits, true)
    }

    fn new_without_display(limits: MemoryLimits) -> Self {
        Self::new_with_display(limits, false)
    }

    fn new_with_display(limits: MemoryLimits, configure_display: bool) -> Self {
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
            .expect("D3D9 IAT initialization");
        memory.seal_owner(owner).expect("seal fixture owner");
        let reflection = memory.pending_reflection().expect("initial reflection");
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .expect("acknowledge fixture memory");
        let endpoint = endpoint(owner);
        let contract = direct3d_create9_contract(endpoint.clone()).expect("D3D9 contract");
        let dispatcher =
            ApiDispatcher::new([endpoint.clone()], [contract]).expect("D3D9 dispatcher");
        Self {
            memory,
            runtime: controlled_runtime(configure_display),
            dispatcher,
            endpoint,
        }
    }

    fn dispatch_factory(
        &mut self,
        stack_pointer: GuestAddress,
        sdk_version: u32,
    ) -> vine_core::ApiImmediateCompletion {
        write_words(
            &mut self.memory,
            stack_pointer,
            &[RETURN_ADDRESS.0, sdk_version],
        );
        acknowledge_pending(&mut self.memory);
        match self.dispatch(stack_pointer) {
            DispatchOutcome::Immediate(completion) => completion,
            DispatchOutcome::Fault(fault) => panic!("unexpected D3D9 fault: {fault:?}"),
            other => panic!("unexpected D3D9 outcome: {other:?}"),
        }
    }

    fn dispatch(&mut self, stack_pointer: GuestAddress) -> DispatchOutcome {
        let trap = ApiTrap {
            endpoint_id: self.endpoint.id,
            endpoint_address: self.endpoint.address,
            stack_pointer,
            return_address: RETURN_ADDRESS,
            argument_bytes: 4,
        };
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        self.dispatcher
            .dispatch(trap, &mut session, dispatch_d3d9_api)
    }

    fn dispatch_com(
        &mut self,
        interface: &vine_core::RuntimeComInterfaceRecord,
        slot: u16,
        stack_pointer: GuestAddress,
    ) -> ComDispatchOutcome {
        let call_site = GuestAddress(u32::from_le_bytes(
            self.memory
                .read(stack_pointer, 4)
                .expect("COM return address")
                .try_into()
                .unwrap(),
        ));
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        session.dispatch_com_call(ComCallTrap {
            endpoint_address: interface.methods[&slot].endpoint,
            stack_pointer,
            return_address: call_site,
            call_site,
        })
    }

    fn dispatch_d3d9_com(
        &mut self,
        policy: &D3d9CompatibilityPolicy,
        interface: &vine_core::RuntimeComInterfaceRecord,
        slot: u16,
        stack_pointer: GuestAddress,
    ) -> ComDispatchOutcome {
        let call_site = GuestAddress(u32::from_le_bytes(
            self.memory
                .read(stack_pointer, 4)
                .expect("COM return address")
                .try_into()
                .unwrap(),
        ));
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        dispatch_d3d9_com_call(
            policy,
            ComCallTrap {
                endpoint_address: interface.methods[&slot].endpoint,
                stack_pointer,
                return_address: call_site,
                call_site,
            },
            &mut session,
        )
    }

    fn acknowledge(&mut self) {
        let delta = self.memory.pending_delta().expect("pending D3D9 delta");
        self.memory
            .acknowledge_delta(delta.generation)
            .expect("acknowledge D3D9 delta");
    }
}

fn endpoint(owner: MemoryOwnerId) -> ApiEndpointIdentity {
    ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(1),
        address: ENDPOINT_ADDRESS,
        requester_owner: owner,
        module: "d3d9.dll".to_owned(),
        iat_rva: vine_core::Rva(0x000d_14f4),
        iat_address: IAT_ADDRESS,
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "Direct3DCreate9".to_owned(),
            hint: 0,
            name_rva: vine_core::Rva(0),
        },
    })
}

fn completed(outcome: ComDispatchOutcome) -> ComCallCompletion {
    match outcome {
        ComDispatchOutcome::Completed(completion) => completion,
        ComDispatchOutcome::Fault(fault) => panic!("unexpected COM fault: {fault:?}"),
    }
}

fn faulted(outcome: ComDispatchOutcome) -> vine_core::ComFault {
    match outcome {
        ComDispatchOutcome::Fault(fault) => fault,
        ComDispatchOutcome::Completed(completion) => {
            panic!("unexpected COM completion: {completion:?}")
        }
    }
}

fn write_words(memory: &mut GuestMemory, address: GuestAddress, words: &[u32]) {
    let bytes = words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<_>>();
    memory.write(address, &bytes).expect("write fixture words");
}

fn acknowledge_pending(memory: &mut GuestMemory) {
    if memory.stats().pending_invalidation.is_some() {
        let delta = memory.pending_delta().expect("pending fixture writeback");
        memory
            .acknowledge_delta(delta.generation)
            .expect("acknowledge fixture writeback");
    }
}

fn controlled_runtime(configure_display: bool) -> RuntimeState {
    let mut runtime = RuntimeState::new(RuntimeBootstrap {
        process_id: RuntimeProcessId(PROCESS_ID),
        initial_thread_id: RuntimeThreadId(THREAD_ID),
        initial_last_error: LAST_ERROR_SENTINEL,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "d3d9-root-fixture".to_owned(),
            bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
        },
        process_image_path_a: RuntimeProcessImagePathA {
            source_authority: "d3d9-root-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\\d3d9-root.exe\0".to_vec(),
            encoding: "windows-932-v1".to_owned(),
        },
        initial_current_directory_a: RuntimeCurrentDirectoryA {
            source_authority: "d3d9-root-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: "windows-932-v1".to_owned(),
        },
        encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec01_0000,
    })
    .expect("controlled D3D9 runtime");
    runtime
        .start_initial_thread()
        .expect("start D3D9 fixture thread");
    if configure_display {
        runtime
            .configure_display_profile(RuntimeDisplayProfile {
                id: "single-monitor-1920x1080-v1".to_owned(),
                primary_monitor_rect: RuntimeRect {
                    left: 0,
                    top: 0,
                    right: 1920,
                    bottom: 1080,
                },
            })
            .expect("configure D3D9 fixture display");
    }
    runtime
}

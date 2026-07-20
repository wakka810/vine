use vine_core::{
    ApiDispatcher, ApiEndpointIdentity, ApiFaultClass, ApiTrap, COM_S_OK, ComCallCompletion,
    ComCallTrap, ComDispatchOutcome, ComFaultClass, ComReturnValue,
    D3D9_CREATE_HARDWARE_VERTEX_PROCESSING_MULTITHREADED, D3D9_D3DADAPTER_IDENTIFIER9_BYTE_LENGTH,
    D3D9_D3DCAPS_READ_SCANLINE, D3D9_D3DCAPS2_DYNAMICTEXTURES, D3D9_D3DCAPS9_BYTE_LENGTH,
    D3D9_D3DDEVTYPE_HAL, D3D9_D3DDISPLAYMODE_BYTE_LENGTH, D3D9_D3DENUM_WHQL_LEVEL,
    D3D9_D3DERR_INVALIDCALL, D3D9_D3DERR_NOTAVAILABLE, D3D9_D3DFMT_D16, D3D9_D3DFMT_UNKNOWN,
    D3D9_D3DFMT_X8R8G8B8, D3D9_D3DMULTISAMPLE_NONE, D3D9_D3DPOOL_SYSTEMMEM,
    D3D9_D3DPRESENT_PARAMETERS_BYTE_LENGTH, D3D9_D3DPS_VERSION_3_0, D3D9_D3DRTYPE_TEXTURE,
    D3D9_D3DSWAPEFFECT_DISCARD, D3D9_D3DUSAGE_DYNAMIC, D3D9_IDIRECT3D9_CLASS_IDENTITY,
    D3D9_IDIRECT3D9_IID, D3D9_IDIRECT3DDEVICE9_CLASS_IDENTITY, D3D9_IDIRECT3DDEVICE9_IID,
    D3D9_IDIRECT3DTEXTURE9_CLASS_IDENTITY, D3D9_IDIRECT3DTEXTURE9_IID, D3D9_SDK_VERSION,
    D3D9_VIRTUAL_DESCRIPTION, D3D9_VIRTUAL_DEVICE_IDENTIFIER_WINDOWS_BYTES,
    D3D9_VIRTUAL_DEVICE_NAME, D3D9_VIRTUAL_DRIVER, D3D9_VIRTUAL_DRIVER_VERSION,
    D3d9CompatibilityPolicy, DispatchOutcome, ExecutionSession, GuestAddress, GuestMemory,
    ImportTarget, ImportThunk, ImportThunkDisposition, ImportThunkId,
    LOGICAL_ENCODED_POINTER_NAMESPACE, MemoryLimits, MemoryOwnerId, MemoryOwnerKind,
    MemoryRegionKind, PageProtection, ProcessAddressSpaceLayout, RUNTIME_PRIMARY_MONITOR_HANDLE,
    RegisterUpdate, RuntimeBootstrap, RuntimeClockState, RuntimeComMethodImplementation,
    RuntimeComReturnKind, RuntimeCurrentDirectoryA, RuntimeDisplayProfile, RuntimeProcessId,
    RuntimeProcessImagePathA, RuntimeRect, RuntimeStartupInfo, RuntimeState, RuntimeThreadId,
    X86_STARTUP_INFO_W_BYTE_LENGTH, d3d9_contract_for_endpoint, direct3d_create9_contract,
    direct3ddevice9_object_spec, direct3dtexture9_object_spec, dispatch_d3d9_api,
    dispatch_d3d9_com_call,
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
const UNKNOWN_WINDOW: u32 = 0x4801_0001;

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
    assert_eq!(interface.methods[&4].name, "GetAdapterCount");
    assert_eq!(interface.methods[&4].explicit_argument_words, 0);
    assert_eq!(interface.methods[&4].return_kind, RuntimeComReturnKind::U32);
    assert_eq!(
        interface.methods[&4].implementation,
        RuntimeComMethodImplementation::Frontend
    );
    assert_eq!(interface.methods[&5].name, "GetAdapterIdentifier");
    assert_eq!(interface.methods[&5].explicit_argument_words, 3);
    assert_eq!(
        interface.methods[&5].return_kind,
        RuntimeComReturnKind::HResult
    );
    assert_eq!(interface.methods[&14].name, "GetDeviceCaps");
    assert_eq!(interface.methods[&14].explicit_argument_words, 3);
    assert_eq!(
        interface.methods[&14].implementation,
        RuntimeComMethodImplementation::Frontend
    );
    assert_eq!(interface.methods[&15].name, "GetAdapterMonitor");
    assert_eq!(interface.methods[&15].explicit_argument_words, 1);
    assert_eq!(
        interface.methods[&15].return_kind,
        RuntimeComReturnKind::U32
    );
    assert_eq!(
        interface.methods[&15].implementation,
        RuntimeComMethodImplementation::Frontend
    );
    assert_eq!(interface.methods[&9].name, "CheckDeviceType");
    assert_eq!(interface.methods[&9].explicit_argument_words, 5);
    assert_eq!(
        interface.methods[&9].implementation,
        RuntimeComMethodImplementation::Frontend
    );
    assert_eq!(interface.methods[&10].name, "CheckDeviceFormat");
    assert_eq!(interface.methods[&10].explicit_argument_words, 6);
    assert_eq!(
        interface.methods[&10].implementation,
        RuntimeComMethodImplementation::Frontend
    );
    assert_eq!(
        interface.methods[&5].implementation,
        RuntimeComMethodImplementation::Frontend
    );
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
fn d3d9_device_and_texture_catalogs_fix_the_complete_x86_vtables() {
    let policy = D3d9CompatibilityPolicy::controlled_v9();
    policy.validate().unwrap();
    let create_texture = policy.create_texture.as_ref().unwrap();
    assert_eq!(create_texture.width, 1_024);
    assert_eq!(create_texture.height, 1_024);
    assert_eq!(create_texture.levels, 1);
    assert_eq!(create_texture.usage, 0);
    assert_eq!(create_texture.format, D3D9_D3DFMT_X8R8G8B8);
    assert_eq!(create_texture.pool, D3D9_D3DPOOL_SYSTEMMEM);
    assert!(create_texture.shared_handle_must_be_null);

    let spec = direct3ddevice9_object_spec();
    assert_eq!(spec.class_identity, D3D9_IDIRECT3DDEVICE9_CLASS_IDENTITY);
    assert_eq!(spec.interfaces.len(), 2);
    let interface = &spec.interfaces[1];
    assert_eq!(interface.iid, D3D9_IDIRECT3DDEVICE9_IID);
    assert_eq!(interface.name, "IDirect3DDevice9");
    assert_eq!(interface.methods.len(), 119);
    for (slot, method) in interface.methods.iter().enumerate() {
        assert!(
            !method.name.is_empty(),
            "empty device method at slot {slot}"
        );
    }
    assert_eq!(interface.methods[6].name, "GetDirect3D");
    assert_eq!(interface.methods[6].explicit_argument_words, 1);
    assert_eq!(interface.methods[17].name, "Present");
    assert_eq!(interface.methods[17].explicit_argument_words, 4);
    assert_eq!(interface.methods[23].name, "CreateTexture");
    assert_eq!(interface.methods[23].explicit_argument_words, 8);
    assert_eq!(
        interface.methods[23].implementation,
        RuntimeComMethodImplementation::Frontend
    );
    assert_eq!(interface.methods[118].name, "CreateQuery");
    assert_eq!(interface.methods[118].explicit_argument_words, 2);
    let return_kinds = interface
        .methods
        .iter()
        .enumerate()
        .map(|(slot, method)| (slot, method.return_kind))
        .collect::<Vec<_>>();
    assert_eq!(
        return_kinds
            .iter()
            .filter(|(_, kind)| *kind == RuntimeComReturnKind::U32)
            .map(|(slot, _)| *slot)
            .collect::<Vec<_>>(),
        [1, 2, 4, 12, 15, 78]
    );
    assert_eq!(
        interface.methods[11].return_kind,
        RuntimeComReturnKind::Void
    );
    assert_eq!(
        interface.methods[21].return_kind,
        RuntimeComReturnKind::Void
    );
    assert_eq!(
        interface.methods[22].return_kind,
        RuntimeComReturnKind::Void
    );
    assert_eq!(
        interface.methods[80].return_kind,
        RuntimeComReturnKind::X87F32
    );
    assert_eq!(
        return_kinds
            .iter()
            .filter(|(_, kind)| *kind == RuntimeComReturnKind::HResult)
            .count(),
        109
    );

    let texture_spec = direct3dtexture9_object_spec();
    assert_eq!(
        texture_spec.class_identity,
        D3D9_IDIRECT3DTEXTURE9_CLASS_IDENTITY
    );
    assert_eq!(texture_spec.interfaces.len(), 2);
    let texture = &texture_spec.interfaces[1];
    assert_eq!(texture.iid, D3D9_IDIRECT3DTEXTURE9_IID);
    assert_eq!(texture.name, "IDirect3DTexture9");
    assert_eq!(texture.methods.len(), 22);
    assert_eq!(texture.methods[3].name, "GetDevice");
    assert_eq!(texture.methods[3].explicit_argument_words, 1);
    assert_eq!(texture.methods[18].name, "GetSurfaceLevel");
    assert_eq!(texture.methods[18].explicit_argument_words, 2);
    assert_eq!(
        texture
            .methods
            .iter()
            .enumerate()
            .filter(|(_, method)| method.return_kind == RuntimeComReturnKind::U32)
            .map(|(slot, _)| slot)
            .collect::<Vec<_>>(),
        [1, 2, 7, 8, 10, 11, 12, 13, 15]
    );
    assert_eq!(
        texture
            .methods
            .iter()
            .enumerate()
            .filter(|(_, method)| method.return_kind == RuntimeComReturnKind::Void)
            .map(|(slot, _)| slot)
            .collect::<Vec<_>>(),
        [9, 16]
    );
    assert_eq!(
        texture
            .methods
            .iter()
            .filter(|method| method.return_kind == RuntimeComReturnKind::HResult)
            .count(),
        11
    );
}

#[test]
fn create_device_rejects_invalid_or_unavailable_requests_without_partial_state() {
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
    let presentation = GuestAddress(MEMORY_BASE.0 + 0x800);
    let output = GuestAddress(MEMORY_BASE.0 + 0x900);
    write_present_parameters(&mut fixture.memory, presentation, UNKNOWN_WINDOW);
    fixture
        .memory
        .write(output, &0xcccc_cccc_u32.to_le_bytes())
        .unwrap();
    acknowledge_pending(&mut fixture.memory);
    let policy = D3d9CompatibilityPolicy::controlled_v9();

    for (adapter, device_type, focus_window, presentation_pointer, output_pointer) in [
        (
            1,
            D3D9_D3DDEVTYPE_HAL,
            UNKNOWN_WINDOW,
            presentation.0,
            output.0,
        ),
        (0, 2, UNKNOWN_WINDOW, presentation.0, output.0),
        (0, D3D9_D3DDEVTYPE_HAL, 0, presentation.0, output.0),
        (0, D3D9_D3DDEVTYPE_HAL, UNKNOWN_WINDOW, 0, output.0),
        (0, D3D9_D3DDEVTYPE_HAL, UNKNOWN_WINDOW, presentation.0, 0),
        (
            0,
            D3D9_D3DDEVTYPE_HAL,
            UNKNOWN_WINDOW,
            presentation.0,
            presentation.0,
        ),
        (
            0,
            D3D9_D3DDEVTYPE_HAL,
            UNKNOWN_WINDOW,
            presentation.0,
            output.0,
        ),
    ] {
        write_words(
            &mut fixture.memory,
            STACK_COM,
            &[
                0x1005_aca5,
                pointer,
                adapter,
                device_type,
                focus_window,
                D3D9_CREATE_HARDWARE_VERTEX_PROCESSING_MULTITHREADED,
                presentation_pointer,
                output_pointer,
            ],
        );
        acknowledge_pending(&mut fixture.memory);
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let completion = completed(fixture.dispatch_d3d9_com(&policy, &interface, 16, STACK_COM));
        assert_eq!(
            completion.return_value,
            ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL)
        );
        assert!(completion.guest_writes.is_empty());
        assert_eq!(fixture.memory.stats(), before_memory);
        assert_eq!(fixture.runtime, before_runtime);
        assert_eq!(
            fixture.memory.read(output, 4).unwrap(),
            0xcccc_cccc_u32.to_le_bytes()
        );
    }

    write_words(
        &mut fixture.memory,
        STACK_COM,
        &[
            0x1005_aca5,
            pointer,
            0,
            D3D9_D3DDEVTYPE_HAL,
            UNKNOWN_WINDOW,
            0,
            presentation.0,
            output.0,
        ],
    );
    acknowledge_pending(&mut fixture.memory);
    let before_memory = fixture.memory.stats();
    let before_runtime = fixture.runtime.clone();
    let unavailable = completed(fixture.dispatch_d3d9_com(&policy, &interface, 16, STACK_COM));
    assert_eq!(
        unavailable.return_value,
        ComReturnValue::HResult(D3D9_D3DERR_NOTAVAILABLE)
    );
    assert_eq!(fixture.memory.stats(), before_memory);
    assert_eq!(fixture.runtime, before_runtime);

    let mut disabled = policy.clone();
    disabled.create_device = None;
    disabled.create_texture = None;
    write_words(
        &mut fixture.memory,
        STACK_COM,
        &[
            0x1005_aca5,
            pointer,
            0,
            D3D9_D3DDEVTYPE_HAL,
            UNKNOWN_WINDOW,
            D3D9_CREATE_HARDWARE_VERTEX_PROCESSING_MULTITHREADED,
            presentation.0,
            output.0,
        ],
    );
    acknowledge_pending(&mut fixture.memory);
    let before_memory = fixture.memory.stats();
    let before_runtime = fixture.runtime.clone();
    let fault = faulted(fixture.dispatch_d3d9_com(&disabled, &interface, 16, STACK_COM));
    assert_eq!(fault.class, ComFaultClass::Unsupported);
    assert_eq!(fault.code, "create-device-policy-disabled");
    assert_eq!(fixture.memory.stats(), before_memory);
    assert_eq!(fixture.runtime, before_runtime);
}

#[test]
fn get_adapter_count_returns_one_policy_owned_logical_adapter_without_mutation() {
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
    write_words(&mut fixture.memory, STACK_COM, &[0x1005_9d36, pointer]);
    acknowledge_pending(&mut fixture.memory);

    let policy = D3d9CompatibilityPolicy::controlled_v9();
    assert!(policy.validate().is_ok());
    let before_memory = fixture.memory.stats();
    let before_runtime = fixture.runtime.clone();
    let completion = completed(fixture.dispatch_d3d9_com(&policy, &interface, 4, STACK_COM));

    assert_eq!(completion.return_value, ComReturnValue::U32(1));
    assert_eq!(completion.stack_pointer, GuestAddress(STACK_COM.0 + 8));
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
fn get_adapter_count_rejects_invalid_policy_or_display_authority_without_mutation() {
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
    let policy = D3d9CompatibilityPolicy::controlled_v9();

    let mut disabled = policy.clone();
    disabled.get_adapter_count = None;
    let mut malformed = policy.clone();
    malformed.get_adapter_count.as_mut().unwrap().adapter_count = 0;
    let mut mismatched = policy.clone();
    mismatched
        .get_adapter_count
        .as_mut()
        .unwrap()
        .display_profile_id = "single-monitor-mismatch-v1".to_owned();

    for rejected_policy in [disabled, malformed, mismatched] {
        write_words(&mut fixture.memory, STACK_COM, &[0x1005_9d36, pointer]);
        acknowledge_pending(&mut fixture.memory);
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let fault = faulted(fixture.dispatch_d3d9_com(&rejected_policy, &interface, 4, STACK_COM));
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
        .expect("IDirect3D9 interface")
        .clone();
    missing_display.acknowledge();
    write_words(
        &mut missing_display.memory,
        STACK_COM,
        &[0x1005_9d36, missing_pointer],
    );
    acknowledge_pending(&mut missing_display.memory);
    let before_memory = missing_display.memory.stats();
    let before_runtime = missing_display.runtime.clone();
    let fault =
        faulted(missing_display.dispatch_d3d9_com(&policy, &missing_interface, 4, STACK_COM));
    assert_eq!(fault.class, ComFaultClass::RuntimeDefect);
    assert_eq!(fault.code, "logical-primary-monitor-missing");
    assert_eq!(missing_display.memory.stats(), before_memory);
    assert_eq!(missing_display.runtime, before_runtime);
}

#[test]
fn get_adapter_monitor_returns_display_owned_primary_identity_without_mutation() {
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
    write_words(&mut fixture.memory, STACK_COM, &[0x1005_9d5e, pointer, 0]);
    acknowledge_pending(&mut fixture.memory);

    let policy = D3d9CompatibilityPolicy::controlled_v9();
    assert!(policy.validate().is_ok());
    let before_memory = fixture.memory.stats();
    let before_runtime = fixture.runtime.clone();
    let completion = completed(fixture.dispatch_d3d9_com(&policy, &interface, 15, STACK_COM));

    assert_eq!(
        completion.return_value,
        ComReturnValue::U32(RUNTIME_PRIMARY_MONITOR_HANDLE)
    );
    assert_eq!(completion.stack_pointer, GuestAddress(STACK_COM.0 + 12));
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
fn get_adapter_monitor_rejects_invalid_policy_and_preserves_invalid_adapter_state() {
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
    let policy = D3d9CompatibilityPolicy::controlled_v9();

    for adapter in [1, u32::MAX] {
        write_words(
            &mut fixture.memory,
            STACK_COM,
            &[0x1005_9d5e, pointer, adapter],
        );
        acknowledge_pending(&mut fixture.memory);
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let completion = completed(fixture.dispatch_d3d9_com(&policy, &interface, 15, STACK_COM));
        assert_eq!(completion.return_value, ComReturnValue::U32(0));
        assert!(completion.guest_writes.is_empty());
        assert_eq!(fixture.memory.stats(), before_memory);
        assert_eq!(fixture.runtime, before_runtime);
        assert_eq!(
            fixture.runtime.current_last_error().unwrap(),
            LAST_ERROR_SENTINEL
        );
    }

    let mut disabled = policy.clone();
    disabled.get_adapter_monitor = None;
    let mut malformed = policy.clone();
    malformed.get_adapter_monitor.as_mut().unwrap().adapter = 1;
    let mut mismatched = policy.clone();
    mismatched
        .get_adapter_monitor
        .as_mut()
        .unwrap()
        .display_profile_id = "single-monitor-mismatch-v1".to_owned();

    for rejected_policy in [disabled, malformed, mismatched] {
        write_words(&mut fixture.memory, STACK_COM, &[0x1005_9d5e, pointer, 0]);
        acknowledge_pending(&mut fixture.memory);
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let fault = faulted(fixture.dispatch_d3d9_com(&rejected_policy, &interface, 15, STACK_COM));
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
        .expect("IDirect3D9 interface")
        .clone();
    missing_display.acknowledge();
    write_words(
        &mut missing_display.memory,
        STACK_COM,
        &[0x1005_9d5e, missing_pointer, 0],
    );
    acknowledge_pending(&mut missing_display.memory);
    let before_memory = missing_display.memory.stats();
    let before_runtime = missing_display.runtime.clone();
    let fault =
        faulted(missing_display.dispatch_d3d9_com(&policy, &missing_interface, 15, STACK_COM));
    assert_eq!(fault.class, ComFaultClass::RuntimeDefect);
    assert_eq!(fault.code, "logical-primary-monitor-missing");
    assert_eq!(missing_display.memory.stats(), before_memory);
    assert_eq!(missing_display.runtime, before_runtime);
}

#[test]
fn get_adapter_identifier_writes_one_policy_owned_virtual_identity() {
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

    let output = GuestAddress(MEMORY_BASE.0 + 0x400);
    let output_bytes = D3D9_D3DADAPTER_IDENTIFIER9_BYTE_LENGTH as usize;
    fixture
        .memory
        .write(output, &vec![0xcc; output_bytes])
        .expect("adapter-identifier sentinel");
    write_words(
        &mut fixture.memory,
        STACK_COM,
        &[0x1005_b91c, pointer, 0, 0, output.0],
    );
    acknowledge_pending(&mut fixture.memory);
    let policy = D3d9CompatibilityPolicy::controlled_v9();
    assert!(policy.validate().is_ok());
    let before_runtime = fixture.runtime.clone();
    let completion = completed(fixture.dispatch_d3d9_com(&policy, &interface, 5, STACK_COM));
    assert_eq!(completion.return_value, ComReturnValue::HResult(COM_S_OK));
    assert_eq!(completion.stack_pointer, GuestAddress(STACK_COM.0 + 20));
    assert_eq!(completion.reference_count, 1);
    assert!(!completion.object_retired);
    assert_eq!(completion.guest_writes.len(), 1);
    assert_eq!(completion.guest_writes[0].address, output);
    let bytes = &completion.guest_writes[0].bytes;
    assert_eq!(bytes.len(), output_bytes);
    assert!(!bytes.contains(&0xcc));
    assert_fixed_ansi(bytes, 0, 512, D3D9_VIRTUAL_DRIVER);
    assert_fixed_ansi(bytes, 512, 512, D3D9_VIRTUAL_DESCRIPTION);
    assert_fixed_ansi(bytes, 1_024, 32, D3D9_VIRTUAL_DEVICE_NAME);
    assert_eq!(
        u64::from_le_bytes(bytes[1_056..1_064].try_into().unwrap()),
        D3D9_VIRTUAL_DRIVER_VERSION
    );
    for offset in [1_064, 1_068, 1_072, 1_076, 1_096] {
        assert_eq!(
            u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()),
            0
        );
    }
    assert_eq!(
        &bytes[1_080..1_096],
        &D3D9_VIRTUAL_DEVICE_IDENTIFIER_WINDOWS_BYTES
    );
    fixture.acknowledge();
    assert_eq!(fixture.memory.read(output, output_bytes).unwrap(), *bytes);
    assert_eq!(fixture.runtime, before_runtime);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
}

#[test]
fn get_adapter_identifier_negative_paths_preserve_memory_and_authority() {
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
    let output = GuestAddress(MEMORY_BASE.0 + 0x400);
    let output_bytes = D3D9_D3DADAPTER_IDENTIFIER9_BYTE_LENGTH as usize;
    let policy = D3d9CompatibilityPolicy::controlled_v9();

    for (adapter, flags, output_pointer) in [(1, 0, output.0), (0, 1, output.0), (0, 0, 0)] {
        fixture
            .memory
            .write(output, &vec![0xcc; output_bytes])
            .expect("negative adapter-identifier sentinel");
        write_words(
            &mut fixture.memory,
            STACK_COM,
            &[0x1005_b91c, pointer, adapter, flags, output_pointer],
        );
        acknowledge_pending(&mut fixture.memory);
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let completion = completed(fixture.dispatch_d3d9_com(&policy, &interface, 5, STACK_COM));
        assert_eq!(
            completion.return_value,
            ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL)
        );
        assert!(completion.guest_writes.is_empty());
        assert_eq!(fixture.memory.stats(), before_memory);
        assert_eq!(fixture.runtime, before_runtime);
        assert_eq!(
            fixture.memory.read(output, output_bytes).unwrap(),
            vec![0xcc; output_bytes]
        );
    }

    write_words(
        &mut fixture.memory,
        STACK_COM,
        &[0x1005_b91c, pointer, 0, D3D9_D3DENUM_WHQL_LEVEL, output.0],
    );
    acknowledge_pending(&mut fixture.memory);
    let before_memory = fixture.memory.stats();
    let before_runtime = fixture.runtime.clone();
    let whql = faulted(fixture.dispatch_d3d9_com(&policy, &interface, 5, STACK_COM));
    assert_eq!(whql.class, ComFaultClass::Unsupported);
    assert_eq!(whql.code, "d3denum-whql-level-unsupported");
    assert_eq!(fixture.memory.stats(), before_memory);
    assert_eq!(fixture.runtime, before_runtime);

    write_words(
        &mut fixture.memory,
        STACK_COM,
        &[0x1005_b91c, pointer, 0, 0, interface.guest_pointer.0],
    );
    acknowledge_pending(&mut fixture.memory);
    let before_memory = fixture.memory.stats();
    let before_runtime = fixture.runtime.clone();
    let unwritable = faulted(fixture.dispatch_d3d9_com(&policy, &interface, 5, STACK_COM));
    assert_eq!(unwritable.class, ComFaultClass::InvalidCall);
    assert_eq!(fixture.memory.stats(), before_memory);
    assert_eq!(fixture.runtime, before_runtime);

    for rejected_policy in {
        let mut disabled = policy.clone();
        disabled.get_adapter_identifier = None;
        let mut mismatched = policy.clone();
        mismatched
            .get_adapter_identifier
            .as_mut()
            .unwrap()
            .display_profile_id = "single-monitor-mismatch-v1".to_owned();
        let mut malformed = policy.clone();
        malformed.get_adapter_identifier.as_mut().unwrap().driver = String::new();
        [disabled, mismatched, malformed]
    } {
        write_words(
            &mut fixture.memory,
            STACK_COM,
            &[0x1005_b91c, pointer, 0, 0, output.0],
        );
        acknowledge_pending(&mut fixture.memory);
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let fault = faulted(fixture.dispatch_d3d9_com(&rejected_policy, &interface, 5, STACK_COM));
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
        &[0x1005_b91c, missing_pointer, 0, 0, output.0],
    );
    acknowledge_pending(&mut missing_display.memory);
    let before_memory = missing_display.memory.stats();
    let before_runtime = missing_display.runtime.clone();
    let missing =
        faulted(missing_display.dispatch_d3d9_com(&policy, &missing_interface, 5, STACK_COM));
    assert_eq!(missing.class, ComFaultClass::RuntimeDefect);
    assert_eq!(missing.code, "logical-primary-monitor-missing");
    assert_eq!(missing_display.memory.stats(), before_memory);
    assert_eq!(missing_display.runtime, before_runtime);
}

#[test]
fn get_device_caps_writes_one_bounded_virtual_capability_profile() {
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

    let output = GuestAddress(MEMORY_BASE.0 + 0x800);
    let output_bytes = D3D9_D3DCAPS9_BYTE_LENGTH as usize;
    fixture
        .memory
        .write(output, &vec![0xcc; output_bytes])
        .expect("device-caps sentinel");
    write_words(
        &mut fixture.memory,
        STACK_COM,
        &[0x1005_b949, pointer, 0, D3D9_D3DDEVTYPE_HAL, output.0],
    );
    acknowledge_pending(&mut fixture.memory);
    let policy = D3d9CompatibilityPolicy::controlled_v9();
    assert!(policy.validate().is_ok());
    let before_runtime = fixture.runtime.clone();
    let completion = completed(fixture.dispatch_d3d9_com(&policy, &interface, 14, STACK_COM));
    assert_eq!(completion.return_value, ComReturnValue::HResult(COM_S_OK));
    assert_eq!(completion.stack_pointer, GuestAddress(STACK_COM.0 + 20));
    assert_eq!(completion.reference_count, 1);
    assert!(!completion.object_retired);
    assert_eq!(completion.guest_writes.len(), 1);
    assert_eq!(completion.guest_writes[0].address, output);
    let bytes = &completion.guest_writes[0].bytes;
    assert_eq!(bytes.len(), output_bytes);
    let words = bytes
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(words.len(), 76);
    assert_eq!(words[0], D3D9_D3DDEVTYPE_HAL);
    assert_eq!(words[1], 0);
    assert_eq!(words[2], D3D9_D3DCAPS_READ_SCANLINE);
    assert_eq!(words[3], D3D9_D3DCAPS2_DYNAMICTEXTURES);
    assert_eq!(words[51], D3D9_D3DPS_VERSION_3_0);
    assert_eq!(words[58], 1);
    for (index, value) in words.into_iter().enumerate() {
        if ![0, 2, 3, 51, 58].contains(&index) {
            assert_eq!(value, 0, "unexpected nonzero D3DCAPS9 word {index}");
        }
    }
    fixture.acknowledge();
    assert_eq!(fixture.memory.read(output, output_bytes).unwrap(), *bytes);
    assert_eq!(fixture.runtime, before_runtime);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
}

#[test]
fn get_device_caps_negative_paths_preserve_memory_and_authority() {
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
    let output = GuestAddress(MEMORY_BASE.0 + 0x800);
    let output_bytes = D3D9_D3DCAPS9_BYTE_LENGTH as usize;
    let policy = D3d9CompatibilityPolicy::controlled_v9();

    for (adapter, device_type, output_pointer) in [
        (1, D3D9_D3DDEVTYPE_HAL, output.0),
        (0, 2, output.0),
        (0, D3D9_D3DDEVTYPE_HAL, 0),
    ] {
        fixture
            .memory
            .write(output, &vec![0xcc; output_bytes])
            .expect("negative device-caps sentinel");
        write_words(
            &mut fixture.memory,
            STACK_COM,
            &[0x1005_b949, pointer, adapter, device_type, output_pointer],
        );
        acknowledge_pending(&mut fixture.memory);
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let completion = completed(fixture.dispatch_d3d9_com(&policy, &interface, 14, STACK_COM));
        assert_eq!(
            completion.return_value,
            ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL)
        );
        assert!(completion.guest_writes.is_empty());
        assert_eq!(fixture.memory.stats(), before_memory);
        assert_eq!(fixture.runtime, before_runtime);
        assert_eq!(
            fixture.memory.read(output, output_bytes).unwrap(),
            vec![0xcc; output_bytes]
        );
    }

    write_words(
        &mut fixture.memory,
        STACK_COM,
        &[
            0x1005_b949,
            pointer,
            0,
            D3D9_D3DDEVTYPE_HAL,
            interface.guest_pointer.0,
        ],
    );
    acknowledge_pending(&mut fixture.memory);
    let before_memory = fixture.memory.stats();
    let before_runtime = fixture.runtime.clone();
    let unwritable = faulted(fixture.dispatch_d3d9_com(&policy, &interface, 14, STACK_COM));
    assert_eq!(unwritable.class, ComFaultClass::InvalidCall);
    assert_eq!(fixture.memory.stats(), before_memory);
    assert_eq!(fixture.runtime, before_runtime);

    for rejected_policy in {
        let mut disabled = policy.clone();
        disabled.get_device_caps = None;
        let mut mismatched = policy.clone();
        mismatched
            .get_device_caps
            .as_mut()
            .unwrap()
            .display_profile_id = "single-monitor-mismatch-v1".to_owned();
        let mut malformed = policy.clone();
        malformed.get_device_caps.as_mut().unwrap().caps2 = 0;
        let mut identity_mismatched = policy.clone();
        identity_mismatched.get_adapter_display_mode = None;
        identity_mismatched
            .get_device_caps
            .as_mut()
            .unwrap()
            .display_profile_id = "single-monitor-mismatch-v1".to_owned();
        [disabled, mismatched, malformed, identity_mismatched]
    } {
        write_words(
            &mut fixture.memory,
            STACK_COM,
            &[0x1005_b949, pointer, 0, D3D9_D3DDEVTYPE_HAL, output.0],
        );
        acknowledge_pending(&mut fixture.memory);
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let fault = faulted(fixture.dispatch_d3d9_com(&rejected_policy, &interface, 14, STACK_COM));
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
        &[
            0x1005_b949,
            missing_pointer,
            0,
            D3D9_D3DDEVTYPE_HAL,
            output.0,
        ],
    );
    acknowledge_pending(&mut missing_display.memory);
    let before_memory = missing_display.memory.stats();
    let before_runtime = missing_display.runtime.clone();
    let missing =
        faulted(missing_display.dispatch_d3d9_com(&policy, &missing_interface, 14, STACK_COM));
    assert_eq!(missing.class, ComFaultClass::RuntimeDefect);
    assert_eq!(missing.code, "logical-primary-monitor-missing");
    assert_eq!(missing_display.memory.stats(), before_memory);
    assert_eq!(missing_display.runtime, before_runtime);
}

#[test]
fn check_device_type_accepts_the_bounded_windowed_and_fullscreen_band() {
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
    let policy = D3d9CompatibilityPolicy::controlled_v9();
    assert!(policy.validate().is_ok());

    for windowed in [1, 0, 2] {
        write_words(
            &mut fixture.memory,
            STACK_COM,
            &[
                0x1005_b9d6,
                pointer,
                0,
                D3D9_D3DDEVTYPE_HAL,
                D3D9_D3DFMT_X8R8G8B8,
                D3D9_D3DFMT_X8R8G8B8,
                windowed,
            ],
        );
        acknowledge_pending(&mut fixture.memory);
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let completion = completed(fixture.dispatch_d3d9_com(&policy, &interface, 9, STACK_COM));
        assert_eq!(completion.return_value, ComReturnValue::HResult(COM_S_OK));
        assert_eq!(completion.stack_pointer, GuestAddress(STACK_COM.0 + 28));
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
}

#[test]
fn check_device_type_failure_classes_preserve_memory_and_authority() {
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
    let policy = D3d9CompatibilityPolicy::controlled_v9();

    for (adapter, device_type) in [(1, D3D9_D3DDEVTYPE_HAL), (0, 2)] {
        write_words(
            &mut fixture.memory,
            STACK_COM,
            &[
                0x1005_b9d6,
                pointer,
                adapter,
                device_type,
                D3D9_D3DFMT_X8R8G8B8,
                D3D9_D3DFMT_X8R8G8B8,
                1,
            ],
        );
        acknowledge_pending(&mut fixture.memory);
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let completion = completed(fixture.dispatch_d3d9_com(&policy, &interface, 9, STACK_COM));
        assert_eq!(
            completion.return_value,
            ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL)
        );
        assert!(completion.guest_writes.is_empty());
        assert_eq!(fixture.memory.stats(), before_memory);
        assert_eq!(fixture.runtime, before_runtime);
    }

    for (adapter_format, back_buffer_format) in
        [(0, D3D9_D3DFMT_X8R8G8B8), (D3D9_D3DFMT_X8R8G8B8, 0)]
    {
        write_words(
            &mut fixture.memory,
            STACK_COM,
            &[
                0x1005_b9d6,
                pointer,
                0,
                D3D9_D3DDEVTYPE_HAL,
                adapter_format,
                back_buffer_format,
                1,
            ],
        );
        acknowledge_pending(&mut fixture.memory);
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let completion = completed(fixture.dispatch_d3d9_com(&policy, &interface, 9, STACK_COM));
        assert_eq!(
            completion.return_value,
            ComReturnValue::HResult(D3D9_D3DERR_NOTAVAILABLE)
        );
        assert!(completion.guest_writes.is_empty());
        assert_eq!(fixture.memory.stats(), before_memory);
        assert_eq!(fixture.runtime, before_runtime);
    }

    let mut disabled = policy.clone();
    disabled.check_device_type = None;
    let mut malformed = policy.clone();
    malformed
        .check_device_type
        .as_mut()
        .unwrap()
        .fullscreen_supported = false;
    let mut mismatched = policy.clone();
    for display_profile_id in [
        mismatched
            .get_adapter_display_mode
            .as_mut()
            .map(|profile| &mut profile.display_profile_id),
        mismatched
            .get_adapter_identifier
            .as_mut()
            .map(|profile| &mut profile.display_profile_id),
        mismatched
            .get_device_caps
            .as_mut()
            .map(|profile| &mut profile.display_profile_id),
        mismatched
            .check_device_type
            .as_mut()
            .map(|profile| &mut profile.display_profile_id),
    ]
    .into_iter()
    .flatten()
    {
        *display_profile_id = "single-monitor-mismatch-v1".to_owned();
    }

    for rejected_policy in [disabled, malformed, mismatched] {
        write_words(
            &mut fixture.memory,
            STACK_COM,
            &[
                0x1005_b9d6,
                pointer,
                0,
                D3D9_D3DDEVTYPE_HAL,
                D3D9_D3DFMT_X8R8G8B8,
                D3D9_D3DFMT_X8R8G8B8,
                1,
            ],
        );
        acknowledge_pending(&mut fixture.memory);
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let fault = faulted(fixture.dispatch_d3d9_com(&rejected_policy, &interface, 9, STACK_COM));
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
        &[
            0x1005_b9d6,
            missing_pointer,
            0,
            D3D9_D3DDEVTYPE_HAL,
            D3D9_D3DFMT_X8R8G8B8,
            D3D9_D3DFMT_X8R8G8B8,
            1,
        ],
    );
    acknowledge_pending(&mut missing_display.memory);
    let before_memory = missing_display.memory.stats();
    let before_runtime = missing_display.runtime.clone();
    let fault =
        faulted(missing_display.dispatch_d3d9_com(&policy, &missing_interface, 9, STACK_COM));
    assert_eq!(fault.class, ComFaultClass::RuntimeDefect);
    assert_eq!(fault.code, "logical-primary-monitor-missing");
    assert_eq!(missing_display.memory.stats(), before_memory);
    assert_eq!(missing_display.runtime, before_runtime);
}

#[test]
fn check_device_format_accepts_the_bounded_texture_usage_band() {
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
    let policy = D3d9CompatibilityPolicy::controlled_v9();
    assert!(policy.validate().is_ok());

    for usage in [0, D3D9_D3DUSAGE_DYNAMIC] {
        write_words(
            &mut fixture.memory,
            STACK_COM,
            &[
                0x1005_ba69,
                pointer,
                0,
                D3D9_D3DDEVTYPE_HAL,
                D3D9_D3DFMT_X8R8G8B8,
                usage,
                D3D9_D3DRTYPE_TEXTURE,
                D3D9_D3DFMT_X8R8G8B8,
            ],
        );
        acknowledge_pending(&mut fixture.memory);
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let completion = completed(fixture.dispatch_d3d9_com(&policy, &interface, 10, STACK_COM));
        assert_eq!(completion.return_value, ComReturnValue::HResult(COM_S_OK));
        assert_eq!(completion.stack_pointer, GuestAddress(STACK_COM.0 + 32));
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
}

#[test]
fn check_device_format_failure_classes_preserve_memory_and_authority() {
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
    let policy = D3d9CompatibilityPolicy::controlled_v9();

    for (adapter, device_type) in [(1, D3D9_D3DDEVTYPE_HAL), (0, 2)] {
        write_words(
            &mut fixture.memory,
            STACK_COM,
            &[
                0x1005_ba69,
                pointer,
                adapter,
                device_type,
                D3D9_D3DFMT_X8R8G8B8,
                D3D9_D3DUSAGE_DYNAMIC,
                D3D9_D3DRTYPE_TEXTURE,
                D3D9_D3DFMT_X8R8G8B8,
            ],
        );
        acknowledge_pending(&mut fixture.memory);
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let completion = completed(fixture.dispatch_d3d9_com(&policy, &interface, 10, STACK_COM));
        assert_eq!(
            completion.return_value,
            ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL)
        );
        assert!(completion.guest_writes.is_empty());
        assert_eq!(fixture.memory.stats(), before_memory);
        assert_eq!(fixture.runtime, before_runtime);
    }

    for (adapter_format, usage, resource_type, check_format) in [
        (
            0,
            D3D9_D3DUSAGE_DYNAMIC,
            D3D9_D3DRTYPE_TEXTURE,
            D3D9_D3DFMT_X8R8G8B8,
        ),
        (
            D3D9_D3DFMT_X8R8G8B8,
            1,
            D3D9_D3DRTYPE_TEXTURE,
            D3D9_D3DFMT_X8R8G8B8,
        ),
        (
            D3D9_D3DFMT_X8R8G8B8,
            D3D9_D3DUSAGE_DYNAMIC,
            4,
            D3D9_D3DFMT_X8R8G8B8,
        ),
        (D3D9_D3DFMT_X8R8G8B8, 0, D3D9_D3DRTYPE_TEXTURE, 118),
    ] {
        write_words(
            &mut fixture.memory,
            STACK_COM,
            &[
                0x1005_ba69,
                pointer,
                0,
                D3D9_D3DDEVTYPE_HAL,
                adapter_format,
                usage,
                resource_type,
                check_format,
            ],
        );
        acknowledge_pending(&mut fixture.memory);
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let completion = completed(fixture.dispatch_d3d9_com(&policy, &interface, 10, STACK_COM));
        assert_eq!(
            completion.return_value,
            ComReturnValue::HResult(D3D9_D3DERR_NOTAVAILABLE)
        );
        assert!(completion.guest_writes.is_empty());
        assert_eq!(fixture.memory.stats(), before_memory);
        assert_eq!(fixture.runtime, before_runtime);
    }

    let mut disabled = policy.clone();
    disabled.check_device_format = None;
    let mut malformed = policy.clone();
    malformed
        .check_device_format
        .as_mut()
        .unwrap()
        .supported_usages = [D3D9_D3DUSAGE_DYNAMIC, 0];
    let mut mismatched = policy.clone();
    for display_profile_id in [
        mismatched
            .get_adapter_display_mode
            .as_mut()
            .map(|profile| &mut profile.display_profile_id),
        mismatched
            .get_adapter_identifier
            .as_mut()
            .map(|profile| &mut profile.display_profile_id),
        mismatched
            .get_device_caps
            .as_mut()
            .map(|profile| &mut profile.display_profile_id),
        mismatched
            .check_device_type
            .as_mut()
            .map(|profile| &mut profile.display_profile_id),
        mismatched
            .check_device_format
            .as_mut()
            .map(|profile| &mut profile.display_profile_id),
    ]
    .into_iter()
    .flatten()
    {
        *display_profile_id = "single-monitor-mismatch-v1".to_owned();
    }

    for rejected_policy in [disabled, malformed, mismatched] {
        write_words(
            &mut fixture.memory,
            STACK_COM,
            &[
                0x1005_ba69,
                pointer,
                0,
                D3D9_D3DDEVTYPE_HAL,
                D3D9_D3DFMT_X8R8G8B8,
                D3D9_D3DUSAGE_DYNAMIC,
                D3D9_D3DRTYPE_TEXTURE,
                D3D9_D3DFMT_X8R8G8B8,
            ],
        );
        acknowledge_pending(&mut fixture.memory);
        let before_memory = fixture.memory.stats();
        let before_runtime = fixture.runtime.clone();
        let fault = faulted(fixture.dispatch_d3d9_com(&rejected_policy, &interface, 10, STACK_COM));
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
        &[
            0x1005_ba69,
            missing_pointer,
            0,
            D3D9_D3DDEVTYPE_HAL,
            D3D9_D3DFMT_X8R8G8B8,
            D3D9_D3DUSAGE_DYNAMIC,
            D3D9_D3DRTYPE_TEXTURE,
            D3D9_D3DFMT_X8R8G8B8,
        ],
    );
    acknowledge_pending(&mut missing_display.memory);
    let before_memory = missing_display.memory.stats();
    let before_runtime = missing_display.runtime.clone();
    let fault =
        faulted(missing_display.dispatch_d3d9_com(&policy, &missing_interface, 10, STACK_COM));
    assert_eq!(fault.class, ComFaultClass::RuntimeDefect);
    assert_eq!(fault.code, "logical-primary-monitor-missing");
    assert_eq!(missing_display.memory.stats(), before_memory);
    assert_eq!(missing_display.runtime, before_runtime);
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
        &D3d9CompatibilityPolicy::controlled_v9(),
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
    let policy = D3d9CompatibilityPolicy::controlled_v9();
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

fn assert_fixed_ansi(bytes: &[u8], offset: usize, capacity: usize, expected: &str) {
    assert_eq!(&bytes[offset..offset + expected.len()], expected.as_bytes());
    assert!(
        bytes[offset + expected.len()..offset + capacity]
            .iter()
            .all(|byte| *byte == 0)
    );
}

fn write_words(memory: &mut GuestMemory, address: GuestAddress, words: &[u32]) {
    let bytes = words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<_>>();
    memory.write(address, &bytes).expect("write fixture words");
}

fn write_present_parameters(memory: &mut GuestMemory, address: GuestAddress, device_window: u32) {
    let words = [
        0,
        0,
        D3D9_D3DFMT_UNKNOWN,
        1,
        D3D9_D3DMULTISAMPLE_NONE,
        0,
        D3D9_D3DSWAPEFFECT_DISCARD,
        device_window,
        1,
        0,
        D3D9_D3DFMT_D16,
        0,
        0,
        0,
    ];
    assert_eq!(
        words.len() * 4,
        D3D9_D3DPRESENT_PARAMETERS_BYTE_LENGTH as usize
    );
    write_words(memory, address, &words);
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

use vine_core::{
    COM_E_NOINTERFACE, COM_E_POINTER, COM_S_OK, ComCallCompletion, ComCallTrap, ComDispatchOutcome,
    ComFaultClass, ComInterfaceSpec, ComMethodSpec, ComObjectSpec, ComReturnValue,
    ExecutionSession, GuestAddress, GuestMemory, LOGICAL_ENCODED_POINTER_NAMESPACE,
    MappingOperation, MemoryAccess, MemoryLimits, MemoryOwnerKind, MemoryRegionKind,
    PageProtection, ProcessAddressAllocationPurpose, ProcessAddressSpaceLayout,
    RUNTIME_COM_IUNKNOWN_IID, RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING, RuntimeBootstrap,
    RuntimeClockState, RuntimeComIid, RuntimeCurrentDirectoryA, RuntimeProcessId,
    RuntimeProcessImagePathA, RuntimeStartupInfo, RuntimeState, RuntimeThreadId,
    X86_STARTUP_INFO_W_BYTE_LENGTH,
};

const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;
const SCRATCH_BASE: GuestAddress = GuestAddress(0x0060_0000);
const IID_IUNKNOWN_ADDRESS: GuestAddress = GuestAddress(SCRATCH_BASE.0 + 0x300);
const IID_UNKNOWN_ADDRESS: GuestAddress = GuestAddress(SCRATCH_BASE.0 + 0x310);
const IID_SYNTHETIC_ADDRESS: GuestAddress = GuestAddress(SCRATCH_BASE.0 + 0x320);
const OUTPUT_KNOWN: GuestAddress = GuestAddress(SCRATCH_BASE.0 + 0x340);
const OUTPUT_UNKNOWN: GuestAddress = GuestAddress(SCRATCH_BASE.0 + 0x344);
const OUTPUT_SYNTHETIC: GuestAddress = GuestAddress(SCRATCH_BASE.0 + 0x348);
const LAST_ERROR_SENTINEL: u32 = 0x1234_5678;
const SYNTHETIC_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0x78, 0x56, 0x34, 0x12, 0x34, 0x12, 0x78, 0x56, 0x90, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67,
]);
const UNKNOWN_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0xef, 0xbe, 0xad, 0xde, 0x11, 0x11, 0x22, 0x22, 0x33, 0x33, 0x44, 0x44, 0x55, 0x55, 0x66, 0x66,
]);

#[test]
fn x86_iunknown_abi_owns_identity_refcounts_and_structured_stops() {
    let mut memory = GuestMemory::new(MemoryLimits::default()).expect("guest memory");
    install_scratch_page(&mut memory);
    let mut runtime = controlled_runtime();
    assert!(serde_json::to_value(&runtime).unwrap().get("com").is_none());
    let installed = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        session
            .install_com_object(synthetic_object_spec())
            .expect("install synthetic COM object")
    };
    let iunknown = installed
        .interfaces
        .iter()
        .find(|interface| interface.iid == RUNTIME_COM_IUNKNOWN_IID)
        .expect("canonical IUnknown")
        .clone();
    let synthetic = installed
        .interfaces
        .iter()
        .find(|interface| interface.iid == SYNTHETIC_IID)
        .expect("synthetic interface")
        .clone();
    assert_ne!(iunknown.id, synthetic.id);
    assert_eq!(iunknown.object_id, synthetic.object_id);
    assert_eq!(installed.object.reference_count, 1);
    assert_eq!(
        installed.object.canonical_iunknown_interface_id,
        iunknown.id
    );
    let allocation_id = installed.object.backing.address_allocation_id;
    let backing_range = installed.object.backing.range;
    assert_eq!(
        runtime
            .process_address_space_allocator()
            .allocation(allocation_id)
            .expect("COM address allocation")
            .purpose,
        ProcessAddressAllocationPurpose::ComEndpoint
    );
    assert!(serde_json::to_value(&runtime).unwrap().get("com").is_some());

    prepare_calls(&mut memory, &synthetic, &iunknown);
    acknowledge_initial(&mut memory);
    assert_eq!(
        memory
            .read(synthetic.guest_pointer, 4)
            .expect("synthetic interface pointer"),
        synthetic.vtable.0.to_le_bytes()
    );
    assert!(
        memory
            .validate_access(synthetic.guest_pointer, 4, MemoryAccess::Read)
            .is_ok()
    );
    assert!(
        memory
            .validate_access(synthetic.guest_pointer, 4, MemoryAccess::Write)
            .is_err()
    );
    assert!(
        memory
            .validate_access(synthetic.methods[&0].endpoint, 2, MemoryAccess::Execute)
            .is_ok()
    );
    assert!(
        memory
            .validate_access(synthetic.methods[&0].endpoint, 2, MemoryAccess::Write)
            .is_err()
    );
    assert_eq!(
        memory
            .read(synthetic.methods[&0].endpoint, 10)
            .expect("owned COM endpoint prefix"),
        [0x0f, 0x0b, b'V', b'C', b'O', b'M', 1, 0, 0, 0]
    );
    assert_eq!(
        u32::from_le_bytes(
            memory
                .read(GuestAddress(synthetic.vtable.0 + 12), 4)
                .expect("unsupported vtable slot")
                .try_into()
                .unwrap()
        ),
        synthetic.methods[&3].endpoint.0
    );

    let known = completed(dispatch(
        &mut memory,
        &mut runtime,
        &synthetic,
        0,
        SCRATCH_BASE,
        GuestAddress(0x0040_1000),
    ));
    assert_eq!(known.return_value, ComReturnValue::HResult(COM_S_OK));
    assert_eq!(known.return_value.eax(), 0);
    assert_eq!(known.stack_pointer, GuestAddress(SCRATCH_BASE.0 + 16));
    assert_eq!(known.frame.return_stack_pointer, known.stack_pointer);
    assert_eq!(known.frame.argument_bytes, 12);
    assert_eq!(known.reference_count, 2);
    assert_eq!(runtime.current_last_error().unwrap(), LAST_ERROR_SENTINEL);
    acknowledge(&mut memory);
    assert_eq!(read_u32(&memory, OUTPUT_KNOWN), iunknown.guest_pointer.0);

    let round_trip = completed(dispatch(
        &mut memory,
        &mut runtime,
        &iunknown,
        0,
        GuestAddress(SCRATCH_BASE.0 + 0x100),
        GuestAddress(0x0040_1010),
    ));
    assert_eq!(round_trip.return_value, ComReturnValue::HResult(COM_S_OK));
    assert_eq!(round_trip.reference_count, 3);
    acknowledge(&mut memory);
    assert_eq!(
        read_u32(&memory, OUTPUT_SYNTHETIC),
        synthetic.guest_pointer.0
    );

    let unknown = completed(dispatch(
        &mut memory,
        &mut runtime,
        &synthetic,
        0,
        GuestAddress(SCRATCH_BASE.0 + 0x20),
        GuestAddress(0x0040_1020),
    ));
    assert_eq!(
        unknown.return_value,
        ComReturnValue::HResult(COM_E_NOINTERFACE)
    );
    assert!(!COM_E_NOINTERFACE.succeeded());
    assert_eq!(unknown.reference_count, 3);
    acknowledge(&mut memory);
    assert_eq!(read_u32(&memory, OUTPUT_UNKNOWN), 0);

    let null_output = completed(dispatch(
        &mut memory,
        &mut runtime,
        &synthetic,
        0,
        GuestAddress(SCRATCH_BASE.0 + 0x40),
        GuestAddress(0x0040_1040),
    ));
    assert_eq!(
        null_output.return_value,
        ComReturnValue::HResult(COM_E_POINTER)
    );
    assert_eq!(null_output.reference_count, 3);
    assert!(null_output.guest_writes.is_empty());

    let add_ref = completed(dispatch(
        &mut memory,
        &mut runtime,
        &iunknown,
        1,
        GuestAddress(SCRATCH_BASE.0 + 0x60),
        GuestAddress(0x0040_1060),
    ));
    assert_eq!(add_ref.return_value, ComReturnValue::U32(4));
    assert_eq!(add_ref.stack_pointer, GuestAddress(SCRATCH_BASE.0 + 0x68));
    assert_eq!(add_ref.frame.argument_bytes, 4);

    let before_unsupported_memory = memory.stats();
    let before_unsupported_runtime = runtime.com().clone();
    let unsupported = faulted(dispatch(
        &mut memory,
        &mut runtime,
        &synthetic,
        3,
        GuestAddress(SCRATCH_BASE.0 + 0x80),
        GuestAddress(0x0040_1080),
    ));
    assert_eq!(unsupported.class, ComFaultClass::Unsupported);
    assert_eq!(unsupported.code, "com-method-unsupported");
    assert_eq!(unsupported.call_site, GuestAddress(0x0040_1080));
    let unsupported_target = unsupported.target.expect("structured COM target");
    assert_eq!(unsupported_target.interface, "ISynthetic");
    assert_eq!(unsupported_target.vtable_slot, 3);
    assert_eq!(unsupported_target.method, "DoWork");
    assert_eq!(memory.stats(), before_unsupported_memory);
    assert_eq!(runtime.com(), &before_unsupported_runtime);
    let serialized = serde_json::to_value(&unsupported_target).unwrap();
    assert_eq!(serialized["objectId"], installed.object.id.0.to_string());
    assert_eq!(serialized["interfaceId"], synthetic.id.0.to_string());
    assert_eq!(serialized["interface"], "ISynthetic");
    assert_eq!(serialized["vtableSlot"], 3);

    for (offset, interface, expected) in [
        (0xa0, &synthetic, 3),
        (0xc0, &iunknown, 2),
        (0xe0, &iunknown, 1),
        (0x120, &synthetic, 0),
    ] {
        let release = completed(dispatch(
            &mut memory,
            &mut runtime,
            interface,
            2,
            GuestAddress(SCRATCH_BASE.0 + offset),
            GuestAddress(0x0040_1100 + offset),
        ));
        assert_eq!(release.return_value, ComReturnValue::U32(expected));
        assert_eq!(release.reference_count, expected);
        assert_eq!(release.object_retired, expected == 0);
    }
    assert!(runtime.com().is_empty());
    assert!(serde_json::to_value(&runtime).unwrap().get("com").is_none());
    assert_eq!(
        runtime
            .process_address_space_allocator()
            .allocation(allocation_id)
            .unwrap_err()
            .code,
        "address-allocation-id-unknown"
    );
    assert!(
        memory
            .region_snapshots()
            .iter()
            .all(|region| region.range.start() != backing_range.base)
    );
    let release_delta = memory.pending_delta().expect("final COM release delta");
    assert!(
        release_delta
            .mapping_operations
            .iter()
            .any(|operation| matches!(
                operation,
                MappingOperation::Release {
                    address_allocation_id: Some(id),
                    range,
                    ..
                } if *id == allocation_id.0 && range.start() == backing_range.base
            ))
    );
    assert_eq!(runtime.current_last_error().unwrap(), LAST_ERROR_SENTINEL);
}

#[test]
fn malformed_frames_and_output_faults_do_not_mutate_com_state() {
    let mut memory = GuestMemory::new(MemoryLimits::default()).expect("guest memory");
    install_scratch_page(&mut memory);
    let mut runtime = controlled_runtime();
    let installed = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        session
            .install_com_object(synthetic_object_spec())
            .expect("install synthetic COM object")
    };
    acknowledge_initial(&mut memory);
    let synthetic = installed
        .interfaces
        .iter()
        .find(|interface| interface.iid == SYNTHETIC_IID)
        .expect("synthetic interface")
        .clone();

    let before_overflow_runtime = runtime.com().clone();
    let before_overflow_memory = memory.stats();
    let overflow = faulted(dispatch(
        &mut memory,
        &mut runtime,
        &synthetic,
        1,
        GuestAddress(0xffff_fff8),
        GuestAddress(0x0040_2000),
    ));
    assert_eq!(overflow.class, ComFaultClass::InvalidCall);
    assert_eq!(overflow.code, "com-return-stack-pointer-overflow");
    assert_eq!(runtime.com(), &before_overflow_runtime);
    assert_eq!(memory.stats(), before_overflow_memory);

    write_words(
        &mut memory,
        SCRATCH_BASE,
        &[0x0050_0100, installed.interfaces[0].guest_pointer.0],
    );
    acknowledge(&mut memory);
    let before_this_runtime = runtime.com().clone();
    let before_this_memory = memory.stats();
    let mismatched_this = faulted(dispatch(
        &mut memory,
        &mut runtime,
        &synthetic,
        1,
        SCRATCH_BASE,
        GuestAddress(0x0040_2020),
    ));
    assert_eq!(mismatched_this.class, ComFaultClass::InvalidCall);
    assert_eq!(mismatched_this.code, "com-this-interface-mismatch");
    assert_eq!(runtime.com(), &before_this_runtime);
    assert_eq!(memory.stats(), before_this_memory);

    memory
        .write(IID_SYNTHETIC_ADDRESS, &SYNTHETIC_IID.windows_bytes())
        .unwrap();
    write_words(
        &mut memory,
        GuestAddress(SCRATCH_BASE.0 + 0x20),
        &[
            0x0050_0000,
            synthetic.guest_pointer.0,
            IID_SYNTHETIC_ADDRESS.0,
            synthetic.guest_pointer.0,
        ],
    );
    acknowledge(&mut memory);
    let before_output_runtime = runtime.com().clone();
    let before_output_memory = memory.stats();
    let vtable_pointer = memory.read(synthetic.guest_pointer, 4).unwrap();
    let output_fault = faulted(dispatch(
        &mut memory,
        &mut runtime,
        &synthetic,
        0,
        GuestAddress(SCRATCH_BASE.0 + 0x20),
        GuestAddress(0x0040_2040),
    ));
    assert_eq!(output_fault.class, ComFaultClass::InvalidCall);
    assert_eq!(output_fault.target.unwrap().interface, "ISynthetic");
    assert_eq!(runtime.com(), &before_output_runtime);
    assert_eq!(memory.stats(), before_output_memory);
    assert_eq!(
        memory.read(synthetic.guest_pointer, 4).unwrap(),
        vtable_pointer
    );
    assert_eq!(
        runtime
            .com()
            .object(installed.object.id)
            .unwrap()
            .reference_count,
        1
    );
}

#[test]
fn failed_installation_rolls_back_memory_identity_and_address_allocation() {
    let mut memory = GuestMemory::new(MemoryLimits::default()).expect("guest memory");
    install_scratch_page(&mut memory);
    acknowledge_initial(&mut memory);
    let mut runtime = controlled_runtime();
    let before_memory = memory.stats();
    let before_regions = memory.region_snapshots();
    let before_com = runtime.com().clone();
    let before_address_space = runtime.process_address_space_allocator().clone();
    let error = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        session
            .install_com_object(ComObjectSpec {
                class_identity: "fixture.invalid-duplicate-iunknown.v1".to_owned(),
                interfaces: vec![ComInterfaceSpec::iunknown(), ComInterfaceSpec::iunknown()],
            })
            .unwrap_err()
    };
    assert_eq!(error.code, "com-interface-iid-duplicate");
    assert_eq!(memory.stats(), before_memory);
    assert_eq!(memory.region_snapshots(), before_regions);
    assert_eq!(runtime.com(), &before_com);
    assert_eq!(
        runtime.process_address_space_allocator(),
        &before_address_space
    );
}

#[test]
fn failed_final_release_preserves_rust_identity_and_address_allocation() {
    let mut memory = GuestMemory::new(MemoryLimits::default()).expect("guest memory");
    install_scratch_page(&mut memory);
    let mut runtime = controlled_runtime();
    let installed = {
        let mut session = ExecutionSession::new(&mut memory, &mut runtime);
        session
            .install_com_object(ComObjectSpec {
                class_identity: "fixture.release-rollback.v1".to_owned(),
                interfaces: vec![ComInterfaceSpec::iunknown()],
            })
            .expect("install synthetic COM object")
    };
    acknowledge_initial(&mut memory);
    let iunknown = installed.interfaces[0].clone();
    memory
        .release_region(
            installed.object.backing.memory_owner,
            installed.object.backing.range.base,
        )
        .expect("inject missing COM backing");
    write_words(
        &mut memory,
        SCRATCH_BASE,
        &[0x0050_0200, iunknown.guest_pointer.0],
    );
    acknowledge(&mut memory);
    let before_com = runtime.com().clone();
    let before_address_space = runtime.process_address_space_allocator().clone();
    let before_memory = memory.stats();
    let release = faulted(dispatch(
        &mut memory,
        &mut runtime,
        &iunknown,
        2,
        SCRATCH_BASE,
        GuestAddress(0x0040_3000),
    ));
    assert_eq!(release.class, ComFaultClass::InvalidCall);
    assert_eq!(release.target.unwrap().method, "Release");
    assert_eq!(runtime.com(), &before_com);
    assert_eq!(
        runtime.process_address_space_allocator(),
        &before_address_space
    );
    assert_eq!(memory.stats(), before_memory);
    assert_eq!(
        runtime
            .com()
            .object(installed.object.id)
            .unwrap()
            .reference_count,
        1
    );
    assert!(
        runtime
            .process_address_space_allocator()
            .allocation(installed.object.backing.address_allocation_id)
            .is_ok()
    );
}

fn dispatch(
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    interface: &vine_core::RuntimeComInterfaceRecord,
    slot: u16,
    stack_pointer: GuestAddress,
    call_site: GuestAddress,
) -> ComDispatchOutcome {
    let mut session = ExecutionSession::new(memory, runtime);
    session.dispatch_com_call(ComCallTrap {
        endpoint_address: interface.methods[&slot].endpoint,
        stack_pointer,
        return_address: GuestAddress(0x0050_0000 + u32::from(slot) * 0x100),
        call_site,
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

fn prepare_calls(
    memory: &mut GuestMemory,
    synthetic: &vine_core::RuntimeComInterfaceRecord,
    iunknown: &vine_core::RuntimeComInterfaceRecord,
) {
    memory
        .write(
            IID_IUNKNOWN_ADDRESS,
            &RUNTIME_COM_IUNKNOWN_IID.windows_bytes(),
        )
        .unwrap();
    memory
        .write(IID_UNKNOWN_ADDRESS, &UNKNOWN_IID.windows_bytes())
        .unwrap();
    memory
        .write(IID_SYNTHETIC_ADDRESS, &SYNTHETIC_IID.windows_bytes())
        .unwrap();
    write_u32(memory, OUTPUT_KNOWN, 0xaaaa_aaaa);
    write_u32(memory, OUTPUT_UNKNOWN, 0xbbbb_bbbb);
    write_u32(memory, OUTPUT_SYNTHETIC, 0xcccc_cccc);
    write_words(
        memory,
        SCRATCH_BASE,
        &[
            0x0050_0000,
            synthetic.guest_pointer.0,
            IID_IUNKNOWN_ADDRESS.0,
            OUTPUT_KNOWN.0,
        ],
    );
    write_words(
        memory,
        GuestAddress(SCRATCH_BASE.0 + 0x20),
        &[
            0x0050_0000,
            synthetic.guest_pointer.0,
            IID_UNKNOWN_ADDRESS.0,
            OUTPUT_UNKNOWN.0,
        ],
    );
    write_words(
        memory,
        GuestAddress(SCRATCH_BASE.0 + 0x40),
        &[
            0x0050_0000,
            synthetic.guest_pointer.0,
            IID_IUNKNOWN_ADDRESS.0,
            0,
        ],
    );
    write_words(
        memory,
        GuestAddress(SCRATCH_BASE.0 + 0x60),
        &[0x0050_0100, iunknown.guest_pointer.0],
    );
    write_words(
        memory,
        GuestAddress(SCRATCH_BASE.0 + 0x80),
        &[0x0050_0300, synthetic.guest_pointer.0, 0xfeed_beef],
    );
    write_words(
        memory,
        GuestAddress(SCRATCH_BASE.0 + 0xa0),
        &[0x0050_0200, synthetic.guest_pointer.0],
    );
    write_words(
        memory,
        GuestAddress(SCRATCH_BASE.0 + 0xc0),
        &[0x0050_0200, iunknown.guest_pointer.0],
    );
    write_words(
        memory,
        GuestAddress(SCRATCH_BASE.0 + 0xe0),
        &[0x0050_0200, iunknown.guest_pointer.0],
    );
    write_words(
        memory,
        GuestAddress(SCRATCH_BASE.0 + 0x100),
        &[
            0x0050_0000,
            iunknown.guest_pointer.0,
            IID_SYNTHETIC_ADDRESS.0,
            OUTPUT_SYNTHETIC.0,
        ],
    );
    write_words(
        memory,
        GuestAddress(SCRATCH_BASE.0 + 0x120),
        &[0x0050_0200, synthetic.guest_pointer.0],
    );
}

fn install_scratch_page(memory: &mut GuestMemory) {
    let owner = memory
        .create_owner(MemoryOwnerKind::Anonymous)
        .expect("scratch owner");
    let range = memory
        .reserve_fixed(owner, MemoryRegionKind::Anonymous, SCRATCH_BASE, 0x1000)
        .expect("scratch reservation");
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
        .expect("scratch commit");
}

fn synthetic_object_spec() -> ComObjectSpec {
    ComObjectSpec {
        class_identity: "fixture.synthetic-com.v1".to_owned(),
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            ComInterfaceSpec {
                iid: SYNTHETIC_IID,
                name: "ISynthetic".to_owned(),
                methods: vec![
                    ComMethodSpec::query_interface(),
                    ComMethodSpec::add_ref(),
                    ComMethodSpec::release(),
                    ComMethodSpec::unsupported_hresult("DoWork", 1),
                ],
            },
        ],
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
            profile_id: "com-iunknown-fixture".to_owned(),
            bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
        },
        process_image_path_a: RuntimeProcessImagePathA {
            source_authority: "com-iunknown-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\\com-fixture.exe\0".to_vec(),
            encoding: RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: RuntimeCurrentDirectoryA {
            source_authority: "com-iunknown-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_cookie: 0xec01_9d3b,
    })
    .expect("controlled runtime");
    runtime
        .start_initial_thread()
        .expect("start initial thread");
    runtime
}

fn acknowledge_initial(memory: &mut GuestMemory) {
    let reflection = memory
        .pending_reflection()
        .expect("initial memory reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge initial memory reflection");
}

fn acknowledge(memory: &mut GuestMemory) {
    let delta = memory.pending_delta().expect("pending memory delta");
    memory
        .acknowledge_delta(delta.generation)
        .expect("acknowledge memory delta");
}

fn write_words(memory: &mut GuestMemory, address: GuestAddress, words: &[u32]) {
    let bytes = words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<_>>();
    memory.write(address, &bytes).expect("write call frame");
}

fn write_u32(memory: &mut GuestMemory, address: GuestAddress, value: u32) {
    memory
        .write(address, &value.to_le_bytes())
        .expect("write u32");
}

fn read_u32(memory: &GuestMemory, address: GuestAddress) -> u32 {
    u32::from_le_bytes(memory.read(address, 4).unwrap().try_into().unwrap())
}

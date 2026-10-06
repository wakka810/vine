use crate::com::{ComInterfaceSpec, ComMethodSpec, ComObjectSpec};
use crate::{
    COM_E_NOINTERFACE, COM_E_POINTER, COM_S_OK, ComCallCompletion, ComCallTrap, ComDispatchOutcome,
    GuestAddress, GuestMemory, MappingOperation, MemoryAccess, MemoryLimits, MemoryOwnerKind,
    MemoryRegionKind, PageProtection, ProcessAddressAllocationPurpose, RUNTIME_COM_IUNKNOWN_IID,
    Runtime, RuntimeBootstrap, RuntimeClockState, RuntimeComIid, RuntimeCurrentDirectoryA,
    RuntimeProcessId, RuntimeProcessImagePathA, RuntimeStartupInfo, RuntimeThreadId,
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
    let mut runtime = controlled_runtime(memory);
    assert!(runtime.com().is_empty());
    let installed = runtime
        .install_com_object(synthetic_object_spec())
        .expect("install synthetic COM object");
    let iunknown = runtime
        .com()
        .interface_for_object_iid(installed.object_id, RUNTIME_COM_IUNKNOWN_IID)
        .unwrap()
        .clone();
    let synthetic = runtime
        .com()
        .interface_for_object_iid(installed.object_id, SYNTHETIC_IID)
        .unwrap()
        .clone();
    assert_ne!(iunknown.id, synthetic.id);
    assert_eq!(iunknown.object_id, synthetic.object_id);
    assert_eq!(
        runtime
            .com()
            .object(installed.object_id)
            .unwrap()
            .reference_count,
        1
    );
    assert_eq!(
        runtime
            .com()
            .object(installed.object_id)
            .unwrap()
            .canonical_iunknown_interface_id,
        iunknown.id
    );
    let allocation_id = runtime
        .com()
        .object(installed.object_id)
        .unwrap()
        .backing
        .address_allocation_id;
    let backing_range = runtime
        .com()
        .object(installed.object_id)
        .unwrap()
        .backing
        .range;
    assert_eq!(
        runtime
            .process_address_space_allocator()
            .allocation(allocation_id)
            .expect("COM address allocation")
            .purpose,
        ProcessAddressAllocationPurpose::ComEndpoint
    );
    assert!(!runtime.com().is_empty());
    let endpoint = synthetic.methods[&0].endpoint;
    assert!(runtime.resolve_com_method(endpoint).unwrap().is_some());
    assert!(
        runtime
            .resolve_com_method(GuestAddress(endpoint.0 + 1))
            .unwrap()
            .is_none()
    );

    prepare_calls(&mut runtime.memory, &synthetic, &iunknown);
    acknowledge_initial(&mut runtime.memory);
    assert_eq!(
        runtime
            .memory
            .read(synthetic.guest_pointer, 4)
            .expect("synthetic interface pointer"),
        synthetic.vtable.0.to_le_bytes()
    );
    assert!(
        runtime
            .memory
            .validate_access(synthetic.guest_pointer, 4, MemoryAccess::Read)
            .is_ok()
    );
    assert!(
        runtime
            .memory
            .validate_access(synthetic.guest_pointer, 4, MemoryAccess::Write)
            .is_err()
    );
    assert!(
        runtime
            .memory
            .validate_access(synthetic.methods[&0].endpoint, 2, MemoryAccess::Execute)
            .is_ok()
    );
    assert!(
        runtime
            .memory
            .validate_access(synthetic.methods[&0].endpoint, 2, MemoryAccess::Write)
            .is_err()
    );
    assert_eq!(
        runtime
            .memory
            .read(synthetic.methods[&0].endpoint, 2)
            .expect("owned COM endpoint prefix"),
        [0x0f, 0x0b]
    );
    assert_eq!(
        u32::from_le_bytes(
            runtime
                .memory
                .read(GuestAddress(synthetic.vtable.0 + 12), 4)
                .expect("unsupported vtable slot")
                .try_into()
                .unwrap()
        ),
        synthetic.methods[&3].endpoint.0
    );

    let known = completed(dispatch(
        &mut runtime,
        &synthetic,
        0,
        SCRATCH_BASE,
        GuestAddress(0x0040_1000),
    ));
    assert_eq!(known.eax, COM_S_OK.bits());
    assert_eq!(known.eax, 0);
    assert_eq!(known.stack_pointer, GuestAddress(SCRATCH_BASE.0 + 16));
    assert_eq!(known.instruction_pointer, GuestAddress(0x0050_0000));
    assert!(known.presented_device.is_none());
    assert_eq!(
        runtime
            .com()
            .object(installed.object_id)
            .unwrap()
            .reference_count,
        2
    );
    assert_eq!(runtime.current_last_error().unwrap(), LAST_ERROR_SENTINEL);
    acknowledge(&mut runtime.memory);
    assert_eq!(
        read_u32(&runtime.memory, OUTPUT_KNOWN),
        iunknown.guest_pointer.0
    );

    let round_trip = completed(dispatch(
        &mut runtime,
        &iunknown,
        0,
        GuestAddress(SCRATCH_BASE.0 + 0x100),
        GuestAddress(0x0040_1010),
    ));
    assert_eq!(round_trip.eax, COM_S_OK.bits());
    assert_eq!(
        runtime
            .com()
            .object(installed.object_id)
            .unwrap()
            .reference_count,
        3
    );
    acknowledge(&mut runtime.memory);
    assert_eq!(
        read_u32(&runtime.memory, OUTPUT_SYNTHETIC),
        synthetic.guest_pointer.0
    );

    let unknown = completed(dispatch(
        &mut runtime,
        &synthetic,
        0,
        GuestAddress(SCRATCH_BASE.0 + 0x20),
        GuestAddress(0x0040_1020),
    ));
    assert_eq!(unknown.eax, COM_E_NOINTERFACE.bits());
    assert!(!COM_E_NOINTERFACE.succeeded());
    assert_eq!(
        runtime
            .com()
            .object(installed.object_id)
            .unwrap()
            .reference_count,
        3
    );
    acknowledge(&mut runtime.memory);
    assert_eq!(read_u32(&runtime.memory, OUTPUT_UNKNOWN), 0);

    let null_output = completed(dispatch(
        &mut runtime,
        &synthetic,
        0,
        GuestAddress(SCRATCH_BASE.0 + 0x40),
        GuestAddress(0x0040_1040),
    ));
    assert_eq!(null_output.eax, COM_E_POINTER.bits());
    assert_eq!(
        runtime
            .com()
            .object(installed.object_id)
            .unwrap()
            .reference_count,
        3
    );

    let add_ref = completed(dispatch(
        &mut runtime,
        &iunknown,
        1,
        GuestAddress(SCRATCH_BASE.0 + 0x60),
        GuestAddress(0x0040_1060),
    ));
    assert_eq!(add_ref.eax, 4);
    assert_eq!(add_ref.stack_pointer, GuestAddress(SCRATCH_BASE.0 + 0x68));
    assert_eq!(add_ref.instruction_pointer, GuestAddress(0x0050_0100));
    assert!(add_ref.presented_device.is_none());

    let before_unsupported_memory = runtime.memory.stats();
    let unsupported = faulted(dispatch(
        &mut runtime,
        &synthetic,
        3,
        GuestAddress(SCRATCH_BASE.0 + 0x80),
        GuestAddress(0x0040_1080),
    ));
    assert_eq!(unsupported.call_site, GuestAddress(0x0040_1080));
    let unsupported_target = unsupported.target.expect("structured COM target");
    assert_eq!(unsupported_target.interface, "ISynthetic");
    assert_eq!(unsupported_target.vtable_slot, 3);
    assert_eq!(unsupported_target.method, "DoWork");
    assert_eq!(runtime.memory.stats(), before_unsupported_memory);
    assert_eq!(runtime.com().objects().len(), 1);
    assert_eq!(runtime.com().interfaces().len(), 2);
    assert_eq!(
        runtime
            .com()
            .object(installed.object_id)
            .unwrap()
            .reference_count,
        4
    );
    assert_eq!(
        runtime
            .com()
            .interface_for_pointer(synthetic.guest_pointer)
            .unwrap()
            .unwrap()
            .id,
        synthetic.id
    );
    assert_eq!(
        runtime
            .com()
            .interface_for_object_iid(installed.object_id, RUNTIME_COM_IUNKNOWN_IID)
            .unwrap()
            .id,
        iunknown.id
    );
    assert_eq!(runtime.current_last_error().unwrap(), LAST_ERROR_SENTINEL);

    for (offset, interface, expected) in [
        (0xa0, &synthetic, 3),
        (0xc0, &iunknown, 2),
        (0xe0, &iunknown, 1),
        (0x120, &synthetic, 0),
    ] {
        let release = completed(dispatch(
            &mut runtime,
            interface,
            2,
            GuestAddress(SCRATCH_BASE.0 + offset),
            GuestAddress(0x0040_1100 + offset),
        ));
        assert_eq!(release.eax, expected);
        if expected == 0 {
            assert!(runtime.com().object(installed.object_id).is_err());
        } else {
            assert_eq!(
                runtime
                    .com()
                    .object(installed.object_id)
                    .unwrap()
                    .reference_count,
                expected
            );
        }
    }
    assert!(runtime.com().is_empty());
    assert!(runtime.resolve_com_method(endpoint).unwrap().is_none());
    faulted(dispatch(
        &mut runtime,
        &synthetic,
        0,
        SCRATCH_BASE,
        GuestAddress(0x0040_1200),
    ));
    assert!(
        runtime
            .process_address_space_allocator()
            .allocation(allocation_id)
            .is_err()
    );
    assert!(
        runtime
            .memory
            .regions()
            .all(|region| region.range.start() != backing_range.base)
    );
    let release_delta = runtime
        .memory
        .pending_delta()
        .expect("final COM release delta");
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
fn checked_interface_output_rejects_then_retries_without_publishing_a_reference() {
    let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
    install_scratch_page(&mut memory);
    let mut runtime = controlled_runtime(memory);
    let installed = runtime.install_com_object(synthetic_object_spec()).unwrap();
    let interface = runtime
        .com()
        .interface_for_object_iid(installed.object_id, SYNTHETIC_IID)
        .unwrap();
    let (interface_id, pointer) = (interface.id, interface.guest_pointer);
    acknowledge_initial(&mut runtime.memory);
    write_u32(&mut runtime.memory, OUTPUT_SYNTHETIC, 0xcccc_cccc);
    acknowledge(&mut runtime.memory);
    let before = runtime.memory.stats();
    let backing_bytes = runtime.memory.read(pointer, 4).unwrap();

    // COM backing is read-only; rejection must not acquire a reference.
    assert!(
        runtime
            .add_ref_interface_with_guest_pointer_write(interface_id, pointer)
            .is_err()
    );
    assert_eq!(runtime.memory.stats(), before);
    assert_eq!(runtime.memory.read(pointer, 4).unwrap(), backing_bytes);
    assert_eq!(read_u32(&runtime.memory, OUTPUT_SYNTHETIC), 0xcccc_cccc);
    assert_eq!(
        runtime
            .com()
            .object(installed.object_id)
            .unwrap()
            .reference_count,
        1
    );
    assert_eq!(
        runtime.com().interface(interface_id).unwrap().guest_pointer,
        pointer
    );
    assert_eq!(runtime.current_last_error().unwrap(), LAST_ERROR_SENTINEL);

    let count = runtime
        .add_ref_interface_with_guest_pointer_write(interface_id, OUTPUT_SYNTHETIC)
        .unwrap();
    assert_eq!(count, 2);
    acknowledge(&mut runtime.memory);
    assert_eq!(read_u32(&runtime.memory, OUTPUT_SYNTHETIC), pointer.0);
    assert_eq!(
        runtime
            .com()
            .object(installed.object_id)
            .unwrap()
            .reference_count,
        2
    );
    assert_eq!(runtime.current_last_error().unwrap(), LAST_ERROR_SENTINEL);
}

#[test]
fn malformed_frames_and_output_faults_do_not_mutate_com_state() {
    let mut memory = GuestMemory::new(MemoryLimits::default()).expect("guest memory");
    install_scratch_page(&mut memory);
    let mut runtime = controlled_runtime(memory);
    let installed = runtime
        .install_com_object(synthetic_object_spec())
        .expect("install synthetic COM object");
    acknowledge_initial(&mut runtime.memory);
    let synthetic = runtime
        .com()
        .interface_for_object_iid(installed.object_id, SYNTHETIC_IID)
        .unwrap()
        .clone();

    let before_overflow_memory = runtime.memory.stats();
    faulted(dispatch(
        &mut runtime,
        &synthetic,
        1,
        GuestAddress(0xffff_fff8),
        GuestAddress(0x0040_2000),
    ));
    assert_eq!(
        runtime
            .com()
            .object(installed.object_id)
            .unwrap()
            .reference_count,
        1
    );
    assert_eq!(
        runtime
            .com()
            .interface_for_pointer(synthetic.guest_pointer)
            .unwrap()
            .unwrap()
            .id,
        synthetic.id
    );
    assert_eq!(runtime.memory.stats(), before_overflow_memory);

    write_words(
        &mut runtime.memory,
        SCRATCH_BASE,
        &[0x0050_0100, installed.interfaces[0].1.0],
    );
    acknowledge(&mut runtime.memory);
    let before_this_memory = runtime.memory.stats();
    faulted(dispatch(
        &mut runtime,
        &synthetic,
        1,
        SCRATCH_BASE,
        GuestAddress(0x0040_2020),
    ));
    assert_eq!(
        runtime
            .com()
            .object(installed.object_id)
            .unwrap()
            .reference_count,
        1
    );
    assert_eq!(
        runtime
            .com()
            .interface_for_pointer(synthetic.guest_pointer)
            .unwrap()
            .unwrap()
            .id,
        synthetic.id
    );
    assert_eq!(runtime.memory.stats(), before_this_memory);

    runtime
        .memory
        .write(IID_SYNTHETIC_ADDRESS, &SYNTHETIC_IID.windows_bytes())
        .unwrap();
    write_words(
        &mut runtime.memory,
        GuestAddress(SCRATCH_BASE.0 + 0x20),
        &[
            0x0050_0000,
            synthetic.guest_pointer.0,
            IID_SYNTHETIC_ADDRESS.0,
            synthetic.guest_pointer.0,
        ],
    );
    acknowledge(&mut runtime.memory);
    let before_output_memory = runtime.memory.stats();
    let vtable_pointer = runtime.memory.read(synthetic.guest_pointer, 4).unwrap();
    let output_fault = faulted(dispatch(
        &mut runtime,
        &synthetic,
        0,
        GuestAddress(SCRATCH_BASE.0 + 0x20),
        GuestAddress(0x0040_2040),
    ));
    assert_eq!(output_fault.target.unwrap().interface, "ISynthetic");
    assert_eq!(runtime.com().objects().len(), 1);
    assert_eq!(runtime.com().interfaces().len(), 2);
    assert_eq!(
        runtime
            .com()
            .interface_for_pointer(synthetic.guest_pointer)
            .unwrap()
            .unwrap()
            .id,
        synthetic.id
    );
    assert_eq!(runtime.current_last_error().unwrap(), LAST_ERROR_SENTINEL);
    assert_eq!(runtime.memory.stats(), before_output_memory);
    assert_eq!(
        runtime.memory.read(synthetic.guest_pointer, 4).unwrap(),
        vtable_pointer
    );
    assert_eq!(
        runtime
            .com()
            .object(installed.object_id)
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
    let mut runtime = controlled_runtime(memory);
    let before_memory = runtime.memory.stats();
    let before_regions = runtime.memory.regions().collect::<Vec<_>>();
    assert!(runtime.com().is_empty());
    assert!(
        runtime
            .process_address_space_allocator()
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::ComEndpoint)
            .is_err()
    );
    runtime
        .install_com_object(ComObjectSpec {
            class: crate::RuntimeComClass::D3d9,
            interfaces: vec![ComInterfaceSpec::iunknown(), ComInterfaceSpec::iunknown()],
        })
        .unwrap_err();

    assert_eq!(runtime.memory.stats(), before_memory);
    assert_eq!(runtime.memory.regions().collect::<Vec<_>>(), before_regions);
    assert!(runtime.com().is_empty());
    assert!(runtime.com().interfaces().is_empty());
    assert!(
        runtime
            .process_address_space_allocator()
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::ComEndpoint)
            .is_err()
    );
    assert_eq!(runtime.current_last_error().unwrap(), LAST_ERROR_SENTINEL);
    let installed = runtime.install_com_object(synthetic_object_spec()).unwrap();
    assert_eq!(runtime.com().objects().len(), 1);
    assert_eq!(
        runtime
            .com()
            .object(installed.object_id)
            .unwrap()
            .reference_count,
        1
    );
}

#[test]
fn failed_final_release_preserves_rust_identity_and_address_allocation() {
    let mut memory = GuestMemory::new(MemoryLimits::default()).expect("guest memory");
    install_scratch_page(&mut memory);
    let mut runtime = controlled_runtime(memory);
    let installed = runtime
        .install_com_object(ComObjectSpec {
            class: crate::RuntimeComClass::D3d9,
            interfaces: vec![ComInterfaceSpec::iunknown()],
        })
        .expect("install synthetic COM object");
    acknowledge_initial(&mut runtime.memory);
    let iunknown = runtime
        .com()
        .interface_for_pointer(installed.interfaces[0].1)
        .unwrap()
        .unwrap()
        .clone();
    let backing = &runtime.com().object(installed.object_id).unwrap().backing;
    let (owner, allocation_id, backing_range) = (
        backing.memory_owner,
        backing.address_allocation_id,
        backing.range,
    );
    // An unexpected second reservation prevents owner destruction, after the
    // transaction has already released the real backing. Both must survive.
    let extra = runtime
        .memory
        .reserve_fixed(
            owner,
            MemoryRegionKind::Runtime,
            GuestAddress(SCRATCH_BASE.0 + 0x10000),
            0x1000,
        )
        .unwrap();
    write_words(
        &mut runtime.memory,
        SCRATCH_BASE,
        &[0x0050_0200, iunknown.guest_pointer.0],
    );
    acknowledge(&mut runtime.memory);
    let before_memory = runtime.memory.stats();
    let pointer_bytes = runtime.memory.read(iunknown.guest_pointer, 4).unwrap();
    let endpoint = iunknown.methods[&2].endpoint;
    let endpoint_bytes = runtime.memory.read(endpoint, 10).unwrap();
    let release = faulted(dispatch(
        &mut runtime,
        &iunknown,
        2,
        SCRATCH_BASE,
        GuestAddress(0x0040_3000),
    ));
    assert_eq!(release.target.unwrap().method, "Release");
    assert_eq!(runtime.com().objects().len(), 1);
    assert_eq!(runtime.com().interfaces().len(), 1);
    assert_eq!(
        runtime
            .com()
            .interface_for_pointer(iunknown.guest_pointer)
            .unwrap()
            .unwrap()
            .id,
        iunknown.id
    );
    assert_eq!(runtime.memory.stats(), before_memory);
    assert_eq!(
        runtime.memory.read(iunknown.guest_pointer, 4).unwrap(),
        pointer_bytes
    );
    assert_eq!(runtime.memory.read(endpoint, 10).unwrap(), endpoint_bytes);
    assert_eq!(
        runtime
            .process_address_space_allocator()
            .allocation(allocation_id)
            .unwrap()
            .range,
        backing_range
    );
    assert_eq!(
        runtime
            .com()
            .object(installed.object_id)
            .unwrap()
            .reference_count,
        1
    );
    assert_eq!(runtime.current_last_error().unwrap(), LAST_ERROR_SENTINEL);
    runtime.memory.release_region(owner, extra.start()).unwrap();
    acknowledge(&mut runtime.memory);
    let release = completed(dispatch(
        &mut runtime,
        &iunknown,
        2,
        SCRATCH_BASE,
        GuestAddress(0x0040_3000),
    ));
    assert_eq!(release.eax, 0);
    assert!(runtime.com().is_empty());
    assert!(runtime.com().interfaces().is_empty());
    assert!(
        runtime
            .com()
            .interface_for_pointer(iunknown.guest_pointer)
            .unwrap()
            .is_none()
    );
    assert!(runtime.resolve_com_method(endpoint).unwrap().is_none());
    assert!(
        runtime
            .process_address_space_allocator()
            .allocation(allocation_id)
            .is_err()
    );
    assert!(runtime.memory.read(iunknown.guest_pointer, 4).is_err());
    assert!(runtime.memory.owner_is_sealed(owner).is_err());
    assert_eq!(runtime.current_last_error().unwrap(), LAST_ERROR_SENTINEL);
}

fn dispatch(
    runtime: &mut Runtime,
    interface: &crate::RuntimeComInterfaceRecord,
    slot: u16,
    stack_pointer: GuestAddress,
    call_site: GuestAddress,
) -> ComDispatchOutcome {
    runtime.dispatch_com_call(ComCallTrap {
        endpoint_address: interface.methods[&slot].endpoint,
        stack_pointer,
        return_address: GuestAddress(0x0050_0000 + u32::from(slot) * 0x100),
        call_site,
    })
}

fn completed(outcome: ComDispatchOutcome) -> ComCallCompletion {
    match outcome {
        ComDispatchOutcome::AudioPending(pending) => {
            panic!("unexpected browser audio pending: {pending:?}")
        }
        ComDispatchOutcome::Completed(completion) => completion,
        ComDispatchOutcome::Fault(fault) => panic!("unexpected COM fault: {fault:?}"),
        other @ (ComDispatchOutcome::GuestCallback(_)
        | ComDispatchOutcome::MediaPending(_)
        | ComDispatchOutcome::MediaData(_)) => panic!("unexpected COM continuation: {other:?}"),
    }
}

fn faulted(outcome: ComDispatchOutcome) -> crate::ComFault {
    match outcome {
        ComDispatchOutcome::AudioPending(pending) => {
            panic!("unexpected browser audio pending: {pending:?}")
        }
        ComDispatchOutcome::Fault(fault) => fault,
        ComDispatchOutcome::Completed(completion) => {
            panic!("unexpected COM completion: {completion:?}")
        }
        other @ (ComDispatchOutcome::GuestCallback(_)
        | ComDispatchOutcome::MediaPending(_)
        | ComDispatchOutcome::MediaData(_)) => panic!("unexpected COM continuation: {other:?}"),
    }
}

fn prepare_calls(
    memory: &mut GuestMemory,
    synthetic: &crate::RuntimeComInterfaceRecord,
    iunknown: &crate::RuntimeComInterfaceRecord,
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
        class: crate::RuntimeComClass::D3d9,
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            ComInterfaceSpec {
                iid: SYNTHETIC_IID,
                name: "ISynthetic".to_owned(),
                methods: vec![
                    ComMethodSpec::query_interface(),
                    ComMethodSpec::add_ref(),
                    ComMethodSpec::release(),
                    ComMethodSpec::unsupported("DoWork"),
                ],
            },
        ],
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
                bytes_with_nul: b"C:\\Vine\\com-fixture.exe\0".to_vec(),
            },
            initial_current_directory_a: RuntimeCurrentDirectoryA {
                bytes_with_nul: b"C:\\Vine\0".to_vec(),
            },
            encoded_pointer_cookie: 0xec01_9d3b,
        },
        memory,
    )
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

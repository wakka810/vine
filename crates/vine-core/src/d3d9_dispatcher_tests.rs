use crate as vine_core;
use crate::com::ComCallCompletion;
use crate::d3d9::{
    D3D9_CREATE_HARDWARE_VERTEX_PROCESSING_MULTITHREADED, D3D9_D3DDEVTYPE_HAL,
    D3D9_D3DERR_INVALIDCALL, D3D9_D3DFMT_UNKNOWN, D3D9_D3DFMT_X8R8G8B8, D3D9_D3DMULTISAMPLE_NONE,
    D3D9_D3DPRESENT_PARAMETERS_BYTE_LENGTH, D3D9_D3DSWAPEFFECT_DISCARD, D3D9_IDIRECT3D9_IID,
    D3D9_SDK_VERSION, dispatch_d3d9_api,
};
use crate::imports::{ImportThunk, ImportThunkDisposition, ImportThunkId};
use crate::runtime::{RuntimeComClass, X86_STARTUP_INFO_W_BYTE_LENGTH};
use vine_core::{
    ApiDispatcher, ApiEndpointIdentity, ApiOwnedTrap, ComCallTrap, ComDispatchOutcome,
    DispatchOutcome, GuestAddress, GuestMemory, ImportTarget, MemoryLimits, MemoryOwnerId,
    MemoryOwnerKind, MemoryRegionKind, PageProtection, Runtime, RuntimeBootstrap,
    RuntimeClockState, RuntimeCurrentDirectoryA, RuntimeDisplayProfile, RuntimeProcessId,
    RuntimeProcessImagePathA, RuntimeRect, RuntimeStartupInfo, RuntimeThreadId,
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
fn generic_d3d9_display_queries_use_rust_monitor_and_checked_guest_outputs() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    let root = fixture.dispatch_factory(STACK_VALID, D3D9_SDK_VERSION);
    let pointer = match root.eax {
        Some(value) => value,
        _ => panic!("no D3D9 root"),
    };
    let interface = fixture
        .runtime
        .com_interface_for_pointer(GuestAddress(pointer))
        .unwrap()
        .expect("root interface")
        .clone();
    let output = GuestAddress(MEMORY_BASE.0 + 0x800);
    write_words(
        &mut fixture.runtime.memory,
        STACK_COM,
        &[RETURN_ADDRESS.0, pointer, 0, output.0],
    );
    acknowledge_pending(&mut fixture.runtime.memory);
    let call = ComCallTrap {
        endpoint_address: interface.methods[&8].endpoint,
        stack_pointer: STACK_COM,
        return_address: RETURN_ADDRESS,
        call_site: RETURN_ADDRESS,
    };
    let outcome = { fixture.runtime.dispatch_com_call(call) };
    assert!(
        matches!(outcome, ComDispatchOutcome::Completed(_)),
        "{outcome:?}"
    );
    acknowledge_pending(&mut fixture.runtime.memory);
    let mode = fixture
        .runtime
        .memory
        .read(output, 16)
        .expect("D3DDISPLAYMODE");
    assert_eq!(u32::from_le_bytes(mode[0..4].try_into().unwrap()), 1920);
    assert_eq!(u32::from_le_bytes(mode[4..8].try_into().unwrap()), 1080);
    assert_eq!(u32::from_le_bytes(mode[8..12].try_into().unwrap()), 60);
    assert_eq!(
        u32::from_le_bytes(mode[12..16].try_into().unwrap()),
        D3D9_D3DFMT_X8R8G8B8
    );
    write_words(
        &mut fixture.runtime.memory,
        STACK_COM,
        &[RETURN_ADDRESS.0, pointer, 1, output.0],
    );
    acknowledge_pending(&mut fixture.runtime.memory);
    let outcome = { fixture.runtime.dispatch_com_call(call) };
    assert!(
        matches!(outcome, ComDispatchOutcome::Completed(ref completion)
        if completion.eax == D3D9_D3DERR_INVALIDCALL.0 as u32)
    );
    assert_eq!(fixture.runtime.memory.read(output, 16).unwrap(), mode);
}

#[test]
fn direct_adapter_queries_use_live_monitor_and_check_full_guest_output_before_writing() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    let root = fixture.dispatch_factory(STACK_VALID, D3D9_SDK_VERSION);
    let Some(pointer) = root.eax else {
        panic!("no D3D9 root")
    };
    let interface = fixture
        .runtime
        .com_interface_for_pointer(GuestAddress(pointer))
        .unwrap()
        .unwrap()
        .clone();
    let output = GuestAddress(MEMORY_BASE.0 + 0x800);
    let invoke = |fixture: &mut Fixture, slot: u16, args: &[u32]| {
        let mut words = vec![RETURN_ADDRESS.0, pointer];
        words.extend_from_slice(args);
        write_words(&mut fixture.runtime.memory, STACK_COM, &words);
        acknowledge_pending(&mut fixture.runtime.memory);
        let result = fixture.runtime.dispatch_com_call(ComCallTrap {
            endpoint_address: interface.methods[&slot].endpoint,
            stack_pointer: STACK_COM,
            return_address: RETURN_ADDRESS,
            call_site: RETURN_ADDRESS,
        });
        if fixture.runtime.memory.has_pending_delta() {
            acknowledge_pending(&mut fixture.runtime.memory);
        }
        result
    };
    assert_eq!(completed(invoke(&mut fixture, 4, &[])).eax, 1);
    let monitor = fixture.runtime.primary_monitor().unwrap().handle;
    assert_eq!(completed(invoke(&mut fixture, 15, &[0])).eax, monitor);
    assert_eq!(completed(invoke(&mut fixture, 15, &[1])).eax, 0);

    assert_eq!(completed(invoke(&mut fixture, 5, &[0, 0, output.0])).eax, 0);
    let identifier = fixture.runtime.memory.read(output, 1_100).unwrap();
    assert!(identifier.starts_with(b"vine-d3d9.dll\0"));
    assert!(identifier[1_024..1_056].starts_with(b"\\\\.\\DISPLAY1\0"));
    assert_eq!(
        completed(invoke(
            &mut fixture,
            14,
            &[0, D3D9_D3DDEVTYPE_HAL, output.0]
        ))
        .eax,
        0
    );
    let caps = fixture.runtime.memory.read(output, 304).unwrap();
    assert_eq!(
        u32::from_le_bytes(caps[0..4].try_into().unwrap()),
        D3D9_D3DDEVTYPE_HAL
    );
    assert_eq!(
        u32::from_le_bytes(caps[8..12].try_into().unwrap()),
        0x0002_0000
    );
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );

    // An invalid adapter must not alter guest bytes or Windows state.
    let object_id = fixture.runtime.com().objects().values().next().unwrap().id;
    let before_bytes = fixture.runtime.memory.read(output, 1_100).unwrap();
    let invalid = completed(invoke(&mut fixture, 5, &[1, 0, output.0]));
    assert_eq!(invalid.eax, D3D9_D3DERR_INVALIDCALL.0 as u32);
    assert_eq!(
        fixture.runtime.memory.read(output, 1_100).unwrap(),
        before_bytes
    );
    assert_eq!(fixture.runtime.com().objects().len(), 1);
    assert_eq!(
        fixture.runtime.com().objects()[&object_id].reference_count,
        1
    );
    assert_eq!(
        fixture
            .runtime
            .com_interface_for_pointer(interface.guest_pointer)
            .unwrap()
            .unwrap()
            .id,
        interface.id
    );
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );

    // COM backing is read-only: a large output may not partially overwrite it.
    let before_backing = fixture
        .runtime
        .memory
        .read(interface.guest_pointer, 4)
        .unwrap();
    let before_memory = fixture.runtime.memory.stats();
    let result = invoke(
        &mut fixture,
        14,
        &[0, D3D9_D3DDEVTYPE_HAL, interface.guest_pointer.0],
    );
    assert!(matches!(result, ComDispatchOutcome::Fault(_)));
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(interface.guest_pointer, 4)
            .unwrap(),
        before_backing
    );
    assert_eq!(fixture.runtime.memory.stats().owners, before_memory.owners);
    assert_eq!(
        fixture.runtime.memory.stats().committed_pages,
        before_memory.committed_pages
    );
    assert_eq!(
        fixture.runtime.com().objects()[&object_id].reference_count,
        1
    );
    assert_eq!(
        fixture
            .runtime
            .com_interface_for_pointer(interface.guest_pointer)
            .unwrap()
            .unwrap()
            .id,
        interface.id
    );
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
    assert_eq!(
        completed(invoke(
            &mut fixture,
            14,
            &[0, D3D9_D3DDEVTYPE_HAL, output.0]
        ))
        .eax,
        0
    );
    assert_eq!(fixture.runtime.memory.read(output, 304).unwrap(), caps);
}

#[test]
fn generic_create_device_rejects_missing_window_without_publishing_com_or_guest_writes() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    let root = fixture.dispatch_factory(STACK_VALID, D3D9_SDK_VERSION);
    let Some(pointer) = root.eax else {
        panic!("no D3D9 root")
    };
    let interface = fixture
        .runtime
        .com_interface_for_pointer(GuestAddress(pointer))
        .unwrap()
        .unwrap()
        .clone();
    let present = GuestAddress(MEMORY_BASE.0 + 0x800);
    let output = GuestAddress(MEMORY_BASE.0 + 0x900);
    write_present_parameters(&mut fixture.runtime.memory, present, UNKNOWN_WINDOW);
    write_words(
        &mut fixture.runtime.memory,
        STACK_COM,
        &[
            RETURN_ADDRESS.0,
            pointer,
            0,
            D3D9_D3DDEVTYPE_HAL,
            UNKNOWN_WINDOW,
            D3D9_CREATE_HARDWARE_VERTEX_PROCESSING_MULTITHREADED,
            present.0,
            output.0,
        ],
    );
    acknowledge_pending(&mut fixture.runtime.memory);
    let before = fixture.runtime.com().objects().len();
    let trap = ComCallTrap {
        endpoint_address: interface.methods[&16].endpoint,
        stack_pointer: STACK_COM,
        return_address: RETURN_ADDRESS,
        call_site: RETURN_ADDRESS,
    };
    let result = { fixture.runtime.dispatch_com_call(trap) };
    assert!(
        matches!(result, ComDispatchOutcome::Completed(ref completion)
                if completion.eax == D3D9_D3DERR_INVALIDCALL.0 as u32
        ),
        "{result:?}"
    );
    assert_eq!(fixture.runtime.com().objects().len(), before);
}

#[test]
fn direct3d_create9_installs_bounded_root_identity_and_retires_atomically() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    write_words(
        &mut fixture.runtime.memory,
        STACK_INVALID,
        &[RETURN_ADDRESS.0, 0],
    );
    acknowledge_pending(&mut fixture.runtime.memory);
    let before_invalid_memory = fixture.runtime.memory.stats();
    assert!(fixture.runtime.com().is_empty());
    let invalid = match fixture.dispatch(STACK_INVALID) {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("invalid SDK did not return NULL: {other:?}"),
    };
    assert_eq!(invalid.eax, Some(0));
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
    assert_eq!(fixture.runtime.memory.stats(), before_invalid_memory);
    assert!(fixture.runtime.com().is_empty());
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );

    let first = fixture.dispatch_factory(STACK_VALID, D3D9_SDK_VERSION);
    let pointer = match first.eax {
        Some(pointer) => pointer,
        other => panic!("Direct3DCreate9 did not set EAX: {other:?}"),
    };
    assert_ne!(pointer, 0);
    assert_eq!(first.stack_pointer, GuestAddress(STACK_VALID.0 + 8));
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
    let object = fixture
        .runtime
        .com()
        .objects()
        .values()
        .next()
        .expect("D3D9 root COM object")
        .clone();
    assert_eq!(object.class, RuntimeComClass::D3d9);
    assert_eq!(object.reference_count, 1);
    let interface = fixture
        .runtime
        .com_interface_for_pointer(GuestAddress(pointer))
        .unwrap()
        .expect("IDirect3D9 pointer")
        .clone();
    assert_eq!(interface.iid, D3D9_IDIRECT3D9_IID);
    assert_eq!(interface.name, "IDirect3D9");
    assert_eq!(interface.methods.len(), 17);
    assert_eq!(interface.methods[&4].name, "GetAdapterCount");

    assert_eq!(interface.methods[&5].name, "GetAdapterIdentifier");

    assert_eq!(interface.methods[&14].name, "GetDeviceCaps");

    assert_eq!(interface.methods[&15].name, "GetAdapterMonitor");

    assert_eq!(interface.methods[&9].name, "CheckDeviceType");

    assert_eq!(interface.methods[&10].name, "CheckDeviceFormat");

    assert_eq!(interface.methods[&8].name, "GetAdapterDisplayMode");

    assert_eq!(interface.methods[&16].name, "CreateDevice");

    fixture.acknowledge();
    assert_eq!(
        u32::from_le_bytes(
            fixture
                .runtime
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
            .runtime
            .memory
            .read(interface.methods[&8].endpoint, 2)
            .expect("IDirect3D9 slot 8 endpoint"),
        [0x0f, 0x0b]
    );
    write_words(
        &mut fixture.runtime.memory,
        STACK_COM,
        &[0x1005_b8d6, pointer, 0, MEMORY_BASE.0 + 0x280],
    );
    acknowledge_pending(&mut fixture.runtime.memory);
    let before_unsupported_memory = fixture.runtime.memory.stats();
    let output_before = fixture
        .runtime
        .memory
        .read(GuestAddress(MEMORY_BASE.0 + 0x280), 16)
        .unwrap();
    let unsupported = faulted(fixture.dispatch_com(&interface, 3, STACK_COM));
    assert_eq!(unsupported.call_site, GuestAddress(0x1005_b8d6));
    let target = unsupported.target.expect("typed IDirect3D9 fault target");
    assert_eq!(target.interface, "IDirect3D9");
    assert_eq!(target.vtable_slot, 3);
    assert_eq!(target.method, "RegisterSoftwareDevice");
    assert_eq!(fixture.runtime.memory.stats(), before_unsupported_memory);
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(GuestAddress(MEMORY_BASE.0 + 0x280), 16)
            .unwrap(),
        output_before
    );
    assert_eq!(fixture.runtime.com().objects().len(), 1);
    assert_eq!(
        fixture.runtime.com().objects()[&object.id].reference_count,
        1
    );
    assert_eq!(
        fixture
            .runtime
            .com_interface_for_pointer(interface.guest_pointer)
            .unwrap()
            .unwrap()
            .id,
        interface.id
    );
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );

    write_words(
        &mut fixture.runtime.memory,
        STACK_COM,
        &[0x1000_1000, pointer],
    );
    acknowledge_pending(&mut fixture.runtime.memory);
    let add_ref = completed(fixture.dispatch_com(&interface, 1, STACK_COM));
    assert_eq!(add_ref.eax, 2);
    let first_release = completed(fixture.dispatch_com(&interface, 2, STACK_COM));
    assert_eq!(first_release.eax, 1);
    assert_eq!(
        fixture
            .runtime
            .com()
            .object(interface.object_id)
            .unwrap()
            .reference_count,
        1
    );
    let final_release = completed(fixture.dispatch_com(&interface, 2, STACK_COM));
    assert_eq!(final_release.eax, 0);
    assert!(fixture.runtime.com().is_empty());
    assert!(
        fixture
            .runtime
            .process_address_space_allocator()
            .allocation(object.backing.address_allocation_id)
            .is_err()
    );
    assert!(
        fixture
            .runtime
            .memory
            .read(interface.guest_pointer, 4)
            .is_err()
    );
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
    fixture.acknowledge();

    let second = fixture.dispatch_factory(STACK_VALID, D3D9_SDK_VERSION);
    assert_eq!(second.eax, Some(pointer));
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
fn direct3d_create9_resource_exhaustion_fails_before_publication() {
    let mut fixture = Fixture::new(MemoryLimits {
        max_physical_pages: 3,
        ..MemoryLimits::default()
    });
    write_words(
        &mut fixture.runtime.memory,
        STACK_VALID,
        &[RETURN_ADDRESS.0, D3D9_SDK_VERSION],
    );
    acknowledge_pending(&mut fixture.runtime.memory);
    let before_memory = fixture.runtime.memory.stats();
    let clock_before = fixture.runtime.clock().clone();
    let allocations_before = fixture.runtime.process_address_space_allocator().clone();
    let outcome = fixture.dispatch(STACK_VALID);
    match outcome {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("resource exhaustion unexpectedly completed: {other:?}"),
    };
    assert_eq!(fixture.runtime.memory.stats(), before_memory);
    assert!(fixture.runtime.com().is_empty());
    assert_eq!(
        fixture.runtime.process_address_space_allocator(),
        &allocations_before
    );
    assert_eq!(fixture.runtime.clock(), &clock_before);
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
            .expect("D3D9 IAT initialization");
        memory.seal_owner(owner).expect("seal fixture owner");
        let reflection = memory.pending_reflection().expect("initial reflection");
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .expect("acknowledge fixture memory");
        let endpoint = endpoint(owner);
        let dispatcher =
            ApiDispatcher::from_endpoints([endpoint.clone()]).expect("D3D9 dispatcher");
        Self {
            runtime: controlled_runtime(memory),
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
            &mut self.runtime.memory,
            stack_pointer,
            &[RETURN_ADDRESS.0, sdk_version],
        );
        acknowledge_pending(&mut self.runtime.memory);
        match self.dispatch(stack_pointer) {
            DispatchOutcome::Immediate(completion) => completion,
            DispatchOutcome::Fault(fault) => panic!("unexpected D3D9 fault: {fault:?}"),
            other => panic!("unexpected D3D9 outcome: {other:?}"),
        }
    }

    fn dispatch(&mut self, stack_pointer: GuestAddress) -> DispatchOutcome {
        let trap = ApiOwnedTrap {
            endpoint_id: self.endpoint.id,
            endpoint_address: self.endpoint.address,
            stack_pointer,
            return_address: RETURN_ADDRESS,
        };
        self.dispatcher
            .dispatch_owned(trap, &mut self.runtime, None, |frame, runtime, _| {
                dispatch_d3d9_api(frame, runtime)
            })
    }

    fn dispatch_com(
        &mut self,
        interface: &crate::runtime::RuntimeComInterfaceRecord,
        slot: u16,
        stack_pointer: GuestAddress,
    ) -> ComDispatchOutcome {
        let call_site = GuestAddress(u32::from_le_bytes(
            self.runtime
                .memory
                .read(stack_pointer, 4)
                .expect("COM return address")
                .try_into()
                .unwrap(),
        ));
        self.runtime.dispatch_com_call(ComCallTrap {
            endpoint_address: interface.methods[&slot].endpoint,
            stack_pointer,
            return_address: call_site,
            call_site,
        })
    }

    fn acknowledge(&mut self) {
        let delta = self
            .runtime
            .memory
            .pending_delta()
            .expect("pending D3D9 delta");
        self.runtime
            .memory
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

fn faulted(outcome: ComDispatchOutcome) -> crate::com::ComFault {
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
        80,
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
                bytes_with_nul: b"C:\\Vine\\d3d9-root.exe\0".to_vec(),
            },
            initial_current_directory_a: RuntimeCurrentDirectoryA {
                bytes_with_nul: b"C:\\Vine\0".to_vec(),
            },
            encoded_pointer_cookie: 0xec01_9d3b,
        },
        memory,
    )
    .expect("controlled D3D9 runtime");
    runtime
        .start_initial_thread()
        .expect("start D3D9 fixture thread");
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
        .expect("configure D3D9 fixture display");
    runtime
}

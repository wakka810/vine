use crate as vine_core;
use crate::dsound::{
    DSOUND_CLASS_E_NOAGGREGATION, DSOUND_DIRECT_SOUND_CREATE_8_ORDINAL,
    DSOUND_DSBCAPS_PRIMARYBUFFER, DSOUND_DSBUFFERDESC_BYTES, DSOUND_DSSCL_NORMAL,
    DSOUND_DSSCL_PRIORITY, DSOUND_E_INVALIDARG, DSOUND_IDIRECTSOUND8_IID,
};
use crate::dsound::{dispatch_dsound_api, dsound_argument_words};
use vine_core::{
    ApiDispatcher, ApiEndpointIdentity, ApiOwnedTrap, ComCallTrap, ComDispatchOutcome,
    DispatchOutcome, GuestAddress, GuestMemory, ImportTarget, ImportThunk, ImportThunkDisposition,
    ImportThunkId, MemoryLimits, MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind, PageProtection,
    Runtime, RuntimeBootstrap, RuntimeClockState, RuntimeComClass, RuntimeCurrentDirectoryA,
    RuntimeProcessId, RuntimeProcessImagePathA, RuntimeStartupInfo, RuntimeThreadId,
    X86_STARTUP_INFO_W_BYTE_LENGTH,
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
    assert_eq!(completion.eax, Some(0));
    assert_eq!(completion.stack_pointer, GuestAddress(STACK.0 + 16));
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
    fixture.acknowledge();

    let pointer = GuestAddress(read_u32(&fixture.runtime.memory, OUTPUT_ADDRESS));
    assert_ne!(pointer.0, 0);
    let object = fixture
        .runtime
        .com()
        .objects()
        .values()
        .next()
        .expect("DirectSound root object")
        .clone();
    assert_eq!(object.class, RuntimeComClass::DirectSound8);
    assert_eq!(object.reference_count, 1);
    let interface = fixture
        .runtime
        .com_interface_for_pointer(pointer)
        .unwrap()
        .expect("IDirectSound8 pointer")
        .clone();
    assert_eq!(interface.iid, DSOUND_IDIRECTSOUND8_IID);
    assert_eq!(interface.name, "IDirectSound8");
    assert_eq!(interface.methods.len(), 12);
    assert_eq!(interface.methods[&3].name, "CreateSoundBuffer");

    assert_eq!(interface.methods[&6].name, "SetCooperativeLevel");

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
        &mut fixture.runtime.memory,
        COM_STACK,
        &[RETURN_ADDRESS.0, pointer.0],
    );
    acknowledge_pending(&mut fixture.runtime.memory);
    let backing = object.backing.range;
    let release = fixture.dispatch_com(&interface, 2);
    let ComDispatchOutcome::Completed(release) = release else {
        panic!("Release did not complete: {release:?}");
    };
    assert_eq!(release.eax, 0);
    assert!(fixture.runtime.com().is_empty());
    assert!(fixture.runtime.audio_devices().is_empty());
    assert!(fixture.runtime.memory.read(backing.base, 4).is_err());
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
}

#[test]
fn buffer_argument_failures_preserve_output_and_authorities() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    immediate(fixture.dispatch_factory(0, OUTPUT_ADDRESS.0, 0));
    fixture.acknowledge();
    let root_pointer = GuestAddress(read_u32(&fixture.runtime.memory, OUTPUT_ADDRESS));
    let root_interface = fixture
        .runtime
        .com_interface_for_pointer(root_pointer)
        .unwrap()
        .expect("IDirectSound8 pointer")
        .clone();

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
        write_words(&mut fixture.runtime.memory, BUFFER_DESCRIPTOR, &descriptor);
        write_words(
            &mut fixture.runtime.memory,
            OUTPUT_ADDRESS,
            &[OUTPUT_SENTINEL],
        );
        write_words(
            &mut fixture.runtime.memory,
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
        let before_memory = fixture.runtime.memory.stats();
        let completion = match fixture.dispatch_frontend(&root_interface, 3) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("invalid descriptor did not return E_INVALIDARG: {other:?}"),
        };
        assert_eq!(completion.eax, DSOUND_E_INVALIDARG.bits());
        assert_eq!(
            read_u32(&fixture.runtime.memory, OUTPUT_ADDRESS),
            OUTPUT_SENTINEL
        );
        assert_eq!(fixture.runtime.memory.stats(), before_memory);
        assert_live_direct_sound(&fixture, &root_interface);
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
        write_words(
            &mut fixture.runtime.memory,
            OUTPUT_ADDRESS,
            &[OUTPUT_SENTINEL],
        );
        write_words(
            &mut fixture.runtime.memory,
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
        let before_memory = fixture.runtime.memory.stats();
        let completion = match fixture.dispatch_frontend(&root_interface, 3) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("invalid CreateSoundBuffer arguments did not complete: {other:?}"),
        };
        assert_eq!(completion.eax, expected);
        assert_eq!(
            read_u32(&fixture.runtime.memory, OUTPUT_ADDRESS),
            OUTPUT_SENTINEL
        );
        assert_eq!(fixture.runtime.memory.stats(), before_memory);
        assert_live_direct_sound(&fixture, &root_interface);
    }
}

#[test]
fn factory_failures_preserve_output_and_all_authorities() {
    for (device_guid, output, outer, expected) in [
        (0, 0, 0, Some(DSOUND_E_INVALIDARG.bits())),
        (
            0,
            OUTPUT_ADDRESS.0,
            0x2000_0100,
            Some(DSOUND_CLASS_E_NOAGGREGATION.bits()),
        ),
        (
            0,
            OUTPUT_ADDRESS.0,
            1,
            Some(DSOUND_CLASS_E_NOAGGREGATION.bits()),
        ),
        (
            0,
            OUTPUT_ADDRESS.0,
            u32::MAX,
            Some(DSOUND_CLASS_E_NOAGGREGATION.bits()),
        ),
        (DEVICE_GUID.0, OUTPUT_ADDRESS.0, 0, None),
    ] {
        let mut fixture = Fixture::new(MemoryLimits::default());
        let clock_before = fixture.runtime.clock().clone();
        let before_memory = fixture.runtime.memory.stats();
        let outcome = fixture.dispatch_factory(device_guid, output, outer);
        match expected {
            Some(expected) => {
                let completion = immediate(outcome);
                assert_eq!(completion.eax, Some(expected));
            }
            None => {
                let DispatchOutcome::Fault(_) = outcome else {
                    panic!("non-default device selection did not fault: {outcome:?}");
                };
            }
        }
        fixture.acknowledge();
        assert_eq!(
            read_u32(&fixture.runtime.memory, OUTPUT_ADDRESS),
            OUTPUT_SENTINEL
        );
        assert_eq!(fixture.runtime.clock(), &clock_before);
        assert_eq!(
            fixture.runtime.current_last_error().unwrap(),
            LAST_ERROR_SENTINEL
        );
        assert_eq!(
            fixture.runtime.memory.stats().committed_pages,
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
    let before_memory = fixture.runtime.memory.stats();
    let allocations_before = fixture.runtime.process_address_space_allocator().clone();
    let clock_before = fixture.runtime.clock().clone();
    let outcome = fixture.dispatch_prepared();
    let DispatchOutcome::Fault(_) = outcome else {
        panic!("exhausted DirectSound factory did not fault: {outcome:?}");
    };
    assert_eq!(
        read_u32(&fixture.runtime.memory, OUTPUT_ADDRESS),
        OUTPUT_SENTINEL
    );
    assert_eq!(fixture.runtime.memory.stats(), before_memory);
    assert_eq!(
        fixture.runtime.process_address_space_allocator(),
        &allocations_before
    );
    assert_eq!(fixture.runtime.clock(), &clock_before);
    assert_eq!(
        fixture.runtime.current_last_error().unwrap(),
        LAST_ERROR_SENTINEL
    );
    assert!(fixture.runtime.com().is_empty());
    assert!(fixture.runtime.audio_devices().is_empty());
}

#[test]
fn cooperative_level_rejects_invalid_or_unreached_relations_atomically() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    immediate(fixture.dispatch_factory(0, OUTPUT_ADDRESS.0, 0));
    fixture.acknowledge();
    let pointer = GuestAddress(read_u32(&fixture.runtime.memory, OUTPUT_ADDRESS));
    let interface = fixture
        .runtime
        .com_interface_for_pointer(pointer)
        .unwrap()
        .expect("IDirectSound8 pointer")
        .clone();

    for (window, level) in [(0, DSOUND_DSSCL_PRIORITY), (0x4801_0001, 0)] {
        fixture.prepare_cooperative_call(pointer, window, level);
        let before_memory = fixture.runtime.memory.stats();
        let completion = match fixture.dispatch_frontend(&interface, 6) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("invalid cooperative arguments did not return HRESULT: {other:?}"),
        };
        assert_eq!(completion.eax, DSOUND_E_INVALIDARG.bits());
        assert_eq!(fixture.runtime.memory.stats(), before_memory);
        assert_live_direct_sound(&fixture, &interface);
    }

    fixture.prepare_cooperative_call(pointer, 0x4801_0001, DSOUND_DSSCL_PRIORITY);
    let before_memory = fixture.runtime.memory.stats();
    let completion = match fixture.dispatch_frontend(&interface, 6) {
        ComDispatchOutcome::Completed(completion) => completion,
        other => panic!("unknown logical HWND did not return HRESULT: {other:?}"),
    };
    assert_eq!(completion.eax, DSOUND_E_INVALIDARG.bits());
    assert_eq!(fixture.runtime.memory.stats(), before_memory);
    assert_live_direct_sound(&fixture, &interface);

    fixture.prepare_cooperative_call(pointer, 0x4801_0001, DSOUND_DSSCL_NORMAL);
    let before_memory = fixture.runtime.memory.stats();
    match fixture.dispatch_frontend(&interface, 6) {
        ComDispatchOutcome::Fault(fault) => fault,
        other => panic!("unreached cooperative variant did not fault: {other:?}"),
    };
    assert_eq!(fixture.runtime.memory.stats(), before_memory);
    assert_live_direct_sound(&fixture, &interface);
}

#[test]
fn factory_catalog_matches_only_the_direct_sound_ordinal() {
    let fixture = Fixture::new(MemoryLimits::default());
    assert!(dsound_argument_words(&fixture.endpoint).is_some());

    let mut named = fixture.endpoint.clone();
    named.symbol = vine_core::ApiSymbol::Name {
        name: "DirectSoundCreate8".to_owned(),
    };
    assert!(dsound_argument_words(&named).is_none());
    let mut other_ordinal = fixture.endpoint;
    other_ordinal.symbol = vine_core::ApiSymbol::Ordinal { ordinal: 12 };
    assert!(dsound_argument_words(&other_ordinal).is_none());
}

fn assert_live_direct_sound(fixture: &Fixture, interface: &vine_core::RuntimeComInterfaceRecord) {
    let live = fixture
        .runtime
        .com_interface_for_pointer(interface.guest_pointer)
        .unwrap()
        .expect("live DirectSound interface");
    assert_eq!(live.id, interface.id);
    assert_eq!(live.object_id, interface.object_id);
    assert_eq!(live.iid, DSOUND_IDIRECTSOUND8_IID);
    assert_eq!(live.vtable, interface.vtable);
    assert_eq!(fixture.runtime.com().objects().len(), 1);
    let object = &fixture.runtime.com().objects()[&interface.object_id];
    assert_eq!(object.class, RuntimeComClass::DirectSound8);
    assert_eq!(object.reference_count, 1);
    assert_eq!(fixture.runtime.audio_devices().len(), 1);
    let (id, device) = fixture.runtime.audio_devices().iter().next().unwrap();
    assert_eq!(*id, device.id);
    assert_eq!(device.com_object_id, interface.object_id);
    assert_eq!(device.cooperative, None);
    assert!(device.buffers.is_empty());
    assert!(fixture.runtime.audio_buffers().is_empty());
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
        vine_core::SchedulerThreadState::Runnable
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
            .expect("DirectSound IAT initialization");
        memory.seal_owner(owner).expect("seal fixture owner");
        let reflection = memory.pending_reflection().expect("initial reflection");
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .expect("acknowledge initial reflection");
        let endpoint = endpoint(owner);
        let dispatcher =
            ApiDispatcher::from_endpoints([endpoint.clone()]).expect("DirectSound dispatcher");
        Self {
            runtime: controlled_runtime(memory),
            dispatcher,
            endpoint,
        }
    }

    fn prepare_factory_call(&mut self, device_guid: u32, output: u32, outer_unknown: u32) {
        self.runtime
            .memory
            .write(OUTPUT_ADDRESS, &OUTPUT_SENTINEL.to_le_bytes())
            .expect("write output sentinel");
        write_words(
            &mut self.runtime.memory,
            STACK,
            &[RETURN_ADDRESS.0, device_guid, output, outer_unknown],
        );
        acknowledge_pending(&mut self.runtime.memory);
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
        let trap = ApiOwnedTrap {
            endpoint_id: self.endpoint.id,
            endpoint_address: self.endpoint.address,
            stack_pointer: STACK,
            return_address: RETURN_ADDRESS,
        };
        self.dispatcher
            .dispatch_owned(trap, &mut self.runtime, None, |frame, runtime, _| {
                dispatch_dsound_api(frame, runtime)
            })
    }

    fn prepare_cooperative_call(&mut self, this: GuestAddress, window: u32, level: u32) {
        write_words(
            &mut self.runtime.memory,
            COM_STACK,
            &[SET_COOPERATIVE_CALL_SITE.0, this.0, window, level],
        );
        acknowledge_pending(&mut self.runtime.memory);
    }

    fn dispatch_com(
        &mut self,
        interface: &vine_core::RuntimeComInterfaceRecord,
        slot: u16,
    ) -> ComDispatchOutcome {
        self.runtime.dispatch_com_call(ComCallTrap {
            endpoint_address: interface.methods[&slot].endpoint,
            stack_pointer: COM_STACK,
            return_address: RETURN_ADDRESS,
            call_site: RETURN_ADDRESS,
        })
    }

    fn dispatch_frontend(
        &mut self,
        interface: &vine_core::RuntimeComInterfaceRecord,
        slot: u16,
    ) -> ComDispatchOutcome {
        self.runtime.dispatch_com_call(ComCallTrap {
            endpoint_address: interface.methods[&slot].endpoint,
            stack_pointer: COM_STACK,
            return_address: SET_COOPERATIVE_CALL_SITE,
            call_site: SET_COOPERATIVE_CALL_SITE,
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
                bytes_with_nul: b"C:\\Vine\\dsound-root.exe\0".to_vec(),
            },
            initial_current_directory_a: RuntimeCurrentDirectoryA {
                bytes_with_nul: b"C:\\Vine\0".to_vec(),
            },
            encoded_pointer_cookie: 0xec01_9d3b,
        },
        memory,
    )
    .expect("controlled DirectSound runtime");
    runtime
        .start_initial_thread()
        .expect("start DirectSound fixture thread");
    runtime
}

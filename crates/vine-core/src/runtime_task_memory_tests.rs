use super::*;
use crate::api::{ApiDispatcher, ApiEndpointIdentity, ApiOwnedTrap, DispatchOutcome};
use crate::imports::{ImportThunk, ImportThunkDisposition, ImportThunkId};
use crate::memory::{GuestMemory, MemoryLimits};
use crate::{ImportTarget, Rva};

const DATA: GuestAddress = GuestAddress(0x0100_0000);
const STACK: GuestAddress = GuestAddress(DATA.0 + 0x100);
const RETURN: GuestAddress = GuestAddress(0x1008_7399);
const LAST_ERROR: u32 = 0x1234_5678;

#[test]
fn com_task_memory_zero_and_reached_size_use_owned_heap_before_co_initialize() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    let objects = format!("{:?}", fixture.runtime.objects);
    let mut pointers = Vec::new();
    for size in [0, 88] {
        let pointer = fixture.alloc(size);
        assert_ne!(pointer.0, 0);
        assert_eq!(pointer.0 % 8, 0);
        let heap = fixture.runtime.heaps.task_heap_handle().unwrap();
        let allocation = fixture.runtime.heaps.allocation(heap, pointer).unwrap();
        assert_eq!(allocation.byte_length, size.max(1));
        assert_eq!(allocation.id.0, pointers.len() as u64 + 1);
        let backing = fixture.runtime.heaps.backing(heap).unwrap();
        assert_eq!(
            fixture
                .runtime
                .memory
                .committed_page(GuestAddress(pointer.0 & !0xfff))
                .unwrap(),
            Some((
                backing.memory_owner,
                PageProtection {
                    read: true,
                    write: true,
                    execute: false
                }
            ))
        );
        fixture
            .runtime
            .memory
            .write(pointer, &vec![0x5a; size.max(1) as usize])
            .unwrap();
        fixture.acknowledge();
        assert_eq!(
            fixture
                .runtime
                .memory
                .read(pointer, size.max(1) as usize)
                .unwrap(),
            vec![0x5a; size.max(1) as usize]
        );
        pointers.push(pointer);
    }
    assert_ne!(pointers[0], pointers[1]);
    assert!(fixture.runtime.current_com_apartment().unwrap().is_none());
    assert_eq!(format!("{:?}", fixture.runtime.objects), objects);
    let heap = fixture.runtime.heaps.task_heap_handle().unwrap();
    assert_eq!(
        fixture.runtime.heaps.heap(heap).unwrap().live_allocations,
        2
    );
    for pointer in pointers {
        fixture.free(pointer);
        assert!(
            fixture
                .runtime
                .heaps
                .allocation_option(heap, pointer)
                .unwrap()
                .is_none()
        );
    }
    assert_eq!(
        fixture.runtime.heaps.heap(heap).unwrap().live_allocations,
        0
    );
}

#[test]
fn com_task_memory_null_foreign_interior_and_retired_free_preserve_authority() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    fixture.assert_unchanged("CoTaskMemFree", 0, false, None);
    fixture.assert_unchanged("CoTaskMemFree", DATA.0, true, None);
    let pointer = fixture.alloc(88);
    let heap = fixture.runtime.allocate_private_heap(0, 0).unwrap();
    let foreign = fixture
        .runtime
        .allocate_private_heap_block(heap, 32, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    fixture.acknowledge();
    fixture.assert_unchanged("CoTaskMemFree", foreign.address.0, true, None);
    fixture.assert_unchanged("CoTaskMemFree", pointer.0 + 8, true, None);
    fixture.assert_unchanged("CoTaskMemFree", 0, false, None);
    fixture.free(pointer);
    fixture.assert_unchanged("CoTaskMemFree", pointer.0, true, None);
    assert_eq!(
        fixture
            .runtime
            .private_heap_allocation(heap, foreign.address)
            .unwrap(),
        foreign
    );
}

#[test]
fn com_task_memory_failed_first_allocation_does_not_publish_heap_or_bytes() {
    let mut fixture = Fixture::new(MemoryLimits::default());
    fixture.assert_unchanged("CoTaskMemAlloc", u32::MAX, false, Some(0));
    assert!(fixture.runtime.heaps.task_heap_handle().is_none());
    for limits in [
        MemoryLimits {
            max_physical_pages: 3,
            ..MemoryLimits::default()
        },
        MemoryLimits {
            max_regions: 1,
            ..MemoryLimits::default()
        },
        MemoryLimits {
            max_owners: 1,
            ..MemoryLimits::default()
        },
    ] {
        let mut fixture = Fixture::new(limits);
        fixture.assert_unchanged("CoTaskMemAlloc", 88, false, Some(0));
        assert!(fixture.runtime.heaps.task_heap_handle().is_none());
        assert!(fixture.runtime.heaps.is_empty());
    }
}

struct Fixture {
    runtime: Runtime,
    dispatcher: ApiDispatcher,
    endpoints: BTreeMap<&'static str, ApiEndpointIdentity>,
}

impl Fixture {
    fn new(limits: MemoryLimits) -> Self {
        let mut memory = GuestMemory::new(limits).unwrap();
        let owner = memory.create_owner(MemoryOwnerKind::Runtime).unwrap();
        let range = memory
            .reserve_fixed(owner, MemoryRegionKind::Runtime, DATA, 0x3000)
            .unwrap();
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
            .unwrap();
        let endpoints: BTreeMap<_, _> = ["CoTaskMemAlloc", "CoTaskMemFree"]
            .into_iter()
            .enumerate()
            .map(|(index, name)| {
                let address = GuestAddress(0x7f00_12c0 + index as u32 * 16);
                let iat_address = GuestAddress(DATA.0 + 0x2000 + index as u32 * 4);
                memory
                    .write_initialization(owner, iat_address, &address.0.to_le_bytes())
                    .unwrap();
                (
                    name,
                    ApiEndpointIdentity::from_import_thunk(&ImportThunk {
                        id: ImportThunkId(700 + index as u32),
                        address,
                        requester_owner: owner,
                        module: "ole32.dll".to_owned(),
                        iat_rva: Rva(0x2000 + index as u32 * 4),
                        iat_address,
                        original_iat_value: 0,
                        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
                        target: ImportTarget::Name {
                            name: name.to_owned(),
                            hint: 0,
                            name_rva: Rva(0),
                        },
                    }),
                )
            })
            .collect();
        memory.seal_owner(owner).unwrap();
        let reflection = memory.pending_reflection().unwrap();
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .unwrap();
        let dispatcher = ApiDispatcher::from_endpoints(endpoints.values().cloned()).unwrap();
        let mut runtime = Runtime::new(
            RuntimeBootstrap {
                process_id: RuntimeProcessId(4096),
                initial_thread_id: RuntimeThreadId(8192),
                initial_last_error: LAST_ERROR,
                clock: RuntimeClockState {
                    wall_clock_file_time: 134_282_016_000_000_000,
                    tick_count: 3_600_000,
                    performance_counter: 12_345_678_901,
                    performance_frequency: 10_000_000,
                },
                startup_info: RuntimeStartupInfo { bytes: vec![0; 68] },
                process_image_path_a: RuntimeProcessImagePathA {
                    bytes_with_nul: b"C:\\Vine\\task-memory.exe\0".to_vec(),
                },
                initial_current_directory_a: RuntimeCurrentDirectoryA {
                    bytes_with_nul: b"C:\\Vine\0".to_vec(),
                },
                encoded_pointer_cookie: 0xec01_9d3b,
            },
            memory,
        )
        .unwrap();
        runtime.start_initial_thread().unwrap();
        Self {
            runtime,
            dispatcher,
            endpoints,
        }
    }

    fn prepare(&mut self, argument: u32) {
        self.runtime
            .memory
            .write(
                STACK,
                &[RETURN.0, argument]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        self.acknowledge();
    }

    fn dispatch(&mut self, name: &str) -> DispatchOutcome {
        let endpoint = &self.endpoints[name];
        self.dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: endpoint.id,
                endpoint_address: endpoint.address,
                stack_pointer: STACK,
                return_address: RETURN,
            },
            &mut self.runtime,
            None,
            |frame, runtime, _| crate::ole32::dispatch_ole32_api(frame, runtime),
        )
    }

    fn alloc(&mut self, size: u32) -> GuestAddress {
        self.prepare(size);
        let DispatchOutcome::Immediate(completion) = self.dispatch("CoTaskMemAlloc") else {
            panic!("task allocation must complete");
        };
        assert_eq!(completion.instruction_pointer, RETURN);
        assert_eq!(completion.stack_pointer, GuestAddress(STACK.0 + 8));
        assert_eq!(self.runtime.current_last_error().unwrap(), LAST_ERROR);
        self.acknowledge();
        GuestAddress(completion.eax.unwrap())
    }

    fn free(&mut self, address: GuestAddress) {
        self.prepare(address.0);
        let DispatchOutcome::Immediate(completion) = self.dispatch("CoTaskMemFree") else {
            panic!("owned task free must complete");
        };
        assert_eq!(completion.eax, None);
        assert_eq!(completion.stack_pointer, GuestAddress(STACK.0 + 8));
        assert_eq!(self.runtime.current_last_error().unwrap(), LAST_ERROR);
        self.acknowledge();
    }

    fn assert_unchanged(&mut self, name: &str, argument: u32, fault: bool, eax: Option<u32>) {
        self.prepare(argument);
        let memory = self.runtime.memory.current_reflection().unwrap();
        let stats = self.runtime.memory.stats();
        let allocator = self.runtime.process_address_space_allocator().clone();
        let heaps = format!("{:?}", self.runtime.heaps);
        let objects = format!("{:?}", self.runtime.objects);
        let scheduler = self.runtime.scheduler().clone();
        let outcome = self.dispatch(name);
        if fault {
            assert!(matches!(outcome, DispatchOutcome::Fault(_)), "{outcome:?}");
        } else {
            let DispatchOutcome::Immediate(completion) = outcome else {
                panic!("{outcome:?}");
            };
            assert_eq!(completion.eax, eax);
        }
        assert_eq!(self.runtime.memory.current_reflection().unwrap(), memory);
        assert_eq!(self.runtime.memory.stats(), stats);
        assert_eq!(self.runtime.process_address_space_allocator(), &allocator);
        assert_eq!(format!("{:?}", self.runtime.heaps), heaps);
        assert_eq!(format!("{:?}", self.runtime.objects), objects);
        assert_eq!(self.runtime.scheduler(), &scheduler);
        assert_eq!(self.runtime.current_last_error().unwrap(), LAST_ERROR);
    }

    fn acknowledge(&mut self) {
        if self.runtime.memory.stats().pending_invalidation.is_some() {
            let delta = self.runtime.memory.pending_delta().unwrap();
            self.runtime
                .memory
                .acknowledge_delta(delta.generation)
                .unwrap();
        }
    }
}

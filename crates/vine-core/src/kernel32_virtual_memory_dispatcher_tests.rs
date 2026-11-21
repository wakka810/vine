use crate as vine_core;
use crate::imports::{ImportThunk, ImportThunkDisposition, ImportThunkId};
use crate::win32::{MEM_COMMIT, MEM_DECOMMIT, MEM_RESERVE, MEM_TOP_DOWN, dispatch_kernel32_api};
use vine_core::{
    ApiDispatcher, ApiEndpointIdentity, ApiOwnedTrap, DispatchOutcome, GuestAddress, GuestMemory,
    ImportTarget, MemoryLimits, MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind, PageProtection,
    ProcessAddressAllocationPurpose, Runtime, RuntimeBootstrap, RuntimeClockState,
    RuntimeCurrentDirectoryA, RuntimeProcessId, RuntimeProcessImagePathA, RuntimeStartupInfo,
    RuntimeThreadId,
};

const DATA: GuestAddress = GuestAddress(0x2000_0000);
const STACK: GuestAddress = GuestAddress(0x2000_0100);
const IAT: GuestAddress = GuestAddress(0x2000_2000);
const ENDPOINT: GuestAddress = GuestAddress(0x7f00_01b0);
const RETURN: GuestAddress = GuestAddress(0x1000_0100);
const LAST_ERROR: u32 = 0x1234_5678;
const READ_WRITE: u32 = 4;

struct Fixture {
    runtime: Runtime,
    owner: MemoryOwnerId,
}

impl Fixture {
    fn new() -> Self {
        Self::with_limits(MemoryLimits {
            max_physical_pages: 8,
            max_regions: 4,
            max_owners: 4,
            max_access_bytes: 0x4000,
        })
    }

    fn with_limits(limits: MemoryLimits) -> Self {
        let mut memory = GuestMemory::new(limits).unwrap();
        let owner = memory.create_owner(MemoryOwnerKind::Runtime).unwrap();
        let range = memory
            .reserve_fixed(owner, MemoryRegionKind::Runtime, DATA, 0x4000)
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
        memory
            .write_initialization(owner, IAT, &ENDPOINT.0.to_le_bytes())
            .unwrap();
        memory.seal_owner(owner).unwrap();
        let reflection = memory.pending_reflection().unwrap();
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .unwrap();
        let mut startup = vec![0; 68];
        startup[..4].copy_from_slice(&68_u32.to_le_bytes());
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
                startup_info: RuntimeStartupInfo { bytes: startup },
                process_image_path_a: RuntimeProcessImagePathA {
                    bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
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
        Self { runtime, owner }
    }

    fn prepare(&mut self, arguments: &[u32]) {
        let bytes = std::iter::once(RETURN.0)
            .chain(arguments.iter().copied())
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        self.runtime.memory.write(STACK, &bytes).unwrap();
        self.acknowledge();
    }

    fn dispatch(&mut self, name: &str) -> DispatchOutcome {
        let endpoint = ApiEndpointIdentity::from_import_thunk(&ImportThunk {
            id: ImportThunkId(28),
            address: ENDPOINT,
            requester_owner: self.owner,
            module: "kernel32.dll".to_owned(),
            iat_rva: vine_core::Rva(IAT.0 - DATA.0),
            iat_address: IAT,
            original_iat_value: 0,
            disposition: ImportThunkDisposition::DeclaredRuntimeExport,
            target: ImportTarget::Name {
                name: name.to_owned(),
                hint: 0,
                name_rva: vine_core::Rva(0),
            },
        });
        let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap();
        dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: endpoint.id,
                endpoint_address: endpoint.address,
                stack_pointer: STACK,
                return_address: RETURN,
            },
            &mut self.runtime,
            None,
            dispatch_kernel32_api,
        )
    }

    fn call(&mut self, name: &str, arguments: &[u32]) -> vine_core::ApiImmediateCompletion {
        self.prepare(arguments);
        match self.dispatch(name) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected completed {name}, got {other:?}"),
        }
    }

    fn reserve(&mut self) -> u32 {
        let result = self.call(
            "VirtualAlloc",
            &[0, 7 * 4096, MEM_RESERVE | MEM_TOP_DOWN, READ_WRITE],
        );
        let Some(base) = result.eax else {
            panic!("missing base")
        };
        assert_ne!(base, 0);
        assert_eq!(self.runtime.current_last_error().unwrap(), LAST_ERROR);
        assert_eq!(result.stack_pointer, GuestAddress(STACK.0 + 20));
        self.acknowledge();
        base
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

#[test]
fn top_down_reservation_faults_on_an_unregistered_late_image_without_changing_authority() {
    let mut fixture = Fixture::new();
    // A mapping installed after Runtime initialization bypasses its image index.
    let image_base = GuestAddress(0x7ffe_0000);
    let image_owner = fixture
        .runtime
        .memory
        .create_owner(MemoryOwnerKind::Image {
            source_sha256: "0".repeat(64),
        })
        .unwrap();
    let image_range = fixture
        .runtime
        .memory
        .reserve_fixed(image_owner, MemoryRegionKind::Image, image_base, 4096)
        .unwrap();
    fixture
        .runtime
        .memory
        .commit(
            image_owner,
            image_range,
            PageProtection {
                read: true,
                write: false,
                execute: true,
            },
        )
        .unwrap();
    let original_bytes = [0x4d, 0x5a, 0x90, 0x00, 0x5a, 0xa5];
    fixture
        .runtime
        .memory
        .write_initialization(image_owner, image_base, &original_bytes)
        .unwrap();
    fixture.runtime.memory.seal_owner(image_owner).unwrap();
    fixture.prepare(&[0, 4096, MEM_RESERVE | MEM_TOP_DOWN, READ_WRITE]);
    let records = fixture.runtime.process_address_space_allocator().clone();
    let memory = fixture.runtime.memory.stats();
    let regions = fixture.runtime.memory.regions().collect::<Vec<_>>();
    let scheduler = fixture.runtime.scheduler_state().unwrap();
    let thread = fixture.runtime.current_thread_id();
    assert!(matches!(
        fixture.dispatch("VirtualAlloc"),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(fixture.runtime.process_address_space_allocator(), &records);
    assert_eq!(fixture.runtime.memory.stats(), memory);
    assert_eq!(
        fixture.runtime.memory.regions().collect::<Vec<_>>(),
        regions
    );
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(image_base, original_bytes.len())
            .unwrap(),
        original_bytes
    );
    assert_eq!(fixture.runtime.scheduler_state().unwrap(), scheduler);
    assert_eq!(fixture.runtime.current_thread_id(), thread);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), LAST_ERROR);
    assert!(!fixture.runtime.memory.has_pending_delta());
    // The anonymous owner created inside the failed transaction was not published.
    assert_eq!(
        fixture
            .runtime
            .memory
            .create_owner(MemoryOwnerKind::Anonymous)
            .unwrap(),
        MemoryOwnerId(image_owner.0 + 1)
    );
}

#[test]
fn virtual_memory_calls_fault_while_the_authoritative_caller_waits_for_host_completion() {
    for (name, arguments) in [
        (
            "VirtualAlloc",
            vec![0, 4096, MEM_RESERVE | MEM_TOP_DOWN, READ_WRITE],
        ),
        (
            "VirtualAlloc",
            vec![0x4000_0000, 4096, MEM_COMMIT, READ_WRITE],
        ),
        ("VirtualFree", vec![0x4000_0000, 4096, MEM_DECOMMIT]),
    ] {
        let mut fixture = Fixture::new();
        fixture.prepare(&arguments);
        let pending = fixture
            .runtime
            .begin_host_request("fixture.pending")
            .unwrap();
        let allocations = fixture.runtime.process_address_space_allocator().clone();
        let memory = fixture.runtime.memory.stats();
        let thread = fixture.runtime.current_thread_id();
        let scheduler = fixture.runtime.scheduler_state().unwrap();
        assert!(matches!(fixture.dispatch(name), DispatchOutcome::Fault(_)));
        assert_eq!(
            fixture.runtime.process_address_space_allocator(),
            &allocations
        );
        assert_eq!(fixture.runtime.memory.stats(), memory);
        assert_eq!(fixture.runtime.current_thread_id(), thread);
        assert_eq!(fixture.runtime.scheduler_state().unwrap(), scheduler);
        assert_eq!(fixture.runtime.current_last_error().unwrap(), LAST_ERROR);
        fixture
            .runtime
            .complete_host_request(pending.request_id)
            .unwrap();
        let retried = fixture.dispatch(name);
        assert!(matches!(retried, DispatchOutcome::Immediate(_)));
    }
}

#[test]
fn successful_virtual_memory_calls_preserve_last_error_from_prior_windows_failures() {
    let mut fixture = Fixture::new();
    let invalid = fixture.call(
        "VirtualAlloc",
        &[0, 0, MEM_RESERVE | MEM_TOP_DOWN, READ_WRITE],
    );
    assert_eq!(invalid.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 87);
    let reserved = fixture.call(
        "VirtualAlloc",
        &[0, 4096, MEM_RESERVE | MEM_TOP_DOWN, READ_WRITE],
    );
    let Some(base) = reserved.eax else {
        panic!("missing reservation base")
    };
    assert_ne!(base, 0);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 87);
    fixture.acknowledge();
    let committed_before_null = fixture.runtime.memory.stats().committed_pages;
    let null_free = fixture.call("VirtualFree", &[0, 4096, MEM_DECOMMIT]);
    assert_eq!(null_free.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 487);
    assert_eq!(
        fixture.runtime.memory.stats().committed_pages,
        committed_before_null
    );
    let outside = fixture.call("VirtualFree", &[0x4000_0000, 4096, MEM_DECOMMIT]);
    assert_eq!(outside.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 487);
    let committed = fixture.call("VirtualAlloc", &[base, 4096, MEM_COMMIT, READ_WRITE]);
    assert_eq!(committed.eax, Some(base));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 487);
    fixture.acknowledge();
    let freed = fixture.call("VirtualFree", &[base, 4096, MEM_DECOMMIT]);
    assert_eq!(freed.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 487);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 487);
}

#[test]
fn reserve_commit_decommit_and_recommit_preserve_one_owned_reservation() {
    let mut fixture = Fixture::new();
    let initial = fixture.runtime.memory.stats();
    let base = fixture.reserve();
    assert_eq!(
        fixture.runtime.memory.stats().reserved_pages,
        initial.reserved_pages + 7
    );
    assert_eq!(
        fixture.runtime.memory.stats().committed_pages,
        initial.committed_pages
    );
    assert!(fixture.runtime.memory.read(GuestAddress(base), 1).is_err());
    let allocation = fixture
        .runtime
        .process_address_space_allocator()
        .allocation_for_purpose(&ProcessAddressAllocationPurpose::VirtualMemory)
        .unwrap()
        .clone();
    assert_eq!(allocation.range.base, GuestAddress(base));
    assert_eq!(allocation.range.byte_length, 7 * 4096);
    assert!(!allocation.executable);

    let committed = fixture.call("VirtualAlloc", &[base, 4097, MEM_COMMIT, READ_WRITE]);
    assert_eq!(committed.eax, Some(base));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), LAST_ERROR);
    assert_eq!(
        fixture.runtime.memory.stats().committed_pages,
        initial.committed_pages + 2
    );
    fixture.acknowledge();
    assert_eq!(
        fixture
            .runtime
            .memory
            .read(GuestAddress(base + 4096), 1)
            .unwrap(),
        [0]
    );
    fixture
        .runtime
        .memory
        .write(GuestAddress(base), &[42])
        .unwrap();
    fixture.acknowledge();

    let freed = fixture.call("VirtualFree", &[base + 1, 4096, MEM_DECOMMIT]);
    assert_eq!(freed.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), LAST_ERROR);
    assert_eq!(freed.stack_pointer, GuestAddress(STACK.0 + 16));
    assert_eq!(
        fixture.runtime.memory.stats().committed_pages,
        initial.committed_pages
    );
    assert_eq!(
        fixture.runtime.memory.stats().reserved_pages,
        initial.reserved_pages + 7
    );
    fixture.acknowledge();
    assert!(fixture.runtime.memory.read(GuestAddress(base), 1).is_err());
    assert_eq!(
        fixture
            .runtime
            .process_address_space_allocator()
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::VirtualMemory)
            .unwrap(),
        &allocation
    );

    let recommitted = fixture.call("VirtualAlloc", &[base, 4096, MEM_COMMIT, READ_WRITE]);
    assert_eq!(recommitted.eax, Some(base));
    fixture.acknowledge();
    assert_eq!(
        fixture.runtime.memory.read(GuestAddress(base), 1).unwrap(),
        [0]
    );
}

#[test]
fn commit_outside_reservation_fails_without_mapping_or_allocator_changes() {
    let mut fixture = Fixture::new();
    let base = fixture.reserve();
    fixture.prepare(&[base + 7 * 4096, 4096, MEM_COMMIT, READ_WRITE]);
    let records = fixture.runtime.process_address_space_allocator().clone();
    let stats = fixture.runtime.memory.stats();
    let regions = fixture.runtime.memory.regions().collect::<Vec<_>>();
    let DispatchOutcome::Immediate(result) = fixture.dispatch("VirtualAlloc") else {
        panic!("missing failure")
    };
    assert_eq!(result.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 487);
    assert_eq!(fixture.runtime.process_address_space_allocator(), &records);
    assert_eq!(fixture.runtime.memory.stats(), stats);
    assert_eq!(
        fixture.runtime.memory.regions().collect::<Vec<_>>(),
        regions
    );
}

#[test]
fn invalid_reservation_inputs_return_windows_errors_without_publication() {
    for (arguments, error) in [
        ([0, 0, MEM_RESERVE | MEM_TOP_DOWN, READ_WRITE], 87),
        ([DATA.0, 4096, MEM_RESERVE | MEM_TOP_DOWN, READ_WRITE], 487),
        ([0, u32::MAX, MEM_RESERVE | MEM_TOP_DOWN, READ_WRITE], 87),
    ] {
        let mut fixture = Fixture::new();
        fixture.prepare(&arguments);
        let records = fixture.runtime.process_address_space_allocator().clone();
        let stats = fixture.runtime.memory.stats();
        let DispatchOutcome::Immediate(result) = fixture.dispatch("VirtualAlloc") else {
            panic!("missing failure")
        };
        assert_eq!(result.eax, Some(0));
        assert_eq!(fixture.runtime.current_last_error().unwrap(), error);
        assert_eq!(fixture.runtime.process_address_space_allocator(), &records);
        assert_eq!(fixture.runtime.memory.stats(), stats);
    }
}

#[test]
fn unsupported_protection_and_allocation_flags_do_not_mutate_authority() {
    for arguments in [
        [0, 4096, MEM_RESERVE | 0x80000, READ_WRITE],
        [0, 4096, MEM_RESERVE | MEM_TOP_DOWN, 0x100],
    ] {
        let mut fixture = Fixture::new();
        fixture.prepare(&arguments);
        let allocations_before = fixture.runtime.process_address_space_allocator().clone();
        let clock_before = fixture.runtime.clock().clone();
        let thread_before = fixture.runtime.current_thread_id();
        let scheduler_before = fixture.runtime.scheduler_state().unwrap();
        let last_error_before = fixture.runtime.current_last_error().unwrap();
        let stats = fixture.runtime.memory.stats();
        assert!(matches!(
            fixture.dispatch("VirtualAlloc"),
            DispatchOutcome::Fault(_)
        ));
        assert_eq!(
            fixture.runtime.process_address_space_allocator(),
            &allocations_before
        );
        assert_eq!(fixture.runtime.clock(), &clock_before);
        assert_eq!(fixture.runtime.current_thread_id(), thread_before);
        assert_eq!(fixture.runtime.scheduler_state().unwrap(), scheduler_before);
        assert_eq!(
            fixture.runtime.current_last_error().unwrap(),
            last_error_before
        );
        assert_eq!(fixture.runtime.memory.stats(), stats);
    }
}

#[test]
fn decommit_invalid_ranges_fail_without_memory_changes() {
    for (offset, size, expected_error) in [
        (1, 7 * 4096, 87),
        (0, u32::MAX, 87),
        (4096 + 1, 0, 487),
        (7 * 4096, 4096, 487),
    ] {
        let mut fixture = Fixture::new();
        let base = fixture.reserve();
        assert_eq!(
            fixture
                .call("VirtualAlloc", &[base, 4096, MEM_COMMIT, READ_WRITE])
                .eax,
            Some(base)
        );
        fixture.acknowledge();
        fixture
            .runtime
            .memory
            .write(GuestAddress(base), &[42])
            .unwrap();
        fixture.prepare(&[base + offset, size, MEM_DECOMMIT]);
        let records = fixture.runtime.process_address_space_allocator().clone();
        let stats = fixture.runtime.memory.stats();
        let DispatchOutcome::Immediate(result) = fixture.dispatch("VirtualFree") else {
            panic!("missing failure")
        };
        assert_eq!(result.eax, Some(0));
        assert_eq!(
            fixture.runtime.current_last_error().unwrap(),
            expected_error
        );
        assert_eq!(fixture.runtime.process_address_space_allocator(), &records);
        assert_eq!(fixture.runtime.memory.stats(), stats);
        assert_eq!(
            fixture.runtime.memory.read(GuestAddress(base), 1).unwrap(),
            [42]
        );
    }
}

#[test]
fn zero_size_decommit_at_the_first_page_keeps_the_reservation() {
    for offset in [0, 1] {
        let mut fixture = Fixture::new();
        let before = fixture.runtime.memory.stats();
        let base = fixture.reserve();
        for page in [base, base + 6 * 4096] {
            assert_eq!(
                fixture
                    .call("VirtualAlloc", &[page, 4096, MEM_COMMIT, READ_WRITE])
                    .eax,
                Some(page)
            );
            fixture.acknowledge();
        }
        assert_eq!(
            fixture
                .call("VirtualFree", &[base + offset, 0, MEM_DECOMMIT])
                .eax,
            Some(1)
        );
        assert_eq!(fixture.runtime.current_last_error().unwrap(), LAST_ERROR);
        fixture.acknowledge();
        let after = fixture.runtime.memory.stats();
        assert_eq!(after.reserved_pages, before.reserved_pages + 7);
        assert_eq!(after.committed_pages, before.committed_pages);
    }
}

#[test]
fn reservation_owner_and_region_capacity_failures_are_atomic() {
    for (max_regions, max_owners) in [(2, 1), (1, 4)] {
        let mut fixture = Fixture::with_limits(MemoryLimits {
            max_physical_pages: 4,
            max_regions,
            max_owners,
            max_access_bytes: 0x4000,
        });
        fixture.prepare(&[0, 4096, MEM_RESERVE | MEM_TOP_DOWN, READ_WRITE]);
        let records = fixture.runtime.process_address_space_allocator().clone();
        let stats = fixture.runtime.memory.stats();
        let regions = fixture.runtime.memory.regions().collect::<Vec<_>>();
        let DispatchOutcome::Immediate(result) = fixture.dispatch("VirtualAlloc") else {
            panic!("missing failure")
        };
        assert_eq!(result.eax, Some(0));
        assert_eq!(fixture.runtime.current_last_error().unwrap(), 8);
        assert_eq!(fixture.runtime.process_address_space_allocator(), &records);
        assert_eq!(fixture.runtime.memory.stats(), stats);
        assert_eq!(
            fixture.runtime.memory.regions().collect::<Vec<_>>(),
            regions
        );
        if max_owners > 1 {
            assert_eq!(
                fixture
                    .runtime
                    .memory
                    .create_owner(MemoryOwnerKind::Anonymous)
                    .unwrap(),
                MemoryOwnerId(fixture.owner.0 + 1),
            );
        }
    }
}

#[test]
fn commit_capacity_failure_leaves_reservation_reusable() {
    let mut fixture = Fixture::with_limits(MemoryLimits {
        max_physical_pages: 5,
        max_regions: 4,
        max_owners: 4,
        max_access_bytes: 0x4000,
    });
    let base = fixture.reserve();
    let reserved_pages = fixture.runtime.memory.stats().reserved_pages;
    let committed_pages = fixture.runtime.memory.stats().committed_pages;
    let failed = fixture.call("VirtualAlloc", &[base, 8192, MEM_COMMIT, READ_WRITE]);
    assert_eq!(failed.eax, Some(0));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 8);
    assert_eq!(
        fixture.runtime.memory.stats().reserved_pages,
        reserved_pages
    );
    assert_eq!(
        fixture.runtime.memory.stats().committed_pages,
        committed_pages
    );
    assert!(!fixture.runtime.memory.has_pending_delta());
    assert!(fixture.runtime.memory.read(GuestAddress(base), 1).is_err());

    let retry = fixture.call("VirtualAlloc", &[base, 4096, MEM_COMMIT, READ_WRITE]);
    assert_eq!(retry.eax, Some(base));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 8);
    assert_eq!(
        fixture.runtime.memory.stats().reserved_pages,
        reserved_pages
    );
    assert_eq!(
        fixture.runtime.memory.stats().committed_pages,
        committed_pages + 1
    );
    fixture.acknowledge();
    assert_eq!(
        fixture.runtime.memory.read(GuestAddress(base), 1).unwrap(),
        [0]
    );
    assert!(
        fixture
            .runtime
            .memory
            .read(GuestAddress(base + 4096), 1)
            .is_err()
    );
}

#[test]
fn guest_region_overlap_does_not_publish_a_process_allocation_or_consume_owner_identity() {
    let mut fixture = Fixture::new();
    // Ask the allocator where this request would fit, then keep an independent
    // GuestMemory region there. The authoritative mapping must reject overlap.
    let base = {
        let mut probe = Fixture::new();
        probe.reserve()
    };
    let owner = fixture
        .runtime
        .memory
        .create_owner(MemoryOwnerKind::Anonymous)
        .unwrap();
    fixture
        .runtime
        .memory
        .reserve_fixed(owner, MemoryRegionKind::Anonymous, GuestAddress(base), 4096)
        .unwrap();
    fixture.acknowledge();
    fixture.prepare(&[0, 7 * 4096, MEM_RESERVE | MEM_TOP_DOWN, READ_WRITE]);
    let records = fixture.runtime.process_address_space_allocator().clone();
    let stats = fixture.runtime.memory.stats();
    let regions = fixture.runtime.memory.regions().collect::<Vec<_>>();
    assert!(matches!(
        fixture.dispatch("VirtualAlloc"),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), LAST_ERROR);
    assert_eq!(fixture.runtime.process_address_space_allocator(), &records);
    assert_eq!(fixture.runtime.memory.stats(), stats);
    assert_eq!(
        fixture.runtime.memory.regions().collect::<Vec<_>>(),
        regions
    );
    assert_eq!(
        fixture
            .runtime
            .memory
            .create_owner(MemoryOwnerKind::Anonymous)
            .unwrap(),
        MemoryOwnerId(owner.0 + 1)
    );
}

#[test]
fn embedded_image_allocation_recommit_protection_and_release_preserve_authority() {
    let mut fixture = Fixture::new();
    let initial_pages = fixture.runtime.memory.stats().committed_pages;
    let base = 0x1001_0000;
    let old_output = GuestAddress(DATA.0 + 0x800);
    assert_eq!(
        fixture
            .call(
                "VirtualAlloc",
                &[base + 3, 0x1ffd, MEM_RESERVE | MEM_COMMIT, READ_WRITE]
            )
            .eax,
        Some(base)
    );
    fixture
        .runtime
        .memory
        .write(GuestAddress(base), &[0x90, 0xc3])
        .unwrap();
    assert_eq!(
        fixture
            .call("VirtualAlloc", &[base + 1, 0x1000, MEM_COMMIT, READ_WRITE])
            .eax,
        Some(base)
    );
    assert_eq!(
        fixture.runtime.memory.read(GuestAddress(base), 2).unwrap(),
        [0x90, 0xc3]
    );
    assert_eq!(
        fixture.runtime.memory.stats().committed_pages,
        initial_pages + 2
    );
    assert_eq!(
        fixture
            .call("VirtualProtect", &[base + 1, 0x1000, 0x20, old_output.0])
            .eax,
        Some(1)
    );
    fixture.acknowledge();
    assert_eq!(
        fixture.runtime.memory.read(old_output, 4).unwrap(),
        4_u32.to_le_bytes()
    );
    assert!(
        fixture
            .runtime
            .memory
            .write(GuestAddress(base), &[0])
            .is_err()
    );
    assert!(
        fixture
            .runtime
            .memory
            .committed_page(GuestAddress(base + 4096))
            .unwrap()
            .unwrap()
            .1
            .execute
    );
    assert_eq!(
        fixture
            .call("VirtualAlloc", &[base, 4096, MEM_COMMIT, READ_WRITE])
            .eax,
        Some(base)
    );
    assert!(
        fixture
            .runtime
            .memory
            .write(GuestAddress(base), &[0])
            .is_err()
    );
    assert_eq!(
        fixture
            .call("VirtualFree", &[base + 4096, 4096, MEM_DECOMMIT])
            .eax,
        Some(1)
    );
    fixture.acknowledge();
    assert_eq!(
        fixture
            .call("VirtualFree", &[base + 4096, 4096, MEM_DECOMMIT])
            .eax,
        Some(1)
    );
    assert!(
        fixture
            .runtime
            .process_address_space_allocator()
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::VirtualMemory)
            .unwrap()
            .executable
    );
    assert_eq!(
        fixture.call("VirtualFree", &[base + 4096, 0, 0x8000]).eax,
        Some(0)
    );
    assert_eq!(fixture.call("VirtualFree", &[base, 0, 0x8000]).eax, Some(1));
    fixture.acknowledge();
    assert_eq!(
        fixture.runtime.memory.stats().committed_pages,
        initial_pages
    );
    assert!(fixture.runtime.memory.read(GuestAddress(base), 1).is_err());
    assert_eq!(
        fixture
            .call(
                "VirtualAlloc",
                &[base, 4096, MEM_RESERVE | MEM_COMMIT, READ_WRITE]
            )
            .eax,
        Some(base)
    );
    fixture.acknowledge();
    assert_eq!(
        fixture.runtime.memory.read(GuestAddress(base), 2).unwrap(),
        [0, 0]
    );
}

#[test]
fn virtual_protection_failure_preserves_pages_and_output() {
    let mut fixture = Fixture::new();
    let base = fixture.reserve();
    let output = GuestAddress(DATA.0 + 0x800);
    fixture.call("VirtualAlloc", &[base, 4096, MEM_COMMIT, READ_WRITE]);
    fixture
        .runtime
        .memory
        .write(output, &0x12345678_u32.to_le_bytes())
        .unwrap();
    assert_eq!(
        fixture
            .call("VirtualProtect", &[base, 8192, 0x20, output.0])
            .eax,
        Some(0)
    );
    assert_eq!(
        fixture.runtime.memory.read(output, 4).unwrap(),
        0x12345678_u32.to_le_bytes()
    );
    assert!(
        fixture
            .runtime
            .memory
            .write(GuestAddress(base), &[42])
            .is_ok()
    );
    fixture.prepare(&[base, 4096, 0x20, base + 4096]);
    let before = fixture.runtime.memory.stats();
    assert!(matches!(
        fixture.dispatch("VirtualProtect"),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(fixture.runtime.memory.stats(), before);
    assert!(
        fixture
            .runtime
            .memory
            .write(GuestAddress(base), &[43])
            .is_ok()
    );
    assert_eq!(
        fixture.call("VirtualProtect", &[base, 4096, 2, base]).eax,
        Some(1)
    );
    fixture.acknowledge();
    assert_eq!(
        fixture.runtime.memory.read(GuestAddress(base), 4).unwrap(),
        4_u32.to_le_bytes()
    );
    assert!(
        fixture
            .runtime
            .memory
            .write(GuestAddress(base), &[44])
            .is_err()
    );
}

#[test]
fn combined_commit_capacity_failure_does_not_publish_reservation() {
    let mut fixture = Fixture::with_limits(MemoryLimits {
        max_physical_pages: 4,
        max_regions: 4,
        max_owners: 4,
        max_access_bytes: 0x4000,
    });
    fixture.prepare(&[0, 4096, MEM_COMMIT | MEM_RESERVE, READ_WRITE]);
    let before = fixture.runtime.memory.stats();
    let allocations = fixture.runtime.process_address_space_allocator().clone();
    assert!(
        matches!(fixture.dispatch("VirtualAlloc"), DispatchOutcome::Immediate(result) if result.eax == Some(0))
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 8);
    assert_eq!(fixture.runtime.memory.stats(), before);
    assert_eq!(
        fixture.runtime.process_address_space_allocator(),
        &allocations
    );
}

#[test]
fn null_commit_allocates_executable_pages_and_decommit_invalidates_code() {
    let mut fixture = Fixture::new();
    let base = fixture
        .call("VirtualAlloc", &[0, 0x3200, MEM_COMMIT, 0x40])
        .eax
        .unwrap();
    assert_ne!(base, 0);
    let delta = fixture.runtime.memory.pending_delta().unwrap();
    assert!(delta.cache_effects.invalidate_generated_code);
    fixture.acknowledge();
    fixture
        .runtime
        .memory
        .write(GuestAddress(base), &[0xc3])
        .unwrap();
    fixture.acknowledge();
    assert_eq!(
        fixture.runtime.memory.fetch(GuestAddress(base), 1).unwrap(),
        [0xc3]
    );
    assert_eq!(
        fixture
            .call("VirtualFree", &[base, 0x3200, MEM_DECOMMIT])
            .eax,
        Some(1)
    );
    let delta = fixture.runtime.memory.pending_delta().unwrap();
    assert!(delta.cache_effects.invalidate_generated_code);
    fixture.acknowledge();
    assert!(fixture.runtime.memory.fetch(GuestAddress(base), 1).is_err());
    assert!(
        !fixture
            .runtime
            .process_address_space_allocator()
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::VirtualMemory)
            .unwrap()
            .executable
    );
}

#[test]
fn instruction_cache_flush_requires_a_valid_process_and_publishes_cpu_invalidation() {
    let mut fixture = Fixture::new();
    let process = fixture.call("GetCurrentProcess", &[]).eax.unwrap();
    assert_eq!(process, u32::MAX);
    fixture.prepare(&[0, DATA.0, 128]);
    let before = fixture.runtime.memory.stats();
    assert!(
        matches!(fixture.dispatch("FlushInstructionCache"), DispatchOutcome::Immediate(result) if result.eax == Some(0))
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 6);
    assert_eq!(fixture.runtime.memory.stats(), before);
    assert_eq!(
        fixture
            .call("FlushInstructionCache", &[process, DATA.0, 128])
            .eax,
        Some(1)
    );
    let delta = fixture.runtime.memory.pending_delta().unwrap();
    assert!(delta.cache_effects.invalidate_instruction_fetch);
    assert!(delta.cache_effects.invalidate_generated_code);
    assert!(delta.mapping_operations.is_empty());
    fixture.acknowledge();
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 6);
    assert_eq!(
        fixture.call("FlushInstructionCache", &[process, 0, 0]).eax,
        Some(1)
    );
}

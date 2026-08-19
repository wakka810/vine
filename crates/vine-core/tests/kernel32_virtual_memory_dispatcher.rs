use vine_core::{
    ApiBehavior, ApiDirection, ApiDispatcher, ApiEndpointIdentity, ApiFaultClass, ApiOwnedTrap,
    ApiReturnSchema, ApiValueSchema, CallbackPolicy, CallingConvention, DispatchOutcome,
    ExecutionSession, GuestAddress, GuestMemory, ImportTarget, ImportThunk, ImportThunkDisposition,
    ImportThunkId, Kernel32CompatibilityPolicy, LastErrorPolicy, MEM_RESERVE, MEM_TOP_DOWN,
    MappingOperation, MemoryLimits, MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind,
    PAGE_READWRITE, PageProtection, ProcessAddressAllocationClassification,
    ProcessAddressAllocationLifetime, ProcessAddressAllocationPurpose,
    ProcessAddressAllocationReleasePolicy, ProcessAddressSpaceLayout,
    RUNTIME_VIRTUAL_MEMORY_ADDRESS_OWNER, ReentrancyPolicy, RegisterUpdate, RuntimeBootstrap,
    RuntimeClockState, RuntimeCurrentDirectoryA, RuntimeProcessId, RuntimeProcessImagePathA,
    RuntimeStartupInfo, RuntimeState, RuntimeThreadId, StackCleanup, VIRTUAL_ALLOC_IMPLEMENTATION,
    dispatch_kernel32_api_with_policy, kernel32_contract_for_endpoint,
};

const MEMORY_BASE: GuestAddress = GuestAddress(0x2000_0000);
const MEMORY_BYTES: u64 = 0x4000;
const STACK: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x100);
const IAT: GuestAddress = GuestAddress(MEMORY_BASE.0 + 0x2000);
const ENDPOINT: GuestAddress = GuestAddress(0x7f00_01b0);
const RETURN_ADDRESS: GuestAddress = GuestAddress(0x1008_fba8);
const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;
const LAST_ERROR_SENTINEL: u32 = 0x1234_5678;
const RESERVE_BYTES: u32 = 96 * 1024 * 1024;
const EXPECTED_BASE: GuestAddress = GuestAddress(0x7900_0000);

struct Fixture {
    memory: GuestMemory,
    runtime: RuntimeState,
    dispatcher: ApiDispatcher,
    endpoint: ApiEndpointIdentity,
    policy: Kernel32CompatibilityPolicy,
}

impl Fixture {
    fn new() -> Self {
        Self::with_limits(MemoryLimits {
            max_physical_pages: 4,
            max_regions: 4,
            max_owners: 4,
            max_access_bytes: MEMORY_BYTES as usize,
        })
    }

    fn with_limits(limits: MemoryLimits) -> Self {
        let mut memory = GuestMemory::new(limits).unwrap();
        let owner = memory.create_owner(MemoryOwnerKind::Runtime).unwrap();
        let range = memory
            .reserve_fixed(owner, MemoryRegionKind::Runtime, MEMORY_BASE, MEMORY_BYTES)
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
        let endpoint = endpoint(owner);
        memory
            .write_initialization(owner, IAT, &ENDPOINT.0.to_le_bytes())
            .unwrap();
        memory.seal_owner(owner).unwrap();
        let reflection = memory.pending_reflection().unwrap();
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .unwrap();
        let contract = kernel32_contract_for_endpoint(endpoint.clone())
            .unwrap()
            .unwrap();
        let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).unwrap();
        let policy = serde_json::from_str(include_str!(
            "../../../fixtures/runtime-policies/kernel32-virtual-alloc-reserve-v1.json"
        ))
        .unwrap();
        Self {
            memory,
            runtime: controlled_runtime(),
            dispatcher,
            endpoint,
            policy,
        }
    }

    fn prepare(&mut self, arguments: [u32; 4]) {
        write_words(
            &mut self.memory,
            STACK,
            &[
                RETURN_ADDRESS.0,
                arguments[0],
                arguments[1],
                arguments[2],
                arguments[3],
            ],
        );
        acknowledge_pending(&mut self.memory);
    }

    fn dispatch_prepared(&mut self) -> DispatchOutcome {
        let policy = self.policy.clone();
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        self.dispatcher.dispatch_owned(
            ApiOwnedTrap {
                endpoint_id: self.endpoint.id,
                endpoint_address: self.endpoint.address,
                stack_pointer: STACK,
                return_address: RETURN_ADDRESS,
            },
            &mut session,
            |contract, frame, session| {
                dispatch_kernel32_api_with_policy(&policy, contract, frame, session)
            },
        )
    }
}

#[test]
fn catalog_exposes_bounded_x86_virtual_alloc_contract() {
    let fixture = Fixture::new();
    let contract = kernel32_contract_for_endpoint(fixture.endpoint)
        .unwrap()
        .unwrap();
    assert_eq!(contract.implementation_id, VIRTUAL_ALLOC_IMPLEMENTATION);
    assert_eq!(contract.abi.calling_convention, CallingConvention::Stdcall);
    assert_eq!(contract.abi.arguments.len(), 4);
    assert_eq!(
        contract.abi.return_value,
        ApiReturnSchema::OpaquePointer { nullable: true }
    );
    assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::Preserve);
    assert_eq!(contract.reentrancy_policy, ReentrancyPolicy::Disallowed);
    assert_eq!(contract.behavior, ApiBehavior::Immediate);
    assert_eq!(contract.callback_policy, CallbackPolicy::Forbidden);
    assert_eq!(contract.abi.arguments[0].direction, ApiDirection::In);
    assert_eq!(
        contract.abi.arguments[0].value,
        ApiValueSchema::OpaquePointer {
            nullable: true,
            alignment: 1,
        }
    );
    assert!(
        contract.abi.arguments[1..]
            .iter()
            .all(|argument| argument.value == ApiValueSchema::U32)
    );
}

#[test]
fn reserve_top_down_registers_one_process_allocation_and_lazy_guest_region() {
    let mut fixture = Fixture::new();
    fixture.prepare([0, RESERVE_BYTES, MEM_RESERVE | MEM_TOP_DOWN, PAGE_READWRITE]);
    let memory_before = fixture.memory.stats();
    let allocations_before = fixture.runtime.process_address_space_allocator().records();
    let completion = immediate(fixture.dispatch_prepared());

    assert_eq!(completion.eax, RegisterUpdate::Set(EXPECTED_BASE.0));
    assert_eq!(completion.instruction_pointer, RETURN_ADDRESS);
    assert_eq!(completion.stack_pointer, GuestAddress(STACK.0 + 20));
    assert_eq!(completion.last_error, LAST_ERROR_SENTINEL);
    assert!(completion.guest_writes.is_empty());

    let allocation = fixture
        .runtime
        .process_address_space_allocator()
        .allocation_for_purpose(&ProcessAddressAllocationPurpose::VirtualMemory)
        .unwrap();
    assert_eq!(allocation.owner, RUNTIME_VIRTUAL_MEMORY_ADDRESS_OWNER);
    assert_eq!(allocation.range.base, EXPECTED_BASE);
    assert_eq!(allocation.range.byte_length, u64::from(RESERVE_BYTES));
    assert_eq!(allocation.usable_byte_length, u64::from(RESERVE_BYTES));
    assert_eq!(allocation.alignment, 65_536);
    assert_eq!(allocation.guard_pages, 0);
    assert_eq!(
        allocation.classification,
        ProcessAddressAllocationClassification::VirtualMemory
    );
    assert_eq!(
        allocation.lifetime,
        ProcessAddressAllocationLifetime::Process
    );
    assert_eq!(
        allocation.release_policy,
        ProcessAddressAllocationReleasePolicy::Explicit
    );
    assert!(!allocation.executable);
    assert_eq!(
        fixture
            .runtime
            .process_address_space_allocator()
            .records()
            .len(),
        allocations_before.len() + 1
    );

    let stats = fixture.memory.stats();
    assert_eq!(stats.owners, memory_before.owners + 1);
    assert_eq!(stats.regions, memory_before.regions + 1);
    assert_eq!(
        stats.reserved_pages,
        memory_before.reserved_pages + RESERVE_BYTES as usize / 4096
    );
    assert_eq!(stats.committed_pages, memory_before.committed_pages);
    assert_eq!(stats.physical_pages, memory_before.physical_pages);
    assert_eq!(stats.dirty_pages, memory_before.dirty_pages);

    let region = fixture
        .memory
        .region_snapshots()
        .into_iter()
        .find(|region| region.address_allocation_id == Some(allocation.id.0))
        .unwrap();
    assert_eq!(region.kind, MemoryRegionKind::Anonymous);
    assert_eq!(region.range.start(), EXPECTED_BASE);
    assert_eq!(region.range.byte_length(), u64::from(RESERVE_BYTES));

    let delta = fixture.memory.pending_delta().unwrap();
    assert_eq!(delta.mapping_operations.len(), 1);
    assert!(delta.dirty_pages.is_empty());
    assert!(matches!(
        delta.mapping_operations[0],
        MappingOperation::Reserve {
            address_allocation_id: Some(id),
            owner,
            kind: MemoryRegionKind::Anonymous,
            range,
        } if id == allocation.id.0
            && owner == region.owner
            && range.start() == EXPECTED_BASE
            && range.byte_length() == u64::from(RESERVE_BYTES)
    ));
}

#[test]
fn unreached_variants_fail_before_runtime_or_memory_authority_mutation() {
    for arguments in [
        [
            0x1000_0000,
            RESERVE_BYTES,
            MEM_RESERVE | MEM_TOP_DOWN,
            PAGE_READWRITE,
        ],
        [0, 0, MEM_RESERVE | MEM_TOP_DOWN, PAGE_READWRITE],
        [
            0,
            RESERVE_BYTES - 0x1000,
            MEM_RESERVE | MEM_TOP_DOWN,
            PAGE_READWRITE,
        ],
        [0, RESERVE_BYTES, MEM_RESERVE, PAGE_READWRITE],
        [0, RESERVE_BYTES, MEM_RESERVE | MEM_TOP_DOWN, 0x40],
    ] {
        let mut fixture = Fixture::new();
        fixture.prepare(arguments);
        let runtime_before = fixture.runtime.clone();
        let memory_before = fixture.memory.stats();
        let regions_before = fixture.memory.region_snapshots();
        let rejected = fault(fixture.dispatch_prepared());
        assert_eq!(rejected.class, ApiFaultClass::Unsupported);
        assert_eq!(rejected.code, "virtual-alloc-variant-unsupported");
        assert_eq!(fixture.runtime, runtime_before);
        assert_eq!(fixture.memory.stats(), memory_before);
        assert_eq!(fixture.memory.region_snapshots(), regions_before);
    }
}

#[test]
fn owner_capacity_failure_is_atomic_across_allocator_and_guest_memory() {
    let mut fixture = Fixture::with_limits(MemoryLimits {
        max_physical_pages: 4,
        max_regions: 2,
        max_owners: 1,
        max_access_bytes: MEMORY_BYTES as usize,
    });
    fixture.prepare([0, RESERVE_BYTES, MEM_RESERVE | MEM_TOP_DOWN, PAGE_READWRITE]);
    let runtime_before = fixture.runtime.clone();
    let memory_before = fixture.memory.stats();
    let regions_before = fixture.memory.region_snapshots();
    let rejected = fault(fixture.dispatch_prepared());
    assert_eq!(rejected.class, ApiFaultClass::ResourceLimit);
    assert_eq!(rejected.code, "memory-owner-limit-exceeded");
    assert_eq!(fixture.runtime, runtime_before);
    assert_eq!(fixture.memory.stats(), memory_before);
    assert_eq!(fixture.memory.region_snapshots(), regions_before);
}

#[test]
fn guest_region_overlap_rejects_before_allocator_commit_or_owner_id_advance() {
    let mut fixture = Fixture::new();
    let blocking_owner = fixture
        .memory
        .create_owner(MemoryOwnerKind::Anonymous)
        .unwrap();
    assert_eq!(blocking_owner, MemoryOwnerId(2));
    fixture
        .memory
        .reserve_fixed(
            blocking_owner,
            MemoryRegionKind::Anonymous,
            EXPECTED_BASE,
            4096,
        )
        .unwrap();
    acknowledge_pending(&mut fixture.memory);
    fixture.prepare([0, RESERVE_BYTES, MEM_RESERVE | MEM_TOP_DOWN, PAGE_READWRITE]);
    let runtime_before = fixture.runtime.clone();
    let memory_before = fixture.memory.stats();
    let regions_before = fixture.memory.region_snapshots();

    let rejected = fault(fixture.dispatch_prepared());
    assert_eq!(rejected.class, ApiFaultClass::InvalidCall);
    assert_eq!(rejected.code, "memory-region-overlap");
    assert_eq!(fixture.runtime, runtime_before);
    assert_eq!(fixture.memory.stats(), memory_before);
    assert_eq!(fixture.memory.region_snapshots(), regions_before);

    fixture
        .memory
        .release_region(blocking_owner, EXPECTED_BASE)
        .unwrap();
    fixture.memory.destroy_owner(blocking_owner).unwrap();
    acknowledge_pending(&mut fixture.memory);
    let next_owner = fixture
        .memory
        .create_owner(MemoryOwnerKind::Anonymous)
        .unwrap();
    assert_eq!(next_owner, MemoryOwnerId(3));
}

fn endpoint(owner: MemoryOwnerId) -> ApiEndpointIdentity {
    ApiEndpointIdentity::from_import_thunk(&ImportThunk {
        id: ImportThunkId(28),
        address: ENDPOINT,
        requester_owner: owner,
        module: "KERNEL32.dll".to_owned(),
        iat_rva: vine_core::Rva(IAT.0 - MEMORY_BASE.0),
        iat_address: IAT,
        original_iat_value: 0,
        disposition: ImportThunkDisposition::DeclaredRuntimeExport,
        target: ImportTarget::Name {
            name: "VirtualAlloc".to_owned(),
            hint: 0,
            name_rva: vine_core::Rva(0),
        },
    })
}

fn controlled_runtime() -> RuntimeState {
    let mut startup = vec![0; 68];
    startup[..4].copy_from_slice(&68_u32.to_le_bytes());
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
            profile_id: "kernel32-virtual-alloc-test-v1".to_owned(),
            bytes: startup,
        },
        process_image_path_a: RuntimeProcessImagePathA {
            source_authority: "kernel32-virtual-alloc-test".to_owned(),
            bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: RuntimeCurrentDirectoryA {
            source_authority: "kernel32-virtual-alloc-test".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: vine_core::LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_cookie: 0xec01_9d3b,
    })
    .unwrap();
    runtime.start_initial_thread().unwrap();
    runtime
}

fn write_words(memory: &mut GuestMemory, address: GuestAddress, words: &[u32]) {
    let bytes = words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<_>>();
    memory.write(address, &bytes).unwrap();
}

fn acknowledge_pending(memory: &mut GuestMemory) {
    if memory.stats().pending_invalidation.is_some() {
        let delta = memory.pending_delta().unwrap();
        memory.acknowledge_delta(delta.generation).unwrap();
    }
}

fn immediate(outcome: DispatchOutcome) -> vine_core::ApiImmediateCompletion {
    match outcome {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected immediate completion, got {other:?}"),
    }
}

fn fault(outcome: DispatchOutcome) -> vine_core::ApiFault {
    match outcome {
        DispatchOutcome::Fault(fault) => fault,
        other => panic!("expected API fault, got {other:?}"),
    }
}

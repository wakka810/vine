use crate as vine_core;
use crate::memory::{
    CacheEffects, CpuWritebackBatch, CpuWritebackPage, GuestMemoryErrorClass, MappingOperation,
    MemoryDelta, MemoryReflection,
};
use vine_core::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestRange, MemoryLimits, MemoryOwnerId,
    MemoryOwnerKind, MemoryRegionKind, PageProtection,
};

const STACK_BASE: GuestAddress = GuestAddress(0x2000_0000);
const HEAP_BASE: GuestAddress = GuestAddress(0x3000_0000);
const SURFACE_BASE: GuestAddress = GuestAddress(0x4000_0000);
const CODE_BASE: GuestAddress = GuestAddress(0x5000_0000);

#[test]
fn reconciles_binary_multi_page_cpu_writeback_into_rust_authority() {
    let (mut memory, reflection, owners) = reflected_fixture();
    let base_generation = reflection.invalidation.generation;
    let reflection_buffers = reflection.encode_binary().expect("binary reflection");
    assert_eq!(reflection_buffers.control.len(), 36 + 7 * 16);
    assert_eq!(
        reflection_buffers.page_bytes.len(),
        7 * GUEST_PAGE_SIZE as usize
    );
    let addresses = [
        GuestAddress(SURFACE_BASE.0 + GUEST_PAGE_SIZE as u32),
        STACK_BASE,
        CODE_BASE,
        GuestAddress(HEAP_BASE.0 + GUEST_PAGE_SIZE as u32),
        SURFACE_BASE,
        GuestAddress(STACK_BASE.0 + GUEST_PAGE_SIZE as u32),
        HEAP_BASE,
    ];
    let mut pages = Vec::new();
    for (index, address) in addresses.into_iter().enumerate() {
        let reflected = reflection
            .pages
            .iter()
            .find(|page| page.address == address)
            .expect("reflected page");
        let mut bytes = reflected.bytes.clone();
        if index + 1 != addresses.len() {
            bytes[index * 17] = 0x80 + index as u8;
        }
        pages.push(CpuWritebackPage {
            address,
            owner: owner_for(address, owners),
            bytes,
        });
    }
    let batch = CpuWritebackBatch {
        base_generation,
        pages,
    };
    let buffers = batch.encode_binary().expect("binary writeback");
    assert_eq!(buffers.control.len(), 28 + 7 * 8);
    assert_eq!(buffers.page_bytes.len(), 7 * GUEST_PAGE_SIZE as usize);
    assert!(!buffers.control.windows(5).any(|window| window == b"bytes"));

    let receipt = memory
        .reconcile_cpu_writeback_binary(&buffers.control, &buffers.page_bytes)
        .expect("CPU writeback");

    assert_eq!(receipt, base_generation + 1);

    assert_eq!(memory.stats().generation, receipt);
    assert_eq!(memory.stats().acknowledged_generation, receipt);
    assert_eq!(memory.stats().dirty_pages, 0);
    assert!(memory.stats().pending_invalidation.is_none());

    for (index, address) in addresses.into_iter().enumerate() {
        let expected = if index + 1 == addresses.len() {
            0
        } else {
            0x80 + index as u8
        };
        assert_eq!(
            memory
                .read(GuestAddress(address.0 + (index * 17) as u32), 1)
                .expect("writeback read"),
            [expected]
        );
    }

    memory.write(HEAP_BASE, &[0xaa]).expect("host write");
    let host_delta = memory.pending_delta().expect("host delta after CPU slice");
    assert_eq!(host_delta.from_generation, receipt);
    assert_eq!(host_delta.generation, receipt + 1);
    let host_buffers = host_delta.encode_binary().expect("binary host delta");
    assert_eq!(host_buffers.control.len(), 52 + 16);
    assert_eq!(host_buffers.page_bytes.len(), GUEST_PAGE_SIZE as usize);
    assert_eq!(&host_buffers.page_bytes[..1], &[0xaa]);
}

#[test]
fn binary_cpu_writeback_compares_every_byte_of_unaligned_payloads() {
    let (mut memory, reflection, owners) = reflected_fixture();
    let page_size = GUEST_PAGE_SIZE as usize;
    let mut generation = reflection.invalidation.generation;
    let mut expected = vec![0; page_size];
    let mut payload = vec![0; page_size + 7];
    let mut buffers = CpuWritebackBatch {
        base_generation: generation,
        pages: vec![CpuWritebackPage {
            address: STACK_BASE,
            owner: owners.stack,
            bytes: expected.clone(),
        }],
    }
    .encode_binary()
    .unwrap();

    for (byte, offset) in (0..page_size)
        .map(|byte| (byte, byte % 8))
        .chain((0..8).map(|offset| (page_size - 1, offset)))
    {
        expected[byte] ^= 0x80;
        payload[offset..offset + page_size].copy_from_slice(&expected);
        let input = &payload[offset..offset + page_size];
        buffers.control[12..20].copy_from_slice(&generation.to_le_bytes());
        let receipt = memory
            .reconcile_cpu_writeback_binary(&buffers.control, input)
            .unwrap();
        assert_eq!(receipt, generation + 1, "byte {byte}, offset {offset}");
        generation = receipt;
        assert_eq!(memory.read(STACK_BASE, page_size).unwrap(), expected);

        buffers.control[12..20].copy_from_slice(&generation.to_le_bytes());
        assert_eq!(
            memory
                .reconcile_cpu_writeback_binary(&buffers.control, input)
                .unwrap(),
            generation,
            "identical byte {byte}, offset {offset}"
        );
    }
}

#[test]
fn reconciles_large_binary_page_batch_without_losing_atomicity() {
    const BASE: GuestAddress = GuestAddress(0x6000_0000);
    const PAGE_COUNT: usize = 32;
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: PAGE_COUNT,
        max_regions: 1,
        max_owners: 1,
        max_access_bytes: GUEST_PAGE_SIZE as usize,
    })
    .expect("memory");
    let owner = install_region(
        &mut memory,
        BASE,
        PAGE_COUNT as u64,
        MemoryRegionKind::Heap,
        false,
    );
    let reflection = memory.pending_reflection().expect("reflection");
    let generation = reflection.invalidation.generation;
    memory.acknowledge_delta(generation).expect("acknowledge");
    let batch = CpuWritebackBatch {
        base_generation: generation,
        pages: (0..PAGE_COUNT)
            .map(|index| CpuWritebackPage {
                address: GuestAddress(BASE.0 + index as u32 * GUEST_PAGE_SIZE as u32),
                owner,
                bytes: vec![index as u8 + 1; GUEST_PAGE_SIZE as usize],
            })
            .collect(),
    };
    let buffers = batch.encode_binary().unwrap();
    let receipt = memory
        .reconcile_cpu_writeback_binary(&buffers.control, &buffers.page_bytes)
        .expect("large binary-page CPU writeback");

    assert_eq!(receipt, generation + 1);
    assert_eq!(
        memory
            .read(
                GuestAddress(BASE.0 + (PAGE_COUNT - 1) as u32 * GUEST_PAGE_SIZE as u32),
                1,
            )
            .unwrap(),
        [PAGE_COUNT as u8],
    );
}

#[test]
fn rejects_invalid_cpu_writeback_without_partial_authority_change() {
    let (mut memory, reflection, owners) = reflected_fixture();
    let stats_before = memory.stats();
    let stack_before = memory.read(STACK_BASE, 1).expect("stack baseline");
    let mut first = reflection
        .pages
        .iter()
        .find(|page| page.address == STACK_BASE)
        .unwrap()
        .bytes
        .clone();
    first[0] = 0xaa;
    let mut second = reflection
        .pages
        .iter()
        .find(|page| page.address == HEAP_BASE)
        .unwrap()
        .bytes
        .clone();
    second[0] = 0xbb;
    let batch = CpuWritebackBatch {
        base_generation: reflection.invalidation.generation,
        pages: vec![
            CpuWritebackPage {
                address: STACK_BASE,
                owner: owners.stack,
                bytes: first,
            },
            CpuWritebackPage {
                address: HEAP_BASE,
                owner: owners.surface,
                bytes: second,
            },
        ],
    };
    let buffers = batch.encode_binary().unwrap();
    let error = memory
        .reconcile_cpu_writeback_binary(&buffers.control, &buffers.page_bytes)
        .expect_err("wrong owner");
    assert_eq!(error.class, GuestMemoryErrorClass::Conflict);
    assert_eq!(memory.stats(), stats_before);
    assert_eq!(memory.read(STACK_BASE, 1).unwrap(), stack_before);

    let valid = CpuWritebackBatch {
        base_generation: reflection.invalidation.generation,
        pages: vec![CpuWritebackPage {
            address: STACK_BASE,
            owner: owners.stack,
            bytes: reflection
                .pages
                .iter()
                .find(|page| page.address == STACK_BASE)
                .unwrap()
                .bytes
                .clone(),
        }],
    }
    .encode_binary()
    .expect("valid encoding");
    let truncated = memory
        .reconcile_cpu_writeback_binary(
            &valid.control,
            &valid.page_bytes[..valid.page_bytes.len() - 1],
        )
        .expect_err("truncated binary payload");
    assert_eq!(truncated.class, GuestMemoryErrorClass::InvalidRequest);
    assert_eq!(memory.stats(), stats_before);

    let mut old_header = valid.control.clone();
    old_header[..8].copy_from_slice(b"VINEWB01");
    assert!(
        memory
            .reconcile_cpu_writeback_binary(&old_header, &valid.page_bytes)
            .is_err()
    );
    assert_eq!(memory.stats(), stats_before);
    assert_eq!(memory.read(STACK_BASE, 1).unwrap(), stack_before);

    let stale = CpuWritebackBatch {
        base_generation: reflection.invalidation.generation - 1,
        pages: vec![],
    };
    let buffers = stale.encode_binary().unwrap();
    let conflict = memory
        .reconcile_cpu_writeback_binary(&buffers.control, &buffers.page_bytes)
        .expect_err("stale CPU generation");
    assert_eq!(conflict.class, GuestMemoryErrorClass::Conflict);
    assert_eq!(memory.stats(), stats_before);
}

#[test]
fn encodes_binary_mapping_operations_without_json_page_payloads() {
    let owner = MemoryOwnerId(0x1020_3040);
    let range = GuestRange::new(SURFACE_BASE, 2 * GUEST_PAGE_SIZE).expect("mapping range");
    let rw = PageProtection {
        read: true,
        write: true,
        execute: false,
    };
    let rx = PageProtection {
        read: true,
        write: false,
        execute: true,
    };
    let delta = MemoryDelta {
        from_generation: 7,
        generation: 12,
        page_size: GUEST_PAGE_SIZE,
        mapping_operations: vec![
            MappingOperation::Reserve {
                address_allocation_id: None,
                owner,
                kind: MemoryRegionKind::Surface,
                range,
            },
            MappingOperation::Commit {
                address_allocation_id: None,
                owner,
                range,
                protection: rw,
            },
            MappingOperation::Protect {
                address_allocation_id: None,
                owner,
                range,
                protection: rx,
            },
            MappingOperation::Decommit {
                address_allocation_id: None,
                owner,
                range,
            },
            MappingOperation::Release {
                address_allocation_id: None,
                owner,
                range,
            },
        ],
        dirty_pages: Vec::new(),
        cache_effects: CacheEffects {
            flush_tlb: true,
            invalidate_instruction_fetch: true,
            invalidate_generated_code: false,
        },
    };

    let buffers = delta.encode_binary().expect("binary mapping delta");
    assert!(buffers.page_bytes.is_empty());
    assert_eq!(&buffers.control[..8], b"VINEDELT");
    assert_eq!(binary_u32(&buffers.control, 8), GUEST_PAGE_SIZE as u32);
    assert_eq!(binary_u64(&buffers.control, 12), 7);
    assert_eq!(binary_u64(&buffers.control, 20), 12);
    assert_eq!(binary_u32(&buffers.control, 28), 5);
    assert_eq!(binary_u32(&buffers.control, 32), 40);
    assert_eq!(binary_u32(&buffers.control, 36), 0);
    assert_eq!(binary_u32(&buffers.control, 40), 16);
    assert_eq!(binary_u32(&buffers.control, 44), 0b011);
    assert_eq!(buffers.control.len(), 52 + 5 * 40);

    let expected = [
        (1, 10, 0),
        (2, 0, 0b011),
        (3, 0, 0b101),
        (4, 0, 0),
        (5, 0, 0),
    ];
    for (index, (operation, kind, protection)) in expected.into_iter().enumerate() {
        let offset = 52 + index * 40;
        assert_eq!(binary_u32(&buffers.control, offset), operation);
        assert_eq!(binary_u32(&buffers.control, offset + 4), owner.0);
        assert_eq!(binary_u32(&buffers.control, offset + 8), kind);
        assert_eq!(binary_u32(&buffers.control, offset + 12), protection);
        assert_eq!(binary_u32(&buffers.control, offset + 16), SURFACE_BASE.0);
        assert_eq!(
            binary_u64(&buffers.control, offset + 20),
            2 * GUEST_PAGE_SIZE
        );
        assert_eq!(binary_u64(&buffers.control, offset + 28), 0);
        assert_eq!(binary_u32(&buffers.control, offset + 36), 0);
    }
}

#[derive(Clone, Copy)]
struct Owners {
    stack: MemoryOwnerId,
    heap: MemoryOwnerId,
    surface: MemoryOwnerId,
    code: MemoryOwnerId,
}

fn reflected_fixture() -> (GuestMemory, MemoryReflection, Owners) {
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 7,
        max_regions: 4,
        max_owners: 4,
        max_access_bytes: GUEST_PAGE_SIZE as usize,
    })
    .expect("memory");
    let stack = install_region(&mut memory, STACK_BASE, 2, MemoryRegionKind::Stack, false);
    let heap = install_region(&mut memory, HEAP_BASE, 2, MemoryRegionKind::Heap, false);
    let surface = install_region(
        &mut memory,
        SURFACE_BASE,
        2,
        MemoryRegionKind::Surface,
        false,
    );
    let code = install_region(&mut memory, CODE_BASE, 1, MemoryRegionKind::Runtime, true);
    let reflection = memory.pending_reflection().expect("reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge");
    (
        memory,
        reflection,
        Owners {
            stack,
            heap,
            surface,
            code,
        },
    )
}

fn install_region(
    memory: &mut GuestMemory,
    base: GuestAddress,
    pages: u64,
    kind: MemoryRegionKind,
    execute: bool,
) -> MemoryOwnerId {
    let owner = memory
        .create_owner(MemoryOwnerKind::Anonymous)
        .expect("owner");
    let range = memory
        .reserve_fixed(owner, kind, base, pages * GUEST_PAGE_SIZE)
        .expect("reserve");
    memory
        .commit(
            owner,
            range,
            PageProtection {
                read: true,
                write: true,
                execute,
            },
        )
        .expect("commit");
    memory.seal_owner(owner).expect("seal");
    owner
}

fn owner_for(address: GuestAddress, owners: Owners) -> MemoryOwnerId {
    match address.0 {
        value if (STACK_BASE.0..STACK_BASE.0 + 0x2000).contains(&value) => owners.stack,
        value if (HEAP_BASE.0..HEAP_BASE.0 + 0x2000).contains(&value) => owners.heap,
        value if (SURFACE_BASE.0..SURFACE_BASE.0 + 0x2000).contains(&value) => owners.surface,
        value if value == CODE_BASE.0 => owners.code,
        _ => panic!("address has no fixture owner"),
    }
}

fn binary_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn binary_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

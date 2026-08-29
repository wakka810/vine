use vine_core::{
    CacheEffects, CpuWritebackBatch, CpuWritebackPage, GUEST_PAGE_SIZE, GuestAddress, GuestMemory,
    GuestMemoryErrorClass, GuestRange, MappingOperation, MemoryDelta, MemoryLimits, MemoryOwnerId,
    MemoryOwnerKind, MemoryReflection, MemoryRegionKind, PageProtection,
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
    assert_eq!(reflection_buffers.control.len(), 40 + 7 * 16);
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
    assert_eq!(buffers.control.len(), 32 + 7 * 8);
    assert_eq!(buffers.page_bytes.len(), 7 * GUEST_PAGE_SIZE as usize);
    assert!(!buffers.control.windows(5).any(|window| window == b"bytes"));

    let (mut fast_memory, fast_reflection, _) = reflected_fixture();
    assert_eq!(fast_reflection.invalidation.generation, base_generation);
    let fast_receipt = fast_memory
        .reconcile_cpu_writeback_binary_fast(&buffers.control, &buffers.page_bytes)
        .expect("fast CPU writeback");

    let (mut view_memory, view_reflection, _) = reflected_fixture();
    assert_eq!(view_reflection.invalidation.generation, base_generation);
    let page_views = batch
        .pages
        .iter()
        .map(|page| (page.address.0, page.bytes.as_slice()))
        .collect::<Vec<_>>();
    let view_receipt = view_memory
        .reconcile_cpu_writeback_pages_fast(base_generation, &page_views)
        .expect("borrowed-page CPU writeback");

    let receipt = memory
        .reconcile_cpu_writeback_binary(&buffers.control, &buffers.page_bytes)
        .expect("CPU writeback");
    assert_eq!(receipt.from_generation, base_generation);
    assert_eq!(receipt.generation, base_generation + 1);
    assert_eq!(receipt.touched_pages, 7);
    assert_eq!(receipt.changed_pages, 6);
    assert_eq!(receipt.unchanged_pages, 1);
    assert_eq!(receipt.executable_pages, 1);
    assert_eq!(receipt.changed_bytes, 6);
    assert_eq!(receipt.transferred_bytes, 7 * GUEST_PAGE_SIZE);
    assert_eq!(receipt.content_sha256.len(), 64);
    assert_eq!(fast_receipt.from_generation, receipt.from_generation);
    assert_eq!(fast_receipt.generation, receipt.generation);
    assert_eq!(fast_receipt.touched_pages, receipt.touched_pages);
    assert_eq!(fast_receipt.changed_pages, receipt.changed_pages);
    assert_eq!(fast_receipt.unchanged_pages, receipt.unchanged_pages);
    assert_eq!(fast_receipt.executable_pages, receipt.executable_pages);
    assert_eq!(fast_receipt.changed_bytes, receipt.changed_bytes);
    assert_eq!(fast_receipt.transferred_bytes, receipt.transferred_bytes);
    assert_eq!(view_receipt, fast_receipt);
    assert_eq!(fast_memory.stats(), memory.stats());
    assert_eq!(view_memory.stats(), memory.stats());
    assert_eq!(
        fast_memory.current_reflection().unwrap().content_sha256,
        memory.current_reflection().unwrap().content_sha256,
    );
    assert_eq!(memory.stats().generation, receipt.generation);
    assert_eq!(memory.stats().acknowledged_generation, receipt.generation);
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

    let host_write = memory.write(HEAP_BASE, &[0xaa]).expect("host write");
    let host_delta = memory.pending_delta().expect("host delta after CPU slice");
    assert_eq!(host_delta.from_generation, receipt.generation);
    assert_eq!(
        host_delta.generation,
        host_write.invalidation.unwrap().generation
    );
    let host_buffers = host_delta.encode_binary().expect("binary host delta");
    assert_eq!(host_buffers.control.len(), 56 + 16);
    assert_eq!(host_buffers.page_bytes.len(), GUEST_PAGE_SIZE as usize);
    assert_eq!(&host_buffers.page_bytes[..1], &[0xaa]);
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
    let error = memory
        .reconcile_cpu_writeback(&batch)
        .expect_err("wrong owner");
    assert_eq!(error.class, GuestMemoryErrorClass::Conflict);
    assert_eq!(error.code, "cpu-writeback-page-owner-mismatch");
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
    assert_eq!(truncated.code, "cpu-writeback-payload-length-mismatch");
    assert_eq!(memory.stats(), stats_before);

    let stale = CpuWritebackBatch {
        base_generation: reflection.invalidation.generation - 1,
        pages: vec![],
    };
    let conflict = memory
        .reconcile_cpu_writeback(&stale)
        .expect_err("stale CPU generation");
    assert_eq!(conflict.class, GuestMemoryErrorClass::Conflict);
    assert_eq!(conflict.code, "cpu-writeback-generation-conflict");
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
        transferred_bytes: 0,
        content_sha256: String::new(),
    };

    let buffers = delta.encode_binary().expect("binary mapping delta");
    assert!(buffers.page_bytes.is_empty());
    assert_eq!(&buffers.control[..8], b"VINEDL01");
    assert_eq!(binary_u32(&buffers.control, 8), 2);
    assert_eq!(binary_u32(&buffers.control, 12), GUEST_PAGE_SIZE as u32);
    assert_eq!(binary_u64(&buffers.control, 16), 7);
    assert_eq!(binary_u64(&buffers.control, 24), 12);
    assert_eq!(binary_u32(&buffers.control, 32), 5);
    assert_eq!(binary_u32(&buffers.control, 36), 40);
    assert_eq!(binary_u32(&buffers.control, 40), 0);
    assert_eq!(binary_u32(&buffers.control, 44), 16);
    assert_eq!(binary_u32(&buffers.control, 48), 0b011);
    assert_eq!(buffers.control.len(), 56 + 5 * 40);

    let expected = [
        (1, 10, 0),
        (2, 0, 0b011),
        (3, 0, 0b101),
        (4, 0, 0),
        (5, 0, 0),
    ];
    for (index, (operation, kind, protection)) in expected.into_iter().enumerate() {
        let offset = 56 + index * 40;
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

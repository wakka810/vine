use vine_core::{
    CacheEffects, GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestMemoryErrorClass,
    MappingOperation, MemoryLimits, MemoryOwnerKind, MemoryRegionKind, PageProtection,
};

const DATA_BASE: GuestAddress = GuestAddress(0x2000_0000);
const CODE_BASE: GuestAddress = GuestAddress(0x3000_0000);

#[test]
fn transfers_incremental_memory_deltas_and_acknowledges_exact_generations() {
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 8,
        max_regions: 8,
        max_owners: 8,
        max_access_bytes: 0x4000,
    })
    .expect("memory");
    let data_owner = memory
        .create_owner(MemoryOwnerKind::Anonymous)
        .expect("data owner");
    let data_range = memory
        .reserve_fixed(data_owner, MemoryRegionKind::Anonymous, DATA_BASE, 0x2000)
        .expect("data reservation");
    memory
        .commit(data_owner, data_range, read_write())
        .expect("data commit");
    memory.seal_owner(data_owner).expect("seal data owner");

    let reflection = memory.pending_reflection().expect("initial reflection");
    assert_eq!(reflection.pages.len(), 2);
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("initial acknowledgement");
    assert_eq!(memory.stats().dirty_pages, 0);

    let mut cpu_page = reflection.pages[0].bytes.clone();
    cpu_page[0x100..0x104].copy_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd]);
    let reconciliation = memory
        .reconcile_reflected_page(
            reflection.invalidation.generation,
            data_owner,
            DATA_BASE,
            &cpu_page,
        )
        .expect("CPU page reconciliation");
    assert_eq!(
        reconciliation.generation,
        reflection.invalidation.generation
    );
    assert_eq!(reconciliation.owner, data_owner);
    assert_eq!(reconciliation.bytes_reconciled, 0x1000);
    assert_eq!(reconciliation.changed_bytes, 4);
    assert_eq!(
        memory.stats().generation,
        reflection.invalidation.generation
    );
    assert_eq!(memory.stats().dirty_pages, 0);

    let first_write = memory
        .write(GuestAddress(DATA_BASE.0 + 0x20), &[1, 2, 3, 4, 5, 6, 7, 8])
        .expect("ordinary data write");
    let first_generation = first_write.invalidation.expect("data delta").generation;
    let first_delta = memory.pending_delta().expect("first data delta");
    assert_eq!(first_delta.generation, first_generation);
    assert_eq!(first_delta.dirty_pages.len(), 1);
    assert_eq!(first_delta.dirty_pages[0].address, DATA_BASE);
    assert_eq!(first_delta.transferred_bytes, 0x1000);
    assert_eq!(
        &first_delta.dirty_pages[0].bytes[0x100..0x104],
        &[0xaa, 0xbb, 0xcc, 0xdd]
    );
    assert_eq!(first_delta.cache_effects, CacheEffects::default());
    assert!(first_delta.mapping_operations.is_empty());
    memory
        .acknowledge_delta(first_delta.generation)
        .expect("first data acknowledgement");
    assert_eq!(memory.stats().dirty_pages, 0);
    assert_eq!(
        memory
            .read(GuestAddress(DATA_BASE.0 + 0x20), 8)
            .expect("read data"),
        [1, 2, 3, 4, 5, 6, 7, 8]
    );

    memory
        .write(GuestAddress(DATA_BASE.0 + 0x0ffe), &[9, 10, 11, 12])
        .expect("cross-page write");
    let cross_delta = memory.pending_delta().expect("cross-page delta");
    assert_eq!(cross_delta.dirty_pages.len(), 2);
    assert_eq!(cross_delta.dirty_pages[0].address, DATA_BASE);
    assert_eq!(
        cross_delta.dirty_pages[1].address,
        GuestAddress(DATA_BASE.0 + 0x1000)
    );
    memory
        .acknowledge_delta(cross_delta.generation)
        .expect("cross-page acknowledgement");

    let old_offer_generation = memory
        .write(GuestAddress(DATA_BASE.0 + 0x40), &[0xaa])
        .expect("first same-page write")
        .invalidation
        .expect("first same-page delta")
        .generation;
    let old_offer = memory
        .pending_delta()
        .expect("offered delta before mutation");
    assert_eq!(old_offer.generation, old_offer_generation);
    let newer_generation = memory
        .write(GuestAddress(DATA_BASE.0 + 0x41), &[0xbb])
        .expect("second same-page write")
        .invalidation
        .expect("second same-page delta")
        .generation;
    memory
        .acknowledge_delta(old_offer.generation)
        .expect("acknowledge transferred older generation");
    assert_eq!(memory.stats().dirty_pages, 1);
    let newer_delta = memory.pending_delta().expect("newer pending delta");
    assert_eq!(newer_delta.from_generation, old_offer.generation);
    assert_eq!(newer_delta.generation, newer_generation);
    assert_eq!(newer_delta.dirty_pages.len(), 1);
    assert_eq!(newer_delta.dirty_pages[0].bytes[0x40], 0xaa);
    assert_eq!(newer_delta.dirty_pages[0].bytes[0x41], 0xbb);
    memory
        .acknowledge_delta(newer_delta.generation)
        .expect("newer acknowledgement");

    let duplicate = memory
        .acknowledge_delta(newer_delta.generation)
        .expect_err("duplicate acknowledgement");
    assert_eq!(duplicate.class, GuestMemoryErrorClass::InvalidRequest);
    assert_eq!(duplicate.code, "memory-delta-acknowledgement-duplicate");
    let stale = memory
        .acknowledge_delta(old_offer.generation)
        .expect_err("stale acknowledgement");
    assert_eq!(stale.code, "memory-delta-acknowledgement-stale");
    let future = memory
        .acknowledge_delta(memory.stats().generation + 1)
        .expect_err("future acknowledgement");
    assert_eq!(future.code, "memory-delta-acknowledgement-future");

    let code_owner = memory
        .create_owner(MemoryOwnerKind::Runtime)
        .expect("code owner");
    let code_range = memory
        .reserve_fixed(code_owner, MemoryRegionKind::Runtime, CODE_BASE, 0x1000)
        .expect("code reservation");
    memory
        .commit(
            code_owner,
            code_range,
            PageProtection {
                read: true,
                write: true,
                execute: true,
            },
        )
        .expect("code commit");
    memory
        .write_initialization(code_owner, CODE_BASE, &[0x90])
        .expect("initialize code");
    memory.seal_owner(code_owner).expect("seal code owner");
    let code_mapping = memory.pending_delta().expect("code mapping delta");
    assert!(code_mapping.cache_effects.flush_tlb);
    memory
        .acknowledge_delta(code_mapping.generation)
        .expect("code mapping acknowledgement");

    memory.write(CODE_BASE, &[0xcc]).expect("write to code");
    let code_delta = memory.pending_delta().expect("write-to-code delta");
    assert_eq!(code_delta.dirty_pages.len(), 1);
    assert!(!code_delta.cache_effects.flush_tlb);
    assert!(code_delta.cache_effects.invalidate_instruction_fetch);
    assert!(code_delta.cache_effects.invalidate_generated_code);
    memory
        .acknowledge_delta(code_delta.generation)
        .expect("write-to-code acknowledgement");

    memory
        .protect(
            data_owner,
            data_range,
            PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .expect("protect data");
    let protect_delta = memory.pending_delta().expect("protect delta");
    assert!(matches!(
        protect_delta.mapping_operations.as_slice(),
        [MappingOperation::Protect { .. }]
    ));
    assert!(protect_delta.cache_effects.flush_tlb);
    memory
        .acknowledge_delta(protect_delta.generation)
        .expect("protect acknowledgement");

    memory
        .decommit(code_owner, code_range)
        .expect("decommit code");
    let decommit_delta = memory.pending_delta().expect("decommit delta");
    assert!(matches!(
        decommit_delta.mapping_operations.as_slice(),
        [MappingOperation::Decommit { .. }]
    ));
    assert!(decommit_delta.dirty_pages.is_empty());
    memory
        .acknowledge_delta(decommit_delta.generation)
        .expect("decommit acknowledgement");
    memory
        .release_region(code_owner, CODE_BASE)
        .expect("release code region");
    let release_delta = memory.pending_delta().expect("release delta");
    assert!(matches!(
        release_delta.mapping_operations.as_slice(),
        [MappingOperation::Release { .. }]
    ));
    assert_eq!(release_delta.cache_effects, CacheEffects::default());
    memory
        .acknowledge_delta(release_delta.generation)
        .expect("release acknowledgement");
}

#[test]
fn rolls_back_every_observable_memory_change_when_transaction_is_dropped() {
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 4,
        max_regions: 4,
        max_owners: 4,
        max_access_bytes: 0x2000,
    })
    .expect("memory");
    let owner = memory
        .create_owner(MemoryOwnerKind::Anonymous)
        .expect("owner");
    let range = memory
        .reserve_fixed(owner, MemoryRegionKind::Anonymous, DATA_BASE, 0x1000)
        .expect("reserve");
    memory.commit(owner, range, read_write()).expect("commit");
    memory
        .write_initialization(owner, DATA_BASE, &[1, 2, 3, 4])
        .expect("initialize");
    memory.seal_owner(owner).expect("seal");
    let reflection = memory.pending_reflection().expect("reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge");
    let stats_before = memory.stats();
    let bytes_before = memory.read(DATA_BASE, 4).expect("baseline bytes");

    {
        let mut transaction = memory.begin_transaction();
        transaction.write(DATA_BASE, &[9, 9, 9, 9]).expect("write");
        transaction
            .protect(
                owner,
                range,
                PageProtection {
                    read: true,
                    write: false,
                    execute: false,
                },
            )
            .expect("protect");
        let temporary_owner = transaction
            .create_owner(MemoryOwnerKind::Runtime)
            .expect("temporary owner");
        let temporary_range = transaction
            .reserve_fixed(
                temporary_owner,
                MemoryRegionKind::Runtime,
                CODE_BASE,
                0x1000,
            )
            .expect("temporary reserve");
        transaction
            .commit(temporary_owner, temporary_range, read_write())
            .expect("temporary commit");
        transaction
            .seal_owner(temporary_owner)
            .expect("temporary seal");
    }

    assert_eq!(memory.stats(), stats_before);
    assert_eq!(
        memory.read(DATA_BASE, 4).expect("restored bytes"),
        bytes_before
    );
    assert!(memory.pending_delta().is_err());
    memory.write(DATA_BASE, &[5]).expect("retry after rollback");
    let retry_delta = memory.pending_delta().expect("retry delta");
    memory
        .acknowledge_delta(retry_delta.generation)
        .expect("retry acknowledgement");
}

#[test]
fn transaction_journal_cost_scales_with_changed_pages_not_baseline_pages() {
    const BASELINE_PAGES: usize = 1_024;
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: BASELINE_PAGES + 1,
        max_regions: 2,
        max_owners: 2,
        max_access_bytes: 0x2000,
    })
    .expect("memory");
    let owner = memory
        .create_owner(MemoryOwnerKind::Anonymous)
        .expect("owner");
    let range = memory
        .reserve_fixed(
            owner,
            MemoryRegionKind::Anonymous,
            DATA_BASE,
            BASELINE_PAGES as u64 * GUEST_PAGE_SIZE,
        )
        .expect("baseline reservation");
    memory.commit(owner, range, read_write()).expect("commit");
    memory.seal_owner(owner).expect("seal");
    let reflection = memory.pending_reflection().expect("reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge");
    let stats_before = memory.stats();

    {
        let mut transaction = memory.begin_transaction();
        let start = transaction.transaction_stats();
        assert_eq!(start.baseline_committed_pages, BASELINE_PAGES);
        assert_eq!(
            start.baseline_committed_bytes,
            BASELINE_PAGES as u64 * GUEST_PAGE_SIZE
        );
        assert_eq!(start.journaled_frame_images, 0);
        assert_eq!(start.journaled_frame_bytes, 0);

        transaction
            .write(GuestAddress(DATA_BASE.0 + 0x20), &[1])
            .expect("first-page write");
        let first_page = transaction.transaction_stats();
        assert_eq!(first_page.journaled_page_states, 1);
        assert_eq!(first_page.journaled_frame_images, 1);
        assert_eq!(first_page.journaled_frame_bytes, GUEST_PAGE_SIZE);

        transaction
            .write(GuestAddress(DATA_BASE.0 + 0x40), &[2])
            .expect("same-page write");
        assert_eq!(transaction.transaction_stats(), first_page);

        transaction
            .write(GuestAddress(DATA_BASE.0 + GUEST_PAGE_SIZE as u32), &[3])
            .expect("second-page write");
        let second_page = transaction.transaction_stats();
        assert_eq!(second_page.journaled_page_states, 2);
        assert_eq!(second_page.journaled_frame_images, 2);
        assert_eq!(second_page.journaled_frame_bytes, 2 * GUEST_PAGE_SIZE);
        assert_eq!(second_page.peak_journaled_frame_bytes, 2 * GUEST_PAGE_SIZE);
    }

    assert_eq!(memory.stats(), stats_before);
    assert_eq!(
        memory.read(GuestAddress(DATA_BASE.0 + 0x20), 1).unwrap(),
        [0]
    );
    assert_eq!(
        memory
            .read(GuestAddress(DATA_BASE.0 + GUEST_PAGE_SIZE as u32), 1)
            .unwrap(),
        [0]
    );
}

#[test]
fn rolls_back_reconciliation_release_and_delta_offer_from_one_journal() {
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 2,
        max_regions: 2,
        max_owners: 2,
        max_access_bytes: 0x3000,
    })
    .expect("memory");
    let owner = memory
        .create_owner(MemoryOwnerKind::Anonymous)
        .expect("owner");
    let range = memory
        .reserve_fixed(owner, MemoryRegionKind::Anonymous, DATA_BASE, 0x2000)
        .expect("reservation");
    memory.commit(owner, range, read_write()).expect("commit");
    memory
        .write_initialization(owner, DATA_BASE, &[1, 2, 3, 4])
        .expect("initialize");
    memory.seal_owner(owner).expect("seal");
    let reflection = memory.pending_reflection().expect("reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge");
    let stats_before = memory.stats();
    let bytes_before = memory.read(DATA_BASE, 4).expect("baseline bytes");
    let mut reconciled = reflection.pages[0].bytes.clone();
    reconciled[..4].copy_from_slice(&[9, 8, 7, 6]);

    {
        let mut transaction = memory.begin_transaction();
        transaction
            .reconcile_reflected_page(
                reflection.invalidation.generation,
                owner,
                DATA_BASE,
                &reconciled,
            )
            .expect("reconcile");
        transaction
            .release_region(owner, DATA_BASE)
            .expect("release region");
        transaction.destroy_owner(owner).expect("destroy owner");
        let delta = transaction.pending_delta().expect("offered release delta");
        assert!(matches!(
            delta.mapping_operations.as_slice(),
            [MappingOperation::Release { .. }]
        ));
        let acknowledgement = transaction
            .acknowledge_delta(delta.generation)
            .expect_err("transaction acknowledgement must be rejected");
        assert_eq!(acknowledgement.class, GuestMemoryErrorClass::Conflict);
        assert_eq!(
            acknowledgement.code,
            "memory-delta-acknowledgement-before-transaction-commit"
        );

        let journal = transaction.transaction_stats();
        assert_eq!(journal.journaled_owner_states, 1);
        assert_eq!(journal.journaled_regions, 1);
        assert_eq!(journal.journaled_page_states, 2);
        assert_eq!(journal.journaled_frame_images, 3);
        assert_eq!(journal.frame_allocator_operations, 2);
        assert_eq!(journal.offered_generations, 1);
    }

    assert_eq!(memory.stats(), stats_before);
    assert_eq!(memory.read(DATA_BASE, 4).unwrap(), bytes_before);
    memory
        .write(GuestAddress(DATA_BASE.0 + 0x1000), &[0xaa])
        .expect("owner and second page restored");
}

fn read_write() -> PageProtection {
    PageProtection {
        read: true,
        write: true,
        execute: false,
    }
}

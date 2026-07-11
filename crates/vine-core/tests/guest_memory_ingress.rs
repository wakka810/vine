use std::fs;
use std::path::PathBuf;

use vine_core::{
    GuestAddress, GuestMemory, GuestMemoryError, GuestMemoryErrorClass, GuestRange, MemoryAccess,
    MemoryFaultReason, MemoryLimits, MemoryOwnerKind, MemoryRegionKind, ModuleSnapshot,
    PageProtection, Pe32Limits, build_pe32_load_plan,
};

const FIXTURE_SHA256: &str = "084bca4dbd9c60a178f751e7c6a87e0b2865fd45cdff8d6135468886316a7aa7";
const IMAGE_BASE: u32 = 0xf100_0000;

#[test]
fn owns_high_image_pages_and_gates_guest_access() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".work/toolchain-spike/high-memory.exe");
    let bytes = fs::read(&fixture).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}); run `npm run check:toolchain` first",
            fixture.display()
        )
    });
    let pe_limits = Pe32Limits::default();
    let snapshot = ModuleSnapshot::from_bytes(bytes, &pe_limits).expect("fixture snapshot");
    snapshot
        .verify_expected_sha256(FIXTURE_SHA256)
        .expect("fixed fixture fingerprint");
    let loader = build_pe32_load_plan(&snapshot, pe_limits).expect("fixture load plan");

    let limits = MemoryLimits {
        max_physical_pages: 7,
        max_regions: 4,
        max_owners: 4,
        max_access_bytes: 8_192,
    };
    let mut memory = GuestMemory::new(limits).expect("memory owner");
    let layout = memory
        .prepare_image_layout(&loader.source, &loader.load_plan)
        .expect("high image layout");
    assert_eq!(layout.owner.0, 1);
    assert_eq!(layout.image_range.start(), GuestAddress(IMAGE_BASE));
    assert_eq!(layout.image_range.byte_length(), 0x6000);
    assert_eq!(layout.reserved_pages, 6);
    assert_eq!(layout.committed_pages, 6);
    assert_eq!(layout.physical_pages, 6);
    assert_eq!(layout.executable_pages, 1);
    assert_eq!(layout.writable_pages, 1);
    assert!(!layout.owner_sealed);
    assert_eq!(layout.invalidation.generation, 2);
    assert!(layout.invalidation.data_tlb);
    assert!(layout.invalidation.instruction_fetch);
    assert!(layout.invalidation.generated_code);

    let text = GuestAddress(IMAGE_BASE + 0x1000);
    let data = GuestAddress(IMAGE_BASE + 0x3000);
    let non_executable = GuestAddress(IMAGE_BASE + 0x4000);
    let read_only = GuestAddress(IMAGE_BASE + 0x5000);

    let synchronization = memory.read(data, 1).expect_err("pending invalidation");
    assert_eq!(
        synchronization.class,
        GuestMemoryErrorClass::SynchronizationRequired
    );
    assert_eq!(synchronization.code, "memory-invalidation-unacknowledged");

    memory
        .write_initialization(layout.owner, text, &[0x90])
        .expect("initialize executable page");
    memory
        .write_initialization(layout.owner, non_executable, &[0xc3])
        .expect("initialize NX probe page");
    let unsealed_reflection = memory
        .pending_reflection()
        .expect_err("executable owner must be sealed before reflection");
    assert_eq!(unsealed_reflection.code, "executable-owner-unsealed");
    let seal = memory.seal_owner(layout.owner).expect("seal image owner");
    assert_eq!(seal.generation, 3);
    assert!(seal.data_tlb);
    assert!(seal.instruction_fetch);
    assert!(seal.generated_code);
    let initial_reflection = memory.pending_reflection().expect("initial reflection");
    assert_eq!(initial_reflection.invalidation, seal);
    memory
        .acknowledge_delta(seal.generation)
        .expect("acknowledge execute gate");
    assert_eq!(memory.fetch(text, 1).expect("fetch executable"), [0x90]);
    assert_fault(
        memory
            .fetch(non_executable, 1)
            .expect_err("NX page must not execute"),
        non_executable,
        MemoryAccess::Execute,
        MemoryFaultReason::ExecuteDenied,
        21,
    );
    assert_fault(
        memory
            .write(read_only, &[0xff])
            .expect_err("read-only page must reject writes"),
        read_only,
        MemoryAccess::Write,
        MemoryFaultReason::WriteDenied,
        7,
    );

    let data_write = memory.write(data, &[1, 2, 3, 4]).expect("write data page");
    assert_eq!(
        data_write
            .invalidation
            .expect("ordinary writes produce a delta")
            .generation,
        4
    );
    let data_delta = memory.pending_delta().expect("ordinary data delta");
    assert_eq!(data_delta.dirty_pages.len(), 1);
    assert!(!data_delta.cache_effects.flush_tlb);
    assert!(!data_delta.cache_effects.invalidate_instruction_fetch);
    assert!(!data_delta.cache_effects.invalidate_generated_code);
    memory
        .acknowledge_delta(data_delta.generation)
        .expect("acknowledge data delta");
    assert_eq!(memory.read(data, 4).expect("read data page"), [1, 2, 3, 4]);

    let cross_owner = memory
        .create_owner(MemoryOwnerKind::Anonymous)
        .expect("cross-page owner");
    let cross_region = memory
        .reserve_fixed(
            cross_owner,
            MemoryRegionKind::Anonymous,
            GuestAddress(0xe200_0000),
            0x2000,
        )
        .expect("cross-page region");
    let first_cross_page =
        GuestRange::new(GuestAddress(0xe200_0000), 0x1000).expect("first cross page");
    let cross_mapping = memory
        .commit(
            cross_owner,
            first_cross_page,
            PageProtection {
                read: true,
                write: true,
                execute: false,
            },
        )
        .expect("commit first cross page");
    assert_eq!(cross_mapping.generation, 6);
    let cross_mapping_delta = memory.pending_delta().expect("cross mapping delta");
    memory
        .acknowledge_delta(cross_mapping_delta.generation)
        .expect("acknowledge cross mapping");
    let cross_address = GuestAddress(0xe200_0ffe);
    memory
        .write(cross_address, &[0xaa, 0xbb])
        .expect("seed cross-page prefix");
    assert_fault(
        memory
            .write(cross_address, &[1, 2, 3, 4])
            .expect_err("second page is unmapped"),
        GuestAddress(0xe200_1000),
        MemoryAccess::Write,
        MemoryFaultReason::Unmapped,
        6,
    );
    let cross_seed_delta = memory.pending_delta().expect("cross seed delta");
    memory
        .acknowledge_delta(cross_seed_delta.generation)
        .expect("acknowledge cross seed");
    assert_eq!(
        memory.read(cross_address, 2).expect("no partial write"),
        [0xaa, 0xbb]
    );

    let data_page = GuestRange::new(data, 0x1000).expect("data page range");
    let protection = memory
        .protect(
            layout.owner,
            data_page,
            PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .expect("protect data page");
    assert_eq!(protection.generation, 8);
    assert_eq!(
        memory
            .read(data, 1)
            .expect_err("stale access must be blocked")
            .class,
        GuestMemoryErrorClass::SynchronizationRequired
    );
    let protection_delta = memory.pending_delta().expect("protection delta");
    memory
        .acknowledge_delta(protection_delta.generation)
        .expect("acknowledge protect");
    assert_fault(
        memory.write(data, &[9]).expect_err("data is now read-only"),
        data,
        MemoryAccess::Write,
        MemoryFaultReason::WriteDenied,
        7,
    );

    let quota_owner = memory
        .create_owner(MemoryOwnerKind::Anonymous)
        .expect("quota owner");
    let quota_base = GuestAddress(0xd000_0000);
    let quota_range = memory
        .reserve_fixed(quota_owner, MemoryRegionKind::Anonymous, quota_base, 0x1000)
        .expect("quota region");
    let quota_error = memory
        .commit(
            quota_owner,
            quota_range,
            PageProtection {
                read: true,
                write: true,
                execute: false,
            },
        )
        .expect_err("physical quota must be enforced");
    assert_eq!(quota_error.class, GuestMemoryErrorClass::ResourceLimit);
    assert_eq!(quota_error.code, "physical-page-quota-exceeded");
    assert_eq!(memory.stats().physical_pages, 7);

    let decommit = memory
        .decommit(cross_owner, first_cross_page)
        .expect("decommit cross page");
    assert_eq!(decommit.generation, 10);
    let reused = memory
        .commit(
            quota_owner,
            quota_range,
            PageProtection {
                read: true,
                write: true,
                execute: false,
            },
        )
        .expect("reuse released physical quota");
    assert_eq!(reused.generation, 11);
    let reuse_delta = memory
        .pending_delta()
        .expect("merged decommit/commit delta");
    memory
        .acknowledge_delta(reuse_delta.generation)
        .expect("acknowledge merged decommit/commit");
    let release = memory
        .release_region(quota_owner, quota_base)
        .expect("release quota region")
        .expect("committed region requires invalidation");
    assert_eq!(release.generation, 12);
    let release_delta = memory.pending_delta().expect("release delta");
    memory
        .acknowledge_delta(release_delta.generation)
        .expect("acknowledge release");
    memory
        .release_region(cross_owner, cross_region.start())
        .expect("release empty cross region")
        .expect("reservation release produces a delta");
    let empty_release_delta = memory.pending_delta().expect("empty release delta");
    memory
        .acknowledge_delta(empty_release_delta.generation)
        .expect("acknowledge empty release");
    memory
        .destroy_owner(quota_owner)
        .expect("destroy quota owner");
    memory
        .destroy_owner(cross_owner)
        .expect("destroy cross owner");

    let stats = memory.stats();
    assert_eq!(stats.owners, 1);
    assert_eq!(stats.image_owners, 1);
    assert_eq!(stats.sealed_owners, 1);
    assert_eq!(stats.regions, 1);
    assert_eq!(stats.image_regions, 1);
    assert_eq!(stats.reserved_pages, 6);
    assert_eq!(stats.committed_pages, 6);
    assert_eq!(stats.physical_pages, 6);
    assert_eq!(stats.dirty_pages, 0);
    assert_eq!(stats.generation, 13);
    assert_eq!(stats.acknowledged_generation, 13);
    assert_eq!(stats.pending_invalidation, None);
}

fn assert_fault(
    error: GuestMemoryError,
    address: GuestAddress,
    access: MemoryAccess,
    reason: MemoryFaultReason,
    page_fault_error_code: u32,
) {
    assert_eq!(error.class, GuestMemoryErrorClass::Fault);
    let fault = error.fault.expect("fault detail");
    assert_eq!(fault.address, address);
    assert_eq!(fault.access, access);
    assert_eq!(fault.reason, reason);
    assert_eq!(fault.page_fault_error_code, page_fault_error_code);
}

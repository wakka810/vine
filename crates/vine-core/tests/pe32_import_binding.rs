use std::fs;
use std::path::PathBuf;

use vine_core::{
    GuestAddress, GuestMemory, GuestMemoryErrorClass, IMPORT_THUNK_ARENA_BASE, MemoryAccess,
    MemoryFaultReason, MemoryLimits, ModuleSnapshot, Pe32Limits, RuntimeExport,
    RuntimeExportCatalog, bind_pe32_imports, build_pe32_load_plan, materialize_pe32_image,
};

const FIXTURE_SHA256: &str = "60f882a22d7d44e0ebe734f284dc8676e29035207ff004ed0cdf6e97c629b9c7";
const LOAD_BASE: u32 = 0x1000_0000;

#[test]
fn binds_owned_thunks_and_seals_the_controlled_image() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".work/toolchain-spike/abi-smoke.exe");
    let source = fs::read(&fixture).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}); run `npm run check:toolchain` first",
            fixture.display()
        )
    });
    let pe_limits = Pe32Limits::default();
    let snapshot = ModuleSnapshot::from_bytes(source, &pe_limits).expect("fixture snapshot");
    snapshot
        .verify_expected_sha256(FIXTURE_SHA256)
        .expect("fixed fixture fingerprint");
    let loader = build_pe32_load_plan(&snapshot, pe_limits).expect("fixture load plan");
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 6,
        max_regions: 2,
        max_owners: 2,
        max_access_bytes: 4_096,
    })
    .expect("guest memory");
    let image = materialize_pe32_image(&mut memory, &snapshot, &loader, GuestAddress(LOAD_BASE))
        .expect("materialized fixture");
    let catalog = RuntimeExportCatalog::new(vec![
        RuntimeExport::named("VINE_FIXTURE.DLL", "fixture_add"),
        RuntimeExport::named("vine_fixture.dll", "fixture_call_callback"),
    ])
    .expect("controlled runtime exports");

    let binding = bind_pe32_imports(&mut memory, &loader, &image, &catalog)
        .expect("controlled import binding");
    assert_eq!(catalog.len(), 2);
    assert_eq!(binding.total_imports, 2);
    assert_eq!(binding.bound_imports, 2);
    assert_eq!(binding.remaining_imports, 0);
    assert!(binding.complete);
    assert!(binding.image_owner_sealed);
    assert_eq!(binding.image_owner, image.layout.owner);
    assert_eq!(binding.image_load_base, GuestAddress(LOAD_BASE));
    assert_eq!(binding.stop.stage, "process-bootstrap");
    assert_eq!(
        binding.stop.code,
        "peb-teb-process-parameters-unimplemented"
    );
    assert_eq!(binding.first_unresolved_import, None);

    let arena = binding.arena.as_ref().expect("one runtime thunk arena");
    assert_eq!(arena.range.start(), IMPORT_THUNK_ARENA_BASE);
    assert_eq!(arena.range.byte_length(), 0x1_0000);
    assert_eq!(arena.committed_pages, 1);
    assert_eq!(arena.slot_size, 0x10);
    assert_eq!(arena.slot_capacity, 4_096);
    assert_eq!(arena.bound_slots, 2);
    assert_eq!(
        arena.content_sha256,
        "bf852b2d7a8fb6faddf0c43f182d3298746eeeb644c2fe35f8413b0b9a6de7dc"
    );
    assert!(arena.owner_sealed);

    assert_eq!(binding.thunks.len(), 2);
    let first = &binding.thunks[0];
    let second = &binding.thunks[1];
    assert_eq!(first.id.0, 1);
    assert_eq!(first.address, GuestAddress(0x7f00_0000));
    assert_eq!(first.iat_rva.0, 0x2034);
    assert_eq!(first.iat_address, GuestAddress(0x1000_2034));
    assert_eq!(second.id.0, 2);
    assert_eq!(second.address, GuestAddress(0x7f00_0010));
    assert_eq!(second.iat_rva.0, 0x2038);
    assert_eq!(second.iat_address, GuestAddress(0x1000_2038));
    assert_eq!(binding.thunk_at(first.address), Some(first));
    assert_eq!(binding.thunk_at(GuestAddress(0x7f00_0020)), None);

    let stats = memory.stats();
    assert_eq!(stats.owners, 2);
    assert_eq!(stats.sealed_owners, 2);
    assert_eq!(stats.regions, 2);
    assert_eq!(stats.reserved_pages, 21);
    assert_eq!(stats.committed_pages, 6);
    assert_eq!(stats.physical_pages, 6);
    assert_eq!(stats.dirty_pages, 6);
    assert_eq!(stats.generation, 4);
    assert_eq!(stats.pending_invalidation, Some(binding.invalidation));
    assert_eq!(binding.invalidation.generation, 4);
    assert!(binding.invalidation.data_tlb);
    assert!(binding.invalidation.instruction_fetch);
    assert!(binding.invalidation.generated_code);

    let blocked = memory
        .read(first.iat_address, 4)
        .expect_err("binding invalidation remains pending");
    assert_eq!(
        blocked.class,
        GuestMemoryErrorClass::SynchronizationRequired
    );
    memory
        .acknowledge_invalidation(binding.invalidation.generation)
        .expect("acknowledge sealed image and thunk mappings");
    assert_eq!(read_u32(&memory, first.iat_address), first.address.0);
    assert_eq!(read_u32(&memory, second.iat_address), second.address.0);
    assert_eq!(
        memory.fetch(first.address, 16).expect("first thunk bytes"),
        [
            0x0f, 0x0b, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
            0xf4, 0xf4
        ]
    );
    assert_eq!(
        memory
            .fetch(GuestAddress(0x7f00_0020), 16)
            .expect("unowned committed thunk bytes"),
        [0xf4; 16]
    );
    let write_fault = memory
        .write(first.iat_address, &[0, 0, 0, 0])
        .expect_err("sealed read-only IAT rejects guest writes");
    let fault = write_fault.fault.expect("write fault detail");
    assert_eq!(fault.address, first.iat_address);
    assert_eq!(fault.access, MemoryAccess::Write);
    assert_eq!(fault.reason, MemoryFaultReason::WriteDenied);
    assert_eq!(fault.page_fault_error_code, 7);

    let first_json = serde_json::to_vec_pretty(&binding).expect("serialize binding");
    let second_json = serde_json::to_vec_pretty(&binding).expect("serialize binding again");
    assert_eq!(
        first_json, second_json,
        "binding JSON must be deterministic"
    );
}

fn read_u32(memory: &GuestMemory, address: GuestAddress) -> u32 {
    let bytes = memory.read(address, 4).expect("bound IAT DWORD");
    u32::from_le_bytes(bytes.try_into().expect("four-byte bound IAT DWORD"))
}

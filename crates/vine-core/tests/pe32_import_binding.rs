use std::fs;
use std::path::PathBuf;

use vine_core::{
    GuestAddress, GuestMemory, GuestMemoryErrorClass, IMPORT_THUNK_ARENA_BASE,
    ImportPlanDisposition, ImportTarget, ImportThunkDisposition, MemoryAccess, MemoryFaultReason,
    MemoryLimits, MemoryOwnerKind, MemoryRegionKind, ModuleSnapshot, Pe32Limits, RuntimeExport,
    RuntimeExportCatalog, bind_pe32_imports, bind_pe32_imports_with_unsupported_traps,
    build_pe32_load_plan, commit_pe32_import_binding, materialize_pe32_image, plan_pe32_imports,
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
    assert_eq!(binding.unsupported_trap_imports, 0);
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
    assert!(binding.unresolved_imports.is_empty());

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
    assert_eq!(
        first.disposition,
        ImportThunkDisposition::DeclaredRuntimeExport
    );
    assert_eq!(second.id.0, 2);
    assert_eq!(second.address, GuestAddress(0x7f00_0010));
    assert_eq!(second.iat_rva.0, 0x2038);
    assert_eq!(second.iat_address, GuestAddress(0x1000_2038));
    assert_eq!(
        second.disposition,
        ImportThunkDisposition::DeclaredRuntimeExport
    );
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
    assert_eq!(stats.generation, 6);
    assert_eq!(stats.pending_invalidation, Some(binding.invalidation));
    assert_eq!(binding.invalidation.generation, 6);
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
    let reflection = memory.pending_reflection().expect("bound image reflection");
    assert_eq!(reflection.invalidation, binding.invalidation);
    memory
        .acknowledge_delta(binding.invalidation.generation)
        .expect("acknowledge sealed image and thunk mappings");
    assert_eq!(memory.stats().dirty_pages, 0);
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
    let json_text = String::from_utf8(first_json).expect("binding JSON UTF-8");
    assert!(!json_text.contains("disposition"));
    assert!(!json_text.contains("unsupportedTrapImports"));

    let rebinding = bind_pe32_imports(&mut memory, &loader, &image, &catalog)
        .expect_err("successful binding is not repeatable");
    assert_eq!(rebinding.code, "imports-already-bound");
}

#[test]
fn binds_missing_import_to_an_identity_preserving_trap() {
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
    let catalog = RuntimeExportCatalog::new(vec![RuntimeExport::named(
        "vine_fixture.dll",
        "fixture_add",
    )])
    .expect("one declared runtime export");

    let binding = bind_pe32_imports_with_unsupported_traps(&mut memory, &loader, &image, &catalog)
        .expect("explicit unsupported trap binding");
    assert_eq!(binding.total_imports, 2);
    assert_eq!(binding.bound_imports, 2);
    assert_eq!(binding.unsupported_trap_imports, 1);
    assert_eq!(binding.remaining_imports, 0);
    assert!(binding.complete);
    assert!(binding.image_owner_sealed);
    assert_eq!(binding.first_unresolved_import, None);
    assert!(binding.unresolved_imports.is_empty());
    assert_eq!(binding.stop.stage, "thunk-dispatch");
    assert_eq!(
        binding.stop.code,
        "unsupported-import-trap-dispatch-pending"
    );

    let declared = &binding.thunks[0];
    let unsupported = &binding.thunks[1];
    assert_eq!(declared.id.0, 1);
    assert_eq!(
        declared.disposition,
        ImportThunkDisposition::DeclaredRuntimeExport
    );
    assert_eq!(unsupported.id.0, 2);
    assert_eq!(unsupported.address, GuestAddress(0x7f00_0010));
    assert_eq!(unsupported.module, "vine_fixture.dll");
    assert_eq!(unsupported.iat_rva.0, 0x2038);
    assert_eq!(unsupported.iat_address, GuestAddress(0x1000_2038));
    assert_eq!(
        unsupported.disposition,
        ImportThunkDisposition::UnsupportedTrap
    );
    assert_eq!(binding.thunk_at(unsupported.address), Some(unsupported));

    let reflection = memory.pending_reflection().expect("trap image reflection");
    assert_eq!(reflection.invalidation, binding.invalidation);
    memory
        .acknowledge_delta(binding.invalidation.generation)
        .expect("acknowledge trap binding");
    assert_eq!(read_u32(&memory, declared.iat_address), declared.address.0);
    assert_eq!(
        read_u32(&memory, unsupported.iat_address),
        unsupported.address.0
    );
    assert_eq!(
        memory
            .fetch(unsupported.address, 16)
            .expect("unsupported trap bytes"),
        [
            0x0f, 0x0b, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
            0xf4, 0xf4
        ]
    );

    let json = serde_json::to_string_pretty(&binding).expect("serialize trap binding");
    assert!(json.contains("\"unsupportedTrapImports\": 1"));
    assert!(json.contains("\"disposition\": \"unsupported-trap\""));
}

#[test]
fn unresolved_imports_leave_memory_unchanged_and_can_be_replanned_on_the_same_image() {
    let (snapshot, loader, mut memory, image) = materialized_fixture(MemoryLimits {
        max_physical_pages: 6,
        max_regions: 2,
        max_owners: 2,
        max_access_bytes: 4_096,
    });
    let only_second = RuntimeExportCatalog::new(vec![RuntimeExport::named(
        "vine_fixture.dll",
        "fixture_call_callback",
    )])
    .expect("partial catalog");
    let stats_before = memory.stats();
    let first_iat = GuestAddress(LOAD_BASE + 0x2034);
    let second_iat = GuestAddress(LOAD_BASE + 0x2038);
    let first_original = read_initialization_u32(&memory, image.layout.owner, first_iat);
    let second_original = read_initialization_u32(&memory, image.layout.owner, second_iat);

    let plan = plan_pe32_imports(&memory, &loader, &image, &only_second)
        .expect("read-only incomplete plan");
    assert!(!plan.complete);
    assert_eq!(plan.imports.len(), 2);
    assert_eq!(plan.unresolved_imports.len(), 1);
    assert_eq!(plan.imports[0].id.0, 1);
    assert_eq!(plan.imports[0].thunk_address, GuestAddress(0x7f00_0000));
    assert_eq!(
        plan.imports[0].disposition,
        ImportPlanDisposition::Unresolved
    );
    assert_eq!(
        plan.imports[1].disposition,
        ImportPlanDisposition::DeclaredRuntimeExport
    );
    assert_eq!(memory.stats(), stats_before);

    let incomplete = bind_pe32_imports(&mut memory, &loader, &image, &only_second)
        .expect("all unresolved imports are a structured stop");
    assert!(!incomplete.complete);
    assert_eq!(incomplete.bound_imports, 0);
    assert_eq!(incomplete.remaining_imports, 2);
    assert_eq!(incomplete.unresolved_imports.len(), 1);
    assert_eq!(incomplete.stop.code, "import-symbols-unimplemented");
    assert_eq!(memory.stats(), stats_before);
    assert_eq!(
        read_initialization_u32(&memory, image.layout.owner, first_iat),
        first_original
    );
    assert_eq!(
        read_initialization_u32(&memory, image.layout.owner, second_iat),
        second_original
    );

    let no_exports = RuntimeExportCatalog::new(Vec::new()).expect("empty catalog");
    let all_unresolved = bind_pe32_imports(&mut memory, &loader, &image, &no_exports)
        .expect("multiple unresolved imports");
    assert_eq!(all_unresolved.unresolved_imports.len(), 2);
    assert_eq!(memory.stats(), stats_before);

    let only_first = RuntimeExportCatalog::new(vec![RuntimeExport::named(
        "vine_fixture.dll",
        "fixture_add",
    )])
    .expect("first-only catalog");
    let unresolved_last = bind_pe32_imports(&mut memory, &loader, &image, &only_first)
        .expect("last unresolved import");
    assert_eq!(unresolved_last.unresolved_imports.len(), 1);
    assert!(matches!(
        &unresolved_last.unresolved_imports[0].target,
        ImportTarget::Name { name, .. } if name == "fixture_call_callback"
    ));
    assert_eq!(memory.stats(), stats_before);

    let complete_catalog = RuntimeExportCatalog::new(vec![
        RuntimeExport::named("vine_fixture.dll", "fixture_add"),
        RuntimeExport::named("vine_fixture.dll", "fixture_call_callback"),
    ])
    .expect("complete catalog");
    let rebound = bind_pe32_imports(&mut memory, &loader, &image, &complete_catalog)
        .expect("retry after catalog extension");
    assert!(rebound.complete);
    assert_eq!(rebound.bound_imports, 2);
    assert_eq!(snapshot.identity(), loader.source);
}

#[test]
fn rejects_iat_changes_between_plan_and_commit_without_additional_mutation() {
    let (_snapshot, loader, mut memory, image) = materialized_fixture(MemoryLimits {
        max_physical_pages: 6,
        max_regions: 2,
        max_owners: 2,
        max_access_bytes: 4_096,
    });
    let catalog = RuntimeExportCatalog::new(vec![
        RuntimeExport::named("vine_fixture.dll", "fixture_add"),
        RuntimeExport::named("vine_fixture.dll", "fixture_call_callback"),
    ])
    .expect("complete catalog");
    let plan = plan_pe32_imports(&memory, &loader, &image, &catalog).expect("binding plan");
    let first = &plan.imports[0];
    memory
        .write_initialization(
            image.layout.owner,
            first.iat_address,
            &0x1234_5678u32.to_le_bytes(),
        )
        .expect("simulate post-plan IAT mutation");
    let before_commit = memory.stats();
    let error = commit_pe32_import_binding(&mut memory, &loader, &image, &plan)
        .expect_err("stale IAT plan");
    assert_eq!(error.code, "iat-slot-changed-after-planning");
    assert_eq!(memory.stats(), before_commit);
    assert_eq!(
        read_initialization_u32(&memory, image.layout.owner, first.iat_address),
        0x1234_5678
    );

    memory
        .write_initialization(
            image.layout.owner,
            first.iat_address,
            &first.original_iat_value.to_le_bytes(),
        )
        .expect("restore planned IAT");
    let binding = commit_pe32_import_binding(&mut memory, &loader, &image, &plan)
        .expect("retry original plan after restoring IAT");
    assert!(binding.complete);
}

#[test]
fn reports_all_unresolved_imports_across_modules_without_mutation() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".work/toolchain-spike/window-pixel.exe");
    let source = fs::read(&fixture).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}); run `npm run check:toolchain` first",
            fixture.display()
        )
    });
    let pe_limits = Pe32Limits::default();
    let snapshot = ModuleSnapshot::from_bytes(source, &pe_limits).expect("window snapshot");
    let loader = build_pe32_load_plan(&snapshot, pe_limits).expect("window load plan");
    assert!(loader.load_plan.imports.len() >= 3);
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 32,
        max_regions: 4,
        max_owners: 4,
        max_access_bytes: 4_096,
    })
    .expect("memory");
    let image = materialize_pe32_image(&mut memory, &snapshot, &loader, GuestAddress(LOAD_BASE))
        .expect("window materialization");
    let mut exports = Vec::new();
    for module in &loader.load_plan.imports {
        for symbol in module.symbols.iter().skip(1) {
            exports.push(runtime_export(&module.name, &symbol.target));
        }
    }
    let catalog = RuntimeExportCatalog::new(exports).expect("partial multi-module catalog");
    let before = memory.stats();
    let binding = bind_pe32_imports(&mut memory, &loader, &image, &catalog)
        .expect("multi-module unresolved report");
    assert!(!binding.complete);
    assert_eq!(binding.bound_imports, 0);
    assert_eq!(
        binding.unresolved_imports.len(),
        loader.load_plan.imports.len()
    );
    for (unresolved, module) in binding
        .unresolved_imports
        .iter()
        .zip(&loader.load_plan.imports)
    {
        assert_eq!(unresolved.module, module.name);
    }
    assert_eq!(memory.stats(), before);
}

#[test]
fn preflights_thunk_capacity_physical_quota_and_fixed_arena_conflicts() {
    let (_snapshot, loader, memory, image) = materialized_fixture(MemoryLimits {
        max_physical_pages: 6,
        max_regions: 2,
        max_owners: 2,
        max_access_bytes: 4_096,
    });
    let mut oversized_loader = loader.clone();
    let symbol = oversized_loader.load_plan.imports[0].symbols[0].clone();
    oversized_loader.load_plan.imports[0].symbols = vec![symbol; 4_097];
    let before = memory.stats();
    let capacity = plan_pe32_imports(
        &memory,
        &oversized_loader,
        &image,
        &RuntimeExportCatalog::new(Vec::new()).expect("empty catalog"),
    )
    .expect_err("thunk capacity");
    assert_eq!(capacity.code, "import-thunk-capacity-exceeded");
    assert_eq!(memory.stats(), before);

    let (_snapshot, loader, memory, image) = materialized_fixture(MemoryLimits {
        max_physical_pages: 5,
        max_regions: 2,
        max_owners: 2,
        max_access_bytes: 4_096,
    });
    let complete_catalog = RuntimeExportCatalog::new(vec![
        RuntimeExport::named("vine_fixture.dll", "fixture_add"),
        RuntimeExport::named("vine_fixture.dll", "fixture_call_callback"),
    ])
    .expect("complete catalog");
    let before = memory.stats();
    let quota = plan_pe32_imports(&memory, &loader, &image, &complete_catalog)
        .expect_err("thunk physical quota");
    assert_eq!(quota.code, "physical-page-quota-exceeded");
    assert_eq!(memory.stats(), before);

    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".work/toolchain-spike/abi-smoke.exe");
    let source = fs::read(&fixture).expect("fixture");
    let pe_limits = Pe32Limits::default();
    let snapshot = ModuleSnapshot::from_bytes(source, &pe_limits).expect("snapshot");
    let loader = build_pe32_load_plan(&snapshot, pe_limits).expect("load plan");
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 6,
        max_regions: 3,
        max_owners: 3,
        max_access_bytes: 4_096,
    })
    .expect("memory");
    let collision_owner = memory
        .create_owner(MemoryOwnerKind::Runtime)
        .expect("collision owner");
    memory
        .reserve_fixed(
            collision_owner,
            MemoryRegionKind::Runtime,
            IMPORT_THUNK_ARENA_BASE,
            0x1_0000,
        )
        .expect("collision range");
    let image = materialize_pe32_image(&mut memory, &snapshot, &loader, GuestAddress(LOAD_BASE))
        .expect("materialize around collision");
    let before = memory.stats();
    let conflict = plan_pe32_imports(&memory, &loader, &image, &complete_catalog)
        .expect_err("fixed arena conflict");
    assert_eq!(conflict.code, "memory-region-overlap");
    assert_eq!(memory.stats(), before);
}

fn materialized_fixture(
    limits: MemoryLimits,
) -> (
    ModuleSnapshot,
    vine_core::LoaderIngressReport,
    GuestMemory,
    vine_core::ImageMaterialization,
) {
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
    let mut memory = GuestMemory::new(limits).expect("guest memory");
    let image = materialize_pe32_image(&mut memory, &snapshot, &loader, GuestAddress(LOAD_BASE))
        .expect("materialized fixture");
    (snapshot, loader, memory, image)
}

fn read_initialization_u32(
    memory: &GuestMemory,
    owner: vine_core::MemoryOwnerId,
    address: GuestAddress,
) -> u32 {
    let bytes = memory
        .read_initialization(owner, address, 4)
        .expect("original IAT DWORD");
    u32::from_le_bytes(bytes.try_into().expect("four-byte original IAT DWORD"))
}

fn runtime_export(module: &str, target: &ImportTarget) -> RuntimeExport {
    match target {
        ImportTarget::Name { name, .. } => RuntimeExport::named(module, name),
        ImportTarget::Ordinal { ordinal } => RuntimeExport::ordinal(module, *ordinal),
    }
}

fn read_u32(memory: &GuestMemory, address: GuestAddress) -> u32 {
    let bytes = memory.read(address, 4).expect("bound IAT DWORD");
    u32::from_le_bytes(bytes.try_into().expect("four-byte bound IAT DWORD"))
}

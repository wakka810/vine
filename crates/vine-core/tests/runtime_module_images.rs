use std::fs;
use std::path::PathBuf;

use vine_core::{
    ExecutionSession, ExportLookup, ExportTarget, GuestAddress, GuestMemory, ImageMaterialization,
    LOGICAL_ENCODED_POINTER_NAMESPACE, LOGICAL_LOADED_MODULE_NAMESPACE, LoaderIngressReport,
    MemoryLimits, ModuleSnapshot, Pe32Limits, ProcessAddressSpaceLayout, RuntimeBootstrap,
    RuntimeClockState, RuntimeModule, RuntimeModuleImageErrorClass, RuntimeProcessId,
    RuntimeStartupInfo, RuntimeState, RuntimeThreadId, X86_STARTUP_INFO_W_BYTE_LENGTH,
    build_pe32_load_plan, materialize_pe32_image,
};

const FIXTURE_SHA256: &str = "a9c2530df2a89317048ae9380e4202c8bd41994d1ff0e14b43fa578bb32c4878";
const ORIGINAL_BASE: u32 = 0x1000_0000;
const BRIDGE_BASE: u32 = 0x1100_0000;
const UNKNOWN_BASE: u32 = 0x1200_0000;
const CYCLE_A_BASE: u32 = 0x1300_0000;
const CYCLE_B_BASE: u32 = 0x1400_0000;

#[test]
fn mapped_module_registry_resolves_bounded_exports_and_preserves_state_on_failure() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".work/toolchain-spike/vine_fixture.dll");
    let fixture_bytes = fs::read(&fixture).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}); run `npm run check:toolchain` first",
            fixture.display()
        )
    });
    let limits = Pe32Limits::default();
    let snapshot = ModuleSnapshot::from_bytes(fixture_bytes.clone(), &limits).unwrap();
    snapshot.verify_expected_sha256(FIXTURE_SHA256).unwrap();
    let loader = build_pe32_load_plan(&snapshot, limits).unwrap();
    let exports = loader
        .load_plan
        .exports
        .as_ref()
        .expect("fixture DLL export directory");
    assert_eq!(exports.module_name, "vine_fixture.dll");
    assert_eq!(exports.function_count, 2);
    assert_eq!(exports.name_count, 2);
    assert_eq!(exports.symbols.len(), 2);
    assert_eq!(exports.symbols[0].ordinal, 1);
    assert_eq!(exports.symbols[0].names, ["fixture_add"]);
    assert!(matches!(
        exports.symbols[0].target,
        ExportTarget::Address { rva } if rva.0 == 0x1000
    ));
    assert_eq!(exports.symbols[1].ordinal, 2);
    assert_eq!(exports.symbols[1].names, ["fixture_call_callback"]);
    assert!(matches!(
        exports.symbols[1].target,
        ExportTarget::Address { rva } if rva.0 == 0x1010
    ));

    let mut export_limited = limits;
    export_limited.max_exports = 1;
    let limited_snapshot =
        ModuleSnapshot::from_bytes(fixture_bytes.clone(), &export_limited).unwrap();
    assert_eq!(
        build_pe32_load_plan(&limited_snapshot, export_limited)
            .expect_err("bounded export count")
            .code,
        "export-function-limit-exceeded"
    );
    let malformed = synthetic_dll(
        &fixture_bytes,
        "malformed.dll",
        BRIDGE_BASE,
        Some("missing-separator"),
        None,
    );
    let malformed_snapshot = ModuleSnapshot::from_bytes(malformed, &limits).unwrap();
    assert_eq!(
        build_pe32_load_plan(&malformed_snapshot, limits)
            .expect_err("forwarder syntax is explicit")
            .code,
        "export-forwarder-syntax-invalid"
    );

    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 15,
        max_regions: 5,
        max_owners: 5,
        ..MemoryLimits::default()
    })
    .unwrap();
    let mut runtime = controlled_runtime();

    let (original_loader, original_image) =
        materialize(&mut memory, fixture_bytes.clone(), ORIGINAL_BASE);
    memory.seal_owner(original_image.layout.owner).unwrap();
    load_guest_module(&mut runtime, 0x4d01_0001, "vine_fixture.dll");
    let original = ExecutionSession::new(&mut memory, &mut runtime)
        .register_module_image(0x4d01_0001, &original_loader, &original_image)
        .unwrap()
        .clone();
    assert_eq!(original.load_base, GuestAddress(ORIGINAL_BASE));
    assert_eq!(original.source.sha256, FIXTURE_SHA256);
    assert_eq!(original.entry_point, None);

    let bridge_bytes = synthetic_dll(
        &fixture_bytes,
        "bridge.dll",
        BRIDGE_BASE,
        Some("vine_fixture.fixture_add"),
        Some("vine_fixture.#2"),
    );
    let (bridge_loader, bridge_image) = materialize(&mut memory, bridge_bytes, BRIDGE_BASE);
    load_guest_module(&mut runtime, 0x4d01_0002, "bridge.dll");
    let before_unsealed = runtime.module_images().clone();
    let unsealed = ExecutionSession::new(&mut memory, &mut runtime)
        .register_module_image(0x4d01_0002, &bridge_loader, &bridge_image)
        .expect_err("unsealed image owner");
    assert_eq!(unsealed.class, RuntimeModuleImageErrorClass::Conflict);
    assert_eq!(unsealed.code, "module-image-owner-unsealed");
    assert_eq!(runtime.module_images(), &before_unsealed);
    memory.seal_owner(bridge_image.layout.owner).unwrap();
    ExecutionSession::new(&mut memory, &mut runtime)
        .register_module_image(0x4d01_0002, &bridge_loader, &bridge_image)
        .unwrap();

    let direct_by_name = runtime
        .resolve_guest_export(
            "VINE_FIXTURE.DLL",
            ExportLookup::Name {
                name: "fixture_add".to_owned(),
            },
        )
        .unwrap();
    assert_eq!(direct_by_name.ordinal, 1);
    assert_eq!(direct_by_name.address, GuestAddress(ORIGINAL_BASE + 0x1000));
    assert!(direct_by_name.forwarders.is_empty());
    let direct_by_ordinal = runtime
        .resolve_guest_export("vine_fixture.dll", ExportLookup::Ordinal { ordinal: 2 })
        .unwrap();
    assert_eq!(direct_by_ordinal.names, ["fixture_call_callback"]);
    assert_eq!(
        direct_by_ordinal.address,
        GuestAddress(ORIGINAL_BASE + 0x1010)
    );

    let forwarded_by_name = runtime
        .resolve_guest_export(
            "bridge.dll",
            ExportLookup::Name {
                name: "fixture_add".to_owned(),
            },
        )
        .unwrap();
    assert_eq!(forwarded_by_name.defining_module, "vine_fixture.dll");
    assert_eq!(forwarded_by_name.address, direct_by_name.address);
    assert_eq!(forwarded_by_name.forwarders.len(), 1);
    assert_eq!(
        forwarded_by_name.forwarders[0].target_symbol,
        ExportLookup::Name {
            name: "fixture_add".to_owned()
        }
    );
    let forwarded_by_ordinal = runtime
        .resolve_guest_export("bridge.dll", ExportLookup::Ordinal { ordinal: 2 })
        .unwrap();
    assert_eq!(forwarded_by_ordinal.address, direct_by_ordinal.address);
    assert_eq!(forwarded_by_ordinal.forwarders.len(), 1);
    assert_eq!(
        forwarded_by_ordinal.forwarders[0].target_symbol,
        ExportLookup::Ordinal { ordinal: 2 }
    );

    let before_invalid_lookup = runtime.module_images().clone();
    assert_eq!(
        runtime
            .resolve_guest_export(
                "vine_fixture.dll",
                ExportLookup::Name {
                    name: "x".repeat(1_025),
                },
            )
            .expect_err("bounded export lookup name")
            .code,
        "export-lookup-name-invalid"
    );
    assert_eq!(
        runtime
            .resolve_guest_export(
                "vine_fixture.dll",
                ExportLookup::Ordinal { ordinal: 65_536 },
            )
            .expect_err("bounded export lookup ordinal")
            .code,
        "export-lookup-ordinal-invalid"
    );
    assert_eq!(runtime.module_images(), &before_invalid_lookup);

    let before_duplicate = runtime.module_images().clone();
    assert_eq!(
        ExecutionSession::new(&mut memory, &mut runtime)
            .register_module_image(0x4d01_0001, &original_loader, &original_image)
            .expect_err("duplicate mapped module")
            .code,
        "module-image-identity-duplicate"
    );
    assert_eq!(runtime.module_images(), &before_duplicate);

    let unknown_bytes = synthetic_dll(
        &fixture_bytes,
        "unknown.dll",
        UNKNOWN_BASE,
        Some("missing.fixture_add"),
        None,
    );
    let (unknown_loader, unknown_image) = materialize(&mut memory, unknown_bytes, UNKNOWN_BASE);
    memory.seal_owner(unknown_image.layout.owner).unwrap();
    load_guest_module(&mut runtime, 0x4d01_0003, "unknown.dll");
    ExecutionSession::new(&mut memory, &mut runtime)
        .register_module_image(0x4d01_0003, &unknown_loader, &unknown_image)
        .unwrap();
    let before_unknown = runtime.module_images().clone();
    assert_eq!(
        runtime
            .resolve_guest_export(
                "unknown.dll",
                ExportLookup::Name {
                    name: "fixture_add".to_owned(),
                },
            )
            .expect_err("unknown forwarder target")
            .code,
        "export-forwarder-target-module-unknown"
    );
    assert_eq!(runtime.module_images(), &before_unknown);

    let cycle_a_bytes = synthetic_dll(
        &fixture_bytes,
        "cycle_a.dll",
        CYCLE_A_BASE,
        Some("cycle_b.fixture_add"),
        None,
    );
    let cycle_b_bytes = synthetic_dll(
        &fixture_bytes,
        "cycle_b.dll",
        CYCLE_B_BASE,
        Some("cycle_a.fixture_add"),
        None,
    );
    let (cycle_a_loader, cycle_a_image) = materialize(&mut memory, cycle_a_bytes, CYCLE_A_BASE);
    let (cycle_b_loader, cycle_b_image) = materialize(&mut memory, cycle_b_bytes, CYCLE_B_BASE);
    memory.seal_owner(cycle_a_image.layout.owner).unwrap();
    memory.seal_owner(cycle_b_image.layout.owner).unwrap();
    load_guest_module(&mut runtime, 0x4d01_0004, "cycle_a.dll");
    load_guest_module(&mut runtime, 0x4d01_0005, "cycle_b.dll");
    ExecutionSession::new(&mut memory, &mut runtime)
        .register_module_image(0x4d01_0004, &cycle_a_loader, &cycle_a_image)
        .unwrap();
    ExecutionSession::new(&mut memory, &mut runtime)
        .register_module_image(0x4d01_0005, &cycle_b_loader, &cycle_b_image)
        .unwrap();
    let before_cycle = runtime.module_images().clone();
    assert_eq!(
        runtime
            .resolve_guest_export(
                "cycle_a.dll",
                ExportLookup::Name {
                    name: "fixture_add".to_owned(),
                },
            )
            .expect_err("forwarder cycle")
            .code,
        "export-forwarder-cycle"
    );
    assert_eq!(runtime.module_images(), &before_cycle);

    let overlap_bytes = synthetic_dll(&fixture_bytes, "overlap.dll", ORIGINAL_BASE, None, None);
    let mut overlap_memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 3,
        max_regions: 1,
        max_owners: 1,
        ..MemoryLimits::default()
    })
    .unwrap();
    let (overlap_loader, overlap_image) =
        materialize(&mut overlap_memory, overlap_bytes, ORIGINAL_BASE);
    overlap_memory
        .seal_owner(overlap_image.layout.owner)
        .unwrap();
    load_guest_module(&mut runtime, 0x4d01_0006, "overlap.dll");
    let before_overlap = runtime.module_images().clone();
    assert_eq!(
        ExecutionSession::new(&mut overlap_memory, &mut runtime)
            .register_module_image(0x4d01_0006, &overlap_loader, &overlap_image)
            .expect_err("overlapping mapped module range")
            .code,
        "module-image-range-overlap"
    );
    assert_eq!(runtime.module_images(), &before_overlap);
}

fn materialize(
    memory: &mut GuestMemory,
    bytes: Vec<u8>,
    load_base: u32,
) -> (LoaderIngressReport, ImageMaterialization) {
    let limits = Pe32Limits::default();
    let snapshot = ModuleSnapshot::from_bytes(bytes, &limits).unwrap();
    let loader = build_pe32_load_plan(&snapshot, limits).unwrap();
    assert_eq!(loader.load_plan.image.preferred_image_base, load_base);
    let image =
        materialize_pe32_image(memory, &snapshot, &loader, GuestAddress(load_base)).unwrap();
    (loader, image)
}

fn synthetic_dll(
    fixture: &[u8],
    module_name: &str,
    image_base: u32,
    first_forwarder: Option<&str>,
    second_forwarder: Option<&str>,
) -> Vec<u8> {
    assert_eq!(fixture.len(), 2_048);
    let mut bytes = fixture.to_vec();
    write_u32(&mut bytes, 0x00ac, image_base);
    write_ascii_slot(&mut bytes, 0x0628, 17, module_name);
    if first_forwarder.is_some() || second_forwarder.is_some() {
        write_u32(&mut bytes, 0x00f4, 0x100);
    }
    if let Some(forwarder) = first_forwarder {
        write_u32(&mut bytes, 0x0639, 0x2070);
        write_ascii_slot(&mut bytes, 0x0670, 32, forwarder);
    }
    if let Some(forwarder) = second_forwarder {
        write_u32(&mut bytes, 0x063d, 0x2090);
        write_ascii_slot(&mut bytes, 0x0690, 32, forwarder);
    }
    bytes
}

fn write_ascii_slot(bytes: &mut [u8], offset: usize, capacity: usize, value: &str) {
    assert!(value.is_ascii());
    assert!(value.len() < capacity);
    bytes[offset..offset + capacity].fill(0);
    bytes[offset..offset + value.len()].copy_from_slice(value.as_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn load_guest_module(runtime: &mut RuntimeState, handle: u32, name: &str) {
    runtime
        .load_module(RuntimeModule {
            owner: "runtime-module-image-test".to_owned(),
            variant_id: format!("mapped-{name}"),
            namespace: LOGICAL_LOADED_MODULE_NAMESPACE.to_owned(),
            handle,
            requested_name: name.to_owned(),
            canonical_name: name.to_owned(),
            runtime_system_module: false,
        })
        .unwrap();
}

fn controlled_runtime() -> RuntimeState {
    RuntimeState::new(RuntimeBootstrap {
        process_id: RuntimeProcessId(4096),
        initial_thread_id: RuntimeThreadId(8192),
        initial_last_error: 0,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
            performance_frequency: 10_000_000,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "runtime-module-image-test".to_owned(),
            bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
        },
        process_image_path_a: vine_core::RuntimeProcessImagePathA {
            source_authority: "runtime-module-image-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: vine_core::RuntimeCurrentDirectoryA {
            source_authority: "runtime-module-image-process-bootstrap".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_cookie: 0xec01_9d3b,
    })
    .unwrap()
}

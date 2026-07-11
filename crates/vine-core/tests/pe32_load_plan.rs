use std::fs;
use std::path::PathBuf;

use vine_core::{
    ImageKind, ImportTarget, LoaderErrorClass, ModuleSnapshot, Pe32Limits, Rva,
    build_pe32_load_plan,
};

const FIXTURE_SHA256: &str = "60f882a22d7d44e0ebe734f284dc8676e29035207ff004ed0cdf6e97c629b9c7";

#[test]
fn plans_fixed_abi_smoke_through_the_bootstrap_boundary() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".work/toolchain-spike/abi-smoke.exe");
    let bytes = fs::read(&fixture).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}); run `npm run check:toolchain` first",
            fixture.display()
        )
    });
    let limits = Pe32Limits::default();
    let snapshot = ModuleSnapshot::from_bytes(bytes, &limits).expect("fixture snapshot");
    snapshot
        .verify_expected_sha256(FIXTURE_SHA256)
        .expect("fixed fixture fingerprint");

    let report = build_pe32_load_plan(&snapshot, limits).expect("fixture load plan");
    assert_eq!(report.schema, "vine.pe32-loader-ingress");
    assert_eq!(report.schema_version, 1);
    assert_eq!(report.source.bytes, 2_560);
    assert_eq!(report.source.sha256, FIXTURE_SHA256);
    assert_eq!(report.load_plan.image.kind, ImageKind::Executable);
    assert_eq!(report.load_plan.image.preferred_image_base, 0x0040_0000);
    assert_eq!(report.load_plan.image.entry_point_rva, Rva(0x1000));
    assert_eq!(report.load_plan.sections.len(), 4);

    let module = report.load_plan.imports.first().expect("one import module");
    assert_eq!(report.load_plan.imports.len(), 1);
    assert_eq!(module.name, "vine_fixture.dll");
    assert_eq!(module.symbols.len(), 2);
    assert!(matches!(
        &module.symbols[0].target,
        ImportTarget::Name { name, .. } if name == "fixture_add"
    ));
    assert!(matches!(
        &module.symbols[1].target,
        ImportTarget::Name { name, .. } if name == "fixture_call_callback"
    ));
    assert_eq!(
        report.load_plan.relocations.entry_count,
        report.load_plan.relocations.absolute_count + report.load_plan.relocations.highlow_count
    );
    assert!(report.load_plan.relocations.highlow_count > 0);
    assert_eq!(report.stop.stage, "process-bootstrap");
    assert_eq!(report.stop.code, "peb-teb-process-parameters-unimplemented");

    let first = serde_json::to_vec_pretty(&report).expect("serialize report");
    let second = serde_json::to_vec_pretty(&report).expect("serialize report again");
    assert_eq!(first, second, "load-plan JSON must be deterministic");
}

#[test]
fn classifies_low_and_malformed_pe_alignment_before_materialization() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".work/toolchain-spike/abi-smoke.exe");
    let original = fs::read(&fixture).expect("fixed fixture");
    let limits = Pe32Limits::default();

    for alignment in [0x200, 0x400] {
        let snapshot =
            ModuleSnapshot::from_bytes(with_alignment(&original, alignment, alignment), &limits)
                .expect("low-alignment snapshot");
        let error = build_pe32_load_plan(&snapshot, limits).expect_err("unsupported low alignment");
        assert_eq!(error.class, LoaderErrorClass::Unsupported);
        assert_eq!(error.stage, "image-layout");
        assert_eq!(
            error.code,
            "low-section-alignment-materialization-unimplemented"
        );
        assert!(
            error
                .message
                .contains(&format!("FileAlignment {alignment:#x}"))
        );
        assert!(
            error
                .message
                .contains(&format!("SectionAlignment {alignment:#x}"))
        );
        assert!(error.message.contains("4 KiB GuestMemory backing"));
    }

    assert_alignment_error(&original, 0x200, 0x400, "low-section-alignment-mismatch");
    assert_alignment_error(
        &original,
        0x400,
        0x200,
        "section-alignment-below-file-alignment",
    );
    assert_alignment_error(&original, 0, 0x1000, "invalid-pe-alignment");
    assert_alignment_error(&original, 0x300, 0x1000, "invalid-pe-alignment");

    let normal = ModuleSnapshot::from_bytes(with_alignment(&original, 0x200, 0x1000), &limits)
        .expect("normal alignment snapshot");
    build_pe32_load_plan(&normal, limits).expect("normal 0x200/0x1000 image");
}

fn assert_alignment_error(original: &[u8], file: u32, section: u32, code: &str) {
    let limits = Pe32Limits::default();
    let snapshot = ModuleSnapshot::from_bytes(with_alignment(original, file, section), &limits)
        .expect("mutated alignment snapshot");
    let error = build_pe32_load_plan(&snapshot, limits).expect_err("invalid alignment");
    assert_eq!(error.class, LoaderErrorClass::InvalidInput);
    assert_eq!(error.code, code);
}

fn with_alignment(original: &[u8], file_alignment: u32, section_alignment: u32) -> Vec<u8> {
    let mut bytes = original.to_vec();
    let pe_offset = u32::from_le_bytes(bytes[0x3c..0x40].try_into().expect("e_lfanew")) as usize;
    let optional_header = pe_offset + 4 + 20;
    bytes[optional_header + 32..optional_header + 36]
        .copy_from_slice(&section_alignment.to_le_bytes());
    bytes[optional_header + 36..optional_header + 40]
        .copy_from_slice(&file_alignment.to_le_bytes());
    bytes
}

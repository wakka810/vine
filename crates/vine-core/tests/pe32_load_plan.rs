use std::fs;
use std::path::PathBuf;

use vine_core::{ImageKind, ImportTarget, ModuleSnapshot, Pe32Limits, Rva, build_pe32_load_plan};

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

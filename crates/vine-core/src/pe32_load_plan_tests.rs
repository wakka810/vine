use std::fs;
use std::path::PathBuf;

use crate::{ImageKind, ImportTarget, ModuleSnapshot, Pe32Limits, Rva, build_pe32_load_plan};

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
    let report = build_pe32_load_plan(&snapshot, limits).expect("fixture load plan");
    assert_eq!(report.source.bytes, 2_560);
    assert_eq!(report.source, snapshot.identity());
    assert_eq!(report.image.kind, ImageKind::Executable);
    assert_eq!(report.image.preferred_image_base, 0x0040_0000);
    assert_eq!(report.image.entry_point_rva, Rva(0x1000));
    assert_eq!(report.sections.len(), 4);

    let module = report.imports.first().expect("one import module");
    assert_eq!(report.imports.len(), 1);
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
    assert!(!report.relocations.is_empty());
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

    assert_alignment_error(&original, 0x200, 0x400);
    assert_alignment_error(&original, 0x400, 0x200);
    assert_alignment_error(&original, 0, 0x1000);
    assert_alignment_error(&original, 0x300, 0x1000);

    let normal = ModuleSnapshot::from_bytes(with_alignment(&original, 0x200, 0x1000), &limits)
        .expect("normal alignment snapshot");
    build_pe32_load_plan(&normal, limits).expect("normal 0x200/0x1000 image");
}

#[test]
fn rejects_unsupported_directories_and_contradictory_relocation_flags() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".work/toolchain-spike/abi-smoke.exe");
    let original = fs::read(fixture).expect("fixed fixture");
    let limits = Pe32Limits::default();
    let pe_offset = u32::from_le_bytes(original[0x3c..0x40].try_into().unwrap()) as usize;
    let optional_header = pe_offset + 4 + 20;

    // Each directory is mapped and bounded but requires unsupported Windows
    // loader behavior. Presence must still stop rather than silently ignoring it.
    for directory in [9, 13, 14] {
        let mut bytes = original.clone();
        let offset = optional_header + 96 + directory * 8;
        bytes[offset..offset + 4].copy_from_slice(&0x1000_u32.to_le_bytes());
        bytes[offset + 4..offset + 8].copy_from_slice(&4_u32.to_le_bytes());
        let snapshot = ModuleSnapshot::from_bytes(bytes, &limits).unwrap();
        build_pe32_load_plan(&snapshot, limits).expect_err("unsupported loader directory");
    }

    let mut bytes = original;
    let flags_offset = pe_offset + 4 + 18;
    let flags = u16::from_le_bytes(bytes[flags_offset..flags_offset + 2].try_into().unwrap());
    bytes[flags_offset..flags_offset + 2].copy_from_slice(&(flags | 1).to_le_bytes());
    let snapshot = ModuleSnapshot::from_bytes(bytes, &limits).unwrap();
    build_pe32_load_plan(&snapshot, limits).expect_err("stripped flag conflicts with relocations");
}

#[test]
fn relocation_limits_reject_without_consuming_the_owned_image() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".work/toolchain-spike/abi-smoke.exe");
    let limits = Pe32Limits::default();
    let snapshot = ModuleSnapshot::from_bytes(fs::read(fixture).unwrap(), &limits).unwrap();
    for bounded in [
        Pe32Limits {
            max_relocation_blocks: 0,
            ..limits
        },
        Pe32Limits {
            max_relocations: 0,
            ..limits
        },
    ] {
        build_pe32_load_plan(&snapshot, bounded).expect_err("bounded relocation parsing");
    }
    let parsed = build_pe32_load_plan(&snapshot, limits).expect("retry with valid limits");
    assert!(!parsed.relocations.is_empty());
}

fn assert_alignment_error(original: &[u8], file: u32, section: u32) {
    let limits = Pe32Limits::default();
    let snapshot = ModuleSnapshot::from_bytes(with_alignment(original, file, section), &limits)
        .expect("mutated alignment snapshot");
    build_pe32_load_plan(&snapshot, limits).expect_err("invalid alignment");
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

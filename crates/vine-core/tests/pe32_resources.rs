use std::fs;
use std::path::PathBuf;

use vine_core::{
    LoaderErrorClass, ModuleSnapshot, Pe32Limits, ResourceIdentifier, Rva, build_pe32_load_plan,
};

const FIXTURE_SHA256: &str = "2ddcca724644e16405cc03d71e3b0d5460ddb72ac60072a1781f128fd5e4da4c";

#[test]
fn plans_bounded_integer_icon_and_cursor_resources_from_the_immutable_pe() {
    let bytes = fixture_bytes();
    let limits = Pe32Limits::default();
    let snapshot = ModuleSnapshot::from_bytes(bytes.clone(), &limits).expect("fixture snapshot");
    snapshot
        .verify_expected_sha256(FIXTURE_SHA256)
        .expect("fixed icon fixture fingerprint");
    let report = build_pe32_load_plan(&snapshot, limits).expect("icon resource plan");
    let resources = report
        .load_plan
        .resources
        .as_ref()
        .expect("resource directory");
    assert_eq!(resources.directory_rva, Rva(0x2000));
    assert_eq!(resources.directory_size, 19_776);
    assert_eq!(resources.directory_count, 11);
    assert_eq!(resources.entry_count, 16);
    assert_eq!(resources.entries.len(), 6);

    let group = resources
        .integer_entries(14, 101)
        .next()
        .expect("group icon 101");
    assert_eq!(group.language, ResourceIdentifier::Id { id: 1041 });
    assert_eq!(group.data_rva, Rva(0x5c48));
    assert_eq!(group.size, 48);

    let icons = resources
        .entries
        .iter()
        .filter(|entry| matches!(&entry.resource_type, ResourceIdentifier::Id { id: 3 }))
        .collect::<Vec<_>>();
    assert_eq!(icons.len(), 3);
    assert_eq!(icons[0].name, ResourceIdentifier::Id { id: 1 });
    assert_eq!(icons[0].size, 9_640);
    assert_eq!(icons[1].name, ResourceIdentifier::Id { id: 2 });
    assert_eq!(icons[1].size, 4_264);
    assert_eq!(icons[2].name, ResourceIdentifier::Id { id: 3 });
    assert_eq!(icons[2].size, 1_128);

    let cursor_group = resources
        .integer_entries(12, 106)
        .next()
        .expect("group cursor 106");
    assert_eq!(cursor_group.language, ResourceIdentifier::Id { id: 1041 });
    assert_eq!(cursor_group.data_rva, Rva(0x6d28));
    assert_eq!(cursor_group.size, 20);
    let cursor = resources
        .integer_entries(1, 4)
        .next()
        .expect("cursor payload 4");
    assert_eq!(cursor.language, ResourceIdentifier::Id { id: 1041 });
    assert_eq!(cursor.data_rva, Rva(0x5c78));
    assert_eq!(cursor.size, 4_268);

    let first = serde_json::to_vec(&report).expect("serialize resource plan");
    let second = serde_json::to_vec(&report).expect("serialize resource plan again");
    assert_eq!(first, second, "resource plan must be deterministic");

    let mut data_limited = limits;
    data_limited.max_resource_data_entries = 5;
    let snapshot = ModuleSnapshot::from_bytes(bytes.clone(), &data_limited).unwrap();
    let error = build_pe32_load_plan(&snapshot, data_limited)
        .expect_err("resource data-entry bound must reject the sixth leaf");
    assert_eq!(error.class, LoaderErrorClass::ResourceLimit);
    assert_eq!(error.stage, "resources");
    assert_eq!(error.code, "resource-data-entry-limit-exceeded");

    let mut directory_limited = limits;
    directory_limited.max_resource_directories = 10;
    let snapshot = ModuleSnapshot::from_bytes(bytes, &directory_limited).unwrap();
    let error = build_pe32_load_plan(&snapshot, directory_limited)
        .expect_err("resource directory bound must reject the eleventh directory");
    assert_eq!(error.class, LoaderErrorClass::ResourceLimit);
    assert_eq!(error.code, "resource-directory-limit-exceeded");
}

#[test]
fn parses_named_resources_and_rejects_malformed_identifier_bands() {
    const RESOURCE_RAW_OFFSET: usize = 0x600;
    const RESOURCE_DIRECTORY_SIZE_OFFSET_FROM_OPTIONAL: usize = 96 + 2 * 8 + 4;

    let limits = Pe32Limits::default();
    let original = fixture_bytes();

    let mut named = original.clone();
    let pe_offset = read_u32(&named, 0x3c) as usize;
    let optional_offset = pe_offset + 4 + 20;
    write_u32(
        &mut named,
        optional_offset + RESOURCE_DIRECTORY_SIZE_OFFSET_FROM_OPTIONAL,
        0x3c00,
    );
    write_u16(&mut named, RESOURCE_RAW_OFFSET + 12, 1);
    write_u16(&mut named, RESOURCE_RAW_OFFSET + 14, 1);
    write_u32(&mut named, RESOURCE_RAW_OFFSET + 16, 0x8000_3be8);
    write_u16(&mut named, RESOURCE_RAW_OFFSET + 0x3be8, 4);
    for (index, unit) in "ICON".encode_utf16().enumerate() {
        write_u16(&mut named, RESOURCE_RAW_OFFSET + 0x3bea + index * 2, unit);
    }
    let snapshot = ModuleSnapshot::from_bytes(named, &limits).unwrap();
    let report = build_pe32_load_plan(&snapshot, limits).expect("named resource type");
    let resources = report.load_plan.resources.unwrap();
    assert_eq!(
        resources.entries[0].resource_type,
        ResourceIdentifier::Name {
            name: "ICON".to_owned()
        }
    );

    let mut invalid_band = original.clone();
    write_u16(&mut invalid_band, RESOURCE_RAW_OFFSET + 12, 1);
    write_u16(&mut invalid_band, RESOURCE_RAW_OFFSET + 14, 1);
    assert_resource_error(invalid_band, limits, "resource-entry-name-band-invalid");

    let mut reserved_id = original;
    write_u32(&mut reserved_id, RESOURCE_RAW_OFFSET + 16, 0x0001_0003);
    assert_resource_error(reserved_id, limits, "resource-id-reserved-bits-nonzero");
}

fn assert_resource_error(bytes: Vec<u8>, limits: Pe32Limits, code: &str) {
    let snapshot = ModuleSnapshot::from_bytes(bytes, &limits).unwrap();
    let error = build_pe32_load_plan(&snapshot, limits).expect_err(code);
    assert_eq!(error.class, LoaderErrorClass::InvalidInput);
    assert_eq!(error.stage, "resources");
    assert_eq!(error.code, code);
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn fixture_bytes() -> Vec<u8> {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".work/toolchain-spike/icon-resource.exe");
    fs::read(&fixture).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}); run `npm run check:toolchain` first",
            fixture.display()
        )
    })
}

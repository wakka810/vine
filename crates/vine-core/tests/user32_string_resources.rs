mod support;

use support::{BASE, Fixture, LAST_ERROR, OUTPUT, imported_pe};
use vine_core::{DispatchOutcome, GuestAddress, GuestRange};

const IMPORTS: &[(&str, &str)] = &[
    ("user32.dll", "LoadStringW"),
    ("user32.dll", "LoadAcceleratorsW"),
    ("kernel32.dll", "FindResourceW"),
    ("kernel32.dll", "LoadResource"),
    ("kernel32.dll", "SizeofResource"),
    ("kernel32.dll", "LockResource"),
];

fn fixture(malformed: bool) -> Fixture {
    Fixture::with_pe_bytes(IMPORTS, fixture_bytes(malformed))
}

fn fixture_bytes(malformed: bool) -> Vec<u8> {
    let mut bytes = imported_pe(IMPORTS);
    let word = |bytes: &mut [u8], offset: usize, value: u16| {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    };
    let dword = |bytes: &mut [u8], offset: usize, value: u32| {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    };
    dword(&mut bytes, 0x98 + 112, 0x3000);
    dword(&mut bytes, 0x98 + 116, 0x300);
    for offset in [0, 24] {
        word(&mut bytes, 0x2200 + offset + 14, 1);
    }
    word(&mut bytes, 0x2200 + 48 + 14, 2);
    for (offset, value) in [
        (16, 6),
        (20, 0x8000_0018),
        (40, 1),
        (44, 0x8000_0030),
        (64, 0x409),
        (68, 80),
        (72, 0x411),
        (76, 96),
    ] {
        dword(&mut bytes, 0x2200 + offset, value);
    }
    for (index, text) in [[0x45_u16, 0x4e, 0x47], [0x65e5, 0, 0x672c]]
        .iter()
        .enumerate()
    {
        let raw = 0x2300 + index * 0x100;
        let mut block = Vec::new();
        for slot in 0..16 {
            block.extend_from_slice(&(if slot == 2 { 3_u16 } else { 0 }).to_le_bytes());
            if slot == 2 {
                for unit in text {
                    block.extend_from_slice(&unit.to_le_bytes());
                }
            }
        }
        bytes[raw..raw + block.len()].copy_from_slice(&block);
        dword(&mut bytes, 0x2200 + 80 + index * 16, (raw + 0xe00) as u32);
        dword(&mut bytes, 0x2200 + 84 + index * 16, block.len() as u32);
    }
    if malformed {
        word(&mut bytes, 0x2404, u16::MAX);
    }
    bytes
}

fn load(f: &mut Fixture, instance: u32, id: u32, address: u32, capacity: u32) -> u32 {
    f.call(
        "user32.dll",
        "LoadStringW",
        &[instance, id, address, capacity],
    )
    .eax
    .unwrap()
}

#[test]
fn load_string_w_selects_japanese_and_preserves_utf16_and_module_storage() {
    let mut f = fixture(false);
    let resource_before = f.read(GuestAddress(0x403200), 38);
    for instance in [0, 0x400000] {
        for (capacity, count, expected) in [
            (1, 0, vec![0, 0]),
            (3, 2, vec![0xe5, 0x65, 0, 0, 0, 0]),
            (8, 3, vec![0xe5, 0x65, 0, 0, 0x2c, 0x67, 0, 0]),
        ] {
            f.write(OUTPUT, &[0xa5; 20]);
            assert_eq!(load(&mut f, instance, 2, OUTPUT.0, capacity), count);
            assert_eq!(f.read(OUTPUT, expected.len()), expected);
            assert_eq!(
                f.read(GuestAddress(OUTPUT.0 + expected.len() as u32), 2),
                [0xa5; 2]
            );
        }
        assert_eq!(load(&mut f, instance, 2, OUTPUT.0, 0), 3);
        assert_eq!(f.read(OUTPUT, 4), 0x403206_u32.to_le_bytes());
        assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    }
    assert_eq!(f.read(GuestAddress(0x403200), 38), resource_before);
    for capacity in [0, 8] {
        let before = f.read(OUTPUT, 0x1000);
        assert!(matches!(
            f.dispatch(
                "user32.dll",
                "LoadStringW",
                &[0x400000, 2, BASE.0 + 0x2ffe, capacity]
            ),
            DispatchOutcome::Fault(_)
        ));
        assert_eq!(f.read(OUTPUT, 0x1000), before);
    }
}

#[test]
fn load_string_w_rejects_missing_and_malformed_resources_without_output() {
    for malformed in [false, true] {
        let mut f = fixture(malformed);
        for (instance, id, capacity, error) in [
            (1, 2, 8, 6),
            (0x400000, 2, u32::MAX, 87),
            (
                0x400000,
                if malformed { 2 } else { 16 },
                8,
                if malformed { 1812 } else { 1814 },
            ),
        ] {
            f.write(OUTPUT, &[0xa5; 20]);
            assert_eq!(load(&mut f, instance, id, OUTPUT.0, capacity), 0);
            assert_eq!(f.runtime.current_last_error().unwrap(), error);
            assert_eq!(f.read(OUTPUT, 20), [0xa5; 20]);
        }
    }
}

#[test]
fn resource_handles_refer_to_owned_module_descriptors_and_data() {
    let mut bytes = fixture_bytes(false);
    bytes[0x220c..0x2210].copy_from_slice(&[1, 0, 0, 0]);
    bytes[0x2210..0x2214].copy_from_slice(&0x8000_0280_u32.to_le_bytes());
    bytes[0x2480..0x248a].copy_from_slice(&[4, 0, b'T', 0, b'E', 0, b'X', 0, b'T', 0]);
    let mut f = Fixture::with_pe_bytes(IMPORTS, bytes);
    f.write(OUTPUT, &[b't', 0, b'e', 0, b'x', 0, b't', 0, 0, 0]);
    f.write(GuestAddress(OUTPUT.0 + 0x20), &[b'#', 0, b'1', 0, 0, 0]);
    let call =
        |f: &mut Fixture, name: &str, args: &[u32]| f.call("kernel32.dll", name, args).eax.unwrap();
    let resource = call(&mut f, "FindResourceW", &[0, 1, OUTPUT.0]);
    assert_eq!(resource, 0x403060);
    assert_eq!(
        call(
            &mut f,
            "FindResourceW",
            &[0x400000, OUTPUT.0 + 0x20, OUTPUT.0]
        ),
        resource
    );
    assert_eq!(call(&mut f, "SizeofResource", &[0, resource]), 38);
    let data = call(&mut f, "LoadResource", &[0x400000, resource]);
    assert_eq!(data, 0x403200);
    assert_eq!(call(&mut f, "LockResource", &[data]), data);
    assert_eq!(
        f.read(GuestAddress(data + 6), 6),
        [0xe5, 0x65, 0, 0, 0x2c, 0x67]
    );
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    for (name, args) in [
        ("SizeofResource", vec![1, resource]),
        ("LoadResource", vec![0, data]),
        ("LockResource", vec![resource]),
        ("LockResource", vec![data + 1]),
    ] {
        assert_eq!(call(&mut f, name, &args), 0);
        assert_eq!(f.runtime.current_last_error().unwrap(), 6);
    }
    assert_eq!(
        f.call("user32.dll", "LoadAcceleratorsW", &[0x400000, 109])
            .eax,
        Some(0)
    );
    assert_eq!(f.runtime.current_last_error().unwrap(), 1813);
    f.runtime
        .memory_mut()
        .decommit(
            f.image_owner,
            GuestRange::new(GuestAddress(0x403000), 4096).unwrap(),
        )
        .unwrap();
    f.ack();
    for (name, args) in [
        ("FindResourceW", vec![0, 1, OUTPUT.0]),
        ("LoadResource", vec![0, resource]),
        ("SizeofResource", vec![0, resource]),
        ("LockResource", vec![data]),
    ] {
        assert!(matches!(
            f.dispatch("kernel32.dll", name, &args),
            DispatchOutcome::Fault(_)
        ));
    }
}

#[test]
fn present_accelerator_resource_does_not_falsely_report_success_or_absence() {
    let mut bytes = fixture_bytes(false);
    bytes[0x2210..0x2214].copy_from_slice(&9_u32.to_le_bytes());
    let mut f = Fixture::with_pe_bytes(IMPORTS, bytes);
    assert!(matches!(
        f.dispatch("user32.dll", "LoadAcceleratorsW", &[0x400000, 1]),
        DispatchOutcome::Fault(_)
    ));
}

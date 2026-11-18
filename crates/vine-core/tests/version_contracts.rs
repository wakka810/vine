mod support;
use support::{BASE, Fixture, OUTPUT, imported_pe};
use vine_core::{DispatchOutcome, GuestAddress};

const IMPORTS: &[(&str, &str)] = &[
    ("version.dll", "GetFileVersionInfoSizeW"),
    ("version.dll", "GetFileVersionInfoW"),
    ("version.dll", "VerQueryValueW"),
];
const PATH: GuestAddress = GuestAddress(OUTPUT.0 + 0x800);
const QUERY: GuestAddress = GuestAddress(OUTPUT.0 + 0xa00);
const POINTER: GuestAddress = GuestAddress(OUTPUT.0 + 0xc00);
const LENGTH: GuestAddress = GuestAddress(OUTPUT.0 + 0xc04);

fn wide(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain([0])
        .flat_map(u16::to_le_bytes)
        .collect()
}
fn record(key: &str, kind: u16, value: &[u8], children: &[Vec<u8>]) -> Vec<u8> {
    let count = if kind == 1 {
        value.len() / 2
    } else {
        value.len()
    };
    let mut bytes = [0_u16, count as u16, kind]
        .into_iter()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    bytes.extend(wide(key));
    bytes.resize((bytes.len() + 3) & !3, 0);
    bytes.extend_from_slice(value);
    for child in children {
        bytes.resize((bytes.len() + 3) & !3, 0);
        bytes.extend_from_slice(child);
    }
    let length = bytes.len() as u16;
    bytes[..2].copy_from_slice(&length.to_le_bytes());
    bytes
}

fn fixture() -> (Fixture, Vec<u8>) {
    let mut fixed = vec![0; 52];
    for (index, value) in [0xfeef04bd_u32, 0x10000, 0x10002, 0x30004]
        .into_iter()
        .enumerate()
    {
        fixed[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    let version = record(
        "VS_VERSION_INFO",
        0,
        &fixed,
        &[
            record(
                "StringFileInfo",
                1,
                &[],
                &[record(
                    "040904b0",
                    1,
                    &[],
                    &[record("CompanyName", 1, &wide("Test 日本"), &[])],
                )],
            ),
            record(
                "VarFileInfo",
                1,
                &[],
                &[record("Translation", 0, &[9, 4, 0xb0, 4], &[])],
            ),
        ],
    );
    let mut pe = imported_pe(IMPORTS);
    let dword = |pe: &mut [u8], offset: usize, value: u32| {
        pe[offset..offset + 4].copy_from_slice(&value.to_le_bytes())
    };
    dword(&mut pe, 0x98 + 112, 0x3000);
    dword(&mut pe, 0x98 + 116, 0x100 + version.len() as u32);
    for offset in [0, 24, 48] {
        pe[0x2200 + offset + 14..0x2200 + offset + 16].copy_from_slice(&1_u16.to_le_bytes());
    }
    for (offset, value) in [
        (16, 16),
        (20, 0x80000018),
        (40, 1),
        (44, 0x80000030),
        (64, 0x409),
        (68, 72),
        (72, 0x3100),
        (76, version.len() as u32),
    ] {
        dword(&mut pe, 0x2200 + offset, value);
    }
    pe[0x2300..0x2300 + version.len()].copy_from_slice(&version);
    let mut f = Fixture::with_pe_bytes(IMPORTS, pe);
    f.write(PATH, &wide("C:\\Vine\\fixture.exe"));
    (f, version)
}

#[test]
fn version_queries_use_immutable_file_bytes_and_return_pointers_into_the_copy() {
    let (mut f, version) = fixture();
    let size = f
        .call(
            "version.dll",
            "GetFileVersionInfoSizeW",
            &[PATH.0, LENGTH.0],
        )
        .eax
        .unwrap();
    assert_eq!(size, version.len() as u32 * 2 + 4);
    assert_eq!(f.read(LENGTH, 4), [0; 4]);
    f.write(GuestAddress(0x403100), &[0; 6]);
    f.write(OUTPUT, &vec![0xa5; size as usize + 4]);
    assert_eq!(
        f.call(
            "version.dll",
            "GetFileVersionInfoW",
            &[PATH.0, 0, size, OUTPUT.0]
        )
        .eax,
        Some(1)
    );
    assert_eq!(f.read(OUTPUT, version.len()), version);
    assert_eq!(f.read(GuestAddress(OUTPUT.0 + size), 4), [0xa5; 4]);
    for (query, expected) in [
        ("\\", version[40..92].to_vec()),
        ("\\varfileinfo\\translation", vec![9, 4, 0xb0, 4]),
        ("\\StringFileInfo\\040904b0\\CompanyName", wide("Test 日本")),
    ] {
        f.write(QUERY, &wide(query));
        assert_eq!(
            f.call(
                "version.dll",
                "VerQueryValueW",
                &[OUTPUT.0, QUERY.0, POINTER.0, LENGTH.0]
            )
            .eax,
            Some(1)
        );
        let pointer = u32::from_le_bytes(f.read(POINTER, 4).try_into().unwrap());
        assert!(pointer >= OUTPUT.0 && pointer < OUTPUT.0 + version.len() as u32);
        assert_eq!(f.read(GuestAddress(pointer), expected.len()), expected);
        let length = u32::from_le_bytes(f.read(LENGTH, 4).try_into().unwrap());
        assert_eq!(
            length as usize,
            if query.contains("CompanyName") {
                expected.len() / 2
            } else {
                expected.len()
            }
        );
    }
    f.write(QUERY, &wide("\\absent"));
    f.words(POINTER, &[0xdeadbeef]);
    assert_eq!(
        f.call(
            "version.dll",
            "VerQueryValueW",
            &[OUTPUT.0, QUERY.0, POINTER.0, LENGTH.0]
        )
        .eax,
        Some(0)
    );
    assert_eq!(f.read(POINTER, 4), 0xdeadbeef_u32.to_le_bytes());
    assert_eq!(f.read(LENGTH, 4), [0; 4]);
    assert_eq!(f.runtime.current_last_error().unwrap(), 1813);
}

#[test]
fn malformed_version_records_and_bad_outputs_do_not_partially_publish() {
    let (mut f, version) = fixture();
    f.write(OUTPUT, &version);
    f.write(QUERY, &wide("\\"));
    f.words(POINTER, &[0xdeadbeef, 0xdeadbeef]);
    for arguments in [
        [OUTPUT.0, QUERY.0, POINTER.0, BASE.0 + 0x2fff],
        [OUTPUT.0, QUERY.0, BASE.0 + 0x2fff, LENGTH.0],
    ] {
        assert!(matches!(
            f.dispatch("version.dll", "VerQueryValueW", &arguments),
            DispatchOutcome::Fault(_)
        ));
        assert_eq!(
            f.read(POINTER, 8),
            [0xef, 0xbe, 0xad, 0xde, 0xef, 0xbe, 0xad, 0xde]
        );
    }
    f.write(OUTPUT, &6_u16.to_le_bytes());
    assert!(matches!(
        f.dispatch(
            "version.dll",
            "VerQueryValueW",
            &[OUTPUT.0, QUERY.0, POINTER.0, LENGTH.0]
        ),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(
        f.read(POINTER, 8),
        [0xef, 0xbe, 0xad, 0xde, 0xef, 0xbe, 0xad, 0xde]
    );
}

mod support;

use support::{Fixture, LAST_ERROR, OUTPUT};
use vine_core::{DispatchOutcome, GuestAddress, GuestRange, PageProtection};

const SOURCE: GuestAddress = GuestAddress(OUTPUT.0 + 256);
const GDI: &str = "gdi32.dll";

// A minimal public-format TrueType face, independent of private game fonts.
fn font_bytes(east_asian: bool) -> Vec<u8> {
    let mut head = vec![0; 54];
    head[18..20].copy_from_slice(&1000_u16.to_be_bytes());
    let mut hhea = vec![0; 36];
    hhea[34..36].copy_from_slice(&1_u16.to_be_bytes());
    let mut os2 = vec![0; 86];
    os2[..2].copy_from_slice(&1_u16.to_be_bytes());
    os2[78..82].copy_from_slice(&(if east_asian { 1_u32 << 17 } else { 1 }).to_be_bytes());
    let mut cmap = vec![0; 274];
    cmap[2..4].copy_from_slice(&1_u16.to_be_bytes());
    cmap[4..6].copy_from_slice(&3_u16.to_be_bytes());
    cmap[6..8].copy_from_slice(&1_u16.to_be_bytes());
    cmap[8..12].copy_from_slice(&12_u32.to_be_bytes());
    cmap[14..16].copy_from_slice(&262_u16.to_be_bytes());
    let tables: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"OS/2", os2),
        (b"cmap", cmap),
        (b"glyf", vec![0; 10]),
        (b"head", head),
        (b"hhea", hhea),
        (b"hmtx", vec![1, 244, 0, 0]),
        (b"loca", vec![0; 4]),
        (b"maxp", vec![0, 1, 0, 0, 0, 1]),
        (b"name", vec![0, 0, 0, 0, 0, 6]),
    ];
    let mut bytes = vec![0; 12 + 16 * tables.len()];
    bytes[..4].copy_from_slice(&0x00010000_u32.to_be_bytes());
    bytes[4..6].copy_from_slice(&(tables.len() as u16).to_be_bytes());
    for (i, (tag, data)) in tables.into_iter().enumerate() {
        let offset = bytes.len() as u32;
        let record = 12 + i * 16;
        bytes[record..record + 4].copy_from_slice(tag);
        bytes[record + 8..record + 12].copy_from_slice(&offset.to_be_bytes());
        bytes[record + 12..record + 16].copy_from_slice(&(data.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&data);
    }
    bytes
}

fn fixture() -> Fixture {
    Fixture::new(&[
        (GDI, "AddFontMemResourceEx"),
        (GDI, "RemoveFontMemResourceEx"),
    ])
}

#[test]
fn memory_fonts_have_private_lifetimes_and_horizontal_vertical_face_counts() {
    let mut f = fixture();
    let mut handles = Vec::new();
    for east_asian in [false, true] {
        let bytes = font_bytes(east_asian);
        f.write(SOURCE, &bytes);
        for _ in 0..2 {
            let handle = f
                .call(
                    GDI,
                    "AddFontMemResourceEx",
                    &[SOURCE.0, bytes.len() as u32, 0, OUTPUT.0],
                )
                .eax
                .unwrap();
            assert_ne!(handle, 0);
            assert!(!handles.contains(&handle));
            handles.push(handle);
            assert_eq!(
                f.read(OUTPUT, 4),
                (if east_asian { 2_u32 } else { 1 }).to_le_bytes()
            );
            assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
        }
        f.write(SOURCE, &vec![0; bytes.len()]);
    }
    for handle in handles {
        assert_eq!(
            f.call(GDI, "RemoveFontMemResourceEx", &[handle]).eax,
            Some(1)
        );
        assert_eq!(
            f.call(GDI, "RemoveFontMemResourceEx", &[handle]).eax,
            Some(0)
        );
    }
    assert_eq!(f.call(GDI, "RemoveFontMemResourceEx", &[0]).eax, Some(0));
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
}

#[test]
fn invalid_fonts_and_unwritable_count_do_not_publish_resources_or_outputs() {
    let mut f = fixture();
    let bytes = font_bytes(true);
    f.write(SOURCE, &bytes);
    f.write(OUTPUT, &[0xa5; 4]);
    for (size, reserved) in [(0, 0), (bytes.len() as u32, 1), (16, 0), (u32::MAX, 0)] {
        assert_eq!(
            f.call(
                GDI,
                "AddFontMemResourceEx",
                &[SOURCE.0, size, reserved, OUTPUT.0]
            )
            .eax,
            Some(0)
        );
        assert_eq!(f.read(OUTPUT, 4), [0xa5; 4]);
    }
    let mut malformed = bytes.clone();
    malformed[20..24].copy_from_slice(&u32::MAX.to_be_bytes());
    f.write(SOURCE, &malformed);
    assert_eq!(
        f.call(
            GDI,
            "AddFontMemResourceEx",
            &[SOURCE.0, malformed.len() as u32, 0, OUTPUT.0]
        )
        .eax,
        Some(0)
    );
    f.write(SOURCE, &bytes);
    f.runtime
        .memory_mut()
        .protect(
            f.owner,
            GuestRange::new(OUTPUT, 4096).unwrap(),
            PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .unwrap();
    f.ack();
    assert!(matches!(
        f.dispatch(
            GDI,
            "AddFontMemResourceEx",
            &[SOURCE.0, bytes.len() as u32, 0, OUTPUT.0]
        ),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(f.read(OUTPUT, 4), [0xa5; 4]);
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    f.runtime
        .memory_mut()
        .protect(
            f.owner,
            GuestRange::new(OUTPUT, 4096).unwrap(),
            PageProtection {
                read: true,
                write: true,
                execute: false,
            },
        )
        .unwrap();
    f.ack();
    assert_eq!(
        f.call(
            GDI,
            "AddFontMemResourceEx",
            &[SOURCE.0, bytes.len() as u32, 0, OUTPUT.0]
        )
        .eax,
        Some(0x47030001)
    );
}

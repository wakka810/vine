mod support;

use support::{Fixture, OUTPUT};
use vine_core::{DispatchOutcome, GuestAddress, RuntimeDisplayProfile, RuntimeRect};

const GDI: &str = "gdi32.dll";

#[test]
fn true_color_dib_uses_only_its_header_and_releases_owned_guest_pixels() {
    let mut f = Fixture::new(
        &[
            "CreateDIBSection",
            "GetObjectA",
            "GetObjectW",
            "DeleteObject",
        ]
        .map(|name| (GDI, name)),
    );
    let header = GuestAddress(support::BASE.0 + 0x3000 - 40);
    let mut bytes = vec![0; 40];
    bytes[..4].copy_from_slice(&40_u32.to_le_bytes());
    bytes[4..8].copy_from_slice(&32_i32.to_le_bytes());
    bytes[8..12].copy_from_slice(&32_i32.to_le_bytes());
    bytes[12..14].copy_from_slice(&1_u16.to_le_bytes());
    bytes[14..16].copy_from_slice(&32_u16.to_le_bytes());
    f.write(header, &bytes);
    let before = f.runtime.memory().stats();
    assert!(matches!(
        f.dispatch(GDI, "CreateDIBSection", &[0, header.0, 0, 0xdeadbeef, 0, 0]),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(f.runtime.memory().stats().regions, before.regions);
    let bitmap = gdi(
        &mut f,
        "CreateDIBSection",
        &[0, header.0, 0, OUTPUT.0, 0, 0],
    );
    assert_ne!(bitmap, 0);
    let pixels = GuestAddress(u32::from_le_bytes(f.read(OUTPUT, 4).try_into().unwrap()));
    assert_eq!(f.read(pixels, 4096), [0; 4096]);
    f.write(pixels, &[0x10, 0x20, 0x30, 0]);
    for api in ["GetObjectA", "GetObjectW"] {
        assert_eq!(gdi(&mut f, api, &[bitmap, 24, OUTPUT.0]), 24);
        let info = f.read(OUTPUT, 24);
        assert_eq!(u32::from_le_bytes(info[12..16].try_into().unwrap()), 128);
        assert_eq!(&info[16..20], &[1, 0, 32, 0]);
        assert_eq!(&info[20..24], &pixels.0.to_le_bytes());
    }
    assert_eq!(gdi(&mut f, "DeleteObject", &[bitmap]), 1);
    assert!(f.runtime.memory().read(pixels, 4).is_err());
}

fn gdi(f: &mut Fixture, name: &str, args: &[u32]) -> u32 {
    f.call(GDI, name, args).eax.unwrap()
}

#[test]
fn screen_compatible_bitmap_roundtrips_colors_and_owns_its_selection() {
    let mut imports = vec![("user32.dll", "GetDC"), ("user32.dll", "ReleaseDC")];
    imports.extend(
        [
            "CreateCompatibleDC",
            "CreateCompatibleBitmap",
            "SetPixel",
            "GetPixel",
            "GetObjectA",
            "SelectObject",
            "DeleteObject",
            "DeleteDC",
        ]
        .map(|name| (GDI, name)),
    );
    let mut f = Fixture::new(&imports);
    let rect = RuntimeRect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    };
    f.runtime
        .configure_display_profile(RuntimeDisplayProfile {
            primary_monitor_rect: rect,
            primary_work_rect: rect,
        })
        .unwrap();
    let screen = f.call("user32.dll", "GetDC", &[0]).eax.unwrap();
    let dc = gdi(&mut f, "CreateCompatibleDC", &[screen]);
    let bitmap = gdi(&mut f, "CreateCompatibleBitmap", &[screen, 1, 1]);
    assert_ne!(bitmap, 0);
    let stock = gdi(&mut f, "SelectObject", &[dc, bitmap]);
    assert_ne!(stock, 0);
    for shift in [0, 8, 16] {
        for byte in 0..=255 {
            let color = byte << shift;
            assert_eq!(gdi(&mut f, "SetPixel", &[dc, 0, 0, color]), color);
            assert_eq!(gdi(&mut f, "GetPixel", &[dc, 0, 0]), color);
        }
    }
    assert_eq!(gdi(&mut f, "SetPixel", &[dc, 1, 0, 0]), u32::MAX);
    assert_eq!(gdi(&mut f, "GetPixel", &[dc, u32::MAX, 0]), u32::MAX);
    assert_eq!(gdi(&mut f, "GetPixel", &[dc, 0, 0]), 0xff0000);
    assert_eq!(gdi(&mut f, "DeleteObject", &[bitmap]), 0);
    let other = gdi(&mut f, "CreateCompatibleDC", &[screen]);
    assert_eq!(gdi(&mut f, "SelectObject", &[other, bitmap]), 0);
    assert_eq!(gdi(&mut f, "GetObjectA", &[bitmap, 24, OUTPUT.0]), 24);
    let description = f.read(OUTPUT, 24);
    assert_eq!(&description[4..12], &[1, 0, 0, 0, 1, 0, 0, 0]);
    assert_eq!(&description[16..24], &[1, 0, 32, 0, 0, 0, 0, 0]);
    assert_eq!(
        gdi(
            &mut f,
            "CreateCompatibleBitmap",
            &[screen, i32::MAX as u32, i32::MAX as u32]
        ),
        0
    );
    assert_eq!(
        gdi(&mut f, "CreateCompatibleBitmap", &[0xdeadbeef, 1, 1]),
        0
    );
    assert_eq!(gdi(&mut f, "SelectObject", &[dc, stock]), bitmap);
    assert_eq!(gdi(&mut f, "DeleteObject", &[bitmap]), 1);
    assert_eq!(gdi(&mut f, "SelectObject", &[other, bitmap]), 0);
    assert_eq!(gdi(&mut f, "DeleteDC", &[dc]), 1);
    assert_eq!(gdi(&mut f, "DeleteDC", &[other]), 1);
    assert_eq!(gdi(&mut f, "GetPixel", &[dc, 0, 0]), u32::MAX);
    assert_eq!(f.call("user32.dll", "ReleaseDC", &[0, screen]).eax, Some(1));
}

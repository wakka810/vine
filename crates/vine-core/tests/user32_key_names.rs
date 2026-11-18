mod support;

use support::{Fixture, LAST_ERROR, OUTPUT};
use vine_core::{DispatchOutcome, GuestAddress, GuestRange, PageProtection};

fn fixture() -> Fixture {
    Fixture::new(&[
        ("user32.dll", "MapVirtualKeyA"),
        ("user32.dll", "MapVirtualKeyW"),
        ("user32.dll", "GetKeyNameTextA"),
        ("user32.dll", "GetKeyNameTextW"),
    ])
}

#[test]
fn virtual_key_mapping_uses_scan_positions_and_preserves_last_error() {
    let mut f = fixture();
    for (key, scan) in [
        (0x41, 0x1e),
        (0x30, 0x0b),
        (0x10, 0x2a),
        (0xa1, 0x36),
        (0xa2, 0x1d),
        (0xa3, 0x1d),
        (0x25, 0x4b),
        (0x64, 0x4b),
        (0x70, 0x3b),
        (0x7b, 0x58),
        (0x87, 0x76),
        (0x90, 0x45),
        (0x2c, 0x54),
        (0x13, 0),
        (0x6c, 0),
        (0, 0),
        (0x100, 0),
    ] {
        for api in ["MapVirtualKeyA", "MapVirtualKeyW"] {
            assert_eq!(f.call("user32.dll", api, &[key, 0]).eax, Some(scan));
            assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
        }
    }
    assert!(matches!(
        f.dispatch("user32.dll", "MapVirtualKeyW", &[0x1e, 1]),
        DispatchOutcome::Fault(_)
    ));
}

#[test]
fn key_names_distinguish_extended_keys_and_generic_modifiers() {
    let mut f = fixture();
    for (bits, name) in [
        (0x001e0000, "A"),
        (0x004b0000, "Num 4"),
        (0x014b0000, "Left"),
        (0x001c0000, "Enter"),
        (0x011c0000, "Num Enter"),
        (0x00360000, "Right Shift"),
        (0x02360000, "Shift"),
        (0x011d0000, "Right Control"),
        (0x031d0000, "Ctrl"),
        (0x01380000, "Right Alt"),
        (0x03380000, "Alt"),
        (0x00450000, "Pause"),
        (0x01450000, "Num Lock"),
        (0x00540000, "Sys Req"),
    ] {
        for api in ["GetKeyNameTextA", "GetKeyNameTextW"] {
            f.write(OUTPUT, &[0xa5; 64]);
            let expected: Vec<u8> = if api.ends_with('W') {
                name.encode_utf16()
                    .chain([0])
                    .flat_map(u16::to_le_bytes)
                    .collect()
            } else {
                name.bytes().chain([0]).collect()
            };
            assert_eq!(
                f.call("user32.dll", api, &[bits, OUTPUT.0, 32]).eax,
                Some(name.len() as u32)
            );
            assert_eq!(
                f.read(OUTPUT, expected.len() + 2),
                [expected, vec![0xa5; 2]].concat()
            );
            assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
        }
    }
}

#[test]
fn key_name_buffers_are_bounded_and_failed_writes_are_atomic() {
    let mut f = fixture();
    f.write(OUTPUT, &[0xa5; 32]);
    assert_eq!(
        f.call("user32.dll", "GetKeyNameTextW", &[0x020e0000, OUTPUT.0, 4])
            .eax,
        Some(3)
    );
    assert_eq!(
        f.read(OUTPUT, 10),
        [b'B', 0, b'a', 0, b'c', 0, 0, 0, 0xa5, 0xa5]
    );
    assert_eq!(
        f.call("user32.dll", "GetKeyNameTextW", &[0x020e0000, OUTPUT.0, 1])
            .eax,
        Some(0)
    );
    assert_eq!(f.read(OUTPUT, 4), [0, 0, b'a', 0]);
    let before = f.read(OUTPUT, 32);
    for (bits, capacity) in [(0x020e0000, 0), (0x020e0000, u32::MAX), (0x02000000, 32)] {
        assert_eq!(
            f.call("user32.dll", "GetKeyNameTextW", &[bits, OUTPUT.0, capacity])
                .eax,
            Some(0)
        );
        assert_eq!(f.read(OUTPUT, 32), before);
        assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    }
    assert_eq!(
        f.call("user32.dll", "GetKeyNameTextW", &[0x02630000, OUTPUT.0, 32])
            .eax,
        Some(0)
    );
    assert_eq!(f.runtime.current_last_error().unwrap(), 87);
    assert_eq!(f.read(OUTPUT, 32), before);
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
    for output in [OUTPUT, GuestAddress(0)] {
        assert!(matches!(
            f.dispatch("user32.dll", "GetKeyNameTextW", &[0x020e0000, output.0, 32]),
            DispatchOutcome::Fault(_)
        ));
        assert_eq!(f.read(OUTPUT, 32), before);
        assert_eq!(f.runtime.current_last_error().unwrap(), 87);
    }
}

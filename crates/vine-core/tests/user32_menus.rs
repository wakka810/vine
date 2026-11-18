mod support;
use support::{Fixture, OUTPUT};
use vine_core::{DispatchOutcome, GuestAddress};

const USER: &str = "user32.dll";

fn fixture() -> Fixture {
    Fixture::new(&[
        (USER, "CreateMenu"),
        (USER, "CreatePopupMenu"),
        (USER, "GetMenuItemCount"),
        (USER, "InsertMenuItemW"),
        (USER, "SetMenuItemInfoW"),
        (USER, "GetMenuItemInfoW"),
        (USER, "RemoveMenu"),
    ])
}

fn info(f: &mut Fixture, mask: u32, command: u32, submenu: u32, text: u32) {
    f.words(
        OUTPUT,
        &[
            48, mask, 0, 0, command, submenu, 0, 0, 0x12345678, text, 0, 0,
        ],
    );
}

#[test]
fn menu_tree_copies_text_and_preserves_detached_submenus() {
    let mut f = fixture();
    let root = f.call(USER, "CreateMenu", &[]).eax.unwrap();
    let child = f.call(USER, "CreatePopupMenu", &[]).eax.unwrap();
    assert_ne!(root, child);
    let text = GuestAddress(OUTPUT.0 + 0x100);
    let title: Vec<u8> = "保存(&S)"
        .encode_utf16()
        .chain([0])
        .flat_map(u16::to_le_bytes)
        .collect();
    f.write(text, &title);
    info(&mut f, 0x33, 0x2001, 0, text.0);
    assert_eq!(
        f.call(USER, "InsertMenuItemW", &[child, u32::MAX, 1, OUTPUT.0])
            .eax,
        Some(1)
    );
    f.write(text, &[0; 32]);
    info(&mut f, 0x37, 0x2000, child, 0);
    assert_eq!(
        f.call(USER, "InsertMenuItemW", &[root, 0, 1, OUTPUT.0]).eax,
        Some(1)
    );

    // Command lookup walks the submenu tree. Size queries do not retain guest pointers.
    info(&mut f, 0x62, 0, 0, 0);
    assert_eq!(
        f.call(USER, "GetMenuItemInfoW", &[root, 0x2001, 0, OUTPUT.0])
            .eax,
        Some(1)
    );
    let result = f.read(OUTPUT, 48);
    assert_eq!(
        u32::from_le_bytes(result[16..20].try_into().unwrap()),
        0x2001
    );
    assert_eq!(
        u32::from_le_bytes(result[32..36].try_into().unwrap()),
        0x12345678
    );
    assert_eq!(u32::from_le_bytes(result[40..44].try_into().unwrap()), 6);
    info(&mut f, 0x40, 0, 0, text.0);
    f.words(GuestAddress(OUTPUT.0 + 40), &[3]);
    assert_eq!(
        f.call(USER, "GetMenuItemInfoW", &[child, 0, 1, OUTPUT.0])
            .eax,
        Some(1)
    );
    assert_eq!(
        f.read(text, 6),
        "保存\0"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    );

    // Updating fType does not discard the item data, submenu, or label.
    info(&mut f, 0x100, 0, 0, 0);
    f.words(GuestAddress(OUTPUT.0 + 8), &[0x4000]);
    assert_eq!(
        f.call(USER, "SetMenuItemInfoW", &[root, 0x2000, 0, OUTPUT.0])
            .eax,
        Some(1)
    );
    info(&mut f, 0x106, 0, 0, 0);
    assert_eq!(
        f.call(USER, "GetMenuItemInfoW", &[root, 0, 1, OUTPUT.0])
            .eax,
        Some(1)
    );
    let result = f.read(OUTPUT, 48);
    assert_eq!(
        u32::from_le_bytes(result[8..12].try_into().unwrap()),
        0x4000
    );
    assert_eq!(
        u32::from_le_bytes(result[20..24].try_into().unwrap()),
        child
    );
    assert_eq!(f.call(USER, "RemoveMenu", &[root, 0, 0x400]).eax, Some(1));
    assert_eq!(f.call(USER, "GetMenuItemCount", &[root]).eax, Some(0));
    assert_eq!(f.call(USER, "GetMenuItemCount", &[child]).eax, Some(1));
    info(&mut f, 2, 0, 0, 0);
    assert_eq!(
        f.call(USER, "GetMenuItemInfoW", &[root, 0x2001, 0, OUTPUT.0])
            .eax,
        Some(0)
    );
    assert_eq!(f.runtime.current_last_error().unwrap(), 1456);
}

#[test]
fn malformed_menu_input_and_failed_output_leave_tree_and_buffers_unchanged() {
    let mut f = fixture();
    let menu = f.call(USER, "CreatePopupMenu", &[]).eax.unwrap();
    info(&mut f, 0x33, 7, 0, 0xfffffff0);
    assert!(matches!(
        f.dispatch(USER, "InsertMenuItemW", &[menu, 0, 1, OUTPUT.0]),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(f.call(USER, "GetMenuItemCount", &[menu]).eax, Some(0));
    info(&mut f, 6, 7, menu, 0);
    assert!(matches!(
        f.dispatch(USER, "InsertMenuItemW", &[menu, 0, 1, OUTPUT.0]),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(f.call(USER, "GetMenuItemCount", &[menu]).eax, Some(0));
    let text = GuestAddress(OUTPUT.0 + 0x100);
    f.write(text, &[65, 0, 0, 0]);
    info(&mut f, 0x42, 7, 0, text.0);
    assert_eq!(
        f.call(USER, "InsertMenuItemW", &[menu, 0, 1, OUTPUT.0]).eax,
        Some(1)
    );
    info(&mut f, 0x42, 0, 0, 0xfffffff0);
    f.words(GuestAddress(OUTPUT.0 + 40), &[2]);
    let before = f.read(OUTPUT, 48);
    assert!(matches!(
        f.dispatch(USER, "GetMenuItemInfoW", &[menu, 0, 1, OUTPUT.0]),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(f.read(OUTPUT, 48), before);
    assert_eq!(f.call(USER, "GetMenuItemCount", &[menu]).eax, Some(1));
    assert_eq!(
        f.call(USER, "GetMenuItemCount", &[0xdeadbeef]).eax,
        Some(u32::MAX)
    );
}

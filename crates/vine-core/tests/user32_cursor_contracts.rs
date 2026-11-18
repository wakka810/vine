mod support;

use std::collections::BTreeSet;
use support::Fixture;

#[test]
fn predefined_cursors_share_ansi_wide_identity_and_support_selection() {
    let mut f = Fixture::new(&[
        ("user32.dll", "LoadCursorA"),
        ("user32.dll", "LoadCursorW"),
        ("user32.dll", "SetCursor"),
    ]);
    let mut handles = BTreeSet::new();
    let mut previous = 0;
    for id in [
        32512, 32513, 32514, 32515, 32516, 32642, 32643, 32644, 32645, 32646, 32648, 32649, 32650,
        32651,
    ] {
        let wide = f.call("user32.dll", "LoadCursorW", &[0, id]).eax.unwrap();
        let ansi = f.call("user32.dll", "LoadCursorA", &[0, id]).eax.unwrap();
        assert_eq!(wide, ansi);
        assert_ne!(wide, 0);
        assert!(handles.insert(wide));
        // Loading the initial arrow establishes the default current cursor.
        if id == 32512 {
            previous = wide;
        }
        assert_eq!(
            f.call("user32.dll", "SetCursor", &[wide]).eax,
            Some(previous)
        );
        previous = wide;
    }
}

mod support;

use support::{Fixture, LAST_ERROR, OUTPUT};
use vine_core::{DispatchOutcome, GuestAddress};

const DLL: &str = "advapi32.dll";

#[test]
fn readonly_registry_handles_missing_overrides_and_atomic_outputs() {
    let mut f = Fixture::new(&[
        (DLL, "RegOpenKeyExW"),
        (DLL, "RegQueryValueExW"),
        (DLL, "RegCloseKey"),
    ]);
    let text = GuestAddress(OUTPUT.0 + 0x100);
    f.write(
        text,
        &"cOnTrOl PaNeL\\CuRsOrS"
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    assert!(matches!(
        f.dispatch(
            DLL,
            "RegOpenKeyExW",
            &[0x80000001, text.0, 0, 0x20019, 0xfffffffe]
        ),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(
        f.call(
            DLL,
            "RegOpenKeyExW",
            &[0x80000001, text.0, 0, 0x20019, OUTPUT.0]
        )
        .eax,
        Some(0)
    );
    let key = u32::from_le_bytes(f.read(OUTPUT, 4).try_into().unwrap());
    assert_eq!(key, 0x52010001); // Bad output did not publish or consume a handle.
    f.write(
        text,
        &"Hand"
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    f.words(OUTPUT, &[0xaaaa, 0xbbbb, 0xcccc]);
    assert_eq!(
        f.call(
            DLL,
            "RegQueryValueExW",
            &[key, text.0, 0, OUTPUT.0, OUTPUT.0 + 8, OUTPUT.0 + 4]
        )
        .eax,
        Some(2)
    );
    assert_eq!(
        f.read(OUTPUT, 12),
        [0_u32, 0, 0xcccc]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    f.words(OUTPUT, &[0xaaaa, 0xbbbb]);
    assert!(matches!(
        f.dispatch(
            DLL,
            "RegQueryValueExW",
            &[key, text.0, 0, OUTPUT.0, 0, 0xfffffffe]
        ),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(
        f.read(OUTPUT, 8),
        [0xaaaa_u32, 0xbbbb]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    assert_eq!(f.call(DLL, "RegCloseKey", &[key]).eax, Some(0));
    assert_eq!(f.call(DLL, "RegCloseKey", &[key]).eax, Some(6));
    assert_eq!(
        f.call(DLL, "RegQueryValueExW", &[key, text.0, 0, 0, 0, 0])
            .eax,
        Some(6)
    );
    assert_eq!(
        f.call(
            DLL,
            "RegOpenKeyExW",
            &[0x80000001, text.0, 0, 0x20019, OUTPUT.0]
        )
        .eax,
        Some(2)
    );
    assert_eq!(f.read(OUTPUT, 4), [0; 4]);
    assert_eq!(
        f.call(DLL, "RegOpenKeyExW", &[0x80000001, 0, 0, 0x20019, OUTPUT.0])
            .eax,
        Some(0)
    );
    assert_eq!(f.read(OUTPUT, 4), 0x80000001_u32.to_le_bytes());
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
}

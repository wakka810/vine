mod support;

use support::{Fixture, OUTPUT};
use vine_core::DispatchOutcome;

#[test]
fn guid_uses_process_entropy_and_rejects_bad_output_before_advancing() {
    let imports = [("ole32.dll", "CoCreateGuid")];
    let mut f = Fixture::new(&imports);
    let mut reference = Fixture::new(&imports);
    f.runtime.configure_guid_seed([0x72; 32]).unwrap();
    reference.runtime.configure_guid_seed([0x72; 32]).unwrap();
    assert!(f.runtime.configure_guid_seed([1; 32]).is_err());
    assert!(matches!(
        f.dispatch("ole32.dll", "CoCreateGuid", &[0]),
        DispatchOutcome::Fault(_)
    ));
    let error = f.runtime.current_last_error().unwrap();
    assert_eq!(
        f.call("ole32.dll", "CoCreateGuid", &[OUTPUT.0]).eax,
        Some(0)
    );
    reference.call("ole32.dll", "CoCreateGuid", &[OUTPUT.0]);
    let first = f.read(OUTPUT, 16);
    assert_eq!(first, reference.read(OUTPUT, 16));
    assert_eq!(first[7] >> 4, 4);
    assert_eq!(first[8] >> 6, 2);
    assert_eq!(f.runtime.current_last_error().unwrap(), error);
    f.call("ole32.dll", "CoCreateGuid", &[OUTPUT.0]);
    assert_ne!(f.read(OUTPUT, 16), first);
}

mod support;
use support::{Fixture, OUTPUT, RETURN};
use vine_core::{DispatchOutcome, GuestAddress};

const K: &str = "kernel32.dll";

#[test]
fn virtual_query_reports_live_protection_and_rejects_output_before_writing() {
    let mut f = Fixture::new(&[
        (K, "VirtualQuery"),
        (K, "VirtualAlloc"),
        (K, "VirtualProtect"),
        (K, "VirtualFree"),
    ]);
    let allocation = f
        .call(K, "VirtualAlloc", &[0, 8192, 0x3000, 4])
        .eax
        .unwrap();
    let query = |f: &mut Fixture, address| {
        assert_eq!(
            f.call(K, "VirtualQuery", &[address, OUTPUT.0, 28]).eax,
            Some(28)
        );
        f.read(OUTPUT, 28)
            .chunks_exact(4)
            .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
            .collect::<Vec<_>>()
    };
    let first = query(&mut f, allocation + 123);
    assert_eq!(first, [allocation, allocation, 4, 8192, 0x1000, 4, 0x20000]);
    assert_eq!(
        f.call(
            K,
            "VirtualProtect",
            &[allocation + 4096, 4096, 2, OUTPUT.0 + 32]
        )
        .eax,
        Some(1)
    );
    assert_eq!(
        query(&mut f, allocation + 4096),
        [allocation + 4096, allocation, 4, 4096, 0x1000, 2, 0x20000]
    );
    f.write(OUTPUT, &[0xa5; 28]);
    assert_eq!(
        f.call(K, "VirtualQuery", &[allocation, OUTPUT.0, 27]).eax,
        Some(0)
    );
    assert_eq!(f.read(OUTPUT, 28), [0xa5; 28]);
    assert!(matches!(
        f.dispatch(K, "VirtualQuery", &[allocation, 0xfffffff0, 28]),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(f.read(OUTPUT, 28), [0xa5; 28]);
    assert_eq!(
        f.call(K, "VirtualFree", &[allocation, 0, 0x8000]).eax,
        Some(1)
    );
    let free = query(&mut f, allocation);
    assert_eq!(free[0], allocation);
    assert_eq!(free[4], 0x10000);
    assert_eq!(free[6], 0);
}

#[test]
fn ansi_environment_snapshots_are_independent_and_keep_peb_authority_current() {
    let imports = [
        "GetEnvironmentStrings",
        "GetEnvironmentStringsW",
        "FreeEnvironmentStringsA",
        "FreeEnvironmentStringsW",
        "SetEnvironmentVariableA",
    ];
    let mut f = Fixture::new(&imports.map(|name| (K, name)));
    let environment = f
        .runtime
        .install_process_environment("fixture.exe")
        .unwrap();
    f.ack();
    let first = f.call(K, imports[0], &[]).eax.unwrap();
    assert_eq!(f.read(GuestAddress(first), 2), [0, 0]);
    f.write(OUTPUT, b"VINE_TEST\0value\0");
    assert_eq!(
        f.call(K, imports[4], &[OUTPUT.0, OUTPUT.0 + 10]).eax,
        Some(1)
    );
    let second = f.call(K, imports[0], &[]).eax.unwrap();
    assert_ne!(first, second);
    assert_eq!(f.read(GuestAddress(first), 2), [0, 0]);
    assert_eq!(f.read(GuestAddress(second), 17), b"VINE_TEST=value\0\0");
    let wide = f.call(K, imports[1], &[]).eax.unwrap();
    let expected: Vec<u8> = "VINE_TEST=value\0\0"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    assert_eq!(f.read(GuestAddress(wide), expected.len()), expected);
    let parameters = environment.process_parameters;
    let pointer = f.read(
        GuestAddress(parameters.process_parameters_address.0 + 0x48),
        4,
    );
    let authority = GuestAddress(u32::from_le_bytes(pointer.try_into().unwrap()));
    assert_eq!(f.read(authority, expected.len()), expected);
    assert_eq!(f.call(K, imports[2], &[wide]).eax, Some(0));
    for pointer in [first, second] {
        assert_eq!(f.call(K, imports[2], &[pointer]).eax, Some(1));
    }
    assert_eq!(f.call(K, imports[3], &[wide]).eax, Some(1));
    assert_eq!(f.call(K, imports[2], &[first]).eax, Some(0));
    f.write(OUTPUT, b"vine_test\0");
    assert_eq!(f.call(K, imports[4], &[OUTPUT.0, 0]).eax, Some(1));
    let empty = f.call(K, imports[0], &[]).eax.unwrap();
    assert_eq!(f.read(GuestAddress(empty), 2), [0, 0]);
}

#[test]
fn ansi_mapping_sizes_before_writing_and_pointer_checks_use_execute_permission() {
    let imports = [
        "LCMapStringA",
        "GetStringTypeA",
        "GetCurrentDirectoryA",
        "IsBadCodePtr",
    ];
    let mut f = Fixture::new(&imports.map(|name| (K, name)));
    f.write(OUTPUT, b"Ab9\0");
    assert_eq!(
        f.call(K, imports[0], &[0x411, 0x100, OUTPUT.0, u32::MAX, 0, 0])
            .eax,
        Some(4)
    );
    f.write(GuestAddress(OUTPUT.0 + 32), &[0xa5; 8]);
    assert_eq!(
        f.call(
            K,
            imports[0],
            &[0x411, 0x100, OUTPUT.0, u32::MAX, OUTPUT.0 + 32, 3]
        )
        .eax,
        Some(0)
    );
    assert_eq!(f.read(GuestAddress(OUTPUT.0 + 32), 8), [0xa5; 8]);
    assert_eq!(
        f.call(
            K,
            imports[0],
            &[0x411, 0x100, OUTPUT.0, u32::MAX, OUTPUT.0 + 32, 4]
        )
        .eax,
        Some(4)
    );
    assert_eq!(f.read(GuestAddress(OUTPUT.0 + 32), 4), b"ab9\0");
    assert_eq!(
        f.call(K, imports[1], &[0x411, 1, OUTPUT.0, 3, OUTPUT.0 + 32])
            .eax,
        Some(1)
    );
    assert_eq!(
        f.read(GuestAddress(OUTPUT.0 + 32), 6),
        [0x81, 3, 0x82, 3, 0x84, 2]
    );
    assert_eq!(f.call(K, imports[2], &[0, 0]).eax, Some(8));
    assert_eq!(f.call(K, imports[2], &[8, OUTPUT.0 + 32]).eax, Some(7));
    assert_eq!(f.read(GuestAddress(OUTPUT.0 + 32), 8), b"C:\\Vine\0");
    assert_eq!(f.call(K, imports[3], &[RETURN.0]).eax, Some(0));
    assert_eq!(f.call(K, imports[3], &[OUTPUT.0]).eax, Some(1));
    assert!(matches!(
        f.dispatch(K, imports[0], &[0x411, 0x100, 0xffff_ffff, 3, OUTPUT.0, 3]),
        DispatchOutcome::Fault(_)
    ));
}

#[test]
fn startup_user_state_and_absent_multimedia_devices_have_real_failures() {
    let mut f = Fixture::new(&[
        ("user32.dll", "SystemParametersInfoA"),
        ("user32.dll", "GetKeyboardState"),
        ("user32.dll", "SetKeyboardState"),
        ("winmm.dll", "midiOutOpen"),
        ("winmm.dll", "joyGetPosEx"),
        (K, "SetStdHandle"),
        (K, "GetStdHandle"),
    ]);
    let u = "user32.dll";
    f.write(OUTPUT, &[0xa5; 256]);
    assert_eq!(
        f.call(u, "SystemParametersInfoA", &[0x10, 0, OUTPUT.0, 0])
            .eax,
        Some(1)
    );
    assert_eq!(f.read(OUTPUT, 4), [0; 4]);
    assert_eq!(
        f.call(u, "SystemParametersInfoA", &[0x11, 1, 0, 0]).eax,
        Some(1)
    );
    assert_eq!(
        f.call(u, "SystemParametersInfoA", &[0x10, 0, OUTPUT.0, 0])
            .eax,
        Some(1)
    );
    assert_eq!(f.read(OUTPUT, 4), 1u32.to_le_bytes());
    assert_eq!(
        f.call(u, "SystemParametersInfoA", &[0x11, 0, 0, 1]).eax,
        Some(0)
    );
    assert_eq!(
        f.call(u, "SystemParametersInfoA", &[0x10, 0, OUTPUT.0, 0])
            .eax,
        Some(1)
    );
    assert_eq!(f.read(OUTPUT, 4), 1u32.to_le_bytes());
    f.write(OUTPUT, &[0xff; 256]);
    assert_eq!(f.call(u, "SetKeyboardState", &[OUTPUT.0]).eax, Some(1));
    assert_eq!(f.call(u, "GetKeyboardState", &[OUTPUT.0]).eax, Some(1));
    assert_eq!(f.read(OUTPUT, 256), [0x81; 256]);
    f.write(OUTPUT, &[0xa5; 52]);
    assert_eq!(
        f.call("winmm.dll", "midiOutOpen", &[OUTPUT.0, u32::MAX, 0, 0, 0])
            .eax,
        Some(6)
    );
    assert_eq!(f.read(OUTPUT, 4), [0xa5; 4]);
    f.words(OUTPUT, &[52, 0xff]);
    let before = f.read(OUTPUT, 52);
    assert_eq!(
        f.call("winmm.dll", "joyGetPosEx", &[0, OUTPUT.0]).eax,
        Some(167)
    );
    assert_eq!(f.read(OUTPUT, 52), before);
    assert_eq!(f.call(K, "SetStdHandle", &[0xfffffff5, 0]).eax, Some(1));
    assert_eq!(f.call(K, "GetStdHandle", &[0xfffffff5]).eax, Some(0));
    assert_eq!(f.call(K, "SetStdHandle", &[0, 123]).eax, Some(0));
}

#[test]
fn process_termination_is_a_terminal_boundary_and_invalid_handles_do_not_exit() {
    use vine_core::RuntimeApiBoundaryDisposition;
    let mut f = Fixture::new(&[(K, "TerminateProcess")]);
    assert_eq!(f.call(K, "TerminateProcess", &[123, 9]).eax, Some(0));
    let exit = f.call(K, "TerminateProcess", &[u32::MAX, 9]);
    assert_eq!(exit.eax, Some(9));
    assert_eq!(
        exit.execution_boundary,
        RuntimeApiBoundaryDisposition::TerminateProcess
    );
    assert!(f.runtime.select_next_executable_thread().is_err());
}

#[test]
fn d3d8_factory_supports_both_public_sdk_versions_with_owned_com_identity() {
    let d = "d3d8.dll";
    let mut f = Fixture::new(&[(d, "Direct3DCreate8")]);
    assert_eq!(f.call(d, "Direct3DCreate8", &[0]).eax, Some(0));
    for version in [120, 220] {
        let pointer = f.call(d, "Direct3DCreate8", &[version]).eax.unwrap();
        assert_ne!(pointer, 0);
        let table = u32::from_le_bytes(f.read(GuestAddress(pointer), 4).try_into().unwrap());
        assert_ne!(table, 0);
        assert_eq!(f.read(GuestAddress(table), 4).len(), 4);
    }
}

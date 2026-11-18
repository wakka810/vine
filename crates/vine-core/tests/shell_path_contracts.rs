mod support;
use support::{BASE, Fixture, LAST_ERROR, OUTPUT};
use vine_core::{
    DispatchOutcome, GuestAddress, GuestRange, HostCompletion, PageProtection,
    RuntimeSourceFileManifest, RuntimeSourceMountManifest, RuntimeSourceSnapshotManifest,
    complete_shell32_special_folder,
};

fn fixture() -> Fixture {
    fixture_with_root(b"C:\\Vine\0".to_vec())
}

fn fixture_with_root(root_path_a: Vec<u8>) -> Fixture {
    let mut f = Fixture::new(&[
        ("shell32.dll", "SHGetSpecialFolderPathW"),
        ("mpr.dll", "WNetGetUniversalNameW"),
        ("kernel32.dll", "GetTempPathW"),
    ]);
    f.runtime
        .register_source_snapshot(RuntimeSourceSnapshotManifest {
            mount: RuntimeSourceMountManifest {
                source_id: 1,
                root_path_a,
            },
            files: vec![RuntimeSourceFileManifest {
                object_id: 1,
                relative_path_a: b"fixture.exe".to_vec(),
                byte_length: 4096,
            }],
        })
        .unwrap();
    f
}

#[test]
fn temporary_path_uses_the_writable_mount_root_and_native_buffer_bounds() {
    let mut f = fixture();
    let name: Vec<u8> = "C:\\vine\\"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    for capacity in [0_u32, 1, 7, 8, 9, 260] {
        f.write(OUTPUT, &[0xa5; 524]);
        let result = f.call("kernel32.dll", "GetTempPathW", &[capacity, OUTPUT.0]);
        assert_eq!(result.eax, Some(if capacity <= 8 { 9 } else { 8 }));
        let mut expected = vec![0; capacity as usize * 2];
        if capacity >= 8 {
            expected[..name.len()].copy_from_slice(&name);
        }
        if !expected.is_empty() {
            assert_eq!(f.read(OUTPUT, expected.len()), expected);
        }
        assert_eq!(f.read(GuestAddress(OUTPUT.0 + capacity * 2), 4), [0xa5; 4]);
        assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    }
    assert_eq!(f.call("kernel32.dll", "GetTempPathW", &[0, 0]).eax, Some(9));
    f.write(GuestAddress(BASE.0 + 0x2ff0), &[0xa5; 16]);
    assert!(matches!(
        f.dispatch("kernel32.dll", "GetTempPathW", &[9, BASE.0 + 0x2ff0]),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(f.read(GuestAddress(BASE.0 + 0x2ff0), 16), [0xa5; 16]);
    assert!(matches!(
        f.dispatch("kernel32.dll", "GetTempPathW", &[32769, OUTPUT.0]),
        DispatchOutcome::Fault(_)
    ));
}

#[test]
fn temporary_path_appends_a_separator_after_cp932_backslash_trail_bytes() {
    let (root, _, invalid) = encoding_rs::SHIFT_JIS.encode("C:\\ソ\0");
    assert!(!invalid);
    let mut f = fixture_with_root(root.into_owned());
    assert_eq!(
        f.call("kernel32.dll", "GetTempPathW", &[260, OUTPUT.0]).eax,
        Some(5)
    );
    let expected: Vec<u8> = "C:\\ソ\\\0"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    assert_eq!(f.read(OUTPUT, expected.len()), expected);
}

#[test]
fn special_folders_publish_only_after_durable_completion() {
    for (csidl, relative) in [(5, "Documents"), (0x1a, "AppData/Roaming")] {
        for observation in [0, 3] {
            let mut f = fixture();
            f.write(OUTPUT, &[0xa5; 520]);
            let DispatchOutcome::Deferred(call) = f.dispatch(
                "shell32.dll",
                "SHGetSpecialFolderPathW",
                &[0, OUTPUT.0, csidl, 0],
            ) else {
                panic!("expected directory provisioning")
            };
            assert_eq!(call.request.service, "browser.opfs-ensure-directory");
            assert_eq!(&call.request.payload[1..], relative.as_bytes());
            assert_eq!(f.read(OUTPUT, 520), [0xa5; 520]);
            assert!(complete_shell32_special_folder(&call, 99, &f.runtime).is_err());
            let result = complete_shell32_special_folder(&call, observation, &f.runtime).unwrap();
            f.runtime
                .complete_host_request(call.request.request_id)
                .unwrap();
            let DispatchOutcome::Immediate(completion) = f.dispatcher.complete_deferred(
                &call,
                HostCompletion {
                    request_id: call.request.request_id,
                    result,
                },
                &mut f.runtime,
            ) else {
                panic!("expected completion")
            };
            assert_eq!(completion.eax, Some(u32::from(observation == 0)));
            f.ack();
            if observation == 0 {
                let expected = format!("C:\\vine\\{}\0", relative.replace('/', "\\"));
                assert_eq!(
                    f.read(OUTPUT, expected.len() * 2),
                    expected
                        .encode_utf16()
                        .flat_map(u16::to_le_bytes)
                        .collect::<Vec<_>>()
                );
                assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
            } else {
                assert_eq!(f.read(OUTPUT, 520), [0xa5; 520]);
                assert_eq!(f.runtime.current_last_error().unwrap(), 5);
            }
        }
    }
}

#[test]
fn special_folder_completion_rejects_an_unwritable_output() {
    let mut f = fixture();
    let DispatchOutcome::Deferred(call) = f.dispatch(
        "shell32.dll",
        "SHGetSpecialFolderPathW",
        &[0, OUTPUT.0, 5, 0],
    ) else {
        panic!("expected provisioning")
    };
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
    let before = f.runtime.memory().stats();
    assert!(complete_shell32_special_folder(&call, 0, &f.runtime).is_err());
    assert_eq!(f.runtime.memory().stats(), before);
}

#[test]
fn local_drives_have_no_universal_network_name_and_do_not_modify_outputs() {
    let mut f = fixture();
    let path = GuestAddress(OUTPUT.0 + 0x100);
    f.write(
        path,
        &"C:\\Vine\\AppData\0"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    f.words(OUTPUT, &[0]);
    let result = f.call(
        "mpr.dll",
        "WNetGetUniversalNameW",
        &[path.0, 1, 0, OUTPUT.0],
    );
    assert_eq!(result.eax, Some(2250));
    assert_eq!(f.read(OUTPUT, 4), [0; 4]);
    assert_eq!(f.runtime.current_last_error().unwrap(), 2250);
}

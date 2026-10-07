mod support;

use support::{BASE, Fixture, LAST_ERROR, OUTPUT, RETURN, THREAD};
use vine_core::{
    DispatchOutcome, GuestAddress, GuestRange, PageProtection, RuntimeApiBoundaryDisposition,
    RuntimeThreadId, SchedulerThreadState,
};

const KERNEL: &str = "kernel32.dll";
const CROSS_PAGE: GuestAddress = GuestAddress(BASE.0 + 0x2ff0);

fn fixture(names: &[&str]) -> Fixture {
    Fixture::new(&names.iter().map(|&name| (KERNEL, name)).collect::<Vec<_>>())
}

fn value(fixture: &mut Fixture, name: &str, args: &[u32]) -> u32 {
    fixture.call(KERNEL, name, args).eax.expect("DWORD result")
}

fn last_error(fixture: &mut Fixture) -> u32 {
    value(fixture, "GetLastError", &[])
}

fn set_error(fixture: &mut Fixture, error: u32) {
    assert_eq!(fixture.call(KERNEL, "SetLastError", &[error]).eax, None);
}

fn fault_preserves_outputs(fixture: &mut Fixture, name: &str, args: &[u32]) {
    let output = fixture.read(OUTPUT, 0x1000);
    let error = fixture.runtime.current_last_error().unwrap();
    assert!(
        matches!(
            fixture.dispatch(KERNEL, name, args),
            DispatchOutcome::Fault(_)
        ),
        "{name}"
    );
    assert_eq!(
        fixture.read(OUTPUT, 0x1000),
        output,
        "{name} partially wrote output"
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), error);
    assert!(
        !fixture.runtime.memory().has_pending_delta(),
        "{name} published a failed write"
    );
}

fn protect_output(fixture: &mut Fixture, writable: bool) {
    fixture
        .runtime
        .memory_mut()
        .protect(
            fixture.owner,
            GuestRange::new(OUTPUT, 4096).unwrap(),
            PageProtection {
                read: true,
                write: writable,
                execute: false,
            },
        )
        .unwrap();
    fixture.ack();
}

#[test]
fn tls_fls_and_last_error_are_thread_local_through_windows_calls() {
    let mut f = fixture(&[
        "TlsAlloc",
        "TlsSetValue",
        "TlsGetValue",
        "FlsAlloc",
        "FlsSetValue",
        "FlsGetValue",
        "GetLastError",
        "SetLastError",
        "GetCurrentThread",
        "GetCurrentThreadId",
    ]);
    let tls = value(&mut f, "TlsAlloc", &[]);
    let fls = value(&mut f, "FlsAlloc", &[0]);
    assert_ne!(tls, u32::MAX);
    assert_ne!(fls, u32::MAX);
    assert_eq!(value(&mut f, "GetCurrentThread", &[]), 0xffff_fffe);
    assert_eq!(value(&mut f, "GetCurrentThreadId", &[]), THREAD.0);
    set_error(&mut f, 87);
    for name in ["TlsSetValue", "FlsSetValue"] {
        let index = if name == "TlsSetValue" { tls } else { fls };
        assert_eq!(value(&mut f, name, &[index, 0x2345_6789]), 1);
        assert_eq!(last_error(&mut f), 87);
    }

    let second = RuntimeThreadId(THREAD.0 + 1);
    f.runtime.register_thread(second, 122).unwrap();
    f.runtime.select_thread(second).unwrap();
    assert_eq!(last_error(&mut f), 122);
    assert_eq!(value(&mut f, "GetCurrentThread", &[]), 0xffff_fffe);
    assert_eq!(value(&mut f, "GetCurrentThreadId", &[]), second.0);
    for (name, index) in [("TlsGetValue", tls), ("FlsGetValue", fls)] {
        assert_eq!(value(&mut f, name, &[index]), 0);
        assert_eq!(last_error(&mut f), 0);
    }
    assert_eq!(value(&mut f, "TlsSetValue", &[tls, 0x8765_4321]), 1);
    assert_eq!(value(&mut f, "FlsSetValue", &[fls, 0x8765_4321]), 1);
    set_error(&mut f, 0xabcdef);
    f.runtime.select_thread(THREAD).unwrap();
    assert_eq!(last_error(&mut f), 87);
    for (name, index) in [("TlsGetValue", tls), ("FlsGetValue", fls)] {
        assert_eq!(value(&mut f, name, &[index]), 0x2345_6789);
        assert_eq!(last_error(&mut f), 0);
        assert_eq!(value(&mut f, name, &[u32::MAX]), 0);
        assert_eq!(last_error(&mut f), 87);
    }
    assert_eq!(value(&mut f, "FlsSetValue", &[u32::MAX, 1]), 0);
    assert_eq!(last_error(&mut f), 87);
    assert_eq!(value(&mut f, "FlsGetValue", &[fls]), 0x2345_6789);
    f.runtime.select_thread(second).unwrap();
    assert_eq!(last_error(&mut f), 0xabcdef);
    assert_eq!(value(&mut f, "TlsGetValue", &[tls]), 0x8765_4321);
    assert_eq!(value(&mut f, "FlsGetValue", &[fls]), 0x8765_4321);
}

#[test]
fn fls_callback_validation_capacity_and_free_preserve_live_values() {
    let mut f = fixture(&[
        "FlsAlloc",
        "FlsSetValue",
        "FlsGetValue",
        "FlsFree",
        "GetLastError",
        "SetLastError",
    ]);
    fault_preserves_outputs(&mut f, "FlsAlloc", &[OUTPUT.0]); // mapped, non-executable callback
    let callback = value(&mut f, "FlsAlloc", &[RETURN.0]);
    assert_ne!(callback, u32::MAX);
    assert_eq!(value(&mut f, "FlsSetValue", &[callback, 42]), 1);
    fault_preserves_outputs(&mut f, "FlsFree", &[callback]); // reached callback is not silently skipped
    assert_eq!(value(&mut f, "FlsGetValue", &[callback]), 42);
    let reusable = value(&mut f, "FlsAlloc", &[0]);
    // Exercise the Windows-visible finite capacity rather than private slot maps or counters.
    let mut full = false;
    for _ in 0..1024 {
        if value(&mut f, "FlsAlloc", &[0]) == u32::MAX {
            full = true;
            break;
        }
    }
    assert!(full, "FLS must fail at its bounded capacity");
    assert_eq!(last_error(&mut f), 8);
    assert_eq!(value(&mut f, "FlsGetValue", &[callback]), 42);
    assert_eq!(value(&mut f, "FlsFree", &[reusable]), 1);
    let recovered = value(&mut f, "FlsAlloc", &[0]);
    assert_ne!(recovered, u32::MAX);
    assert_eq!(value(&mut f, "FlsGetValue", &[recovered]), 0);
    assert_eq!(value(&mut f, "FlsFree", &[u32::MAX]), 0);
    assert_eq!(last_error(&mut f), 87);
}

#[test]
fn fixed_environment_queries_preserve_windows_last_error() {
    let mut f = fixture(&[
        "GetACP",
        "GetSystemDefaultLangID",
        "GetCurrentProcessId",
        "GetCurrentThreadId",
        "IsProcessorFeaturePresent",
        "GetLastError",
        "SetLastError",
    ]);
    set_error(&mut f, LAST_ERROR);
    for (name, args, expected) in [
        ("GetACP", vec![], 932),
        ("GetSystemDefaultLangID", vec![], 0x411),
        ("GetCurrentProcessId", vec![], 4096),
        ("GetCurrentThreadId", vec![], THREAD.0),
        ("IsProcessorFeaturePresent", vec![10], 1),
        ("IsProcessorFeaturePresent", vec![6], 0),
        ("IsProcessorFeaturePresent", vec![7], 0),
        ("IsProcessorFeaturePresent", vec![u32::MAX], 0),
    ] {
        assert_eq!(value(&mut f, name, &args), expected);
        assert_eq!(last_error(&mut f), LAST_ERROR);
    }
}

#[test]
fn affinity_validates_thread_handles_and_the_single_processor_mask() {
    let mut f = fixture(&[
        "SetThreadAffinityMask",
        "CreateEventA",
        "CloseHandle",
        "GetLastError",
        "SetLastError",
    ]);
    for mask in [0, 2, 3, 4] {
        assert_eq!(
            value(&mut f, "SetThreadAffinityMask", &[0xffff_fffe, mask]),
            0
        );
        assert_eq!(last_error(&mut f), 87);
    }
    assert_eq!(value(&mut f, "SetThreadAffinityMask", &[0xffff_fffe, 1]), 1);
    assert_eq!(last_error(&mut f), 87);
    let real = f
        .runtime
        .register_thread(RuntimeThreadId(THREAD.0 + 1), 0)
        .unwrap()
        .0;
    assert_eq!(value(&mut f, "SetThreadAffinityMask", &[real, 1]), 1);
    let event = value(&mut f, "CreateEventA", &[0, 0, 0, 0]);
    for handle in [0, event] {
        assert_eq!(value(&mut f, "SetThreadAffinityMask", &[handle, 1]), 0);
        assert_eq!(last_error(&mut f), 6);
    }
    assert_eq!(value(&mut f, "CloseHandle", &[real]), 1);
    assert_eq!(value(&mut f, "SetThreadAffinityMask", &[real, 1]), 0);
    assert_eq!(last_error(&mut f), 6);
}

#[test]
fn system_info_and_memory_status_bound_every_output_before_writing() {
    let mut f = fixture(&["GetSystemInfo", "GlobalMemoryStatus"]);
    f.write(OUTPUT, &[0xa5; 64]);
    assert_eq!(f.call(KERNEL, "GetSystemInfo", &[OUTPUT.0]).eax, None);
    let words: Vec<_> = f
        .read(OUTPUT, 36)
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect();
    assert_eq!(words, [0, 4096, 0x10000, 0x7ffe_ffff, 1, 1, 586, 65536, 6]);
    assert_eq!(f.read(GuestAddress(OUTPUT.0 + 36), 28), [0xa5; 28]);
    f.write(OUTPUT, &[0xa5; 64]);
    let limits = f.runtime.memory().limits();
    let stats = f.runtime.memory().stats();
    assert_eq!(f.call(KERNEL, "GlobalMemoryStatus", &[OUTPUT.0]).eax, None);
    let words: Vec<_> = f
        .read(OUTPUT, 32)
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect();
    let total = (limits.max_physical_pages as u64 * 4096).min(u64::from(u32::MAX)) as u32;
    let available = ((limits.max_physical_pages - stats.physical_pages) as u64 * 4096)
        .min(u64::from(total)) as u32;
    assert_eq!(
        words[..6],
        [
            32,
            ((u64::from(total - available) * 100) / u64::from(total)) as u32,
            total,
            available,
            total,
            available
        ]
    );
    assert_eq!(words[6], 0x7ffe_0000);
    assert!(words[7] <= words[6]);
    assert_eq!(f.read(GuestAddress(OUTPUT.0 + 32), 32), [0xa5; 32]);
    for name in ["GetSystemInfo", "GlobalMemoryStatus"] {
        for output in [0, CROSS_PAGE.0] {
            fault_preserves_outputs(&mut f, name, &[output]);
        }
    }
    protect_output(&mut f, false);
    for name in ["GetSystemInfo", "GlobalMemoryStatus"] {
        fault_preserves_outputs(&mut f, name, &[OUTPUT.0]);
    }
    protect_output(&mut f, true);
    f.call(KERNEL, "GetSystemInfo", &[OUTPUT.0]);
}

#[test]
fn version_layout_preserves_unused_bytes_and_rejects_invalid_outputs() {
    let mut f = fixture(&["GetVersionExA"]);
    for size in [148_u32, 156] {
        let mut before = vec![0xa5; size as usize + 4];
        before[..4].copy_from_slice(&size.to_le_bytes());
        f.write(OUTPUT, &before);
        assert_eq!(value(&mut f, "GetVersionExA", &[OUTPUT.0]), 1);
        let after = f.read(OUTPUT, before.len());
        let words: Vec<_> = after[..20]
            .chunks_exact(4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
            .collect();
        assert_eq!(words, [size, 10, 0, 19045, 2]);
        assert_eq!(after[20], 0);
        assert_eq!(&after[21..148], &[0xa5; 127]);
        if size == 156 {
            assert_eq!(&after[148..156], &[0, 0, 0, 0, 0, 0, 1, 0]);
        }
        assert_eq!(&after[size as usize..], &[0xa5; 4]);
        assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    }
    f.words(OUTPUT, &[147]);
    let before = f.read(OUTPUT, 160);
    assert_eq!(value(&mut f, "GetVersionExA", &[OUTPUT.0]), 0);
    assert_eq!(f.runtime.current_last_error().unwrap(), 122);
    assert_eq!(f.read(OUTPUT, 160), before);
    f.words(CROSS_PAGE, &[148]);
    for output in [0, CROSS_PAGE.0] {
        fault_preserves_outputs(&mut f, "GetVersionExA", &[output]);
    }
    f.words(OUTPUT, &[148]);
    protect_output(&mut f, false);
    fault_preserves_outputs(&mut f, "GetVersionExA", &[OUTPUT.0]);
}

#[test]
fn cp_info_accepts_windows_932_and_checks_writable_guest_range() {
    let mut f = fixture(&["GetCPInfo"]);
    let expected = [
        2, 0, 0, 0, b'?', 0, 0x81, 0x9f, 0xe0, 0xfc, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    for code_page in [0, 932] {
        f.write(OUTPUT, &[0xa5; 32]);
        assert_eq!(value(&mut f, "GetCPInfo", &[code_page, OUTPUT.0]), 1);
        assert_eq!(f.read(OUTPUT, 18), expected);
        assert_eq!(f.read(GuestAddress(OUTPUT.0 + 18), 14), [0xa5; 14]);
    }
    for args in [[65001, OUTPUT.0], [932, CROSS_PAGE.0], [932, 0]] {
        fault_preserves_outputs(&mut f, "GetCPInfo", &args);
    }
    protect_output(&mut f, false);
    fault_preserves_outputs(&mut f, "GetCPInfo", &[932, OUTPUT.0]);
}

#[test]
fn drive_root_query_uses_the_guest_mount_and_requires_terminated_input() {
    let mut f = fixture(&["GetDriveTypeA"]);
    assert_eq!(value(&mut f, "GetDriveTypeA", &[0]), 3);
    for (path, expected) in [
        (b"C:\\\0".as_slice(), 3),
        (b"c:/\0", 3),
        (b"D:\\\0", 1),
        (b"C:\\Vine\0", 1),
        (b".\0", 1),
        (b"\0", 1),
    ] {
        f.write(OUTPUT, path);
        assert_eq!(value(&mut f, "GetDriveTypeA", &[OUTPUT.0]), expected);
        assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    }
    f.write(OUTPUT, &[b'A'; 260]);
    fault_preserves_outputs(&mut f, "GetDriveTypeA", &[OUTPUT.0]);
}

#[test]
fn exception_filter_registration_returns_the_previous_pointer_without_dereferencing_it() {
    let mut f = fixture(&["SetUnhandledExceptionFilter"]);
    for (filter, previous) in [
        (OUTPUT.0, 0),
        (u32::MAX, OUTPUT.0),
        (0, u32::MAX),
        (0x3000_0000, 0),
        (RETURN.0, 0x3000_0000),
    ] {
        assert_eq!(
            value(&mut f, "SetUnhandledExceptionFilter", &[filter]),
            previous
        );
        assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    }
}

#[test]
fn pointer_encoding_round_trips_guest_values_without_fixing_cookie_tokens() {
    let mut f = fixture(&["EncodePointer", "DecodePointer"]);
    let mut tokens = Vec::new();
    for pointer in [0, RETURN.0, RETURN.0 + 1, OUTPUT.0, u32::MAX] {
        let encoded = value(&mut f, "EncodePointer", &[pointer]);
        assert_eq!(value(&mut f, "DecodePointer", &[encoded]), pointer);
        assert!(!tokens.contains(&encoded));
        tokens.push(encoded);
        assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    }
    for token in [0, 0xdead_beef, 0xa5a5_5a5a] {
        let decoded = value(&mut f, "DecodePointer", &[token]);
        assert_eq!(value(&mut f, "EncodePointer", &[decoded]), token);
    }
}

#[test]
fn standard_handles_have_stable_guest_identity_and_file_type_errors() {
    let mut f = fixture(&["GetStdHandle", "GetFileType", "SetHandleCount"]);
    let mut handles = Vec::new();
    for (selector, file_type) in [(0xffff_fff6, 2), (0xffff_fff5, 3), (0xffff_fff4, 3)] {
        let handle = value(&mut f, "GetStdHandle", &[selector]);
        assert!(![0, u32::MAX].contains(&handle));
        assert!(!handles.contains(&handle));
        assert_eq!(value(&mut f, "GetStdHandle", &[selector]), handle);
        assert_eq!(value(&mut f, "GetFileType", &[handle]), file_type);
        assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
        handles.push(handle);
    }
    for count in [0, 32, u32::MAX] {
        assert_eq!(value(&mut f, "SetHandleCount", &[count]), count);
    }
    assert_eq!(value(&mut f, "GetStdHandle", &[0xffff_fff3]), u32::MAX);
    assert_eq!(f.runtime.current_last_error().unwrap(), 87);
    for handle in [0, u32::MAX, 0xdead_beef] {
        assert_eq!(value(&mut f, "GetFileType", &[handle]), 0);
        assert_eq!(f.runtime.current_last_error().unwrap(), 6);
    }
    assert_eq!(value(&mut f, "GetFileType", &[handles[0]]), 2);
    assert_eq!(f.runtime.current_last_error().unwrap(), 6);
}

#[test]
fn command_line_and_environment_copy_follow_guest_acquire_release_lifetime() {
    let mut f = fixture(&[
        "GetCommandLineA",
        "GetEnvironmentStringsW",
        "FreeEnvironmentStringsW",
    ]);
    f.runtime
        .install_process_environment("fixture.exe")
        .unwrap();
    f.ack();
    let command_line = value(&mut f, "GetCommandLineA", &[]);
    assert_eq!(f.read(GuestAddress(command_line), 12), b"fixture.exe\0");
    assert_eq!(value(&mut f, "GetCommandLineA", &[]), command_line);
    let environment = value(&mut f, "GetEnvironmentStringsW", &[]);
    assert_eq!(f.read(GuestAddress(environment), 4), [0, 0, 0, 0]);
    fault_preserves_outputs(&mut f, "GetEnvironmentStringsW", &[]);
    fault_preserves_outputs(&mut f, "FreeEnvironmentStringsW", &[command_line]);
    assert_eq!(value(&mut f, "FreeEnvironmentStringsW", &[environment]), 1);
    fault_preserves_outputs(&mut f, "FreeEnvironmentStringsW", &[environment]);
    let reacquired = value(&mut f, "GetEnvironmentStringsW", &[]);
    assert_eq!(f.read(GuestAddress(reacquired), 4), [0, 0, 0, 0]);
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
}

#[test]
fn module_filename_truncation_and_bad_buffers_preserve_the_guest_path() {
    let mut f = fixture(&["GetModuleFileNameA"]);
    const PATH: &[u8] = b"C:\\Vine\\fixture.exe\0";
    f.write(OUTPUT, &[0xa5; 64]);
    assert_eq!(
        value(&mut f, "GetModuleFileNameA", &[0, OUTPUT.0, 64]),
        PATH.len() as u32 - 1
    );
    assert_eq!(f.read(OUTPUT, PATH.len() + 1), [PATH, &[0xa5]].concat());
    assert_eq!(f.runtime.current_last_error().unwrap(), 0);
    for capacity in [PATH.len() as u32, PATH.len() as u32 - 1, 1] {
        f.write(OUTPUT, &[0xa5; 64]);
        let result = value(&mut f, "GetModuleFileNameA", &[0, OUTPUT.0, capacity]);
        if capacity == PATH.len() as u32 {
            assert_eq!(result, capacity - 1);
            assert_eq!(f.runtime.current_last_error().unwrap(), 0);
        } else {
            assert_eq!(result, capacity);
            assert_eq!(f.runtime.current_last_error().unwrap(), 122);
        }
        assert_eq!(
            f.read(OUTPUT, capacity as usize),
            [&PATH[..capacity as usize - 1], &[0]].concat()
        );
        assert_eq!(f.read(GuestAddress(OUTPUT.0 + capacity), 1), [0xa5]);
    }
    assert_eq!(value(&mut f, "GetModuleFileNameA", &[0, 0, 0]), 0);
    assert_eq!(f.runtime.current_last_error().unwrap(), 122);
    assert_eq!(value(&mut f, "GetModuleFileNameA", &[0, 0, 1]), 0);
    assert_eq!(f.runtime.current_last_error().unwrap(), 87);
    for args in [
        [0, OUTPUT.0, 32769],
        [0xdead_beef, OUTPUT.0, 64],
        [0, CROSS_PAGE.0, 64],
    ] {
        fault_preserves_outputs(&mut f, "GetModuleFileNameA", &args);
    }
}

#[test]
fn private_heap_calls_preserve_live_bytes_on_failure_and_return_exact_guest_size() {
    let mut f = fixture(&[
        "HeapCreate",
        "HeapAlloc",
        "HeapReAlloc",
        "HeapFree",
        "HeapSize",
    ]);
    let heap = value(&mut f, "HeapCreate", &[0, 4096, 4096]);
    let foreign_heap = value(&mut f, "HeapCreate", &[1, 4096, 4096]);
    assert_ne!(heap, 0);
    assert_ne!(foreign_heap, 0);
    assert_ne!(heap, foreign_heap);
    fault_preserves_outputs(&mut f, "HeapCreate", &[4, 4096, 0]);
    let allocation = value(&mut f, "HeapAlloc", &[heap, 9, 129]);
    assert_ne!(allocation, 0);
    assert_eq!(allocation % 8, 0);
    assert_eq!(f.read(GuestAddress(allocation), 129), [0; 129]);
    assert_eq!(value(&mut f, "HeapSize", &[heap, 0, allocation]), 129);
    let foreign = value(&mut f, "HeapAlloc", &[foreign_heap, 8, 17]);
    assert_ne!(foreign, 0);
    f.write(
        GuestAddress(allocation),
        &(0..129).map(|index| index as u8).collect::<Vec<_>>(),
    );
    let before = f.read(GuestAddress(allocation), 129);
    for pointer in [0, allocation + 1, allocation + 8, foreign] {
        assert_eq!(value(&mut f, "HeapSize", &[heap, 0, pointer]), u32::MAX);
        assert_eq!(f.runtime.current_last_error().unwrap(), 87);
        if pointer != 0 {
            assert_eq!(value(&mut f, "HeapFree", &[heap, 0, pointer]), 0);
        }
        assert_eq!(value(&mut f, "HeapReAlloc", &[heap, 0, pointer, 200]), 0);
        assert_eq!(f.read(GuestAddress(allocation), 129), before);
        assert_eq!(value(&mut f, "HeapSize", &[heap, 1, allocation]), 129);
    }
    for (name, args) in [
        ("HeapAlloc", vec![heap, 4, 16]),
        ("HeapFree", vec![heap, 4, allocation]),
        ("HeapSize", vec![heap, 8, allocation]),
        ("HeapReAlloc", vec![heap, 4, allocation, 200]),
        ("HeapSize", vec![0xdead_beef, 0, allocation]),
    ] {
        fault_preserves_outputs(&mut f, name, &args);
        assert_eq!(f.read(GuestAddress(allocation), 129), before);
    }
    f.runtime.set_current_last_error(LAST_ERROR).unwrap();
    for _ in 0..2 {
        assert_eq!(
            value(&mut f, "HeapReAlloc", &[heap, 0, allocation, 0x6000]),
            0
        );
        assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
        assert_eq!(f.read(GuestAddress(allocation), 129), before);
        assert_eq!(value(&mut f, "HeapSize", &[heap, 0, allocation]), 129);
    }
    assert_eq!(value(&mut f, "HeapAlloc", &[heap, 0, 0x7fff_fff8]), 0);
    let grown = value(&mut f, "HeapReAlloc", &[heap, 8, allocation, 256]);
    assert_ne!(grown, 0);
    assert_eq!(f.read(GuestAddress(grown), 129), before);
    assert_eq!(f.read(GuestAddress(grown + 129), 127), [0; 127]);
    assert_eq!(value(&mut f, "HeapSize", &[heap, 0, grown]), 256);
    let shrunk = value(&mut f, "HeapReAlloc", &[heap, 0, grown, 64]);
    assert_ne!(shrunk, 0);
    assert_eq!(f.read(GuestAddress(shrunk), 64), before[..64]);
    assert_eq!(value(&mut f, "HeapSize", &[heap, 0, shrunk]), 64);
    assert_eq!(value(&mut f, "HeapFree", &[heap, 0, 0]), 1);
    assert_eq!(value(&mut f, "HeapFree", &[heap, 1, shrunk]), 1);
    assert_eq!(value(&mut f, "HeapSize", &[heap, 0, shrunk]), u32::MAX);
    assert_eq!(value(&mut f, "HeapFree", &[heap, 0, shrunk]), 0);
    assert_eq!(f.runtime.current_last_error().unwrap(), 87);
    assert_eq!(value(&mut f, "HeapSize", &[foreign_heap, 0, foreign]), 17);
    let recovered = value(&mut f, "HeapAlloc", &[heap, 8, 64]);
    assert_ne!(recovered, 0);
    assert_eq!(f.read(GuestAddress(recovered), 64), [0; 64]);
}

#[test]
fn windows_932_conversion_preserves_query_partial_output_and_default_character_contracts() {
    let mut f = fixture(&["WideCharToMultiByte", "MultiByteToWideChar"]);
    let wide = OUTPUT.0;
    let ansi = OUTPUT.0 + 0x100;
    let destination = OUTPUT.0 + 0x200;
    let used_default = OUTPUT.0 + 0x300;
    const UTF16: &[u8] = &[0x41, 0, 0x42, 0x30, 0, 0];
    const CP932: &[u8] = &[0x41, 0x82, 0xa0, 0];
    f.write(GuestAddress(wide), UTF16);
    f.write(GuestAddress(ansi), CP932);
    f.write(GuestAddress(destination), &[0xa5; 16]);
    for code_page in [0, 932] {
        assert_eq!(
            value(
                &mut f,
                "WideCharToMultiByte",
                &[code_page, 0, wide, 3, 0, 0, 0, used_default]
            ),
            4
        );
        assert_eq!(f.read(GuestAddress(used_default), 4), [0; 4]);
        assert_eq!(
            value(
                &mut f,
                "WideCharToMultiByte",
                &[code_page, 0, wide, 3, destination, 4, 0, used_default]
            ),
            4
        );
        assert_eq!(f.read(GuestAddress(destination), 4), CP932);
        assert_eq!(
            value(
                &mut f,
                "MultiByteToWideChar",
                &[code_page, 0, ansi, u32::MAX, 0, 0]
            ),
            3
        );
        assert_eq!(
            value(
                &mut f,
                "MultiByteToWideChar",
                &[code_page, 1, ansi, 4, destination, 3]
            ),
            3
        );
        assert_eq!(f.read(GuestAddress(destination), 6), UTF16);
        assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    }
    f.write(GuestAddress(destination), &[0xa5; 8]);
    assert_eq!(
        value(
            &mut f,
            "WideCharToMultiByte",
            &[932, 0, wide, 3, destination, 1, 0, used_default]
        ),
        0
    );
    assert_eq!(f.runtime.current_last_error().unwrap(), 122);
    assert_eq!(f.read(GuestAddress(destination), 2), [0x41, 0xa5]);
    assert_eq!(f.read(GuestAddress(used_default), 4), [0; 4]);
    f.write(GuestAddress(destination), &[0xa5; 8]);
    assert_eq!(
        value(
            &mut f,
            "MultiByteToWideChar",
            &[932, 1, ansi, 4, destination, 1]
        ),
        0
    );
    assert_eq!(f.runtime.current_last_error().unwrap(), 122);
    assert_eq!(f.read(GuestAddress(destination), 4), [0x41, 0, 0xa5, 0xa5]);
    f.write(GuestAddress(wide), &[0x3d, 0xd8, 0, 0xde, 0, 0]);
    assert_eq!(
        value(
            &mut f,
            "WideCharToMultiByte",
            &[932, 0, wide, 3, destination, 8, 0, used_default]
        ),
        3
    );
    assert_eq!(f.read(GuestAddress(destination), 3), [b'?', b'?', 0]);
    assert_eq!(f.read(GuestAddress(used_default), 4), 1_u32.to_le_bytes());
    f.write(GuestAddress(wide + 1), UTF16);
    assert_eq!(
        value(
            &mut f,
            "WideCharToMultiByte",
            &[932, 0, wide + 1, 3, destination, 4, 0, used_default]
        ),
        4
    );
    assert_eq!(f.read(GuestAddress(destination), 4), CP932);
    f.write(GuestAddress(wide), UTF16);
    for (name, args) in [
        ("WideCharToMultiByte", vec![932, 0, wide, 0, 0, 0, 0, 0]),
        ("WideCharToMultiByte", vec![932, 0, wide, 3, wide, 4, 0, 0]),
        ("MultiByteToWideChar", vec![932, 0, ansi, 0, 0, 0]),
        ("MultiByteToWideChar", vec![932, 0, ansi, 4, destination, 0]),
        ("MultiByteToWideChar", vec![932, 0, ansi, 4, ansi + 2, 3]),
    ] {
        let before = f.read(OUTPUT, 0x1000);
        assert_eq!(value(&mut f, name, &args), 0);
        assert_eq!(f.runtime.current_last_error().unwrap(), 87);
        assert_eq!(f.read(OUTPUT, 0x1000), before);
    }
    for (name, args) in [
        (
            "WideCharToMultiByte",
            vec![65001, 0, wide, 3, destination, 4, 0, used_default],
        ),
        (
            "WideCharToMultiByte",
            vec![932, 0, 0xffff_fffd, 3, destination, 4, 0, used_default],
        ),
        (
            "WideCharToMultiByte",
            vec![932, 0, BASE.0 + 0x3000, 3, destination, 4, 0, used_default],
        ),
        (
            "WideCharToMultiByte",
            vec![932, 0, wide, 3, destination, 4, 0, BASE.0 + 0x3000],
        ),
        (
            "MultiByteToWideChar",
            vec![65001, 0, ansi, 4, destination, 3],
        ),
        ("MultiByteToWideChar", vec![932, 8, ansi, 4, destination, 3]),
        (
            "MultiByteToWideChar",
            vec![932, 0, ansi, 4, BASE.0 + 0x2ffc, 3],
        ),
    ] {
        fault_preserves_outputs(&mut f, name, &args);
    }
}

#[test]
fn windows_932_single_byte_mapping_keeps_halfwidth_and_private_use_units() {
    let mut f = fixture(&["MultiByteToWideChar"]);
    let mut source: Vec<_> = (0_u16..=255).map(|byte| byte as u8).collect();
    let mut units: Vec<_> = (0_u16..=255).collect();
    for byte in [0_u8].into_iter().chain(0x81..=0x9f).chain(0xe0..=0xfc) {
        source[byte as usize] = b' ';
        units[byte as usize] = u16::from(b' ');
    }
    units[0xa0] = 0xf8f0;
    for (offset, unit) in units[0xa1..=0xdf].iter_mut().enumerate() {
        *unit = 0xff61 + offset as u16;
    }
    units[0xfd..].copy_from_slice(&[0xf8f1, 0xf8f2, 0xf8f3]);
    f.write(OUTPUT, &source);
    let output = OUTPUT.0 + 0x200;
    assert_eq!(
        value(
            &mut f,
            "MultiByteToWideChar",
            &[932, 1, OUTPUT.0, 256, 0, 0]
        ),
        256
    );
    assert_eq!(
        value(
            &mut f,
            "MultiByteToWideChar",
            &[932, 1, OUTPUT.0, 256, output, 256]
        ),
        256
    );
    assert_eq!(
        f.read(GuestAddress(output), 512),
        units
            .iter()
            .flat_map(|unit| unit.to_le_bytes())
            .collect::<Vec<_>>()
    );
}

fn section_bytes(spin: u32, owner: u32, recursion: u32) -> Vec<u8> {
    [
        u32::MAX,
        if recursion == 0 {
            u32::MAX
        } else {
            u32::MAX - 1
        },
        recursion,
        owner,
        0,
        spin,
    ]
    .iter()
    .flat_map(|word| word.to_le_bytes())
    .collect()
}

#[test]
fn critical_section_guest_layout_tracks_recursion_and_transfers_a_blocked_thread() {
    let mut f = fixture(&[
        "InitializeCriticalSectionAndSpinCount",
        "EnterCriticalSection",
        "LeaveCriticalSection",
        "DeleteCriticalSection",
    ]);
    assert_eq!(
        value(
            &mut f,
            "InitializeCriticalSectionAndSpinCount",
            &[OUTPUT.0, 4000]
        ),
        1
    );
    assert_eq!(f.read(OUTPUT, 24), section_bytes(4000, 0, 0));
    for recursion in [1, 2] {
        assert_eq!(
            f.call(KERNEL, "EnterCriticalSection", &[OUTPUT.0]).eax,
            None
        );
        assert_eq!(f.read(OUTPUT, 24), section_bytes(4000, THREAD.0, recursion));
    }
    assert_eq!(
        f.call(KERNEL, "LeaveCriticalSection", &[OUTPUT.0]).eax,
        None
    );
    let second = RuntimeThreadId(THREAD.0 + 1);
    f.runtime.register_thread(second, 0x5678).unwrap();
    f.runtime.select_thread(second).unwrap();
    let contended = f.call(KERNEL, "EnterCriticalSection", &[OUTPUT.0]);
    assert_eq!(contended.eax, None);
    assert_eq!(
        contended.execution_boundary,
        RuntimeApiBoundaryDisposition::BlockCurrentThread
    );
    assert_eq!(
        f.runtime.scheduler_state().unwrap(),
        SchedulerThreadState::WaitingCriticalSection { address: OUTPUT.0 }
    );
    assert_eq!(f.read(OUTPUT, 24), section_bytes(4000, THREAD.0, 1));
    assert_eq!(f.runtime.current_last_error().unwrap(), 0x5678);
    f.runtime.select_thread(THREAD).unwrap();
    f.call(KERNEL, "LeaveCriticalSection", &[OUTPUT.0]);
    assert_eq!(f.read(OUTPUT, 24), section_bytes(4000, second.0, 1));
    assert_eq!(
        f.runtime.scheduler_state_for_thread(second).unwrap(),
        SchedulerThreadState::Runnable
    );
    f.runtime.select_thread(second).unwrap();
    f.call(KERNEL, "LeaveCriticalSection", &[OUTPUT.0]);
    assert_eq!(f.read(OUTPUT, 24), section_bytes(4000, 0, 0));
    assert_eq!(
        f.call(KERNEL, "DeleteCriticalSection", &[OUTPUT.0]).eax,
        None
    );
    assert_eq!(f.read(OUTPUT, 24), [0; 24]);
    fault_preserves_outputs(&mut f, "EnterCriticalSection", &[OUTPUT.0]);
}

#[test]
fn critical_section_validates_a_read_only_tail_before_state_change_and_can_retry() {
    let names = [
        "InitializeCriticalSectionAndSpinCount",
        "EnterCriticalSection",
        "LeaveCriticalSection",
        "DeleteCriticalSection",
    ];
    let section = GuestAddress(OUTPUT.0 - 16);
    for (operation, name) in names.into_iter().enumerate() {
        let mut f = fixture(&names);
        if operation != 0 {
            value(&mut f, names[0], &[section.0, 4000]);
        }
        if operation == 2 {
            f.call(KERNEL, names[1], &[section.0]);
        }
        let before = f.read(section, 24);
        protect_output(&mut f, false);
        let args = if operation == 0 {
            vec![section.0, 4000]
        } else {
            vec![section.0]
        };
        fault_preserves_outputs(&mut f, name, &args);
        assert_eq!(f.read(section, 24), before);
        protect_output(&mut f, true);
        f.call(KERNEL, name, &args);
        assert_eq!(
            f.read(section, 24),
            match operation {
                0 | 2 => section_bytes(4000, 0, 0),
                1 => section_bytes(4000, THREAD.0, 1),
                3 => vec![0; 24],
                _ => unreachable!(),
            }
        );
        assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    }
    let mut f = fixture(&["InitializeCriticalSection"]);
    for address in [0, OUTPUT.0 + 1, CROSS_PAGE.0] {
        fault_preserves_outputs(&mut f, "InitializeCriticalSection", &[address]);
    }
    assert_eq!(
        f.call(KERNEL, "InitializeCriticalSection", &[OUTPUT.0]).eax,
        None
    );
    assert_eq!(f.read(OUTPUT, 24), section_bytes(0x0200_07d0, 0, 0));
    fault_preserves_outputs(&mut f, "InitializeCriticalSection", &[OUTPUT.0]);
}

#[test]
fn interlocked_updates_wrap_and_reject_invalid_guest_targets_atomically() {
    let mut f = fixture(&["InterlockedIncrement", "InterlockedDecrement"]);
    for (name, initial, expected) in [
        ("InterlockedIncrement", 0, 1),
        ("InterlockedIncrement", i32::MAX as u32, i32::MIN as u32),
        ("InterlockedIncrement", u32::MAX, 0),
        ("InterlockedDecrement", 0, u32::MAX),
        ("InterlockedDecrement", 1, 0),
        ("InterlockedDecrement", i32::MIN as u32, i32::MAX as u32),
    ] {
        f.words(OUTPUT, &[initial]);
        assert_eq!(value(&mut f, name, &[OUTPUT.0]), expected);
        assert_eq!(f.read(OUTPUT, 4), expected.to_le_bytes());
        assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    }
    for name in ["InterlockedIncrement", "InterlockedDecrement"] {
        for address in [0, OUTPUT.0 + 1, BASE.0 + 0x3000, 0xffff_fffc] {
            fault_preserves_outputs(&mut f, name, &[address]);
        }
    }
    protect_output(&mut f, false);
    for name in ["InterlockedIncrement", "InterlockedDecrement"] {
        fault_preserves_outputs(&mut f, name, &[OUTPUT.0]);
    }
}

#[test]
fn named_mutex_copies_the_guest_name_and_duplicate_handles_share_lifetime() {
    let mut f = fixture(&["CreateMutexA", "CloseHandle"]);
    const NAME: &[u8] = b"Vine public mutex\0";
    f.write(OUTPUT, NAME);
    let first = value(&mut f, "CreateMutexA", &[0, 1, OUTPUT.0]);
    assert_ne!(first, 0);
    assert_eq!(f.runtime.current_last_error().unwrap(), 0);
    f.write(OUTPUT, b"Changed after return\0");
    let alias_name = GuestAddress(OUTPUT.0 + 0x100);
    f.write(alias_name, NAME);
    let duplicate = value(&mut f, "CreateMutexA", &[0, 1, alias_name.0]);
    assert_ne!(first, duplicate);
    assert_eq!(f.runtime.current_last_error().unwrap(), 183);
    fault_preserves_outputs(&mut f, "CreateMutexA", &[4, 1, alias_name.0]);
    f.write(OUTPUT, &[b'X'; 260]);
    fault_preserves_outputs(&mut f, "CreateMutexA", &[0, 0, OUTPUT.0]);
    assert_eq!(value(&mut f, "CloseHandle", &[first]), 1);
    assert_eq!(value(&mut f, "CloseHandle", &[duplicate]), 1);
    assert_ne!(value(&mut f, "CreateMutexA", &[0, 0, alias_name.0]), 0);
    assert_eq!(f.runtime.current_last_error().unwrap(), 0);
    let unnamed = value(&mut f, "CreateMutexA", &[0, 0, 0]);
    assert_ne!(unnamed, 0);
    assert_ne!(unnamed, duplicate);
    assert_eq!(f.runtime.current_last_error().unwrap(), 0);
}

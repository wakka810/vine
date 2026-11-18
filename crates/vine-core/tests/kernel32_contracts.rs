mod support;

use support::{BASE, Fixture, LAST_ERROR, OUTPUT, RETURN, THREAD};
use vine_core::{
    DispatchOutcome, GuestAddress, GuestRange, PageProtection, RuntimeApiBoundaryDisposition,
    RuntimeThreadId, SchedulerThreadState,
};

const KERNEL: &str = "kernel32.dll";
const CROSS_PAGE: GuestAddress = GuestAddress(BASE.0 + 0x2ff0);

#[test]
fn global_atoms_copy_names_count_references_and_validate_before_mutation() {
    let mut f = fixture(&["GlobalAddAtomA", "GlobalDeleteAtom"]);
    f.write(OUTPUT, b"TabletProperty\0");
    let atom = value(&mut f, "GlobalAddAtomA", &[OUTPUT.0]);
    assert!((0xc000..=0xffff).contains(&atom));
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    f.write(OUTPUT, b"TABLETPROPERTY\0");
    assert_eq!(value(&mut f, "GlobalAddAtomA", &[OUTPUT.0]), atom);
    f.write(OUTPUT, b"DifferentName\0");
    let other = value(&mut f, "GlobalAddAtomA", &[OUTPUT.0]);
    assert_ne!(atom, other);
    for _ in 0..2 {
        assert_eq!(value(&mut f, "GlobalDeleteAtom", &[atom]), 0);
        assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    }
    assert_eq!(value(&mut f, "GlobalDeleteAtom", &[atom]), 0);
    assert_eq!(f.runtime.current_last_error().unwrap(), 87);
    for name in [b"#0012\0".as_slice(), b"#49151\0"] {
        f.write(OUTPUT, name);
        let expected = if name == b"#0012\0" { 12 } else { 49151 };
        assert_eq!(value(&mut f, "GlobalAddAtomA", &[OUTPUT.0]), expected);
        assert_eq!(value(&mut f, "GlobalAddAtomA", &[expected]), expected);
        assert_eq!(value(&mut f, "GlobalDeleteAtom", &[expected]), 0);
    }
    for (name, error) in [(b"\0".as_slice(), 123), (b"#0\0", 87), (b"#49152\0", 87)] {
        f.write(OUTPUT, name);
        assert_eq!(value(&mut f, "GlobalAddAtomA", &[OUTPUT.0]), 0);
        assert_eq!(f.runtime.current_last_error().unwrap(), error);
    }
    f.write(OUTPUT, &[vec![b'a'; 256], vec![0]].concat());
    assert_eq!(value(&mut f, "GlobalAddAtomA", &[OUTPUT.0]), 0);
    assert_eq!(f.runtime.current_last_error().unwrap(), 87);
    // The native A entry point limits the decoded Unicode name, not ANSI bytes.
    f.write(OUTPUT, &[[0x82, 0xa0].repeat(128), vec![0]].concat());
    assert_ne!(value(&mut f, "GlobalAddAtomA", &[OUTPUT.0]), 0);
    assert!(matches!(
        f.dispatch(KERNEL, "GlobalAddAtomA", &[u32::MAX]),
        DispatchOutcome::Fault(_)
    ));
    f.write(OUTPUT, b"DifferentName\0");
    assert_eq!(value(&mut f, "GlobalAddAtomA", &[OUTPUT.0]), other);
    for pointer in [0, 0xc000, 0xffff] {
        assert_eq!(value(&mut f, "GlobalAddAtomA", &[pointer]), 0);
        assert_eq!(f.runtime.current_last_error().unwrap(), 87);
    }
}

#[test]
fn heap_destroy_invalidates_private_handles_and_preserves_the_process_heap() {
    let mut f = fixture(&["HeapCreate", "HeapAlloc", "HeapDestroy", "GetProcessHeap"]);
    let heap = value(&mut f, "HeapCreate", &[0, 0, 0]);
    let address = value(&mut f, "HeapAlloc", &[heap, 0, 64]);
    assert_ne!(address, 0);
    assert_eq!(value(&mut f, "HeapDestroy", &[heap]), 1);
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    assert!(f.runtime.memory().read(GuestAddress(address), 1).is_err());
    assert_eq!(value(&mut f, "HeapDestroy", &[heap]), 0);
    assert_eq!(f.runtime.current_last_error().unwrap(), 87);
    let process = value(&mut f, "GetProcessHeap", &[]);
    assert_eq!(value(&mut f, "HeapDestroy", &[process]), 0);
    assert_ne!(value(&mut f, "HeapAlloc", &[process, 0, 64]), 0);
    assert_eq!(value(&mut f, "HeapDestroy", &[0]), 0);
}

#[test]
fn tls_free_preserves_error_on_success_and_rejects_double_release() {
    let mut f = fixture(&["TlsAlloc", "TlsSetValue", "TlsGetValue", "TlsFree"]);
    let index = value(&mut f, "TlsAlloc", &[]);
    assert_eq!(value(&mut f, "TlsSetValue", &[index, 0x12345678]), 1);
    assert_eq!(value(&mut f, "TlsFree", &[index]), 1);
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    assert_eq!(value(&mut f, "TlsFree", &[index]), 0);
    assert_eq!(f.runtime.current_last_error().unwrap(), 87);
    assert_eq!(value(&mut f, "TlsFree", &[u32::MAX]), 0);
    assert_eq!(f.runtime.current_last_error().unwrap(), 87);
    assert_eq!(value(&mut f, "TlsAlloc", &[]), index);
    assert_eq!(value(&mut f, "TlsGetValue", &[index]), 0);
    assert_eq!(f.runtime.current_last_error().unwrap(), 0);
}

#[test]
fn scalar_math_and_compare_exchange_preserve_signed_results_and_memory_authority() {
    let mut f = fixture(&["MulDiv", "InterlockedCompareExchange"]);
    for (a, b, d, expected) in [
        (1280, 1, 1, 1280),
        (1, 1, 2, 1),
        (-1, 1, 2, -1),
        (1, 1, -2, -1),
        (-1, 1, -2, 1),
        (3, 1, 2, 2),
        (-3, 1, 2, -2),
        (i32::MAX, 2, 2, i32::MAX),
        (i32::MIN, 1, 1, -1),
        (i32::MAX, 2, 1, -1),
        (1, 1, 0, -1),
    ] {
        assert_eq!(
            f.call(KERNEL, "MulDiv", &[a as u32, b as u32, d as u32])
                .eax,
            Some(expected as u32)
        );
    }
    f.words(OUTPUT, &[5, 0xaabbccdd]);
    assert_eq!(
        f.call(KERNEL, "InterlockedCompareExchange", &[OUTPUT.0, 9, 4])
            .eax,
        Some(5)
    );
    assert_eq!(f.read(OUTPUT, 4), 5_u32.to_le_bytes());
    assert_eq!(
        f.call(KERNEL, "InterlockedCompareExchange", &[OUTPUT.0, 9, 5])
            .eax,
        Some(5)
    );
    assert_eq!(
        f.read(OUTPUT, 8),
        [9_u32, 0xaabbccdd]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    fault_preserves_outputs(&mut f, "InterlockedCompareExchange", &[OUTPUT.0 + 1, 0, 0]);
    protect_output(&mut f, false);
    fault_preserves_outputs(&mut f, "InterlockedCompareExchange", &[OUTPUT.0, 0, 0]);
}

#[test]
fn slist_initialization_zeros_only_the_aligned_x86_header() {
    let mut f = fixture(&["InitializeSListHead"]);
    f.write(OUTPUT, &[0xa5; 16]);
    assert_eq!(f.call(KERNEL, "InitializeSListHead", &[OUTPUT.0]).eax, None);
    assert_eq!(f.read(OUTPUT, 16), [[0; 8], [0xa5; 8]].concat());
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    fault_preserves_outputs(&mut f, "InitializeSListHead", &[OUTPUT.0 + 4]);
    fault_preserves_outputs(&mut f, "InitializeSListHead", &[0]);
    protect_output(&mut f, false);
    fault_preserves_outputs(&mut f, "InitializeSListHead", &[OUTPUT.0]);
}

#[test]
fn extended_locale_mapping_uses_the_same_bounded_utf16_mapping_and_query_rules() {
    let mut f = fixture(&["LCMapStringEx"]);
    let source = GuestAddress(OUTPUT.0 + 0x100);
    let destination = GuestAddress(OUTPUT.0 + 0x200);
    let wide = |text: &str| {
        text.encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    };
    f.write(OUTPUT, &wide("ja-JP\0"));
    f.write(source, &wide("AbC\0"));
    let mut args = [OUTPUT.0, 0x100, source.0, 4, 0xffff_ffff, 0, 0, 0, 0];
    assert_eq!(value(&mut f, "LCMapStringEx", &args), 4);
    args[4] = destination.0;
    args[5] = 4;
    assert_eq!(value(&mut f, "LCMapStringEx", &args), 4);
    assert_eq!(f.read(destination, 8), wide("abc\0"));
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    args[1] = 0x200;
    assert_eq!(value(&mut f, "LCMapStringEx", &args), 4);
    assert_eq!(f.read(destination, 8), wide("ABC\0"));
    args[5] = 3;
    assert_eq!(value(&mut f, "LCMapStringEx", &args), 0);
    assert_eq!(f.runtime.current_last_error().unwrap(), 122);
    assert_eq!(f.read(destination, 8), wide("ABC\0"));
    args[5] = 4;
    args[8] = 1;
    fault_preserves_outputs(&mut f, "LCMapStringEx", &args);
}

#[test]
fn extended_critical_sections_and_empty_condition_variables_preserve_storage_and_last_error() {
    let mut f = fixture(&[
        "InitializeCriticalSectionEx",
        "EnterCriticalSection",
        "LeaveCriticalSection",
        "InitializeConditionVariable",
        "WakeAllConditionVariable",
    ]);
    assert_eq!(
        value(&mut f, "InitializeCriticalSectionEx", &[OUTPUT.0, 4000, 0]),
        1
    );
    f.call(KERNEL, "EnterCriticalSection", &[OUTPUT.0]);
    f.call(KERNEL, "LeaveCriticalSection", &[OUTPUT.0]);
    let cv = GuestAddress(OUTPUT.0 + 0x80);
    f.write(cv, &[0xff; 8]);
    assert_eq!(
        f.call(KERNEL, "InitializeConditionVariable", &[cv.0]).eax,
        None
    );
    assert_eq!(f.read(cv, 8), [0, 0, 0, 0, 255, 255, 255, 255]);
    assert_eq!(
        f.call(KERNEL, "WakeAllConditionVariable", &[cv.0]).eax,
        None
    );
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    f.write(cv, &[1, 0, 0, 0]);
    fault_preserves_outputs(&mut f, "WakeAllConditionVariable", &[cv.0]);
    protect_output(&mut f, false);
    fault_preserves_outputs(&mut f, "InitializeConditionVariable", &[cv.0]);
}

#[test]
fn load_library_ex_resolves_reached_api_sets_to_system_module_identity() {
    let mut f = fixture(&[
        "LoadLibraryExW",
        "LoadLibraryExA",
        "FreeLibrary",
        "GetModuleHandleW",
        "GetProcAddress",
        "GetTickCount",
    ]);
    let host = f.runtime.module_by_name(KERNEL).unwrap().unwrap().handle;
    f.runtime.mark_module_ready(host).unwrap();
    f.runtime.mark_module_ready(0x4d01_0001).unwrap();
    let sentinel = f
        .runtime
        .process_address_space_allocator()
        .allocation_for_purpose(&vine_core::ProcessAddressAllocationPurpose::CallbackReturns)
        .unwrap()
        .range
        .base;
    f.runtime.begin_dll_process_attach(sentinel).unwrap();
    for contract in [
        "api-ms-win-core-synch-l1-2-0",
        "API-MS-WIN-CORE-FIBERS-L1-1-1.DLL",
        "api-ms-win-core-localization-l1-2-1",
    ] {
        let bytes = contract
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        f.write(OUTPUT, &bytes);
        assert_eq!(value(&mut f, "LoadLibraryExW", &[OUTPUT.0, 0, 0x800]), host);
        assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
        assert_eq!(value(&mut f, "GetModuleHandleW", &[OUTPUT.0]), host);
    }
    f.write(OUTPUT, b"api-ms-win-core-fibers-l1-1-1\0");
    assert_eq!(value(&mut f, "LoadLibraryExA", &[OUTPUT.0, 0, 0]), host);
    f.write(OUTPUT, b"GetTickCount\0");
    assert_eq!(
        value(&mut f, "GetProcAddress", &[host, OUTPUT.0]),
        f.endpoint_address(KERNEL, "GetTickCount").0
    );
    for _ in 0..5 {
        assert_eq!(value(&mut f, "FreeLibrary", &[host]), 1);
    }
    assert_eq!(
        f.runtime.module_by_name(KERNEL).unwrap().unwrap().handle,
        host
    );
    assert_eq!(
        value(&mut f, "GetProcAddress", &[host, OUTPUT.0]),
        f.endpoint_address(KERNEL, "GetTickCount").0
    );
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    assert_eq!(value(&mut f, "FreeLibrary", &[0]), 0);
    assert_eq!(f.runtime.current_last_error().unwrap(), 6);
    assert_eq!(value(&mut f, "LoadLibraryExW", &[OUTPUT.0, 1, 0]), 0);
    assert_eq!(f.runtime.current_last_error().unwrap(), 87);
    f.write(
        OUTPUT,
        &"missing.dll\0"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    assert_eq!(value(&mut f, "LoadLibraryExW", &[OUTPUT.0, 0, 0x800]), 0);
    assert_eq!(f.runtime.current_last_error().unwrap(), 126);
    fault_preserves_outputs(&mut f, "LoadLibraryExW", &[OUTPUT.0, 0, 2]);
}

#[test]
fn get_proc_address_resolves_loaded_system_exports_and_preserves_name_semantics() {
    let mut f = fixture(&["GetProcAddress", "GetTickCount"]);
    let module = f
        .runtime
        .module_by_name("KERNEL32.DLL")
        .unwrap()
        .unwrap()
        .handle;
    f.write(OUTPUT, b"GetTickCount\0");
    let address = value(&mut f, "GetProcAddress", &[module, OUTPUT.0]);
    assert_eq!(address, f.endpoint_address(KERNEL, "GetTickCount").0);
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    assert_eq!(
        value(&mut f, "GetProcAddress", &[module, OUTPUT.0]),
        address
    );
    f.write(OUTPUT, b"gettickcount\0");
    assert_eq!(value(&mut f, "GetProcAddress", &[module, OUTPUT.0]), 0);
    assert_eq!(f.runtime.current_last_error().unwrap(), 127);
    assert_eq!(value(&mut f, "GetProcAddress", &[module, 1]), 0);
    assert_eq!(f.runtime.current_last_error().unwrap(), 127);
}

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
        "GetUserDefaultUILanguage",
        "GetSystemDefaultUILanguage",
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
        ("GetUserDefaultUILanguage", vec![], 0x411),
        ("GetSystemDefaultUILanguage", vec![], 0x411),
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
fn locale_language_name_and_code_page_count_terminators_and_preserve_failed_outputs() {
    let mut f = fixture(&["GetLocaleInfoW", "GetLocaleInfoA"]);
    for (api, unit) in [("GetLocaleInfoW", 2), ("GetLocaleInfoA", 1)] {
        for (kind, text) in [(0x1001, "Japanese"), (0x1004, "932")] {
            let count = text.len() as u32 + 1;
            f.runtime.set_current_last_error(LAST_ERROR).unwrap();
            assert_eq!(
                f.call(KERNEL, api, &[0x411, kind, u32::MAX, 0]).eax,
                Some(count)
            );
            f.write(OUTPUT, &[0xa5; 24]);
            assert_eq!(
                f.call(KERNEL, api, &[0x411, kind, OUTPUT.0, count]).eax,
                Some(count)
            );
            let expected: Vec<u8> = if unit == 2 {
                text.encode_utf16()
                    .chain([0])
                    .flat_map(u16::to_le_bytes)
                    .collect()
            } else {
                text.bytes().chain([0]).collect()
            };
            assert_eq!(f.read(OUTPUT, expected.len()), expected);
            assert_eq!(f.read(GuestAddress(OUTPUT.0 + count * unit), 2), [0xa5; 2]);
            assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
            let before = f.read(OUTPUT, 24);
            assert_eq!(
                f.call(KERNEL, api, &[0x411, kind, OUTPUT.0, count - 1]).eax,
                Some(0)
            );
            assert_eq!(f.runtime.current_last_error().unwrap(), 122);
            assert_eq!(f.read(OUTPUT, 24), before);
        }
        assert_eq!(
            f.call(KERNEL, api, &[0x400, 0x2000_1004, OUTPUT.0, 4 / unit])
                .eax,
            Some(4 / unit)
        );
        assert_eq!(f.read(OUTPUT, 4), 932_u32.to_le_bytes());
    }
    assert_eq!(
        f.call(KERNEL, "GetLocaleInfoW", &[0x411, 0x1001, 0, 9]).eax,
        Some(0)
    );
    assert_eq!(f.runtime.current_last_error().unwrap(), 87);
    let before = f.read(CROSS_PAGE, 16);
    assert!(matches!(
        f.dispatch(KERNEL, "GetLocaleInfoW", &[0x411, 0x1001, CROSS_PAGE.0, 9]),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(f.read(CROSS_PAGE, 16), before);
}

#[test]
fn wide_unnamed_events_use_runtime_wait_reset_and_close_semantics() {
    let mut f = fixture(&[
        "CreateEventW",
        "SetEvent",
        "ResetEvent",
        "WaitForSingleObject",
        "CloseHandle",
    ]);
    for manual in [0, 1] {
        let event = value(&mut f, "CreateEventW", &[0, manual, 1, 0]);
        assert_ne!(event, 0);
        assert_eq!(value(&mut f, "WaitForSingleObject", &[event, 0]), 0);
        assert_eq!(
            value(&mut f, "WaitForSingleObject", &[event, 0]),
            if manual != 0 { 0 } else { 258 }
        );
        assert_eq!(value(&mut f, "SetEvent", &[event]), 1);
        assert_eq!(value(&mut f, "ResetEvent", &[event]), 1);
        assert_eq!(value(&mut f, "WaitForSingleObject", &[event, 0]), 258);
        assert_eq!(value(&mut f, "CloseHandle", &[event]), 1);
        assert_eq!(value(&mut f, "SetEvent", &[event]), 0);
        assert_eq!(f.runtime.current_last_error().unwrap(), 6);
    }
    fault_preserves_outputs(&mut f, "CreateEventW", &[4, 0, 0, 0]);
    fault_preserves_outputs(&mut f, "CreateEventW", &[0, 0, 0, OUTPUT.0]);
}

#[test]
fn process_query_handles_preserve_identity_and_affinity_outputs_on_failure() {
    let mut f = fixture(&[
        "OpenProcess",
        "GetProcessAffinityMask",
        "GetCurrentProcessId",
        "CloseHandle",
    ]);
    let pid = value(&mut f, "GetCurrentProcessId", &[]);
    let first = value(&mut f, "OpenProcess", &[0x400, 0, pid]);
    let second = value(&mut f, "OpenProcess", &[0x1000, 0, pid]);
    assert_ne!(first, 0);
    assert_ne!(first, second);
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    assert_eq!(value(&mut f, "CloseHandle", &[first]), 1);
    for handle in [second, u32::MAX] {
        f.write(OUTPUT, &[0xa5; 16]);
        assert_eq!(
            value(
                &mut f,
                "GetProcessAffinityMask",
                &[handle, OUTPUT.0, OUTPUT.0 + 4]
            ),
            1
        );
        assert_eq!(f.read(OUTPUT, 8), [1, 0, 0, 0, 1, 0, 0, 0]);
        assert_eq!(f.read(GuestAddress(OUTPUT.0 + 8), 8), [0xa5; 8]);
        fault_preserves_outputs(
            &mut f,
            "GetProcessAffinityMask",
            &[handle, OUTPUT.0, BASE.0 + 0x2ffe],
        );
    }
    f.write(OUTPUT, &[0xa5; 16]);
    for handle in [first, 0, 0xffff_fffe] {
        assert_eq!(
            value(
                &mut f,
                "GetProcessAffinityMask",
                &[handle, OUTPUT.0, OUTPUT.0 + 4]
            ),
            0
        );
        assert_eq!(f.runtime.current_last_error().unwrap(), 6);
        assert_eq!(f.read(OUTPUT, 16), [0xa5; 16]);
    }
    for other_pid in [0, pid + 1] {
        assert_eq!(value(&mut f, "OpenProcess", &[0x400, 0, other_pid]), 0);
        assert_eq!(f.runtime.current_last_error().unwrap(), 87);
    }
    fault_preserves_outputs(&mut f, "OpenProcess", &[0x10, 0, pid]);
    fault_preserves_outputs(&mut f, "OpenProcess", &[0x400, 1, pid]);
    assert_eq!(value(&mut f, "CloseHandle", &[second]), 1);
    let reopened = value(&mut f, "OpenProcess", &[0x400, 0, pid]);
    assert_ne!(reopened, first);
    assert_ne!(reopened, second);
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
fn time_zone_information_matches_japan_without_daylight_saving_and_checks_the_full_output() {
    let mut f = fixture(&["GetTimeZoneInformation"]);
    f.write(OUTPUT, &[0xa5; 176]);
    assert_eq!(value(&mut f, "GetTimeZoneInformation", &[OUTPUT.0]), 0);
    let bytes = f.read(OUTPUT, 176);
    assert_eq!(i32::from_le_bytes(bytes[..4].try_into().unwrap()), -540);
    for (offset, expected) in [(4, "Tokyo Standard Time"), (88, "Tokyo Daylight Time")] {
        let units: Vec<_> = bytes[offset..offset + 64]
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .take_while(|&u| u != 0)
            .collect();
        assert_eq!(String::from_utf16(&units).unwrap(), expected);
    }
    assert_eq!(&bytes[68..88], &[0; 20]);
    assert_eq!(&bytes[152..172], &[0; 20]);
    assert_eq!(&bytes[172..], &[0xa5; 4]);
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    fault_preserves_outputs(&mut f, "GetTimeZoneInformation", &[CROSS_PAGE.0]);
    protect_output(&mut f, false);
    fault_preserves_outputs(&mut f, "GetTimeZoneInformation", &[OUTPUT.0]);
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
fn wide_version_and_dbcs_classification_follow_the_fixed_windows_environment() {
    let mut f = fixture(&["GetVersionExW", "IsDBCSLeadByte"]);
    for size in [276_u32, 284] {
        f.write(OUTPUT, &[0xa5; 288]);
        f.words(OUTPUT, &[size]);
        assert_eq!(value(&mut f, "GetVersionExW", &[OUTPUT.0]), 1);
        assert_eq!(
            f.read(GuestAddress(OUTPUT.0 + 4), 18),
            [10_u32, 0, 19045, 2]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .chain([0, 0])
                .collect::<Vec<_>>()
        );
        assert_eq!(f.read(GuestAddress(OUTPUT.0 + 22), 254), [0xa5; 254]);
        if size == 284 {
            assert_eq!(
                f.read(GuestAddress(OUTPUT.0 + 276), 8),
                [0, 0, 0, 0, 0, 0, 1, 0]
            );
        }
    }
    for byte in 0..=255 {
        assert_eq!(
            value(&mut f, "IsDBCSLeadByte", &[byte]),
            u32::from(matches!(byte, 0x81..=0x9f | 0xe0..=0xfc))
        );
    }
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
}

#[test]
fn full_path_resolves_without_opening_files_and_preserves_short_outputs() {
    let mut f = fixture(&["GetFullPathNameW"]);
    let input = GuestAddress(OUTPUT.0 + 0x400);
    let part = GuestAddress(OUTPUT.0 + 0x300);
    for (requested, expected, filename) in [
        ("data/../日本.dat", "C:\\Vine\\日本.dat", Some("日本.dat")),
        ("\\folder\\file", "C:\\folder\\file", Some("file")),
        ("C:folder/", "C:\\Vine\\folder\\", None),
    ] {
        f.write(
            input,
            &requested
                .encode_utf16()
                .chain([0])
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        let required = expected.encode_utf16().count() as u32 + 1;
        f.write(OUTPUT, &[0xa5; 256]);
        f.words(part, &[0xdeadbeef]);
        assert_eq!(
            value(
                &mut f,
                "GetFullPathNameW",
                &[input.0, required - 1, OUTPUT.0, part.0]
            ),
            required
        );
        assert_eq!(f.read(OUTPUT, 256), [0xa5; 256]);
        assert_eq!(f.read(part, 4), 0xdeadbeef_u32.to_le_bytes());
        assert_eq!(
            value(
                &mut f,
                "GetFullPathNameW",
                &[input.0, 128, OUTPUT.0, part.0]
            ),
            required - 1
        );
        assert_eq!(
            f.read(OUTPUT, required as usize * 2),
            expected
                .encode_utf16()
                .chain([0])
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>()
        );
        let pointer = u32::from_le_bytes(f.read(part, 4).try_into().unwrap());
        if let Some(filename) = filename {
            assert_eq!(
                f.read(
                    GuestAddress(pointer),
                    (filename.encode_utf16().count() + 1) * 2
                ),
                filename
                    .encode_utf16()
                    .chain([0])
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>()
            );
        } else {
            assert_eq!(pointer, 0);
        }
    }
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    fault_preserves_outputs(
        &mut f,
        "GetFullPathNameW",
        &[input.0, 128, OUTPUT.0, CROSS_PAGE.0 + 14],
    );
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
fn wide_command_line_returns_the_existing_process_parameter_buffer() {
    let mut f = fixture(&["GetCommandLineW", "GetCommandLineA"]);
    let text = "\"日本語.exe\" /test";
    let env = f.runtime.install_process_environment(text).unwrap();
    f.ack();
    let pointer = value(&mut f, "GetCommandLineW", &[]);
    assert_eq!(
        pointer,
        env.process_parameters.command_line_buffer_address.0
    );
    assert_eq!(value(&mut f, "GetCommandLineW", &[]), pointer);
    let expected = text
        .encode_utf16()
        .chain([0])
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    assert_eq!(f.read(GuestAddress(pointer), expected.len()), expected);
    assert_ne!(value(&mut f, "GetCommandLineA", &[]), pointer);
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
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
    let second = value(&mut f, "GetEnvironmentStringsW", &[]);
    assert_ne!(second, environment);
    assert_eq!(value(&mut f, "FreeEnvironmentStringsW", &[command_line]), 0);
    assert_eq!(value(&mut f, "FreeEnvironmentStringsW", &[second]), 1);
    assert_eq!(value(&mut f, "FreeEnvironmentStringsW", &[environment]), 1);
    assert_eq!(value(&mut f, "FreeEnvironmentStringsW", &[environment]), 0);
    f.runtime.set_current_last_error(LAST_ERROR).unwrap();
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
    for args in [[0, OUTPUT.0, 32769], [0, CROSS_PAGE.0, 64]] {
        fault_preserves_outputs(&mut f, "GetModuleFileNameA", &args);
    }
    f.write(OUTPUT, &[0xa5; 64]);
    assert_eq!(
        value(&mut f, "GetModuleFileNameA", &[0xdead_beef, OUTPUT.0, 64]),
        0
    );
    assert_eq!(f.runtime.current_last_error().unwrap(), 126);
    assert_eq!(f.read(OUTPUT, 64), [0xa5; 64]);
}

#[test]
fn wide_module_filename_counts_utf16_units_and_validates_the_entire_buffer() {
    let mut f = fixture(&["GetModuleFileNameW"]);
    let path: Vec<u16> = "C:\\Vine\\fixture.exe\0".encode_utf16().collect();
    for capacity in [64, path.len() as u32, path.len() as u32 - 1, 1] {
        f.write(OUTPUT, &[0xa5; 128]);
        let result = value(&mut f, "GetModuleFileNameW", &[0, OUTPUT.0, capacity]);
        let truncated = (capacity as usize) < path.len();
        assert_eq!(
            result,
            if truncated {
                capacity
            } else {
                path.len() as u32 - 1
            }
        );
        assert_eq!(
            f.runtime.current_last_error().unwrap(),
            if truncated { 122 } else { 0 }
        );
        let count = (capacity as usize).min(path.len());
        let mut expected = path[..count].to_vec();
        expected[count - 1] = 0;
        let bytes: Vec<_> = expected.into_iter().flat_map(u16::to_le_bytes).collect();
        assert_eq!(f.read(OUTPUT, bytes.len()), bytes);
        assert_eq!(
            f.read(GuestAddress(OUTPUT.0 + bytes.len() as u32), 2),
            [0xa5; 2]
        );
    }
    assert_eq!(value(&mut f, "GetModuleFileNameW", &[0, 0, 0]), 0);
    assert_eq!(f.runtime.current_last_error().unwrap(), 122);
    for args in [[0, CROSS_PAGE.0, 32], [0, OUTPUT.0, 32769]] {
        fault_preserves_outputs(&mut f, "GetModuleFileNameW", &args);
    }
}

#[test]
fn module_filename_resolves_guest_image_handles_and_retains_loaded_dll_paths() {
    let mut f = fixture(&["GetModuleFileNameA", "GetModuleFileNameW"]);
    let mut bytes = support::imported_pe(&[]);
    bytes[0x96..0x98].copy_from_slice(&0x2103_u16.to_le_bytes());
    bytes[0xb4..0xb8].copy_from_slice(&0x12000000_u32.to_le_bytes());
    let limits = vine_core::Pe32Limits::default();
    let snapshot = vine_core::ModuleSnapshot::from_bytes(bytes, &limits).unwrap();
    let plan = vine_core::build_pe32_load_plan(&snapshot, limits).unwrap();
    let image = vine_core::materialize_pe32_image(
        f.runtime.memory_mut(),
        &snapshot,
        &plan,
        GuestAddress(0x12000000),
    )
    .unwrap();
    let path = "C:\\Vine\\plugin\\日本語.dll";
    f.runtime
        .memory_mut()
        .seal_owner(image.layout.owner)
        .unwrap();
    f.runtime
        .load_module(vine_core::RuntimeModule {
            handle: 0x4d010003,
            requested_name: path.into(),
            canonical_name: "日本語.dll".into(),
            runtime_system_module: false,
        })
        .unwrap();
    f.runtime
        .register_module_image(0x4d010003, &plan, &image)
        .unwrap();
    f.ack();
    for (handle, path) in [(0x00400000, "C:\\Vine\\fixture.exe"), (0x12000000, path)] {
        let wide: Vec<u8> = path
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(
            value(&mut f, "GetModuleFileNameW", &[handle, OUTPUT.0, 128]),
            path.encode_utf16().count() as u32
        );
        assert_eq!(f.read(OUTPUT, wide.len()), wide);
        let mut ansi = encoding_rs::SHIFT_JIS.encode(path).0.into_owned();
        let length = ansi.len();
        ansi.push(0);
        assert_eq!(
            value(&mut f, "GetModuleFileNameA", &[handle, OUTPUT.0, 128]),
            length as u32
        );
        assert_eq!(f.read(OUTPUT, ansi.len()), ansi);
    }
    f.write(OUTPUT, &[0xa5; 32]);
    assert_eq!(
        value(&mut f, "GetModuleFileNameW", &[0x12000000, OUTPUT.0, 5]),
        5
    );
    assert_eq!(
        f.read(OUTPUT, 12),
        [b'C', 0, b':', 0, b'\\', 0, b'V', 0, 0, 0, 0xa5, 0xa5]
    );
    assert_eq!(f.runtime.current_last_error().unwrap(), 122);
    for invalid in [0x4d010003, 0x12000001] {
        f.write(OUTPUT, &[0xa5; 32]);
        assert_eq!(
            value(&mut f, "GetModuleFileNameW", &[invalid, OUTPUT.0, 16]),
            0
        );
        assert_eq!(f.read(OUTPUT, 32), [0xa5; 32]);
        assert_eq!(f.runtime.current_last_error().unwrap(), 126);
    }
}

#[test]
fn process_heap_is_shared_by_threads_and_supports_the_regular_heap_calls() {
    let mut f = fixture(&[
        "GetProcessHeap",
        "HeapCreate",
        "HeapAlloc",
        "HeapReAlloc",
        "HeapFree",
        "HeapSize",
    ]);
    let private = value(&mut f, "HeapCreate", &[0, 0, 0]);
    let heap = value(&mut f, "GetProcessHeap", &[]);
    assert_ne!(heap, 0);
    assert_ne!(heap, private);
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    assert_eq!(value(&mut f, "GetProcessHeap", &[]), heap);
    let allocation = value(&mut f, "HeapAlloc", &[heap, 8, 129]);
    assert_ne!(allocation, 0);
    assert_eq!(f.read(GuestAddress(allocation), 129), [0; 129]);
    f.write(GuestAddress(allocation), &[1, 2, 3, 4]);
    let second = RuntimeThreadId(THREAD.0 + 1);
    f.runtime.register_thread(second, 122).unwrap();
    f.runtime.select_thread(second).unwrap();
    assert_eq!(value(&mut f, "GetProcessHeap", &[]), heap);
    assert_eq!(f.runtime.current_last_error().unwrap(), 122);
    assert_eq!(value(&mut f, "HeapSize", &[heap, 0, allocation]), 129);
    let grown = value(&mut f, "HeapReAlloc", &[heap, 8, allocation, 256]);
    assert_ne!(grown, 0);
    assert_eq!(f.read(GuestAddress(grown), 4), [1, 2, 3, 4]);
    assert_eq!(f.read(GuestAddress(grown + 129), 127), [0; 127]);
    assert_eq!(value(&mut f, "HeapFree", &[private, 0, grown]), 0);
    assert_eq!(f.read(GuestAddress(grown), 4), [1, 2, 3, 4]);
    assert_eq!(value(&mut f, "HeapFree", &[heap, 0, grown]), 1);
    assert_eq!(value(&mut f, "GetProcessHeap", &[]), heap);
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
fn bad_pointer_checks_cover_owned_ranges_and_distinguish_read_from_write() {
    let mut f = fixture(&["IsBadReadPtr", "IsBadWritePtr"]);
    let before = f.read(OUTPUT, 64);
    for (pointer, length, expected) in [
        (0, 0, 0),
        (u32::MAX, 0, 0),
        (0, 1, 1),
        (BASE.0, 0x3000, 0),
        (BASE.0 + 0xfff, 2, 0),
        (BASE.0 + 0x2fff, 2, 1),
        (0xffff_fff0, 32, 1),
    ] {
        for name in ["IsBadReadPtr", "IsBadWritePtr"] {
            assert_eq!(value(&mut f, name, &[pointer, length]), expected);
            assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
            assert_eq!(f.read(OUTPUT, 64), before);
        }
    }
    protect_output(&mut f, false);
    assert_eq!(value(&mut f, "IsBadReadPtr", &[OUTPUT.0, 1]), 0);
    assert_eq!(value(&mut f, "IsBadWritePtr", &[OUTPUT.0, 1]), 1);
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    assert_eq!(f.read(OUTPUT, 64), before);
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
    for flags in [0x200, 0x220] {
        assert_eq!(
            value(
                &mut f,
                "WideCharToMultiByte",
                &[932, flags, wide, 3, destination, 4, 0, used_default]
            ),
            4
        );
        assert_eq!(f.read(GuestAddress(destination), 4), CP932);
        assert_eq!(f.read(GuestAddress(used_default), 4), [0; 4]);
    }
    f.write(GuestAddress(wide), &[0x4b, 0x30, 0x99, 0x30, 0, 0]);
    fault_preserves_outputs(
        &mut f,
        "WideCharToMultiByte",
        &[932, 0x220, wide, 3, destination, 8, 0, used_default],
    );
    f.write(GuestAddress(wide), UTF16);
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
        ("MultiByteToWideChar", vec![932, 2, ansi, 4, destination, 3]),
        (
            "MultiByteToWideChar",
            vec![932, 0, ansi, 4, BASE.0 + 0x2ffc, 3],
        ),
    ] {
        fault_preserves_outputs(&mut f, name, &args);
    }
}

#[test]
fn windows_932_strict_conversion_rejects_invalid_sequences_without_output() {
    let mut f = fixture(&["MultiByteToWideChar"]);
    let output = OUTPUT.0 + 0x200;
    for flags in [8, 9] {
        for code_page in [0, 932] {
            // CP932 private-use single bytes remain valid with strict conversion.
            f.write(OUTPUT, &[0x82, 0xa0, 0xa0, 0xfd, 0xfe, 0xff, 0]);
            f.runtime.set_current_last_error(LAST_ERROR).unwrap();
            assert_eq!(
                value(
                    &mut f,
                    "MultiByteToWideChar",
                    &[code_page, flags, OUTPUT.0, u32::MAX, 0, 0]
                ),
                6
            );
            assert_eq!(
                value(
                    &mut f,
                    "MultiByteToWideChar",
                    &[code_page, flags, OUTPUT.0, 7, output, 6]
                ),
                6
            );
            assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
            assert_eq!(
                f.read(GuestAddress(output), 12),
                [
                    0x42, 0x30, 0xf0, 0xf8, 0xf1, 0xf8, 0xf2, 0xf8, 0xf3, 0xf8, 0, 0
                ]
            );

            for invalid in [&[b'A', 0x82][..], &[0x82, b' '], &[0x81, 0xad]] {
                f.write(OUTPUT, invalid);
                f.write(GuestAddress(output), &[0xa5; 16]);
                for (destination, capacity) in [(0, 0), (output, 8)] {
                    assert_eq!(
                        value(
                            &mut f,
                            "MultiByteToWideChar",
                            &[
                                code_page,
                                flags,
                                OUTPUT.0,
                                invalid.len() as u32,
                                destination,
                                capacity
                            ]
                        ),
                        0
                    );
                    assert_eq!(f.runtime.current_last_error().unwrap(), 1113);
                    assert_eq!(f.read(GuestAddress(output), 16), [0xa5; 16]);
                }
            }
        }
    }
    f.write(OUTPUT, &[b'A', 0x82]);
    assert_eq!(
        value(
            &mut f,
            "MultiByteToWideChar",
            &[932, 1, OUTPUT.0, 2, output, 8]
        ),
        2
    );
    assert_eq!(f.read(GuestAddress(output), 4), [b'A', 0, 0xfd, 0xff]);
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
    f.write(OUTPUT, &[0xa5; 24]);
    assert_eq!(
        f.call(KERNEL, "InitializeCriticalSection", &[OUTPUT.0]).eax,
        None
    );
    assert_eq!(f.read(OUTPUT, 24), section_bytes(0x0200_07d0, 0, 0));
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
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

#[test]
fn unicode_mutex_names_share_ansi_identity_and_preserve_non_acp_characters() {
    let mut f = fixture(&["CreateMutexA", "CreateMutexW", "CloseHandle"]);
    let wide = GuestAddress(OUTPUT.0 + 0x100);
    f.write(OUTPUT, &[0x82, 0xa0, 0]); // CP932: あ
    f.write(
        wide,
        &"あ\0"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    let first = value(&mut f, "CreateMutexA", &[0, 1, OUTPUT.0]);
    let second = value(&mut f, "CreateMutexW", &[0, 1, wide.0]);
    assert_ne!(first, second);
    assert_eq!(f.runtime.current_last_error().unwrap(), 183);
    f.write(
        wide,
        &"mutex😀\0"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    let unicode = value(&mut f, "CreateMutexW", &[0, 0, wide.0]);
    assert_ne!(unicode, 0);
    assert_eq!(f.runtime.current_last_error().unwrap(), 0);
    assert_ne!(value(&mut f, "CreateMutexW", &[0, 0, wide.0]), unicode);
    assert_eq!(f.runtime.current_last_error().unwrap(), 183);
    assert_eq!(value(&mut f, "CloseHandle", &[first]), 1);
    assert_eq!(value(&mut f, "CloseHandle", &[second]), 1);
    assert_ne!(value(&mut f, "CreateMutexA", &[0, 0, OUTPUT.0]), 0);
    assert_eq!(f.runtime.current_last_error().unwrap(), 0);
}

#[test]
fn extended_memory_status_and_native_system_info_match_the_guest_environment() {
    let mut f = fixture(&[
        "GlobalMemoryStatus",
        "GlobalMemoryStatusEx",
        "GetSystemInfo",
        "GetNativeSystemInfo",
    ]);
    f.call(KERNEL, "GlobalMemoryStatus", &[OUTPUT.0]);
    let basic = f.read(OUTPUT, 32);
    let extended = GuestAddress(OUTPUT.0 + 0x100);
    f.write(extended, &[0x5a; 64]);
    assert_eq!(value(&mut f, "GlobalMemoryStatusEx", &[extended.0]), 0);
    assert_eq!(f.read(extended, 64), [0x5a; 64]);
    assert_eq!(f.runtime.current_last_error().unwrap(), 87);
    f.words(extended, &[64]);
    assert_eq!(value(&mut f, "GlobalMemoryStatusEx", &[extended.0]), 1);
    let result = f.read(extended, 64);
    assert_eq!(&result[4..8], &basic[4..8]);
    for index in 0..6 {
        let old = u32::from_le_bytes(basic[8 + index * 4..12 + index * 4].try_into().unwrap());
        let new = u64::from_le_bytes(result[8 + index * 8..16 + index * 8].try_into().unwrap());
        assert_eq!(new, u64::from(old));
    }
    assert_eq!(&result[56..64], &[0; 8]);
    f.call(KERNEL, "GetSystemInfo", &[OUTPUT.0]);
    f.call(KERNEL, "GetNativeSystemInfo", &[extended.0]);
    assert_eq!(f.read(OUTPUT, 36), f.read(extended, 36));
}

mod support;

use support::{Fixture, LAST_ERROR, OUTPUT};
use vine_core::{DispatchOutcome, GuestAddress};

#[test]
fn compact_reports_coalesced_committed_free_space_and_preserves_live_bytes() {
    const K: &str = "kernel32.dll";
    let mut f = Fixture::new(&[
        (K, "HeapCreate"),
        (K, "HeapAlloc"),
        (K, "HeapFree"),
        (K, "HeapCompact"),
    ]);
    let heap = f.call(K, "HeapCreate", &[0, 4096, 4096]).eax.unwrap();
    let a = f.call(K, "HeapAlloc", &[heap, 8, 1024]).eax.unwrap();
    let b = f.call(K, "HeapAlloc", &[heap, 8, 1024]).eax.unwrap();
    let c = f.call(K, "HeapAlloc", &[heap, 8, 1536]).eax.unwrap();
    f.write(GuestAddress(c), &[0x5a; 1536]);
    assert_eq!(f.call(K, "HeapCompact", &[heap, 0]).eax, Some(512));
    for (block, available) in [(a, 1024), (b, 2048)] {
        assert_eq!(f.call(K, "HeapFree", &[heap, 0, block]).eax, Some(1));
        assert_eq!(f.call(K, "HeapCompact", &[heap, 0]).eax, Some(available));
        assert_eq!(f.read(GuestAddress(c), 1536), [0x5a; 1536]);
    }
    assert_eq!(f.call(K, "HeapFree", &[heap, 0, c]).eax, Some(1));
    assert_eq!(f.call(K, "HeapCompact", &[heap, 0]).eax, Some(4096));
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
}

#[test]
fn fixed_global_memory_uses_the_process_heap_and_zeroes_requested_bytes() {
    const K: &str = "kernel32.dll";
    let mut f = Fixture::new(&[
        (K, "GlobalAlloc"),
        (K, "GlobalFree"),
        (K, "GetProcessHeap"),
        (K, "HeapSize"),
    ]);
    let heap = f.call(K, "GetProcessHeap", &[]).eax.unwrap();
    let block = f.call(K, "GlobalAlloc", &[0x40, 40]).eax.unwrap();
    assert_ne!(block, 0);
    assert_eq!(block & 7, 0);
    assert_eq!(f.read(GuestAddress(block), 40), [0; 40]);
    assert_eq!(f.call(K, "HeapSize", &[heap, 0, block]).eax, Some(40));
    assert!(matches!(
        f.dispatch(K, "GlobalAlloc", &[2, 40]),
        DispatchOutcome::Fault(_)
    ));
    f.write(GuestAddress(block), &[0xff; 40]);
    assert_eq!(f.call(K, "GlobalFree", &[block]).eax, Some(0));
    assert_eq!(f.call(K, "GlobalFree", &[0]).eax, Some(0));
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    assert_eq!(f.call(K, "GlobalFree", &[block]).eax, Some(block));
    assert_eq!(f.runtime.current_last_error().unwrap(), 6);
    let zero = f.call(K, "GlobalAlloc", &[0x40, 0]).eax.unwrap();
    assert_ne!(zero, 0);
    assert_eq!(f.call(K, "GlobalFree", &[zero]).eax, Some(0));
}

#[test]
fn heap_compatibility_enables_size_classes_without_changing_requested_sizes() {
    const K: &str = "kernel32.dll";
    let mut f = Fixture::new(&[
        (K, "HeapCreate"),
        (K, "HeapSetInformation"),
        (K, "HeapAlloc"),
        (K, "HeapFree"),
        (K, "HeapSize"),
    ]);
    let heap = f.call(K, "HeapCreate", &[0, 0, 0]).eax.unwrap();
    f.words(OUTPUT, &[2]);
    assert!(matches!(
        f.dispatch(K, "HeapSetInformation", &[heap, 0, 0xfffffffe, 4]),
        DispatchOutcome::Fault(_)
    ));
    for _ in 0..2 {
        assert_eq!(
            f.call(K, "HeapSetInformation", &[heap, 0, OUTPUT.0, 4]).eax,
            Some(1)
        );
        assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    }
    let a = f.call(K, "HeapAlloc", &[heap, 8, 259]).eax.unwrap();
    let b = f.call(K, "HeapAlloc", &[heap, 8, 259]).eax.unwrap();
    assert_ne!(a, b);
    assert_eq!(f.call(K, "HeapSize", &[heap, 0, a]).eax, Some(259));
    f.write(GuestAddress(a), &[0x55; 259]);
    assert_eq!(f.call(K, "HeapFree", &[heap, 0, a]).eax, Some(1));
    let reused = f.call(K, "HeapAlloc", &[heap, 8, 270]).eax.unwrap();
    assert_eq!(reused, a);
    assert_eq!(f.read(GuestAddress(reused), 270), [0; 270]);
    assert_eq!(f.call(K, "HeapSize", &[heap, 0, reused]).eax, Some(270));
    for (flags, maximum) in [(1, 0), (0, 65536)] {
        let fixed = f.call(K, "HeapCreate", &[flags, 0, maximum]).eax.unwrap();
        assert_eq!(
            f.call(K, "HeapSetInformation", &[fixed, 0, OUTPUT.0, 4])
                .eax,
            Some(0)
        );
        assert_eq!(f.runtime.current_last_error().unwrap(), 87);
    }
    f.words(OUTPUT, &[0]);
    assert_eq!(
        f.call(K, "HeapSetInformation", &[heap, 0, OUTPUT.0, 4]).eax,
        Some(0)
    );
    assert_eq!(f.runtime.current_last_error().unwrap(), 31);
}

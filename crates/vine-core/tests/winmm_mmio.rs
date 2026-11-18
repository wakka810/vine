mod support;
use support::{Fixture, OUTPUT};
use vine_core::{
    DispatchOutcome, GuestAddress, RuntimeSourceFileManifest, RuntimeSourceMountManifest,
    RuntimeSourceReadHostResponse, RuntimeSourceSnapshotManifest, complete_kernel32_source_read,
};

const M: &str = "winmm.dll";
fn fixture() -> Fixture {
    Fixture::new(
        &[
            "mmioOpenA",
            "mmioClose",
            "mmioDescend",
            "mmioAscend",
            "mmioRead",
            "mmioSeek",
            "mmioGetInfo",
            "mmioSetInfo",
            "mmioAdvance",
        ]
        .map(|name| (M, name)),
    )
}
fn riff() -> Vec<u8> {
    let mut bytes =
        b"RIFF\0\0\0\0WAVEJUNK\x03\0\0\0abc\0fmt \x04\0\0\0testdata\x06\0\0\0sample".to_vec();
    let len = bytes.len() as u32 - 8;
    bytes[4..8].copy_from_slice(&len.to_le_bytes());
    bytes
}
fn word(f: &Fixture, address: u32) -> u32 {
    u32::from_le_bytes(f.read(GuestAddress(address), 4).try_into().unwrap())
}

#[test]
fn memory_riff_stream_preserves_chunks_padding_seek_and_buffer_cursor() {
    let mut f = fixture();
    let bytes = riff();
    let data = OUTPUT.0 + 512;
    f.write(GuestAddress(data), &bytes);
    f.words(
        OUTPUT,
        &[
            0,
            0x204d454d,
            0,
            0,
            0,
            bytes.len() as u32,
            data,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ],
    );
    let handle = f.call(M, "mmioOpenA", &[0, OUTPUT.0, 0]).eax.unwrap();
    assert_ne!(handle, 0);
    let chunk = OUTPUT.0 + 128;
    f.write(GuestAddress(chunk), &[0; 20]);
    assert_eq!(
        f.call(M, "mmioDescend", &[handle, chunk, 0, 0]).eax,
        Some(0)
    );
    assert_eq!(word(&f, chunk), 0x46464952);
    let child = chunk + 32;
    f.words(GuestAddress(child), &[0x20746d66, 0, 0, 0, 0]);
    assert_eq!(
        f.call(M, "mmioDescend", &[handle, child, chunk, 0x10]).eax,
        Some(0)
    );
    assert_eq!(word(&f, child + 4), 4);
    assert_eq!(f.call(M, "mmioRead", &[handle, child + 32, 4]).eax, Some(4));
    assert_eq!(f.read(GuestAddress(child + 32), 4), b"test");
    assert_eq!(f.call(M, "mmioAscend", &[handle, child, 0]).eax, Some(0));
    f.words(GuestAddress(child), &[0x61746164, 0, 0, 0, 0]);
    assert_eq!(
        f.call(M, "mmioDescend", &[handle, child, chunk, 0x10]).eax,
        Some(0)
    );
    assert_eq!(
        f.call(M, "mmioGetInfo", &[handle, OUTPUT.0, 0]).eax,
        Some(0)
    );
    let position = word(&f, OUTPUT.0 + 28);
    f.words(GuestAddress(OUTPUT.0 + 28), &[position + 2]);
    assert_eq!(
        f.call(M, "mmioSetInfo", &[handle, OUTPUT.0, 0]).eax,
        Some(0)
    );
    assert_eq!(
        f.call(M, "mmioRead", &[handle, child + 32, 20]).eax,
        Some(4)
    );
    assert_eq!(f.read(GuestAddress(child + 32), 4), b"mple");
    assert_eq!(
        f.call(M, "mmioSeek", &[handle, u32::MAX, 0]).eax,
        Some(u32::MAX)
    );
    assert_eq!(f.call(M, "mmioClose", &[handle, 0]).eax, Some(0));
    assert_eq!(
        f.call(M, "mmioRead", &[handle, child, 1]).eax,
        Some(u32::MAX)
    );
}

#[test]
fn source_open_publishes_only_after_matching_browser_completion() {
    let mut f = fixture();
    let bytes = riff();
    f.runtime
        .register_source_snapshot(RuntimeSourceSnapshotManifest {
            mount: RuntimeSourceMountManifest {
                source_id: 1,
                root_path_a: b"C:\\Vine\0".to_vec(),
            },
            files: vec![RuntimeSourceFileManifest {
                object_id: 1,
                relative_path_a: b"sound.wav".to_vec(),
                byte_length: bytes.len() as u64,
            }],
        })
        .unwrap();
    f.write(OUTPUT, b"sound.wav\0");
    let info = GuestAddress(OUTPUT.0 + 64);
    f.write(info, &[0; 72]);
    let DispatchOutcome::Deferred(call) = f.dispatch(M, "mmioOpenA", &[OUTPUT.0, info.0, 0x10000])
    else {
        panic!("expected source read")
    };
    assert_eq!(f.read(info, 72), [0; 72]);
    let id = call.request.request_id;
    let outcome = complete_kernel32_source_read(
        &f.dispatcher,
        &call,
        RuntimeSourceReadHostResponse {
            request_id: id,
            source_id: 1,
            object_id: 1,
            offset: 0,
            requested_bytes: bytes.len() as u32,
            transferred_bytes: bytes.len() as u32,
            win32_error: 0,
            bytes,
        },
        &mut f.runtime,
    );
    let DispatchOutcome::Immediate(completion) = outcome else {
        panic!("{outcome:?}")
    };
    let handle = completion.eax.unwrap();
    assert_ne!(handle, 0);
    f.ack();
    assert_eq!(word(&f, info.0 + 68), handle);
    assert_eq!(f.call(M, "mmioRead", &[handle, OUTPUT.0, 4]).eax, Some(4));
    assert_eq!(f.read(OUTPUT, 4), b"RIFF");
    assert_eq!(f.call(M, "mmioClose", &[handle, 0]).eax, Some(0));
}

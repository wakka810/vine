mod support;
use support::{Fixture, OUTPUT};
use vine_core::{
    DispatchOutcome, Gdi32TextRasterHostResponse, GuestAddress, RuntimeDisplayProfile, RuntimeRect,
    complete_gdi32_text_raster,
};
const G: &str = "gdi32.dll";
const U: &str = "user32.dll";
fn fixture() -> (Fixture, u32, u32) {
    let mut f = Fixture::new(
        &[
            "CreateCompatibleDC",
            "CreateFontIndirectA",
            "CreateDIBSection",
            "SelectObject",
            "SetTextColor",
            "SetBkMode",
        ]
        .map(|n| (G, n))
        .into_iter()
        .chain([(U, "DrawTextA"), (U, "DrawTextW")])
        .collect::<Vec<_>>(),
    );
    let rect = RuntimeRect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    };
    f.runtime
        .configure_display_profile(RuntimeDisplayProfile {
            primary_monitor_rect: rect,
            primary_work_rect: rect,
        })
        .unwrap();
    let dc = f.call(G, "CreateCompatibleDC", &[0]).eax.unwrap();
    f.write(OUTPUT, &[0; 60]);
    f.words(OUTPUT, &[16, 8, 0, 0, 400]);
    let font = f.call(G, "CreateFontIndirectA", &[OUTPUT.0]).eax.unwrap();
    f.call(G, "SelectObject", &[dc, font]);
    let mut header = [0u8; 40];
    header[..4].copy_from_slice(&40u32.to_le_bytes());
    header[4..8].copy_from_slice(&32u32.to_le_bytes());
    header[8..12].copy_from_slice(&(-32i32).to_le_bytes());
    header[12..14].copy_from_slice(&1u16.to_le_bytes());
    header[14..16].copy_from_slice(&32u16.to_le_bytes());
    f.write(OUTPUT, &header);
    let bitmap = f
        .call(
            G,
            "CreateDIBSection",
            &[0, OUTPUT.0, 0, OUTPUT.0 + 64, 0, 0],
        )
        .eax
        .unwrap();
    let pixels = u32::from_le_bytes(f.read(GuestAddress(OUTPUT.0 + 64), 4).try_into().unwrap());
    f.call(G, "SelectObject", &[dc, bitmap]);
    (f, dc, pixels)
}
#[test]
fn ansi_and_unicode_calcrect_share_advances_and_do_not_draw() {
    let (mut f, dc, pixels) = fixture();
    let text = GuestAddress(OUTPUT.0 + 128);
    let rect = GuestAddress(OUTPUT.0 + 256);
    f.write(text, &[0x41, 0x93, 0xfa, 0]);
    f.words(rect, &[10, 20, 100, 100]);
    assert_eq!(
        f.call(U, "DrawTextA", &[dc, text.0, 3, rect.0, 0xc00]).eax,
        Some(16)
    );
    assert_eq!(
        f.read(rect, 16),
        [10u32, 20, 34, 36]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    f.write(
        text,
        &"A日"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    f.words(rect, &[10, 20, 100, 100]);
    assert_eq!(
        f.call(U, "DrawTextW", &[dc, text.0, 2, rect.0, 0xc00]).eax,
        Some(16)
    );
    assert_eq!(
        f.read(rect, 16),
        [10u32, 20, 34, 36]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    assert_eq!(f.read(GuestAddress(pixels), 4096), [0; 4096]);
}
#[test]
fn positioned_text_blends_coverage_and_rejects_stale_completions() {
    let (mut f, dc, pixels) = fixture();
    let text = GuestAddress(OUTPUT.0 + 128);
    let rect = GuestAddress(OUTPUT.0 + 256);
    f.write(text, b"A");
    f.words(rect, &[2, 3, 20, 20]);
    f.call(G, "SetBkMode", &[dc, 1]);
    f.call(G, "SetTextColor", &[dc, 255]);
    let DispatchOutcome::Deferred(call) =
        f.dispatch(U, "DrawTextA", &[dc, text.0, 1, rect.0, 0x800])
    else {
        panic!("expected Chrome coverage request")
    };
    let mut coverage = vec![0; 1024];
    coverage[3 * 32 + 2] = 255;
    let (outcome, host) = complete_gdi32_text_raster(
        &f.dispatcher,
        &call,
        Gdi32TextRasterHostResponse {
            request_id: call.request.request_id,
            pixels: coverage,
        },
        &mut f.runtime,
    );
    let DispatchOutcome::Immediate(done) = outcome else {
        panic!("{outcome:?}")
    };
    assert_eq!(done.eax, Some(16));
    assert!(host.is_some());
    f.ack();
    assert_eq!(
        f.read(GuestAddress(pixels + (3 * 32 + 2) * 4), 4),
        [0, 0, 255, 0]
    );
    assert_eq!(f.read(GuestAddress(pixels), 4), [0; 4]);
    assert!(matches!(
        complete_gdi32_text_raster(
            &f.dispatcher,
            &call,
            Gdi32TextRasterHostResponse {
                request_id: call.request.request_id,
                pixels: vec![0; 1024]
            },
            &mut f.runtime
        )
        .0,
        DispatchOutcome::Fault(_)
    ));
}

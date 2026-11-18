mod support;

use support::{BASE, Fixture, LAST_ERROR, OUTPUT, RETURN, SENTINEL, STACK};
use vine_core::{
    DispatchOutcome, GuestAddress, GuestCallbackReturn, RuntimeDisplayProfile, RuntimeRect,
};

const GDI: &str = "gdi32.dll";

fn fixture() -> Fixture {
    let mut f = Fixture::new(
        &[
            "GetStockObject",
            "GetObjectA",
            "GetObjectW",
            "CreateFontIndirectW",
            "CreateCompatibleDC",
            "SelectObject",
            "DeleteObject",
            "DeleteDC",
            "EnumFontsW",
            "GetOutlineTextMetricsW",
        ]
        .map(|name| (GDI, name))
        .into_iter()
        .chain([("user32.dll", "GetDC"), ("user32.dll", "ReleaseDC")])
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
    f
}

fn call(f: &mut Fixture, name: &str, args: &[u32]) -> u32 {
    f.call(GDI, name, args).eax.unwrap()
}

#[test]
fn unicode_logical_fonts_roundtrip_and_preserve_dc_selection_lifetimes() {
    let mut f = fixture();
    let dc = call(&mut f, "CreateCompatibleDC", &[0]);
    let system = call(&mut f, "GetStockObject", &[13]);
    assert_ne!(system, 0);
    for index in [11, 17] {
        let font = call(&mut f, "GetStockObject", &[index]);
        assert_ne!(font, 0);
        assert_eq!(call(&mut f, "GetStockObject", &[index]), font);
        assert_eq!(call(&mut f, "SelectObject", &[dc, font]), system);
        assert_eq!(call(&mut f, "SelectObject", &[dc, system]), font);
        assert_eq!(call(&mut f, "GetObjectW", &[font, 0, 0]), 92);
        assert_eq!(call(&mut f, "GetObjectW", &[font, 92, OUTPUT.0]), 92);
        assert_eq!(
            i32::from_le_bytes(f.read(OUTPUT, 4).try_into().unwrap()),
            if index == 11 { 12 } else { -11 }
        );
    }
    let mut logical = vec![0; 92];
    logical[..4].copy_from_slice(&(-12_i32).to_le_bytes());
    logical[23] = 1;
    let face = "日本語フォント"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    logical[28..28 + face.len()].copy_from_slice(&face);
    f.write(OUTPUT, &logical);
    let font = call(&mut f, "CreateFontIndirectW", &[OUTPUT.0]);
    assert_ne!(font, 0);
    f.write(OUTPUT, &[0xa5; 100]);
    assert_eq!(call(&mut f, "GetObjectW", &[font, 92, OUTPUT.0]), 92);
    assert_eq!(f.read(OUTPUT, 92), logical);
    assert_eq!(f.read(GuestAddress(OUTPUT.0 + 92), 8), [0xa5; 8]);
    assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    assert_eq!(call(&mut f, "GetObjectA", &[font, 60, OUTPUT.0]), 60);
    let (ansi_face, _, invalid) = encoding_rs::SHIFT_JIS.encode("日本語フォント");
    assert!(!invalid);
    assert_eq!(
        f.read(GuestAddress(OUTPUT.0 + 28), ansi_face.len()),
        ansi_face.as_ref()
    );
    assert_eq!(call(&mut f, "SelectObject", &[dc, font]), system);
    assert_eq!(call(&mut f, "DeleteObject", &[font]), 0);
    assert_eq!(call(&mut f, "SelectObject", &[dc, system]), font);
    assert_eq!(call(&mut f, "DeleteObject", &[font]), 1);
    assert_eq!(call(&mut f, "GetObjectW", &[font, 92, OUTPUT.0]), 0);
    assert_eq!(call(&mut f, "DeleteDC", &[dc]), 1);
}

#[test]
fn invalid_unicode_font_buffers_do_not_publish_objects_or_modify_outputs() {
    let mut f = fixture();
    f.write(OUTPUT, &[0xa5; 100]);
    assert_eq!(call(&mut f, "GetObjectW", &[u32::MAX, 92, OUTPUT.0]), 0);
    assert_eq!(f.read(OUTPUT, 100), [0xa5; 100]);
    for address in [OUTPUT.0, BASE.0 + 0x2ffe] {
        assert!(matches!(
            f.dispatch(GDI, "CreateFontIndirectW", &[address]),
            DispatchOutcome::Fault(_)
        ));
    }
    f.write(OUTPUT, &[0; 92]);
    let font = call(&mut f, "CreateFontIndirectW", &[OUTPUT.0]);
    assert_eq!(font, 0x47020001);
    assert!(matches!(
        f.dispatch(GDI, "GetObjectW", &[font, 92, BASE.0 + 0x2ffe]),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(call(&mut f, "GetObjectW", &[font, 92, OUTPUT.0]), 92);
    assert_eq!(f.read(OUTPUT, 92), [0; 92]);
}

#[test]
fn unicode_font_enumeration_uses_owned_callback_storage_and_native_empty_result() {
    let mut f = fixture();
    let dc = f.call("user32.dll", "GetDC", &[0]).eax.unwrap();
    for (face, result) in [
        (None, 0),
        (Some("ＭＳ ゴシック"), 1),
        (Some("ms gothic"), 0x12345678),
    ] {
        let face_address = if let Some(face) = face {
            f.write(
                OUTPUT,
                &face
                    .encode_utf16()
                    .chain([0])
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>(),
            );
            OUTPUT.0
        } else {
            0
        };
        let DispatchOutcome::GuestCallback(call) =
            f.dispatch(GDI, "EnumFontsW", &[dc, face_address, RETURN.0, 0x87654321])
        else {
            panic!("expected the available virtual font")
        };
        let prepared = f.runtime.begin_guest_callback(call).unwrap();
        f.ack();
        let frame: Vec<u32> = f
            .read(prepared.stack_pointer, 20)
            .chunks_exact(4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
            .collect();
        assert_eq!(
            frame,
            [
                SENTINEL.0,
                prepared.scratch_address.0,
                prepared.scratch_address.0 + 92,
                4,
                0x87654321
            ]
        );
        let font = f.read(prepared.scratch_address, 92);
        let name: Vec<u16> = font[28..]
            .chunks_exact(2)
            .map(|bytes| u16::from_le_bytes(bytes.try_into().unwrap()))
            .take_while(|&unit| unit != 0)
            .collect();
        assert_eq!(String::from_utf16(&name).unwrap(), "ＭＳ ゴシック");
        assert_eq!(font[23], 128);
        let metrics = f.read(GuestAddress(frame[2]), 76);
        assert_eq!(metrics[44..52], [0, 0, 0xee, 0xff, 0x65, 0xff, 2, 0]);
        assert_eq!(metrics[56], 128);
        assert_eq!(&metrics[60..64], &0x240040_u32.to_le_bytes());
        let GuestCallbackReturn::Completed(completion) = f
            .runtime
            .complete_guest_callback(&f.dispatcher, SENTINEL, result)
            .unwrap()
        else {
            panic!("single available face must complete enumeration")
        };
        assert_eq!(completion.eax, Some(result));
        assert_eq!(completion.instruction_pointer, RETURN);
        assert_eq!(completion.stack_pointer, GuestAddress(STACK.0 + 20));
        assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
        f.ack();
    }
    for face in ["", "Missing Vine Font"] {
        f.write(
            OUTPUT,
            &face
                .encode_utf16()
                .chain([0])
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        assert_eq!(call(&mut f, "EnumFontsW", &[dc, OUTPUT.0, 0, 0]), 1);
        assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    }
    for args in [[dc, 0, OUTPUT.0, 0], [dc, BASE.0 + 0x3000, RETURN.0, 0]] {
        assert!(matches!(
            f.dispatch(GDI, "EnumFontsW", &args),
            DispatchOutcome::Fault(_)
        ));
    }
    f.write(OUTPUT, &[0x41; 64]);
    assert!(matches!(
        f.dispatch(GDI, "EnumFontsW", &[dc, OUTPUT.0, RETURN.0, 0]),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(f.read(OUTPUT, 64), [0x41; 64]);
    assert_eq!(f.call("user32.dll", "ReleaseDC", &[0, dc]).eax, Some(1));
    assert_eq!(call(&mut f, "EnumFontsW", &[dc, 0, RETURN.0, 0]), 0);
    assert_eq!(f.runtime.current_last_error().unwrap(), 6);
}

#[test]
fn unicode_outline_metrics_resolve_default_width_and_preserve_partial_output_contract() {
    let mut f = fixture();
    let dc = call(&mut f, "CreateCompatibleDC", &[0]);
    let mut logical = vec![0; 92];
    logical[..4].copy_from_slice(&(-12_i32).to_le_bytes());
    logical[16..20].copy_from_slice(&400_i32.to_le_bytes());
    logical[23] = 1;
    for (index, unit) in "MS Gothic".encode_utf16().enumerate() {
        logical[28 + index * 2..30 + index * 2].copy_from_slice(&unit.to_le_bytes());
    }
    f.write(OUTPUT, &logical);
    let font = call(&mut f, "CreateFontIndirectW", &[OUTPUT.0]);
    call(&mut f, "SelectObject", &[dc, font]);
    assert_eq!(call(&mut f, "GetOutlineTextMetricsW", &[dc, 0, 0]), 290);
    assert_eq!(call(&mut f, "GetOutlineTextMetricsW", &[dc, 1, 0]), 290);
    assert_eq!(
        call(&mut f, "GetOutlineTextMetricsW", &[dc, 0, OUTPUT.0]),
        290
    );
    assert_eq!(f.read(OUTPUT, 92), logical);
    for size in [1_u32, 3, 4, 100, 215, 216, 217, 289, 290, 310] {
        f.write(OUTPUT, &[0xa5; 320]);
        let expected = if size <= 216 {
            size
        } else if size < 290 {
            0
        } else {
            290
        };
        assert_eq!(
            call(&mut f, "GetOutlineTextMetricsW", &[dc, size, OUTPUT.0]),
            expected
        );
        let output = f.read(OUTPUT, 320);
        assert!(output[expected as usize..].iter().all(|&byte| byte == 0xa5));
        if expected > 0 {
            assert_eq!(
                &output[..4.min(size as usize)],
                &290_u32.to_le_bytes()[..4.min(size as usize)]
            );
        }
        if expected >= 104 {
            assert_eq!(&output[100..104], &10_i32.to_le_bytes());
            assert_eq!(&output[24..28], &6_i32.to_le_bytes());
            assert_eq!(output[60], 128);
        }
        if expected >= 216 && size <= 216 {
            assert_eq!(&output[200..216], &[0; 16]);
        }
        if expected == 290 {
            for (offset, text) in [
                (200, "ＭＳ ゴシック"),
                (204, "ＭＳ ゴシック"),
                (208, "標準"),
                (212, "Microsoft:ＭＳ ゴシック"),
            ] {
                let start =
                    u32::from_le_bytes(output[offset..offset + 4].try_into().unwrap()) as usize;
                let units: Vec<u16> = output[start..290]
                    .chunks_exact(2)
                    .map(|bytes| u16::from_le_bytes(bytes.try_into().unwrap()))
                    .take_while(|&unit| unit != 0)
                    .collect();
                assert_eq!(String::from_utf16(&units).unwrap(), text);
            }
        }
        assert_eq!(f.runtime.current_last_error().unwrap(), LAST_ERROR);
    }
    f.write(GuestAddress(BASE.0 + 0x2ff0), &[0xa5; 16]);
    assert!(matches!(
        f.dispatch(GDI, "GetOutlineTextMetricsW", &[dc, 290, BASE.0 + 0x2ff0]),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(f.read(GuestAddress(BASE.0 + 0x2ff0), 16), [0xa5; 16]);
    assert_eq!(call(&mut f, "GetObjectW", &[font, 92, OUTPUT.0]), 92);
    assert_eq!(
        f.read(OUTPUT, 92),
        logical,
        "realization must not change the logical font"
    );
    assert_eq!(
        call(&mut f, "GetOutlineTextMetricsW", &[u32::MAX, 290, OUTPUT.0]),
        0
    );
    assert_eq!(f.runtime.current_last_error().unwrap(), 6);
}

mod support;

use support::{Fixture, OUTPUT, SENTINEL, STACK};
use vine_core::{DispatchOutcome, GuestAddress, GuestCallbackReturn};

const USER: &str = "user32.dll";

#[test]
fn unicode_monitor_name_is_bounded_and_empty_message_wait_parks_the_thread() {
    let mut f = Fixture::new(&[
        (USER, "GetMonitorInfoW"),
        (USER, "MonitorFromRect"),
        (USER, "WaitMessage"),
    ]);
    let rect = vine_core::RuntimeRect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    };
    f.runtime
        .configure_display_profile(vine_core::RuntimeDisplayProfile {
            primary_monitor_rect: rect,
            primary_work_rect: rect,
        })
        .unwrap();
    let monitor = 0x4f01_0001; // The fixed logical primary display.
    for (rectangle, intersects) in [
        ([78_i32, 78, 1374, 837], true),
        ([-100, -100, -1, -1], false),
        ([1920, 0, 1921, 20], false),
        ([20, 20, 20, 21], false),
        ([i32::MIN, i32::MIN, i32::MAX, i32::MAX], true),
    ] {
        f.write(
            OUTPUT,
            &rectangle
                .into_iter()
                .flat_map(i32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        for flags in 0..=2 {
            assert_eq!(
                f.call(USER, "MonitorFromRect", &[OUTPUT.0, flags]).eax,
                Some(if flags != 0 || intersects { monitor } else { 0 })
            );
        }
    }
    assert!(matches!(
        f.dispatch(USER, "MonitorFromRect", &[u32::MAX - 4, 2]),
        DispatchOutcome::Fault(_)
    ));
    f.write(OUTPUT, &[0xa5; 108]);
    f.words(OUTPUT, &[104]);
    assert_eq!(
        f.call(USER, "GetMonitorInfoW", &[monitor, OUTPUT.0]).eax,
        Some(1)
    );
    let name: Vec<u8> = r"\\.\DISPLAY1"
        .encode_utf16()
        .chain([0])
        .flat_map(u16::to_le_bytes)
        .collect();
    let result = f.read(OUTPUT, 108);
    assert_eq!(&result[40..40 + name.len()], name);
    assert!(result[40 + name.len()..104].iter().all(|byte| *byte == 0));
    assert_eq!(&result[104..], &[0xa5; 4]);
    let boundary = GuestAddress(support::BASE.0 + 0x2ffc);
    f.words(boundary, &[104]);
    assert!(matches!(
        f.dispatch(USER, "GetMonitorInfoW", &[monitor, boundary.0]),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(f.read(boundary, 4), 104_u32.to_le_bytes());
    let wait = f.call(USER, "WaitMessage", &[]);
    assert_eq!(wait.eax, Some(1));
    assert_eq!(
        wait.execution_boundary,
        vine_core::RuntimeApiBoundaryDisposition::BlockCurrentThread
    );
    assert_eq!(
        f.runtime.scheduler_state().unwrap(),
        vine_core::SchedulerThreadState::WaitingMessage
    );
}

#[test]
fn adjusted_resizable_frame_matches_created_client_geometry() {
    let mut f = Fixture::new(&[
        (USER, "AdjustWindowRectEx"),
        (USER, "RegisterClassExW"),
        (USER, "CreateWindowExW"),
        (USER, "DefWindowProcW"),
        (USER, "GetClientRect"),
        (USER, "GetWindowRect"),
        (USER, "ClientToScreen"),
        (USER, "CreateMenu"),
        (USER, "SetMenu"),
        (USER, "GetMenu"),
        (USER, "DrawMenuBar"),
        (USER, "ShowWindow"),
        (USER, "GetFocus"),
        (USER, "IsWindowVisible"),
        (USER, "IsWindowEnabled"),
        (USER, "IsZoomed"),
        (USER, "GetKeyboardLayout"),
        (USER, "CreateCaret"),
        (USER, "GetWindowTextW"),
        (USER, "GetWindowTextLengthW"),
        (USER, "SetWindowTextW"),
        (USER, "GetPropA"),
        (USER, "SetPropA"),
        ("kernel32.dll", "GlobalAddAtomA"),
        ("kernel32.dll", "GlobalDeleteAtom"),
        (USER, "PeekMessageW"),
        (USER, "WaitMessage"),
        (USER, "GetQueueStatus"),
        (USER, "DispatchMessageW"),
        ("imm32.dll", "ImmIsIME"),
        (USER, "UpdateWindow"),
        (USER, "RedrawWindow"),
        (USER, "BeginPaint"),
        (USER, "EndPaint"),
        (USER, "PostMessageW"),
        (USER, "PeekMessageA"),
        (USER, "SendMessageW"),
        (USER, "SetWindowLongW"),
        (USER, "GetWindowLongW"),
        (USER, "GetSystemMenu"),
        (USER, "EnableMenuItem"),
        (USER, "SetWindowPos"),
        (USER, "InvalidateRect"),
        (USER, "CreateAcceleratorTableW"),
        (USER, "DestroyAcceleratorTable"),
        (USER, "TranslateAcceleratorW"),
    ]);
    let name = GuestAddress(OUTPUT.0 + 0x100);
    wide(&mut f, name, "Resizable");
    f.words(
        OUTPUT,
        &[
            48,
            0x48,
            f.endpoint_address(USER, "DefWindowProcW").0,
            0,
            0,
            0x00400000,
            0,
            0,
            0,
            0,
            name.0,
            0,
        ],
    );
    assert_ne!(f.call(USER, "RegisterClassExW", &[OUTPUT.0]).eax, Some(0));
    for (style, ex_style, expected) in [
        (0x00cf0000, 0x40010, [-8_i32, -31, 18, 18]),
        (0x80ca0000, 0x40000, [-3, -26, 13, 13]),
        (0x80000000, 0x80, [0, 0, 10, 10]),
    ] {
        f.words(OUTPUT, &[0, 0, 10, 10]);
        assert_eq!(
            f.call(USER, "AdjustWindowRectEx", &[OUTPUT.0, style, 0, ex_style])
                .eax,
            Some(1)
        );
        assert_eq!(
            f.read(OUTPUT, 16),
            expected
                .into_iter()
                .flat_map(i32::to_le_bytes)
                .collect::<Vec<_>>()
        );
    }
    f.words(OUTPUT, &[i32::MIN as u32, 0, 10, 10]);
    let before = f.read(OUTPUT, 16);
    assert_eq!(
        f.call(
            USER,
            "AdjustWindowRectEx",
            &[OUTPUT.0, 0xcf0000, 0, 0x40010]
        )
        .eax,
        Some(0)
    );
    assert_eq!(f.read(OUTPUT, 16), before);
    assert!(matches!(
        f.dispatch(
            USER,
            "AdjustWindowRectEx",
            &[OUTPUT.0, 0xcf0000, 1, 0x40010]
        ),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(f.read(OUTPUT, 16), before);

    let DispatchOutcome::GuestCallback(call) = f.dispatch(
        USER,
        "CreateWindowExW",
        &[
            0x40010,
            name.0,
            0,
            0x02cf0000,
            i32::MIN as u32,
            i32::MIN as u32,
            26,
            49,
            0,
            0,
            0x00400000,
            0,
        ],
    ) else {
        panic!("expected window creation callback");
    };
    let mut prepared = f.runtime.begin_guest_callback(call).unwrap();
    f.ack();
    let hwnd = loop {
        let words: Vec<u32> = f
            .read(prepared.stack_pointer, 20)
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        let DispatchOutcome::Immediate(result) = f.dispatch_at(
            USER,
            "DefWindowProcW",
            GuestAddress(STACK.0 + 0x200),
            &words[1..],
        ) else {
            panic!("default WndProc failed");
        };
        f.ack();
        let next = f
            .runtime
            .complete_guest_callback(&f.dispatcher, SENTINEL, result.eax.unwrap())
            .unwrap();
        f.ack();
        match next {
            GuestCallbackReturn::Continue(next) => prepared = next,
            GuestCallbackReturn::Completed(result) => break result.eax.unwrap(),
            other => panic!("unexpected {other:?}"),
        }
    };
    assert_ne!(hwnd, 0);
    check_window_properties(&mut f, hwnd);
    let menu_bar = f.call(USER, "CreateMenu", &[]).eax.unwrap();
    assert_eq!(f.call(USER, "SetMenu", &[hwnd, menu_bar]).eax, Some(1));
    assert_eq!(f.call(USER, "GetMenu", &[hwnd]).eax, Some(menu_bar));
    assert_eq!(f.call(USER, "DrawMenuBar", &[hwnd]).eax, Some(1));
    assert_eq!(f.call(USER, "SetMenu", &[hwnd, 0]).eax, Some(1));
    assert_eq!(f.call(USER, "GetMenu", &[hwnd]).eax, Some(0));
    f.words(OUTPUT, &[(-2_i32) as u32, 3]);
    assert_eq!(
        f.call(USER, "ClientToScreen", &[hwnd, OUTPUT.0]).eax,
        Some(1)
    );
    assert_eq!(
        f.read(OUTPUT, 8),
        [38_u32, 66]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    f.words(OUTPUT, &[i32::MAX as u32, 0]);
    let point = f.read(OUTPUT, 8);
    assert_eq!(
        f.call(USER, "ClientToScreen", &[hwnd, OUTPUT.0]).eax,
        Some(0)
    );
    assert_eq!(f.read(OUTPUT, 8), point);

    assert_eq!(f.call(USER, "ShowWindow", &[hwnd, 0]).eax, Some(0));
    assert_eq!(f.call(USER, "UpdateWindow", &[hwnd]).eax, Some(1));
    assert_eq!(
        f.call(USER, "SetWindowLongW", &[hwnd, (-20_i32) as u32, 0x100])
            .eax,
        Some(0x40110)
    );
    assert_eq!(
        f.call(USER, "GetWindowLongW", &[hwnd, (-20_i32) as u32])
            .eax,
        Some(0x100)
    );
    assert_eq!(
        f.call(USER, "DefWindowProcW", &[hwnd, 0x80, 1, 0]).eax,
        Some(0)
    );
    assert_eq!(
        f.call(USER, "DefWindowProcW", &[hwnd, 0x81, 0, 0]).eax,
        Some(0)
    );
    let DispatchOutcome::GuestCallback(call) =
        f.dispatch(USER, "SetWindowPos", &[hwnd, 0, 123, 456, 789, 123, 0x37])
    else {
        panic!("expected frame recalculation callbacks");
    };
    let mut prepared = f.runtime.begin_guest_callback(call).unwrap();
    f.ack();
    let mut messages = Vec::new();
    loop {
        let words: Vec<u32> = f
            .read(prepared.stack_pointer, 20)
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        messages.push(words[2]);
        let DispatchOutcome::Immediate(result) = f.dispatch_at(
            USER,
            "DefWindowProcW",
            GuestAddress(STACK.0 + 0x200),
            &words[1..],
        ) else {
            panic!("frame default processing failed");
        };
        f.ack();
        let next = f
            .runtime
            .complete_guest_callback(&f.dispatcher, SENTINEL, result.eax.unwrap())
            .unwrap();
        f.ack();
        match next {
            GuestCallbackReturn::Continue(next) => prepared = next,
            GuestCallbackReturn::Completed(result) => {
                assert_eq!(result.eax, Some(1));
                break;
            }
            other => panic!("unexpected frame callback {other:?}"),
        }
    }
    assert_eq!(messages, [0x46, 0x83, 0x47]);
    assert_eq!(
        f.call(USER, "GetWindowLongW", &[hwnd, (-20_i32) as u32])
            .eax,
        Some(0x100)
    );
    assert_eq!(f.call(USER, "ShowWindow", &[hwnd, 0]).eax, Some(0));
    assert_eq!(f.call(USER, "UpdateWindow", &[hwnd]).eax, Some(1));
    assert_eq!(f.call(USER, "InvalidateRect", &[hwnd, 0, 1]).eax, Some(1));
    assert_eq!(
        f.call(USER, "InvalidateRect", &[0xdeadbeef, 0, 1]).eax,
        Some(0)
    );
    let menu = f.call(USER, "GetSystemMenu", &[hwnd, 0]).eax.unwrap();
    assert_ne!(menu, 0);
    assert_eq!(f.call(USER, "GetSystemMenu", &[hwnd, 1]).eax, Some(0));
    assert_eq!(
        f.call(USER, "EnableMenuItem", &[menu, 0xf060, 0]).eax,
        Some(u32::MAX)
    );
    assert_ne!(f.call(USER, "GetSystemMenu", &[hwnd, 0]).eax, Some(menu));
    f.runtime
        .configure_input_profile(vine_core::RuntimeInputProfile {
            initial_mouse_buttons_swapped: false,
            initial_keyboard_source_focused: Some(true),
            cursor_position: Some(vine_core::RuntimeCursorPositionProfile {
                initial_screen_x: 10,
                initial_screen_y: 20,
            }),
        })
        .unwrap();
    let entries = GuestAddress(OUTPUT.0 + 0x200);
    f.write(entries, &[1, 0xcc, 123, 0, 0, 0x20]);
    assert!(matches!(
        f.dispatch(USER, "CreateAcceleratorTableW", &[0xfffffff0, 32767]),
        DispatchOutcome::Fault(_)
    ));
    let table = f
        .call(USER, "CreateAcceleratorTableW", &[entries.0, 1])
        .eax
        .unwrap();
    assert_ne!(table, 0);
    f.write(entries, &[0; 6]); // A caller can release its source immediately.
    f.words(OUTPUT, &[hwnd, 0x100, 122, 0, 0, 0, 0]);
    assert_eq!(
        f.call(USER, "TranslateAcceleratorW", &[hwnd, table, OUTPUT.0])
            .eax,
        Some(0)
    );
    f.words(OUTPUT, &[hwnd, 0x100, 123, 0, 0, 0, 0]);
    let DispatchOutcome::GuestCallback(call) =
        f.dispatch(USER, "TranslateAcceleratorW", &[hwnd, table, OUTPUT.0])
    else {
        panic!("expected accelerator command callback");
    };
    let prepared = f.runtime.begin_guest_callback(call).unwrap();
    f.ack();
    assert_eq!(
        f.read(GuestAddress(prepared.stack_pointer.0 + 4), 16),
        [hwnd, 0x111, 0x12000, 0]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    // The copied command remains valid if its callback retires the table.
    let DispatchOutcome::Immediate(destroyed) = f.dispatch_at(
        USER,
        "DestroyAcceleratorTable",
        GuestAddress(STACK.0 + 0x200),
        &[table],
    ) else {
        panic!("destroy did not complete");
    };
    assert_eq!(destroyed.eax, Some(1));
    f.ack();
    let GuestCallbackReturn::Completed(result) = f
        .runtime
        .complete_guest_callback(&f.dispatcher, SENTINEL, 0)
        .unwrap()
    else {
        panic!("accelerator callback did not complete");
    };
    assert_eq!(result.eax, Some(1));
    f.ack();
    assert_eq!(
        f.call(USER, "TranslateAcceleratorW", &[hwnd, table, OUTPUT.0])
            .eax,
        Some(0)
    );
    assert_eq!(f.runtime.current_last_error().unwrap(), 1403);
    assert_eq!(
        f.call(USER, "PostMessageW", &[hwnd, 0x405, 123, 456]).eax,
        Some(1)
    );
    assert_eq!(
        f.call(USER, "PeekMessageA", &[OUTPUT.0, 0, 0, 0, 1]).eax,
        Some(1)
    );
    assert_eq!(
        f.read(OUTPUT, 16),
        [hwnd, 0x405, 123, 456]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    let DispatchOutcome::GuestCallback(call) =
        f.dispatch(USER, "SendMessageW", &[hwnd, 0x406, 321, 654])
    else {
        panic!("expected synchronous window callback");
    };
    let prepared = f.runtime.begin_guest_callback(call).unwrap();
    f.ack();
    assert_eq!(
        f.read(GuestAddress(prepared.stack_pointer.0 + 4), 16),
        [hwnd, 0x406, 321, 654]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    let GuestCallbackReturn::Completed(result) = f
        .runtime
        .complete_guest_callback(&f.dispatcher, SENTINEL, 0x12345678)
        .unwrap()
    else {
        panic!("message did not return");
    };
    assert_eq!(result.eax, Some(0x12345678));
    f.ack();
    assert_eq!(
        f.call(USER, "GetClientRect", &[hwnd, OUTPUT.0]).eax,
        Some(1)
    );
    assert_eq!(
        f.read(OUTPUT, 16),
        [0_i32, 0, 10, 10]
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        f.call(USER, "GetWindowRect", &[hwnd, OUTPUT.0]).eax,
        Some(1)
    );
    assert_eq!(
        f.read(OUTPUT, 16),
        [32_i32, 32, 58, 81]
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    for (args, expected_messages, rect) in [
        (
            [hwnd, 0, 999, 999, 26, 106, 0x216],
            vec![0x46, 0x83, 0x47],
            [32, 32, 58, 138],
        ),
        (
            [hwnd, 0, 26, 26, 0, 0, 0x2215],
            vec![0x46, 0x47],
            [26, 26, 52, 132],
        ),
        (
            [hwnd, 0, 999, 999, 1296, 759, 0x216],
            vec![0x46, 0x83, 0x47],
            [26, 26, 1322, 785],
        ),
        (
            [hwnd, 0, 999, 999, 1296, 759, 0x216],
            vec![0x46],
            [26, 26, 1322, 785],
        ),
    ] {
        let outcome = f.dispatch(USER, "SetWindowPos", &args);
        assert_eq!(default_callbacks(&mut f, outcome), (1, expected_messages));
        assert_eq!(
            f.call(USER, "GetWindowRect", &[hwnd, OUTPUT.0]).eax,
            Some(1)
        );
        assert_eq!(
            f.read(OUTPUT, 16),
            rect.into_iter()
                .flat_map(i32::to_le_bytes)
                .collect::<Vec<_>>()
        );
        assert_eq!(f.call(USER, "ShowWindow", &[hwnd, 0]).eax, Some(0));
        assert_eq!(f.runtime.foreground_window_handle().unwrap(), None);
    }
    assert_eq!(
        f.call(USER, "GetClientRect", &[hwnd, OUTPUT.0]).eax,
        Some(1)
    );
    assert_eq!(
        f.read(OUTPUT, 16),
        [0_i32, 0, 1280, 720]
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    assert_eq!(f.call(USER, "IsWindowVisible", &[hwnd]).eax, Some(0));
    assert_eq!(
        f.call(USER, "RedrawWindow", &[hwnd, 0, 0, 0x121]).eax,
        Some(1)
    );
    assert!(matches!(
        f.dispatch(USER, "RedrawWindow", &[hwnd, 0, 0, 0x521]),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(f.call(USER, "GetFocus", &[]).eax, Some(0));
    let outcome = f.dispatch(USER, "ShowWindow", &[hwnd, 5]);
    let (previous_visible, messages) = default_callbacks(&mut f, outcome);
    assert_eq!(previous_visible, 0);
    assert_eq!(
        messages,
        [0x18, 0x46, 0x46, 0x1c, 0x86, 6, 0x281, 0x282, 7, 0x47, 5, 3]
    );
    assert_eq!(f.runtime.foreground_window_handle().unwrap(), Some(hwnd));
    assert_eq!(f.call(USER, "ShowWindow", &[hwnd, 5]).eax, Some(1));
    assert_eq!(f.call(USER, "IsWindowVisible", &[hwnd]).eax, Some(1));
    assert_eq!(
        f.call(USER, "PeekMessageW", &[u32::MAX, hwnd, 0x40f, 0x411, 1])
            .eax,
        Some(0)
    );
    assert!(matches!(
        f.dispatch(USER, "PeekMessageW", &[u32::MAX - 4, 0, 0, 0, 1]),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(
        f.call(USER, "GetQueueStatus", &[0x20]).eax,
        Some(0x0020_0020)
    );
    assert_eq!(
        f.call(USER, "GetQueueStatus", &[0x20]).eax,
        Some(0x0020_0000)
    );
    f.call(USER, "PostMessageW", &[hwnd, 0x405, 0, 0]);
    let wait = f.call(USER, "WaitMessage", &[]);
    assert_eq!(wait.eax, Some(1));
    assert_eq!(
        wait.execution_boundary,
        vine_core::RuntimeApiBoundaryDisposition::ContinueQuantum
    );
    // Posted messages precede synthesized painting. Removing WM_PAINT leaves its region pending.
    assert_eq!(
        f.call(USER, "PeekMessageW", &[OUTPUT.0, hwnd, 0x0f, 0x0f, 0])
            .eax,
        Some(1)
    );
    assert_eq!(
        f.read(GuestAddress(OUTPUT.0 + 4), 4),
        0x0f_u32.to_le_bytes()
    );
    assert_eq!(
        f.call(USER, "PeekMessageW", &[u32::MAX, hwnd, 0x40f, 0x411, 1])
            .eax,
        Some(0)
    );
    for (remove, expected_message) in [(1, 0x405_u32), (0, 0x0f), (1, 0x0f), (1, 0x0f)] {
        assert_eq!(
            f.call(USER, "PeekMessageW", &[OUTPUT.0, 0, 0, 0, remove])
                .eax,
            Some(1)
        );
        assert_eq!(
            f.read(GuestAddress(OUTPUT.0 + 4), 4),
            expected_message.to_le_bytes()
        );
    }
    // Synchronous redraw enters the actual window procedure and completes only
    // after its BeginPaint/EndPaint pair has validated the pending region.
    let DispatchOutcome::GuestCallback(call) =
        f.dispatch(USER, "RedrawWindow", &[hwnd, 0, 0, 0x125])
    else {
        panic!("visible redraw must paint synchronously");
    };
    let prepared = f.runtime.begin_guest_callback(call).unwrap();
    f.ack();
    assert_eq!(
        f.read(GuestAddress(prepared.stack_pointer.0 + 4), 16),
        [hwnd, 0x0f, 0, 0]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    for api in ["BeginPaint", "EndPaint"] {
        let DispatchOutcome::Immediate(result) =
            f.dispatch_at(USER, api, GuestAddress(STACK.0 + 0x200), &[hwnd, OUTPUT.0])
        else {
            panic!("paint operation did not complete");
        };
        assert_ne!(result.eax, Some(0));
        f.ack();
    }
    let GuestCallbackReturn::Completed(result) = f
        .runtime
        .complete_guest_callback(&f.dispatcher, SENTINEL, 0)
        .unwrap()
    else {
        panic!("redraw did not complete");
    };
    assert_eq!(result.eax, Some(1));
    f.ack();
    assert_eq!(f.call(USER, "UpdateWindow", &[hwnd]).eax, Some(1));
    assert_eq!(f.call(USER, "GetQueueStatus", &[0x20]).eax, Some(0));
    assert!(matches!(
        f.dispatch_at(USER, "RedrawWindow", support::BASE, &[hwnd, 0, 0, 0x125]),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(f.call(USER, "GetQueueStatus", &[0x20]).eax, Some(0));
    assert_eq!(
        f.call(USER, "PeekMessageW", &[OUTPUT.0, 0, 0, 0, 1]).eax,
        Some(0)
    );
    assert_eq!(f.call(USER, "IsWindowEnabled", &[hwnd]).eax, Some(1));
    assert_eq!(f.call(USER, "IsZoomed", &[hwnd]).eax, Some(0));
    assert_eq!(f.call(USER, "IsZoomed", &[0]).eax, Some(0));
    assert_eq!(f.runtime.current_last_error().unwrap(), 1400);
    assert_eq!(f.call(USER, "GetFocus", &[]).eax, Some(hwnd));
    assert_eq!(f.call(USER, "CreateCaret", &[hwnd, 0, 1, 1]).eax, Some(1));
    assert_eq!(
        f.call(USER, "GetKeyboardLayout", &[0]).eax,
        Some(0x04110411)
    );
    assert_eq!(f.call("imm32.dll", "ImmIsIME", &[0x04110411]).eax, Some(1));
    assert_eq!(f.call("imm32.dll", "ImmIsIME", &[0]).eax, Some(0));
    wide(&mut f, name, "窓見出");
    let outcome = f.dispatch(USER, "SetWindowTextW", &[hwnd, name.0]);
    assert_eq!(default_callbacks(&mut f, outcome), (1, vec![0x0c]));
    wide(&mut f, name, "released source");
    let outcome = f.dispatch(USER, "GetWindowTextLengthW", &[hwnd]);
    assert_eq!(default_callbacks(&mut f, outcome), (3, vec![0x0e]));
    let outcome = f.dispatch(USER, "GetWindowTextW", &[hwnd, OUTPUT.0, 2]);
    assert_eq!(default_callbacks(&mut f, outcome), (1, vec![0x0d]));
    assert_eq!(
        f.read(OUTPUT, 4),
        [0x7a93_u16, 0]
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    );
    assert!(matches!(
        f.dispatch(USER, "SetWindowTextW", &[hwnd, u32::MAX - 1]),
        DispatchOutcome::Fault(_)
    ));
    let outcome = f.dispatch(USER, "GetWindowTextLengthW", &[hwnd]);
    assert_eq!(default_callbacks(&mut f, outcome), (3, vec![0x0e]));
    let outcome = f.dispatch(USER, "SetWindowTextW", &[hwnd, 0]);
    assert_eq!(default_callbacks(&mut f, outcome), (1, vec![0x0c]));
    let outcome = f.dispatch(USER, "GetWindowTextLengthW", &[hwnd]);
    assert_eq!(default_callbacks(&mut f, outcome), (0, vec![0x0e]));
    assert_eq!(f.call(USER, "SetWindowTextW", &[0, name.0]).eax, Some(0));
    assert_eq!(f.runtime.current_last_error().unwrap(), 1400);
    assert_eq!(
        f.call(USER, "PostMessageW", &[hwnd, 0x405, 0, 0]).eax,
        Some(1)
    );
    assert_eq!(
        f.call(USER, "PeekMessageW", &[OUTPUT.0, 0, 0, 0, 1]).eax,
        Some(1)
    );
    let outcome = f.dispatch(USER, "DispatchMessageW", &[OUTPUT.0]);
    assert_eq!(default_callbacks(&mut f, outcome), (0, vec![0x405]));
}

fn check_window_properties(f: &mut Fixture, hwnd: u32) {
    let name = GuestAddress(OUTPUT.0 + 0x500);
    f.write(name, b"TabletProperty\0");
    assert_eq!(f.call(USER, "GetPropA", &[hwnd, name.0]).eax, Some(0));
    assert_eq!(f.runtime.current_last_error().unwrap(), 2);
    let atom = f
        .call("kernel32.dll", "GlobalAddAtomA", &[name.0])
        .eax
        .unwrap();
    assert_eq!(
        f.call(USER, "SetPropA", &[hwnd, name.0, 0x10019]).eax,
        Some(1)
    );
    f.write(name, b"TABLETPROPERTY\0");
    assert_eq!(f.call(USER, "GetPropA", &[hwnd, name.0]).eax, Some(0x10019));
    f.write(name, b"Unrelated\0");
    assert_eq!(f.call(USER, "GetPropA", &[hwnd, atom]).eax, Some(0x10019));
    f.call("kernel32.dll", "GlobalDeleteAtom", &[atom]);
    f.write(name, b"TabletProperty\0");
    assert_eq!(f.call(USER, "GetPropA", &[hwnd, name.0]).eax, Some(0x10019));
    assert_eq!(f.call(USER, "SetPropA", &[hwnd, atom, 0]).eax, Some(1));
    f.runtime
        .set_current_last_error(support::LAST_ERROR)
        .unwrap();
    assert_eq!(f.call(USER, "GetPropA", &[hwnd, name.0]).eax, Some(0));
    assert_eq!(f.runtime.current_last_error().unwrap(), support::LAST_ERROR);
    assert!(matches!(
        f.dispatch(USER, "SetPropA", &[hwnd, u32::MAX, 7]),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(f.call(USER, "SetPropA", &[0, u32::MAX, 7]).eax, Some(0));
    assert_eq!(f.runtime.current_last_error().unwrap(), 1400);
    assert_eq!(f.call(USER, "GetPropA", &[hwnd, atom]).eax, Some(0));
    assert_eq!(f.runtime.current_last_error().unwrap(), 1400);
    // Replacing a string property acquires another atom reference on Windows.
    f.call(USER, "SetPropA", &[hwnd, name.0, 9]);
    f.call("kernel32.dll", "GlobalDeleteAtom", &[atom]);
    assert_eq!(f.call(USER, "GetPropA", &[hwnd, name.0]).eax, Some(9));
    f.call("kernel32.dll", "GlobalDeleteAtom", &[atom]);
    assert_eq!(f.call(USER, "GetPropA", &[hwnd, name.0]).eax, Some(0));
    assert_eq!(f.runtime.current_last_error().unwrap(), 2);
    assert_eq!(f.call(USER, "GetPropA", &[hwnd, atom]).eax, Some(9));
}

fn wide(f: &mut Fixture, address: GuestAddress, text: &str) {
    f.write(
        address,
        &text
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
}

#[test]
fn unicode_popup_preserves_text_and_subclasses_each_window_independently() {
    let imports = [
        "GetClassInfoExW",
        "RegisterClassExW",
        "CreateWindowExW",
        "DefWindowProcW",
        "SetWindowLongW",
        "GetWindowLongW",
        "GetWindowLongA",
        "EnumWindows",
        "GetWindowThreadProcessId",
        "SetTimer",
        "KillTimer",
    ];
    let mut imports = imports.map(|name| (USER, name)).to_vec();
    imports.extend(
        [
            "ImmGetContext",
            "ImmReleaseContext",
            "ImmGetOpenStatus",
            "ImmSetOpenStatus",
            "ImmGetConversionStatus",
            "ImmAssociateContext",
        ]
        .map(|name| ("imm32.dll", name)),
    );
    let mut f = Fixture::new(&imports);
    let name = GuestAddress(OUTPUT.0 + 0x100);
    let title = GuestAddress(OUTPUT.0 + 0x180);
    wide(&mut f, name, "窓クラス");
    wide(&mut f, title, "");
    let class_words = [
        48,
        0,
        f.endpoint_address(USER, "DefWindowProcW").0,
        0,
        0,
        0x00400000,
        0,
        0,
        0,
        0,
        name.0,
        0,
    ];
    f.words(OUTPUT, &class_words);
    assert_eq!(
        f.call(
            USER,
            "GetClassInfoExW",
            &[0x00400000, name.0, OUTPUT.0 + 0x200]
        )
        .eax,
        Some(0)
    );
    assert_eq!(f.runtime.current_last_error().unwrap(), 1411);
    assert_eq!(
        f.read(OUTPUT, 48),
        class_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    let atom = f.call(USER, "RegisterClassExW", &[OUTPUT.0]).eax.unwrap();
    assert_ne!(atom, 0);
    assert_eq!(
        f.call(
            USER,
            "GetClassInfoExW",
            &[0x00400000, name.0, OUTPUT.0 + 0x200]
        )
        .eax,
        Some(atom)
    );
    assert_eq!(
        f.read(GuestAddress(OUTPUT.0 + 0x200), 48),
        f.read(OUTPUT, 48)
    );
    let DispatchOutcome::GuestCallback(call) = f.dispatch(
        USER,
        "CreateWindowExW",
        &[
            0x80, name.0, title.0, 0x80000000, 0, 0, 0, 0, 0, 0, 0x00400000, 0,
        ],
    ) else {
        panic!("window creation must enter WndProc");
    };
    let mut prepared = f.runtime.begin_guest_callback(call).unwrap();
    f.ack();
    let hwnd;
    loop {
        assert_eq!(prepared.target, f.endpoint_address(USER, "DefWindowProcW"));
        let words: Vec<u32> = f
            .read(prepared.stack_pointer, 20)
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        if words[2] == 0x81 {
            let DispatchOutcome::Immediate(data) = f.dispatch_at(
                USER,
                "GetWindowLongW",
                GuestAddress(STACK.0 + 0x200),
                &[words[1], (-21_i32) as u32],
            ) else {
                panic!("pending window data must be readable by WndProc");
            };
            assert_eq!(data.eax, Some(0));
            f.ack();
            let create = f.read(GuestAddress(words[4]), 48);
            let class_pointer = u32::from_le_bytes(create[40..44].try_into().unwrap());
            assert_eq!(
                f.read(GuestAddress(class_pointer), 10),
                "窓クラス"
                    .encode_utf16()
                    .chain([0])
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>()
            );
        }
        let DispatchOutcome::Immediate(result) = f.dispatch_at(
            USER,
            "DefWindowProcW",
            GuestAddress(STACK.0 + 0x200),
            &words[1..],
        ) else {
            panic!("default WndProc failed");
        };
        f.ack();
        let next = f
            .runtime
            .complete_guest_callback(&f.dispatcher, SENTINEL, result.eax.unwrap())
            .unwrap();
        f.ack();
        match next {
            GuestCallbackReturn::Continue(next) => prepared = next,
            GuestCallbackReturn::Completed(result) => {
                hwnd = result.eax.unwrap();
                break;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(
        f.call(USER, "SetWindowLongW", &[hwnd, (-4_i32) as u32, 0x00401000])
            .eax,
        Some(f.endpoint_address(USER, "DefWindowProcW").0)
    );
    assert_eq!(
        f.call(
            USER,
            "SetWindowLongW",
            &[hwnd, (-21_i32) as u32, 0x12345678]
        )
        .eax,
        Some(0)
    );
    assert_eq!(
        f.call(USER, "SetWindowLongW", &[hwnd, (-21_i32) as u32, 0])
            .eax,
        Some(0x12345678)
    );
    for api in ["GetWindowLongA", "GetWindowLongW"] {
        for (index, expected) in [(-21_i32, 0), (-4, 0x00401000), (-6, 0x00400000), (-8, 0)] {
            assert_eq!(f.call(USER, api, &[hwnd, index as u32]).eax, Some(expected));
        }
        assert_eq!(
            f.call(USER, api, &[0xdeadbeef, (-21_i32) as u32]).eax,
            Some(0)
        );
        assert_eq!(f.runtime.current_last_error().unwrap(), 1400);
    }
    // A per-window procedure change must not rewrite the registered class.
    f.call(
        USER,
        "GetClassInfoExW",
        &[0x00400000, atom, OUTPUT.0 + 0x200],
    );
    assert_eq!(
        u32::from_le_bytes(
            f.read(GuestAddress(OUTPUT.0 + 0x208), 4)
                .try_into()
                .unwrap()
        ),
        f.endpoint_address(USER, "DefWindowProcW").0
    );

    for result in [0, 1] {
        let DispatchOutcome::GuestCallback(call) =
            f.dispatch(USER, "EnumWindows", &[0x00401000, 0x12345678])
        else {
            panic!("expected owned top-level window");
        };
        let prepared = f.runtime.begin_guest_callback(call).unwrap();
        f.ack();
        let words: Vec<u32> = f
            .read(prepared.stack_pointer, 12)
            .chunks_exact(4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
            .collect();
        assert_eq!(&words[1..], &[hwnd, 0x12345678]);
        let DispatchOutcome::Immediate(owner) = f.dispatch_at(
            USER,
            "GetWindowThreadProcessId",
            GuestAddress(STACK.0 + 0x200),
            &[hwnd, OUTPUT.0 + 0x300],
        ) else {
            panic!("expected owner query");
        };
        assert_eq!(owner.eax, Some(8192));
        f.ack();
        assert_eq!(
            f.read(GuestAddress(OUTPUT.0 + 0x300), 4),
            4096_u32.to_le_bytes()
        );
        let GuestCallbackReturn::Completed(completion) = f
            .runtime
            .complete_guest_callback(&f.dispatcher, SENTINEL, result)
            .unwrap()
        else {
            panic!("enumeration should finish");
        };
        assert_eq!(completion.eax, Some(result));
        f.ack();
    }
    assert_eq!(f.call(USER, "KillTimer", &[hwnd, 1]).eax, Some(0));
    let imm = "imm32.dll";
    let context = f.call(imm, "ImmGetContext", &[hwnd]).eax.unwrap();
    assert_ne!(context, 0);
    assert_eq!(f.call(imm, "ImmGetOpenStatus", &[context]).eax, Some(0));
    assert_eq!(f.call(imm, "ImmSetOpenStatus", &[context, 0]).eax, Some(1));
    assert!(matches!(
        f.dispatch(imm, "ImmSetOpenStatus", &[context, 1]),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(f.call(imm, "ImmGetOpenStatus", &[context]).eax, Some(0));
    f.write(OUTPUT, &[0xa5; 8]);
    assert!(matches!(
        f.dispatch(
            imm,
            "ImmGetConversionStatus",
            &[context, OUTPUT.0, 0xdeadbeef]
        ),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(f.read(OUTPUT, 8), [0xa5; 8]);
    assert_eq!(
        f.call(
            imm,
            "ImmGetConversionStatus",
            &[context, OUTPUT.0, OUTPUT.0 + 4]
        )
        .eax,
        Some(1)
    );
    assert_eq!(
        f.read(OUTPUT, 8),
        [25_u32, 8]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    for _ in 0..2 {
        assert_eq!(
            f.call(imm, "ImmReleaseContext", &[hwnd, context]).eax,
            Some(1)
        );
        assert_eq!(f.call(imm, "ImmGetContext", &[hwnd]).eax, Some(context));
    }
    assert_eq!(
        f.call(imm, "ImmAssociateContext", &[hwnd, 0]).eax,
        Some(context)
    );
    assert_eq!(f.call(imm, "ImmGetContext", &[hwnd]).eax, Some(0));
    assert_eq!(
        f.call(imm, "ImmAssociateContext", &[hwnd, context]).eax,
        Some(0)
    );
    assert_eq!(f.call(imm, "ImmGetContext", &[hwnd]).eax, Some(context));
    assert_eq!(f.call(USER, "SetTimer", &[hwnd, 1, 50, 0]).eax, Some(1));
    assert_eq!(f.call(USER, "KillTimer", &[hwnd, 1]).eax, Some(1));
    assert_eq!(f.call(USER, "KillTimer", &[hwnd, 1]).eax, Some(0));
}

fn default_callbacks(f: &mut Fixture, outcome: DispatchOutcome) -> (u32, Vec<u32>) {
    default_callbacks_at(f, outcome, 0)
}

fn default_callbacks_at(f: &mut Fixture, outcome: DispatchOutcome, depth: u32) -> (u32, Vec<u32>) {
    assert!(depth < 8);
    let DispatchOutcome::GuestCallback(call) = outcome else {
        panic!("expected window callback, got {outcome:?}");
    };
    let mut prepared = f.runtime.begin_guest_callback(call).unwrap();
    f.ack();
    let mut messages = Vec::new();
    loop {
        let words: Vec<u32> = f
            .read(prepared.stack_pointer, 20)
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        messages.push(words[2]);
        let outcome = f.dispatch_at(
            USER,
            "DefWindowProcW",
            GuestAddress(STACK.0 + 0x200 + 0x100 * depth),
            &words[1..],
        );
        let eax = match outcome {
            DispatchOutcome::Immediate(result) => result.eax.unwrap(),
            DispatchOutcome::GuestCallback(_) => {
                let (eax, nested) = default_callbacks_at(f, outcome, depth + 1);
                messages.extend(nested);
                eax
            }
            other => panic!("default processing failed: {other:?}"),
        };
        f.ack();
        let next = f
            .runtime
            .complete_guest_callback(&f.dispatcher, SENTINEL, eax)
            .unwrap();
        f.ack();
        match next {
            GuestCallbackReturn::Continue(next) => prepared = next,
            GuestCallbackReturn::Completed(result) => return (result.eax.unwrap(), messages),
            other => panic!("unexpected {other:?}"),
        }
    }
}

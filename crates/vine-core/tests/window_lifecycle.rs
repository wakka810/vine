mod support;
use support::{Fixture, OUTPUT, SENTINEL, STACK};
use vine_core::{
    DispatchOutcome, GuestAddress, GuestCallbackReturn, RuntimeCursorPositionProfile,
    RuntimeDisplayProfile, RuntimeInputProfile, RuntimeRect,
};
const U: &str = "user32.dll";

fn callbacks(f: &mut Fixture, outcome: DispatchOutcome) -> (u32, Vec<u32>) {
    let DispatchOutcome::GuestCallback(call) = outcome else {
        panic!("{outcome:?}")
    };
    let mut prepared = f.runtime.begin_guest_callback(call).unwrap();
    f.ack();
    let mut messages = Vec::new();
    loop {
        let words: Vec<_> = f
            .read(prepared.stack_pointer, 20)
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        messages.push(words[2]);
        let DispatchOutcome::Immediate(default) = f.dispatch_at(
            U,
            "DefWindowProcA",
            GuestAddress(STACK.0 + 0x300),
            &words[1..],
        ) else {
            panic!("default processing failed for {words:?}")
        };
        f.ack();
        match f
            .runtime
            .complete_guest_callback(&f.dispatcher, SENTINEL, default.eax.unwrap())
            .unwrap()
        {
            GuestCallbackReturn::Continue(next) => {
                prepared = next;
                f.ack();
            }
            GuestCallbackReturn::Completed(done) => {
                f.ack();
                return (done.eax.unwrap(), messages);
            }
            other => panic!("{other:?}"),
        }
    }
}

fn create_visible(f: &mut Fixture, args: &[u32; 12]) -> u32 {
    let created = f.dispatch(U, "CreateWindowExA", args);
    let DispatchOutcome::GuestCallback(call) = created else {
        panic!("{created:?}");
    };
    let mut prepared = f.runtime.begin_guest_callback(call).unwrap();
    f.ack();
    loop {
        let words: Vec<_> = f
            .read(prepared.stack_pointer, 20)
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        let def_eax = match f.dispatch_at(
            U,
            "DefWindowProcA",
            GuestAddress(STACK.0 + 0x300),
            &words[1..],
        ) {
            DispatchOutcome::Immediate(done) => {
                f.ack();
                done.eax.unwrap()
            }
            DispatchOutcome::GuestCallback(nested) => {
                let mut inner = f.runtime.begin_guest_callback(nested).unwrap();
                f.ack();
                loop {
                    let iw: Vec<_> = f
                        .read(inner.stack_pointer, 20)
                        .chunks_exact(4)
                        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                        .collect();
                    let ieax = match f.dispatch_at(
                        U,
                        "DefWindowProcA",
                        GuestAddress(STACK.0 + 0x500),
                        &iw[1..],
                    ) {
                        DispatchOutcome::Immediate(d) => {
                            f.ack();
                            d.eax.unwrap()
                        }
                        DispatchOutcome::GuestCallback(leaf) => {
                            let lp = f.runtime.begin_guest_callback(leaf).unwrap();
                            f.ack();
                            let lw: Vec<_> = f
                                .read(lp.stack_pointer, 20)
                                .chunks_exact(4)
                                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                                .collect();
                            let DispatchOutcome::Immediate(ld) = f.dispatch_at(
                                U,
                                "DefWindowProcA",
                                GuestAddress(STACK.0 + 0x700),
                                &lw[1..],
                            ) else {
                                panic!("expected leaf immediate");
                            };
                            f.ack();
                            let GuestCallbackReturn::Completed(done) = f
                                .runtime
                                .complete_guest_callback(&f.dispatcher, SENTINEL, ld.eax.unwrap())
                                .unwrap()
                            else {
                                panic!("expected leaf completion");
                            };
                            f.ack();
                            done.eax.unwrap()
                        }
                        other => panic!("{other:?}"),
                    };
                    match f
                        .runtime
                        .complete_guest_callback(&f.dispatcher, SENTINEL, ieax)
                        .unwrap()
                    {
                        GuestCallbackReturn::Continue(next) => {
                            inner = next;
                            f.ack();
                        }
                        GuestCallbackReturn::Completed(done) => {
                            f.ack();
                            break done.eax.unwrap();
                        }
                        other => panic!("{other:?}"),
                    }
                }
            }
            other => panic!("{other:?}"),
        };
        match f
            .runtime
            .complete_guest_callback(&f.dispatcher, SENTINEL, def_eax)
            .unwrap()
        {
            GuestCallbackReturn::Continue(next) => {
                prepared = next;
                f.ack();
            }
            GuestCallbackReturn::Completed(done) => {
                f.ack();
                break done.eax.unwrap();
            }
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn move_collapses_the_client_and_destroy_retires_hwnd_after_nc_destroy() {
    let mut f = Fixture::new(
        &[
            "RegisterClassExA",
            "CreateWindowExA",
            "DefWindowProcA",
            "MoveWindow",
            "DestroyWindow",
            "GetClientRect",
            "InvalidateRect",
        ]
        .map(|n| (U, n))
        .into_iter()
        .chain([("gdi32.dll", "GetStockObject")])
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
    let name = GuestAddress(OUTPUT.0 + 0x100);
    f.write(name, b"Lifecycle\0");
    let brush = f.call("gdi32.dll", "GetStockObject", &[4]).eax.unwrap();
    assert_ne!(brush, 0);
    f.words(
        OUTPUT,
        &[
            48,
            0x48,
            f.endpoint_address(U, "DefWindowProcA").0,
            0,
            0,
            0x400000,
            0,
            0,
            brush,
            0,
            name.0,
            0,
        ],
    );
    assert_ne!(f.call(U, "RegisterClassExA", &[OUTPUT.0]).eax, Some(0));
    let outcome = f.dispatch(
        U,
        "CreateWindowExA",
        &[
            0, name.0, 0, 0x80000000, 20, 30, 640, 480, 0, 0, 0x400000, 0,
        ],
    );
    let (hwnd, _) = callbacks(&mut f, outcome);
    assert_eq!(f.call(U, "InvalidateRect", &[hwnd, 0, 1]).eax, Some(1));
    let paint = f.dispatch(U, "DefWindowProcA", &[hwnd, 0xf, 0, 0]);
    assert_eq!(callbacks(&mut f, paint), (0, vec![0x14]));
    assert_eq!(f.call(U, "DefWindowProcA", &[hwnd, 0xf, 0, 0]).eax, Some(0));
    assert_eq!(
        f.call(U, "DefWindowProcA", &[hwnd, 0x113, 0, 0]).eax,
        Some(0)
    );
    let outcome = f.dispatch(U, "MoveWindow", &[hwnd, 0, 0, 0, 0, 0]);
    let (result, messages) = callbacks(&mut f, outcome);
    assert_eq!(result, 1);
    assert_eq!(messages, [0x46, 0x83, 0x47, 3, 5]);
    assert_eq!(f.call(U, "GetClientRect", &[hwnd, OUTPUT.0]).eax, Some(1));
    assert_eq!(f.read(OUTPUT, 16), [0; 16]);
    assert_eq!(f.call(U, "GetClientRect", &[hwnd, OUTPUT.0]).eax, Some(1));
    let outcome = f.dispatch(U, "DestroyWindow", &[hwnd]);
    let (result, messages) = callbacks(&mut f, outcome);
    assert_eq!(result, 1);
    assert_eq!(messages, [2, 0x82]);
    assert_eq!(f.call(U, "GetClientRect", &[hwnd, OUTPUT.0]).eax, Some(0));
    assert_eq!(f.call(U, "DestroyWindow", &[hwnd]).eax, Some(0));
}

#[test]
fn show_window_hide_deactivates_visible_window_before_move_and_destroy() {
    let mut f = Fixture::new(
        &[
            "RegisterClassExA",
            "CreateWindowExA",
            "DefWindowProcA",
            "ShowWindow",
            "IsWindowVisible",
            "GetForegroundWindow",
            "MoveWindow",
            "DestroyWindow",
        ]
        .map(|n| (U, n))
        .into_iter()
        .chain([("gdi32.dll", "GetStockObject")])
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
    let name = GuestAddress(OUTPUT.0 + 0x100);
    f.write(name, b"Teardown\0");
    let brush = f.call("gdi32.dll", "GetStockObject", &[0]).eax.unwrap();
    f.words(
        OUTPUT,
        &[
            48,
            0,
            f.endpoint_address(U, "DefWindowProcA").0,
            0,
            0,
            0x400000,
            0,
            0,
            brush,
            0,
            name.0,
            0,
        ],
    );
    assert_ne!(f.call(U, "RegisterClassExA", &[OUTPUT.0]).eax, Some(0));
    let hwnd = create_visible(
        &mut f,
        &[
            0, name.0, 0, 0x100A0000, 20, 30, 646, 505, 0, 0, 0x400000, 0,
        ],
    );
    assert_eq!(f.call(U, "IsWindowVisible", &[hwnd]).eax, Some(1));
    assert_eq!(f.call(U, "GetForegroundWindow", &[]).eax, Some(hwnd));

    let hide = f.dispatch(U, "ShowWindow", &[hwnd, 0]);
    let (hide_result, hide_messages) = callbacks(&mut f, hide);
    assert_eq!(hide_result, 1);
    assert_eq!(hide_messages, [0x18, 0x46, 0x47, 0x86, 0x06, 0x1c, 0x08]);
    assert_eq!(f.call(U, "IsWindowVisible", &[hwnd]).eax, Some(0));
    assert_eq!(f.call(U, "GetForegroundWindow", &[]).eax, Some(0));
    assert_eq!(f.call(U, "ShowWindow", &[hwnd, 0]).eax, Some(0));

    let moved = f.dispatch(U, "MoveWindow", &[hwnd, 0, 0, 0, 0, 0]);
    let (move_result, move_messages) = callbacks(&mut f, moved);
    assert_eq!(move_result, 1);
    assert_eq!(move_messages, [0x46, 0x83, 0x47, 3, 5]);

    let destroyed = f.dispatch(U, "DestroyWindow", &[hwnd]);
    let (destroy_result, destroy_messages) = callbacks(&mut f, destroyed);
    assert_eq!(destroy_result, 1);
    assert_eq!(destroy_messages, [2, 0x82]);
}

#[test]
fn post_message_pointer_and_close_and_post_quit_message_round_trip_through_queue() {
    let mut f = Fixture::new(
        &[
            "RegisterClassExA",
            "CreateWindowExA",
            "DefWindowProcA",
            "PostMessageA",
            "PostQuitMessage",
            "PeekMessageA",
            "TranslateMessage",
            "DispatchMessageA",
        ]
        .map(|n| (U, n))
        .into_iter()
        .chain([("gdi32.dll", "GetStockObject")])
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
    f.runtime
        .configure_input_profile(RuntimeInputProfile {
            initial_mouse_buttons_swapped: false,
            initial_keyboard_source_focused: Some(true),
            cursor_position: Some(RuntimeCursorPositionProfile {
                initial_screen_x: 100,
                initial_screen_y: 100,
            }),
        })
        .unwrap();
    let name = GuestAddress(OUTPUT.0 + 0x100);
    f.write(name, b"Posted\0");
    let brush = f.call("gdi32.dll", "GetStockObject", &[0]).eax.unwrap();
    f.words(
        OUTPUT,
        &[
            48,
            0,
            f.endpoint_address(U, "DefWindowProcA").0,
            0,
            0,
            0x400000,
            0,
            0,
            brush,
            0,
            name.0,
            0,
        ],
    );
    assert_ne!(f.call(U, "RegisterClassExA", &[OUTPUT.0]).eax, Some(0));
    let hwnd = create_visible(
        &mut f,
        &[
            0, name.0, 0, 0x90000000, 10, 20, 1280, 720, 0, 0, 0x400000, 0,
        ],
    );

    let lparam = (585_u32 << 16) | 290;
    assert_eq!(
        f.call(U, "PostMessageA", &[hwnd, 0x0201, 1, lparam]).eax,
        Some(1)
    );
    assert_eq!(
        f.call(U, "PostMessageA", &[hwnd, 0x0202, 0, lparam]).eax,
        Some(1)
    );
    assert_eq!(
        f.call(U, "PostMessageA", &[hwnd, 0x0010, 0, 0]).eax,
        Some(1)
    );

    let msg_buf = GuestAddress(OUTPUT.0 + 0x200);
    for expected_msg in [0x0201_u32, 0x0202] {
        assert_eq!(
            f.call(U, "PeekMessageA", &[msg_buf.0, 0, 0, 0, 1]).eax,
            Some(1)
        );
        let msg_words: Vec<_> = f
            .read(msg_buf, 28)
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        assert_eq!(msg_words[1], expected_msg);
        assert_eq!(f.call(U, "TranslateMessage", &[msg_buf.0]).eax, Some(0));
        let disp = f.dispatch(U, "DispatchMessageA", &[msg_buf.0]);
        assert_eq!(callbacks(&mut f, disp), (0, vec![expected_msg]));
    }

    // WM_CLOSE dispatched to DefWindowProcA destroys the window and returns 0.
    assert_eq!(
        f.call(U, "PeekMessageA", &[msg_buf.0, 0, 0, 0, 1]).eax,
        Some(1)
    );
    let close_disp = f.dispatch(U, "DispatchMessageA", &[msg_buf.0]);
    let DispatchOutcome::GuestCallback(close_call) = close_disp else {
        panic!("{close_disp:?}");
    };
    let close_prep = f.runtime.begin_guest_callback(close_call).unwrap();
    f.ack();
    let close_words: Vec<_> = f
        .read(close_prep.stack_pointer, 20)
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    assert_eq!(close_words[2], 0x0010);
    let def_close = f.dispatch_at(
        U,
        "DefWindowProcA",
        GuestAddress(STACK.0 + 0x400),
        &close_words[1..],
    );
    let (def_res, destroy_msgs) = callbacks(&mut f, def_close);
    assert_eq!(def_res, 0);
    assert_eq!(destroy_msgs, [0x18, 0x08, 0x06, 0x86, 0x1c, 2, 0x82]);
    let GuestCallbackReturn::Completed(done_close) = f
        .runtime
        .complete_guest_callback(&f.dispatcher, SENTINEL, def_res)
        .unwrap()
    else {
        panic!("expected DispatchMessageA completion");
    };
    f.ack();
    assert_eq!(done_close.eax, Some(0));

    // PostQuitMessage enqueues WM_QUIT (0x0012) with the exit code in wParam.
    assert_eq!(f.call(U, "PostQuitMessage", &[42]).eax, None);
    assert_eq!(
        f.call(U, "PeekMessageA", &[msg_buf.0, 0, 0, 0, 1]).eax,
        Some(1)
    );
    let quit_words: Vec<_> = f
        .read(msg_buf, 28)
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    assert_eq!(
        (quit_words[0], quit_words[1], quit_words[2]),
        (0, 0x0012, 42)
    );
}

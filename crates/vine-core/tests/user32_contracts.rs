mod support;

use support::{BASE, Fixture, LAST_ERROR, OUTPUT, RETURN, SENTINEL, STACK, THREAD};
use vine_core::{
    DispatchOutcome, GuestAddress, GuestCallbackReturn, GuestRange, PageProtection,
    RuntimeCursorPositionProfile, RuntimeDisplayProfile, RuntimeInputProfile, RuntimeRect,
    RuntimeThreadId,
};

fn display(fixture: &mut Fixture) {
    fixture
        .runtime
        .configure_display_profile(RuntimeDisplayProfile {
            primary_monitor_rect: RuntimeRect {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            },
            primary_work_rect: RuntimeRect {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1032,
            },
        })
        .unwrap();
}

fn input(fixture: &mut Fixture) {
    fixture
        .runtime
        .configure_input_profile(RuntimeInputProfile {
            initial_mouse_buttons_swapped: false,
            initial_keyboard_source_focused: Some(true),
            cursor_position: Some(RuntimeCursorPositionProfile {
                initial_screen_x: 960,
                initial_screen_y: 540,
            }),
        })
        .unwrap();
}

fn enum_fixture() -> Fixture {
    let mut fixture = Fixture::new(&[("user32.dll", "EnumDisplayMonitors")]);
    display(&mut fixture);
    fixture
}

fn begin_enum(fixture: &mut Fixture, stack: GuestAddress, data: u32) -> vine_core::GuestAddress {
    let outcome = fixture.dispatch_at(
        "user32.dll",
        "EnumDisplayMonitors",
        stack,
        &[0, 0, RETURN.0, data],
    );
    let DispatchOutcome::GuestCallback(call) = outcome else {
        panic!("{outcome:?}")
    };
    let prepared = fixture.runtime.begin_guest_callback(call).unwrap();
    assert_eq!(prepared.target, RETURN);
    assert_eq!(prepared.return_sentinel, SENTINEL);
    fixture.ack();
    prepared.stack_pointer
}

#[test]
fn monitor_callback_has_x86_arguments_and_normalizes_bool_return() {
    for (result, expected) in [(0, 0), (7, 1)] {
        let mut fixture = enum_fixture();
        let callback_stack = begin_enum(&mut fixture, STACK, 0x5566_7788);
        let frame = fixture.read(callback_stack, 20);
        let words: Vec<_> = frame
            .chunks_exact(4)
            .map(|word| u32::from_le_bytes(word.try_into().unwrap()))
            .collect();
        assert_eq!(words[0], SENTINEL.0);
        assert_ne!(words[1], 0, "HMONITOR is an opaque live handle");
        assert_eq!(words[2], 0);
        assert_eq!(words[4], 0x5566_7788);
        assert_eq!(
            fixture.read(GuestAddress(words[3]), 16),
            [0_i32, 0, 1920, 1080]
                .into_iter()
                .flat_map(i32::to_le_bytes)
                .collect::<Vec<_>>()
        );
        let GuestCallbackReturn::Completed(completion) = fixture
            .runtime
            .complete_guest_callback(&fixture.dispatcher, SENTINEL, result)
            .unwrap()
        else {
            panic!("monitor enumeration must return")
        };
        assert_eq!(completion.eax, Some(expected));
        assert_eq!(completion.instruction_pointer, RETURN);
        assert_eq!(completion.stack_pointer, GuestAddress(STACK.0 + 20));
        assert_eq!(fixture.runtime.current_last_error().unwrap(), LAST_ERROR);
    }
}

#[test]
fn callback_returns_require_the_live_thread_and_sentinel_and_retire_once() {
    let mut fixture = enum_fixture();
    let callback_stack = begin_enum(&mut fixture, STACK, 0);
    let bytes = fixture.read(callback_stack, 20);
    fixture
        .runtime
        .register_thread(RuntimeThreadId(8193), 23)
        .unwrap();
    fixture
        .runtime
        .select_thread(RuntimeThreadId(8193))
        .unwrap();
    assert!(
        fixture
            .runtime
            .complete_guest_callback(&fixture.dispatcher, SENTINEL, 7)
            .is_err()
    );
    assert_eq!(fixture.read(callback_stack, 20), bytes);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 23);
    fixture.runtime.select_thread(THREAD).unwrap();
    assert!(
        fixture
            .runtime
            .complete_guest_callback(&fixture.dispatcher, GuestAddress(SENTINEL.0 + 4), 7)
            .is_err()
    );
    assert_eq!(fixture.read(callback_stack, 20), bytes);
    let GuestCallbackReturn::Completed(completion) = fixture
        .runtime
        .complete_guest_callback(&fixture.dispatcher, SENTINEL, 7)
        .unwrap()
    else {
        panic!("matching callback must still return")
    };
    assert_eq!(completion.eax, Some(1));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), LAST_ERROR);
    assert!(
        fixture
            .runtime
            .complete_guest_callback(&fixture.dispatcher, SENTINEL, 7)
            .is_err()
    );
}

#[test]
fn nested_callbacks_return_to_the_outer_guest_frame_and_keep_its_scratch() {
    let mut fixture = enum_fixture();
    let outer = begin_enum(&mut fixture, STACK, 17);
    let outer_bytes = fixture.read(outer, 20);
    let outer_rect = u32::from_le_bytes(outer_bytes[12..16].try_into().unwrap());
    let rect = fixture.read(GuestAddress(outer_rect), 16);
    begin_enum(&mut fixture, GuestAddress(outer.0 - 20), 23);
    let GuestCallbackReturn::Completed(nested) = fixture
        .runtime
        .complete_guest_callback(&fixture.dispatcher, SENTINEL, 0)
        .unwrap()
    else {
        panic!("nested callback must return")
    };
    assert_eq!(nested.eax, Some(0));
    assert_eq!(nested.stack_pointer, outer);
    assert_eq!(fixture.read(outer, 20), outer_bytes);
    assert_eq!(fixture.read(GuestAddress(outer_rect), 16), rect);
    fixture.runtime.set_current_last_error(57).unwrap();
    let GuestCallbackReturn::Completed(completion) = fixture
        .runtime
        .complete_guest_callback(&fixture.dispatcher, SENTINEL, 7)
        .unwrap()
    else {
        panic!("outer callback must return")
    };
    assert_eq!(completion.eax, Some(1));
    assert_eq!(completion.stack_pointer, GuestAddress(STACK.0 + 20));
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 57);
}

#[test]
fn monitor_enumeration_rejects_unsupported_arguments_and_nonexecutable_callback() {
    let mut fixture = enum_fixture();
    for args in [
        [1, 0, RETURN.0, 0],
        [0, OUTPUT.0, RETURN.0, 0],
        [0, 0, STACK.0, 0],
    ] {
        fixture.write(OUTPUT, &[0xa5; 16]);
        assert!(matches!(
            fixture.dispatch("user32.dll", "EnumDisplayMonitors", &args),
            DispatchOutcome::Fault(_)
        ));
        assert_eq!(fixture.read(OUTPUT, 16), [0xa5; 16]);
        assert_eq!(fixture.runtime.current_last_error().unwrap(), LAST_ERROR);
    }
    begin_enum(&mut fixture, STACK, 0);
    assert!(
        fixture
            .runtime
            .complete_guest_callback(&fixture.dispatcher, SENTINEL, 1)
            .is_ok()
    );
}

#[test]
fn callback_stack_failure_preserves_guest_bytes_and_allows_a_valid_retry() {
    let mut fixture = enum_fixture();
    let short_stack = GuestAddress(BASE.0 + 8);
    let outcome = fixture.dispatch_at(
        "user32.dll",
        "EnumDisplayMonitors",
        short_stack,
        &[0, 0, RETURN.0, 0],
    );
    let DispatchOutcome::GuestCallback(call) = outcome else {
        panic!("{outcome:?}")
    };
    let before = fixture.read(BASE, 32);
    assert!(fixture.runtime.begin_guest_callback(call).is_err());
    assert_eq!(fixture.read(BASE, 32), before);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), LAST_ERROR);
    begin_enum(&mut fixture, STACK, 0);
    assert!(
        fixture
            .runtime
            .complete_guest_callback(&fixture.dispatcher, SENTINEL, 1)
            .is_ok()
    );
}

#[test]
fn enumeration_requires_display_and_reports_live_geometry() {
    let mut fixture = Fixture::new(&[("user32.dll", "EnumDisplayMonitors")]);
    assert!(matches!(
        fixture.dispatch("user32.dll", "EnumDisplayMonitors", &[0, 0, RETURN.0, 0]),
        DispatchOutcome::Fault(_)
    ));
    fixture
        .runtime
        .configure_display_profile(RuntimeDisplayProfile {
            primary_monitor_rect: RuntimeRect {
                left: 0,
                top: 0,
                right: 1280,
                bottom: 720,
            },
            primary_work_rect: RuntimeRect {
                left: 0,
                top: 0,
                right: 1280,
                bottom: 680,
            },
        })
        .unwrap();
    let stack = begin_enum(&mut fixture, STACK, 0);
    let rect = u32::from_le_bytes(
        fixture
            .read(GuestAddress(stack.0 + 12), 4)
            .try_into()
            .unwrap(),
    );
    assert_eq!(
        fixture.read(GuestAddress(rect), 16),
        [0_i32, 0, 1280, 720]
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect::<Vec<_>>()
    );
}

#[test]
fn system_metrics_use_live_display_preserve_last_error_and_reject_unknown_indices() {
    let mut fixture = Fixture::new(&[
        ("user32.dll", "GetSystemMetrics"),
        ("user32.dll", "SwapMouseButton"),
    ]);
    assert!(matches!(
        fixture.dispatch("user32.dll", "GetSystemMetrics", &[0]),
        DispatchOutcome::Fault(_)
    ));
    display(&mut fixture);
    assert!(matches!(
        fixture.dispatch("user32.dll", "GetSystemMetrics", &[23]),
        DispatchOutcome::Fault(_)
    ));
    fixture
        .runtime
        .configure_input_profile(RuntimeInputProfile {
            initial_mouse_buttons_swapped: false,
            initial_keyboard_source_focused: None,
            cursor_position: None,
        })
        .unwrap();
    for (index, expected) in [
        (0, 1920),
        (1, 1080),
        (4, 23),
        (7, 3),
        (8, 3),
        (23, 0),
        (49, 16),
        (50, 16),
    ] {
        assert_eq!(
            fixture.call("user32.dll", "GetSystemMetrics", &[index]).eax,
            Some(expected)
        );
        assert_eq!(fixture.runtime.current_last_error().unwrap(), LAST_ERROR);
    }
    for swapped in [1, 0, 1] {
        fixture.call("user32.dll", "SwapMouseButton", &[swapped]);
        assert_eq!(
            fixture.call("user32.dll", "GetSystemMetrics", &[23]).eax,
            Some(swapped)
        );
        assert_eq!(fixture.runtime.current_last_error().unwrap(), LAST_ERROR);
    }
    assert!(matches!(
        fixture.dispatch("user32.dll", "GetSystemMetrics", &[0xffff_ffff]),
        DispatchOutcome::Fault(_)
    ));
    let mut fixture = Fixture::new(&[("user32.dll", "GetSystemMetrics")]);
    fixture
        .runtime
        .configure_display_profile(RuntimeDisplayProfile {
            primary_monitor_rect: RuntimeRect {
                left: 0,
                top: 0,
                right: 1280,
                bottom: 720,
            },
            primary_work_rect: RuntimeRect {
                left: 0,
                top: 0,
                right: 1280,
                bottom: 680,
            },
        })
        .unwrap();
    assert_eq!(
        fixture.call("user32.dll", "GetSystemMetrics", &[0]).eax,
        Some(1280)
    );
    assert_eq!(
        fixture.call("user32.dll", "GetSystemMetrics", &[1]).eax,
        Some(720)
    );
}

#[test]
fn show_cursor_returns_signed_count_without_resetting_last_error() {
    let mut fixture = Fixture::new(&[("user32.dll", "ShowCursor")]);
    for (show, expected) in [(0, u32::MAX), (0, u32::MAX - 1), (1, u32::MAX), (1, 0)] {
        assert_eq!(
            fixture.call("user32.dll", "ShowCursor", &[show]).eax,
            Some(expected)
        );
        assert_eq!(fixture.runtime.current_last_error().unwrap(), LAST_ERROR);
    }
}

#[test]
fn cursor_position_guest_output_is_atomic_across_null_readonly_and_cross_boundary_failures() {
    let mut fixture = Fixture::new(&[("user32.dll", "GetCursorPos")]);
    input(&mut fixture);
    fixture.write(OUTPUT, &[0xa5; 8]);
    assert_eq!(
        fixture.call("user32.dll", "GetCursorPos", &[0]).eax,
        Some(0)
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 998);
    fixture.runtime.set_current_last_error(LAST_ERROR).unwrap();
    let partial = GuestAddress(BASE.0 + 0x2ffc);
    fixture.write(partial, &[0xa5; 4]);
    assert!(matches!(
        fixture.dispatch("user32.dll", "GetCursorPos", &[partial.0]),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(fixture.read(partial, 4), [0xa5; 4]);
    let output_page = GuestRange::new(OUTPUT, 0x1000).unwrap();
    fixture
        .runtime
        .memory_mut()
        .protect(
            fixture.owner,
            output_page,
            PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .unwrap();
    fixture.ack();
    assert!(matches!(
        fixture.dispatch("user32.dll", "GetCursorPos", &[OUTPUT.0]),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(fixture.read(OUTPUT, 8), [0xa5; 8]);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), LAST_ERROR);
    fixture
        .runtime
        .memory_mut()
        .protect(
            fixture.owner,
            output_page,
            PageProtection {
                read: true,
                write: true,
                execute: false,
            },
        )
        .unwrap();
    fixture.ack();
    assert_eq!(
        fixture.call("user32.dll", "GetCursorPos", &[OUTPUT.0]).eax,
        Some(1)
    );
    assert_eq!(
        fixture.read(OUTPUT, 8),
        [960_i32, 540]
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect::<Vec<_>>()
    );
}

#[test]
fn swap_mouse_button_returns_previous_state_and_missing_input_can_retry() {
    let mut fixture = Fixture::new(&[("user32.dll", "SwapMouseButton")]);
    assert!(matches!(
        fixture.dispatch("user32.dll", "SwapMouseButton", &[1]),
        DispatchOutcome::Fault(_)
    ));
    input(&mut fixture);
    for (swapped, previous) in [(1, 0), (0, 1), (0, 0)] {
        assert_eq!(
            fixture
                .call("user32.dll", "SwapMouseButton", &[swapped])
                .eax,
            Some(previous)
        );
        assert_eq!(fixture.runtime.current_last_error().unwrap(), LAST_ERROR);
    }
}

#[test]
fn set_rect_writes_signed_coordinates_and_preserves_failed_output() {
    let mut fixture = Fixture::new(&[("user32.dll", "SetRect")]);
    let coordinates = [-23_i32, -45, 67, 89];
    let args = [
        OUTPUT.0,
        coordinates[0] as u32,
        coordinates[1] as u32,
        67,
        89,
    ];
    assert_eq!(fixture.call("user32.dll", "SetRect", &args).eax, Some(1));
    assert_eq!(
        fixture.read(OUTPUT, 16),
        coordinates
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        fixture.call("user32.dll", "SetRect", &[0, 1, 2, 3, 4]).eax,
        Some(0)
    );
    let partial = GuestAddress(BASE.0 + 0x2ff8);
    fixture.write(partial, &[0xa5; 8]);
    assert!(matches!(
        fixture.dispatch("user32.dll", "SetRect", &[partial.0, 1, 2, 3, 4]),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(fixture.read(partial, 8), [0xa5; 8]);
    let page = GuestRange::new(OUTPUT, 0x1000).unwrap();
    fixture
        .runtime
        .memory_mut()
        .protect(
            fixture.owner,
            page,
            PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .unwrap();
    fixture.ack();
    let output = fixture.read(OUTPUT, 16);
    assert!(matches!(
        fixture.dispatch("user32.dll", "SetRect", &args),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(fixture.read(OUTPUT, 16), output);
    assert_eq!(fixture.runtime.current_last_error().unwrap(), LAST_ERROR);
}

#[test]
fn invalid_menu_handles_return_native_errors_while_unsupported_variants_stop() {
    let mut fixture = Fixture::new(&[
        ("user32.dll", "GetSystemMenu"),
        ("user32.dll", "EnableMenuItem"),
    ]);
    for handle in [0, 0x4801_ffff] {
        assert_eq!(
            fixture
                .call("user32.dll", "GetSystemMenu", &[handle, 0])
                .eax,
            Some(0)
        );
        assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
    }
    for handle in [0, 0x4b01_ffff] {
        assert_eq!(
            fixture
                .call("user32.dll", "EnableMenuItem", &[handle, 0xf060, 0])
                .eax,
            Some(u32::MAX)
        );
        assert_eq!(fixture.runtime.current_last_error().unwrap(), 1401);
    }
    fixture.runtime.set_current_last_error(LAST_ERROR).unwrap();
    assert_eq!(
        fixture.call("user32.dll", "GetSystemMenu", &[0, 1]).eax,
        Some(0)
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), 1400);
}

#[test]
fn async_key_state_sign_extends_short_consumes_only_queried_press_and_respects_focus() {
    use vine_core::{RuntimeKeyboardSourceEvent, RuntimeKeyboardSourceEventKind as Kind};
    let mut fixture = Fixture::new(&[("user32.dll", "GetAsyncKeyState")]);
    assert!(matches!(
        fixture.dispatch("user32.dll", "GetAsyncKeyState", &[0x41]),
        DispatchOutcome::Fault(_)
    ));
    input(&mut fixture);
    for (sequence, key) in [(1, 0x41), (2, 0x42)] {
        fixture
            .runtime
            .ingest_keyboard_event(RuntimeKeyboardSourceEvent {
                sequence,
                kind: Kind::KeyDown,
                virtual_key: Some(key),
                repeat: false,
            })
            .unwrap();
    }
    assert_eq!(
        fixture.call("user32.dll", "GetAsyncKeyState", &[0x100]).eax,
        Some(0)
    );
    assert_eq!(
        fixture.call("user32.dll", "GetAsyncKeyState", &[0x41]).eax,
        Some(0xffff_8001)
    );
    assert_eq!(
        fixture.call("user32.dll", "GetAsyncKeyState", &[0x41]).eax,
        Some(0xffff_8000)
    );
    assert_eq!(
        fixture.call("user32.dll", "GetAsyncKeyState", &[0x42]).eax,
        Some(0xffff_8001)
    );
    fixture
        .runtime
        .ingest_keyboard_event(RuntimeKeyboardSourceEvent {
            sequence: 3,
            kind: Kind::FocusLost,
            virtual_key: None,
            repeat: false,
        })
        .unwrap();
    assert_eq!(
        fixture.call("user32.dll", "GetAsyncKeyState", &[0x41]).eax,
        Some(0)
    );
    fixture
        .runtime
        .ingest_keyboard_event(RuntimeKeyboardSourceEvent {
            sequence: 4,
            kind: Kind::FocusGained,
            virtual_key: None,
            repeat: false,
        })
        .unwrap();
    assert_eq!(
        fixture.call("user32.dll", "GetAsyncKeyState", &[0x41]).eax,
        Some(0)
    );
    assert_eq!(fixture.runtime.current_last_error().unwrap(), LAST_ERROR);
}

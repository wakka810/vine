#[test]
fn dinput_keyboard_format_acquisition_output_and_lifetime_are_owned() {
    let mut runtime = d3d9_fixture_runtime(d3d9_fixture_memory());
    let window = seed_created_d3d9_window(&mut runtime);
    runtime
        .configure_input_profile(crate::runtime::RuntimeInputProfile {
            initial_mouse_buttons_swapped: false,
            initial_keyboard_source_focused: Some(true),
            cursor_position: None,
        })
        .unwrap();
    let root = runtime
        .install_com_object(crate::dinput8::direct_input8a_object_spec())
        .unwrap();
    let root = runtime
        .com()
        .interface_for_object_iid(root.object_id, crate::dinput8::DINPUT8_IDIRECTINPUT8A_IID)
        .unwrap()
        .clone();
    let guid = D3D9_FIXTURE_PRESENTATION;
    runtime
        .memory
        .write(
            guid,
            &[
                0x61, 0x2b, 0x1d, 0x6f, 0xa0, 0xd5, 0xcf, 0x11, 0xbf, 0xc7, 0x44, 0x45, 0x53, 0x54,
                0, 0,
            ],
        )
        .unwrap();
    assert_eq!(
        texture_discard_call(&mut runtime, &root, 3, &[guid.0, D3D9_FIXTURE_OUTPUT.0, 0]),
        0
    );
    let keyboard = texture_discard_output_interface(&runtime);
    assert_eq!(
        texture_discard_call(&mut runtime, &keyboard, 7, &[]),
        0x80070015
    );
    let format = D3D9_FIXTURE_PRESENTATION;
    let objects = GuestAddress(format.0 + 24);
    write_d3d9_fixture_words(
        &mut runtime.memory,
        format,
        &[
            24, 16, 2, 256, 2, objects.0, 0, 0x1e, 0x1e0c, 0, 0, 0xc8, 0xc80c, 0,
        ],
    );
    assert_eq!(
        texture_discard_call(&mut runtime, &keyboard, 11, &[format.0]),
        0
    );
    let before = runtime
        .direct_input_device(keyboard.object_id)
        .unwrap()
        .clone();
    write_d3d9_fixture_words(&mut runtime.memory, GuestAddress(objects.0 + 20), &[0x1e]);
    assert_eq!(
        texture_discard_call(&mut runtime, &keyboard, 11, &[format.0]),
        0x80070057
    );
    assert_eq!(
        runtime.direct_input_device(keyboard.object_id).unwrap(),
        &before
    );
    assert_eq!(
        texture_discard_call(&mut runtime, &keyboard, 13, &[window.handle, 10]),
        0
    );
    assert_eq!(texture_discard_call(&mut runtime, &keyboard, 7, &[]), 0);
    assert_eq!(texture_discard_call(&mut runtime, &keyboard, 7, &[]), 1);
    assert_eq!(
        texture_discard_call(&mut runtime, &keyboard, 11, &[format.0]),
        0x800700aa
    );
    for (sequence, virtual_key) in [(1, 0x41), (2, 0x26)] {
        runtime
            .input
            .ingest_keyboard_event(crate::runtime::RuntimeKeyboardSourceEvent {
                sequence,
                virtual_key: Some(virtual_key),
                repeat: false,
                kind: crate::runtime::RuntimeKeyboardSourceEventKind::KeyDown,
            })
            .unwrap();
    }
    let output = GuestAddress(D3D9_FIXTURE_OUTPUT.0 + 0x100);
    runtime.memory.write(output, &[0xcc; 256]).unwrap();
    assert_eq!(
        texture_discard_call(&mut runtime, &keyboard, 9, &[255, output.0]),
        0x80070057
    );
    assert_eq!(runtime.memory.read(output, 256).unwrap(), &[0xcc; 256]);
    assert_eq!(
        texture_discard_call(&mut runtime, &keyboard, 9, &[256, output.0]),
        0
    );
    let bytes = runtime.memory.read(output, 256).unwrap();
    assert_eq!(bytes[0x1e], 0x80);
    assert_eq!(bytes[0xc8], 0x80);
    assert_eq!(bytes.iter().filter(|b| **b != 0).count(), 2);
    assert_eq!(texture_discard_call(&mut runtime, &keyboard, 8, &[]), 0);
    assert_eq!(
        texture_discard_call(&mut runtime, &keyboard, 9, &[256, output.0]),
        0x8007000c
    );
    assert_eq!(texture_discard_call(&mut runtime, &keyboard, 2, &[]), 0);
    assert!(runtime.direct_input_device(keyboard.object_id).is_err());
    assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
}

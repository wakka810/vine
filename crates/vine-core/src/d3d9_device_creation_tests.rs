#[test]
fn d3d8_fullscreen_d16_creation_transforms_and_depth_clear_are_owned() {
    let mut runtime = d3d9_fixture_runtime(d3d9_fixture_memory());
    let window = seed_created_d3d9_window(&mut runtime);
    let root = runtime
        .install_com_object(crate::d3d8::root_spec())
        .unwrap();
    let root = runtime
        .com()
        .interface_for_object_iid(root.object_id, crate::d3d8::D3D8_IID)
        .unwrap()
        .clone();
    acknowledge_d3d9_fixture_memory(&mut runtime.memory);
    write_d3d9_fixture_words(
        &mut runtime.memory,
        D3D9_FIXTURE_PRESENTATION,
        &[640, 480, 22, 0, 0, 2, 0, 0, 1, 80, 1, 60, 1],
    );
    assert_eq!(
        texture_discard_call(
            &mut runtime,
            &root,
            15,
            &[
                0,
                1,
                window.handle,
                0x20,
                D3D9_FIXTURE_PRESENTATION.0,
                D3D9_FIXTURE_OUTPUT.0
            ]
        ),
        0
    );
    let pointer = u32::from_le_bytes(
        runtime
            .memory
            .read(D3D9_FIXTURE_OUTPUT, 4)
            .unwrap()
            .try_into()
            .unwrap(),
    );
    let device = runtime
        .com_interface_for_pointer(GuestAddress(pointer))
        .unwrap()
        .unwrap()
        .clone();
    let references = runtime
        .com()
        .object(root.object_id)
        .unwrap()
        .reference_count;
    assert_eq!(
        texture_discard_call(&mut runtime, &device, 6, &[D3D9_FIXTURE_OUTPUT.0]),
        0
    );
    assert_eq!(texture_discard_output_interface(&runtime).id, root.id);
    assert_eq!(
        runtime
            .com()
            .object(root.object_id)
            .unwrap()
            .reference_count,
        references + 1
    );
    assert_eq!(
        texture_discard_call(&mut runtime, &root, 2, &[]),
        references
    );
    assert_eq!(
        texture_discard_call(&mut runtime, &device, 8, &[D3D9_FIXTURE_OUTPUT.0]),
        0
    );
    assert_eq!(
        runtime.memory.read(D3D9_FIXTURE_OUTPUT, 16).unwrap(),
        [640u32, 480, 60, 22]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        texture_discard_call(
            &mut runtime,
            &device,
            20,
            &[8, 8, 1, 0, 21, 1, D3D9_FIXTURE_OUTPUT.0]
        ),
        0
    );
    let texture = texture_discard_output_interface(&runtime);
    assert_eq!(texture.iid, crate::d3d8::TEXTURE8_IID);
    assert_eq!(
        texture_discard_call(&mut runtime, &texture, 15, &[0, D3D9_FIXTURE_OUTPUT.0]),
        0
    );
    let level = texture_discard_output_interface(&runtime);
    assert_eq!(
        texture_discard_call(&mut runtime, &texture, 15, &[0, D3D9_FIXTURE_OUTPUT.0]),
        0
    );
    assert_eq!(texture_discard_output_interface(&runtime).id, level.id);
    assert_eq!(texture_discard_call(&mut runtime, &level, 2, &[]), 1);
    assert_eq!(
        texture_discard_call(&mut runtime, &level, 8, &[D3D9_FIXTURE_OUTPUT.0]),
        0
    );
    assert_eq!(
        runtime.memory.read(D3D9_FIXTURE_OUTPUT, 32).unwrap(),
        [21u32, 1, 0, 1, 256, 0, 8, 8]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        texture_discard_call(&mut runtime, &level, 9, &[D3D9_FIXTURE_OUTPUT.0, 0, 0]),
        0
    );
    let texels = runtime
        .graphics_texture_for_com_object(texture.object_id)
        .unwrap()
        .backing
        .unwrap()
        .range
        .start();
    runtime.memory.write(texels, &[1, 2, 3, 4]).unwrap();
    assert_eq!(texture_discard_call(&mut runtime, &level, 10, &[]), 0);
    assert_eq!(
        texture_discard_call(&mut runtime, &device, 61, &[0, texture.guest_pointer.0]),
        0
    );
    assert_eq!(texture_discard_call(&mut runtime, &device, 61, &[0, 0]), 0);
    assert_eq!(texture_discard_call(&mut runtime, &texture, 2, &[]), 1);
    assert_eq!(runtime.memory.read(texels, 4).unwrap(), [1, 2, 3, 4]);
    assert_eq!(texture_discard_call(&mut runtime, &level, 2, &[]), 0);
    assert!(runtime.memory.read(texels, 4).is_err());
    assert_eq!(
        texture_discard_call(
            &mut runtime,
            &device,
            23,
            &[80, 0, 0x102, 1, D3D9_FIXTURE_OUTPUT.0]
        ),
        0
    );
    let vb = texture_discard_output_interface(&runtime);
    assert_eq!(vb.iid, crate::d3d8::VERTEX_BUFFER8_IID);
    assert_eq!(
        texture_discard_call(&mut runtime, &vb, 11, &[0, 80, D3D9_FIXTURE_OUTPUT.0, 0]),
        0
    );
    let address = GuestAddress(u32::from_le_bytes(
        runtime
            .memory
            .read(D3D9_FIXTURE_OUTPUT, 4)
            .unwrap()
            .try_into()
            .unwrap(),
    ));
    runtime.memory.write(address, &[0; 80]).unwrap();
    assert_eq!(texture_discard_call(&mut runtime, &vb, 12, &[]), 0);
    assert!(runtime.memory.write(address, &[1]).is_err());
    assert_eq!(texture_discard_call(&mut runtime, &vb, 2, &[]), 0);
    assert!(runtime.memory.read(address, 4).is_err());
    for args in [
        [0, 4, 4],
        [0, 5, 2],
        [0, 6, 3],
        [0, 13, 1],
        [0, 16, 2],
        [0, 24, 2],
    ] {
        assert_eq!(texture_discard_call(&mut runtime, &device, 63, &args), 0);
        assert_eq!(
            texture_discard_call(
                &mut runtime,
                &device,
                62,
                &[args[0], args[1], D3D9_FIXTURE_OUTPUT.0]
            ),
            0
        );
        assert_eq!(
            runtime.memory.read(D3D9_FIXTURE_OUTPUT, 4).unwrap(),
            args[2].to_le_bytes()
        );
    }
    assert_eq!(
        texture_discard_call(
            &mut runtime,
            &device,
            27,
            &[37, 19, 22, D3D9_FIXTURE_OUTPUT.0]
        ),
        0
    );
    let surface = texture_discard_output_interface(&runtime);
    assert_eq!(surface.iid, crate::d3d8::SURFACE8_IID);
    assert_eq!(
        texture_discard_call(&mut runtime, &surface, 8, &[D3D9_FIXTURE_OUTPUT.0]),
        0
    );
    assert_eq!(
        runtime.memory.read(D3D9_FIXTURE_OUTPUT, 32).unwrap(),
        [22u32, 1, 0, 2, 37 * 19 * 4, 0, 37, 19]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    let graphics_before = runtime.graphics.clone();
    texture_discard_prepare(&mut runtime, &surface, &[0xfffffff0, 0, 0]);
    assert!(matches!(
        dispatch_d3d9_fixture_basic(&mut runtime, &surface, 9, D3D9_FIXTURE_CALL_SITE),
        ComDispatchOutcome::Fault(_)
    ));
    assert_eq!(runtime.graphics, graphics_before);
    assert_eq!(
        texture_discard_call(&mut runtime, &surface, 9, &[D3D9_FIXTURE_OUTPUT.0, 0, 0]),
        0
    );
    let lock = runtime.memory.read(D3D9_FIXTURE_OUTPUT, 8).unwrap();
    assert_eq!(&lock[..4], &148u32.to_le_bytes());
    let bits = GuestAddress(u32::from_le_bytes(lock[4..8].try_into().unwrap()));
    runtime.memory.write(bits, &[1, 2, 3, 4]).unwrap();
    assert_eq!(texture_discard_call(&mut runtime, &surface, 10, &[]), 0);
    let before_read = runtime
        .graphics_texture_for_com_object(surface.object_id)
        .unwrap()
        .content
        .content_generation;
    assert_eq!(
        texture_discard_call(&mut runtime, &surface, 9, &[D3D9_FIXTURE_OUTPUT.0, 0, 0x10]),
        0
    );
    assert_eq!(runtime.memory.read(bits, 4).unwrap(), [1, 2, 3, 4]);
    assert!(runtime.memory.write(bits, &[0; 4]).is_err());
    assert_eq!(texture_discard_call(&mut runtime, &surface, 10, &[]), 0);
    assert_eq!(
        runtime
            .graphics_texture_for_com_object(surface.object_id)
            .unwrap()
            .content
            .content_generation,
        before_read
    );
    assert_eq!(
        texture_discard_call(&mut runtime, &device, 16, &[0, 0, D3D9_FIXTURE_OUTPUT.0]),
        0
    );
    let back = texture_discard_output_interface(&runtime);
    texture_discard_prepare(&mut runtime, &back, &[D3D9_FIXTURE_OUTPUT.0, 0, 0]);
    assert!(matches!(
        dispatch_d3d9_fixture_basic(&mut runtime, &back, 9, D3D9_FIXTURE_CALL_SITE),
        ComDispatchOutcome::Fault(_)
    ));
    assert_eq!(
        texture_discard_call(&mut runtime, &device, 16, &[0, 0, D3D9_FIXTURE_OUTPUT.0]),
        0
    );
    assert_eq!(
        texture_discard_output_interface(&runtime).guest_pointer,
        back.guest_pointer
    );
    assert_eq!(texture_discard_call(&mut runtime, &back, 2, &[]), 1);
    let before_copy = runtime.graphics.clone();
    write_d3d9_fixture_words(
        &mut runtime.memory,
        D3D9_FIXTURE_PRESENTATION,
        &[0, 0, 38, 19],
    );
    assert_eq!(
        texture_discard_call(
            &mut runtime,
            &device,
            28,
            &[
                surface.guest_pointer.0,
                D3D9_FIXTURE_PRESENTATION.0,
                1,
                back.guest_pointer.0,
                0
            ]
        ),
        crate::d3d9::D3D9_D3DERR_INVALIDCALL.bits()
    );
    assert_eq!(runtime.graphics, before_copy);
    assert_eq!(
        texture_discard_call(
            &mut runtime,
            &device,
            28,
            &[surface.guest_pointer.0, 0, 0, back.guest_pointer.0, 0]
        ),
        0
    );
    let frame = &runtime
        .graphics_device_by_com_object(device.object_id)
        .unwrap()
        .back_buffer;
    assert_eq!(&frame.draw_submissions[0].texture_data[..4], &[1, 2, 3, 4]);
    assert_eq!(frame.draw_submissions[0].depth, None);
    assert_eq!(texture_discard_call(&mut runtime, &back, 2, &[]), 0);
    assert_eq!(texture_discard_call(&mut runtime, &surface, 2, &[]), 0);
    assert!(runtime.memory.read(bits, 4).is_err());
    assert!(
        !runtime
            .graphics_device_by_com_object(device.object_id)
            .unwrap()
            .presentation
            .windowed
    );
    let matrix: [u32; 16] = std::array::from_fn(|i| (i as f32 / 10.0).to_bits());
    write_d3d9_fixture_words(&mut runtime.memory, D3D9_FIXTURE_PRESENTATION, &matrix);
    assert_eq!(
        texture_discard_call(&mut runtime, &device, 37, &[2, D3D9_FIXTURE_PRESENTATION.0]),
        0
    );
    assert_eq!(
        texture_discard_call(&mut runtime, &device, 38, &[2, D3D9_FIXTURE_OUTPUT.0]),
        0
    );
    assert_eq!(
        runtime.memory.read(D3D9_FIXTURE_OUTPUT, 64).unwrap(),
        matrix
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        texture_discard_call(
            &mut runtime,
            &device,
            36,
            &[0, 0, 3, 0xff123456, 1.0f32.to_bits(), 0]
        ),
        0
    );
    let state = runtime
        .graphics_device_by_com_object(device.object_id)
        .unwrap();
    assert_eq!(state.back_buffer.clear_color, Some(0xff123456));
    assert_eq!(state.back_buffer.depth_clears.len(), 1);
    assert_eq!(state.back_buffer.depth_clears[0].depth, 1.0f32.to_bits());
    assert_eq!(
        texture_discard_call(
            &mut runtime,
            &device,
            36,
            &[0, 0, 2, 0, f32::NAN.to_bits(), 0]
        ),
        crate::d3d9::D3D9_D3DERR_INVALIDCALL.bits()
    );
}

#[test]
fn d3d9_windowed_back_buffer_size_is_independent_of_client_size() {
    for (width, height, expected_width, expected_height) in [
        (1920, 1080, 1920, 1080),
        (0, 1080, 800, 1080),
        (1280, 0, 1280, 600),
    ] {
        let mut runtime = d3d9_fixture_runtime(d3d9_fixture_memory());
        let window = seed_created_d3d9_window(&mut runtime);
        let root = runtime.install_com_object(direct3d9_object_spec()).unwrap();
        let interface = runtime
            .com()
            .interface_for_object_iid(root.object_id, D3D9_IDIRECT3D9_IID)
            .unwrap()
            .clone();
        acknowledge_d3d9_fixture_memory(&mut runtime.memory);
        write_d3d9_fixture_words(
            &mut runtime.memory,
            D3D9_FIXTURE_PRESENTATION,
            &[
                width,
                height,
                0,
                0,
                0,
                0,
                3,
                window.handle,
                1,
                0,
                0,
                0,
                0,
                0x8000_0000,
            ],
        );
        let arguments = [
            0,
            1,
            window.handle,
            crate::d3d9::D3D9_CREATE_SOFTWARE_VERTEX_PROCESSING_MULTITHREADED,
            D3D9_FIXTURE_PRESENTATION.0,
            D3D9_FIXTURE_OUTPUT.0,
        ];
        let mut invalid = arguments;
        invalid[5] = 0xffff_f000;
        texture_discard_prepare(&mut runtime, &interface, &invalid);
        let objects = runtime.com().objects().len();
        let original = runtime.memory.read(D3D9_FIXTURE_PRESENTATION, 56).unwrap();
        assert!(matches!(
            dispatch_d3d9_fixture_basic(&mut runtime, &interface, 16, D3D9_FIXTURE_CALL_SITE),
            ComDispatchOutcome::Fault(_)
        ));
        assert_eq!(runtime.com().objects().len(), objects);
        assert_eq!(
            runtime.memory.read(D3D9_FIXTURE_PRESENTATION, 56).unwrap(),
            original
        );
        assert_eq!(
            texture_discard_call(&mut runtime, &interface, 16, &arguments),
            COM_S_OK.bits()
        );
        let device = texture_discard_output_interface(&runtime);
        let state = runtime
            .graphics_device_by_com_object(device.object_id)
            .unwrap();
        assert_eq!(
            (state.presentation.width, state.presentation.height),
            (expected_width, expected_height)
        );
        assert_eq!(
            runtime.window(window.handle).unwrap().creation.geometry,
            window.creation.geometry
        );
        let expected_prefix = [expected_width, expected_height, D3D9_D3DFMT_X8R8G8B8, 1]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(
            runtime.memory.read(D3D9_FIXTURE_PRESENTATION, 16).unwrap(),
            expected_prefix
        );
        assert_eq!(
            runtime
                .graphics_device_by_com_object(device.object_id)
                .unwrap()
                .presentation
                .present_mode,
            crate::runtime::RuntimeGraphicsPresentMode::Copy
        );
        assert_eq!(
            texture_discard_call(
                &mut runtime,
                &device,
                43,
                &[0, 0, 1, 0xff12_3456, 1_f32.to_bits(), 0]
            ),
            0
        );
        write_d3d9_fixture_words(
            &mut runtime.memory,
            D3D9_FIXTURE_PRESENTATION,
            &[0, 0, 800, 600],
        );
        let present = [
            D3D9_FIXTURE_PRESENTATION.0,
            D3D9_FIXTURE_PRESENTATION.0,
            window.handle,
            0,
        ];
        for _ in 0..2 {
            assert_eq!(texture_discard_call(&mut runtime, &device, 17, &present), 0);
            let id = runtime
                .graphics_device_by_com_object(device.object_id)
                .unwrap()
                .id;
            let (width, height, frame) = runtime.presented_graphics_frame(id).unwrap();
            assert_eq!(
                (width, height, frame.clear_color),
                (800, 600, Some(0xff12_3456))
            );
        }
        let before = runtime.graphics.clone();
        let mut invalid = present;
        invalid[0] = 0xffff_fff0;
        texture_discard_prepare(&mut runtime, &device, &invalid);
        assert!(matches!(
            dispatch_d3d9_fixture_basic(&mut runtime, &device, 17, D3D9_FIXTURE_CALL_SITE),
            ComDispatchOutcome::Fault(_)
        ));
        assert_eq!(runtime.graphics, before);
        write_d3d9_fixture_words(
            &mut runtime.memory,
            D3D9_FIXTURE_PRESENTATION,
            &[0, 0, 0, 600],
        );
        assert_eq!(
            texture_discard_call(&mut runtime, &device, 17, &present),
            crate::d3d9::D3D9_D3DERR_INVALIDCALL.bits()
        );
        assert_eq!(runtime.graphics, before);
    }
}

#[test]
fn d3d9_user_memory_quad_uses_point_clamp_and_checked_snapshots() {
    let (mut runtime, device) = texture_discard_fixture();
    let texture =
        texture_discard_create(&mut runtime, &device, crate::d3d9::D3D9_D3DUSAGE_DYNAMIC, 0);
    assert_eq!(
        texture_discard_call(
            &mut runtime,
            &texture,
            19,
            &[0, D3D9_FIXTURE_OUTPUT.0, 0, 0x8000]
        ),
        0
    );
    let bits = GuestAddress(u32::from_le_bytes(
        runtime
            .memory
            .read(GuestAddress(D3D9_FIXTURE_OUTPUT.0 + 4), 4)
            .unwrap()
            .try_into()
            .unwrap(),
    ));
    runtime.memory.write(bits, &[0x5a; 24]).unwrap();
    assert_eq!(texture_discard_call(&mut runtime, &texture, 20, &[0]), 0);
    for args in [[0, 5, 1], [0, 6, 1], [0, 1, 3], [0, 2, 3]] {
        assert_eq!(texture_discard_call(&mut runtime, &device, 69, &args), 0);
    }
    for args in [[22, 1], [137, 0], [7, 0], [27, 0]] {
        assert_eq!(texture_discard_call(&mut runtime, &device, 57, &args), 0);
    }
    for args in [[0, 1, 2], [0, 2, 2], [0, 4, 1]] {
        assert_eq!(texture_discard_call(&mut runtime, &device, 67, &args), 0);
    }
    assert_eq!(texture_discard_call(&mut runtime, &device, 89, &[0x104]), 0);
    assert_eq!(
        texture_discard_call(&mut runtime, &device, 65, &[0, texture.guest_pointer.0]),
        0
    );
    let vertices = GuestAddress(D3D9_FIXTURE_MEMORY_BASE.0 + 0xa00);
    let data = [
        [-0.5_f32, -0.5, 1., 1., 0., 0.],
        [799.5, -0.5, 1., 1., 1., 0.],
        [-0.5, 599.5, 1., 1., 0., 1.],
        [799.5, 599.5, 1., 1., 1., 1.],
    ]
    .into_iter()
    .flatten()
    .flat_map(f32::to_le_bytes)
    .collect::<Vec<_>>();
    runtime.memory.write(vertices, &data).unwrap();
    write_d3d9_fixture_words(
        &mut runtime.memory,
        D3D9_FIXTURE_PRESENTATION,
        &[32, 16, 384, 448, 0, 1.0f32.to_bits()],
    );
    assert_eq!(
        texture_discard_call(&mut runtime, &device, 47, &[D3D9_FIXTURE_PRESENTATION.0]),
        0
    );
    assert_eq!(texture_discard_call(&mut runtime, &device, 41, &[]), 0);
    let before = runtime.graphics.clone();
    texture_discard_prepare(&mut runtime, &device, &[5, 2, 0xffff_fff0, 24]);
    assert!(matches!(
        dispatch_d3d9_fixture_basic(&mut runtime, &device, 83, D3D9_FIXTURE_CALL_SITE),
        ComDispatchOutcome::Fault(_)
    ));
    assert_eq!(runtime.graphics, before);
    assert_eq!(
        texture_discard_call(&mut runtime, &device, 83, &[5, 2, vertices.0, 24]),
        0
    );
    runtime.memory.write(vertices, &[0; 96]).unwrap();
    assert_eq!(texture_discard_call(&mut runtime, &device, 65, &[0, 0]), 0);
    assert_eq!(texture_discard_call(&mut runtime, &device, 42, &[]), 0);
    assert_eq!(
        texture_discard_call(&mut runtime, &device, 17, &[0, 0, 0, 0]),
        0
    );
    let id = runtime
        .graphics_device_by_com_object(device.object_id)
        .unwrap()
        .id;
    let (_, _, frame) = runtime.presented_graphics_frame(id).unwrap();
    let draw = &frame.draw_submissions[0];
    assert_eq!(
        draw.clip_rect,
        Some(RuntimeRect {
            left: 32,
            top: 16,
            right: 416,
            bottom: 464
        })
    );
    assert_eq!(draw.texture_data.as_slice(), &[0x5a; 24]);
    assert_eq!(&draw.vertex_data[..16], &data[..16]);
    assert_eq!(&draw.vertex_data[16..20], &[255; 4]);
    assert_eq!(
        draw.min_filter,
        crate::runtime::RuntimeGraphicsTextureFilter::Point
    );
    assert_eq!(
        draw.address_v,
        crate::runtime::RuntimeGraphicsTextureAddress::Clamp
    );
}

#[test]
fn d3d9_default_viewport_and_device_caps_validate_guest_buffers() {
    let (mut runtime, device) = texture_discard_fixture();
    let viewport = D3D9_FIXTURE_PRESENTATION;
    write_d3d9_fixture_words(
        &mut runtime.memory,
        viewport,
        &[0, 0, 800, 600, 0, 1.0_f32.to_bits()],
    );
    assert_eq!(
        texture_discard_call(&mut runtime, &device, 47, &[viewport.0]),
        COM_S_OK.bits()
    );
    let mut before = runtime
        .graphics_device_by_com_object(device.object_id)
        .unwrap()
        .clone();
    for words in [
        [0, 0, 801, 600, 0, 1.0_f32.to_bits()],
        [u32::MAX, 0, 800, 600, 0, 1.0_f32.to_bits()],
        [0, 0, 800, 0, 0, 1.0_f32.to_bits()],
        [0, 0, 800, 600, f32::NAN.to_bits(), 1.0_f32.to_bits()],
        [0, 0, 800, 600, 1.0_f32.to_bits(), 0],
    ] {
        write_d3d9_fixture_words(&mut runtime.memory, viewport, &words);
        assert_eq!(
            texture_discard_call(&mut runtime, &device, 47, &[viewport.0]),
            D3D9_D3DERR_INVALIDCALL.bits()
        );
        assert_eq!(
            runtime
                .graphics_device_by_com_object(device.object_id)
                .unwrap(),
            &before
        );
    }
    write_d3d9_fixture_words(
        &mut runtime.memory,
        viewport,
        &[0, 0, 400, 600, 0, 1.0_f32.to_bits()],
    );
    assert_eq!(
        texture_discard_call(&mut runtime, &device, 47, &[viewport.0]),
        0
    );
    assert_eq!(
        runtime
            .graphics_device_by_com_object(device.object_id)
            .unwrap()
            .viewport,
        [0, 0, 400, 600, 0, 1.0_f32.to_bits()]
    );
    before = runtime
        .graphics_device_by_com_object(device.object_id)
        .unwrap()
        .clone();
    texture_discard_prepare(&mut runtime, &device, &[0xffff_fff0]);
    assert!(matches!(
        dispatch_d3d9_fixture_basic(&mut runtime, &device, 47, D3D9_FIXTURE_CALL_SITE),
        ComDispatchOutcome::Fault(_)
    ));
    assert_eq!(
        runtime
            .graphics_device_by_com_object(device.object_id)
            .unwrap(),
        &before
    );
    let output = GuestAddress(D3D9_FIXTURE_MEMORY_BASE.0 + 0xa00);
    runtime.memory.write(output, &[0xcc; 308]).unwrap();
    assert_eq!(
        texture_discard_call(&mut runtime, &device, 7, &[output.0]),
        COM_S_OK.bits()
    );
    assert_eq!(runtime.memory.read(output, 4).unwrap(), 1_u32.to_le_bytes());
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(output.0 + 304), 4)
            .unwrap(),
        [0xcc; 4]
    );
    let bytes = runtime.memory.read(output, 308).unwrap();
    texture_discard_prepare(&mut runtime, &device, &[0xffff_fff0]);
    assert!(matches!(
        dispatch_d3d9_fixture_basic(&mut runtime, &device, 7, D3D9_FIXTURE_CALL_SITE),
        ComDispatchOutcome::Fault(_)
    ));
    assert_eq!(runtime.memory.read(output, 308).unwrap(), bytes);
    assert_eq!(
        runtime
            .graphics_device_by_com_object(device.object_id)
            .unwrap(),
        &before
    );
}

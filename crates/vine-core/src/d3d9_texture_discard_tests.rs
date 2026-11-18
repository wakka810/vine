fn texture_discard_prepare(
    runtime: &mut Runtime,
    interface: &RuntimeComInterfaceRecord,
    args: &[u32],
) {
    let mut words = vec![D3D9_FIXTURE_CALL_SITE.0, interface.guest_pointer.0];
    words.extend_from_slice(args);
    write_d3d9_fixture_words(&mut runtime.memory, D3D9_FIXTURE_STACK, &words);
    acknowledge_d3d9_fixture_memory(&mut runtime.memory);
}

fn texture_discard_call(
    runtime: &mut Runtime,
    interface: &RuntimeComInterfaceRecord,
    slot: u16,
    args: &[u32],
) -> u32 {
    texture_discard_prepare(runtime, interface, args);
    let outcome = dispatch_d3d9_fixture_basic(runtime, interface, slot, D3D9_FIXTURE_CALL_SITE);
    let ComDispatchOutcome::Completed(completion) = outcome else {
        panic!("texture call did not complete: {outcome:?}");
    };
    acknowledge_d3d9_fixture_memory(&mut runtime.memory);
    completion.eax
}

fn texture_discard_output_interface(runtime: &Runtime) -> RuntimeComInterfaceRecord {
    let pointer = GuestAddress(u32::from_le_bytes(
        runtime
            .memory
            .read(D3D9_FIXTURE_OUTPUT, 4)
            .unwrap()
            .try_into()
            .unwrap(),
    ));
    runtime
        .com_interface_for_pointer(pointer)
        .unwrap()
        .unwrap()
        .clone()
}

fn texture_discard_fixture() -> (Runtime, RuntimeComInterfaceRecord) {
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
            0,
            0,
            D3D9_D3DFMT_UNKNOWN,
            1,
            0,
            0,
            D3D9_D3DSWAPEFFECT_DISCARD,
            window.handle,
            1,
            0,
            80,
            0,
            0,
            0,
        ],
    );
    assert_eq!(
        texture_discard_call(
            &mut runtime,
            &interface,
            16,
            &[
                0,
                D3D9_D3DDEVTYPE_HAL,
                window.handle,
                D3D9_CREATE_HARDWARE_VERTEX_PROCESSING_MULTITHREADED,
                D3D9_FIXTURE_PRESENTATION.0,
                D3D9_FIXTURE_OUTPUT.0,
            ]
        ),
        COM_S_OK.bits()
    );
    let device = texture_discard_output_interface(&runtime);
    (runtime, device)
}

fn texture_discard_create(
    runtime: &mut Runtime,
    device: &RuntimeComInterfaceRecord,
    usage: u32,
    pool: u32,
) -> RuntimeComInterfaceRecord {
    assert_eq!(
        texture_discard_call(
            runtime,
            device,
            23,
            &[
                3,
                2,
                1,
                usage,
                D3D9_D3DFMT_X8R8G8B8,
                pool,
                D3D9_FIXTURE_OUTPUT.0,
                0,
            ]
        ),
        COM_S_OK.bits()
    );
    texture_discard_output_interface(runtime)
}

fn texture_discard_rejected(
    runtime: &mut Runtime,
    texture: &RuntimeComInterfaceRecord,
    args: &[u32],
    fault: bool,
) {
    texture_discard_prepare(runtime, texture, args);
    let graphics = runtime.graphics.clone();
    let memory = runtime.memory.current_reflection().unwrap();
    let stats = runtime.memory.stats();
    let allocations = runtime.address_space_allocator.clone();
    let outcome = dispatch_d3d9_fixture_basic(runtime, texture, 19, D3D9_FIXTURE_CALL_SITE);
    if fault {
        assert!(
            matches!(outcome, ComDispatchOutcome::Fault(_)),
            "{outcome:?}"
        );
    } else {
        let ComDispatchOutcome::Completed(completion) = outcome else {
            panic!("invalid texture lock did not return HRESULT: {outcome:?}");
        };
        assert_eq!(completion.eax, D3D9_D3DERR_INVALIDCALL.bits());
    }
    assert_eq!(runtime.graphics, graphics);
    assert_eq!(runtime.memory.current_reflection().unwrap(), memory);
    assert_eq!(runtime.memory.stats(), stats);
    assert_eq!(runtime.address_space_allocator, allocations);
    assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
}

#[test]
fn texture_discard_dynamic_full_lock_preserves_owned_bytes_and_lifetime() {
    use crate::d3d9::{D3D9_D3DLOCK_DISCARD, D3D9_D3DLOCK_NO_DIRTY_UPDATE, D3D9_D3DUSAGE_DYNAMIC};
    let (mut runtime, device) = texture_discard_fixture();
    let texture = texture_discard_create(&mut runtime, &device, D3D9_D3DUSAGE_DYNAMIC, 0);
    let mut previous_generation = 0;
    for (flags, pixels) in [
        (D3D9_D3DLOCK_DISCARD, [0x37; 24]),
        (
            D3D9_D3DLOCK_DISCARD | D3D9_D3DLOCK_NO_DIRTY_UPDATE,
            [0x92; 24],
        ),
    ] {
        assert_eq!(
            texture_discard_call(
                &mut runtime,
                &texture,
                19,
                &[0, D3D9_FIXTURE_OUTPUT.0, 0, flags]
            ),
            COM_S_OK.bits()
        );
        let output = runtime.memory.read(D3D9_FIXTURE_OUTPUT, 8).unwrap();
        assert_eq!(u32::from_le_bytes(output[..4].try_into().unwrap()), 12);
        let bits = GuestAddress(u32::from_le_bytes(output[4..].try_into().unwrap()));
        let backing = runtime
            .graphics_texture_for_com_object(texture.object_id)
            .unwrap()
            .backing
            .unwrap();
        assert_eq!(bits, backing.range.start());
        runtime.memory.write(bits, &pixels).unwrap();
        acknowledge_d3d9_fixture_memory(&mut runtime.memory);
        assert_eq!(
            runtime
                .memory
                .read_owned(bits, 24, backing.memory_owner)
                .unwrap(),
            pixels
        );
        assert_eq!(
            texture_discard_call(&mut runtime, &texture, 20, &[0]),
            COM_S_OK.bits()
        );
        let unlocked = runtime
            .graphics_texture_for_com_object(texture.object_id)
            .unwrap();
        assert!(unlocked.active_lock.is_none());
        assert!(unlocked.content.content_generation > previous_generation);
        previous_generation = unlocked.content.content_generation;
        assert_eq!(
            unlocked.content.pixel_source.unwrap().source_texture_id,
            unlocked.id
        );
        assert_eq!(
            runtime
                .memory
                .read_owned(bits, 24, backing.memory_owner)
                .unwrap(),
            pixels
        );
        assert!(runtime.memory.write(bits, &[1]).is_err());
    }
    assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
}

#[test]
fn texture_discard_invalid_usage_rect_flags_and_outputs_publish_nothing() {
    use crate::d3d9::{D3D9_D3DLOCK_DISCARD, D3D9_D3DUSAGE_DYNAMIC};
    let (mut runtime, device) = texture_discard_fixture();
    let dynamic = texture_discard_create(&mut runtime, &device, D3D9_D3DUSAGE_DYNAMIC, 0);
    let system = texture_discard_create(&mut runtime, &device, 0, D3D9_D3DPOOL_SYSTEMMEM);
    let ordinary = texture_discard_create(&mut runtime, &device, 0, 0);
    write_d3d9_fixture_words(
        &mut runtime.memory,
        D3D9_FIXTURE_OUTPUT,
        &[0xcccc_cccc, 0xdddd_dddd],
    );
    for texture in [&system, &ordinary] {
        texture_discard_rejected(
            &mut runtime,
            texture,
            &[0, D3D9_FIXTURE_OUTPUT.0, 0, D3D9_D3DLOCK_DISCARD],
            false,
        );
    }
    for rect in [[0, 0, 3, 2], [0, 0, 2, 1]] {
        write_d3d9_fixture_words(&mut runtime.memory, D3D9_FIXTURE_PRESENTATION, &rect);
        texture_discard_rejected(
            &mut runtime,
            &dynamic,
            &[
                0,
                D3D9_FIXTURE_OUTPUT.0,
                D3D9_FIXTURE_PRESENTATION.0,
                D3D9_D3DLOCK_DISCARD,
            ],
            false,
        );
    }
    for flags in [0x1000, 0x4000, D3D9_D3DLOCK_DISCARD | 0x10] {
        texture_discard_rejected(
            &mut runtime,
            &dynamic,
            &[0, D3D9_FIXTURE_OUTPUT.0, 0, flags],
            false,
        );
    }
    texture_discard_rejected(
        &mut runtime,
        &dynamic,
        &[0, D3D9_FIXTURE_OUTPUT.0, 0, 0x10],
        false,
    );
    for output in [D3D9_FIXTURE_MEMORY_BASE.0 + 0x2ffc, dynamic.guest_pointer.0] {
        texture_discard_rejected(
            &mut runtime,
            &dynamic,
            &[0, output, 0, D3D9_D3DLOCK_DISCARD],
            true,
        );
    }
    assert_eq!(
        texture_discard_call(
            &mut runtime,
            &dynamic,
            19,
            &[0, D3D9_FIXTURE_OUTPUT.0, 0, D3D9_D3DLOCK_DISCARD]
        ),
        COM_S_OK.bits()
    );
    texture_discard_rejected(
        &mut runtime,
        &dynamic,
        &[0, D3D9_FIXTURE_OUTPUT.0, 0, D3D9_D3DLOCK_DISCARD],
        false,
    );
    assert_eq!(
        texture_discard_call(&mut runtime, &dynamic, 20, &[0]),
        COM_S_OK.bits()
    );
}

#[test]
fn texture_discard_failed_backing_retries_and_dirty_update_follows_lock_flags() {
    use crate::d3d9::{D3D9_D3DLOCK_DISCARD, D3D9_D3DLOCK_NO_DIRTY_UPDATE, D3D9_D3DUSAGE_DYNAMIC};
    use crate::runtime::graphics::RuntimeGraphicsTextureDirtyRegion;
    let (mut runtime, device) = texture_discard_fixture();
    let dynamic = texture_discard_create(&mut runtime, &device, D3D9_D3DUSAGE_DYNAMIC, 0);
    assert_eq!(
        texture_discard_call(
            &mut runtime,
            &dynamic,
            19,
            &[0, D3D9_FIXTURE_OUTPUT.0, 0, D3D9_D3DLOCK_DISCARD]
        ),
        COM_S_OK.bits()
    );
    assert_eq!(
        texture_discard_call(&mut runtime, &dynamic, 20, &[0]),
        COM_S_OK.bits()
    );
    let backing = runtime
        .graphics_texture_for_com_object(dynamic.object_id)
        .unwrap()
        .backing
        .unwrap();
    runtime
        .memory
        .decommit(backing.memory_owner, backing.range)
        .unwrap();
    texture_discard_rejected(
        &mut runtime,
        &dynamic,
        &[0, D3D9_FIXTURE_OUTPUT.0, 0, D3D9_D3DLOCK_DISCARD],
        true,
    );
    runtime
        .memory
        .commit(
            backing.memory_owner,
            backing.range,
            PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .unwrap();
    assert_eq!(
        texture_discard_call(
            &mut runtime,
            &dynamic,
            19,
            &[0, D3D9_FIXTURE_OUTPUT.0, 0, D3D9_D3DLOCK_DISCARD]
        ),
        COM_S_OK.bits()
    );
    assert_eq!(
        texture_discard_call(&mut runtime, &dynamic, 20, &[0]),
        COM_S_OK.bits()
    );

    let source = texture_discard_create(&mut runtime, &device, 0, D3D9_D3DPOOL_SYSTEMMEM);
    let destination = texture_discard_create(&mut runtime, &device, 0, 0);
    for (flags, expected_dirty, value) in [
        (0, RuntimeGraphicsTextureDirtyRegion::WholeTexture, 0x11),
        (
            D3D9_D3DLOCK_NO_DIRTY_UPDATE,
            RuntimeGraphicsTextureDirtyRegion::Clean,
            0x22,
        ),
        (0, RuntimeGraphicsTextureDirtyRegion::WholeTexture, 0x33),
    ] {
        assert_eq!(
            texture_discard_call(
                &mut runtime,
                &source,
                19,
                &[0, D3D9_FIXTURE_OUTPUT.0, 0, flags]
            ),
            COM_S_OK.bits()
        );
        let backing = runtime
            .graphics_texture_for_com_object(source.object_id)
            .unwrap()
            .backing
            .unwrap();
        runtime
            .memory
            .write(backing.range.start(), &[value; 24])
            .unwrap();
        assert_eq!(
            texture_discard_call(&mut runtime, &source, 20, &[0]),
            COM_S_OK.bits()
        );
        assert_eq!(
            runtime
                .graphics_texture_for_com_object(source.object_id)
                .unwrap()
                .dirty_region,
            expected_dirty
        );
        if flags == 0 {
            assert_eq!(
                texture_discard_call(
                    &mut runtime,
                    &device,
                    31,
                    &[source.guest_pointer.0, destination.guest_pointer.0]
                ),
                COM_S_OK.bits()
            );
            assert_eq!(
                runtime
                    .graphics_texture_for_com_object(source.object_id)
                    .unwrap()
                    .dirty_region,
                RuntimeGraphicsTextureDirtyRegion::Clean
            );
            let pixels = runtime
                .graphics_texture_for_com_object(destination.object_id)
                .unwrap()
                .content
                .pixel_source
                .unwrap();
            assert_eq!(
                runtime
                    .memory
                    .read_owned(pixels.range.start(), 24, backing.memory_owner)
                    .unwrap(),
                [value; 24]
            );
        }
    }
    assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);

    // Releasing the system-memory staging texture before the default-pool GPU
    // texture succeeds and preserves the destination texture's copied texels.
    assert_eq!(texture_discard_call(&mut runtime, &source, 2, &[]), 0);
    let destination_after_source_release = runtime
        .graphics_texture_for_com_object(destination.object_id)
        .unwrap();
    assert!(destination_after_source_release.content.pixel_source.is_none());
    let retained = destination_after_source_release
        .content
        .retained_pixels
        .as_ref()
        .expect("retained destination pixels");
    assert_eq!(retained.row_pitch, 12);
    assert_eq!(retained.data.as_slice(), &[0x33; 24]);
    assert_eq!(texture_discard_call(&mut runtime, &destination, 2, &[]), 0);
}

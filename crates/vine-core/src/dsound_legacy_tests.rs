#[test]
fn dsound_secondary_pcm_controls_are_optional_and_capability_checked() {
    use crate::dsound::{DSOUND_IDIRECTSOUND8_IID, direct_sound_object_spec};
    let mut runtime = d3d9_fixture_runtime(d3d9_fixture_memory());
    let window = seed_created_d3d9_window(&mut runtime);
    let root = runtime
        .install_com_object(direct_sound_object_spec(false))
        .unwrap();
    let root = runtime
        .com()
        .interface_for_object_iid(root.object_id, DSOUND_IDIRECTSOUND8_IID)
        .unwrap()
        .clone();
    acknowledge_d3d9_fixture_memory(&mut runtime.memory);
    assert_eq!(
        texture_discard_call(&mut runtime, &root, 6, &[window.handle, 2]),
        0
    );
    let format = D3D9_FIXTURE_PRESENTATION;
    let descriptor = GuestAddress(format.0 + 64);
    runtime
        .memory
        .write(
            format,
            &[
                1, 0, 2, 0, 0x44, 0xac, 0, 0, 0x10, 0xb1, 2, 0, 4, 0, 16, 0, 0, 0,
            ],
        )
        .unwrap();
    for flags in [0, 8, 0x8008, 0x8088] {
        write_d3d9_fixture_words(
            &mut runtime.memory,
            descriptor,
            &[20, flags, 32768, 0, format.0],
        );
        assert_eq!(
            texture_discard_call(
                &mut runtime,
                &root,
                3,
                &[descriptor.0, D3D9_FIXTURE_OUTPUT.0, 0]
            ),
            0
        );
        let buffer = texture_discard_output_interface(&runtime);
        assert_eq!(
            runtime
                .audio_buffer_for_com_object(buffer.object_id)
                .unwrap()
                .descriptor
                .flags,
            flags
        );
        let before = runtime.audio.clone();
        assert_eq!(
            texture_discard_call(&mut runtime, &buffer, 17, &[22050]),
            0x8878001e
        );
        assert_eq!(runtime.audio, before);
        assert_eq!(
            texture_discard_call(&mut runtime, &buffer, 15, &[0]),
            if flags & 0x80 == 0 { 0x8878001e } else { 0 }
        );
        if flags == 0x8088 {
            let out = D3D9_FIXTURE_OUTPUT;
            assert_eq!(
                texture_discard_call(
                    &mut runtime,
                    &buffer,
                    11,
                    &[0, 32768, out.0, out.0 + 4, out.0 + 8, out.0 + 12, 0]
                ),
                0
            );
            let backing = runtime
                .audio_buffer_for_com_object(buffer.object_id)
                .unwrap()
                .backing
                .unwrap();
            runtime
                .memory
                .write(backing.range.start(), &[1, 2, 3, 4])
                .unwrap();
            assert_eq!(
                texture_discard_call(
                    &mut runtime,
                    &buffer,
                    19,
                    &[backing.range.start().0, 32768, 0, 0]
                ),
                0
            );
            let before = runtime.audio.clone();
            let before_com = runtime.com.objects().len();
            texture_discard_prepare(&mut runtime, &root, &[buffer.guest_pointer.0, 0xfffffff0]);
            assert!(matches!(
                dispatch_d3d9_fixture_basic(&mut runtime, &root, 5, D3D9_FIXTURE_CALL_SITE),
                ComDispatchOutcome::Fault(_)
            ));
            assert_eq!(runtime.audio, before);
            assert_eq!(runtime.com.objects().len(), before_com);
            assert_eq!(
                texture_discard_call(&mut runtime, &root, 5, &[buffer.guest_pointer.0, out.0]),
                0
            );
            let duplicate = texture_discard_output_interface(&runtime);
            assert_ne!(duplicate.object_id, buffer.object_id);
            assert_eq!(
                runtime
                    .audio_buffer_for_com_object(duplicate.object_id)
                    .unwrap()
                    .backing,
                Some(backing)
            );
            assert_eq!(
                texture_discard_call(&mut runtime, &duplicate, 15, &[(-100i32) as u32]),
                0
            );
            assert_eq!(
                runtime
                    .audio_buffer_for_com_object(buffer.object_id)
                    .unwrap()
                    .controls
                    .volume_hundredths_db,
                0
            );
            assert_eq!(
                texture_discard_call(
                    &mut runtime,
                    &duplicate,
                    11,
                    &[0, 4, out.0, out.0 + 4, out.0 + 8, out.0 + 12, 0]
                ),
                0
            );
            runtime
                .memory
                .write(backing.range.start(), &[5, 6, 7, 8])
                .unwrap();
            assert_eq!(
                texture_discard_call(
                    &mut runtime,
                    &duplicate,
                    19,
                    &[backing.range.start().0, 4, 0, 0]
                ),
                0
            );
            assert_eq!(
                runtime.memory.read(backing.range.start(), 4).unwrap(),
                &[5, 6, 7, 8]
            );
            assert_eq!(texture_discard_call(&mut runtime, &buffer, 2, &[]), 0);
            assert_eq!(
                runtime.memory.read(backing.range.start(), 4).unwrap(),
                &[5, 6, 7, 8]
            );
            assert!(
                runtime
                    .set_audio_buffer_play(
                        duplicate.object_id,
                        RuntimeAudioPlaybackState::PlayingOnce
                    )
                    .unwrap()
            );
            assert_eq!(texture_discard_call(&mut runtime, &duplicate, 2, &[]), 0);
            assert!(runtime.memory.read(backing.range.start(), 4).is_err());
        }
    }
}

#[test]
fn dsound_legacy_primary_descriptor_and_format_outputs_are_bounded() {
    use crate::dsound::{
        DSOUND_IDIRECTSOUND_IID, DSOUND_IDIRECTSOUNDBUFFER_IID, direct_sound_object_spec,
    };
    let mut runtime = d3d9_fixture_runtime(d3d9_fixture_memory());
    let window = seed_created_d3d9_window(&mut runtime);
    let installed = runtime
        .install_com_object(direct_sound_object_spec(true))
        .unwrap();
    let root = runtime
        .com()
        .interface_for_object_iid(installed.object_id, DSOUND_IDIRECTSOUND_IID)
        .unwrap()
        .clone();
    acknowledge_d3d9_fixture_memory(&mut runtime.memory);
    assert_eq!(
        texture_discard_call(&mut runtime, &root, 6, &[window.handle, 2]),
        0
    );
    assert_eq!(
        texture_discard_call(&mut runtime, &root, 8, &[D3D9_FIXTURE_OUTPUT.0]),
        0
    );
    assert_eq!(
        runtime.memory.read(D3D9_FIXTURE_OUTPUT, 4).unwrap(),
        0x0014_0004_u32.to_le_bytes()
    );
    // A DSBUFFERDESC1 at the end of its mapping must not read a trailing GUID.
    let descriptor = GuestAddress(D3D9_FIXTURE_MEMORY_BASE.0 + 0x3000 - 20);
    write_d3d9_fixture_words(&mut runtime.memory, descriptor, &[20, 9, 0, 0, 0]);
    assert_eq!(
        texture_discard_call(
            &mut runtime,
            &root,
            3,
            &[descriptor.0, D3D9_FIXTURE_OUTPUT.0, 0]
        ),
        0
    );
    let buffer = texture_discard_output_interface(&runtime);
    assert_eq!(buffer.iid, DSOUND_IDIRECTSOUNDBUFFER_IID);
    assert_eq!(buffer.methods.len(), 21);
    let input = D3D9_FIXTURE_PRESENTATION;
    let mut format = vec![0; 40];
    format[..2].copy_from_slice(&0xfffe_u16.to_le_bytes());
    format[2..4].copy_from_slice(&2_u16.to_le_bytes());
    format[4..8].copy_from_slice(&48_000_u32.to_le_bytes());
    format[8..12].copy_from_slice(&192_000_u32.to_le_bytes());
    format[12..14].copy_from_slice(&4_u16.to_le_bytes());
    format[14..16].copy_from_slice(&16_u16.to_le_bytes());
    format[16..18].copy_from_slice(&22_u16.to_le_bytes());
    format[18..20].copy_from_slice(&16_u16.to_le_bytes());
    format[20..24].copy_from_slice(&3_u32.to_le_bytes());
    format[24..].copy_from_slice(&[1, 0, 0, 0, 0, 0, 16, 0, 128, 0, 0, 170, 0, 56, 155, 113]);
    runtime.memory.write(input, &format).unwrap();
    assert_eq!(
        texture_discard_call(&mut runtime, &buffer, 14, &[input.0]),
        0
    );
    let output = GuestAddress(D3D9_FIXTURE_MEMORY_BASE.0 + 0xa00);
    let written = GuestAddress(output.0 + 48);
    runtime.memory.write(output, &[0xcc; 52]).unwrap();
    assert_eq!(
        texture_discard_call(&mut runtime, &buffer, 5, &[0, 0, written.0]),
        0
    );
    assert_eq!(
        runtime.memory.read(written, 4).unwrap(),
        18_u32.to_le_bytes()
    );
    for capacity in [18, 40] {
        runtime.memory.write(output, &[0xcc; 40]).unwrap();
        assert_eq!(
            texture_discard_call(&mut runtime, &buffer, 5, &[output.0, capacity, written.0]),
            0
        );
        let mut expected = format[..16].to_vec();
        expected[..2].copy_from_slice(&1_u16.to_le_bytes());
        expected.extend_from_slice(&[0, 0]);
        let length = capacity.min(18) as usize;
        assert_eq!(
            runtime.memory.read(output, length).unwrap(),
            expected[..length]
        );
        assert_eq!(
            runtime
                .memory
                .read(GuestAddress(output.0 + length as u32), 40 - length)
                .unwrap(),
            vec![0xcc; 40 - length]
        );
    }
    for capacity in [0, 8, 16, 17] {
        runtime.memory.write(output, &[0xcc; 52]).unwrap();
        assert_eq!(
            texture_discard_call(&mut runtime, &buffer, 5, &[output.0, capacity, written.0]),
            0x8007_0057
        );
        assert_eq!(runtime.memory.read(output, 52).unwrap(), vec![0xcc; 52]);
    }
    let before_audio = runtime.audio.clone();
    let before_output = runtime.memory.read(output, 40).unwrap();
    texture_discard_prepare(&mut runtime, &buffer, &[output.0, 18, 0xffff_fff0]);
    assert!(matches!(
        dispatch_d3d9_fixture_basic(&mut runtime, &buffer, 5, D3D9_FIXTURE_CALL_SITE),
        ComDispatchOutcome::Fault(_)
    ));
    assert_eq!(runtime.memory.read(output, 40).unwrap(), before_output);
    assert_eq!(runtime.audio, before_audio);
    format[24] = 3; // IEEE float is outside the integer PCM device format.
    runtime.memory.write(input, &format).unwrap();
    assert_eq!(
        texture_discard_call(&mut runtime, &buffer, 14, &[input.0]),
        0x8878_0064
    );
    assert_eq!(runtime.audio, before_audio);
}

#[test]
fn dsound_playing_cursors_follow_time_frequency_and_wrap_with_atomic_outputs() {
    use super::super::audio::RuntimeAudioBufferControlUpdate;
    use crate::dsound::{DSOUND_IDIRECTSOUND_IID, direct_sound_object_spec};
    let mut runtime = d3d9_fixture_runtime(d3d9_fixture_memory());
    let window = seed_created_d3d9_window(&mut runtime);
    let installed = runtime
        .install_com_object(direct_sound_object_spec(true))
        .unwrap();
    let root = runtime
        .com()
        .interface_for_object_iid(installed.object_id, DSOUND_IDIRECTSOUND_IID)
        .unwrap()
        .clone();
    acknowledge_d3d9_fixture_memory(&mut runtime.memory);
    assert_eq!(
        texture_discard_call(&mut runtime, &root, 6, &[window.handle, 2]),
        0
    );
    let format = D3D9_FIXTURE_PRESENTATION;
    let descriptor = GuestAddress(format.0 + 64);
    let output = GuestAddress(format.0 + 128);
    let mut pcm = Vec::new();
    pcm.extend_from_slice(&1_u16.to_le_bytes());
    pcm.extend_from_slice(&1_u16.to_le_bytes());
    pcm.extend_from_slice(&48_000_u32.to_le_bytes());
    pcm.extend_from_slice(&96_000_u32.to_le_bytes());
    pcm.extend_from_slice(&2_u16.to_le_bytes());
    pcm.extend_from_slice(&16_u16.to_le_bytes());
    pcm.extend_from_slice(&0_u16.to_le_bytes());
    runtime.memory.write(format, &pcm).unwrap();
    write_d3d9_fixture_words(
        &mut runtime.memory,
        descriptor,
        &[20, 0x181e8, 96_000, 0, format.0],
    );
    assert_eq!(
        texture_discard_call(
            &mut runtime,
            &root,
            3,
            &[descriptor.0, D3D9_FIXTURE_OUTPUT.0, 0]
        ),
        0
    );
    let buffer = texture_discard_output_interface(&runtime);
    let object = buffer.object_id;
    assert!(
        runtime
            .lock_audio_buffer(
                object,
                0,
                96_000,
                0,
                [0, 4, 8, 12].map(|offset| GuestAddress(output.0 + offset))
            )
            .unwrap()
    );
    let lock = runtime
        .audio_buffer_for_com_object(object)
        .unwrap()
        .active_locks[0];
    assert!(
        runtime
            .unlock_audio_buffer(object, lock.first_segment, lock.second_segment)
            .unwrap()
    );
    acknowledge_d3d9_fixture_memory(&mut runtime.memory);
    let BrowserAudioPreparation::Pending(play) = runtime
        .prepare_browser_audio_play(object, RuntimeAudioPlaybackState::PlayingLooping)
        .unwrap()
    else {
        panic!("initial PCM upload");
    };
    assert_eq!(runtime.complete_browser_audio(&play, true).unwrap(), 0);
    let start = runtime.process.host_elapsed_time_100ns();
    for (elapsed, expected) in [
        (0, [0u32, 1440]),
        (2_500_000, [24_000, 25_440]),
        (9_900_000, [95_040, 480]),
    ] {
        runtime
            .synchronize_scheduled_clock_for_host_execution(0, start + elapsed)
            .unwrap();
        assert_eq!(
            texture_discard_call(&mut runtime, &buffer, 4, &[output.0, output.0 + 4]),
            0
        );
        assert_eq!(
            runtime.memory.read(output, 8).unwrap(),
            expected
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>()
        );
    }
    runtime
        .set_audio_buffer_control(object, RuntimeAudioBufferControlUpdate::Frequency(24_000))
        .unwrap();
    runtime
        .synchronize_scheduled_clock_for_host_execution(0, start + 10_100_000)
        .unwrap();
    assert_eq!(
        texture_discard_call(&mut runtime, &buffer, 4, &[output.0, output.0 + 4]),
        0
    );
    assert_eq!(
        runtime.memory.read(output, 8).unwrap(),
        [0u32, 720]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    );
    runtime.memory.write(output, &[0xcc; 8]).unwrap();
    texture_discard_prepare(&mut runtime, &buffer, &[output.0, 0xffff_fff0]);
    let before = runtime.audio.clone();
    assert!(matches!(
        dispatch_d3d9_fixture_basic(&mut runtime, &buffer, 4, D3D9_FIXTURE_CALL_SITE),
        ComDispatchOutcome::Fault(_)
    ));
    assert_eq!(runtime.memory.read(output, 8).unwrap(), vec![0xcc; 8]);
    assert_eq!(runtime.audio, before);
    runtime
        .set_audio_buffer_play(object, RuntimeAudioPlaybackState::Stopped)
        .unwrap();
    runtime
        .synchronize_scheduled_clock_for_host_execution(0, start + 20_100_000)
        .unwrap();
    assert_eq!(
        texture_discard_call(&mut runtime, &buffer, 4, &[output.0, output.0 + 4]),
        0
    );
    assert_eq!(runtime.memory.read(output, 8).unwrap(), vec![0; 8]);
}

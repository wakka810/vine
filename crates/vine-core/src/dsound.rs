use crate::api::{
    ApiCallFrame, ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiSymbol,
};
use crate::com::{
    COM_S_OK, ComDispatchOutcome, ComFrontendCall, ComHResult, ComInterfaceSpec, ComMethodSpec,
    ComObjectSpec,
};
use crate::memory::{GuestAddress, MemoryAccess};
use crate::runtime::{
    BrowserAudioPreparation, RUNTIME_AUDIO_NOTIFICATION_LIMIT, RUNTIME_AUDIO_PAN_MAX_HUNDREDTHS_DB,
    RUNTIME_AUDIO_PAN_MIN_HUNDREDTHS_DB, RUNTIME_AUDIO_PCM_MAX_CHANNELS,
    RUNTIME_AUDIO_PCM_MAX_SAMPLES_PER_SECOND, RUNTIME_AUDIO_PCM_MIN_SAMPLES_PER_SECOND,
    RUNTIME_AUDIO_PLAYBACK_FREQUENCY_MAX_HZ, RUNTIME_AUDIO_PLAYBACK_FREQUENCY_MIN_HZ,
    RUNTIME_AUDIO_RESOURCE_ARENA_SIZE, RUNTIME_AUDIO_VOLUME_MAX_HUNDREDTHS_DB,
    RUNTIME_AUDIO_VOLUME_MIN_HUNDREDTHS_DB, RuntimeAudioBufferControlUpdate,
    RuntimeAudioBufferDescriptor, RuntimeAudioBufferKind, RuntimeAudioBufferSegment,
    RuntimeAudioNotification, RuntimeAudioPcmFormat, RuntimeAudioPlaybackState, RuntimeComClass,
    RuntimeComIid, RuntimeHandle, RuntimeStateError,
};

pub(crate) const DSOUND_DIRECT_SOUND_CREATE_8_ORDINAL: u16 = 11;
pub(crate) const DSOUND_IDIRECTSOUND8_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0x93, 0x7e, 0x0a, 0xc5, 0x95, 0xf3, 0x34, 0x48, 0x9e, 0xf6, 0x7f, 0xa9, 0x9d, 0xe5, 0x09, 0x66,
]);
pub(crate) const DSOUND_IDIRECTSOUNDBUFFER8_IID: RuntimeComIid =
    RuntimeComIid::from_windows_bytes([
        0x49, 0xa4, 0x25, 0x68, 0x24, 0x75, 0x82, 0x4d, 0x92, 0x0f, 0x50, 0xe3, 0x6a, 0xb3, 0xab,
        0x1e,
    ]);
const DSOUND_IDIRECTSOUNDNOTIFY_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0x83, 0x07, 0x21, 0xb0, 0xcd, 0x89, 0xd0, 0x11, 0xaf, 0x08, 0x00, 0xa0, 0xc9, 0x25, 0xcd, 0x16,
]);
pub(crate) const DSOUND_CLASS_E_NOAGGREGATION: ComHResult = ComHResult(0x8004_0110_u32 as i32);
pub(crate) const DSOUND_E_INVALIDARG: ComHResult = ComHResult(0x8007_0057_u32 as i32);
const DSOUND_DSERR_INVALIDCALL: ComHResult = ComHResult(0x8878_0032_u32 as i32);
pub(crate) const DSOUND_DSSCL_NORMAL: u32 = 1;
pub(crate) const DSOUND_DSSCL_PRIORITY: u32 = 2;
const DSOUND_DSSCL_WRITEPRIMARY: u32 = 4;
pub(crate) const DSOUND_DSBUFFERDESC_BYTES: u32 = 36;
pub(crate) const DSOUND_DSBCAPS_PRIMARYBUFFER: u32 = 0x0000_0001;
const DSOUND_DSBCAPS_CTRLFREQUENCY: u32 = 0x0000_0020;
const DSOUND_DSBCAPS_CTRLPAN: u32 = 0x0000_0040;
const DSOUND_DSBCAPS_CTRLVOLUME: u32 = 0x0000_0080;
const DSOUND_DSBCAPS_CTRLPOSITIONNOTIFY: u32 = 0x0000_0100;
pub(crate) const DSOUND_PRIMARY_BUFFER_FLAGS: u32 = DSOUND_DSBCAPS_PRIMARYBUFFER;
const DSOUND_SECONDARY_BUFFER_FLAGS: u32 = 0x0000_80a8;
pub(crate) const DSOUND_SECONDARY_BUFFER_FORWARD_FLAGS: u32 = 0x0001_81e8;
const DSOUND_SECONDARY_BUFFER_MAX_BYTES: u32 = RUNTIME_AUDIO_RESOURCE_ARENA_SIZE as u32;
const DSOUND_WAVE_FORMAT_PCM: u16 = 1;
pub(crate) const DSOUND_DSBPLAY_LOOPING: u32 = 1;
const DSOUND_DSBSTATUS_PLAYING: u32 = 0x0000_0001;
const DSOUND_DSBSTATUS_LOOPING: u32 = 0x0000_0004;
const DSOUND_DSBFREQUENCY_ORIGINAL: u32 = 0;
const DSOUND_DSERR_CONTROLUNAVAIL: ComHResult = ComHResult(0x8878_001e_u32 as i32);

const PRIMARY_PCM_FORMAT: RuntimeAudioPcmFormat = RuntimeAudioPcmFormat {
    channels: 2,
    samples_per_second: 44_100,
    average_bytes_per_second: 176_400,
    block_align: 4,
    bits_per_sample: 16,
};

pub(crate) fn dispatch_dsound_api(
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let endpoint = &frame.endpoint;
    if endpoint.module.eq_ignore_ascii_case("dsound.dll")
        && matches!(
            endpoint.symbol,
            ApiSymbol::Ordinal {
                ordinal: DSOUND_DIRECT_SOUND_CREATE_8_ORDINAL
            }
        )
    {
        handle_direct_sound_create_8(endpoint, frame, runtime)
    } else {
        ApiHandlerResult::Fault(ApiFault::new(format!(
            "unsupported DirectSound import {:?}",
            endpoint.symbol
        )))
    }
}

pub(crate) fn dispatch_dsound_com(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let root_target = call.class() == RuntimeComClass::DirectSound8
        && call.frame().iid == DSOUND_IDIRECTSOUND8_IID;
    let buffer_class = call.class() == RuntimeComClass::DirectSoundBuffer8;
    let notification_target = buffer_class && call.frame().iid == DSOUND_IDIRECTSOUNDNOTIFY_IID;
    let buffer_target = buffer_class && call.frame().iid == DSOUND_IDIRECTSOUNDBUFFER8_IID;
    if !root_target && !buffer_target && !notification_target {
        return call
            .fault("DirectSound frontend received a COM object outside its owned interfaces");
    }
    if notification_target {
        return match (
            call.frame().vtable_slot,
            call.frame().explicit_arguments.len(),
        ) {
            (3, 2) => handle_set_notification_positions(call),
            _ => call.fault("registered IDirectSoundNotify ABI does not match its method endpoint"),
        };
    }
    match (
        root_target,
        buffer_target,
        call.frame().vtable_slot,
        call.frame().explicit_arguments.len(),
    ) {
        (true, false, 3, 3) => handle_create_sound_buffer(call),
        (true, false, 6, 2) => handle_set_priority_cooperative_level(call),
        (false, true, 4, 2) => handle_buffer_get_current_position(call),
        (false, true, 9, 1) => handle_buffer_get_status(call),
        (false, true, 11, 7) => handle_buffer_lock(call),
        (false, true, 12, 3) => handle_buffer_play(call),
        (false, true, 13, 1) => {
            handle_buffer_control(call, RuntimeAudioBufferControlUpdate::CurrentPosition)
        }
        (false, true, 14, 1) => handle_buffer_set_format(call),
        (false, true, 15, 1) => handle_buffer_control(call, |value| {
            RuntimeAudioBufferControlUpdate::Volume(value as i32)
        }),
        (false, true, 16, 1) => handle_buffer_control(call, |value| {
            RuntimeAudioBufferControlUpdate::Pan(value as i32)
        }),
        (false, true, 17, 1) => handle_buffer_frequency(call),
        (false, true, 18, 0) => handle_buffer_stop(call),
        (false, true, 19, 4) => handle_buffer_unlock(call),
        _ => call.fault("registered DirectSound frontend ABI does not match its method endpoint"),
    }
}

fn handle_create_sound_buffer(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [descriptor_pointer, output_pointer, outer_unknown] =
        call.frame().explicit_arguments.as_slice()
    else {
        unreachable!("frontend ABI preflight fixed the DirectSound argument count");
    };
    let (descriptor_pointer, output_pointer, outer_unknown) =
        (*descriptor_pointer, *output_pointer, *outer_unknown);
    if descriptor_pointer == 0 || output_pointer == 0 {
        return call.complete(DSOUND_E_INVALIDARG.bits());
    }
    if outer_unknown != 0 {
        return call.complete(DSOUND_CLASS_E_NOAGGREGATION.bits());
    }
    let output_address = GuestAddress(output_pointer);
    if let Err(error) = call
        .runtime
        .memory
        .validate_access(output_address, 4, MemoryAccess::Write)
    {
        return call.memory_fault(error);
    }
    let descriptor_bytes = match call.runtime.memory.read(
        GuestAddress(descriptor_pointer),
        DSOUND_DSBUFFERDESC_BYTES as usize,
    ) {
        Ok(bytes) => bytes,
        Err(error) => return call.memory_fault(error),
    };
    let Some(descriptor) = parse_buffer_descriptor(&descriptor_bytes) else {
        return call.complete(DSOUND_E_INVALIDARG.bits());
    };
    if descriptor.size != DSOUND_DSBUFFERDESC_BYTES
        || descriptor.reserved != 0
        || descriptor.algorithm.iter().any(|byte| *byte != 0)
    {
        return call.complete(DSOUND_E_INVALIDARG.bits());
    }

    let logical_descriptor = if descriptor.flags & DSOUND_DSBCAPS_PRIMARYBUFFER != 0 {
        if descriptor.byte_length != 0 || descriptor.format_pointer != 0 {
            return call.complete(DSOUND_E_INVALIDARG.bits());
        }
        if descriptor.flags != DSOUND_PRIMARY_BUFFER_FLAGS {
            return unsupported_buffer_variant(
                call,
                "primary DirectSound buffer flags are unsupported",
            );
        }
        RuntimeAudioBufferDescriptor {
            kind: RuntimeAudioBufferKind::Primary,
            flags: descriptor.flags,
            byte_length: 0,
            format: None,
        }
    } else {
        if descriptor.byte_length == 0 || descriptor.format_pointer == 0 {
            return call.complete(DSOUND_E_INVALIDARG.bits());
        }
        let format_bytes = match call
            .runtime
            .memory
            .read(GuestAddress(descriptor.format_pointer), 16)
        {
            Ok(bytes) => bytes,
            Err(error) => return call.memory_fault(error),
        };
        let Some(format) = parse_pcm_format(&format_bytes) else {
            return call.complete(DSOUND_E_INVALIDARG.bits());
        };
        let whole_pcm_frames = descriptor
            .byte_length
            .is_multiple_of(u32::from(format.block_align));
        if descriptor.flags & DSOUND_SECONDARY_BUFFER_FLAGS != DSOUND_SECONDARY_BUFFER_FLAGS
            || descriptor.flags & !DSOUND_SECONDARY_BUFFER_FORWARD_FLAGS != 0
            || descriptor.byte_length > DSOUND_SECONDARY_BUFFER_MAX_BYTES
            || !(1..=RUNTIME_AUDIO_PCM_MAX_CHANNELS).contains(&format.channels)
            || !(RUNTIME_AUDIO_PCM_MIN_SAMPLES_PER_SECOND
                ..=RUNTIME_AUDIO_PCM_MAX_SAMPLES_PER_SECOND)
                .contains(&format.samples_per_second)
            || !matches!(format.bits_per_sample, 8 | 16)
            || !whole_pcm_frames
        {
            return unsupported_buffer_variant(
                call,
                format!(
                    "secondary DirectSound buffer is outside the bounded PCM band: flags={:#010x}, bytes={}, channels={}, rate={}, bits={}",
                    descriptor.flags,
                    descriptor.byte_length,
                    format.channels,
                    format.samples_per_second,
                    format.bits_per_sample,
                ),
            );
        }
        RuntimeAudioBufferDescriptor {
            kind: RuntimeAudioBufferKind::Secondary,
            flags: descriptor.flags,
            byte_length: descriptor.byte_length,
            format: Some(format),
        }
    };

    let creation = match call
        .runtime
        .validate_audio_buffer_creation(call.frame().object_id, logical_descriptor)
    {
        Ok(plan) => plan,
        Err(error) => return call.runtime_fault(error),
    };
    let parent_interface_id = call.frame().interface_id;
    let installed = match call.runtime.install_com_object_with_guest_writes(
        Some(parent_interface_id),
        direct_sound_buffer8_object_spec(),
        |object| {
            let interface = object
                .interfaces
                .iter()
                .find(|(iid, _)| *iid == DSOUND_IDIRECTSOUNDBUFFER8_IID)
                .ok_or_else(|| {
                    RuntimeStateError::new(
                        "installed DirectSound buffer object has no IDirectSoundBuffer8 interface",
                    )
                })?;
            Ok(vec![(output_address, interface.1.0.to_le_bytes().to_vec())])
        },
    ) {
        Ok(result) => result,
        Err(error) => return call.runtime_fault(error),
    };
    call.runtime
        .insert_audio_buffer(creation, installed.object_id);
    call.complete(COM_S_OK.bits())
}

fn handle_buffer_lock(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [
        offset,
        byte_length,
        data1_output,
        bytes1_output,
        data2_output,
        bytes2_output,
        flags,
    ] = call.frame().explicit_arguments.as_slice()
    else {
        unreachable!("frontend ABI preflight fixed the DirectSound argument count");
    };
    let (offset, byte_length, data1_output, bytes1_output, data2_output, bytes2_output, flags) = (
        *offset,
        *byte_length,
        *data1_output,
        *bytes1_output,
        *data2_output,
        *bytes2_output,
        *flags,
    );
    if [data1_output, bytes1_output, data2_output, bytes2_output].contains(&0) {
        return call.complete(DSOUND_E_INVALIDARG.bits());
    }
    // FROMWRITECURSOR and ENTIREBUFFER are valid modes, but not implemented yet.
    if flags & !3 != 0 {
        return call.complete(DSOUND_E_INVALIDARG.bits());
    }
    if flags != 0 {
        return call.fault("DirectSound Lock flags require an unsupported locking mode");
    }
    let output_addresses = [
        GuestAddress(data1_output),
        GuestAddress(bytes1_output),
        GuestAddress(data2_output),
        GuestAddress(bytes2_output),
    ];
    for address in output_addresses {
        if let Err(error) = call
            .runtime
            .memory
            .validate_access(address, 4, MemoryAccess::Write)
        {
            return call.memory_fault(error);
        }
    }
    let buffer = match call
        .runtime
        .audio_buffer_for_com_object(call.frame().object_id)
    {
        Ok(buffer) => buffer,
        Err(error) => return call.runtime_fault(error),
    };
    if buffer.descriptor.kind != RuntimeAudioBufferKind::Secondary
        || offset >= buffer.descriptor.byte_length
        || byte_length == 0
        || byte_length > buffer.descriptor.byte_length
        || buffer.active_format.is_some_and(|format| {
            !offset.is_multiple_of(u32::from(format.block_align))
                || !byte_length.is_multiple_of(u32::from(format.block_align))
        })
    {
        return call.complete(DSOUND_E_INVALIDARG.bits());
    }
    let buffer_com_object_id = call.frame().object_id;
    match call.runtime.lock_audio_buffer(
        buffer_com_object_id,
        offset,
        byte_length,
        flags,
        output_addresses,
    ) {
        Ok(true) => call.complete(COM_S_OK.bits()),
        Ok(false) => call.complete(DSOUND_E_INVALIDARG.bits()),
        Err(error) => call.runtime_fault(error),
    }
}

fn handle_buffer_get_status(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let output_pointer = call.frame().explicit_arguments[0];
    if output_pointer == 0 {
        return call.complete(DSOUND_E_INVALIDARG.bits());
    }
    let buffer = match call
        .runtime
        .audio_buffer_for_com_object(call.frame().object_id)
    {
        Ok(buffer) => buffer,
        Err(error) => return call.runtime_fault(error),
    };
    let status = dsound_buffer_status(buffer.playback);
    complete_dsound_u32_write(call, GuestAddress(output_pointer), status)
}

fn handle_buffer_get_current_position(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [play_cursor_output, write_cursor_output] = call.frame().explicit_arguments.as_slice()
    else {
        unreachable!("frontend ABI preflight fixed the DirectSound argument count");
    };
    let (play_cursor_output, write_cursor_output) = (*play_cursor_output, *write_cursor_output);
    if play_cursor_output == 0 && write_cursor_output == 0 {
        return call.complete(DSOUND_E_INVALIDARG.bits());
    }
    let buffer = match call
        .runtime
        .audio_buffer_for_com_object(call.frame().object_id)
    {
        Ok(buffer) => buffer,
        Err(error) => return call.runtime_fault(error),
    };
    if buffer.descriptor.kind != RuntimeAudioBufferKind::Secondary
        || buffer.playback != RuntimeAudioPlaybackState::Stopped
    {
        return call.fault("bounded DirectSound cursor queries support stopped secondary buffers");
    }
    let position = match call.runtime.audio_buffer_position(call.frame().object_id) {
        Ok(position) => position,
        Err(error) => return call.runtime_fault(error),
    };
    let outputs = [
        (GuestAddress(play_cursor_output), position),
        (GuestAddress(write_cursor_output), position),
    ];
    let outputs = match (play_cursor_output == 0, write_cursor_output == 0) {
        (false, false) => &outputs[..],
        (false, true) => &outputs[..1],
        (true, false) => &outputs[1..],
        (true, true) => unreachable!("empty cursor outputs were rejected"),
    };
    complete_dsound_u32_writes(call, outputs)
}

fn dsound_buffer_status(playback: RuntimeAudioPlaybackState) -> u32 {
    match playback {
        RuntimeAudioPlaybackState::Stopped => 0,
        RuntimeAudioPlaybackState::PlayingOnce => DSOUND_DSBSTATUS_PLAYING,
        RuntimeAudioPlaybackState::PlayingLooping => {
            DSOUND_DSBSTATUS_PLAYING | DSOUND_DSBSTATUS_LOOPING
        }
    }
}

fn complete_dsound_u32_write(
    call: ComFrontendCall<'_>,
    output: GuestAddress,
    value: u32,
) -> ComDispatchOutcome {
    complete_dsound_u32_writes(call, &[(output, value)])
}

fn complete_dsound_u32_writes(
    call: ComFrontendCall<'_>,
    outputs: &[(GuestAddress, u32)],
) -> ComDispatchOutcome {
    for (output, _) in outputs {
        if let Err(error) = call
            .runtime
            .memory
            .validate_access(*output, 4, MemoryAccess::Write)
        {
            return call.memory_fault(error);
        }
    }
    let mut transaction = call.runtime.memory.begin_transaction();
    for &(output, value) in outputs {
        let bytes = value.to_le_bytes();
        match transaction.write(output, &bytes) {
            Ok(_) => {}
            Err(error) => {
                drop(transaction);
                return call.memory_fault(error);
            }
        };
    }
    transaction.commit_transaction();
    call.complete(COM_S_OK.bits())
}

fn handle_set_notification_positions(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [count, notifications_pointer] = call.frame().explicit_arguments.as_slice() else {
        unreachable!("frontend ABI preflight fixed the notification argument count");
    };
    let (count, notifications_pointer) = (*count, *notifications_pointer);
    let Ok(count) = usize::try_from(count) else {
        return call.complete(DSOUND_E_INVALIDARG.bits());
    };
    if count == 0 || count > RUNTIME_AUDIO_NOTIFICATION_LIMIT || notifications_pointer == 0 {
        return call.complete(DSOUND_E_INVALIDARG.bits());
    }
    let byte_length = match count.checked_mul(8) {
        Some(byte_length) => byte_length,
        None => return call.complete(DSOUND_E_INVALIDARG.bits()),
    };
    let bytes = match call
        .runtime
        .memory
        .read(GuestAddress(notifications_pointer), byte_length)
    {
        Ok(bytes) => bytes,
        Err(error) => return call.memory_fault(error),
    };
    let buffer = match call
        .runtime
        .audio_buffer_for_com_object(call.frame().object_id)
    {
        Ok(buffer) => buffer,
        Err(error) => return call.runtime_fault(error),
    };
    if buffer.descriptor.flags & DSOUND_DSBCAPS_CTRLPOSITIONNOTIFY == 0 {
        return call.complete(DSOUND_DSERR_CONTROLUNAVAIL.bits());
    }
    let mut notifications = Vec::with_capacity(count);
    for index in 0..count {
        let offset = index * 8;
        let notification = RuntimeAudioNotification {
            offset_bytes: read_u32(&bytes, offset).expect("validated notification byte range"),
            event_handle: RuntimeHandle(
                read_u32(&bytes, offset + 4).expect("validated notification byte range"),
            ),
        };
        notifications.push(notification);
    }
    match call
        .runtime
        .set_audio_buffer_notifications(call.frame().object_id, notifications)
    {
        Ok(true) => call.complete(COM_S_OK.bits()),
        Ok(false) => call.complete(DSOUND_E_INVALIDARG.bits()),
        Err(error) => call.runtime_fault(error),
    }
}

fn handle_buffer_unlock(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [data1, bytes1, data2, bytes2] = call.frame().explicit_arguments.as_slice() else {
        unreachable!("frontend ABI preflight fixed the DirectSound argument count");
    };
    let (data1, bytes1, data2, bytes2) = (*data1, *bytes1, *data2, *bytes2);
    if data1 == 0 {
        return call.complete(DSOUND_E_INVALIDARG.bits());
    }
    if (data2 == 0) != (bytes2 == 0) {
        return call.complete(DSOUND_E_INVALIDARG.bits());
    }
    let second_segment = (data2 != 0).then_some(RuntimeAudioBufferSegment {
        guest_address: GuestAddress(data2),
        byte_length: bytes2,
    });
    let buffer_com_object_id = call.frame().object_id;
    match call.runtime.prepare_browser_audio_unlock(
        buffer_com_object_id,
        RuntimeAudioBufferSegment {
            guest_address: GuestAddress(data1),
            byte_length: bytes1,
        },
        second_segment,
    ) {
        Ok(BrowserAudioPreparation::Complete(true)) => call.complete(COM_S_OK.bits()),
        Ok(BrowserAudioPreparation::Complete(false)) => call.complete(DSOUND_E_INVALIDARG.bits()),
        Ok(BrowserAudioPreparation::Pending(pending)) => call.defer_audio(pending),
        Err(error) => call.runtime_fault(error),
    }
}

fn handle_buffer_set_format(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let format_pointer = call.frame().explicit_arguments[0];
    if format_pointer == 0 {
        return call.complete(DSOUND_E_INVALIDARG.bits());
    }
    let format_bytes = match call.runtime.memory.read(GuestAddress(format_pointer), 16) {
        Ok(bytes) => bytes,
        Err(error) => return call.memory_fault(error),
    };
    let Some(format) = parse_pcm_format(&format_bytes) else {
        return call.complete(DSOUND_E_INVALIDARG.bits());
    };
    let buffer = match call
        .runtime
        .audio_buffer_for_com_object(call.frame().object_id)
    {
        Ok(buffer) => buffer,
        Err(error) => return call.runtime_fault(error),
    };
    if buffer.descriptor.kind == RuntimeAudioBufferKind::Secondary {
        return call.complete(DSOUND_DSERR_INVALIDCALL.bits());
    }
    if format != PRIMARY_PCM_FORMAT {
        return call.fault("DirectSound primary format is outside the supported PCM format");
    }
    match call
        .runtime
        .set_audio_buffer_format(call.frame().object_id, format)
    {
        Ok(_) => call.complete(COM_S_OK.bits()),
        Err(error) => call.runtime_fault(error),
    }
}

fn handle_buffer_control<F>(call: ComFrontendCall<'_>, build_update: F) -> ComDispatchOutcome
where
    F: FnOnce(u32) -> RuntimeAudioBufferControlUpdate,
{
    let raw_value = call.frame().explicit_arguments[0];
    commit_buffer_control(call, build_update(raw_value))
}

fn handle_buffer_frequency(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let raw_value = call.frame().explicit_arguments[0];
    let frequency = if raw_value == DSOUND_DSBFREQUENCY_ORIGINAL {
        let buffer = match call
            .runtime
            .audio_buffer_for_com_object(call.frame().object_id)
        {
            Ok(buffer) => buffer,
            Err(error) => return call.runtime_fault(error),
        };
        let Some(format) = buffer.active_format else {
            return call.complete(DSOUND_E_INVALIDARG.bits());
        };
        format.samples_per_second
    } else {
        raw_value
    };
    commit_buffer_control(call, RuntimeAudioBufferControlUpdate::Frequency(frequency))
}

fn commit_buffer_control(
    call: ComFrontendCall<'_>,
    update: RuntimeAudioBufferControlUpdate,
) -> ComDispatchOutcome {
    let (required_flag, value_valid) = match update {
        RuntimeAudioBufferControlUpdate::Volume(value) => (
            Some(DSOUND_DSBCAPS_CTRLVOLUME),
            (RUNTIME_AUDIO_VOLUME_MIN_HUNDREDTHS_DB..=RUNTIME_AUDIO_VOLUME_MAX_HUNDREDTHS_DB)
                .contains(&value),
        ),
        RuntimeAudioBufferControlUpdate::Pan(value) => (
            Some(DSOUND_DSBCAPS_CTRLPAN),
            (RUNTIME_AUDIO_PAN_MIN_HUNDREDTHS_DB..=RUNTIME_AUDIO_PAN_MAX_HUNDREDTHS_DB)
                .contains(&value),
        ),
        RuntimeAudioBufferControlUpdate::Frequency(value) => (
            Some(DSOUND_DSBCAPS_CTRLFREQUENCY),
            (RUNTIME_AUDIO_PLAYBACK_FREQUENCY_MIN_HZ..=RUNTIME_AUDIO_PLAYBACK_FREQUENCY_MAX_HZ)
                .contains(&value),
        ),
        RuntimeAudioBufferControlUpdate::CurrentPosition(_) => (None, true),
    };
    if !value_valid {
        return call.complete(DSOUND_E_INVALIDARG.bits());
    }
    let buffer = match call
        .runtime
        .audio_buffer_for_com_object(call.frame().object_id)
    {
        Ok(buffer) => buffer,
        Err(error) => return call.runtime_fault(error),
    };
    if required_flag.is_some_and(|flag| buffer.descriptor.flags & flag == 0) {
        return call.complete(DSOUND_DSERR_CONTROLUNAVAIL.bits());
    }
    match call
        .runtime
        .prepare_browser_audio_control(call.frame().object_id, update)
    {
        Ok(BrowserAudioPreparation::Complete(true)) => call.complete(COM_S_OK.bits()),
        Ok(BrowserAudioPreparation::Complete(false)) => call.complete(DSOUND_E_INVALIDARG.bits()),
        Ok(BrowserAudioPreparation::Pending(pending)) => call.defer_audio(pending),
        Err(error) => call.runtime_fault(error),
    }
}

fn handle_buffer_play(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [reserved, priority, flags] = call.frame().explicit_arguments.as_slice() else {
        unreachable!("frontend ABI preflight fixed the DirectSound argument count");
    };
    let (reserved, priority, flags) = (*reserved, *priority, *flags);
    if reserved != 0 || priority != 0 {
        return call.complete(DSOUND_E_INVALIDARG.bits());
    }
    let buffer = match call
        .runtime
        .audio_buffer_for_com_object(call.frame().object_id)
    {
        Ok(buffer) => buffer,
        Err(error) => return call.runtime_fault(error),
    };
    let playback = match buffer.descriptor.kind {
        RuntimeAudioBufferKind::Primary if flags == DSOUND_DSBPLAY_LOOPING => {
            RuntimeAudioPlaybackState::PlayingLooping
        }
        RuntimeAudioBufferKind::Primary => {
            return call.complete(DSOUND_E_INVALIDARG.bits());
        }
        RuntimeAudioBufferKind::Secondary => {
            if flags & !DSOUND_DSBPLAY_LOOPING != 0 {
                return call.complete(DSOUND_E_INVALIDARG.bits());
            }
            if flags & DSOUND_DSBPLAY_LOOPING != 0 {
                RuntimeAudioPlaybackState::PlayingLooping
            } else {
                RuntimeAudioPlaybackState::PlayingOnce
            }
        }
    };
    let object_id = call.frame().object_id;
    match call.runtime.prepare_browser_audio_play(object_id, playback) {
        Ok(BrowserAudioPreparation::Complete(true)) => call.complete(COM_S_OK.bits()),
        Ok(BrowserAudioPreparation::Complete(false)) => {
            call.complete(DSOUND_DSERR_INVALIDCALL.bits())
        }
        Ok(BrowserAudioPreparation::Pending(pending)) => call.defer_audio(pending),
        Err(error) => call.runtime_fault(error),
    }
}

fn handle_buffer_stop(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let object_id = call.frame().object_id;
    match call
        .runtime
        .prepare_browser_audio_play(object_id, RuntimeAudioPlaybackState::Stopped)
    {
        Ok(BrowserAudioPreparation::Complete(true)) => call.complete(COM_S_OK.bits()),
        Ok(BrowserAudioPreparation::Complete(false)) => {
            call.complete(DSOUND_DSERR_INVALIDCALL.bits())
        }
        Ok(BrowserAudioPreparation::Pending(pending)) => call.defer_audio(pending),
        Err(error) => call.runtime_fault(error),
    }
}

fn unsupported_buffer_variant(
    call: ComFrontendCall<'_>,
    message: impl Into<String>,
) -> ComDispatchOutcome {
    call.fault(message)
}

fn handle_set_priority_cooperative_level(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [window_handle, level] = call.frame().explicit_arguments.as_slice() else {
        unreachable!("frontend ABI preflight fixed the DirectSound argument count");
    };
    let (window_handle, level) = (*window_handle, *level);
    if window_handle == 0 || !(DSOUND_DSSCL_NORMAL..=DSOUND_DSSCL_WRITEPRIMARY).contains(&level) {
        return call.complete(DSOUND_E_INVALIDARG.bits());
    }
    if level != DSOUND_DSSCL_PRIORITY {
        return call.fault("DirectSound cooperative level is outside the supported priority level");
    }

    match call
        .runtime
        .set_audio_priority_cooperative_level(call.frame().object_id, window_handle)
    {
        Ok(true) => call.complete(COM_S_OK.bits()),
        Ok(false) => call.complete(DSOUND_E_INVALIDARG.bits()),
        Err(error) => call.runtime_fault(error),
    }
}

fn handle_direct_sound_create_8(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let Some(device_guid) = frame.argument_u32(0) else {
        return decoded_argument_missing();
    };
    let Some(output_pointer) = frame.argument_u32(1) else {
        return decoded_argument_missing();
    };
    let Some(outer_unknown) = frame.argument_u32(2) else {
        return decoded_argument_missing();
    };

    if output_pointer == 0 {
        return immediate_hresult(DSOUND_E_INVALIDARG);
    }
    if outer_unknown != 0 {
        return immediate_hresult(DSOUND_CLASS_E_NOAGGREGATION);
    }
    if device_guid != 0 {
        return ApiHandlerResult::Fault(ApiFault::new(
            "DirectSound device GUID selection requires the default device",
        ));
    }

    let output_address = GuestAddress(output_pointer);
    match runtime.install_com_object_with_guest_writes(
        None,
        direct_sound8_object_spec(),
        |planned| {
            let interface = planned
                .interfaces
                .iter()
                .find(|(iid, _)| *iid == DSOUND_IDIRECTSOUND8_IID)
                .ok_or_else(|| {
                    RuntimeStateError::new(
                        "installed DirectSound root object omitted IDirectSound8",
                    )
                })?;
            Ok(vec![(output_address, interface.1.0.to_le_bytes().to_vec())])
        },
    ) {
        Ok(_) => {}
        Err(error) => {
            return ApiHandlerResult::Fault(ApiFault::from_runtime(error, endpoint));
        }
    };
    ApiHandlerResult::Immediate(ApiImmediate::new(Some((COM_S_OK).bits())))
}

fn decoded_argument_missing() -> ApiHandlerResult {
    ApiHandlerResult::Fault(ApiFault::new(
        "decoded DirectSoundCreate8 frame is missing an argument",
    ))
}

fn immediate_hresult(result: ComHResult) -> ApiHandlerResult {
    ApiHandlerResult::Immediate(ApiImmediate::new(Some((result).bits())))
}

pub(crate) fn dsound_argument_words(endpoint: &ApiEndpointIdentity) -> Option<u8> {
    (endpoint.module.eq_ignore_ascii_case("dsound.dll")
        && matches!(
            &endpoint.symbol,
            ApiSymbol::Ordinal {
                ordinal: DSOUND_DIRECT_SOUND_CREATE_8_ORDINAL
            }
        ))
    .then_some(3)
}

pub(crate) fn direct_sound8_object_spec() -> ComObjectSpec {
    ComObjectSpec {
        class: RuntimeComClass::DirectSound8,
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            ComInterfaceSpec {
                iid: DSOUND_IDIRECTSOUND8_IID,
                name: "IDirectSound8".to_owned(),
                methods: vec![
                    ComMethodSpec::query_interface(),
                    ComMethodSpec::add_ref(),
                    ComMethodSpec::release(),
                    ComMethodSpec::frontend("CreateSoundBuffer", 3),
                    ComMethodSpec::unsupported("GetCaps"),
                    ComMethodSpec::unsupported("DuplicateSoundBuffer"),
                    ComMethodSpec::frontend("SetCooperativeLevel", 2),
                    ComMethodSpec::unsupported("Compact"),
                    ComMethodSpec::unsupported("GetSpeakerConfig"),
                    ComMethodSpec::unsupported("SetSpeakerConfig"),
                    ComMethodSpec::unsupported("Initialize"),
                    ComMethodSpec::unsupported("VerifyCertification"),
                ],
            },
        ],
    }
}

pub(crate) fn direct_sound_buffer8_object_spec() -> ComObjectSpec {
    ComObjectSpec {
        class: RuntimeComClass::DirectSoundBuffer8,
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            ComInterfaceSpec {
                iid: DSOUND_IDIRECTSOUNDBUFFER8_IID,
                name: "IDirectSoundBuffer8".to_owned(),
                methods: vec![
                    ComMethodSpec::query_interface(),
                    ComMethodSpec::add_ref(),
                    ComMethodSpec::release(),
                    ComMethodSpec::unsupported("GetCaps"),
                    ComMethodSpec::frontend("GetCurrentPosition", 2),
                    ComMethodSpec::unsupported("GetFormat"),
                    ComMethodSpec::unsupported("GetVolume"),
                    ComMethodSpec::unsupported("GetPan"),
                    ComMethodSpec::unsupported("GetFrequency"),
                    ComMethodSpec::frontend("GetStatus", 1),
                    ComMethodSpec::unsupported("Initialize"),
                    ComMethodSpec::frontend("Lock", 7),
                    ComMethodSpec::frontend("Play", 3),
                    ComMethodSpec::frontend("SetCurrentPosition", 1),
                    ComMethodSpec::frontend("SetFormat", 1),
                    ComMethodSpec::frontend("SetVolume", 1),
                    ComMethodSpec::frontend("SetPan", 1),
                    ComMethodSpec::frontend("SetFrequency", 1),
                    ComMethodSpec::frontend("Stop", 0),
                    ComMethodSpec::frontend("Unlock", 4),
                    ComMethodSpec::unsupported("Restore"),
                    ComMethodSpec::unsupported("SetFX"),
                    ComMethodSpec::unsupported("AcquireResources"),
                    ComMethodSpec::unsupported("GetObjectInPath"),
                ],
            },
            ComInterfaceSpec {
                iid: DSOUND_IDIRECTSOUNDNOTIFY_IID,
                name: "IDirectSoundNotify".to_owned(),
                methods: vec![
                    ComMethodSpec::query_interface(),
                    ComMethodSpec::add_ref(),
                    ComMethodSpec::release(),
                    ComMethodSpec::frontend("SetNotificationPositions", 2),
                ],
            },
        ],
    }
}

struct DsoundBufferDescriptor {
    size: u32,
    flags: u32,
    byte_length: u32,
    reserved: u32,
    format_pointer: u32,
    algorithm: [u8; 16],
}

fn parse_buffer_descriptor(bytes: &[u8]) -> Option<DsoundBufferDescriptor> {
    if bytes.len() != DSOUND_DSBUFFERDESC_BYTES as usize {
        return None;
    }
    let mut algorithm = [0_u8; 16];
    algorithm.copy_from_slice(bytes.get(20..36)?);
    Some(DsoundBufferDescriptor {
        size: read_u32(bytes, 0)?,
        flags: read_u32(bytes, 4)?,
        byte_length: read_u32(bytes, 8)?,
        reserved: read_u32(bytes, 12)?,
        format_pointer: read_u32(bytes, 16)?,
        algorithm,
    })
}

fn parse_pcm_format(bytes: &[u8]) -> Option<RuntimeAudioPcmFormat> {
    if bytes.len() != 16 || read_u16(bytes, 0)? != DSOUND_WAVE_FORMAT_PCM {
        return None;
    }
    let format = RuntimeAudioPcmFormat {
        channels: read_u16(bytes, 2)?,
        samples_per_second: read_u32(bytes, 4)?,
        average_bytes_per_second: read_u32(bytes, 8)?,
        block_align: read_u16(bytes, 12)?,
        bits_per_sample: read_u16(bytes, 14)?,
    };
    let bytes_per_sample = u32::from(format.bits_per_sample).checked_div(8)?;
    let expected_block_align = u32::from(format.channels).checked_mul(bytes_per_sample)?;
    let expected_average = format
        .samples_per_second
        .checked_mul(expected_block_align)?;
    if format.channels == 0
        || format.samples_per_second == 0
        || format.bits_per_sample == 0
        || !format.bits_per_sample.is_multiple_of(8)
        || expected_block_align != u32::from(format.block_align)
        || expected_average != format.average_bytes_per_second
    {
        return None;
    }
    Some(format)
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_spec_keeps_legacy_alias_outside_the_reached_band() {
        let spec = direct_sound8_object_spec();
        assert_eq!(spec.interfaces.len(), 2);
        assert_eq!(
            spec.interfaces[0].iid,
            crate::runtime::RUNTIME_COM_IUNKNOWN_IID
        );
        assert_eq!(spec.interfaces[1].iid, DSOUND_IDIRECTSOUND8_IID);
        assert_eq!(spec.interfaces[1].methods.len(), 12);
        assert_eq!(spec.interfaces[1].methods[3].name, "CreateSoundBuffer");
        assert_eq!(spec.interfaces[1].methods[6].name, "SetCooperativeLevel");
    }

    #[test]
    fn buffer_spec_exposes_reached_extended_and_notification_identities() {
        let spec = direct_sound_buffer8_object_spec();
        assert_eq!(spec.interfaces.len(), 3);
        assert_eq!(spec.interfaces[1].iid, DSOUND_IDIRECTSOUNDBUFFER8_IID);
        assert_eq!(spec.interfaces[2].iid, DSOUND_IDIRECTSOUNDNOTIFY_IID);
        assert_eq!(
            spec.interfaces[2].methods[3].name,
            "SetNotificationPositions"
        );
        let methods = &spec.interfaces[1].methods;
        assert_eq!(methods.len(), 24);
        assert_eq!(methods[11].name, "Lock");
        assert_eq!(methods[12].name, "Play");
        assert_eq!(methods[14].name, "SetFormat");
    }

    #[test]
    fn supported_descriptors_and_pcm_format_parse_exactly() {
        let mut primary = vec![0_u8; DSOUND_DSBUFFERDESC_BYTES as usize];
        primary[0..4].copy_from_slice(&DSOUND_DSBUFFERDESC_BYTES.to_le_bytes());
        primary[4..8].copy_from_slice(&DSOUND_PRIMARY_BUFFER_FLAGS.to_le_bytes());
        let parsed = parse_buffer_descriptor(&primary).expect("primary descriptor");
        assert_eq!(parsed.flags, DSOUND_PRIMARY_BUFFER_FLAGS);
        assert_eq!(parsed.byte_length, 0);
        assert_eq!(parsed.format_pointer, 0);

        let format = [
            0x01, 0x00, 0x02, 0x00, 0x44, 0xac, 0x00, 0x00, 0x10, 0xb1, 0x02, 0x00, 0x04, 0x00,
            0x10, 0x00,
        ];
        assert_eq!(parse_pcm_format(&format), Some(PRIMARY_PCM_FORMAT));
        let mut incoherent = format;
        incoherent[12] = 2;
        assert_eq!(parse_pcm_format(&incoherent), None);
    }

    #[test]
    fn native_hresult_constants_are_fixed() {
        assert_eq!(DSOUND_CLASS_E_NOAGGREGATION.bits(), 0x8004_0110);
        assert_eq!(DSOUND_E_INVALIDARG.bits(), 0x8007_0057);
        assert_eq!(DSOUND_DSERR_INVALIDCALL.bits(), 0x8878_0032);
        assert_eq!(crate::com::COM_E_NOINTERFACE.bits(), 0x8000_4002);
    }

    #[test]
    fn buffer_status_projects_only_authoritative_playback_bits() {
        assert_eq!(dsound_buffer_status(RuntimeAudioPlaybackState::Stopped), 0);
        assert_eq!(
            dsound_buffer_status(RuntimeAudioPlaybackState::PlayingOnce),
            DSOUND_DSBSTATUS_PLAYING
        );
        assert_eq!(
            dsound_buffer_status(RuntimeAudioPlaybackState::PlayingLooping),
            DSOUND_DSBSTATUS_PLAYING | DSOUND_DSBSTATUS_LOOPING
        );
    }
}

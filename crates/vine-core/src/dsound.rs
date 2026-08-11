use crate::api::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiCallFrame, ApiContract, ApiDirection,
    ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiPointeeSchema,
    ApiPointerConstraint, ApiReturnSchema, ApiReturnValue, ApiSymbol, ApiValueSchema,
    ApiWriteRequirement, CallbackPolicy, CallingConvention, ExecutionSession, LastErrorPolicy,
    ReentrancyPolicy, StackCleanup,
};
use crate::com::{
    COM_S_OK, ComCallTrap, ComDispatchOutcome, ComFaultClass, ComFrontendCall,
    ComGuestWriteReceipt, ComHResult, ComInterfaceSpec, ComMethodSpec, ComObjectSpec,
    ComReturnValue,
};
use crate::memory::{GuestAddress, MemoryAccess};
use crate::runtime::{
    RUNTIME_AUDIO_NOTIFICATION_LIMIT, RUNTIME_AUDIO_PAN_MAX_HUNDREDTHS_DB,
    RUNTIME_AUDIO_PAN_MIN_HUNDREDTHS_DB, RUNTIME_AUDIO_PCM_MAX_CHANNELS,
    RUNTIME_AUDIO_PCM_MAX_SAMPLES_PER_SECOND, RUNTIME_AUDIO_PCM_MIN_SAMPLES_PER_SECOND,
    RUNTIME_AUDIO_PLAYBACK_FREQUENCY_MAX_HZ, RUNTIME_AUDIO_PLAYBACK_FREQUENCY_MIN_HZ,
    RUNTIME_AUDIO_RESOURCE_ARENA_SIZE, RUNTIME_AUDIO_VOLUME_MAX_HUNDREDTHS_DB,
    RUNTIME_AUDIO_VOLUME_MIN_HUNDREDTHS_DB, RuntimeAudioBufferControlRequest,
    RuntimeAudioBufferControlUpdate, RuntimeAudioBufferCreateRequest, RuntimeAudioBufferDescriptor,
    RuntimeAudioBufferFormatRequest, RuntimeAudioBufferKind, RuntimeAudioBufferLockRequest,
    RuntimeAudioBufferNotificationRequest, RuntimeAudioBufferPlayRequest,
    RuntimeAudioBufferSegment, RuntimeAudioBufferUnlockRequest, RuntimeAudioCooperativeLevel,
    RuntimeAudioCooperativeLevelRequest, RuntimeAudioDeviceCreateRequest,
    RuntimeAudioDeviceSelector, RuntimeAudioFrontend, RuntimeAudioNotification,
    RuntimeAudioPcmFormat, RuntimeAudioPlaybackState, RuntimeComIid, RuntimeComReturnKind,
    RuntimeHandle, RuntimeStateError, RuntimeStateErrorClass,
};
use serde::Serialize;

pub const DSOUND_DIRECT_SOUND_CREATE_8_ORDINAL: u16 = 11;
pub const DSOUND_DIRECT_SOUND_CREATE_8_IMPLEMENTATION: &str = "dsound.direct-sound-create-8.v1";
pub const DSOUND_IDIRECTSOUND8_CLASS_IDENTITY: &str = "dsound.idirectsound8-root.v1";
pub const DSOUND_IDIRECTSOUNDBUFFER8_CLASS_IDENTITY: &str = "dsound.idirectsoundbuffer8.v1";
pub const DSOUND_IDIRECTSOUND_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0x83, 0xfa, 0x9a, 0x27, 0x81, 0x49, 0xce, 0x11, 0xa5, 0x21, 0x00, 0x20, 0xaf, 0x0b, 0xe5, 0x60,
]);
pub const DSOUND_IDIRECTSOUND8_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0x93, 0x7e, 0x0a, 0xc5, 0x95, 0xf3, 0x34, 0x48, 0x9e, 0xf6, 0x7f, 0xa9, 0x9d, 0xe5, 0x09, 0x66,
]);
pub const DSOUND_IDIRECTSOUNDBUFFER_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0x85, 0xfa, 0x9a, 0x27, 0x81, 0x49, 0xce, 0x11, 0xa5, 0x21, 0x00, 0x20, 0xaf, 0x0b, 0xe5, 0x60,
]);
pub const DSOUND_IDIRECTSOUNDBUFFER8_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0x49, 0xa4, 0x25, 0x68, 0x24, 0x75, 0x82, 0x4d, 0x92, 0x0f, 0x50, 0xe3, 0x6a, 0xb3, 0xab, 0x1e,
]);
pub const DSOUND_IDIRECTSOUNDNOTIFY_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0x83, 0x07, 0x21, 0xb0, 0xcd, 0x89, 0xd0, 0x11, 0xaf, 0x08, 0x00, 0xa0, 0xc9, 0x25, 0xcd, 0x16,
]);
pub const DSOUND_CLASS_E_NOAGGREGATION: ComHResult = ComHResult(0x8004_0110_u32 as i32);
pub const DSOUND_E_INVALIDARG: ComHResult = ComHResult(0x8007_0057_u32 as i32);
pub const DSOUND_DSERR_INVALIDCALL: ComHResult = ComHResult(0x8878_0032_u32 as i32);
pub const DSOUND_DSSCL_NORMAL: u32 = 1;
pub const DSOUND_DSSCL_PRIORITY: u32 = 2;
pub const DSOUND_DSSCL_EXCLUSIVE: u32 = 3;
pub const DSOUND_DSSCL_WRITEPRIMARY: u32 = 4;
pub const DSOUND_COMPATIBILITY_POLICY_SCHEMA: &str = "vine.dsound-compatibility-policy";
pub const DSOUND_COMPATIBILITY_POLICY_VERSION: u32 = 2;
pub const DSOUND_DSBUFFERDESC_BYTES: u32 = 36;
pub const DSOUND_DSBCAPS_PRIMARYBUFFER: u32 = 0x0000_0001;
pub const DSOUND_DSBCAPS_CTRLFREQUENCY: u32 = 0x0000_0020;
pub const DSOUND_DSBCAPS_CTRLPAN: u32 = 0x0000_0040;
pub const DSOUND_DSBCAPS_CTRLVOLUME: u32 = 0x0000_0080;
pub const DSOUND_DSBCAPS_CTRLPOSITIONNOTIFY: u32 = 0x0000_0100;
pub const DSOUND_PRIMARY_BUFFER_FLAGS: u32 = DSOUND_DSBCAPS_PRIMARYBUFFER;
pub const DSOUND_SECONDARY_BUFFER_FLAGS: u32 = 0x0000_80a8;
pub const DSOUND_SECONDARY_BUFFER_FORWARD_FLAGS: u32 = 0x0001_81e8;
pub const DSOUND_SECONDARY_BUFFER_BYTES: u32 = 88_200;
pub const DSOUND_SECONDARY_BUFFER_MAX_BYTES: u32 = RUNTIME_AUDIO_RESOURCE_ARENA_SIZE as u32;
pub const DSOUND_WAVE_FORMAT_PCM: u16 = 1;
pub const DSOUND_DSBPLAY_LOOPING: u32 = 1;
pub const DSOUND_DSBSTATUS_PLAYING: u32 = 0x0000_0001;
pub const DSOUND_DSBSTATUS_LOOPING: u32 = 0x0000_0004;
pub const DSOUND_DSBFREQUENCY_ORIGINAL: u32 = 0;
pub const DSOUND_DSERR_CONTROLUNAVAIL: ComHResult = ComHResult(0x8878_001e_u32 as i32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DsoundCooperativeLevelPolicy {
    pub profile_id: String,
    pub level: u32,
    pub require_live_top_level_window: bool,
    pub require_window_owner_thread: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DsoundInitialBuffersPolicy {
    pub profile_id: String,
    pub primary_flags: u32,
    pub secondary_required_flags: u32,
    pub secondary_allowed_flags: u32,
    pub secondary_buffer_min_bytes: u32,
    pub secondary_buffer_max_bytes: u32,
    pub secondary_min_channels: u16,
    pub secondary_max_channels: u16,
    pub secondary_min_samples_per_second: u32,
    pub secondary_max_samples_per_second: u32,
    pub secondary_bits_per_sample: Vec<u16>,
    pub pcm_format: RuntimeAudioPcmFormat,
    pub primary_play_flags: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DsoundBufferTransferPolicy {
    pub profile_id: String,
    pub lock_offset: u32,
    pub lock_flags: u32,
    pub secondary_play_flags: u32,
    pub allow_secondary_once: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DsoundCompatibilityPolicy {
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub set_cooperative_level: Option<DsoundCooperativeLevelPolicy>,
    pub initial_buffers: Option<DsoundInitialBuffersPolicy>,
    pub buffer_transfer: Option<DsoundBufferTransferPolicy>,
}

impl DsoundCompatibilityPolicy {
    pub fn controlled_v1() -> Self {
        Self::with_secondary_band(
            "dsound-primary-default-device-v1",
            "primary-stereo-pcm-and-secondary-88200-v1",
            DSOUND_SECONDARY_BUFFER_BYTES,
            DSOUND_SECONDARY_BUFFER_BYTES,
            2,
            2,
            44_100,
            44_100,
            vec![16],
            false,
        )
    }

    pub fn forward_v2() -> Self {
        Self::with_secondary_band(
            "dsound-default-device-forward-v2",
            "primary-stereo-pcm-secondary-bounded-v2",
            1,
            DSOUND_SECONDARY_BUFFER_MAX_BYTES,
            1,
            RUNTIME_AUDIO_PCM_MAX_CHANNELS,
            RUNTIME_AUDIO_PCM_MIN_SAMPLES_PER_SECOND,
            RUNTIME_AUDIO_PCM_MAX_SAMPLES_PER_SECOND,
            vec![8, 16],
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn with_secondary_band(
        id: &str,
        buffer_profile_id: &str,
        secondary_buffer_min_bytes: u32,
        secondary_buffer_max_bytes: u32,
        secondary_min_channels: u16,
        secondary_max_channels: u16,
        secondary_min_samples_per_second: u32,
        secondary_max_samples_per_second: u32,
        secondary_bits_per_sample: Vec<u16>,
        allow_secondary_once: bool,
    ) -> Self {
        Self {
            schema: DSOUND_COMPATIBILITY_POLICY_SCHEMA.to_owned(),
            schema_version: DSOUND_COMPATIBILITY_POLICY_VERSION,
            id: id.to_owned(),
            set_cooperative_level: Some(DsoundCooperativeLevelPolicy {
                profile_id: "primary-live-window-priority-v1".to_owned(),
                level: DSOUND_DSSCL_PRIORITY,
                require_live_top_level_window: true,
                require_window_owner_thread: true,
            }),
            initial_buffers: Some(DsoundInitialBuffersPolicy {
                profile_id: buffer_profile_id.to_owned(),
                primary_flags: DSOUND_PRIMARY_BUFFER_FLAGS,
                secondary_required_flags: DSOUND_SECONDARY_BUFFER_FLAGS,
                secondary_allowed_flags: if allow_secondary_once {
                    DSOUND_SECONDARY_BUFFER_FORWARD_FLAGS
                } else {
                    DSOUND_SECONDARY_BUFFER_FLAGS
                },
                secondary_buffer_min_bytes,
                secondary_buffer_max_bytes,
                secondary_min_channels,
                secondary_max_channels,
                secondary_min_samples_per_second,
                secondary_max_samples_per_second,
                secondary_bits_per_sample,
                pcm_format: RuntimeAudioPcmFormat {
                    channels: 2,
                    samples_per_second: 44_100,
                    average_bytes_per_second: 176_400,
                    block_align: 4,
                    bits_per_sample: 16,
                },
                primary_play_flags: DSOUND_DSBPLAY_LOOPING,
            }),
            buffer_transfer: Some(DsoundBufferTransferPolicy {
                profile_id: "secondary-bounded-lock-play-v2".to_owned(),
                lock_offset: 0,
                lock_flags: 0,
                secondary_play_flags: DSOUND_DSBPLAY_LOOPING,
                allow_secondary_once,
            }),
        }
    }

    pub fn validate(&self) -> Result<(), ApiFault> {
        if self.schema != DSOUND_COMPATIBILITY_POLICY_SCHEMA
            || self.schema_version != DSOUND_COMPATIBILITY_POLICY_VERSION
            || !valid_policy_id(&self.id)
        {
            return Err(ApiFault::runtime_defect(
                "dsound-compatibility-policy",
                "dsound-compatibility-policy-header-invalid",
                "DirectSound compatibility policy has an unsupported schema, version, or ID",
            ));
        }
        if let Some(cooperative) = self.set_cooperative_level.as_ref()
            && (!valid_policy_id(&cooperative.profile_id)
                || cooperative.level != DSOUND_DSSCL_PRIORITY
                || !cooperative.require_live_top_level_window
                || !cooperative.require_window_owner_thread)
        {
            return Err(ApiFault::runtime_defect(
                "dsound-compatibility-policy",
                "dsound-cooperative-level-profile-invalid",
                "DirectSound cooperative policy must describe the evidenced live top-level priority relation",
            ));
        }
        if let Some(buffers) = self.initial_buffers.as_ref()
            && (!valid_policy_id(&buffers.profile_id)
                || buffers.primary_flags != DSOUND_PRIMARY_BUFFER_FLAGS
                || buffers.secondary_required_flags != DSOUND_SECONDARY_BUFFER_FLAGS
                || buffers.secondary_required_flags & !buffers.secondary_allowed_flags != 0
                || buffers.secondary_allowed_flags & !DSOUND_SECONDARY_BUFFER_FORWARD_FLAGS != 0
                || buffers.secondary_buffer_min_bytes == 0
                || buffers.secondary_buffer_min_bytes > buffers.secondary_buffer_max_bytes
                || u64::from(buffers.secondary_buffer_max_bytes)
                    > RUNTIME_AUDIO_RESOURCE_ARENA_SIZE
                || buffers.secondary_min_channels == 0
                || buffers.secondary_min_channels > buffers.secondary_max_channels
                || buffers.secondary_max_channels > RUNTIME_AUDIO_PCM_MAX_CHANNELS
                || buffers.secondary_min_samples_per_second
                    < RUNTIME_AUDIO_PCM_MIN_SAMPLES_PER_SECOND
                || buffers.secondary_min_samples_per_second
                    > buffers.secondary_max_samples_per_second
                || buffers.secondary_max_samples_per_second
                    > RUNTIME_AUDIO_PCM_MAX_SAMPLES_PER_SECOND
                || buffers.secondary_bits_per_sample.is_empty()
                || buffers
                    .secondary_bits_per_sample
                    .iter()
                    .any(|bits| !matches!(bits, 8 | 16))
                || buffers.pcm_format
                    != (RuntimeAudioPcmFormat {
                        channels: 2,
                        samples_per_second: 44_100,
                        average_bytes_per_second: 176_400,
                        block_align: 4,
                        bits_per_sample: 16,
                    })
                || buffers.primary_play_flags != DSOUND_DSBPLAY_LOOPING)
        {
            return Err(ApiFault::runtime_defect(
                "dsound-compatibility-policy",
                "dsound-initial-buffers-profile-invalid",
                "DirectSound initial-buffer policy must match the evidenced PCM setup sequence",
            ));
        }
        if let Some(transfer) = self.buffer_transfer.as_ref()
            && (!valid_policy_id(&transfer.profile_id)
                || self.initial_buffers.is_none()
                || transfer.lock_offset != 0
                || transfer.lock_flags != 0
                || transfer.secondary_play_flags != DSOUND_DSBPLAY_LOOPING)
        {
            return Err(ApiFault::runtime_defect(
                "dsound-compatibility-policy",
                "dsound-buffer-transfer-profile-invalid",
                "DirectSound buffer-transfer policy must describe bounded Lock/Unlock and secondary Play",
            ));
        }
        Ok(())
    }
}

fn valid_policy_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

pub fn dispatch_dsound_api(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    match contract.implementation_id.as_str() {
        DSOUND_DIRECT_SOUND_CREATE_8_IMPLEMENTATION => {
            handle_direct_sound_create_8(contract, frame, session)
        }
        implementation_id => ApiHandlerResult::Fault(ApiFault::unsupported(
            "dsound-api-dispatch",
            "implementation-id-unsupported",
            format!("DirectSound implementation {implementation_id} is not registered"),
        )),
    }
}

pub fn dispatch_dsound_com_call(
    policy: &DsoundCompatibilityPolicy,
    trap: ComCallTrap,
    session: &mut ExecutionSession<'_>,
) -> ComDispatchOutcome {
    session
        .dispatch_com_call_with_frontend(trap, |call| dispatch_dsound_frontend_call(policy, call))
}

fn dispatch_dsound_frontend_call(
    policy: &DsoundCompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let root_target = call.class_identity() == DSOUND_IDIRECTSOUND8_CLASS_IDENTITY
        && call.frame().iid == DSOUND_IDIRECTSOUND8_IID;
    let buffer_class = call.class_identity() == DSOUND_IDIRECTSOUNDBUFFER8_CLASS_IDENTITY;
    let notification_target = buffer_class && call.frame().iid == DSOUND_IDIRECTSOUNDNOTIFY_IID;
    let buffer_target = buffer_class && call.frame().iid == DSOUND_IDIRECTSOUNDBUFFER8_IID;
    if !root_target && !buffer_target && !notification_target {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "dsound-com-dispatch",
            "dsound-com-target-mismatch",
            "DirectSound frontend received a COM object outside its owned interfaces",
        );
    }
    if let Err(fault) = policy.validate() {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            fault.stage,
            fault.code,
            fault.message,
        );
    }
    if notification_target {
        return match (
            call.frame().vtable_slot,
            call.frame().method.as_str(),
            call.explicit_argument_words(),
            call.frame().explicit_arguments.len(),
            call.return_kind(),
        ) {
            (3, "SetNotificationPositions", 2, 2, RuntimeComReturnKind::HResult) => {
                handle_set_notification_positions(call)
            }
            _ => call.fault(
                ComFaultClass::RuntimeDefect,
                "dsound-com-dispatch",
                "dsound-notify-abi-mismatch",
                "registered IDirectSoundNotify ABI does not match its method contract",
            ),
        };
    }
    match (
        root_target,
        buffer_target,
        call.frame().vtable_slot,
        call.frame().method.as_str(),
        call.explicit_argument_words(),
        call.frame().explicit_arguments.len(),
        call.return_kind(),
    ) {
        (true, false, 3, "CreateSoundBuffer", 3, 3, RuntimeComReturnKind::HResult) => {
            handle_create_sound_buffer(policy, call)
        }
        (true, false, 6, "SetCooperativeLevel", 2, 2, RuntimeComReturnKind::HResult) => {
            handle_set_cooperative_level(policy, call)
        }
        (false, true, 9, "GetStatus", 1, 1, RuntimeComReturnKind::HResult) => {
            handle_buffer_get_status(call)
        }
        (false, true, 11, "Lock", 7, 7, RuntimeComReturnKind::HResult) => {
            handle_buffer_lock(policy, call)
        }
        (false, true, 12, "Play", 3, 3, RuntimeComReturnKind::HResult) => {
            handle_buffer_play(policy, call)
        }
        (false, true, 13, "SetCurrentPosition", 1, 1, RuntimeComReturnKind::HResult) => {
            handle_buffer_control(call, RuntimeAudioBufferControlUpdate::CurrentPosition)
        }
        (false, true, 14, "SetFormat", 1, 1, RuntimeComReturnKind::HResult) => {
            handle_buffer_set_format(policy, call)
        }
        (false, true, 15, "SetVolume", 1, 1, RuntimeComReturnKind::HResult) => {
            handle_buffer_control(call, |value| {
                RuntimeAudioBufferControlUpdate::Volume(value as i32)
            })
        }
        (false, true, 16, "SetPan", 1, 1, RuntimeComReturnKind::HResult) => {
            handle_buffer_control(call, |value| {
                RuntimeAudioBufferControlUpdate::Pan(value as i32)
            })
        }
        (false, true, 17, "SetFrequency", 1, 1, RuntimeComReturnKind::HResult) => {
            handle_buffer_frequency(call)
        }
        (false, true, 19, "Unlock", 4, 4, RuntimeComReturnKind::HResult) => {
            handle_buffer_unlock(policy, call)
        }
        _ => call.fault(
            ComFaultClass::RuntimeDefect,
            "dsound-com-dispatch",
            "dsound-com-abi-mismatch",
            "registered DirectSound frontend ABI does not match its method contract",
        ),
    }
}

fn handle_create_sound_buffer(
    policy: &DsoundCompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let [descriptor_pointer, output_pointer, outer_unknown] =
        call.frame().explicit_arguments.as_slice()
    else {
        unreachable!("frontend ABI preflight fixed the DirectSound argument count");
    };
    let (descriptor_pointer, output_pointer, outer_unknown) =
        (*descriptor_pointer, *output_pointer, *outer_unknown);
    if descriptor_pointer == 0 || output_pointer == 0 {
        return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
    }
    if outer_unknown != 0 {
        return call.complete(
            ComReturnValue::HResult(DSOUND_CLASS_E_NOAGGREGATION),
            Vec::new(),
        );
    }
    let Some(buffers_policy) = policy.initial_buffers.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "com-dispatch",
            "com-method-unsupported",
            "COM interface IDirectSound8 slot 3 (CreateSoundBuffer) is not enabled by the active policy",
        );
    };
    let output_address = GuestAddress(output_pointer);
    if let Err(error) = call
        .memory()
        .validate_access(output_address, 4, MemoryAccess::Write)
    {
        return call.memory_fault(error);
    }
    let descriptor_bytes = match call.memory().read(
        GuestAddress(descriptor_pointer),
        DSOUND_DSBUFFERDESC_BYTES as usize,
    ) {
        Ok(bytes) => bytes,
        Err(error) => return call.memory_fault(error),
    };
    let Some(descriptor) = parse_buffer_descriptor(&descriptor_bytes) else {
        return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
    };
    if descriptor.size != DSOUND_DSBUFFERDESC_BYTES
        || descriptor.reserved != 0
        || descriptor.algorithm.iter().any(|byte| *byte != 0)
    {
        return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
    }

    let logical_descriptor = if descriptor.flags & DSOUND_DSBCAPS_PRIMARYBUFFER != 0 {
        if descriptor.byte_length != 0 || descriptor.format_pointer != 0 {
            return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
        }
        if descriptor.flags != buffers_policy.primary_flags {
            return unsupported_buffer_variant(
                call,
                "primary DirectSound buffer flags are not evidenced",
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
            return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
        }
        let format_bytes = match call
            .memory()
            .read(GuestAddress(descriptor.format_pointer), 16)
        {
            Ok(bytes) => bytes,
            Err(error) => return call.memory_fault(error),
        };
        let Some(format) = parse_pcm_format(&format_bytes) else {
            return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
        };
        let whole_pcm_frames = descriptor
            .byte_length
            .is_multiple_of(u32::from(format.block_align));
        if descriptor.flags & buffers_policy.secondary_required_flags
            != buffers_policy.secondary_required_flags
            || descriptor.flags & !buffers_policy.secondary_allowed_flags != 0
            || !(buffers_policy.secondary_buffer_min_bytes
                ..=buffers_policy.secondary_buffer_max_bytes)
                .contains(&descriptor.byte_length)
            || !(buffers_policy.secondary_min_channels..=buffers_policy.secondary_max_channels)
                .contains(&format.channels)
            || !(buffers_policy.secondary_min_samples_per_second
                ..=buffers_policy.secondary_max_samples_per_second)
                .contains(&format.samples_per_second)
            || !buffers_policy
                .secondary_bits_per_sample
                .contains(&format.bits_per_sample)
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

    let audio_plan =
        match call
            .runtime()
            .plan_audio_buffer_creation(RuntimeAudioBufferCreateRequest {
                device_com_object_id: call.frame().object_id,
                descriptor: logical_descriptor,
            }) {
            Ok(plan) => plan,
            Err(error) => return call.runtime_fault(error),
        };
    if let Err(error) = call.runtime().preflight_audio_buffer_creation(&audio_plan) {
        return call.runtime_fault(error);
    }
    let planned_buffer_id = audio_plan.id();
    let planned_device_id = audio_plan.device_id();
    let planned_descriptor = audio_plan.descriptor();
    let mut call = call;
    let (installed, guest_writes) = match call.install_child_com_object_with_guest_writes(
        direct_sound_buffer8_object_spec(),
        |object| {
            let interface = object
                .interfaces
                .iter()
                .find(|interface| interface.iid == DSOUND_IDIRECTSOUNDBUFFER8_IID)
                .ok_or_else(|| {
                    RuntimeStateError::runtime_defect(
                        "dsound-create-sound-buffer",
                        "idirectsoundbuffer8-installation-missing",
                        "installed DirectSound buffer object has no IDirectSoundBuffer8 interface",
                    )
                })?;
            Ok(vec![(
                output_address,
                interface.guest_pointer.0.to_le_bytes().to_vec(),
            )])
        },
    ) {
        Ok(result) => result,
        Err(error) => return call.runtime_fault(error),
    };
    let buffer = call
        .runtime_mut()
        .commit_audio_buffer_creation(audio_plan, installed.object.id);
    debug_assert_eq!(buffer.id, planned_buffer_id);
    debug_assert_eq!(buffer.device_id, planned_device_id);
    debug_assert_eq!(buffer.descriptor, planned_descriptor);
    call.complete(ComReturnValue::HResult(COM_S_OK), guest_writes)
}

fn handle_buffer_lock(
    policy: &DsoundCompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
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
    let Some(transfer) = policy.buffer_transfer.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "com-dispatch",
            "com-method-unsupported",
            "COM interface IDirectSoundBuffer8 slot 11 (Lock) is not enabled by the active policy",
        );
    };
    if [data1_output, bytes1_output, data2_output, bytes2_output].contains(&0) {
        return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
    }
    let controlled_full_range = !transfer.allow_secondary_once
        && call
            .runtime()
            .audio_buffer_for_com_object(call.frame().object_id)
            .is_ok_and(|buffer| byte_length == buffer.descriptor.byte_length);
    if flags != transfer.lock_flags
        || (!transfer.allow_secondary_once
            && (offset != transfer.lock_offset || !controlled_full_range))
    {
        return call.fault(
            ComFaultClass::Unsupported,
            "dsound-buffer-lock",
            "dsound-buffer-lock-variant-unsupported",
            "DirectSound Lock is outside the active bounded secondary-buffer transfer variant",
        );
    }
    let output_addresses = [
        GuestAddress(data1_output),
        GuestAddress(bytes1_output),
        GuestAddress(data2_output),
        GuestAddress(bytes2_output),
    ];
    for address in output_addresses {
        if let Err(error) = call
            .memory()
            .validate_access(address, 4, MemoryAccess::Write)
        {
            return call.memory_fault(error);
        }
    }
    let mut call = call;
    match call.lock_audio_buffer_with_guest_writes(
        RuntimeAudioBufferLockRequest {
            buffer_com_object_id: call.frame().object_id,
            offset,
            byte_length,
            flags,
        },
        output_addresses,
    ) {
        Ok((_lock, guest_writes)) => call.complete(ComReturnValue::HResult(COM_S_OK), guest_writes),
        Err(error) if error.code == "audio-buffer-already-locked" => {
            call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new())
        }
        Err(error) if error.code == "audio-buffer-lock-variant-unsupported" => call.fault(
            ComFaultClass::Unsupported,
            "dsound-buffer-lock",
            "dsound-buffer-lock-target-unsupported",
            error.message,
        ),
        Err(error) => call.runtime_fault(error),
    }
}

fn handle_buffer_get_status(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let output_pointer = call.frame().explicit_arguments[0];
    if output_pointer == 0 {
        return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
    }
    let buffer = match call
        .runtime()
        .audio_buffer_for_com_object(call.frame().object_id)
    {
        Ok(buffer) => buffer,
        Err(error) => return call.runtime_fault(error),
    };
    let status = dsound_buffer_status(buffer.playback);
    complete_dsound_u32_write(call, GuestAddress(output_pointer), status)
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
    if let Err(error) = call
        .memory()
        .validate_access(output, 4, MemoryAccess::Write)
    {
        return call.memory_fault(error);
    }
    let bytes = value.to_le_bytes().to_vec();
    let mut call = call;
    let mut transaction = call.memory_mut().begin_transaction();
    let result = match transaction.write(output, &bytes) {
        Ok(result) => result,
        Err(error) => {
            drop(transaction);
            return call.memory_fault(error);
        }
    };
    transaction.commit_transaction();
    call.complete(
        ComReturnValue::HResult(COM_S_OK),
        vec![ComGuestWriteReceipt {
            address: output,
            bytes,
            result,
        }],
    )
}

fn handle_set_notification_positions(mut call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [count, notifications_pointer] = call.frame().explicit_arguments.as_slice() else {
        unreachable!("frontend ABI preflight fixed the notification argument count");
    };
    let (count, notifications_pointer) = (*count, *notifications_pointer);
    let Ok(count) = usize::try_from(count) else {
        return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
    };
    if count == 0 || count > RUNTIME_AUDIO_NOTIFICATION_LIMIT || notifications_pointer == 0 {
        return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
    }
    let byte_length = match count.checked_mul(8) {
        Some(byte_length) => byte_length,
        None => return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new()),
    };
    let bytes = match call
        .memory()
        .read(GuestAddress(notifications_pointer), byte_length)
    {
        Ok(bytes) => bytes,
        Err(error) => return call.memory_fault(error),
    };
    let buffer = match call
        .runtime()
        .audio_buffer_for_com_object(call.frame().object_id)
    {
        Ok(buffer) => buffer,
        Err(error) => return call.runtime_fault(error),
    };
    if buffer.descriptor.flags & DSOUND_DSBCAPS_CTRLPOSITIONNOTIFY == 0 {
        return call.complete(
            ComReturnValue::HResult(DSOUND_DSERR_CONTROLUNAVAIL),
            Vec::new(),
        );
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
        if call
            .runtime()
            .event_for_handle(notification.event_handle)
            .is_err()
        {
            return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
        }
        notifications.push(notification);
    }
    let plan = match call.runtime().plan_audio_buffer_notifications(
        RuntimeAudioBufferNotificationRequest {
            buffer_com_object_id: call.frame().object_id,
            notifications,
        },
    ) {
        Ok(plan) => plan,
        Err(error) if error.class != RuntimeStateErrorClass::RuntimeDefect => {
            return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
        }
        Err(error) => return call.runtime_fault(error),
    };
    if let Err(error) = call.runtime_mut().commit_audio_buffer_notifications(plan) {
        return call.runtime_fault(error);
    }
    call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new())
}

fn handle_buffer_unlock(
    policy: &DsoundCompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let [data1, bytes1, data2, bytes2] = call.frame().explicit_arguments.as_slice() else {
        unreachable!("frontend ABI preflight fixed the DirectSound argument count");
    };
    let (data1, bytes1, data2, bytes2) = (*data1, *bytes1, *data2, *bytes2);
    let Some(_transfer) = policy.buffer_transfer.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "com-dispatch",
            "com-method-unsupported",
            "COM interface IDirectSoundBuffer8 slot 19 (Unlock) is not enabled by the active policy",
        );
    };
    if data1 == 0 {
        return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
    }
    if (data2 == 0) != (bytes2 == 0) {
        return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
    }
    let second_segment = (data2 != 0).then_some(RuntimeAudioBufferSegment {
        guest_address: GuestAddress(data2),
        byte_length: bytes2,
    });
    let mut call = call;
    match call.unlock_audio_buffer(RuntimeAudioBufferUnlockRequest {
        buffer_com_object_id: call.frame().object_id,
        first_segment: RuntimeAudioBufferSegment {
            guest_address: GuestAddress(data1),
            byte_length: bytes1,
        },
        second_segment,
    }) {
        Ok(_result) => call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new()),
        Err(error) if error.code == "audio-buffer-not-locked" => {
            call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new())
        }
        Err(error) if error.code == "audio-buffer-unlock-segments-mismatch" => call.fault(
            ComFaultClass::Unsupported,
            "dsound-buffer-unlock",
            "dsound-buffer-unlock-segments-unsupported",
            "DirectSound Unlock did not return the exact active Lock segments",
        ),
        Err(error) => call.runtime_fault(error),
    }
}

fn handle_buffer_set_format(
    policy: &DsoundCompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let format_pointer = call.frame().explicit_arguments[0];
    if format_pointer == 0 {
        return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
    }
    let Some(buffers_policy) = policy.initial_buffers.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "dsound-buffer-set-format",
            "dsound-initial-buffers-policy-disabled",
            "DirectSound initial-buffer support is disabled by the active policy",
        );
    };
    let format_bytes = match call.memory().read(GuestAddress(format_pointer), 16) {
        Ok(bytes) => bytes,
        Err(error) => return call.memory_fault(error),
    };
    let Some(format) = parse_pcm_format(&format_bytes) else {
        return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
    };
    let buffer = match call
        .runtime()
        .audio_buffer_for_com_object(call.frame().object_id)
    {
        Ok(buffer) => buffer,
        Err(error) => return call.runtime_fault(error),
    };
    if buffer.descriptor.kind == RuntimeAudioBufferKind::Secondary {
        return call.complete(
            ComReturnValue::HResult(DSOUND_DSERR_INVALIDCALL),
            Vec::new(),
        );
    }
    if format != buffers_policy.pcm_format {
        return call.fault(
            ComFaultClass::Unsupported,
            "dsound-buffer-set-format",
            "dsound-buffer-format-variant-unsupported",
            "DirectSound primary format is outside the evidenced PCM policy",
        );
    }
    let plan = match call
        .runtime()
        .plan_audio_buffer_format(RuntimeAudioBufferFormatRequest {
            buffer_com_object_id: call.frame().object_id,
            format,
        }) {
        Ok(plan) => plan,
        Err(error) => return call.runtime_fault(error),
    };
    let mut call = call;
    if let Err(error) = call.runtime_mut().commit_audio_buffer_format(plan) {
        return call.runtime_fault(error);
    }
    call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new())
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
            .runtime()
            .audio_buffer_for_com_object(call.frame().object_id)
        {
            Ok(buffer) => buffer,
            Err(error) => return call.runtime_fault(error),
        };
        let Some(format) = buffer.active_format else {
            return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
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
        return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
    }
    let buffer = match call
        .runtime()
        .audio_buffer_for_com_object(call.frame().object_id)
    {
        Ok(buffer) => buffer,
        Err(error) => return call.runtime_fault(error),
    };
    if required_flag.is_some_and(|flag| buffer.descriptor.flags & flag == 0) {
        return call.complete(
            ComReturnValue::HResult(DSOUND_DSERR_CONTROLUNAVAIL),
            Vec::new(),
        );
    }
    let plan = match call
        .runtime()
        .plan_audio_buffer_control(RuntimeAudioBufferControlRequest {
            buffer_com_object_id: call.frame().object_id,
            update,
        }) {
        Ok(plan) => plan,
        Err(error) if error.code == "audio-buffer-control-value-invalid" => {
            return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
        }
        Err(error) => return call.runtime_fault(error),
    };
    let mut call = call;
    if let Err(error) = call.runtime_mut().commit_audio_buffer_control(plan) {
        return call.runtime_fault(error);
    }
    call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new())
}

fn handle_buffer_play(
    policy: &DsoundCompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let [reserved, priority, flags] = call.frame().explicit_arguments.as_slice() else {
        unreachable!("frontend ABI preflight fixed the DirectSound argument count");
    };
    let (reserved, priority, flags) = (*reserved, *priority, *flags);
    if reserved != 0 || priority != 0 {
        return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
    }
    let Some(buffers_policy) = policy.initial_buffers.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "dsound-buffer-play",
            "dsound-initial-buffers-policy-disabled",
            "DirectSound initial-buffer support is disabled by the active policy",
        );
    };
    let buffer = match call
        .runtime()
        .audio_buffer_for_com_object(call.frame().object_id)
    {
        Ok(buffer) => buffer,
        Err(error) => return call.runtime_fault(error),
    };
    let playback = match buffer.descriptor.kind {
        RuntimeAudioBufferKind::Primary if flags == buffers_policy.primary_play_flags => {
            RuntimeAudioPlaybackState::PlayingLooping
        }
        RuntimeAudioBufferKind::Primary => {
            return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
        }
        RuntimeAudioBufferKind::Secondary => {
            let Some(transfer) = policy.buffer_transfer.as_ref() else {
                return call.fault(
                    ComFaultClass::Unsupported,
                    "dsound-buffer-play",
                    "dsound-secondary-buffer-play-unsupported",
                    "secondary DirectSound playback is disabled by the active policy",
                );
            };
            if flags & !transfer.secondary_play_flags != 0
                || (flags == 0 && !transfer.allow_secondary_once)
            {
                return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
            }
            if flags & DSOUND_DSBPLAY_LOOPING != 0 {
                RuntimeAudioPlaybackState::PlayingLooping
            } else {
                RuntimeAudioPlaybackState::PlayingOnce
            }
        }
    };
    let plan = match call
        .runtime()
        .plan_audio_buffer_play(RuntimeAudioBufferPlayRequest {
            buffer_com_object_id: call.frame().object_id,
            playback,
        }) {
        Ok(plan) => plan,
        Err(error) => return call.runtime_fault(error),
    };
    let mut call = call;
    if let Err(error) = call.runtime_mut().commit_audio_buffer_play(plan) {
        return call.runtime_fault(error);
    }
    call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new())
}

fn unsupported_buffer_variant(
    call: ComFrontendCall<'_>,
    message: impl Into<String>,
) -> ComDispatchOutcome {
    call.fault(
        ComFaultClass::Unsupported,
        "dsound-create-sound-buffer",
        "dsound-buffer-descriptor-variant-unsupported",
        message,
    )
}

fn handle_set_cooperative_level(
    policy: &DsoundCompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let [window_handle, level] = call.frame().explicit_arguments.as_slice() else {
        unreachable!("frontend ABI preflight fixed the DirectSound argument count");
    };
    let (window_handle, level) = (*window_handle, *level);
    if window_handle == 0 || !(DSOUND_DSSCL_NORMAL..=DSOUND_DSSCL_WRITEPRIMARY).contains(&level) {
        return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
    }
    let Some(cooperative) = policy.set_cooperative_level.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "dsound-set-cooperative-level",
            "dsound-cooperative-level-policy-disabled",
            "DirectSound cooperative-level support is disabled by the active policy",
        );
    };
    if level != cooperative.level {
        return call.fault(
            ComFaultClass::Unsupported,
            "dsound-set-cooperative-level",
            "dsound-cooperative-level-variant-unsupported",
            "DirectSound cooperative level is outside the evidenced priority policy",
        );
    }

    let plan =
        match call
            .runtime()
            .plan_audio_cooperative_level(RuntimeAudioCooperativeLevelRequest {
                device_com_object_id: call.frame().object_id,
                window_handle,
                level: RuntimeAudioCooperativeLevel::Priority,
            }) {
            Ok(plan) => plan,
            Err(error) if is_invalid_window_relation(&error) => {
                return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
            }
            Err(error) => return call.runtime_fault(error),
        };
    let mut call = call;
    if let Err(error) = call.runtime_mut().commit_audio_cooperative_level(plan) {
        return call.runtime_fault(error);
    }
    call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new())
}

fn is_invalid_window_relation(error: &RuntimeStateError) -> bool {
    matches!(
        (error.class, error.code),
        (
            RuntimeStateErrorClass::InvalidInput
                | RuntimeStateErrorClass::NotFound
                | RuntimeStateErrorClass::Conflict,
            "audio-window-handle-unknown"
                | "audio-window-lifecycle-state-invalid"
                | "audio-window-not-top-level"
                | "audio-window-thread-mismatch"
        )
    )
}

fn handle_direct_sound_create_8(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
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
        return ApiHandlerResult::Fault(ApiFault::unsupported(
            "dsound-direct-sound-create-8",
            "dsound-device-guid-unsupported",
            "DirectSound device GUID selection is outside the evidenced default-device band",
        ));
    }

    let output_address = GuestAddress(output_pointer);
    let installed = match session.install_audio_device_com_object_with_guest_writes(
        direct_sound8_object_spec(),
        RuntimeAudioDeviceCreateRequest {
            frontend: RuntimeAudioFrontend::DirectSound8,
            selector: RuntimeAudioDeviceSelector::DefaultPlayback,
        },
        |planned| {
            let interface = planned
                .interfaces
                .iter()
                .find(|interface| interface.iid == DSOUND_IDIRECTSOUND8_IID)
                .ok_or_else(|| {
                    RuntimeStateError::runtime_defect(
                        "dsound-api-dispatch",
                        "requested-interface-installation-missing",
                        "installed DirectSound root object omitted IDirectSound8",
                    )
                })?;
            Ok(vec![(
                output_address,
                interface.guest_pointer.0.to_le_bytes().to_vec(),
            )])
        },
    ) {
        Ok((installed, device, _guest_writes)) => {
            debug_assert_eq!(device.com_object_id, installed.object.id);
            installed
        }
        Err(error) => {
            return ApiHandlerResult::Fault(ApiFault::from_runtime(error, &contract.endpoint));
        }
    };
    let interface = installed
        .interfaces
        .iter()
        .find(|interface| interface.iid == DSOUND_IDIRECTSOUND8_IID)
        .expect("atomic DirectSound installation validated IDirectSound8");
    ApiHandlerResult::Immediate(
        ApiImmediate::new(ApiReturnValue::HResult(COM_S_OK)).with_guest_write(
            output_address,
            interface.guest_pointer.0.to_le_bytes().to_vec(),
        ),
    )
}

fn decoded_argument_missing() -> ApiHandlerResult {
    ApiHandlerResult::Fault(ApiFault::runtime_defect(
        "dsound-api-dispatch",
        "decoded-argument-missing",
        "decoded DirectSoundCreate8 frame is missing an argument",
    ))
}

fn immediate_hresult(result: ComHResult) -> ApiHandlerResult {
    ApiHandlerResult::Immediate(ApiImmediate::new(ApiReturnValue::HResult(result)))
}

pub fn dsound_contract_for_endpoint(
    endpoint: ApiEndpointIdentity,
) -> Result<Option<ApiContract>, ApiFault> {
    if !endpoint.module.eq_ignore_ascii_case("dsound.dll") {
        return Ok(None);
    }
    match endpoint.symbol {
        ApiSymbol::Ordinal {
            ordinal: DSOUND_DIRECT_SOUND_CREATE_8_ORDINAL,
        } => Ok(Some(direct_sound_create_8_contract(endpoint)?)),
        _ => Ok(None),
    }
}

pub fn direct_sound_create_8_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                optional_input_struct("deviceGuid", "GUID", 16),
                optional_output_struct("interface", "COM_INTERFACE_POINTER", 4),
                opaque_pointer("outerUnknown", true),
            ],
            return_value: ApiReturnSchema::HResult,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Disallowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: DSOUND_DIRECT_SOUND_CREATE_8_IMPLEMENTATION.to_owned(),
    })
}

pub fn direct_sound8_object_spec() -> ComObjectSpec {
    ComObjectSpec {
        class_identity: DSOUND_IDIRECTSOUND8_CLASS_IDENTITY.to_owned(),
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            ComInterfaceSpec {
                iid: DSOUND_IDIRECTSOUND8_IID,
                name: "IDirectSound8".to_owned(),
                methods: vec![
                    ComMethodSpec::query_interface(),
                    ComMethodSpec::add_ref(),
                    ComMethodSpec::release(),
                    ComMethodSpec::frontend_hresult("CreateSoundBuffer", 3),
                    ComMethodSpec::unsupported_hresult("GetCaps", 1),
                    ComMethodSpec::unsupported_hresult("DuplicateSoundBuffer", 2),
                    ComMethodSpec::frontend_hresult("SetCooperativeLevel", 2),
                    ComMethodSpec::unsupported_hresult("Compact", 0),
                    ComMethodSpec::unsupported_hresult("GetSpeakerConfig", 1),
                    ComMethodSpec::unsupported_hresult("SetSpeakerConfig", 1),
                    ComMethodSpec::unsupported_hresult("Initialize", 1),
                    ComMethodSpec::unsupported_hresult("VerifyCertification", 1),
                ],
            },
        ],
    }
}

pub fn direct_sound_buffer8_object_spec() -> ComObjectSpec {
    ComObjectSpec {
        class_identity: DSOUND_IDIRECTSOUNDBUFFER8_CLASS_IDENTITY.to_owned(),
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            ComInterfaceSpec {
                iid: DSOUND_IDIRECTSOUNDBUFFER8_IID,
                name: "IDirectSoundBuffer8".to_owned(),
                methods: vec![
                    ComMethodSpec::query_interface(),
                    ComMethodSpec::add_ref(),
                    ComMethodSpec::release(),
                    ComMethodSpec::unsupported_hresult("GetCaps", 1),
                    ComMethodSpec::unsupported_hresult("GetCurrentPosition", 2),
                    ComMethodSpec::unsupported_hresult("GetFormat", 3),
                    ComMethodSpec::unsupported_hresult("GetVolume", 1),
                    ComMethodSpec::unsupported_hresult("GetPan", 1),
                    ComMethodSpec::unsupported_hresult("GetFrequency", 1),
                    ComMethodSpec::frontend_hresult("GetStatus", 1),
                    ComMethodSpec::unsupported_hresult("Initialize", 2),
                    ComMethodSpec::frontend_hresult("Lock", 7),
                    ComMethodSpec::frontend_hresult("Play", 3),
                    ComMethodSpec::frontend_hresult("SetCurrentPosition", 1),
                    ComMethodSpec::frontend_hresult("SetFormat", 1),
                    ComMethodSpec::frontend_hresult("SetVolume", 1),
                    ComMethodSpec::frontend_hresult("SetPan", 1),
                    ComMethodSpec::frontend_hresult("SetFrequency", 1),
                    ComMethodSpec::unsupported_hresult("Stop", 0),
                    ComMethodSpec::frontend_hresult("Unlock", 4),
                    ComMethodSpec::unsupported_hresult("Restore", 0),
                    ComMethodSpec::unsupported_hresult("SetFX", 3),
                    ComMethodSpec::unsupported_hresult("AcquireResources", 3),
                    ComMethodSpec::unsupported_hresult("GetObjectInPath", 4),
                ],
            },
            ComInterfaceSpec {
                iid: DSOUND_IDIRECTSOUNDNOTIFY_IID,
                name: "IDirectSoundNotify".to_owned(),
                methods: vec![
                    ComMethodSpec::query_interface(),
                    ComMethodSpec::add_ref(),
                    ComMethodSpec::release(),
                    ComMethodSpec::frontend_hresult("SetNotificationPositions", 2),
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

fn opaque_pointer(name: &str, nullable: bool) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::OpaquePointer {
            nullable,
            alignment: 4,
        },
    }
}

fn optional_input_struct(name: &str, type_id: &str, byte_length: u32) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::Pointer {
            constraint: ApiPointerConstraint {
                nullable: true,
                alignment: 4,
                byte_length,
                pointee: ApiPointeeSchema::Struct {
                    type_id: type_id.to_owned(),
                },
                write_requirement: ApiWriteRequirement::None,
            },
        },
    }
}

fn optional_output_struct(name: &str, type_id: &str, byte_length: u32) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::Out,
        value: ApiValueSchema::Pointer {
            constraint: ApiPointerConstraint {
                nullable: true,
                alignment: 4,
                byte_length,
                pointee: ApiPointeeSchema::Struct {
                    type_id: type_id.to_owned(),
                },
                write_requirement: ApiWriteRequirement::Optional,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_policy_rejects_unmeasured_cooperative_claims() {
        let mut policy = DsoundCompatibilityPolicy::controlled_v1();
        policy
            .set_cooperative_level
            .as_mut()
            .expect("cooperative policy")
            .level = DSOUND_DSSCL_NORMAL;
        assert_eq!(
            policy.validate().unwrap_err().code,
            "dsound-cooperative-level-profile-invalid"
        );

        let mut header = DsoundCompatibilityPolicy::controlled_v1();
        header.schema_version += 1;
        assert_eq!(
            header.validate().unwrap_err().code,
            "dsound-compatibility-policy-header-invalid"
        );

        let mut buffers = DsoundCompatibilityPolicy::controlled_v1();
        buffers
            .initial_buffers
            .as_mut()
            .expect("initial-buffer policy")
            .secondary_buffer_max_bytes = 0;
        assert_eq!(
            buffers.validate().unwrap_err().code,
            "dsound-initial-buffers-profile-invalid"
        );

        let mut transfer = DsoundCompatibilityPolicy::controlled_v1();
        transfer
            .buffer_transfer
            .as_mut()
            .expect("buffer-transfer policy")
            .lock_offset = 1;
        assert_eq!(
            transfer.validate().unwrap_err().code,
            "dsound-buffer-transfer-profile-invalid"
        );
    }

    #[test]
    fn root_spec_keeps_legacy_alias_outside_the_reached_band() {
        let spec = direct_sound8_object_spec();
        assert_eq!(spec.interfaces.len(), 2);
        assert_eq!(
            spec.interfaces[0].iid,
            crate::runtime::RUNTIME_COM_IUNKNOWN_IID
        );
        assert_eq!(spec.interfaces[1].iid, DSOUND_IDIRECTSOUND8_IID);
        assert!(
            spec.interfaces
                .iter()
                .all(|interface| interface.iid != DSOUND_IDIRECTSOUND_IID)
        );
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
        assert!(
            spec.interfaces
                .iter()
                .all(|interface| interface.iid != DSOUND_IDIRECTSOUNDBUFFER_IID)
        );
        let methods = &spec.interfaces[1].methods;
        assert_eq!(methods.len(), 24);
        assert_eq!(methods[11].name, "Lock");
        assert_eq!(methods[12].name, "Play");
        assert_eq!(methods[14].name, "SetFormat");
    }

    #[test]
    fn evidenced_descriptors_and_pcm_format_parse_exactly() {
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
        assert_eq!(
            parse_pcm_format(&format),
            DsoundCompatibilityPolicy::controlled_v1()
                .initial_buffers
                .expect("initial-buffer policy")
                .pcm_format
                .into()
        );
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

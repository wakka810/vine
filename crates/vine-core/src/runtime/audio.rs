use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::{
    ProcessAddressAllocationId, RuntimeComObjectId, RuntimeStateError, RuntimeThreadId,
    RuntimeWindowId,
};
use crate::memory::{GuestAddress, GuestRange, MemoryOwnerId};

pub const RUNTIME_AUDIO_DEVICE_ID_NAMESPACE: &str = "vine.logical-audio-device.v1";
pub const RUNTIME_AUDIO_BUFFER_ID_NAMESPACE: &str = "vine.logical-audio-buffer.v1";
pub const RUNTIME_AUDIO_RESOURCE_ARENA_BASE: GuestAddress = GuestAddress(0x4500_0000);
pub const RUNTIME_AUDIO_RESOURCE_ARENA_SIZE: u64 = 0x0100_0000;
pub(crate) const RUNTIME_AUDIO_RESOURCE_ADDRESS_OWNER: &str = "runtime-audio-resource";
pub const RUNTIME_AUDIO_DEVICE_LIMIT: usize = 4_096;
pub const RUNTIME_AUDIO_BUFFER_LIMIT: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeAudioDeviceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeAudioBufferId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeAudioFrontend {
    DirectSound8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeAudioDeviceSelector {
    DefaultPlayback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeAudioCooperativeLevel {
    Normal,
    Priority,
    Exclusive,
    WritePrimary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAudioCooperativeState {
    pub window_id: RuntimeWindowId,
    pub window_handle: u32,
    pub owner_thread_id: RuntimeThreadId,
    pub level: RuntimeAudioCooperativeLevel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAudioDevice {
    pub id: RuntimeAudioDeviceId,
    pub namespace: String,
    pub frontend: RuntimeAudioFrontend,
    pub selector: RuntimeAudioDeviceSelector,
    pub com_object_id: RuntimeComObjectId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooperative: Option<RuntimeAudioCooperativeState>,
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub buffers: BTreeSet<RuntimeAudioBufferId>,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeAudioBufferKind {
    Primary,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAudioPcmFormat {
    pub channels: u16,
    pub samples_per_second: u32,
    pub average_bytes_per_second: u32,
    pub block_align: u16,
    pub bits_per_sample: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAudioBufferDescriptor {
    pub kind: RuntimeAudioBufferKind,
    pub flags: u32,
    pub byte_length: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<RuntimeAudioPcmFormat>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAudioBufferBacking {
    pub address_allocation_id: ProcessAddressAllocationId,
    pub memory_owner: MemoryOwnerId,
    pub range: GuestRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAudioBufferSegment {
    pub guest_address: GuestAddress,
    pub byte_length: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAudioBufferLock {
    pub offset: u32,
    pub byte_length: u32,
    pub flags: u32,
    pub first_segment: RuntimeAudioBufferSegment,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub second_segment: Option<RuntimeAudioBufferSegment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeAudioPlaybackState {
    Stopped,
    PlayingLooping,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAudioBuffer {
    pub id: RuntimeAudioBufferId,
    pub namespace: String,
    pub device_id: RuntimeAudioDeviceId,
    pub com_object_id: RuntimeComObjectId,
    pub descriptor: RuntimeAudioBufferDescriptor,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backing: Option<RuntimeAudioBufferBacking>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_format: Option<RuntimeAudioPcmFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_lock: Option<RuntimeAudioBufferLock>,
    pub content_generation: u64,
    pub playback: RuntimeAudioPlaybackState,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeAudioDeviceCreateRequest {
    pub frontend: RuntimeAudioFrontend,
    pub selector: RuntimeAudioDeviceSelector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeAudioBufferCreateRequest {
    pub device_com_object_id: RuntimeComObjectId,
    pub descriptor: RuntimeAudioBufferDescriptor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeAudioBufferFormatRequest {
    pub buffer_com_object_id: RuntimeComObjectId,
    pub format: RuntimeAudioPcmFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeAudioBufferPlayRequest {
    pub buffer_com_object_id: RuntimeComObjectId,
    pub playback: RuntimeAudioPlaybackState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeAudioBufferLockRequest {
    pub buffer_com_object_id: RuntimeComObjectId,
    pub offset: u32,
    pub byte_length: u32,
    pub flags: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeAudioBufferUnlockRequest {
    pub buffer_com_object_id: RuntimeComObjectId,
    pub first_segment: RuntimeAudioBufferSegment,
    pub second_segment: Option<RuntimeAudioBufferSegment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeAudioCooperativeLevelRequest {
    pub device_com_object_id: RuntimeComObjectId,
    pub window_handle: u32,
    pub level: RuntimeAudioCooperativeLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeAudioWindowTarget {
    pub window_id: RuntimeWindowId,
    pub window_handle: u32,
    pub owner_thread_id: RuntimeThreadId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeAudioCooperativeLevelUpdateRequest {
    pub device_com_object_id: RuntimeComObjectId,
    pub target: RuntimeAudioWindowTarget,
    pub level: RuntimeAudioCooperativeLevel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeAudioDeviceCreationPlan {
    expected_generation: u64,
    next_generation: u64,
    id: RuntimeAudioDeviceId,
    request: RuntimeAudioDeviceCreateRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeAudioBufferCreationPlan {
    expected_generation: u64,
    next_generation: u64,
    id: RuntimeAudioBufferId,
    device_id: RuntimeAudioDeviceId,
    expected_device_generation: u64,
    next_device_generation: u64,
    request: RuntimeAudioBufferCreateRequest,
}

impl RuntimeAudioBufferCreationPlan {
    pub(crate) fn id(&self) -> RuntimeAudioBufferId {
        self.id
    }

    pub(crate) fn device_id(&self) -> RuntimeAudioDeviceId {
        self.device_id
    }

    pub(crate) fn descriptor(&self) -> RuntimeAudioBufferDescriptor {
        self.request.descriptor
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeAudioBufferFormatPlan {
    expected_generation: u64,
    next_generation: u64,
    buffer_id: RuntimeAudioBufferId,
    expected_buffer_generation: u64,
    next_buffer_generation: u64,
    previous: Option<RuntimeAudioPcmFormat>,
    next: RuntimeAudioPcmFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeAudioBufferPlayPlan {
    expected_generation: u64,
    next_generation: u64,
    buffer_id: RuntimeAudioBufferId,
    expected_buffer_generation: u64,
    next_buffer_generation: u64,
    previous: RuntimeAudioPlaybackState,
    next: RuntimeAudioPlaybackState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeAudioBufferBackingPlan {
    pub address_allocation_id: ProcessAddressAllocationId,
    pub range: GuestRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeAudioBufferLockPlan {
    expected_generation: u64,
    next_generation: u64,
    buffer_id: RuntimeAudioBufferId,
    expected_buffer_generation: u64,
    next_buffer_generation: u64,
    previous_backing: Option<RuntimeAudioBufferBacking>,
    backing_installation: Option<RuntimeAudioBufferBackingPlan>,
    lock: RuntimeAudioBufferLock,
}

impl RuntimeAudioBufferLockPlan {
    pub(crate) fn buffer_id(&self) -> RuntimeAudioBufferId {
        self.buffer_id
    }

    pub(crate) fn backing_installation(&self) -> Option<RuntimeAudioBufferBackingPlan> {
        self.backing_installation
    }

    pub(crate) fn lock(&self) -> RuntimeAudioBufferLock {
        self.lock
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeAudioBufferUnlockPlan {
    expected_generation: u64,
    next_generation: u64,
    buffer_id: RuntimeAudioBufferId,
    expected_buffer_generation: u64,
    next_buffer_generation: u64,
    expected_content_generation: u64,
    next_content_generation: u64,
    lock: RuntimeAudioBufferLock,
}

impl RuntimeAudioBufferUnlockPlan {
    pub(crate) fn buffer_id(&self) -> RuntimeAudioBufferId {
        self.buffer_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeAudioCooperativeLevelPlan {
    expected_generation: u64,
    next_generation: u64,
    device_id: RuntimeAudioDeviceId,
    expected_device_generation: u64,
    next_device_generation: u64,
    previous: Option<RuntimeAudioCooperativeState>,
    next: RuntimeAudioCooperativeState,
}

impl RuntimeAudioCooperativeLevelPlan {
    pub(super) fn next_state(&self) -> RuntimeAudioCooperativeState {
        self.next
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAudioCooperativeLevelResult {
    pub device_id: RuntimeAudioDeviceId,
    pub previous: Option<RuntimeAudioCooperativeState>,
    pub current: RuntimeAudioCooperativeState,
    pub changed: bool,
    pub device_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAudioBufferFormatResult {
    pub buffer_id: RuntimeAudioBufferId,
    pub previous: Option<RuntimeAudioPcmFormat>,
    pub current: RuntimeAudioPcmFormat,
    pub changed: bool,
    pub buffer_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAudioBufferPlayResult {
    pub buffer_id: RuntimeAudioBufferId,
    pub previous: RuntimeAudioPlaybackState,
    pub current: RuntimeAudioPlaybackState,
    pub changed: bool,
    pub buffer_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAudioBufferUnlockResult {
    pub buffer_id: RuntimeAudioBufferId,
    pub buffer_generation: u64,
    pub content_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeAudioComRetirementPlan {
    expected_generation: u64,
    next_generation: u64,
    device_associations: Vec<(RuntimeComObjectId, RuntimeAudioDeviceId)>,
    buffer_associations: Vec<(RuntimeComObjectId, RuntimeAudioBufferId)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeAudio {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    devices: BTreeMap<RuntimeAudioDeviceId, RuntimeAudioDevice>,
    #[serde(skip)]
    device_com_object_index: BTreeMap<RuntimeComObjectId, RuntimeAudioDeviceId>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    buffers: BTreeMap<RuntimeAudioBufferId, RuntimeAudioBuffer>,
    #[serde(skip)]
    buffer_com_object_index: BTreeMap<RuntimeComObjectId, RuntimeAudioBufferId>,
    next_device_id: u64,
    next_buffer_id: u64,
    generation: u64,
}

impl Default for RuntimeAudio {
    fn default() -> Self {
        Self {
            devices: BTreeMap::new(),
            device_com_object_index: BTreeMap::new(),
            buffers: BTreeMap::new(),
            buffer_com_object_index: BTreeMap::new(),
            next_device_id: 1,
            next_buffer_id: 1,
            generation: 0,
        }
    }
}

impl RuntimeAudio {
    pub(super) fn is_empty(&self) -> bool {
        self.devices.is_empty() && self.buffers.is_empty()
    }

    pub(super) fn devices(&self) -> &BTreeMap<RuntimeAudioDeviceId, RuntimeAudioDevice> {
        &self.devices
    }

    pub(super) fn buffers(&self) -> &BTreeMap<RuntimeAudioBufferId, RuntimeAudioBuffer> {
        &self.buffers
    }

    pub(super) fn device(
        &self,
        id: RuntimeAudioDeviceId,
    ) -> Result<&RuntimeAudioDevice, RuntimeStateError> {
        self.devices.get(&id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-audio",
                "audio-device-unknown",
                "logical audio device identity is not live",
            )
        })
    }

    pub(super) fn device_by_com_object(
        &self,
        com_object_id: RuntimeComObjectId,
    ) -> Result<&RuntimeAudioDevice, RuntimeStateError> {
        let id = self
            .device_com_object_index
            .get(&com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-audio",
                    "audio-device-com-object-unknown",
                    "logical audio device has no live COM object association",
                )
            })?;
        let device = self.device(id)?;
        if device.com_object_id != com_object_id {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-audio",
                "audio-device-com-object-index-mismatch",
                "logical audio device COM index disagrees with its authoritative record",
            ));
        }
        Ok(device)
    }

    pub(super) fn buffer(
        &self,
        id: RuntimeAudioBufferId,
    ) -> Result<&RuntimeAudioBuffer, RuntimeStateError> {
        self.buffers.get(&id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-audio",
                "audio-buffer-unknown",
                "logical audio buffer identity is not live",
            )
        })
    }

    pub(super) fn buffer_by_com_object(
        &self,
        com_object_id: RuntimeComObjectId,
    ) -> Result<&RuntimeAudioBuffer, RuntimeStateError> {
        let id = self
            .buffer_com_object_index
            .get(&com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-audio",
                    "audio-buffer-com-object-unknown",
                    "logical audio buffer has no live COM object association",
                )
            })?;
        let buffer = self.buffer(id)?;
        if buffer.com_object_id != com_object_id {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-audio",
                "audio-buffer-com-object-index-mismatch",
                "logical audio buffer COM index disagrees with its authoritative record",
            ));
        }
        Ok(buffer)
    }

    pub(super) fn plan_device_creation(
        &self,
        request: RuntimeAudioDeviceCreateRequest,
    ) -> Result<RuntimeAudioDeviceCreationPlan, RuntimeStateError> {
        validate_device_create_request(request)?;
        if self.devices.len() >= RUNTIME_AUDIO_DEVICE_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-audio",
                "audio-device-capacity-reached",
                "logical audio device capacity is exhausted",
            ));
        }
        let id = RuntimeAudioDeviceId(self.next_device_id);
        if id.0 == 0 || self.devices.contains_key(&id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-audio",
                "audio-device-id-collision",
                "logical audio device allocator selected a live identity",
            ));
        }
        Ok(RuntimeAudioDeviceCreationPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            id,
            request,
        })
    }

    pub(super) fn preflight_device_creation(
        &self,
        plan: &RuntimeAudioDeviceCreationPlan,
        com_object_id: RuntimeComObjectId,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.next_device_id != plan.id.0
            || self.devices.len() >= RUNTIME_AUDIO_DEVICE_LIMIT
            || self.devices.contains_key(&plan.id)
            || com_object_id.0 == 0
            || self.device_com_object_index.contains_key(&com_object_id)
        {
            return Err(stale_plan("audio-device-creation-plan-stale"));
        }
        validate_device_create_request(plan.request)
    }

    pub(super) fn commit_device_creation(
        &mut self,
        plan: RuntimeAudioDeviceCreationPlan,
        com_object_id: RuntimeComObjectId,
    ) -> RuntimeAudioDevice {
        debug_assert!(self.preflight_device_creation(&plan, com_object_id).is_ok());
        let next_device_id = plan
            .id
            .0
            .checked_add(1)
            .expect("preflighted audio device ID must advance");
        let device = RuntimeAudioDevice {
            id: plan.id,
            namespace: RUNTIME_AUDIO_DEVICE_ID_NAMESPACE.to_owned(),
            frontend: plan.request.frontend,
            selector: plan.request.selector,
            com_object_id,
            cooperative: None,
            buffers: BTreeSet::new(),
            generation: plan.next_generation,
        };
        self.device_com_object_index
            .insert(com_object_id, device.id);
        self.devices.insert(device.id, device.clone());
        self.next_device_id = next_device_id;
        self.generation = plan.next_generation;
        device
    }

    pub(super) fn plan_buffer_creation(
        &self,
        request: RuntimeAudioBufferCreateRequest,
    ) -> Result<RuntimeAudioBufferCreationPlan, RuntimeStateError> {
        validate_buffer_descriptor(request.descriptor)?;
        if self.buffers.len() >= RUNTIME_AUDIO_BUFFER_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-audio",
                "audio-buffer-capacity-reached",
                "logical audio buffer capacity is exhausted",
            ));
        }
        let device = self.device_by_com_object(request.device_com_object_id)?;
        require_priority_cooperative_state(device)?;
        if request.descriptor.kind == RuntimeAudioBufferKind::Primary
            && device.buffers.iter().any(|id| {
                self.buffers
                    .get(id)
                    .is_some_and(|buffer| buffer.descriptor.kind == RuntimeAudioBufferKind::Primary)
            })
        {
            return Err(RuntimeStateError::conflict(
                "runtime-audio",
                "audio-primary-buffer-already-live",
                "logical audio device already owns a live primary buffer",
            ));
        }
        let id = RuntimeAudioBufferId(self.next_buffer_id);
        if id.0 == 0 || self.buffers.contains_key(&id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-audio",
                "audio-buffer-id-collision",
                "logical audio buffer allocator selected a live identity",
            ));
        }
        Ok(RuntimeAudioBufferCreationPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            id,
            device_id: device.id,
            expected_device_generation: device.generation,
            next_device_generation: checked_device_generation(device.generation)?,
            request,
        })
    }

    pub(super) fn preflight_buffer_creation(
        &self,
        plan: &RuntimeAudioBufferCreationPlan,
    ) -> Result<(), RuntimeStateError> {
        let Some(device) = self.devices.get(&plan.device_id) else {
            return Err(stale_plan("audio-buffer-creation-plan-stale"));
        };
        if self.generation != plan.expected_generation
            || self.next_buffer_id != plan.id.0
            || self.buffers.len() >= RUNTIME_AUDIO_BUFFER_LIMIT
            || self.buffers.contains_key(&plan.id)
            || device.generation != plan.expected_device_generation
            || device.generation.checked_add(1) != Some(plan.next_device_generation)
            || self
                .device_com_object_index
                .get(&plan.request.device_com_object_id)
                != Some(&device.id)
            || validate_buffer_descriptor(plan.request.descriptor).is_err()
            || require_priority_cooperative_state(device).is_err()
            || (plan.request.descriptor.kind == RuntimeAudioBufferKind::Primary
                && device.buffers.iter().any(|id| {
                    self.buffers.get(id).is_some_and(|buffer| {
                        buffer.descriptor.kind == RuntimeAudioBufferKind::Primary
                    })
                }))
        {
            return Err(stale_plan("audio-buffer-creation-plan-stale"));
        }
        Ok(())
    }

    pub(super) fn commit_buffer_creation(
        &mut self,
        plan: RuntimeAudioBufferCreationPlan,
        com_object_id: RuntimeComObjectId,
    ) -> RuntimeAudioBuffer {
        debug_assert!(self.preflight_buffer_creation(&plan).is_ok());
        debug_assert_ne!(com_object_id.0, 0);
        debug_assert!(!self.device_com_object_index.contains_key(&com_object_id));
        debug_assert!(!self.buffer_com_object_index.contains_key(&com_object_id));
        let next_buffer_id = plan
            .id
            .0
            .checked_add(1)
            .expect("preflighted audio buffer ID must advance");
        let buffer = RuntimeAudioBuffer {
            id: plan.id,
            namespace: RUNTIME_AUDIO_BUFFER_ID_NAMESPACE.to_owned(),
            device_id: plan.device_id,
            com_object_id,
            descriptor: plan.request.descriptor,
            backing: None,
            active_format: plan.request.descriptor.format,
            active_lock: None,
            content_generation: 0,
            playback: RuntimeAudioPlaybackState::Stopped,
            generation: plan.next_generation,
        };
        let device = self
            .devices
            .get_mut(&plan.device_id)
            .expect("preflighted audio device disappeared");
        device.buffers.insert(buffer.id);
        device.generation = plan.next_device_generation;
        self.buffer_com_object_index
            .insert(com_object_id, buffer.id);
        self.buffers.insert(buffer.id, buffer.clone());
        self.next_buffer_id = next_buffer_id;
        self.generation = plan.next_generation;
        buffer
    }

    pub(super) fn plan_buffer_format(
        &self,
        request: RuntimeAudioBufferFormatRequest,
    ) -> Result<RuntimeAudioBufferFormatPlan, RuntimeStateError> {
        validate_pcm_format(request.format)?;
        let buffer = self.buffer_by_com_object(request.buffer_com_object_id)?;
        if buffer.descriptor.kind != RuntimeAudioBufferKind::Primary {
            return Err(RuntimeStateError::invalid_input(
                "runtime-audio",
                "audio-secondary-buffer-format-control-unavailable",
                "secondary logical audio buffers do not expose primary format control",
            ));
        }
        let device = self.device(buffer.device_id)?;
        require_priority_cooperative_state(device)?;
        let changed = buffer.active_format != Some(request.format);
        Ok(RuntimeAudioBufferFormatPlan {
            expected_generation: self.generation,
            next_generation: if changed {
                checked_next_generation(self.generation)?
            } else {
                self.generation
            },
            buffer_id: buffer.id,
            expected_buffer_generation: buffer.generation,
            next_buffer_generation: if changed {
                checked_buffer_generation(buffer.generation)?
            } else {
                buffer.generation
            },
            previous: buffer.active_format,
            next: request.format,
        })
    }

    pub(super) fn preflight_buffer_format(
        &self,
        plan: &RuntimeAudioBufferFormatPlan,
    ) -> Result<(), RuntimeStateError> {
        let Some(buffer) = self.buffers.get(&plan.buffer_id) else {
            return Err(stale_plan("audio-buffer-format-plan-stale"));
        };
        let changed = plan.previous != Some(plan.next);
        let device = self.device(buffer.device_id)?;
        if self.generation != plan.expected_generation
            || buffer.generation != plan.expected_buffer_generation
            || buffer.active_format != plan.previous
            || buffer.descriptor.kind != RuntimeAudioBufferKind::Primary
            || self.buffer_com_object_index.get(&buffer.com_object_id) != Some(&buffer.id)
            || validate_pcm_format(plan.next).is_err()
            || require_priority_cooperative_state(device).is_err()
            || (changed
                && (self.generation.checked_add(1) != Some(plan.next_generation)
                    || buffer.generation.checked_add(1) != Some(plan.next_buffer_generation)))
            || (!changed
                && (plan.next_generation != self.generation
                    || plan.next_buffer_generation != buffer.generation))
        {
            return Err(stale_plan("audio-buffer-format-plan-stale"));
        }
        Ok(())
    }

    pub(super) fn commit_buffer_format(
        &mut self,
        plan: RuntimeAudioBufferFormatPlan,
    ) -> RuntimeAudioBufferFormatResult {
        debug_assert!(self.preflight_buffer_format(&plan).is_ok());
        let changed = plan.previous != Some(plan.next);
        let buffer = self
            .buffers
            .get_mut(&plan.buffer_id)
            .expect("preflighted audio buffer disappeared");
        if changed {
            buffer.active_format = Some(plan.next);
            buffer.generation = plan.next_buffer_generation;
            self.generation = plan.next_generation;
        }
        RuntimeAudioBufferFormatResult {
            buffer_id: buffer.id,
            previous: plan.previous,
            current: plan.next,
            changed,
            buffer_generation: buffer.generation,
        }
    }

    pub(super) fn plan_buffer_lock(
        &self,
        request: RuntimeAudioBufferLockRequest,
        backing_installation: Option<RuntimeAudioBufferBackingPlan>,
    ) -> Result<RuntimeAudioBufferLockPlan, RuntimeStateError> {
        let buffer = self.buffer_by_com_object(request.buffer_com_object_id)?;
        if buffer.descriptor.kind != RuntimeAudioBufferKind::Secondary
            || buffer.active_format.is_none()
            || request.offset != 0
            || request.byte_length != buffer.descriptor.byte_length
            || request.flags != 0
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-audio",
                "audio-buffer-lock-variant-unsupported",
                "bounded audio locking supports only the reached full-range secondary-buffer variant",
            ));
        }
        if buffer.active_lock.is_some() {
            return Err(RuntimeStateError::conflict(
                "runtime-audio",
                "audio-buffer-already-locked",
                "logical audio buffer already has an active lock",
            ));
        }
        let backing_start = match (buffer.backing, backing_installation) {
            (Some(backing), None)
                if backing.address_allocation_id.0 != 0
                    && backing.memory_owner.0 != 0
                    && backing.range.byte_length() >= u64::from(buffer.descriptor.byte_length) =>
            {
                backing.range.start()
            }
            (None, Some(backing))
                if backing.address_allocation_id.0 != 0
                    && backing.range.start().0 != 0
                    && backing.range.byte_length() >= u64::from(buffer.descriptor.byte_length) =>
            {
                backing.range.start()
            }
            _ => {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-audio",
                    "audio-buffer-backing-plan-invalid",
                    "logical audio Lock requires exactly one existing or planned sample backing",
                ));
            }
        };
        let lock = RuntimeAudioBufferLock {
            offset: request.offset,
            byte_length: request.byte_length,
            flags: request.flags,
            first_segment: RuntimeAudioBufferSegment {
                guest_address: backing_start,
                byte_length: request.byte_length,
            },
            second_segment: None,
        };
        Ok(RuntimeAudioBufferLockPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            buffer_id: buffer.id,
            expected_buffer_generation: buffer.generation,
            next_buffer_generation: checked_buffer_generation(buffer.generation)?,
            previous_backing: buffer.backing,
            backing_installation,
            lock,
        })
    }

    pub(super) fn preflight_buffer_lock(
        &self,
        plan: &RuntimeAudioBufferLockPlan,
        backing_memory_owner: Option<MemoryOwnerId>,
    ) -> Result<(), RuntimeStateError> {
        let Some(buffer) = self.buffers.get(&plan.buffer_id) else {
            return Err(stale_plan("audio-buffer-lock-plan-stale"));
        };
        let backing_shape_matches = match (
            buffer.backing,
            plan.previous_backing,
            plan.backing_installation,
            backing_memory_owner,
        ) {
            (Some(current), Some(previous), None, None) => current == previous,
            (None, None, Some(backing), Some(memory_owner)) => {
                backing.address_allocation_id.0 != 0
                    && backing.range.start() == plan.lock.first_segment.guest_address
                    && backing.range.byte_length() >= u64::from(plan.lock.byte_length)
                    && memory_owner.0 != 0
            }
            _ => false,
        };
        if self.generation != plan.expected_generation
            || buffer.generation != plan.expected_buffer_generation
            || buffer.descriptor.kind != RuntimeAudioBufferKind::Secondary
            || buffer.active_format.is_none()
            || buffer.active_lock.is_some()
            || buffer.backing != plan.previous_backing
            || !backing_shape_matches
            || plan.lock.offset != 0
            || plan.lock.byte_length != buffer.descriptor.byte_length
            || plan.lock.flags != 0
            || plan.lock.first_segment.byte_length != buffer.descriptor.byte_length
            || plan.lock.second_segment.is_some()
            || self.buffer_com_object_index.get(&buffer.com_object_id) != Some(&buffer.id)
            || self.generation.checked_add(1) != Some(plan.next_generation)
            || buffer.generation.checked_add(1) != Some(plan.next_buffer_generation)
        {
            return Err(stale_plan("audio-buffer-lock-plan-stale"));
        }
        Ok(())
    }

    pub(super) fn commit_buffer_lock(
        &mut self,
        plan: RuntimeAudioBufferLockPlan,
        backing_memory_owner: Option<MemoryOwnerId>,
    ) -> RuntimeAudioBufferLock {
        debug_assert!(
            self.preflight_buffer_lock(&plan, backing_memory_owner)
                .is_ok()
        );
        let buffer = self
            .buffers
            .get_mut(&plan.buffer_id)
            .expect("preflighted audio buffer disappeared");
        if let Some(backing) = plan.backing_installation {
            buffer.backing = Some(RuntimeAudioBufferBacking {
                address_allocation_id: backing.address_allocation_id,
                memory_owner: backing_memory_owner
                    .expect("preflighted audio backing omitted its memory owner"),
                range: backing.range,
            });
        }
        buffer.active_lock = Some(plan.lock);
        buffer.generation = plan.next_buffer_generation;
        self.generation = plan.next_generation;
        plan.lock
    }

    pub(super) fn plan_buffer_unlock(
        &self,
        request: RuntimeAudioBufferUnlockRequest,
    ) -> Result<RuntimeAudioBufferUnlockPlan, RuntimeStateError> {
        let buffer = self.buffer_by_com_object(request.buffer_com_object_id)?;
        let lock = buffer.active_lock.ok_or_else(|| {
            RuntimeStateError::conflict(
                "runtime-audio",
                "audio-buffer-not-locked",
                "logical audio buffer has no active lock",
            )
        })?;
        if request.first_segment != lock.first_segment
            || request.second_segment != lock.second_segment
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-audio",
                "audio-buffer-unlock-segments-mismatch",
                "audio Unlock segments do not match the active Lock",
            ));
        }
        Ok(RuntimeAudioBufferUnlockPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            buffer_id: buffer.id,
            expected_buffer_generation: buffer.generation,
            next_buffer_generation: checked_buffer_generation(buffer.generation)?,
            expected_content_generation: buffer.content_generation,
            next_content_generation: buffer.content_generation.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-audio",
                    "audio-buffer-content-generation-exhausted",
                    "logical audio buffer content generation is exhausted",
                )
            })?,
            lock,
        })
    }

    pub(super) fn preflight_buffer_unlock(
        &self,
        plan: &RuntimeAudioBufferUnlockPlan,
    ) -> Result<(), RuntimeStateError> {
        let Some(buffer) = self.buffers.get(&plan.buffer_id) else {
            return Err(stale_plan("audio-buffer-unlock-plan-stale"));
        };
        if self.generation != plan.expected_generation
            || buffer.generation != plan.expected_buffer_generation
            || buffer.content_generation != plan.expected_content_generation
            || buffer.active_lock != Some(plan.lock)
            || buffer.backing.is_none()
            || self.buffer_com_object_index.get(&buffer.com_object_id) != Some(&buffer.id)
            || self.generation.checked_add(1) != Some(plan.next_generation)
            || buffer.generation.checked_add(1) != Some(plan.next_buffer_generation)
            || buffer.content_generation.checked_add(1) != Some(plan.next_content_generation)
        {
            return Err(stale_plan("audio-buffer-unlock-plan-stale"));
        }
        Ok(())
    }

    pub(super) fn commit_buffer_unlock(
        &mut self,
        plan: RuntimeAudioBufferUnlockPlan,
    ) -> RuntimeAudioBufferUnlockResult {
        debug_assert!(self.preflight_buffer_unlock(&plan).is_ok());
        let buffer = self
            .buffers
            .get_mut(&plan.buffer_id)
            .expect("preflighted audio buffer disappeared");
        buffer.active_lock = None;
        buffer.content_generation = plan.next_content_generation;
        buffer.generation = plan.next_buffer_generation;
        self.generation = plan.next_generation;
        RuntimeAudioBufferUnlockResult {
            buffer_id: buffer.id,
            buffer_generation: buffer.generation,
            content_generation: buffer.content_generation,
        }
    }

    pub(super) fn plan_buffer_play(
        &self,
        request: RuntimeAudioBufferPlayRequest,
    ) -> Result<RuntimeAudioBufferPlayPlan, RuntimeStateError> {
        let buffer = self.buffer_by_com_object(request.buffer_com_object_id)?;
        if request.playback != RuntimeAudioPlaybackState::PlayingLooping
            || !buffer_ready_for_looping_play(buffer)
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-audio",
                "audio-buffer-play-state-invalid",
                "logical audio playback requires an initialized unlocked buffer and looping state",
            ));
        }
        require_priority_cooperative_state(self.device(buffer.device_id)?)?;
        let changed = buffer.playback != request.playback;
        Ok(RuntimeAudioBufferPlayPlan {
            expected_generation: self.generation,
            next_generation: if changed {
                checked_next_generation(self.generation)?
            } else {
                self.generation
            },
            buffer_id: buffer.id,
            expected_buffer_generation: buffer.generation,
            next_buffer_generation: if changed {
                checked_buffer_generation(buffer.generation)?
            } else {
                buffer.generation
            },
            previous: buffer.playback,
            next: request.playback,
        })
    }

    pub(super) fn preflight_buffer_play(
        &self,
        plan: &RuntimeAudioBufferPlayPlan,
    ) -> Result<(), RuntimeStateError> {
        let Some(buffer) = self.buffers.get(&plan.buffer_id) else {
            return Err(stale_plan("audio-buffer-play-plan-stale"));
        };
        let changed = plan.previous != plan.next;
        let device = self.device(buffer.device_id)?;
        if self.generation != plan.expected_generation
            || buffer.generation != plan.expected_buffer_generation
            || buffer.playback != plan.previous
            || !buffer_ready_for_looping_play(buffer)
            || plan.next != RuntimeAudioPlaybackState::PlayingLooping
            || self.buffer_com_object_index.get(&buffer.com_object_id) != Some(&buffer.id)
            || require_priority_cooperative_state(device).is_err()
            || (changed
                && (self.generation.checked_add(1) != Some(plan.next_generation)
                    || buffer.generation.checked_add(1) != Some(plan.next_buffer_generation)))
            || (!changed
                && (plan.next_generation != self.generation
                    || plan.next_buffer_generation != buffer.generation))
        {
            return Err(stale_plan("audio-buffer-play-plan-stale"));
        }
        Ok(())
    }

    pub(super) fn commit_buffer_play(
        &mut self,
        plan: RuntimeAudioBufferPlayPlan,
    ) -> RuntimeAudioBufferPlayResult {
        debug_assert!(self.preflight_buffer_play(&plan).is_ok());
        let changed = plan.previous != plan.next;
        let buffer = self
            .buffers
            .get_mut(&plan.buffer_id)
            .expect("preflighted audio buffer disappeared");
        if changed {
            buffer.playback = plan.next;
            buffer.generation = plan.next_buffer_generation;
            self.generation = plan.next_generation;
        }
        RuntimeAudioBufferPlayResult {
            buffer_id: buffer.id,
            previous: plan.previous,
            current: plan.next,
            changed,
            buffer_generation: buffer.generation,
        }
    }

    pub(super) fn plan_cooperative_level(
        &self,
        request: RuntimeAudioCooperativeLevelUpdateRequest,
    ) -> Result<RuntimeAudioCooperativeLevelPlan, RuntimeStateError> {
        validate_cooperative_target(request.target)?;
        let device = self.device_by_com_object(request.device_com_object_id)?;
        let next = RuntimeAudioCooperativeState {
            window_id: request.target.window_id,
            window_handle: request.target.window_handle,
            owner_thread_id: request.target.owner_thread_id,
            level: request.level,
        };
        let changed = device.cooperative != Some(next);
        let next_generation = if changed {
            checked_next_generation(self.generation)?
        } else {
            self.generation
        };
        let next_device_generation = if changed {
            device.generation.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-audio",
                    "audio-device-generation-exhausted",
                    "logical audio device generation is exhausted",
                )
            })?
        } else {
            device.generation
        };
        Ok(RuntimeAudioCooperativeLevelPlan {
            expected_generation: self.generation,
            next_generation,
            device_id: device.id,
            expected_device_generation: device.generation,
            next_device_generation,
            previous: device.cooperative,
            next,
        })
    }

    pub(super) fn preflight_cooperative_level(
        &self,
        plan: &RuntimeAudioCooperativeLevelPlan,
    ) -> Result<(), RuntimeStateError> {
        let Some(device) = self.devices.get(&plan.device_id) else {
            return Err(stale_plan("audio-cooperative-level-plan-stale"));
        };
        let changed = plan.previous != Some(plan.next);
        if self.generation != plan.expected_generation
            || device.generation != plan.expected_device_generation
            || device.cooperative != plan.previous
            || self.device_com_object_index.get(&device.com_object_id) != Some(&device.id)
            || (changed
                && (self.generation.checked_add(1) != Some(plan.next_generation)
                    || device.generation.checked_add(1) != Some(plan.next_device_generation)))
            || (!changed
                && (plan.next_generation != self.generation
                    || plan.next_device_generation != device.generation))
            || validate_cooperative_state(plan.next).is_err()
        {
            return Err(stale_plan("audio-cooperative-level-plan-stale"));
        }
        Ok(())
    }

    pub(super) fn commit_cooperative_level(
        &mut self,
        plan: RuntimeAudioCooperativeLevelPlan,
    ) -> RuntimeAudioCooperativeLevelResult {
        debug_assert!(self.preflight_cooperative_level(&plan).is_ok());
        let changed = plan.previous != Some(plan.next);
        let device = self
            .devices
            .get_mut(&plan.device_id)
            .expect("preflighted audio device disappeared");
        if changed {
            device.cooperative = Some(plan.next);
            device.generation = plan.next_device_generation;
            self.generation = plan.next_generation;
        }
        RuntimeAudioCooperativeLevelResult {
            device_id: device.id,
            previous: plan.previous,
            current: plan.next,
            changed,
            device_generation: device.generation,
        }
    }

    pub(crate) fn plan_com_object_retirements(
        &self,
        com_object_ids: &BTreeSet<RuntimeComObjectId>,
    ) -> Result<Option<RuntimeAudioComRetirementPlan>, RuntimeStateError> {
        let device_associations = com_object_ids
            .iter()
            .filter_map(|com_object_id| {
                self.device_com_object_index
                    .get(com_object_id)
                    .copied()
                    .map(|device_id| (*com_object_id, device_id))
            })
            .collect::<Vec<_>>();
        let buffer_associations = com_object_ids
            .iter()
            .filter_map(|com_object_id| {
                self.buffer_com_object_index
                    .get(com_object_id)
                    .copied()
                    .map(|buffer_id| (*com_object_id, buffer_id))
            })
            .collect::<Vec<_>>();
        if device_associations.is_empty() && buffer_associations.is_empty() {
            return Ok(None);
        }
        self.validate_retirement_associations(
            &device_associations,
            &buffer_associations,
            com_object_ids,
        )?;
        Ok(Some(RuntimeAudioComRetirementPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            device_associations,
            buffer_associations,
        }))
    }

    pub(crate) fn buffer_backings_for_retirement(
        &self,
        plan: &RuntimeAudioComRetirementPlan,
    ) -> Result<Vec<(RuntimeAudioBufferBacking, u32)>, RuntimeStateError> {
        self.preflight_com_object_retirement(plan)?;
        Ok(plan
            .buffer_associations
            .iter()
            .filter_map(|(_, buffer_id)| {
                let buffer = self
                    .buffers
                    .get(buffer_id)
                    .expect("validated retiring audio buffer disappeared");
                buffer
                    .backing
                    .map(|backing| (backing, buffer.descriptor.byte_length))
            })
            .collect())
    }

    pub(crate) fn preflight_com_object_retirement(
        &self,
        plan: &RuntimeAudioComRetirementPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self
                .validate_retirement_associations(
                    &plan.device_associations,
                    &plan.buffer_associations,
                    &plan
                        .device_associations
                        .iter()
                        .map(|(com_object_id, _)| *com_object_id)
                        .chain(
                            plan.buffer_associations
                                .iter()
                                .map(|(com_object_id, _)| *com_object_id),
                        )
                        .collect(),
                )
                .is_err()
        {
            return Err(stale_plan("audio-com-retirement-plan-stale"));
        }
        Ok(())
    }

    pub(crate) fn commit_com_object_retirement(&mut self, plan: RuntimeAudioComRetirementPlan) {
        debug_assert!(self.preflight_com_object_retirement(&plan).is_ok());
        let retiring_devices = plan
            .device_associations
            .iter()
            .map(|(_, device_id)| *device_id)
            .collect::<BTreeSet<_>>();
        for (com_object_id, buffer_id) in plan.buffer_associations {
            let buffer = self
                .buffers
                .remove(&buffer_id)
                .expect("preflighted logical audio buffer disappeared");
            self.buffer_com_object_index.remove(&com_object_id);
            if !retiring_devices.contains(&buffer.device_id) {
                let device = self
                    .devices
                    .get_mut(&buffer.device_id)
                    .expect("preflighted logical audio buffer parent disappeared");
                device.buffers.remove(&buffer.id);
                device.generation = device
                    .generation
                    .checked_add(1)
                    .expect("preflighted audio device generation must advance");
            }
        }
        for (com_object_id, device_id) in plan.device_associations {
            self.device_com_object_index.remove(&com_object_id);
            self.devices
                .remove(&device_id)
                .expect("preflighted logical audio device disappeared");
        }
        self.generation = plan.next_generation;
    }

    fn validate_retirement_associations(
        &self,
        device_associations: &[(RuntimeComObjectId, RuntimeAudioDeviceId)],
        buffer_associations: &[(RuntimeComObjectId, RuntimeAudioBufferId)],
        retiring_com_objects: &BTreeSet<RuntimeComObjectId>,
    ) -> Result<(), RuntimeStateError> {
        let retiring_device_ids = device_associations
            .iter()
            .map(|(_, device_id)| *device_id)
            .collect::<BTreeSet<_>>();
        for (com_object_id, device_id) in device_associations {
            let device = self.devices.get(device_id).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-audio",
                    "audio-retirement-device-missing",
                    "audio COM retirement references no live logical device",
                )
            })?;
            if device.com_object_id != *com_object_id
                || self.device_com_object_index.get(com_object_id) != Some(device_id)
            {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-audio",
                    "audio-retirement-association-invalid",
                    "audio COM retirement association disagrees with authoritative indexes",
                ));
            }
            if device.buffers.iter().any(|buffer_id| {
                self.buffers
                    .get(buffer_id)
                    .is_none_or(|buffer| !retiring_com_objects.contains(&buffer.com_object_id))
            }) {
                return Err(RuntimeStateError::conflict(
                    "runtime-audio",
                    "audio-device-retirement-has-live-buffers",
                    "logical audio device cannot retire before all owned buffers",
                ));
            }
        }
        for (com_object_id, buffer_id) in buffer_associations {
            let buffer = self.buffers.get(buffer_id).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-audio",
                    "audio-retirement-buffer-missing",
                    "audio COM retirement references no live logical buffer",
                )
            })?;
            let device = self.devices.get(&buffer.device_id).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-audio",
                    "audio-retirement-buffer-device-missing",
                    "audio buffer retirement references no live logical device",
                )
            })?;
            if buffer.com_object_id != *com_object_id
                || self.buffer_com_object_index.get(com_object_id) != Some(buffer_id)
                || !device.buffers.contains(buffer_id)
            {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-audio",
                    "audio-buffer-retirement-association-invalid",
                    "audio buffer COM retirement association disagrees with authoritative indexes",
                ));
            }
            if !retiring_device_ids.contains(&device.id)
                && device.generation.checked_add(1).is_none()
            {
                return Err(RuntimeStateError::resource_limit(
                    "runtime-audio",
                    "audio-device-generation-exhausted",
                    "logical audio device generation is exhausted",
                ));
            }
        }
        Ok(())
    }
}

fn validate_device_create_request(
    request: RuntimeAudioDeviceCreateRequest,
) -> Result<(), RuntimeStateError> {
    if request.frontend != RuntimeAudioFrontend::DirectSound8
        || request.selector != RuntimeAudioDeviceSelector::DefaultPlayback
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-audio",
            "audio-device-create-request-invalid",
            "logical audio device request is outside the DirectSound 8 default-playback band",
        ));
    }
    Ok(())
}

fn validate_cooperative_target(target: RuntimeAudioWindowTarget) -> Result<(), RuntimeStateError> {
    if target.window_id.0 == 0 || target.window_handle == 0 || target.owner_thread_id.0 == 0 {
        return Err(RuntimeStateError::invalid_input(
            "runtime-audio",
            "audio-cooperative-window-target-invalid",
            "audio cooperative state requires nonzero logical window identities",
        ));
    }
    Ok(())
}

fn validate_cooperative_state(
    state: RuntimeAudioCooperativeState,
) -> Result<(), RuntimeStateError> {
    validate_cooperative_target(RuntimeAudioWindowTarget {
        window_id: state.window_id,
        window_handle: state.window_handle,
        owner_thread_id: state.owner_thread_id,
    })
}

fn validate_buffer_descriptor(
    descriptor: RuntimeAudioBufferDescriptor,
) -> Result<(), RuntimeStateError> {
    match descriptor.kind {
        RuntimeAudioBufferKind::Primary
            if descriptor.byte_length == 0 && descriptor.format.is_none() => {}
        RuntimeAudioBufferKind::Secondary
            if descriptor.byte_length != 0 && descriptor.format.is_some() =>
        {
            validate_pcm_format(descriptor.format.expect("checked secondary format"))?;
        }
        _ => {
            return Err(RuntimeStateError::invalid_input(
                "runtime-audio",
                "audio-buffer-descriptor-invalid",
                "logical primary and secondary audio buffer descriptor shapes differ",
            ));
        }
    }
    if descriptor.flags == 0 {
        return Err(RuntimeStateError::invalid_input(
            "runtime-audio",
            "audio-buffer-flags-invalid",
            "logical audio buffer flags must identify a bounded DirectSound variant",
        ));
    }
    Ok(())
}

fn buffer_ready_for_looping_play(buffer: &RuntimeAudioBuffer) -> bool {
    match buffer.descriptor.kind {
        RuntimeAudioBufferKind::Primary => buffer.active_format.is_some(),
        RuntimeAudioBufferKind::Secondary => {
            buffer.active_format.is_some()
                && buffer.backing.is_some()
                && buffer.active_lock.is_none()
                && buffer.content_generation != 0
        }
    }
}

fn validate_pcm_format(format: RuntimeAudioPcmFormat) -> Result<(), RuntimeStateError> {
    let bytes_per_sample = u32::from(format.bits_per_sample) / 8;
    let expected_block_align = u32::from(format.channels)
        .checked_mul(bytes_per_sample)
        .filter(|_| format.bits_per_sample.is_multiple_of(8));
    let expected_average = expected_block_align
        .and_then(|block_align| format.samples_per_second.checked_mul(block_align));
    if format.channels == 0
        || format.samples_per_second == 0
        || format.bits_per_sample == 0
        || expected_block_align != Some(u32::from(format.block_align))
        || expected_average != Some(format.average_bytes_per_second)
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-audio",
            "audio-pcm-format-invalid",
            "logical PCM format fields are incoherent or overflow their x86 representation",
        ));
    }
    Ok(())
}

fn require_priority_cooperative_state(
    device: &RuntimeAudioDevice,
) -> Result<(), RuntimeStateError> {
    if device
        .cooperative
        .is_none_or(|state| state.level != RuntimeAudioCooperativeLevel::Priority)
    {
        return Err(RuntimeStateError::conflict(
            "runtime-audio",
            "audio-priority-cooperative-level-required",
            "logical audio buffer initialization requires priority cooperative state",
        ));
    }
    Ok(())
}

fn checked_device_generation(generation: u64) -> Result<u64, RuntimeStateError> {
    generation.checked_add(1).ok_or_else(|| {
        RuntimeStateError::resource_limit(
            "runtime-audio",
            "audio-device-generation-exhausted",
            "logical audio device generation is exhausted",
        )
    })
}

fn checked_buffer_generation(generation: u64) -> Result<u64, RuntimeStateError> {
    generation.checked_add(1).ok_or_else(|| {
        RuntimeStateError::resource_limit(
            "runtime-audio",
            "audio-buffer-generation-exhausted",
            "logical audio buffer generation is exhausted",
        )
    })
}

fn checked_next_generation(generation: u64) -> Result<u64, RuntimeStateError> {
    generation.checked_add(1).ok_or_else(|| {
        RuntimeStateError::resource_limit(
            "runtime-audio",
            "audio-generation-exhausted",
            "logical audio subsystem generation is exhausted",
        )
    })
}

fn stale_plan(code: &'static str) -> RuntimeStateError {
    RuntimeStateError::conflict(
        "runtime-audio",
        code,
        "logical audio state changed after mutation planning",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> RuntimeAudioDeviceCreateRequest {
        RuntimeAudioDeviceCreateRequest {
            frontend: RuntimeAudioFrontend::DirectSound8,
            selector: RuntimeAudioDeviceSelector::DefaultPlayback,
        }
    }

    fn target() -> RuntimeAudioWindowTarget {
        RuntimeAudioWindowTarget {
            window_id: RuntimeWindowId(7),
            window_handle: 0x4801_0001,
            owner_thread_id: RuntimeThreadId(11),
        }
    }

    fn pcm_format() -> RuntimeAudioPcmFormat {
        RuntimeAudioPcmFormat {
            channels: 2,
            samples_per_second: 44_100,
            average_bytes_per_second: 176_400,
            block_align: 4,
            bits_per_sample: 16,
        }
    }

    fn primary_descriptor() -> RuntimeAudioBufferDescriptor {
        RuntimeAudioBufferDescriptor {
            kind: RuntimeAudioBufferKind::Primary,
            flags: 1,
            byte_length: 0,
            format: None,
        }
    }

    fn secondary_descriptor() -> RuntimeAudioBufferDescriptor {
        RuntimeAudioBufferDescriptor {
            kind: RuntimeAudioBufferKind::Secondary,
            flags: 0x80a8,
            byte_length: 88_200,
            format: Some(pcm_format()),
        }
    }

    fn priority_audio() -> RuntimeAudio {
        let mut audio = RuntimeAudio::default();
        let creation = audio.plan_device_creation(request()).unwrap();
        audio.commit_device_creation(creation, RuntimeComObjectId(13));
        let cooperative = audio
            .plan_cooperative_level(RuntimeAudioCooperativeLevelUpdateRequest {
                device_com_object_id: RuntimeComObjectId(13),
                target: target(),
                level: RuntimeAudioCooperativeLevel::Priority,
            })
            .unwrap();
        audio.commit_cooperative_level(cooperative);
        audio
    }

    #[test]
    fn device_cooperative_state_and_com_retirement_are_planned_and_owned() {
        let mut audio = RuntimeAudio::default();
        let creation = audio.plan_device_creation(request()).unwrap();
        let stale_creation = creation.clone();
        let device = audio.commit_device_creation(creation, RuntimeComObjectId(13));
        assert_eq!(device.id, RuntimeAudioDeviceId(1));
        assert_eq!(device.generation, 1);
        assert_eq!(device.cooperative, None);
        assert_eq!(
            audio
                .preflight_device_creation(&stale_creation, RuntimeComObjectId(14))
                .unwrap_err()
                .code,
            "audio-device-creation-plan-stale"
        );

        let update = audio
            .plan_cooperative_level(RuntimeAudioCooperativeLevelUpdateRequest {
                device_com_object_id: RuntimeComObjectId(13),
                target: target(),
                level: RuntimeAudioCooperativeLevel::Priority,
            })
            .unwrap();
        let stale_update = update.clone();
        let result = audio.commit_cooperative_level(update);
        assert!(result.changed);
        assert_eq!(result.current.window_id, RuntimeWindowId(7));
        assert_eq!(result.current.level, RuntimeAudioCooperativeLevel::Priority);
        assert_eq!(result.device_generation, 2);
        assert_eq!(
            audio
                .preflight_cooperative_level(&stale_update)
                .unwrap_err()
                .code,
            "audio-cooperative-level-plan-stale"
        );

        let unchanged = audio
            .plan_cooperative_level(RuntimeAudioCooperativeLevelUpdateRequest {
                device_com_object_id: RuntimeComObjectId(13),
                target: target(),
                level: RuntimeAudioCooperativeLevel::Priority,
            })
            .unwrap();
        let result = audio.commit_cooperative_level(unchanged);
        assert!(!result.changed);
        assert_eq!(result.device_generation, 2);

        let retirements = [RuntimeComObjectId(13)].into_iter().collect();
        let retirement = audio
            .plan_com_object_retirements(&retirements)
            .unwrap()
            .unwrap();
        audio.preflight_com_object_retirement(&retirement).unwrap();
        audio.commit_com_object_retirement(retirement);
        assert!(audio.is_empty());
        assert!(audio.device_com_object_index.is_empty());
    }

    #[test]
    fn invalid_targets_and_capacity_fail_without_mutation() {
        let mut audio = RuntimeAudio::default();
        let before = audio.clone();
        assert_eq!(
            audio
                .plan_cooperative_level(RuntimeAudioCooperativeLevelUpdateRequest {
                    device_com_object_id: RuntimeComObjectId(1),
                    target: RuntimeAudioWindowTarget {
                        window_id: RuntimeWindowId(0),
                        ..target()
                    },
                    level: RuntimeAudioCooperativeLevel::Priority,
                })
                .unwrap_err()
                .code,
            "audio-cooperative-window-target-invalid"
        );
        assert_eq!(audio, before);

        for index in 0..RUNTIME_AUDIO_DEVICE_LIMIT {
            let plan = audio.plan_device_creation(request()).unwrap();
            audio.commit_device_creation(plan, RuntimeComObjectId(index as u64 + 1));
        }
        let before = audio.clone();
        assert_eq!(
            audio.plan_device_creation(request()).unwrap_err().code,
            "audio-device-capacity-reached"
        );
        assert_eq!(audio, before);
    }

    #[test]
    fn buffers_own_format_playback_and_parent_retirement_relations() {
        let mut audio = priority_audio();
        let primary_plan = audio
            .plan_buffer_creation(RuntimeAudioBufferCreateRequest {
                device_com_object_id: RuntimeComObjectId(13),
                descriptor: primary_descriptor(),
            })
            .unwrap();
        audio.preflight_buffer_creation(&primary_plan).unwrap();
        let primary = audio.commit_buffer_creation(primary_plan, RuntimeComObjectId(21));
        assert_eq!(primary.id, RuntimeAudioBufferId(1));
        assert_eq!(primary.device_id, RuntimeAudioDeviceId(1));
        assert_eq!(primary.active_format, None);
        assert_eq!(primary.playback, RuntimeAudioPlaybackState::Stopped);

        let format_plan = audio
            .plan_buffer_format(RuntimeAudioBufferFormatRequest {
                buffer_com_object_id: RuntimeComObjectId(21),
                format: pcm_format(),
            })
            .unwrap();
        let format_result = audio.commit_buffer_format(format_plan);
        assert!(format_result.changed);
        assert_eq!(format_result.current, pcm_format());

        let play_plan = audio
            .plan_buffer_play(RuntimeAudioBufferPlayRequest {
                buffer_com_object_id: RuntimeComObjectId(21),
                playback: RuntimeAudioPlaybackState::PlayingLooping,
            })
            .unwrap();
        let play_result = audio.commit_buffer_play(play_plan);
        assert!(play_result.changed);
        assert_eq!(
            play_result.current,
            RuntimeAudioPlaybackState::PlayingLooping
        );

        let secondary_plan = audio
            .plan_buffer_creation(RuntimeAudioBufferCreateRequest {
                device_com_object_id: RuntimeComObjectId(13),
                descriptor: secondary_descriptor(),
            })
            .unwrap();
        let stale_secondary_plan = secondary_plan.clone();
        let secondary = audio.commit_buffer_creation(secondary_plan, RuntimeComObjectId(22));
        assert_eq!(secondary.id, RuntimeAudioBufferId(2));
        assert_eq!(secondary.active_format, Some(pcm_format()));
        assert_eq!(
            audio.device(RuntimeAudioDeviceId(1)).unwrap().buffers.len(),
            2
        );
        assert_eq!(
            audio
                .preflight_buffer_creation(&stale_secondary_plan)
                .unwrap_err()
                .code,
            "audio-buffer-creation-plan-stale"
        );

        let device_only = [RuntimeComObjectId(13)].into_iter().collect();
        assert_eq!(
            audio
                .plan_com_object_retirements(&device_only)
                .unwrap_err()
                .code,
            "audio-device-retirement-has-live-buffers"
        );
        let primary_only = [RuntimeComObjectId(21)].into_iter().collect();
        let retirement = audio
            .plan_com_object_retirements(&primary_only)
            .unwrap()
            .unwrap();
        audio.commit_com_object_retirement(retirement);
        assert!(audio.buffer(RuntimeAudioBufferId(1)).is_err());
        assert_eq!(
            audio.device(RuntimeAudioDeviceId(1)).unwrap().buffers.len(),
            1
        );

        let remaining = [RuntimeComObjectId(13), RuntimeComObjectId(22)]
            .into_iter()
            .collect();
        let retirement = audio
            .plan_com_object_retirements(&remaining)
            .unwrap()
            .unwrap();
        audio.commit_com_object_retirement(retirement);
        assert!(audio.is_empty());
        assert!(audio.device_com_object_index.is_empty());
        assert!(audio.buffer_com_object_index.is_empty());
    }

    #[test]
    fn secondary_lock_unlock_and_play_preserve_backing_and_generation_invariants() {
        let mut audio = priority_audio();
        let secondary_plan = audio
            .plan_buffer_creation(RuntimeAudioBufferCreateRequest {
                device_com_object_id: RuntimeComObjectId(13),
                descriptor: secondary_descriptor(),
            })
            .unwrap();
        audio.commit_buffer_creation(secondary_plan, RuntimeComObjectId(22));
        let before_uninitialized_play = audio.clone();
        assert_eq!(
            audio
                .plan_buffer_play(RuntimeAudioBufferPlayRequest {
                    buffer_com_object_id: RuntimeComObjectId(22),
                    playback: RuntimeAudioPlaybackState::PlayingLooping,
                })
                .unwrap_err()
                .code,
            "audio-buffer-play-state-invalid"
        );
        assert_eq!(audio, before_uninitialized_play);

        let backing_plan = RuntimeAudioBufferBackingPlan {
            address_allocation_id: ProcessAddressAllocationId(99),
            range: GuestRange::new(GuestAddress(0x4500_0000), 0x1_6000).unwrap(),
        };
        let lock_plan = audio
            .plan_buffer_lock(
                RuntimeAudioBufferLockRequest {
                    buffer_com_object_id: RuntimeComObjectId(22),
                    offset: 0,
                    byte_length: 88_200,
                    flags: 0,
                },
                Some(backing_plan),
            )
            .unwrap();
        let stale_lock = lock_plan.clone();
        audio
            .preflight_buffer_lock(&lock_plan, Some(MemoryOwnerId(77)))
            .unwrap();
        let lock = audio.commit_buffer_lock(lock_plan, Some(MemoryOwnerId(77)));
        assert_eq!(lock.first_segment.guest_address, GuestAddress(0x4500_0000));
        assert_eq!(lock.first_segment.byte_length, 88_200);
        assert_eq!(lock.second_segment, None);
        let secondary = audio.buffer(RuntimeAudioBufferId(1)).unwrap();
        assert_eq!(secondary.active_lock, Some(lock));
        let installed_backing = secondary.backing.expect("installed audio sample backing");
        assert_eq!(
            installed_backing,
            RuntimeAudioBufferBacking {
                address_allocation_id: ProcessAddressAllocationId(99),
                memory_owner: MemoryOwnerId(77),
                range: backing_plan.range,
            }
        );
        assert_eq!(secondary.content_generation, 0);
        assert_eq!(
            audio
                .preflight_buffer_lock(&stale_lock, Some(MemoryOwnerId(77)))
                .unwrap_err()
                .code,
            "audio-buffer-lock-plan-stale"
        );

        let before_double_lock = audio.clone();
        assert_eq!(
            audio
                .plan_buffer_lock(
                    RuntimeAudioBufferLockRequest {
                        buffer_com_object_id: RuntimeComObjectId(22),
                        offset: 0,
                        byte_length: 88_200,
                        flags: 0,
                    },
                    None,
                )
                .unwrap_err()
                .code,
            "audio-buffer-already-locked"
        );
        assert_eq!(audio, before_double_lock);
        assert_eq!(
            audio
                .plan_buffer_unlock(RuntimeAudioBufferUnlockRequest {
                    buffer_com_object_id: RuntimeComObjectId(22),
                    first_segment: RuntimeAudioBufferSegment {
                        guest_address: GuestAddress(0x4500_0004),
                        byte_length: 88_200,
                    },
                    second_segment: None,
                })
                .unwrap_err()
                .code,
            "audio-buffer-unlock-segments-mismatch"
        );
        assert_eq!(audio, before_double_lock);

        let unlock_plan = audio
            .plan_buffer_unlock(RuntimeAudioBufferUnlockRequest {
                buffer_com_object_id: RuntimeComObjectId(22),
                first_segment: lock.first_segment,
                second_segment: lock.second_segment,
            })
            .unwrap();
        let stale_unlock = unlock_plan.clone();
        let unlocked = audio.commit_buffer_unlock(unlock_plan);
        assert_eq!(unlocked.content_generation, 1);
        assert_eq!(
            audio
                .preflight_buffer_unlock(&stale_unlock)
                .unwrap_err()
                .code,
            "audio-buffer-unlock-plan-stale"
        );
        let play_plan = audio
            .plan_buffer_play(RuntimeAudioBufferPlayRequest {
                buffer_com_object_id: RuntimeComObjectId(22),
                playback: RuntimeAudioPlaybackState::PlayingLooping,
            })
            .unwrap();
        let played = audio.commit_buffer_play(play_plan);
        assert!(played.changed);
        assert_eq!(played.current, RuntimeAudioPlaybackState::PlayingLooping);

        let retirements = [RuntimeComObjectId(22)].into_iter().collect();
        let retirement = audio
            .plan_com_object_retirements(&retirements)
            .unwrap()
            .unwrap();
        assert_eq!(
            audio.buffer_backings_for_retirement(&retirement).unwrap(),
            vec![(installed_backing, 88_200)]
        );
    }

    #[test]
    fn invalid_and_duplicate_buffers_fail_without_mutation() {
        let mut no_priority = RuntimeAudio::default();
        let creation = no_priority.plan_device_creation(request()).unwrap();
        no_priority.commit_device_creation(creation, RuntimeComObjectId(13));
        let before = no_priority.clone();
        assert_eq!(
            no_priority
                .plan_buffer_creation(RuntimeAudioBufferCreateRequest {
                    device_com_object_id: RuntimeComObjectId(13),
                    descriptor: primary_descriptor(),
                })
                .unwrap_err()
                .code,
            "audio-priority-cooperative-level-required"
        );
        assert_eq!(no_priority, before);

        let mut audio = priority_audio();
        let before = audio.clone();
        assert_eq!(
            audio
                .plan_buffer_creation(RuntimeAudioBufferCreateRequest {
                    device_com_object_id: RuntimeComObjectId(13),
                    descriptor: RuntimeAudioBufferDescriptor {
                        flags: 0,
                        ..primary_descriptor()
                    },
                })
                .unwrap_err()
                .code,
            "audio-buffer-flags-invalid"
        );
        assert_eq!(audio, before);

        let primary = audio
            .plan_buffer_creation(RuntimeAudioBufferCreateRequest {
                device_com_object_id: RuntimeComObjectId(13),
                descriptor: primary_descriptor(),
            })
            .unwrap();
        audio.commit_buffer_creation(primary, RuntimeComObjectId(21));
        let before = audio.clone();
        assert_eq!(
            audio
                .plan_buffer_creation(RuntimeAudioBufferCreateRequest {
                    device_com_object_id: RuntimeComObjectId(13),
                    descriptor: primary_descriptor(),
                })
                .unwrap_err()
                .code,
            "audio-primary-buffer-already-live"
        );
        assert_eq!(audio, before);
    }
}

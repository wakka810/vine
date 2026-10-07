use std::collections::{BTreeMap, BTreeSet};

use super::{
    ProcessAddressAllocationId, RuntimeComInterfaceId, RuntimeComObjectId, RuntimeHandle,
    RuntimeStateError, RuntimeThreadId, RuntimeWindowId,
};
use crate::memory::{GuestAddress, GuestRange, MemoryOwnerId};

pub const RUNTIME_AUDIO_NOTIFICATION_PACING_SERVICE: &str = "browser.audio-delay";
pub(crate) const RUNTIME_AUDIO_RESOURCE_ARENA_BASE: GuestAddress = GuestAddress(0x4500_0000);
pub(crate) const RUNTIME_AUDIO_RESOURCE_ARENA_SIZE: u64 = 0x0100_0000;
pub(crate) const RUNTIME_AUDIO_PCM_MIN_SAMPLES_PER_SECOND: u32 = 8_000;
pub(crate) const RUNTIME_AUDIO_PCM_MAX_SAMPLES_PER_SECOND: u32 = 192_000;
pub(crate) const RUNTIME_AUDIO_PCM_MAX_CHANNELS: u16 = 2;
pub(crate) const RUNTIME_AUDIO_VOLUME_MIN_HUNDREDTHS_DB: i32 = -10_000;
pub(crate) const RUNTIME_AUDIO_VOLUME_MAX_HUNDREDTHS_DB: i32 = 0;
pub(crate) const RUNTIME_AUDIO_PAN_MIN_HUNDREDTHS_DB: i32 = -10_000;
pub(crate) const RUNTIME_AUDIO_PAN_MAX_HUNDREDTHS_DB: i32 = 10_000;
pub(crate) const RUNTIME_AUDIO_PLAYBACK_FREQUENCY_MIN_HZ: u32 = 100;
pub(crate) const RUNTIME_AUDIO_PLAYBACK_FREQUENCY_MAX_HZ: u32 = 200_000;
pub(crate) const RUNTIME_AUDIO_DEVICE_LIMIT: usize = 4_096;
pub(crate) const RUNTIME_AUDIO_BUFFER_LIMIT: usize = 16_384;
pub(crate) const RUNTIME_AUDIO_NOTIFICATION_LIMIT: usize = 32;
const RUNTIME_AUDIO_ACTIVE_LOCK_LIMIT: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RuntimeAudioDeviceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RuntimeAudioBufferId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeAudioCooperativeState {
    pub window_id: RuntimeWindowId,
    pub window_handle: u32,
    pub owner_thread_id: RuntimeThreadId,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub(crate) struct RuntimeAudioDevice {
    pub id: RuntimeAudioDeviceId,

    pub com_object_id: RuntimeComObjectId,
    pub cooperative: Option<RuntimeAudioCooperativeState>,
    pub buffers: BTreeSet<RuntimeAudioBufferId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeAudioBufferKind {
    Primary,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeAudioPcmFormat {
    pub channels: u16,
    pub samples_per_second: u32,
    pub average_bytes_per_second: u32,
    pub block_align: u16,
    pub bits_per_sample: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeAudioBufferDescriptor {
    pub kind: RuntimeAudioBufferKind,
    pub flags: u32,
    pub byte_length: u32,
    pub format: Option<RuntimeAudioPcmFormat>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeAudioBufferBacking {
    pub address_allocation_id: ProcessAddressAllocationId,
    pub memory_owner: MemoryOwnerId,
    pub range: GuestRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeAudioBufferSegment {
    pub guest_address: GuestAddress,
    pub byte_length: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeAudioBufferLock {
    pub first_segment: RuntimeAudioBufferSegment,
    pub second_segment: Option<RuntimeAudioBufferSegment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeAudioPlaybackState {
    Stopped,
    PlayingOnce,
    PlayingLooping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeAudioBufferControls {
    pub volume_hundredths_db: i32,
    pub pan_hundredths_db: i32,
    pub frequency_hz: Option<u32>,
    pub current_position_bytes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeAudioNotification {
    pub offset_bytes: u32,
    pub event_handle: RuntimeHandle,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub(crate) struct RuntimeAudioBuffer {
    pub id: RuntimeAudioBufferId,
    pub device_id: RuntimeAudioDeviceId,
    pub com_object_id: RuntimeComObjectId,
    pub descriptor: RuntimeAudioBufferDescriptor,
    pub backing: Option<RuntimeAudioBufferBacking>,
    pub active_format: Option<RuntimeAudioPcmFormat>,
    pub active_locks: Vec<RuntimeAudioBufferLock>,
    pub initialized: bool,
    pub playback: RuntimeAudioPlaybackState,
    pub browser_resource_id: Option<u64>,
    pub controls: RuntimeAudioBufferControls,
    pub notifications: Vec<RuntimeAudioNotification>,
    pub notification_cursor: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeAudioBufferControlUpdate {
    Volume(i32),
    Pan(i32),
    Frequency(u32),
    CurrentPosition(u32),
}

#[derive(Debug)]
pub(crate) enum BrowserAudioAction {
    Playback {
        object_id: RuntimeComObjectId,
        playback: RuntimeAudioPlaybackState,
    },
    Unlock {
        object_id: RuntimeComObjectId,
        first: RuntimeAudioBufferSegment,
        second: Option<RuntimeAudioBufferSegment>,
    },
    Control {
        object_id: RuntimeComObjectId,
        update: RuntimeAudioBufferControlUpdate,
    },
    Release {
        interface_id: RuntimeComInterfaceId,
    },
}

#[derive(Debug)]
pub(crate) struct BrowserAudioBufferState {
    pub object_id: RuntimeComObjectId,
    id: RuntimeAudioBufferId,
    backing: Option<RuntimeAudioBufferBacking>,
    format: Option<RuntimeAudioPcmFormat>,
    playback: RuntimeAudioPlaybackState,
    controls: RuntimeAudioBufferControls,
    resource_id: Option<u64>,
    lock_count: usize,
    initialized: bool,
}

impl BrowserAudioBufferState {
    pub(super) fn capture(buffer: &RuntimeAudioBuffer) -> Self {
        Self {
            object_id: buffer.com_object_id,
            id: buffer.id,
            backing: buffer.backing,
            format: buffer.active_format,
            playback: buffer.playback,
            controls: buffer.controls,
            resource_id: buffer.browser_resource_id,
            lock_count: buffer.active_locks.len(),
            initialized: buffer.initialized,
        }
    }

    pub(super) fn matches(&self, buffer: &RuntimeAudioBuffer) -> bool {
        self.object_id == buffer.com_object_id
            && self.id == buffer.id
            && self.backing == buffer.backing
            && self.format == buffer.active_format
            && self.playback == buffer.playback
            && self.controls == buffer.controls
            && self.resource_id == buffer.browser_resource_id
            && self.lock_count == buffer.active_locks.len()
            && self.initialized == buffer.initialized
    }
}

#[derive(Debug)]
pub(crate) struct PreparedBrowserAudio {
    pub request_id: u64,
    pub thread_id: RuntimeThreadId,
    pub command: Vec<u8>,
    pub action: BrowserAudioAction,
    pub buffers: Vec<BrowserAudioBufferState>,
    pub resource_id: Option<u64>,
}

#[derive(Debug)]
pub(crate) enum BrowserAudioPreparation {
    Complete(bool),
    Pending(PreparedBrowserAudio),
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub(crate) struct RuntimeAudioNotificationWaitPlan {
    buffer_id: RuntimeAudioBufferId,
    next_position_bytes: u32,
    next_cursor: usize,
    delay_milliseconds: u32,
    result_index: Option<u32>,
    timeout_milliseconds: u32,
}

impl RuntimeAudioNotificationWaitPlan {
    pub(crate) const fn result_index(&self) -> Option<u32> {
        self.result_index
    }

    pub(crate) fn encode_host_payload(&self, handles: &[RuntimeHandle]) -> Vec<u8> {
        let words = [
            1,
            self.delay_milliseconds,
            self.buffer_id.0 as u32,
            (self.buffer_id.0 >> 32) as u32,
            self.timeout_milliseconds,
            handles.len() as u32,
        ];
        let mut payload = Vec::with_capacity((words.len() + handles.len()) * 4);
        for word in words
            .into_iter()
            .chain(handles.iter().map(|handle| handle.0))
        {
            payload.extend_from_slice(&word.to_le_bytes());
        }
        payload
    }

    pub(crate) fn matches_host_payload(&self, handles: &[RuntimeHandle], payload: &[u8]) -> bool {
        // Other waiters may advance position and cursor; completion uses live state.
        // The captured mode, delay, buffer identity, timeout, and handles stay fixed.
        if payload.len() != (6 + handles.len()) * 4 {
            return false;
        }
        let word = |index: usize| {
            u32::from_le_bytes(payload[index * 4..index * 4 + 4].try_into().unwrap())
        };
        word(0) == 1
            && word(1) == self.delay_milliseconds
            && (u64::from(word(2)) | (u64::from(word(3)) << 32)) == self.buffer_id.0
            && word(4) == self.timeout_milliseconds
            && word(5) == handles.len() as u32
            && handles
                .iter()
                .enumerate()
                .all(|(index, handle)| word(6 + index) == handle.0)
    }
}

// Validated cleanup targets consumed synchronously by final COM release.
#[derive(Debug)]
pub(crate) struct RuntimeAudioComRetirement {
    device_associations: Vec<(RuntimeComObjectId, RuntimeAudioDeviceId)>,
    buffer_associations: Vec<(RuntimeComObjectId, RuntimeAudioBufferId)>,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub(super) struct RuntimeAudio {
    devices: BTreeMap<RuntimeAudioDeviceId, RuntimeAudioDevice>,
    device_com_object_index: BTreeMap<RuntimeComObjectId, RuntimeAudioDeviceId>,
    buffers: BTreeMap<RuntimeAudioBufferId, RuntimeAudioBuffer>,
    buffer_com_object_index: BTreeMap<RuntimeComObjectId, RuntimeAudioBufferId>,
    next_device_id: u64,
    next_buffer_id: u64,
    next_browser_operation_id: u64,
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
            next_browser_operation_id: 1,
        }
    }
}

impl RuntimeAudio {
    pub(super) fn has_browser_end_pending(&self) -> bool {
        self.buffers.values().any(|buffer| {
            buffer.browser_resource_id.is_some()
                && buffer.playback == RuntimeAudioPlaybackState::PlayingOnce
        })
    }

    pub(super) fn next_browser_operation_id(&mut self) -> Result<u64, RuntimeStateError> {
        let id = self.next_browser_operation_id;
        self.next_browser_operation_id = id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::new("browser audio operation identity is exhausted")
        })?;
        Ok(id)
    }

    pub(super) fn set_browser_resource(
        &mut self,
        object_id: RuntimeComObjectId,
        resource_id: Option<u64>,
    ) {
        let id = self.buffer_com_object_index[&object_id];
        self.buffers
            .get_mut(&id)
            .expect("validated audio buffer")
            .browser_resource_id = resource_id;
    }

    pub(super) fn buffer_for_browser_resource(
        &self,
        resource_id: u64,
    ) -> Option<&RuntimeAudioBuffer> {
        self.buffers
            .values()
            .find(|buffer| buffer.browser_resource_id == Some(resource_id))
    }

    pub(super) fn browser_buffers_for_objects(
        &self,
        objects: &BTreeSet<RuntimeComObjectId>,
    ) -> Vec<&RuntimeAudioBuffer> {
        self.buffers
            .values()
            .filter(|buffer| {
                objects.contains(&buffer.com_object_id) && buffer.browser_resource_id.is_some()
            })
            .collect()
    }
    #[cfg(test)]
    pub(super) fn devices(&self) -> &BTreeMap<RuntimeAudioDeviceId, RuntimeAudioDevice> {
        &self.devices
    }

    #[cfg(test)]
    pub(super) fn buffers(&self) -> &BTreeMap<RuntimeAudioBufferId, RuntimeAudioBuffer> {
        &self.buffers
    }

    pub(super) fn device(
        &self,
        id: RuntimeAudioDeviceId,
    ) -> Result<&RuntimeAudioDevice, RuntimeStateError> {
        self.devices
            .get(&id)
            .ok_or_else(|| RuntimeStateError::new("logical audio device identity is not live"))
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
                RuntimeStateError::new("logical audio device has no live COM object association")
            })?;
        let device = self.device(id)?;
        if device.id != id || device.com_object_id != com_object_id {
            return Err(RuntimeStateError::new(
                "logical audio device COM index disagrees with its authoritative record",
            ));
        }
        Ok(device)
    }

    pub(super) fn buffer(
        &self,
        id: RuntimeAudioBufferId,
    ) -> Result<&RuntimeAudioBuffer, RuntimeStateError> {
        self.buffers
            .get(&id)
            .ok_or_else(|| RuntimeStateError::new("logical audio buffer identity is not live"))
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
                RuntimeStateError::new("logical audio buffer has no live COM object association")
            })?;
        let buffer = self.buffer(id)?;
        if buffer.id != id || buffer.com_object_id != com_object_id {
            return Err(RuntimeStateError::new(
                "logical audio buffer COM index disagrees with its authoritative record",
            ));
        }
        Ok(buffer)
    }

    pub(super) fn create_device(
        &mut self,
        com_object_id: RuntimeComObjectId,
    ) -> Result<RuntimeAudioDeviceId, RuntimeStateError> {
        if self.devices.len() >= RUNTIME_AUDIO_DEVICE_LIMIT {
            return Err(RuntimeStateError::new(
                "logical audio device capacity is exhausted",
            ));
        }
        let id = RuntimeAudioDeviceId(self.next_device_id);
        if id.0 == 0 || self.devices.contains_key(&id) {
            return Err(RuntimeStateError::new(
                "logical audio device allocator selected a live identity",
            ));
        }
        let next_device_id = id.0.checked_add(1).ok_or_else(|| {
            RuntimeStateError::new("logical audio device identity space is exhausted")
        })?;

        if com_object_id.0 == 0
            || self.device_com_object_index.contains_key(&com_object_id)
            || self.buffer_com_object_index.contains_key(&com_object_id)
        {
            return Err(RuntimeStateError::new(
                "audio device requires a fresh nonzero COM identity",
            ));
        }
        let device = RuntimeAudioDevice {
            id,

            com_object_id,
            cooperative: None,
            buffers: BTreeSet::new(),
        };
        self.device_com_object_index
            .insert(com_object_id, device.id);
        self.devices.insert(id, device);
        self.next_device_id = next_device_id;

        Ok(id)
    }

    pub(super) fn validate_buffer_creation(
        &self,
        device_com_object_id: RuntimeComObjectId,
        descriptor: RuntimeAudioBufferDescriptor,
    ) -> Result<
        (
            RuntimeAudioBufferId,
            RuntimeAudioDeviceId,
            RuntimeAudioBufferDescriptor,
        ),
        RuntimeStateError,
    > {
        validate_buffer_descriptor(descriptor)?;
        if self.buffers.len() >= RUNTIME_AUDIO_BUFFER_LIMIT {
            return Err(RuntimeStateError::new(
                "logical audio buffer capacity is exhausted",
            ));
        }
        let device = self.device_by_com_object(device_com_object_id)?;
        require_priority_cooperative_state(device)?;
        for id in &device.buffers {
            let buffer = self.buffers.get(id).ok_or_else(|| {
                RuntimeStateError::new(
                    "creating audio device has a dangling buffer ownership index",
                )
            })?;
            if buffer.id != *id
                || buffer.device_id != device.id
                || self.buffer_com_object_index.get(&buffer.com_object_id) != Some(id)
            {
                return Err(RuntimeStateError::new(
                    "creating audio device buffer index disagrees with live ownership",
                ));
            }
        }
        if self
            .buffers
            .values()
            .any(|buffer| buffer.device_id == device.id && !device.buffers.contains(&buffer.id))
        {
            return Err(RuntimeStateError::new(
                "live audio buffer is absent from its creating device's ownership index",
            ));
        }
        if descriptor.kind == RuntimeAudioBufferKind::Primary
            && device.buffers.iter().any(|id| {
                self.buffers
                    .get(id)
                    .is_some_and(|buffer| buffer.descriptor.kind == RuntimeAudioBufferKind::Primary)
            })
        {
            return Err(RuntimeStateError::new(
                "logical audio device already owns a live primary buffer",
            ));
        }
        let id = RuntimeAudioBufferId(self.next_buffer_id);
        if id.0 == 0 || self.buffers.contains_key(&id) {
            return Err(RuntimeStateError::new(
                "logical audio buffer allocator selected a live identity",
            ));
        }
        id.0.checked_add(1).ok_or_else(|| {
            RuntimeStateError::new("logical audio buffer identity space is exhausted")
        })?;

        Ok((id, device.id, descriptor))
    }

    pub(super) fn insert_buffer(
        &mut self,
        creation: (
            RuntimeAudioBufferId,
            RuntimeAudioDeviceId,
            RuntimeAudioBufferDescriptor,
        ),
        com_object_id: RuntimeComObjectId,
    ) -> RuntimeAudioBufferId {
        let (id, device_id, descriptor) = creation;
        let next_buffer_id =
            id.0.checked_add(1)
                .expect("validated audio buffer identity");
        debug_assert_ne!(com_object_id.0, 0);
        debug_assert!(!self.device_com_object_index.contains_key(&com_object_id));
        debug_assert!(!self.buffer_com_object_index.contains_key(&com_object_id));
        let buffer = RuntimeAudioBuffer {
            id,
            device_id,
            com_object_id,
            descriptor,
            backing: None,
            active_format: descriptor.format,
            active_locks: Vec::new(),
            initialized: false,
            playback: RuntimeAudioPlaybackState::Stopped,
            browser_resource_id: None,
            controls: RuntimeAudioBufferControls {
                volume_hundredths_db: 0,
                pan_hundredths_db: 0,
                frequency_hz: descriptor.format.map(|format| format.samples_per_second),
                current_position_bytes: 0,
            },
            notifications: Vec::new(),
            notification_cursor: 0,
        };
        let device = self
            .devices
            .get_mut(&device_id)
            .expect("validated audio device disappeared");
        device.buffers.insert(buffer.id);
        self.buffer_com_object_index
            .insert(com_object_id, buffer.id);
        self.buffers.insert(id, buffer);
        self.next_buffer_id = next_buffer_id;

        id
    }

    pub(super) fn set_buffer_format(
        &mut self,
        buffer_com_object_id: RuntimeComObjectId,
        format: RuntimeAudioPcmFormat,
    ) -> Result<(), RuntimeStateError> {
        validate_pcm_format(format)?;
        let buffer = self.buffer_by_com_object(buffer_com_object_id)?;
        if buffer.descriptor.kind != RuntimeAudioBufferKind::Primary {
            return Err(RuntimeStateError::new(
                "secondary logical audio buffers do not expose primary format control",
            ));
        }
        require_priority_cooperative_state(self.device(buffer.device_id)?)?;
        let id = buffer.id;
        if buffer.active_format == Some(format) {
            return Ok(());
        }

        let buffer = self.buffers.get_mut(&id).expect("checked audio buffer");
        buffer.active_format = Some(format);

        Ok(())
    }

    pub(super) fn preview_buffer_lock(
        &self,
        buffer_com_object_id: RuntimeComObjectId,
        offset: u32,
        byte_length: u32,
        flags: u32,
        backing_range: Option<GuestRange>,
    ) -> Result<Option<RuntimeAudioBufferLock>, RuntimeStateError> {
        let buffer = self.buffer_by_com_object(buffer_com_object_id)?;
        if buffer.descriptor.kind != RuntimeAudioBufferKind::Secondary || flags != 0 {
            return Err(RuntimeStateError::new(
                "bounded audio locking supports aligned secondary-buffer regions with no Lock flags",
            ));
        }
        if buffer.active_locks.len() >= RUNTIME_AUDIO_ACTIVE_LOCK_LIMIT {
            return Err(RuntimeStateError::new(
                "logical audio buffer active Lock capacity is exhausted",
            ));
        }
        let backing_start = match (buffer.backing, backing_range) {
            (Some(backing), None)
                if backing.address_allocation_id.0 != 0
                    && backing.memory_owner.0 != 0
                    && backing.range.byte_length() >= u64::from(buffer.descriptor.byte_length) =>
            {
                backing.range.start()
            }
            (None, Some(range))
                if range.start().0 != 0
                    && range.byte_length() >= u64::from(buffer.descriptor.byte_length) =>
            {
                range.start()
            }
            _ => {
                return Err(RuntimeStateError::new(
                    "logical audio Lock requires exactly one existing or planned sample backing",
                ));
            }
        };
        let lock = derive_buffer_lock(buffer, offset, byte_length, backing_start)?;
        if buffer
            .active_locks
            .iter()
            .any(|active| audio_buffer_locks_overlap(*active, lock))
        {
            return Ok(None);
        }

        Ok(Some(lock))
    }

    #[cfg(test)]
    fn lock_buffer(
        &mut self,
        buffer_com_object_id: RuntimeComObjectId,
        offset: u32,
        byte_length: u32,
        flags: u32,
        backing_installation: Option<RuntimeAudioBufferBacking>,
    ) -> Result<Option<RuntimeAudioBufferLock>, RuntimeStateError> {
        if backing_installation.is_some_and(|backing| {
            backing.address_allocation_id.0 == 0 || backing.memory_owner.0 == 0
        }) {
            return Err(RuntimeStateError::new(
                "initial audio Lock requires a live backing allocation and nonzero owner",
            ));
        }
        let Some(lock) = self.preview_buffer_lock(
            buffer_com_object_id,
            offset,
            byte_length,
            flags,
            backing_installation.map(|backing| backing.range),
        )?
        else {
            return Ok(None);
        };
        let id = self.buffer_by_com_object(buffer_com_object_id)?.id;
        self.publish_buffer_lock(id, lock, backing_installation);
        Ok(Some(lock))
    }

    pub(super) fn publish_buffer_lock(
        &mut self,
        buffer_id: RuntimeAudioBufferId,
        lock: RuntimeAudioBufferLock,
        backing_installation: Option<RuntimeAudioBufferBacking>,
    ) {
        // Only memory operations occur between validation and this publication.
        let buffer = self
            .buffers
            .get_mut(&buffer_id)
            .expect("validated audio buffer");
        if let Some(backing) = backing_installation {
            buffer.backing = Some(backing);
        }
        buffer.active_locks.push(lock);
    }

    pub(super) fn select_buffer_unlock(
        &self,
        buffer_com_object_id: RuntimeComObjectId,
        first_segment: RuntimeAudioBufferSegment,
        second_segment: Option<RuntimeAudioBufferSegment>,
    ) -> Result<Option<(RuntimeAudioBufferId, usize, bool)>, RuntimeStateError> {
        let buffer = self.buffer_by_com_object(buffer_com_object_id)?;
        if buffer.active_locks.is_empty() {
            return Ok(None);
        }
        let Some(lock_index) = buffer.active_locks.iter().position(|lock| {
            audio_buffer_segment_contains(lock.first_segment, first_segment)
                && match (second_segment, lock.second_segment) {
                    (None, None) => true,
                    (Some(request), Some(lock)) => audio_buffer_segment_contains(lock, request),
                    _ => false,
                }
        }) else {
            return Ok(None);
        };
        if buffer.backing.is_none() {
            return Err(RuntimeStateError::new(
                "locked audio buffer has no sample backing",
            ));
        }

        Ok(Some((
            buffer.id,
            lock_index,
            buffer.active_locks.len() == 1,
        )))
    }

    pub(super) fn finish_buffer_unlock(
        &mut self,
        buffer_id: RuntimeAudioBufferId,
        lock_index: usize,
    ) {
        // Selection, guest-memory protection, and this update are one synchronous call:
        // protecting the backing cannot change Rust-owned audio identity or locks.
        let buffer = self
            .buffers
            .get_mut(&buffer_id)
            .expect("selected audio buffer disappeared");
        buffer.active_locks.remove(lock_index);
        buffer.initialized = true;
    }

    pub(super) fn set_buffer_notifications(
        &mut self,
        buffer_com_object_id: RuntimeComObjectId,
        notifications: Vec<RuntimeAudioNotification>,
    ) -> Result<bool, RuntimeStateError> {
        let buffer = self.buffer_by_com_object(buffer_com_object_id)?;
        if !validate_notifications(buffer, &notifications)? {
            return Ok(false);
        }
        if buffer.notifications == notifications {
            return Ok(true);
        }
        let id = buffer.id;
        let cursor = first_notification_after(buffer, &notifications);

        let buffer = self.buffers.get_mut(&id).expect("checked audio buffer");
        buffer.notifications = notifications;
        buffer.notification_cursor = cursor;

        Ok(true)
    }

    fn notification_buffer_for_handles(
        &self,
        handles: &[RuntimeHandle],
    ) -> Option<&RuntimeAudioBuffer> {
        self.buffers.values().find(|buffer| {
            buffer.notifications.len() == handles.len()
                && buffer
                    .notifications
                    .iter()
                    .zip(handles)
                    .all(|(notification, handle)| notification.event_handle == *handle)
        })
    }

    pub(super) fn matches_notification_wait_handles(&self, handles: &[RuntimeHandle]) -> bool {
        self.notification_buffer_for_handles(handles).is_some()
    }

    pub(super) fn plan_notification_wait(
        &self,
        handles: &[RuntimeHandle],
        timeout_milliseconds: u32,
    ) -> Result<Option<RuntimeAudioNotificationWaitPlan>, RuntimeStateError> {
        if handles.is_empty() || handles.len() > RUNTIME_AUDIO_NOTIFICATION_LIMIT {
            return Err(RuntimeStateError::new(
                "audio notification wait requires a bounded nonempty handle array",
            ));
        }
        let buffer = self
            .notification_buffer_for_handles(handles)
            .ok_or_else(|| {
                RuntimeStateError::new("wait handle array does not match a notification buffer")
            })?;
        if buffer.playback != RuntimeAudioPlaybackState::PlayingLooping {
            return Ok(None);
        }
        let format = buffer.active_format.ok_or_else(|| {
            RuntimeStateError::new("playing notification buffer has no PCM format")
        })?;
        let notification_index = normalize_notification_cursor(buffer, buffer.notification_cursor)
            .ok_or_else(|| {
                RuntimeStateError::new("notification wait has no playable position notification")
            })?;
        let notification = buffer.notifications[notification_index];
        let position = buffer.controls.current_position_bytes;
        let byte_length = buffer.descriptor.byte_length;
        let distance = if notification.offset_bytes > position {
            notification.offset_bytes - position
        } else {
            byte_length - position + notification.offset_bytes
        };
        let milliseconds_to_notification = u64::from(distance)
            .saturating_mul(1000)
            .div_ceil(u64::from(format.average_bytes_per_second))
            .max(1);
        let fires = milliseconds_to_notification <= u64::from(timeout_milliseconds);
        let delay_milliseconds = if fires {
            milliseconds_to_notification as u32
        } else {
            timeout_milliseconds
        };
        let advanced_bytes = if fires {
            distance
        } else {
            u64::from(format.average_bytes_per_second)
                .saturating_mul(u64::from(delay_milliseconds))
                .checked_div(1000)
                .unwrap_or(0) as u32
        };
        let next_position_bytes = if fires {
            notification.offset_bytes
        } else {
            (u64::from(position) + u64::from(advanced_bytes))
                .checked_rem(u64::from(byte_length))
                .unwrap_or(0) as u32
        };
        Ok(Some(RuntimeAudioNotificationWaitPlan {
            buffer_id: buffer.id,
            next_position_bytes,
            next_cursor: if fires {
                next_playable_notification(&buffer.notifications, notification_index)
                    .unwrap_or(notification_index)
            } else {
                notification_index
            },
            delay_milliseconds,
            result_index: fires.then_some(notification_index as u32),
            timeout_milliseconds,
        }))
    }

    pub(super) fn validate_stopped_notification_wait(
        &self,
        handles: &[RuntimeHandle],
        timeout_milliseconds: u32,
        active_payload: &[u8],
    ) -> Result<(), RuntimeStateError> {
        let buffer = self
            .notification_buffer_for_handles(handles)
            .ok_or_else(|| {
                RuntimeStateError::new("wait handle array does not match a notification buffer")
            })?;
        let word = |index: usize| {
            active_payload
                .get(index * 4..index * 4 + 4)
                .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        };
        let payload_buffer_id = word(2)
            .zip(word(3))
            .map(|(low, high)| u64::from(low) | (u64::from(high) << 32));
        if buffer.playback != RuntimeAudioPlaybackState::Stopped
            || handles.is_empty()
            || handles.len() > RUNTIME_AUDIO_NOTIFICATION_LIMIT
            || active_payload.len() != (6 + handles.len()) * 4
            || word(0) != Some(1)
            || payload_buffer_id != Some(buffer.id.0)
            || word(4) != Some(timeout_milliseconds)
            || word(5) != Some(handles.len() as u32)
            || !handles
                .iter()
                .enumerate()
                .all(|(index, handle)| word(6 + index) == Some(handle.0))
        {
            return Err(RuntimeStateError::new(
                "stopped notification completion does not match its active wait contract",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_notification_wait(&mut self, plan: RuntimeAudioNotificationWaitPlan) {
        let buffer = self
            .buffers
            .get_mut(&plan.buffer_id)
            .expect("validated audio notification buffer disappeared");
        buffer.controls.current_position_bytes = plan.next_position_bytes;
        buffer.notification_cursor = plan.next_cursor;
    }

    pub(super) fn validate_buffer_play(
        &self,
        buffer_com_object_id: RuntimeComObjectId,
        playback: RuntimeAudioPlaybackState,
    ) -> Result<Option<Option<RuntimeHandle>>, RuntimeStateError> {
        let buffer = self.buffer_by_com_object(buffer_com_object_id)?;
        let device = self.device(buffer.device_id)?;
        if device.id != buffer.device_id
            || self.device_com_object_index.get(&device.com_object_id) != Some(&device.id)
            || !device.buffers.contains(&buffer.id)
        {
            return Err(RuntimeStateError::new(
                "audio playback buffer has no matching parent device ownership",
            ));
        }
        let ready = buffer_ready_for_play(buffer)?;
        if playback != RuntimeAudioPlaybackState::Stopped && !ready {
            return Ok(None);
        }
        require_priority_cooperative_state(device)?;
        if buffer.playback == playback {
            return Ok(Some(None));
        }

        Ok(Some(
            (playback == RuntimeAudioPlaybackState::Stopped)
                .then(|| {
                    buffer
                        .notifications
                        .iter()
                        .find(|notification| notification.offset_bytes == u32::MAX)
                        .map(|notification| notification.event_handle)
                })
                .flatten(),
        ))
    }

    #[cfg(test)]
    fn set_buffer_play(
        &mut self,
        buffer_com_object_id: RuntimeComObjectId,
        playback: RuntimeAudioPlaybackState,
    ) -> Result<bool, RuntimeStateError> {
        if self
            .validate_buffer_play(buffer_com_object_id, playback)?
            .is_none()
        {
            return Ok(false);
        }
        self.finish_buffer_play(buffer_com_object_id, playback);
        Ok(true)
    }

    pub(super) fn finish_buffer_play(
        &mut self,
        buffer_com_object_id: RuntimeComObjectId,
        playback: RuntimeAudioPlaybackState,
    ) {
        // Called immediately after validation; signaling a stop event cannot mutate audio state.
        let buffer_id = self.buffer_com_object_index[&buffer_com_object_id];
        let buffer = self
            .buffers
            .get_mut(&buffer_id)
            .expect("validated audio buffer disappeared");
        let changed = buffer.playback != playback;
        if changed {
            buffer.playback = playback;
            if playback != RuntimeAudioPlaybackState::Stopped {
                buffer.notification_cursor =
                    first_notification_after(buffer, &buffer.notifications);
            }
        }
    }

    pub(super) fn set_buffer_control(
        &mut self,
        buffer_com_object_id: RuntimeComObjectId,
        update: RuntimeAudioBufferControlUpdate,
    ) -> Result<bool, RuntimeStateError> {
        let Some(next) = self.preview_buffer_control(buffer_com_object_id, update)? else {
            return Ok(false);
        };
        let buffer = self.buffer_by_com_object(buffer_com_object_id)?;
        let id = buffer.id;
        if buffer.controls == next {
            return Ok(true);
        }

        let buffer = self.buffers.get_mut(&id).expect("checked audio buffer");
        buffer.controls = next;
        if matches!(update, RuntimeAudioBufferControlUpdate::CurrentPosition(_)) {
            buffer.notification_cursor = first_notification_after(buffer, &buffer.notifications);
        }

        Ok(true)
    }

    pub(super) fn preview_buffer_control(
        &self,
        object_id: RuntimeComObjectId,
        update: RuntimeAudioBufferControlUpdate,
    ) -> Result<Option<RuntimeAudioBufferControls>, RuntimeStateError> {
        let buffer = self.buffer_by_com_object(object_id)?;
        let Some(next) = apply_buffer_control(buffer, update) else {
            return Ok(None);
        };
        require_priority_cooperative_state(self.device(buffer.device_id)?)?;
        Ok(Some(next))
    }

    pub(super) fn set_priority_cooperative_level(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        target: RuntimeAudioCooperativeState,
    ) -> Result<(), RuntimeStateError> {
        validate_cooperative_target(target)?;
        let device = self.device_by_com_object(device_com_object_id)?;
        let id = device.id;
        if device.cooperative == Some(target) {
            return Ok(());
        }

        self.devices
            .get_mut(&id)
            .expect("checked audio device")
            .cooperative = Some(target);

        Ok(())
    }

    pub(crate) fn validate_com_object_retirements(
        &self,
        com_object_ids: &BTreeSet<RuntimeComObjectId>,
    ) -> Result<Option<RuntimeAudioComRetirement>, RuntimeStateError> {
        // An absent reverse index must not silently omit a live retiring resource.
        for (device_id, device) in &self.devices {
            if com_object_ids.contains(&device.com_object_id)
                && (device.id != *device_id
                    || self.device_com_object_index.get(&device.com_object_id) != Some(device_id))
            {
                return Err(RuntimeStateError::new(
                    "retiring audio device disagrees with its live identity or COM index",
                ));
            }
        }
        for (buffer_id, buffer) in &self.buffers {
            if com_object_ids.contains(&buffer.com_object_id)
                && (buffer.id != *buffer_id
                    || self.buffer_com_object_index.get(&buffer.com_object_id) != Some(buffer_id))
            {
                return Err(RuntimeStateError::new(
                    "retiring audio buffer disagrees with its live identity or COM index",
                ));
            }
        }
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
        Ok(Some(RuntimeAudioComRetirement {
            device_associations,
            buffer_associations,
        }))
    }

    pub(crate) fn buffer_backings_for_retirement(
        &self,
        plan: &RuntimeAudioComRetirement,
    ) -> Vec<(RuntimeAudioBufferBacking, u32)> {
        plan.buffer_associations
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
            .collect()
    }

    pub(crate) fn remove_validated_com_objects(&mut self, plan: RuntimeAudioComRetirement) {
        let retiring_devices = plan
            .device_associations
            .iter()
            .map(|(_, device_id)| *device_id)
            .collect::<BTreeSet<_>>();
        for (com_object_id, buffer_id) in plan.buffer_associations {
            let buffer = self
                .buffers
                .remove(&buffer_id)
                .expect("validated logical audio buffer disappeared");
            self.buffer_com_object_index.remove(&com_object_id);
            if !retiring_devices.contains(&buffer.device_id) {
                let device = self
                    .devices
                    .get_mut(&buffer.device_id)
                    .expect("validated logical audio buffer parent disappeared");
                device.buffers.remove(&buffer.id);
            }
        }
        for (com_object_id, device_id) in plan.device_associations {
            self.device_com_object_index.remove(&com_object_id);
            self.devices
                .remove(&device_id)
                .expect("validated logical audio device disappeared");
        }
    }

    fn validate_retirement_associations(
        &self,
        device_associations: &[(RuntimeComObjectId, RuntimeAudioDeviceId)],
        buffer_associations: &[(RuntimeComObjectId, RuntimeAudioBufferId)],
        retiring_com_objects: &BTreeSet<RuntimeComObjectId>,
    ) -> Result<(), RuntimeStateError> {
        for (com_object_id, device_id) in device_associations {
            let device = self.devices.get(device_id).ok_or_else(|| {
                RuntimeStateError::new("audio COM retirement references no live logical device")
            })?;
            if device.id != *device_id
                || device.com_object_id != *com_object_id
                || self.device_com_object_index.get(com_object_id) != Some(device_id)
            {
                return Err(RuntimeStateError::new(
                    "audio COM retirement association disagrees with authoritative indexes",
                ));
            }
            if device.buffers.iter().any(|buffer_id| {
                self.buffers
                    .get(buffer_id)
                    .is_none_or(|buffer| !retiring_com_objects.contains(&buffer.com_object_id))
            }) {
                return Err(RuntimeStateError::new(
                    "logical audio device cannot retire before all owned buffers",
                ));
            }
        }
        for (com_object_id, buffer_id) in buffer_associations {
            let buffer = self.buffers.get(buffer_id).ok_or_else(|| {
                RuntimeStateError::new("audio COM retirement references no live logical buffer")
            })?;
            let device = self.devices.get(&buffer.device_id).ok_or_else(|| {
                RuntimeStateError::new("audio buffer retirement references no live logical device")
            })?;
            if buffer.id != *buffer_id
                || buffer.com_object_id != *com_object_id
                || self.buffer_com_object_index.get(com_object_id) != Some(buffer_id)
                || device.id != buffer.device_id
                || self.device_com_object_index.get(&device.com_object_id) != Some(&device.id)
                || !device.buffers.contains(buffer_id)
            {
                return Err(RuntimeStateError::new(
                    "audio buffer COM retirement association disagrees with authoritative indexes",
                ));
            }
        }
        Ok(())
    }
}

fn derive_buffer_lock(
    buffer: &RuntimeAudioBuffer,
    offset: u32,
    byte_length: u32,
    backing_start: GuestAddress,
) -> Result<RuntimeAudioBufferLock, RuntimeStateError> {
    let format = buffer.active_format.ok_or_else(|| {
        RuntimeStateError::new("secondary audio Lock requires an initialized PCM format")
    })?;
    let buffer_length = buffer.descriptor.byte_length;
    let block_align = u32::from(format.block_align);
    if offset >= buffer_length
        || byte_length == 0
        || byte_length > buffer_length
        || !offset.is_multiple_of(block_align)
        || !byte_length.is_multiple_of(block_align)
    {
        return Err(RuntimeStateError::new(
            "audio Lock region must be nonempty, PCM-aligned, and no larger than the secondary buffer",
        ));
    }
    let first_byte_length = byte_length.min(buffer_length - offset);
    let second_byte_length = byte_length - first_byte_length;
    let first_address = backing_start.0.checked_add(offset).ok_or_else(|| {
        RuntimeStateError::new("audio Lock segment overflowed the x86 guest address space")
    })?;
    Ok(RuntimeAudioBufferLock {
        first_segment: RuntimeAudioBufferSegment {
            guest_address: GuestAddress(first_address),
            byte_length: first_byte_length,
        },
        second_segment: (second_byte_length != 0).then_some(RuntimeAudioBufferSegment {
            guest_address: backing_start,
            byte_length: second_byte_length,
        }),
    })
}

fn audio_buffer_locks_overlap(left: RuntimeAudioBufferLock, right: RuntimeAudioBufferLock) -> bool {
    [Some(left.first_segment), left.second_segment]
        .into_iter()
        .flatten()
        .any(|left_segment| {
            [Some(right.first_segment), right.second_segment]
                .into_iter()
                .flatten()
                .any(|right_segment| audio_buffer_segments_overlap(left_segment, right_segment))
        })
}

fn audio_buffer_segment_contains(
    lock: RuntimeAudioBufferSegment,
    request: RuntimeAudioBufferSegment,
) -> bool {
    request
        .guest_address
        .0
        .checked_sub(lock.guest_address.0)
        .and_then(|offset| lock.byte_length.checked_sub(offset))
        .is_some_and(|remaining| request.byte_length <= remaining)
}

fn audio_buffer_segments_overlap(
    left: RuntimeAudioBufferSegment,
    right: RuntimeAudioBufferSegment,
) -> bool {
    let left_start = u64::from(left.guest_address.0);
    let left_end = left_start + u64::from(left.byte_length);
    let right_start = u64::from(right.guest_address.0);
    let right_end = right_start + u64::from(right.byte_length);
    left_start < right_end && right_start < left_end
}

fn validate_cooperative_target(
    target: RuntimeAudioCooperativeState,
) -> Result<(), RuntimeStateError> {
    if target.window_id.0 == 0 || target.window_handle == 0 || target.owner_thread_id.0 == 0 {
        return Err(RuntimeStateError::new(
            "audio cooperative state requires nonzero logical window identities",
        ));
    }
    Ok(())
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
            let format = descriptor.format.expect("checked secondary format");
            validate_pcm_format(format)?;
            if u64::from(descriptor.byte_length) > RUNTIME_AUDIO_RESOURCE_ARENA_SIZE
                || !descriptor
                    .byte_length
                    .is_multiple_of(u32::from(format.block_align))
            {
                return Err(RuntimeStateError::new(
                    "logical secondary audio buffer must fit the audio arena and contain whole PCM frames",
                ));
            }
        }
        _ => {
            return Err(RuntimeStateError::new(
                "logical primary and secondary audio buffer descriptor shapes differ",
            ));
        }
    }
    if descriptor.flags == 0 {
        return Err(RuntimeStateError::new(
            "logical audio buffer flags must identify a bounded DirectSound variant",
        ));
    }
    Ok(())
}

fn validate_notifications(
    buffer: &RuntimeAudioBuffer,
    notifications: &[RuntimeAudioNotification],
) -> Result<bool, RuntimeStateError> {
    if buffer.descriptor.kind != RuntimeAudioBufferKind::Secondary
        || notifications.is_empty()
        || notifications.len() > RUNTIME_AUDIO_NOTIFICATION_LIMIT
    {
        return Ok(false);
    }
    let Some(format) = buffer.active_format else {
        return Ok(false);
    };
    let block_align = u32::from(format.block_align);
    if block_align == 0 {
        return Err(RuntimeStateError::new(
            "audio notification buffer has zero PCM block alignment",
        ));
    }
    let mut handles = BTreeSet::new();
    let mut stop_count = 0;
    let mut previous_offset = None;
    for notification in notifications {
        if notification.event_handle.0 == 0 || !handles.insert(notification.event_handle) {
            return Ok(false);
        }
        if notification.offset_bytes == u32::MAX {
            stop_count += 1;
            continue;
        }
        if notification.offset_bytes >= buffer.descriptor.byte_length
            || !notification.offset_bytes.is_multiple_of(block_align)
            || previous_offset.is_some_and(|previous| notification.offset_bytes <= previous)
        {
            return Ok(false);
        }
        previous_offset = Some(notification.offset_bytes);
    }
    if stop_count > 1 {
        return Ok(false);
    }
    Ok(true)
}

fn first_notification_after(
    buffer: &RuntimeAudioBuffer,
    notifications: &[RuntimeAudioNotification],
) -> usize {
    notifications
        .iter()
        .position(|notification| {
            notification.offset_bytes != u32::MAX
                && notification.offset_bytes > buffer.controls.current_position_bytes
        })
        .or_else(|| {
            notifications
                .iter()
                .position(|notification| notification.offset_bytes != u32::MAX)
        })
        .unwrap_or(0)
}

fn normalize_notification_cursor(buffer: &RuntimeAudioBuffer, cursor: usize) -> Option<usize> {
    if buffer
        .notifications
        .get(cursor)
        .is_some_and(|notification| notification.offset_bytes != u32::MAX)
    {
        Some(cursor)
    } else {
        buffer
            .notifications
            .iter()
            .position(|notification| notification.offset_bytes != u32::MAX)
    }
}

fn next_playable_notification(
    notifications: &[RuntimeAudioNotification],
    current: usize,
) -> Option<usize> {
    (1..=notifications.len())
        .map(|step| (current + step) % notifications.len())
        .find(|&index| notifications[index].offset_bytes != u32::MAX)
}

fn buffer_ready_for_play(buffer: &RuntimeAudioBuffer) -> Result<bool, RuntimeStateError> {
    if buffer
        .active_format
        .is_some_and(|format| format.block_align == 0)
    {
        return Err(RuntimeStateError::new(
            "audio playback buffer has zero PCM block alignment",
        ));
    }
    if let Some(backing) = buffer.backing
        && (backing.memory_owner.0 == 0
            || backing.address_allocation_id.0 == 0
            || backing.range.byte_length() < u64::from(buffer.descriptor.byte_length))
    {
        return Err(RuntimeStateError::new(
            "audio playback buffer has an invalid owned sample backing",
        ));
    }
    if (buffer.initialized || !buffer.active_locks.is_empty()) && buffer.backing.is_none() {
        return Err(RuntimeStateError::new(
            "initialized or locked audio buffer has no sample backing",
        ));
    }
    Ok(match buffer.descriptor.kind {
        RuntimeAudioBufferKind::Primary => buffer.active_format.is_some(),
        RuntimeAudioBufferKind::Secondary => {
            buffer.active_format.is_some()
                && buffer.backing.is_some()
                && buffer.active_locks.is_empty()
                && buffer.initialized
        }
    })
}

fn apply_buffer_control(
    buffer: &RuntimeAudioBuffer,
    update: RuntimeAudioBufferControlUpdate,
) -> Option<RuntimeAudioBufferControls> {
    let mut next = buffer.controls;
    match update {
        RuntimeAudioBufferControlUpdate::Volume(value)
            if (RUNTIME_AUDIO_VOLUME_MIN_HUNDREDTHS_DB
                ..=RUNTIME_AUDIO_VOLUME_MAX_HUNDREDTHS_DB)
                .contains(&value) =>
        {
            next.volume_hundredths_db = value;
        }
        RuntimeAudioBufferControlUpdate::Pan(value)
            if (RUNTIME_AUDIO_PAN_MIN_HUNDREDTHS_DB..=RUNTIME_AUDIO_PAN_MAX_HUNDREDTHS_DB)
                .contains(&value) =>
        {
            next.pan_hundredths_db = value;
        }
        RuntimeAudioBufferControlUpdate::Frequency(value)
            if (RUNTIME_AUDIO_PLAYBACK_FREQUENCY_MIN_HZ
                ..=RUNTIME_AUDIO_PLAYBACK_FREQUENCY_MAX_HZ)
                .contains(&value)
                && buffer.active_format.is_some() =>
        {
            next.frequency_hz = Some(value);
        }
        RuntimeAudioBufferControlUpdate::CurrentPosition(value)
            if buffer.descriptor.kind == RuntimeAudioBufferKind::Secondary
                && value < buffer.descriptor.byte_length
                && buffer
                    .active_format
                    .is_some_and(|format| value.is_multiple_of(u32::from(format.block_align))) =>
        {
            next.current_position_bytes = value;
        }
        _ => {
            return None;
        }
    }
    Some(next)
}

fn validate_pcm_format(format: RuntimeAudioPcmFormat) -> Result<(), RuntimeStateError> {
    let bytes_per_sample = u32::from(format.bits_per_sample) / 8;
    let expected_block_align = u32::from(format.channels)
        .checked_mul(bytes_per_sample)
        .filter(|_| format.bits_per_sample.is_multiple_of(8));
    let expected_average = expected_block_align
        .and_then(|block_align| format.samples_per_second.checked_mul(block_align));
    if format.channels == 0
        || format.channels > RUNTIME_AUDIO_PCM_MAX_CHANNELS
        || !(RUNTIME_AUDIO_PCM_MIN_SAMPLES_PER_SECOND..=RUNTIME_AUDIO_PCM_MAX_SAMPLES_PER_SECOND)
            .contains(&format.samples_per_second)
        || !matches!(format.bits_per_sample, 8 | 16)
        || expected_block_align != Some(u32::from(format.block_align))
        || expected_average != Some(format.average_bytes_per_second)
    {
        return Err(RuntimeStateError::new(
            "logical PCM format fields are incoherent or overflow their x86 representation",
        ));
    }
    Ok(())
}

fn require_priority_cooperative_state(
    device: &RuntimeAudioDevice,
) -> Result<(), RuntimeStateError> {
    if device.cooperative.is_none() {
        return Err(RuntimeStateError::new(
            "logical audio buffer initialization requires priority cooperative state",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> RuntimeAudioCooperativeState {
        RuntimeAudioCooperativeState {
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
        audio.create_device(RuntimeComObjectId(13)).unwrap();
        audio
            .set_priority_cooperative_level(RuntimeComObjectId(13), target())
            .unwrap();
        audio
    }

    #[test]
    fn direct_buffer_updates_validate_before_mutation_and_preserve_noops() {
        let mut audio = priority_audio();
        let primary = audio
            .validate_buffer_creation(RuntimeComObjectId(13), primary_descriptor())
            .unwrap();
        audio.insert_buffer(primary, RuntimeComObjectId(21));
        let secondary = audio
            .validate_buffer_creation(RuntimeComObjectId(13), secondary_descriptor())
            .unwrap();
        audio.insert_buffer(secondary, RuntimeComObjectId(22));

        let invalid_format = RuntimeAudioPcmFormat {
            block_align: 1,
            ..pcm_format()
        };
        let before = audio.clone();
        assert!(
            audio
                .set_buffer_format(RuntimeComObjectId(21), invalid_format)
                .is_err()
        );
        assert_eq!(audio, before);
        assert!(
            audio
                .set_buffer_format(RuntimeComObjectId(22), pcm_format())
                .is_err()
        );
        assert_eq!(audio, before);

        let format = pcm_format();
        audio
            .set_buffer_format(RuntimeComObjectId(21), format)
            .unwrap();
        assert_eq!(
            audio
                .buffer_by_com_object(RuntimeComObjectId(21))
                .unwrap()
                .active_format,
            Some(format)
        );
        let after_format = audio.clone();
        audio
            .set_buffer_format(RuntimeComObjectId(21), format)
            .unwrap();
        assert_eq!(audio, after_format);

        let event =
            RuntimeHandle::from_parts(crate::runtime::RuntimeObjectKind::Event, 1, 1).unwrap();
        let invalid_notifications = vec![
            RuntimeAudioNotification {
                offset_bytes: 0,
                event_handle: event,
            },
            RuntimeAudioNotification {
                offset_bytes: 4,
                event_handle: event,
            },
        ];
        let before = audio.clone();
        assert!(
            !audio
                .set_buffer_notifications(RuntimeComObjectId(22), invalid_notifications)
                .unwrap()
        );
        assert_eq!(audio, before);
        let notifications = vec![RuntimeAudioNotification {
            offset_bytes: 0,
            event_handle: event,
        }];
        audio
            .set_buffer_notifications(RuntimeComObjectId(22), notifications.clone())
            .unwrap();
        assert_eq!(
            audio
                .buffer_by_com_object(RuntimeComObjectId(22))
                .unwrap()
                .notifications,
            notifications
        );
        let after_notifications = audio.clone();
        audio
            .set_buffer_notifications(RuntimeComObjectId(22), notifications)
            .unwrap();
        assert_eq!(audio, after_notifications);

        let before = audio.clone();
        assert!(
            !audio
                .set_buffer_control(
                    RuntimeComObjectId(22),
                    RuntimeAudioBufferControlUpdate::Volume(-10_001)
                )
                .unwrap()
        );
        assert_eq!(audio, before);
        let control = RuntimeAudioBufferControlUpdate::Volume(-1_900);
        audio
            .set_buffer_control(RuntimeComObjectId(22), control)
            .unwrap();
        let after_control = audio.clone();
        audio
            .set_buffer_control(RuntimeComObjectId(22), control)
            .unwrap();
        assert_eq!(audio, after_control);
    }

    #[test]
    fn device_creation_rejects_invalid_and_duplicate_com_identity() {
        let mut audio = RuntimeAudio::default();
        let before = audio.clone();
        assert!(audio.create_device(RuntimeComObjectId(0)).is_err());
        assert_eq!(audio, before);
        let first = audio.create_device(RuntimeComObjectId(13)).unwrap();
        let before = audio.clone();
        assert!(audio.create_device(RuntimeComObjectId(13)).is_err());
        assert_eq!(audio, before);
        audio
            .set_priority_cooperative_level(RuntimeComObjectId(13), target())
            .unwrap();
        let creation = audio
            .validate_buffer_creation(RuntimeComObjectId(13), secondary_descriptor())
            .unwrap();
        audio.insert_buffer(creation, RuntimeComObjectId(14));
        let before = audio.clone();
        assert!(audio.create_device(RuntimeComObjectId(14)).is_err());
        assert_eq!(audio, before);
        let second = audio.create_device(RuntimeComObjectId(15)).unwrap();
        assert_ne!(second, first);
        assert_eq!(
            audio
                .device_by_com_object(RuntimeComObjectId(15))
                .unwrap()
                .id,
            second
        );
    }

    #[test]
    fn direct_device_creation_and_deferred_com_retirement_preserve_owner_state() {
        let mut audio = RuntimeAudio::default();
        let device = audio.create_device(RuntimeComObjectId(13)).unwrap();
        assert_eq!(device, RuntimeAudioDeviceId(1));
        assert_eq!(audio.device(device).unwrap().cooperative, None);

        audio
            .set_priority_cooperative_level(RuntimeComObjectId(13), target())
            .unwrap();
        let device = audio.device(RuntimeAudioDeviceId(1)).unwrap();
        let cooperative = device.cooperative.unwrap();
        assert_eq!(cooperative.window_id, RuntimeWindowId(7));
        let before_unchanged = audio.clone();
        audio
            .set_priority_cooperative_level(RuntimeComObjectId(13), target())
            .unwrap();
        assert_eq!(audio, before_unchanged);

        let retirements = [RuntimeComObjectId(13)].into_iter().collect();
        let retirement = audio
            .validate_com_object_retirements(&retirements)
            .unwrap()
            .unwrap();
        audio.remove_validated_com_objects(retirement);
        assert!(audio.devices.is_empty() && audio.buffers.is_empty());
        assert!(audio.device_com_object_index.is_empty());
    }

    #[test]
    fn unchanged_cooperative_level_preserves_state_and_validates_live_target() {
        let mut audio = priority_audio();
        let before = audio.clone();
        audio
            .set_priority_cooperative_level(RuntimeComObjectId(13), target())
            .unwrap();
        assert_eq!(audio, before);
        for object in [RuntimeComObjectId(0), RuntimeComObjectId(999)] {
            assert!(
                audio
                    .set_priority_cooperative_level(object, target(),)
                    .is_err()
            );
            assert_eq!(audio, before);
        }
        assert!(
            audio
                .set_priority_cooperative_level(
                    RuntimeComObjectId(13),
                    RuntimeAudioCooperativeState {
                        window_id: RuntimeWindowId(0),
                        ..target()
                    },
                )
                .is_err()
        );
        assert_eq!(audio, before);
    }

    #[test]
    fn invalid_targets_and_capacity_fail_without_mutation() {
        let mut audio = RuntimeAudio::default();
        let before = audio.clone();
        audio
            .set_priority_cooperative_level(
                RuntimeComObjectId(1),
                RuntimeAudioCooperativeState {
                    window_id: RuntimeWindowId(0),
                    ..target()
                },
            )
            .unwrap_err();
        assert_eq!(audio, before);

        for index in 0..RUNTIME_AUDIO_DEVICE_LIMIT {
            audio
                .create_device(RuntimeComObjectId(index as u64 + 1))
                .unwrap();
        }
        let before = audio.clone();
        audio
            .create_device(RuntimeComObjectId(u64::MAX))
            .unwrap_err();
        assert_eq!(audio, before);
    }

    #[test]
    fn buffers_own_format_playback_and_parent_retirement_relations() {
        let mut audio = priority_audio();
        let primary_plan = audio
            .validate_buffer_creation(RuntimeComObjectId(13), primary_descriptor())
            .unwrap();
        let primary_id = audio.insert_buffer(primary_plan, RuntimeComObjectId(21));
        assert_eq!(primary_id, RuntimeAudioBufferId(1));
        assert_eq!(
            audio.buffers[&primary_id].device_id,
            RuntimeAudioDeviceId(1)
        );
        assert_eq!(audio.buffers[&primary_id].active_format, None);
        assert_eq!(
            audio.buffers[&primary_id].playback,
            RuntimeAudioPlaybackState::Stopped
        );

        audio
            .set_buffer_format(RuntimeComObjectId(21), pcm_format())
            .unwrap();
        assert_eq!(
            audio.buffer(primary_id).unwrap().active_format,
            Some(pcm_format())
        );

        audio
            .set_buffer_play(
                RuntimeComObjectId(21),
                RuntimeAudioPlaybackState::PlayingLooping,
            )
            .unwrap();
        assert_eq!(
            audio.buffer(RuntimeAudioBufferId(1)).unwrap().playback,
            RuntimeAudioPlaybackState::PlayingLooping
        );
        audio
            .set_buffer_play(RuntimeComObjectId(21), RuntimeAudioPlaybackState::Stopped)
            .unwrap();
        assert_eq!(
            audio.buffer(RuntimeAudioBufferId(1)).unwrap().playback,
            RuntimeAudioPlaybackState::Stopped
        );
        audio
            .set_buffer_play(RuntimeComObjectId(21), RuntimeAudioPlaybackState::Stopped)
            .unwrap();

        let secondary_plan = audio
            .validate_buffer_creation(RuntimeComObjectId(13), secondary_descriptor())
            .unwrap();
        let secondary_id = audio.insert_buffer(secondary_plan, RuntimeComObjectId(22));
        assert_eq!(secondary_id, RuntimeAudioBufferId(2));
        assert_eq!(
            audio.buffers[&secondary_id].active_format,
            Some(pcm_format())
        );
        assert_eq!(
            audio.device(RuntimeAudioDeviceId(1)).unwrap().buffers.len(),
            2
        );

        let device_only = [RuntimeComObjectId(13)].into_iter().collect();
        audio
            .validate_com_object_retirements(&device_only)
            .unwrap_err();
        let primary_only = [RuntimeComObjectId(21)].into_iter().collect();
        let retirement = audio
            .validate_com_object_retirements(&primary_only)
            .unwrap()
            .unwrap();
        audio.remove_validated_com_objects(retirement);
        assert!(audio.buffer(RuntimeAudioBufferId(1)).is_err());
        assert_eq!(
            audio.device(RuntimeAudioDeviceId(1)).unwrap().buffers.len(),
            1
        );

        let remaining = [RuntimeComObjectId(13), RuntimeComObjectId(22)]
            .into_iter()
            .collect();
        let retirement = audio
            .validate_com_object_retirements(&remaining)
            .unwrap()
            .unwrap();
        audio.remove_validated_com_objects(retirement);
        assert!(audio.devices.is_empty() && audio.buffers.is_empty());
        assert!(audio.device_com_object_index.is_empty());
        assert!(audio.buffer_com_object_index.is_empty());
    }

    #[test]
    fn secondary_lock_unlock_and_play_preserve_backing_and_initialization() {
        let mut audio = priority_audio();
        let secondary_plan = audio
            .validate_buffer_creation(RuntimeComObjectId(13), secondary_descriptor())
            .unwrap();
        audio.insert_buffer(secondary_plan, RuntimeComObjectId(22));
        let before_uninitialized_play = audio.clone();
        assert!(
            !audio
                .set_buffer_play(
                    RuntimeComObjectId(22),
                    RuntimeAudioPlaybackState::PlayingLooping,
                )
                .unwrap()
        );
        assert_eq!(audio, before_uninitialized_play);

        let backing_plan = RuntimeAudioBufferBacking {
            memory_owner: MemoryOwnerId(77),
            address_allocation_id: ProcessAddressAllocationId(99),
            range: GuestRange::new(GuestAddress(0x4500_0000), 0x1_6000).unwrap(),
        };
        let lock = audio
            .lock_buffer(RuntimeComObjectId(22), 0, 88_200, 0, Some(backing_plan))
            .unwrap()
            .unwrap();
        assert_eq!(lock.first_segment.guest_address, GuestAddress(0x4500_0000));
        assert_eq!(lock.first_segment.byte_length, 88_200);
        assert_eq!(lock.second_segment, None);
        let secondary = audio.buffer(RuntimeAudioBufferId(1)).unwrap();
        assert_eq!(secondary.active_locks, vec![lock]);
        let installed_backing = secondary.backing.expect("installed audio sample backing");
        assert_eq!(
            installed_backing,
            RuntimeAudioBufferBacking {
                address_allocation_id: ProcessAddressAllocationId(99),
                memory_owner: MemoryOwnerId(77),
                range: backing_plan.range,
            }
        );
        assert!(!secondary.initialized);

        let before_double_lock = audio.clone();
        assert!(
            audio
                .preview_buffer_lock(RuntimeComObjectId(22), 0, 88_200, 0, None,)
                .unwrap()
                .is_none()
        );
        assert_eq!(audio, before_double_lock);
        assert_eq!(
            audio
                .select_buffer_unlock(
                    RuntimeComObjectId(22),
                    RuntimeAudioBufferSegment {
                        guest_address: GuestAddress(0x4500_0004),
                        byte_length: 88_200,
                    },
                    None
                )
                .unwrap(),
            None
        );
        assert_eq!(audio, before_double_lock);

        let selection = audio
            .select_buffer_unlock(
                RuntimeComObjectId(22),
                RuntimeAudioBufferSegment {
                    guest_address: GuestAddress(lock.first_segment.guest_address.0 + 4),
                    byte_length: 17_636,
                },
                lock.second_segment,
            )
            .unwrap();
        let Some((buffer_id, lock_index, true)) = selection else {
            panic!("contained unlock must release the last lock");
        };
        audio.finish_buffer_unlock(buffer_id, lock_index);
        assert!(audio.buffer(buffer_id).unwrap().initialized);
        assert_eq!(
            audio
                .select_buffer_unlock(
                    RuntimeComObjectId(22),
                    lock.first_segment,
                    lock.second_segment
                )
                .unwrap(),
            None
        );
        let handles = [
            RuntimeHandle::from_parts(crate::runtime::RuntimeObjectKind::Event, 1, 1).unwrap(),
            RuntimeHandle::from_parts(crate::runtime::RuntimeObjectKind::Event, 1, 2).unwrap(),
            RuntimeHandle::from_parts(crate::runtime::RuntimeObjectKind::Event, 1, 3).unwrap(),
        ];
        audio
            .set_buffer_notifications(
                RuntimeComObjectId(22),
                vec![
                    RuntimeAudioNotification {
                        offset_bytes: 0,
                        event_handle: handles[0],
                    },
                    RuntimeAudioNotification {
                        offset_bytes: 44_100,
                        event_handle: handles[1],
                    },
                    RuntimeAudioNotification {
                        offset_bytes: u32::MAX,
                        event_handle: handles[2],
                    },
                ],
            )
            .unwrap();
        let stopped = audio.clone();
        assert!(
            audio
                .plan_notification_wait(&handles, 100)
                .unwrap()
                .is_none()
        );
        assert_eq!(audio, stopped);
        let other_handles = [handles[2], handles[1], handles[0]];
        assert!(audio.plan_notification_wait(&other_handles, 100).is_err());
        assert_eq!(audio, stopped);
        audio
            .set_buffer_play(
                RuntimeComObjectId(22),
                RuntimeAudioPlaybackState::PlayingLooping,
            )
            .unwrap();
        assert_eq!(
            audio.buffer(RuntimeAudioBufferId(1)).unwrap().playback,
            RuntimeAudioPlaybackState::PlayingLooping
        );
        let pending_wait = audio
            .plan_notification_wait(&handles, 100)
            .unwrap()
            .unwrap();
        let pending_wait_payload = pending_wait.encode_host_payload(&handles);
        audio
            .set_buffer_control(
                RuntimeComObjectId(22),
                RuntimeAudioBufferControlUpdate::Volume(-1_900),
            )
            .unwrap();
        let rebased_wait = audio
            .plan_notification_wait(&handles, 100)
            .unwrap()
            .unwrap();
        assert_eq!(
            rebased_wait.encode_host_payload(&handles),
            pending_wait_payload
        );
        assert!(rebased_wait.matches_host_payload(&handles, &pending_wait_payload));
        assert_eq!(rebased_wait.result_index(), None);
        audio.commit_notification_wait(rebased_wait);
        assert_eq!(
            audio
                .buffer(RuntimeAudioBufferId(1))
                .unwrap()
                .controls
                .current_position_bytes,
            17_640
        );
        let overlapped_wait = audio
            .plan_notification_wait(&handles, 100)
            .unwrap()
            .unwrap();
        assert!(overlapped_wait.matches_host_payload(&handles, &pending_wait_payload));
        let mut changed_delay_payload = pending_wait_payload.clone();
        changed_delay_payload[4..8].copy_from_slice(&99u32.to_le_bytes());
        assert!(!overlapped_wait.matches_host_payload(&handles, &changed_delay_payload));
        let mut changed_handle_payload = pending_wait_payload.clone();
        changed_handle_payload[6 * 4..7 * 4].copy_from_slice(&0u32.to_le_bytes());
        assert!(!overlapped_wait.matches_host_payload(&handles, &changed_handle_payload));
        for length in 0..pending_wait_payload.len() {
            assert!(
                !overlapped_wait.matches_host_payload(&handles, &pending_wait_payload[..length])
            );
        }
        for index in 0..9 {
            let mut altered = pending_wait_payload.clone();
            altered[index * 4] ^= 1;
            assert!(!overlapped_wait.matches_host_payload(&handles, &altered));
        }
        let wait = audio
            .plan_notification_wait(&handles, 100)
            .unwrap()
            .unwrap();
        assert_eq!(wait.delay_milliseconds, 100);
        assert_eq!(wait.result_index(), None);
        // Another device's lifecycle/settings must not invalidate this buffer's wait.
        audio.create_device(RuntimeComObjectId(33)).unwrap();
        audio
            .set_priority_cooperative_level(RuntimeComObjectId(33), target())
            .unwrap();
        assert_eq!(
            audio
                .plan_notification_wait(&handles, 100)
                .unwrap()
                .as_ref(),
            Some(&wait)
        );
        let unrelated = [RuntimeComObjectId(33)].into_iter().collect();
        let retirement = audio
            .validate_com_object_retirements(&unrelated)
            .unwrap()
            .unwrap();
        audio.remove_validated_com_objects(retirement);
        assert_eq!(
            audio
                .plan_notification_wait(&handles, 100)
                .unwrap()
                .as_ref(),
            Some(&wait)
        );
        audio.commit_notification_wait(wait);
        assert_eq!(
            audio
                .buffer(RuntimeAudioBufferId(1))
                .unwrap()
                .controls
                .current_position_bytes,
            35_280
        );
        let wait = audio
            .plan_notification_wait(&handles, 100)
            .unwrap()
            .unwrap();
        assert_eq!(wait.delay_milliseconds, 50);
        assert_eq!(wait.result_index(), Some(1));
        audio.commit_notification_wait(wait);
        assert_eq!(
            audio
                .buffer(RuntimeAudioBufferId(1))
                .unwrap()
                .controls
                .current_position_bytes,
            44_100
        );

        let stopped_wait_payload = audio
            .plan_notification_wait(&handles, 100)
            .unwrap()
            .unwrap()
            .encode_host_payload(&handles);
        audio
            .set_buffer_play(RuntimeComObjectId(22), RuntimeAudioPlaybackState::Stopped)
            .unwrap();
        audio
            .validate_stopped_notification_wait(&handles, 100, &stopped_wait_payload)
            .unwrap();
        let mut wrong_handles_payload = stopped_wait_payload;
        wrong_handles_payload[6 * 4..7 * 4].copy_from_slice(&0u32.to_le_bytes());
        audio
            .validate_stopped_notification_wait(&handles, 100, &wrong_handles_payload)
            .unwrap_err();

        let retirements = [RuntimeComObjectId(22)].into_iter().collect();
        let retirement = audio
            .validate_com_object_retirements(&retirements)
            .unwrap()
            .unwrap();
        assert_eq!(
            audio.buffer_backings_for_retirement(&retirement),
            vec![(installed_backing, 88_200)]
        );
    }

    #[test]
    fn initial_lock_rejects_invalid_backing_without_mutation_then_retries() {
        let mut audio = priority_audio();
        let creation = audio
            .validate_buffer_creation(RuntimeComObjectId(13), secondary_descriptor())
            .unwrap();
        audio.insert_buffer(creation, RuntimeComObjectId(22));
        let backing = RuntimeAudioBufferBacking {
            address_allocation_id: ProcessAddressAllocationId(99),
            memory_owner: MemoryOwnerId(77),
            range: GuestRange::new(GuestAddress(0x4500_0000), 0x1_6000).unwrap(),
        };
        for installation in [
            None,
            Some(RuntimeAudioBufferBacking {
                memory_owner: MemoryOwnerId(0),
                ..backing
            }),
            Some(RuntimeAudioBufferBacking {
                address_allocation_id: ProcessAddressAllocationId(0),
                ..backing
            }),
            Some(RuntimeAudioBufferBacking {
                range: GuestRange::new(backing.range.start(), 4).unwrap(),
                ..backing
            }),
        ] {
            let before = audio.clone();
            assert!(
                audio
                    .lock_buffer(RuntimeComObjectId(22), 0, 17_640, 0, installation)
                    .is_err()
            );
            assert_eq!(audio, before);
        }
        let lock = audio
            .lock_buffer(RuntimeComObjectId(22), 0, 17_640, 0, Some(backing))
            .unwrap()
            .unwrap();
        assert_eq!(lock.first_segment.guest_address, backing.range.start());
        assert_eq!(lock.first_segment.byte_length, 17_640);
    }

    #[test]
    fn secondary_non_overlapping_locks_can_be_active_and_unlock_independently() {
        let mut audio = priority_audio();
        let secondary_plan = audio
            .validate_buffer_creation(RuntimeComObjectId(13), secondary_descriptor())
            .unwrap();
        audio.insert_buffer(secondary_plan, RuntimeComObjectId(22));
        let backing_plan = RuntimeAudioBufferBacking {
            memory_owner: MemoryOwnerId(77),
            address_allocation_id: ProcessAddressAllocationId(99),
            range: GuestRange::new(GuestAddress(0x4500_0000), 0x1_6000).unwrap(),
        };

        let first = audio
            .lock_buffer(RuntimeComObjectId(22), 0, 17_640, 0, Some(backing_plan))
            .unwrap()
            .unwrap();
        let second = audio
            .lock_buffer(RuntimeComObjectId(22), 17_640, 17_640, 0, None)
            .unwrap()
            .unwrap();
        assert_eq!(
            audio.buffer(RuntimeAudioBufferId(1)).unwrap().active_locks,
            vec![first, second]
        );

        let before_overlapping_lock = audio.clone();
        assert!(
            audio
                .preview_buffer_lock(RuntimeComObjectId(22), 17_636, 8, 0, None,)
                .unwrap()
                .is_none()
        );
        assert_eq!(audio, before_overlapping_lock);

        let first_unlock = audio
            .select_buffer_unlock(
                RuntimeComObjectId(22),
                first.first_segment,
                first.second_segment,
            )
            .unwrap();
        let Some((buffer_id, lock_index, false)) = first_unlock else {
            panic!("first disjoint unlock must preserve the second lock");
        };
        audio.finish_buffer_unlock(buffer_id, lock_index);
        let secondary = audio.buffer(RuntimeAudioBufferId(1)).unwrap();
        assert_eq!(secondary.active_locks, vec![second]);
        assert!(secondary.initialized);
        assert!(
            !audio
                .set_buffer_play(
                    RuntimeComObjectId(22),
                    RuntimeAudioPlaybackState::PlayingLooping,
                )
                .unwrap()
        );

        let second_unlock = audio
            .select_buffer_unlock(
                RuntimeComObjectId(22),
                second.first_segment,
                second.second_segment,
            )
            .unwrap();
        let Some((buffer_id, lock_index, true)) = second_unlock else {
            panic!("second disjoint unlock must release the last lock");
        };
        audio.finish_buffer_unlock(buffer_id, lock_index);
        let secondary = audio.buffer(RuntimeAudioBufferId(1)).unwrap();
        assert!(secondary.active_locks.is_empty());
        assert!(secondary.initialized);
        audio
            .validate_buffer_play(
                RuntimeComObjectId(22),
                RuntimeAudioPlaybackState::PlayingLooping,
            )
            .unwrap();
    }

    #[test]
    fn secondary_partial_and_wrapped_locks_share_the_full_buffer_backing() {
        let mut audio = priority_audio();
        let secondary_plan = audio
            .validate_buffer_creation(RuntimeComObjectId(13), secondary_descriptor())
            .unwrap();
        audio.insert_buffer(secondary_plan, RuntimeComObjectId(22));
        let backing_plan = RuntimeAudioBufferBacking {
            memory_owner: MemoryOwnerId(77),
            address_allocation_id: ProcessAddressAllocationId(99),
            range: GuestRange::new(GuestAddress(0x4500_0000), 0x1_6000).unwrap(),
        };

        let partial = audio
            .lock_buffer(RuntimeComObjectId(22), 4, 80_000, 0, Some(backing_plan))
            .unwrap()
            .unwrap();
        assert_eq!(
            partial,
            RuntimeAudioBufferLock {
                first_segment: RuntimeAudioBufferSegment {
                    guest_address: GuestAddress(0x4500_0004),
                    byte_length: 80_000,
                },
                second_segment: None,
            }
        );
        let unlock = audio
            .select_buffer_unlock(
                RuntimeComObjectId(22),
                partial.first_segment,
                partial.second_segment,
            )
            .unwrap();
        let Some((buffer_id, lock_index, _)) = unlock else {
            panic!("partial Lock must be unlockable");
        };
        audio.finish_buffer_unlock(buffer_id, lock_index);

        let wrapped = audio
            .lock_buffer(RuntimeComObjectId(22), 88_196, 8, 0, None)
            .unwrap()
            .unwrap();
        assert_eq!(
            wrapped.first_segment.guest_address,
            GuestAddress(0x4501_5884)
        );
        assert_eq!(wrapped.first_segment.byte_length, 4);
        assert_eq!(
            wrapped.second_segment,
            Some(RuntimeAudioBufferSegment {
                guest_address: GuestAddress(0x4500_0000),
                byte_length: 4,
            })
        );
        let unlock = audio
            .select_buffer_unlock(
                RuntimeComObjectId(22),
                wrapped.first_segment,
                wrapped.second_segment,
            )
            .unwrap();
        let Some((buffer_id, lock_index, _)) = unlock else {
            panic!("wrapped Lock must be unlockable");
        };
        audio.finish_buffer_unlock(buffer_id, lock_index);

        let before_invalid = audio.clone();
        audio
            .preview_buffer_lock(RuntimeComObjectId(22), 2, 4, 0, None)
            .unwrap_err();
        assert_eq!(audio, before_invalid);
    }

    #[test]
    fn invalid_and_duplicate_buffers_fail_without_mutation() {
        let valid = priority_audio();
        let before = valid.clone();
        for parent in [RuntimeComObjectId(0), RuntimeComObjectId(999)] {
            assert!(
                valid
                    .validate_buffer_creation(parent, primary_descriptor())
                    .is_err()
            );
            assert_eq!(valid, before);
        }
        let mut no_priority = RuntimeAudio::default();
        no_priority.create_device(RuntimeComObjectId(13)).unwrap();
        let before = no_priority.clone();
        no_priority
            .validate_buffer_creation(RuntimeComObjectId(13), primary_descriptor())
            .unwrap_err();
        assert_eq!(no_priority, before);

        let mut audio = priority_audio();
        let before = audio.clone();
        audio
            .validate_buffer_creation(
                RuntimeComObjectId(13),
                RuntimeAudioBufferDescriptor {
                    flags: 0,
                    ..primary_descriptor()
                },
            )
            .unwrap_err();
        assert_eq!(audio, before);

        let primary = audio
            .validate_buffer_creation(RuntimeComObjectId(13), primary_descriptor())
            .unwrap();
        audio.insert_buffer(primary, RuntimeComObjectId(21));
        let before = audio.clone();
        audio
            .validate_buffer_creation(RuntimeComObjectId(13), primary_descriptor())
            .unwrap_err();
        assert_eq!(audio, before);
    }
}

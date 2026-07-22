use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::{RuntimeComObjectId, RuntimeStateError, RuntimeThreadId, RuntimeWindowId};

pub const RUNTIME_AUDIO_DEVICE_ID_NAMESPACE: &str = "vine.logical-audio-device.v1";
pub const RUNTIME_AUDIO_DEVICE_LIMIT: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeAudioDeviceId(pub u64);

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
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeAudioDeviceCreateRequest {
    pub frontend: RuntimeAudioFrontend,
    pub selector: RuntimeAudioDeviceSelector,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeAudioComRetirementPlan {
    expected_generation: u64,
    next_generation: u64,
    associations: Vec<(RuntimeComObjectId, RuntimeAudioDeviceId)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeAudio {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    devices: BTreeMap<RuntimeAudioDeviceId, RuntimeAudioDevice>,
    #[serde(skip)]
    device_com_object_index: BTreeMap<RuntimeComObjectId, RuntimeAudioDeviceId>,
    next_device_id: u64,
    generation: u64,
}

impl Default for RuntimeAudio {
    fn default() -> Self {
        Self {
            devices: BTreeMap::new(),
            device_com_object_index: BTreeMap::new(),
            next_device_id: 1,
            generation: 0,
        }
    }
}

impl RuntimeAudio {
    pub(super) fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }

    pub(super) fn devices(&self) -> &BTreeMap<RuntimeAudioDeviceId, RuntimeAudioDevice> {
        &self.devices
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
            generation: plan.next_generation,
        };
        self.device_com_object_index
            .insert(com_object_id, device.id);
        self.devices.insert(device.id, device.clone());
        self.next_device_id = next_device_id;
        self.generation = plan.next_generation;
        device
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
        let associations = com_object_ids
            .iter()
            .filter_map(|com_object_id| {
                self.device_com_object_index
                    .get(com_object_id)
                    .copied()
                    .map(|device_id| (*com_object_id, device_id))
            })
            .collect::<Vec<_>>();
        if associations.is_empty() {
            return Ok(None);
        }
        self.validate_retirement_associations(&associations)?;
        Ok(Some(RuntimeAudioComRetirementPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            associations,
        }))
    }

    pub(crate) fn preflight_com_object_retirement(
        &self,
        plan: &RuntimeAudioComRetirementPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self
                .validate_retirement_associations(&plan.associations)
                .is_err()
        {
            return Err(stale_plan("audio-com-retirement-plan-stale"));
        }
        Ok(())
    }

    pub(crate) fn commit_com_object_retirement(&mut self, plan: RuntimeAudioComRetirementPlan) {
        debug_assert!(self.preflight_com_object_retirement(&plan).is_ok());
        for (com_object_id, device_id) in plan.associations {
            self.device_com_object_index.remove(&com_object_id);
            self.devices
                .remove(&device_id)
                .expect("preflighted logical audio device disappeared");
        }
        self.generation = plan.next_generation;
    }

    fn validate_retirement_associations(
        &self,
        associations: &[(RuntimeComObjectId, RuntimeAudioDeviceId)],
    ) -> Result<(), RuntimeStateError> {
        for (com_object_id, device_id) in associations {
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
}

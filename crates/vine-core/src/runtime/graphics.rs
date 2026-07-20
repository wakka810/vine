use std::collections::BTreeMap;

use serde::Serialize;

use super::{RuntimeComObjectId, RuntimeStateError, RuntimeThreadId, RuntimeWindowId};

pub const RUNTIME_GRAPHICS_DEVICE_ID_NAMESPACE: &str = "vine.logical-graphics-device.v1";
pub const RUNTIME_BROWSER_RENDERER_ID_NAMESPACE: &str = "vine.browser-renderer-resource.v1";
pub const RUNTIME_GRAPHICS_DEVICE_LIMIT: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeGraphicsDeviceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeBrowserRendererResourceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeGraphicsFrontend {
    D3d9,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeGraphicsColorFormat {
    B8g8r8x8Unorm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeGraphicsPresentMode {
    Discard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsPresentation {
    pub width: u32,
    pub height: u32,
    pub color_format: RuntimeGraphicsColorFormat,
    pub back_buffer_count: u32,
    pub sample_count: u32,
    pub present_mode: RuntimeGraphicsPresentMode,
    pub windowed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeGraphicsRendererOwnership {
    NonOwning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsRendererAssociation {
    pub namespace: String,
    pub resource_id: RuntimeBrowserRendererResourceId,
    pub ownership: RuntimeGraphicsRendererOwnership,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsDevice {
    pub id: RuntimeGraphicsDeviceId,
    pub namespace: String,
    pub frontend: RuntimeGraphicsFrontend,
    pub window_id: RuntimeWindowId,
    pub window_handle: u32,
    pub owner_thread_id: RuntimeThreadId,
    pub presentation: RuntimeGraphicsPresentation,
    pub com_object_id: RuntimeComObjectId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub renderer: Option<RuntimeGraphicsRendererAssociation>,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeGraphicsWindowTarget {
    pub window_id: RuntimeWindowId,
    pub window_handle: u32,
    pub owner_thread_id: RuntimeThreadId,
    pub client_width: u32,
    pub client_height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsDeviceCreateRequest {
    pub frontend: RuntimeGraphicsFrontend,
    pub window_handle: u32,
    pub color_format: RuntimeGraphicsColorFormat,
    pub back_buffer_count: u32,
    pub sample_count: u32,
    pub present_mode: RuntimeGraphicsPresentMode,
    pub windowed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeGraphicsDeviceCreationRequest {
    pub frontend: RuntimeGraphicsFrontend,
    pub target: RuntimeGraphicsWindowTarget,
    pub color_format: RuntimeGraphicsColorFormat,
    pub back_buffer_count: u32,
    pub sample_count: u32,
    pub present_mode: RuntimeGraphicsPresentMode,
    pub windowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsDeviceCreationPlan {
    expected_generation: u64,
    next_generation: u64,
    id: RuntimeGraphicsDeviceId,
    request: RuntimeGraphicsDeviceCreationRequest,
}

impl RuntimeGraphicsDeviceCreationPlan {
    pub(crate) fn id(&self) -> RuntimeGraphicsDeviceId {
        self.id
    }

    pub(crate) fn presentation(&self) -> RuntimeGraphicsPresentation {
        RuntimeGraphicsPresentation {
            width: self.request.target.client_width,
            height: self.request.target.client_height,
            color_format: self.request.color_format,
            back_buffer_count: self.request.back_buffer_count,
            sample_count: self.request.sample_count,
            present_mode: self.request.present_mode,
            windowed: self.request.windowed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsDeviceRetirementPlan {
    expected_generation: u64,
    next_generation: u64,
    associations: Vec<(RuntimeComObjectId, RuntimeGraphicsDeviceId)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeGraphicsTeardownPlan {
    expected_generation: u64,
    next_generation: u64,
    devices: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeGraphics {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    devices: BTreeMap<RuntimeGraphicsDeviceId, RuntimeGraphicsDevice>,
    #[serde(skip)]
    com_object_index: BTreeMap<RuntimeComObjectId, RuntimeGraphicsDeviceId>,
    next_device_id: u64,
    generation: u64,
}

impl Default for RuntimeGraphics {
    fn default() -> Self {
        Self {
            devices: BTreeMap::new(),
            com_object_index: BTreeMap::new(),
            next_device_id: 1,
            generation: 0,
        }
    }
}

impl RuntimeGraphics {
    pub(super) fn plan_device_creation(
        &self,
        request: RuntimeGraphicsDeviceCreationRequest,
    ) -> Result<RuntimeGraphicsDeviceCreationPlan, RuntimeStateError> {
        validate_creation_request(request)?;
        if self.devices.len() >= RUNTIME_GRAPHICS_DEVICE_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-graphics",
                "graphics-device-capacity-reached",
                "logical graphics device capacity is exhausted",
            ));
        }
        let id = RuntimeGraphicsDeviceId(self.next_device_id);
        if id.0 == 0 || self.devices.contains_key(&id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-device-id-collision",
                "logical graphics device allocator selected a live identity",
            ));
        }
        Ok(RuntimeGraphicsDeviceCreationPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            id,
            request,
        })
    }

    pub(super) fn preflight_device_creation(
        &self,
        plan: &RuntimeGraphicsDeviceCreationPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.next_device_id != plan.id.0
            || self.devices.len() >= RUNTIME_GRAPHICS_DEVICE_LIMIT
            || self.devices.contains_key(&plan.id)
        {
            return Err(stale_plan("graphics-device-creation-plan-stale"));
        }
        validate_creation_request(plan.request)
    }

    pub(super) fn commit_device_creation(
        &mut self,
        plan: RuntimeGraphicsDeviceCreationPlan,
        com_object_id: RuntimeComObjectId,
    ) -> RuntimeGraphicsDevice {
        debug_assert!(self.preflight_device_creation(&plan).is_ok());
        debug_assert_ne!(com_object_id.0, 0);
        debug_assert!(!self.com_object_index.contains_key(&com_object_id));
        let next_device_id = plan
            .id
            .0
            .checked_add(1)
            .expect("preflighted graphics device ID must advance");
        let presentation = plan.presentation();
        let device = RuntimeGraphicsDevice {
            id: plan.id,
            namespace: RUNTIME_GRAPHICS_DEVICE_ID_NAMESPACE.to_owned(),
            frontend: plan.request.frontend,
            window_id: plan.request.target.window_id,
            window_handle: plan.request.target.window_handle,
            owner_thread_id: plan.request.target.owner_thread_id,
            presentation,
            com_object_id,
            renderer: None,
            generation: plan.next_generation,
        };
        self.com_object_index.insert(com_object_id, device.id);
        self.devices.insert(device.id, device.clone());
        self.next_device_id = next_device_id;
        self.generation = plan.next_generation;
        device
    }

    pub(crate) fn plan_com_object_retirements(
        &self,
        com_object_ids: &std::collections::BTreeSet<RuntimeComObjectId>,
    ) -> Result<Option<RuntimeGraphicsDeviceRetirementPlan>, RuntimeStateError> {
        let associations = com_object_ids
            .iter()
            .filter_map(|com_object_id| {
                self.com_object_index
                    .get(com_object_id)
                    .copied()
                    .map(|device_id| (*com_object_id, device_id))
            })
            .collect::<Vec<_>>();
        if associations.is_empty() {
            return Ok(None);
        }
        self.validate_retirement_associations(&associations)?;
        Ok(Some(RuntimeGraphicsDeviceRetirementPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            associations,
        }))
    }

    pub(crate) fn preflight_com_object_retirement(
        &self,
        plan: &RuntimeGraphicsDeviceRetirementPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self
                .validate_retirement_associations(&plan.associations)
                .is_err()
        {
            return Err(stale_plan("graphics-device-retirement-plan-stale"));
        }
        Ok(())
    }

    pub(crate) fn commit_com_object_retirement(
        &mut self,
        plan: RuntimeGraphicsDeviceRetirementPlan,
    ) {
        debug_assert!(self.preflight_com_object_retirement(&plan).is_ok());
        for (com_object_id, device_id) in plan.associations {
            self.com_object_index.remove(&com_object_id);
            self.devices.remove(&device_id);
        }
        self.generation = plan.next_generation;
    }

    fn validate_retirement_associations(
        &self,
        associations: &[(RuntimeComObjectId, RuntimeGraphicsDeviceId)],
    ) -> Result<(), RuntimeStateError> {
        if associations.is_empty() {
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-retirement-set-empty",
                "logical graphics retirement requires at least one COM association",
            ));
        }
        for (com_object_id, device_id) in associations {
            if self.com_object_index.get(com_object_id) != Some(device_id) {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-graphics",
                    "graphics-com-index-dangling",
                    "graphics COM association does not resolve to a live logical device",
                ));
            }
            if self
                .devices
                .get(device_id)
                .map(|device| device.com_object_id)
                != Some(*com_object_id)
            {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-graphics",
                    "graphics-com-association-mismatch",
                    "logical graphics device and COM index disagree",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn plan_process_teardown(
        &self,
    ) -> Result<Option<RuntimeGraphicsTeardownPlan>, RuntimeStateError> {
        if self.is_empty() {
            return Ok(None);
        }
        Ok(Some(RuntimeGraphicsTeardownPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            devices: self.devices.len(),
        }))
    }

    pub(super) fn preflight_process_teardown(
        &self,
        plan: &RuntimeGraphicsTeardownPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.devices.len() != plan.devices
            || self.com_object_index.len() != plan.devices
        {
            return Err(stale_plan("graphics-teardown-plan-stale"));
        }
        Ok(())
    }

    pub(super) fn commit_process_teardown(&mut self, plan: RuntimeGraphicsTeardownPlan) {
        debug_assert!(self.preflight_process_teardown(&plan).is_ok());
        self.devices.clear();
        self.com_object_index.clear();
        self.generation = plan.next_generation;
    }

    pub fn device(
        &self,
        id: RuntimeGraphicsDeviceId,
    ) -> Result<&RuntimeGraphicsDevice, RuntimeStateError> {
        self.devices.get(&id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-graphics",
                "graphics-device-id-unknown",
                format!("logical graphics device {} is not live", id.0),
            )
        })
    }

    pub fn devices(&self) -> &BTreeMap<RuntimeGraphicsDeviceId, RuntimeGraphicsDevice> {
        &self.devices
    }

    pub(super) fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }
}

fn validate_creation_request(
    request: RuntimeGraphicsDeviceCreationRequest,
) -> Result<(), RuntimeStateError> {
    if request.target.window_id.0 == 0
        || request.target.window_handle == 0
        || request.target.owner_thread_id.0 == 0
        || request.target.client_width == 0
        || request.target.client_height == 0
        || request.back_buffer_count == 0
        || request.sample_count == 0
        || !request.windowed
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-graphics",
            "graphics-device-creation-request-invalid",
            "logical graphics device creation requires a live window and bounded presentation",
        ));
    }
    Ok(())
}

fn checked_next_generation(generation: u64) -> Result<u64, RuntimeStateError> {
    generation.checked_add(1).ok_or_else(|| {
        RuntimeStateError::resource_limit(
            "runtime-graphics",
            "graphics-generation-exhausted",
            "logical graphics generation space is exhausted",
        )
    })
}

fn stale_plan(code: &'static str) -> RuntimeStateError {
    RuntimeStateError::conflict(
        "runtime-graphics",
        code,
        "logical graphics state changed after mutation planning",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> RuntimeGraphicsDeviceCreationRequest {
        RuntimeGraphicsDeviceCreationRequest {
            frontend: RuntimeGraphicsFrontend::D3d9,
            target: RuntimeGraphicsWindowTarget {
                window_id: RuntimeWindowId(1),
                window_handle: 0x4801_0001,
                owner_thread_id: RuntimeThreadId(0x2000),
                client_width: 800,
                client_height: 600,
            },
            color_format: RuntimeGraphicsColorFormat::B8g8r8x8Unorm,
            back_buffer_count: 1,
            sample_count: 1,
            present_mode: RuntimeGraphicsPresentMode::Discard,
            windowed: true,
        }
    }

    #[test]
    fn creation_retirement_and_stale_plans_preserve_owner_invariants() {
        let mut graphics = RuntimeGraphics::default();
        let plan = graphics.plan_device_creation(request()).unwrap();
        let stale = plan.clone();
        assert_eq!(plan.id(), RuntimeGraphicsDeviceId(1));
        assert_eq!(plan.presentation().width, 800);
        graphics.preflight_device_creation(&plan).unwrap();
        let device = graphics.commit_device_creation(plan, RuntimeComObjectId(7));
        assert_eq!(device.namespace, RUNTIME_GRAPHICS_DEVICE_ID_NAMESPACE);
        assert_eq!(device.com_object_id, RuntimeComObjectId(7));
        assert_eq!(device.renderer, None);
        assert_eq!(
            graphics.preflight_device_creation(&stale).unwrap_err().code,
            "graphics-device-creation-plan-stale"
        );

        let retirement = graphics
            .plan_com_object_retirements(&[RuntimeComObjectId(7)].into_iter().collect())
            .unwrap()
            .unwrap();
        let stale_retirement = retirement.clone();
        graphics
            .preflight_com_object_retirement(&retirement)
            .unwrap();
        graphics.commit_com_object_retirement(retirement);
        assert!(graphics.is_empty());
        assert_eq!(
            graphics
                .preflight_com_object_retirement(&stale_retirement)
                .unwrap_err()
                .code,
            "graphics-device-retirement-plan-stale"
        );
    }

    #[test]
    fn process_teardown_is_planned_and_rejects_stale_state() {
        let mut graphics = RuntimeGraphics::default();
        let first = graphics.plan_device_creation(request()).unwrap();
        graphics.commit_device_creation(first, RuntimeComObjectId(11));
        let teardown = graphics.plan_process_teardown().unwrap().unwrap();
        let stale = teardown.clone();
        let second = graphics.plan_device_creation(request()).unwrap();
        graphics.commit_device_creation(second, RuntimeComObjectId(12));
        assert_eq!(
            graphics
                .preflight_process_teardown(&stale)
                .unwrap_err()
                .code,
            "graphics-teardown-plan-stale"
        );
        let teardown = graphics.plan_process_teardown().unwrap().unwrap();
        graphics.preflight_process_teardown(&teardown).unwrap();
        graphics.commit_process_teardown(teardown);
        assert!(graphics.is_empty());
    }

    #[test]
    fn device_capacity_failure_preserves_allocator_and_generation() {
        let mut graphics = RuntimeGraphics::default();
        for index in 0..RUNTIME_GRAPHICS_DEVICE_LIMIT {
            let plan = graphics.plan_device_creation(request()).unwrap();
            graphics.commit_device_creation(plan, RuntimeComObjectId(index as u64 + 1));
        }
        let before = graphics.clone();
        let error = graphics.plan_device_creation(request()).unwrap_err();
        assert_eq!(error.code, "graphics-device-capacity-reached");
        assert_eq!(graphics, before);
    }
}

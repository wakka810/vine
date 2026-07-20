use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::{RuntimeComObjectId, RuntimeStateError, RuntimeThreadId, RuntimeWindowId};

pub const RUNTIME_GRAPHICS_DEVICE_ID_NAMESPACE: &str = "vine.logical-graphics-device.v1";
pub const RUNTIME_GRAPHICS_TEXTURE_ID_NAMESPACE: &str = "vine.logical-graphics-texture.v1";
pub const RUNTIME_GRAPHICS_SURFACE_ID_NAMESPACE: &str = "vine.logical-graphics-surface.v1";
pub const RUNTIME_BROWSER_RENDERER_ID_NAMESPACE: &str = "vine.browser-renderer-resource.v1";
pub const RUNTIME_GRAPHICS_DEVICE_LIMIT: usize = 4_096;
pub const RUNTIME_GRAPHICS_TEXTURE_LIMIT: usize = 4_096;
pub const RUNTIME_GRAPHICS_SURFACE_LIMIT: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeGraphicsDeviceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeGraphicsTextureId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeGraphicsSurfaceId(pub u64);

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
pub enum RuntimeGraphicsTextureUsage {
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeGraphicsTexturePool {
    SystemMemory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsTextureDescriptor {
    pub width: u32,
    pub height: u32,
    pub levels: u32,
    pub usage: RuntimeGraphicsTextureUsage,
    pub color_format: RuntimeGraphicsColorFormat,
    pub pool: RuntimeGraphicsTexturePool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsSurfaceDescriptor {
    pub level: u32,
    pub width: u32,
    pub height: u32,
    pub color_format: RuntimeGraphicsColorFormat,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsTexture {
    pub id: RuntimeGraphicsTextureId,
    pub namespace: String,
    pub device_id: RuntimeGraphicsDeviceId,
    pub descriptor: RuntimeGraphicsTextureDescriptor,
    pub com_object_id: RuntimeComObjectId,
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsSurface {
    pub id: RuntimeGraphicsSurfaceId,
    pub namespace: String,
    pub texture_id: RuntimeGraphicsTextureId,
    pub descriptor: RuntimeGraphicsSurfaceDescriptor,
    pub com_object_id: RuntimeComObjectId,
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
pub(crate) struct RuntimeGraphicsTextureCreateRequest {
    pub device_com_object_id: RuntimeComObjectId,
    pub descriptor: RuntimeGraphicsTextureDescriptor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsSurfaceCreateRequest {
    pub texture_com_object_id: RuntimeComObjectId,
    pub level: u32,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsTextureCreationPlan {
    expected_generation: u64,
    next_generation: u64,
    id: RuntimeGraphicsTextureId,
    device_id: RuntimeGraphicsDeviceId,
    request: RuntimeGraphicsTextureCreateRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsSurfaceCreationPlan {
    expected_generation: u64,
    next_generation: u64,
    id: RuntimeGraphicsSurfaceId,
    texture_id: RuntimeGraphicsTextureId,
    request: RuntimeGraphicsSurfaceCreateRequest,
    descriptor: RuntimeGraphicsSurfaceDescriptor,
}

impl RuntimeGraphicsSurfaceCreationPlan {
    pub(crate) fn id(&self) -> RuntimeGraphicsSurfaceId {
        self.id
    }

    pub(crate) fn texture_id(&self) -> RuntimeGraphicsTextureId {
        self.texture_id
    }

    pub(crate) fn descriptor(&self) -> RuntimeGraphicsSurfaceDescriptor {
        self.descriptor
    }
}

impl RuntimeGraphicsTextureCreationPlan {
    pub(crate) fn id(&self) -> RuntimeGraphicsTextureId {
        self.id
    }

    pub(crate) fn device_id(&self) -> RuntimeGraphicsDeviceId {
        self.device_id
    }

    pub(crate) fn descriptor(&self) -> RuntimeGraphicsTextureDescriptor {
        self.request.descriptor
    }
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
pub(crate) struct RuntimeGraphicsComRetirementPlan {
    expected_generation: u64,
    next_generation: u64,
    device_associations: Vec<(RuntimeComObjectId, RuntimeGraphicsDeviceId)>,
    texture_associations: Vec<(RuntimeComObjectId, RuntimeGraphicsTextureId)>,
    surface_associations: Vec<(RuntimeComObjectId, RuntimeGraphicsSurfaceId)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeGraphicsTeardownPlan {
    expected_generation: u64,
    next_generation: u64,
    devices: usize,
    textures: usize,
    surfaces: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeGraphics {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    devices: BTreeMap<RuntimeGraphicsDeviceId, RuntimeGraphicsDevice>,
    #[serde(skip)]
    device_com_object_index: BTreeMap<RuntimeComObjectId, RuntimeGraphicsDeviceId>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    textures: BTreeMap<RuntimeGraphicsTextureId, RuntimeGraphicsTexture>,
    #[serde(skip)]
    texture_com_object_index: BTreeMap<RuntimeComObjectId, RuntimeGraphicsTextureId>,
    #[serde(skip)]
    device_texture_index: BTreeMap<RuntimeGraphicsDeviceId, BTreeSet<RuntimeGraphicsTextureId>>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    surfaces: BTreeMap<RuntimeGraphicsSurfaceId, RuntimeGraphicsSurface>,
    #[serde(skip)]
    surface_com_object_index: BTreeMap<RuntimeComObjectId, RuntimeGraphicsSurfaceId>,
    #[serde(skip)]
    texture_surface_index: BTreeMap<RuntimeGraphicsTextureId, BTreeSet<RuntimeGraphicsSurfaceId>>,
    #[serde(skip)]
    texture_level_surface_index:
        BTreeMap<(RuntimeGraphicsTextureId, u32), RuntimeGraphicsSurfaceId>,
    next_device_id: u64,
    next_texture_id: u64,
    next_surface_id: u64,
    generation: u64,
}

impl Default for RuntimeGraphics {
    fn default() -> Self {
        Self {
            devices: BTreeMap::new(),
            device_com_object_index: BTreeMap::new(),
            textures: BTreeMap::new(),
            texture_com_object_index: BTreeMap::new(),
            device_texture_index: BTreeMap::new(),
            surfaces: BTreeMap::new(),
            surface_com_object_index: BTreeMap::new(),
            texture_surface_index: BTreeMap::new(),
            texture_level_surface_index: BTreeMap::new(),
            next_device_id: 1,
            next_texture_id: 1,
            next_surface_id: 1,
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
        debug_assert!(!self.device_com_object_index.contains_key(&com_object_id));
        debug_assert!(!self.texture_com_object_index.contains_key(&com_object_id));
        debug_assert!(!self.surface_com_object_index.contains_key(&com_object_id));
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
        self.device_com_object_index
            .insert(com_object_id, device.id);
        self.device_texture_index.insert(device.id, BTreeSet::new());
        self.devices.insert(device.id, device.clone());
        self.next_device_id = next_device_id;
        self.generation = plan.next_generation;
        device
    }

    pub(super) fn plan_texture_creation(
        &self,
        request: RuntimeGraphicsTextureCreateRequest,
    ) -> Result<RuntimeGraphicsTextureCreationPlan, RuntimeStateError> {
        validate_texture_creation_request(request)?;
        let device_id = self
            .device_com_object_index
            .get(&request.device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-texture-device-com-object-unknown",
                    "logical texture creation requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(request.device_com_object_id, device_id)?;
        if self.textures.len() >= RUNTIME_GRAPHICS_TEXTURE_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-graphics",
                "graphics-texture-capacity-reached",
                "logical graphics texture capacity is exhausted",
            ));
        }
        let id = RuntimeGraphicsTextureId(self.next_texture_id);
        if id.0 == 0 || self.textures.contains_key(&id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-texture-id-collision",
                "logical graphics texture allocator selected a live identity",
            ));
        }
        Ok(RuntimeGraphicsTextureCreationPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            id,
            device_id,
            request,
        })
    }

    pub(super) fn preflight_texture_creation(
        &self,
        plan: &RuntimeGraphicsTextureCreationPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.next_texture_id != plan.id.0
            || self.textures.len() >= RUNTIME_GRAPHICS_TEXTURE_LIMIT
            || self.textures.contains_key(&plan.id)
            || self
                .device_com_object_index
                .get(&plan.request.device_com_object_id)
                != Some(&plan.device_id)
        {
            return Err(stale_plan("graphics-texture-creation-plan-stale"));
        }
        validate_texture_creation_request(plan.request)?;
        self.validate_device_association(plan.request.device_com_object_id, plan.device_id)
    }

    pub(super) fn commit_texture_creation(
        &mut self,
        plan: RuntimeGraphicsTextureCreationPlan,
        com_object_id: RuntimeComObjectId,
    ) -> RuntimeGraphicsTexture {
        debug_assert!(self.preflight_texture_creation(&plan).is_ok());
        debug_assert_ne!(com_object_id.0, 0);
        debug_assert!(!self.device_com_object_index.contains_key(&com_object_id));
        debug_assert!(!self.texture_com_object_index.contains_key(&com_object_id));
        debug_assert!(!self.surface_com_object_index.contains_key(&com_object_id));
        let next_texture_id = plan
            .id
            .0
            .checked_add(1)
            .expect("preflighted graphics texture ID must advance");
        let texture = RuntimeGraphicsTexture {
            id: plan.id,
            namespace: RUNTIME_GRAPHICS_TEXTURE_ID_NAMESPACE.to_owned(),
            device_id: plan.device_id,
            descriptor: plan.request.descriptor,
            com_object_id,
            generation: plan.next_generation,
        };
        self.texture_com_object_index
            .insert(com_object_id, texture.id);
        self.device_texture_index
            .get_mut(&texture.device_id)
            .expect("preflighted graphics texture device disappeared")
            .insert(texture.id);
        self.texture_surface_index
            .insert(texture.id, BTreeSet::new());
        self.textures.insert(texture.id, texture.clone());
        self.next_texture_id = next_texture_id;
        self.generation = plan.next_generation;
        texture
    }

    pub(super) fn plan_surface_creation(
        &self,
        request: RuntimeGraphicsSurfaceCreateRequest,
    ) -> Result<RuntimeGraphicsSurfaceCreationPlan, RuntimeStateError> {
        validate_surface_creation_request(request)?;
        let texture_id = self
            .texture_com_object_index
            .get(&request.texture_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-surface-texture-com-object-unknown",
                    "logical surface creation requires a live graphics texture COM object",
                )
            })?;
        self.validate_texture_association(request.texture_com_object_id, texture_id)?;
        let texture = self
            .textures
            .get(&texture_id)
            .expect("validated logical texture association disappeared");
        if request.level >= texture.descriptor.levels {
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-surface-level-out-of-range",
                "logical surface level is outside the texture descriptor",
            ));
        }
        if self
            .texture_level_surface_index
            .contains_key(&(texture_id, request.level))
        {
            return Err(RuntimeStateError::conflict(
                "runtime-graphics",
                "graphics-surface-level-already-published",
                "logical texture level already has a live surface identity",
            ));
        }
        if self.surfaces.len() >= RUNTIME_GRAPHICS_SURFACE_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-graphics",
                "graphics-surface-capacity-reached",
                "logical graphics surface capacity is exhausted",
            ));
        }
        let id = RuntimeGraphicsSurfaceId(self.next_surface_id);
        if id.0 == 0 || self.surfaces.contains_key(&id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-surface-id-collision",
                "logical graphics surface allocator selected a live identity",
            ));
        }
        let descriptor = RuntimeGraphicsSurfaceDescriptor {
            level: request.level,
            width: mip_dimension(texture.descriptor.width, request.level),
            height: mip_dimension(texture.descriptor.height, request.level),
            color_format: texture.descriptor.color_format,
        };
        Ok(RuntimeGraphicsSurfaceCreationPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            id,
            texture_id,
            request,
            descriptor,
        })
    }

    pub(super) fn preflight_surface_creation(
        &self,
        plan: &RuntimeGraphicsSurfaceCreationPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.next_surface_id != plan.id.0
            || self.surfaces.len() >= RUNTIME_GRAPHICS_SURFACE_LIMIT
            || self.surfaces.contains_key(&plan.id)
            || self
                .texture_com_object_index
                .get(&plan.request.texture_com_object_id)
                != Some(&plan.texture_id)
            || self
                .texture_level_surface_index
                .contains_key(&(plan.texture_id, plan.request.level))
        {
            return Err(stale_plan("graphics-surface-creation-plan-stale"));
        }
        validate_surface_creation_request(plan.request)?;
        self.validate_texture_association(plan.request.texture_com_object_id, plan.texture_id)?;
        let texture = self
            .textures
            .get(&plan.texture_id)
            .expect("validated logical texture association disappeared");
        if plan.request.level >= texture.descriptor.levels
            || plan.descriptor
                != (RuntimeGraphicsSurfaceDescriptor {
                    level: plan.request.level,
                    width: mip_dimension(texture.descriptor.width, plan.request.level),
                    height: mip_dimension(texture.descriptor.height, plan.request.level),
                    color_format: texture.descriptor.color_format,
                })
        {
            return Err(stale_plan("graphics-surface-creation-plan-stale"));
        }
        Ok(())
    }

    pub(super) fn commit_surface_creation(
        &mut self,
        plan: RuntimeGraphicsSurfaceCreationPlan,
        com_object_id: RuntimeComObjectId,
    ) -> RuntimeGraphicsSurface {
        debug_assert!(self.preflight_surface_creation(&plan).is_ok());
        debug_assert_ne!(com_object_id.0, 0);
        debug_assert!(!self.device_com_object_index.contains_key(&com_object_id));
        debug_assert!(!self.texture_com_object_index.contains_key(&com_object_id));
        debug_assert!(!self.surface_com_object_index.contains_key(&com_object_id));
        let next_surface_id = plan
            .id
            .0
            .checked_add(1)
            .expect("preflighted graphics surface ID must advance");
        let surface = RuntimeGraphicsSurface {
            id: plan.id,
            namespace: RUNTIME_GRAPHICS_SURFACE_ID_NAMESPACE.to_owned(),
            texture_id: plan.texture_id,
            descriptor: plan.descriptor,
            com_object_id,
            generation: plan.next_generation,
        };
        self.surface_com_object_index
            .insert(com_object_id, surface.id);
        self.texture_surface_index
            .get_mut(&surface.texture_id)
            .expect("preflighted graphics surface texture disappeared")
            .insert(surface.id);
        self.texture_level_surface_index
            .insert((surface.texture_id, surface.descriptor.level), surface.id);
        self.surfaces.insert(surface.id, surface.clone());
        self.next_surface_id = next_surface_id;
        self.generation = plan.next_generation;
        surface
    }

    pub(crate) fn plan_com_object_retirements(
        &self,
        com_object_ids: &BTreeSet<RuntimeComObjectId>,
    ) -> Result<Option<RuntimeGraphicsComRetirementPlan>, RuntimeStateError> {
        let device_associations = com_object_ids
            .iter()
            .filter_map(|com_object_id| {
                self.device_com_object_index
                    .get(com_object_id)
                    .copied()
                    .map(|device_id| (*com_object_id, device_id))
            })
            .collect::<Vec<_>>();
        let texture_associations = com_object_ids
            .iter()
            .filter_map(|com_object_id| {
                self.texture_com_object_index
                    .get(com_object_id)
                    .copied()
                    .map(|texture_id| (*com_object_id, texture_id))
            })
            .collect::<Vec<_>>();
        let surface_associations = com_object_ids
            .iter()
            .filter_map(|com_object_id| {
                self.surface_com_object_index
                    .get(com_object_id)
                    .copied()
                    .map(|surface_id| (*com_object_id, surface_id))
            })
            .collect::<Vec<_>>();
        if device_associations.is_empty()
            && texture_associations.is_empty()
            && surface_associations.is_empty()
        {
            return Ok(None);
        }
        self.validate_retirement_associations(
            &device_associations,
            &texture_associations,
            &surface_associations,
        )?;
        Ok(Some(RuntimeGraphicsComRetirementPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            device_associations,
            texture_associations,
            surface_associations,
        }))
    }

    pub(crate) fn preflight_com_object_retirement(
        &self,
        plan: &RuntimeGraphicsComRetirementPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self
                .validate_retirement_associations(
                    &plan.device_associations,
                    &plan.texture_associations,
                    &plan.surface_associations,
                )
                .is_err()
        {
            return Err(stale_plan("graphics-com-retirement-plan-stale"));
        }
        Ok(())
    }

    pub(crate) fn commit_com_object_retirement(&mut self, plan: RuntimeGraphicsComRetirementPlan) {
        debug_assert!(self.preflight_com_object_retirement(&plan).is_ok());
        for (com_object_id, surface_id) in plan.surface_associations {
            self.surface_com_object_index.remove(&com_object_id);
            let surface = self
                .surfaces
                .remove(&surface_id)
                .expect("preflighted logical surface disappeared during retirement");
            self.texture_surface_index
                .get_mut(&surface.texture_id)
                .expect("preflighted logical surface texture disappeared during retirement")
                .remove(&surface_id);
            self.texture_level_surface_index
                .remove(&(surface.texture_id, surface.descriptor.level));
        }
        for (com_object_id, texture_id) in plan.texture_associations {
            self.texture_com_object_index.remove(&com_object_id);
            let texture = self
                .textures
                .remove(&texture_id)
                .expect("preflighted logical texture disappeared during retirement");
            self.device_texture_index
                .get_mut(&texture.device_id)
                .expect("preflighted logical texture device disappeared during retirement")
                .remove(&texture_id);
            self.texture_surface_index.remove(&texture_id);
        }
        for (com_object_id, device_id) in plan.device_associations {
            self.device_com_object_index.remove(&com_object_id);
            self.device_texture_index.remove(&device_id);
            self.devices.remove(&device_id);
        }
        self.generation = plan.next_generation;
    }

    fn validate_retirement_associations(
        &self,
        device_associations: &[(RuntimeComObjectId, RuntimeGraphicsDeviceId)],
        texture_associations: &[(RuntimeComObjectId, RuntimeGraphicsTextureId)],
        surface_associations: &[(RuntimeComObjectId, RuntimeGraphicsSurfaceId)],
    ) -> Result<(), RuntimeStateError> {
        if device_associations.is_empty()
            && texture_associations.is_empty()
            && surface_associations.is_empty()
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-retirement-set-empty",
                "logical graphics retirement requires at least one COM association",
            ));
        }
        for (com_object_id, device_id) in device_associations {
            self.validate_device_association(*com_object_id, *device_id)?;
        }
        for (com_object_id, texture_id) in texture_associations {
            self.validate_texture_association(*com_object_id, *texture_id)?;
        }
        for (com_object_id, surface_id) in surface_associations {
            self.validate_surface_association(*com_object_id, *surface_id)?;
        }
        let retiring_surfaces = surface_associations
            .iter()
            .map(|(_, surface_id)| *surface_id)
            .collect::<BTreeSet<_>>();
        for (_, texture_id) in texture_associations {
            let children = self.texture_surface_index.get(texture_id).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-graphics",
                    "graphics-texture-surface-index-missing",
                    "logical graphics texture has no surface relation index",
                )
            })?;
            if !children.is_subset(&retiring_surfaces) {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-graphics",
                    "graphics-texture-retirement-has-live-surfaces",
                    "logical graphics texture cannot retire while a child surface remains live",
                ));
            }
        }
        let retiring_textures = texture_associations
            .iter()
            .map(|(_, texture_id)| *texture_id)
            .collect::<BTreeSet<_>>();
        for (_, device_id) in device_associations {
            let children = self.device_texture_index.get(device_id).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-graphics",
                    "graphics-device-texture-index-missing",
                    "logical graphics device has no texture relation index",
                )
            })?;
            if !children.is_subset(&retiring_textures) {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-graphics",
                    "graphics-device-retirement-has-live-textures",
                    "logical graphics device cannot retire while a child texture remains live",
                ));
            }
        }
        Ok(())
    }

    fn validate_device_association(
        &self,
        com_object_id: RuntimeComObjectId,
        device_id: RuntimeGraphicsDeviceId,
    ) -> Result<(), RuntimeStateError> {
        if self.device_com_object_index.get(&com_object_id) != Some(&device_id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-device-com-index-dangling",
                "graphics device COM association does not resolve to a live logical device",
            ));
        }
        if self
            .devices
            .get(&device_id)
            .map(|device| device.com_object_id)
            != Some(com_object_id)
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-device-com-association-mismatch",
                "logical graphics device and COM index disagree",
            ));
        }
        if !self.device_texture_index.contains_key(&device_id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-device-texture-index-missing",
                "logical graphics device has no texture relation index",
            ));
        }
        Ok(())
    }

    fn validate_texture_association(
        &self,
        com_object_id: RuntimeComObjectId,
        texture_id: RuntimeGraphicsTextureId,
    ) -> Result<(), RuntimeStateError> {
        if self.texture_com_object_index.get(&com_object_id) != Some(&texture_id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-texture-com-index-dangling",
                "graphics texture COM association does not resolve to a live logical texture",
            ));
        }
        let texture = self.textures.get(&texture_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-texture-com-association-mismatch",
                "logical graphics texture and COM index disagree",
            )
        })?;
        if texture.com_object_id != com_object_id
            || !self
                .device_texture_index
                .get(&texture.device_id)
                .is_some_and(|textures| textures.contains(&texture_id))
            || !self.texture_surface_index.contains_key(&texture_id)
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-texture-com-association-mismatch",
                "logical graphics texture, device relation, and COM index disagree",
            ));
        }
        Ok(())
    }

    fn validate_surface_association(
        &self,
        com_object_id: RuntimeComObjectId,
        surface_id: RuntimeGraphicsSurfaceId,
    ) -> Result<(), RuntimeStateError> {
        if self.surface_com_object_index.get(&com_object_id) != Some(&surface_id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-surface-com-index-dangling",
                "graphics surface COM association does not resolve to a live logical surface",
            ));
        }
        let surface = self.surfaces.get(&surface_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-surface-com-association-mismatch",
                "logical graphics surface and COM index disagree",
            )
        })?;
        if surface.com_object_id != com_object_id
            || !self
                .texture_surface_index
                .get(&surface.texture_id)
                .is_some_and(|surfaces| surfaces.contains(&surface_id))
            || self
                .texture_level_surface_index
                .get(&(surface.texture_id, surface.descriptor.level))
                != Some(&surface_id)
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-surface-com-association-mismatch",
                "logical graphics surface, texture relation, and COM indexes disagree",
            ));
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
            textures: self.textures.len(),
            surfaces: self.surfaces.len(),
        }))
    }

    pub(super) fn preflight_process_teardown(
        &self,
        plan: &RuntimeGraphicsTeardownPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.devices.len() != plan.devices
            || self.device_com_object_index.len() != plan.devices
            || self.device_texture_index.len() != plan.devices
            || self.textures.len() != plan.textures
            || self.texture_com_object_index.len() != plan.textures
            || self.texture_surface_index.len() != plan.textures
            || self.surfaces.len() != plan.surfaces
            || self.surface_com_object_index.len() != plan.surfaces
            || self.texture_level_surface_index.len() != plan.surfaces
        {
            return Err(stale_plan("graphics-teardown-plan-stale"));
        }
        Ok(())
    }

    pub(super) fn commit_process_teardown(&mut self, plan: RuntimeGraphicsTeardownPlan) {
        debug_assert!(self.preflight_process_teardown(&plan).is_ok());
        self.surfaces.clear();
        self.surface_com_object_index.clear();
        self.texture_level_surface_index.clear();
        self.texture_surface_index.clear();
        self.textures.clear();
        self.texture_com_object_index.clear();
        self.device_texture_index.clear();
        self.devices.clear();
        self.device_com_object_index.clear();
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

    pub fn texture(
        &self,
        id: RuntimeGraphicsTextureId,
    ) -> Result<&RuntimeGraphicsTexture, RuntimeStateError> {
        self.textures.get(&id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-graphics",
                "graphics-texture-id-unknown",
                format!("logical graphics texture {} is not live", id.0),
            )
        })
    }

    pub fn textures(&self) -> &BTreeMap<RuntimeGraphicsTextureId, RuntimeGraphicsTexture> {
        &self.textures
    }

    pub fn surface(
        &self,
        id: RuntimeGraphicsSurfaceId,
    ) -> Result<&RuntimeGraphicsSurface, RuntimeStateError> {
        self.surfaces.get(&id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-graphics",
                "graphics-surface-id-unknown",
                format!("logical graphics surface {} is not live", id.0),
            )
        })
    }

    pub fn surfaces(&self) -> &BTreeMap<RuntimeGraphicsSurfaceId, RuntimeGraphicsSurface> {
        &self.surfaces
    }

    pub(super) fn surface_for_texture_level(
        &self,
        texture_com_object_id: RuntimeComObjectId,
        level: u32,
    ) -> Result<Option<&RuntimeGraphicsSurface>, RuntimeStateError> {
        let texture_id = self
            .texture_com_object_index
            .get(&texture_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-surface-texture-com-object-unknown",
                    "logical surface lookup requires a live graphics texture COM object",
                )
            })?;
        self.validate_texture_association(texture_com_object_id, texture_id)?;
        let texture = self
            .textures
            .get(&texture_id)
            .expect("validated logical texture association disappeared");
        if level >= texture.descriptor.levels {
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-surface-level-out-of-range",
                "logical surface level is outside the texture descriptor",
            ));
        }
        let Some(surface_id) = self
            .texture_level_surface_index
            .get(&(texture_id, level))
            .copied()
        else {
            return Ok(None);
        };
        let surface = self.surface(surface_id)?;
        self.validate_surface_association(surface.com_object_id, surface.id)?;
        Ok(Some(surface))
    }

    pub(super) fn is_empty(&self) -> bool {
        self.devices.is_empty() && self.textures.is_empty() && self.surfaces.is_empty()
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

fn validate_texture_creation_request(
    request: RuntimeGraphicsTextureCreateRequest,
) -> Result<(), RuntimeStateError> {
    if request.device_com_object_id.0 == 0
        || request.descriptor.width == 0
        || request.descriptor.height == 0
        || request.descriptor.levels == 0
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-graphics",
            "graphics-texture-creation-request-invalid",
            "logical graphics texture creation requires a live device and nonzero descriptor",
        ));
    }
    Ok(())
}

fn validate_surface_creation_request(
    request: RuntimeGraphicsSurfaceCreateRequest,
) -> Result<(), RuntimeStateError> {
    if request.texture_com_object_id.0 == 0 {
        return Err(RuntimeStateError::invalid_input(
            "runtime-graphics",
            "graphics-surface-creation-request-invalid",
            "logical graphics surface creation requires a live texture",
        ));
    }
    Ok(())
}

fn mip_dimension(base: u32, level: u32) -> u32 {
    base.checked_shr(level).unwrap_or(0).max(1)
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

    fn texture_request(com_object_id: RuntimeComObjectId) -> RuntimeGraphicsTextureCreateRequest {
        RuntimeGraphicsTextureCreateRequest {
            device_com_object_id: com_object_id,
            descriptor: RuntimeGraphicsTextureDescriptor {
                width: 1_024,
                height: 1_024,
                levels: 1,
                usage: RuntimeGraphicsTextureUsage::None,
                color_format: RuntimeGraphicsColorFormat::B8g8r8x8Unorm,
                pool: RuntimeGraphicsTexturePool::SystemMemory,
            },
        }
    }

    fn surface_request(
        com_object_id: RuntimeComObjectId,
        level: u32,
    ) -> RuntimeGraphicsSurfaceCreateRequest {
        RuntimeGraphicsSurfaceCreateRequest {
            texture_com_object_id: com_object_id,
            level,
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
            "graphics-com-retirement-plan-stale"
        );
    }

    #[test]
    fn texture_creation_and_cascading_retirement_preserve_device_relation() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics.plan_device_creation(request()).unwrap();
        let device = graphics.commit_device_creation(device_plan, RuntimeComObjectId(7));
        let texture_plan = graphics
            .plan_texture_creation(texture_request(RuntimeComObjectId(7)))
            .unwrap();
        let stale = texture_plan.clone();
        assert_eq!(texture_plan.id(), RuntimeGraphicsTextureId(1));
        assert_eq!(texture_plan.device_id(), device.id);
        assert_eq!(texture_plan.descriptor().width, 1_024);
        let texture = graphics.commit_texture_creation(texture_plan, RuntimeComObjectId(8));
        assert_eq!(texture.namespace, RUNTIME_GRAPHICS_TEXTURE_ID_NAMESPACE);
        assert_eq!(texture.device_id, device.id);
        assert_eq!(texture.com_object_id, RuntimeComObjectId(8));
        assert_eq!(
            graphics
                .preflight_texture_creation(&stale)
                .unwrap_err()
                .code,
            "graphics-texture-creation-plan-stale"
        );

        let surface_plan = graphics
            .plan_surface_creation(surface_request(RuntimeComObjectId(8), 0))
            .unwrap();
        let stale_surface = surface_plan.clone();
        assert_eq!(surface_plan.id(), RuntimeGraphicsSurfaceId(1));
        assert_eq!(surface_plan.texture_id(), texture.id);
        assert_eq!(surface_plan.descriptor().width, 1_024);
        let surface = graphics.commit_surface_creation(surface_plan, RuntimeComObjectId(9));
        assert_eq!(surface.namespace, RUNTIME_GRAPHICS_SURFACE_ID_NAMESPACE);
        assert_eq!(surface.texture_id, texture.id);
        assert_eq!(surface.descriptor.level, 0);
        assert_eq!(surface.com_object_id, RuntimeComObjectId(9));
        assert_eq!(
            graphics
                .preflight_surface_creation(&stale_surface)
                .unwrap_err()
                .code,
            "graphics-surface-creation-plan-stale"
        );
        assert_eq!(
            graphics
                .surface_for_texture_level(RuntimeComObjectId(8), 0)
                .unwrap()
                .map(|surface| surface.id),
            Some(surface.id)
        );

        let incomplete = graphics
            .plan_com_object_retirements(&[RuntimeComObjectId(7)].into_iter().collect())
            .unwrap_err();
        assert_eq!(
            incomplete.code,
            "graphics-device-retirement-has-live-textures"
        );

        let incomplete = graphics
            .plan_com_object_retirements(&[RuntimeComObjectId(8)].into_iter().collect())
            .unwrap_err();
        assert_eq!(
            incomplete.code,
            "graphics-texture-retirement-has-live-surfaces"
        );

        let retirement = graphics
            .plan_com_object_retirements(
                &[
                    RuntimeComObjectId(7),
                    RuntimeComObjectId(8),
                    RuntimeComObjectId(9),
                ]
                .into_iter()
                .collect(),
            )
            .unwrap()
            .unwrap();
        graphics
            .preflight_com_object_retirement(&retirement)
            .unwrap();
        graphics.commit_com_object_retirement(retirement);
        assert!(graphics.is_empty());
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

    #[test]
    fn texture_capacity_failure_preserves_allocator_relations_and_generation() {
        let mut graphics = RuntimeGraphics::default();
        let device = graphics.plan_device_creation(request()).unwrap();
        graphics.commit_device_creation(device, RuntimeComObjectId(1));
        for index in 0..RUNTIME_GRAPHICS_TEXTURE_LIMIT {
            let plan = graphics
                .plan_texture_creation(texture_request(RuntimeComObjectId(1)))
                .unwrap();
            graphics.commit_texture_creation(plan, RuntimeComObjectId(index as u64 + 2));
        }
        let before = graphics.clone();
        let error = graphics
            .plan_texture_creation(texture_request(RuntimeComObjectId(1)))
            .unwrap_err();
        assert_eq!(error.code, "graphics-texture-capacity-reached");
        assert_eq!(graphics, before);
    }

    #[test]
    fn surface_capacity_and_duplicate_level_failures_preserve_owner_state() {
        let mut graphics = RuntimeGraphics::default();
        let device = graphics.plan_device_creation(request()).unwrap();
        graphics.commit_device_creation(device, RuntimeComObjectId(1));
        let mut request = texture_request(RuntimeComObjectId(1));
        request.descriptor.levels = RUNTIME_GRAPHICS_SURFACE_LIMIT as u32 + 1;
        let texture = graphics.plan_texture_creation(request).unwrap();
        graphics.commit_texture_creation(texture, RuntimeComObjectId(2));
        for index in 0..RUNTIME_GRAPHICS_SURFACE_LIMIT {
            let surface = graphics
                .plan_surface_creation(surface_request(RuntimeComObjectId(2), index as u32))
                .unwrap();
            graphics.commit_surface_creation(surface, RuntimeComObjectId(index as u64 + 3));
        }
        let duplicate_before = graphics.clone();
        let duplicate = graphics
            .plan_surface_creation(surface_request(RuntimeComObjectId(2), 0))
            .unwrap_err();
        assert_eq!(duplicate.code, "graphics-surface-level-already-published");
        assert_eq!(graphics, duplicate_before);

        let before = graphics.clone();
        let error = graphics
            .plan_surface_creation(surface_request(
                RuntimeComObjectId(2),
                RUNTIME_GRAPHICS_SURFACE_LIMIT as u32,
            ))
            .unwrap_err();
        assert_eq!(error.code, "graphics-surface-capacity-reached");
        assert_eq!(graphics, before);
    }
}

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::{
    ProcessAddressAllocationId, RuntimeComObjectId, RuntimeRect, RuntimeStateError,
    RuntimeThreadId, RuntimeWindowId,
};
use crate::memory::{GuestAddress, GuestRange, MemoryOwnerId};

pub const RUNTIME_GRAPHICS_DEVICE_ID_NAMESPACE: &str = "vine.logical-graphics-device.v1";
pub const RUNTIME_GRAPHICS_TEXTURE_ID_NAMESPACE: &str = "vine.logical-graphics-texture.v1";
pub const RUNTIME_GRAPHICS_SURFACE_ID_NAMESPACE: &str = "vine.logical-graphics-surface.v1";
pub const RUNTIME_GRAPHICS_VERTEX_BUFFER_ID_NAMESPACE: &str =
    "vine.logical-graphics-vertex-buffer.v1";
pub const RUNTIME_BROWSER_RENDERER_ID_NAMESPACE: &str = "vine.browser-renderer-resource.v1";
pub const RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE: crate::memory::GuestAddress =
    crate::memory::GuestAddress(0x4400_0000);
pub const RUNTIME_GRAPHICS_RESOURCE_ARENA_SIZE: u64 = 0x0100_0000;
pub(crate) const RUNTIME_GRAPHICS_RESOURCE_ADDRESS_OWNER: &str = "runtime-graphics-resource";
pub const RUNTIME_GRAPHICS_DEVICE_LIMIT: usize = 4_096;
pub const RUNTIME_GRAPHICS_TEXTURE_LIMIT: usize = 4_096;
pub const RUNTIME_GRAPHICS_SURFACE_LIMIT: usize = 4_096;
pub const RUNTIME_GRAPHICS_VERTEX_BUFFER_LIMIT: usize = 4_096;
pub const RUNTIME_GRAPHICS_SURFACE_FILL_LIMIT: usize = 16_384;

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
pub struct RuntimeGraphicsVertexBufferId(pub u64);

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
    Dynamic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeGraphicsTexturePool {
    Default,
    SystemMemory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeGraphicsVertexBufferUsage {
    WriteOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeGraphicsVertexBufferPool {
    Managed,
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
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsVertexBufferDescriptor {
    pub byte_length: u32,
    pub usage: RuntimeGraphicsVertexBufferUsage,
    pub flexible_vertex_format: u32,
    pub pool: RuntimeGraphicsVertexBufferPool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsVertexBufferBacking {
    pub address_allocation_id: ProcessAddressAllocationId,
    pub memory_owner: MemoryOwnerId,
    pub range: GuestRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsVertexBufferLock {
    pub offset: u32,
    pub byte_length: u32,
    pub flags: u32,
    pub guest_address: GuestAddress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsSurfaceFill {
    pub sequence: u64,
    pub rect: RuntimeRect,
    pub color_ref: u32,
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
    pub fills: Vec<RuntimeGraphicsSurfaceFill>,
    pub content_generation: u64,
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsVertexBuffer {
    pub id: RuntimeGraphicsVertexBufferId,
    pub namespace: String,
    pub device_id: RuntimeGraphicsDeviceId,
    pub descriptor: RuntimeGraphicsVertexBufferDescriptor,
    pub backing: RuntimeGraphicsVertexBufferBacking,
    pub com_object_id: RuntimeComObjectId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_lock: Option<RuntimeGraphicsVertexBufferLock>,
    pub content_generation: u64,
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
pub(crate) struct RuntimeGraphicsSurfaceFillRequest {
    pub surface_com_object_id: RuntimeComObjectId,
    pub rect: RuntimeRect,
    pub color_ref: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsVertexBufferCreateRequest {
    pub device_com_object_id: RuntimeComObjectId,
    pub descriptor: RuntimeGraphicsVertexBufferDescriptor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsVertexBufferLockRequest {
    pub vertex_buffer_com_object_id: RuntimeComObjectId,
    pub offset: u32,
    pub byte_length: u32,
    pub flags: u32,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsSurfaceFillPlan {
    expected_generation: u64,
    next_generation: u64,
    surface_id: RuntimeGraphicsSurfaceId,
    surface_generation: u64,
    next_surface_generation: u64,
    surface_content_generation: u64,
    next_surface_content_generation: u64,
    fill: Option<RuntimeGraphicsSurfaceFill>,
    next_fill_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsVertexBufferCreationPlan {
    expected_generation: u64,
    next_generation: u64,
    id: RuntimeGraphicsVertexBufferId,
    device_id: RuntimeGraphicsDeviceId,
    request: RuntimeGraphicsVertexBufferCreateRequest,
    address_allocation_id: ProcessAddressAllocationId,
    backing_range: GuestRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsVertexBufferLockPlan {
    expected_generation: u64,
    next_generation: u64,
    vertex_buffer_id: RuntimeGraphicsVertexBufferId,
    vertex_buffer_generation: u64,
    next_vertex_buffer_generation: u64,
    lock: RuntimeGraphicsVertexBufferLock,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsVertexBufferUnlockPlan {
    expected_generation: u64,
    next_generation: u64,
    vertex_buffer_id: RuntimeGraphicsVertexBufferId,
    vertex_buffer_generation: u64,
    next_vertex_buffer_generation: u64,
    content_generation: u64,
    next_content_generation: u64,
    lock: RuntimeGraphicsVertexBufferLock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsVertexBufferUnlockResult {
    pub vertex_buffer_id: RuntimeGraphicsVertexBufferId,
    pub generation: u64,
    pub content_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsSurfaceFillResult {
    pub surface_id: RuntimeGraphicsSurfaceId,
    pub surface_generation: u64,
    pub content_generation: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<RuntimeGraphicsSurfaceFill>,
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

impl RuntimeGraphicsVertexBufferCreationPlan {
    pub(crate) fn descriptor(&self) -> RuntimeGraphicsVertexBufferDescriptor {
        self.request.descriptor
    }

    pub(crate) fn address_allocation_id(&self) -> ProcessAddressAllocationId {
        self.address_allocation_id
    }

    pub(crate) fn backing_range(&self) -> GuestRange {
        self.backing_range
    }
}

impl RuntimeGraphicsVertexBufferLockPlan {
    pub(crate) fn vertex_buffer_id(&self) -> RuntimeGraphicsVertexBufferId {
        self.vertex_buffer_id
    }

    pub(crate) fn lock(&self) -> RuntimeGraphicsVertexBufferLock {
        self.lock
    }
}

impl RuntimeGraphicsVertexBufferUnlockPlan {
    pub(crate) fn vertex_buffer_id(&self) -> RuntimeGraphicsVertexBufferId {
        self.vertex_buffer_id
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
    vertex_buffer_associations: Vec<(RuntimeComObjectId, RuntimeGraphicsVertexBufferId)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeGraphicsTeardownPlan {
    expected_generation: u64,
    next_generation: u64,
    devices: usize,
    textures: usize,
    surfaces: usize,
    vertex_buffers: usize,
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
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    vertex_buffers: BTreeMap<RuntimeGraphicsVertexBufferId, RuntimeGraphicsVertexBuffer>,
    #[serde(skip)]
    vertex_buffer_com_object_index: BTreeMap<RuntimeComObjectId, RuntimeGraphicsVertexBufferId>,
    #[serde(skip)]
    device_vertex_buffer_index:
        BTreeMap<RuntimeGraphicsDeviceId, BTreeSet<RuntimeGraphicsVertexBufferId>>,
    next_device_id: u64,
    next_texture_id: u64,
    next_surface_id: u64,
    next_vertex_buffer_id: u64,
    next_surface_fill_sequence: u64,
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
            vertex_buffers: BTreeMap::new(),
            vertex_buffer_com_object_index: BTreeMap::new(),
            device_vertex_buffer_index: BTreeMap::new(),
            next_device_id: 1,
            next_texture_id: 1,
            next_surface_id: 1,
            next_vertex_buffer_id: 1,
            next_surface_fill_sequence: 1,
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
        debug_assert!(
            !self
                .vertex_buffer_com_object_index
                .contains_key(&com_object_id)
        );
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
        self.device_vertex_buffer_index
            .insert(device.id, BTreeSet::new());
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
        debug_assert!(
            !self
                .vertex_buffer_com_object_index
                .contains_key(&com_object_id)
        );
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

    pub(super) fn plan_vertex_buffer_creation(
        &self,
        request: RuntimeGraphicsVertexBufferCreateRequest,
        address_allocation_id: ProcessAddressAllocationId,
        backing_range: GuestRange,
    ) -> Result<RuntimeGraphicsVertexBufferCreationPlan, RuntimeStateError> {
        validate_vertex_buffer_creation_request(request)?;
        let device_id = self
            .device_com_object_index
            .get(&request.device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-vertex-buffer-device-com-object-unknown",
                    "logical vertex-buffer creation requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(request.device_com_object_id, device_id)?;
        if address_allocation_id.0 == 0
            || backing_range.byte_length() < u64::from(request.descriptor.byte_length)
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-vertex-buffer-backing-invalid",
                "logical vertex-buffer backing must identify sufficient guest memory",
            ));
        }
        if self.vertex_buffers.len() >= RUNTIME_GRAPHICS_VERTEX_BUFFER_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-graphics",
                "graphics-vertex-buffer-capacity-reached",
                "logical graphics vertex-buffer capacity is exhausted",
            ));
        }
        if self.vertex_buffers.values().any(|vertex_buffer| {
            vertex_buffer.backing.address_allocation_id == address_allocation_id
                || vertex_buffer.backing.range == backing_range
        }) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-vertex-buffer-backing-collision",
                "logical vertex-buffer allocator selected live guest backing",
            ));
        }
        let id = RuntimeGraphicsVertexBufferId(self.next_vertex_buffer_id);
        if id.0 == 0 || self.vertex_buffers.contains_key(&id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-vertex-buffer-id-collision",
                "logical graphics vertex-buffer allocator selected a live identity",
            ));
        }
        Ok(RuntimeGraphicsVertexBufferCreationPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            id,
            device_id,
            request,
            address_allocation_id,
            backing_range,
        })
    }

    pub(super) fn preflight_vertex_buffer_creation(
        &self,
        plan: &RuntimeGraphicsVertexBufferCreationPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.next_vertex_buffer_id != plan.id.0
            || self.vertex_buffers.len() >= RUNTIME_GRAPHICS_VERTEX_BUFFER_LIMIT
            || self.vertex_buffers.contains_key(&plan.id)
            || self
                .device_com_object_index
                .get(&plan.request.device_com_object_id)
                != Some(&plan.device_id)
            || self.vertex_buffers.values().any(|vertex_buffer| {
                vertex_buffer.backing.address_allocation_id == plan.address_allocation_id
                    || vertex_buffer.backing.range == plan.backing_range
            })
        {
            return Err(stale_plan("graphics-vertex-buffer-creation-plan-stale"));
        }
        validate_vertex_buffer_creation_request(plan.request)?;
        if plan.address_allocation_id.0 == 0
            || plan.backing_range.byte_length() < u64::from(plan.request.descriptor.byte_length)
        {
            return Err(stale_plan("graphics-vertex-buffer-creation-plan-stale"));
        }
        self.validate_device_association(plan.request.device_com_object_id, plan.device_id)
    }

    pub(super) fn commit_vertex_buffer_creation(
        &mut self,
        plan: RuntimeGraphicsVertexBufferCreationPlan,
        com_object_id: RuntimeComObjectId,
        memory_owner: MemoryOwnerId,
    ) -> RuntimeGraphicsVertexBuffer {
        debug_assert!(self.preflight_vertex_buffer_creation(&plan).is_ok());
        debug_assert_ne!(com_object_id.0, 0);
        debug_assert_ne!(memory_owner.0, 0);
        debug_assert!(!self.device_com_object_index.contains_key(&com_object_id));
        debug_assert!(!self.texture_com_object_index.contains_key(&com_object_id));
        debug_assert!(!self.surface_com_object_index.contains_key(&com_object_id));
        debug_assert!(
            !self
                .vertex_buffer_com_object_index
                .contains_key(&com_object_id)
        );
        let next_vertex_buffer_id = plan
            .id
            .0
            .checked_add(1)
            .expect("preflighted graphics vertex-buffer ID must advance");
        let vertex_buffer = RuntimeGraphicsVertexBuffer {
            id: plan.id,
            namespace: RUNTIME_GRAPHICS_VERTEX_BUFFER_ID_NAMESPACE.to_owned(),
            device_id: plan.device_id,
            descriptor: plan.request.descriptor,
            backing: RuntimeGraphicsVertexBufferBacking {
                address_allocation_id: plan.address_allocation_id,
                memory_owner,
                range: plan.backing_range,
            },
            com_object_id,
            active_lock: None,
            content_generation: 0,
            generation: plan.next_generation,
        };
        self.vertex_buffer_com_object_index
            .insert(com_object_id, vertex_buffer.id);
        self.device_vertex_buffer_index
            .get_mut(&vertex_buffer.device_id)
            .expect("preflighted graphics vertex-buffer device disappeared")
            .insert(vertex_buffer.id);
        self.vertex_buffers
            .insert(vertex_buffer.id, vertex_buffer.clone());
        self.next_vertex_buffer_id = next_vertex_buffer_id;
        self.generation = plan.next_generation;
        vertex_buffer
    }

    pub(super) fn plan_vertex_buffer_lock(
        &self,
        request: RuntimeGraphicsVertexBufferLockRequest,
    ) -> Result<RuntimeGraphicsVertexBufferLockPlan, RuntimeStateError> {
        let vertex_buffer =
            self.vertex_buffer_for_com_object(request.vertex_buffer_com_object_id)?;
        if request.offset != 0 || request.byte_length != 0 || request.flags != 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-vertex-buffer-lock-variant-unsupported",
                "bounded vertex-buffer locking supports only a full-range lock with zero flags",
            ));
        }
        if vertex_buffer.active_lock.is_some() {
            return Err(RuntimeStateError::conflict(
                "runtime-graphics",
                "graphics-vertex-buffer-already-locked",
                "logical graphics vertex buffer already has an active lock",
            ));
        }
        let lock = RuntimeGraphicsVertexBufferLock {
            offset: 0,
            byte_length: vertex_buffer.descriptor.byte_length,
            flags: 0,
            guest_address: vertex_buffer.backing.range.start(),
        };
        Ok(RuntimeGraphicsVertexBufferLockPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            vertex_buffer_id: vertex_buffer.id,
            vertex_buffer_generation: vertex_buffer.generation,
            next_vertex_buffer_generation: vertex_buffer.generation.checked_add(1).ok_or_else(
                || {
                    RuntimeStateError::resource_limit(
                        "runtime-graphics",
                        "graphics-vertex-buffer-generation-exhausted",
                        "logical graphics vertex-buffer generation is exhausted",
                    )
                },
            )?,
            lock,
        })
    }

    pub(super) fn preflight_vertex_buffer_lock(
        &self,
        plan: &RuntimeGraphicsVertexBufferLockPlan,
    ) -> Result<(), RuntimeStateError> {
        let Some(vertex_buffer) = self.vertex_buffers.get(&plan.vertex_buffer_id) else {
            return Err(stale_plan("graphics-vertex-buffer-lock-plan-stale"));
        };
        if self.generation != plan.expected_generation
            || vertex_buffer.generation != plan.vertex_buffer_generation
            || vertex_buffer.active_lock.is_some()
            || plan.lock.offset != 0
            || plan.lock.byte_length != vertex_buffer.descriptor.byte_length
            || plan.lock.flags != 0
            || plan.lock.guest_address != vertex_buffer.backing.range.start()
            || self.generation.checked_add(1) != Some(plan.next_generation)
            || vertex_buffer.generation.checked_add(1) != Some(plan.next_vertex_buffer_generation)
        {
            return Err(stale_plan("graphics-vertex-buffer-lock-plan-stale"));
        }
        self.validate_vertex_buffer_association(vertex_buffer.com_object_id, vertex_buffer.id)
    }

    pub(super) fn commit_vertex_buffer_lock(
        &mut self,
        plan: RuntimeGraphicsVertexBufferLockPlan,
    ) -> RuntimeGraphicsVertexBufferLock {
        debug_assert!(self.preflight_vertex_buffer_lock(&plan).is_ok());
        let vertex_buffer = self
            .vertex_buffers
            .get_mut(&plan.vertex_buffer_id)
            .expect("preflighted graphics vertex buffer disappeared");
        vertex_buffer.active_lock = Some(plan.lock);
        vertex_buffer.generation = plan.next_vertex_buffer_generation;
        self.generation = plan.next_generation;
        plan.lock
    }

    pub(super) fn plan_vertex_buffer_unlock(
        &self,
        vertex_buffer_com_object_id: RuntimeComObjectId,
    ) -> Result<RuntimeGraphicsVertexBufferUnlockPlan, RuntimeStateError> {
        let vertex_buffer = self.vertex_buffer_for_com_object(vertex_buffer_com_object_id)?;
        let lock = vertex_buffer.active_lock.ok_or_else(|| {
            RuntimeStateError::conflict(
                "runtime-graphics",
                "graphics-vertex-buffer-not-locked",
                "logical graphics vertex buffer has no active lock",
            )
        })?;
        Ok(RuntimeGraphicsVertexBufferUnlockPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            vertex_buffer_id: vertex_buffer.id,
            vertex_buffer_generation: vertex_buffer.generation,
            next_vertex_buffer_generation: vertex_buffer.generation.checked_add(1).ok_or_else(
                || {
                    RuntimeStateError::resource_limit(
                        "runtime-graphics",
                        "graphics-vertex-buffer-generation-exhausted",
                        "logical graphics vertex-buffer generation is exhausted",
                    )
                },
            )?,
            content_generation: vertex_buffer.content_generation,
            next_content_generation: vertex_buffer.content_generation.checked_add(1).ok_or_else(
                || {
                    RuntimeStateError::resource_limit(
                        "runtime-graphics",
                        "graphics-vertex-buffer-content-generation-exhausted",
                        "logical graphics vertex-buffer content generation is exhausted",
                    )
                },
            )?,
            lock,
        })
    }

    pub(super) fn preflight_vertex_buffer_unlock(
        &self,
        plan: &RuntimeGraphicsVertexBufferUnlockPlan,
    ) -> Result<(), RuntimeStateError> {
        let Some(vertex_buffer) = self.vertex_buffers.get(&plan.vertex_buffer_id) else {
            return Err(stale_plan("graphics-vertex-buffer-unlock-plan-stale"));
        };
        if self.generation != plan.expected_generation
            || vertex_buffer.generation != plan.vertex_buffer_generation
            || vertex_buffer.content_generation != plan.content_generation
            || vertex_buffer.active_lock != Some(plan.lock)
            || self.generation.checked_add(1) != Some(plan.next_generation)
            || vertex_buffer.generation.checked_add(1) != Some(plan.next_vertex_buffer_generation)
            || vertex_buffer.content_generation.checked_add(1) != Some(plan.next_content_generation)
        {
            return Err(stale_plan("graphics-vertex-buffer-unlock-plan-stale"));
        }
        self.validate_vertex_buffer_association(vertex_buffer.com_object_id, vertex_buffer.id)
    }

    pub(super) fn commit_vertex_buffer_unlock(
        &mut self,
        plan: RuntimeGraphicsVertexBufferUnlockPlan,
    ) -> RuntimeGraphicsVertexBufferUnlockResult {
        debug_assert!(self.preflight_vertex_buffer_unlock(&plan).is_ok());
        let vertex_buffer = self
            .vertex_buffers
            .get_mut(&plan.vertex_buffer_id)
            .expect("preflighted graphics vertex buffer disappeared");
        vertex_buffer.active_lock = None;
        vertex_buffer.content_generation = plan.next_content_generation;
        vertex_buffer.generation = plan.next_vertex_buffer_generation;
        self.generation = plan.next_generation;
        RuntimeGraphicsVertexBufferUnlockResult {
            vertex_buffer_id: vertex_buffer.id,
            generation: vertex_buffer.generation,
            content_generation: vertex_buffer.content_generation,
        }
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
        debug_assert!(
            !self
                .vertex_buffer_com_object_index
                .contains_key(&com_object_id)
        );
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
            fills: Vec::new(),
            content_generation: 0,
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

    pub(crate) fn plan_surface_fill(
        &self,
        request: RuntimeGraphicsSurfaceFillRequest,
    ) -> Result<RuntimeGraphicsSurfaceFillPlan, RuntimeStateError> {
        validate_surface_fill_request(request)?;
        let surface_id = self
            .surface_com_object_index
            .get(&request.surface_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-surface-fill-com-object-unknown",
                    "logical surface fill requires a live graphics surface COM object",
                )
            })?;
        self.validate_surface_association(request.surface_com_object_id, surface_id)?;
        let surface = self
            .surfaces
            .get(&surface_id)
            .expect("validated logical surface association disappeared");
        validate_surface_fill_rect(request.rect, surface.descriptor)?;
        let empty =
            request.rect.left == request.rect.right || request.rect.top == request.rect.bottom;
        if !empty && surface.fills.len() >= RUNTIME_GRAPHICS_SURFACE_FILL_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-graphics",
                "graphics-surface-fill-capacity-reached",
                "logical graphics surface fill history capacity is exhausted",
            ));
        }
        let fill = (!empty).then_some(RuntimeGraphicsSurfaceFill {
            sequence: self.next_surface_fill_sequence,
            rect: request.rect,
            color_ref: request.color_ref,
        });
        let (
            next_generation,
            next_surface_generation,
            next_surface_content_generation,
            next_fill_sequence,
        ) = if fill.is_some() {
            (
                checked_next_generation(self.generation)?,
                surface.generation.checked_add(1).ok_or_else(|| {
                    RuntimeStateError::resource_limit(
                        "runtime-graphics",
                        "graphics-surface-generation-exhausted",
                        "logical graphics surface generation is exhausted",
                    )
                })?,
                surface.content_generation.checked_add(1).ok_or_else(|| {
                    RuntimeStateError::resource_limit(
                        "runtime-graphics",
                        "graphics-surface-content-generation-exhausted",
                        "logical graphics surface content generation is exhausted",
                    )
                })?,
                self.next_surface_fill_sequence
                    .checked_add(1)
                    .ok_or_else(|| {
                        RuntimeStateError::resource_limit(
                            "runtime-graphics",
                            "graphics-surface-fill-sequence-exhausted",
                            "logical graphics surface fill sequence is exhausted",
                        )
                    })?,
            )
        } else {
            (
                self.generation,
                surface.generation,
                surface.content_generation,
                self.next_surface_fill_sequence,
            )
        };
        Ok(RuntimeGraphicsSurfaceFillPlan {
            expected_generation: self.generation,
            next_generation,
            surface_id,
            surface_generation: surface.generation,
            next_surface_generation,
            surface_content_generation: surface.content_generation,
            next_surface_content_generation,
            fill,
            next_fill_sequence,
        })
    }

    pub(crate) fn preflight_surface_fill(
        &self,
        plan: &RuntimeGraphicsSurfaceFillPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation {
            return Err(stale_plan("graphics-surface-fill-plan-stale"));
        }
        let surface = self
            .surfaces
            .get(&plan.surface_id)
            .ok_or_else(|| stale_plan("graphics-surface-fill-plan-stale"))?;
        if surface.generation != plan.surface_generation
            || surface.content_generation != plan.surface_content_generation
        {
            return Err(stale_plan("graphics-surface-fill-plan-stale"));
        }
        if let Some(fill) = plan.fill {
            if fill.sequence != self.next_surface_fill_sequence
                || self.next_surface_fill_sequence.checked_add(1) != Some(plan.next_fill_sequence)
                || surface.fills.len() >= RUNTIME_GRAPHICS_SURFACE_FILL_LIMIT
                || surface.generation.checked_add(1) != Some(plan.next_surface_generation)
                || surface.content_generation.checked_add(1)
                    != Some(plan.next_surface_content_generation)
                || self.generation.checked_add(1) != Some(plan.next_generation)
            {
                return Err(stale_plan("graphics-surface-fill-plan-stale"));
            }
            validate_surface_fill_rect(fill.rect, surface.descriptor)?;
        } else if plan.next_generation != self.generation
            || plan.next_surface_generation != surface.generation
            || plan.next_surface_content_generation != surface.content_generation
            || plan.next_fill_sequence != self.next_surface_fill_sequence
        {
            return Err(stale_plan("graphics-surface-fill-plan-stale"));
        }
        Ok(())
    }

    pub(crate) fn commit_surface_fill(
        &mut self,
        plan: RuntimeGraphicsSurfaceFillPlan,
    ) -> RuntimeGraphicsSurfaceFillResult {
        debug_assert!(self.preflight_surface_fill(&plan).is_ok());
        let surface = self
            .surfaces
            .get_mut(&plan.surface_id)
            .expect("preflighted graphics surface disappeared");
        if let Some(fill) = plan.fill {
            surface.fills.push(fill);
            surface.content_generation = plan.next_surface_content_generation;
            surface.generation = plan.next_surface_generation;
            self.next_surface_fill_sequence = plan.next_fill_sequence;
            self.generation = plan.next_generation;
        }
        RuntimeGraphicsSurfaceFillResult {
            surface_id: surface.id,
            surface_generation: surface.generation,
            content_generation: surface.content_generation,
            fill: plan.fill,
        }
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
        let vertex_buffer_associations = com_object_ids
            .iter()
            .filter_map(|com_object_id| {
                self.vertex_buffer_com_object_index
                    .get(com_object_id)
                    .copied()
                    .map(|vertex_buffer_id| (*com_object_id, vertex_buffer_id))
            })
            .collect::<Vec<_>>();
        if device_associations.is_empty()
            && texture_associations.is_empty()
            && surface_associations.is_empty()
            && vertex_buffer_associations.is_empty()
        {
            return Ok(None);
        }
        self.validate_retirement_associations(
            &device_associations,
            &texture_associations,
            &surface_associations,
            &vertex_buffer_associations,
        )?;
        Ok(Some(RuntimeGraphicsComRetirementPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            device_associations,
            texture_associations,
            surface_associations,
            vertex_buffer_associations,
        }))
    }

    pub(crate) fn vertex_buffer_backings_for_retirement(
        &self,
        plan: &RuntimeGraphicsComRetirementPlan,
    ) -> Result<Vec<(RuntimeGraphicsVertexBufferBacking, u32)>, RuntimeStateError> {
        self.preflight_com_object_retirement(plan)?;
        Ok(plan
            .vertex_buffer_associations
            .iter()
            .map(|(_, vertex_buffer_id)| {
                let vertex_buffer = self
                    .vertex_buffers
                    .get(vertex_buffer_id)
                    .expect("validated retiring graphics vertex buffer disappeared");
                (vertex_buffer.backing, vertex_buffer.descriptor.byte_length)
            })
            .collect())
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
                    &plan.vertex_buffer_associations,
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
        for (com_object_id, vertex_buffer_id) in plan.vertex_buffer_associations {
            self.vertex_buffer_com_object_index.remove(&com_object_id);
            let vertex_buffer = self
                .vertex_buffers
                .remove(&vertex_buffer_id)
                .expect("preflighted logical vertex buffer disappeared during retirement");
            self.device_vertex_buffer_index
                .get_mut(&vertex_buffer.device_id)
                .expect("preflighted logical vertex-buffer device disappeared during retirement")
                .remove(&vertex_buffer_id);
        }
        for (com_object_id, device_id) in plan.device_associations {
            self.device_com_object_index.remove(&com_object_id);
            self.device_texture_index.remove(&device_id);
            self.device_vertex_buffer_index.remove(&device_id);
            self.devices.remove(&device_id);
        }
        self.generation = plan.next_generation;
    }

    fn validate_retirement_associations(
        &self,
        device_associations: &[(RuntimeComObjectId, RuntimeGraphicsDeviceId)],
        texture_associations: &[(RuntimeComObjectId, RuntimeGraphicsTextureId)],
        surface_associations: &[(RuntimeComObjectId, RuntimeGraphicsSurfaceId)],
        vertex_buffer_associations: &[(RuntimeComObjectId, RuntimeGraphicsVertexBufferId)],
    ) -> Result<(), RuntimeStateError> {
        if device_associations.is_empty()
            && texture_associations.is_empty()
            && surface_associations.is_empty()
            && vertex_buffer_associations.is_empty()
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
        for (com_object_id, vertex_buffer_id) in vertex_buffer_associations {
            self.validate_vertex_buffer_association(*com_object_id, *vertex_buffer_id)?;
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
        let retiring_vertex_buffers = vertex_buffer_associations
            .iter()
            .map(|(_, vertex_buffer_id)| *vertex_buffer_id)
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
            let vertex_buffers =
                self.device_vertex_buffer_index
                    .get(device_id)
                    .ok_or_else(|| {
                        RuntimeStateError::runtime_defect(
                            "runtime-graphics",
                            "graphics-device-vertex-buffer-index-missing",
                            "logical graphics device has no vertex-buffer relation index",
                        )
                    })?;
            if !vertex_buffers.is_subset(&retiring_vertex_buffers) {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-graphics",
                    "graphics-device-retirement-has-live-vertex-buffers",
                    "logical graphics device cannot retire while a child vertex buffer remains live",
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
        if !self.device_texture_index.contains_key(&device_id)
            || !self.device_vertex_buffer_index.contains_key(&device_id)
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-device-texture-index-missing",
                "logical graphics device is missing a resource relation index",
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

    fn validate_vertex_buffer_association(
        &self,
        com_object_id: RuntimeComObjectId,
        vertex_buffer_id: RuntimeGraphicsVertexBufferId,
    ) -> Result<(), RuntimeStateError> {
        if self.vertex_buffer_com_object_index.get(&com_object_id) != Some(&vertex_buffer_id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-vertex-buffer-com-index-dangling",
                "graphics vertex-buffer COM association does not resolve to a live logical buffer",
            ));
        }
        let vertex_buffer = self.vertex_buffers.get(&vertex_buffer_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-vertex-buffer-com-association-mismatch",
                "logical graphics vertex buffer and COM index disagree",
            )
        })?;
        if vertex_buffer.com_object_id != com_object_id
            || !self
                .device_vertex_buffer_index
                .get(&vertex_buffer.device_id)
                .is_some_and(|vertex_buffers| vertex_buffers.contains(&vertex_buffer_id))
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-vertex-buffer-com-association-mismatch",
                "logical graphics vertex buffer, device relation, and COM index disagree",
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
            vertex_buffers: self.vertex_buffers.len(),
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
            || self.device_vertex_buffer_index.len() != plan.devices
            || self.textures.len() != plan.textures
            || self.texture_com_object_index.len() != plan.textures
            || self.texture_surface_index.len() != plan.textures
            || self.surfaces.len() != plan.surfaces
            || self.surface_com_object_index.len() != plan.surfaces
            || self.texture_level_surface_index.len() != plan.surfaces
            || self.vertex_buffers.len() != plan.vertex_buffers
            || self.vertex_buffer_com_object_index.len() != plan.vertex_buffers
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
        self.vertex_buffers.clear();
        self.vertex_buffer_com_object_index.clear();
        self.device_vertex_buffer_index.clear();
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

    pub(super) fn texture_for_com_object(
        &self,
        com_object_id: RuntimeComObjectId,
    ) -> Result<&RuntimeGraphicsTexture, RuntimeStateError> {
        let texture_id = self
            .texture_com_object_index
            .get(&com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-texture-com-object-unknown",
                    "logical graphics texture COM object is not live",
                )
            })?;
        self.validate_texture_association(com_object_id, texture_id)?;
        Ok(self
            .textures
            .get(&texture_id)
            .expect("validated logical texture association disappeared"))
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

    pub fn vertex_buffer(
        &self,
        id: RuntimeGraphicsVertexBufferId,
    ) -> Result<&RuntimeGraphicsVertexBuffer, RuntimeStateError> {
        self.vertex_buffers.get(&id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-graphics",
                "graphics-vertex-buffer-id-unknown",
                format!("logical graphics vertex buffer {} is not live", id.0),
            )
        })
    }

    pub fn vertex_buffers(
        &self,
    ) -> &BTreeMap<RuntimeGraphicsVertexBufferId, RuntimeGraphicsVertexBuffer> {
        &self.vertex_buffers
    }

    pub(super) fn vertex_buffer_for_com_object(
        &self,
        com_object_id: RuntimeComObjectId,
    ) -> Result<&RuntimeGraphicsVertexBuffer, RuntimeStateError> {
        let vertex_buffer_id = self
            .vertex_buffer_com_object_index
            .get(&com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-vertex-buffer-com-object-unknown",
                    "logical graphics vertex-buffer COM object is not live",
                )
            })?;
        self.validate_vertex_buffer_association(com_object_id, vertex_buffer_id)?;
        Ok(self
            .vertex_buffers
            .get(&vertex_buffer_id)
            .expect("validated logical vertex-buffer association disappeared"))
    }

    pub(super) fn surface_for_com_object(
        &self,
        com_object_id: RuntimeComObjectId,
    ) -> Result<&RuntimeGraphicsSurface, RuntimeStateError> {
        let surface_id = self
            .surface_com_object_index
            .get(&com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-surface-com-object-unknown",
                    "logical graphics surface COM object is not live",
                )
            })?;
        self.validate_surface_association(com_object_id, surface_id)?;
        Ok(self
            .surfaces
            .get(&surface_id)
            .expect("validated logical surface association disappeared"))
    }

    pub(super) fn surface_for_texture_level(
        &self,
        texture_com_object_id: RuntimeComObjectId,
        level: u32,
    ) -> Result<Option<&RuntimeGraphicsSurface>, RuntimeStateError> {
        let texture = self.texture_for_com_object(texture_com_object_id)?;
        let texture_id = texture.id;
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
        self.devices.is_empty()
            && self.textures.is_empty()
            && self.surfaces.is_empty()
            && self.vertex_buffers.is_empty()
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

fn validate_vertex_buffer_creation_request(
    request: RuntimeGraphicsVertexBufferCreateRequest,
) -> Result<(), RuntimeStateError> {
    if request.device_com_object_id.0 == 0
        || request.descriptor.byte_length == 0
        || request.descriptor.flexible_vertex_format == 0
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-graphics",
            "graphics-vertex-buffer-creation-request-invalid",
            "logical graphics vertex-buffer creation requires a live device and nonzero descriptor",
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

fn validate_surface_fill_request(
    request: RuntimeGraphicsSurfaceFillRequest,
) -> Result<(), RuntimeStateError> {
    if request.surface_com_object_id.0 == 0 || request.color_ref > 0x00ff_ffff {
        return Err(RuntimeStateError::invalid_input(
            "runtime-graphics",
            "graphics-surface-fill-request-invalid",
            "logical graphics surface fill requires a live surface and a Win32 COLORREF",
        ));
    }
    Ok(())
}

fn validate_surface_fill_rect(
    rect: RuntimeRect,
    descriptor: RuntimeGraphicsSurfaceDescriptor,
) -> Result<(), RuntimeStateError> {
    let width = i32::try_from(descriptor.width).map_err(|_| {
        RuntimeStateError::runtime_defect(
            "runtime-graphics",
            "graphics-surface-width-invalid",
            "logical graphics surface width does not fit x86 coordinates",
        )
    })?;
    let height = i32::try_from(descriptor.height).map_err(|_| {
        RuntimeStateError::runtime_defect(
            "runtime-graphics",
            "graphics-surface-height-invalid",
            "logical graphics surface height does not fit x86 coordinates",
        )
    })?;
    if rect.left < 0
        || rect.top < 0
        || rect.right < rect.left
        || rect.bottom < rect.top
        || rect.right > width
        || rect.bottom > height
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-graphics",
            "graphics-surface-fill-rect-out-of-bounds",
            "bounded graphics surface fill requires an ordered in-bounds rectangle",
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

    fn fill_request(
        surface_com_object_id: RuntimeComObjectId,
        rect: RuntimeRect,
    ) -> RuntimeGraphicsSurfaceFillRequest {
        RuntimeGraphicsSurfaceFillRequest {
            surface_com_object_id,
            rect,
            color_ref: 0,
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
    fn vertex_buffer_lock_unlock_and_retirement_preserve_backing_identity() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics.plan_device_creation(request()).unwrap();
        let device = graphics.commit_device_creation(device_plan, RuntimeComObjectId(7));
        let backing_range = GuestRange::new(RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE, 0x1000).unwrap();
        let creation = graphics
            .plan_vertex_buffer_creation(
                RuntimeGraphicsVertexBufferCreateRequest {
                    device_com_object_id: RuntimeComObjectId(7),
                    descriptor: RuntimeGraphicsVertexBufferDescriptor {
                        byte_length: 112,
                        usage: RuntimeGraphicsVertexBufferUsage::WriteOnly,
                        flexible_vertex_format: 0x144,
                        pool: RuntimeGraphicsVertexBufferPool::Managed,
                    },
                },
                ProcessAddressAllocationId(41),
                backing_range,
            )
            .unwrap();
        let stale_creation = creation.clone();
        graphics
            .preflight_vertex_buffer_creation(&creation)
            .unwrap();
        let vertex_buffer = graphics.commit_vertex_buffer_creation(
            creation,
            RuntimeComObjectId(8),
            MemoryOwnerId(23),
        );
        assert_eq!(vertex_buffer.device_id, device.id);
        assert_eq!(vertex_buffer.backing.address_allocation_id.0, 41);
        assert_eq!(vertex_buffer.backing.memory_owner.0, 23);
        assert_eq!(vertex_buffer.backing.range, backing_range);
        assert_eq!(
            graphics
                .preflight_vertex_buffer_creation(&stale_creation)
                .unwrap_err()
                .code,
            "graphics-vertex-buffer-creation-plan-stale"
        );

        let lock_plan = graphics
            .plan_vertex_buffer_lock(RuntimeGraphicsVertexBufferLockRequest {
                vertex_buffer_com_object_id: RuntimeComObjectId(8),
                offset: 0,
                byte_length: 0,
                flags: 0,
            })
            .unwrap();
        let stale_lock = lock_plan.clone();
        let lock = graphics.commit_vertex_buffer_lock(lock_plan);
        assert_eq!(lock.guest_address, backing_range.start());
        assert_eq!(lock.byte_length, 112);
        assert_eq!(
            graphics
                .preflight_vertex_buffer_lock(&stale_lock)
                .unwrap_err()
                .code,
            "graphics-vertex-buffer-lock-plan-stale"
        );
        assert_eq!(
            graphics
                .plan_vertex_buffer_lock(RuntimeGraphicsVertexBufferLockRequest {
                    vertex_buffer_com_object_id: RuntimeComObjectId(8),
                    offset: 0,
                    byte_length: 0,
                    flags: 0,
                })
                .unwrap_err()
                .code,
            "graphics-vertex-buffer-already-locked"
        );

        let unlock_plan = graphics
            .plan_vertex_buffer_unlock(RuntimeComObjectId(8))
            .unwrap();
        let stale_unlock = unlock_plan.clone();
        let unlock = graphics.commit_vertex_buffer_unlock(unlock_plan);
        assert_eq!(unlock.vertex_buffer_id, vertex_buffer.id);
        assert_eq!(unlock.content_generation, 1);
        assert_eq!(
            graphics
                .preflight_vertex_buffer_unlock(&stale_unlock)
                .unwrap_err()
                .code,
            "graphics-vertex-buffer-unlock-plan-stale"
        );
        assert_eq!(
            graphics
                .plan_vertex_buffer_unlock(RuntimeComObjectId(8))
                .unwrap_err()
                .code,
            "graphics-vertex-buffer-not-locked"
        );

        let retirement = graphics
            .plan_com_object_retirements(&[RuntimeComObjectId(8)].into_iter().collect())
            .unwrap()
            .unwrap();
        assert_eq!(
            graphics
                .vertex_buffer_backings_for_retirement(&retirement)
                .unwrap(),
            vec![(vertex_buffer.backing, 112)]
        );
        graphics.commit_com_object_retirement(retirement);
        assert!(graphics.vertex_buffers().is_empty());
        assert_eq!(graphics.device(device.id).unwrap().id, device.id);
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

    #[test]
    fn surface_fill_owns_content_history_and_rejects_stale_or_invalid_plans() {
        let mut graphics = RuntimeGraphics::default();
        let device = graphics.plan_device_creation(request()).unwrap();
        graphics.commit_device_creation(device, RuntimeComObjectId(1));
        let texture = graphics
            .plan_texture_creation(texture_request(RuntimeComObjectId(1)))
            .unwrap();
        graphics.commit_texture_creation(texture, RuntimeComObjectId(2));
        let surface = graphics
            .plan_surface_creation(surface_request(RuntimeComObjectId(2), 0))
            .unwrap();
        let surface = graphics.commit_surface_creation(surface, RuntimeComObjectId(3));

        let rect = RuntimeRect {
            left: 0,
            top: 0,
            right: 800,
            bottom: 600,
        };
        let plan = graphics
            .plan_surface_fill(fill_request(RuntimeComObjectId(3), rect))
            .unwrap();
        let stale = plan.clone();
        graphics.preflight_surface_fill(&plan).unwrap();
        let result = graphics.commit_surface_fill(plan);
        assert_eq!(result.surface_id, surface.id);
        assert_eq!(result.content_generation, 1);
        assert_eq!(result.fill.unwrap().rect, rect);
        let filled = graphics.surface(surface.id).unwrap();
        assert_eq!(filled.fills.len(), 1);
        assert_eq!(filled.fills[0].color_ref, 0);
        assert_eq!(filled.content_generation, 1);
        assert_eq!(
            graphics.preflight_surface_fill(&stale).unwrap_err().code,
            "graphics-surface-fill-plan-stale"
        );

        let before_empty = graphics.clone();
        let empty = graphics
            .plan_surface_fill(fill_request(
                RuntimeComObjectId(3),
                RuntimeRect {
                    left: 1,
                    top: 1,
                    right: 1,
                    bottom: 1,
                },
            ))
            .unwrap();
        let result = graphics.commit_surface_fill(empty);
        assert!(result.fill.is_none());
        assert_eq!(graphics, before_empty);

        let before_invalid = graphics.clone();
        assert_eq!(
            graphics
                .plan_surface_fill(fill_request(
                    RuntimeComObjectId(3),
                    RuntimeRect {
                        left: 0,
                        top: 0,
                        right: 1_025,
                        bottom: 1,
                    },
                ))
                .unwrap_err()
                .code,
            "graphics-surface-fill-rect-out-of-bounds"
        );
        assert_eq!(graphics, before_invalid);
    }
}

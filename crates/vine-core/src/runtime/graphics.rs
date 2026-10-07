use std::collections::{BTreeMap, BTreeSet};

use super::{
    ProcessAddressAllocationId, RuntimeComObjectId, RuntimeRect, RuntimeStateError,
    RuntimeThreadId, RuntimeWindowId,
};
use crate::memory::{GuestAddress, GuestRange, MemoryOwnerId};

pub const RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE: crate::memory::GuestAddress =
    crate::memory::GuestAddress(0x4600_0000);
// Share the gap after audio and before thread stacks among texels, vertex data
// and GDI DIBs. Reservation is virtual; committed pages retain memory quotas.
pub const RUNTIME_GRAPHICS_RESOURCE_ARENA_SIZE: u64 = 0x0a00_0000;
pub const RUNTIME_GRAPHICS_DEVICE_LIMIT: usize = 4_096;
pub const RUNTIME_GRAPHICS_TEXTURE_LIMIT: usize = 4_096;
pub const RUNTIME_GRAPHICS_SURFACE_LIMIT: usize = 4_096;
pub const RUNTIME_GRAPHICS_VERTEX_BUFFER_LIMIT: usize = 4_096;
pub const RUNTIME_GRAPHICS_SURFACE_FILL_LIMIT: usize = 16_384;
pub const RUNTIME_GRAPHICS_DRAW_SUBMISSION_LIMIT: usize = 4_096;
pub const RUNTIME_GRAPHICS_TEXTURE_LOCK_DISCARD: u32 = 0x0000_2000;
pub const RUNTIME_GRAPHICS_TEXTURE_LOCK_NO_DIRTY_UPDATE: u32 = 0x0000_8000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeGraphicsDeviceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeGraphicsTextureId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeGraphicsSurfaceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeGraphicsVertexBufferId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsCullMode {
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsTextureFilter {
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsTextureOperation {
    Modulate,
    SelectArgumentOne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsTextureArgument {
    Diffuse,
    Texture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsFlexibleVertexFormat {
    XyzRhwDiffuseTexture1,
}

#[derive(Debug, Default)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub struct RuntimeGraphicsTextureBindings {
    pub stage_zero: Option<RuntimeGraphicsTextureId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeGraphicsStreamSourceBinding {
    pub vertex_buffer_id: RuntimeGraphicsVertexBufferId,
    pub offset_in_bytes: u32,
    pub stride: u32,
}

#[derive(Debug, Default)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub struct RuntimeGraphicsStreamSources {
    pub stream_zero: Option<RuntimeGraphicsStreamSourceBinding>,
}

#[derive(Debug, Default, PartialEq, Eq)]
#[cfg_attr(test, derive(Clone))]
pub struct RuntimeGraphicsSamplerState {
    pub min_filter: Option<RuntimeGraphicsTextureFilter>,
    pub mag_filter: Option<RuntimeGraphicsTextureFilter>,
}

#[derive(Debug, Default, PartialEq, Eq)]
#[cfg_attr(test, derive(Clone))]
pub struct RuntimeGraphicsSamplerStates {
    pub sampler_zero: RuntimeGraphicsSamplerState,
}

impl RuntimeGraphicsSamplerStates {
    fn current_for(
        &self,
        update: RuntimeGraphicsSamplerStateUpdate,
    ) -> Option<RuntimeGraphicsSamplerStateUpdate> {
        match update {
            RuntimeGraphicsSamplerStateUpdate::MinFilter { sampler: 0, .. } => self
                .sampler_zero
                .min_filter
                .map(|filter| RuntimeGraphicsSamplerStateUpdate::MinFilter { sampler: 0, filter }),
            RuntimeGraphicsSamplerStateUpdate::MagFilter { sampler: 0, .. } => self
                .sampler_zero
                .mag_filter
                .map(|filter| RuntimeGraphicsSamplerStateUpdate::MagFilter { sampler: 0, filter }),
            _ => None,
        }
    }

    fn apply(&mut self, update: RuntimeGraphicsSamplerStateUpdate) {
        match update {
            RuntimeGraphicsSamplerStateUpdate::MinFilter { sampler: 0, filter } => {
                self.sampler_zero.min_filter = Some(filter);
            }
            RuntimeGraphicsSamplerStateUpdate::MagFilter { sampler: 0, filter } => {
                self.sampler_zero.mag_filter = Some(filter);
            }
            _ => unreachable!("validated sampler-state update has an unsupported sampler"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsSamplerStateUpdate {
    MinFilter {
        sampler: u32,
        filter: RuntimeGraphicsTextureFilter,
    },
    MagFilter {
        sampler: u32,
        filter: RuntimeGraphicsTextureFilter,
    },
}

impl RuntimeGraphicsSamplerStateUpdate {
    fn validate(self) -> Result<(), RuntimeStateError> {
        let sampler = match self {
            Self::MinFilter { sampler, .. } | Self::MagFilter { sampler, .. } => sampler,
        };
        if sampler != 0 {
            return Err(RuntimeStateError::new(
                "logical sampler-state authority currently supports sampler zero only",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
#[cfg_attr(test, derive(Clone))]
pub struct RuntimeGraphicsTextureStageState {
    pub color_operation: Option<RuntimeGraphicsTextureOperation>,
    pub color_argument_one: Option<RuntimeGraphicsTextureArgument>,
    pub color_argument_two: Option<RuntimeGraphicsTextureArgument>,
    pub alpha_operation: Option<RuntimeGraphicsTextureOperation>,
    pub alpha_argument_one: Option<RuntimeGraphicsTextureArgument>,
    pub alpha_argument_two: Option<RuntimeGraphicsTextureArgument>,
}

#[derive(Debug, Default, PartialEq, Eq)]
#[cfg_attr(test, derive(Clone))]
pub struct RuntimeGraphicsTextureStageStates {
    pub stage_zero: RuntimeGraphicsTextureStageState,
}

impl RuntimeGraphicsTextureStageStates {
    fn current_for(
        &self,
        update: RuntimeGraphicsTextureStageStateUpdate,
    ) -> Option<RuntimeGraphicsTextureStageStateUpdate> {
        match update {
            RuntimeGraphicsTextureStageStateUpdate::ColorOperation { stage: 0, .. } => {
                self.stage_zero.color_operation.map(|operation| {
                    RuntimeGraphicsTextureStageStateUpdate::ColorOperation {
                        stage: 0,
                        operation,
                    }
                })
            }
            RuntimeGraphicsTextureStageStateUpdate::ColorArgumentOne { stage: 0, .. } => {
                self.stage_zero.color_argument_one.map(|argument| {
                    RuntimeGraphicsTextureStageStateUpdate::ColorArgumentOne { stage: 0, argument }
                })
            }
            RuntimeGraphicsTextureStageStateUpdate::ColorArgumentTwo { stage: 0, .. } => {
                self.stage_zero.color_argument_two.map(|argument| {
                    RuntimeGraphicsTextureStageStateUpdate::ColorArgumentTwo { stage: 0, argument }
                })
            }
            RuntimeGraphicsTextureStageStateUpdate::AlphaOperation { stage: 0, .. } => {
                self.stage_zero.alpha_operation.map(|operation| {
                    RuntimeGraphicsTextureStageStateUpdate::AlphaOperation {
                        stage: 0,
                        operation,
                    }
                })
            }
            RuntimeGraphicsTextureStageStateUpdate::AlphaArgumentOne { stage: 0, .. } => {
                self.stage_zero.alpha_argument_one.map(|argument| {
                    RuntimeGraphicsTextureStageStateUpdate::AlphaArgumentOne { stage: 0, argument }
                })
            }
            RuntimeGraphicsTextureStageStateUpdate::AlphaArgumentTwo { stage: 0, .. } => {
                self.stage_zero.alpha_argument_two.map(|argument| {
                    RuntimeGraphicsTextureStageStateUpdate::AlphaArgumentTwo { stage: 0, argument }
                })
            }
            _ => None,
        }
    }

    fn apply(&mut self, update: RuntimeGraphicsTextureStageStateUpdate) {
        match update {
            RuntimeGraphicsTextureStageStateUpdate::ColorOperation {
                stage: 0,
                operation,
            } => self.stage_zero.color_operation = Some(operation),
            RuntimeGraphicsTextureStageStateUpdate::ColorArgumentOne { stage: 0, argument } => {
                self.stage_zero.color_argument_one = Some(argument)
            }
            RuntimeGraphicsTextureStageStateUpdate::ColorArgumentTwo { stage: 0, argument } => {
                self.stage_zero.color_argument_two = Some(argument)
            }
            RuntimeGraphicsTextureStageStateUpdate::AlphaOperation {
                stage: 0,
                operation,
            } => self.stage_zero.alpha_operation = Some(operation),
            RuntimeGraphicsTextureStageStateUpdate::AlphaArgumentOne { stage: 0, argument } => {
                self.stage_zero.alpha_argument_one = Some(argument)
            }
            RuntimeGraphicsTextureStageStateUpdate::AlphaArgumentTwo { stage: 0, argument } => {
                self.stage_zero.alpha_argument_two = Some(argument)
            }
            _ => unreachable!("validated texture-stage-state update has an unsupported stage"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsTextureStageStateUpdate {
    ColorOperation {
        stage: u32,
        operation: RuntimeGraphicsTextureOperation,
    },
    ColorArgumentOne {
        stage: u32,
        argument: RuntimeGraphicsTextureArgument,
    },
    ColorArgumentTwo {
        stage: u32,
        argument: RuntimeGraphicsTextureArgument,
    },
    AlphaOperation {
        stage: u32,
        operation: RuntimeGraphicsTextureOperation,
    },
    AlphaArgumentOne {
        stage: u32,
        argument: RuntimeGraphicsTextureArgument,
    },
    AlphaArgumentTwo {
        stage: u32,
        argument: RuntimeGraphicsTextureArgument,
    },
}

impl RuntimeGraphicsTextureStageStateUpdate {
    fn validate(self) -> Result<(), RuntimeStateError> {
        let stage = match self {
            Self::ColorOperation { stage, .. }
            | Self::ColorArgumentOne { stage, .. }
            | Self::ColorArgumentTwo { stage, .. }
            | Self::AlphaOperation { stage, .. }
            | Self::AlphaArgumentOne { stage, .. }
            | Self::AlphaArgumentTwo { stage, .. } => stage,
        };
        if stage != 0 {
            return Err(RuntimeStateError::new(
                "logical texture-stage-state authority currently supports stage zero only",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
#[cfg_attr(test, derive(Clone))]
pub struct RuntimeGraphicsRenderStates {
    pub cull_mode: Option<RuntimeGraphicsCullMode>,
    pub depth_test_enabled: Option<bool>,
    pub lighting_enabled: Option<bool>,
    pub alpha_blend_enabled: Option<bool>,
    pub source_blend_factor: Option<RuntimeGraphicsBlendFactor>,
    pub destination_blend_factor: Option<RuntimeGraphicsBlendFactor>,
    pub blend_operation: Option<RuntimeGraphicsBlendOperation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsBlendFactor {
    SourceAlpha,
    InverseSourceAlpha,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsBlendOperation {
    Add,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsRenderStateUpdate {
    CullMode(RuntimeGraphicsCullMode),
    DepthTestEnabled(bool),
    LightingEnabled(bool),
    AlphaBlendEnabled(bool),
    SourceBlendFactor(RuntimeGraphicsBlendFactor),
    DestinationBlendFactor(RuntimeGraphicsBlendFactor),
    BlendOperation(RuntimeGraphicsBlendOperation),
}

impl RuntimeGraphicsRenderStates {
    fn current_for(
        &self,
        update: RuntimeGraphicsRenderStateUpdate,
    ) -> Option<RuntimeGraphicsRenderStateUpdate> {
        match update {
            RuntimeGraphicsRenderStateUpdate::CullMode(_) => self
                .cull_mode
                .map(RuntimeGraphicsRenderStateUpdate::CullMode),
            RuntimeGraphicsRenderStateUpdate::DepthTestEnabled(_) => self
                .depth_test_enabled
                .map(RuntimeGraphicsRenderStateUpdate::DepthTestEnabled),
            RuntimeGraphicsRenderStateUpdate::LightingEnabled(_) => self
                .lighting_enabled
                .map(RuntimeGraphicsRenderStateUpdate::LightingEnabled),
            RuntimeGraphicsRenderStateUpdate::AlphaBlendEnabled(_) => self
                .alpha_blend_enabled
                .map(RuntimeGraphicsRenderStateUpdate::AlphaBlendEnabled),
            RuntimeGraphicsRenderStateUpdate::SourceBlendFactor(_) => self
                .source_blend_factor
                .map(RuntimeGraphicsRenderStateUpdate::SourceBlendFactor),
            RuntimeGraphicsRenderStateUpdate::DestinationBlendFactor(_) => self
                .destination_blend_factor
                .map(RuntimeGraphicsRenderStateUpdate::DestinationBlendFactor),
            RuntimeGraphicsRenderStateUpdate::BlendOperation(_) => self
                .blend_operation
                .map(RuntimeGraphicsRenderStateUpdate::BlendOperation),
        }
    }

    fn apply(&mut self, update: RuntimeGraphicsRenderStateUpdate) {
        match update {
            RuntimeGraphicsRenderStateUpdate::CullMode(value) => self.cull_mode = Some(value),
            RuntimeGraphicsRenderStateUpdate::DepthTestEnabled(value) => {
                self.depth_test_enabled = Some(value);
            }
            RuntimeGraphicsRenderStateUpdate::LightingEnabled(value) => {
                self.lighting_enabled = Some(value);
            }
            RuntimeGraphicsRenderStateUpdate::AlphaBlendEnabled(value) => {
                self.alpha_blend_enabled = Some(value);
            }
            RuntimeGraphicsRenderStateUpdate::SourceBlendFactor(value) => {
                self.source_blend_factor = Some(value);
            }
            RuntimeGraphicsRenderStateUpdate::DestinationBlendFactor(value) => {
                self.destination_blend_factor = Some(value);
            }
            RuntimeGraphicsRenderStateUpdate::BlendOperation(value) => {
                self.blend_operation = Some(value);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsColorFormat {
    B8g8r8x8Unorm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsTextureUsage {
    None,
    Dynamic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsTexturePool {
    Default,
    SystemMemory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsVertexBufferUsage {
    WriteOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsVertexBufferPool {
    Managed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeGraphicsTextureDescriptor {
    pub width: u32,
    pub height: u32,
    pub levels: u32,
    pub usage: RuntimeGraphicsTextureUsage,
    pub color_format: RuntimeGraphicsColorFormat,
    pub pool: RuntimeGraphicsTexturePool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsTextureDirtyRegion {
    Clean,
    Rect(RuntimeRect),
    WholeTexture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeGraphicsSurfaceDescriptor {
    pub level: u32,
    pub width: u32,
    pub height: u32,
    pub color_format: RuntimeGraphicsColorFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeGraphicsVertexBufferDescriptor {
    pub byte_length: u32,
    pub usage: RuntimeGraphicsVertexBufferUsage,
    pub flexible_vertex_format: u32,
    pub pool: RuntimeGraphicsVertexBufferPool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeGraphicsVertexBufferBacking {
    pub address_allocation_id: ProcessAddressAllocationId,
    pub memory_owner: MemoryOwnerId,
    pub range: GuestRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeGraphicsTextureBacking {
    pub address_allocation_id: ProcessAddressAllocationId,
    pub memory_owner: MemoryOwnerId,
    pub range: GuestRange,
    pub row_pitch: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeGraphicsTextureLock {
    pub level: u32,
    pub rect: RuntimeRect,
    pub flags: u32,
    pub pitch: u32,
    pub guest_address: GuestAddress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeGraphicsTexturePixelSource {
    pub source_texture_id: RuntimeGraphicsTextureId,
    pub source_content_generation: u64,
    pub range: GuestRange,
    pub row_pitch: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeGraphicsSurfaceFill {
    pub rect: RuntimeRect,
    pub color_ref: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsTextureUpdate {
    pub source_texture_id: RuntimeGraphicsTextureId,
    pub source_content_generation: u64,
}

#[derive(Debug, Default)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub struct RuntimeGraphicsTextureContent {
    pub content_generation: u64,
    pub fills: Vec<RuntimeGraphicsSurfaceFill>,
    pub pixel_source: Option<RuntimeGraphicsTexturePixelSource>,
    pub(crate) last_update: Option<RuntimeGraphicsTextureUpdate>,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub struct RuntimeGraphicsDrawSubmission {
    pub vertex_data: Vec<u8>,
    pub texture_data: Vec<u8>,

    pub texture_width: u32,
    pub texture_height: u32,
    pub texture_fills: Vec<RuntimeGraphicsSurfaceFill>,
    pub texture_pitch: Option<u32>,
}

#[derive(Debug, Default)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub struct RuntimeGraphicsBackBuffer {
    pub clear_color: Option<u32>,
    pub draw_submissions: Vec<RuntimeGraphicsDrawSubmission>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsPresentMode {
    Discard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeGraphicsPresentation {
    pub width: u32,
    pub height: u32,
    pub color_format: RuntimeGraphicsColorFormat,
    pub back_buffer_count: u32,
    pub sample_count: u32,
    pub present_mode: RuntimeGraphicsPresentMode,
    pub windowed: bool,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub(crate) struct RuntimeGraphicsDevice {
    pub id: RuntimeGraphicsDeviceId,

    pub presentation: RuntimeGraphicsPresentation,
    pub com_object_id: RuntimeComObjectId,
    pub render_states: RuntimeGraphicsRenderStates,
    pub sampler_states: RuntimeGraphicsSamplerStates,
    pub texture_stage_states: RuntimeGraphicsTextureStageStates,
    pub texture_bindings: RuntimeGraphicsTextureBindings,
    pub flexible_vertex_format: Option<RuntimeGraphicsFlexibleVertexFormat>,
    pub stream_sources: RuntimeGraphicsStreamSources,
    pub back_buffer: RuntimeGraphicsBackBuffer,
    pub scene_active: bool,
    pub presented_frame: Option<RuntimeGraphicsBackBuffer>,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub(crate) struct RuntimeGraphicsTexture {
    pub id: RuntimeGraphicsTextureId,
    pub device_id: RuntimeGraphicsDeviceId,
    pub descriptor: RuntimeGraphicsTextureDescriptor,
    pub backing: Option<RuntimeGraphicsTextureBacking>,
    pub com_object_id: RuntimeComObjectId,
    pub active_lock: Option<RuntimeGraphicsTextureLock>,
    pub dirty_region: RuntimeGraphicsTextureDirtyRegion,
    pub content: RuntimeGraphicsTextureContent,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub(crate) struct RuntimeGraphicsSurface {
    pub(crate) id: RuntimeGraphicsSurfaceId,
    texture_id: RuntimeGraphicsTextureId,
    pub(crate) descriptor: RuntimeGraphicsSurfaceDescriptor,
    pub(crate) com_object_id: RuntimeComObjectId,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub(crate) struct RuntimeGraphicsVertexBuffer {
    pub id: RuntimeGraphicsVertexBufferId,
    pub device_id: RuntimeGraphicsDeviceId,
    pub descriptor: RuntimeGraphicsVertexBufferDescriptor,
    pub backing: RuntimeGraphicsVertexBufferBacking,
    pub com_object_id: RuntimeComObjectId,
    pub locked: bool,
    pub initialized: bool,
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
pub(crate) struct RuntimeGraphicsDeviceCreationRequest {
    pub(super) target: RuntimeGraphicsWindowTarget,
    pub(super) color_format: RuntimeGraphicsColorFormat,
    pub(super) back_buffer_count: u32,
    pub(super) sample_count: u32,
    pub(super) present_mode: RuntimeGraphicsPresentMode,
    pub(super) windowed: bool,
}

#[derive(Debug)]
struct RuntimeGraphicsDrawResolvedState<'a> {
    vertex_memory_owner: MemoryOwnerId,

    vertex_range: GuestRange,

    texture: &'a RuntimeGraphicsTexture,
}

impl RuntimeGraphicsDeviceCreationRequest {
    pub(crate) fn presentation(&self) -> RuntimeGraphicsPresentation {
        RuntimeGraphicsPresentation {
            width: self.target.client_width,
            height: self.target.client_height,
            color_format: self.color_format,
            back_buffer_count: self.back_buffer_count,
            sample_count: self.sample_count,
            present_mode: self.present_mode,
            windowed: self.windowed,
        }
    }
}

// Validated cleanup targets consumed synchronously after the root's memory transaction.
#[derive(Debug)]
pub(crate) struct RuntimeGraphicsComRetirement {
    device_associations: Vec<(RuntimeComObjectId, RuntimeGraphicsDeviceId)>,
    texture_associations: Vec<(RuntimeComObjectId, RuntimeGraphicsTextureId)>,
    surface_associations: Vec<(RuntimeComObjectId, RuntimeGraphicsSurfaceId)>,
    vertex_buffer_associations: Vec<(RuntimeComObjectId, RuntimeGraphicsVertexBufferId)>,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub(super) struct RuntimeGraphics {
    devices: BTreeMap<RuntimeGraphicsDeviceId, RuntimeGraphicsDevice>,
    device_com_object_index: BTreeMap<RuntimeComObjectId, RuntimeGraphicsDeviceId>,
    textures: BTreeMap<RuntimeGraphicsTextureId, RuntimeGraphicsTexture>,
    texture_com_object_index: BTreeMap<RuntimeComObjectId, RuntimeGraphicsTextureId>,
    device_texture_index: BTreeMap<RuntimeGraphicsDeviceId, BTreeSet<RuntimeGraphicsTextureId>>,
    surfaces: BTreeMap<RuntimeGraphicsSurfaceId, RuntimeGraphicsSurface>,
    surface_com_object_index: BTreeMap<RuntimeComObjectId, RuntimeGraphicsSurfaceId>,
    texture_surface_index: BTreeMap<RuntimeGraphicsTextureId, BTreeSet<RuntimeGraphicsSurfaceId>>,
    texture_level_surface_index:
        BTreeMap<(RuntimeGraphicsTextureId, u32), RuntimeGraphicsSurfaceId>,
    vertex_buffers: BTreeMap<RuntimeGraphicsVertexBufferId, RuntimeGraphicsVertexBuffer>,
    vertex_buffer_com_object_index: BTreeMap<RuntimeComObjectId, RuntimeGraphicsVertexBufferId>,
    device_vertex_buffer_index:
        BTreeMap<RuntimeGraphicsDeviceId, BTreeSet<RuntimeGraphicsVertexBufferId>>,
    next_device_id: u64,
    next_texture_id: u64,
    next_surface_id: u64,
    next_vertex_buffer_id: u64,
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
        }
    }
}

impl RuntimeGraphics {
    pub(super) fn validate_device_creation(
        &self,
        request: RuntimeGraphicsDeviceCreationRequest,
    ) -> Result<Option<RuntimeGraphicsDeviceCreationRequest>, RuntimeStateError> {
        if !valid_creation_request(request) {
            return Ok(None);
        }
        if self.devices.len() >= RUNTIME_GRAPHICS_DEVICE_LIMIT {
            return Err(RuntimeStateError::new(
                "logical graphics device capacity is exhausted",
            ));
        }
        let id = RuntimeGraphicsDeviceId(self.next_device_id);
        if id.0 == 0 || self.devices.contains_key(&id) {
            return Err(RuntimeStateError::new(
                "logical graphics device allocator selected a live identity",
            ));
        }
        id.0.checked_add(1).ok_or_else(|| {
            RuntimeStateError::new("logical graphics device identity space is exhausted")
        })?;

        Ok(Some(request))
    }

    // The caller validates creation before COM/guest-output publication. Those operations
    // do not mutate graphics, so no allocator snapshot or stale-plan check is needed.
    pub(super) fn insert_device(
        &mut self,
        request: RuntimeGraphicsDeviceCreationRequest,
        com_object_id: RuntimeComObjectId,
    ) -> RuntimeGraphicsDeviceId {
        debug_assert_ne!(com_object_id.0, 0);
        debug_assert!(!self.device_com_object_index.contains_key(&com_object_id));
        debug_assert!(!self.texture_com_object_index.contains_key(&com_object_id));
        debug_assert!(!self.surface_com_object_index.contains_key(&com_object_id));
        debug_assert!(
            !self
                .vertex_buffer_com_object_index
                .contains_key(&com_object_id)
        );
        let next_device_id = self
            .next_device_id
            .checked_add(1)
            .expect("device identity validated before publication");
        let presentation = request.presentation();
        let device = RuntimeGraphicsDevice {
            id: RuntimeGraphicsDeviceId(self.next_device_id),

            presentation,
            com_object_id,
            render_states: RuntimeGraphicsRenderStates::default(),
            sampler_states: RuntimeGraphicsSamplerStates::default(),
            texture_stage_states: RuntimeGraphicsTextureStageStates::default(),
            texture_bindings: RuntimeGraphicsTextureBindings::default(),
            flexible_vertex_format: None,
            stream_sources: RuntimeGraphicsStreamSources::default(),
            back_buffer: RuntimeGraphicsBackBuffer::default(),
            scene_active: false,
            presented_frame: None,
        };
        self.device_com_object_index
            .insert(com_object_id, device.id);
        self.device_texture_index.insert(device.id, BTreeSet::new());
        self.device_vertex_buffer_index
            .insert(device.id, BTreeSet::new());
        let id = device.id;
        self.devices.insert(id, device);
        self.next_device_id = next_device_id;

        id
    }

    pub(super) fn update_render_state(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        update: RuntimeGraphicsRenderStateUpdate,
    ) -> Result<(), RuntimeStateError> {
        let device_id = self
            .device_com_object_index
            .get(&device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical render-state update requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(device_com_object_id, device_id)?;
        let device = self.device(device_id)?;
        let previous = device.render_states.current_for(update);
        let changed = previous != Some(update);
        // All fallible association/overflow checks precede the first mutation.
        let device = self
            .devices
            .get_mut(&device_id)
            .expect("validated graphics device");
        if changed {
            device.render_states.apply(update);
        }
        Ok(())
    }

    pub(super) fn update_sampler_state(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        update: RuntimeGraphicsSamplerStateUpdate,
    ) -> Result<(), RuntimeStateError> {
        update.validate()?;
        let device_id = self
            .device_com_object_index
            .get(&device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical sampler-state update requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(device_com_object_id, device_id)?;
        let device = self.device(device_id)?;
        let previous = device.sampler_states.current_for(update);
        let changed = previous != Some(update);
        let device = self
            .devices
            .get_mut(&device_id)
            .expect("validated graphics device");
        if changed {
            device.sampler_states.apply(update);
        }
        Ok(())
    }

    pub(super) fn update_texture_stage_state(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        update: RuntimeGraphicsTextureStageStateUpdate,
    ) -> Result<(), RuntimeStateError> {
        update.validate()?;
        let device_id = self
            .device_com_object_index
            .get(&device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical texture-stage-state update requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(device_com_object_id, device_id)?;
        let device = self.device(device_id)?;
        let previous = device.texture_stage_states.current_for(update);
        let changed = previous != Some(update);
        let device = self
            .devices
            .get_mut(&device_id)
            .expect("validated graphics device");
        if changed {
            device.texture_stage_states.apply(update);
        }
        Ok(())
    }

    pub(super) fn bind_texture(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        stage: u32,
        texture_com_object_id: Option<RuntimeComObjectId>,
    ) -> Result<bool, RuntimeStateError> {
        if stage != 0 {
            return Ok(false);
        }
        let device_id = self
            .device_com_object_index
            .get(&device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical texture binding requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(device_com_object_id, device_id)?;
        let texture_id = if let Some(com_object_id) = texture_com_object_id {
            let texture = self.texture_for_com_object(com_object_id)?;
            if texture.active_lock.is_some() || texture.device_id != device_id {
                return Ok(false);
            }
            Some(texture.id)
        } else {
            None
        };
        let device = self.device(device_id)?;
        let previous_texture_id = device.texture_bindings.stage_zero;
        let changed = previous_texture_id != texture_id;
        let device = self
            .devices
            .get_mut(&device_id)
            .expect("validated device identity");
        if changed {
            device.texture_bindings.stage_zero = texture_id;
        }
        Ok(true)
    }

    pub(super) fn update_flexible_vertex_format(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        format: RuntimeGraphicsFlexibleVertexFormat,
    ) -> Result<(), RuntimeStateError> {
        let device_id = self
            .device_com_object_index
            .get(&device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new("logical flexible vertex format update requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(device_com_object_id, device_id)?;
        let device = self.device(device_id)?;
        let previous = device.flexible_vertex_format;
        let changed = previous != Some(format);
        let device = self
            .devices
            .get_mut(&device_id)
            .expect("validated graphics device");
        if changed {
            device.flexible_vertex_format = Some(format);
        }
        Ok(())
    }

    pub(super) fn bind_stream_source(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        stream: u32,
        vertex_buffer_com_object_id: RuntimeComObjectId,
        offset_in_bytes: u32,
        stride: u32,
    ) -> Result<bool, RuntimeStateError> {
        if stream != 0 || stride == 0 {
            return Ok(false);
        }
        let device_id = self
            .device_com_object_index
            .get(&device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical stream-source update requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(device_com_object_id, device_id)?;
        let Some(binding) = self.resolve_stream_source_binding(
            device_id,
            vertex_buffer_com_object_id,
            offset_in_bytes,
            stride,
        )?
        else {
            return Ok(false);
        };
        let device = self.device(device_id)?;
        let previous = device.stream_sources.stream_zero;
        let changed = previous != Some(binding);
        let device = self
            .devices
            .get_mut(&device_id)
            .expect("validated device identity");
        if changed {
            device.stream_sources.stream_zero = Some(binding);
        }
        Ok(true)
    }

    fn resolve_stream_source_binding(
        &self,
        device_id: RuntimeGraphicsDeviceId,
        vertex_buffer_com_object_id: RuntimeComObjectId,
        offset_in_bytes: u32,
        stride: u32,
    ) -> Result<Option<RuntimeGraphicsStreamSourceBinding>, RuntimeStateError> {
        let vertex_buffer_id = self
            .vertex_buffer_com_object_index
            .get(&vertex_buffer_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical stream-source binding requires a live vertex-buffer COM object",
                )
            })?;
        self.validate_vertex_buffer_association(vertex_buffer_com_object_id, vertex_buffer_id)?;
        let vertex_buffer = self
            .vertex_buffers
            .get(&vertex_buffer_id)
            .expect("validated logical vertex-buffer association disappeared");
        if vertex_buffer.device_id != device_id {
            return Ok(None);
        }
        Ok(Some(RuntimeGraphicsStreamSourceBinding {
            vertex_buffer_id,
            offset_in_bytes,
            stride,
        }))
    }

    pub(super) fn submit_draw(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        start_vertex: u32,
        primitive_count: u32,
        memory: &crate::memory::GuestMemory,
    ) -> Result<bool, RuntimeStateError> {
        if start_vertex != 0 || primitive_count != 2 {
            return Ok(false);
        }
        let device_id = self
            .device_com_object_index
            .get(&device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical draw submission requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(device_com_object_id, device_id)?;
        let device = self.device(device_id)?;
        let Some(resolved) = self.resolve_draw_state(device, start_vertex, primitive_count)? else {
            return Ok(false);
        };
        let texture_memory = resolved
            .texture
            .content
            .pixel_source
            .map(|pixels| {
                self.texture(pixels.source_texture_id).and_then(|source| {
                    let backing = source.backing.expect("resolved pixel source backing");
                    GuestRange::new(
                        backing.range.start(),
                        u64::from(backing.row_pitch) * u64::from(source.descriptor.height),
                    )
                    .map(|range| (range, backing.memory_owner))
                    .map_err(RuntimeStateError::from_memory)
                })
            })
            .transpose()?;
        let captured_bytes = device
            .back_buffer
            .draw_submissions
            .iter()
            .map(|draw| draw.texture_data.len() as u64)
            .sum::<u64>();
        if texture_memory.is_some_and(|(range, _)| {
            range.byte_length() > (16u64 * 1024 * 1024).saturating_sub(captured_bytes)
        }) {
            return Err(RuntimeStateError::new(
                "captured texture bytes exceed the bounded frame budget",
            ));
        }
        let expected_draw_submission_count = device.back_buffer.draw_submissions.len();
        if expected_draw_submission_count >= RUNTIME_GRAPHICS_DRAW_SUBMISSION_LIMIT {
            return Err(RuntimeStateError::new(
                "logical back-buffer draw-submission capacity is exhausted",
            ));
        }
        let vertex_byte_length =
            usize::try_from(resolved.vertex_range.byte_length()).map_err(|_| {
                RuntimeStateError::new(
                    "logical draw vertex byte length does not fit host accounting",
                )
            })?;

        let vertex_data = memory
            .read_owned(
                resolved.vertex_range.start(),
                vertex_byte_length,
                resolved.vertex_memory_owner,
            )
            .map_err(RuntimeStateError::from_memory)?;
        let texture_data = texture_memory
            .map(|(range, owner)| {
                memory
                    .read_owned(range.start(), range.byte_length() as usize, owner)
                    .map_err(RuntimeStateError::from_memory)
            })
            .transpose()?
            .unwrap_or_default();
        let submission = RuntimeGraphicsDrawSubmission {
            vertex_data,
            texture_data,

            texture_width: resolved.texture.descriptor.width,
            texture_height: resolved.texture.descriptor.height,
            texture_fills: resolved.texture.content.fills.clone(),
            texture_pitch: resolved
                .texture
                .content
                .pixel_source
                .map(|pixels| pixels.row_pitch),
        };
        let device = self
            .devices
            .get_mut(&device_id)
            .expect("validated device identity");
        device.back_buffer.draw_submissions.push(submission);

        Ok(true)
    }

    fn resolve_draw_state<'a>(
        &'a self,
        device: &RuntimeGraphicsDevice,
        start_vertex: u32,
        primitive_count: u32,
    ) -> Result<Option<RuntimeGraphicsDrawResolvedState<'a>>, RuntimeStateError> {
        if !supported_draw_device_state(device) {
            return Ok(None);
        }
        if !device.scene_active {
            return Ok(None);
        }
        let Some(stream_source) = device.stream_sources.stream_zero else {
            return Ok(None);
        };
        let vertex_buffer = self.vertex_buffer(stream_source.vertex_buffer_id)?;
        self.validate_vertex_buffer_association(vertex_buffer.com_object_id, vertex_buffer.id)?;
        if vertex_buffer.device_id != device.id
            || vertex_buffer.locked
            || !vertex_buffer.initialized
            || vertex_buffer.descriptor.byte_length != 112
            || vertex_buffer.descriptor.usage != RuntimeGraphicsVertexBufferUsage::WriteOnly
            || vertex_buffer.descriptor.flexible_vertex_format != 0x144
            || vertex_buffer.descriptor.pool != RuntimeGraphicsVertexBufferPool::Managed
            || stream_source.offset_in_bytes != 0
            || stream_source.stride != 28
        {
            return Ok(None);
        }
        let vertex_count = primitive_count
            .checked_add(2)
            .ok_or_else(|| RuntimeStateError::new("logical draw vertex count overflowed"))?;
        let start_byte = u64::from(stream_source.offset_in_bytes)
            .checked_add(u64::from(start_vertex) * u64::from(stream_source.stride))
            .ok_or_else(|| RuntimeStateError::new("logical draw vertex offset overflowed"))?;
        let vertex_byte_length = u64::from(vertex_count)
            .checked_mul(u64::from(stream_source.stride))
            .ok_or_else(|| RuntimeStateError::new("logical draw vertex byte length overflowed"))?;
        let end_byte = start_byte
            .checked_add(vertex_byte_length)
            .ok_or_else(|| RuntimeStateError::new("logical draw vertex range overflowed"))?;
        if end_byte > u64::from(vertex_buffer.descriptor.byte_length)
            || end_byte > vertex_buffer.backing.range.byte_length()
        {
            return Err(RuntimeStateError::new(
                "bound vertex backing is smaller than its checked draw range",
            ));
        }
        let vertex_address = u64::from(vertex_buffer.backing.range.start().0)
            .checked_add(start_byte)
            .and_then(|address| u32::try_from(address).ok())
            .map(GuestAddress)
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical draw vertex address exceeds the x86 guest address space",
                )
            })?;
        let vertex_range = GuestRange::new(vertex_address, vertex_byte_length)
            .map_err(RuntimeStateError::from_memory)?;
        let Some(texture_id) = device.texture_bindings.stage_zero else {
            return Ok(None);
        };
        let texture = self.texture(texture_id)?;
        self.validate_texture_association(texture.com_object_id, texture.id)?;
        if texture.device_id != device.id
            || texture.descriptor.levels != 1
            || texture.descriptor.pool != RuntimeGraphicsTexturePool::Default
            || texture.descriptor.color_format != RuntimeGraphicsColorFormat::B8g8r8x8Unorm
            || texture.active_lock.is_some()
            || texture.content.content_generation == 0
        {
            return Ok(None);
        }
        if let Some(pixel_source) = texture.content.pixel_source {
            let source = self.texture(pixel_source.source_texture_id)?;
            let backing = source.backing.ok_or_else(|| {
                RuntimeStateError::new(
                    "logical draw texture pixel source has no live guest-memory backing",
                )
            })?;
            if source.device_id != device.id
                || backing.range != pixel_source.range
                || backing.row_pitch != pixel_source.row_pitch
                || source.descriptor.width != texture.descriptor.width
                || source.descriptor.height != texture.descriptor.height
                || source.descriptor.levels != texture.descriptor.levels
                || source.descriptor.color_format != texture.descriptor.color_format
            {
                return Err(RuntimeStateError::new(
                    "draw texture source no longer matches its owned descriptor and backing",
                ));
            }
            if source.active_lock.is_some()
                || source.content.content_generation != pixel_source.source_content_generation
                || source.content.pixel_source != Some(pixel_source)
            {
                return Ok(None);
            }
        }
        Ok(Some(RuntimeGraphicsDrawResolvedState {
            vertex_memory_owner: vertex_buffer.backing.memory_owner,

            vertex_range,

            texture,
        }))
    }

    pub(super) fn clear_back_buffer(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        color_argb: u32,
    ) -> Result<(), RuntimeStateError> {
        let device_id = self
            .device_com_object_index
            .get(&device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical back-buffer clear requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(device_com_object_id, device_id)?;
        let device = self
            .devices
            .get_mut(&device_id)
            .expect("validated device identity");
        device.back_buffer.clear_color = Some(color_argb);
        device.back_buffer.draw_submissions.clear();

        Ok(())
    }

    pub(super) fn begin_scene(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
    ) -> Result<bool, RuntimeStateError> {
        let device_id = self
            .device_com_object_index
            .get(&device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical scene begin requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(device_com_object_id, device_id)?;
        let device = self.device(device_id)?;
        if device.scene_active {
            return Ok(false);
        }
        let device = self
            .devices
            .get_mut(&device_id)
            .expect("validated device identity");
        device.scene_active = true;

        Ok(true)
    }

    pub(super) fn end_scene(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
    ) -> Result<bool, RuntimeStateError> {
        let device_id = self
            .device_com_object_index
            .get(&device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical scene end requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(device_com_object_id, device_id)?;
        let device = self.device(device_id)?;
        if !device.scene_active {
            return Ok(false);
        }
        let device = self
            .devices
            .get_mut(&device_id)
            .expect("validated device identity");
        device.scene_active = false;

        Ok(true)
    }

    pub(super) fn present(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
    ) -> Result<bool, RuntimeStateError> {
        let device_id = self
            .device_com_object_index
            .get(&device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical presentation requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(device_com_object_id, device_id)?;
        let device = self.device(device_id)?;
        if device.scene_active {
            return Ok(false);
        }
        if device.back_buffer.clear_color.is_none()
            && device.back_buffer.draw_submissions.is_empty()
        {
            return Ok(false);
        }
        let device = self
            .devices
            .get_mut(&device_id)
            .expect("validated device identity");
        device.presented_frame = Some(std::mem::take(&mut device.back_buffer));

        Ok(true)
    }

    pub(super) fn validate_texture_creation(
        &self,
        device_com_object_id: RuntimeComObjectId,
        descriptor: RuntimeGraphicsTextureDescriptor,
    ) -> Result<
        (
            RuntimeGraphicsTextureId,
            RuntimeGraphicsDeviceId,
            RuntimeGraphicsTextureDescriptor,
        ),
        RuntimeStateError,
    > {
        if descriptor.width == 0 || descriptor.height == 0 || descriptor.levels == 0 {
            return Err(RuntimeStateError::new(
                "texture creation requires a nonzero descriptor",
            ));
        }
        texture_byte_layout(descriptor)?;
        let device_id = self
            .device_com_object_index
            .get(&device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical texture creation requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(device_com_object_id, device_id)?;
        if self.textures.len() >= RUNTIME_GRAPHICS_TEXTURE_LIMIT {
            return Err(RuntimeStateError::new(
                "logical graphics texture capacity is exhausted",
            ));
        }
        let id = RuntimeGraphicsTextureId(self.next_texture_id);
        if id.0 == 0 || self.textures.contains_key(&id) {
            return Err(RuntimeStateError::new(
                "logical graphics texture allocator selected a live identity",
            ));
        }
        if !self.device_texture_index.contains_key(&device_id) {
            return Err(RuntimeStateError::new(
                "live graphics device has no texture ownership index",
            ));
        }

        self.next_texture_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::new("graphics texture identity space is exhausted")
        })?;
        Ok((id, device_id, descriptor))
    }

    pub(super) fn insert_texture(
        &mut self,
        creation: (
            RuntimeGraphicsTextureId,
            RuntimeGraphicsDeviceId,
            RuntimeGraphicsTextureDescriptor,
        ),
        com_object_id: RuntimeComObjectId,
    ) -> RuntimeGraphicsTextureId {
        let (id, device_id, descriptor) = creation;
        debug_assert_ne!(com_object_id.0, 0);
        debug_assert!(!self.device_com_object_index.contains_key(&com_object_id));
        debug_assert!(!self.texture_com_object_index.contains_key(&com_object_id));
        debug_assert!(!self.surface_com_object_index.contains_key(&com_object_id));
        debug_assert!(
            !self
                .vertex_buffer_com_object_index
                .contains_key(&com_object_id)
        );
        let next_texture_id =
            id.0.checked_add(1)
                .expect("preflighted graphics texture ID must advance");
        let texture = RuntimeGraphicsTexture {
            id,
            device_id,
            descriptor,
            backing: None,
            com_object_id,
            active_lock: None,
            dirty_region: RuntimeGraphicsTextureDirtyRegion::WholeTexture,
            content: RuntimeGraphicsTextureContent::default(),
        };
        self.texture_com_object_index
            .insert(com_object_id, texture.id);
        self.device_texture_index
            .get_mut(&texture.device_id)
            .expect("preflighted graphics texture device disappeared")
            .insert(texture.id);
        self.texture_surface_index
            .insert(texture.id, BTreeSet::new());
        self.textures.insert(id, texture);
        self.next_texture_id = next_texture_id;

        id
    }

    pub(super) fn preview_texture_lock(
        &self,
        texture_com_object_id: RuntimeComObjectId,
        level: u32,
        rect: RuntimeRect,
        flags: u32,
        backing_installation: Option<GuestRange>,
    ) -> Result<Option<RuntimeGraphicsTextureLock>, RuntimeStateError> {
        let texture = self.texture_for_com_object(texture_com_object_id)?;
        if level != 0
            || flags
                & !(RUNTIME_GRAPHICS_TEXTURE_LOCK_DISCARD
                    | RUNTIME_GRAPHICS_TEXTURE_LOCK_NO_DIRTY_UPDATE)
                != 0
            || texture.descriptor.levels != 1
            || !matches!(
                (texture.descriptor.usage, texture.descriptor.pool),
                (
                    RuntimeGraphicsTextureUsage::None,
                    RuntimeGraphicsTexturePool::SystemMemory
                ) | (
                    RuntimeGraphicsTextureUsage::Dynamic,
                    RuntimeGraphicsTexturePool::Default
                )
            )
            || texture.descriptor.color_format != RuntimeGraphicsColorFormat::B8g8r8x8Unorm
        {
            return Ok(None);
        }
        if !valid_texture_dirty_rect(rect, texture.descriptor)? {
            return Ok(None);
        }
        if flags & RUNTIME_GRAPHICS_TEXTURE_LOCK_DISCARD != 0
            && (texture.descriptor.usage != RuntimeGraphicsTextureUsage::Dynamic
                || rect != full_texture_rect(texture.descriptor)?)
        {
            return Ok(None);
        }
        if texture.active_lock.is_some() {
            return Ok(None);
        }
        let (row_pitch, byte_length) = texture_byte_layout(texture.descriptor)?;
        let backing_start = match (texture.backing, backing_installation) {
            (Some(backing), None)
                if backing.address_allocation_id.0 != 0
                    && backing.memory_owner.0 != 0
                    && backing.row_pitch == row_pitch
                    && backing.range.byte_length() >= u64::from(byte_length) =>
            {
                backing.range.start()
            }
            (None, Some(range))
                if range.start().0 != 0 && range.byte_length() >= u64::from(byte_length) =>
            {
                range.start()
            }
            _ => {
                return Err(RuntimeStateError::new(
                    "texture lock has no unique valid texel backing",
                ));
            }
        };
        let byte_offset = u64::try_from(rect.top)
            .ok()
            .and_then(|top| top.checked_mul(u64::from(row_pitch)))
            .and_then(|offset| {
                u64::try_from(rect.left)
                    .ok()
                    .and_then(|left| left.checked_mul(4))
                    .and_then(|left| offset.checked_add(left))
            })
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical texture LockRect offset exceeds the x86 guest address space",
                )
            })?;
        let guest_address = u64::from(backing_start.0)
            .checked_add(byte_offset)
            .and_then(|address| u32::try_from(address).ok())
            .map(GuestAddress)
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical texture LockRect address exceeds the x86 guest address space",
                )
            })?;
        let lock = RuntimeGraphicsTextureLock {
            level,
            rect,
            flags,
            pitch: row_pitch,
            guest_address,
        };

        Ok(Some(lock))
    }

    #[cfg(test)]
    fn lock_texture(
        &mut self,
        texture_com_object_id: RuntimeComObjectId,
        level: u32,
        rect: RuntimeRect,
        flags: u32,
        backing_installation: Option<RuntimeGraphicsTextureBacking>,
    ) -> Result<Option<RuntimeGraphicsTextureLock>, RuntimeStateError> {
        if let Some(backing) = backing_installation
            && (backing.address_allocation_id.0 == 0
                || backing.memory_owner.0 == 0
                || backing.row_pitch
                    != texture_byte_layout(
                        self.texture_for_com_object(texture_com_object_id)?
                            .descriptor,
                    )?
                    .0)
        {
            return Err(RuntimeStateError::new(
                "texture backing requires a live allocation, memory owner and matching pitch",
            ));
        }
        let Some(lock) = self.preview_texture_lock(
            texture_com_object_id,
            level,
            rect,
            flags,
            backing_installation.map(|backing| backing.range),
        )?
        else {
            return Ok(None);
        };
        let id = self.texture_for_com_object(texture_com_object_id)?.id;
        self.publish_texture_lock(id, lock, backing_installation);
        Ok(Some(lock))
    }

    pub(super) fn publish_texture_lock(
        &mut self,
        id: RuntimeGraphicsTextureId,
        lock: RuntimeGraphicsTextureLock,
        backing_installation: Option<RuntimeGraphicsTextureBacking>,
    ) {
        let texture = self
            .textures
            .get_mut(&id)
            .expect("validated texture disappeared");
        if let Some(backing) = backing_installation {
            texture.backing = Some(backing);
        }
        if lock.flags & RUNTIME_GRAPHICS_TEXTURE_LOCK_NO_DIRTY_UPDATE == 0 {
            texture.dirty_region = merged_texture_dirty_region(
                texture.dirty_region,
                Some(lock.rect),
                texture.descriptor,
            )
            .expect("validated texture lock descriptor")
            .expect("validated texture lock rectangle");
        }
        // Draw submissions own their texel bytes. DISCARD may reuse this backing:
        // previous contents are undefined and cannot change submitted frames.
        texture.active_lock = Some(lock);
    }

    pub(super) fn unlock_texture(
        &mut self,
        com_object_id: RuntimeComObjectId,
    ) -> Result<bool, RuntimeStateError> {
        let texture = self.texture_for_com_object(com_object_id)?;
        if texture.active_lock.is_none() {
            return Ok(false);
        }
        let id = texture.id;
        let backing = texture.backing.ok_or_else(|| {
            RuntimeStateError::new(
                "logical graphics texture has an active lock without texel backing",
            )
        })?;
        let next_content_generation = texture
            .content
            .content_generation
            .checked_add(1)
            .ok_or_else(|| {
                RuntimeStateError::new("logical graphics texture content generation is exhausted")
            })?;
        let pixel_source = RuntimeGraphicsTexturePixelSource {
            source_texture_id: id,
            source_content_generation: next_content_generation,
            range: backing.range,
            row_pitch: backing.row_pitch,
        };
        let texture = self.textures.get_mut(&id).expect("validated live texture");
        texture.active_lock = None;
        texture.content.content_generation = next_content_generation;
        texture.content.fills.clear();
        texture.content.pixel_source = Some(pixel_source);
        texture.content.last_update = None;

        Ok(true)
    }

    pub(super) fn mark_texture_dirty_region(
        &mut self,
        texture_com_object_id: RuntimeComObjectId,
        rect: Option<RuntimeRect>,
    ) -> Result<bool, RuntimeStateError> {
        let texture_id = self
            .texture_com_object_index
            .get(&texture_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical texture dirty-region mark requires a live texture COM object",
                )
            })?;
        self.validate_texture_association(texture_com_object_id, texture_id)?;
        let texture = self
            .textures
            .get(&texture_id)
            .expect("validated logical texture association disappeared");
        if texture.active_lock.is_some() {
            return Ok(false);
        }
        let Some(dirty_region) =
            merged_texture_dirty_region(texture.dirty_region, rect, texture.descriptor)?
        else {
            return Ok(false);
        };
        let changed = dirty_region != texture.dirty_region;
        // All fallible checks precede the first mutation; there is no guest
        // callback or browser round-trip between validation and publication.
        let texture = self
            .textures
            .get_mut(&texture_id)
            .expect("validated texture identity");
        if changed {
            texture.dirty_region = dirty_region;
        }
        Ok(true)
    }

    pub(super) fn update_texture(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        source_texture_com_object_id: RuntimeComObjectId,
        destination_texture_com_object_id: RuntimeComObjectId,
    ) -> Result<bool, RuntimeStateError> {
        let device_id = self
            .device_com_object_index
            .get(&device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical texture update requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(device_com_object_id, device_id)?;
        let source_texture_id = self
            .texture_com_object_index
            .get(&source_texture_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical texture update requires a live source texture COM object",
                )
            })?;
        let destination_texture_id = self
            .texture_com_object_index
            .get(&destination_texture_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical texture update requires a live destination texture COM object",
                )
            })?;
        self.validate_texture_association(source_texture_com_object_id, source_texture_id)?;
        self.validate_texture_association(
            destination_texture_com_object_id,
            destination_texture_id,
        )?;
        let source = self
            .textures
            .get(&source_texture_id)
            .expect("validated source texture association disappeared");
        let destination = self
            .textures
            .get(&destination_texture_id)
            .expect("validated destination texture association disappeared");
        if !validate_texture_update_pair(device_id, source, destination)?
            || !validate_partial_texture_update_lineage(source, destination)?
        {
            return Ok(false);
        }
        if source.dirty_region == RuntimeGraphicsTextureDirtyRegion::Clean {
            return Ok(true);
        }
        let next_content_generation = destination
            .content
            .content_generation
            .checked_add(1)
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical destination texture content generation is exhausted",
                )
            })?;
        let update = RuntimeGraphicsTextureUpdate {
            source_texture_id,
            source_content_generation: source.content.content_generation,
        };
        let source_fills = source.content.fills.clone();
        let source_pixel_source = source.content.pixel_source;
        let source = self
            .textures
            .get_mut(&source_texture_id)
            .expect("validated source");
        source.dirty_region = RuntimeGraphicsTextureDirtyRegion::Clean;

        let destination = self
            .textures
            .get_mut(&destination_texture_id)
            .expect("validated destination");
        destination.content.fills = source_fills;
        destination.content.pixel_source = source_pixel_source;
        destination.content.content_generation = next_content_generation;
        destination.content.last_update = Some(update);

        Ok(true)
    }

    pub(super) fn validate_vertex_buffer_creation(
        &self,
        device_com_object_id: RuntimeComObjectId,
        descriptor: RuntimeGraphicsVertexBufferDescriptor,
        address_allocation_id: ProcessAddressAllocationId,
        backing_range: GuestRange,
    ) -> Result<
        (
            RuntimeGraphicsVertexBufferId,
            RuntimeGraphicsDeviceId,
            RuntimeGraphicsVertexBufferDescriptor,
        ),
        RuntimeStateError,
    > {
        if descriptor.byte_length == 0 || descriptor.flexible_vertex_format == 0 {
            return Err(RuntimeStateError::new(
                "vertex-buffer creation requires a nonzero descriptor",
            ));
        }
        let device_id = self
            .device_com_object_index
            .get(&device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical vertex-buffer creation requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(device_com_object_id, device_id)?;
        if address_allocation_id.0 == 0
            || backing_range.byte_length() < u64::from(descriptor.byte_length)
        {
            return Err(RuntimeStateError::new(
                "logical vertex-buffer backing must identify sufficient guest memory",
            ));
        }
        if self.vertex_buffers.len() >= RUNTIME_GRAPHICS_VERTEX_BUFFER_LIMIT {
            return Err(RuntimeStateError::new(
                "logical graphics vertex-buffer capacity is exhausted",
            ));
        }
        if self.vertex_buffers.values().any(|vertex_buffer| {
            vertex_buffer.backing.address_allocation_id == address_allocation_id
                || vertex_buffer.backing.range == backing_range
        }) {
            return Err(RuntimeStateError::new(
                "logical vertex-buffer allocator selected live guest backing",
            ));
        }
        let id = RuntimeGraphicsVertexBufferId(self.next_vertex_buffer_id);
        if id.0 == 0 || self.vertex_buffers.contains_key(&id) {
            return Err(RuntimeStateError::new(
                "logical graphics vertex-buffer allocator selected a live identity",
            ));
        }
        if !self.device_vertex_buffer_index.contains_key(&device_id) {
            return Err(RuntimeStateError::new(
                "live device has no vertex-buffer ownership index",
            ));
        }

        self.next_vertex_buffer_id
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("vertex-buffer identity space is exhausted"))?;
        Ok((id, device_id, descriptor))
    }

    pub(super) fn insert_vertex_buffer(
        &mut self,
        creation: (
            RuntimeGraphicsVertexBufferId,
            RuntimeGraphicsDeviceId,
            RuntimeGraphicsVertexBufferDescriptor,
        ),
        com_object_id: RuntimeComObjectId,
        memory_owner: MemoryOwnerId,
        address_allocation_id: ProcessAddressAllocationId,
        backing_range: GuestRange,
    ) -> RuntimeGraphicsVertexBufferId {
        let (id, device_id, descriptor) = creation;
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
        let next_vertex_buffer_id =
            id.0.checked_add(1)
                .expect("preflighted graphics vertex-buffer ID must advance");
        let vertex_buffer = RuntimeGraphicsVertexBuffer {
            id,
            device_id,
            descriptor,
            backing: RuntimeGraphicsVertexBufferBacking {
                address_allocation_id,
                memory_owner,
                range: backing_range,
            },
            com_object_id,
            locked: false,
            initialized: false,
        };
        self.vertex_buffer_com_object_index
            .insert(com_object_id, vertex_buffer.id);
        self.device_vertex_buffer_index
            .get_mut(&vertex_buffer.device_id)
            .expect("preflighted graphics vertex-buffer device disappeared")
            .insert(vertex_buffer.id);
        self.vertex_buffers.insert(id, vertex_buffer);
        self.next_vertex_buffer_id = next_vertex_buffer_id;

        id
    }

    pub(super) fn preview_vertex_buffer_lock(
        &self,
        vertex_buffer_com_object_id: RuntimeComObjectId,
        offset: u32,
        byte_length: u32,
        flags: u32,
    ) -> Result<Option<GuestAddress>, RuntimeStateError> {
        let vertex_buffer = self.vertex_buffer_for_com_object(vertex_buffer_com_object_id)?;
        if offset != 0 || byte_length != 0 || flags != 0 || vertex_buffer.locked {
            return Ok(None);
        }

        Ok(Some(vertex_buffer.backing.range.start()))
    }

    #[cfg(test)]
    fn lock_vertex_buffer(
        &mut self,
        vertex_buffer_com_object_id: RuntimeComObjectId,
        offset: u32,
        byte_length: u32,
        flags: u32,
    ) -> Result<Option<GuestAddress>, RuntimeStateError> {
        let Some(lock) = self.preview_vertex_buffer_lock(
            vertex_buffer_com_object_id,
            offset,
            byte_length,
            flags,
        )?
        else {
            return Ok(None);
        };
        let vertex_buffer = self.vertex_buffer_for_com_object(vertex_buffer_com_object_id)?;
        let id = vertex_buffer.id;
        self.publish_vertex_buffer_lock(id);
        Ok(Some(lock))
    }

    pub(super) fn publish_vertex_buffer_lock(&mut self, id: RuntimeGraphicsVertexBufferId) {
        let vertex_buffer = self
            .vertex_buffers
            .get_mut(&id)
            .expect("validated live vertex buffer");
        vertex_buffer.locked = true;
    }

    pub(super) fn unlock_vertex_buffer(
        &mut self,
        com_object_id: RuntimeComObjectId,
    ) -> Result<bool, RuntimeStateError> {
        let vertex_buffer = self.vertex_buffer_for_com_object(com_object_id)?;
        if !vertex_buffer.locked {
            return Ok(false);
        }
        let id = vertex_buffer.id;
        let vertex_buffer = self
            .vertex_buffers
            .get_mut(&id)
            .expect("validated live vertex buffer");
        vertex_buffer.locked = false;
        vertex_buffer.initialized = true;

        Ok(true)
    }

    pub(super) fn validate_surface_creation(
        &self,
        texture_com_object_id: RuntimeComObjectId,
        level: u32,
    ) -> Result<
        Option<(
            RuntimeGraphicsSurfaceId,
            RuntimeGraphicsTextureId,
            RuntimeGraphicsSurfaceDescriptor,
        )>,
        RuntimeStateError,
    > {
        let texture_id = self
            .texture_com_object_index
            .get(&texture_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical surface creation requires a live graphics texture COM object",
                )
            })?;
        self.validate_texture_association(texture_com_object_id, texture_id)?;
        let texture = self
            .textures
            .get(&texture_id)
            .expect("validated logical texture association disappeared");
        if level >= texture.descriptor.levels {
            return Ok(None);
        }
        if self
            .texture_level_surface_index
            .contains_key(&(texture_id, level))
        {
            return Err(RuntimeStateError::new(
                "logical texture level already has a live surface identity",
            ));
        }
        if self.surfaces.len() >= RUNTIME_GRAPHICS_SURFACE_LIMIT {
            return Err(RuntimeStateError::new(
                "logical graphics surface capacity is exhausted",
            ));
        }
        let id = RuntimeGraphicsSurfaceId(self.next_surface_id);
        if id.0 == 0 || self.surfaces.contains_key(&id) {
            return Err(RuntimeStateError::new(
                "logical graphics surface allocator selected a live identity",
            ));
        }
        if !self.texture_surface_index.contains_key(&texture_id) {
            return Err(RuntimeStateError::new(
                "live texture has no surface ownership index",
            ));
        }
        let descriptor = RuntimeGraphicsSurfaceDescriptor {
            level,
            width: mip_dimension(texture.descriptor.width, level),
            height: mip_dimension(texture.descriptor.height, level),
            color_format: texture.descriptor.color_format,
        };

        self.next_surface_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::new("graphics surface identity space is exhausted")
        })?;
        Ok(Some((id, texture_id, descriptor)))
    }

    pub(super) fn insert_surface(
        &mut self,
        creation: (
            RuntimeGraphicsSurfaceId,
            RuntimeGraphicsTextureId,
            RuntimeGraphicsSurfaceDescriptor,
        ),
        com_object_id: RuntimeComObjectId,
    ) -> RuntimeGraphicsSurfaceId {
        let (id, texture_id, descriptor) = creation;
        debug_assert_ne!(com_object_id.0, 0);
        debug_assert!(!self.device_com_object_index.contains_key(&com_object_id));
        debug_assert!(!self.texture_com_object_index.contains_key(&com_object_id));
        debug_assert!(!self.surface_com_object_index.contains_key(&com_object_id));
        debug_assert!(
            !self
                .vertex_buffer_com_object_index
                .contains_key(&com_object_id)
        );
        let next_surface_id =
            id.0.checked_add(1)
                .expect("preflighted graphics surface ID must advance");
        let surface = RuntimeGraphicsSurface {
            id,
            texture_id,
            descriptor,
            com_object_id,
        };
        self.surface_com_object_index
            .insert(com_object_id, surface.id);
        self.texture_surface_index
            .get_mut(&surface.texture_id)
            .expect("preflighted graphics surface texture disappeared")
            .insert(surface.id);
        self.texture_level_surface_index
            .insert((surface.texture_id, surface.descriptor.level), surface.id);
        self.surfaces.insert(id, surface);
        self.next_surface_id = next_surface_id;

        id
    }

    pub(crate) fn fill_surface(
        &mut self,
        surface_com_object_id: RuntimeComObjectId,
        rect: RuntimeRect,
        color_ref: u32,
    ) -> Result<(), RuntimeStateError> {
        if surface_com_object_id.0 == 0 || color_ref > 0x00ff_ffff {
            return Err(RuntimeStateError::new(
                "logical graphics surface fill requires a live surface and a Win32 COLORREF",
            ));
        }
        let surface_id = self
            .surface_com_object_index
            .get(&surface_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical surface fill requires a live graphics surface COM object",
                )
            })?;
        self.validate_surface_association(surface_com_object_id, surface_id)?;
        let surface = self
            .surfaces
            .get(&surface_id)
            .expect("validated logical surface association disappeared");
        validate_surface_fill_rect(rect, surface.descriptor)?;
        if surface.descriptor.level != 0 {
            return Err(RuntimeStateError::new(
                "logical graphics surface fills currently require a level-zero texture view",
            ));
        }
        let texture_id = surface.texture_id;
        let texture = self.textures.get(&texture_id).ok_or_else(|| {
            RuntimeStateError::new("logical graphics surface fill has no live texture authority")
        })?;
        if texture.active_lock.is_some() || texture.content.pixel_source.is_some() {
            return Err(RuntimeStateError::new(
                "logical surface fill cannot overlap an active or guest-backed texture content path",
            ));
        }
        if rect.left == rect.right || rect.top == rect.bottom {
            return Ok(());
        }
        if texture.content.fills.len() >= RUNTIME_GRAPHICS_SURFACE_FILL_LIMIT {
            return Err(RuntimeStateError::new(
                "logical graphics texture fill history capacity is exhausted",
            ));
        }
        let next_dirty_region =
            merged_texture_dirty_region(texture.dirty_region, Some(rect), texture.descriptor)?
                .ok_or_else(|| {
                    RuntimeStateError::new(
                        "validated surface fill produced an invalid dirty rectangle",
                    )
                })?;
        let next_content_generation = texture
            .content
            .content_generation
            .checked_add(1)
            .ok_or_else(|| {
                RuntimeStateError::new("logical graphics texture content generation is exhausted")
            })?;
        // No browser, callback, or second owner mutates between these checks and publication.
        let texture = self
            .textures
            .get_mut(&texture_id)
            .expect("validated texture");
        texture
            .content
            .fills
            .push(RuntimeGraphicsSurfaceFill { rect, color_ref });
        texture.content.content_generation = next_content_generation;
        texture.dirty_region = next_dirty_region;

        Ok(())
    }

    pub(crate) fn validate_com_object_retirements(
        &self,
        com_object_ids: &BTreeSet<RuntimeComObjectId>,
    ) -> Result<Option<RuntimeGraphicsComRetirement>, RuntimeStateError> {
        // Do not interpret a missing reverse COM index as an unrelated resource.
        for (id, resource) in &self.devices {
            if com_object_ids.contains(&resource.com_object_id)
                && (resource.id != *id
                    || self.device_com_object_index.get(&resource.com_object_id) != Some(id))
            {
                return Err(RuntimeStateError::new(
                    "retiring graphics device disagrees with its live identity or COM index",
                ));
            }
        }
        for (id, resource) in &self.textures {
            if com_object_ids.contains(&resource.com_object_id)
                && (resource.id != *id
                    || self.texture_com_object_index.get(&resource.com_object_id) != Some(id))
            {
                return Err(RuntimeStateError::new(
                    "retiring graphics texture disagrees with its live identity or COM index",
                ));
            }
        }
        for (id, resource) in &self.surfaces {
            if com_object_ids.contains(&resource.com_object_id)
                && (resource.id != *id
                    || self.surface_com_object_index.get(&resource.com_object_id) != Some(id))
            {
                return Err(RuntimeStateError::new(
                    "retiring graphics surface disagrees with its live identity or COM index",
                ));
            }
        }
        for (id, resource) in &self.vertex_buffers {
            if com_object_ids.contains(&resource.com_object_id)
                && (resource.id != *id
                    || self
                        .vertex_buffer_com_object_index
                        .get(&resource.com_object_id)
                        != Some(id))
            {
                return Err(RuntimeStateError::new(
                    "retiring graphics vertex-buffer disagrees with its live identity or COM index",
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
        Ok(Some(RuntimeGraphicsComRetirement {
            device_associations,
            texture_associations,
            surface_associations,
            vertex_buffer_associations,
        }))
    }

    pub(crate) fn vertex_buffer_backings_for_retirement(
        &self,
        plan: &RuntimeGraphicsComRetirement,
    ) -> Result<Vec<(RuntimeGraphicsVertexBufferBacking, u32)>, RuntimeStateError> {
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

    pub(crate) fn texture_backings_for_retirement(
        &self,
        plan: &RuntimeGraphicsComRetirement,
    ) -> Result<Vec<(RuntimeGraphicsTextureBacking, u32)>, RuntimeStateError> {
        plan.texture_associations
            .iter()
            .filter_map(|(_, texture_id)| {
                let texture = self
                    .textures
                    .get(texture_id)
                    .expect("validated retiring graphics texture disappeared");
                texture.backing.map(|backing| {
                    texture_byte_layout(texture.descriptor)
                        .map(|(_, byte_length)| (backing, byte_length))
                })
            })
            .collect()
    }

    pub(crate) fn remove_validated_com_objects(&mut self, plan: RuntimeGraphicsComRetirement) {
        for (com_object_id, surface_id) in plan.surface_associations {
            self.surface_com_object_index.remove(&com_object_id);
            let surface = self
                .surfaces
                .remove(&surface_id)
                .expect("validated logical surface disappeared during retirement");
            self.texture_surface_index
                .get_mut(&surface.texture_id)
                .expect("validated logical surface texture disappeared during retirement")
                .remove(&surface_id);
            self.texture_level_surface_index
                .remove(&(surface.texture_id, surface.descriptor.level));
        }
        for (com_object_id, texture_id) in plan.texture_associations {
            self.texture_com_object_index.remove(&com_object_id);
            let texture = self
                .textures
                .remove(&texture_id)
                .expect("validated logical texture disappeared during retirement");
            let device = self
                .devices
                .get_mut(&texture.device_id)
                .expect("validated texture device disappeared during retirement");
            if device.texture_bindings.stage_zero == Some(texture_id) {
                device.texture_bindings.stage_zero = None;
            }
            self.device_texture_index
                .get_mut(&texture.device_id)
                .expect("validated logical texture device disappeared during retirement")
                .remove(&texture_id);
            self.texture_surface_index.remove(&texture_id);
        }
        for (com_object_id, vertex_buffer_id) in plan.vertex_buffer_associations {
            self.vertex_buffer_com_object_index.remove(&com_object_id);
            let vertex_buffer = self
                .vertex_buffers
                .remove(&vertex_buffer_id)
                .expect("validated logical vertex buffer disappeared during retirement");
            let device = self
                .devices
                .get_mut(&vertex_buffer.device_id)
                .expect("validated vertex-buffer device disappeared during retirement");
            if device
                .stream_sources
                .stream_zero
                .is_some_and(|binding| binding.vertex_buffer_id == vertex_buffer_id)
            {
                device.stream_sources.stream_zero = None;
            }
            self.device_vertex_buffer_index
                .get_mut(&vertex_buffer.device_id)
                .expect("validated logical vertex-buffer device disappeared during retirement")
                .remove(&vertex_buffer_id);
        }
        for (com_object_id, device_id) in plan.device_associations {
            self.device_com_object_index.remove(&com_object_id);
            self.device_texture_index.remove(&device_id);
            self.device_vertex_buffer_index.remove(&device_id);
            self.devices.remove(&device_id);
        }
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
            return Err(RuntimeStateError::new(
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
                RuntimeStateError::new("logical graphics texture has no surface relation index")
            })?;
            if !children.is_subset(&retiring_surfaces) {
                return Err(RuntimeStateError::new(
                    "logical graphics texture cannot retire while a child surface remains live",
                ));
            }
        }
        let retiring_textures = texture_associations
            .iter()
            .map(|(_, texture_id)| *texture_id)
            .collect::<BTreeSet<_>>();
        for texture in self.textures.values() {
            if retiring_textures.contains(&texture.id) {
                continue;
            }
            if texture
                .content
                .pixel_source
                .is_some_and(|source| retiring_textures.contains(&source.source_texture_id))
            {
                return Err(RuntimeStateError::new(
                    "logical texture backing cannot retire while live texture content still references it",
                ));
            }
        }
        let retiring_vertex_buffers = vertex_buffer_associations
            .iter()
            .map(|(_, vertex_buffer_id)| *vertex_buffer_id)
            .collect::<BTreeSet<_>>();
        for (_, device_id) in device_associations {
            let children = self.device_texture_index.get(device_id).ok_or_else(|| {
                RuntimeStateError::new("logical graphics device has no texture relation index")
            })?;
            if !children.is_subset(&retiring_textures) {
                return Err(RuntimeStateError::new(
                    "logical graphics device cannot retire while a child texture remains live",
                ));
            }
            let vertex_buffers =
                self.device_vertex_buffer_index
                    .get(device_id)
                    .ok_or_else(|| {
                        RuntimeStateError::new(
                            "logical graphics device has no vertex-buffer relation index",
                        )
                    })?;
            if !vertex_buffers.is_subset(&retiring_vertex_buffers) {
                return Err(RuntimeStateError::new(
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
            return Err(RuntimeStateError::new(
                "graphics device COM association does not resolve to a live logical device",
            ));
        }
        if self
            .devices
            .get(&device_id)
            .map(|device| (device.id, device.com_object_id))
            != Some((device_id, com_object_id))
        {
            return Err(RuntimeStateError::new(
                "logical graphics device and COM index disagree",
            ));
        }
        if !self.device_texture_index.contains_key(&device_id)
            || !self.device_vertex_buffer_index.contains_key(&device_id)
        {
            return Err(RuntimeStateError::new(
                "logical graphics device is missing a resource relation index",
            ));
        }
        if let Some(texture_id) = self
            .devices
            .get(&device_id)
            .and_then(|device| device.texture_bindings.stage_zero)
            && self
                .textures
                .get(&texture_id)
                .is_none_or(|texture| texture.device_id != device_id)
        {
            return Err(RuntimeStateError::new(
                "logical graphics device texture binding does not resolve to a live owned texture",
            ));
        }
        if let Some(binding) = self
            .devices
            .get(&device_id)
            .and_then(|device| device.stream_sources.stream_zero)
            && self
                .vertex_buffers
                .get(&binding.vertex_buffer_id)
                .is_none_or(|vertex_buffer| vertex_buffer.device_id != device_id)
        {
            return Err(RuntimeStateError::new(
                "logical graphics device stream source does not resolve to a live owned vertex buffer",
            ));
        }
        Ok(())
    }

    pub(super) fn device_by_com_object(
        &self,
        com_object_id: RuntimeComObjectId,
    ) -> Result<&RuntimeGraphicsDevice, RuntimeStateError> {
        let device_id = self
            .device_com_object_index
            .get(&com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical graphics device query requires a live device COM object",
                )
            })?;
        self.validate_device_association(com_object_id, device_id)?;
        self.device(device_id)
    }

    fn validate_texture_association(
        &self,
        com_object_id: RuntimeComObjectId,
        texture_id: RuntimeGraphicsTextureId,
    ) -> Result<(), RuntimeStateError> {
        if self.texture_com_object_index.get(&com_object_id) != Some(&texture_id) {
            return Err(RuntimeStateError::new(
                "graphics texture COM association does not resolve to a live logical texture",
            ));
        }
        let texture = self.textures.get(&texture_id).ok_or_else(|| {
            RuntimeStateError::new("logical graphics texture and COM index disagree")
        })?;
        if texture.id != texture_id
            || texture.com_object_id != com_object_id
            || !self
                .device_texture_index
                .get(&texture.device_id)
                .is_some_and(|textures| textures.contains(&texture_id))
            || !self.texture_surface_index.contains_key(&texture_id)
        {
            return Err(RuntimeStateError::new(
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
            return Err(RuntimeStateError::new(
                "graphics surface COM association does not resolve to a live logical surface",
            ));
        }
        let surface = self.surfaces.get(&surface_id).ok_or_else(|| {
            RuntimeStateError::new("logical graphics surface and COM index disagree")
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
            return Err(RuntimeStateError::new(
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
            return Err(RuntimeStateError::new(
                "graphics vertex-buffer COM association does not resolve to a live logical buffer",
            ));
        }
        let vertex_buffer = self.vertex_buffers.get(&vertex_buffer_id).ok_or_else(|| {
            RuntimeStateError::new("logical graphics vertex buffer and COM index disagree")
        })?;
        if vertex_buffer.id != vertex_buffer_id
            || vertex_buffer.com_object_id != com_object_id
            || !self
                .device_vertex_buffer_index
                .get(&vertex_buffer.device_id)
                .is_some_and(|vertex_buffers| vertex_buffers.contains(&vertex_buffer_id))
        {
            return Err(RuntimeStateError::new(
                "logical graphics vertex buffer, device relation, and COM index disagree",
            ));
        }
        Ok(())
    }

    pub fn device(
        &self,
        id: RuntimeGraphicsDeviceId,
    ) -> Result<&RuntimeGraphicsDevice, RuntimeStateError> {
        self.devices.get(&id).ok_or_else(|| {
            RuntimeStateError::new(format!("logical graphics device {} is not live", id.0))
        })
    }

    #[cfg(test)]
    pub(super) fn devices(&self) -> &BTreeMap<RuntimeGraphicsDeviceId, RuntimeGraphicsDevice> {
        &self.devices
    }

    pub fn texture(
        &self,
        id: RuntimeGraphicsTextureId,
    ) -> Result<&RuntimeGraphicsTexture, RuntimeStateError> {
        self.textures.get(&id).ok_or_else(|| {
            RuntimeStateError::new(format!("logical graphics texture {} is not live", id.0))
        })
    }

    #[cfg(test)]
    pub(super) fn textures(&self) -> &BTreeMap<RuntimeGraphicsTextureId, RuntimeGraphicsTexture> {
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
                RuntimeStateError::new("live texture COM object has no logical graphics texture")
            })?;
        self.validate_texture_association(com_object_id, texture_id)?;
        Ok(self
            .textures
            .get(&texture_id)
            .expect("validated logical texture association disappeared"))
    }

    fn surface(
        &self,
        id: RuntimeGraphicsSurfaceId,
    ) -> Result<&RuntimeGraphicsSurface, RuntimeStateError> {
        self.surfaces.get(&id).ok_or_else(|| {
            RuntimeStateError::new(format!("logical graphics surface {} is not live", id.0))
        })
    }

    #[cfg(test)]
    fn surfaces(&self) -> &BTreeMap<RuntimeGraphicsSurfaceId, RuntimeGraphicsSurface> {
        &self.surfaces
    }

    pub fn vertex_buffer(
        &self,
        id: RuntimeGraphicsVertexBufferId,
    ) -> Result<&RuntimeGraphicsVertexBuffer, RuntimeStateError> {
        self.vertex_buffers.get(&id).ok_or_else(|| {
            RuntimeStateError::new(format!(
                "logical graphics vertex buffer {} is not live",
                id.0
            ))
        })
    }

    #[cfg(test)]
    pub(super) fn vertex_buffers(
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
                RuntimeStateError::new("logical graphics vertex-buffer COM object is not live")
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
                RuntimeStateError::new("logical graphics surface COM object is not live")
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
            return Ok(None);
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

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.devices.is_empty()
            && self.textures.is_empty()
            && self.surfaces.is_empty()
            && self.vertex_buffers.is_empty()
    }
}

fn valid_creation_request(request: RuntimeGraphicsDeviceCreationRequest) -> bool {
    !(request.target.window_id.0 == 0
        || request.target.window_handle == 0
        || request.target.owner_thread_id.0 == 0
        || request.target.client_width == 0
        || request.target.client_height == 0
        || request.target.client_width > i32::MAX as u32
        || request.target.client_height > i32::MAX as u32
        || request.back_buffer_count == 0
        || request.sample_count == 0
        || !request.windowed)
}

fn supported_draw_device_state(device: &RuntimeGraphicsDevice) -> bool {
    let supported_render_states = RuntimeGraphicsRenderStates {
        cull_mode: Some(RuntimeGraphicsCullMode::None),
        depth_test_enabled: Some(false),
        lighting_enabled: Some(false),
        alpha_blend_enabled: Some(true),
        source_blend_factor: Some(RuntimeGraphicsBlendFactor::SourceAlpha),
        destination_blend_factor: Some(RuntimeGraphicsBlendFactor::InverseSourceAlpha),
        blend_operation: Some(RuntimeGraphicsBlendOperation::Add),
    };
    let supported_sampler_states = RuntimeGraphicsSamplerStates {
        sampler_zero: RuntimeGraphicsSamplerState {
            min_filter: Some(RuntimeGraphicsTextureFilter::Linear),
            mag_filter: Some(RuntimeGraphicsTextureFilter::Linear),
        },
    };
    let supported_texture_stage_states = RuntimeGraphicsTextureStageStates {
        stage_zero: RuntimeGraphicsTextureStageState {
            color_operation: Some(RuntimeGraphicsTextureOperation::Modulate),
            color_argument_one: Some(RuntimeGraphicsTextureArgument::Texture),
            color_argument_two: Some(RuntimeGraphicsTextureArgument::Diffuse),
            alpha_operation: Some(RuntimeGraphicsTextureOperation::SelectArgumentOne),
            alpha_argument_one: Some(RuntimeGraphicsTextureArgument::Texture),
            alpha_argument_two: Some(RuntimeGraphicsTextureArgument::Diffuse),
        },
    };
    !(device.render_states != supported_render_states
        || device.sampler_states != supported_sampler_states
        || device.texture_stage_states != supported_texture_stage_states
        || device.flexible_vertex_format
            != Some(RuntimeGraphicsFlexibleVertexFormat::XyzRhwDiffuseTexture1))
}

pub(super) fn texture_byte_layout(
    descriptor: RuntimeGraphicsTextureDescriptor,
) -> Result<(u32, u32), RuntimeStateError> {
    let bytes_per_pixel = match descriptor.color_format {
        RuntimeGraphicsColorFormat::B8g8r8x8Unorm => 4_u32,
    };
    let row_pitch = descriptor
        .width
        .checked_mul(bytes_per_pixel)
        .ok_or_else(|| {
            RuntimeStateError::new(
                "logical graphics texture row pitch exceeds x86 resource accounting",
            )
        })?;
    let byte_length = row_pitch.checked_mul(descriptor.height).ok_or_else(|| {
        RuntimeStateError::new(
            "logical graphics texture byte length exceeds x86 resource accounting",
        )
    })?;
    if row_pitch == 0 || byte_length == 0 {
        return Err(RuntimeStateError::new(
            "logical graphics texture layout must contain texels",
        ));
    }
    Ok((row_pitch, byte_length))
}

fn validate_texture_update_pair(
    device_id: RuntimeGraphicsDeviceId,
    source: &RuntimeGraphicsTexture,
    destination: &RuntimeGraphicsTexture,
) -> Result<bool, RuntimeStateError> {
    if source.id == destination.id
        || source.device_id != device_id
        || destination.device_id != device_id
        || source.descriptor.pool != RuntimeGraphicsTexturePool::SystemMemory
        || source.descriptor.usage != RuntimeGraphicsTextureUsage::None
        || destination.descriptor.pool != RuntimeGraphicsTexturePool::Default
        || source.descriptor.width != destination.descriptor.width
        || source.descriptor.height != destination.descriptor.height
        || source.descriptor.levels != destination.descriptor.levels
        || source.descriptor.color_format != destination.descriptor.color_format
        || source.active_lock.is_some()
        || destination.active_lock.is_some()
    {
        return Ok(false);
    }
    if let Some(pixel_source) = source.content.pixel_source {
        let backing = source.backing.ok_or_else(|| {
            RuntimeStateError::new("logical texture pixel source has no live guest-memory backing")
        })?;
        if pixel_source.source_texture_id != source.id
            || pixel_source.source_content_generation != source.content.content_generation
            || pixel_source.range != backing.range
            || pixel_source.row_pitch != backing.row_pitch
        {
            return Err(RuntimeStateError::new(
                "logical texture pixel source does not match its live guest-memory backing",
            ));
        }
    }
    Ok(true)
}

fn validate_partial_texture_update_lineage(
    source: &RuntimeGraphicsTexture,
    destination: &RuntimeGraphicsTexture,
) -> Result<bool, RuntimeStateError> {
    if !matches!(
        source.dirty_region,
        RuntimeGraphicsTextureDirtyRegion::Rect(_)
    ) {
        return Ok(true);
    }
    let Some(pixel_source) = source.content.pixel_source else {
        return Ok(false);
    };
    let Some(previous_update) = destination.content.last_update else {
        return Ok(false);
    };
    if pixel_source.source_texture_id != source.id
        || previous_update.source_texture_id != source.id
        || previous_update.source_content_generation >= source.content.content_generation
    {
        return Ok(false);
    }
    Ok(true)
}

fn full_texture_rect(
    descriptor: RuntimeGraphicsTextureDescriptor,
) -> Result<RuntimeRect, RuntimeStateError> {
    let right = i32::try_from(descriptor.width).map_err(|_| {
        RuntimeStateError::new("logical graphics texture width does not fit x86 coordinates")
    })?;
    let bottom = i32::try_from(descriptor.height).map_err(|_| {
        RuntimeStateError::new("logical graphics texture height does not fit x86 coordinates")
    })?;
    Ok(RuntimeRect {
        left: 0,
        top: 0,
        right,
        bottom,
    })
}

fn valid_texture_dirty_rect(
    rect: RuntimeRect,
    descriptor: RuntimeGraphicsTextureDescriptor,
) -> Result<bool, RuntimeStateError> {
    let full = full_texture_rect(descriptor)?;
    if rect.left < 0
        || rect.top < 0
        || rect.right <= rect.left
        || rect.bottom <= rect.top
        || rect.right > full.right
        || rect.bottom > full.bottom
    {
        return Ok(false);
    }
    Ok(true)
}

fn merged_texture_dirty_region(
    current: RuntimeGraphicsTextureDirtyRegion,
    rect: Option<RuntimeRect>,
    descriptor: RuntimeGraphicsTextureDescriptor,
) -> Result<Option<RuntimeGraphicsTextureDirtyRegion>, RuntimeStateError> {
    let full = full_texture_rect(descriptor)?;
    let Some(rect) = rect else {
        return Ok(Some(RuntimeGraphicsTextureDirtyRegion::WholeTexture));
    };
    if !valid_texture_dirty_rect(rect, descriptor)? {
        return Ok(None);
    }
    if current == RuntimeGraphicsTextureDirtyRegion::WholeTexture {
        return Ok(Some(current));
    }
    let merged = match current {
        RuntimeGraphicsTextureDirtyRegion::Clean => rect,
        RuntimeGraphicsTextureDirtyRegion::Rect(previous) => RuntimeRect {
            left: previous.left.min(rect.left),
            top: previous.top.min(rect.top),
            right: previous.right.max(rect.right),
            bottom: previous.bottom.max(rect.bottom),
        },
        RuntimeGraphicsTextureDirtyRegion::WholeTexture => unreachable!(),
    };
    Ok(Some(if merged == full {
        RuntimeGraphicsTextureDirtyRegion::WholeTexture
    } else {
        RuntimeGraphicsTextureDirtyRegion::Rect(merged)
    }))
}

fn validate_surface_fill_rect(
    rect: RuntimeRect,
    descriptor: RuntimeGraphicsSurfaceDescriptor,
) -> Result<(), RuntimeStateError> {
    let width = i32::try_from(descriptor.width).map_err(|_| {
        RuntimeStateError::new("logical graphics surface width does not fit x86 coordinates")
    })?;
    let height = i32::try_from(descriptor.height).map_err(|_| {
        RuntimeStateError::new("logical graphics surface height does not fit x86 coordinates")
    })?;
    if rect.left < 0
        || rect.top < 0
        || rect.right < rect.left
        || rect.bottom < rect.top
        || rect.right > width
        || rect.bottom > height
    {
        return Err(RuntimeStateError::new(
            "bounded graphics surface fill requires an ordered in-bounds rectangle",
        ));
    }
    Ok(())
}

fn mip_dimension(base: u32, level: u32) -> u32 {
    base.checked_shr(level).unwrap_or(0).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> RuntimeGraphicsDeviceCreationRequest {
        RuntimeGraphicsDeviceCreationRequest {
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

    fn texture_descriptor() -> RuntimeGraphicsTextureDescriptor {
        RuntimeGraphicsTextureDescriptor {
            width: 1_024,
            height: 1_024,
            levels: 1,
            usage: RuntimeGraphicsTextureUsage::None,
            color_format: RuntimeGraphicsColorFormat::B8g8r8x8Unorm,
            pool: RuntimeGraphicsTexturePool::SystemMemory,
        }
    }

    fn prepared_draw_graphics() -> (
        RuntimeGraphics,
        RuntimeGraphicsDeviceId,
        RuntimeGraphicsTextureId,
        RuntimeGraphicsVertexBufferId,
        GuestRange,
    ) {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics
            .validate_device_creation(request())
            .unwrap()
            .unwrap();
        let device_id = graphics.insert_device(device_plan, RuntimeComObjectId(7));
        let texture_plan = graphics
            .validate_texture_creation(
                RuntimeComObjectId(7),
                RuntimeGraphicsTextureDescriptor {
                    width: 2_048,
                    height: 1_024,
                    levels: 1,
                    usage: RuntimeGraphicsTextureUsage::None,
                    color_format: RuntimeGraphicsColorFormat::B8g8r8x8Unorm,
                    pool: RuntimeGraphicsTexturePool::Default,
                },
            )
            .unwrap();
        let texture_id = graphics.insert_texture(texture_plan, RuntimeComObjectId(8));
        let backing_range = GuestRange::new(RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE, 0x1000).unwrap();
        let vertex_buffer_plan = graphics
            .validate_vertex_buffer_creation(
                RuntimeComObjectId(7),
                RuntimeGraphicsVertexBufferDescriptor {
                    byte_length: 112,
                    usage: RuntimeGraphicsVertexBufferUsage::WriteOnly,
                    flexible_vertex_format: 0x144,
                    pool: RuntimeGraphicsVertexBufferPool::Managed,
                },
                ProcessAddressAllocationId(41),
                backing_range,
            )
            .unwrap();
        let vertex_buffer_id = graphics.insert_vertex_buffer(
            vertex_buffer_plan,
            RuntimeComObjectId(9),
            MemoryOwnerId(23),
            ProcessAddressAllocationId(41),
            backing_range,
        );
        {
            let texture = graphics.textures.get_mut(&texture_id).unwrap();
            texture.content.content_generation = 1;
        }
        {
            let vertex_buffer = graphics.vertex_buffers.get_mut(&vertex_buffer_id).unwrap();
            vertex_buffer.initialized = true;
        }
        {
            let device = graphics.devices.get_mut(&device_id).unwrap();
            device.render_states = RuntimeGraphicsRenderStates {
                cull_mode: Some(RuntimeGraphicsCullMode::None),
                depth_test_enabled: Some(false),
                lighting_enabled: Some(false),
                alpha_blend_enabled: Some(true),
                source_blend_factor: Some(RuntimeGraphicsBlendFactor::SourceAlpha),
                destination_blend_factor: Some(RuntimeGraphicsBlendFactor::InverseSourceAlpha),
                blend_operation: Some(RuntimeGraphicsBlendOperation::Add),
            };
            device.sampler_states = RuntimeGraphicsSamplerStates {
                sampler_zero: RuntimeGraphicsSamplerState {
                    min_filter: Some(RuntimeGraphicsTextureFilter::Linear),
                    mag_filter: Some(RuntimeGraphicsTextureFilter::Linear),
                },
            };
            device.texture_stage_states = RuntimeGraphicsTextureStageStates {
                stage_zero: RuntimeGraphicsTextureStageState {
                    color_operation: Some(RuntimeGraphicsTextureOperation::Modulate),
                    color_argument_one: Some(RuntimeGraphicsTextureArgument::Texture),
                    color_argument_two: Some(RuntimeGraphicsTextureArgument::Diffuse),
                    alpha_operation: Some(RuntimeGraphicsTextureOperation::SelectArgumentOne),
                    alpha_argument_one: Some(RuntimeGraphicsTextureArgument::Texture),
                    alpha_argument_two: Some(RuntimeGraphicsTextureArgument::Diffuse),
                },
            };
            device.texture_bindings.stage_zero = Some(texture_id);
            device.flexible_vertex_format =
                Some(RuntimeGraphicsFlexibleVertexFormat::XyzRhwDiffuseTexture1);
            device.stream_sources.stream_zero = Some(RuntimeGraphicsStreamSourceBinding {
                vertex_buffer_id,
                offset_in_bytes: 0,
                stride: 28,
            });
            device.back_buffer.clear_color = Some(0xff00_0000);
            device.scene_active = true;
        }
        (
            graphics,
            device_id,
            texture_id,
            vertex_buffer_id,
            backing_range,
        )
    }

    fn draw_memory(bytes: &[u8]) -> crate::memory::GuestMemory {
        use crate::memory::{
            GuestMemory, MemoryLimits, MemoryOwnerKind, MemoryRegionKind, PageProtection,
        };
        let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
        for _ in 0..22 {
            let owner = memory.create_owner(MemoryOwnerKind::Runtime).unwrap();
            memory.seal_owner(owner).unwrap();
        }
        let owner = memory.create_owner(MemoryOwnerKind::Runtime).unwrap();
        assert_eq!(owner, MemoryOwnerId(23));
        let range = memory
            .reserve_fixed(
                owner,
                MemoryRegionKind::Runtime,
                RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE,
                4096,
            )
            .unwrap();
        memory
            .commit(
                owner,
                range,
                PageProtection {
                    read: true,
                    write: true,
                    execute: false,
                },
            )
            .unwrap();
        memory.write(range.start(), bytes).unwrap();
        memory.seal_owner(owner).unwrap();
        let reflection = memory.pending_reflection().unwrap();
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .unwrap();
        memory
    }

    #[test]
    fn retirement_rejects_corrupt_identity_and_exhaustion_before_mutation() {
        let mut graphics = RuntimeGraphics::default();
        let device_id = graphics
            .validate_device_creation(request())
            .map(|creation| graphics.insert_device(creation.unwrap(), RuntimeComObjectId(7)))
            .unwrap();
        let texture_creation = graphics
            .validate_texture_creation(RuntimeComObjectId(7), texture_descriptor())
            .unwrap();
        let texture_id = graphics.insert_texture(texture_creation, RuntimeComObjectId(8));
        graphics
            .bind_texture(RuntimeComObjectId(7), 0, Some(RuntimeComObjectId(8)))
            .unwrap();
        for fault in [2, 3, 4] {
            let mut rejected = graphics.clone();
            match fault {
                2 => rejected.textures.get_mut(&texture_id).unwrap().id.0 += 1,
                3 => {
                    rejected.device_texture_index.remove(&device_id);
                }
                4 => {
                    rejected
                        .texture_com_object_index
                        .remove(&RuntimeComObjectId(8));
                }
                _ => unreachable!(),
            }
            let before = rejected.clone();
            assert!(
                rejected
                    .validate_com_object_retirements(&[RuntimeComObjectId(8)].into_iter().collect())
                    .is_err()
            );
            assert_eq!(rejected, before);
        }
    }

    #[test]
    fn device_creation_and_retirement_preserve_owner_invariants() {
        let mut graphics = RuntimeGraphics::default();
        let plan = graphics
            .validate_device_creation(request())
            .unwrap()
            .unwrap();
        assert_eq!(plan.presentation().width, 800);
        let device_id = graphics.insert_device(plan, RuntimeComObjectId(7));
        assert_eq!(device_id, RuntimeGraphicsDeviceId(1));
        assert_eq!(
            graphics.device(device_id).unwrap().com_object_id,
            RuntimeComObjectId(7)
        );
        assert_eq!(
            graphics
                .device_by_com_object(RuntimeComObjectId(7))
                .unwrap()
                .id,
            device_id
        );
        graphics
            .device_by_com_object(RuntimeComObjectId(99))
            .unwrap_err();
        let retirement = graphics
            .validate_com_object_retirements(&[RuntimeComObjectId(7)].into_iter().collect())
            .unwrap()
            .unwrap();
        graphics.remove_validated_com_objects(retirement);
        assert!(graphics.is_empty());
        graphics
            .device_by_com_object(RuntimeComObjectId(7))
            .unwrap_err();
    }

    #[test]
    fn render_state_updates_are_typed_idempotent_and_failure_atomic() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics
            .validate_device_creation(request())
            .unwrap()
            .unwrap();
        let device_id = graphics.insert_device(device_plan, RuntimeComObjectId(7));
        assert_eq!(
            graphics.device(device_id).unwrap().render_states,
            RuntimeGraphicsRenderStates::default()
        );

        for update in [
            RuntimeGraphicsRenderStateUpdate::CullMode(RuntimeGraphicsCullMode::None),
            RuntimeGraphicsRenderStateUpdate::DepthTestEnabled(false),
            RuntimeGraphicsRenderStateUpdate::LightingEnabled(false),
            RuntimeGraphicsRenderStateUpdate::AlphaBlendEnabled(true),
            RuntimeGraphicsRenderStateUpdate::SourceBlendFactor(
                RuntimeGraphicsBlendFactor::SourceAlpha,
            ),
            RuntimeGraphicsRenderStateUpdate::DestinationBlendFactor(
                RuntimeGraphicsBlendFactor::InverseSourceAlpha,
            ),
            RuntimeGraphicsRenderStateUpdate::BlendOperation(RuntimeGraphicsBlendOperation::Add),
        ] {
            graphics
                .update_render_state(RuntimeComObjectId(7), update)
                .unwrap();
        }
        let committed = graphics.device(device_id).unwrap();
        assert_eq!(
            committed.render_states.cull_mode,
            Some(RuntimeGraphicsCullMode::None)
        );
        assert_eq!(committed.render_states.depth_test_enabled, Some(false));
        assert_eq!(committed.render_states.lighting_enabled, Some(false));
        assert_eq!(committed.render_states.alpha_blend_enabled, Some(true));
        assert_eq!(
            committed.render_states.source_blend_factor,
            Some(RuntimeGraphicsBlendFactor::SourceAlpha)
        );
        assert_eq!(
            committed.render_states.destination_blend_factor,
            Some(RuntimeGraphicsBlendFactor::InverseSourceAlpha)
        );
        assert_eq!(
            committed.render_states.blend_operation,
            Some(RuntimeGraphicsBlendOperation::Add)
        );

        let before_idempotent = graphics.clone();
        graphics
            .update_render_state(
                RuntimeComObjectId(7),
                RuntimeGraphicsRenderStateUpdate::AlphaBlendEnabled(true),
            )
            .unwrap();

        assert_eq!(graphics, before_idempotent);

        let before_unknown = graphics.clone();
        graphics
            .update_render_state(
                RuntimeComObjectId(99),
                RuntimeGraphicsRenderStateUpdate::LightingEnabled(false),
            )
            .unwrap_err();
        assert_eq!(graphics, before_unknown);
    }

    #[test]
    fn sampler_state_updates_are_typed_idempotent_and_failure_atomic() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics
            .validate_device_creation(request())
            .unwrap()
            .unwrap();
        let device_id = graphics.insert_device(device_plan, RuntimeComObjectId(7));
        assert_eq!(
            graphics.device(device_id).unwrap().sampler_states,
            RuntimeGraphicsSamplerStates::default()
        );

        let min_filter = RuntimeGraphicsSamplerStateUpdate::MinFilter {
            sampler: 0,
            filter: RuntimeGraphicsTextureFilter::Linear,
        };
        graphics
            .update_sampler_state(RuntimeComObjectId(7), min_filter)
            .unwrap();

        let before_idempotent = graphics.clone();
        graphics
            .update_sampler_state(RuntimeComObjectId(7), min_filter)
            .unwrap();

        assert_eq!(graphics, before_idempotent);

        let mag_filter = RuntimeGraphicsSamplerStateUpdate::MagFilter {
            sampler: 0,
            filter: RuntimeGraphicsTextureFilter::Linear,
        };
        graphics
            .update_sampler_state(RuntimeComObjectId(7), mag_filter)
            .unwrap();

        let committed = graphics.device(device_id).unwrap();
        assert_eq!(
            committed.sampler_states.sampler_zero.min_filter,
            Some(RuntimeGraphicsTextureFilter::Linear)
        );
        assert_eq!(
            committed.sampler_states.sampler_zero.mag_filter,
            Some(RuntimeGraphicsTextureFilter::Linear)
        );

        let before_invalid = graphics.clone();
        graphics
            .update_sampler_state(
                RuntimeComObjectId(7),
                RuntimeGraphicsSamplerStateUpdate::MinFilter {
                    sampler: 1,
                    filter: RuntimeGraphicsTextureFilter::Linear,
                },
            )
            .unwrap_err();
        assert_eq!(graphics, before_invalid);

        let before_unknown = graphics.clone();
        graphics
            .update_sampler_state(RuntimeComObjectId(99), min_filter)
            .unwrap_err();
        assert_eq!(graphics, before_unknown);

        let exhausted = graphics.clone();
        graphics
            .update_sampler_state(RuntimeComObjectId(7), min_filter)
            .unwrap();
        assert_eq!(graphics, exhausted);
    }

    #[test]
    fn texture_stage_state_updates_are_typed_idempotent_and_failure_atomic() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics
            .validate_device_creation(request())
            .unwrap()
            .unwrap();
        let device_id = graphics.insert_device(device_plan, RuntimeComObjectId(7));
        assert_eq!(
            graphics.device(device_id).unwrap().texture_stage_states,
            RuntimeGraphicsTextureStageStates::default()
        );

        let updates = [
            RuntimeGraphicsTextureStageStateUpdate::ColorOperation {
                stage: 0,
                operation: RuntimeGraphicsTextureOperation::Modulate,
            },
            RuntimeGraphicsTextureStageStateUpdate::ColorArgumentOne {
                stage: 0,
                argument: RuntimeGraphicsTextureArgument::Texture,
            },
            RuntimeGraphicsTextureStageStateUpdate::ColorArgumentTwo {
                stage: 0,
                argument: RuntimeGraphicsTextureArgument::Diffuse,
            },
            RuntimeGraphicsTextureStageStateUpdate::AlphaOperation {
                stage: 0,
                operation: RuntimeGraphicsTextureOperation::SelectArgumentOne,
            },
            RuntimeGraphicsTextureStageStateUpdate::AlphaArgumentOne {
                stage: 0,
                argument: RuntimeGraphicsTextureArgument::Texture,
            },
            RuntimeGraphicsTextureStageStateUpdate::AlphaArgumentTwo {
                stage: 0,
                argument: RuntimeGraphicsTextureArgument::Diffuse,
            },
        ];
        graphics
            .update_texture_stage_state(RuntimeComObjectId(7), updates[0])
            .unwrap();

        for update in updates.into_iter().skip(1) {
            graphics
                .update_texture_stage_state(RuntimeComObjectId(7), update)
                .unwrap();
        }
        let committed = graphics.device(device_id).unwrap();
        assert_eq!(
            committed.texture_stage_states.stage_zero,
            RuntimeGraphicsTextureStageState {
                color_operation: Some(RuntimeGraphicsTextureOperation::Modulate),
                color_argument_one: Some(RuntimeGraphicsTextureArgument::Texture),
                color_argument_two: Some(RuntimeGraphicsTextureArgument::Diffuse),
                alpha_operation: Some(RuntimeGraphicsTextureOperation::SelectArgumentOne),
                alpha_argument_one: Some(RuntimeGraphicsTextureArgument::Texture),
                alpha_argument_two: Some(RuntimeGraphicsTextureArgument::Diffuse),
            }
        );

        let before_repeat = graphics.clone();
        graphics
            .update_texture_stage_state(RuntimeComObjectId(7), updates[0])
            .unwrap();

        assert_eq!(graphics, before_repeat);

        let before_invalid = graphics.clone();
        graphics
            .update_texture_stage_state(
                RuntimeComObjectId(7),
                RuntimeGraphicsTextureStageStateUpdate::ColorOperation {
                    stage: 1,
                    operation: RuntimeGraphicsTextureOperation::Modulate,
                },
            )
            .unwrap_err();
        assert_eq!(graphics, before_invalid);

        let before_unknown = graphics.clone();
        graphics
            .update_texture_stage_state(RuntimeComObjectId(99), updates[0])
            .unwrap_err();
        assert_eq!(graphics, before_unknown);

        let exhausted = graphics.clone();
        graphics
            .update_texture_stage_state(RuntimeComObjectId(7), updates[0])
            .unwrap();

        assert_eq!(graphics, exhausted);
    }

    #[test]
    fn texture_bindings_are_non_owning_idempotent_and_retirement_safe() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics
            .validate_device_creation(request())
            .unwrap()
            .unwrap();
        let device_id = graphics.insert_device(device_plan, RuntimeComObjectId(7));
        let texture_plan = graphics
            .validate_texture_creation(RuntimeComObjectId(7), texture_descriptor())
            .unwrap();
        let texture_id = graphics.insert_texture(texture_plan, RuntimeComObjectId(8));

        graphics
            .bind_texture(RuntimeComObjectId(7), 0, Some(RuntimeComObjectId(8)))
            .unwrap();
        assert_eq!(
            graphics
                .device(device_id)
                .unwrap()
                .texture_bindings
                .stage_zero,
            Some(texture_id)
        );
        let before_repeat = graphics.clone();
        graphics
            .bind_texture(RuntimeComObjectId(7), 0, Some(RuntimeComObjectId(8)))
            .unwrap();
        assert_eq!(graphics, before_repeat);

        let second_device_plan = graphics
            .validate_device_creation(request())
            .unwrap()
            .unwrap();
        graphics.insert_device(second_device_plan, RuntimeComObjectId(9));
        let second_texture_plan = graphics
            .validate_texture_creation(RuntimeComObjectId(9), texture_descriptor())
            .unwrap();
        graphics.insert_texture(second_texture_plan, RuntimeComObjectId(10));
        let before_wrong_owner = graphics.clone();
        assert!(
            !graphics
                .bind_texture(RuntimeComObjectId(7), 0, Some(RuntimeComObjectId(10)))
                .unwrap()
        );
        assert_eq!(graphics, before_wrong_owner);

        let before_invalid_stage = graphics.clone();
        assert!(
            !graphics
                .bind_texture(RuntimeComObjectId(7), 1, Some(RuntimeComObjectId(8)))
                .unwrap()
        );
        assert_eq!(graphics, before_invalid_stage);

        let retirement = graphics
            .validate_com_object_retirements(&[RuntimeComObjectId(8)].into_iter().collect())
            .unwrap()
            .unwrap();
        graphics.remove_validated_com_objects(retirement);
        let device_after_retirement = graphics.device(device_id).unwrap();
        assert_eq!(device_after_retirement.texture_bindings.stage_zero, None);
        assert!(graphics.texture(texture_id).is_err());

        graphics
            .bind_texture(RuntimeComObjectId(7), 0, None)
            .unwrap();

        let remaining_texture = graphics
            .texture_for_com_object(RuntimeComObjectId(10))
            .unwrap()
            .clone();
        assert_eq!(remaining_texture.device_id, RuntimeGraphicsDeviceId(2));
    }

    #[test]
    fn flexible_vertex_format_updates_are_typed_idempotent_and_failure_atomic() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics
            .validate_device_creation(request())
            .unwrap()
            .unwrap();
        let device_id = graphics.insert_device(device_plan, RuntimeComObjectId(7));
        assert_eq!(
            graphics.device(device_id).unwrap().flexible_vertex_format,
            None
        );

        let format = RuntimeGraphicsFlexibleVertexFormat::XyzRhwDiffuseTexture1;
        graphics
            .update_flexible_vertex_format(RuntimeComObjectId(7), format)
            .unwrap();

        assert_eq!(
            graphics.device(device_id).unwrap().flexible_vertex_format,
            Some(format)
        );
        let before_repeat = graphics.clone();
        graphics
            .update_flexible_vertex_format(RuntimeComObjectId(7), format)
            .unwrap();

        assert_eq!(graphics, before_repeat);

        let before_invalid = graphics.clone();
        graphics
            .update_flexible_vertex_format(RuntimeComObjectId(0), format)
            .unwrap_err();
        assert_eq!(graphics, before_invalid);
        graphics
            .update_flexible_vertex_format(RuntimeComObjectId(99), format)
            .unwrap_err();
        assert_eq!(graphics, before_invalid);
    }

    #[test]
    fn device_state_updates_are_idempotent_and_reject_retired_devices() {
        for kind in 0..3 {
            let update = |graphics: &mut RuntimeGraphics| -> Result<(), RuntimeStateError> {
                match kind {
                    0 => graphics.update_sampler_state(
                        RuntimeComObjectId(7),
                        RuntimeGraphicsSamplerStateUpdate::MinFilter {
                            sampler: 0,
                            filter: RuntimeGraphicsTextureFilter::Linear,
                        },
                    ),
                    1 => graphics.update_texture_stage_state(
                        RuntimeComObjectId(7),
                        RuntimeGraphicsTextureStageStateUpdate::ColorOperation {
                            stage: 0,
                            operation: RuntimeGraphicsTextureOperation::Modulate,
                        },
                    ),
                    _ => graphics.update_flexible_vertex_format(
                        RuntimeComObjectId(7),
                        RuntimeGraphicsFlexibleVertexFormat::XyzRhwDiffuseTexture1,
                    ),
                }
            };
            let mut graphics = RuntimeGraphics::default();
            let plan = graphics
                .validate_device_creation(request())
                .unwrap()
                .unwrap();
            graphics.insert_device(plan, RuntimeComObjectId(7));
            update(&mut graphics).unwrap();

            let before = graphics.clone();
            update(&mut graphics).unwrap();
            assert_eq!(graphics, before, "same-value update changed state");

            let retirement = graphics
                .validate_com_object_retirements(&[RuntimeComObjectId(7)].into_iter().collect())
                .unwrap()
                .unwrap();
            graphics.remove_validated_com_objects(retirement);
            let before = graphics.clone();
            assert!(update(&mut graphics).is_err());
            assert_eq!(graphics, before, "retired device accepted a state update");
        }
    }

    #[test]
    fn stream_sources_are_non_owning_idempotent_owner_checked_and_retirement_safe() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics
            .validate_device_creation(request())
            .unwrap()
            .unwrap();
        let device_id = graphics.insert_device(device_plan, RuntimeComObjectId(7));
        let backing_range = GuestRange::new(RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE, 0x1000).unwrap();
        let vertex_buffer_plan = graphics
            .validate_vertex_buffer_creation(
                RuntimeComObjectId(7),
                RuntimeGraphicsVertexBufferDescriptor {
                    byte_length: 112,
                    usage: RuntimeGraphicsVertexBufferUsage::WriteOnly,
                    flexible_vertex_format: 0x144,
                    pool: RuntimeGraphicsVertexBufferPool::Managed,
                },
                ProcessAddressAllocationId(41),
                backing_range,
            )
            .unwrap();
        let vertex_buffer_id = graphics.insert_vertex_buffer(
            vertex_buffer_plan,
            RuntimeComObjectId(8),
            MemoryOwnerId(23),
            ProcessAddressAllocationId(41),
            backing_range,
        );
        assert!(
            graphics
                .device(device_id)
                .unwrap()
                .stream_sources
                .stream_zero
                .is_none()
        );

        graphics
            .bind_stream_source(RuntimeComObjectId(7), 0, RuntimeComObjectId(8), 0, 28)
            .unwrap();
        assert_eq!(
            graphics
                .device(device_id)
                .unwrap()
                .stream_sources
                .stream_zero,
            Some(RuntimeGraphicsStreamSourceBinding {
                vertex_buffer_id,
                offset_in_bytes: 0,
                stride: 28,
            })
        );
        let before_repeat = graphics.clone();
        graphics
            .bind_stream_source(RuntimeComObjectId(7), 0, RuntimeComObjectId(8), 0, 28)
            .unwrap();
        assert_eq!(graphics, before_repeat);

        let second_device_plan = graphics
            .validate_device_creation(request())
            .unwrap()
            .unwrap();
        graphics.insert_device(second_device_plan, RuntimeComObjectId(9));
        let second_backing = GuestRange::new(
            GuestAddress(
                RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE
                    .0
                    .checked_add(0x1000)
                    .unwrap(),
            ),
            0x1000,
        )
        .unwrap();
        let second_vertex_buffer_plan = graphics
            .validate_vertex_buffer_creation(
                RuntimeComObjectId(9),
                graphics.vertex_buffers[&vertex_buffer_id].descriptor,
                ProcessAddressAllocationId(42),
                second_backing,
            )
            .unwrap();
        graphics.insert_vertex_buffer(
            second_vertex_buffer_plan,
            RuntimeComObjectId(10),
            MemoryOwnerId(24),
            ProcessAddressAllocationId(42),
            second_backing,
        );
        let before_wrong_owner = graphics.clone();
        assert!(
            !graphics
                .bind_stream_source(RuntimeComObjectId(7), 0, RuntimeComObjectId(10), 0, 28)
                .unwrap()
        );
        assert_eq!(graphics, before_wrong_owner);

        for (device_com_object_id, stream, vertex_buffer_com_object_id, offset_in_bytes, stride) in [
            (RuntimeComObjectId(7), 1, RuntimeComObjectId(8), 0, 28),
            (RuntimeComObjectId(7), 0, RuntimeComObjectId(8), 0, 0),
        ] {
            let before_invalid = graphics.clone();
            assert!(
                !graphics
                    .bind_stream_source(
                        device_com_object_id,
                        stream,
                        vertex_buffer_com_object_id,
                        offset_in_bytes,
                        stride,
                    )
                    .unwrap()
            );
            assert_eq!(graphics, before_invalid);
        }

        let before_missing_buffer = graphics.clone();
        assert!(
            graphics
                .bind_stream_source(RuntimeComObjectId(7), 0, RuntimeComObjectId(0), 0, 28)
                .is_err()
        );
        assert_eq!(graphics, before_missing_buffer);

        let retirement = graphics
            .validate_com_object_retirements(&[RuntimeComObjectId(8)].into_iter().collect())
            .unwrap()
            .unwrap();
        graphics.remove_validated_com_objects(retirement);
        let device_after_retirement = graphics.device(device_id).unwrap();
        assert_eq!(device_after_retirement.stream_sources.stream_zero, None);
        assert!(graphics.vertex_buffer(vertex_buffer_id).is_err());
    }

    #[test]
    fn back_buffer_clears_replace_uniform_content_and_fail_atomically() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics
            .validate_device_creation(request())
            .unwrap()
            .unwrap();
        let device_id = graphics.insert_device(device_plan, RuntimeComObjectId(7));
        assert_eq!(
            graphics.device(device_id).unwrap().back_buffer,
            RuntimeGraphicsBackBuffer::default()
        );

        graphics
            .clear_back_buffer(RuntimeComObjectId(7), 0xff00_0000)
            .unwrap();
        let first = graphics
            .device(device_id)
            .unwrap()
            .back_buffer
            .clear_color
            .unwrap();
        assert_eq!(first, 0xff00_0000);
        let committed = graphics.device(device_id).unwrap();
        assert_eq!(committed.back_buffer.clear_color, Some(first));

        graphics
            .clear_back_buffer(RuntimeComObjectId(7), 0xff00_00ff)
            .unwrap();
        let second = graphics.device(device_id).unwrap();
        assert_eq!(second.back_buffer.clear_color.unwrap(), 0xff00_00ff);
        for device_com_object_id in [RuntimeComObjectId(0), RuntimeComObjectId(99)] {
            let before = graphics.clone();
            graphics
                .clear_back_buffer(device_com_object_id, 0xff00_0000)
                .unwrap_err();

            assert_eq!(graphics, before);
        }
    }

    #[test]
    fn draw_submissions_snapshot_prepared_state_and_clear_or_retirement_remain_safe() {
        let (mut graphics, device_id, texture_id, _, _) = prepared_draw_graphics();
        let (device_com_object_id, start_vertex, primitive_count) = (RuntimeComObjectId(7), 0, 2);
        let before = graphics.clone();
        let unmapped =
            crate::memory::GuestMemory::new(crate::memory::MemoryLimits::default()).unwrap();
        assert!(
            graphics
                .submit_draw(
                    device_com_object_id,
                    start_vertex,
                    primitive_count,
                    &unmapped
                )
                .is_err()
        );
        assert_eq!(graphics, before);
        let vertex_data = (0_u8..112).collect::<Vec<_>>();
        let memory = draw_memory(&vertex_data);
        graphics
            .submit_draw(device_com_object_id, start_vertex, primitive_count, &memory)
            .unwrap();
        let device = graphics.device(device_id).unwrap();
        let submission = &device.back_buffer.draw_submissions[0];
        assert_eq!(submission.vertex_data, vertex_data);
        let texture = graphics.texture(texture_id).unwrap();
        assert_eq!(submission.texture_width, texture.descriptor.width);
        assert_eq!(submission.texture_height, texture.descriptor.height);
        assert_eq!(submission.texture_fills, texture.content.fills);
        let committed_draws = device.back_buffer.draw_submissions.clone();

        let retirement = graphics
            .validate_com_object_retirements(
                &[RuntimeComObjectId(8), RuntimeComObjectId(9)]
                    .into_iter()
                    .collect(),
            )
            .unwrap()
            .unwrap();
        graphics.remove_validated_com_objects(retirement);
        let retired_device = graphics.device(device_id).unwrap();
        assert_eq!(retired_device.texture_bindings.stage_zero, None);
        assert_eq!(retired_device.stream_sources.stream_zero, None);
        assert_eq!(retired_device.back_buffer.draw_submissions, committed_draws);

        graphics
            .clear_back_buffer(RuntimeComObjectId(7), 0xff00_00ff)
            .unwrap();
        let cleared = graphics.device(device_id).unwrap();
        assert!(cleared.back_buffer.draw_submissions.is_empty());
        assert_eq!(cleared.back_buffer.clear_color.unwrap(), 0xff00_00ff);
    }

    #[test]
    fn captured_draw_keeps_rendering_bytes_and_fills_after_live_mutation() {
        let (mut graphics, device_id, texture_id, _, vertex_range) = prepared_draw_graphics();
        let fill = RuntimeGraphicsSurfaceFill {
            rect: RuntimeRect {
                left: 0,
                top: 0,
                right: 8,
                bottom: 8,
            },
            color_ref: 0x00ff_0000,
        };
        graphics
            .textures
            .get_mut(&texture_id)
            .unwrap()
            .content
            .fills
            .push(fill);
        let mut memory = draw_memory(&[0x5a; 112]);
        graphics
            .submit_draw(RuntimeComObjectId(7), 0, 2, &memory)
            .unwrap();

        graphics
            .textures
            .get_mut(&texture_id)
            .unwrap()
            .content
            .fills
            .clear();
        memory.write(vertex_range.start(), &[0xa5; 112]).unwrap();
        let draw = &graphics
            .device(device_id)
            .unwrap()
            .back_buffer
            .draw_submissions[0];
        assert_eq!(draw.vertex_data, [0x5a; 112]);
        assert_eq!(draw.texture_fills, [fill]);
        assert_eq!((draw.texture_width, draw.texture_height), (2048, 1024));
        assert_eq!(draw.texture_pitch, None);
        assert!(draw.texture_data.is_empty());
    }

    #[test]
    fn draw_memory_failure_after_vertex_read_preserves_all_graphics_state() {
        let (mut graphics, _, texture_id, vertex_buffer_id, _) = prepared_draw_graphics();
        let memory = draw_memory(&[0x5a; 112]);
        let (device_com_object_id, start_vertex, primitive_count) = (RuntimeComObjectId(7), 0, 2);
        graphics
            .vertex_buffers
            .get_mut(&vertex_buffer_id)
            .unwrap()
            .backing
            .memory_owner = MemoryOwnerId(22);
        let original_range = graphics.vertex_buffers[&vertex_buffer_id].backing.range;
        graphics
            .vertex_buffers
            .get_mut(&vertex_buffer_id)
            .unwrap()
            .backing
            .range = GuestRange::new(original_range.start(), 4).unwrap();
        let before_backing_failure = graphics.clone();
        assert!(
            graphics
                .submit_draw(device_com_object_id, start_vertex, primitive_count, &memory)
                .is_err()
        );
        assert_eq!(graphics, before_backing_failure);
        graphics
            .vertex_buffers
            .get_mut(&vertex_buffer_id)
            .unwrap()
            .backing
            .range = original_range;
        let before_owner_failure = graphics.clone();
        assert!(
            graphics
                .submit_draw(device_com_object_id, start_vertex, primitive_count, &memory)
                .is_err()
        );
        assert_eq!(graphics, before_owner_failure);
        graphics
            .vertex_buffers
            .get_mut(&vertex_buffer_id)
            .unwrap()
            .backing
            .memory_owner = MemoryOwnerId(23);
        let mut descriptor = graphics.texture(texture_id).unwrap().descriptor;
        descriptor.pool = RuntimeGraphicsTexturePool::SystemMemory;
        let plan = graphics
            .validate_texture_creation(RuntimeComObjectId(7), descriptor)
            .unwrap();
        let source_id = graphics.insert_texture(plan, RuntimeComObjectId(10));
        let range = GuestRange::new(GuestAddress(0x50000000), 2048 * 1024 * 4).unwrap();
        let pixels = RuntimeGraphicsTexturePixelSource {
            source_texture_id: source_id,
            source_content_generation: 1,
            range,
            row_pitch: 2048 * 4,
        };
        let source = graphics.textures.get_mut(&source_id).unwrap();
        source.backing = Some(RuntimeGraphicsTextureBacking {
            address_allocation_id: ProcessAddressAllocationId(42),
            memory_owner: MemoryOwnerId(24),
            range,
            row_pitch: 2048 * 4,
        });
        source.content.content_generation = 1;
        source.content.pixel_source = Some(pixels);
        graphics
            .textures
            .get_mut(&texture_id)
            .unwrap()
            .content
            .pixel_source = Some(pixels);
        let before_texture_failure = graphics.clone();
        assert!(
            graphics
                .submit_draw(device_com_object_id, start_vertex, primitive_count, &memory)
                .is_err()
        );
        assert_eq!(graphics, before_texture_failure);
    }

    #[test]
    fn draw_submission_limits_and_incomplete_state_fail_before_mutation() {
        let (mut graphics, device_id, _, _, _) = prepared_draw_graphics();
        let (device_com_object_id, start_vertex, primitive_count) = (RuntimeComObjectId(7), 0, 2);
        let memory = draw_memory(&[0x5a; 112]);
        graphics
            .submit_draw(device_com_object_id, start_vertex, primitive_count, &memory)
            .unwrap();
        let seed = graphics
            .device(device_id)
            .unwrap()
            .back_buffer
            .draw_submissions[0]
            .clone();

        let mut count_limited = graphics.clone();
        count_limited
            .devices
            .get_mut(&device_id)
            .unwrap()
            .back_buffer
            .draw_submissions = vec![seed; RUNTIME_GRAPHICS_DRAW_SUBMISSION_LIMIT];
        let before_count_limit = count_limited.clone();
        count_limited
            .submit_draw(device_com_object_id, start_vertex, primitive_count, &memory)
            .unwrap_err();
        assert_eq!(count_limited, before_count_limit);

        let mut incomplete = graphics.clone();
        incomplete.devices.get_mut(&device_id).unwrap().scene_active = false;
        let before_incomplete = incomplete.clone();
        assert!(
            !incomplete
                .submit_draw(device_com_object_id, start_vertex, primitive_count, &memory)
                .unwrap()
        );
        assert_eq!(incomplete, before_incomplete);

        for (device_com_object_id, start_vertex, primitive_count) in [
            (device_com_object_id, 1, primitive_count),
            (device_com_object_id, start_vertex, 1),
        ] {
            let before = graphics.clone();
            assert!(
                !graphics
                    .submit_draw(device_com_object_id, start_vertex, primitive_count, &memory)
                    .unwrap()
            );
            assert_eq!(graphics, before);
        }
    }

    #[test]
    fn scene_begin_owns_active_lifecycle_and_rejects_nested_scenes_atomically() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics
            .validate_device_creation(request())
            .unwrap()
            .unwrap();
        let device_id = graphics.insert_device(device_plan, RuntimeComObjectId(7));
        assert!(!graphics.device(device_id).unwrap().scene_active);

        graphics.begin_scene(RuntimeComObjectId(7)).unwrap();
        assert!(graphics.device(device_id).unwrap().scene_active);

        let before_nested = graphics.clone();
        assert!(!graphics.begin_scene(RuntimeComObjectId(7)).unwrap());
        assert_eq!(graphics, before_nested);

        for device_com_object_id in [RuntimeComObjectId(0), RuntimeComObjectId(99)] {
            let before = graphics.clone();
            graphics.begin_scene(device_com_object_id).unwrap_err();

            assert_eq!(graphics, before);
        }
    }

    #[test]
    fn scene_end_closes_only_the_active_lifecycle_and_preserves_draw_content_atomically() {
        let (mut graphics, device_id, _, _, _) = prepared_draw_graphics();
        graphics
            .submit_draw(RuntimeComObjectId(7), 0, 2, &draw_memory(&[0x5a; 112]))
            .unwrap();
        let back_buffer = graphics.device(device_id).unwrap().back_buffer.clone();

        graphics.end_scene(RuntimeComObjectId(7)).unwrap();
        let device = graphics.device(device_id).unwrap();
        assert!(!device.scene_active);
        assert_eq!(device.back_buffer, back_buffer);

        let before_repeat = graphics.clone();
        assert!(!graphics.end_scene(RuntimeComObjectId(7)).unwrap());
        assert_eq!(graphics, before_repeat);

        graphics.begin_scene(RuntimeComObjectId(7)).unwrap();
        assert!(graphics.device(device_id).unwrap().scene_active);
        assert_eq!(graphics.device(device_id).unwrap().back_buffer, back_buffer);

        for device_com_object_id in [RuntimeComObjectId(0), RuntimeComObjectId(99)] {
            let before = graphics.clone();
            graphics.end_scene(device_com_object_id).unwrap_err();

            assert_eq!(graphics, before);
        }
    }

    #[test]
    fn present_moves_closed_back_buffer_to_front_frame_and_rejects_invalid_state_atomically() {
        let (mut graphics, device_id, _, _, _) = prepared_draw_graphics();
        graphics
            .submit_draw(RuntimeComObjectId(7), 0, 2, &draw_memory(&[0x5a; 112]))
            .unwrap();
        graphics.end_scene(RuntimeComObjectId(7)).unwrap();

        let request = RuntimeComObjectId(7);
        graphics.present(request).unwrap();
        let device = graphics.device(device_id).unwrap();
        let frame = device.presented_frame.as_ref().unwrap();
        assert_eq!(frame.draw_submissions.len(), 1);
        assert_eq!(device.back_buffer.clear_color, None);
        assert!(device.back_buffer.draw_submissions.is_empty());

        let before_empty = graphics.clone();
        assert!(!graphics.present(request).unwrap());
        assert_eq!(graphics, before_empty);

        for device_com_object_id in [RuntimeComObjectId(0), RuntimeComObjectId(99)] {
            let before = graphics.clone();
            graphics.present(device_com_object_id).unwrap_err();

            assert_eq!(graphics, before);
        }

        let (mut active, _, _, _, _) = prepared_draw_graphics();
        let before_active = active.clone();
        assert!(!active.present(request).unwrap());
        assert_eq!(active, before_active);
    }

    #[test]
    fn texture_creation_and_cascading_retirement_preserve_device_relation() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics
            .validate_device_creation(request())
            .unwrap()
            .unwrap();
        let device_id = graphics.insert_device(device_plan, RuntimeComObjectId(7));
        for failure in [1, 2, 3, 4] {
            let mut rejected = graphics.clone();
            match failure {
                1 => rejected.next_texture_id = u64::MAX,
                2 => rejected.next_texture_id = 0,
                3 => rejected.devices.get_mut(&device_id).unwrap().id = RuntimeGraphicsDeviceId(99),
                4 => {
                    rejected.device_texture_index.remove(&device_id);
                }
                _ => unreachable!(),
            }
            let before = rejected.clone();
            assert!(
                rejected
                    .validate_texture_creation(RuntimeComObjectId(7), texture_descriptor())
                    .is_err()
            );
            assert_eq!(rejected, before);
        }
        let texture_plan = graphics
            .validate_texture_creation(RuntimeComObjectId(7), texture_descriptor())
            .unwrap();
        assert_eq!(texture_plan.0, RuntimeGraphicsTextureId(1));
        assert_eq!(texture_plan.1, device_id);
        assert_eq!(texture_plan.2.width, 1_024);
        let texture_id = graphics.insert_texture(texture_plan, RuntimeComObjectId(8));
        assert_eq!(graphics.textures[&texture_id].device_id, device_id);
        assert_eq!(
            graphics.textures[&texture_id].com_object_id,
            RuntimeComObjectId(8)
        );
        assert_eq!(
            graphics.textures[&texture_id].dirty_region,
            RuntimeGraphicsTextureDirtyRegion::WholeTexture
        );
        for failure in [1, 2, 3, 4] {
            let mut rejected = graphics.clone();
            match failure {
                1 => rejected.next_surface_id = u64::MAX,
                2 => rejected.next_surface_id = 0,
                3 => {
                    rejected.textures.get_mut(&texture_id).unwrap().id =
                        RuntimeGraphicsTextureId(99)
                }
                4 => {
                    rejected.texture_surface_index.remove(&texture_id);
                }
                _ => unreachable!(),
            }
            let before = rejected.clone();
            assert!(
                rejected
                    .validate_surface_creation(RuntimeComObjectId(8), 0)
                    .is_err()
            );
            assert_eq!(rejected, before);
        }
        let surface_plan = graphics
            .validate_surface_creation(RuntimeComObjectId(8), 0)
            .unwrap()
            .unwrap();
        assert_eq!(surface_plan.0, RuntimeGraphicsSurfaceId(1));
        assert_eq!(surface_plan.1, texture_id);
        assert_eq!(surface_plan.2.width, 1_024);
        let surface_id = graphics.insert_surface(surface_plan, RuntimeComObjectId(9));
        assert_eq!(graphics.surfaces[&surface_id].texture_id, texture_id);
        assert_eq!(graphics.surfaces[&surface_id].descriptor.level, 0);
        assert_eq!(
            graphics.surfaces[&surface_id].com_object_id,
            RuntimeComObjectId(9)
        );
        assert_eq!(
            graphics
                .surface_for_texture_level(RuntimeComObjectId(8), 0)
                .unwrap()
                .map(|surface| surface.id),
            Some(surface_id)
        );

        let _incomplete = graphics
            .validate_com_object_retirements(&[RuntimeComObjectId(7)].into_iter().collect())
            .unwrap_err();

        let _incomplete = graphics
            .validate_com_object_retirements(&[RuntimeComObjectId(8)].into_iter().collect())
            .unwrap_err();

        let retirement = graphics
            .validate_com_object_retirements(
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
        graphics.remove_validated_com_objects(retirement);
        assert!(graphics.is_empty());
    }

    #[test]
    fn texture_lock_unlock_owns_backing_content_and_failure_atomicity() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics
            .validate_device_creation(request())
            .unwrap()
            .unwrap();
        graphics.insert_device(device_plan, RuntimeComObjectId(7));
        let texture_plan = graphics
            .validate_texture_creation(RuntimeComObjectId(7), texture_descriptor())
            .unwrap();
        let texture_id = graphics.insert_texture(texture_plan, RuntimeComObjectId(8));
        let backing_range = GuestRange::new(RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE, 0x40_0000)
            .expect("one 1024x1024 B8G8R8X8 texture backing");
        let rect = RuntimeRect {
            left: 0,
            top: 0,
            right: 800,
            bottom: 600,
        };
        for failure in [2, 3, 4, 5, 6] {
            let mut rejected = graphics.clone();
            let mut backing = RuntimeGraphicsTextureBacking {
                address_allocation_id: ProcessAddressAllocationId(41),
                memory_owner: MemoryOwnerId(23),
                range: backing_range,
                row_pitch: 4_096,
            };
            match failure {
                2 => {
                    rejected.textures.get_mut(&texture_id).unwrap().id =
                        RuntimeGraphicsTextureId(99)
                }
                3 => backing.memory_owner = MemoryOwnerId(0),
                4 => backing.address_allocation_id = ProcessAddressAllocationId(0),
                5 => backing.row_pitch = 4,
                6 => backing.range = GuestRange::new(backing_range.start(), 4_096).unwrap(),
                _ => unreachable!(),
            }
            let before = rejected.clone();
            assert!(
                rejected
                    .lock_texture(
                        RuntimeComObjectId(8),
                        0,
                        rect,
                        RUNTIME_GRAPHICS_TEXTURE_LOCK_NO_DIRTY_UPDATE,
                        Some(backing)
                    )
                    .is_err()
            );
            assert_eq!(rejected, before);
        }
        let lock = graphics
            .lock_texture(
                RuntimeComObjectId(8),
                0,
                rect,
                RUNTIME_GRAPHICS_TEXTURE_LOCK_NO_DIRTY_UPDATE,
                Some(RuntimeGraphicsTextureBacking {
                    address_allocation_id: ProcessAddressAllocationId(41),
                    memory_owner: MemoryOwnerId(23),
                    range: backing_range,
                    row_pitch: 4_096,
                }),
            )
            .unwrap()
            .unwrap();
        assert_eq!(lock.pitch, 4_096);
        assert_eq!(lock.guest_address, backing_range.start());
        let locked = graphics.texture(texture_id).unwrap();
        assert_eq!(locked.active_lock, Some(lock));
        assert_eq!(
            locked.backing,
            Some(RuntimeGraphicsTextureBacking {
                address_allocation_id: ProcessAddressAllocationId(41),
                memory_owner: MemoryOwnerId(23),
                range: backing_range,
                row_pitch: 4_096,
            })
        );
        assert!(
            graphics
                .lock_texture(
                    RuntimeComObjectId(8),
                    0,
                    rect,
                    RUNTIME_GRAPHICS_TEXTURE_LOCK_NO_DIRTY_UPDATE,
                    None
                )
                .unwrap()
                .is_none()
        );
        assert!(
            !graphics
                .mark_texture_dirty_region(RuntimeComObjectId(8), Some(rect))
                .unwrap()
        );

        let before_unlock = graphics.clone();
        for failure in [2, 3, 4] {
            let mut rejected = before_unlock.clone();
            match failure {
                2 => {
                    rejected
                        .textures
                        .get_mut(&texture_id)
                        .unwrap()
                        .content
                        .content_generation = u64::MAX
                }
                3 => rejected.textures.get_mut(&texture_id).unwrap().id.0 += 1,
                4 => rejected.textures.get_mut(&texture_id).unwrap().backing = None,
                _ => unreachable!(),
            }
            let before = rejected.clone();
            assert!(rejected.unlock_texture(RuntimeComObjectId(8)).is_err());
            assert_eq!(rejected, before);
        }
        graphics.unlock_texture(RuntimeComObjectId(8)).unwrap();
        let unlocked = graphics.texture(texture_id).unwrap();
        assert_eq!(unlocked.content.content_generation, 1);
        assert_eq!(
            unlocked.content.pixel_source,
            Some(RuntimeGraphicsTexturePixelSource {
                source_texture_id: texture_id,
                source_content_generation: 1,
                range: backing_range,
                row_pitch: 4_096,
            })
        );
        assert!(unlocked.active_lock.is_none());
        assert_eq!(
            unlocked.dirty_region,
            RuntimeGraphicsTextureDirtyRegion::WholeTexture,
            "D3DLOCK_NO_DIRTY_UPDATE must not mutate dirty-region authority"
        );
        let unlocked_backing = unlocked.backing.unwrap();
        let before_repeated_unlock = graphics.clone();
        assert!(!graphics.unlock_texture(RuntimeComObjectId(8)).unwrap());
        assert_eq!(graphics, before_repeated_unlock);

        for (level, flags, lock_rect) in [
            (1, RUNTIME_GRAPHICS_TEXTURE_LOCK_NO_DIRTY_UPDATE, rect),
            (0, 0x1000, rect), // NOOVERWRITE is not a texture lock flag.
            (
                0,
                RUNTIME_GRAPHICS_TEXTURE_LOCK_NO_DIRTY_UPDATE,
                RuntimeRect {
                    right: 1_025,
                    ..rect
                },
            ),
        ] {
            let before = graphics.clone();
            assert!(
                graphics
                    .lock_texture(RuntimeComObjectId(8), level, lock_rect, flags, None)
                    .unwrap()
                    .is_none()
            );
            assert_eq!(graphics, before);
        }

        let retirement = graphics
            .validate_com_object_retirements(&[RuntimeComObjectId(8)].into_iter().collect())
            .unwrap()
            .unwrap();
        assert_eq!(
            graphics
                .texture_backings_for_retirement(&retirement)
                .unwrap(),
            vec![(unlocked_backing, 0x40_0000)]
        );
        graphics.remove_validated_com_objects(retirement);
        assert!(graphics.textures().is_empty());
    }

    #[test]
    fn texture_dirty_regions_are_bounded_coalesced_and_failure_atomic() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics
            .validate_device_creation(request())
            .unwrap()
            .unwrap();
        graphics.insert_device(device_plan, RuntimeComObjectId(7));
        let texture_plan = graphics
            .validate_texture_creation(RuntimeComObjectId(7), texture_descriptor())
            .unwrap();
        let texture_id = graphics.insert_texture(texture_plan, RuntimeComObjectId(8));

        let before_initial_mark = graphics.clone();
        graphics
            .mark_texture_dirty_region(
                RuntimeComObjectId(8),
                Some(RuntimeRect {
                    left: 0,
                    top: 0,
                    right: 800,
                    bottom: 600,
                }),
            )
            .unwrap();
        assert_eq!(graphics, before_initial_mark);

        graphics.textures.get_mut(&texture_id).unwrap().dirty_region =
            RuntimeGraphicsTextureDirtyRegion::Clean;
        graphics
            .mark_texture_dirty_region(
                RuntimeComObjectId(8),
                Some(RuntimeRect {
                    left: 1,
                    top: 2,
                    right: 17,
                    bottom: 19,
                }),
            )
            .unwrap();
        assert_eq!(
            graphics.texture(texture_id).unwrap().dirty_region,
            RuntimeGraphicsTextureDirtyRegion::Rect(RuntimeRect {
                left: 1,
                top: 2,
                right: 17,
                bottom: 19
            })
        );

        graphics
            .mark_texture_dirty_region(
                RuntimeComObjectId(8),
                Some(RuntimeRect {
                    left: 8,
                    top: 10,
                    right: 32,
                    bottom: 40,
                }),
            )
            .unwrap();
        assert_eq!(
            graphics.texture(texture_id).unwrap().dirty_region,
            RuntimeGraphicsTextureDirtyRegion::Rect(RuntimeRect {
                left: 1,
                top: 2,
                right: 32,
                bottom: 40,
            })
        );

        for rect in [
            RuntimeRect {
                left: 4,
                top: 5,
                right: 4,
                bottom: 6,
            },
            RuntimeRect {
                left: -1,
                top: 0,
                right: 1,
                bottom: 1,
            },
            RuntimeRect {
                left: 0,
                top: 0,
                right: 1_025,
                bottom: 1_024,
            },
        ] {
            let before = graphics.clone();
            assert!(
                !graphics
                    .mark_texture_dirty_region(RuntimeComObjectId(8), Some(rect))
                    .unwrap()
            );
            assert_eq!(graphics, before);
        }

        graphics
            .mark_texture_dirty_region(RuntimeComObjectId(8), None)
            .unwrap();
        assert_eq!(
            graphics.texture(texture_id).unwrap().dirty_region,
            RuntimeGraphicsTextureDirtyRegion::WholeTexture
        );
    }

    #[test]
    fn vertex_buffer_lock_unlock_and_retirement_preserve_backing_identity() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics
            .validate_device_creation(request())
            .unwrap()
            .unwrap();
        let device_id = graphics.insert_device(device_plan, RuntimeComObjectId(7));
        let backing_range = GuestRange::new(RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE, 0x1000).unwrap();
        let descriptor = RuntimeGraphicsVertexBufferDescriptor {
            byte_length: 112,
            usage: RuntimeGraphicsVertexBufferUsage::WriteOnly,
            flexible_vertex_format: 0x144,
            pool: RuntimeGraphicsVertexBufferPool::Managed,
        };
        for failure in [1, 2, 3, 4, 5, 6] {
            let mut rejected = graphics.clone();
            let mut allocation = ProcessAddressAllocationId(41);
            let mut range = backing_range;
            match failure {
                1 => rejected.next_vertex_buffer_id = u64::MAX,
                2 => rejected.next_vertex_buffer_id = 0,
                3 => rejected.devices.get_mut(&device_id).unwrap().id = RuntimeGraphicsDeviceId(99),
                4 => {
                    rejected.device_vertex_buffer_index.remove(&device_id);
                }
                5 => allocation = ProcessAddressAllocationId(0),
                6 => range = GuestRange::new(backing_range.start(), 4).unwrap(),
                _ => unreachable!(),
            }
            let before = rejected.clone();
            assert!(
                rejected
                    .validate_vertex_buffer_creation(
                        RuntimeComObjectId(7),
                        descriptor,
                        allocation,
                        range
                    )
                    .is_err()
            );
            assert_eq!(rejected, before);
        }
        let creation = graphics
            .validate_vertex_buffer_creation(
                RuntimeComObjectId(7),
                descriptor,
                ProcessAddressAllocationId(41),
                backing_range,
            )
            .unwrap();
        let vertex_buffer_id = graphics.insert_vertex_buffer(
            creation,
            RuntimeComObjectId(8),
            MemoryOwnerId(23),
            ProcessAddressAllocationId(41),
            backing_range,
        );
        assert_eq!(
            graphics.vertex_buffers[&vertex_buffer_id].device_id,
            device_id
        );
        assert_eq!(
            graphics.vertex_buffers[&vertex_buffer_id]
                .backing
                .address_allocation_id
                .0,
            41
        );
        assert_eq!(
            graphics.vertex_buffers[&vertex_buffer_id]
                .backing
                .memory_owner
                .0,
            23
        );
        assert_eq!(
            graphics.vertex_buffers[&vertex_buffer_id].backing.range,
            backing_range
        );
        let before_unsupported = graphics.clone();
        assert!(
            graphics
                .lock_vertex_buffer(RuntimeComObjectId(8), 1, 0, 0)
                .unwrap()
                .is_none()
        );
        assert_eq!(graphics, before_unsupported);
        {
            let failure = 1;
            let mut rejected = graphics.clone();
            match failure {
                1 => {
                    rejected
                        .vertex_buffers
                        .get_mut(&vertex_buffer_id)
                        .unwrap()
                        .id
                        .0 += 1
                }
                _ => unreachable!(),
            }
            let before = rejected.clone();
            assert!(
                rejected
                    .lock_vertex_buffer(RuntimeComObjectId(8), 0, 0, 0)
                    .is_err()
            );
            assert_eq!(rejected, before);
        }
        let lock = graphics
            .lock_vertex_buffer(RuntimeComObjectId(8), 0, 0, 0)
            .unwrap()
            .unwrap();
        assert_eq!(lock, backing_range.start());
        assert!(graphics.vertex_buffer(vertex_buffer_id).unwrap().locked);
        let before_relock = graphics.clone();
        assert!(
            graphics
                .lock_vertex_buffer(RuntimeComObjectId(8), 0, 0, 0)
                .unwrap()
                .is_none()
        );
        assert_eq!(graphics, before_relock);

        let before_unlock = graphics.clone();
        {
            let failure = 1;
            let mut rejected = before_unlock.clone();
            match failure {
                1 => {
                    rejected
                        .vertex_buffers
                        .get_mut(&vertex_buffer_id)
                        .unwrap()
                        .id
                        .0 += 1
                }
                _ => unreachable!(),
            }
            let before = rejected.clone();
            assert!(
                rejected
                    .unlock_vertex_buffer(RuntimeComObjectId(8))
                    .is_err()
            );
            assert_eq!(rejected, before);
        }
        assert!(
            graphics
                .unlock_vertex_buffer(RuntimeComObjectId(8))
                .unwrap()
        );
        let unlocked = graphics.vertex_buffer(vertex_buffer_id).unwrap();
        assert!(!unlocked.locked);
        assert!(unlocked.initialized);
        let before_repeat = graphics.clone();
        assert!(
            !graphics
                .unlock_vertex_buffer(RuntimeComObjectId(8))
                .unwrap()
        );
        assert_eq!(graphics, before_repeat);

        let retirement = graphics
            .validate_com_object_retirements(&[RuntimeComObjectId(8)].into_iter().collect())
            .unwrap()
            .unwrap();
        assert_eq!(
            graphics
                .vertex_buffer_backings_for_retirement(&retirement)
                .unwrap(),
            vec![(graphics.vertex_buffers[&vertex_buffer_id].backing, 112)]
        );
        graphics.remove_validated_com_objects(retirement);
        assert!(graphics.vertex_buffers().is_empty());
        assert_eq!(graphics.device(device_id).unwrap().id, device_id);
    }

    #[test]
    fn device_capacity_failure_preserves_allocator() {
        let mut graphics = RuntimeGraphics::default();
        for index in 0..RUNTIME_GRAPHICS_DEVICE_LIMIT {
            let plan = graphics
                .validate_device_creation(request())
                .unwrap()
                .unwrap();
            graphics.insert_device(plan, RuntimeComObjectId(index as u64 + 1));
        }
        let before = graphics.clone();
        graphics.validate_device_creation(request()).unwrap_err();
        assert_eq!(graphics, before);
    }

    #[test]
    fn texture_capacity_failure_preserves_allocator_relations() {
        let mut graphics = RuntimeGraphics::default();
        let device = graphics
            .validate_device_creation(request())
            .unwrap()
            .unwrap();
        graphics.insert_device(device, RuntimeComObjectId(1));
        for index in 0..RUNTIME_GRAPHICS_TEXTURE_LIMIT {
            let plan = graphics
                .validate_texture_creation(RuntimeComObjectId(1), texture_descriptor())
                .unwrap();
            graphics.insert_texture(plan, RuntimeComObjectId(index as u64 + 2));
        }
        let before = graphics.clone();
        graphics
            .validate_texture_creation(RuntimeComObjectId(1), texture_descriptor())
            .unwrap_err();
        assert_eq!(graphics, before);
    }

    #[test]
    fn surface_capacity_and_duplicate_level_failures_preserve_owner_state() {
        let mut graphics = RuntimeGraphics::default();
        let device = graphics
            .validate_device_creation(request())
            .unwrap()
            .unwrap();
        graphics.insert_device(device, RuntimeComObjectId(1));
        let mut descriptor = texture_descriptor();
        descriptor.levels = RUNTIME_GRAPHICS_SURFACE_LIMIT as u32 + 1;
        let texture = graphics
            .validate_texture_creation(RuntimeComObjectId(1), descriptor)
            .unwrap();
        graphics.insert_texture(texture, RuntimeComObjectId(2));
        for index in 0..RUNTIME_GRAPHICS_SURFACE_LIMIT {
            let surface = graphics
                .validate_surface_creation(RuntimeComObjectId(2), index as u32)
                .unwrap()
                .unwrap();
            graphics.insert_surface(surface, RuntimeComObjectId(index as u64 + 3));
        }
        let duplicate_before = graphics.clone();
        let _duplicate = graphics
            .validate_surface_creation(RuntimeComObjectId(2), 0)
            .unwrap_err();
        assert_eq!(graphics, duplicate_before);

        let before = graphics.clone();
        graphics
            .validate_surface_creation(RuntimeComObjectId(2), RUNTIME_GRAPHICS_SURFACE_LIMIT as u32)
            .unwrap_err();
        assert_eq!(graphics, before);
    }

    #[test]
    fn surface_fill_mutates_texture_content_and_rejects_invalid_input_atomically() {
        let mut graphics = RuntimeGraphics::default();
        let device = graphics
            .validate_device_creation(request())
            .unwrap()
            .unwrap();
        graphics.insert_device(device, RuntimeComObjectId(1));
        let texture = graphics
            .validate_texture_creation(RuntimeComObjectId(1), texture_descriptor())
            .unwrap();
        graphics.insert_texture(texture, RuntimeComObjectId(2));
        let surface = graphics
            .validate_surface_creation(RuntimeComObjectId(2), 0)
            .unwrap()
            .unwrap();
        let surface_id = graphics.insert_surface(surface, RuntimeComObjectId(3));

        let rect = RuntimeRect {
            left: 0,
            top: 0,
            right: 800,
            bottom: 600,
        };
        graphics
            .fill_surface(RuntimeComObjectId(3), rect, 0x0022_3344)
            .unwrap();
        let filled = graphics
            .texture(graphics.surfaces[&surface_id].texture_id)
            .unwrap();
        assert_eq!(filled.content.fills.len(), 1);
        assert_eq!(filled.content.fills[0].rect, rect);
        assert_eq!(filled.content.fills[0].color_ref, 0x0022_3344);
        assert_eq!(filled.content.content_generation, 1);
        assert_eq!(
            filled.dirty_region,
            RuntimeGraphicsTextureDirtyRegion::WholeTexture
        );
        let stored = graphics.surface(surface_id).unwrap();
        assert_eq!(stored.id, surface_id);
        assert_eq!(stored.texture_id, surface.1);
        assert_eq!(stored.descriptor, surface.2);
        assert_eq!(stored.com_object_id, RuntimeComObjectId(3));

        graphics
            .fill_surface(RuntimeComObjectId(3), rect, 0x0055_6677)
            .unwrap();
        let filled = graphics
            .texture(graphics.surfaces[&surface_id].texture_id)
            .unwrap();
        assert_eq!(
            filled.content.fills,
            [
                RuntimeGraphicsSurfaceFill {
                    rect,
                    color_ref: 0x0022_3344
                },
                RuntimeGraphicsSurfaceFill {
                    rect,
                    color_ref: 0x0055_6677
                },
            ]
        );
        assert_eq!(filled.content.content_generation, 2);

        let before_empty = graphics.clone();
        graphics
            .fill_surface(
                RuntimeComObjectId(3),
                RuntimeRect {
                    left: 1,
                    top: 1,
                    right: 1,
                    bottom: 1,
                },
                0,
            )
            .unwrap();
        assert_eq!(graphics, before_empty);

        let before_invalid = graphics.clone();
        graphics
            .fill_surface(
                RuntimeComObjectId(3),
                RuntimeRect {
                    left: 0,
                    top: 0,
                    right: 1_025,
                    bottom: 1,
                },
                0,
            )
            .unwrap_err();
        assert_eq!(graphics, before_invalid);
        let before_color = graphics.clone();
        graphics
            .fill_surface(RuntimeComObjectId(3), rect, 0x0100_0000)
            .unwrap_err();
        assert_eq!(graphics, before_color);

        let mut exhausted = graphics.clone();
        exhausted
            .textures
            .values_mut()
            .next()
            .unwrap()
            .content
            .content_generation = u64::MAX;
        let before_exhaustion = exhausted.clone();
        assert!(
            exhausted
                .fill_surface(RuntimeComObjectId(3), rect, 0)
                .is_err()
        );
        assert_eq!(exhausted, before_exhaustion);
    }

    #[test]
    fn texture_update_consumes_whole_dirty_content_after_surface_retirement_atomically() {
        let mut graphics = RuntimeGraphics::default();
        let device = graphics
            .validate_device_creation(request())
            .unwrap()
            .unwrap();
        let device_id = graphics.insert_device(device, RuntimeComObjectId(1));
        let source = graphics
            .validate_texture_creation(RuntimeComObjectId(1), texture_descriptor())
            .unwrap();
        let source_id = graphics.insert_texture(source, RuntimeComObjectId(2));
        let surface = graphics
            .validate_surface_creation(RuntimeComObjectId(2), 0)
            .unwrap()
            .unwrap();
        let surface_id = graphics.insert_surface(surface, RuntimeComObjectId(3));
        let rect = RuntimeRect {
            left: 0,
            top: 0,
            right: 800,
            bottom: 600,
        };
        graphics
            .fill_surface(RuntimeComObjectId(3), rect, 0)
            .unwrap();

        let mut destination_descriptor = texture_descriptor();
        destination_descriptor.pool = RuntimeGraphicsTexturePool::Default;
        let destination = graphics
            .validate_texture_creation(RuntimeComObjectId(1), destination_descriptor)
            .unwrap();
        let destination_id = graphics.insert_texture(destination, RuntimeComObjectId(4));

        let surface_retirement = graphics
            .validate_com_object_retirements(&[RuntimeComObjectId(3)].into_iter().collect())
            .unwrap()
            .unwrap();
        graphics.remove_validated_com_objects(surface_retirement);
        assert!(graphics.surfaces().is_empty());
        assert_eq!(
            graphics.texture(source_id).unwrap().content.fills[0].rect,
            rect
        );

        let (device_com_object_id, source_texture_com_object_id, destination_texture_com_object_id) = (
            RuntimeComObjectId(1),
            RuntimeComObjectId(2),
            RuntimeComObjectId(4),
        );
        for (device, source, destination) in [
            (
                RuntimeComObjectId(0),
                source_texture_com_object_id,
                destination_texture_com_object_id,
            ),
            (
                device_com_object_id,
                RuntimeComObjectId(0),
                destination_texture_com_object_id,
            ),
            (
                device_com_object_id,
                source_texture_com_object_id,
                RuntimeComObjectId(99),
            ),
        ] {
            let before = graphics.clone();
            graphics
                .update_texture(device, source, destination)
                .unwrap_err();
            assert_eq!(graphics, before);
        }
        let before_same_texture = graphics.clone();
        assert!(
            !graphics
                .update_texture(
                    device_com_object_id,
                    source_texture_com_object_id,
                    source_texture_com_object_id
                )
                .unwrap()
        );
        assert_eq!(graphics, before_same_texture);
        {
            let mut exhausted = graphics.clone();

            exhausted
                .textures
                .get_mut(&destination_id)
                .unwrap()
                .content
                .content_generation = u64::MAX;

            let before = exhausted.clone();
            assert!(
                exhausted
                    .update_texture(
                        device_com_object_id,
                        source_texture_com_object_id,
                        destination_texture_com_object_id
                    )
                    .is_err()
            );
            assert_eq!(
                exhausted, before,
                "content-generation exhaustion changed either texture"
            );
        }
        graphics
            .update_texture(
                device_com_object_id,
                source_texture_com_object_id,
                destination_texture_com_object_id,
            )
            .unwrap();
        let committed_source = graphics.texture(source_id).unwrap();
        assert_eq!(
            committed_source.dirty_region,
            RuntimeGraphicsTextureDirtyRegion::Clean
        );
        assert_eq!(committed_source.content.fills.len(), 1);
        let committed_destination = graphics.texture(destination_id).unwrap();
        assert_eq!(
            committed_destination.content.fills,
            committed_source.content.fills
        );
        assert_eq!(committed_destination.content.content_generation, 1);
        assert_eq!(
            committed_destination.content.last_update,
            Some(RuntimeGraphicsTextureUpdate {
                source_texture_id: source_id,
                source_content_generation: 1,
            })
        );
        let before_repeat = graphics.clone();
        graphics
            .update_texture(
                device_com_object_id,
                source_texture_com_object_id,
                destination_texture_com_object_id,
            )
            .unwrap();
        assert_eq!(graphics, before_repeat);

        let before_reversed = graphics.clone();
        assert!(
            !graphics
                .update_texture(
                    RuntimeComObjectId(1),
                    RuntimeComObjectId(4),
                    RuntimeComObjectId(2),
                )
                .unwrap()
        );
        assert_eq!(graphics, before_reversed);

        let backing_range = GuestRange::new(RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE, 0x40_0000)
            .expect("one 1024x1024 B8G8R8X8 texture backing");
        let rect = RuntimeRect {
            left: 0,
            top: 0,
            right: 800,
            bottom: 600,
        };
        let _lock = graphics
            .lock_texture(
                RuntimeComObjectId(2),
                0,
                rect,
                RUNTIME_GRAPHICS_TEXTURE_LOCK_NO_DIRTY_UPDATE,
                Some(RuntimeGraphicsTextureBacking {
                    address_allocation_id: ProcessAddressAllocationId(41),
                    memory_owner: MemoryOwnerId(23),
                    range: backing_range,
                    row_pitch: 4_096,
                }),
            )
            .unwrap()
            .unwrap();
        graphics.unlock_texture(RuntimeComObjectId(2)).unwrap();

        let partial_rect = RuntimeRect {
            left: 1,
            top: 2,
            right: 17,
            bottom: 19,
        };
        graphics
            .mark_texture_dirty_region(RuntimeComObjectId(2), Some(partial_rect))
            .unwrap();
        graphics
            .update_texture(
                device_com_object_id,
                source_texture_com_object_id,
                destination_texture_com_object_id,
            )
            .unwrap();
        let committed_source = graphics.texture(source_id).unwrap();
        let committed_destination = graphics.texture(destination_id).unwrap();
        assert_eq!(
            committed_source.dirty_region,
            RuntimeGraphicsTextureDirtyRegion::Clean
        );
        assert_eq!(
            committed_destination.content.pixel_source,
            committed_source.content.pixel_source
        );
        assert_eq!(
            committed_destination
                .content
                .last_update
                .unwrap()
                .source_content_generation,
            2
        );

        // D3DLOCK_NO_DIRTY_UPDATE leaves dirty-region authority with the caller.
        // Multiple guest writes may therefore advance the source generation
        // before AddDirtyRect describes the accumulated transfer region.
        for _ in 0..2 {
            graphics
                .lock_texture(
                    RuntimeComObjectId(2),
                    0,
                    rect,
                    RUNTIME_GRAPHICS_TEXTURE_LOCK_NO_DIRTY_UPDATE,
                    None,
                )
                .unwrap()
                .unwrap();
            graphics.unlock_texture(RuntimeComObjectId(2)).unwrap();
        }
        assert_eq!(
            graphics
                .texture(source_id)
                .unwrap()
                .content
                .content_generation,
            4
        );
        graphics
            .mark_texture_dirty_region(RuntimeComObjectId(2), Some(partial_rect))
            .unwrap();
        graphics
            .update_texture(
                device_com_object_id,
                source_texture_com_object_id,
                destination_texture_com_object_id,
            )
            .unwrap();
        assert_eq!(
            graphics
                .texture(destination_id)
                .unwrap()
                .content
                .last_update
                .unwrap()
                .source_content_generation,
            4
        );
        let before_source_only_retirement = graphics.clone();
        graphics
            .validate_com_object_retirements(&[RuntimeComObjectId(2)].into_iter().collect())
            .unwrap_err();
        assert_eq!(graphics, before_source_only_retirement);
        graphics
            .validate_com_object_retirements(
                &[RuntimeComObjectId(2), RuntimeComObjectId(4)]
                    .into_iter()
                    .collect(),
            )
            .unwrap()
            .unwrap();
        let second_destination = graphics
            .validate_texture_creation(RuntimeComObjectId(1), destination_descriptor)
            .unwrap();
        graphics.insert_texture(second_destination, RuntimeComObjectId(5));
        graphics
            .lock_texture(
                RuntimeComObjectId(2),
                0,
                rect,
                RUNTIME_GRAPHICS_TEXTURE_LOCK_NO_DIRTY_UPDATE,
                None,
            )
            .unwrap()
            .unwrap();
        graphics.unlock_texture(RuntimeComObjectId(2)).unwrap();
        graphics
            .mark_texture_dirty_region(RuntimeComObjectId(2), Some(partial_rect))
            .unwrap();
        let before_missing_lineage = graphics.clone();
        assert!(
            !graphics
                .update_texture(
                    RuntimeComObjectId(1),
                    RuntimeComObjectId(2),
                    RuntimeComObjectId(5),
                )
                .unwrap()
        );
        assert_eq!(graphics, before_missing_lineage);
        assert_eq!(graphics.device(device_id).unwrap().id, device_id);
        assert!(!graphics.surfaces.contains_key(&surface_id));
        assert!(graphics.textures.contains_key(&source_id));
    }
}

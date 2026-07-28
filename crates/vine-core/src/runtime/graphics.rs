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
pub const RUNTIME_GRAPHICS_DRAW_SUBMISSION_LIMIT: usize = 4_096;
pub const RUNTIME_GRAPHICS_DRAW_VERTEX_BYTES_LIMIT: usize = 1024 * 1024;

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
pub enum RuntimeGraphicsCullMode {
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeGraphicsTextureFilter {
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeGraphicsTextureOperation {
    Modulate,
    SelectArgumentOne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeGraphicsTextureArgument {
    Diffuse,
    Texture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeGraphicsFlexibleVertexFormat {
    XyzRhwDiffuseTexture1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeGraphicsPrimitiveTopology {
    TriangleStrip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsTextureBindings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage_zero: Option<RuntimeGraphicsTextureId>,
}

impl RuntimeGraphicsTextureBindings {
    fn is_pristine(&self) -> bool {
        self.stage_zero.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsStreamSourceBinding {
    pub vertex_buffer_id: RuntimeGraphicsVertexBufferId,
    pub offset_in_bytes: u32,
    pub stride: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsStreamSources {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_zero: Option<RuntimeGraphicsStreamSourceBinding>,
}

impl RuntimeGraphicsStreamSources {
    fn is_pristine(&self) -> bool {
        self.stream_zero.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsSamplerState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_filter: Option<RuntimeGraphicsTextureFilter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mag_filter: Option<RuntimeGraphicsTextureFilter>,
}

impl RuntimeGraphicsSamplerState {
    fn is_pristine(&self) -> bool {
        self.min_filter.is_none() && self.mag_filter.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsSamplerStates {
    #[serde(skip_serializing_if = "RuntimeGraphicsSamplerState::is_pristine")]
    pub sampler_zero: RuntimeGraphicsSamplerState,
}

impl RuntimeGraphicsSamplerStates {
    fn is_pristine(&self) -> bool {
        self.sampler_zero.is_pristine()
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
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
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-sampler-state-sampler-unsupported",
                "logical sampler-state authority currently supports sampler zero only",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsTextureStageState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_operation: Option<RuntimeGraphicsTextureOperation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_argument_one: Option<RuntimeGraphicsTextureArgument>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_argument_two: Option<RuntimeGraphicsTextureArgument>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpha_operation: Option<RuntimeGraphicsTextureOperation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpha_argument_one: Option<RuntimeGraphicsTextureArgument>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpha_argument_two: Option<RuntimeGraphicsTextureArgument>,
}

impl RuntimeGraphicsTextureStageState {
    fn is_pristine(&self) -> bool {
        self.color_operation.is_none()
            && self.color_argument_one.is_none()
            && self.color_argument_two.is_none()
            && self.alpha_operation.is_none()
            && self.alpha_argument_one.is_none()
            && self.alpha_argument_two.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsTextureStageStates {
    #[serde(skip_serializing_if = "RuntimeGraphicsTextureStageState::is_pristine")]
    pub stage_zero: RuntimeGraphicsTextureStageState,
}

impl RuntimeGraphicsTextureStageStates {
    fn is_pristine(&self) -> bool {
        self.stage_zero.is_pristine()
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
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
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-texture-stage-state-stage-unsupported",
                "logical texture-stage-state authority currently supports stage zero only",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsRenderStates {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cull_mode: Option<RuntimeGraphicsCullMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth_test_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lighting_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpha_blend_enabled: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeGraphicsRenderStateUpdate {
    CullMode(RuntimeGraphicsCullMode),
    DepthTestEnabled(bool),
    LightingEnabled(bool),
    AlphaBlendEnabled(bool),
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
        }
    }
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
#[serde(tag = "kind", content = "rect", rename_all = "kebab-case")]
pub enum RuntimeGraphicsTextureDirtyRegion {
    Clean,
    Rect(RuntimeRect),
    WholeTexture,
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
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsTextureUpdate {
    pub sequence: u64,
    pub source_texture_id: RuntimeGraphicsTextureId,
    pub source_content_generation: u64,
    pub dirty_region: RuntimeGraphicsTextureDirtyRegion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsTextureContent {
    pub content_generation: u64,
    pub fills: Vec<RuntimeGraphicsSurfaceFill>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_update: Option<RuntimeGraphicsTextureUpdate>,
}

impl RuntimeGraphicsTextureContent {
    fn is_pristine(&self) -> bool {
        self.content_generation == 0 && self.fills.is_empty() && self.last_update.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsBackBufferClear {
    pub sequence: u64,
    pub rect: RuntimeRect,
    pub color_argb: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsDrawTextureSnapshot {
    pub texture_id: RuntimeGraphicsTextureId,
    pub descriptor: RuntimeGraphicsTextureDescriptor,
    pub generation: u64,
    pub content: RuntimeGraphicsTextureContent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsDrawSubmission {
    pub sequence: u64,
    pub scene_sequence: u64,
    pub primitive_topology: RuntimeGraphicsPrimitiveTopology,
    pub start_vertex: u32,
    pub primitive_count: u32,
    pub vertex_count: u32,
    pub flexible_vertex_format: RuntimeGraphicsFlexibleVertexFormat,
    pub stream_source: RuntimeGraphicsStreamSourceBinding,
    pub vertex_buffer_content_generation: u64,
    pub vertex_data: Vec<u8>,
    pub render_states: RuntimeGraphicsRenderStates,
    pub sampler_states: RuntimeGraphicsSamplerStates,
    pub texture_stage_states: RuntimeGraphicsTextureStageStates,
    pub texture: RuntimeGraphicsDrawTextureSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsBackBuffer {
    pub content_generation: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uniform_clear: Option<RuntimeGraphicsBackBufferClear>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub draw_submissions: Vec<RuntimeGraphicsDrawSubmission>,
}

impl RuntimeGraphicsBackBuffer {
    fn is_pristine(&self) -> bool {
        self.content_generation == 0
            && self.uniform_clear.is_none()
            && self.draw_submissions.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsScene {
    pub sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsPresentedFrame {
    pub sequence: u64,
    pub back_buffer: RuntimeGraphicsBackBuffer,
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
    pub render_states: RuntimeGraphicsRenderStates,
    #[serde(skip_serializing_if = "RuntimeGraphicsSamplerStates::is_pristine")]
    pub sampler_states: RuntimeGraphicsSamplerStates,
    #[serde(skip_serializing_if = "RuntimeGraphicsTextureStageStates::is_pristine")]
    pub texture_stage_states: RuntimeGraphicsTextureStageStates,
    #[serde(skip_serializing_if = "RuntimeGraphicsTextureBindings::is_pristine")]
    pub texture_bindings: RuntimeGraphicsTextureBindings,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flexible_vertex_format: Option<RuntimeGraphicsFlexibleVertexFormat>,
    #[serde(skip_serializing_if = "RuntimeGraphicsStreamSources::is_pristine")]
    pub stream_sources: RuntimeGraphicsStreamSources,
    #[serde(skip_serializing_if = "RuntimeGraphicsBackBuffer::is_pristine")]
    pub back_buffer: RuntimeGraphicsBackBuffer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_scene: Option<RuntimeGraphicsScene>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presented_frame: Option<RuntimeGraphicsPresentedFrame>,
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
    pub dirty_region: RuntimeGraphicsTextureDirtyRegion,
    #[serde(skip_serializing_if = "RuntimeGraphicsTextureContent::is_pristine")]
    pub content: RuntimeGraphicsTextureContent,
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
pub(crate) struct RuntimeGraphicsRenderStateUpdateRequest {
    pub device_com_object_id: RuntimeComObjectId,
    pub update: RuntimeGraphicsRenderStateUpdate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsSamplerStateUpdateRequest {
    pub device_com_object_id: RuntimeComObjectId,
    pub update: RuntimeGraphicsSamplerStateUpdate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsTextureStageStateUpdateRequest {
    pub device_com_object_id: RuntimeComObjectId,
    pub update: RuntimeGraphicsTextureStageStateUpdate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsTextureBindingUpdateRequest {
    pub device_com_object_id: RuntimeComObjectId,
    pub stage: u32,
    pub texture_com_object_id: Option<RuntimeComObjectId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsFlexibleVertexFormatUpdateRequest {
    pub device_com_object_id: RuntimeComObjectId,
    pub format: RuntimeGraphicsFlexibleVertexFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsStreamSourceUpdateRequest {
    pub device_com_object_id: RuntimeComObjectId,
    pub stream: u32,
    pub vertex_buffer_com_object_id: RuntimeComObjectId,
    pub offset_in_bytes: u32,
    pub stride: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsBackBufferClearRequest {
    pub device_com_object_id: RuntimeComObjectId,
    pub color_argb: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsDrawCaptureRequest {
    pub device_com_object_id: RuntimeComObjectId,
    pub primitive_topology: RuntimeGraphicsPrimitiveTopology,
    pub start_vertex: u32,
    pub primitive_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsSceneBeginRequest {
    pub device_com_object_id: RuntimeComObjectId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsSceneEndRequest {
    pub device_com_object_id: RuntimeComObjectId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsPresentRequest {
    pub device_com_object_id: RuntimeComObjectId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsTextureCreateRequest {
    pub device_com_object_id: RuntimeComObjectId,
    pub descriptor: RuntimeGraphicsTextureDescriptor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsTextureDirtyRegionMarkRequest {
    pub texture_com_object_id: RuntimeComObjectId,
    pub rect: Option<RuntimeRect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsTextureUpdateRequest {
    pub device_com_object_id: RuntimeComObjectId,
    pub source_texture_com_object_id: RuntimeComObjectId,
    pub destination_texture_com_object_id: RuntimeComObjectId,
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
pub(crate) struct RuntimeGraphicsRenderStateUpdatePlan {
    expected_generation: u64,
    next_generation: u64,
    device_id: RuntimeGraphicsDeviceId,
    device_generation: u64,
    next_device_generation: u64,
    previous: Option<RuntimeGraphicsRenderStateUpdate>,
    update: RuntimeGraphicsRenderStateUpdate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsSamplerStateUpdatePlan {
    expected_generation: u64,
    next_generation: u64,
    device_id: RuntimeGraphicsDeviceId,
    device_generation: u64,
    next_device_generation: u64,
    previous: Option<RuntimeGraphicsSamplerStateUpdate>,
    update: RuntimeGraphicsSamplerStateUpdate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsTextureStageStateUpdatePlan {
    expected_generation: u64,
    next_generation: u64,
    device_id: RuntimeGraphicsDeviceId,
    device_generation: u64,
    next_device_generation: u64,
    previous: Option<RuntimeGraphicsTextureStageStateUpdate>,
    update: RuntimeGraphicsTextureStageStateUpdate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsTextureBindingUpdatePlan {
    expected_generation: u64,
    next_generation: u64,
    device_id: RuntimeGraphicsDeviceId,
    device_generation: u64,
    next_device_generation: u64,
    previous_texture_id: Option<RuntimeGraphicsTextureId>,
    texture_id: Option<RuntimeGraphicsTextureId>,
    request: RuntimeGraphicsTextureBindingUpdateRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsFlexibleVertexFormatUpdatePlan {
    expected_generation: u64,
    next_generation: u64,
    device_id: RuntimeGraphicsDeviceId,
    device_generation: u64,
    next_device_generation: u64,
    previous: Option<RuntimeGraphicsFlexibleVertexFormat>,
    format: RuntimeGraphicsFlexibleVertexFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsStreamSourceUpdatePlan {
    expected_generation: u64,
    next_generation: u64,
    device_id: RuntimeGraphicsDeviceId,
    device_generation: u64,
    next_device_generation: u64,
    previous: Option<RuntimeGraphicsStreamSourceBinding>,
    binding: RuntimeGraphicsStreamSourceBinding,
    request: RuntimeGraphicsStreamSourceUpdateRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsBackBufferClearPlan {
    expected_generation: u64,
    next_generation: u64,
    device_id: RuntimeGraphicsDeviceId,
    device_generation: u64,
    next_device_generation: u64,
    content_generation: u64,
    next_content_generation: u64,
    previous_uniform_clear: Option<RuntimeGraphicsBackBufferClear>,
    previous_draw_submission_count: usize,
    previous_draw_vertex_bytes: usize,
    clear: RuntimeGraphicsBackBufferClear,
    next_clear_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsDrawCapturePlan {
    expected_generation: u64,
    next_generation: u64,
    device_id: RuntimeGraphicsDeviceId,
    device_generation: u64,
    next_device_generation: u64,
    content_generation: u64,
    next_content_generation: u64,
    expected_draw_submission_count: usize,
    expected_draw_vertex_bytes: usize,
    next_draw_sequence: u64,
    vertex_buffer_id: RuntimeGraphicsVertexBufferId,
    vertex_buffer_generation: u64,
    texture_id: RuntimeGraphicsTextureId,
    texture_generation: u64,
    vertex_range: GuestRange,
    submission: RuntimeGraphicsDrawSubmission,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeGraphicsDrawResolvedState {
    scene: RuntimeGraphicsScene,
    flexible_vertex_format: RuntimeGraphicsFlexibleVertexFormat,
    stream_source: RuntimeGraphicsStreamSourceBinding,
    vertex_buffer_id: RuntimeGraphicsVertexBufferId,
    vertex_buffer_generation: u64,
    vertex_buffer_content_generation: u64,
    vertex_range: GuestRange,
    render_states: RuntimeGraphicsRenderStates,
    sampler_states: RuntimeGraphicsSamplerStates,
    texture_stage_states: RuntimeGraphicsTextureStageStates,
    texture: RuntimeGraphicsDrawTextureSnapshot,
}

impl RuntimeGraphicsDrawCapturePlan {
    pub(crate) fn vertex_range(&self) -> GuestRange {
        self.vertex_range
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsDrawSubmissionPlan {
    capture: RuntimeGraphicsDrawCapturePlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsSceneBeginPlan {
    expected_generation: u64,
    next_generation: u64,
    device_id: RuntimeGraphicsDeviceId,
    device_generation: u64,
    next_device_generation: u64,
    scene: RuntimeGraphicsScene,
    next_scene_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsSceneEndPlan {
    expected_generation: u64,
    next_generation: u64,
    device_id: RuntimeGraphicsDeviceId,
    device_generation: u64,
    next_device_generation: u64,
    scene: RuntimeGraphicsScene,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsPresentPlan {
    expected_generation: u64,
    next_generation: u64,
    device_id: RuntimeGraphicsDeviceId,
    device_generation: u64,
    next_device_generation: u64,
    next_back_buffer_content_generation: u64,
    previous_presented_frame: Option<RuntimeGraphicsPresentedFrame>,
    frame: RuntimeGraphicsPresentedFrame,
    next_present_sequence: u64,
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
pub(crate) struct RuntimeGraphicsTextureDirtyRegionMarkPlan {
    expected_generation: u64,
    next_generation: u64,
    texture_id: RuntimeGraphicsTextureId,
    texture_generation: u64,
    next_texture_generation: u64,
    previous_dirty_region: RuntimeGraphicsTextureDirtyRegion,
    dirty_region: RuntimeGraphicsTextureDirtyRegion,
    request: RuntimeGraphicsTextureDirtyRegionMarkRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGraphicsTextureUpdatePlan {
    expected_generation: u64,
    next_generation: u64,
    device_id: RuntimeGraphicsDeviceId,
    source_texture_id: RuntimeGraphicsTextureId,
    source_texture_generation: u64,
    next_source_texture_generation: u64,
    source_dirty_region: RuntimeGraphicsTextureDirtyRegion,
    source_content_generation: u64,
    destination_texture_id: RuntimeGraphicsTextureId,
    destination_texture_generation: u64,
    next_destination_texture_generation: u64,
    destination_content_generation: u64,
    next_destination_content_generation: u64,
    destination_fills: Vec<RuntimeGraphicsSurfaceFill>,
    update: Option<RuntimeGraphicsTextureUpdate>,
    next_update_sequence: u64,
    request: RuntimeGraphicsTextureUpdateRequest,
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
    texture_id: RuntimeGraphicsTextureId,
    texture_generation: u64,
    next_texture_generation: u64,
    texture_dirty_region: RuntimeGraphicsTextureDirtyRegion,
    next_texture_dirty_region: RuntimeGraphicsTextureDirtyRegion,
    texture_content_generation: u64,
    next_texture_content_generation: u64,
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
pub struct RuntimeGraphicsRenderStateUpdateResult {
    pub device_id: RuntimeGraphicsDeviceId,
    pub update: RuntimeGraphicsRenderStateUpdate,
    pub changed: bool,
    pub device_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsSamplerStateUpdateResult {
    pub device_id: RuntimeGraphicsDeviceId,
    pub update: RuntimeGraphicsSamplerStateUpdate,
    pub changed: bool,
    pub device_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsTextureStageStateUpdateResult {
    pub device_id: RuntimeGraphicsDeviceId,
    pub update: RuntimeGraphicsTextureStageStateUpdate,
    pub changed: bool,
    pub device_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsTextureBindingUpdateResult {
    pub device_id: RuntimeGraphicsDeviceId,
    pub stage: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub texture_id: Option<RuntimeGraphicsTextureId>,
    pub changed: bool,
    pub device_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsFlexibleVertexFormatUpdateResult {
    pub device_id: RuntimeGraphicsDeviceId,
    pub format: RuntimeGraphicsFlexibleVertexFormat,
    pub changed: bool,
    pub device_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsStreamSourceUpdateResult {
    pub device_id: RuntimeGraphicsDeviceId,
    pub stream: u32,
    pub binding: RuntimeGraphicsStreamSourceBinding,
    pub changed: bool,
    pub device_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsBackBufferClearResult {
    pub device_id: RuntimeGraphicsDeviceId,
    pub device_generation: u64,
    pub content_generation: u64,
    pub clear: RuntimeGraphicsBackBufferClear,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsDrawSubmissionResult {
    pub device_id: RuntimeGraphicsDeviceId,
    pub device_generation: u64,
    pub content_generation: u64,
    pub submission: RuntimeGraphicsDrawSubmission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsSceneBeginResult {
    pub device_id: RuntimeGraphicsDeviceId,
    pub device_generation: u64,
    pub scene: RuntimeGraphicsScene,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsSceneEndResult {
    pub device_id: RuntimeGraphicsDeviceId,
    pub device_generation: u64,
    pub scene: RuntimeGraphicsScene,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsPresentResult {
    pub device_id: RuntimeGraphicsDeviceId,
    pub device_generation: u64,
    pub frame: RuntimeGraphicsPresentedFrame,
    pub next_back_buffer_content_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsTextureDirtyRegionMarkResult {
    pub texture_id: RuntimeGraphicsTextureId,
    pub dirty_region: RuntimeGraphicsTextureDirtyRegion,
    pub changed: bool,
    pub texture_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsTextureUpdateResult {
    pub source_texture_id: RuntimeGraphicsTextureId,
    pub destination_texture_id: RuntimeGraphicsTextureId,
    pub transferred: bool,
    pub source_texture_generation: u64,
    pub destination_texture_generation: u64,
    pub destination_content_generation: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update: Option<RuntimeGraphicsTextureUpdate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphicsSurfaceFillResult {
    pub surface_id: RuntimeGraphicsSurfaceId,
    pub texture_id: RuntimeGraphicsTextureId,
    pub texture_generation: u64,
    pub content_generation: u64,
    pub dirty_region: RuntimeGraphicsTextureDirtyRegion,
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
    texture_binding_clearances: Vec<RuntimeGraphicsTextureBindingClearance>,
    stream_source_clearances: Vec<RuntimeGraphicsStreamSourceClearance>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeGraphicsTextureBindingClearance {
    device_id: RuntimeGraphicsDeviceId,
    texture_id: RuntimeGraphicsTextureId,
    device_generation: u64,
    next_device_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeGraphicsStreamSourceClearance {
    device_id: RuntimeGraphicsDeviceId,
    binding: RuntimeGraphicsStreamSourceBinding,
    device_generation: u64,
    next_device_generation: u64,
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
    #[serde(skip_serializing_if = "is_initial_sequence")]
    next_texture_update_sequence: u64,
    #[serde(skip_serializing_if = "is_initial_sequence")]
    next_back_buffer_clear_sequence: u64,
    #[serde(skip_serializing_if = "is_initial_sequence")]
    next_scene_sequence: u64,
    #[serde(skip_serializing_if = "is_initial_sequence")]
    next_draw_sequence: u64,
    #[serde(skip_serializing_if = "is_initial_sequence")]
    next_present_sequence: u64,
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
            next_texture_update_sequence: 1,
            next_back_buffer_clear_sequence: 1,
            next_scene_sequence: 1,
            next_draw_sequence: 1,
            next_present_sequence: 1,
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
            render_states: RuntimeGraphicsRenderStates::default(),
            sampler_states: RuntimeGraphicsSamplerStates::default(),
            texture_stage_states: RuntimeGraphicsTextureStageStates::default(),
            texture_bindings: RuntimeGraphicsTextureBindings::default(),
            flexible_vertex_format: None,
            stream_sources: RuntimeGraphicsStreamSources::default(),
            back_buffer: RuntimeGraphicsBackBuffer::default(),
            active_scene: None,
            presented_frame: None,
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

    pub(super) fn plan_render_state_update(
        &self,
        request: RuntimeGraphicsRenderStateUpdateRequest,
    ) -> Result<RuntimeGraphicsRenderStateUpdatePlan, RuntimeStateError> {
        let device_id = self
            .device_com_object_index
            .get(&request.device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-render-state-device-com-object-unknown",
                    "logical render-state update requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(request.device_com_object_id, device_id)?;
        let device = self.device(device_id)?;
        let previous = device.render_states.current_for(request.update);
        let changed = previous != Some(request.update);
        let next_generation = if changed {
            checked_next_generation(self.generation)?
        } else {
            self.generation
        };
        let next_device_generation = if changed {
            device.generation.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-graphics",
                    "graphics-device-generation-exhausted",
                    "logical graphics device generation is exhausted",
                )
            })?
        } else {
            device.generation
        };
        Ok(RuntimeGraphicsRenderStateUpdatePlan {
            expected_generation: self.generation,
            next_generation,
            device_id,
            device_generation: device.generation,
            next_device_generation,
            previous,
            update: request.update,
        })
    }

    pub(super) fn preflight_render_state_update(
        &self,
        plan: &RuntimeGraphicsRenderStateUpdatePlan,
    ) -> Result<(), RuntimeStateError> {
        let Some(device) = self.devices.get(&plan.device_id) else {
            return Err(stale_plan("graphics-render-state-update-plan-stale"));
        };
        let changed = plan.previous != Some(plan.update);
        if self.generation != plan.expected_generation
            || device.generation != plan.device_generation
            || device.render_states.current_for(plan.update) != plan.previous
            || (changed
                && (self.generation.checked_add(1) != Some(plan.next_generation)
                    || device.generation.checked_add(1) != Some(plan.next_device_generation)))
            || (!changed
                && (plan.next_generation != self.generation
                    || plan.next_device_generation != device.generation))
        {
            return Err(stale_plan("graphics-render-state-update-plan-stale"));
        }
        self.validate_device_association(device.com_object_id, device.id)
    }

    pub(super) fn commit_render_state_update(
        &mut self,
        plan: RuntimeGraphicsRenderStateUpdatePlan,
    ) -> RuntimeGraphicsRenderStateUpdateResult {
        debug_assert!(self.preflight_render_state_update(&plan).is_ok());
        let changed = plan.previous != Some(plan.update);
        let device = self
            .devices
            .get_mut(&plan.device_id)
            .expect("preflighted graphics device disappeared");
        if changed {
            device.render_states.apply(plan.update);
            device.generation = plan.next_device_generation;
            self.generation = plan.next_generation;
        }
        RuntimeGraphicsRenderStateUpdateResult {
            device_id: device.id,
            update: plan.update,
            changed,
            device_generation: device.generation,
        }
    }

    pub(super) fn plan_sampler_state_update(
        &self,
        request: RuntimeGraphicsSamplerStateUpdateRequest,
    ) -> Result<RuntimeGraphicsSamplerStateUpdatePlan, RuntimeStateError> {
        request.update.validate()?;
        let device_id = self
            .device_com_object_index
            .get(&request.device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-sampler-state-device-com-object-unknown",
                    "logical sampler-state update requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(request.device_com_object_id, device_id)?;
        let device = self.device(device_id)?;
        let previous = device.sampler_states.current_for(request.update);
        let changed = previous != Some(request.update);
        let next_generation = if changed {
            checked_next_generation(self.generation)?
        } else {
            self.generation
        };
        let next_device_generation = if changed {
            device.generation.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-graphics",
                    "graphics-device-generation-exhausted",
                    "logical graphics device generation is exhausted",
                )
            })?
        } else {
            device.generation
        };
        Ok(RuntimeGraphicsSamplerStateUpdatePlan {
            expected_generation: self.generation,
            next_generation,
            device_id,
            device_generation: device.generation,
            next_device_generation,
            previous,
            update: request.update,
        })
    }

    pub(super) fn preflight_sampler_state_update(
        &self,
        plan: &RuntimeGraphicsSamplerStateUpdatePlan,
    ) -> Result<(), RuntimeStateError> {
        let Some(device) = self.devices.get(&plan.device_id) else {
            return Err(stale_plan("graphics-sampler-state-update-plan-stale"));
        };
        let changed = plan.previous != Some(plan.update);
        if self.generation != plan.expected_generation
            || device.generation != plan.device_generation
            || device.sampler_states.current_for(plan.update) != plan.previous
            || (changed
                && (self.generation.checked_add(1) != Some(plan.next_generation)
                    || device.generation.checked_add(1) != Some(plan.next_device_generation)))
            || (!changed
                && (plan.next_generation != self.generation
                    || plan.next_device_generation != device.generation))
        {
            return Err(stale_plan("graphics-sampler-state-update-plan-stale"));
        }
        self.validate_device_association(device.com_object_id, device.id)
    }

    pub(super) fn commit_sampler_state_update(
        &mut self,
        plan: RuntimeGraphicsSamplerStateUpdatePlan,
    ) -> RuntimeGraphicsSamplerStateUpdateResult {
        debug_assert!(self.preflight_sampler_state_update(&plan).is_ok());
        let changed = plan.previous != Some(plan.update);
        let device = self
            .devices
            .get_mut(&plan.device_id)
            .expect("preflighted graphics device disappeared");
        if changed {
            device.sampler_states.apply(plan.update);
            device.generation = plan.next_device_generation;
            self.generation = plan.next_generation;
        }
        RuntimeGraphicsSamplerStateUpdateResult {
            device_id: device.id,
            update: plan.update,
            changed,
            device_generation: device.generation,
        }
    }

    pub(super) fn plan_texture_stage_state_update(
        &self,
        request: RuntimeGraphicsTextureStageStateUpdateRequest,
    ) -> Result<RuntimeGraphicsTextureStageStateUpdatePlan, RuntimeStateError> {
        request.update.validate()?;
        let device_id = self
            .device_com_object_index
            .get(&request.device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-texture-stage-state-device-com-object-unknown",
                    "logical texture-stage-state update requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(request.device_com_object_id, device_id)?;
        let device = self.device(device_id)?;
        let previous = device.texture_stage_states.current_for(request.update);
        let changed = previous != Some(request.update);
        let next_generation = if changed {
            checked_next_generation(self.generation)?
        } else {
            self.generation
        };
        let next_device_generation = if changed {
            device.generation.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-graphics",
                    "graphics-device-generation-exhausted",
                    "logical graphics device generation is exhausted",
                )
            })?
        } else {
            device.generation
        };
        Ok(RuntimeGraphicsTextureStageStateUpdatePlan {
            expected_generation: self.generation,
            next_generation,
            device_id,
            device_generation: device.generation,
            next_device_generation,
            previous,
            update: request.update,
        })
    }

    pub(super) fn preflight_texture_stage_state_update(
        &self,
        plan: &RuntimeGraphicsTextureStageStateUpdatePlan,
    ) -> Result<(), RuntimeStateError> {
        let Some(device) = self.devices.get(&plan.device_id) else {
            return Err(stale_plan("graphics-texture-stage-state-update-plan-stale"));
        };
        let changed = plan.previous != Some(plan.update);
        if self.generation != plan.expected_generation
            || device.generation != plan.device_generation
            || device.texture_stage_states.current_for(plan.update) != plan.previous
            || (changed
                && (self.generation.checked_add(1) != Some(plan.next_generation)
                    || device.generation.checked_add(1) != Some(plan.next_device_generation)))
            || (!changed
                && (plan.next_generation != self.generation
                    || plan.next_device_generation != device.generation))
        {
            return Err(stale_plan("graphics-texture-stage-state-update-plan-stale"));
        }
        self.validate_device_association(device.com_object_id, device.id)
    }

    pub(super) fn commit_texture_stage_state_update(
        &mut self,
        plan: RuntimeGraphicsTextureStageStateUpdatePlan,
    ) -> RuntimeGraphicsTextureStageStateUpdateResult {
        debug_assert!(self.preflight_texture_stage_state_update(&plan).is_ok());
        let changed = plan.previous != Some(plan.update);
        let device = self
            .devices
            .get_mut(&plan.device_id)
            .expect("preflighted graphics device disappeared");
        if changed {
            device.texture_stage_states.apply(plan.update);
            device.generation = plan.next_device_generation;
            self.generation = plan.next_generation;
        }
        RuntimeGraphicsTextureStageStateUpdateResult {
            device_id: device.id,
            update: plan.update,
            changed,
            device_generation: device.generation,
        }
    }

    pub(super) fn plan_texture_binding_update(
        &self,
        request: RuntimeGraphicsTextureBindingUpdateRequest,
    ) -> Result<RuntimeGraphicsTextureBindingUpdatePlan, RuntimeStateError> {
        validate_texture_binding_update_request(request)?;
        let device_id = self
            .device_com_object_index
            .get(&request.device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-texture-binding-device-com-object-unknown",
                    "logical texture binding requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(request.device_com_object_id, device_id)?;
        let texture_id = self.resolve_texture_binding_target(device_id, request)?;
        let device = self.device(device_id)?;
        let previous_texture_id = device.texture_bindings.stage_zero;
        let changed = previous_texture_id != texture_id;
        let next_generation = if changed {
            checked_next_generation(self.generation)?
        } else {
            self.generation
        };
        let next_device_generation = if changed {
            device.generation.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-graphics",
                    "graphics-device-generation-exhausted",
                    "logical graphics device generation is exhausted",
                )
            })?
        } else {
            device.generation
        };
        Ok(RuntimeGraphicsTextureBindingUpdatePlan {
            expected_generation: self.generation,
            next_generation,
            device_id,
            device_generation: device.generation,
            next_device_generation,
            previous_texture_id,
            texture_id,
            request,
        })
    }

    pub(super) fn preflight_texture_binding_update(
        &self,
        plan: &RuntimeGraphicsTextureBindingUpdatePlan,
    ) -> Result<(), RuntimeStateError> {
        let Some(device) = self.devices.get(&plan.device_id) else {
            return Err(stale_plan("graphics-texture-binding-update-plan-stale"));
        };
        let texture_id = self
            .resolve_texture_binding_target(plan.device_id, plan.request)
            .map_err(|_| stale_plan("graphics-texture-binding-update-plan-stale"))?;
        let changed = plan.previous_texture_id != plan.texture_id;
        if self.generation != plan.expected_generation
            || device.generation != plan.device_generation
            || device.texture_bindings.stage_zero != plan.previous_texture_id
            || texture_id != plan.texture_id
            || (changed
                && (self.generation.checked_add(1) != Some(plan.next_generation)
                    || device.generation.checked_add(1) != Some(plan.next_device_generation)))
            || (!changed
                && (plan.next_generation != self.generation
                    || plan.next_device_generation != device.generation))
        {
            return Err(stale_plan("graphics-texture-binding-update-plan-stale"));
        }
        self.validate_device_association(device.com_object_id, device.id)
    }

    pub(super) fn commit_texture_binding_update(
        &mut self,
        plan: RuntimeGraphicsTextureBindingUpdatePlan,
    ) -> RuntimeGraphicsTextureBindingUpdateResult {
        debug_assert!(self.preflight_texture_binding_update(&plan).is_ok());
        let changed = plan.previous_texture_id != plan.texture_id;
        let device = self
            .devices
            .get_mut(&plan.device_id)
            .expect("preflighted graphics device disappeared");
        if changed {
            device.texture_bindings.stage_zero = plan.texture_id;
            device.generation = plan.next_device_generation;
            self.generation = plan.next_generation;
        }
        RuntimeGraphicsTextureBindingUpdateResult {
            device_id: device.id,
            stage: plan.request.stage,
            texture_id: plan.texture_id,
            changed,
            device_generation: device.generation,
        }
    }

    pub(super) fn plan_flexible_vertex_format_update(
        &self,
        request: RuntimeGraphicsFlexibleVertexFormatUpdateRequest,
    ) -> Result<RuntimeGraphicsFlexibleVertexFormatUpdatePlan, RuntimeStateError> {
        validate_flexible_vertex_format_update_request(request)?;
        let device_id = self
            .device_com_object_index
            .get(&request.device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-flexible-vertex-format-device-com-object-unknown",
                    "logical flexible vertex format update requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(request.device_com_object_id, device_id)?;
        let device = self.device(device_id)?;
        let previous = device.flexible_vertex_format;
        let changed = previous != Some(request.format);
        let next_generation = if changed {
            checked_next_generation(self.generation)?
        } else {
            self.generation
        };
        let next_device_generation = if changed {
            device.generation.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-graphics",
                    "graphics-device-generation-exhausted",
                    "logical graphics device generation is exhausted",
                )
            })?
        } else {
            device.generation
        };
        Ok(RuntimeGraphicsFlexibleVertexFormatUpdatePlan {
            expected_generation: self.generation,
            next_generation,
            device_id,
            device_generation: device.generation,
            next_device_generation,
            previous,
            format: request.format,
        })
    }

    pub(super) fn preflight_flexible_vertex_format_update(
        &self,
        plan: &RuntimeGraphicsFlexibleVertexFormatUpdatePlan,
    ) -> Result<(), RuntimeStateError> {
        let Some(device) = self.devices.get(&plan.device_id) else {
            return Err(stale_plan(
                "graphics-flexible-vertex-format-update-plan-stale",
            ));
        };
        let changed = plan.previous != Some(plan.format);
        if self.generation != plan.expected_generation
            || device.generation != plan.device_generation
            || device.flexible_vertex_format != plan.previous
            || (changed
                && (self.generation.checked_add(1) != Some(plan.next_generation)
                    || device.generation.checked_add(1) != Some(plan.next_device_generation)))
            || (!changed
                && (plan.next_generation != self.generation
                    || plan.next_device_generation != device.generation))
        {
            return Err(stale_plan(
                "graphics-flexible-vertex-format-update-plan-stale",
            ));
        }
        self.validate_device_association(device.com_object_id, device.id)
    }

    pub(super) fn commit_flexible_vertex_format_update(
        &mut self,
        plan: RuntimeGraphicsFlexibleVertexFormatUpdatePlan,
    ) -> RuntimeGraphicsFlexibleVertexFormatUpdateResult {
        debug_assert!(self.preflight_flexible_vertex_format_update(&plan).is_ok());
        let changed = plan.previous != Some(plan.format);
        let device = self
            .devices
            .get_mut(&plan.device_id)
            .expect("preflighted graphics device disappeared");
        if changed {
            device.flexible_vertex_format = Some(plan.format);
            device.generation = plan.next_device_generation;
            self.generation = plan.next_generation;
        }
        RuntimeGraphicsFlexibleVertexFormatUpdateResult {
            device_id: device.id,
            format: plan.format,
            changed,
            device_generation: device.generation,
        }
    }

    pub(super) fn plan_stream_source_update(
        &self,
        request: RuntimeGraphicsStreamSourceUpdateRequest,
    ) -> Result<RuntimeGraphicsStreamSourceUpdatePlan, RuntimeStateError> {
        validate_stream_source_update_request(request)?;
        let device_id = self
            .device_com_object_index
            .get(&request.device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-stream-source-device-com-object-unknown",
                    "logical stream-source update requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(request.device_com_object_id, device_id)?;
        let binding = self.resolve_stream_source_binding(device_id, request)?;
        let device = self.device(device_id)?;
        let previous = device.stream_sources.stream_zero;
        let changed = previous != Some(binding);
        let next_generation = if changed {
            checked_next_generation(self.generation)?
        } else {
            self.generation
        };
        let next_device_generation = if changed {
            device.generation.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-graphics",
                    "graphics-device-generation-exhausted",
                    "logical graphics device generation is exhausted",
                )
            })?
        } else {
            device.generation
        };
        Ok(RuntimeGraphicsStreamSourceUpdatePlan {
            expected_generation: self.generation,
            next_generation,
            device_id,
            device_generation: device.generation,
            next_device_generation,
            previous,
            binding,
            request,
        })
    }

    pub(super) fn preflight_stream_source_update(
        &self,
        plan: &RuntimeGraphicsStreamSourceUpdatePlan,
    ) -> Result<(), RuntimeStateError> {
        let Some(device) = self.devices.get(&plan.device_id) else {
            return Err(stale_plan("graphics-stream-source-update-plan-stale"));
        };
        let binding = self
            .resolve_stream_source_binding(plan.device_id, plan.request)
            .map_err(|_| stale_plan("graphics-stream-source-update-plan-stale"))?;
        let changed = plan.previous != Some(plan.binding);
        if self.generation != plan.expected_generation
            || device.generation != plan.device_generation
            || device.stream_sources.stream_zero != plan.previous
            || binding != plan.binding
            || (changed
                && (self.generation.checked_add(1) != Some(plan.next_generation)
                    || device.generation.checked_add(1) != Some(plan.next_device_generation)))
            || (!changed
                && (plan.next_generation != self.generation
                    || plan.next_device_generation != device.generation))
        {
            return Err(stale_plan("graphics-stream-source-update-plan-stale"));
        }
        self.validate_device_association(device.com_object_id, device.id)
    }

    pub(super) fn commit_stream_source_update(
        &mut self,
        plan: RuntimeGraphicsStreamSourceUpdatePlan,
    ) -> RuntimeGraphicsStreamSourceUpdateResult {
        debug_assert!(self.preflight_stream_source_update(&plan).is_ok());
        let changed = plan.previous != Some(plan.binding);
        let device = self
            .devices
            .get_mut(&plan.device_id)
            .expect("preflighted graphics device disappeared");
        if changed {
            device.stream_sources.stream_zero = Some(plan.binding);
            device.generation = plan.next_device_generation;
            self.generation = plan.next_generation;
        }
        RuntimeGraphicsStreamSourceUpdateResult {
            device_id: device.id,
            stream: plan.request.stream,
            binding: plan.binding,
            changed,
            device_generation: device.generation,
        }
    }

    fn resolve_stream_source_binding(
        &self,
        device_id: RuntimeGraphicsDeviceId,
        request: RuntimeGraphicsStreamSourceUpdateRequest,
    ) -> Result<RuntimeGraphicsStreamSourceBinding, RuntimeStateError> {
        let vertex_buffer_id = self
            .vertex_buffer_com_object_index
            .get(&request.vertex_buffer_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-stream-source-vertex-buffer-com-object-unknown",
                    "logical stream-source binding requires a live vertex-buffer COM object",
                )
            })?;
        self.validate_vertex_buffer_association(
            request.vertex_buffer_com_object_id,
            vertex_buffer_id,
        )?;
        let vertex_buffer = self
            .vertex_buffers
            .get(&vertex_buffer_id)
            .expect("validated logical vertex-buffer association disappeared");
        if vertex_buffer.device_id != device_id {
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-stream-source-device-mismatch",
                "logical stream-source binding requires the device that created the vertex buffer",
            ));
        }
        Ok(RuntimeGraphicsStreamSourceBinding {
            vertex_buffer_id,
            offset_in_bytes: request.offset_in_bytes,
            stride: request.stride,
        })
    }

    pub(super) fn plan_draw_capture(
        &self,
        request: RuntimeGraphicsDrawCaptureRequest,
    ) -> Result<RuntimeGraphicsDrawCapturePlan, RuntimeStateError> {
        validate_draw_capture_request(request)?;
        let device_id = self
            .device_com_object_index
            .get(&request.device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-draw-device-com-object-unknown",
                    "logical draw submission requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(request.device_com_object_id, device_id)?;
        let device = self.device(device_id)?;
        let resolved = self.resolve_draw_state(device, request)?;
        let expected_draw_submission_count = device.back_buffer.draw_submissions.len();
        if expected_draw_submission_count >= RUNTIME_GRAPHICS_DRAW_SUBMISSION_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-graphics",
                "graphics-draw-submission-capacity-reached",
                "logical back-buffer draw-submission capacity is exhausted",
            ));
        }
        let expected_draw_vertex_bytes = draw_vertex_bytes(&device.back_buffer)?;
        let vertex_byte_length =
            usize::try_from(resolved.vertex_range.byte_length()).map_err(|_| {
                RuntimeStateError::runtime_defect(
                    "runtime-graphics",
                    "graphics-draw-vertex-byte-length-invalid",
                    "logical draw vertex byte length does not fit host accounting",
                )
            })?;
        let next_draw_vertex_bytes = expected_draw_vertex_bytes
            .checked_add(vertex_byte_length)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-graphics",
                    "graphics-draw-vertex-byte-capacity-reached",
                    "logical back-buffer captured vertex-byte capacity is exhausted",
                )
            })?;
        if next_draw_vertex_bytes > RUNTIME_GRAPHICS_DRAW_VERTEX_BYTES_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-graphics",
                "graphics-draw-vertex-byte-capacity-reached",
                "logical back-buffer captured vertex-byte capacity is exhausted",
            ));
        }
        let next_generation = checked_next_generation(self.generation)?;
        let next_device_generation = device.generation.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-graphics",
                "graphics-device-generation-exhausted",
                "logical graphics device generation is exhausted",
            )
        })?;
        let next_content_generation = device
            .back_buffer
            .content_generation
            .checked_add(1)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-graphics",
                    "graphics-back-buffer-content-generation-exhausted",
                    "logical graphics back-buffer content generation is exhausted",
                )
            })?;
        let next_draw_sequence = self.next_draw_sequence.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-graphics",
                "graphics-draw-sequence-exhausted",
                "logical graphics draw-submission sequence is exhausted",
            )
        })?;
        let vertex_count = request
            .primitive_count
            .checked_add(2)
            .expect("validated triangle-strip quad count must fit u32");
        let submission = RuntimeGraphicsDrawSubmission {
            sequence: self.next_draw_sequence,
            scene_sequence: resolved.scene.sequence,
            primitive_topology: request.primitive_topology,
            start_vertex: request.start_vertex,
            primitive_count: request.primitive_count,
            vertex_count,
            flexible_vertex_format: resolved.flexible_vertex_format,
            stream_source: resolved.stream_source,
            vertex_buffer_content_generation: resolved.vertex_buffer_content_generation,
            vertex_data: Vec::new(),
            render_states: resolved.render_states,
            sampler_states: resolved.sampler_states,
            texture_stage_states: resolved.texture_stage_states,
            texture: resolved.texture,
        };
        Ok(RuntimeGraphicsDrawCapturePlan {
            expected_generation: self.generation,
            next_generation,
            device_id,
            device_generation: device.generation,
            next_device_generation,
            content_generation: device.back_buffer.content_generation,
            next_content_generation,
            expected_draw_submission_count,
            expected_draw_vertex_bytes,
            next_draw_sequence,
            vertex_buffer_id: resolved.vertex_buffer_id,
            vertex_buffer_generation: resolved.vertex_buffer_generation,
            texture_id: submission.texture.texture_id,
            texture_generation: submission.texture.generation,
            vertex_range: resolved.vertex_range,
            submission,
        })
    }

    pub(super) fn prepare_draw_submission(
        &self,
        mut capture: RuntimeGraphicsDrawCapturePlan,
        vertex_data: Vec<u8>,
    ) -> Result<RuntimeGraphicsDrawSubmissionPlan, RuntimeStateError> {
        self.preflight_draw_capture(&capture)?;
        let expected_byte_length =
            usize::try_from(capture.vertex_range.byte_length()).map_err(|_| {
                RuntimeStateError::runtime_defect(
                    "runtime-graphics",
                    "graphics-draw-vertex-byte-length-invalid",
                    "logical draw vertex byte length does not fit host accounting",
                )
            })?;
        if vertex_data.len() != expected_byte_length {
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-draw-vertex-snapshot-length-mismatch",
                "logical draw submission requires the complete planned vertex snapshot",
            ));
        }
        capture.submission.vertex_data = vertex_data;
        Ok(RuntimeGraphicsDrawSubmissionPlan { capture })
    }

    fn preflight_draw_capture(
        &self,
        plan: &RuntimeGraphicsDrawCapturePlan,
    ) -> Result<(), RuntimeStateError> {
        let Some(device) = self.devices.get(&plan.device_id) else {
            return Err(stale_plan("graphics-draw-submission-plan-stale"));
        };
        let request = RuntimeGraphicsDrawCaptureRequest {
            device_com_object_id: device.com_object_id,
            primitive_topology: plan.submission.primitive_topology,
            start_vertex: plan.submission.start_vertex,
            primitive_count: plan.submission.primitive_count,
        };
        let resolved = self
            .resolve_draw_state(device, request)
            .map_err(|_| stale_plan("graphics-draw-submission-plan-stale"))?;
        let draw_vertex_bytes = draw_vertex_bytes(&device.back_buffer)
            .map_err(|_| stale_plan("graphics-draw-submission-plan-stale"))?;
        if self.generation != plan.expected_generation
            || self.generation.checked_add(1) != Some(plan.next_generation)
            || device.generation != plan.device_generation
            || device.generation.checked_add(1) != Some(plan.next_device_generation)
            || device.back_buffer.content_generation != plan.content_generation
            || device.back_buffer.content_generation.checked_add(1)
                != Some(plan.next_content_generation)
            || device.back_buffer.draw_submissions.len() != plan.expected_draw_submission_count
            || draw_vertex_bytes != plan.expected_draw_vertex_bytes
            || plan.expected_draw_submission_count >= RUNTIME_GRAPHICS_DRAW_SUBMISSION_LIMIT
            || plan
                .expected_draw_vertex_bytes
                .checked_add(plan.vertex_range.byte_length() as usize)
                .is_none_or(|bytes| bytes > RUNTIME_GRAPHICS_DRAW_VERTEX_BYTES_LIMIT)
            || plan.submission.sequence != self.next_draw_sequence
            || self.next_draw_sequence.checked_add(1) != Some(plan.next_draw_sequence)
            || plan.submission.scene_sequence != resolved.scene.sequence
            || plan.submission.vertex_count != plan.submission.primitive_count + 2
            || plan.submission.flexible_vertex_format != resolved.flexible_vertex_format
            || plan.submission.stream_source != resolved.stream_source
            || plan.submission.vertex_buffer_content_generation
                != resolved.vertex_buffer_content_generation
            || plan.submission.render_states != resolved.render_states
            || plan.submission.sampler_states != resolved.sampler_states
            || plan.submission.texture_stage_states != resolved.texture_stage_states
            || plan.submission.texture != resolved.texture
            || plan.vertex_buffer_id != resolved.vertex_buffer_id
            || plan.vertex_buffer_generation != resolved.vertex_buffer_generation
            || plan.texture_id != resolved.texture.texture_id
            || plan.texture_generation != resolved.texture.generation
            || plan.vertex_range != resolved.vertex_range
        {
            return Err(stale_plan("graphics-draw-submission-plan-stale"));
        }
        self.validate_device_association(device.com_object_id, device.id)
    }

    pub(super) fn preflight_draw_submission(
        &self,
        plan: &RuntimeGraphicsDrawSubmissionPlan,
    ) -> Result<(), RuntimeStateError> {
        self.preflight_draw_capture(&plan.capture)?;
        if plan.capture.submission.vertex_data.len()
            != plan.capture.vertex_range.byte_length() as usize
        {
            return Err(stale_plan("graphics-draw-submission-plan-stale"));
        }
        Ok(())
    }

    pub(super) fn commit_draw_submission(
        &mut self,
        plan: RuntimeGraphicsDrawSubmissionPlan,
    ) -> RuntimeGraphicsDrawSubmissionResult {
        debug_assert!(self.preflight_draw_submission(&plan).is_ok());
        let capture = plan.capture;
        let submission = capture.submission;
        let device = self
            .devices
            .get_mut(&capture.device_id)
            .expect("preflighted graphics device disappeared");
        device.back_buffer.content_generation = capture.next_content_generation;
        device.back_buffer.draw_submissions.push(submission.clone());
        device.generation = capture.next_device_generation;
        self.next_draw_sequence = capture.next_draw_sequence;
        self.generation = capture.next_generation;
        RuntimeGraphicsDrawSubmissionResult {
            device_id: device.id,
            device_generation: device.generation,
            content_generation: device.back_buffer.content_generation,
            submission,
        }
    }

    fn resolve_draw_state(
        &self,
        device: &RuntimeGraphicsDevice,
        request: RuntimeGraphicsDrawCaptureRequest,
    ) -> Result<RuntimeGraphicsDrawResolvedState, RuntimeStateError> {
        validate_supported_draw_device_state(device)?;
        let scene = device.active_scene.ok_or_else(|| {
            RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-draw-scene-inactive",
                "logical draw submission requires an active device scene",
            )
        })?;
        let flexible_vertex_format = device.flexible_vertex_format.ok_or_else(|| {
            RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-draw-flexible-vertex-format-missing",
                "logical draw submission requires the supported typed vertex format",
            )
        })?;
        let stream_source = device.stream_sources.stream_zero.ok_or_else(|| {
            RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-draw-stream-source-missing",
                "logical draw submission requires a live stream-zero vertex buffer",
            )
        })?;
        let vertex_buffer = self.vertex_buffer(stream_source.vertex_buffer_id)?;
        self.validate_vertex_buffer_association(vertex_buffer.com_object_id, vertex_buffer.id)?;
        if vertex_buffer.device_id != device.id
            || vertex_buffer.active_lock.is_some()
            || vertex_buffer.content_generation == 0
            || vertex_buffer.descriptor.byte_length != 112
            || vertex_buffer.descriptor.usage != RuntimeGraphicsVertexBufferUsage::WriteOnly
            || vertex_buffer.descriptor.flexible_vertex_format != 0x144
            || vertex_buffer.descriptor.pool != RuntimeGraphicsVertexBufferPool::Managed
            || stream_source.offset_in_bytes != 0
            || stream_source.stride != 28
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-draw-vertex-buffer-state-invalid",
                "logical draw submission requires the reached unlocked initialized quad vertex buffer",
            ));
        }
        let vertex_count = request.primitive_count.checked_add(2).ok_or_else(|| {
            RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-draw-vertex-count-overflow",
                "logical draw vertex count overflowed",
            )
        })?;
        let start_byte = u64::from(stream_source.offset_in_bytes)
            .checked_add(u64::from(request.start_vertex) * u64::from(stream_source.stride))
            .ok_or_else(|| {
                RuntimeStateError::invalid_input(
                    "runtime-graphics",
                    "graphics-draw-vertex-range-overflow",
                    "logical draw vertex offset overflowed",
                )
            })?;
        let vertex_byte_length = u64::from(vertex_count)
            .checked_mul(u64::from(stream_source.stride))
            .ok_or_else(|| {
                RuntimeStateError::invalid_input(
                    "runtime-graphics",
                    "graphics-draw-vertex-range-overflow",
                    "logical draw vertex byte length overflowed",
                )
            })?;
        let end_byte = start_byte.checked_add(vertex_byte_length).ok_or_else(|| {
            RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-draw-vertex-range-overflow",
                "logical draw vertex range overflowed",
            )
        })?;
        if end_byte > u64::from(vertex_buffer.descriptor.byte_length)
            || end_byte > vertex_buffer.backing.range.byte_length()
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-draw-vertex-range-out-of-bounds",
                "logical draw vertex range exceeds the bound vertex buffer",
            ));
        }
        let vertex_address = u64::from(vertex_buffer.backing.range.start().0)
            .checked_add(start_byte)
            .and_then(|address| u32::try_from(address).ok())
            .map(GuestAddress)
            .ok_or_else(|| {
                RuntimeStateError::invalid_input(
                    "runtime-graphics",
                    "graphics-draw-vertex-address-overflow",
                    "logical draw vertex address exceeds the x86 guest address space",
                )
            })?;
        let vertex_range = GuestRange::new(vertex_address, vertex_byte_length)
            .map_err(RuntimeStateError::from_memory)?;
        let texture_id = device.texture_bindings.stage_zero.ok_or_else(|| {
            RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-draw-texture-missing",
                "logical draw submission requires a live stage-zero texture",
            )
        })?;
        let texture = self.texture(texture_id)?;
        self.validate_texture_association(texture.com_object_id, texture.id)?;
        if texture.device_id != device.id
            || texture.descriptor.levels != 1
            || texture.descriptor.usage != RuntimeGraphicsTextureUsage::None
            || texture.descriptor.pool != RuntimeGraphicsTexturePool::Default
            || texture.descriptor.color_format != RuntimeGraphicsColorFormat::B8g8r8x8Unorm
            || texture.content.content_generation == 0
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-draw-texture-state-invalid",
                "logical draw submission requires the reached initialized static DEFAULT texture",
            ));
        }
        Ok(RuntimeGraphicsDrawResolvedState {
            scene,
            flexible_vertex_format,
            stream_source,
            vertex_buffer_id: vertex_buffer.id,
            vertex_buffer_generation: vertex_buffer.generation,
            vertex_buffer_content_generation: vertex_buffer.content_generation,
            vertex_range,
            render_states: device.render_states.clone(),
            sampler_states: device.sampler_states.clone(),
            texture_stage_states: device.texture_stage_states.clone(),
            texture: RuntimeGraphicsDrawTextureSnapshot {
                texture_id: texture.id,
                descriptor: texture.descriptor,
                generation: texture.generation,
                content: texture.content.clone(),
            },
        })
    }

    fn resolve_texture_binding_target(
        &self,
        device_id: RuntimeGraphicsDeviceId,
        request: RuntimeGraphicsTextureBindingUpdateRequest,
    ) -> Result<Option<RuntimeGraphicsTextureId>, RuntimeStateError> {
        let Some(texture_com_object_id) = request.texture_com_object_id else {
            return Ok(None);
        };
        let texture_id = self
            .texture_com_object_index
            .get(&texture_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-texture-binding-texture-com-object-unknown",
                    "logical texture binding requires a live texture COM object",
                )
            })?;
        self.validate_texture_association(texture_com_object_id, texture_id)?;
        let texture = self
            .textures
            .get(&texture_id)
            .expect("validated logical texture association disappeared");
        if texture.device_id != device_id {
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-texture-binding-device-mismatch",
                "logical texture binding requires the device that created the texture",
            ));
        }
        Ok(Some(texture_id))
    }

    pub(super) fn plan_back_buffer_clear(
        &self,
        request: RuntimeGraphicsBackBufferClearRequest,
    ) -> Result<RuntimeGraphicsBackBufferClearPlan, RuntimeStateError> {
        validate_back_buffer_clear_request(request)?;
        let device_id = self
            .device_com_object_index
            .get(&request.device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-back-buffer-clear-device-com-object-unknown",
                    "logical back-buffer clear requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(request.device_com_object_id, device_id)?;
        let device = self.device(device_id)?;
        let previous_draw_submission_count = device.back_buffer.draw_submissions.len();
        let previous_draw_vertex_bytes = draw_vertex_bytes(&device.back_buffer)?;
        let next_generation = checked_next_generation(self.generation)?;
        let next_device_generation = device.generation.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-graphics",
                "graphics-device-generation-exhausted",
                "logical graphics device generation is exhausted",
            )
        })?;
        let next_content_generation = device
            .back_buffer
            .content_generation
            .checked_add(1)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-graphics",
                    "graphics-back-buffer-content-generation-exhausted",
                    "logical graphics back-buffer content generation is exhausted",
                )
            })?;
        let clear = RuntimeGraphicsBackBufferClear {
            sequence: self.next_back_buffer_clear_sequence,
            rect: full_back_buffer_rect(device.presentation)?,
            color_argb: request.color_argb,
        };
        let next_clear_sequence = self
            .next_back_buffer_clear_sequence
            .checked_add(1)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-graphics",
                    "graphics-back-buffer-clear-sequence-exhausted",
                    "logical graphics back-buffer clear sequence is exhausted",
                )
            })?;
        Ok(RuntimeGraphicsBackBufferClearPlan {
            expected_generation: self.generation,
            next_generation,
            device_id,
            device_generation: device.generation,
            next_device_generation,
            content_generation: device.back_buffer.content_generation,
            next_content_generation,
            previous_uniform_clear: device.back_buffer.uniform_clear,
            previous_draw_submission_count,
            previous_draw_vertex_bytes,
            clear,
            next_clear_sequence,
        })
    }

    pub(super) fn preflight_back_buffer_clear(
        &self,
        plan: &RuntimeGraphicsBackBufferClearPlan,
    ) -> Result<(), RuntimeStateError> {
        let Some(device) = self.devices.get(&plan.device_id) else {
            return Err(stale_plan("graphics-back-buffer-clear-plan-stale"));
        };
        if self.generation != plan.expected_generation
            || self.generation.checked_add(1) != Some(plan.next_generation)
            || device.generation != plan.device_generation
            || device.generation.checked_add(1) != Some(plan.next_device_generation)
            || device.back_buffer.content_generation != plan.content_generation
            || device.back_buffer.content_generation.checked_add(1)
                != Some(plan.next_content_generation)
            || device.back_buffer.uniform_clear != plan.previous_uniform_clear
            || device.back_buffer.draw_submissions.len() != plan.previous_draw_submission_count
            || draw_vertex_bytes(&device.back_buffer)? != plan.previous_draw_vertex_bytes
            || plan.clear.sequence != self.next_back_buffer_clear_sequence
            || self.next_back_buffer_clear_sequence.checked_add(1) != Some(plan.next_clear_sequence)
            || plan.clear.rect != full_back_buffer_rect(device.presentation)?
        {
            return Err(stale_plan("graphics-back-buffer-clear-plan-stale"));
        }
        self.validate_device_association(device.com_object_id, device.id)
    }

    pub(super) fn commit_back_buffer_clear(
        &mut self,
        plan: RuntimeGraphicsBackBufferClearPlan,
    ) -> RuntimeGraphicsBackBufferClearResult {
        debug_assert!(self.preflight_back_buffer_clear(&plan).is_ok());
        let device = self
            .devices
            .get_mut(&plan.device_id)
            .expect("preflighted graphics device disappeared");
        device.back_buffer.content_generation = plan.next_content_generation;
        device.back_buffer.uniform_clear = Some(plan.clear);
        device.back_buffer.draw_submissions.clear();
        device.generation = plan.next_device_generation;
        self.next_back_buffer_clear_sequence = plan.next_clear_sequence;
        self.generation = plan.next_generation;
        RuntimeGraphicsBackBufferClearResult {
            device_id: device.id,
            device_generation: device.generation,
            content_generation: device.back_buffer.content_generation,
            clear: plan.clear,
        }
    }

    pub(super) fn plan_scene_begin(
        &self,
        request: RuntimeGraphicsSceneBeginRequest,
    ) -> Result<RuntimeGraphicsSceneBeginPlan, RuntimeStateError> {
        if request.device_com_object_id.0 == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-scene-begin-request-invalid",
                "logical scene begin requires a non-null graphics device COM object",
            ));
        }
        let device_id = self
            .device_com_object_index
            .get(&request.device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-scene-begin-device-com-object-unknown",
                    "logical scene begin requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(request.device_com_object_id, device_id)?;
        let device = self.device(device_id)?;
        if device.active_scene.is_some() {
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-scene-already-active",
                "logical graphics device cannot begin a nested scene",
            ));
        }
        let next_generation = checked_next_generation(self.generation)?;
        let next_device_generation = device.generation.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-graphics",
                "graphics-device-generation-exhausted",
                "logical graphics device generation is exhausted",
            )
        })?;
        let scene = RuntimeGraphicsScene {
            sequence: self.next_scene_sequence,
        };
        let next_scene_sequence = self.next_scene_sequence.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-graphics",
                "graphics-scene-sequence-exhausted",
                "logical graphics scene sequence is exhausted",
            )
        })?;
        Ok(RuntimeGraphicsSceneBeginPlan {
            expected_generation: self.generation,
            next_generation,
            device_id,
            device_generation: device.generation,
            next_device_generation,
            scene,
            next_scene_sequence,
        })
    }

    pub(super) fn preflight_scene_begin(
        &self,
        plan: &RuntimeGraphicsSceneBeginPlan,
    ) -> Result<(), RuntimeStateError> {
        let Some(device) = self.devices.get(&plan.device_id) else {
            return Err(stale_plan("graphics-scene-begin-plan-stale"));
        };
        if self.generation != plan.expected_generation
            || self.generation.checked_add(1) != Some(plan.next_generation)
            || device.generation != plan.device_generation
            || device.generation.checked_add(1) != Some(plan.next_device_generation)
            || device.active_scene.is_some()
            || plan.scene.sequence != self.next_scene_sequence
            || self.next_scene_sequence.checked_add(1) != Some(plan.next_scene_sequence)
        {
            return Err(stale_plan("graphics-scene-begin-plan-stale"));
        }
        self.validate_device_association(device.com_object_id, device.id)
    }

    pub(super) fn commit_scene_begin(
        &mut self,
        plan: RuntimeGraphicsSceneBeginPlan,
    ) -> RuntimeGraphicsSceneBeginResult {
        debug_assert!(self.preflight_scene_begin(&plan).is_ok());
        let device = self
            .devices
            .get_mut(&plan.device_id)
            .expect("preflighted graphics device disappeared");
        device.active_scene = Some(plan.scene);
        device.generation = plan.next_device_generation;
        self.next_scene_sequence = plan.next_scene_sequence;
        self.generation = plan.next_generation;
        RuntimeGraphicsSceneBeginResult {
            device_id: device.id,
            device_generation: device.generation,
            scene: plan.scene,
        }
    }

    pub(super) fn plan_scene_end(
        &self,
        request: RuntimeGraphicsSceneEndRequest,
    ) -> Result<RuntimeGraphicsSceneEndPlan, RuntimeStateError> {
        if request.device_com_object_id.0 == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-scene-end-request-invalid",
                "logical scene end requires a non-null graphics device COM object",
            ));
        }
        let device_id = self
            .device_com_object_index
            .get(&request.device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-scene-end-device-com-object-unknown",
                    "logical scene end requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(request.device_com_object_id, device_id)?;
        let device = self.device(device_id)?;
        let scene = device.active_scene.ok_or_else(|| {
            RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-scene-not-active",
                "logical graphics device cannot end an inactive scene",
            )
        })?;
        let next_generation = checked_next_generation(self.generation)?;
        let next_device_generation = device.generation.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-graphics",
                "graphics-device-generation-exhausted",
                "logical graphics device generation is exhausted",
            )
        })?;
        Ok(RuntimeGraphicsSceneEndPlan {
            expected_generation: self.generation,
            next_generation,
            device_id,
            device_generation: device.generation,
            next_device_generation,
            scene,
        })
    }

    pub(super) fn preflight_scene_end(
        &self,
        plan: &RuntimeGraphicsSceneEndPlan,
    ) -> Result<(), RuntimeStateError> {
        let Some(device) = self.devices.get(&plan.device_id) else {
            return Err(stale_plan("graphics-scene-end-plan-stale"));
        };
        if self.generation != plan.expected_generation
            || self.generation.checked_add(1) != Some(plan.next_generation)
            || device.generation != plan.device_generation
            || device.generation.checked_add(1) != Some(plan.next_device_generation)
            || device.active_scene != Some(plan.scene)
        {
            return Err(stale_plan("graphics-scene-end-plan-stale"));
        }
        self.validate_device_association(device.com_object_id, device.id)
    }

    pub(super) fn commit_scene_end(
        &mut self,
        plan: RuntimeGraphicsSceneEndPlan,
    ) -> RuntimeGraphicsSceneEndResult {
        debug_assert!(self.preflight_scene_end(&plan).is_ok());
        let device = self
            .devices
            .get_mut(&plan.device_id)
            .expect("preflighted graphics device disappeared");
        device.active_scene = None;
        device.generation = plan.next_device_generation;
        self.generation = plan.next_generation;
        RuntimeGraphicsSceneEndResult {
            device_id: device.id,
            device_generation: device.generation,
            scene: plan.scene,
        }
    }

    pub(super) fn plan_present(
        &self,
        request: RuntimeGraphicsPresentRequest,
    ) -> Result<RuntimeGraphicsPresentPlan, RuntimeStateError> {
        if request.device_com_object_id.0 == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-present-request-invalid",
                "logical presentation requires a non-null graphics device COM object",
            ));
        }
        let device_id = self
            .device_com_object_index
            .get(&request.device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-present-device-com-object-unknown",
                    "logical presentation requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(request.device_com_object_id, device_id)?;
        let device = self.device(device_id)?;
        if device.active_scene.is_some() {
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-present-scene-active",
                "logical presentation requires a closed graphics scene",
            ));
        }
        if device.back_buffer.uniform_clear.is_none()
            && device.back_buffer.draw_submissions.is_empty()
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-present-back-buffer-empty",
                "logical presentation requires prepared back-buffer content",
            ));
        }
        let next_generation = checked_next_generation(self.generation)?;
        let next_device_generation = device.generation.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-graphics",
                "graphics-device-generation-exhausted",
                "logical graphics device generation is exhausted",
            )
        })?;
        let next_back_buffer_content_generation = device
            .back_buffer
            .content_generation
            .checked_add(1)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-graphics",
                    "graphics-back-buffer-content-generation-exhausted",
                    "logical graphics back-buffer content generation is exhausted",
                )
            })?;
        let frame = RuntimeGraphicsPresentedFrame {
            sequence: self.next_present_sequence,
            back_buffer: device.back_buffer.clone(),
        };
        let next_present_sequence = self.next_present_sequence.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-graphics",
                "graphics-present-sequence-exhausted",
                "logical graphics presentation sequence is exhausted",
            )
        })?;
        Ok(RuntimeGraphicsPresentPlan {
            expected_generation: self.generation,
            next_generation,
            device_id,
            device_generation: device.generation,
            next_device_generation,
            next_back_buffer_content_generation,
            previous_presented_frame: device.presented_frame.clone(),
            frame,
            next_present_sequence,
        })
    }

    pub(super) fn preflight_present(
        &self,
        plan: &RuntimeGraphicsPresentPlan,
    ) -> Result<(), RuntimeStateError> {
        let Some(device) = self.devices.get(&plan.device_id) else {
            return Err(stale_plan("graphics-present-plan-stale"));
        };
        if self.generation != plan.expected_generation
            || self.generation.checked_add(1) != Some(plan.next_generation)
            || device.generation != plan.device_generation
            || device.generation.checked_add(1) != Some(plan.next_device_generation)
            || device.active_scene.is_some()
            || device.presented_frame != plan.previous_presented_frame
            || plan.frame.sequence != self.next_present_sequence
            || plan.frame.back_buffer != device.back_buffer
            || device.back_buffer.content_generation.checked_add(1)
                != Some(plan.next_back_buffer_content_generation)
            || self.next_present_sequence.checked_add(1) != Some(plan.next_present_sequence)
        {
            return Err(stale_plan("graphics-present-plan-stale"));
        }
        if device.back_buffer.uniform_clear.is_none()
            && device.back_buffer.draw_submissions.is_empty()
        {
            return Err(stale_plan("graphics-present-plan-stale"));
        }
        self.validate_device_association(device.com_object_id, device.id)
    }

    pub(super) fn commit_present(
        &mut self,
        plan: RuntimeGraphicsPresentPlan,
    ) -> RuntimeGraphicsPresentResult {
        debug_assert!(self.preflight_present(&plan).is_ok());
        let device = self
            .devices
            .get_mut(&plan.device_id)
            .expect("preflighted graphics device disappeared");
        device.presented_frame = Some(plan.frame.clone());
        device.back_buffer = RuntimeGraphicsBackBuffer {
            content_generation: plan.next_back_buffer_content_generation,
            uniform_clear: None,
            draw_submissions: Vec::new(),
        };
        device.generation = plan.next_device_generation;
        self.next_present_sequence = plan.next_present_sequence;
        self.generation = plan.next_generation;
        RuntimeGraphicsPresentResult {
            device_id: device.id,
            device_generation: device.generation,
            frame: plan.frame,
            next_back_buffer_content_generation: device.back_buffer.content_generation,
        }
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
            dirty_region: RuntimeGraphicsTextureDirtyRegion::WholeTexture,
            content: RuntimeGraphicsTextureContent::default(),
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

    pub(super) fn plan_texture_dirty_region_mark(
        &self,
        request: RuntimeGraphicsTextureDirtyRegionMarkRequest,
    ) -> Result<RuntimeGraphicsTextureDirtyRegionMarkPlan, RuntimeStateError> {
        validate_texture_dirty_region_mark_request(request)?;
        let texture_id = self
            .texture_com_object_index
            .get(&request.texture_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-texture-dirty-region-com-object-unknown",
                    "logical texture dirty-region mark requires a live texture COM object",
                )
            })?;
        self.validate_texture_association(request.texture_com_object_id, texture_id)?;
        let texture = self
            .textures
            .get(&texture_id)
            .expect("validated logical texture association disappeared");
        let dirty_region =
            merged_texture_dirty_region(texture.dirty_region, request.rect, texture.descriptor)?;
        let changed = dirty_region != texture.dirty_region;
        let next_generation = if changed {
            checked_next_generation(self.generation)?
        } else {
            self.generation
        };
        let next_texture_generation = if changed {
            texture.generation.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-graphics",
                    "graphics-texture-generation-exhausted",
                    "logical graphics texture generation is exhausted",
                )
            })?
        } else {
            texture.generation
        };
        Ok(RuntimeGraphicsTextureDirtyRegionMarkPlan {
            expected_generation: self.generation,
            next_generation,
            texture_id,
            texture_generation: texture.generation,
            next_texture_generation,
            previous_dirty_region: texture.dirty_region,
            dirty_region,
            request,
        })
    }

    pub(super) fn preflight_texture_dirty_region_mark(
        &self,
        plan: &RuntimeGraphicsTextureDirtyRegionMarkPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation {
            return Err(stale_plan("graphics-texture-dirty-region-plan-stale"));
        }
        let texture = self
            .textures
            .get(&plan.texture_id)
            .ok_or_else(|| stale_plan("graphics-texture-dirty-region-plan-stale"))?;
        let dirty_region = merged_texture_dirty_region(
            texture.dirty_region,
            plan.request.rect,
            texture.descriptor,
        )?;
        let changed = dirty_region != texture.dirty_region;
        if texture.com_object_id != plan.request.texture_com_object_id
            || texture.generation != plan.texture_generation
            || texture.dirty_region != plan.previous_dirty_region
            || dirty_region != plan.dirty_region
            || (changed
                && (self.generation.checked_add(1) != Some(plan.next_generation)
                    || texture.generation.checked_add(1) != Some(plan.next_texture_generation)))
            || (!changed
                && (plan.next_generation != self.generation
                    || plan.next_texture_generation != texture.generation))
        {
            return Err(stale_plan("graphics-texture-dirty-region-plan-stale"));
        }
        self.validate_texture_association(texture.com_object_id, texture.id)
    }

    pub(super) fn commit_texture_dirty_region_mark(
        &mut self,
        plan: RuntimeGraphicsTextureDirtyRegionMarkPlan,
    ) -> RuntimeGraphicsTextureDirtyRegionMarkResult {
        debug_assert!(self.preflight_texture_dirty_region_mark(&plan).is_ok());
        let texture = self
            .textures
            .get_mut(&plan.texture_id)
            .expect("preflighted graphics texture disappeared");
        let changed = texture.dirty_region != plan.dirty_region;
        if changed {
            texture.dirty_region = plan.dirty_region;
            texture.generation = plan.next_texture_generation;
            self.generation = plan.next_generation;
        }
        RuntimeGraphicsTextureDirtyRegionMarkResult {
            texture_id: texture.id,
            dirty_region: texture.dirty_region,
            changed,
            texture_generation: texture.generation,
        }
    }

    pub(super) fn plan_texture_update(
        &self,
        request: RuntimeGraphicsTextureUpdateRequest,
    ) -> Result<RuntimeGraphicsTextureUpdatePlan, RuntimeStateError> {
        validate_texture_update_request(request)?;
        let device_id = self
            .device_com_object_index
            .get(&request.device_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-texture-update-device-com-object-unknown",
                    "logical texture update requires a live graphics device COM object",
                )
            })?;
        self.validate_device_association(request.device_com_object_id, device_id)?;
        let source_texture_id = self
            .texture_com_object_index
            .get(&request.source_texture_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-texture-update-source-com-object-unknown",
                    "logical texture update requires a live source texture COM object",
                )
            })?;
        let destination_texture_id = self
            .texture_com_object_index
            .get(&request.destination_texture_com_object_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-texture-update-destination-com-object-unknown",
                    "logical texture update requires a live destination texture COM object",
                )
            })?;
        self.validate_texture_association(request.source_texture_com_object_id, source_texture_id)?;
        self.validate_texture_association(
            request.destination_texture_com_object_id,
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
        validate_texture_update_pair(device_id, source, destination)?;
        if matches!(
            source.dirty_region,
            RuntimeGraphicsTextureDirtyRegion::Rect(_)
        ) {
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-texture-update-partial-dirty-region-unsupported",
                "logical texture update currently requires a whole-texture or clean source",
            ));
        }
        let transferred = source.dirty_region != RuntimeGraphicsTextureDirtyRegion::Clean;
        let (
            next_generation,
            next_source_texture_generation,
            next_destination_texture_generation,
            next_destination_content_generation,
            destination_fills,
            update,
            next_update_sequence,
        ) = if transferred {
            let next_generation = checked_next_generation(self.generation)?;
            let next_source_texture_generation =
                checked_texture_generation(source.generation, "source")?;
            let next_destination_texture_generation =
                checked_texture_generation(destination.generation, "destination")?;
            let next_destination_content_generation = destination
                .content
                .content_generation
                .checked_add(1)
                .ok_or_else(|| {
                    RuntimeStateError::resource_limit(
                        "runtime-graphics",
                        "graphics-texture-content-generation-exhausted",
                        "logical destination texture content generation is exhausted",
                    )
                })?;
            let next_update_sequence = self
                .next_texture_update_sequence
                .checked_add(1)
                .ok_or_else(|| {
                    RuntimeStateError::resource_limit(
                        "runtime-graphics",
                        "graphics-texture-update-sequence-exhausted",
                        "logical texture update sequence is exhausted",
                    )
                })?;
            (
                next_generation,
                next_source_texture_generation,
                next_destination_texture_generation,
                next_destination_content_generation,
                source.content.fills.clone(),
                Some(RuntimeGraphicsTextureUpdate {
                    sequence: self.next_texture_update_sequence,
                    source_texture_id,
                    source_content_generation: source.content.content_generation,
                    dirty_region: source.dirty_region,
                }),
                next_update_sequence,
            )
        } else {
            (
                self.generation,
                source.generation,
                destination.generation,
                destination.content.content_generation,
                Vec::new(),
                None,
                self.next_texture_update_sequence,
            )
        };
        Ok(RuntimeGraphicsTextureUpdatePlan {
            expected_generation: self.generation,
            next_generation,
            device_id,
            source_texture_id,
            source_texture_generation: source.generation,
            next_source_texture_generation,
            source_dirty_region: source.dirty_region,
            source_content_generation: source.content.content_generation,
            destination_texture_id,
            destination_texture_generation: destination.generation,
            next_destination_texture_generation,
            destination_content_generation: destination.content.content_generation,
            next_destination_content_generation,
            destination_fills,
            update,
            next_update_sequence,
            request,
        })
    }

    pub(super) fn preflight_texture_update(
        &self,
        plan: &RuntimeGraphicsTextureUpdatePlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation {
            return Err(stale_plan("graphics-texture-update-plan-stale"));
        }
        validate_texture_update_request(plan.request)?;
        let source = self
            .textures
            .get(&plan.source_texture_id)
            .ok_or_else(|| stale_plan("graphics-texture-update-plan-stale"))?;
        let destination = self
            .textures
            .get(&plan.destination_texture_id)
            .ok_or_else(|| stale_plan("graphics-texture-update-plan-stale"))?;
        self.validate_device_association(plan.request.device_com_object_id, plan.device_id)?;
        self.validate_texture_association(
            plan.request.source_texture_com_object_id,
            plan.source_texture_id,
        )?;
        self.validate_texture_association(
            plan.request.destination_texture_com_object_id,
            plan.destination_texture_id,
        )?;
        validate_texture_update_pair(plan.device_id, source, destination)?;
        if source.generation != plan.source_texture_generation
            || source.dirty_region != plan.source_dirty_region
            || source.content.content_generation != plan.source_content_generation
            || destination.generation != plan.destination_texture_generation
            || destination.content.content_generation != plan.destination_content_generation
        {
            return Err(stale_plan("graphics-texture-update-plan-stale"));
        }
        let transferred = source.dirty_region != RuntimeGraphicsTextureDirtyRegion::Clean;
        if transferred {
            if matches!(
                source.dirty_region,
                RuntimeGraphicsTextureDirtyRegion::Rect(_)
            ) || self.generation.checked_add(1) != Some(plan.next_generation)
                || source.generation.checked_add(1) != Some(plan.next_source_texture_generation)
                || destination.generation.checked_add(1)
                    != Some(plan.next_destination_texture_generation)
                || destination.content.content_generation.checked_add(1)
                    != Some(plan.next_destination_content_generation)
                || plan.destination_fills != source.content.fills
                || plan.update
                    != Some(RuntimeGraphicsTextureUpdate {
                        sequence: self.next_texture_update_sequence,
                        source_texture_id: source.id,
                        source_content_generation: source.content.content_generation,
                        dirty_region: source.dirty_region,
                    })
                || self.next_texture_update_sequence.checked_add(1)
                    != Some(plan.next_update_sequence)
            {
                return Err(stale_plan("graphics-texture-update-plan-stale"));
            }
        } else if plan.next_generation != self.generation
            || plan.next_source_texture_generation != source.generation
            || plan.next_destination_texture_generation != destination.generation
            || plan.next_destination_content_generation != destination.content.content_generation
            || !plan.destination_fills.is_empty()
            || plan.update.is_some()
            || plan.next_update_sequence != self.next_texture_update_sequence
        {
            return Err(stale_plan("graphics-texture-update-plan-stale"));
        }
        Ok(())
    }

    pub(super) fn commit_texture_update(
        &mut self,
        plan: RuntimeGraphicsTextureUpdatePlan,
    ) -> RuntimeGraphicsTextureUpdateResult {
        debug_assert!(self.preflight_texture_update(&plan).is_ok());
        let transferred = plan.update.is_some();
        if transferred {
            let source = self
                .textures
                .get_mut(&plan.source_texture_id)
                .expect("preflighted source texture disappeared");
            source.dirty_region = RuntimeGraphicsTextureDirtyRegion::Clean;
            source.generation = plan.next_source_texture_generation;
            let destination = self
                .textures
                .get_mut(&plan.destination_texture_id)
                .expect("preflighted destination texture disappeared");
            destination.content.fills = plan.destination_fills;
            destination.content.content_generation = plan.next_destination_content_generation;
            destination.content.last_update = plan.update;
            destination.generation = plan.next_destination_texture_generation;
            self.next_texture_update_sequence = plan.next_update_sequence;
            self.generation = plan.next_generation;
        }
        let source = self
            .textures
            .get(&plan.source_texture_id)
            .expect("preflighted source texture disappeared after update");
        let destination = self
            .textures
            .get(&plan.destination_texture_id)
            .expect("preflighted destination texture disappeared after update");
        RuntimeGraphicsTextureUpdateResult {
            source_texture_id: source.id,
            destination_texture_id: destination.id,
            transferred,
            source_texture_generation: source.generation,
            destination_texture_generation: destination.generation,
            destination_content_generation: destination.content.content_generation,
            update: plan.update,
        }
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
        if surface.descriptor.level != 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-surface-fill-level-unsupported",
                "logical graphics surface fills currently require a level-zero texture view",
            ));
        }
        let texture = self.textures.get(&surface.texture_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-surface-fill-texture-missing",
                "logical graphics surface fill has no live texture authority",
            )
        })?;
        let empty =
            request.rect.left == request.rect.right || request.rect.top == request.rect.bottom;
        if !empty && texture.content.fills.len() >= RUNTIME_GRAPHICS_SURFACE_FILL_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-graphics",
                "graphics-surface-fill-capacity-reached",
                "logical graphics texture fill history capacity is exhausted",
            ));
        }
        let fill = (!empty).then_some(RuntimeGraphicsSurfaceFill {
            sequence: self.next_surface_fill_sequence,
            rect: request.rect,
            color_ref: request.color_ref,
        });
        let (
            next_generation,
            next_texture_generation,
            next_texture_dirty_region,
            next_texture_content_generation,
            next_fill_sequence,
        ) = if fill.is_some() {
            (
                checked_next_generation(self.generation)?,
                checked_texture_generation(texture.generation, "surface-fill target")?,
                merged_texture_dirty_region(
                    texture.dirty_region,
                    Some(request.rect),
                    texture.descriptor,
                )?,
                texture
                    .content
                    .content_generation
                    .checked_add(1)
                    .ok_or_else(|| {
                        RuntimeStateError::resource_limit(
                            "runtime-graphics",
                            "graphics-texture-content-generation-exhausted",
                            "logical graphics texture content generation is exhausted",
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
                texture.generation,
                texture.dirty_region,
                texture.content.content_generation,
                self.next_surface_fill_sequence,
            )
        };
        Ok(RuntimeGraphicsSurfaceFillPlan {
            expected_generation: self.generation,
            next_generation,
            surface_id,
            surface_generation: surface.generation,
            texture_id: texture.id,
            texture_generation: texture.generation,
            next_texture_generation,
            texture_dirty_region: texture.dirty_region,
            next_texture_dirty_region,
            texture_content_generation: texture.content.content_generation,
            next_texture_content_generation,
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
        let texture = self
            .textures
            .get(&plan.texture_id)
            .ok_or_else(|| stale_plan("graphics-surface-fill-plan-stale"))?;
        if surface.generation != plan.surface_generation
            || surface.texture_id != texture.id
            || texture.generation != plan.texture_generation
            || texture.dirty_region != plan.texture_dirty_region
            || texture.content.content_generation != plan.texture_content_generation
        {
            return Err(stale_plan("graphics-surface-fill-plan-stale"));
        }
        if let Some(fill) = plan.fill {
            if fill.sequence != self.next_surface_fill_sequence
                || self.next_surface_fill_sequence.checked_add(1) != Some(plan.next_fill_sequence)
                || texture.content.fills.len() >= RUNTIME_GRAPHICS_SURFACE_FILL_LIMIT
                || texture.generation.checked_add(1) != Some(plan.next_texture_generation)
                || merged_texture_dirty_region(
                    texture.dirty_region,
                    Some(fill.rect),
                    texture.descriptor,
                )? != plan.next_texture_dirty_region
                || texture.content.content_generation.checked_add(1)
                    != Some(plan.next_texture_content_generation)
                || self.generation.checked_add(1) != Some(plan.next_generation)
            {
                return Err(stale_plan("graphics-surface-fill-plan-stale"));
            }
            validate_surface_fill_rect(fill.rect, surface.descriptor)?;
        } else if plan.next_generation != self.generation
            || plan.next_texture_generation != texture.generation
            || plan.next_texture_dirty_region != texture.dirty_region
            || plan.next_texture_content_generation != texture.content.content_generation
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
            .get(&plan.surface_id)
            .expect("preflighted graphics surface disappeared");
        if let Some(fill) = plan.fill {
            let texture = self
                .textures
                .get_mut(&plan.texture_id)
                .expect("preflighted graphics texture disappeared");
            texture.content.fills.push(fill);
            texture.content.content_generation = plan.next_texture_content_generation;
            texture.dirty_region = plan.next_texture_dirty_region;
            texture.generation = plan.next_texture_generation;
            self.next_surface_fill_sequence = plan.next_fill_sequence;
            self.generation = plan.next_generation;
        }
        let texture = self
            .textures
            .get(&plan.texture_id)
            .expect("preflighted graphics texture disappeared after fill");
        RuntimeGraphicsSurfaceFillResult {
            surface_id: surface.id,
            texture_id: texture.id,
            texture_generation: texture.generation,
            content_generation: texture.content.content_generation,
            dirty_region: texture.dirty_region,
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
        let texture_binding_clearances = self.texture_binding_clearances_for_retirement(
            &device_associations,
            &texture_associations,
        )?;
        let stream_source_clearances = self.stream_source_clearances_for_retirement(
            &device_associations,
            &vertex_buffer_associations,
            &texture_binding_clearances,
        )?;
        Ok(Some(RuntimeGraphicsComRetirementPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            device_associations,
            texture_associations,
            surface_associations,
            vertex_buffer_associations,
            texture_binding_clearances,
            stream_source_clearances,
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
        let texture_binding_clearances = self.texture_binding_clearances_for_retirement(
            &plan.device_associations,
            &plan.texture_associations,
        );
        let stream_source_clearances = match texture_binding_clearances.as_ref() {
            Ok(texture_binding_clearances) => self.stream_source_clearances_for_retirement(
                &plan.device_associations,
                &plan.vertex_buffer_associations,
                texture_binding_clearances,
            ),
            Err(_) => Err(stale_plan("graphics-com-retirement-plan-stale")),
        };
        if self.generation != plan.expected_generation
            || texture_binding_clearances.as_ref() != Ok(&plan.texture_binding_clearances)
            || stream_source_clearances.as_ref() != Ok(&plan.stream_source_clearances)
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
        for clearance in plan.texture_binding_clearances {
            let device = self
                .devices
                .get_mut(&clearance.device_id)
                .expect("preflighted texture-binding device disappeared during retirement");
            debug_assert_eq!(
                device.texture_bindings.stage_zero,
                Some(clearance.texture_id)
            );
            device.texture_bindings.stage_zero = None;
            device.generation = clearance.next_device_generation;
        }
        for clearance in plan.stream_source_clearances {
            let device = self
                .devices
                .get_mut(&clearance.device_id)
                .expect("preflighted stream-source device disappeared during retirement");
            debug_assert_eq!(device.stream_sources.stream_zero, Some(clearance.binding));
            debug_assert_eq!(device.generation, clearance.device_generation);
            device.stream_sources.stream_zero = None;
            device.generation = clearance.next_device_generation;
        }
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

    fn texture_binding_clearances_for_retirement(
        &self,
        device_associations: &[(RuntimeComObjectId, RuntimeGraphicsDeviceId)],
        texture_associations: &[(RuntimeComObjectId, RuntimeGraphicsTextureId)],
    ) -> Result<Vec<RuntimeGraphicsTextureBindingClearance>, RuntimeStateError> {
        let retiring_devices = device_associations
            .iter()
            .map(|(_, device_id)| *device_id)
            .collect::<BTreeSet<_>>();
        let mut clearances = Vec::new();
        for (_, texture_id) in texture_associations {
            let texture = self.textures.get(texture_id).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-graphics",
                    "graphics-texture-binding-retirement-texture-missing",
                    "retiring logical texture is missing while deriving binding clearances",
                )
            })?;
            if retiring_devices.contains(&texture.device_id) {
                continue;
            }
            let device = self.devices.get(&texture.device_id).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-graphics",
                    "graphics-texture-binding-retirement-device-missing",
                    "retiring bound texture has no live creating device",
                )
            })?;
            if device.texture_bindings.stage_zero == Some(*texture_id) {
                clearances.push(RuntimeGraphicsTextureBindingClearance {
                    device_id: device.id,
                    texture_id: *texture_id,
                    device_generation: device.generation,
                    next_device_generation: device.generation.checked_add(1).ok_or_else(|| {
                        RuntimeStateError::resource_limit(
                            "runtime-graphics",
                            "graphics-device-generation-exhausted",
                            "logical graphics device generation is exhausted",
                        )
                    })?,
                });
            }
        }
        Ok(clearances)
    }

    fn stream_source_clearances_for_retirement(
        &self,
        device_associations: &[(RuntimeComObjectId, RuntimeGraphicsDeviceId)],
        vertex_buffer_associations: &[(RuntimeComObjectId, RuntimeGraphicsVertexBufferId)],
        texture_binding_clearances: &[RuntimeGraphicsTextureBindingClearance],
    ) -> Result<Vec<RuntimeGraphicsStreamSourceClearance>, RuntimeStateError> {
        let retiring_devices = device_associations
            .iter()
            .map(|(_, device_id)| *device_id)
            .collect::<BTreeSet<_>>();
        let mut clearances = Vec::new();
        for (_, vertex_buffer_id) in vertex_buffer_associations {
            let vertex_buffer = self.vertex_buffers.get(vertex_buffer_id).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-graphics",
                    "graphics-stream-source-retirement-vertex-buffer-missing",
                    "retiring logical vertex buffer is missing while deriving stream-source clearances",
                )
            })?;
            if retiring_devices.contains(&vertex_buffer.device_id) {
                continue;
            }
            let device = self.devices.get(&vertex_buffer.device_id).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-graphics",
                    "graphics-stream-source-retirement-device-missing",
                    "retiring bound vertex buffer has no live creating device",
                )
            })?;
            let Some(binding) = device.stream_sources.stream_zero else {
                continue;
            };
            if binding.vertex_buffer_id != *vertex_buffer_id {
                continue;
            }
            let preceding_clearances = texture_binding_clearances
                .iter()
                .filter(|clearance| clearance.device_id == device.id)
                .count() as u64;
            let device_generation = device
                .generation
                .checked_add(preceding_clearances)
                .ok_or_else(|| {
                    RuntimeStateError::resource_limit(
                        "runtime-graphics",
                        "graphics-device-generation-exhausted",
                        "logical graphics device generation is exhausted",
                    )
                })?;
            clearances.push(RuntimeGraphicsStreamSourceClearance {
                device_id: device.id,
                binding,
                device_generation,
                next_device_generation: device_generation.checked_add(1).ok_or_else(|| {
                    RuntimeStateError::resource_limit(
                        "runtime-graphics",
                        "graphics-device-generation-exhausted",
                        "logical graphics device generation is exhausted",
                    )
                })?,
            });
        }
        Ok(clearances)
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
        if let Some(texture_id) = self
            .devices
            .get(&device_id)
            .and_then(|device| device.texture_bindings.stage_zero)
            && self
                .textures
                .get(&texture_id)
                .is_none_or(|texture| texture.device_id != device_id)
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-device-texture-binding-dangling",
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
            return Err(RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-device-stream-source-dangling",
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
                RuntimeStateError::not_found(
                    "runtime-graphics",
                    "graphics-device-com-object-unknown",
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
        || request.target.client_width > i32::MAX as u32
        || request.target.client_height > i32::MAX as u32
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

fn validate_back_buffer_clear_request(
    request: RuntimeGraphicsBackBufferClearRequest,
) -> Result<(), RuntimeStateError> {
    if request.device_com_object_id.0 == 0 {
        return Err(RuntimeStateError::invalid_input(
            "runtime-graphics",
            "graphics-back-buffer-clear-request-invalid",
            "logical back-buffer clear requires a live graphics device",
        ));
    }
    Ok(())
}

fn validate_texture_binding_update_request(
    request: RuntimeGraphicsTextureBindingUpdateRequest,
) -> Result<(), RuntimeStateError> {
    if request.device_com_object_id.0 == 0
        || request.stage != 0
        || request
            .texture_com_object_id
            .is_some_and(|texture_com_object_id| texture_com_object_id.0 == 0)
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-graphics",
            "graphics-texture-binding-request-invalid",
            "logical texture binding requires stage zero and live COM object identities",
        ));
    }
    Ok(())
}

fn validate_flexible_vertex_format_update_request(
    request: RuntimeGraphicsFlexibleVertexFormatUpdateRequest,
) -> Result<(), RuntimeStateError> {
    if request.device_com_object_id.0 == 0 {
        return Err(RuntimeStateError::invalid_input(
            "runtime-graphics",
            "graphics-flexible-vertex-format-request-invalid",
            "logical flexible vertex format update requires a live graphics device",
        ));
    }
    Ok(())
}

fn validate_stream_source_update_request(
    request: RuntimeGraphicsStreamSourceUpdateRequest,
) -> Result<(), RuntimeStateError> {
    if request.device_com_object_id.0 == 0
        || request.stream != 0
        || request.vertex_buffer_com_object_id.0 == 0
        || request.stride == 0
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-graphics",
            "graphics-stream-source-request-invalid",
            "logical stream-source binding requires stream zero, live COM identities, and a nonzero stride",
        ));
    }
    Ok(())
}

fn validate_draw_capture_request(
    request: RuntimeGraphicsDrawCaptureRequest,
) -> Result<(), RuntimeStateError> {
    if request.device_com_object_id.0 == 0
        || request.primitive_topology != RuntimeGraphicsPrimitiveTopology::TriangleStrip
        || request.start_vertex != 0
        || request.primitive_count != 2
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-graphics",
            "graphics-draw-capture-request-invalid",
            "logical draw capture currently supports the reached four-vertex triangle-strip quad only",
        ));
    }
    Ok(())
}

fn validate_supported_draw_device_state(
    device: &RuntimeGraphicsDevice,
) -> Result<(), RuntimeStateError> {
    let supported_render_states = RuntimeGraphicsRenderStates {
        cull_mode: Some(RuntimeGraphicsCullMode::None),
        depth_test_enabled: Some(false),
        lighting_enabled: Some(false),
        alpha_blend_enabled: Some(true),
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
    if device.render_states != supported_render_states
        || device.sampler_states != supported_sampler_states
        || device.texture_stage_states != supported_texture_stage_states
        || device.flexible_vertex_format
            != Some(RuntimeGraphicsFlexibleVertexFormat::XyzRhwDiffuseTexture1)
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-graphics",
            "graphics-draw-fixed-function-state-incomplete",
            "logical draw submission requires the reached fixed-function render, sampler, texture-stage, and vertex-format state",
        ));
    }
    Ok(())
}

fn draw_vertex_bytes(back_buffer: &RuntimeGraphicsBackBuffer) -> Result<usize, RuntimeStateError> {
    back_buffer
        .draw_submissions
        .iter()
        .try_fold(0_usize, |total, submission| {
            total
                .checked_add(submission.vertex_data.len())
                .ok_or_else(|| {
                    RuntimeStateError::runtime_defect(
                        "runtime-graphics",
                        "graphics-draw-vertex-byte-accounting-overflow",
                        "logical back-buffer captured vertex-byte accounting overflowed",
                    )
                })
        })
}

fn full_back_buffer_rect(
    presentation: RuntimeGraphicsPresentation,
) -> Result<RuntimeRect, RuntimeStateError> {
    let right = i32::try_from(presentation.width).map_err(|_| {
        RuntimeStateError::runtime_defect(
            "runtime-graphics",
            "graphics-back-buffer-width-invalid",
            "logical graphics back-buffer width does not fit x86 coordinates",
        )
    })?;
    let bottom = i32::try_from(presentation.height).map_err(|_| {
        RuntimeStateError::runtime_defect(
            "runtime-graphics",
            "graphics-back-buffer-height-invalid",
            "logical graphics back-buffer height does not fit x86 coordinates",
        )
    })?;
    Ok(RuntimeRect {
        left: 0,
        top: 0,
        right,
        bottom,
    })
}

fn is_initial_sequence(sequence: &u64) -> bool {
    *sequence == 1
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

fn validate_texture_dirty_region_mark_request(
    request: RuntimeGraphicsTextureDirtyRegionMarkRequest,
) -> Result<(), RuntimeStateError> {
    if request.texture_com_object_id.0 == 0 {
        return Err(RuntimeStateError::invalid_input(
            "runtime-graphics",
            "graphics-texture-dirty-region-request-invalid",
            "logical texture dirty-region mark requires a live texture COM object",
        ));
    }
    Ok(())
}

fn validate_texture_update_request(
    request: RuntimeGraphicsTextureUpdateRequest,
) -> Result<(), RuntimeStateError> {
    if request.device_com_object_id.0 == 0
        || request.source_texture_com_object_id.0 == 0
        || request.destination_texture_com_object_id.0 == 0
        || request.source_texture_com_object_id == request.destination_texture_com_object_id
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-graphics",
            "graphics-texture-update-request-invalid",
            "logical texture update requires one live device and distinct live source and destination textures",
        ));
    }
    Ok(())
}

fn validate_texture_update_pair(
    device_id: RuntimeGraphicsDeviceId,
    source: &RuntimeGraphicsTexture,
    destination: &RuntimeGraphicsTexture,
) -> Result<(), RuntimeStateError> {
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
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-graphics",
            "graphics-texture-update-pair-invalid",
            "logical texture update requires compatible distinct SYSTEMMEM and DEFAULT textures owned by the calling device",
        ));
    }
    Ok(())
}

fn checked_texture_generation(
    generation: u64,
    role: &'static str,
) -> Result<u64, RuntimeStateError> {
    generation.checked_add(1).ok_or_else(|| {
        RuntimeStateError::resource_limit(
            "runtime-graphics",
            "graphics-texture-generation-exhausted",
            format!("logical {role} texture generation is exhausted"),
        )
    })
}

fn full_texture_rect(
    descriptor: RuntimeGraphicsTextureDescriptor,
) -> Result<RuntimeRect, RuntimeStateError> {
    let right = i32::try_from(descriptor.width).map_err(|_| {
        RuntimeStateError::runtime_defect(
            "runtime-graphics",
            "graphics-texture-width-invalid",
            "logical graphics texture width does not fit x86 coordinates",
        )
    })?;
    let bottom = i32::try_from(descriptor.height).map_err(|_| {
        RuntimeStateError::runtime_defect(
            "runtime-graphics",
            "graphics-texture-height-invalid",
            "logical graphics texture height does not fit x86 coordinates",
        )
    })?;
    Ok(RuntimeRect {
        left: 0,
        top: 0,
        right,
        bottom,
    })
}

fn validate_texture_dirty_rect(
    rect: RuntimeRect,
    descriptor: RuntimeGraphicsTextureDescriptor,
) -> Result<(), RuntimeStateError> {
    let full = full_texture_rect(descriptor)?;
    if rect.left < 0
        || rect.top < 0
        || rect.right <= rect.left
        || rect.bottom <= rect.top
        || rect.right > full.right
        || rect.bottom > full.bottom
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-graphics",
            "graphics-texture-dirty-rect-out-of-bounds",
            "logical texture dirty rectangle must be nonempty, ordered, and in bounds",
        ));
    }
    Ok(())
}

fn merged_texture_dirty_region(
    current: RuntimeGraphicsTextureDirtyRegion,
    rect: Option<RuntimeRect>,
    descriptor: RuntimeGraphicsTextureDescriptor,
) -> Result<RuntimeGraphicsTextureDirtyRegion, RuntimeStateError> {
    let full = full_texture_rect(descriptor)?;
    let Some(rect) = rect else {
        return Ok(RuntimeGraphicsTextureDirtyRegion::WholeTexture);
    };
    validate_texture_dirty_rect(rect, descriptor)?;
    if current == RuntimeGraphicsTextureDirtyRegion::WholeTexture {
        return Ok(current);
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
    Ok(if merged == full {
        RuntimeGraphicsTextureDirtyRegion::WholeTexture
    } else {
        RuntimeGraphicsTextureDirtyRegion::Rect(merged)
    })
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

    fn prepared_draw_graphics() -> (
        RuntimeGraphics,
        RuntimeGraphicsDeviceId,
        RuntimeGraphicsTextureId,
        RuntimeGraphicsVertexBufferId,
        GuestRange,
    ) {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics.plan_device_creation(request()).unwrap();
        let device = graphics.commit_device_creation(device_plan, RuntimeComObjectId(7));
        let texture_plan = graphics
            .plan_texture_creation(RuntimeGraphicsTextureCreateRequest {
                device_com_object_id: RuntimeComObjectId(7),
                descriptor: RuntimeGraphicsTextureDescriptor {
                    width: 2_048,
                    height: 1_024,
                    levels: 1,
                    usage: RuntimeGraphicsTextureUsage::None,
                    color_format: RuntimeGraphicsColorFormat::B8g8r8x8Unorm,
                    pool: RuntimeGraphicsTexturePool::Default,
                },
            })
            .unwrap();
        let texture = graphics.commit_texture_creation(texture_plan, RuntimeComObjectId(8));
        let backing_range = GuestRange::new(RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE, 0x1000).unwrap();
        let vertex_buffer_plan = graphics
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
        let vertex_buffer = graphics.commit_vertex_buffer_creation(
            vertex_buffer_plan,
            RuntimeComObjectId(9),
            MemoryOwnerId(23),
        );
        {
            let texture = graphics.textures.get_mut(&texture.id).unwrap();
            texture.content.content_generation = 1;
            texture.generation = 1;
        }
        {
            let vertex_buffer = graphics.vertex_buffers.get_mut(&vertex_buffer.id).unwrap();
            vertex_buffer.content_generation = 1;
            vertex_buffer.generation = 1;
        }
        {
            let device = graphics.devices.get_mut(&device.id).unwrap();
            device.render_states = RuntimeGraphicsRenderStates {
                cull_mode: Some(RuntimeGraphicsCullMode::None),
                depth_test_enabled: Some(false),
                lighting_enabled: Some(false),
                alpha_blend_enabled: Some(true),
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
            device.texture_bindings.stage_zero = Some(texture.id);
            device.flexible_vertex_format =
                Some(RuntimeGraphicsFlexibleVertexFormat::XyzRhwDiffuseTexture1);
            device.stream_sources.stream_zero = Some(RuntimeGraphicsStreamSourceBinding {
                vertex_buffer_id: vertex_buffer.id,
                offset_in_bytes: 0,
                stride: 28,
            });
            device.back_buffer.content_generation = 1;
            device.back_buffer.uniform_clear = Some(RuntimeGraphicsBackBufferClear {
                sequence: 1,
                rect: RuntimeRect {
                    left: 0,
                    top: 0,
                    right: 800,
                    bottom: 600,
                },
                color_argb: 0xff00_0000,
            });
            device.active_scene = Some(RuntimeGraphicsScene { sequence: 1 });
        }
        (
            graphics,
            device.id,
            texture.id,
            vertex_buffer.id,
            backing_range,
        )
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
            graphics
                .device_by_com_object(RuntimeComObjectId(7))
                .unwrap()
                .id,
            device.id
        );
        assert_eq!(
            graphics
                .device_by_com_object(RuntimeComObjectId(99))
                .unwrap_err()
                .code,
            "graphics-device-com-object-unknown"
        );
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
                .device_by_com_object(RuntimeComObjectId(7))
                .unwrap_err()
                .code,
            "graphics-device-com-object-unknown"
        );
        assert_eq!(
            graphics
                .preflight_com_object_retirement(&stale_retirement)
                .unwrap_err()
                .code,
            "graphics-com-retirement-plan-stale"
        );
    }

    #[test]
    fn render_state_updates_are_typed_idempotent_and_failure_atomic() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics.plan_device_creation(request()).unwrap();
        let device = graphics.commit_device_creation(device_plan, RuntimeComObjectId(7));
        assert_eq!(device.render_states, RuntimeGraphicsRenderStates::default());

        for update in [
            RuntimeGraphicsRenderStateUpdate::CullMode(RuntimeGraphicsCullMode::None),
            RuntimeGraphicsRenderStateUpdate::DepthTestEnabled(false),
            RuntimeGraphicsRenderStateUpdate::LightingEnabled(false),
            RuntimeGraphicsRenderStateUpdate::AlphaBlendEnabled(true),
        ] {
            let plan = graphics
                .plan_render_state_update(RuntimeGraphicsRenderStateUpdateRequest {
                    device_com_object_id: RuntimeComObjectId(7),
                    update,
                })
                .unwrap();
            let stale = plan.clone();
            graphics.preflight_render_state_update(&plan).unwrap();
            let result = graphics.commit_render_state_update(plan);
            assert_eq!(result.device_id, device.id);
            assert_eq!(result.update, update);
            assert!(result.changed);
            assert_eq!(
                graphics
                    .preflight_render_state_update(&stale)
                    .unwrap_err()
                    .code,
                "graphics-render-state-update-plan-stale"
            );
        }
        let committed = graphics.device(device.id).unwrap();
        assert_eq!(
            committed.render_states.cull_mode,
            Some(RuntimeGraphicsCullMode::None)
        );
        assert_eq!(committed.render_states.depth_test_enabled, Some(false));
        assert_eq!(committed.render_states.lighting_enabled, Some(false));
        assert_eq!(committed.render_states.alpha_blend_enabled, Some(true));

        let before_idempotent = graphics.clone();
        let idempotent = graphics
            .plan_render_state_update(RuntimeGraphicsRenderStateUpdateRequest {
                device_com_object_id: RuntimeComObjectId(7),
                update: RuntimeGraphicsRenderStateUpdate::AlphaBlendEnabled(true),
            })
            .unwrap();
        let result = graphics.commit_render_state_update(idempotent);
        assert!(!result.changed);
        assert_eq!(graphics, before_idempotent);

        let stale = graphics
            .plan_render_state_update(RuntimeGraphicsRenderStateUpdateRequest {
                device_com_object_id: RuntimeComObjectId(7),
                update: RuntimeGraphicsRenderStateUpdate::DepthTestEnabled(true),
            })
            .unwrap();
        let intervening = graphics
            .plan_render_state_update(RuntimeGraphicsRenderStateUpdateRequest {
                device_com_object_id: RuntimeComObjectId(7),
                update: RuntimeGraphicsRenderStateUpdate::LightingEnabled(true),
            })
            .unwrap();
        graphics.commit_render_state_update(intervening);
        let before_stale = graphics.clone();
        assert_eq!(
            graphics
                .preflight_render_state_update(&stale)
                .unwrap_err()
                .code,
            "graphics-render-state-update-plan-stale"
        );
        assert_eq!(graphics, before_stale);

        let before_unknown = graphics.clone();
        assert_eq!(
            graphics
                .plan_render_state_update(RuntimeGraphicsRenderStateUpdateRequest {
                    device_com_object_id: RuntimeComObjectId(99),
                    update: RuntimeGraphicsRenderStateUpdate::LightingEnabled(false),
                })
                .unwrap_err()
                .code,
            "graphics-render-state-device-com-object-unknown"
        );
        assert_eq!(graphics, before_unknown);

        graphics.generation = u64::MAX;
        let exhausted = graphics.clone();
        assert_eq!(
            graphics
                .plan_render_state_update(RuntimeGraphicsRenderStateUpdateRequest {
                    device_com_object_id: RuntimeComObjectId(7),
                    update: RuntimeGraphicsRenderStateUpdate::AlphaBlendEnabled(false),
                })
                .unwrap_err()
                .code,
            "graphics-generation-exhausted"
        );
        assert_eq!(graphics, exhausted);
    }

    #[test]
    fn sampler_state_updates_are_typed_idempotent_and_failure_atomic() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics.plan_device_creation(request()).unwrap();
        let device = graphics.commit_device_creation(device_plan, RuntimeComObjectId(7));
        assert_eq!(
            device.sampler_states,
            RuntimeGraphicsSamplerStates::default()
        );

        let min_filter = RuntimeGraphicsSamplerStateUpdate::MinFilter {
            sampler: 0,
            filter: RuntimeGraphicsTextureFilter::Linear,
        };
        let min_plan = graphics
            .plan_sampler_state_update(RuntimeGraphicsSamplerStateUpdateRequest {
                device_com_object_id: RuntimeComObjectId(7),
                update: min_filter,
            })
            .unwrap();
        let stale_min = min_plan.clone();
        let before_min = graphics.clone();
        graphics.preflight_sampler_state_update(&min_plan).unwrap();
        assert_eq!(
            graphics, before_min,
            "planning and preflight must not mutate"
        );
        let min_result = graphics.commit_sampler_state_update(min_plan);
        assert_eq!(min_result.device_id, device.id);
        assert_eq!(min_result.update, min_filter);
        assert!(min_result.changed);
        assert_eq!(min_result.device_generation, device.generation + 1);
        assert_eq!(
            graphics
                .preflight_sampler_state_update(&stale_min)
                .unwrap_err()
                .code,
            "graphics-sampler-state-update-plan-stale"
        );

        let before_idempotent = graphics.clone();
        let repeated_min = graphics
            .plan_sampler_state_update(RuntimeGraphicsSamplerStateUpdateRequest {
                device_com_object_id: RuntimeComObjectId(7),
                update: min_filter,
            })
            .unwrap();
        let repeated_result = graphics.commit_sampler_state_update(repeated_min);
        assert!(!repeated_result.changed);
        assert_eq!(graphics, before_idempotent);

        let mag_filter = RuntimeGraphicsSamplerStateUpdate::MagFilter {
            sampler: 0,
            filter: RuntimeGraphicsTextureFilter::Linear,
        };
        let stale_mag = graphics
            .plan_sampler_state_update(RuntimeGraphicsSamplerStateUpdateRequest {
                device_com_object_id: RuntimeComObjectId(7),
                update: mag_filter,
            })
            .unwrap();
        let intervening = graphics
            .plan_render_state_update(RuntimeGraphicsRenderStateUpdateRequest {
                device_com_object_id: RuntimeComObjectId(7),
                update: RuntimeGraphicsRenderStateUpdate::LightingEnabled(false),
            })
            .unwrap();
        graphics.commit_render_state_update(intervening);
        let before_stale = graphics.clone();
        assert_eq!(
            graphics
                .preflight_sampler_state_update(&stale_mag)
                .unwrap_err()
                .code,
            "graphics-sampler-state-update-plan-stale"
        );
        assert_eq!(graphics, before_stale);

        let mag_plan = graphics
            .plan_sampler_state_update(RuntimeGraphicsSamplerStateUpdateRequest {
                device_com_object_id: RuntimeComObjectId(7),
                update: mag_filter,
            })
            .unwrap();
        let mag_result = graphics.commit_sampler_state_update(mag_plan);
        assert!(mag_result.changed);
        let committed = graphics.device(device.id).unwrap();
        assert_eq!(
            committed.sampler_states.sampler_zero.min_filter,
            Some(RuntimeGraphicsTextureFilter::Linear)
        );
        assert_eq!(
            committed.sampler_states.sampler_zero.mag_filter,
            Some(RuntimeGraphicsTextureFilter::Linear)
        );

        let before_invalid = graphics.clone();
        assert_eq!(
            graphics
                .plan_sampler_state_update(RuntimeGraphicsSamplerStateUpdateRequest {
                    device_com_object_id: RuntimeComObjectId(7),
                    update: RuntimeGraphicsSamplerStateUpdate::MinFilter {
                        sampler: 1,
                        filter: RuntimeGraphicsTextureFilter::Linear,
                    },
                })
                .unwrap_err()
                .code,
            "graphics-sampler-state-sampler-unsupported"
        );
        assert_eq!(graphics, before_invalid);

        let before_unknown = graphics.clone();
        assert_eq!(
            graphics
                .plan_sampler_state_update(RuntimeGraphicsSamplerStateUpdateRequest {
                    device_com_object_id: RuntimeComObjectId(99),
                    update: min_filter,
                })
                .unwrap_err()
                .code,
            "graphics-sampler-state-device-com-object-unknown"
        );
        assert_eq!(graphics, before_unknown);

        graphics.generation = u64::MAX;
        let exhausted = graphics.clone();
        assert_eq!(
            graphics
                .plan_sampler_state_update(RuntimeGraphicsSamplerStateUpdateRequest {
                    device_com_object_id: RuntimeComObjectId(7),
                    update: RuntimeGraphicsSamplerStateUpdate::MinFilter {
                        sampler: 0,
                        filter: RuntimeGraphicsTextureFilter::Linear,
                    },
                })
                .unwrap()
                .next_generation,
            u64::MAX,
            "same-value updates remain idempotent at generation exhaustion"
        );
        assert_eq!(graphics, exhausted);

        let mut changed_at_exhaustion = RuntimeGraphics::default();
        let plan = changed_at_exhaustion
            .plan_device_creation(request())
            .unwrap();
        changed_at_exhaustion.commit_device_creation(plan, RuntimeComObjectId(8));
        changed_at_exhaustion.generation = u64::MAX;
        let before_exhausted_change = changed_at_exhaustion.clone();
        assert_eq!(
            changed_at_exhaustion
                .plan_sampler_state_update(RuntimeGraphicsSamplerStateUpdateRequest {
                    device_com_object_id: RuntimeComObjectId(8),
                    update: min_filter,
                })
                .unwrap_err()
                .code,
            "graphics-generation-exhausted"
        );
        assert_eq!(changed_at_exhaustion, before_exhausted_change);
    }

    #[test]
    fn texture_stage_state_updates_are_typed_idempotent_and_failure_atomic() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics.plan_device_creation(request()).unwrap();
        let device = graphics.commit_device_creation(device_plan, RuntimeComObjectId(7));
        assert_eq!(
            device.texture_stage_states,
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
        let first_plan = graphics
            .plan_texture_stage_state_update(RuntimeGraphicsTextureStageStateUpdateRequest {
                device_com_object_id: RuntimeComObjectId(7),
                update: updates[0],
            })
            .unwrap();
        let stale_first = first_plan.clone();
        let before_first = graphics.clone();
        graphics
            .preflight_texture_stage_state_update(&first_plan)
            .unwrap();
        assert_eq!(
            graphics, before_first,
            "planning and preflight must not mutate"
        );
        let first = graphics.commit_texture_stage_state_update(first_plan);
        assert_eq!(first.device_id, device.id);
        assert_eq!(first.update, updates[0]);
        assert!(first.changed);
        assert_eq!(
            graphics
                .preflight_texture_stage_state_update(&stale_first)
                .unwrap_err()
                .code,
            "graphics-texture-stage-state-update-plan-stale"
        );

        for update in updates.into_iter().skip(1) {
            let plan = graphics
                .plan_texture_stage_state_update(RuntimeGraphicsTextureStageStateUpdateRequest {
                    device_com_object_id: RuntimeComObjectId(7),
                    update,
                })
                .unwrap();
            let result = graphics.commit_texture_stage_state_update(plan);
            assert!(result.changed);
            assert_eq!(result.update, update);
        }
        let committed = graphics.device(device.id).unwrap();
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
        assert_eq!(committed.generation, device.generation + 6);

        let before_repeat = graphics.clone();
        let repeat = graphics
            .plan_texture_stage_state_update(RuntimeGraphicsTextureStageStateUpdateRequest {
                device_com_object_id: RuntimeComObjectId(7),
                update: updates[0],
            })
            .unwrap();
        assert!(!graphics.commit_texture_stage_state_update(repeat).changed);
        assert_eq!(graphics, before_repeat);

        let before_invalid = graphics.clone();
        assert_eq!(
            graphics
                .plan_texture_stage_state_update(RuntimeGraphicsTextureStageStateUpdateRequest {
                    device_com_object_id: RuntimeComObjectId(7),
                    update: RuntimeGraphicsTextureStageStateUpdate::ColorOperation {
                        stage: 1,
                        operation: RuntimeGraphicsTextureOperation::Modulate,
                    },
                })
                .unwrap_err()
                .code,
            "graphics-texture-stage-state-stage-unsupported"
        );
        assert_eq!(graphics, before_invalid);

        let before_unknown = graphics.clone();
        assert_eq!(
            graphics
                .plan_texture_stage_state_update(RuntimeGraphicsTextureStageStateUpdateRequest {
                    device_com_object_id: RuntimeComObjectId(99),
                    update: updates[0],
                })
                .unwrap_err()
                .code,
            "graphics-texture-stage-state-device-com-object-unknown"
        );
        assert_eq!(graphics, before_unknown);

        graphics.generation = u64::MAX;
        let exhausted = graphics.clone();
        let idempotent = graphics
            .plan_texture_stage_state_update(RuntimeGraphicsTextureStageStateUpdateRequest {
                device_com_object_id: RuntimeComObjectId(7),
                update: updates[0],
            })
            .unwrap();
        assert_eq!(idempotent.next_generation, u64::MAX);
        assert_eq!(graphics, exhausted);

        let mut changed_at_exhaustion = RuntimeGraphics::default();
        let plan = changed_at_exhaustion
            .plan_device_creation(request())
            .unwrap();
        changed_at_exhaustion.commit_device_creation(plan, RuntimeComObjectId(8));
        changed_at_exhaustion.generation = u64::MAX;
        let before_exhausted_change = changed_at_exhaustion.clone();
        assert_eq!(
            changed_at_exhaustion
                .plan_texture_stage_state_update(RuntimeGraphicsTextureStageStateUpdateRequest {
                    device_com_object_id: RuntimeComObjectId(8),
                    update: updates[0],
                })
                .unwrap_err()
                .code,
            "graphics-generation-exhausted"
        );
        assert_eq!(changed_at_exhaustion, before_exhausted_change);
    }

    #[test]
    fn texture_bindings_are_non_owning_idempotent_and_retirement_safe() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics.plan_device_creation(request()).unwrap();
        let device = graphics.commit_device_creation(device_plan, RuntimeComObjectId(7));
        let texture_plan = graphics
            .plan_texture_creation(texture_request(RuntimeComObjectId(7)))
            .unwrap();
        let texture = graphics.commit_texture_creation(texture_plan, RuntimeComObjectId(8));

        let bind_request = RuntimeGraphicsTextureBindingUpdateRequest {
            device_com_object_id: RuntimeComObjectId(7),
            stage: 0,
            texture_com_object_id: Some(RuntimeComObjectId(8)),
        };
        let before_plan = graphics.clone();
        let bind = graphics.plan_texture_binding_update(bind_request).unwrap();
        let stale_bind = bind.clone();
        graphics.preflight_texture_binding_update(&bind).unwrap();
        assert_eq!(graphics, before_plan);
        let bound = graphics.commit_texture_binding_update(bind);
        assert_eq!(bound.device_id, device.id);
        assert_eq!(bound.texture_id, Some(texture.id));
        assert!(bound.changed);
        assert_eq!(
            graphics
                .device(device.id)
                .unwrap()
                .texture_bindings
                .stage_zero,
            Some(texture.id)
        );
        assert_eq!(
            graphics
                .preflight_texture_binding_update(&stale_bind)
                .unwrap_err()
                .code,
            "graphics-texture-binding-update-plan-stale"
        );

        let before_repeat = graphics.clone();
        let repeat = graphics.plan_texture_binding_update(bind_request).unwrap();
        assert!(!graphics.commit_texture_binding_update(repeat).changed);
        assert_eq!(graphics, before_repeat);

        let second_device_plan = graphics.plan_device_creation(request()).unwrap();
        graphics.commit_device_creation(second_device_plan, RuntimeComObjectId(9));
        let second_texture_plan = graphics
            .plan_texture_creation(texture_request(RuntimeComObjectId(9)))
            .unwrap();
        graphics.commit_texture_creation(second_texture_plan, RuntimeComObjectId(10));
        let before_wrong_owner = graphics.clone();
        assert_eq!(
            graphics
                .plan_texture_binding_update(RuntimeGraphicsTextureBindingUpdateRequest {
                    device_com_object_id: RuntimeComObjectId(7),
                    stage: 0,
                    texture_com_object_id: Some(RuntimeComObjectId(10)),
                })
                .unwrap_err()
                .code,
            "graphics-texture-binding-device-mismatch"
        );
        assert_eq!(graphics, before_wrong_owner);

        let before_invalid_stage = graphics.clone();
        assert_eq!(
            graphics
                .plan_texture_binding_update(RuntimeGraphicsTextureBindingUpdateRequest {
                    device_com_object_id: RuntimeComObjectId(7),
                    stage: 1,
                    texture_com_object_id: Some(RuntimeComObjectId(8)),
                })
                .unwrap_err()
                .code,
            "graphics-texture-binding-request-invalid"
        );
        assert_eq!(graphics, before_invalid_stage);

        let retirement = graphics
            .plan_com_object_retirements(&[RuntimeComObjectId(8)].into_iter().collect())
            .unwrap()
            .unwrap();
        let device_generation = graphics.device(device.id).unwrap().generation;
        graphics
            .preflight_com_object_retirement(&retirement)
            .unwrap();
        graphics.commit_com_object_retirement(retirement);
        let device_after_retirement = graphics.device(device.id).unwrap();
        assert_eq!(device_after_retirement.texture_bindings.stage_zero, None);
        assert_eq!(device_after_retirement.generation, device_generation + 1);
        assert!(graphics.texture(texture.id).is_err());

        let unbind = graphics
            .plan_texture_binding_update(RuntimeGraphicsTextureBindingUpdateRequest {
                device_com_object_id: RuntimeComObjectId(7),
                stage: 0,
                texture_com_object_id: None,
            })
            .unwrap();
        assert!(!graphics.commit_texture_binding_update(unbind).changed);

        let remaining_texture = graphics
            .texture_for_com_object(RuntimeComObjectId(10))
            .unwrap()
            .clone();
        assert_eq!(remaining_texture.device_id, RuntimeGraphicsDeviceId(2));
    }

    #[test]
    fn flexible_vertex_format_updates_are_typed_idempotent_and_failure_atomic() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics.plan_device_creation(request()).unwrap();
        let device = graphics.commit_device_creation(device_plan, RuntimeComObjectId(7));
        assert_eq!(device.flexible_vertex_format, None);

        let format_request = RuntimeGraphicsFlexibleVertexFormatUpdateRequest {
            device_com_object_id: RuntimeComObjectId(7),
            format: RuntimeGraphicsFlexibleVertexFormat::XyzRhwDiffuseTexture1,
        };
        let before_plan = graphics.clone();
        let plan = graphics
            .plan_flexible_vertex_format_update(format_request)
            .unwrap();
        let stale = plan.clone();
        graphics
            .preflight_flexible_vertex_format_update(&plan)
            .unwrap();
        assert_eq!(graphics, before_plan);
        let result = graphics.commit_flexible_vertex_format_update(plan);
        assert_eq!(result.device_id, device.id);
        assert_eq!(result.format, format_request.format);
        assert!(result.changed);
        assert_eq!(result.device_generation, device.generation + 1);
        assert_eq!(
            graphics.device(device.id).unwrap().flexible_vertex_format,
            Some(format_request.format)
        );
        assert_eq!(
            graphics
                .preflight_flexible_vertex_format_update(&stale)
                .unwrap_err()
                .code,
            "graphics-flexible-vertex-format-update-plan-stale"
        );

        let before_repeat = graphics.clone();
        let repeat = graphics
            .plan_flexible_vertex_format_update(format_request)
            .unwrap();
        assert!(
            !graphics
                .commit_flexible_vertex_format_update(repeat)
                .changed
        );
        assert_eq!(graphics, before_repeat);

        let before_invalid = graphics.clone();
        assert_eq!(
            graphics
                .plan_flexible_vertex_format_update(
                    RuntimeGraphicsFlexibleVertexFormatUpdateRequest {
                        device_com_object_id: RuntimeComObjectId(0),
                        format: format_request.format,
                    },
                )
                .unwrap_err()
                .code,
            "graphics-flexible-vertex-format-request-invalid"
        );
        assert_eq!(graphics, before_invalid);
        assert_eq!(
            graphics
                .plan_flexible_vertex_format_update(
                    RuntimeGraphicsFlexibleVertexFormatUpdateRequest {
                        device_com_object_id: RuntimeComObjectId(99),
                        format: format_request.format,
                    },
                )
                .unwrap_err()
                .code,
            "graphics-flexible-vertex-format-device-com-object-unknown"
        );
        assert_eq!(graphics, before_invalid);

        let mut exhausted = RuntimeGraphics::default();
        let exhausted_plan = exhausted.plan_device_creation(request()).unwrap();
        exhausted.commit_device_creation(exhausted_plan, RuntimeComObjectId(8));
        exhausted.generation = u64::MAX;
        let before_exhaustion = exhausted.clone();
        assert_eq!(
            exhausted
                .plan_flexible_vertex_format_update(
                    RuntimeGraphicsFlexibleVertexFormatUpdateRequest {
                        device_com_object_id: RuntimeComObjectId(8),
                        format: format_request.format,
                    },
                )
                .unwrap_err()
                .code,
            "graphics-generation-exhausted"
        );
        assert_eq!(exhausted, before_exhaustion);
    }

    #[test]
    fn stream_sources_are_non_owning_idempotent_owner_checked_and_retirement_safe() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics.plan_device_creation(request()).unwrap();
        let device = graphics.commit_device_creation(device_plan, RuntimeComObjectId(7));
        let backing_range = GuestRange::new(RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE, 0x1000).unwrap();
        let vertex_buffer_plan = graphics
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
        let vertex_buffer = graphics.commit_vertex_buffer_creation(
            vertex_buffer_plan,
            RuntimeComObjectId(8),
            MemoryOwnerId(23),
        );
        assert!(
            graphics
                .device(device.id)
                .unwrap()
                .stream_sources
                .is_pristine()
        );

        let stream_request = RuntimeGraphicsStreamSourceUpdateRequest {
            device_com_object_id: RuntimeComObjectId(7),
            stream: 0,
            vertex_buffer_com_object_id: RuntimeComObjectId(8),
            offset_in_bytes: 0,
            stride: 28,
        };
        let before_plan = graphics.clone();
        let plan = graphics.plan_stream_source_update(stream_request).unwrap();
        let stale = plan.clone();
        graphics.preflight_stream_source_update(&plan).unwrap();
        assert_eq!(graphics, before_plan);
        let result = graphics.commit_stream_source_update(plan);
        assert_eq!(result.device_id, device.id);
        assert_eq!(result.stream, 0);
        assert_eq!(
            result.binding,
            RuntimeGraphicsStreamSourceBinding {
                vertex_buffer_id: vertex_buffer.id,
                offset_in_bytes: 0,
                stride: 28,
            }
        );
        assert!(result.changed);
        assert_eq!(result.device_generation, device.generation + 1);
        assert_eq!(
            graphics
                .device(device.id)
                .unwrap()
                .stream_sources
                .stream_zero,
            Some(result.binding)
        );
        assert_eq!(
            graphics
                .preflight_stream_source_update(&stale)
                .unwrap_err()
                .code,
            "graphics-stream-source-update-plan-stale"
        );

        let before_repeat = graphics.clone();
        let repeat = graphics.plan_stream_source_update(stream_request).unwrap();
        assert!(!graphics.commit_stream_source_update(repeat).changed);
        assert_eq!(graphics, before_repeat);

        let second_device_plan = graphics.plan_device_creation(request()).unwrap();
        graphics.commit_device_creation(second_device_plan, RuntimeComObjectId(9));
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
            .plan_vertex_buffer_creation(
                RuntimeGraphicsVertexBufferCreateRequest {
                    device_com_object_id: RuntimeComObjectId(9),
                    descriptor: vertex_buffer.descriptor,
                },
                ProcessAddressAllocationId(42),
                second_backing,
            )
            .unwrap();
        graphics.commit_vertex_buffer_creation(
            second_vertex_buffer_plan,
            RuntimeComObjectId(10),
            MemoryOwnerId(24),
        );
        let before_wrong_owner = graphics.clone();
        assert_eq!(
            graphics
                .plan_stream_source_update(RuntimeGraphicsStreamSourceUpdateRequest {
                    vertex_buffer_com_object_id: RuntimeComObjectId(10),
                    ..stream_request
                })
                .unwrap_err()
                .code,
            "graphics-stream-source-device-mismatch"
        );
        assert_eq!(graphics, before_wrong_owner);

        for invalid in [
            RuntimeGraphicsStreamSourceUpdateRequest {
                stream: 1,
                ..stream_request
            },
            RuntimeGraphicsStreamSourceUpdateRequest {
                stride: 0,
                ..stream_request
            },
            RuntimeGraphicsStreamSourceUpdateRequest {
                vertex_buffer_com_object_id: RuntimeComObjectId(0),
                ..stream_request
            },
        ] {
            let before_invalid = graphics.clone();
            assert_eq!(
                graphics
                    .plan_stream_source_update(invalid)
                    .unwrap_err()
                    .code,
                "graphics-stream-source-request-invalid"
            );
            assert_eq!(graphics, before_invalid);
        }

        let mut exhausted = graphics.clone();
        exhausted.generation = u64::MAX;
        let idempotent = exhausted.plan_stream_source_update(stream_request).unwrap();
        assert_eq!(idempotent.next_generation, u64::MAX);
        assert!(!exhausted.commit_stream_source_update(idempotent).changed);
        let before_exhausted_change = exhausted.clone();
        assert_eq!(
            exhausted
                .plan_stream_source_update(RuntimeGraphicsStreamSourceUpdateRequest {
                    offset_in_bytes: 4,
                    ..stream_request
                })
                .unwrap_err()
                .code,
            "graphics-generation-exhausted"
        );
        assert_eq!(exhausted, before_exhausted_change);

        let retirement = graphics
            .plan_com_object_retirements(&[RuntimeComObjectId(8)].into_iter().collect())
            .unwrap()
            .unwrap();
        let device_generation = graphics.device(device.id).unwrap().generation;
        graphics
            .preflight_com_object_retirement(&retirement)
            .unwrap();
        graphics.commit_com_object_retirement(retirement);
        let device_after_retirement = graphics.device(device.id).unwrap();
        assert_eq!(device_after_retirement.stream_sources.stream_zero, None);
        assert_eq!(device_after_retirement.generation, device_generation + 1);
        assert!(graphics.vertex_buffer(vertex_buffer.id).is_err());
    }

    #[test]
    fn back_buffer_clears_replace_uniform_content_and_reject_stale_plans_atomically() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics.plan_device_creation(request()).unwrap();
        let device = graphics.commit_device_creation(device_plan, RuntimeComObjectId(7));
        assert!(device.back_buffer.is_pristine());

        let plan = graphics
            .plan_back_buffer_clear(RuntimeGraphicsBackBufferClearRequest {
                device_com_object_id: RuntimeComObjectId(7),
                color_argb: 0xff00_0000,
            })
            .unwrap();
        let stale = plan.clone();
        graphics.preflight_back_buffer_clear(&plan).unwrap();
        let first = graphics.commit_back_buffer_clear(plan);
        assert_eq!(first.device_id, device.id);
        assert_eq!(first.content_generation, 1);
        assert_eq!(first.clear.sequence, 1);
        assert_eq!(first.clear.color_argb, 0xff00_0000);
        assert_eq!(
            first.clear.rect,
            RuntimeRect {
                left: 0,
                top: 0,
                right: 800,
                bottom: 600,
            }
        );
        assert_eq!(
            graphics
                .preflight_back_buffer_clear(&stale)
                .unwrap_err()
                .code,
            "graphics-back-buffer-clear-plan-stale"
        );
        let committed = graphics.device(device.id).unwrap();
        assert_eq!(committed.back_buffer.content_generation, 1);
        assert_eq!(committed.back_buffer.uniform_clear, Some(first.clear));
        assert_eq!(committed.generation, device.generation + 1);

        let stale = graphics
            .plan_back_buffer_clear(RuntimeGraphicsBackBufferClearRequest {
                device_com_object_id: RuntimeComObjectId(7),
                color_argb: 0xff00_0000,
            })
            .unwrap();
        let intervening = graphics
            .plan_back_buffer_clear(RuntimeGraphicsBackBufferClearRequest {
                device_com_object_id: RuntimeComObjectId(7),
                color_argb: 0xff00_00ff,
            })
            .unwrap();
        let second = graphics.commit_back_buffer_clear(intervening);
        assert_eq!(second.clear.sequence, 2);
        assert_eq!(second.content_generation, 2);
        assert_eq!(second.clear.color_argb, 0xff00_00ff);
        let before_stale = graphics.clone();
        assert_eq!(
            graphics
                .preflight_back_buffer_clear(&stale)
                .unwrap_err()
                .code,
            "graphics-back-buffer-clear-plan-stale"
        );
        assert_eq!(graphics, before_stale);

        for device_com_object_id in [RuntimeComObjectId(0), RuntimeComObjectId(99)] {
            let before = graphics.clone();
            let error = graphics
                .plan_back_buffer_clear(RuntimeGraphicsBackBufferClearRequest {
                    device_com_object_id,
                    color_argb: 0xff00_0000,
                })
                .unwrap_err();
            assert!(matches!(
                error.code,
                "graphics-back-buffer-clear-request-invalid"
                    | "graphics-back-buffer-clear-device-com-object-unknown"
            ));
            assert_eq!(graphics, before);
        }

        graphics.next_back_buffer_clear_sequence = u64::MAX;
        let exhausted = graphics.clone();
        assert_eq!(
            graphics
                .plan_back_buffer_clear(RuntimeGraphicsBackBufferClearRequest {
                    device_com_object_id: RuntimeComObjectId(7),
                    color_argb: 0xff00_0000,
                })
                .unwrap_err()
                .code,
            "graphics-back-buffer-clear-sequence-exhausted"
        );
        assert_eq!(graphics, exhausted);
    }

    #[test]
    fn draw_submissions_snapshot_prepared_state_and_clear_or_retirement_remain_safe() {
        let (mut graphics, device_id, texture_id, vertex_buffer_id, backing_range) =
            prepared_draw_graphics();
        let request = RuntimeGraphicsDrawCaptureRequest {
            device_com_object_id: RuntimeComObjectId(7),
            primitive_topology: RuntimeGraphicsPrimitiveTopology::TriangleStrip,
            start_vertex: 0,
            primitive_count: 2,
        };
        let before_capture = graphics.clone();
        let capture = graphics.plan_draw_capture(request).unwrap();
        assert_eq!(
            capture.vertex_range(),
            GuestRange::new(backing_range.start(), 112).unwrap()
        );
        assert_eq!(graphics, before_capture);
        let before_short_snapshot = graphics.clone();
        assert_eq!(
            graphics
                .prepare_draw_submission(capture.clone(), vec![0; 111])
                .unwrap_err()
                .code,
            "graphics-draw-vertex-snapshot-length-mismatch"
        );
        assert_eq!(graphics, before_short_snapshot);

        let vertex_data = (0_u8..112).collect::<Vec<_>>();
        let plan = graphics
            .prepare_draw_submission(capture, vertex_data.clone())
            .unwrap();
        let stale = plan.clone();
        graphics.preflight_draw_submission(&plan).unwrap();
        assert_eq!(graphics, before_capture);
        let result = graphics.commit_draw_submission(plan);
        assert_eq!(result.device_id, device_id);
        assert_eq!(result.content_generation, 2);
        assert_eq!(result.submission.sequence, 1);
        assert_eq!(result.submission.scene_sequence, 1);
        assert_eq!(result.submission.vertex_count, 4);
        assert_eq!(result.submission.vertex_data, vertex_data);
        assert_eq!(result.submission.texture.texture_id, texture_id);
        assert_eq!(result.submission.texture.content.content_generation, 1);
        assert_eq!(
            result.submission.stream_source.vertex_buffer_id,
            vertex_buffer_id
        );
        assert_eq!(result.submission.vertex_buffer_content_generation, 1);
        assert_eq!(
            graphics.preflight_draw_submission(&stale).unwrap_err().code,
            "graphics-draw-submission-plan-stale"
        );
        let committed_draws = graphics
            .device(device_id)
            .unwrap()
            .back_buffer
            .draw_submissions
            .clone();
        assert_eq!(committed_draws, vec![result.submission.clone()]);

        let retirement = graphics
            .plan_com_object_retirements(
                &[RuntimeComObjectId(8), RuntimeComObjectId(9)]
                    .into_iter()
                    .collect(),
            )
            .unwrap()
            .unwrap();
        graphics
            .preflight_com_object_retirement(&retirement)
            .unwrap();
        graphics.commit_com_object_retirement(retirement);
        let retired_device = graphics.device(device_id).unwrap();
        assert_eq!(retired_device.texture_bindings.stage_zero, None);
        assert_eq!(retired_device.stream_sources.stream_zero, None);
        assert_eq!(retired_device.back_buffer.draw_submissions, committed_draws);

        let clear = graphics
            .plan_back_buffer_clear(RuntimeGraphicsBackBufferClearRequest {
                device_com_object_id: RuntimeComObjectId(7),
                color_argb: 0xff00_00ff,
            })
            .unwrap();
        graphics.preflight_back_buffer_clear(&clear).unwrap();
        graphics.commit_back_buffer_clear(clear);
        let cleared = graphics.device(device_id).unwrap();
        assert!(cleared.back_buffer.draw_submissions.is_empty());
        assert_eq!(
            cleared.back_buffer.uniform_clear.unwrap().color_argb,
            0xff00_00ff
        );
    }

    #[test]
    fn draw_submission_limits_and_incomplete_state_fail_before_mutation() {
        let (mut graphics, device_id, _, _, _) = prepared_draw_graphics();
        let request = RuntimeGraphicsDrawCaptureRequest {
            device_com_object_id: RuntimeComObjectId(7),
            primitive_topology: RuntimeGraphicsPrimitiveTopology::TriangleStrip,
            start_vertex: 0,
            primitive_count: 2,
        };
        let capture = graphics.plan_draw_capture(request).unwrap();
        let plan = graphics
            .prepare_draw_submission(capture, vec![0x5a; 112])
            .unwrap();
        let seed = graphics.commit_draw_submission(plan).submission;

        let mut count_limited = graphics.clone();
        count_limited
            .devices
            .get_mut(&device_id)
            .unwrap()
            .back_buffer
            .draw_submissions = vec![seed.clone(); RUNTIME_GRAPHICS_DRAW_SUBMISSION_LIMIT];
        let before_count_limit = count_limited.clone();
        assert_eq!(
            count_limited.plan_draw_capture(request).unwrap_err().code,
            "graphics-draw-submission-capacity-reached"
        );
        assert_eq!(count_limited, before_count_limit);

        let mut byte_limited = graphics.clone();
        let mut oversized = seed;
        oversized.vertex_data = vec![0; RUNTIME_GRAPHICS_DRAW_VERTEX_BYTES_LIMIT];
        byte_limited
            .devices
            .get_mut(&device_id)
            .unwrap()
            .back_buffer
            .draw_submissions = vec![oversized];
        let before_byte_limit = byte_limited.clone();
        assert_eq!(
            byte_limited.plan_draw_capture(request).unwrap_err().code,
            "graphics-draw-vertex-byte-capacity-reached"
        );
        assert_eq!(byte_limited, before_byte_limit);

        let mut incomplete = graphics.clone();
        incomplete.devices.get_mut(&device_id).unwrap().active_scene = None;
        let before_incomplete = incomplete.clone();
        assert_eq!(
            incomplete.plan_draw_capture(request).unwrap_err().code,
            "graphics-draw-scene-inactive"
        );
        assert_eq!(incomplete, before_incomplete);

        for unsupported in [
            RuntimeGraphicsDrawCaptureRequest {
                start_vertex: 1,
                ..request
            },
            RuntimeGraphicsDrawCaptureRequest {
                primitive_count: 1,
                ..request
            },
        ] {
            let before = graphics.clone();
            assert_eq!(
                graphics.plan_draw_capture(unsupported).unwrap_err().code,
                "graphics-draw-capture-request-invalid"
            );
            assert_eq!(graphics, before);
        }
    }

    #[test]
    fn scene_begin_owns_active_lifecycle_and_rejects_nested_or_stale_plans_atomically() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics.plan_device_creation(request()).unwrap();
        let device = graphics.commit_device_creation(device_plan, RuntimeComObjectId(7));
        assert_eq!(device.active_scene, None);

        let before_plan = graphics.clone();
        let plan = graphics
            .plan_scene_begin(RuntimeGraphicsSceneBeginRequest {
                device_com_object_id: RuntimeComObjectId(7),
            })
            .unwrap();
        assert_eq!(graphics, before_plan);
        let stale = plan.clone();
        graphics.preflight_scene_begin(&plan).unwrap();
        let result = graphics.commit_scene_begin(plan);
        assert_eq!(result.device_id, device.id);
        assert_eq!(result.device_generation, device.generation + 1);
        assert_eq!(result.scene.sequence, 1);
        assert_eq!(
            graphics.device(device.id).unwrap().active_scene,
            Some(RuntimeGraphicsScene { sequence: 1 })
        );
        assert_eq!(graphics.next_scene_sequence, 2);

        let before_stale = graphics.clone();
        assert_eq!(
            graphics.preflight_scene_begin(&stale).unwrap_err().code,
            "graphics-scene-begin-plan-stale"
        );
        assert_eq!(graphics, before_stale);

        let before_nested = graphics.clone();
        assert_eq!(
            graphics
                .plan_scene_begin(RuntimeGraphicsSceneBeginRequest {
                    device_com_object_id: RuntimeComObjectId(7),
                })
                .unwrap_err()
                .code,
            "graphics-scene-already-active"
        );
        assert_eq!(graphics, before_nested);

        for device_com_object_id in [RuntimeComObjectId(0), RuntimeComObjectId(99)] {
            let before = graphics.clone();
            let error = graphics
                .plan_scene_begin(RuntimeGraphicsSceneBeginRequest {
                    device_com_object_id,
                })
                .unwrap_err();
            assert!(matches!(
                error.code,
                "graphics-scene-begin-request-invalid"
                    | "graphics-scene-begin-device-com-object-unknown"
            ));
            assert_eq!(graphics, before);
        }

        let mut exhausted = RuntimeGraphics::default();
        let device_plan = exhausted.plan_device_creation(request()).unwrap();
        exhausted.commit_device_creation(device_plan, RuntimeComObjectId(7));
        exhausted.next_scene_sequence = u64::MAX;
        let before_exhausted = exhausted.clone();
        assert_eq!(
            exhausted
                .plan_scene_begin(RuntimeGraphicsSceneBeginRequest {
                    device_com_object_id: RuntimeComObjectId(7),
                })
                .unwrap_err()
                .code,
            "graphics-scene-sequence-exhausted"
        );
        assert_eq!(exhausted, before_exhausted);
    }

    #[test]
    fn scene_end_closes_only_the_active_lifecycle_and_preserves_draw_content_atomically() {
        let (mut graphics, device_id, _, _, _) = prepared_draw_graphics();
        let draw = graphics
            .plan_draw_capture(RuntimeGraphicsDrawCaptureRequest {
                device_com_object_id: RuntimeComObjectId(7),
                primitive_topology: RuntimeGraphicsPrimitiveTopology::TriangleStrip,
                start_vertex: 0,
                primitive_count: 2,
            })
            .unwrap();
        let draw = graphics
            .prepare_draw_submission(draw, vec![0x5a; 112])
            .unwrap();
        graphics.commit_draw_submission(draw);
        let back_buffer = graphics.device(device_id).unwrap().back_buffer.clone();
        let next_scene_sequence = graphics.next_scene_sequence;

        let before_plan = graphics.clone();
        let plan = graphics
            .plan_scene_end(RuntimeGraphicsSceneEndRequest {
                device_com_object_id: RuntimeComObjectId(7),
            })
            .unwrap();
        assert_eq!(graphics, before_plan);
        let stale = plan.clone();
        graphics.preflight_scene_end(&plan).unwrap();
        let previous_device_generation = graphics.device(device_id).unwrap().generation;
        let result = graphics.commit_scene_end(plan);
        assert_eq!(result.device_id, device_id);
        assert_eq!(result.device_generation, previous_device_generation + 1);
        assert_eq!(result.scene, RuntimeGraphicsScene { sequence: 1 });
        let device = graphics.device(device_id).unwrap();
        assert_eq!(device.active_scene, None);
        assert_eq!(device.back_buffer, back_buffer);
        assert_eq!(graphics.next_scene_sequence, next_scene_sequence);

        let before_stale = graphics.clone();
        assert_eq!(
            graphics.preflight_scene_end(&stale).unwrap_err().code,
            "graphics-scene-end-plan-stale"
        );
        assert_eq!(graphics, before_stale);

        let before_repeat = graphics.clone();
        assert_eq!(
            graphics
                .plan_scene_end(RuntimeGraphicsSceneEndRequest {
                    device_com_object_id: RuntimeComObjectId(7),
                })
                .unwrap_err()
                .code,
            "graphics-scene-not-active"
        );
        assert_eq!(graphics, before_repeat);

        let begin = graphics
            .plan_scene_begin(RuntimeGraphicsSceneBeginRequest {
                device_com_object_id: RuntimeComObjectId(7),
            })
            .unwrap();
        let second_scene = graphics.commit_scene_begin(begin).scene;
        assert_eq!(second_scene.sequence, next_scene_sequence);
        assert_eq!(graphics.device(device_id).unwrap().back_buffer, back_buffer);

        for device_com_object_id in [RuntimeComObjectId(0), RuntimeComObjectId(99)] {
            let before = graphics.clone();
            let error = graphics
                .plan_scene_end(RuntimeGraphicsSceneEndRequest {
                    device_com_object_id,
                })
                .unwrap_err();
            assert!(matches!(
                error.code,
                "graphics-scene-end-request-invalid"
                    | "graphics-scene-end-device-com-object-unknown"
            ));
            assert_eq!(graphics, before);
        }

        let mut exhausted = graphics.clone();
        exhausted.devices.get_mut(&device_id).unwrap().generation = u64::MAX;
        let before_exhausted = exhausted.clone();
        assert_eq!(
            exhausted
                .plan_scene_end(RuntimeGraphicsSceneEndRequest {
                    device_com_object_id: RuntimeComObjectId(7),
                })
                .unwrap_err()
                .code,
            "graphics-device-generation-exhausted"
        );
        assert_eq!(exhausted, before_exhausted);
    }

    #[test]
    fn present_moves_closed_back_buffer_to_front_frame_and_rejects_invalid_or_stale_state_atomically()
     {
        let (mut graphics, device_id, _, _, _) = prepared_draw_graphics();
        let draw = graphics
            .plan_draw_capture(RuntimeGraphicsDrawCaptureRequest {
                device_com_object_id: RuntimeComObjectId(7),
                primitive_topology: RuntimeGraphicsPrimitiveTopology::TriangleStrip,
                start_vertex: 0,
                primitive_count: 2,
            })
            .unwrap();
        let draw = graphics
            .prepare_draw_submission(draw, vec![0x5a; 112])
            .unwrap();
        graphics.commit_draw_submission(draw);
        let end = graphics
            .plan_scene_end(RuntimeGraphicsSceneEndRequest {
                device_com_object_id: RuntimeComObjectId(7),
            })
            .unwrap();
        graphics.commit_scene_end(end);

        let request = RuntimeGraphicsPresentRequest {
            device_com_object_id: RuntimeComObjectId(7),
        };
        let before_plan = graphics.clone();
        let plan = graphics.plan_present(request).unwrap();
        let stale = plan.clone();
        graphics.preflight_present(&plan).unwrap();
        assert_eq!(graphics, before_plan);
        let previous_device_generation = graphics.device(device_id).unwrap().generation;
        let result = graphics.commit_present(plan);
        assert_eq!(result.device_id, device_id);
        assert_eq!(result.device_generation, previous_device_generation + 1);
        assert_eq!(result.frame.sequence, 1);
        assert_eq!(result.frame.back_buffer.content_generation, 2);
        assert_eq!(result.frame.back_buffer.draw_submissions.len(), 1);
        assert_eq!(result.next_back_buffer_content_generation, 3);
        let device = graphics.device(device_id).unwrap();
        assert_eq!(device.presented_frame, Some(result.frame.clone()));
        assert_eq!(device.back_buffer.content_generation, 3);
        assert_eq!(device.back_buffer.uniform_clear, None);
        assert!(device.back_buffer.draw_submissions.is_empty());
        assert_eq!(graphics.next_present_sequence, 2);

        let before_stale = graphics.clone();
        assert_eq!(
            graphics.preflight_present(&stale).unwrap_err().code,
            "graphics-present-plan-stale"
        );
        assert_eq!(graphics, before_stale);
        assert_eq!(
            graphics.plan_present(request).unwrap_err().code,
            "graphics-present-back-buffer-empty"
        );
        assert_eq!(graphics, before_stale);

        for device_com_object_id in [RuntimeComObjectId(0), RuntimeComObjectId(99)] {
            let before = graphics.clone();
            let error = graphics
                .plan_present(RuntimeGraphicsPresentRequest {
                    device_com_object_id,
                })
                .unwrap_err();
            assert!(matches!(
                error.code,
                "graphics-present-request-invalid" | "graphics-present-device-com-object-unknown"
            ));
            assert_eq!(graphics, before);
        }

        let (active, _, _, _, _) = prepared_draw_graphics();
        let before_active = active.clone();
        assert_eq!(
            active.plan_present(request).unwrap_err().code,
            "graphics-present-scene-active"
        );
        assert_eq!(active, before_active);

        let (mut exhausted, exhausted_device_id, _, _, _) = prepared_draw_graphics();
        let end = exhausted
            .plan_scene_end(RuntimeGraphicsSceneEndRequest {
                device_com_object_id: RuntimeComObjectId(7),
            })
            .unwrap();
        exhausted.commit_scene_end(end);
        exhausted.next_present_sequence = u64::MAX;
        let before_sequence_exhausted = exhausted.clone();
        assert_eq!(
            exhausted.plan_present(request).unwrap_err().code,
            "graphics-present-sequence-exhausted"
        );
        assert_eq!(exhausted, before_sequence_exhausted);

        exhausted.next_present_sequence = 1;
        exhausted
            .devices
            .get_mut(&exhausted_device_id)
            .unwrap()
            .back_buffer
            .content_generation = u64::MAX;
        let before_content_exhausted = exhausted.clone();
        assert_eq!(
            exhausted.plan_present(request).unwrap_err().code,
            "graphics-back-buffer-content-generation-exhausted"
        );
        assert_eq!(exhausted, before_content_exhausted);
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
            texture.dirty_region,
            RuntimeGraphicsTextureDirtyRegion::WholeTexture
        );
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
    fn texture_dirty_regions_are_bounded_coalesced_and_failure_atomic() {
        let mut graphics = RuntimeGraphics::default();
        let device_plan = graphics.plan_device_creation(request()).unwrap();
        graphics.commit_device_creation(device_plan, RuntimeComObjectId(7));
        let texture_plan = graphics
            .plan_texture_creation(texture_request(RuntimeComObjectId(7)))
            .unwrap();
        let texture = graphics.commit_texture_creation(texture_plan, RuntimeComObjectId(8));

        let before_initial_mark = graphics.clone();
        let initial_mark = graphics
            .plan_texture_dirty_region_mark(RuntimeGraphicsTextureDirtyRegionMarkRequest {
                texture_com_object_id: RuntimeComObjectId(8),
                rect: Some(RuntimeRect {
                    left: 0,
                    top: 0,
                    right: 800,
                    bottom: 600,
                }),
            })
            .unwrap();
        let result = graphics.commit_texture_dirty_region_mark(initial_mark);
        assert!(!result.changed);
        assert_eq!(
            result.dirty_region,
            RuntimeGraphicsTextureDirtyRegion::WholeTexture
        );
        assert_eq!(graphics, before_initial_mark);

        graphics.textures.get_mut(&texture.id).unwrap().dirty_region =
            RuntimeGraphicsTextureDirtyRegion::Clean;
        let first = graphics
            .plan_texture_dirty_region_mark(RuntimeGraphicsTextureDirtyRegionMarkRequest {
                texture_com_object_id: RuntimeComObjectId(8),
                rect: Some(RuntimeRect {
                    left: 1,
                    top: 2,
                    right: 17,
                    bottom: 19,
                }),
            })
            .unwrap();
        let stale = first.clone();
        graphics
            .preflight_texture_dirty_region_mark(&first)
            .unwrap();
        let first = graphics.commit_texture_dirty_region_mark(first);
        assert!(first.changed);
        assert_eq!(
            first.dirty_region,
            RuntimeGraphicsTextureDirtyRegion::Rect(RuntimeRect {
                left: 1,
                top: 2,
                right: 17,
                bottom: 19,
            })
        );
        assert_eq!(
            graphics
                .preflight_texture_dirty_region_mark(&stale)
                .unwrap_err()
                .code,
            "graphics-texture-dirty-region-plan-stale"
        );

        let second = graphics
            .plan_texture_dirty_region_mark(RuntimeGraphicsTextureDirtyRegionMarkRequest {
                texture_com_object_id: RuntimeComObjectId(8),
                rect: Some(RuntimeRect {
                    left: 8,
                    top: 10,
                    right: 32,
                    bottom: 40,
                }),
            })
            .unwrap();
        let second = graphics.commit_texture_dirty_region_mark(second);
        assert_eq!(
            second.dirty_region,
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
            assert_eq!(
                graphics
                    .plan_texture_dirty_region_mark(RuntimeGraphicsTextureDirtyRegionMarkRequest {
                        texture_com_object_id: RuntimeComObjectId(8),
                        rect: Some(rect),
                    },)
                    .unwrap_err()
                    .code,
                "graphics-texture-dirty-rect-out-of-bounds"
            );
            assert_eq!(graphics, before);
        }

        let whole = graphics
            .plan_texture_dirty_region_mark(RuntimeGraphicsTextureDirtyRegionMarkRequest {
                texture_com_object_id: RuntimeComObjectId(8),
                rect: None,
            })
            .unwrap();
        let whole = graphics.commit_texture_dirty_region_mark(whole);
        assert!(whole.changed);
        assert_eq!(
            whole.dirty_region,
            RuntimeGraphicsTextureDirtyRegion::WholeTexture
        );

        graphics.textures.get_mut(&texture.id).unwrap().dirty_region =
            RuntimeGraphicsTextureDirtyRegion::Clean;
        graphics.generation = u64::MAX;
        let exhausted = graphics.clone();
        assert_eq!(
            graphics
                .plan_texture_dirty_region_mark(RuntimeGraphicsTextureDirtyRegionMarkRequest {
                    texture_com_object_id: RuntimeComObjectId(8),
                    rect: Some(RuntimeRect {
                        left: 0,
                        top: 0,
                        right: 1,
                        bottom: 1,
                    }),
                })
                .unwrap_err()
                .code,
            "graphics-generation-exhausted"
        );
        assert_eq!(graphics, exhausted);
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
    fn surface_fill_mutates_texture_content_and_rejects_stale_or_invalid_plans() {
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
        assert_eq!(result.texture_id, surface.texture_id);
        assert_eq!(result.content_generation, 1);
        assert_eq!(
            result.dirty_region,
            RuntimeGraphicsTextureDirtyRegion::WholeTexture
        );
        assert_eq!(result.fill.unwrap().rect, rect);
        let filled = graphics.texture(surface.texture_id).unwrap();
        assert_eq!(filled.content.fills.len(), 1);
        assert_eq!(filled.content.fills[0].color_ref, 0);
        assert_eq!(filled.content.content_generation, 1);
        assert_eq!(graphics.surface(surface.id).unwrap(), &surface);
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

    #[test]
    fn texture_update_consumes_whole_dirty_content_after_surface_retirement_atomically() {
        let mut graphics = RuntimeGraphics::default();
        let device = graphics.plan_device_creation(request()).unwrap();
        let device = graphics.commit_device_creation(device, RuntimeComObjectId(1));
        let source = graphics
            .plan_texture_creation(texture_request(RuntimeComObjectId(1)))
            .unwrap();
        let source = graphics.commit_texture_creation(source, RuntimeComObjectId(2));
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
        let fill = graphics
            .plan_surface_fill(fill_request(RuntimeComObjectId(3), rect))
            .unwrap();
        graphics.commit_surface_fill(fill);

        let mut destination_request = texture_request(RuntimeComObjectId(1));
        destination_request.descriptor.pool = RuntimeGraphicsTexturePool::Default;
        let destination = graphics.plan_texture_creation(destination_request).unwrap();
        let destination = graphics.commit_texture_creation(destination, RuntimeComObjectId(4));

        let surface_retirement = graphics
            .plan_com_object_retirements(&[RuntimeComObjectId(3)].into_iter().collect())
            .unwrap()
            .unwrap();
        graphics.commit_com_object_retirement(surface_retirement);
        assert!(graphics.surfaces().is_empty());
        assert_eq!(
            graphics.texture(source.id).unwrap().content.fills[0].rect,
            rect
        );

        let request = RuntimeGraphicsTextureUpdateRequest {
            device_com_object_id: RuntimeComObjectId(1),
            source_texture_com_object_id: RuntimeComObjectId(2),
            destination_texture_com_object_id: RuntimeComObjectId(4),
        };
        let plan = graphics.plan_texture_update(request).unwrap();
        let stale = plan.clone();
        graphics.preflight_texture_update(&plan).unwrap();
        let result = graphics.commit_texture_update(plan);
        assert_eq!(result.source_texture_id, source.id);
        assert_eq!(result.destination_texture_id, destination.id);
        assert!(result.transferred);
        assert_eq!(result.destination_content_generation, 1);
        assert_eq!(result.update.unwrap().sequence, 1);
        let committed_source = graphics.texture(source.id).unwrap();
        assert_eq!(
            committed_source.dirty_region,
            RuntimeGraphicsTextureDirtyRegion::Clean
        );
        assert_eq!(committed_source.content.fills.len(), 1);
        let committed_destination = graphics.texture(destination.id).unwrap();
        assert_eq!(
            committed_destination.content.fills,
            committed_source.content.fills
        );
        assert_eq!(committed_destination.content.content_generation, 1);
        assert_eq!(
            committed_destination.content.last_update,
            Some(RuntimeGraphicsTextureUpdate {
                sequence: 1,
                source_texture_id: source.id,
                source_content_generation: 1,
                dirty_region: RuntimeGraphicsTextureDirtyRegion::WholeTexture,
            })
        );
        assert_eq!(
            graphics.preflight_texture_update(&stale).unwrap_err().code,
            "graphics-texture-update-plan-stale"
        );

        let before_repeat = graphics.clone();
        let repeat = graphics.plan_texture_update(request).unwrap();
        let repeat = graphics.commit_texture_update(repeat);
        assert!(!repeat.transferred);
        assert!(repeat.update.is_none());
        assert_eq!(graphics, before_repeat);

        let before_reversed = graphics.clone();
        assert_eq!(
            graphics
                .plan_texture_update(RuntimeGraphicsTextureUpdateRequest {
                    device_com_object_id: RuntimeComObjectId(1),
                    source_texture_com_object_id: RuntimeComObjectId(4),
                    destination_texture_com_object_id: RuntimeComObjectId(2),
                })
                .unwrap_err()
                .code,
            "graphics-texture-update-pair-invalid"
        );
        assert_eq!(graphics, before_reversed);

        let mark = graphics
            .plan_texture_dirty_region_mark(RuntimeGraphicsTextureDirtyRegionMarkRequest {
                texture_com_object_id: RuntimeComObjectId(2),
                rect: Some(RuntimeRect {
                    left: 1,
                    top: 2,
                    right: 17,
                    bottom: 19,
                }),
            })
            .unwrap();
        graphics.commit_texture_dirty_region_mark(mark);
        let before_partial = graphics.clone();
        assert_eq!(
            graphics.plan_texture_update(request).unwrap_err().code,
            "graphics-texture-update-partial-dirty-region-unsupported"
        );
        assert_eq!(graphics, before_partial);
        assert_eq!(graphics.device(device.id).unwrap().id, device.id);
        assert_eq!(graphics.next_texture_update_sequence, 2);
        assert_eq!(surface.texture_id, source.id);
    }
}

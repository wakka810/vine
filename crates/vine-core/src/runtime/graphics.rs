use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

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
pub(crate) const RUNTIME_GRAPHICS_TEXTURE_LOCK_READONLY: u32 = 0x10;
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
    Point,
    Linear,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsTextureAddress {
    #[default]
    Wrap,
    Clamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsTextureOperation {
    Disable,
    Modulate,
    SelectArgumentOne,
    Add,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsTextureArgument {
    Diffuse,
    Texture,
    TFactor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsFlexibleVertexFormat {
    XyzRhwDiffuse,
    XyzTexture1,
    XyzDiffuseTexture1,
    XyzRhwTexture1,
    XyzRhwDiffuseTexture1,
}

impl RuntimeGraphicsFlexibleVertexFormat {
    pub(crate) fn from_d3d8_code(code: u32) -> Option<Self> {
        match code {
            0x044 => Some(Self::XyzRhwDiffuse),
            0x102 => Some(Self::XyzTexture1),
            0x142 => Some(Self::XyzDiffuseTexture1),
            0x104 => Some(Self::XyzRhwTexture1),
            0x144 => Some(Self::XyzRhwDiffuseTexture1),
            _ => None,
        }
    }

    pub(crate) fn stride(self) -> u32 {
        match self {
            Self::XyzRhwDiffuse | Self::XyzTexture1 => 20,
            Self::XyzDiffuseTexture1 | Self::XyzRhwTexture1 => 24,
            Self::XyzRhwDiffuseTexture1 => 28,
        }
    }

    pub(crate) fn is_untransformed_xyz(self) -> bool {
        matches!(self, Self::XyzTexture1 | Self::XyzDiffuseTexture1)
    }

    pub(crate) fn has_diffuse(self) -> bool {
        matches!(
            self,
            Self::XyzRhwDiffuse | Self::XyzDiffuseTexture1 | Self::XyzRhwDiffuseTexture1
        )
    }

    pub(crate) fn has_tex1(self) -> bool {
        !matches!(self, Self::XyzRhwDiffuse)
    }
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
    pub address_u: RuntimeGraphicsTextureAddress,
    pub address_v: RuntimeGraphicsTextureAddress,
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
            RuntimeGraphicsSamplerStateUpdate::AddressU { sampler: 0, .. } => {
                Some(RuntimeGraphicsSamplerStateUpdate::AddressU {
                    sampler: 0,
                    address: self.sampler_zero.address_u,
                })
            }
            RuntimeGraphicsSamplerStateUpdate::AddressV { sampler: 0, .. } => {
                Some(RuntimeGraphicsSamplerStateUpdate::AddressV {
                    sampler: 0,
                    address: self.sampler_zero.address_v,
                })
            }
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
            RuntimeGraphicsSamplerStateUpdate::AddressU {
                sampler: 0,
                address,
            } => {
                self.sampler_zero.address_u = address;
            }
            RuntimeGraphicsSamplerStateUpdate::AddressV {
                sampler: 0,
                address,
            } => {
                self.sampler_zero.address_v = address;
            }
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
    AddressU {
        sampler: u32,
        address: RuntimeGraphicsTextureAddress,
    },
    AddressV {
        sampler: u32,
        address: RuntimeGraphicsTextureAddress,
    },
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
            Self::MinFilter { sampler, .. }
            | Self::MagFilter { sampler, .. }
            | Self::AddressU { sampler, .. }
            | Self::AddressV { sampler, .. } => sampler,
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
    One,
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
#[allow(
    clippy::enum_variant_names,
    reason = "formats name both the packed channel layout and normalized component type"
)]
pub enum RuntimeGraphicsColorFormat {
    B8g8r8x8Unorm,
    B8g8r8a8Unorm,
    R5g6b5Unorm,
    X1r5g5b5Unorm,
    A1r5g5b5Unorm,
    A4r4g4b4Unorm,
}

impl RuntimeGraphicsColorFormat {
    pub const fn bytes_per_pixel(self) -> u32 {
        match self {
            Self::B8g8r8x8Unorm | Self::B8g8r8a8Unorm => 4,
            Self::R5g6b5Unorm | Self::X1r5g5b5Unorm | Self::A1r5g5b5Unorm | Self::A4r4g4b4Unorm => {
                2
            }
        }
    }

    pub const fn has_alpha(self) -> bool {
        matches!(
            self,
            Self::B8g8r8a8Unorm | Self::A1r5g5b5Unorm | Self::A4r4g4b4Unorm
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsTextureUsage {
    None,
    Dynamic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsTexturePool {
    Default,
    Managed,
    SystemMemory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsVertexBufferUsage {
    None,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeGraphicsRetainedTexturePixels {
    pub data: Arc<Vec<u8>>,
    pub row_pitch: u32,
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
    pub retained_pixels: Option<RuntimeGraphicsRetainedTexturePixels>,
    pub(crate) last_update: Option<RuntimeGraphicsTextureUpdate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeGraphicsFogState {
    pub color_argb: u32,
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct RuntimeGraphicsFragmentState {
    pub texture_alpha: bool,
    pub diffuse_alpha: bool,
    pub blend: bool,
    pub additive_blend: bool,
    pub color_add: bool,
    pub alpha_test: Option<(u32, u32)>,
    pub fog: Option<RuntimeGraphicsFogState>,
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct RuntimeGraphicsDrawSubmission {
    pub clip_rect: Option<RuntimeRect>,
    pub fragment: Option<RuntimeGraphicsFragmentState>,
    pub texture_alpha: bool,
    pub depth: Option<RuntimeGraphicsDepthDrawState>,
    pub vertex_data: Vec<u8>,
    pub texture_data: Arc<Vec<u8>>,
    pub(crate) texture_source_key: Option<(RuntimeGraphicsTextureId, u64)>,
    pub min_filter: RuntimeGraphicsTextureFilter,
    pub mag_filter: RuntimeGraphicsTextureFilter,
    pub address_u: RuntimeGraphicsTextureAddress,
    pub address_v: RuntimeGraphicsTextureAddress,

    pub texture_width: u32,
    pub texture_height: u32,
    pub texture_fills: Vec<RuntimeGraphicsSurfaceFill>,
    pub texture_pitch: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeGraphicsDepthDrawState {
    pub write: bool,
    pub compare: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeGraphicsDepthClear {
    pub before_draw: usize,
    pub depth: u32,
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct RuntimeGraphicsBackBuffer {
    pub preserve_previous: bool,
    pub depth_clears: Vec<RuntimeGraphicsDepthClear>,
    pub clear_color: Option<u32>,
    pub color_clear_before_draw: usize,
    pub draw_submissions: Vec<RuntimeGraphicsDrawSubmission>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGraphicsPresentMode {
    Discard,
    Copy,
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
    pub viewport: [u32; 6],
    pub d3d8: Option<crate::d3d8::DeviceState>,
    pub id: RuntimeGraphicsDeviceId,
    pub(super) target: RuntimeGraphicsWindowTarget,
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
    pub presented_extent: Option<(u32, u32)>,
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
    pub(super) back_buffer_width: u32,
    pub(super) back_buffer_height: u32,
    pub(super) color_format: RuntimeGraphicsColorFormat,
    pub(super) back_buffer_count: u32,
    pub(super) sample_count: u32,
    pub(super) present_mode: RuntimeGraphicsPresentMode,
    pub(super) windowed: bool,
}

#[derive(Debug)]
struct RuntimeGraphicsDrawResolvedState {
    vertex_memory_owner: MemoryOwnerId,

    vertex_range: GuestRange,

    texture_id: Option<RuntimeGraphicsTextureId>,
}

impl RuntimeGraphicsDeviceCreationRequest {
    pub(crate) fn presentation(&self) -> RuntimeGraphicsPresentation {
        RuntimeGraphicsPresentation {
            width: self.back_buffer_width,
            height: self.back_buffer_height,
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
    pub(super) fn copy_d3d8_surface(
        &mut self,
        device_object: RuntimeComObjectId,
        source_object: RuntimeComObjectId,
        rectangles: &[(RuntimeRect, i32, i32)],
        memory: &crate::memory::GuestMemory,
    ) -> Result<bool, RuntimeStateError> {
        let device = self.device_by_com_object(device_object)?;
        let source = self.texture_for_com_object(source_object)?;
        if device.d3d8.is_none() || source.device_id != device.id || source.active_lock.is_some() {
            return Ok(false);
        }
        let Some(backing) = source.backing else {
            return Ok(false);
        };
        let d = source.descriptor;
        let default = [(
            RuntimeRect {
                left: 0,
                top: 0,
                right: d.width as i32,
                bottom: d.height as i32,
            },
            0,
            0,
        )];
        let rectangles = if rectangles.is_empty() {
            &default[..]
        } else {
            rectangles
        };
        for (r, x, y) in rectangles {
            if r.left < 0
                || r.top < 0
                || r.right <= r.left
                || r.bottom <= r.top
                || r.right as u32 > d.width
                || r.bottom as u32 > d.height
                || *x < 0
                || *y < 0
                || i64::from(*x) + i64::from(r.right - r.left)
                    > i64::from(device.presentation.width)
                || i64::from(*y) + i64::from(r.bottom - r.top)
                    > i64::from(device.presentation.height)
            {
                return Ok(false);
            }
        }
        let full_cover = rectangles.len() == 1
            && rectangles[0].1 == 0
            && rectangles[0].2 == 0
            && (rectangles[0].0.right - rectangles[0].0.left) as u32 == device.presentation.width
            && (rectangles[0].0.bottom - rectangles[0].0.top) as u32 == device.presentation.height;
        let trailing_depth_clear = device
            .back_buffer
            .depth_clears
            .last()
            .filter(|c| c.before_draw == device.back_buffer.draw_submissions.len())
            .copied();
        let no_depth_written = device.back_buffer.depth_clears.is_empty()
            && device
                .back_buffer
                .draw_submissions
                .iter()
                .all(|draw| draw.depth.is_none_or(|d| !d.write));
        let replaces_all = full_cover && (no_depth_written || trailing_depth_clear.is_some());
        let retained = if replaces_all {
            0
        } else {
            device.back_buffer.draw_submissions.len()
        };
        let retained_bytes = if replaces_all {
            0
        } else {
            let mut seen_ptrs = BTreeSet::new();
            let mut sum = 0usize;
            for d_sub in &device.back_buffer.draw_submissions {
                if !d_sub.texture_data.is_empty()
                    && seen_ptrs.insert(Arc::as_ptr(&d_sub.texture_data) as usize)
                {
                    sum = sum.saturating_add(d_sub.texture_data.len());
                }
            }
            sum
        };
        let length = backing.row_pitch as usize * d.height as usize;
        let snapshot_bytes = (d.width as usize)
            .saturating_mul(d.height as usize)
            .saturating_mul(4);
        if retained + rectangles.len() > RUNTIME_GRAPHICS_DRAW_SUBMISSION_LIMIT
            || snapshot_bytes > (16 * 1024 * 1024usize).saturating_sub(retained_bytes)
        {
            return Err(RuntimeStateError::new(
                "CopyRects exceeds the bounded frame snapshot capacity",
            ));
        }
        let raw_pixels = memory
            .read_owned(backing.range.start(), length, backing.memory_owner)
            .map_err(RuntimeStateError::from_memory)?;
        let (normalized_pixels, snapshot_pitch) = normalize_texture_snapshot_to_bgra8(
            raw_pixels,
            d.width,
            d.height,
            backing.row_pitch,
            d.color_format,
        );
        let pixels = Arc::new(normalized_pixels);
        let mut submissions = Vec::new();
        for (r, x, y) in rectangles {
            let mut vertices = Vec::with_capacity(112);
            for (px, py, u, v) in [
                (*x, *y, r.left, r.top),
                (*x + r.right - r.left, *y, r.right, r.top),
                (*x, *y + r.bottom - r.top, r.left, r.bottom),
                (
                    *x + r.right - r.left,
                    *y + r.bottom - r.top,
                    r.right,
                    r.bottom,
                ),
            ] {
                for value in [px as f32 - 0.5, py as f32 - 0.5, 0.0, 1.0] {
                    vertices.extend(value.to_le_bytes());
                }
                vertices.extend([255; 4]);
                vertices.extend((u as f32 / d.width as f32).to_le_bytes());
                vertices.extend((v as f32 / d.height as f32).to_le_bytes());
            }
            submissions.push(RuntimeGraphicsDrawSubmission {
                clip_rect: None,
                fragment: None,
                texture_alpha: false,
                depth: None,
                vertex_data: vertices,
                texture_data: pixels.clone(),
                texture_source_key: None,
                min_filter: RuntimeGraphicsTextureFilter::Point,
                mag_filter: RuntimeGraphicsTextureFilter::Point,
                address_u: RuntimeGraphicsTextureAddress::Clamp,
                address_v: RuntimeGraphicsTextureAddress::Clamp,
                texture_width: d.width,
                texture_height: d.height,
                texture_fills: Vec::new(),
                texture_pitch: Some(snapshot_pitch),
            });
        }
        let id = device.id;
        let device = self.devices.get_mut(&id).expect("validated device");
        if replaces_all {
            let back_buffer = &mut device.back_buffer;
            back_buffer.clear_color = None;
            back_buffer.color_clear_before_draw = 0;
            back_buffer.draw_submissions.clear();
            back_buffer.depth_clears.clear();
            if let Some(clear) = trailing_depth_clear {
                back_buffer.preserve_previous = false;
                back_buffer.depth_clears.push(RuntimeGraphicsDepthClear {
                    before_draw: 0,
                    depth: clear.depth,
                });
            }
        } else {
            for (r, x, y) in rectangles {
                prune_occluded_draws(
                    &mut device.back_buffer,
                    RuntimeRect {
                        left: *x,
                        top: *y,
                        right: *x + (r.right - r.left),
                        bottom: *y + (r.bottom - r.top),
                    },
                );
            }
        }
        device.back_buffer.draw_submissions.extend(submissions);
        Ok(true)
    }

    pub(super) fn configure_d3d8(
        &mut self,
        object: RuntimeComObjectId,
        state: crate::d3d8::DeviceState,
    ) {
        let id = self.device_com_object_index[&object];
        let device = self.devices.get_mut(&id).expect("live device");
        device.presentation.windowed = state.parameters[7] != 0;
        device.render_states.depth_test_enabled = Some(state.parameters[8] != 0);
        device.render_states.lighting_enabled = Some(true);
        device.render_states.alpha_blend_enabled = Some(false);
        device.render_states.blend_operation = Some(RuntimeGraphicsBlendOperation::Add);
        device.texture_stage_states.stage_zero = RuntimeGraphicsTextureStageState {
            color_operation: Some(RuntimeGraphicsTextureOperation::Modulate),
            color_argument_one: Some(RuntimeGraphicsTextureArgument::Texture),
            color_argument_two: Some(RuntimeGraphicsTextureArgument::Diffuse),
            alpha_operation: Some(RuntimeGraphicsTextureOperation::SelectArgumentOne),
            alpha_argument_one: Some(RuntimeGraphicsTextureArgument::Texture),
            alpha_argument_two: Some(RuntimeGraphicsTextureArgument::Diffuse),
        };
        device.sampler_states.sampler_zero.min_filter = Some(RuntimeGraphicsTextureFilter::Point);
        device.sampler_states.sampler_zero.mag_filter = Some(RuntimeGraphicsTextureFilter::Point);
        device.d3d8 = Some(state);
    }

    pub(super) fn d3d8_state_mut(
        &mut self,
        object: RuntimeComObjectId,
    ) -> Result<&mut crate::d3d8::DeviceState, RuntimeStateError> {
        let id = self
            .device_com_object_index
            .get(&object)
            .copied()
            .ok_or_else(|| RuntimeStateError::new("D3D8 device is not live"))?;
        self.devices
            .get_mut(&id)
            .and_then(|device| device.d3d8.as_mut())
            .ok_or_else(|| RuntimeStateError::new("device has no D3D8 state"))
    }

    pub(super) fn clear_d3d8(
        &mut self,
        object: RuntimeComObjectId,
        flags: u32,
        color: u32,
        depth: u32,
    ) -> Result<bool, RuntimeStateError> {
        let id = self
            .device_com_object_index
            .get(&object)
            .copied()
            .ok_or_else(|| RuntimeStateError::new("D3D8 device is not live"))?;
        let device = self.devices.get_mut(&id).expect("live device");
        if flags == 0
            || flags & !3 != 0
            || flags & 2 != 0
                && (device.d3d8.as_ref().is_none_or(|s| s.parameters[8] == 0)
                    || !f32::from_bits(depth).is_finite()
                    || !(0.0..=1.0).contains(&f32::from_bits(depth)))
        {
            return Ok(false);
        }
        if flags & 2 != 0
            && flags & 1 == 0
            && device.back_buffer.depth_clears.len() >= RUNTIME_GRAPHICS_DRAW_SUBMISSION_LIMIT
        {
            return Err(RuntimeStateError::new(
                "depth clear replay capacity exhausted",
            ));
        }
        let back_buffer = &mut device.back_buffer;
        if flags & 1 != 0 {
            if flags & 2 != 0 {
                back_buffer.preserve_previous = false;
                back_buffer.draw_submissions.clear();
                back_buffer.depth_clears.clear();
            } else {
                // Replay only the depth writes that survive the last depth clear.
                // Their color is erased by the new clear after this prefix.
                if let Some(clear) = back_buffer.depth_clears.last().copied() {
                    back_buffer.draw_submissions.drain(..clear.before_draw);
                    back_buffer.depth_clears.clear();
                    back_buffer.depth_clears.push(RuntimeGraphicsDepthClear {
                        before_draw: 0,
                        depth: clear.depth,
                    });
                }
                back_buffer
                    .draw_submissions
                    .retain(|draw| draw.depth.is_some_and(|depth| depth.write));
            }
            back_buffer.clear_color = Some(color);
            back_buffer.color_clear_before_draw = back_buffer.draw_submissions.len();
        }
        if flags & 2 != 0 {
            back_buffer.depth_clears.push(RuntimeGraphicsDepthClear {
                before_draw: back_buffer.draw_submissions.len(),
                depth,
            });
        }
        Ok(true)
    }
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
            viewport: [
                0,
                0,
                presentation.width,
                presentation.height,
                0,
                1.0f32.to_bits(),
            ],
            d3d8: None,
            id: RuntimeGraphicsDeviceId(self.next_device_id),
            target: request.target,

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
            presented_extent: None,
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

    pub(super) fn set_viewport(
        &mut self,
        object: RuntimeComObjectId,
        viewport: [u32; 6],
    ) -> Result<(), RuntimeStateError> {
        let id = self.device_by_com_object(object)?.id;
        self.devices
            .get_mut(&id)
            .expect("validated device")
            .viewport = viewport;
        Ok(())
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

    pub(super) fn available_texture_memory(
        &self,
        device_com_object_id: RuntimeComObjectId,
    ) -> Result<u32, RuntimeStateError> {
        let _ = self.device_by_com_object(device_com_object_id)?;
        let mut used = 0_u64;
        for texture in self.textures.values() {
            if let Some(backing) = texture.backing {
                used = used.saturating_add(backing.range.byte_length());
            } else if let Ok((_, byte_length)) = texture_byte_layout(texture.descriptor) {
                used = used.saturating_add(u64::from(byte_length));
            }
        }
        for vb in self.vertex_buffers.values() {
            used = used.saturating_add(vb.backing.range.byte_length());
        }
        let remaining = RUNTIME_GRAPHICS_RESOURCE_ARENA_SIZE.saturating_sub(used);
        let rounded = (remaining & !0x000f_ffff).min(u64::from(u32::MAX)) as u32;
        Ok(if remaining > 0 && rounded == 0 {
            1024 * 1024
        } else {
            rounded
        })
    }

    pub(super) fn submit_draw(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        start_vertex: u32,
        primitive_count: u32,
        memory: &crate::memory::GuestMemory,
    ) -> Result<bool, RuntimeStateError> {
        if primitive_count != 2 {
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
        if device.d3d8.is_none() && start_vertex != 0 {
            return Ok(false);
        }
        let Some(resolved) = self.resolve_draw_state(device, start_vertex, primitive_count)? else {
            return Ok(false);
        };
        let bytes = memory
            .read_owned(
                resolved.vertex_range.start(),
                resolved.vertex_range.byte_length() as usize,
                resolved.vertex_memory_owner,
            )
            .map_err(RuntimeStateError::from_memory)?;
        self.submit_resolved_quad_bytes(device_id, resolved.texture_id, &bytes, false, memory)
    }

    pub(super) fn submit_draw_up(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        primitive_count: u32,
        vertices: GuestAddress,
        stride: u32,
        memory: &crate::memory::GuestMemory,
    ) -> Result<bool, RuntimeStateError> {
        let device = self.device_by_com_object(device_com_object_id)?;
        let device_id = device.id;
        self.validate_device_association(device_com_object_id, device_id)?;
        let Some(fvf) = device.flexible_vertex_format else {
            return Ok(false);
        };
        if device.d3d8.is_none()
            && !matches!(
                fvf,
                RuntimeGraphicsFlexibleVertexFormat::XyzRhwTexture1
                    | RuntimeGraphicsFlexibleVertexFormat::XyzRhwDiffuseTexture1
            )
        {
            return Ok(false);
        }
        let expected_stride = fvf.stride();
        if primitive_count != 2 || stride != expected_stride || vertices.0 == 0 {
            return Ok(false);
        }
        let texture_id = if device.d3d8.is_some() && !uses_stage_zero_texture(device) {
            if !supported_draw_device_state(device) || !device.scene_active {
                return Ok(false);
            }
            None
        } else {
            let Some(texture) = self.resolve_draw_texture(device)? else {
                return Ok(false);
            };
            Some(texture.id)
        };
        // UP consumes caller-owned bytes before returning and never retains a guest pointer.
        let bytes = memory
            .read(vertices, stride as usize * 4)
            .map_err(RuntimeStateError::from_memory)?;
        self.submit_resolved_quad_bytes(device_id, texture_id, &bytes, true, memory)
    }

    fn submit_resolved_quad_bytes(
        &mut self,
        device_id: RuntimeGraphicsDeviceId,
        texture_id: Option<RuntimeGraphicsTextureId>,
        bytes: &[u8],
        clear_stream: bool,
        memory: &crate::memory::GuestMemory,
    ) -> Result<bool, RuntimeStateError> {
        let device = self.device(device_id)?;
        let texture = texture_id.map(|id| self.texture(id)).transpose()?;
        let quads = if device.d3d8.is_some() {
            let Some(quads) = lower_d3d8_quad_vertices(device, bytes) else {
                return Ok(false);
            };
            quads
        } else {
            let stride = device
                .flexible_vertex_format
                .expect("validated FVF")
                .stride() as usize;
            vec![canonical_draw_vertices(device, bytes, stride)]
        };
        if quads.is_empty() {
            if clear_stream {
                self.devices
                    .get_mut(&device_id)
                    .expect("validated draw device")
                    .stream_sources
                    .stream_zero = None;
            }
            return Ok(true);
        }
        let submissions = self.capture_draws(device, texture, quads, memory)?;
        for (index, submission) in submissions.into_iter().enumerate() {
            self.publish_draw(device_id, submission, clear_stream && index == 0);
        }
        if clear_stream {
            self.devices
                .get_mut(&device_id)
                .expect("validated draw device")
                .stream_sources
                .stream_zero = None;
        }
        Ok(true)
    }

    fn publish_draw(
        &mut self,
        device_id: RuntimeGraphicsDeviceId,
        submission: RuntimeGraphicsDrawSubmission,
        clear_stream: bool,
    ) {
        let device = self
            .devices
            .get_mut(&device_id)
            .expect("validated draw device");
        if device.presentation.present_mode == RuntimeGraphicsPresentMode::Copy
            && is_unconditional_opaque_submission(&device.back_buffer, &submission)
        {
            if let Some(opaque_rect) =
                axis_aligned_quad_rect(&submission.vertex_data, submission.clip_rect)
            {
                prune_occluded_draws(&mut device.back_buffer, opaque_rect);
            } else if device.d3d8.is_none() {
                device.back_buffer.draw_submissions.retain(|draw| {
                    draw.clip_rect != submission.clip_rect
                        || !same_draw_coverage(&submission.vertex_data, &draw.vertex_data)
                });
            }
        }
        device.back_buffer.draw_submissions.push(submission);
        if clear_stream {
            device.stream_sources.stream_zero = None;
        }
    }

    fn capture_draws(
        &self,
        device: &RuntimeGraphicsDevice,
        texture: Option<&RuntimeGraphicsTexture>,
        quads: Vec<Vec<u8>>,
        memory: &crate::memory::GuestMemory,
    ) -> Result<Vec<RuntimeGraphicsDrawSubmission>, RuntimeStateError> {
        let retained_pixels = texture.and_then(|t| t.content.retained_pixels.as_ref());
        let texture_memory = if retained_pixels.is_some() {
            None
        } else {
            texture
                .and_then(|t| t.content.pixel_source)
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
                .transpose()?
        };
        let source_key = if device.d3d8.is_some() {
            texture
                .and_then(|t| t.content.pixel_source)
                .map(|p| (p.source_texture_id, p.source_content_generation))
        } else {
            None
        };
        let reused_texture_data = source_key.and_then(|key| {
            device
                .back_buffer
                .draw_submissions
                .iter()
                .find(|draw| draw.texture_source_key == Some(key) && !draw.texture_data.is_empty())
                .map(|draw| Arc::clone(&draw.texture_data))
        });
        let first_quad = &quads[0];
        let mut seen_ptrs = BTreeSet::new();
        let captured_bytes = if device.d3d8.is_some() {
            let mut sum = 0_u64;
            for draw in &device.back_buffer.draw_submissions {
                if !draw.texture_data.is_empty()
                    && seen_ptrs.insert(Arc::as_ptr(&draw.texture_data) as usize)
                {
                    sum = sum.saturating_add(draw.texture_data.len() as u64);
                }
            }
            sum
        } else {
            let mut sum = 0_u64;
            for draw in device
                .back_buffer
                .draw_submissions
                .iter()
                .filter(|draw| !replaces_opaque_draw(device, first_quad, draw))
            {
                if !draw.texture_data.is_empty()
                    && seen_ptrs.insert(Arc::as_ptr(&draw.texture_data) as usize)
                {
                    sum = sum.saturating_add(draw.texture_data.len() as u64);
                }
            }
            sum
        };
        let new_bytes = if reused_texture_data.is_some() {
            0
        } else if let Some(retained) = retained_pixels {
            if seen_ptrs.contains(&(Arc::as_ptr(&retained.data) as usize)) {
                0
            } else {
                retained.data.len() as u64
            }
        } else {
            texture_memory
                .map(|(range, _)| range.byte_length())
                .unwrap_or(0)
        };
        if new_bytes > (16u64 * 1024 * 1024).saturating_sub(captured_bytes) {
            return Err(RuntimeStateError::new(
                "captured texture bytes exceed the bounded frame budget",
            ));
        }
        let expected_draw_submission_count = device
            .back_buffer
            .draw_submissions
            .iter()
            .filter(|draw| !replaces_opaque_draw(device, first_quad, draw))
            .count()
            .saturating_add(quads.len());
        if expected_draw_submission_count > RUNTIME_GRAPHICS_DRAW_SUBMISSION_LIMIT {
            return Err(RuntimeStateError::new(
                "logical back-buffer draw-submission capacity is exhausted",
            ));
        }
        let (texture_data, normalized_pitch) = if let Some(retained) = retained_pixels {
            (Arc::clone(&retained.data), Some(retained.row_pitch))
        } else if let Some(reused) = reused_texture_data {
            if let Some((range, owner)) = texture_memory {
                memory
                    .read_owned(range.start(), 1, owner)
                    .map_err(RuntimeStateError::from_memory)?;
            }
            let pitch = texture.and_then(|tex| {
                tex.content.pixel_source.map(|pixels| {
                    if tex.descriptor.color_format.bytes_per_pixel() == 2 {
                        tex.descriptor.width * 4
                    } else {
                        pixels.row_pitch
                    }
                })
            });
            (reused, pitch)
        } else {
            let raw = texture_memory
                .map(|(range, owner)| {
                    memory
                        .read_owned(range.start(), range.byte_length() as usize, owner)
                        .map_err(RuntimeStateError::from_memory)
                })
                .transpose()?
                .unwrap_or_default();
            if let Some(tex) = texture {
                if let Some(pixels) = tex.content.pixel_source {
                    let (expanded, pitch) = normalize_texture_snapshot_to_bgra8(
                        raw,
                        tex.descriptor.width,
                        tex.descriptor.height,
                        pixels.row_pitch,
                        tex.descriptor.color_format,
                    );
                    (Arc::new(expanded), Some(pitch))
                } else {
                    (Arc::new(raw), None)
                }
            } else {
                (Arc::new(raw), None)
            }
        };
        let (texture_alpha_format, texture_width, texture_height, texture_fills, texture_pitch) =
            if let Some(texture) = texture {
                (
                    texture.descriptor.color_format.has_alpha(),
                    texture.descriptor.width,
                    texture.descriptor.height,
                    texture.content.fills.clone(),
                    normalized_pitch,
                )
            } else {
                (
                    false,
                    1,
                    1,
                    vec![RuntimeGraphicsSurfaceFill {
                        rect: RuntimeRect {
                            left: 0,
                            top: 0,
                            right: 1,
                            bottom: 1,
                        },
                        color_ref: 0x00ff_ffff,
                    }],
                    None,
                )
            };
        let stage = &device.texture_stage_states.stage_zero;
        let uses_tex_alpha = match stage.alpha_operation {
            Some(RuntimeGraphicsTextureOperation::SelectArgumentOne) => {
                stage.alpha_argument_one == Some(RuntimeGraphicsTextureArgument::Texture)
            }
            Some(RuntimeGraphicsTextureOperation::Modulate) => {
                stage.alpha_argument_one == Some(RuntimeGraphicsTextureArgument::Texture)
                    || stage.alpha_argument_two == Some(RuntimeGraphicsTextureArgument::Texture)
            }
            _ => false,
        };
        let uses_diff_alpha = match stage.alpha_operation {
            Some(RuntimeGraphicsTextureOperation::SelectArgumentOne) => matches!(
                stage.alpha_argument_one,
                Some(
                    RuntimeGraphicsTextureArgument::Diffuse
                        | RuntimeGraphicsTextureArgument::TFactor
                )
            ),
            Some(RuntimeGraphicsTextureOperation::Modulate) => {
                matches!(
                    stage.alpha_argument_one,
                    Some(
                        RuntimeGraphicsTextureArgument::Diffuse
                            | RuntimeGraphicsTextureArgument::TFactor
                    )
                ) || matches!(
                    stage.alpha_argument_two,
                    Some(
                        RuntimeGraphicsTextureArgument::Diffuse
                            | RuntimeGraphicsTextureArgument::TFactor
                    )
                )
            }
            _ => false,
        };
        let is_xyz = device
            .flexible_vertex_format
            .is_some_and(RuntimeGraphicsFlexibleVertexFormat::is_untransformed_xyz);
        let fragment = device.d3d8.as_ref().map(|state| {
            let blend = device.render_states.alpha_blend_enabled == Some(true);
            let fog = (is_xyz
                && state.render_values.get(&28) == Some(&1)
                && state.render_values.get(&35) == Some(&3))
            .then(|| RuntimeGraphicsFogState {
                color_argb: state.render_values.get(&34).copied().unwrap_or(0),
                start: state.render_values.get(&36).copied().unwrap_or(0),
                end: state
                    .render_values
                    .get(&37)
                    .copied()
                    .unwrap_or(1.0_f32.to_bits()),
            });
            RuntimeGraphicsFragmentState {
                texture_alpha: uses_tex_alpha,
                diffuse_alpha: uses_diff_alpha,
                blend,
                additive_blend: blend
                    && device.render_states.destination_blend_factor
                        == Some(RuntimeGraphicsBlendFactor::One),
                color_add: stage.color_operation == Some(RuntimeGraphicsTextureOperation::Add),
                alpha_test: (state.render_values.get(&15) == Some(&1)).then(|| {
                    (
                        state.render_values.get(&25).copied().unwrap_or(8),
                        state.render_values.get(&24).copied().unwrap_or(0),
                    )
                }),
                fog,
            }
        });
        let depth = if device.render_states.depth_test_enabled == Some(true) {
            Some(RuntimeGraphicsDepthDrawState {
                write: device
                    .d3d8
                    .as_ref()
                    .and_then(|s| s.render_values.get(&14))
                    .copied()
                    .unwrap_or(1)
                    != 0,
                compare: device
                    .d3d8
                    .as_ref()
                    .and_then(|s| s.render_values.get(&23))
                    .copied()
                    .unwrap_or(4),
            })
        } else {
            None
        };
        let clip_rect = Some(RuntimeRect {
            left: device.viewport[0] as i32,
            top: device.viewport[1] as i32,
            right: (device.viewport[0] + device.viewport[2]) as i32,
            bottom: (device.viewport[1] + device.viewport[3]) as i32,
        });
        let min_filter = device
            .sampler_states
            .sampler_zero
            .min_filter
            .expect("validated filter");
        let mag_filter = device
            .sampler_states
            .sampler_zero
            .mag_filter
            .expect("validated filter");
        let address_u = device.sampler_states.sampler_zero.address_u;
        let address_v = device.sampler_states.sampler_zero.address_v;

        Ok(quads
            .into_iter()
            .map(|vertex_data| RuntimeGraphicsDrawSubmission {
                clip_rect,
                texture_alpha: texture_alpha_format,
                fragment: fragment.clone(),
                depth,
                vertex_data,
                min_filter,
                mag_filter,
                address_u,
                address_v,
                texture_data: Arc::clone(&texture_data),
                texture_source_key: source_key,
                texture_width,
                texture_height,
                texture_fills: texture_fills.clone(),
                texture_pitch,
            })
            .collect())
    }

    fn resolve_draw_state(
        &self,
        device: &RuntimeGraphicsDevice,
        start_vertex: u32,
        primitive_count: u32,
    ) -> Result<Option<RuntimeGraphicsDrawResolvedState>, RuntimeStateError> {
        let texture_id = if device.d3d8.is_some() && !uses_stage_zero_texture(device) {
            if !supported_draw_device_state(device) || !device.scene_active {
                return Ok(None);
            }
            None
        } else {
            let Some(texture) = self.resolve_draw_texture(device)? else {
                return Ok(None);
            };
            Some(texture.id)
        };
        let Some(fvf) = device.flexible_vertex_format else {
            return Ok(None);
        };
        let Some(stream_source) = device.stream_sources.stream_zero else {
            return Ok(None);
        };
        let vertex_buffer = self.vertex_buffer(stream_source.vertex_buffer_id)?;
        self.validate_vertex_buffer_association(vertex_buffer.com_object_id, vertex_buffer.id)?;
        if device.d3d8.is_none() {
            if fvf != RuntimeGraphicsFlexibleVertexFormat::XyzRhwDiffuseTexture1
                || vertex_buffer.device_id != device.id
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
        } else if vertex_buffer.device_id != device.id
            || vertex_buffer.locked
            || !vertex_buffer.initialized
            || !matches!(
                vertex_buffer.descriptor.usage,
                RuntimeGraphicsVertexBufferUsage::None
                    | RuntimeGraphicsVertexBufferUsage::WriteOnly
            )
            || vertex_buffer.descriptor.pool != RuntimeGraphicsVertexBufferPool::Managed
            || stream_source.stride != fvf.stride()
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
        Ok(Some(RuntimeGraphicsDrawResolvedState {
            vertex_memory_owner: vertex_buffer.backing.memory_owner,
            vertex_range,
            texture_id,
        }))
    }

    fn resolve_draw_texture(
        &self,
        device: &RuntimeGraphicsDevice,
    ) -> Result<Option<&RuntimeGraphicsTexture>, RuntimeStateError> {
        if !supported_draw_device_state(device) || !device.scene_active {
            return Ok(None);
        }
        let Some(texture_id) = device.texture_bindings.stage_zero else {
            return Ok(None);
        };
        let texture = self.texture(texture_id)?;
        self.validate_texture_association(texture.com_object_id, texture.id)?;
        if texture.device_id != device.id
            || texture.descriptor.levels != 1
            || !matches!(
                texture.descriptor.pool,
                RuntimeGraphicsTexturePool::Default | RuntimeGraphicsTexturePool::Managed
            )
            || texture.active_lock.is_some()
            || texture.content.content_generation == 0
        {
            return Ok(None);
        }
        if texture.content.retained_pixels.is_none()
            && let Some(pixel_source) = texture.content.pixel_source
        {
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
        Ok(Some(texture))
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
        device.back_buffer.preserve_previous = false;
        device.back_buffer.clear_color = Some(color_argb);
        device.back_buffer.color_clear_before_draw = 0;
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
        if device.presentation.present_mode == RuntimeGraphicsPresentMode::Copy
            && device.back_buffer.draw_submissions.len() > 1
        {
            prune_back_buffer_on_end_scene(device);
        }

        Ok(true)
    }

    pub(super) fn present(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        extent: Option<(u32, u32)>,
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
        let back_buffer_extent = (device.presentation.width, device.presentation.height);
        let extent = extent.unwrap_or(back_buffer_extent);
        if extent.0 == 0
            || extent.1 == 0
            || extent.0 > device.presentation.width
            || extent.1 > device.presentation.height
        {
            return Ok(false);
        }
        if device.scene_active {
            return Ok(false);
        }
        if device.back_buffer.clear_color.is_none()
            && device.back_buffer.draw_submissions.is_empty()
            && device.back_buffer.depth_clears.is_empty()
            && !device.back_buffer.preserve_previous
        {
            return Ok(false);
        }
        let device = self
            .devices
            .get_mut(&device_id)
            .expect("validated device identity");
        let mut presented = match device.presentation.present_mode {
            RuntimeGraphicsPresentMode::Discard => std::mem::take(&mut device.back_buffer),
            RuntimeGraphicsPresentMode::Copy if device.d3d8.is_some() => {
                let presented = std::mem::take(&mut device.back_buffer);
                device.back_buffer.preserve_previous = true;
                presented
            }
            RuntimeGraphicsPresentMode::Copy => device.back_buffer.clone(),
        };
        if extent != back_buffer_extent {
            // Viewports are captured in back-buffer coordinates. An unscaled
            // upper-left Present crop constrains the copy sent to the renderer.
            crop_presented_frame(&mut presented, extent);
        }
        device.presented_frame = Some(presented);
        device.presented_extent = Some(extent);

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
                    | RUNTIME_GRAPHICS_TEXTURE_LOCK_NO_DIRTY_UPDATE
                    | RUNTIME_GRAPHICS_TEXTURE_LOCK_READONLY)
                != 0
            || texture.descriptor.levels != 1
            || !matches!(
                (texture.descriptor.usage, texture.descriptor.pool),
                (
                    RuntimeGraphicsTextureUsage::None,
                    RuntimeGraphicsTexturePool::SystemMemory | RuntimeGraphicsTexturePool::Managed
                ) | (
                    RuntimeGraphicsTextureUsage::Dynamic,
                    RuntimeGraphicsTexturePool::Default
                )
            )
        {
            return Ok(None);
        }
        if flags & RUNTIME_GRAPHICS_TEXTURE_LOCK_READONLY != 0
            && texture.descriptor.usage == RuntimeGraphicsTextureUsage::Dynamic
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
        let bytes_per_pixel = u64::from(texture.descriptor.color_format.bytes_per_pixel());
        let byte_offset = u64::try_from(rect.top)
            .ok()
            .and_then(|top| top.checked_mul(u64::from(row_pitch)))
            .and_then(|offset| {
                u64::try_from(rect.left)
                    .ok()
                    .and_then(|left| left.checked_mul(bytes_per_pixel))
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
        if lock.flags
            & (RUNTIME_GRAPHICS_TEXTURE_LOCK_NO_DIRTY_UPDATE
                | RUNTIME_GRAPHICS_TEXTURE_LOCK_READONLY)
            == 0
        {
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
        if texture
            .active_lock
            .is_some_and(|lock| lock.flags & RUNTIME_GRAPHICS_TEXTURE_LOCK_READONLY != 0)
        {
            self.textures
                .get_mut(&id)
                .expect("live texture")
                .active_lock = None;
            return Ok(true);
        }
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
        texture.content.retained_pixels = None;
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

    #[cfg(test)]
    pub(super) fn update_texture(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        source_texture_com_object_id: RuntimeComObjectId,
        destination_texture_com_object_id: RuntimeComObjectId,
    ) -> Result<bool, RuntimeStateError> {
        self.update_texture_with_memory(
            device_com_object_id,
            source_texture_com_object_id,
            destination_texture_com_object_id,
            None,
        )
    }

    pub(super) fn update_texture_with_memory(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        source_texture_com_object_id: RuntimeComObjectId,
        destination_texture_com_object_id: RuntimeComObjectId,
        memory: Option<&crate::memory::GuestMemory>,
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
        let retained_pixels = if let (Some(memory), Some(pixels)) = (memory, source_pixel_source) {
            let backing = source.backing.ok_or_else(|| {
                RuntimeStateError::new(
                    "logical texture pixel source has no live guest-memory backing",
                )
            })?;
            let range = GuestRange::new(
                backing.range.start(),
                u64::from(backing.row_pitch) * u64::from(source.descriptor.height),
            )
            .map_err(RuntimeStateError::from_memory)?;
            let raw = memory
                .read_owned(
                    range.start(),
                    range.byte_length() as usize,
                    backing.memory_owner,
                )
                .map_err(RuntimeStateError::from_memory)?;
            let (expanded, row_pitch) = normalize_texture_snapshot_to_bgra8(
                raw,
                destination.descriptor.width,
                destination.descriptor.height,
                pixels.row_pitch,
                destination.descriptor.color_format,
            );
            Some(RuntimeGraphicsRetainedTexturePixels {
                data: Arc::new(expanded),
                row_pitch,
            })
        } else {
            None
        };
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
        destination.content.retained_pixels = retained_pixels;
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
        if offset != 0
            || (byte_length != 0 && byte_length != vertex_buffer.descriptor.byte_length)
            || flags != 0
            || vertex_buffer.locked
        {
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
        if texture.active_lock.is_some()
            || texture.content.pixel_source.is_some()
            || texture.content.retained_pixels.is_some()
        {
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
        let retired_textures = plan
            .texture_associations
            .iter()
            .map(|(_, texture_id)| *texture_id)
            .collect::<BTreeSet<_>>();
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
        if !retired_textures.is_empty() {
            for texture in self.textures.values_mut() {
                if texture
                    .content
                    .pixel_source
                    .is_some_and(|source| retired_textures.contains(&source.source_texture_id))
                {
                    texture.content.pixel_source = None;
                }
                if texture
                    .content
                    .last_update
                    .is_some_and(|update| retired_textures.contains(&update.source_texture_id))
                {
                    texture.content.last_update = None;
                }
            }
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
            if texture.content.retained_pixels.is_none()
                && texture
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
        || request.back_buffer_width == 0
        || request.back_buffer_height == 0
        || request.back_buffer_width > 4096
        || request.back_buffer_height > 4096
        || request.back_buffer_count == 0
        || request.sample_count == 0
        || !request.windowed)
}

fn uses_stage_zero_texture(device: &RuntimeGraphicsDevice) -> bool {
    let stage = &device.texture_stage_states.stage_zero;
    let color_uses = match stage.color_operation {
        Some(RuntimeGraphicsTextureOperation::SelectArgumentOne) => {
            stage.color_argument_one == Some(RuntimeGraphicsTextureArgument::Texture)
        }
        Some(RuntimeGraphicsTextureOperation::Modulate | RuntimeGraphicsTextureOperation::Add) => {
            stage.color_argument_one == Some(RuntimeGraphicsTextureArgument::Texture)
                || stage.color_argument_two == Some(RuntimeGraphicsTextureArgument::Texture)
        }
        _ => false,
    };
    let alpha_uses = match stage.alpha_operation {
        Some(RuntimeGraphicsTextureOperation::SelectArgumentOne) => {
            stage.alpha_argument_one == Some(RuntimeGraphicsTextureArgument::Texture)
        }
        Some(RuntimeGraphicsTextureOperation::Modulate) => {
            stage.alpha_argument_one == Some(RuntimeGraphicsTextureArgument::Texture)
                || stage.alpha_argument_two == Some(RuntimeGraphicsTextureArgument::Texture)
        }
        _ => false,
    };
    color_uses || alpha_uses
}

fn supported_draw_device_state(device: &RuntimeGraphicsDevice) -> bool {
    let render = &device.render_states;
    let stage = &device.texture_stage_states.stage_zero;
    let sampler = &device.sampler_states.sampler_zero;
    if let Some(d3d8) = device.d3d8.as_ref() {
        let blend = render.alpha_blend_enabled == Some(false)
            || (render.alpha_blend_enabled == Some(true)
                && render.source_blend_factor == Some(RuntimeGraphicsBlendFactor::SourceAlpha)
                && matches!(
                    render.destination_blend_factor,
                    Some(
                        RuntimeGraphicsBlendFactor::InverseSourceAlpha
                            | RuntimeGraphicsBlendFactor::One
                    )
                )
                && render.blend_operation == Some(RuntimeGraphicsBlendOperation::Add));
        let color = (stage.color_operation
            == Some(RuntimeGraphicsTextureOperation::SelectArgumentOne)
            && matches!(
                stage.color_argument_one,
                Some(
                    RuntimeGraphicsTextureArgument::Texture
                        | RuntimeGraphicsTextureArgument::Diffuse
                        | RuntimeGraphicsTextureArgument::TFactor
                )
            ))
            || (matches!(
                stage.color_operation,
                Some(
                    RuntimeGraphicsTextureOperation::Modulate
                        | RuntimeGraphicsTextureOperation::Add
                )
            ) && stage.color_argument_one == Some(RuntimeGraphicsTextureArgument::Texture)
                && matches!(
                    stage.color_argument_two,
                    Some(
                        RuntimeGraphicsTextureArgument::Diffuse
                            | RuntimeGraphicsTextureArgument::TFactor
                    )
                ));
        let alpha = (stage.alpha_operation == Some(RuntimeGraphicsTextureOperation::Disable)
            && render.alpha_blend_enabled == Some(false))
            || (stage.alpha_operation == Some(RuntimeGraphicsTextureOperation::SelectArgumentOne)
                && matches!(
                    stage.alpha_argument_one,
                    Some(
                        RuntimeGraphicsTextureArgument::Texture
                            | RuntimeGraphicsTextureArgument::Diffuse
                            | RuntimeGraphicsTextureArgument::TFactor
                    )
                ))
            || (stage.alpha_operation == Some(RuntimeGraphicsTextureOperation::Modulate)
                && stage.alpha_argument_one == Some(RuntimeGraphicsTextureArgument::Texture)
                && matches!(
                    stage.alpha_argument_two,
                    Some(
                        RuntimeGraphicsTextureArgument::Diffuse
                            | RuntimeGraphicsTextureArgument::TFactor
                    )
                ));
        let is_xyz = device
            .flexible_vertex_format
            .is_some_and(RuntimeGraphicsFlexibleVertexFormat::is_untransformed_xyz);
        let fog_ok = if is_xyz && d3d8.render_values.get(&28) == Some(&1) {
            matches!(d3d8.render_values.get(&35).copied().unwrap_or(0), 0 | 3)
        } else {
            true
        };
        let tex_transform_ok =
            matches!(d3d8.stage_values.get(&(0, 24)).copied().unwrap_or(0), 0 | 2);
        return render.cull_mode == Some(RuntimeGraphicsCullMode::None)
            && (render.depth_test_enabled == Some(false)
                || (render.depth_test_enabled == Some(true) && d3d8.parameters[8] != 0))
            && render.lighting_enabled == Some(false)
            && blend
            && color
            && alpha
            && fog_ok
            && tex_transform_ok
            && device.flexible_vertex_format.is_some()
            && sampler.min_filter.is_some()
            && sampler.mag_filter.is_some();
    }
    let blend = render.alpha_blend_enabled == Some(false)
        || (render.alpha_blend_enabled == Some(true)
            && render.source_blend_factor == Some(RuntimeGraphicsBlendFactor::SourceAlpha)
            && render.destination_blend_factor
                == Some(RuntimeGraphicsBlendFactor::InverseSourceAlpha)
            && render.blend_operation == Some(RuntimeGraphicsBlendOperation::Add));
    let color = stage.color_argument_one == Some(RuntimeGraphicsTextureArgument::Texture)
        && (stage.color_operation == Some(RuntimeGraphicsTextureOperation::SelectArgumentOne)
            || (stage.color_operation == Some(RuntimeGraphicsTextureOperation::Modulate)
                && stage.color_argument_two == Some(RuntimeGraphicsTextureArgument::Diffuse)));
    let alpha = (stage.alpha_operation == Some(RuntimeGraphicsTextureOperation::Disable)
        && render.alpha_blend_enabled == Some(false))
        || (stage.alpha_operation == Some(RuntimeGraphicsTextureOperation::SelectArgumentOne)
            && stage.alpha_argument_one == Some(RuntimeGraphicsTextureArgument::Texture));
    render.cull_mode == Some(RuntimeGraphicsCullMode::None)
        && render.depth_test_enabled == Some(false)
        && render.lighting_enabled == Some(false)
        && blend
        && color
        && alpha
        && device.flexible_vertex_format.is_some()
        && sampler.min_filter.is_some()
        && sampler.mag_filter.is_some()
}

#[derive(Clone, Copy)]
struct ClipVertex {
    cx: f32,
    cy: f32,
    cz: f32,
    cw: f32,
    u: f32,
    v: f32,
    color: [f32; 4], // B, G, R, A in 0..=255
}

fn identity_matrix() -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        0.0, 0.0, 0.0, 1.0,
    ]
}

fn decode_matrix(words: Option<&[u32; 16]>) -> Option<[f32; 16]> {
    let Some(words) = words else {
        return Some(identity_matrix());
    };
    let mut out = [0.0_f32; 16];
    for (i, w) in words.iter().enumerate() {
        let f = f32::from_bits(*w);
        if !f.is_finite() {
            return None;
        }
        out[i] = f;
    }
    Some(out)
}

fn mul_vec4_mat4(v: [f32; 4], m: &[f32; 16]) -> [f32; 4] {
    std::array::from_fn(|col| {
        v[0] * m[col] + v[1] * m[4 + col] + v[2] * m[8 + col] + v[3] * m[12 + col]
    })
}

fn clip_polygon<F: Fn(&ClipVertex) -> f32>(poly: &[ClipVertex], dist: F) -> Vec<ClipVertex> {
    if poly.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(poly.len() + 1);
    for i in 0..poly.len() {
        let cur = poly[i];
        let nxt = poly[(i + 1) % poly.len()];
        let d_cur = dist(&cur);
        let d_nxt = dist(&nxt);
        let cur_in = d_cur >= 0.0;
        let nxt_in = d_nxt >= 0.0;
        if cur_in {
            out.push(cur);
        }
        if cur_in != nxt_in {
            let denom = d_cur - d_nxt;
            let t = if denom.abs() > 1e-12 {
                (d_cur / denom).clamp(0.0, 1.0)
            } else {
                0.5
            };
            let lerp = |a: f32, b: f32| a + (b - a) * t;
            out.push(ClipVertex {
                cx: lerp(cur.cx, nxt.cx),
                cy: lerp(cur.cy, nxt.cy),
                cz: lerp(cur.cz, nxt.cz),
                cw: lerp(cur.cw, nxt.cw),
                u: lerp(cur.u, nxt.u),
                v: lerp(cur.v, nxt.v),
                color: std::array::from_fn(|k| lerp(cur.color[k], nxt.color[k])),
            });
        }
    }
    out
}

fn resolve_d3d8_vertex_bgra(device: &RuntimeGraphicsDevice, v_bgra: [u8; 4]) -> [u8; 4] {
    let stage = &device.texture_stage_states.stage_zero;
    let tf_bgra = device
        .d3d8
        .as_ref()
        .and_then(|s| s.render_values.get(&60))
        .copied()
        .unwrap_or(0xffff_ffff)
        .to_le_bytes();
    let rgb_src = match stage.color_operation {
        Some(RuntimeGraphicsTextureOperation::SelectArgumentOne) => {
            match stage.color_argument_one {
                Some(RuntimeGraphicsTextureArgument::Diffuse) => v_bgra,
                Some(RuntimeGraphicsTextureArgument::TFactor) => tf_bgra,
                _ => [255; 4],
            }
        }
        Some(RuntimeGraphicsTextureOperation::Modulate | RuntimeGraphicsTextureOperation::Add) => {
            match stage.color_argument_two {
                Some(RuntimeGraphicsTextureArgument::TFactor) => tf_bgra,
                Some(RuntimeGraphicsTextureArgument::Diffuse) => v_bgra,
                _ => [255; 4],
            }
        }
        _ => [255; 4],
    };
    let alpha_src = match stage.alpha_operation {
        Some(RuntimeGraphicsTextureOperation::SelectArgumentOne) => {
            match stage.alpha_argument_one {
                Some(RuntimeGraphicsTextureArgument::Diffuse) => v_bgra[3],
                Some(RuntimeGraphicsTextureArgument::TFactor) => tf_bgra[3],
                _ => 255,
            }
        }
        Some(RuntimeGraphicsTextureOperation::Modulate) => match stage.alpha_argument_two {
            Some(RuntimeGraphicsTextureArgument::TFactor) => tf_bgra[3],
            Some(RuntimeGraphicsTextureArgument::Diffuse) => v_bgra[3],
            _ => match stage.alpha_argument_one {
                Some(RuntimeGraphicsTextureArgument::TFactor) => tf_bgra[3],
                Some(RuntimeGraphicsTextureArgument::Diffuse) => v_bgra[3],
                _ => 255,
            },
        },
        _ => 255,
    };
    [rgb_src[0], rgb_src[1], rgb_src[2], alpha_src]
}

fn valid_screen_vertex(
    [x, y, z, rhw]: [f32; 4],
    [u, v]: [f32; 2],
    width: u32,
    height: u32,
) -> bool {
    if !x.is_finite()
        || !y.is_finite()
        || !z.is_finite()
        || !rhw.is_finite()
        || !u.is_finite()
        || !v.is_finite()
        || !(0.0..=1.0).contains(&z)
        || rhw <= 0.0
    {
        return false;
    }
    let w = 1.0 / rhw;
    w.is_finite()
        && w <= f32::MAX
        && (((x + 0.5) / (width as f32) * 2.0 - 1.0) * w).abs() <= f32::MAX
        && (((y + 0.5) / (height as f32) * 2.0 - 1.0) * w).abs() <= f32::MAX
}

fn lower_d3d8_quad_vertices(device: &RuntimeGraphicsDevice, bytes: &[u8]) -> Option<Vec<Vec<u8>>> {
    let fvf = device.flexible_vertex_format?;
    let stride = fvf.stride() as usize;
    if bytes.len() != stride * 4 {
        return None;
    }
    let width = device.presentation.width;
    let height = device.presentation.height;
    let read_f32 = |slice: &[u8], off: usize| {
        f32::from_le_bytes(slice[off..off + 4].try_into().expect("f32 slice"))
    };

    if !fvf.is_untransformed_xyz() {
        let mut out = Vec::with_capacity(112);
        for v in bytes.chunks_exact(stride) {
            let x = read_f32(v, 0);
            let y = read_f32(v, 4);
            let z_raw = read_f32(v, 8);
            let rhw = read_f32(v, 12);
            if !z_raw.is_finite() {
                return None;
            }
            let z = z_raw.clamp(0.0, 1.0);
            let v_bgra = if fvf.has_diffuse() {
                v[16..20].try_into().expect("bgra")
            } else {
                [255; 4]
            };
            let bgra = resolve_d3d8_vertex_bgra(device, v_bgra);
            let (tu, tv) = if fvf.has_tex1() {
                (read_f32(v, stride - 8), read_f32(v, stride - 4))
            } else {
                (0.0, 0.0)
            };
            if !valid_screen_vertex([x, y, z, rhw], [tu, tv], width, height) {
                return None;
            }
            for f in [x, y, z, rhw] {
                out.extend_from_slice(&f.to_le_bytes());
            }
            out.extend_from_slice(&bgra);
            out.extend_from_slice(&tu.to_le_bytes());
            out.extend_from_slice(&tv.to_le_bytes());
        }
        return Some(vec![out]);
    }

    let state = device.d3d8.as_ref()?;
    let world = decode_matrix(state.transforms.get(&256))?;
    let view = decode_matrix(state.transforms.get(&2))?;
    let proj = decode_matrix(state.transforms.get(&3))?;
    let tex_transform = if state.stage_values.get(&(0, 24)) == Some(&2) {
        Some(decode_matrix(state.transforms.get(&16))?)
    } else {
        None
    };

    let vp_x = device.viewport[0] as f32;
    let vp_y = device.viewport[1] as f32;
    let vp_w = device.viewport[2] as f32;
    let vp_h = device.viewport[3] as f32;
    let min_z = f32::from_bits(device.viewport[4]);
    let max_z = f32::from_bits(device.viewport[5]);
    if !min_z.is_finite() || !max_z.is_finite() {
        return None;
    }

    let mut clip_verts = Vec::with_capacity(4);
    for v in bytes.chunks_exact(stride) {
        let px = read_f32(v, 0);
        let py = read_f32(v, 4);
        let pz = read_f32(v, 8);
        if !px.is_finite() || !py.is_finite() || !pz.is_finite() {
            return None;
        }
        let v_bgra = if fvf.has_diffuse() {
            v[12..16].try_into().expect("bgra")
        } else {
            [255; 4]
        };
        let bgra = resolve_d3d8_vertex_bgra(device, v_bgra);
        let mut tu = read_f32(v, stride - 8);
        let mut tv = read_f32(v, stride - 4);
        if !tu.is_finite() || !tv.is_finite() {
            return None;
        }
        if let Some(tm) = &tex_transform {
            let nu = tu * tm[0] + tv * tm[4] + tm[8];
            let nv = tu * tm[1] + tv * tm[5] + tm[9];
            if !nu.is_finite() || !nv.is_finite() {
                return None;
            }
            tu = nu;
            tv = nv;
        }
        let p_world = mul_vec4_mat4([px, py, pz, 1.0], &world);
        let p_view = mul_vec4_mat4(p_world, &view);
        let [cx, cy, cz, cw] = mul_vec4_mat4(p_view, &proj);
        if !cx.is_finite() || !cy.is_finite() || !cz.is_finite() || !cw.is_finite() {
            return None;
        }
        clip_verts.push(ClipVertex {
            cx,
            cy,
            cz,
            cw,
            u: tu,
            v: tv,
            color: bgra.map(f32::from),
        });
    }

    let project = |cv: &ClipVertex| -> Option<[u8; 28]> {
        let rhw = 1.0 / cv.cw;
        let nx = cv.cx * rhw;
        let ny = cv.cy * rhw;
        let nz = (cv.cz * rhw).clamp(0.0, 1.0);
        let sx = vp_x + (nx + 1.0) * 0.5 * vp_w;
        let sy = vp_y + (1.0 - ny) * 0.5 * vp_h;
        let sz = (min_z + nz * (max_z - min_z)).clamp(0.0, 1.0);
        if !valid_screen_vertex([sx, sy, sz, rhw], [cv.u, cv.v], width, height) {
            return None;
        }
        let mut out = [0_u8; 28];
        out[0..4].copy_from_slice(&sx.to_le_bytes());
        out[4..8].copy_from_slice(&sy.to_le_bytes());
        out[8..12].copy_from_slice(&sz.to_le_bytes());
        out[12..16].copy_from_slice(&rhw.to_le_bytes());
        for (k, c) in cv.color.iter().enumerate() {
            out[16 + k] = c.round().clamp(0.0, 255.0) as u8;
        }
        out[20..24].copy_from_slice(&cv.u.to_le_bytes());
        out[24..28].copy_from_slice(&cv.v.to_le_bytes());
        Some(out)
    };

    let all_unclipped = clip_verts
        .iter()
        .all(|cv| cv.cw >= 1e-4 && cv.cz >= -1e-5 * cv.cw && cv.cz <= cv.cw * (1.0 + 1e-5));
    if all_unclipped {
        let mut quad = Vec::with_capacity(112);
        for cv in &clip_verts {
            quad.extend_from_slice(&project(cv)?);
        }
        return Some(vec![quad]);
    }

    let mut quads = Vec::new();
    for tri in [
        [clip_verts[0], clip_verts[1], clip_verts[2]],
        [clip_verts[1], clip_verts[3], clip_verts[2]],
    ] {
        let poly = clip_polygon(&tri, |v| v.cw - 1e-4);
        let poly = clip_polygon(&poly, |v| v.cz);
        let poly = clip_polygon(&poly, |v| v.cw - v.cz);
        if poly.len() < 3 {
            continue;
        }
        let projected: Option<Vec<[u8; 28]>> = poly.iter().map(project).collect();
        let projected = projected?;
        let p0 = projected[0];
        let mut idx = 1;
        while idx + 1 < projected.len() {
            let mut quad = Vec::with_capacity(112);
            if idx + 2 < projected.len() {
                quad.extend_from_slice(&projected[idx]);
                quad.extend_from_slice(&projected[idx + 1]);
                quad.extend_from_slice(&p0);
                quad.extend_from_slice(&projected[idx + 2]);
                idx += 2;
            } else {
                quad.extend_from_slice(&p0);
                quad.extend_from_slice(&projected[idx]);
                quad.extend_from_slice(&projected[idx + 1]);
                quad.extend_from_slice(&projected[idx + 1]);
                idx += 1;
            }
            quads.push(quad);
        }
    }
    Some(quads)
}

fn canonical_draw_vertices(device: &RuntimeGraphicsDevice, bytes: &[u8], stride: usize) -> Vec<u8> {
    let texture_only = stride == 24
        || device.texture_stage_states.stage_zero.color_operation
            == Some(RuntimeGraphicsTextureOperation::SelectArgumentOne);
    let mut output = Vec::with_capacity(112);
    for vertex in bytes.chunks_exact(stride) {
        output.extend_from_slice(&vertex[..16]);
        output.extend_from_slice(if texture_only {
            &[255; 4]
        } else {
            &vertex[16..20]
        });
        if stride == 28
            && device.d3d8.is_some()
            && device.texture_stage_states.stage_zero.alpha_operation
                == Some(RuntimeGraphicsTextureOperation::Modulate)
        {
            let index = output.len() - 1;
            output[index] = vertex[19];
        }
        output.extend_from_slice(&vertex[stride - 8..stride]);
    }
    output
}

fn alpha_test_passes_value(alpha_test: Option<(u32, u32)>, value: u32) -> bool {
    match alpha_test {
        None => true,
        Some((compare, reference)) => match compare {
            1 => false,
            2 => value < reference,
            3 => value == reference,
            4 => value <= reference,
            5 => value > reference,
            6 => value != reference,
            7 => value >= reference,
            8 => true,
            _ => false,
        },
    }
}

fn vertices_all_alpha_255(vertices: &[u8]) -> bool {
    vertices.len() == 112 && vertices.chunks_exact(28).all(|chunk| chunk[19] == 255)
}

fn vertices_max_z(vertices: &[u8]) -> Option<f32> {
    if vertices.len() != 112 {
        return None;
    }
    let mut max_z = f32::NEG_INFINITY;
    for chunk in vertices.chunks_exact(28) {
        let z = f32::from_le_bytes(chunk[8..12].try_into().unwrap());
        if !z.is_finite() {
            return None;
        }
        max_z = max_z.max(z);
    }
    Some(max_z)
}

fn axis_texel_intervals(
    min_coord: f32,
    max_coord: f32,
    size: usize,
    address: RuntimeGraphicsTextureAddress,
) -> [(usize, usize); 2] {
    if size == 0 {
        return [(0, 0), (0, 0)];
    }
    let sz = size as f32;
    match address {
        RuntimeGraphicsTextureAddress::Clamp => {
            let c0 = min_coord.clamp(0.0, 1.0);
            let c1 = max_coord.clamp(0.0, 1.0);
            let i0 = ((c0 * sz - 1.0).floor() as isize).clamp(0, size as isize - 1) as usize;
            let i1 = ((c1 * sz + 1.0).ceil() as isize).clamp(0, size as isize) as usize;
            [(i0, i1.max(i0 + 1)), (0, 0)]
        }
        RuntimeGraphicsTextureAddress::Wrap => {
            if max_coord - min_coord >= 1.0 - 2.0 / sz {
                return [(0, size), (0, 0)];
            }
            let a = (min_coord * sz - 1.0).floor() as isize;
            let b = (max_coord * sz + 1.0).ceil() as isize;
            if b - a >= size as isize {
                return [(0, size), (0, 0)];
            }
            let s = a.rem_euclid(size as isize) as usize;
            let len = (b - a).max(1) as usize;
            if s + len <= size {
                [(s, s + len), (0, 0)]
            } else {
                [(s, size), (0, s + len - size)]
            }
        }
    }
}

fn sampled_texels_all_alpha_255(
    submission: &RuntimeGraphicsDrawSubmission,
    whole_texture_opaque_cache: &mut Option<&mut BTreeMap<usize, bool>>,
) -> bool {
    if !submission.texture_alpha {
        return true;
    }
    if submission.texture_data.is_empty() || !submission.texture_fills.is_empty() {
        return false;
    }
    let Some(pitch) = submission.texture_pitch.map(|p| p as usize) else {
        return false;
    };
    let (w, h) = (
        submission.texture_width as usize,
        submission.texture_height as usize,
    );
    if w == 0 || h == 0 || submission.vertex_data.len() != 112 {
        return false;
    }
    let mut min_u = f32::INFINITY;
    let mut max_u = f32::NEG_INFINITY;
    let mut min_v = f32::INFINITY;
    let mut max_v = f32::NEG_INFINITY;
    for chunk in submission.vertex_data.chunks_exact(28) {
        let u = f32::from_le_bytes(chunk[20..24].try_into().unwrap());
        let v = f32::from_le_bytes(chunk[24..28].try_into().unwrap());
        if !u.is_finite() || !v.is_finite() {
            return false;
        }
        min_u = min_u.min(u);
        max_u = max_u.max(u);
        min_v = min_v.min(v);
        max_v = max_v.max(v);
    }
    let xs = axis_texel_intervals(min_u, max_u, w, submission.address_u);
    let ys = axis_texel_intervals(min_v, max_v, h, submission.address_v);
    let x_count = (xs[0].1 - xs[0].0) + (xs[1].1 - xs[1].0);
    let y_count = (ys[0].1 - ys[0].0) + (ys[1].1 - ys[1].0);
    if x_count == w
        && y_count == h
        && let Some(cache) = whole_texture_opaque_cache.as_deref_mut()
    {
        let ptr = Arc::as_ptr(&submission.texture_data) as usize;
        if let Some(&cached) = cache.get(&ptr) {
            return cached;
        }
        if w * h <= 1_048_576 {
            let data = &*submission.texture_data;
            let mut all_opaque = true;
            'outer: for y in 0..h {
                let row = y * pitch;
                for x in 0..w {
                    if data[row + x * 4 + 3] != 255 {
                        all_opaque = false;
                        break 'outer;
                    }
                }
            }
            cache.insert(ptr, all_opaque);
            return all_opaque;
        }
        return false;
    }
    if x_count > 0 && y_count > 0 && x_count * y_count <= 262_144 {
        let data = &*submission.texture_data;
        for &(y0, y1) in &ys {
            for y in y0..y1 {
                let row = y * pitch;
                for &(x0, x1) in &xs {
                    for x in x0..x1 {
                        if data[row + x * 4 + 3] != 255 {
                            return false;
                        }
                    }
                }
            }
        }
        return true;
    }
    false
}

fn is_pixel_opaque_submission(
    submission: &RuntimeGraphicsDrawSubmission,
    mut whole_texture_opaque_cache: Option<&mut BTreeMap<usize, bool>>,
) -> bool {
    let Some(f) = &submission.fragment else {
        return true;
    };
    if f.additive_blend && f.blend {
        return false;
    }
    if !f.blend {
        return f.alpha_test.is_none_or(|(cmp, r)| {
            cmp == 8
                || ((!(f.texture_alpha && submission.texture_alpha)
                    || sampled_texels_all_alpha_255(submission, &mut whole_texture_opaque_cache))
                    && (!f.diffuse_alpha || vertices_all_alpha_255(&submission.vertex_data))
                    && alpha_test_passes_value(Some((cmp, r)), 255))
        });
    }
    if !alpha_test_passes_value(f.alpha_test, 255) {
        return false;
    }
    if f.diffuse_alpha && !vertices_all_alpha_255(&submission.vertex_data) {
        return false;
    }
    if f.texture_alpha
        && submission.texture_alpha
        && !sampled_texels_all_alpha_255(submission, &mut whole_texture_opaque_cache)
    {
        return false;
    }
    true
}

fn is_unconditional_opaque_submission(
    back_buffer: &RuntimeGraphicsBackBuffer,
    submission: &RuntimeGraphicsDrawSubmission,
) -> bool {
    if !is_pixel_opaque_submission(submission, None) {
        return false;
    }
    match submission.depth {
        None => true,
        Some(depth) if depth.compare == 8 => true,
        Some(depth) if depth.compare == 4 => {
            if let Some(clear) = back_buffer.depth_clears.last() {
                clear.before_draw == back_buffer.draw_submissions.len()
                    && vertices_max_z(&submission.vertex_data)
                        .is_some_and(|max_z| max_z <= f32::from_bits(clear.depth))
            } else {
                !back_buffer.preserve_previous
                    && back_buffer
                        .draw_submissions
                        .iter()
                        .all(|d| d.depth.is_none_or(|dp| !dp.write))
                    && vertices_max_z(&submission.vertex_data).is_some_and(|max_z| max_z <= 1.0)
            }
        }
        _ => false,
    }
}

fn intersect_rects(a: RuntimeRect, b: Option<RuntimeRect>) -> Option<RuntimeRect> {
    let r = if let Some(b) = b {
        RuntimeRect {
            left: a.left.max(b.left),
            top: a.top.max(b.top),
            right: a.right.min(b.right),
            bottom: a.bottom.min(b.bottom),
        }
    } else {
        a
    };
    (r.right > r.left && r.bottom > r.top).then_some(r)
}

fn axis_aligned_quad_rect(vertices: &[u8], clip: Option<RuntimeRect>) -> Option<RuntimeRect> {
    if vertices.len() != 112 {
        return None;
    }
    let mut pts = [(0.0_f32, 0.0_f32); 4];
    for (i, chunk) in vertices.chunks_exact(28).enumerate() {
        let x = f32::from_le_bytes(chunk[0..4].try_into().unwrap()) + 0.5;
        let y = f32::from_le_bytes(chunk[4..8].try_into().unwrap()) + 0.5;
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        pts[i] = (x, y);
    }
    let eps = 1e-3_f32;
    let aligned_h = (pts[0].1 - pts[1].1).abs() <= eps
        && (pts[2].1 - pts[3].1).abs() <= eps
        && (pts[0].0 - pts[2].0).abs() <= eps
        && (pts[1].0 - pts[3].0).abs() <= eps;
    let aligned_v = (pts[0].0 - pts[1].0).abs() <= eps
        && (pts[2].0 - pts[3].0).abs() <= eps
        && (pts[0].1 - pts[2].1).abs() <= eps
        && (pts[1].1 - pts[3].1).abs() <= eps;
    if !aligned_h && !aligned_v {
        return None;
    }
    let min_x = pts.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
    let max_x = pts.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max);
    let min_y = pts.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
    let max_y = pts.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max);
    let rect = RuntimeRect {
        left: min_x.ceil() as i32,
        top: min_y.ceil() as i32,
        right: max_x.floor() as i32,
        bottom: max_y.floor() as i32,
    };
    intersect_rects(rect, clip)
}

fn draw_screen_bounds(draw: &RuntimeGraphicsDrawSubmission) -> Option<RuntimeRect> {
    if draw.vertex_data.len() != 112 {
        return None;
    }
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for chunk in draw.vertex_data.chunks_exact(28) {
        let x = f32::from_le_bytes(chunk[0..4].try_into().unwrap()) + 0.5;
        let y = f32::from_le_bytes(chunk[4..8].try_into().unwrap()) + 0.5;
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    let rect = RuntimeRect {
        left: min_x.floor() as i32,
        top: min_y.floor() as i32,
        right: max_x.ceil() as i32,
        bottom: max_y.ceil() as i32,
    };
    intersect_rects(rect, draw.clip_rect)
}

fn crop_presented_frame(frame: &mut RuntimeGraphicsBackBuffer, extent: (u32, u32)) {
    let bounds = RuntimeRect {
        left: 0,
        top: 0,
        right: extent.0 as i32,
        bottom: extent.1 as i32,
    };
    let mut removed_prefix = Vec::with_capacity(frame.draw_submissions.len() + 1);
    removed_prefix.push(0);
    let mut removed = 0;
    frame.draw_submissions.retain_mut(|draw| {
        // A draw without an explicit scissor is already clipped by the surface.
        let visible = if let Some(clip) = draw.clip_rect {
            draw.clip_rect = intersect_rects(bounds, Some(clip));
            draw.clip_rect.is_some()
        } else {
            true
        };
        removed += usize::from(!visible);
        removed_prefix.push(removed);
        visible
    });
    if removed != 0 {
        reindex_back_buffer_clears(frame, &removed_prefix);
    }
}

fn prune_occluded_draws(back_buffer: &mut RuntimeGraphicsBackBuffer, opaque_rect: RuntimeRect) {
    if opaque_rect.right <= opaque_rect.left || opaque_rect.bottom <= opaque_rect.top {
        return;
    }
    let wiped_depth_limit = back_buffer.depth_clears.last().map_or(0, |c| c.before_draw);
    let old_draws = std::mem::take(&mut back_buffer.draw_submissions);
    let mut removed_prefix = Vec::with_capacity(old_draws.len() + 1);
    removed_prefix.push(0usize);
    let mut removed = 0usize;
    let mut kept = Vec::with_capacity(old_draws.len());
    for (idx, draw) in old_draws.into_iter().enumerate() {
        let depth_dead = idx < wiped_depth_limit || draw.depth.is_none_or(|d| !d.write);
        let occluded = depth_dead
            && draw_screen_bounds(&draw).is_some_and(|b| {
                b.left >= opaque_rect.left
                    && b.top >= opaque_rect.top
                    && b.right <= opaque_rect.right
                    && b.bottom <= opaque_rect.bottom
            });
        if occluded {
            removed += 1;
        } else {
            kept.push(draw);
        }
        removed_prefix.push(removed);
    }
    back_buffer.draw_submissions = kept;
    if removed > 0 {
        reindex_back_buffer_clears(back_buffer, &removed_prefix);
    }
}

fn reindex_back_buffer_clears(
    back_buffer: &mut RuntimeGraphicsBackBuffer,
    removed_prefix: &[usize],
) {
    back_buffer.color_clear_before_draw -= removed_prefix[back_buffer.color_clear_before_draw];
    let mut adjusted: Vec<RuntimeGraphicsDepthClear> = Vec::new();
    for mut clear in std::mem::take(&mut back_buffer.depth_clears) {
        clear.before_draw -= removed_prefix[clear.before_draw];
        if let Some(prev) = adjusted
            .last_mut()
            .filter(|prev| prev.before_draw == clear.before_draw)
        {
            prev.depth = clear.depth;
        } else {
            adjusted.push(clear);
        }
    }
    back_buffer.depth_clears = adjusted;
}

fn set_bit_span(row: &mut [u64], left: usize, right: usize) {
    if right <= left {
        return;
    }
    let first_word = left / 64;
    let last_word = (right - 1) / 64;
    let start_mask = !0_u64 << (left % 64);
    let end_bits = right % 64;
    let end_mask = if end_bits == 0 {
        !0_u64
    } else {
        (1_u64 << end_bits) - 1
    };
    if first_word == last_word {
        row[first_word] |= start_mask & end_mask;
    } else {
        row[first_word] |= start_mask;
        for w in &mut row[first_word + 1..last_word] {
            *w = !0_u64;
        }
        row[last_word] |= end_mask;
    }
}

fn bit_span_all_set(row: &[u64], left: usize, right: usize) -> bool {
    if right <= left {
        return true;
    }
    let first_word = left / 64;
    let last_word = (right - 1) / 64;
    let start_mask = !0_u64 << (left % 64);
    let end_bits = right % 64;
    let end_mask = if end_bits == 0 {
        !0_u64
    } else {
        (1_u64 << end_bits) - 1
    };
    if first_word == last_word {
        let mask = start_mask & end_mask;
        (row[first_word] & mask) == mask
    } else {
        if (row[first_word] & start_mask) != start_mask {
            return false;
        }
        for &w in &row[first_word + 1..last_word] {
            if w != !0_u64 {
                return false;
            }
        }
        (row[last_word] & end_mask) == end_mask
    }
}

fn rect_fully_covered(
    covered: &[u64],
    words_per_row: usize,
    surface_w: i32,
    surface_h: i32,
    bounds: RuntimeRect,
) -> bool {
    let left = bounds.left.clamp(0, surface_w) as usize;
    let right = bounds.right.clamp(0, surface_w) as usize;
    let top = bounds.top.clamp(0, surface_h) as usize;
    let bottom = bounds.bottom.clamp(0, surface_h) as usize;
    if right <= left || bottom <= top {
        return true;
    }
    for y in top..bottom {
        let row = &covered[y * words_per_row..(y + 1) * words_per_row];
        if !bit_span_all_set(row, left, right) {
            return false;
        }
    }
    true
}

fn rasterize_quad_coverage(
    covered: &mut [u64],
    words_per_row: usize,
    surface_w: i32,
    surface_h: i32,
    draw: &RuntimeGraphicsDrawSubmission,
) {
    if draw.vertex_data.len() != 112 {
        return;
    }
    let mut pts = [(0.0_f64, 0.0_f64); 4];
    for (i, chunk) in draw.vertex_data.chunks_exact(28).enumerate() {
        let x = f64::from(f32::from_le_bytes(chunk[0..4].try_into().unwrap())) + 0.5;
        let y = f64::from(f32::from_le_bytes(chunk[4..8].try_into().unwrap())) + 0.5;
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        pts[i] = (x, y);
    }
    let clip_left = draw.clip_rect.map_or(0, |c| c.left.max(0)).min(surface_w);
    let clip_top = draw.clip_rect.map_or(0, |c| c.top.max(0)).min(surface_h);
    let clip_right = draw
        .clip_rect
        .map_or(surface_w, |c| c.right.min(surface_w))
        .max(0);
    let clip_bottom = draw
        .clip_rect
        .map_or(surface_h, |c| c.bottom.min(surface_h))
        .max(0);
    if clip_right <= clip_left || clip_bottom <= clip_top {
        return;
    }
    for [ia, ib, ic] in [[0usize, 1, 2], [2, 1, 3]] {
        let a = pts[ia];
        let (mut b, mut c) = (pts[ib], pts[ic]);
        let area = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
        if area == 0.0 {
            continue;
        }
        if area < 0.0 {
            std::mem::swap(&mut b, &mut c);
        }
        let min_y = ((a.1.min(b.1).min(c.1) - 0.5).ceil() as i32).max(clip_top);
        let max_y = ((a.1.max(b.1).max(c.1) - 0.5).floor() as i32 + 1).min(clip_bottom);
        if max_y <= min_y {
            continue;
        }
        let edges = [(b, c), (c, a), (a, b)];
        let inclusives = [
            c.1 < b.1 || (c.1 == b.1 && c.0 > b.0),
            a.1 < c.1 || (a.1 == c.1 && a.0 > c.0),
            b.1 < a.1 || (b.1 == a.1 && b.0 > a.0),
        ];
        'rows: for y in min_y..max_y {
            let cy = f64::from(y) + 0.5;
            let mut row_left = clip_left;
            let mut row_right = clip_right;
            for (&(p0, p1), &inc) in edges.iter().zip(inclusives.iter()) {
                let dx = p1.0 - p0.0;
                let dy = p1.1 - p0.1;
                if dy == 0.0 {
                    let side = dx * (cy - p0.1);
                    if side < 0.0 || (side == 0.0 && !inc) {
                        continue 'rows;
                    }
                } else {
                    let x_int = p0.0 + dx * (cy - p0.1) / dy;
                    let bound = (x_int - 0.5).ceil() as i32;
                    if dy > 0.0 {
                        // dy > 0 => p1.1 > p0.1 => inc is false: x + 0.5 < x_int => x <= bound - 1 => right = bound
                        row_right = row_right.min(bound);
                    } else {
                        // dy < 0 => p1.1 < p0.1 => inc is true: x + 0.5 >= x_int => x >= bound
                        row_left = row_left.max(bound);
                    }
                    if row_right <= row_left {
                        continue 'rows;
                    }
                }
            }
            let row = &mut covered[y as usize * words_per_row..(y as usize + 1) * words_per_row];
            set_bit_span(row, row_left as usize, row_right as usize);
        }
    }
}

fn has_unconditional_depth_pass(draw: &RuntimeGraphicsDrawSubmission) -> bool {
    match draw.depth {
        None => true,
        Some(d) if d.compare == 8 => true,
        Some(d) if d.compare == 4 => {
            // D16 depth buffer values are in [0.0, 1.0], so z <= 0.0 always passes LESSEQUAL.
            vertices_max_z(&draw.vertex_data).is_some_and(|mz| mz <= 0.0)
        }
        _ => false,
    }
}

fn prune_back_buffer_on_end_scene(device: &mut RuntimeGraphicsDevice) {
    let w = device.presentation.width;
    let h = device.presentation.height;
    if w == 0 || h == 0 || w > 4096 || h > 4096 {
        return;
    }
    let n = device.back_buffer.draw_submissions.len();
    if n <= 1 {
        return;
    }
    let words_per_row = (w as usize).div_ceil(64);
    let mut covered = vec![0_u64; h as usize * words_per_row];
    let (surface_w, surface_h) = (w as i32, h as i32);
    let last_clear = device.back_buffer.depth_clears.last().copied();
    let wiped_depth_limit = last_clear.map_or(0, |c| c.before_draw.min(n));
    let mut tex_cache = BTreeMap::new();
    let mut is_opaque = vec![false; n];
    for (i, draw) in device.back_buffer.draw_submissions.iter().enumerate() {
        is_opaque[i] = is_pixel_opaque_submission(draw, Some(&mut tex_cache));
    }
    let post_clear_covered = if let Some(clear) = last_clear.filter(|_| wiped_depth_limit > 0) {
        let clear_z = f32::from_bits(clear.depth);
        let total_words = h as usize * words_per_row;
        let mut pc_cov = vec![0_u64; total_words];
        let mut dirty_depth = vec![0_u64; total_words];
        let mut scratch = vec![0_u64; total_words];
        for (i, draw) in device.back_buffer.draw_submissions[wiped_depth_limit..]
            .iter()
            .enumerate()
        {
            let idx = wiped_depth_limit + i;
            if is_opaque[idx] {
                if has_unconditional_depth_pass(draw) {
                    rasterize_quad_coverage(&mut pc_cov, words_per_row, surface_w, surface_h, draw);
                } else if draw.depth.is_some_and(|d| d.compare == 4)
                    && vertices_max_z(&draw.vertex_data).is_some_and(|mz| mz <= clear_z)
                {
                    scratch.fill(0);
                    rasterize_quad_coverage(
                        &mut scratch,
                        words_per_row,
                        surface_w,
                        surface_h,
                        draw,
                    );
                    for ((dst, &src), &dirty) in pc_cov
                        .iter_mut()
                        .zip(scratch.iter())
                        .zip(dirty_depth.iter())
                    {
                        *dst |= src & !dirty;
                    }
                } else if draw.depth.is_some_and(|d| d.write) {
                    rasterize_quad_coverage(
                        &mut dirty_depth,
                        words_per_row,
                        surface_w,
                        surface_h,
                        draw,
                    );
                }
            } else if draw.depth.is_some_and(|d| d.write) {
                rasterize_quad_coverage(
                    &mut dirty_depth,
                    words_per_row,
                    surface_w,
                    surface_h,
                    draw,
                );
            }
        }
        Some(pc_cov)
    } else {
        None
    };
    let first_depth_write = if device.back_buffer.depth_clears.is_empty() {
        device
            .back_buffer
            .draw_submissions
            .iter()
            .position(|d| d.depth.is_some_and(|dp| dp.write))
            .unwrap_or(n)
    } else {
        0
    };
    let mut keep = vec![true; n];
    let mut removed_any = false;
    for idx in (wiped_depth_limit..n).rev() {
        let draw = &device.back_buffer.draw_submissions[idx];
        // Later scenes and COPY presentations can still use these depth writes.
        if draw.depth.is_none_or(|d| !d.write)
            && draw_screen_bounds(draw).is_some_and(|b| {
                rect_fully_covered(&covered, words_per_row, surface_w, surface_h, b)
            })
        {
            keep[idx] = false;
            removed_any = true;
            continue;
        }
        let uncond_depth = has_unconditional_depth_pass(draw);
        let virgin_depth_pass = !device.back_buffer.preserve_previous
            && idx <= first_depth_write
            && draw.depth.is_some_and(|d| d.compare == 4)
            && vertices_max_z(&draw.vertex_data).is_some_and(|mz| mz <= 1.0);
        if is_opaque[idx] && (uncond_depth || virgin_depth_pass) {
            rasterize_quad_coverage(&mut covered, words_per_row, surface_w, surface_h, draw);
        }
    }
    if wiped_depth_limit > 0 {
        if let Some(pc_cov) = post_clear_covered {
            for (dst, src) in covered.iter_mut().zip(pc_cov) {
                *dst |= src;
            }
        }
        for idx in (0..wiped_depth_limit).rev() {
            let draw = &device.back_buffer.draw_submissions[idx];
            if draw_screen_bounds(draw).is_some_and(|b| {
                rect_fully_covered(&covered, words_per_row, surface_w, surface_h, b)
            }) {
                keep[idx] = false;
                removed_any = true;
                continue;
            }
            if is_opaque[idx] && has_unconditional_depth_pass(draw) {
                rasterize_quad_coverage(&mut covered, words_per_row, surface_w, surface_h, draw);
            }
        }
    }
    if !removed_any {
        return;
    }
    let old_draws = std::mem::take(&mut device.back_buffer.draw_submissions);
    let mut removed_prefix = Vec::with_capacity(n + 1);
    removed_prefix.push(0usize);
    let mut removed = 0usize;
    let mut kept = Vec::with_capacity(n);
    for (idx, draw) in old_draws.into_iter().enumerate() {
        if keep[idx] {
            kept.push(draw);
        } else {
            removed += 1;
        }
        removed_prefix.push(removed);
    }
    device.back_buffer.draw_submissions = kept;
    reindex_back_buffer_clears(&mut device.back_buffer, &removed_prefix);
}

fn same_draw_coverage(left: &[u8], right: &[u8]) -> bool {
    left.len() == 112
        && right.len() == 112
        && left
            .chunks_exact(28)
            .zip(right.chunks_exact(28))
            .all(|(a, b)| a[..16] == b[..16])
}

fn replaces_opaque_draw(
    device: &RuntimeGraphicsDevice,
    vertices: &[u8],
    draw: &RuntimeGraphicsDrawSubmission,
) -> bool {
    device.d3d8.is_none()
        && device.presentation.present_mode == RuntimeGraphicsPresentMode::Copy
        && draw.clip_rect
            == Some(RuntimeRect {
                left: device.viewport[0] as i32,
                top: device.viewport[1] as i32,
                right: (device.viewport[0] + device.viewport[2]) as i32,
                bottom: (device.viewport[1] + device.viewport[3]) as i32,
            })
        && same_draw_coverage(vertices, &draw.vertex_data)
}

fn normalize_texture_snapshot_to_bgra8(
    raw: Vec<u8>,
    width: u32,
    height: u32,
    row_pitch: u32,
    format: RuntimeGraphicsColorFormat,
) -> (Vec<u8>, u32) {
    if format.bytes_per_pixel() == 4 || raw.is_empty() {
        return (raw, row_pitch);
    }
    let out_pitch = width * 4;
    let mut out = vec![0_u8; (out_pitch * height) as usize];
    for y in 0..height as usize {
        let src_row = y * row_pitch as usize;
        let dst_row = y * out_pitch as usize;
        for x in 0..width as usize {
            let word = u16::from_le_bytes([raw[src_row + x * 2], raw[src_row + x * 2 + 1]]);
            let (r, g, b, a) = match format {
                RuntimeGraphicsColorFormat::R5g6b5Unorm => {
                    let r5 = u32::from((word >> 11) & 0x1f);
                    let g6 = u32::from((word >> 5) & 0x3f);
                    let b5 = u32::from(word & 0x1f);
                    (
                        ((r5 * 255 + 15) / 31) as u8,
                        ((g6 * 255 + 31) / 63) as u8,
                        ((b5 * 255 + 15) / 31) as u8,
                        255,
                    )
                }
                RuntimeGraphicsColorFormat::X1r5g5b5Unorm => {
                    let r5 = u32::from((word >> 10) & 0x1f);
                    let g5 = u32::from((word >> 5) & 0x1f);
                    let b5 = u32::from(word & 0x1f);
                    (
                        ((r5 * 255 + 15) / 31) as u8,
                        ((g5 * 255 + 15) / 31) as u8,
                        ((b5 * 255 + 15) / 31) as u8,
                        255,
                    )
                }
                RuntimeGraphicsColorFormat::A1r5g5b5Unorm => {
                    let r5 = u32::from((word >> 10) & 0x1f);
                    let g5 = u32::from((word >> 5) & 0x1f);
                    let b5 = u32::from(word & 0x1f);
                    let a = if word & 0x8000 != 0 { 255 } else { 0 };
                    (
                        ((r5 * 255 + 15) / 31) as u8,
                        ((g5 * 255 + 15) / 31) as u8,
                        ((b5 * 255 + 15) / 31) as u8,
                        a,
                    )
                }
                RuntimeGraphicsColorFormat::A4r4g4b4Unorm => {
                    let a4 = ((word >> 12) & 0x0f) as u8;
                    let r4 = ((word >> 8) & 0x0f) as u8;
                    let g4 = ((word >> 4) & 0x0f) as u8;
                    let b4 = (word & 0x0f) as u8;
                    (r4 * 17, g4 * 17, b4 * 17, a4 * 17)
                }
                RuntimeGraphicsColorFormat::B8g8r8x8Unorm
                | RuntimeGraphicsColorFormat::B8g8r8a8Unorm => unreachable!(),
            };
            let dst = dst_row + x * 4;
            out[dst..dst + 4].copy_from_slice(&[b, g, r, a]);
        }
    }
    (out, out_pitch)
}

pub(super) fn texture_byte_layout(
    descriptor: RuntimeGraphicsTextureDescriptor,
) -> Result<(u32, u32), RuntimeStateError> {
    let bytes_per_pixel = descriptor.color_format.bytes_per_pixel();
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
            back_buffer_width: 800,
            back_buffer_height: 600,
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
                    ..RuntimeGraphicsSamplerState::default()
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
    fn shared_pixel_snapshots_survive_source_writes_present_and_retirement() {
        use crate::memory::{MemoryOwnerKind, MemoryRegionKind, PageProtection};

        let (mut graphics, device_id, texture_id, _, _) = prepared_draw_graphics();
        let mut memory = draw_memory(&[0x5a; 112]);
        let owner = memory.create_owner(MemoryOwnerKind::Runtime).unwrap();
        let range = memory
            .reserve_fixed(
                owner,
                MemoryRegionKind::GraphicsResource,
                GuestAddress(0x5000_0000),
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
        memory.write(range.start(), &[0x12; 16]).unwrap();
        memory.seal_owner(owner).unwrap();
        let delta = memory.pending_delta().unwrap();
        memory.acknowledge_delta(delta.generation).unwrap();
        let pixels = RuntimeGraphicsTexturePixelSource {
            source_texture_id: texture_id,
            source_content_generation: 1,
            range,
            row_pitch: 8,
        };
        let texture = graphics.textures.get_mut(&texture_id).unwrap();
        texture.descriptor.width = 2;
        texture.descriptor.height = 2;
        texture.backing = Some(RuntimeGraphicsTextureBacking {
            address_allocation_id: ProcessAddressAllocationId(42),
            memory_owner: owner,
            range,
            row_pitch: 8,
        });
        texture.content.pixel_source = Some(pixels);
        assert!(
            graphics
                .submit_draw(RuntimeComObjectId(7), 0, 2, &memory)
                .unwrap()
        );
        let first = Arc::clone(
            &graphics
                .device(device_id)
                .unwrap()
                .back_buffer
                .draw_submissions[0]
                .texture_data,
        );

        memory.write(range.start(), &[0x34; 16]).unwrap();
        let delta = memory.pending_delta().unwrap();
        memory.acknowledge_delta(delta.generation).unwrap();
        assert!(
            graphics
                .submit_draw(RuntimeComObjectId(7), 0, 2, &memory)
                .unwrap()
        );
        let second = Arc::clone(
            &graphics
                .device(device_id)
                .unwrap()
                .back_buffer
                .draw_submissions[1]
                .texture_data,
        );
        assert_eq!(first.as_slice(), &[0x12; 16]);
        assert_eq!(second.as_slice(), &[0x34; 16]);
        assert!(!Arc::ptr_eq(&first, &second));

        // Sharing does not bypass the source owner check or partially append a draw.
        graphics
            .textures
            .get_mut(&texture_id)
            .unwrap()
            .backing
            .as_mut()
            .unwrap()
            .memory_owner = MemoryOwnerId(0);
        let before = graphics.clone();
        assert!(
            graphics
                .submit_draw(RuntimeComObjectId(7), 0, 2, &memory)
                .is_err()
        );
        assert_eq!(graphics, before);
        drop(before);
        graphics
            .textures
            .get_mut(&texture_id)
            .unwrap()
            .backing
            .as_mut()
            .unwrap()
            .memory_owner = owner;

        {
            let mut limited = graphics.clone();
            limited
                .devices
                .get_mut(&device_id)
                .unwrap()
                .back_buffer
                .draw_submissions[0]
                .texture_data = Arc::new(vec![0; 16 * 1024 * 1024 - 32]);
            assert!(
                limited
                    .submit_draw(RuntimeComObjectId(7), 0, 2, &memory)
                    .unwrap()
            );
            let before = limited.clone();
            assert!(
                limited
                    .submit_draw(RuntimeComObjectId(7), 0, 2, &memory)
                    .is_err()
            );
            assert_eq!(limited, before);
        }

        graphics.end_scene(RuntimeComObjectId(7)).unwrap();
        graphics.present(RuntimeComObjectId(7), None).unwrap();
        let frame = graphics
            .device(device_id)
            .unwrap()
            .presented_frame
            .as_ref()
            .unwrap();
        assert!(Arc::ptr_eq(&first, &frame.draw_submissions[0].texture_data));
        assert!(Arc::ptr_eq(
            &second,
            &frame.draw_submissions[1].texture_data
        ));
        graphics
            .clear_back_buffer(RuntimeComObjectId(7), 0xff00_0000)
            .unwrap();
        graphics.present(RuntimeComObjectId(7), None).unwrap();
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
        assert!(graphics.device(device_id).is_err());
        assert_eq!(Arc::strong_count(&first), 1);
        assert_eq!(Arc::strong_count(&second), 1);
        assert_eq!(first.as_slice(), &[0x12; 16]);
        assert_eq!(second.as_slice(), &[0x34; 16]);
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
    fn copy_present_retains_content_and_up_snapshots_caller_bytes_and_detaches_stream() {
        let (mut graphics, device_id, _, _, _) = prepared_draw_graphics();
        let object = RuntimeComObjectId(7);
        let device = graphics.devices.get_mut(&device_id).unwrap();
        device.presentation.present_mode = RuntimeGraphicsPresentMode::Copy;
        device.flexible_vertex_format = Some(RuntimeGraphicsFlexibleVertexFormat::XyzRhwTexture1);
        device.render_states.alpha_blend_enabled = Some(false);
        device.texture_stage_states.stage_zero.color_operation =
            Some(RuntimeGraphicsTextureOperation::SelectArgumentOne);
        device.texture_stage_states.stage_zero.alpha_operation =
            Some(RuntimeGraphicsTextureOperation::Disable);
        device.sampler_states.sampler_zero.min_filter = Some(RuntimeGraphicsTextureFilter::Point);
        device.sampler_states.sampler_zero.mag_filter = Some(RuntimeGraphicsTextureFilter::Point);
        device.sampler_states.sampler_zero.address_u = RuntimeGraphicsTextureAddress::Clamp;
        device.sampler_states.sampler_zero.address_v = RuntimeGraphicsTextureAddress::Clamp;
        let vertices = [
            [-0.5_f32, -0.5, 1., 1., 0., 0.],
            [799.5, -0.5, 1., 1., 1., 0.],
            [-0.5, 599.5, 1., 1., 0., 1.],
            [799.5, 599.5, 1., 1., 1., 1.],
        ]
        .into_iter()
        .flatten()
        .flat_map(f32::to_le_bytes)
        .collect::<Vec<_>>();
        let mut memory = draw_memory(&vertices);
        let before = graphics.clone();
        assert!(
            graphics
                .submit_draw_up(object, 2, GuestAddress(0xffff_fff0), 24, &memory)
                .is_err()
        );
        assert_eq!(graphics, before);
        assert!(
            !graphics
                .submit_draw_up(object, 2, RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE, 28, &memory)
                .unwrap()
        );
        assert_eq!(graphics, before);
        assert!(
            graphics
                .submit_draw_up(object, 2, RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE, 24, &memory)
                .unwrap()
        );
        let device = graphics.device(device_id).unwrap();
        assert!(device.stream_sources.stream_zero.is_none());
        let draw = &device.back_buffer.draw_submissions[0];
        assert_eq!(
            (draw.min_filter, draw.mag_filter),
            (
                RuntimeGraphicsTextureFilter::Point,
                RuntimeGraphicsTextureFilter::Point
            )
        );
        assert_eq!(draw.address_u, RuntimeGraphicsTextureAddress::Clamp);
        for (input, canonical) in vertices
            .chunks_exact(24)
            .zip(draw.vertex_data.chunks_exact(28))
        {
            assert_eq!(&input[..16], &canonical[..16]);
            assert_eq!(&canonical[16..20], &[255; 4]);
            assert_eq!(&input[16..], &canonical[20..]);
        }
        graphics.end_scene(object).unwrap();
        let original = graphics.device(device_id).unwrap().back_buffer.clone();
        assert!(graphics.present(object, Some((400, 300))).unwrap());
        let captured = graphics
            .device(device_id)
            .unwrap()
            .presented_frame
            .clone()
            .unwrap();
        assert_eq!(
            captured.draw_submissions[0].clip_rect,
            Some(RuntimeRect {
                left: 0,
                top: 0,
                right: 400,
                bottom: 300,
            })
        );
        assert_eq!(graphics.device(device_id).unwrap().back_buffer, original);
        assert_eq!(
            graphics.device(device_id).unwrap().presented_extent,
            Some((400, 300))
        );
        assert!(graphics.present(object, None).unwrap());
        assert_eq!(
            graphics.device(device_id).unwrap().presented_frame.as_ref(),
            Some(&original)
        );
        assert!(graphics.present(object, Some((400, 300))).unwrap());
        let before = graphics.clone();
        assert!(!graphics.present(object, Some((801, 300))).unwrap());
        assert_eq!(graphics, before);
        graphics.begin_scene(object).unwrap();
        // Replace repeated opaque quads, while preserving different geometry.
        for _ in 0..64 {
            assert!(
                graphics
                    .submit_draw_up(object, 2, RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE, 24, &memory)
                    .unwrap()
            );
        }
        assert_eq!(
            graphics
                .device(device_id)
                .unwrap()
                .back_buffer
                .draw_submissions
                .len(),
            1
        );
        memory
            .write(RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE, &20_f32.to_le_bytes())
            .unwrap();
        let delta = memory.pending_delta().unwrap();
        memory.acknowledge_delta(delta.generation).unwrap();
        assert!(
            graphics
                .submit_draw_up(object, 2, RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE, 24, &memory)
                .unwrap()
        );
        assert_eq!(
            graphics
                .device(device_id)
                .unwrap()
                .back_buffer
                .draw_submissions
                .len(),
            2
        );
        assert_eq!(
            graphics
                .device(device_id)
                .unwrap()
                .presented_frame
                .as_ref()
                .unwrap(),
            &captured
        );
    }

    #[test]
    fn cropped_presentation_preserves_clear_order_when_draws_are_hidden() {
        let (mut graphics, device, _, _, _) = prepared_draw_graphics();
        graphics
            .submit_draw(RuntimeComObjectId(7), 0, 2, &draw_memory(&[0x5a; 112]))
            .unwrap();
        let mut frame = graphics.device(device).unwrap().back_buffer.clone();
        let mut hidden = frame.draw_submissions[0].clone();
        hidden.clip_rect = Some(RuntimeRect {
            left: 400,
            top: 0,
            right: 800,
            bottom: 600,
        });
        frame.draw_submissions.insert(0, hidden.clone());
        frame.draw_submissions.push(hidden);
        frame.clear_color = Some(0xff12_3456);
        frame.color_clear_before_draw = 1;
        frame.depth_clears = vec![
            RuntimeGraphicsDepthClear {
                before_draw: 1,
                depth: 0,
            },
            RuntimeGraphicsDepthClear {
                before_draw: 3,
                depth: 1.0_f32.to_bits(),
            },
        ];

        crop_presented_frame(&mut frame, (400, 300));

        assert_eq!(frame.draw_submissions.len(), 1);
        assert_eq!(frame.color_clear_before_draw, 0);
        assert_eq!(
            frame
                .depth_clears
                .iter()
                .map(|clear| clear.before_draw)
                .collect::<Vec<_>>(),
            [0, 1]
        );
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
    fn d3d8_copy_keeps_occluded_depth_writes_and_orders_target_only_clear() {
        let (mut graphics, device_id, _, _, _) = prepared_draw_graphics();
        let object = RuntimeComObjectId(7);
        let mut vertices = Vec::new();
        for (x, y) in [(-0.5_f32, -0.5_f32), (1.5, -0.5), (-0.5, 1.5), (1.5, 1.5)] {
            for value in [x, y, 0.25, 1.0] {
                vertices.extend(value.to_le_bytes());
            }
            vertices.extend([255; 4]);
            vertices.extend([0; 8]);
        }
        assert!(
            graphics
                .submit_draw(object, 0, 2, &draw_memory(&vertices))
                .unwrap()
        );
        let device = graphics.devices.get_mut(&device_id).unwrap();
        device.presentation.present_mode = RuntimeGraphicsPresentMode::Copy;
        let mut parameters = [0; 13];
        parameters[8] = 1;
        device.d3d8 = Some(crate::d3d8::DeviceState {
            parameters,
            transforms: BTreeMap::new(),
            render_values: BTreeMap::new(),
            stage_values: BTreeMap::new(),
            back_buffer_surface: None,
        });
        let mut overlay = device.back_buffer.draw_submissions[0].clone();
        overlay.clip_rect = None;
        let writer = &mut device.back_buffer.draw_submissions[0];
        writer.clip_rect = None;
        writer.depth = Some(RuntimeGraphicsDepthDrawState {
            write: true,
            compare: 4,
        });
        device.back_buffer.draw_submissions.push(overlay);
        assert!(graphics.end_scene(object).unwrap());
        assert_eq!(
            graphics
                .device(device_id)
                .unwrap()
                .back_buffer
                .draw_submissions
                .len(),
            2
        );

        assert!(graphics.clear_d3d8(object, 1, 0xff12_3456, 0).unwrap());
        assert!(graphics.present(object, None).unwrap());
        let frame = graphics
            .device(device_id)
            .unwrap()
            .presented_frame
            .as_ref()
            .unwrap();
        assert_eq!(frame.clear_color, Some(0xff12_3456));
        assert_eq!(frame.color_clear_before_draw, 1);
        assert_eq!(frame.draw_submissions.len(), 1);
        assert!(frame.draw_submissions[0].depth.unwrap().write);

        assert!(
            graphics
                .clear_d3d8(object, 2, 0, 0.5_f32.to_bits())
                .unwrap()
        );
        assert!(graphics.clear_d3d8(object, 1, 0xff00_0000, 0).unwrap());
        assert!(graphics.present(object, None).unwrap());
        let frame = graphics
            .device(device_id)
            .unwrap()
            .presented_frame
            .as_ref()
            .unwrap();
        assert!(frame.preserve_previous);
        assert_eq!(frame.color_clear_before_draw, 0);
        assert_eq!(
            frame.depth_clears,
            [RuntimeGraphicsDepthClear {
                before_draw: 0,
                depth: 0.5_f32.to_bits()
            }]
        );
    }

    #[test]
    fn present_moves_closed_back_buffer_to_front_frame_and_rejects_invalid_state_atomically() {
        let (mut graphics, device_id, _, _, _) = prepared_draw_graphics();
        graphics
            .submit_draw(RuntimeComObjectId(7), 0, 2, &draw_memory(&[0x5a; 112]))
            .unwrap();
        graphics.end_scene(RuntimeComObjectId(7)).unwrap();

        let request = RuntimeComObjectId(7);
        graphics.present(request, None).unwrap();
        let device = graphics.device(device_id).unwrap();
        let frame = device.presented_frame.as_ref().unwrap();
        assert_eq!(frame.draw_submissions.len(), 1);
        assert_eq!(device.back_buffer.clear_color, None);
        assert!(device.back_buffer.draw_submissions.is_empty());

        let before_empty = graphics.clone();
        assert!(!graphics.present(request, None).unwrap());
        assert_eq!(graphics, before_empty);

        for device_com_object_id in [RuntimeComObjectId(0), RuntimeComObjectId(99)] {
            let before = graphics.clone();
            graphics.present(device_com_object_id, None).unwrap_err();

            assert_eq!(graphics, before);
        }

        let (mut active, _, _, _, _) = prepared_draw_graphics();
        let before_active = active.clone();
        assert!(!active.present(request, None).unwrap());
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
        let texture_plan = graphics
            .validate_texture_creation(RuntimeComObjectId(7), texture_descriptor())
            .unwrap();
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
        let surface_plan = graphics
            .validate_surface_creation(RuntimeComObjectId(8), 0)
            .unwrap()
            .unwrap();
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
        for failure in [3, 4, 5, 6] {
            let mut rejected = graphics.clone();
            let mut backing = RuntimeGraphicsTextureBacking {
                address_allocation_id: ProcessAddressAllocationId(41),
                memory_owner: MemoryOwnerId(23),
                range: backing_range,
                row_pitch: 4_096,
            };
            match failure {
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
        for failure in [5, 6] {
            let rejected = graphics.clone();
            let mut allocation = ProcessAddressAllocationId(41);
            let mut range = backing_range;
            match failure {
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

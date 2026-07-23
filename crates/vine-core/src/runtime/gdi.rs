use std::collections::{BTreeMap, BTreeSet};

use serde::{Serialize, Serializer, ser::SerializeStruct};

use crate::memory::{GuestAddress, GuestRange, MemoryOwnerId};

use super::address_space::ProcessAddressAllocationId;
use super::com::RuntimeComObjectId;
use super::graphics::{RuntimeGraphicsColorFormat, RuntimeGraphicsSurfaceId};
use super::{RuntimeRect, RuntimeStateError, RuntimeThreadId, RuntimeWindowId};

pub const LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE: &str = "vine.logical-gdi-dc.v1";
pub const LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE: &str = "vine.logical-gdi-object.v1";
pub const RUNTIME_DEVICE_CONTEXT_HANDLE_BASE: u32 = 0x4701_0001;
pub const RUNTIME_GDI_OBJECT_HANDLE_BASE: u32 = 0x4702_0001;
pub const RUNTIME_GDI_BLACK_BRUSH_INDEX: i32 = 4;
pub const RUNTIME_GDI_SYSTEM_FONT_INDEX: i32 = 13;
pub const RUNTIME_GDI_DEFAULT_BITMAP_INDEX: i32 = 21;
pub const RUNTIME_GDI_DEVICE_CONTEXT_LIMIT: usize = 4_096;
pub const RUNTIME_GDI_OBJECT_LIMIT: usize = 4_096;
pub const RUNTIME_GDI_SURFACE_LIMIT: usize = 4_096;
pub const RUNTIME_GDI_SURFACE_FILL_LIMIT: usize = 16_384;
pub const RUNTIME_LOG_FONT_A_BYTE_LENGTH: usize = 60;
pub const RUNTIME_LOG_FONT_A_FACE_CAPACITY: usize = 32;
pub const RUNTIME_GDI_BITMAP_BYTE_LENGTH: usize = 24;
pub const RUNTIME_GDI_DIB_RGB_COLORS: u32 = 0;
pub const RUNTIME_GDI_DIB_BI_RGB: u32 = 0;
pub const RUNTIME_GDI_RESOURCE_ADDRESS_OWNER: &str = "runtime-gdi-resource";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeDeviceContextId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeGdiObjectId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeGdiSurfaceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeGdiWindowTarget {
    pub window_id: RuntimeWindowId,
    pub window_handle: u32,
    pub owner_thread_id: RuntimeThreadId,
    pub client_width: u32,
    pub client_height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeGdiScreenTarget {
    pub monitor_handle: u32,
    pub owner_thread_id: RuntimeThreadId,
    pub rect: RuntimeRect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeGdiGraphicsSurfaceTarget {
    pub surface_id: RuntimeGraphicsSurfaceId,
    pub surface_com_object_id: RuntimeComObjectId,
    pub owner_thread_id: RuntimeThreadId,
    pub width: u32,
    pub height: u32,
    pub color_format: RuntimeGraphicsColorFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum RuntimeDeviceContextTarget {
    Window {
        window_id: RuntimeWindowId,
        window_handle: u32,
        surface_id: RuntimeGdiSurfaceId,
    },
    Screen {
        monitor_handle: u32,
        rect: RuntimeRect,
    },
    Memory {
        compatible_monitor_handle: u32,
        compatible_rect: RuntimeRect,
    },
    GraphicsSurface {
        surface_id: RuntimeGraphicsSurfaceId,
        surface_com_object_id: RuntimeComObjectId,
        width: u32,
        height: u32,
        color_format: RuntimeGraphicsColorFormat,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDeviceContext {
    pub id: RuntimeDeviceContextId,
    pub handle: u32,
    pub namespace: String,
    pub owner_thread_id: RuntimeThreadId,
    pub target: RuntimeDeviceContextTarget,
    pub selected_font_id: Option<RuntimeGdiObjectId>,
    pub selected_bitmap_id: Option<RuntimeGdiObjectId>,
    pub selection_generation: u64,
}

impl RuntimeDeviceContext {
    pub fn window_surface_id(&self) -> Option<RuntimeGdiSurfaceId> {
        match self.target {
            RuntimeDeviceContextTarget::Window { surface_id, .. } => Some(surface_id),
            RuntimeDeviceContextTarget::Screen { .. }
            | RuntimeDeviceContextTarget::Memory { .. }
            | RuntimeDeviceContextTarget::GraphicsSurface { .. } => None,
        }
    }
}

impl Serialize for RuntimeDeviceContext {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.target {
            RuntimeDeviceContextTarget::Window {
                window_id,
                window_handle,
                surface_id,
            } => {
                let mut state = serializer.serialize_struct("RuntimeDeviceContext", 7)?;
                state.serialize_field("id", &self.id)?;
                state.serialize_field("handle", &self.handle)?;
                state.serialize_field("namespace", &self.namespace)?;
                state.serialize_field("windowId", &window_id)?;
                state.serialize_field("windowHandle", &window_handle)?;
                state.serialize_field("ownerThreadId", &self.owner_thread_id)?;
                state.serialize_field("surfaceId", &surface_id)?;
                state.end()
            }
            RuntimeDeviceContextTarget::Screen {
                monitor_handle,
                rect,
            } => {
                let mut state = serializer.serialize_struct("RuntimeDeviceContext", 7)?;
                state.serialize_field("id", &self.id)?;
                state.serialize_field("handle", &self.handle)?;
                state.serialize_field("namespace", &self.namespace)?;
                state.serialize_field("ownerThreadId", &self.owner_thread_id)?;
                state.serialize_field("targetKind", "screen")?;
                state.serialize_field("monitorHandle", &monitor_handle)?;
                state.serialize_field("rect", &rect)?;
                state.end()
            }
            RuntimeDeviceContextTarget::Memory {
                compatible_monitor_handle,
                compatible_rect,
            } => {
                let mut state = serializer.serialize_struct("RuntimeDeviceContext", 10)?;
                state.serialize_field("id", &self.id)?;
                state.serialize_field("handle", &self.handle)?;
                state.serialize_field("namespace", &self.namespace)?;
                state.serialize_field("ownerThreadId", &self.owner_thread_id)?;
                state.serialize_field("targetKind", "memory")?;
                state.serialize_field("compatibleMonitorHandle", &compatible_monitor_handle)?;
                state.serialize_field("compatibleRect", &compatible_rect)?;
                state.serialize_field("selectedFontId", &self.selected_font_id)?;
                state.serialize_field("selectedBitmapId", &self.selected_bitmap_id)?;
                state.serialize_field("selectionGeneration", &self.selection_generation)?;
                state.end()
            }
            RuntimeDeviceContextTarget::GraphicsSurface {
                surface_id,
                surface_com_object_id,
                width,
                height,
                color_format,
            } => {
                let mut state = serializer.serialize_struct("RuntimeDeviceContext", 10)?;
                state.serialize_field("id", &self.id)?;
                state.serialize_field("handle", &self.handle)?;
                state.serialize_field("namespace", &self.namespace)?;
                state.serialize_field("ownerThreadId", &self.owner_thread_id)?;
                state.serialize_field("targetKind", "graphics-surface")?;
                state.serialize_field("surfaceId", &surface_id)?;
                state.serialize_field("surfaceComObjectId", &surface_com_object_id)?;
                state.serialize_field("width", &width)?;
                state.serialize_field("height", &height)?;
                state.serialize_field("colorFormat", &color_format)?;
                state.end()
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum RuntimeGdiObjectKind {
    StockBrush {
        index: i32,
        color_ref: u32,
    },
    StockFont {
        index: i32,
    },
    StockBitmap {
        index: i32,
        width: u32,
        height: u32,
        bits_per_pixel: u16,
    },
    FontA {
        log_font: RuntimeLogFontA,
    },
    DibSection {
        descriptor: RuntimeGdiDibSectionDescriptor,
        backing: RuntimeGdiDibSectionBacking,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGdiRgbQuad {
    pub blue: u8,
    pub green: u8,
    pub red: u8,
    pub reserved: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGdiDibSectionDescriptor {
    pub width: i32,
    pub height: i32,
    pub planes: u16,
    pub bits_per_pixel: u16,
    pub compression: u32,
    pub stride_bytes: u32,
    pub pixel_byte_length: u32,
    pub palette: Vec<RuntimeGdiRgbQuad>,
}

impl RuntimeGdiDibSectionDescriptor {
    pub fn bitmap_bytes(
        &self,
        bits_address: GuestAddress,
    ) -> Result<[u8; RUNTIME_GDI_BITMAP_BYTE_LENGTH], RuntimeStateError> {
        validate_dib_section_descriptor(self)?;
        let height = self.height.checked_abs().ok_or_else(|| {
            RuntimeStateError::invalid_input(
                "runtime-gdi",
                "dib-section-height-invalid",
                "logical DIB section height cannot be i32::MIN",
            )
        })?;
        let mut bytes = [0_u8; RUNTIME_GDI_BITMAP_BYTE_LENGTH];
        bytes[4..8].copy_from_slice(&self.width.to_le_bytes());
        bytes[8..12].copy_from_slice(&height.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.stride_bytes.to_le_bytes());
        bytes[16..18].copy_from_slice(&self.planes.to_le_bytes());
        bytes[18..20].copy_from_slice(&self.bits_per_pixel.to_le_bytes());
        bytes[20..24].copy_from_slice(&bits_address.0.to_le_bytes());
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGdiDibSectionBacking {
    pub address_allocation_id: ProcessAddressAllocationId,
    pub memory_owner: MemoryOwnerId,
    pub range: GuestRange,
    pub content_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLogFontA {
    pub height: i32,
    pub width: i32,
    pub escapement: i32,
    pub orientation: i32,
    pub weight: i32,
    pub italic: u8,
    pub underline: u8,
    pub strike_out: u8,
    pub character_set: u8,
    pub output_precision: u8,
    pub clip_precision: u8,
    pub quality: u8,
    pub pitch_and_family: u8,
    pub face_name_windows_932: Vec<u8>,
}

impl RuntimeLogFontA {
    pub fn to_bytes(&self) -> Result<[u8; RUNTIME_LOG_FONT_A_BYTE_LENGTH], RuntimeStateError> {
        validate_log_font_a(self)?;
        let mut bytes = [0; RUNTIME_LOG_FONT_A_BYTE_LENGTH];
        bytes[0..4].copy_from_slice(&self.height.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.width.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.escapement.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.orientation.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.weight.to_le_bytes());
        bytes[20] = self.italic;
        bytes[21] = self.underline;
        bytes[22] = self.strike_out;
        bytes[23] = self.character_set;
        bytes[24] = self.output_precision;
        bytes[25] = self.clip_precision;
        bytes[26] = self.quality;
        bytes[27] = self.pitch_and_family;
        bytes[28..28 + self.face_name_windows_932.len()]
            .copy_from_slice(&self.face_name_windows_932);
        Ok(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGdiObject {
    pub id: RuntimeGdiObjectId,
    pub handle: u32,
    pub namespace: String,
    pub shared: bool,
    pub object: RuntimeGdiObjectKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGdiSurfaceFill {
    pub sequence: u64,
    pub rect: RuntimeRect,
    pub brush_id: RuntimeGdiObjectId,
    pub color_ref: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGdiSurface {
    pub id: RuntimeGdiSurfaceId,
    pub window_id: RuntimeWindowId,
    pub width: u32,
    pub height: u32,
    pub fills: Vec<RuntimeGdiSurfaceFill>,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGetDcResult {
    pub device_context_id: RuntimeDeviceContextId,
    pub handle: u32,
    pub surface_id: RuntimeGdiSurfaceId,
    pub surface_created: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGetScreenDcResult {
    pub device_context_id: RuntimeDeviceContextId,
    pub handle: u32,
    pub monitor_handle: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGetGraphicsSurfaceDcResult {
    pub device_context_id: RuntimeDeviceContextId,
    pub handle: u32,
    pub surface_id: RuntimeGraphicsSurfaceId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGetStockObjectResult {
    pub object_id: RuntimeGdiObjectId,
    pub handle: u32,
    pub already_created: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCreateFontAResult {
    pub object_id: RuntimeGdiObjectId,
    pub handle: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCreateCompatibleDcResult {
    pub device_context_id: RuntimeDeviceContextId,
    pub handle: u32,
    pub selected_font_id: RuntimeGdiObjectId,
    pub selected_bitmap_id: RuntimeGdiObjectId,
    pub default_objects_created: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCreateDibSectionResult {
    pub object_id: RuntimeGdiObjectId,
    pub handle: u32,
    pub bits_address: GuestAddress,
    pub backing: RuntimeGdiDibSectionBacking,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGdiTextRasterTarget {
    pub device_context_id: RuntimeDeviceContextId,
    pub device_context_handle: u32,
    pub owner_thread_id: RuntimeThreadId,
    pub selection_generation: u64,
    pub font_id: RuntimeGdiObjectId,
    pub font_handle: u32,
    pub log_font: RuntimeLogFontA,
    pub dib_section_id: RuntimeGdiObjectId,
    pub dib_section_handle: u32,
    pub descriptor: RuntimeGdiDibSectionDescriptor,
    pub backing: RuntimeGdiDibSectionBacking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGdiTextRasterResult {
    pub device_context_id: RuntimeDeviceContextId,
    pub font_id: RuntimeGdiObjectId,
    pub dib_section_id: RuntimeGdiObjectId,
    pub selection_generation: u64,
    pub content_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSelectGdiObjectResult {
    pub device_context_id: RuntimeDeviceContextId,
    pub selected_object_id: RuntimeGdiObjectId,
    pub selected_handle: u32,
    pub previous_object_id: RuntimeGdiObjectId,
    pub previous_handle: u32,
    pub selection_generation: u64,
    pub changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFillRectResult {
    pub surface_id: RuntimeGdiSurfaceId,
    pub surface_generation: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<RuntimeGdiSurfaceFill>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeGdi {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    device_contexts: BTreeMap<RuntimeDeviceContextId, RuntimeDeviceContext>,
    #[serde(skip)]
    device_context_handle_index: BTreeMap<u32, RuntimeDeviceContextId>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    objects: BTreeMap<RuntimeGdiObjectId, RuntimeGdiObject>,
    #[serde(skip)]
    object_handle_index: BTreeMap<u32, RuntimeGdiObjectId>,
    #[serde(skip)]
    stock_object_index: BTreeMap<i32, RuntimeGdiObjectId>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    surfaces: BTreeMap<RuntimeGdiSurfaceId, RuntimeGdiSurface>,
    #[serde(skip)]
    window_surface_index: BTreeMap<RuntimeWindowId, RuntimeGdiSurfaceId>,
    #[serde(skip)]
    graphics_surface_dc_index: BTreeMap<RuntimeGraphicsSurfaceId, RuntimeDeviceContextId>,
    next_device_context_id: u64,
    next_device_context_handle: u32,
    next_object_id: u64,
    next_object_handle: u32,
    next_surface_id: u64,
    next_fill_sequence: u64,
    generation: u64,
}

impl Default for RuntimeGdi {
    fn default() -> Self {
        Self {
            device_contexts: BTreeMap::new(),
            device_context_handle_index: BTreeMap::new(),
            objects: BTreeMap::new(),
            object_handle_index: BTreeMap::new(),
            stock_object_index: BTreeMap::new(),
            surfaces: BTreeMap::new(),
            window_surface_index: BTreeMap::new(),
            graphics_surface_dc_index: BTreeMap::new(),
            next_device_context_id: 1,
            next_device_context_handle: RUNTIME_DEVICE_CONTEXT_HANDLE_BASE,
            next_object_id: 1,
            next_object_handle: RUNTIME_GDI_OBJECT_HANDLE_BASE,
            next_surface_id: 1,
            next_fill_sequence: 1,
            generation: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeGetDcPlan {
    expected_generation: u64,
    next_generation: u64,
    device_context: RuntimeDeviceContext,
    new_surface: Option<RuntimeGdiSurface>,
    next_device_context_id: u64,
    next_device_context_handle: u32,
    next_surface_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeGetScreenDcPlan {
    expected_generation: u64,
    next_generation: u64,
    device_context: RuntimeDeviceContext,
    next_device_context_id: u64,
    next_device_context_handle: u32,
}

impl RuntimeGetScreenDcPlan {
    fn result(&self) -> RuntimeGetScreenDcResult {
        let RuntimeDeviceContextTarget::Screen { monitor_handle, .. } = self.device_context.target
        else {
            unreachable!("screen DC plan must target a logical monitor")
        };
        RuntimeGetScreenDcResult {
            device_context_id: self.device_context.id,
            handle: self.device_context.handle,
            monitor_handle,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeCreateCompatibleDcPlan {
    expected_generation: u64,
    next_generation: u64,
    device_context: RuntimeDeviceContext,
    new_default_objects: Vec<RuntimeGdiObject>,
    next_device_context_id: u64,
    next_device_context_handle: u32,
    next_object_id: u64,
    next_object_handle: u32,
}

impl RuntimeCreateCompatibleDcPlan {
    fn result(&self) -> RuntimeCreateCompatibleDcResult {
        let (Some(selected_font_id), Some(selected_bitmap_id)) = (
            self.device_context.selected_font_id,
            self.device_context.selected_bitmap_id,
        ) else {
            unreachable!("memory DC plan must publish initial font and bitmap selections")
        };
        RuntimeCreateCompatibleDcResult {
            device_context_id: self.device_context.id,
            handle: self.device_context.handle,
            selected_font_id,
            selected_bitmap_id,
            default_objects_created: !self.new_default_objects.is_empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGetGraphicsSurfaceDcPlan {
    expected_generation: u64,
    next_generation: u64,
    device_context: RuntimeDeviceContext,
    next_device_context_id: u64,
    next_device_context_handle: u32,
}

impl RuntimeGetGraphicsSurfaceDcPlan {
    pub(crate) fn result(&self) -> RuntimeGetGraphicsSurfaceDcResult {
        let RuntimeDeviceContextTarget::GraphicsSurface { surface_id, .. } =
            self.device_context.target
        else {
            unreachable!("graphics surface DC plan must target a graphics surface")
        };
        RuntimeGetGraphicsSurfaceDcResult {
            device_context_id: self.device_context.id,
            handle: self.device_context.handle,
            surface_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeGetStockObjectPlan {
    expected_generation: u64,
    next_generation: u64,
    object: RuntimeGdiObject,
    already_created: bool,
    next_object_id: u64,
    next_object_handle: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeCreateFontAPlan {
    expected_generation: u64,
    next_generation: u64,
    object: RuntimeGdiObject,
    next_object_id: u64,
    next_object_handle: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeCreateDibSectionPlan {
    expected_generation: u64,
    next_generation: u64,
    source_device_context: RuntimeDeviceContext,
    object_id: RuntimeGdiObjectId,
    handle: u32,
    descriptor: RuntimeGdiDibSectionDescriptor,
    address_allocation_id: ProcessAddressAllocationId,
    backing_range: GuestRange,
    next_object_id: u64,
    next_object_handle: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeGdiTextRasterPlan {
    expected_generation: u64,
    next_generation: u64,
    target: RuntimeGdiTextRasterTarget,
    next_content_generation: u64,
}

impl RuntimeCreateDibSectionPlan {
    pub(super) fn descriptor(&self) -> &RuntimeGdiDibSectionDescriptor {
        &self.descriptor
    }

    pub(super) fn address_allocation_id(&self) -> ProcessAddressAllocationId {
        self.address_allocation_id
    }

    pub(super) fn backing_range(&self) -> GuestRange {
        self.backing_range
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeGdiSelectionSlot {
    Font,
    Bitmap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSelectGdiObjectPlan {
    expected_generation: u64,
    next_generation: u64,
    device_context: RuntimeDeviceContext,
    selected_object: RuntimeGdiObject,
    previous_object: RuntimeGdiObject,
    slot: RuntimeGdiSelectionSlot,
    next_selection_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeDeleteGdiObjectPlan {
    expected_generation: u64,
    next_generation: u64,
    object: RuntimeGdiObject,
}

impl RuntimeDeleteGdiObjectPlan {
    pub(super) fn object(&self) -> &RuntimeGdiObject {
        &self.object
    }

    pub(super) fn backing(&self) -> Option<RuntimeGdiDibSectionBacking> {
        match &self.object.object {
            RuntimeGdiObjectKind::DibSection { backing, .. } => Some(*backing),
            RuntimeGdiObjectKind::StockBrush { .. }
            | RuntimeGdiObjectKind::StockFont { .. }
            | RuntimeGdiObjectKind::StockBitmap { .. }
            | RuntimeGdiObjectKind::FontA { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeReleaseDcPlan {
    expected_generation: u64,
    next_generation: u64,
    device_context: RuntimeDeviceContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeFillRectPlan {
    expected_generation: u64,
    next_generation: u64,
    device_context: RuntimeDeviceContext,
    brush_id: RuntimeGdiObjectId,
    surface_generation: u64,
    next_surface_generation: u64,
    fill: Option<RuntimeGdiSurfaceFill>,
    next_fill_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeGraphicsSurfaceFillRectPlan {
    expected_generation: u64,
    device_context: RuntimeDeviceContext,
    brush_id: RuntimeGdiObjectId,
    rect: RuntimeRect,
    color_ref: u32,
}

impl RuntimeGraphicsSurfaceFillRectPlan {
    pub(super) fn surface_id(&self) -> RuntimeGraphicsSurfaceId {
        let RuntimeDeviceContextTarget::GraphicsSurface { surface_id, .. } =
            self.device_context.target
        else {
            unreachable!("graphics fill plan must target a graphics surface")
        };
        surface_id
    }

    pub(super) fn surface_com_object_id(&self) -> RuntimeComObjectId {
        let RuntimeDeviceContextTarget::GraphicsSurface {
            surface_com_object_id,
            ..
        } = self.device_context.target
        else {
            unreachable!("graphics fill plan must target a graphics surface")
        };
        surface_com_object_id
    }

    pub(super) fn rect(&self) -> RuntimeRect {
        self.rect
    }

    pub(super) fn color_ref(&self) -> u32 {
        self.color_ref
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGdiComRetirementGuard {
    expected_generation: u64,
    retired_graphics_surface_objects: BTreeSet<RuntimeComObjectId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeGdiTeardownPlan {
    expected_generation: u64,
    next_generation: u64,
    device_contexts: usize,
    objects: usize,
    surfaces: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeDeviceContextAllocation {
    id: RuntimeDeviceContextId,
    handle: u32,
    next_id: u64,
    next_handle: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeGdiObjectAllocation {
    id: RuntimeGdiObjectId,
    handle: u32,
    next_id: u64,
    next_handle: u32,
}

impl RuntimeGdi {
    pub(super) fn plan_get_dc(
        &self,
        target: RuntimeGdiWindowTarget,
    ) -> Result<RuntimeGetDcPlan, RuntimeStateError> {
        validate_window_target(target)?;
        let allocation = self.plan_device_context_allocation()?;

        let (surface_id, new_surface, next_surface_id) =
            if let Some(surface_id) = self.window_surface_index.get(&target.window_id).copied() {
                let surface = self.surfaces.get(&surface_id).ok_or_else(|| {
                    RuntimeStateError::runtime_defect(
                        "runtime-gdi",
                        "window-surface-index-dangling",
                        "logical window surface index does not resolve to a live surface",
                    )
                })?;
                if surface.window_id != target.window_id
                    || surface.width != target.client_width
                    || surface.height != target.client_height
                {
                    return Err(RuntimeStateError::conflict(
                        "runtime-gdi",
                        "window-surface-geometry-changed",
                        "logical window client geometry changed after its GDI surface was created",
                    ));
                }
                (surface_id, None, self.next_surface_id)
            } else {
                if self.surfaces.len() >= RUNTIME_GDI_SURFACE_LIMIT {
                    return Err(RuntimeStateError::resource_limit(
                        "runtime-gdi",
                        "surface-capacity-exhausted",
                        "logical GDI surface capacity is exhausted",
                    ));
                }
                let surface_id = RuntimeGdiSurfaceId(self.next_surface_id);
                let next_surface_id =
                    checked_next_identity(self.next_surface_id, "surface-identity-exhausted")?;
                (
                    surface_id,
                    Some(RuntimeGdiSurface {
                        id: surface_id,
                        window_id: target.window_id,
                        width: target.client_width,
                        height: target.client_height,
                        fills: Vec::new(),
                        generation: 0,
                    }),
                    next_surface_id,
                )
            };
        let next_generation = checked_next_generation(self.generation)?;
        Ok(RuntimeGetDcPlan {
            expected_generation: self.generation,
            next_generation,
            device_context: RuntimeDeviceContext {
                id: allocation.id,
                handle: allocation.handle,
                namespace: LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE.to_owned(),
                owner_thread_id: target.owner_thread_id,
                target: RuntimeDeviceContextTarget::Window {
                    window_id: target.window_id,
                    window_handle: target.window_handle,
                    surface_id,
                },
                selected_font_id: None,
                selected_bitmap_id: None,
                selection_generation: 0,
            },
            new_surface,
            next_device_context_id: allocation.next_id,
            next_device_context_handle: allocation.next_handle,
            next_surface_id,
        })
    }

    pub(super) fn commit_get_dc(
        &mut self,
        plan: RuntimeGetDcPlan,
    ) -> Result<RuntimeGetDcResult, RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.next_device_context_id != plan.device_context.id.0
            || self.next_device_context_handle != plan.device_context.handle
            || self.next_surface_id
                != plan
                    .new_surface
                    .as_ref()
                    .map_or(plan.next_surface_id, |surface| surface.id.0)
            || self
                .device_context_handle_index
                .contains_key(&plan.device_context.handle)
        {
            return Err(stale_plan("get-dc-plan-stale"));
        }
        let surface_created = plan.new_surface.is_some();
        if let Some(surface) = plan.new_surface {
            if self.window_surface_index.contains_key(&surface.window_id)
                || self.surfaces.contains_key(&surface.id)
            {
                return Err(stale_plan("get-dc-plan-stale"));
            }
            self.window_surface_index
                .insert(surface.window_id, surface.id);
            self.surfaces.insert(surface.id, surface);
        } else if !matches!(
            plan.device_context.target,
            RuntimeDeviceContextTarget::Window { surface_id, .. }
                if self.surfaces.contains_key(&surface_id)
        ) {
            return Err(stale_plan("get-dc-plan-stale"));
        }
        self.device_context_handle_index
            .insert(plan.device_context.handle, plan.device_context.id);
        self.device_contexts
            .insert(plan.device_context.id, plan.device_context.clone());
        self.next_device_context_id = plan.next_device_context_id;
        self.next_device_context_handle = plan.next_device_context_handle;
        self.next_surface_id = plan.next_surface_id;
        self.generation = plan.next_generation;
        let RuntimeDeviceContextTarget::Window { surface_id, .. } = plan.device_context.target
        else {
            unreachable!("window GetDC plan must publish a window target")
        };
        Ok(RuntimeGetDcResult {
            device_context_id: plan.device_context.id,
            handle: plan.device_context.handle,
            surface_id,
            surface_created,
        })
    }

    pub(super) fn plan_get_screen_dc(
        &self,
        target: RuntimeGdiScreenTarget,
    ) -> Result<RuntimeGetScreenDcPlan, RuntimeStateError> {
        validate_screen_target(target)?;
        let allocation = self.plan_device_context_allocation()?;
        Ok(RuntimeGetScreenDcPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            device_context: RuntimeDeviceContext {
                id: allocation.id,
                handle: allocation.handle,
                namespace: LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE.to_owned(),
                owner_thread_id: target.owner_thread_id,
                target: RuntimeDeviceContextTarget::Screen {
                    monitor_handle: target.monitor_handle,
                    rect: target.rect,
                },
                selected_font_id: None,
                selected_bitmap_id: None,
                selection_generation: 0,
            },
            next_device_context_id: allocation.next_id,
            next_device_context_handle: allocation.next_handle,
        })
    }

    pub(super) fn commit_get_screen_dc(
        &mut self,
        plan: RuntimeGetScreenDcPlan,
    ) -> Result<RuntimeGetScreenDcResult, RuntimeStateError> {
        let result = plan.result();
        if self.generation != plan.expected_generation
            || self.next_device_context_id != plan.device_context.id.0
            || self.next_device_context_handle != plan.device_context.handle
            || self
                .device_context_handle_index
                .contains_key(&plan.device_context.handle)
            || self.device_contexts.contains_key(&plan.device_context.id)
        {
            return Err(stale_plan("screen-get-dc-plan-stale"));
        }
        self.device_context_handle_index
            .insert(plan.device_context.handle, plan.device_context.id);
        self.device_contexts
            .insert(plan.device_context.id, plan.device_context);
        self.next_device_context_id = plan.next_device_context_id;
        self.next_device_context_handle = plan.next_device_context_handle;
        self.generation = plan.next_generation;
        Ok(result)
    }

    pub(super) fn plan_create_compatible_dc(
        &self,
        target: RuntimeGdiScreenTarget,
    ) -> Result<RuntimeCreateCompatibleDcPlan, RuntimeStateError> {
        validate_screen_target(target)?;
        let device_context_allocation = self.plan_device_context_allocation()?;
        let existing_font = self.stock_object_by_index(RUNTIME_GDI_SYSTEM_FONT_INDEX)?;
        if let Some(object) = existing_font {
            validate_default_font_object(object)?;
        }
        let existing_bitmap = self.stock_object_by_index(RUNTIME_GDI_DEFAULT_BITMAP_INDEX)?;
        if let Some(object) = existing_bitmap {
            validate_default_bitmap_object(object)?;
        }
        let missing_default_objects =
            usize::from(existing_font.is_none()) + usize::from(existing_bitmap.is_none());
        let mut allocations = self
            .plan_object_allocations(missing_default_objects)?
            .into_iter();
        let mut new_default_objects = Vec::with_capacity(missing_default_objects);
        let selected_font_id = existing_font.map_or_else(
            || {
                let allocation = allocations
                    .next()
                    .expect("missing default font must reserve one object allocation");
                new_default_objects.push(RuntimeGdiObject {
                    id: allocation.id,
                    handle: allocation.handle,
                    namespace: LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE.to_owned(),
                    shared: true,
                    object: RuntimeGdiObjectKind::StockFont {
                        index: RUNTIME_GDI_SYSTEM_FONT_INDEX,
                    },
                });
                allocation.id
            },
            |object| object.id,
        );
        let selected_bitmap_id = existing_bitmap.map_or_else(
            || {
                let allocation = allocations
                    .next()
                    .expect("missing default bitmap must reserve one object allocation");
                new_default_objects.push(RuntimeGdiObject {
                    id: allocation.id,
                    handle: allocation.handle,
                    namespace: LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE.to_owned(),
                    shared: true,
                    object: RuntimeGdiObjectKind::StockBitmap {
                        index: RUNTIME_GDI_DEFAULT_BITMAP_INDEX,
                        width: 1,
                        height: 1,
                        bits_per_pixel: 1,
                    },
                });
                allocation.id
            },
            |object| object.id,
        );
        debug_assert!(allocations.next().is_none());
        let (next_object_id, next_object_handle) = new_default_objects.last().map_or(
            (self.next_object_id, self.next_object_handle),
            |object| {
                (
                    object
                        .id
                        .0
                        .checked_add(1)
                        .expect("planned object ID advances"),
                    object
                        .handle
                        .checked_add(1)
                        .expect("planned object handle advances"),
                )
            },
        );
        Ok(RuntimeCreateCompatibleDcPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            device_context: RuntimeDeviceContext {
                id: device_context_allocation.id,
                handle: device_context_allocation.handle,
                namespace: LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE.to_owned(),
                owner_thread_id: target.owner_thread_id,
                target: RuntimeDeviceContextTarget::Memory {
                    compatible_monitor_handle: target.monitor_handle,
                    compatible_rect: target.rect,
                },
                selected_font_id: Some(selected_font_id),
                selected_bitmap_id: Some(selected_bitmap_id),
                selection_generation: 0,
            },
            new_default_objects,
            next_device_context_id: device_context_allocation.next_id,
            next_device_context_handle: device_context_allocation.next_handle,
            next_object_id,
            next_object_handle,
        })
    }

    pub(super) fn commit_create_compatible_dc(
        &mut self,
        plan: RuntimeCreateCompatibleDcPlan,
    ) -> Result<RuntimeCreateCompatibleDcResult, RuntimeStateError> {
        let result = plan.result();
        if self.generation != plan.expected_generation
            || self.next_device_context_id != plan.device_context.id.0
            || self.next_device_context_handle != plan.device_context.handle
            || self
                .device_context_handle_index
                .contains_key(&plan.device_context.handle)
            || self.device_contexts.contains_key(&plan.device_context.id)
        {
            return Err(stale_plan("create-compatible-dc-plan-stale"));
        }
        let mut expected_object_id = self.next_object_id;
        let mut expected_object_handle = self.next_object_handle;
        for object in &plan.new_default_objects {
            let index = default_object_index(object)
                .ok_or_else(|| stale_plan("create-compatible-dc-plan-stale"))?;
            if object.id.0 != expected_object_id
                || object.handle != expected_object_handle
                || !object.shared
                || self.objects.contains_key(&object.id)
                || self.object_handle_index.contains_key(&object.handle)
                || self.stock_object_index.contains_key(&index)
            {
                return Err(stale_plan("create-compatible-dc-plan-stale"));
            }
            expected_object_id =
                checked_next_identity(expected_object_id, "gdi-object-identity-exhausted")?;
            expected_object_handle = expected_object_handle.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-gdi",
                    "gdi-object-handle-exhausted",
                    "logical GDI object handle namespace is exhausted",
                )
            })?;
        }
        if plan.next_object_id != expected_object_id
            || plan.next_object_handle != expected_object_handle
            || !self.plan_resolves_default_object(
                result.selected_font_id,
                &plan.new_default_objects,
                validate_default_font_object,
            )?
            || !self.plan_resolves_default_object(
                result.selected_bitmap_id,
                &plan.new_default_objects,
                validate_default_bitmap_object,
            )?
        {
            return Err(stale_plan("create-compatible-dc-plan-stale"));
        }
        for object in plan.new_default_objects {
            let index = default_object_index(&object)
                .expect("validated compatible DC default object must have a stock index");
            self.stock_object_index.insert(index, object.id);
            self.object_handle_index.insert(object.handle, object.id);
            self.objects.insert(object.id, object);
        }
        self.device_context_handle_index
            .insert(plan.device_context.handle, plan.device_context.id);
        self.device_contexts
            .insert(plan.device_context.id, plan.device_context);
        self.next_device_context_id = plan.next_device_context_id;
        self.next_device_context_handle = plan.next_device_context_handle;
        self.next_object_id = plan.next_object_id;
        self.next_object_handle = plan.next_object_handle;
        self.generation = plan.next_generation;
        Ok(result)
    }

    pub(crate) fn plan_get_graphics_surface_dc(
        &self,
        target: RuntimeGdiGraphicsSurfaceTarget,
    ) -> Result<RuntimeGetGraphicsSurfaceDcPlan, RuntimeStateError> {
        validate_graphics_surface_target(target)?;
        if self
            .graphics_surface_dc_index
            .contains_key(&target.surface_id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "graphics-surface-dc-already-acquired",
                "logical graphics surface already has an outstanding HDC",
            ));
        }
        let allocation = self.plan_device_context_allocation()?;
        Ok(RuntimeGetGraphicsSurfaceDcPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            device_context: RuntimeDeviceContext {
                id: allocation.id,
                handle: allocation.handle,
                namespace: LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE.to_owned(),
                owner_thread_id: target.owner_thread_id,
                target: RuntimeDeviceContextTarget::GraphicsSurface {
                    surface_id: target.surface_id,
                    surface_com_object_id: target.surface_com_object_id,
                    width: target.width,
                    height: target.height,
                    color_format: target.color_format,
                },
                selected_font_id: None,
                selected_bitmap_id: None,
                selection_generation: 0,
            },
            next_device_context_id: allocation.next_id,
            next_device_context_handle: allocation.next_handle,
        })
    }

    pub(crate) fn commit_get_graphics_surface_dc(
        &mut self,
        plan: RuntimeGetGraphicsSurfaceDcPlan,
    ) -> Result<RuntimeGetGraphicsSurfaceDcResult, RuntimeStateError> {
        let result = plan.result();
        if self.generation != plan.expected_generation
            || self.next_device_context_id != plan.device_context.id.0
            || self.next_device_context_handle != plan.device_context.handle
            || self
                .device_context_handle_index
                .contains_key(&plan.device_context.handle)
            || self.device_contexts.contains_key(&plan.device_context.id)
            || self
                .graphics_surface_dc_index
                .contains_key(&result.surface_id)
        {
            return Err(stale_plan("graphics-surface-get-dc-plan-stale"));
        }
        self.graphics_surface_dc_index
            .insert(result.surface_id, plan.device_context.id);
        self.device_context_handle_index
            .insert(plan.device_context.handle, plan.device_context.id);
        self.device_contexts
            .insert(plan.device_context.id, plan.device_context);
        self.next_device_context_id = plan.next_device_context_id;
        self.next_device_context_handle = plan.next_device_context_handle;
        self.generation = plan.next_generation;
        Ok(result)
    }

    pub(super) fn plan_get_stock_object(
        &self,
        index: i32,
    ) -> Result<RuntimeGetStockObjectPlan, RuntimeStateError> {
        if index != RUNTIME_GDI_BLACK_BRUSH_INDEX {
            return Err(RuntimeStateError::invalid_input(
                "runtime-gdi",
                "stock-object-index-unsupported",
                format!("GDI stock object index {index} is outside the reached support band"),
            ));
        }
        if let Some(object_id) = self.stock_object_index.get(&index).copied() {
            let object = self.objects.get(&object_id).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-gdi",
                    "stock-object-index-dangling",
                    "GDI stock object index does not resolve to a live object",
                )
            })?;
            return Ok(RuntimeGetStockObjectPlan {
                expected_generation: self.generation,
                next_generation: self.generation,
                object: object.clone(),
                already_created: true,
                next_object_id: self.next_object_id,
                next_object_handle: self.next_object_handle,
            });
        }
        let allocation = self.plan_object_allocation()?;
        Ok(RuntimeGetStockObjectPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            object: RuntimeGdiObject {
                id: allocation.id,
                handle: allocation.handle,
                namespace: LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE.to_owned(),
                shared: true,
                object: RuntimeGdiObjectKind::StockBrush {
                    index,
                    color_ref: 0,
                },
            },
            already_created: false,
            next_object_id: allocation.next_id,
            next_object_handle: allocation.next_handle,
        })
    }

    pub(super) fn commit_get_stock_object(
        &mut self,
        plan: RuntimeGetStockObjectPlan,
    ) -> Result<RuntimeGetStockObjectResult, RuntimeStateError> {
        if self.generation != plan.expected_generation {
            return Err(stale_plan("get-stock-object-plan-stale"));
        }
        if plan.already_created {
            let live = self
                .objects
                .get(&plan.object.id)
                .ok_or_else(|| stale_plan("get-stock-object-plan-stale"))?;
            if live != &plan.object || self.object_handle_index.get(&live.handle) != Some(&live.id)
            {
                return Err(stale_plan("get-stock-object-plan-stale"));
            }
        } else {
            if self.next_object_id != plan.object.id.0
                || self.next_object_handle != plan.object.handle
                || self.objects.contains_key(&plan.object.id)
                || self.object_handle_index.contains_key(&plan.object.handle)
            {
                return Err(stale_plan("get-stock-object-plan-stale"));
            }
            let RuntimeGdiObjectKind::StockBrush { index, .. } = &plan.object.object else {
                return Err(stale_plan("get-stock-object-plan-stale"));
            };
            if self.stock_object_index.contains_key(index) {
                return Err(stale_plan("get-stock-object-plan-stale"));
            }
            self.stock_object_index.insert(*index, plan.object.id);
            self.object_handle_index
                .insert(plan.object.handle, plan.object.id);
            self.objects.insert(plan.object.id, plan.object.clone());
            self.next_object_id = plan.next_object_id;
            self.next_object_handle = plan.next_object_handle;
            self.generation = plan.next_generation;
        }
        Ok(RuntimeGetStockObjectResult {
            object_id: plan.object.id,
            handle: plan.object.handle,
            already_created: plan.already_created,
        })
    }

    pub(super) fn plan_create_font_a(
        &self,
        log_font: RuntimeLogFontA,
    ) -> Result<RuntimeCreateFontAPlan, RuntimeStateError> {
        validate_log_font_a(&log_font)?;
        let allocation = self.plan_object_allocation()?;
        Ok(RuntimeCreateFontAPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            object: RuntimeGdiObject {
                id: allocation.id,
                handle: allocation.handle,
                namespace: LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE.to_owned(),
                shared: false,
                object: RuntimeGdiObjectKind::FontA { log_font },
            },
            next_object_id: allocation.next_id,
            next_object_handle: allocation.next_handle,
        })
    }

    pub(super) fn commit_create_font_a(
        &mut self,
        plan: RuntimeCreateFontAPlan,
    ) -> Result<RuntimeCreateFontAResult, RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.next_object_id != plan.object.id.0
            || self.next_object_handle != plan.object.handle
            || self.objects.contains_key(&plan.object.id)
            || self.object_handle_index.contains_key(&plan.object.handle)
        {
            return Err(stale_plan("create-font-a-plan-stale"));
        }
        let RuntimeGdiObjectKind::FontA { ref log_font } = plan.object.object else {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "create-font-a-plan-kind-invalid",
                "logical font creation plan does not contain a font object",
            ));
        };
        validate_log_font_a(log_font)?;
        let result = RuntimeCreateFontAResult {
            object_id: plan.object.id,
            handle: plan.object.handle,
        };
        self.object_handle_index
            .insert(plan.object.handle, plan.object.id);
        self.objects.insert(plan.object.id, plan.object);
        self.next_object_id = plan.next_object_id;
        self.next_object_handle = plan.next_object_handle;
        self.generation = plan.next_generation;
        Ok(result)
    }

    pub(super) fn plan_create_dib_section(
        &self,
        device_context_handle: u32,
        thread_id: RuntimeThreadId,
        descriptor: RuntimeGdiDibSectionDescriptor,
        address_allocation_id: ProcessAddressAllocationId,
        backing_range: GuestRange,
    ) -> Result<RuntimeCreateDibSectionPlan, RuntimeStateError> {
        validate_dib_section_descriptor(&descriptor)?;
        validate_dib_section_backing_shape(&descriptor, address_allocation_id, backing_range)?;
        let source_device_context = self
            .device_context_by_handle(device_context_handle)?
            .clone();
        if !matches!(
            source_device_context.target,
            RuntimeDeviceContextTarget::Memory { .. }
        ) || source_device_context.owner_thread_id != thread_id
        {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "dib-section-device-context-relation-mismatch",
                "CreateDIBSection requires a live compatible memory DC owned by the current guest thread",
            ));
        }
        self.validate_memory_dc_selections(&source_device_context)?;
        if self.objects.values().any(|object| {
            matches!(
                &object.object,
                RuntimeGdiObjectKind::DibSection { backing, .. }
                    if backing.address_allocation_id == address_allocation_id
                        || backing.range == backing_range
            )
        }) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "dib-section-backing-collision",
                "logical DIB section allocator selected live guest backing",
            ));
        }
        let allocation = self.plan_object_allocation()?;
        Ok(RuntimeCreateDibSectionPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            source_device_context,
            object_id: allocation.id,
            handle: allocation.handle,
            descriptor,
            address_allocation_id,
            backing_range,
            next_object_id: allocation.next_id,
            next_object_handle: allocation.next_handle,
        })
    }

    pub(super) fn preflight_create_dib_section(
        &self,
        plan: &RuntimeCreateDibSectionPlan,
        memory_owner: MemoryOwnerId,
    ) -> Result<(), RuntimeStateError> {
        if memory_owner.0 == 0
            || self.generation != plan.expected_generation
            || self.next_object_id != plan.object_id.0
            || self.next_object_handle != plan.handle
            || self.objects.contains_key(&plan.object_id)
            || self.object_handle_index.contains_key(&plan.handle)
            || self.device_contexts.get(&plan.source_device_context.id)
                != Some(&plan.source_device_context)
            || self
                .device_context_handle_index
                .get(&plan.source_device_context.handle)
                != Some(&plan.source_device_context.id)
            || self.objects.values().any(|object| {
                matches!(
                    &object.object,
                    RuntimeGdiObjectKind::DibSection { backing, .. }
                        if backing.address_allocation_id == plan.address_allocation_id
                            || backing.range == plan.backing_range
                )
            })
        {
            return Err(stale_plan("create-dib-section-plan-stale"));
        }
        validate_dib_section_descriptor(&plan.descriptor)?;
        validate_dib_section_backing_shape(
            &plan.descriptor,
            plan.address_allocation_id,
            plan.backing_range,
        )?;
        self.validate_memory_dc_selections(&plan.source_device_context)
    }

    pub(super) fn commit_create_dib_section(
        &mut self,
        plan: RuntimeCreateDibSectionPlan,
        memory_owner: MemoryOwnerId,
    ) -> Result<RuntimeCreateDibSectionResult, RuntimeStateError> {
        self.preflight_create_dib_section(&plan, memory_owner)?;
        let backing = RuntimeGdiDibSectionBacking {
            address_allocation_id: plan.address_allocation_id,
            memory_owner,
            range: plan.backing_range,
            content_generation: 0,
        };
        let result = RuntimeCreateDibSectionResult {
            object_id: plan.object_id,
            handle: plan.handle,
            bits_address: plan.backing_range.start(),
            backing,
        };
        let object = RuntimeGdiObject {
            id: plan.object_id,
            handle: plan.handle,
            namespace: LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE.to_owned(),
            shared: false,
            object: RuntimeGdiObjectKind::DibSection {
                descriptor: plan.descriptor,
                backing,
            },
        };
        self.object_handle_index.insert(object.handle, object.id);
        self.objects.insert(object.id, object);
        self.next_object_id = plan.next_object_id;
        self.next_object_handle = plan.next_object_handle;
        self.generation = plan.next_generation;
        Ok(result)
    }

    pub(super) fn plan_select_object(
        &self,
        device_context_handle: u32,
        object_handle: u32,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeSelectGdiObjectPlan, RuntimeStateError> {
        let device_context = self
            .device_context_by_handle(device_context_handle)?
            .clone();
        if !matches!(
            device_context.target,
            RuntimeDeviceContextTarget::Memory { .. }
        ) || device_context.owner_thread_id != thread_id
        {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "select-object-device-context-relation-mismatch",
                "SelectObject requires a compatible memory DC owned by the current guest thread",
            ));
        }
        self.validate_memory_dc_selections(&device_context)?;
        let selected_object = self.object_by_handle(object_handle)?.clone();
        let (slot, previous_id) = match &selected_object.object {
            RuntimeGdiObjectKind::StockFont { .. } | RuntimeGdiObjectKind::FontA { .. } => (
                RuntimeGdiSelectionSlot::Font,
                device_context
                    .selected_font_id
                    .expect("validated memory DC has a selected font"),
            ),
            RuntimeGdiObjectKind::StockBitmap { .. } | RuntimeGdiObjectKind::DibSection { .. } => (
                RuntimeGdiSelectionSlot::Bitmap,
                device_context
                    .selected_bitmap_id
                    .expect("validated memory DC has a selected bitmap"),
            ),
            RuntimeGdiObjectKind::StockBrush { .. } => {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-gdi",
                    "select-object-kind-unsupported",
                    "SelectObject currently supports logical font and bitmap objects",
                ));
            }
        };
        let previous_object = self.objects.get(&previous_id).cloned().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "selected-object-identity-dangling",
                "logical memory DC selection does not resolve to a live object",
            )
        })?;
        let changed = previous_object.id != selected_object.id;
        Ok(RuntimeSelectGdiObjectPlan {
            expected_generation: self.generation,
            next_generation: if changed {
                checked_next_generation(self.generation)?
            } else {
                self.generation
            },
            next_selection_generation: if changed {
                device_context
                    .selection_generation
                    .checked_add(1)
                    .ok_or_else(|| {
                        RuntimeStateError::resource_limit(
                            "runtime-gdi",
                            "selection-generation-exhausted",
                            "logical memory DC selection generation is exhausted",
                        )
                    })?
            } else {
                device_context.selection_generation
            },
            device_context,
            selected_object,
            previous_object,
            slot,
        })
    }

    pub(super) fn commit_select_object(
        &mut self,
        plan: RuntimeSelectGdiObjectPlan,
    ) -> Result<RuntimeSelectGdiObjectResult, RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.device_contexts.get(&plan.device_context.id) != Some(&plan.device_context)
            || self
                .device_context_handle_index
                .get(&plan.device_context.handle)
                != Some(&plan.device_context.id)
            || self.objects.get(&plan.selected_object.id) != Some(&plan.selected_object)
            || self.objects.get(&plan.previous_object.id) != Some(&plan.previous_object)
        {
            return Err(stale_plan("select-object-plan-stale"));
        }
        let previous_id = match plan.slot {
            RuntimeGdiSelectionSlot::Font => plan.device_context.selected_font_id,
            RuntimeGdiSelectionSlot::Bitmap => plan.device_context.selected_bitmap_id,
        };
        if previous_id != Some(plan.previous_object.id) {
            return Err(stale_plan("select-object-plan-stale"));
        }
        let changed = plan.previous_object.id != plan.selected_object.id;
        if changed {
            let device_context = self
                .device_contexts
                .get_mut(&plan.device_context.id)
                .expect("preflighted memory DC disappeared");
            match plan.slot {
                RuntimeGdiSelectionSlot::Font => {
                    device_context.selected_font_id = Some(plan.selected_object.id);
                }
                RuntimeGdiSelectionSlot::Bitmap => {
                    device_context.selected_bitmap_id = Some(plan.selected_object.id);
                }
            }
            device_context.selection_generation = plan.next_selection_generation;
            self.generation = plan.next_generation;
        }
        Ok(RuntimeSelectGdiObjectResult {
            device_context_id: plan.device_context.id,
            selected_object_id: plan.selected_object.id,
            selected_handle: plan.selected_object.handle,
            previous_object_id: plan.previous_object.id,
            previous_handle: plan.previous_object.handle,
            selection_generation: plan.next_selection_generation,
            changed,
        })
    }

    pub(super) fn plan_delete_object(
        &self,
        object_handle: u32,
    ) -> Result<RuntimeDeleteGdiObjectPlan, RuntimeStateError> {
        let object = self.object_by_handle(object_handle)?.clone();
        if object.shared {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "delete-object-shared",
                "shared logical GDI objects cannot be deleted",
            ));
        }
        if self.device_contexts.values().any(|device_context| {
            device_context.selected_font_id == Some(object.id)
                || device_context.selected_bitmap_id == Some(object.id)
        }) {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "delete-object-selected",
                "logical GDI object must be restored out of every DC before deletion",
            ));
        }
        if !matches!(
            &object.object,
            RuntimeGdiObjectKind::FontA { .. } | RuntimeGdiObjectKind::DibSection { .. }
        ) {
            return Err(RuntimeStateError::invalid_input(
                "runtime-gdi",
                "delete-object-kind-unsupported",
                "DeleteObject currently supports logical FontA and DIB section objects",
            ));
        }
        Ok(RuntimeDeleteGdiObjectPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            object,
        })
    }

    pub(super) fn preflight_delete_object(
        &self,
        plan: &RuntimeDeleteGdiObjectPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.objects.get(&plan.object.id) != Some(&plan.object)
            || self.object_handle_index.get(&plan.object.handle) != Some(&plan.object.id)
            || plan.object.shared
            || self.device_contexts.values().any(|device_context| {
                device_context.selected_font_id == Some(plan.object.id)
                    || device_context.selected_bitmap_id == Some(plan.object.id)
            })
        {
            return Err(stale_plan("delete-object-plan-stale"));
        }
        Ok(())
    }

    pub(super) fn commit_delete_object(
        &mut self,
        plan: RuntimeDeleteGdiObjectPlan,
    ) -> Result<RuntimeGdiObject, RuntimeStateError> {
        self.preflight_delete_object(&plan)?;
        self.object_handle_index.remove(&plan.object.handle);
        let object = self
            .objects
            .remove(&plan.object.id)
            .expect("preflighted logical GDI object disappeared");
        self.generation = plan.next_generation;
        Ok(object)
    }

    pub(super) fn object_bytes(
        &self,
        object_handle: u32,
        requested_byte_length: u32,
    ) -> Result<Vec<u8>, RuntimeStateError> {
        let object = self.object_by_handle(object_handle)?;
        match &object.object {
            RuntimeGdiObjectKind::FontA { log_font }
                if requested_byte_length == RUNTIME_LOG_FONT_A_BYTE_LENGTH as u32 =>
            {
                Ok(log_font.to_bytes()?.to_vec())
            }
            RuntimeGdiObjectKind::DibSection {
                descriptor,
                backing,
            } if requested_byte_length == RUNTIME_GDI_BITMAP_BYTE_LENGTH as u32 => {
                Ok(descriptor.bitmap_bytes(backing.range.start())?.to_vec())
            }
            RuntimeGdiObjectKind::StockBrush { .. }
            | RuntimeGdiObjectKind::StockFont { .. }
            | RuntimeGdiObjectKind::StockBitmap { .. }
            | RuntimeGdiObjectKind::FontA { .. }
            | RuntimeGdiObjectKind::DibSection { .. } => Err(RuntimeStateError::invalid_input(
                "runtime-gdi",
                "get-object-a-variant-unsupported",
                "GetObjectA request does not match the reached FontA or DIB BITMAP shape",
            )),
        }
    }

    pub(super) fn selected_font_a(
        &self,
        device_context_handle: u32,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeLogFontA, RuntimeStateError> {
        let device_context = self.device_context_by_handle(device_context_handle)?;
        if !matches!(
            device_context.target,
            RuntimeDeviceContextTarget::Memory { .. }
        ) || device_context.owner_thread_id != thread_id
        {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "outline-text-metrics-device-context-relation-mismatch",
                "GetOutlineTextMetricsA requires a compatible memory DC owned by the current guest thread",
            ));
        }
        self.validate_memory_dc_selections(device_context)?;
        let font_id = device_context
            .selected_font_id
            .expect("validated memory DC has a selected font");
        let font = self.objects.get(&font_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "outline-text-metrics-selected-font-dangling",
                "logical memory DC selected font does not resolve to a live object",
            )
        })?;
        match &font.object {
            RuntimeGdiObjectKind::FontA { log_font } => Ok(log_font.clone()),
            RuntimeGdiObjectKind::StockFont { .. } => Err(RuntimeStateError::invalid_input(
                "runtime-gdi",
                "outline-text-metrics-selected-font-unsupported",
                "GetOutlineTextMetricsA currently supports the evidenced created FontA selection",
            )),
            RuntimeGdiObjectKind::StockBrush { .. }
            | RuntimeGdiObjectKind::StockBitmap { .. }
            | RuntimeGdiObjectKind::DibSection { .. } => Err(RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "outline-text-metrics-selected-object-kind-invalid",
                "logical memory DC font selection resolved to a non-font object",
            )),
        }
    }

    pub(super) fn text_raster_target(
        &self,
        device_context_handle: u32,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeGdiTextRasterTarget, RuntimeStateError> {
        let device_context = self.device_context_by_handle(device_context_handle)?;
        if !matches!(
            device_context.target,
            RuntimeDeviceContextTarget::Memory { .. }
        ) || device_context.owner_thread_id != thread_id
        {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "text-raster-device-context-relation-mismatch",
                "text rasterization requires a compatible memory DC owned by the current guest thread",
            ));
        }
        self.validate_memory_dc_selections(device_context)?;
        let font_id = device_context
            .selected_font_id
            .expect("validated memory DC has a selected font");
        let bitmap_id = device_context
            .selected_bitmap_id
            .expect("validated memory DC has a selected bitmap");
        let font = self.objects.get(&font_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "text-raster-selected-font-dangling",
                "logical memory DC selected font does not resolve to a live object",
            )
        })?;
        let bitmap = self.objects.get(&bitmap_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "text-raster-selected-bitmap-dangling",
                "logical memory DC selected bitmap does not resolve to a live object",
            )
        })?;
        let RuntimeGdiObjectKind::FontA { log_font } = &font.object else {
            return Err(RuntimeStateError::invalid_input(
                "runtime-gdi",
                "text-raster-selected-font-unsupported",
                "text rasterization requires an explicitly created FontA object",
            ));
        };
        let RuntimeGdiObjectKind::DibSection {
            descriptor,
            backing,
        } = &bitmap.object
        else {
            return Err(RuntimeStateError::invalid_input(
                "runtime-gdi",
                "text-raster-selected-bitmap-unsupported",
                "text rasterization requires a selected DIB section",
            ));
        };
        Ok(RuntimeGdiTextRasterTarget {
            device_context_id: device_context.id,
            device_context_handle: device_context.handle,
            owner_thread_id: device_context.owner_thread_id,
            selection_generation: device_context.selection_generation,
            font_id: font.id,
            font_handle: font.handle,
            log_font: log_font.clone(),
            dib_section_id: bitmap.id,
            dib_section_handle: bitmap.handle,
            descriptor: descriptor.clone(),
            backing: *backing,
        })
    }

    pub(super) fn plan_text_raster_commit(
        &self,
        expected_target: RuntimeGdiTextRasterTarget,
    ) -> Result<RuntimeGdiTextRasterPlan, RuntimeStateError> {
        let current = self.text_raster_target(
            expected_target.device_context_handle,
            expected_target.owner_thread_id,
        )?;
        if current != expected_target {
            return Err(stale_plan("text-raster-plan-stale"));
        }
        let next_content_generation = expected_target
            .backing
            .content_generation
            .checked_add(1)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-gdi",
                    "text-raster-content-generation-exhausted",
                    "logical DIB text content generation is exhausted",
                )
            })?;
        Ok(RuntimeGdiTextRasterPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            target: expected_target,
            next_content_generation,
        })
    }

    pub(super) fn preflight_text_raster_commit(
        &self,
        plan: &RuntimeGdiTextRasterPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.text_raster_target(
                plan.target.device_context_handle,
                plan.target.owner_thread_id,
            )? != plan.target
        {
            return Err(stale_plan("text-raster-plan-stale"));
        }
        Ok(())
    }

    pub(super) fn commit_text_raster(
        &mut self,
        plan: RuntimeGdiTextRasterPlan,
    ) -> Result<RuntimeGdiTextRasterResult, RuntimeStateError> {
        self.preflight_text_raster_commit(&plan)?;
        let bitmap = self
            .objects
            .get_mut(&plan.target.dib_section_id)
            .expect("preflighted DIB section disappeared");
        let RuntimeGdiObjectKind::DibSection { backing, .. } = &mut bitmap.object else {
            unreachable!("preflighted text raster bitmap changed kind")
        };
        backing.content_generation = plan.next_content_generation;
        self.generation = plan.next_generation;
        Ok(RuntimeGdiTextRasterResult {
            device_context_id: plan.target.device_context_id,
            font_id: plan.target.font_id,
            dib_section_id: plan.target.dib_section_id,
            selection_generation: plan.target.selection_generation,
            content_generation: plan.next_content_generation,
        })
    }

    pub(super) fn plan_release_dc(
        &self,
        window_handle: u32,
        device_context_handle: u32,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeReleaseDcPlan, RuntimeStateError> {
        let device_context = self
            .device_context_by_handle(device_context_handle)?
            .clone();
        if !matches!(
            device_context.target,
            RuntimeDeviceContextTarget::Window {
                window_handle: acquired_window_handle,
                ..
            } if acquired_window_handle == window_handle
        ) || device_context.owner_thread_id != thread_id
        {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "device-context-relation-mismatch",
                "ReleaseDC requires the exact HWND, HDC, and acquiring guest thread relation",
            ));
        }
        Ok(RuntimeReleaseDcPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            device_context,
        })
    }

    pub(super) fn plan_release_graphics_surface_dc(
        &self,
        target: RuntimeGdiGraphicsSurfaceTarget,
        device_context_handle: u32,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeReleaseDcPlan, RuntimeStateError> {
        validate_graphics_surface_target(target)?;
        let device_context = self
            .device_context_by_handle(device_context_handle)?
            .clone();
        if device_context.owner_thread_id != thread_id
            || device_context.target
                != (RuntimeDeviceContextTarget::GraphicsSurface {
                    surface_id: target.surface_id,
                    surface_com_object_id: target.surface_com_object_id,
                    width: target.width,
                    height: target.height,
                    color_format: target.color_format,
                })
            || self.graphics_surface_dc_index.get(&target.surface_id) != Some(&device_context.id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "graphics-surface-dc-relation-mismatch",
                "ReleaseDC requires the exact graphics surface, HDC, and acquiring guest thread relation",
            ));
        }
        Ok(RuntimeReleaseDcPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            device_context,
        })
    }

    pub(super) fn plan_release_screen_dc(
        &self,
        target: RuntimeGdiScreenTarget,
        device_context_handle: u32,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeReleaseDcPlan, RuntimeStateError> {
        let device_context =
            self.validate_screen_dc_access(target, device_context_handle, thread_id)?;
        Ok(RuntimeReleaseDcPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            device_context,
        })
    }

    pub(super) fn plan_delete_compatible_dc(
        &self,
        device_context_handle: u32,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeReleaseDcPlan, RuntimeStateError> {
        let device_context = self
            .device_context_by_handle(device_context_handle)?
            .clone();
        if !matches!(
            device_context.target,
            RuntimeDeviceContextTarget::Memory { .. }
        ) || device_context.owner_thread_id != thread_id
        {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "delete-dc-relation-mismatch",
                "DeleteDC requires a compatible memory DC owned by the current guest thread",
            ));
        }
        self.validate_memory_dc_selections(&device_context)?;
        Ok(RuntimeReleaseDcPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            device_context,
        })
    }

    pub(super) fn validate_screen_dc_access(
        &self,
        target: RuntimeGdiScreenTarget,
        device_context_handle: u32,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeDeviceContext, RuntimeStateError> {
        validate_screen_target(target)?;
        let device_context = self
            .device_context_by_handle(device_context_handle)?
            .clone();
        if target.owner_thread_id != thread_id
            || device_context.owner_thread_id != thread_id
            || device_context.target
                != (RuntimeDeviceContextTarget::Screen {
                    monitor_handle: target.monitor_handle,
                    rect: target.rect,
                })
        {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "screen-device-context-relation-mismatch",
                "screen GDI access requires the exact HDC, primary monitor, and acquiring guest thread relation",
            ));
        }
        Ok(device_context)
    }

    pub(super) fn commit_release_dc(
        &mut self,
        plan: RuntimeReleaseDcPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.device_contexts.get(&plan.device_context.id) != Some(&plan.device_context)
            || self
                .device_context_handle_index
                .get(&plan.device_context.handle)
                != Some(&plan.device_context.id)
        {
            return Err(stale_plan("release-dc-plan-stale"));
        }
        if let RuntimeDeviceContextTarget::GraphicsSurface { surface_id, .. } =
            plan.device_context.target
        {
            if self.graphics_surface_dc_index.get(&surface_id) != Some(&plan.device_context.id) {
                return Err(stale_plan("release-dc-plan-stale"));
            }
            self.graphics_surface_dc_index.remove(&surface_id);
        }
        self.device_contexts.remove(&plan.device_context.id);
        self.device_context_handle_index
            .remove(&plan.device_context.handle);
        self.generation = plan.next_generation;
        Ok(())
    }

    pub(super) fn plan_fill_rect(
        &self,
        device_context_handle: u32,
        rect: RuntimeRect,
        brush_handle: u32,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeFillRectPlan, RuntimeStateError> {
        let device_context = self
            .device_context_by_handle(device_context_handle)?
            .clone();
        if device_context.owner_thread_id != thread_id {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "device-context-thread-mismatch",
                "FillRect requires the guest thread that acquired the logical HDC",
            ));
        }
        let RuntimeDeviceContextTarget::Window { surface_id, .. } = device_context.target else {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "fill-rect-target-mismatch",
                "window FillRect planning received a non-window HDC",
            ));
        };
        let brush_id = self
            .object_handle_index
            .get(&brush_handle)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-gdi",
                    "gdi-object-handle-unknown",
                    format!("logical GDI object {brush_handle:#010x} is not live"),
                )
            })?;
        let brush = self.objects.get(&brush_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "gdi-object-handle-index-dangling",
                "logical GDI object handle index does not resolve to a live object",
            )
        })?;
        let RuntimeGdiObjectKind::StockBrush { color_ref, .. } = &brush.object else {
            return Err(RuntimeStateError::invalid_input(
                "runtime-gdi",
                "fill-rect-object-not-brush",
                "FillRect requires a logical brush object",
            ));
        };
        let surface = self.surfaces.get(&surface_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "device-context-surface-dangling",
                "logical HDC does not resolve to its window surface",
            )
        })?;
        validate_fill_rect(rect, surface)?;
        let empty = rect.left == rect.right || rect.top == rect.bottom;
        if !empty && surface.fills.len() >= RUNTIME_GDI_SURFACE_FILL_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-gdi",
                "surface-fill-capacity-exhausted",
                "logical GDI surface fill history capacity is exhausted",
            ));
        }
        let fill = (!empty).then_some(RuntimeGdiSurfaceFill {
            sequence: self.next_fill_sequence,
            rect,
            brush_id,
            color_ref: *color_ref,
        });
        let (next_generation, next_surface_generation, next_fill_sequence) = if fill.is_some() {
            (
                checked_next_generation(self.generation)?,
                surface.generation.checked_add(1).ok_or_else(|| {
                    RuntimeStateError::resource_limit(
                        "runtime-gdi",
                        "surface-generation-exhausted",
                        "logical GDI surface generation is exhausted",
                    )
                })?,
                checked_next_identity(self.next_fill_sequence, "surface-fill-sequence-exhausted")?,
            )
        } else {
            (self.generation, surface.generation, self.next_fill_sequence)
        };
        Ok(RuntimeFillRectPlan {
            expected_generation: self.generation,
            next_generation,
            device_context,
            brush_id,
            surface_generation: surface.generation,
            next_surface_generation,
            fill,
            next_fill_sequence,
        })
    }

    pub(super) fn commit_fill_rect(
        &mut self,
        plan: RuntimeFillRectPlan,
    ) -> Result<RuntimeFillRectResult, RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.device_contexts.get(&plan.device_context.id) != Some(&plan.device_context)
            || !self.objects.contains_key(&plan.brush_id)
        {
            return Err(stale_plan("fill-rect-plan-stale"));
        }
        let surface = self
            .surfaces
            .get_mut(&match plan.device_context.target {
                RuntimeDeviceContextTarget::Window { surface_id, .. } => surface_id,
                RuntimeDeviceContextTarget::Screen { .. }
                | RuntimeDeviceContextTarget::Memory { .. }
                | RuntimeDeviceContextTarget::GraphicsSurface { .. } => {
                    return Err(stale_plan("fill-rect-plan-stale"));
                }
            })
            .ok_or_else(|| stale_plan("fill-rect-plan-stale"))?;
        if surface.generation != plan.surface_generation {
            return Err(stale_plan("fill-rect-plan-stale"));
        }
        if let Some(fill) = plan.fill {
            if fill.sequence != self.next_fill_sequence
                || surface.fills.len() >= RUNTIME_GDI_SURFACE_FILL_LIMIT
                || surface.generation.checked_add(1) != Some(plan.next_surface_generation)
            {
                return Err(stale_plan("fill-rect-plan-stale"));
            }
            surface.fills.push(fill);
            surface.generation = plan.next_surface_generation;
            self.next_fill_sequence = plan.next_fill_sequence;
            self.generation = plan.next_generation;
        }
        Ok(RuntimeFillRectResult {
            surface_id: surface.id,
            surface_generation: surface.generation,
            fill: plan.fill,
        })
    }

    pub(super) fn plan_graphics_surface_fill_rect(
        &self,
        device_context_handle: u32,
        rect: RuntimeRect,
        brush_handle: u32,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeGraphicsSurfaceFillRectPlan, RuntimeStateError> {
        let device_context = self
            .device_context_by_handle(device_context_handle)?
            .clone();
        if device_context.owner_thread_id != thread_id {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "device-context-thread-mismatch",
                "FillRect requires the guest thread that acquired the logical HDC",
            ));
        }
        let RuntimeDeviceContextTarget::GraphicsSurface { width, height, .. } =
            device_context.target
        else {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "fill-rect-target-mismatch",
                "graphics FillRect planning received a window HDC",
            ));
        };
        let brush_id = self
            .object_handle_index
            .get(&brush_handle)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-gdi",
                    "gdi-object-handle-unknown",
                    format!("logical GDI object {brush_handle:#010x} is not live"),
                )
            })?;
        let brush = self.objects.get(&brush_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "gdi-object-handle-index-dangling",
                "logical GDI object handle index does not resolve to a live object",
            )
        })?;
        let RuntimeGdiObjectKind::StockBrush { color_ref, .. } = &brush.object else {
            return Err(RuntimeStateError::invalid_input(
                "runtime-gdi",
                "fill-rect-object-not-brush",
                "FillRect requires a logical brush object",
            ));
        };
        validate_fill_rect_bounds(rect, width, height)?;
        Ok(RuntimeGraphicsSurfaceFillRectPlan {
            expected_generation: self.generation,
            device_context,
            brush_id,
            rect,
            color_ref: *color_ref,
        })
    }

    pub(super) fn preflight_graphics_surface_fill_rect(
        &self,
        plan: &RuntimeGraphicsSurfaceFillRectPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.device_contexts.get(&plan.device_context.id) != Some(&plan.device_context)
            || self
                .device_context_handle_index
                .get(&plan.device_context.handle)
                != Some(&plan.device_context.id)
            || !self.objects.contains_key(&plan.brush_id)
        {
            return Err(stale_plan("graphics-fill-rect-plan-stale"));
        }
        let RuntimeDeviceContextTarget::GraphicsSurface { width, height, .. } =
            plan.device_context.target
        else {
            return Err(stale_plan("graphics-fill-rect-plan-stale"));
        };
        validate_fill_rect_bounds(plan.rect, width, height)
    }

    pub fn device_context_by_handle(
        &self,
        handle: u32,
    ) -> Result<&RuntimeDeviceContext, RuntimeStateError> {
        let id = self
            .device_context_handle_index
            .get(&handle)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-gdi",
                    "device-context-handle-unknown",
                    format!("logical HDC {handle:#010x} is not live"),
                )
            })?;
        self.device_contexts.get(&id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "device-context-handle-index-dangling",
                "logical HDC index does not resolve to a live device context",
            )
        })
    }

    pub fn object_by_handle(&self, handle: u32) -> Result<&RuntimeGdiObject, RuntimeStateError> {
        let id = self
            .object_handle_index
            .get(&handle)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-gdi",
                    "gdi-object-handle-unknown",
                    format!("logical GDI object {handle:#010x} is not live"),
                )
            })?;
        self.objects.get(&id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "gdi-object-handle-index-dangling",
                "logical GDI object handle index does not resolve to a live object",
            )
        })
    }

    pub fn device_contexts(&self) -> &BTreeMap<RuntimeDeviceContextId, RuntimeDeviceContext> {
        &self.device_contexts
    }

    pub fn objects(&self) -> &BTreeMap<RuntimeGdiObjectId, RuntimeGdiObject> {
        &self.objects
    }

    pub fn surfaces(&self) -> &BTreeMap<RuntimeGdiSurfaceId, RuntimeGdiSurface> {
        &self.surfaces
    }

    pub(crate) fn plan_com_object_retirements(
        &self,
        com_object_ids: &BTreeSet<RuntimeComObjectId>,
    ) -> Result<Option<RuntimeGdiComRetirementGuard>, RuntimeStateError> {
        if com_object_ids.is_empty() {
            return Ok(None);
        }
        self.reject_live_graphics_surface_dc_retirements(com_object_ids)?;
        Ok(Some(RuntimeGdiComRetirementGuard {
            expected_generation: self.generation,
            retired_graphics_surface_objects: com_object_ids.clone(),
        }))
    }

    pub(crate) fn preflight_com_object_retirements(
        &self,
        guard: &RuntimeGdiComRetirementGuard,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != guard.expected_generation {
            return Err(stale_plan("gdi-com-retirement-guard-stale"));
        }
        self.reject_live_graphics_surface_dc_retirements(&guard.retired_graphics_surface_objects)
    }

    fn reject_live_graphics_surface_dc_retirements(
        &self,
        com_object_ids: &BTreeSet<RuntimeComObjectId>,
    ) -> Result<(), RuntimeStateError> {
        if self.device_contexts.values().any(|device_context| {
            matches!(
                device_context.target,
                RuntimeDeviceContextTarget::GraphicsSurface {
                    surface_com_object_id,
                    ..
                } if com_object_ids.contains(&surface_com_object_id)
            )
        }) {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "graphics-surface-dc-still-acquired",
                "logical graphics surface cannot retire while its HDC is outstanding",
            ));
        }
        Ok(())
    }

    pub(super) fn plan_process_teardown(
        &self,
    ) -> Result<Option<RuntimeGdiTeardownPlan>, RuntimeStateError> {
        if self.is_empty() {
            return Ok(None);
        }
        Ok(Some(RuntimeGdiTeardownPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            device_contexts: self.device_contexts.len(),
            objects: self.objects.len(),
            surfaces: self.surfaces.len(),
        }))
    }

    pub(super) fn preflight_process_teardown(
        &self,
        plan: &RuntimeGdiTeardownPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.device_contexts.len() != plan.device_contexts
            || self.objects.len() != plan.objects
            || self.surfaces.len() != plan.surfaces
        {
            return Err(stale_plan("gdi-teardown-plan-stale"));
        }
        Ok(())
    }

    pub(super) fn commit_process_teardown(&mut self, plan: RuntimeGdiTeardownPlan) {
        self.device_contexts.clear();
        self.device_context_handle_index.clear();
        self.objects.clear();
        self.object_handle_index.clear();
        self.stock_object_index.clear();
        self.surfaces.clear();
        self.window_surface_index.clear();
        self.graphics_surface_dc_index.clear();
        self.generation = plan.next_generation;
    }

    pub(super) fn is_empty(&self) -> bool {
        self.device_contexts.is_empty() && self.objects.is_empty() && self.surfaces.is_empty()
    }

    fn stock_object_by_index(
        &self,
        index: i32,
    ) -> Result<Option<&RuntimeGdiObject>, RuntimeStateError> {
        let Some(object_id) = self.stock_object_index.get(&index) else {
            return Ok(None);
        };
        self.objects.get(object_id).map(Some).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "stock-object-index-dangling",
                "GDI stock object index does not resolve to a live object",
            )
        })
    }

    fn plan_resolves_default_object(
        &self,
        object_id: RuntimeGdiObjectId,
        new_objects: &[RuntimeGdiObject],
        validate: fn(&RuntimeGdiObject) -> Result<(), RuntimeStateError>,
    ) -> Result<bool, RuntimeStateError> {
        let object = new_objects
            .iter()
            .find(|object| object.id == object_id)
            .or_else(|| self.objects.get(&object_id));
        let Some(object) = object else {
            return Ok(false);
        };
        validate(object)?;
        Ok(true)
    }

    fn validate_memory_dc_selections(
        &self,
        device_context: &RuntimeDeviceContext,
    ) -> Result<(), RuntimeStateError> {
        let Some(font_id) = device_context.selected_font_id else {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "memory-dc-font-selection-missing",
                "logical memory DC has no selected font identity",
            ));
        };
        let Some(bitmap_id) = device_context.selected_bitmap_id else {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "memory-dc-bitmap-selection-missing",
                "logical memory DC has no selected bitmap identity",
            ));
        };
        let font = self.objects.get(&font_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "memory-dc-font-selection-dangling",
                "logical memory DC selected font does not resolve to a live object",
            )
        })?;
        validate_selectable_font_object(font)?;
        let bitmap = self.objects.get(&bitmap_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "memory-dc-bitmap-selection-dangling",
                "logical memory DC selected bitmap does not resolve to a live object",
            )
        })?;
        validate_selectable_bitmap_object(bitmap)
    }

    fn plan_object_allocation(&self) -> Result<RuntimeGdiObjectAllocation, RuntimeStateError> {
        self.plan_object_allocations(1)?
            .into_iter()
            .next()
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-gdi",
                    "gdi-object-allocation-missing",
                    "single logical GDI object allocation plan is empty",
                )
            })
    }

    fn plan_object_allocations(
        &self,
        count: usize,
    ) -> Result<Vec<RuntimeGdiObjectAllocation>, RuntimeStateError> {
        if count == 0 {
            return Ok(Vec::new());
        }
        if self
            .objects
            .len()
            .checked_add(count)
            .is_none_or(|length| length > RUNTIME_GDI_OBJECT_LIMIT)
        {
            return Err(RuntimeStateError::resource_limit(
                "runtime-gdi",
                "gdi-object-capacity-exhausted",
                "logical GDI object capacity is exhausted",
            ));
        }
        let mut allocations = Vec::with_capacity(count);
        let mut next_id = self.next_object_id;
        let mut next_handle = self.next_object_handle;
        for _ in 0..count {
            let id = RuntimeGdiObjectId(next_id);
            let handle = next_handle;
            if self.objects.contains_key(&id)
                || handle < RUNTIME_GDI_OBJECT_HANDLE_BASE
                || self.object_handle_index.contains_key(&handle)
            {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-gdi",
                    "gdi-object-handle-allocator-invalid",
                    "logical GDI object allocator is not at a free identity and handle pair",
                ));
            }
            next_id = checked_next_identity(next_id, "gdi-object-identity-exhausted")?;
            next_handle = handle.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-gdi",
                    "gdi-object-handle-exhausted",
                    "logical GDI object handle namespace is exhausted",
                )
            })?;
            allocations.push(RuntimeGdiObjectAllocation {
                id,
                handle,
                next_id,
                next_handle,
            });
        }
        Ok(allocations)
    }

    fn plan_device_context_allocation(
        &self,
    ) -> Result<RuntimeDeviceContextAllocation, RuntimeStateError> {
        if self.device_contexts.len() >= RUNTIME_GDI_DEVICE_CONTEXT_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-gdi",
                "device-context-capacity-exhausted",
                "logical device-context capacity is exhausted",
            ));
        }
        let id = RuntimeDeviceContextId(self.next_device_context_id);
        let next_id = checked_next_identity(
            self.next_device_context_id,
            "device-context-identity-exhausted",
        )?;
        let handle = self.next_device_context_handle;
        if handle < RUNTIME_DEVICE_CONTEXT_HANDLE_BASE
            || self.device_context_handle_index.contains_key(&handle)
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "device-context-handle-allocator-invalid",
                "logical device-context handle allocator is not at a free namespace value",
            ));
        }
        let next_handle = handle.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-gdi",
                "device-context-handle-exhausted",
                "logical device-context handle namespace is exhausted",
            )
        })?;
        Ok(RuntimeDeviceContextAllocation {
            id,
            handle,
            next_id,
            next_handle,
        })
    }
}

fn default_object_index(object: &RuntimeGdiObject) -> Option<i32> {
    match object.object {
        RuntimeGdiObjectKind::StockFont {
            index: RUNTIME_GDI_SYSTEM_FONT_INDEX,
        } => Some(RUNTIME_GDI_SYSTEM_FONT_INDEX),
        RuntimeGdiObjectKind::StockBitmap {
            index: RUNTIME_GDI_DEFAULT_BITMAP_INDEX,
            width: 1,
            height: 1,
            bits_per_pixel: 1,
        } => Some(RUNTIME_GDI_DEFAULT_BITMAP_INDEX),
        RuntimeGdiObjectKind::StockBrush { .. }
        | RuntimeGdiObjectKind::StockFont { .. }
        | RuntimeGdiObjectKind::StockBitmap { .. }
        | RuntimeGdiObjectKind::FontA { .. }
        | RuntimeGdiObjectKind::DibSection { .. } => None,
    }
}

fn validate_default_font_object(object: &RuntimeGdiObject) -> Result<(), RuntimeStateError> {
    if !object.shared
        || object.namespace != LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE
        || !matches!(
            object.object,
            RuntimeGdiObjectKind::StockFont {
                index: RUNTIME_GDI_SYSTEM_FONT_INDEX,
            }
        )
    {
        return Err(RuntimeStateError::runtime_defect(
            "runtime-gdi",
            "memory-dc-default-font-invalid",
            "logical memory DC default font identity is not the shared system font",
        ));
    }
    Ok(())
}

fn validate_default_bitmap_object(object: &RuntimeGdiObject) -> Result<(), RuntimeStateError> {
    if !object.shared
        || object.namespace != LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE
        || !matches!(
            object.object,
            RuntimeGdiObjectKind::StockBitmap {
                index: RUNTIME_GDI_DEFAULT_BITMAP_INDEX,
                width: 1,
                height: 1,
                bits_per_pixel: 1,
            }
        )
    {
        return Err(RuntimeStateError::runtime_defect(
            "runtime-gdi",
            "memory-dc-default-bitmap-invalid",
            "logical memory DC default bitmap identity is not the shared 1x1 monochrome bitmap",
        ));
    }
    Ok(())
}

fn validate_selectable_font_object(object: &RuntimeGdiObject) -> Result<(), RuntimeStateError> {
    match &object.object {
        RuntimeGdiObjectKind::StockFont { .. } => validate_default_font_object(object),
        RuntimeGdiObjectKind::FontA { log_font } if !object.shared => validate_log_font_a(log_font),
        RuntimeGdiObjectKind::StockBrush { .. }
        | RuntimeGdiObjectKind::StockBitmap { .. }
        | RuntimeGdiObjectKind::DibSection { .. }
        | RuntimeGdiObjectKind::FontA { .. } => Err(RuntimeStateError::runtime_defect(
            "runtime-gdi",
            "memory-dc-font-selection-invalid",
            "logical memory DC selected font identity is not a supported font object",
        )),
    }
}

fn validate_selectable_bitmap_object(object: &RuntimeGdiObject) -> Result<(), RuntimeStateError> {
    match &object.object {
        RuntimeGdiObjectKind::StockBitmap { .. } => validate_default_bitmap_object(object),
        RuntimeGdiObjectKind::DibSection {
            descriptor,
            backing,
        } if !object.shared => {
            validate_dib_section_descriptor(descriptor)?;
            validate_dib_section_backing_shape(
                descriptor,
                backing.address_allocation_id,
                backing.range,
            )?;
            if backing.memory_owner.0 == 0 {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-gdi",
                    "dib-section-memory-owner-invalid",
                    "logical DIB section backing has no guest-memory owner",
                ));
            }
            Ok(())
        }
        RuntimeGdiObjectKind::StockBrush { .. }
        | RuntimeGdiObjectKind::StockFont { .. }
        | RuntimeGdiObjectKind::FontA { .. }
        | RuntimeGdiObjectKind::DibSection { .. } => Err(RuntimeStateError::runtime_defect(
            "runtime-gdi",
            "memory-dc-bitmap-selection-invalid",
            "logical memory DC selected bitmap identity is not a supported bitmap object",
        )),
    }
}

fn validate_dib_section_descriptor(
    descriptor: &RuntimeGdiDibSectionDescriptor,
) -> Result<(), RuntimeStateError> {
    if descriptor.width <= 0
        || descriptor.height == 0
        || descriptor.height == i32::MIN
        || descriptor.planes != 1
        || descriptor.bits_per_pixel != 8
        || descriptor.compression != RUNTIME_GDI_DIB_BI_RGB
        || descriptor.palette.len() != 256
        || descriptor.palette.iter().any(|color| color.reserved != 0)
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-gdi",
            "dib-section-descriptor-invalid",
            "logical DIB section requires a nonempty 8bpp BI_RGB shape and 256 RGBQUAD colors",
        ));
    }
    let width = u32::try_from(descriptor.width).map_err(|_| {
        RuntimeStateError::invalid_input(
            "runtime-gdi",
            "dib-section-width-invalid",
            "logical DIB section width must be a positive x86 integer",
        )
    })?;
    let height = u32::try_from(descriptor.height.abs()).map_err(|_| {
        RuntimeStateError::invalid_input(
            "runtime-gdi",
            "dib-section-height-invalid",
            "logical DIB section height must fit unsigned x86 pixel accounting",
        )
    })?;
    let stride_bytes = width
        .checked_mul(u32::from(descriptor.bits_per_pixel))
        .and_then(|bits| bits.checked_add(31))
        .map(|bits| bits / 32)
        .and_then(|words| words.checked_mul(4))
        .ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-gdi",
                "dib-section-stride-overflow",
                "logical DIB section stride overflowed x86 accounting",
            )
        })?;
    let pixel_byte_length = stride_bytes.checked_mul(height).ok_or_else(|| {
        RuntimeStateError::resource_limit(
            "runtime-gdi",
            "dib-section-pixel-length-overflow",
            "logical DIB section pixel byte length overflowed x86 accounting",
        )
    })?;
    if descriptor.stride_bytes != stride_bytes
        || descriptor.pixel_byte_length != pixel_byte_length
        || pixel_byte_length == 0
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-gdi",
            "dib-section-derived-layout-invalid",
            "logical DIB section stride or pixel byte length does not match its descriptor",
        ));
    }
    Ok(())
}

fn validate_dib_section_backing_shape(
    descriptor: &RuntimeGdiDibSectionDescriptor,
    address_allocation_id: ProcessAddressAllocationId,
    backing_range: GuestRange,
) -> Result<(), RuntimeStateError> {
    if address_allocation_id.0 == 0
        || backing_range.start().0 == 0
        || !backing_range
            .start()
            .0
            .is_multiple_of(crate::memory::GUEST_PAGE_SIZE as u32)
        || !backing_range
            .byte_length()
            .is_multiple_of(crate::memory::GUEST_PAGE_SIZE)
        || backing_range.byte_length() < u64::from(descriptor.pixel_byte_length)
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-gdi",
            "dib-section-backing-invalid",
            "logical DIB section backing must be allocation-owned, page-aligned, and large enough",
        ));
    }
    Ok(())
}

fn validate_log_font_a(log_font: &RuntimeLogFontA) -> Result<(), RuntimeStateError> {
    if log_font.face_name_windows_932.len() >= RUNTIME_LOG_FONT_A_FACE_CAPACITY
        || log_font.face_name_windows_932.contains(&0)
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-gdi",
            "log-font-a-face-name-invalid",
            "logical LOGFONTA face name must fit LF_FACESIZE with one trailing NUL",
        ));
    }
    Ok(())
}

fn validate_window_target(target: RuntimeGdiWindowTarget) -> Result<(), RuntimeStateError> {
    if target.window_handle == 0
        || target.owner_thread_id.0 == 0
        || target.client_width == 0
        || target.client_height == 0
        || i32::try_from(target.client_width).is_err()
        || i32::try_from(target.client_height).is_err()
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-gdi",
            "window-dc-target-invalid",
            "logical window DC target requires nonzero identity and x86 RECT-sized client geometry",
        ));
    }
    Ok(())
}

fn validate_screen_target(target: RuntimeGdiScreenTarget) -> Result<(), RuntimeStateError> {
    if target.monitor_handle == 0
        || target.owner_thread_id.0 == 0
        || target.rect.left != 0
        || target.rect.top != 0
        || target.rect.right <= target.rect.left
        || target.rect.bottom <= target.rect.top
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-gdi",
            "screen-dc-target-invalid",
            "logical screen DC target requires a live monitor, guest thread, and positive origin-based rectangle",
        ));
    }
    Ok(())
}

fn validate_graphics_surface_target(
    target: RuntimeGdiGraphicsSurfaceTarget,
) -> Result<(), RuntimeStateError> {
    if target.surface_id.0 == 0
        || target.surface_com_object_id.0 == 0
        || target.owner_thread_id.0 == 0
        || target.width == 0
        || target.height == 0
        || i32::try_from(target.width).is_err()
        || i32::try_from(target.height).is_err()
        || target.color_format != RuntimeGraphicsColorFormat::B8g8r8x8Unorm
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-gdi",
            "graphics-surface-dc-target-invalid",
            "logical graphics surface DC target requires live identity and supported x86 geometry and color format",
        ));
    }
    Ok(())
}

fn validate_fill_rect(
    rect: RuntimeRect,
    surface: &RuntimeGdiSurface,
) -> Result<(), RuntimeStateError> {
    validate_fill_rect_bounds(rect, surface.width, surface.height)
}

fn validate_fill_rect_bounds(
    rect: RuntimeRect,
    surface_width: u32,
    surface_height: u32,
) -> Result<(), RuntimeStateError> {
    let width = i32::try_from(surface_width).map_err(|_| {
        RuntimeStateError::runtime_defect(
            "runtime-gdi",
            "surface-width-invalid",
            "logical GDI surface width does not fit x86 coordinates",
        )
    })?;
    let height = i32::try_from(surface_height).map_err(|_| {
        RuntimeStateError::runtime_defect(
            "runtime-gdi",
            "surface-height-invalid",
            "logical GDI surface height does not fit x86 coordinates",
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
            "runtime-gdi",
            "fill-rect-outside-supported-surface",
            "bounded FillRect requires an ordered rectangle inside the logical client surface",
        ));
    }
    Ok(())
}

fn checked_next_identity(value: u64, code: &'static str) -> Result<u64, RuntimeStateError> {
    value.checked_add(1).ok_or_else(|| {
        RuntimeStateError::resource_limit(
            "runtime-gdi",
            code,
            "logical GDI identity sequence is exhausted",
        )
    })
}

fn checked_next_generation(value: u64) -> Result<u64, RuntimeStateError> {
    value.checked_add(1).ok_or_else(|| {
        RuntimeStateError::resource_limit(
            "runtime-gdi",
            "gdi-generation-exhausted",
            "logical GDI owner generation is exhausted",
        )
    })
}

fn stale_plan(code: &'static str) -> RuntimeStateError {
    RuntimeStateError::conflict(
        "runtime-gdi",
        code,
        "logical GDI state changed after the mutation was planned",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> RuntimeGdiWindowTarget {
        RuntimeGdiWindowTarget {
            window_id: RuntimeWindowId(1),
            window_handle: 0x4801_0001,
            owner_thread_id: RuntimeThreadId(8192),
            client_width: 800,
            client_height: 600,
        }
    }

    fn graphics_target(
        surface_id: u64,
        surface_com_object_id: u64,
    ) -> RuntimeGdiGraphicsSurfaceTarget {
        RuntimeGdiGraphicsSurfaceTarget {
            surface_id: RuntimeGraphicsSurfaceId(surface_id),
            surface_com_object_id: RuntimeComObjectId(surface_com_object_id),
            owner_thread_id: RuntimeThreadId(8192),
            width: 1_024,
            height: 1_024,
            color_format: RuntimeGraphicsColorFormat::B8g8r8x8Unorm,
        }
    }

    fn screen_target() -> RuntimeGdiScreenTarget {
        RuntimeGdiScreenTarget {
            monitor_handle: 0x4f01_0001,
            owner_thread_id: RuntimeThreadId(8192),
            rect: RuntimeRect {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            },
        }
    }

    fn font() -> RuntimeLogFontA {
        RuntimeLogFontA {
            height: 64,
            width: 32,
            escapement: 0,
            orientation: 0,
            weight: 100,
            italic: 0,
            underline: 0,
            strike_out: 0,
            character_set: 128,
            output_precision: 4,
            clip_precision: 0,
            quality: 2,
            pitch_and_family: 0,
            face_name_windows_932: vec![
                0x82, 0x6c, 0x82, 0x72, 0x20, 0x83, 0x53, 0x83, 0x56, 0x83, 0x62, 0x83, 0x4e,
            ],
        }
    }

    fn dib_descriptor() -> RuntimeGdiDibSectionDescriptor {
        RuntimeGdiDibSectionDescriptor {
            width: 128,
            height: -96,
            planes: 1,
            bits_per_pixel: 8,
            compression: RUNTIME_GDI_DIB_BI_RGB,
            stride_bytes: 128,
            pixel_byte_length: 12_288,
            palette: (0..256)
                .map(|index| {
                    let intensity = u8::try_from(255 - index).unwrap();
                    RuntimeGdiRgbQuad {
                        blue: intensity,
                        green: intensity,
                        red: intensity,
                        reserved: 0,
                    }
                })
                .collect(),
        }
    }

    #[test]
    fn device_context_serialization_preserves_window_shape_and_types_graphics_targets() {
        let mut gdi = RuntimeGdi::default();
        let window = gdi
            .commit_get_dc(gdi.plan_get_dc(target()).unwrap())
            .unwrap();
        assert_eq!(
            serde_json::to_value(gdi.device_context_by_handle(window.handle).unwrap()).unwrap(),
            serde_json::json!({
                "id": 1,
                "handle": 0x4701_0001_u32,
                "namespace": "vine.logical-gdi-dc.v1",
                "windowId": 1,
                "windowHandle": 0x4801_0001_u32,
                "ownerThreadId": 8192,
                "surfaceId": 1,
            })
        );

        let graphics = gdi
            .commit_get_graphics_surface_dc(
                gdi.plan_get_graphics_surface_dc(graphics_target(7, 5))
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            serde_json::to_value(gdi.device_context_by_handle(graphics.handle).unwrap()).unwrap(),
            serde_json::json!({
                "id": 2,
                "handle": 0x4701_0002_u32,
                "namespace": "vine.logical-gdi-dc.v1",
                "ownerThreadId": 8192,
                "targetKind": "graphics-surface",
                "surfaceId": 7,
                "surfaceComObjectId": "5",
                "width": 1024,
                "height": 1024,
                "colorFormat": "b8g8r8x8-unorm",
            })
        );
    }

    #[test]
    fn screen_device_context_acquisitions_are_distinct_typed_and_relation_checked() {
        let mut gdi = RuntimeGdi::default();
        let invalid_before = gdi.clone();
        let mut invalid_target = screen_target();
        invalid_target.monitor_handle = 0;
        assert_eq!(
            gdi.plan_get_screen_dc(invalid_target).unwrap_err().code,
            "screen-dc-target-invalid"
        );
        assert_eq!(gdi, invalid_before);

        let first_plan = gdi.plan_get_screen_dc(screen_target()).unwrap();
        let stale_acquisition = first_plan.clone();
        let first = gdi.commit_get_screen_dc(first_plan).unwrap();
        assert_eq!(first.handle, RUNTIME_DEVICE_CONTEXT_HANDLE_BASE);
        assert_eq!(first.monitor_handle, screen_target().monitor_handle);
        assert_eq!(
            serde_json::to_value(gdi.device_context_by_handle(first.handle).unwrap()).unwrap(),
            serde_json::json!({
                "id": 1,
                "handle": 0x4701_0001_u32,
                "namespace": "vine.logical-gdi-dc.v1",
                "ownerThreadId": 8192,
                "targetKind": "screen",
                "monitorHandle": 0x4f01_0001_u32,
                "rect": {
                    "left": 0,
                    "top": 0,
                    "right": 1920,
                    "bottom": 1080,
                },
            })
        );
        assert_eq!(
            gdi.commit_get_screen_dc(stale_acquisition)
                .unwrap_err()
                .code,
            "screen-get-dc-plan-stale"
        );

        let second = gdi
            .commit_get_screen_dc(gdi.plan_get_screen_dc(screen_target()).unwrap())
            .unwrap();
        assert_ne!(first.handle, second.handle);
        assert!(gdi.surfaces.is_empty());

        let before_wrong_release = gdi.clone();
        let mut wrong_target = screen_target();
        wrong_target.monitor_handle += 1;
        assert_eq!(
            gdi.plan_release_screen_dc(
                wrong_target,
                first.handle,
                screen_target().owner_thread_id,
            )
            .unwrap_err()
            .code,
            "screen-device-context-relation-mismatch"
        );
        assert_eq!(gdi, before_wrong_release);

        let before_wrong_thread = gdi.clone();
        assert_eq!(
            gdi.plan_release_screen_dc(screen_target(), first.handle, RuntimeThreadId(8193))
                .unwrap_err()
                .code,
            "screen-device-context-relation-mismatch"
        );
        assert_eq!(gdi, before_wrong_thread);

        let release = gdi
            .plan_release_screen_dc(
                screen_target(),
                first.handle,
                screen_target().owner_thread_id,
            )
            .unwrap();
        let stale_release = release.clone();
        gdi.commit_release_dc(release).unwrap();
        assert_eq!(
            gdi.commit_release_dc(stale_release).unwrap_err().code,
            "release-dc-plan-stale"
        );
        assert!(gdi.device_context_by_handle(first.handle).is_err());
        assert!(gdi.device_context_by_handle(second.handle).is_ok());
    }

    #[test]
    fn compatible_memory_dcs_share_defaults_and_delete_exact_owner_identity_atomically() {
        let mut gdi = RuntimeGdi::default();
        let mut invalid_target = screen_target();
        invalid_target.monitor_handle = 0;
        let before_invalid = gdi.clone();
        assert_eq!(
            gdi.plan_create_compatible_dc(invalid_target)
                .unwrap_err()
                .code,
            "screen-dc-target-invalid"
        );
        assert_eq!(gdi, before_invalid);

        let first_plan = gdi.plan_create_compatible_dc(screen_target()).unwrap();
        let stale_first = first_plan.clone();
        let first = gdi.commit_create_compatible_dc(first_plan).unwrap();
        assert_eq!(first.handle, RUNTIME_DEVICE_CONTEXT_HANDLE_BASE);
        assert_eq!(first.selected_font_id, RuntimeGdiObjectId(1));
        assert_eq!(first.selected_bitmap_id, RuntimeGdiObjectId(2));
        assert!(first.default_objects_created);
        assert_eq!(gdi.objects.len(), 2);
        validate_default_font_object(&gdi.objects[&first.selected_font_id]).unwrap();
        validate_default_bitmap_object(&gdi.objects[&first.selected_bitmap_id]).unwrap();
        assert_eq!(
            serde_json::to_value(gdi.device_context_by_handle(first.handle).unwrap()).unwrap(),
            serde_json::json!({
                "id": 1,
                "handle": 0x4701_0001_u32,
                "namespace": "vine.logical-gdi-dc.v1",
                "ownerThreadId": 8192,
                "targetKind": "memory",
                "compatibleMonitorHandle": 0x4f01_0001_u32,
                "compatibleRect": {
                    "left": 0,
                    "top": 0,
                    "right": 1920,
                    "bottom": 1080,
                },
                "selectedFontId": 1,
                "selectedBitmapId": 2,
                "selectionGeneration": 0,
            })
        );
        assert_eq!(
            gdi.commit_create_compatible_dc(stale_first)
                .unwrap_err()
                .code,
            "create-compatible-dc-plan-stale"
        );

        let second = gdi
            .commit_create_compatible_dc(gdi.plan_create_compatible_dc(screen_target()).unwrap())
            .unwrap();
        assert_eq!(second.handle, RUNTIME_DEVICE_CONTEXT_HANDLE_BASE + 1);
        assert_eq!(second.selected_font_id, first.selected_font_id);
        assert_eq!(second.selected_bitmap_id, first.selected_bitmap_id);
        assert!(!second.default_objects_created);
        assert_eq!(gdi.objects.len(), 2);

        let before_wrong_thread = gdi.clone();
        assert_eq!(
            gdi.plan_delete_compatible_dc(first.handle, RuntimeThreadId(8193))
                .unwrap_err()
                .code,
            "delete-dc-relation-mismatch"
        );
        assert_eq!(gdi, before_wrong_thread);

        let screen = gdi
            .commit_get_screen_dc(gdi.plan_get_screen_dc(screen_target()).unwrap())
            .unwrap();
        let before_wrong_target = gdi.clone();
        assert_eq!(
            gdi.plan_delete_compatible_dc(screen.handle, screen_target().owner_thread_id)
                .unwrap_err()
                .code,
            "delete-dc-relation-mismatch"
        );
        assert_eq!(gdi, before_wrong_target);

        let delete = gdi
            .plan_delete_compatible_dc(first.handle, screen_target().owner_thread_id)
            .unwrap();
        let stale_delete = delete.clone();
        gdi.commit_release_dc(delete).unwrap();
        assert_eq!(
            gdi.commit_release_dc(stale_delete).unwrap_err().code,
            "release-dc-plan-stale"
        );
        assert!(gdi.device_context_by_handle(first.handle).is_err());
        assert!(gdi.device_context_by_handle(second.handle).is_ok());
        assert_eq!(gdi.objects.len(), 2);
    }

    #[test]
    fn compatible_memory_dc_multi_object_preflight_preserves_all_allocators() {
        let gdi = RuntimeGdi {
            next_object_handle: u32::MAX - 1,
            ..RuntimeGdi::default()
        };
        let before = gdi.clone();
        assert_eq!(
            gdi.plan_create_compatible_dc(screen_target())
                .unwrap_err()
                .code,
            "gdi-object-handle-exhausted"
        );
        assert_eq!(gdi, before);
    }

    #[test]
    fn selected_font_query_requires_same_thread_memory_dc_and_created_font() {
        let mut gdi = RuntimeGdi::default();
        let device_context = gdi
            .commit_create_compatible_dc(gdi.plan_create_compatible_dc(screen_target()).unwrap())
            .unwrap();
        let before_stock = gdi.clone();
        assert_eq!(
            gdi.selected_font_a(device_context.handle, screen_target().owner_thread_id)
                .unwrap_err()
                .code,
            "outline-text-metrics-selected-font-unsupported"
        );
        assert_eq!(gdi, before_stock);
        assert_eq!(
            gdi.selected_font_a(device_context.handle, RuntimeThreadId(8193))
                .unwrap_err()
                .code,
            "outline-text-metrics-device-context-relation-mismatch"
        );
        assert_eq!(gdi, before_stock);

        let created = gdi
            .commit_create_font_a(gdi.plan_create_font_a(font()).unwrap())
            .unwrap();
        let selected = gdi
            .plan_select_object(
                device_context.handle,
                created.handle,
                screen_target().owner_thread_id,
            )
            .unwrap();
        gdi.commit_select_object(selected).unwrap();
        let before_query = gdi.clone();
        assert_eq!(
            gdi.selected_font_a(device_context.handle, screen_target().owner_thread_id)
                .unwrap(),
            font()
        );
        assert_eq!(gdi, before_query);
    }

    #[test]
    fn text_raster_plan_requires_exact_font_dib_selection_and_advances_content_once() {
        let mut gdi = RuntimeGdi::default();
        let device_context = gdi
            .commit_create_compatible_dc(gdi.plan_create_compatible_dc(screen_target()).unwrap())
            .unwrap();
        let created_font = gdi
            .commit_create_font_a(gdi.plan_create_font_a(font()).unwrap())
            .unwrap();
        let dib_plan = gdi
            .plan_create_dib_section(
                device_context.handle,
                screen_target().owner_thread_id,
                dib_descriptor(),
                ProcessAddressAllocationId(1),
                GuestRange::new(GuestAddress(0x7200_0000), 0x3000).unwrap(),
            )
            .unwrap();
        let created_dib = gdi
            .commit_create_dib_section(dib_plan, MemoryOwnerId(1))
            .unwrap();

        let before_stock_font = gdi.clone();
        assert_eq!(
            gdi.text_raster_target(device_context.handle, screen_target().owner_thread_id)
                .unwrap_err()
                .code,
            "text-raster-selected-font-unsupported"
        );
        assert_eq!(gdi, before_stock_font);
        gdi.commit_select_object(
            gdi.plan_select_object(
                device_context.handle,
                created_font.handle,
                screen_target().owner_thread_id,
            )
            .unwrap(),
        )
        .unwrap();
        let before_stock_bitmap = gdi.clone();
        assert_eq!(
            gdi.text_raster_target(device_context.handle, screen_target().owner_thread_id)
                .unwrap_err()
                .code,
            "text-raster-selected-bitmap-unsupported"
        );
        assert_eq!(gdi, before_stock_bitmap);
        gdi.commit_select_object(
            gdi.plan_select_object(
                device_context.handle,
                created_dib.handle,
                screen_target().owner_thread_id,
            )
            .unwrap(),
        )
        .unwrap();

        let target = gdi
            .text_raster_target(device_context.handle, screen_target().owner_thread_id)
            .unwrap();
        assert_eq!(target.log_font, font());
        assert_eq!(target.descriptor, dib_descriptor());
        assert_eq!(target.backing.content_generation, 0);
        let plan = gdi.plan_text_raster_commit(target.clone()).unwrap();
        let stale = plan.clone();
        let result = gdi.commit_text_raster(plan).unwrap();
        assert_eq!(result.content_generation, 1);
        assert_eq!(result.selection_generation, 2);
        assert_eq!(
            gdi.text_raster_target(device_context.handle, screen_target().owner_thread_id)
                .unwrap()
                .backing
                .content_generation,
            1
        );
        assert_eq!(
            gdi.commit_text_raster(stale).unwrap_err().code,
            "text-raster-plan-stale"
        );
        assert_eq!(
            gdi.text_raster_target(device_context.handle, RuntimeThreadId(8193))
                .unwrap_err()
                .code,
            "text-raster-device-context-relation-mismatch"
        );
    }

    #[test]
    fn device_context_acquisitions_are_distinct_and_release_is_relation_checked() {
        let mut gdi = RuntimeGdi::default();
        let first_plan = gdi.plan_get_dc(target()).unwrap();
        let stale = first_plan.clone();
        let first = gdi.commit_get_dc(first_plan).unwrap();
        assert!(first.surface_created);
        assert_eq!(first.handle, RUNTIME_DEVICE_CONTEXT_HANDLE_BASE);
        assert_eq!(
            gdi.commit_get_dc(stale).unwrap_err().code,
            "get-dc-plan-stale"
        );

        let second = gdi
            .commit_get_dc(gdi.plan_get_dc(target()).unwrap())
            .unwrap();
        assert_ne!(first.handle, second.handle);
        assert_eq!(first.surface_id, second.surface_id);
        assert!(!second.surface_created);
        let before_wrong_release = gdi.clone();
        assert_eq!(
            gdi.plan_release_dc(0x4801_0002, first.handle, RuntimeThreadId(8192))
                .unwrap_err()
                .code,
            "device-context-relation-mismatch"
        );
        assert_eq!(gdi, before_wrong_release);

        let release = gdi
            .plan_release_dc(
                target().window_handle,
                first.handle,
                target().owner_thread_id,
            )
            .unwrap();
        let stale_release = release.clone();
        gdi.commit_release_dc(release).unwrap();
        assert_eq!(
            gdi.commit_release_dc(stale_release).unwrap_err().code,
            "release-dc-plan-stale"
        );
        assert_eq!(
            gdi.device_context_by_handle(first.handle)
                .unwrap_err()
                .class,
            super::super::RuntimeStateErrorClass::NotFound
        );
        assert!(gdi.device_context_by_handle(second.handle).is_ok());
    }

    #[test]
    fn stock_brush_and_surface_fill_preserve_failure_atomicity() {
        let mut gdi = RuntimeGdi::default();
        let dc = gdi
            .commit_get_dc(gdi.plan_get_dc(target()).unwrap())
            .unwrap();
        let invalid_before = gdi.clone();
        assert_eq!(
            gdi.plan_get_stock_object(-1).unwrap_err().code,
            "stock-object-index-unsupported"
        );
        assert_eq!(gdi, invalid_before);

        let brush = gdi
            .commit_get_stock_object(
                gdi.plan_get_stock_object(RUNTIME_GDI_BLACK_BRUSH_INDEX)
                    .unwrap(),
            )
            .unwrap();
        let repeated = gdi
            .commit_get_stock_object(
                gdi.plan_get_stock_object(RUNTIME_GDI_BLACK_BRUSH_INDEX)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(brush.handle, repeated.handle);
        assert!(!brush.already_created);
        assert!(repeated.already_created);

        let fill = gdi
            .commit_fill_rect(
                gdi.plan_fill_rect(
                    dc.handle,
                    RuntimeRect {
                        left: 0,
                        top: 0,
                        right: 800,
                        bottom: 600,
                    },
                    brush.handle,
                    target().owner_thread_id,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(fill.surface_generation, 1);
        assert_eq!(fill.fill.unwrap().color_ref, 0);
        let before_empty = gdi.clone();
        let empty = gdi
            .commit_fill_rect(
                gdi.plan_fill_rect(
                    dc.handle,
                    RuntimeRect {
                        left: 0,
                        top: 0,
                        right: 0,
                        bottom: 0,
                    },
                    brush.handle,
                    target().owner_thread_id,
                )
                .unwrap(),
            )
            .unwrap();
        assert!(empty.fill.is_none());
        assert_eq!(gdi, before_empty);

        let before_invalid = gdi.clone();
        assert_eq!(
            gdi.plan_fill_rect(
                dc.handle,
                RuntimeRect {
                    left: -1,
                    top: 0,
                    right: 1,
                    bottom: 1,
                },
                brush.handle,
                target().owner_thread_id,
            )
            .unwrap_err()
            .code,
            "fill-rect-outside-supported-surface"
        );
        assert_eq!(gdi, before_invalid);

        let surface = gdi.surfaces.get_mut(&dc.surface_id).unwrap();
        surface.generation = u64::MAX;
        let before_generation_exhaustion = gdi.clone();
        assert_eq!(
            gdi.plan_fill_rect(
                dc.handle,
                RuntimeRect {
                    left: 0,
                    top: 0,
                    right: 1,
                    bottom: 1,
                },
                brush.handle,
                target().owner_thread_id,
            )
            .unwrap_err()
            .code,
            "surface-generation-exhausted"
        );
        assert_eq!(gdi, before_generation_exhaustion);
    }

    #[test]
    fn font_creation_owns_distinct_identity_and_rejects_stale_or_invalid_plans() {
        let mut gdi = RuntimeGdi::default();
        let mut invalid = font();
        invalid.face_name_windows_932 = vec![b'x'; RUNTIME_LOG_FONT_A_FACE_CAPACITY];
        let before_invalid = gdi.clone();
        assert_eq!(
            gdi.plan_create_font_a(invalid).unwrap_err().code,
            "log-font-a-face-name-invalid"
        );
        assert_eq!(gdi, before_invalid);

        let first_plan = gdi.plan_create_font_a(font()).unwrap();
        let stale_plan = first_plan.clone();
        let first = gdi.commit_create_font_a(first_plan).unwrap();
        assert_eq!(first.object_id, RuntimeGdiObjectId(1));
        assert_eq!(first.handle, RUNTIME_GDI_OBJECT_HANDLE_BASE);
        assert_eq!(
            gdi.commit_create_font_a(stale_plan).unwrap_err().code,
            "create-font-a-plan-stale"
        );
        let second = gdi
            .commit_create_font_a(gdi.plan_create_font_a(font()).unwrap())
            .unwrap();
        assert_eq!(second.object_id, RuntimeGdiObjectId(2));
        assert_eq!(second.handle, RUNTIME_GDI_OBJECT_HANDLE_BASE + 1);

        let object = gdi.objects.get(&first.object_id).unwrap();
        let RuntimeGdiObjectKind::FontA { log_font } = &object.object else {
            panic!("created GDI object is not a font")
        };
        assert_eq!(log_font, &font());
        assert_eq!(
            &log_font.to_bytes().unwrap()[..46],
            &[
                64, 0, 0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100, 0, 0, 0, 0, 0, 0, 128, 4, 0,
                2, 0, 130, 108, 130, 114, 32, 131, 83, 131, 86, 131, 98, 131, 78, 0, 0, 0, 0, 0,
            ]
        );

        let dc = gdi
            .commit_get_dc(gdi.plan_get_dc(target()).unwrap())
            .unwrap();
        let before_wrong_kind = gdi.clone();
        assert_eq!(
            gdi.plan_fill_rect(
                dc.handle,
                RuntimeRect {
                    left: 0,
                    top: 0,
                    right: 1,
                    bottom: 1,
                },
                first.handle,
                target().owner_thread_id,
            )
            .unwrap_err()
            .code,
            "fill-rect-object-not-brush"
        );
        assert_eq!(gdi, before_wrong_kind);
    }

    #[test]
    fn graphics_surface_dc_is_single_outstanding_and_guards_com_retirement() {
        let mut gdi = RuntimeGdi::default();
        let target = graphics_target(1, 9);
        let plan = gdi.plan_get_graphics_surface_dc(target).unwrap();
        let stale_acquire = plan.clone();
        let acquired = gdi.commit_get_graphics_surface_dc(plan).unwrap();
        assert_eq!(acquired.handle, RUNTIME_DEVICE_CONTEXT_HANDLE_BASE);
        assert_eq!(acquired.surface_id, target.surface_id);
        assert_eq!(gdi.device_contexts.len(), 1);

        let before_repeat = gdi.clone();
        assert_eq!(
            gdi.plan_get_graphics_surface_dc(target).unwrap_err().code,
            "graphics-surface-dc-already-acquired"
        );
        assert_eq!(gdi, before_repeat);
        assert_eq!(
            gdi.commit_get_graphics_surface_dc(stale_acquire)
                .unwrap_err()
                .code,
            "graphics-surface-get-dc-plan-stale"
        );

        let before_retirement = gdi.clone();
        assert_eq!(
            gdi.plan_com_object_retirements(&[target.surface_com_object_id].into_iter().collect())
                .unwrap_err()
                .code,
            "graphics-surface-dc-still-acquired"
        );
        assert_eq!(gdi, before_retirement);

        let before_wrong_release = gdi.clone();
        assert_eq!(
            gdi.plan_release_graphics_surface_dc(
                graphics_target(1, 10),
                acquired.handle,
                target.owner_thread_id,
            )
            .unwrap_err()
            .code,
            "graphics-surface-dc-relation-mismatch"
        );
        assert_eq!(gdi, before_wrong_release);

        let release = gdi
            .plan_release_graphics_surface_dc(target, acquired.handle, target.owner_thread_id)
            .unwrap();
        let stale_release = release.clone();
        gdi.commit_release_dc(release).unwrap();
        assert!(gdi.device_contexts.is_empty());
        assert_eq!(
            gdi.commit_release_dc(stale_release).unwrap_err().code,
            "release-dc-plan-stale"
        );

        let guard = gdi
            .plan_com_object_retirements(&[target.surface_com_object_id].into_iter().collect())
            .unwrap()
            .unwrap();
        gdi.preflight_com_object_retirements(&guard).unwrap();
        let second = gdi
            .plan_get_graphics_surface_dc(graphics_target(2, 10))
            .unwrap();
        gdi.commit_get_graphics_surface_dc(second).unwrap();
        assert_eq!(
            gdi.preflight_com_object_retirements(&guard)
                .unwrap_err()
                .code,
            "gdi-com-retirement-guard-stale"
        );
    }

    #[test]
    fn graphics_surface_dc_capacity_failure_preserves_allocator_state() {
        let mut gdi = RuntimeGdi::default();
        for index in 0..RUNTIME_GDI_DEVICE_CONTEXT_LIMIT {
            let target = graphics_target(index as u64 + 1, index as u64 + 1);
            let plan = gdi.plan_get_graphics_surface_dc(target).unwrap();
            gdi.commit_get_graphics_surface_dc(plan).unwrap();
        }
        let before = gdi.clone();
        assert_eq!(
            gdi.plan_get_graphics_surface_dc(graphics_target(
                RUNTIME_GDI_DEVICE_CONTEXT_LIMIT as u64 + 1,
                RUNTIME_GDI_DEVICE_CONTEXT_LIMIT as u64 + 1,
            ))
            .unwrap_err()
            .code,
            "device-context-capacity-exhausted"
        );
        assert_eq!(gdi, before);
    }
}

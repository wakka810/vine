use std::collections::{BTreeMap, BTreeSet};

use crate::memory::{GuestAddress, GuestRange, MemoryOwnerId};

use super::address_space::ProcessAddressAllocationId;
use super::com::RuntimeComObjectId;
use super::graphics::{RuntimeGraphicsColorFormat, RuntimeGraphicsSurfaceId};
use super::{RuntimeRect, RuntimeStateError, RuntimeThreadId, RuntimeWindowId};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeDeviceContextId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeGdiObjectId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeGdiSurfaceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub owner_thread_id: RuntimeThreadId,
    pub target: RuntimeDeviceContextTarget,
    pub selected_font_id: Option<RuntimeGdiObjectId>,
    pub selected_bitmap_id: Option<RuntimeGdiObjectId>,
    pub selection_generation: u64,
}

impl RuntimeDeviceContext {
    #[cfg(test)]
    pub(crate) fn window_surface_id(&self) -> Option<RuntimeGdiSurfaceId> {
        match self.target {
            RuntimeDeviceContextTarget::Window { surface_id, .. } => Some(surface_id),
            RuntimeDeviceContextTarget::Screen { .. }
            | RuntimeDeviceContextTarget::Memory { .. }
            | RuntimeDeviceContextTarget::GraphicsSurface { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeGdiRgbQuad {
    pub blue: u8,
    pub green: u8,
    pub red: u8,
    pub reserved: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
            RuntimeStateError::new("logical DIB section height cannot be i32::MIN")
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeGdiDibSectionBacking {
    pub address_allocation_id: ProcessAddressAllocationId,
    pub memory_owner: MemoryOwnerId,
    pub range: GuestRange,
    pub content_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeGdiObject {
    pub id: RuntimeGdiObjectId,
    pub handle: u32,
    pub shared: bool,
    pub object: RuntimeGdiObjectKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeGdiSurfaceFill {
    pub rect: RuntimeRect,
    pub color_ref: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeGdiSurface {
    pub id: RuntimeGdiSurfaceId,
    pub window_id: RuntimeWindowId,
    pub width: u32,
    pub height: u32,
    pub fills: Vec<RuntimeGdiSurfaceFill>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeGetDcResult {
    pub device_context_id: RuntimeDeviceContextId,
    pub handle: u32,
    pub surface_id: RuntimeGdiSurfaceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeGdi {
    device_contexts: BTreeMap<RuntimeDeviceContextId, RuntimeDeviceContext>,
    device_context_handle_index: BTreeMap<u32, RuntimeDeviceContextId>,
    objects: BTreeMap<RuntimeGdiObjectId, RuntimeGdiObject>,
    object_handle_index: BTreeMap<u32, RuntimeGdiObjectId>,
    stock_object_index: BTreeMap<i32, RuntimeGdiObjectId>,
    surfaces: BTreeMap<RuntimeGdiSurfaceId, RuntimeGdiSurface>,
    window_surface_index: BTreeMap<RuntimeWindowId, RuntimeGdiSurfaceId>,
    graphics_surface_dc_index: BTreeMap<RuntimeGraphicsSurfaceId, RuntimeDeviceContextId>,
    next_device_context_id: u64,
    next_device_context_handle: u32,
    next_object_id: u64,
    next_object_handle: u32,
    next_surface_id: u64,
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
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeGdiSelectionSlot {
    Font,
    Bitmap,
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
    pub(super) fn validate_window_surface_resize(
        &self,
        window_id: RuntimeWindowId,
        previous_width: u32,
        previous_height: u32,
        next_width: u32,
        next_height: u32,
    ) -> Result<Option<RuntimeGdiSurfaceId>, RuntimeStateError> {
        if previous_width == 0
            || previous_height == 0
            || next_width == 0
            || next_height == 0
            || previous_width > i32::MAX as u32
            || previous_height > i32::MAX as u32
            || next_width > i32::MAX as u32
            || next_height > i32::MAX as u32
            || (previous_width == next_width && previous_height == next_height)
        {
            return Err(RuntimeStateError::new(
                "logical window surface resize requires changed positive x86 dimensions",
            ));
        }

        if let Some(surface_id) = self.window_surface_index.get(&window_id).copied() {
            let surface = self.surfaces.get(&surface_id).ok_or_else(|| {
                RuntimeStateError::new(
                    "logical window surface index does not resolve to a live surface",
                )
            })?;
            if surface.id != surface_id
                || surface.window_id != window_id
                || surface.width != previous_width
                || surface.height != previous_height
            {
                return Err(RuntimeStateError::new(
                    "logical GDI surface does not match the pre-resize client geometry",
                ));
            }
            self.require_no_live_window_surface_dc(window_id, Some(surface_id))?;

            Ok(Some(surface_id))
        } else {
            if self
                .surfaces
                .values()
                .any(|surface| surface.window_id == window_id)
            {
                return Err(RuntimeStateError::new(
                    "logical window surface exists without its window index",
                ));
            }
            self.require_no_live_window_surface_dc(window_id, None)?;
            Ok(None)
        }
    }

    pub(super) fn resize_window_surface(
        &mut self,
        window_id: RuntimeWindowId,
        previous_width: u32,
        previous_height: u32,
        next_width: u32,
        next_height: u32,
    ) -> Result<(), RuntimeStateError> {
        let surface_id = self.validate_window_surface_resize(
            window_id,
            previous_width,
            previous_height,
            next_width,
            next_height,
        )?;
        if let Some(surface_id) = surface_id {
            let surface = self
                .surfaces
                .get_mut(&surface_id)
                .expect("validated live window surface");
            surface.width = next_width;
            surface.height = next_height;
            surface.fills.clear();
        }
        Ok(())
    }

    fn require_no_live_window_surface_dc(
        &self,
        window_id: RuntimeWindowId,
        surface_id: Option<RuntimeGdiSurfaceId>,
    ) -> Result<(), RuntimeStateError> {
        let live = self.device_contexts.values().any(|device_context| {
            matches!(
                device_context.target,
                RuntimeDeviceContextTarget::Window {
                    window_id: target_window_id,
                    surface_id: target_surface_id,
                    ..
                } if target_window_id == window_id
                    && surface_id.is_none_or(|expected| expected == target_surface_id)
            )
        });
        if live {
            return Err(RuntimeStateError::new(
                "logical window surface cannot resize while a window device context is live",
            ));
        }
        Ok(())
    }

    pub(super) fn preview_window_dc(
        &self,
        target: RuntimeGdiWindowTarget,
    ) -> Result<RuntimeGetDcResult, RuntimeStateError> {
        validate_window_target(target)?;
        let allocation = self.plan_device_context_allocation()?;

        let surface_id =
            if let Some(surface_id) = self.window_surface_index.get(&target.window_id).copied() {
                let surface = self.surfaces.get(&surface_id).ok_or_else(|| {
                    RuntimeStateError::new(
                        "logical window surface index does not resolve to a live surface",
                    )
                })?;
                if surface.id != surface_id || surface.window_id != target.window_id {
                    return Err(RuntimeStateError::new(
                        "logical window surface index resolves to another surface or window",
                    ));
                }
                if surface.width != target.client_width || surface.height != target.client_height {
                    return Err(RuntimeStateError::new(
                        "logical window client geometry changed after its GDI surface was created",
                    ));
                }
                surface_id
            } else {
                if self
                    .surfaces
                    .values()
                    .any(|surface| surface.window_id == target.window_id)
                {
                    return Err(RuntimeStateError::new(
                        "logical window surface exists without its window index",
                    ));
                }
                if self.surfaces.len() >= RUNTIME_GDI_SURFACE_LIMIT {
                    return Err(RuntimeStateError::new(
                        "logical GDI surface capacity is exhausted",
                    ));
                }
                checked_next_identity(self.next_surface_id)?;
                let surface_id = RuntimeGdiSurfaceId(self.next_surface_id);
                if self.surfaces.contains_key(&surface_id) {
                    return Err(RuntimeStateError::new(
                        "logical GDI surface allocator identifies a live surface",
                    ));
                }
                surface_id
            };
        Ok(RuntimeGetDcResult {
            device_context_id: allocation.id,
            handle: allocation.handle,
            surface_id,
        })
    }

    pub(super) fn get_window_dc(
        &mut self,
        target: RuntimeGdiWindowTarget,
    ) -> Result<RuntimeGetDcResult, RuntimeStateError> {
        let result = self.preview_window_dc(target)?;
        self.publish_window_dc(target, result);
        Ok(result)
    }

    pub(super) fn publish_window_dc(
        &mut self,
        target: RuntimeGdiWindowTarget,
        result: RuntimeGetDcResult,
    ) {
        let surface_created = !self.window_surface_index.contains_key(&target.window_id);
        let device_context = RuntimeDeviceContext {
            id: result.device_context_id,
            handle: result.handle,
            owner_thread_id: target.owner_thread_id,
            target: RuntimeDeviceContextTarget::Window {
                window_id: target.window_id,
                window_handle: target.window_handle,
                surface_id: result.surface_id,
            },
            selected_font_id: None,
            selected_bitmap_id: None,
            selection_generation: 0,
        };
        if surface_created {
            self.window_surface_index
                .insert(target.window_id, result.surface_id);
            self.surfaces.insert(
                result.surface_id,
                RuntimeGdiSurface {
                    id: result.surface_id,
                    window_id: target.window_id,
                    width: target.client_width,
                    height: target.client_height,
                    fills: Vec::new(),
                },
            );
        }
        self.device_context_handle_index
            .insert(result.handle, result.device_context_id);
        self.device_contexts
            .insert(result.device_context_id, device_context);
        self.next_device_context_id = result.device_context_id.0 + 1;
        self.next_device_context_handle = result.handle + 1;
        if surface_created {
            self.next_surface_id += 1;
        }
    }

    pub(super) fn get_screen_dc(
        &mut self,
        target: RuntimeGdiScreenTarget,
    ) -> Result<u32, RuntimeStateError> {
        validate_screen_target(target)?;
        let allocation = self.plan_device_context_allocation()?;

        self.device_context_handle_index
            .insert(allocation.handle, allocation.id);
        self.device_contexts.insert(
            allocation.id,
            RuntimeDeviceContext {
                id: allocation.id,
                handle: allocation.handle,
                owner_thread_id: target.owner_thread_id,
                target: RuntimeDeviceContextTarget::Screen {
                    monitor_handle: target.monitor_handle,
                    rect: target.rect,
                },
                selected_font_id: None,
                selected_bitmap_id: None,
                selection_generation: 0,
            },
        );
        self.next_device_context_id = allocation.next_id;
        self.next_device_context_handle = allocation.next_handle;

        Ok(allocation.handle)
    }

    pub(super) fn create_compatible_dc(
        &mut self,
        target: RuntimeGdiScreenTarget,
    ) -> Result<Option<u32>, RuntimeStateError> {
        validate_screen_target(target)?;
        let Some(device_context_allocation) = self.try_plan_device_context_allocation()? else {
            return Ok(None);
        };
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
        let Some(allocations) = self.try_plan_object_allocations(missing_default_objects)? else {
            return Ok(None);
        };
        let (next_object_id, next_object_handle) = allocations.last().map_or(
            (self.next_object_id, self.next_object_handle),
            |allocation| (allocation.next_id, allocation.next_handle),
        );

        let mut allocations = allocations.into_iter();
        let mut new_default_objects = Vec::with_capacity(missing_default_objects);
        let selected_font_id = existing_font.map_or_else(
            || {
                let allocation = allocations
                    .next()
                    .expect("missing default font must reserve one object allocation");
                new_default_objects.push((
                    RUNTIME_GDI_SYSTEM_FONT_INDEX,
                    RuntimeGdiObject {
                        id: allocation.id,
                        handle: allocation.handle,
                        shared: true,
                        object: RuntimeGdiObjectKind::StockFont {
                            index: RUNTIME_GDI_SYSTEM_FONT_INDEX,
                        },
                    },
                ));
                allocation.id
            },
            |object| object.id,
        );
        let selected_bitmap_id = existing_bitmap.map_or_else(
            || {
                let allocation = allocations
                    .next()
                    .expect("missing default bitmap must reserve one object allocation");
                new_default_objects.push((
                    RUNTIME_GDI_DEFAULT_BITMAP_INDEX,
                    RuntimeGdiObject {
                        id: allocation.id,
                        handle: allocation.handle,
                        shared: true,
                        object: RuntimeGdiObjectKind::StockBitmap {
                            index: RUNTIME_GDI_DEFAULT_BITMAP_INDEX,
                            width: 1,
                            height: 1,
                            bits_per_pixel: 1,
                        },
                    },
                ));
                allocation.id
            },
            |object| object.id,
        );
        for (index, object) in new_default_objects {
            self.stock_object_index.insert(index, object.id);
            self.object_handle_index.insert(object.handle, object.id);
            self.objects.insert(object.id, object);
        }
        self.device_context_handle_index.insert(
            device_context_allocation.handle,
            device_context_allocation.id,
        );
        self.device_contexts.insert(
            device_context_allocation.id,
            RuntimeDeviceContext {
                id: device_context_allocation.id,
                handle: device_context_allocation.handle,
                owner_thread_id: target.owner_thread_id,
                target: RuntimeDeviceContextTarget::Memory {
                    compatible_monitor_handle: target.monitor_handle,
                    compatible_rect: target.rect,
                },
                selected_font_id: Some(selected_font_id),
                selected_bitmap_id: Some(selected_bitmap_id),
                selection_generation: 0,
            },
        );
        self.next_device_context_id = device_context_allocation.next_id;
        self.next_device_context_handle = device_context_allocation.next_handle;
        self.next_object_id = next_object_id;
        self.next_object_handle = next_object_handle;

        Ok(Some(device_context_allocation.handle))
    }

    pub(crate) fn preview_graphics_surface_dc(
        &self,
        target: RuntimeGdiGraphicsSurfaceTarget,
    ) -> Result<Option<u32>, RuntimeStateError> {
        validate_graphics_surface_target(target)?;
        if self
            .graphics_surface_dc_index
            .contains_key(&target.surface_id)
        {
            return Ok(None);
        }
        let allocation = self.plan_device_context_allocation()?;

        Ok(Some(allocation.handle))
    }

    pub(crate) fn get_graphics_surface_dc(
        &mut self,
        target: RuntimeGdiGraphicsSurfaceTarget,
    ) -> Result<Option<u32>, RuntimeStateError> {
        let Some(handle) = self.preview_graphics_surface_dc(target)? else {
            return Ok(None);
        };
        let id = RuntimeDeviceContextId(self.next_device_context_id);
        let next_device_context_id = id.0 + 1; // allocator checked
        let next_device_context_handle = handle + 1; // allocator checked
        let device_context = RuntimeDeviceContext {
            id,
            handle,
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
        };
        self.graphics_surface_dc_index.insert(target.surface_id, id);
        self.device_context_handle_index.insert(handle, id);
        self.device_contexts.insert(id, device_context);
        self.next_device_context_id = next_device_context_id;
        self.next_device_context_handle = next_device_context_handle;

        Ok(Some(handle))
    }

    pub(super) fn get_stock_object(
        &mut self,
        index: i32,
    ) -> Result<Option<u32>, RuntimeStateError> {
        if !(0..=RUNTIME_GDI_DEFAULT_BITMAP_INDEX).contains(&index) {
            return Ok(None);
        }
        if index != RUNTIME_GDI_BLACK_BRUSH_INDEX {
            return Err(RuntimeStateError::new(format!(
                "GDI stock object index {index} is outside the reached support band"
            )));
        }
        if let Some(object) = self.stock_object_by_index(index)? {
            return Ok(Some(object.handle));
        }
        let Some(allocation) = self.try_plan_object_allocation()? else {
            return Ok(None);
        };

        let object = RuntimeGdiObject {
            id: allocation.id,
            handle: allocation.handle,
            shared: true,
            object: RuntimeGdiObjectKind::StockBrush {
                index,
                color_ref: 0,
            },
        };
        self.stock_object_index.insert(index, object.id);
        self.object_handle_index.insert(object.handle, object.id);
        self.objects.insert(object.id, object);
        self.next_object_id = allocation.next_id;
        self.next_object_handle = allocation.next_handle;

        Ok(Some(allocation.handle))
    }

    pub(super) fn create_font_a(
        &mut self,
        log_font: RuntimeLogFontA,
    ) -> Result<Option<u32>, RuntimeStateError> {
        validate_log_font_a(&log_font)?;
        let Some(allocation) = self.try_plan_object_allocation()? else {
            return Ok(None);
        };

        let object = RuntimeGdiObject {
            id: allocation.id,
            handle: allocation.handle,
            shared: false,
            object: RuntimeGdiObjectKind::FontA { log_font },
        };
        self.object_handle_index.insert(object.handle, object.id);
        self.objects.insert(object.id, object);
        self.next_object_id = allocation.next_id;
        self.next_object_handle = allocation.next_handle;

        Ok(Some(allocation.handle))
    }

    pub(super) fn create_dib_section(
        &mut self,
        device_context_handle: u32,
        thread_id: RuntimeThreadId,
        descriptor: RuntimeGdiDibSectionDescriptor,
        address_allocation_id: ProcessAddressAllocationId,
        backing_range: GuestRange,
        install_memory: impl FnOnce() -> Result<Option<MemoryOwnerId>, RuntimeStateError>,
    ) -> Result<Option<u32>, RuntimeStateError> {
        validate_dib_section_descriptor(&descriptor)?;
        validate_dib_section_backing_shape(&descriptor, address_allocation_id, backing_range)?;
        let source_device_context = self.device_context_by_handle(device_context_handle)?;
        if !matches!(
            source_device_context.target,
            RuntimeDeviceContextTarget::Memory { .. }
        ) || source_device_context.owner_thread_id != thread_id
        {
            return Err(RuntimeStateError::new(
                "CreateDIBSection requires a live compatible memory DC owned by the current guest thread",
            ));
        }
        self.validate_memory_dc_selections(source_device_context)?;
        if self.objects.values().any(|object| {
            matches!(
                &object.object,
                RuntimeGdiObjectKind::DibSection { backing, .. }
                    if backing.address_allocation_id == address_allocation_id || backing.range == backing_range
            )
        }) {
            return Err(RuntimeStateError::new("logical DIB section allocator selected live guest backing",
            ));
        }
        let Some(allocation) = self.try_plan_object_allocation()? else {
            return Ok(None);
        };
        let Some(memory_owner) = install_memory()? else {
            return Ok(None);
        };

        let object = RuntimeGdiObject {
            id: allocation.id,
            handle: allocation.handle,
            shared: false,
            object: RuntimeGdiObjectKind::DibSection {
                descriptor,
                backing: RuntimeGdiDibSectionBacking {
                    address_allocation_id,
                    memory_owner,
                    range: backing_range,
                    content_generation: 0,
                },
            },
        };
        self.object_handle_index.insert(object.handle, object.id);
        self.objects.insert(object.id, object);
        self.next_object_id = allocation.next_id;
        self.next_object_handle = allocation.next_handle;

        Ok(Some(allocation.handle))
    }

    pub(super) fn select_object(
        &mut self,
        device_context_handle: u32,
        object_handle: u32,
        thread_id: RuntimeThreadId,
    ) -> Result<Option<u32>, RuntimeStateError> {
        let Some(device_context) = self.device_context_option(device_context_handle)? else {
            return Ok(None);
        };
        if !matches!(
            device_context.target,
            RuntimeDeviceContextTarget::Memory { .. }
        ) || device_context.owner_thread_id != thread_id
        {
            return Ok(None);
        }
        self.validate_memory_dc_selections(device_context)?;
        let Some(selected_object) = self.object_option(object_handle)? else {
            return Ok(None);
        };
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
                return Ok(None);
            }
        };
        let previous_object = self.objects.get(&previous_id).ok_or_else(|| {
            RuntimeStateError::new("logical memory DC selection does not resolve to a live object")
        })?;
        let changed = previous_id != selected_object.id;
        let selection_generation = if changed {
            device_context
                .selection_generation
                .checked_add(1)
                .ok_or_else(|| {
                    RuntimeStateError::new("logical memory DC selection generation is exhausted")
                })?
        } else {
            device_context.selection_generation
        };
        let device_context_id = device_context.id;
        let selected_object_id = selected_object.id;
        let previous_handle = previous_object.handle;
        if changed {
            let device_context = self
                .device_contexts
                .get_mut(&device_context_id)
                .expect("validated memory DC disappeared");
            match slot {
                RuntimeGdiSelectionSlot::Font => {
                    device_context.selected_font_id = Some(selected_object_id)
                }
                RuntimeGdiSelectionSlot::Bitmap => {
                    device_context.selected_bitmap_id = Some(selected_object_id)
                }
            }
            device_context.selection_generation = selection_generation;
        }
        Ok(Some(previous_handle))
    }

    fn validate_delete_object(
        &self,
        object_handle: u32,
    ) -> Result<Option<&RuntimeGdiObject>, RuntimeStateError> {
        let Some(object) = self.object_option(object_handle)? else {
            return Ok(None);
        };
        if object.shared {
            return Ok(None);
        }
        if self.device_contexts.values().any(|device_context| {
            device_context.selected_font_id == Some(object.id)
                || device_context.selected_bitmap_id == Some(object.id)
        }) {
            return Ok(None);
        }
        if !matches!(
            &object.object,
            RuntimeGdiObjectKind::FontA { .. } | RuntimeGdiObjectKind::DibSection { .. }
        ) {
            return Ok(None);
        }

        Ok(Some(object))
    }

    pub(super) fn delete_object(
        &mut self,
        object_handle: u32,
        release_backing: impl FnOnce(&RuntimeGdiObject) -> Result<(), RuntimeStateError>,
    ) -> Result<Option<RuntimeGdiObject>, RuntimeStateError> {
        let Some(object) = self.validate_delete_object(object_handle)? else {
            return Ok(None);
        };
        let object_id = object.id;
        release_backing(object)?;

        self.object_handle_index.remove(&object_handle);
        let object = self
            .objects
            .remove(&object_id)
            .expect("validated logical GDI object disappeared");

        Ok(Some(object))
    }

    pub(super) fn object_bytes(
        &self,
        object_handle: u32,
        requested_byte_length: u32,
    ) -> Result<Option<Vec<u8>>, RuntimeStateError> {
        let Some(object) = self.object_option(object_handle)? else {
            return Ok(None);
        };
        match &object.object {
            RuntimeGdiObjectKind::FontA { log_font }
                if requested_byte_length == RUNTIME_LOG_FONT_A_BYTE_LENGTH as u32 =>
            {
                Ok(Some(log_font.to_bytes()?.to_vec()))
            }
            RuntimeGdiObjectKind::DibSection {
                descriptor,
                backing,
            } if requested_byte_length == RUNTIME_GDI_BITMAP_BYTE_LENGTH as u32 => Ok(Some(
                descriptor.bitmap_bytes(backing.range.start())?.to_vec(),
            )),
            RuntimeGdiObjectKind::StockBrush { .. }
            | RuntimeGdiObjectKind::StockFont { .. }
            | RuntimeGdiObjectKind::StockBitmap { .. }
            | RuntimeGdiObjectKind::FontA { .. }
            | RuntimeGdiObjectKind::DibSection { .. } => Ok(None),
        }
    }

    pub(super) fn selected_font_a(
        &self,
        device_context_handle: u32,
        thread_id: RuntimeThreadId,
    ) -> Result<&RuntimeLogFontA, RuntimeStateError> {
        let device_context = self.device_context_by_handle(device_context_handle)?;
        if !matches!(
            device_context.target,
            RuntimeDeviceContextTarget::Memory { .. }
        ) || device_context.owner_thread_id != thread_id
        {
            return Err(RuntimeStateError::new(
                "GetOutlineTextMetricsA requires a compatible memory DC owned by the current guest thread",
            ));
        }
        self.validate_memory_dc_selections(device_context)?;
        let font_id = device_context
            .selected_font_id
            .expect("validated memory DC has a selected font");
        let font = self.objects.get(&font_id).ok_or_else(|| {
            RuntimeStateError::new(
                "logical memory DC selected font does not resolve to a live object",
            )
        })?;
        match &font.object {
            RuntimeGdiObjectKind::FontA { log_font } => Ok(log_font),
            RuntimeGdiObjectKind::StockFont { .. } => Err(RuntimeStateError::new(
                "GetOutlineTextMetricsA requires a supported created FontA selection",
            )),
            RuntimeGdiObjectKind::StockBrush { .. }
            | RuntimeGdiObjectKind::StockBitmap { .. }
            | RuntimeGdiObjectKind::DibSection { .. } => Err(RuntimeStateError::new(
                "logical memory DC font selection resolved to a non-font object",
            )),
        }
    }

    pub(super) fn text_raster_target(
        &self,
        device_context_handle: u32,
        thread_id: RuntimeThreadId,
    ) -> Result<Option<RuntimeGdiTextRasterTarget>, RuntimeStateError> {
        let Some(device_context) = self.device_context_option(device_context_handle)? else {
            return Ok(None);
        };
        if !matches!(
            device_context.target,
            RuntimeDeviceContextTarget::Memory { .. }
        ) || device_context.owner_thread_id != thread_id
        {
            return Ok(None);
        }
        self.validate_memory_dc_selections(device_context)?;
        let font_id = device_context
            .selected_font_id
            .expect("validated memory DC has a selected font");
        let bitmap_id = device_context
            .selected_bitmap_id
            .expect("validated memory DC has a selected bitmap");
        let font = self.objects.get(&font_id).ok_or_else(|| {
            RuntimeStateError::new(
                "logical memory DC selected font does not resolve to a live object",
            )
        })?;
        let bitmap = self.objects.get(&bitmap_id).ok_or_else(|| {
            RuntimeStateError::new(
                "logical memory DC selected bitmap does not resolve to a live object",
            )
        })?;
        let RuntimeGdiObjectKind::FontA { log_font } = &font.object else {
            return Err(RuntimeStateError::new(
                "text rasterization requires an explicitly created FontA object",
            ));
        };
        let RuntimeGdiObjectKind::DibSection {
            descriptor,
            backing,
        } = &bitmap.object
        else {
            return Err(RuntimeStateError::new(
                "text rasterization requires a selected DIB section",
            ));
        };
        Ok(Some(RuntimeGdiTextRasterTarget {
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
        }))
    }

    pub(super) fn validate_text_raster_completion(
        &self,
        target: &RuntimeGdiTextRasterTarget,
    ) -> Result<(), RuntimeStateError> {
        let current = self
            .text_raster_target(target.device_context_handle, target.owner_thread_id)?
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "pending text raster device context is no longer live or owned by its thread",
                )
            })?;
        if &current != target {
            return Err(RuntimeStateError::new(
                "selected text raster target changed while browser rasterization was pending",
            ));
        }
        target
            .backing
            .content_generation
            .checked_add(1)
            .ok_or_else(|| {
                RuntimeStateError::new("logical DIB text content generation is exhausted")
            })?;

        Ok(())
    }

    pub(super) fn publish_text_raster_completion(&mut self, target: &RuntimeGdiTextRasterTarget) {
        let bitmap = self
            .objects
            .get_mut(&target.dib_section_id)
            .expect("validated live DIB section");
        let RuntimeGdiObjectKind::DibSection { backing, .. } = &mut bitmap.object else {
            unreachable!("validated text raster bitmap kind")
        };
        backing.content_generation += 1;
    }

    pub(super) fn release_window_dc(
        &mut self,
        window_handle: u32,
        device_context_handle: u32,
        thread_id: RuntimeThreadId,
    ) -> Result<(), RuntimeStateError> {
        let device_context = self.device_context_by_handle(device_context_handle)?;
        if !matches!(
            device_context.target,
            RuntimeDeviceContextTarget::Window {
                window_handle: acquired_window_handle,
                ..
            } if acquired_window_handle == window_handle
        ) || device_context.owner_thread_id != thread_id
        {
            return Err(RuntimeStateError::new(
                "ReleaseDC requires the exact HWND, HDC, and acquiring guest thread relation",
            ));
        }
        self.remove_device_context(device_context_handle)
    }

    pub(super) fn release_graphics_surface_dc(
        &mut self,
        target: RuntimeGdiGraphicsSurfaceTarget,
        device_context_handle: u32,
        thread_id: RuntimeThreadId,
    ) -> Result<bool, RuntimeStateError> {
        validate_graphics_surface_target(target)?;
        if !self
            .device_context_handle_index
            .contains_key(&device_context_handle)
        {
            return Ok(false);
        }
        let device_context = self.device_context_by_handle(device_context_handle)?;
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
            return Ok(false);
        }
        self.remove_device_context(device_context_handle)?;
        Ok(true)
    }

    pub(super) fn release_screen_dc(
        &mut self,
        target: RuntimeGdiScreenTarget,
        device_context_handle: u32,
        thread_id: RuntimeThreadId,
    ) -> Result<(), RuntimeStateError> {
        self.validate_screen_dc_access(target, device_context_handle, thread_id)?;
        self.remove_device_context(device_context_handle)
    }

    pub(super) fn delete_compatible_dc(
        &mut self,
        device_context_handle: u32,
        thread_id: RuntimeThreadId,
    ) -> Result<(), RuntimeStateError> {
        let device_context = self.device_context_by_handle(device_context_handle)?;
        if !matches!(
            device_context.target,
            RuntimeDeviceContextTarget::Memory { .. }
        ) || device_context.owner_thread_id != thread_id
        {
            return Err(RuntimeStateError::new(
                "DeleteDC requires a compatible memory DC owned by the current guest thread",
            ));
        }
        self.validate_memory_dc_selections(device_context)?;
        self.remove_device_context(device_context_handle)
    }

    pub(super) fn validate_screen_dc_access(
        &self,
        target: RuntimeGdiScreenTarget,
        device_context_handle: u32,
        thread_id: RuntimeThreadId,
    ) -> Result<(), RuntimeStateError> {
        validate_screen_target(target)?;
        let device_context = self.device_context_by_handle(device_context_handle)?;
        if target.owner_thread_id != thread_id
            || device_context.owner_thread_id != thread_id
            || device_context.target
                != (RuntimeDeviceContextTarget::Screen {
                    monitor_handle: target.monitor_handle,
                    rect: target.rect,
                })
        {
            return Err(RuntimeStateError::new(
                "screen GDI access requires the exact HDC, primary monitor, and acquiring guest thread relation",
            ));
        }
        Ok(())
    }

    fn remove_device_context(&mut self, handle: u32) -> Result<(), RuntimeStateError> {
        let device_context = self.device_context_by_handle(handle)?;
        let id = device_context.id;
        if let RuntimeDeviceContextTarget::GraphicsSurface { surface_id, .. } =
            device_context.target
            && self.graphics_surface_dc_index.get(&surface_id) != Some(&device_context.id)
        {
            return Err(RuntimeStateError::new(
                "graphics surface HDC association disagrees with the live device context",
            ));
        }
        let surface_id = match device_context.target {
            RuntimeDeviceContextTarget::GraphicsSurface { surface_id, .. } => Some(surface_id),
            _ => None,
        };

        if let Some(surface_id) = surface_id {
            self.graphics_surface_dc_index.remove(&surface_id);
        }
        self.device_contexts.remove(&id);
        self.device_context_handle_index.remove(&handle);

        Ok(())
    }

    pub(super) fn fill_rect(
        &mut self,
        device_context_handle: u32,
        rect: RuntimeRect,
        brush_handle: u32,
        thread_id: RuntimeThreadId,
    ) -> Result<(), RuntimeStateError> {
        let device_context = self.device_context_by_handle(device_context_handle)?;
        if device_context.owner_thread_id != thread_id {
            return Err(RuntimeStateError::new(
                "FillRect requires the guest thread that acquired the logical HDC",
            ));
        }
        let RuntimeDeviceContextTarget::Window { surface_id, .. } = device_context.target else {
            return Err(RuntimeStateError::new(
                "window FillRect received a non-window HDC",
            ));
        };
        let brush = self.object_by_handle(brush_handle)?;
        let RuntimeGdiObjectKind::StockBrush { color_ref, .. } = &brush.object else {
            return Err(RuntimeStateError::new(
                "FillRect requires a logical brush object",
            ));
        };
        let surface = self.surfaces.get(&surface_id).ok_or_else(|| {
            RuntimeStateError::new("logical HDC does not resolve to its window surface")
        })?;
        validate_fill_rect(rect, surface)?;
        let empty = rect.left == rect.right || rect.top == rect.bottom;
        if !empty && surface.fills.len() >= RUNTIME_GDI_SURFACE_FILL_LIMIT {
            return Err(RuntimeStateError::new(
                "logical GDI surface fill history capacity is exhausted",
            ));
        }
        let fill = (!empty).then_some(RuntimeGdiSurfaceFill {
            rect,
            color_ref: *color_ref,
        });
        if let Some(fill) = fill {
            let surface = self
                .surfaces
                .get_mut(&surface_id)
                .expect("validated GDI surface");
            surface.fills.push(fill);
        }
        Ok(())
    }

    pub(super) fn graphics_surface_fill_color(
        &self,
        device_context_handle: u32,
        rect: RuntimeRect,
        brush_handle: u32,
        thread_id: RuntimeThreadId,
    ) -> Result<u32, RuntimeStateError> {
        let device_context = self.device_context_by_handle(device_context_handle)?;
        if device_context.owner_thread_id != thread_id {
            return Err(RuntimeStateError::new(
                "FillRect requires the guest thread that acquired the logical HDC",
            ));
        }
        let RuntimeDeviceContextTarget::GraphicsSurface { width, height, .. } =
            device_context.target
        else {
            return Err(RuntimeStateError::new(
                "graphics FillRect planning received a window HDC",
            ));
        };
        let brush = self.object_by_handle(brush_handle)?;
        let RuntimeGdiObjectKind::StockBrush { color_ref, .. } = &brush.object else {
            return Err(RuntimeStateError::new(
                "FillRect requires a logical brush object",
            ));
        };
        validate_fill_rect_bounds(rect, width, height)?;
        Ok(*color_ref)
    }

    pub(super) fn device_context_option(
        &self,
        handle: u32,
    ) -> Result<Option<&RuntimeDeviceContext>, RuntimeStateError> {
        let Some(id) = self.device_context_handle_index.get(&handle).copied() else {
            return Ok(None);
        };
        let device_context = self.device_contexts.get(&id).ok_or_else(|| {
            RuntimeStateError::new("logical HDC index does not resolve to a live device context")
        })?;
        if device_context.handle != handle || device_context.id != id {
            return Err(RuntimeStateError::new(
                "logical HDC index and device context identity disagree",
            ));
        }
        Ok(Some(device_context))
    }

    pub fn device_context_by_handle(
        &self,
        handle: u32,
    ) -> Result<&RuntimeDeviceContext, RuntimeStateError> {
        self.device_context_option(handle)?.ok_or_else(|| {
            RuntimeStateError::new(format!("logical HDC {handle:#010x} is not live"))
        })
    }

    pub(super) fn object_option(
        &self,
        handle: u32,
    ) -> Result<Option<&RuntimeGdiObject>, RuntimeStateError> {
        let Some(id) = self.object_handle_index.get(&handle).copied() else {
            return Ok(None);
        };
        let object = self.objects.get(&id).ok_or_else(|| {
            RuntimeStateError::new(
                "logical GDI object handle index does not resolve to a live object",
            )
        })?;
        if object.id != id || object.handle != handle {
            return Err(RuntimeStateError::new(
                "logical GDI object handle index disagrees with the object identity",
            ));
        }
        Ok(Some(object))
    }

    pub fn object_by_handle(&self, handle: u32) -> Result<&RuntimeGdiObject, RuntimeStateError> {
        self.object_option(handle)?.ok_or_else(|| {
            RuntimeStateError::new(format!("logical GDI object {handle:#010x} is not live"))
        })
    }

    #[cfg(test)]
    pub(crate) fn device_contexts(
        &self,
    ) -> &BTreeMap<RuntimeDeviceContextId, RuntimeDeviceContext> {
        &self.device_contexts
    }

    #[cfg(test)]
    pub(crate) fn objects(&self) -> &BTreeMap<RuntimeGdiObjectId, RuntimeGdiObject> {
        &self.objects
    }

    #[cfg(test)]
    pub(crate) fn surfaces(&self) -> &BTreeMap<RuntimeGdiSurfaceId, RuntimeGdiSurface> {
        &self.surfaces
    }

    pub(crate) fn validate_com_object_retirements(
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
            return Err(RuntimeStateError::new(
                "logical graphics surface cannot retire while its HDC is outstanding",
            ));
        }
        Ok(())
    }

    fn stock_object_by_index(
        &self,
        index: i32,
    ) -> Result<Option<&RuntimeGdiObject>, RuntimeStateError> {
        let Some(object_id) = self.stock_object_index.get(&index) else {
            return Ok(None);
        };
        let object = self.objects.get(object_id).ok_or_else(|| {
            RuntimeStateError::new("GDI stock object index does not resolve to a live object")
        })?;
        let matching_index = match &object.object {
            RuntimeGdiObjectKind::StockFont {
                index: stored_index,
            }
            | RuntimeGdiObjectKind::StockBitmap {
                index: stored_index,
                ..
            }
            | RuntimeGdiObjectKind::StockBrush {
                index: stored_index,
                ..
            } => *stored_index == index,
            _ => false,
        };
        if object.id != *object_id || !object.shared || !matching_index {
            return Err(RuntimeStateError::new(
                "GDI stock object index does not match a shared live stock identity",
            ));
        }
        if self.object_handle_index.get(&object.handle) != Some(object_id) {
            return Err(RuntimeStateError::new(
                "GDI stock object handle index does not resolve to the same object",
            ));
        }
        Ok(Some(object))
    }

    fn validate_memory_dc_selections(
        &self,
        device_context: &RuntimeDeviceContext,
    ) -> Result<(), RuntimeStateError> {
        let Some(font_id) = device_context.selected_font_id else {
            return Err(RuntimeStateError::new(
                "logical memory DC has no selected font identity",
            ));
        };
        let Some(bitmap_id) = device_context.selected_bitmap_id else {
            return Err(RuntimeStateError::new(
                "logical memory DC has no selected bitmap identity",
            ));
        };
        let font = self.objects.get(&font_id).ok_or_else(|| {
            RuntimeStateError::new(
                "logical memory DC selected font does not resolve to a live object",
            )
        })?;
        validate_selectable_font_object(font)?;
        let bitmap = self.objects.get(&bitmap_id).ok_or_else(|| {
            RuntimeStateError::new(
                "logical memory DC selected bitmap does not resolve to a live object",
            )
        })?;
        validate_selectable_bitmap_object(bitmap)
    }

    fn try_plan_object_allocation(
        &self,
    ) -> Result<Option<RuntimeGdiObjectAllocation>, RuntimeStateError> {
        if self.objects.len() >= RUNTIME_GDI_OBJECT_LIMIT {
            return Ok(None);
        }
        self.try_checked_object_allocation(self.next_object_id, self.next_object_handle)
    }

    fn try_plan_object_allocations(
        &self,
        count: usize,
    ) -> Result<Option<Vec<RuntimeGdiObjectAllocation>>, RuntimeStateError> {
        if count == 0 {
            return Ok(Some(Vec::new()));
        }
        if self
            .objects
            .len()
            .checked_add(count)
            .is_none_or(|length| length > RUNTIME_GDI_OBJECT_LIMIT)
        {
            return Ok(None);
        }
        let mut allocations = Vec::with_capacity(count);
        let mut next_id = self.next_object_id;
        let mut next_handle = self.next_object_handle;
        for _ in 0..count {
            let Some(allocation) = self.try_checked_object_allocation(next_id, next_handle)? else {
                return Ok(None);
            };
            next_id = allocation.next_id;
            next_handle = allocation.next_handle;
            allocations.push(allocation);
        }
        Ok(Some(allocations))
    }

    fn try_checked_object_allocation(
        &self,
        next_id: u64,
        handle: u32,
    ) -> Result<Option<RuntimeGdiObjectAllocation>, RuntimeStateError> {
        let id = RuntimeGdiObjectId(next_id);
        if self.objects.contains_key(&id)
            || handle < RUNTIME_GDI_OBJECT_HANDLE_BASE
            || self.object_handle_index.contains_key(&handle)
        {
            return Err(RuntimeStateError::new(
                "logical GDI object allocator is not at a free identity and handle pair",
            ));
        }
        let (Some(next_id), Some(next_handle)) = (next_id.checked_add(1), handle.checked_add(1))
        else {
            return Ok(None);
        };
        Ok(Some(RuntimeGdiObjectAllocation {
            id,
            handle,
            next_id,
            next_handle,
        }))
    }

    fn plan_device_context_allocation(
        &self,
    ) -> Result<RuntimeDeviceContextAllocation, RuntimeStateError> {
        self.try_plan_device_context_allocation()?.ok_or_else(|| {
            RuntimeStateError::new("logical device-context capacity or identity space is exhausted")
        })
    }

    fn try_plan_device_context_allocation(
        &self,
    ) -> Result<Option<RuntimeDeviceContextAllocation>, RuntimeStateError> {
        if self.device_contexts.len() >= RUNTIME_GDI_DEVICE_CONTEXT_LIMIT {
            return Ok(None);
        }
        let id = RuntimeDeviceContextId(self.next_device_context_id);
        let handle = self.next_device_context_handle;
        if handle < RUNTIME_DEVICE_CONTEXT_HANDLE_BASE
            || self.device_context_handle_index.contains_key(&handle)
            || self.device_contexts.contains_key(&id)
        {
            return Err(RuntimeStateError::new(
                "logical device-context handle allocator is not at a free namespace value",
            ));
        }
        let (Some(next_id), Some(next_handle)) = (
            self.next_device_context_id.checked_add(1),
            handle.checked_add(1),
        ) else {
            return Ok(None);
        };
        Ok(Some(RuntimeDeviceContextAllocation {
            id,
            handle,
            next_id,
            next_handle,
        }))
    }
}

fn validate_default_font_object(object: &RuntimeGdiObject) -> Result<(), RuntimeStateError> {
    if !object.shared
        || !matches!(
            object.object,
            RuntimeGdiObjectKind::StockFont {
                index: RUNTIME_GDI_SYSTEM_FONT_INDEX,
            }
        )
    {
        return Err(RuntimeStateError::new(
            "logical memory DC default font identity is not the shared system font",
        ));
    }
    Ok(())
}

fn validate_default_bitmap_object(object: &RuntimeGdiObject) -> Result<(), RuntimeStateError> {
    if !object.shared
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
        return Err(RuntimeStateError::new(
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
        | RuntimeGdiObjectKind::FontA { .. } => Err(RuntimeStateError::new(
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
                return Err(RuntimeStateError::new(
                    "logical DIB section backing has no guest-memory owner",
                ));
            }
            Ok(())
        }
        RuntimeGdiObjectKind::StockBrush { .. }
        | RuntimeGdiObjectKind::StockFont { .. }
        | RuntimeGdiObjectKind::FontA { .. }
        | RuntimeGdiObjectKind::DibSection { .. } => Err(RuntimeStateError::new(
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
        return Err(RuntimeStateError::new(
            "logical DIB section requires a nonempty 8bpp BI_RGB shape and 256 RGBQUAD colors",
        ));
    }
    let width = u32::try_from(descriptor.width).map_err(|_| {
        RuntimeStateError::new("logical DIB section width must be a positive x86 integer")
    })?;
    let height = u32::try_from(descriptor.height.abs()).map_err(|_| {
        RuntimeStateError::new("logical DIB section height must fit unsigned x86 pixel accounting")
    })?;
    let stride_bytes = width
        .checked_mul(u32::from(descriptor.bits_per_pixel))
        .and_then(|bits| bits.checked_add(31))
        .map(|bits| bits / 32)
        .and_then(|words| words.checked_mul(4))
        .ok_or_else(|| {
            RuntimeStateError::new("logical DIB section stride overflowed x86 accounting")
        })?;
    let pixel_byte_length = stride_bytes.checked_mul(height).ok_or_else(|| {
        RuntimeStateError::new("logical DIB section pixel byte length overflowed x86 accounting")
    })?;
    if descriptor.stride_bytes != stride_bytes
        || descriptor.pixel_byte_length != pixel_byte_length
        || pixel_byte_length == 0
    {
        return Err(RuntimeStateError::new(
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
        return Err(RuntimeStateError::new(
            "logical DIB section backing must be allocation-owned, page-aligned, and large enough",
        ));
    }
    Ok(())
}

fn validate_log_font_a(log_font: &RuntimeLogFontA) -> Result<(), RuntimeStateError> {
    if log_font.face_name_windows_932.len() >= RUNTIME_LOG_FONT_A_FACE_CAPACITY
        || log_font.face_name_windows_932.contains(&0)
    {
        return Err(RuntimeStateError::new(
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
        return Err(RuntimeStateError::new(
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
        return Err(RuntimeStateError::new(
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
        return Err(RuntimeStateError::new(
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
        RuntimeStateError::new("logical GDI surface width does not fit x86 coordinates")
    })?;
    let height = i32::try_from(surface_height).map_err(|_| {
        RuntimeStateError::new("logical GDI surface height does not fit x86 coordinates")
    })?;
    if rect.left < 0
        || rect.top < 0
        || rect.right < rect.left
        || rect.bottom < rect.top
        || rect.right > width
        || rect.bottom > height
    {
        return Err(RuntimeStateError::new(
            "bounded FillRect requires an ordered rectangle inside the logical client surface",
        ));
    }
    Ok(())
}

fn checked_next_identity(value: u64) -> Result<u64, RuntimeStateError> {
    value
        .checked_add(1)
        .ok_or_else(|| RuntimeStateError::new("logical GDI identity sequence is exhausted"))
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
    fn device_context_targets_keep_their_typed_owner_and_surface() {
        let mut gdi = RuntimeGdi::default();
        let window = gdi.get_window_dc(target()).unwrap();
        let window_dc = gdi.device_context_by_handle(window.handle).unwrap();
        assert_eq!(window_dc.id, RuntimeDeviceContextId(1));
        assert_eq!(window_dc.owner_thread_id, RuntimeThreadId(8192));
        assert_eq!(
            window_dc.target,
            RuntimeDeviceContextTarget::Window {
                window_id: RuntimeWindowId(1),
                window_handle: 0x4801_0001,
                surface_id: RuntimeGdiSurfaceId(1),
            }
        );

        let graphics = gdi
            .get_graphics_surface_dc(graphics_target(7, 5))
            .unwrap()
            .unwrap();
        let graphics_dc = gdi.device_context_by_handle(graphics).unwrap();
        assert_eq!(graphics_dc.id, RuntimeDeviceContextId(2));
        assert_eq!(graphics_dc.owner_thread_id, RuntimeThreadId(8192));
        assert_eq!(
            graphics_dc.target,
            RuntimeDeviceContextTarget::GraphicsSurface {
                surface_id: RuntimeGraphicsSurfaceId(7),
                surface_com_object_id: RuntimeComObjectId(5),
                width: 1024,
                height: 1024,
                color_format: RuntimeGraphicsColorFormat::B8g8r8x8Unorm,
            }
        );
    }

    #[test]
    fn screen_device_context_acquisitions_are_distinct_typed_and_relation_checked() {
        let mut gdi = RuntimeGdi::default();
        let invalid_before = gdi.clone();
        let mut invalid_target = screen_target();
        invalid_target.monitor_handle = 0;
        gdi.get_screen_dc(invalid_target).unwrap_err();
        assert_eq!(gdi, invalid_before);

        let first = gdi.get_screen_dc(screen_target()).unwrap();
        assert_eq!(first, RUNTIME_DEVICE_CONTEXT_HANDLE_BASE);
        let screen_dc = gdi.device_context_by_handle(first).unwrap();
        assert_eq!(screen_dc.id, RuntimeDeviceContextId(1));
        assert_eq!(screen_dc.owner_thread_id, RuntimeThreadId(8192));
        assert_eq!(
            screen_dc.target,
            RuntimeDeviceContextTarget::Screen {
                monitor_handle: screen_target().monitor_handle,
                rect: screen_target().rect,
            }
        );
        let second = gdi.get_screen_dc(screen_target()).unwrap();
        assert_ne!(first, second);
        assert!(gdi.surfaces.is_empty());

        let before_wrong_release = gdi.clone();
        let mut wrong_target = screen_target();
        wrong_target.monitor_handle += 1;
        gdi.release_screen_dc(wrong_target, first, screen_target().owner_thread_id)
            .unwrap_err();
        assert_eq!(gdi, before_wrong_release);

        let before_wrong_thread = gdi.clone();
        gdi.release_screen_dc(screen_target(), first, RuntimeThreadId(8193))
            .unwrap_err();
        assert_eq!(gdi, before_wrong_thread);

        gdi.release_screen_dc(screen_target(), first, screen_target().owner_thread_id)
            .unwrap();
        assert!(
            gdi.release_screen_dc(screen_target(), first, screen_target().owner_thread_id)
                .is_err()
        );
        assert!(gdi.device_context_by_handle(first).is_err());
        assert!(gdi.device_context_by_handle(second).is_ok());
    }

    #[test]
    fn forged_hdc_alias_cannot_retire_the_original_context() {
        let mut gdi = RuntimeGdi::default();
        let dc = gdi.get_screen_dc(screen_target()).unwrap();
        let alias = dc + 1;
        let id = gdi.device_context_by_handle(dc).unwrap().id;
        assert!(gdi.device_context_option(alias).unwrap().is_none());
        gdi.device_context_handle_index.insert(alias, id);
        let before = gdi.clone();
        gdi.device_context_option(alias).unwrap_err();
        gdi.device_context_by_handle(alias).unwrap_err();
        gdi.validate_screen_dc_access(screen_target(), alias, screen_target().owner_thread_id)
            .unwrap_err();
        gdi.release_screen_dc(screen_target(), alias, screen_target().owner_thread_id)
            .unwrap_err();
        assert_eq!(gdi, before);
        assert!(gdi.device_context_by_handle(dc).is_ok());
    }

    #[test]
    fn screen_dc_allocator_rejects_an_occupied_id_without_overwriting_the_first_dc() {
        let mut gdi = RuntimeGdi::default();
        let first = gdi.get_screen_dc(screen_target()).unwrap();
        let id = gdi.device_context_by_handle(first).unwrap().id;
        gdi.next_device_context_id = id.0;
        let before = gdi.clone();
        assert!(gdi.get_screen_dc(screen_target()).is_err());
        assert_eq!(gdi, before);
        assert_eq!(gdi.device_context_by_handle(first).unwrap().id, id);
    }

    #[test]
    fn compatible_memory_dcs_share_defaults_and_delete_exact_owner_identity_atomically() {
        let mut gdi = RuntimeGdi::default();
        let mut invalid_target = screen_target();
        invalid_target.monitor_handle = 0;
        let before_invalid = gdi.clone();
        gdi.create_compatible_dc(invalid_target).unwrap_err();
        assert_eq!(gdi, before_invalid);

        let first = gdi.create_compatible_dc(screen_target()).unwrap().unwrap();
        assert_eq!(first, RUNTIME_DEVICE_CONTEXT_HANDLE_BASE);
        assert_eq!(gdi.objects.len(), 2);
        let memory_dc = gdi.device_context_by_handle(first).unwrap();
        validate_default_font_object(&gdi.objects[&memory_dc.selected_font_id.unwrap()]).unwrap();
        validate_default_bitmap_object(&gdi.objects[&memory_dc.selected_bitmap_id.unwrap()])
            .unwrap();
        assert_eq!(memory_dc.id, RuntimeDeviceContextId(1));
        assert_eq!(memory_dc.owner_thread_id, RuntimeThreadId(8192));
        assert_eq!(
            memory_dc.target,
            RuntimeDeviceContextTarget::Memory {
                compatible_monitor_handle: screen_target().monitor_handle,
                compatible_rect: screen_target().rect,
            }
        );
        assert_eq!(memory_dc.selected_font_id, Some(RuntimeGdiObjectId(1)));
        assert_eq!(memory_dc.selected_bitmap_id, Some(RuntimeGdiObjectId(2)));
        assert_eq!(memory_dc.selection_generation, 0);
        let defaults = (memory_dc.selected_font_id, memory_dc.selected_bitmap_id);
        let second = gdi.create_compatible_dc(screen_target()).unwrap().unwrap();
        assert_eq!(second, RUNTIME_DEVICE_CONTEXT_HANDLE_BASE + 1);
        let second_dc = gdi.device_context_by_handle(second).unwrap();
        assert_eq!(
            (second_dc.selected_font_id, second_dc.selected_bitmap_id),
            defaults
        );
        assert_eq!(gdi.objects.len(), 2);

        let before_wrong_thread = gdi.clone();
        gdi.delete_compatible_dc(first, RuntimeThreadId(8193))
            .unwrap_err();
        assert_eq!(gdi, before_wrong_thread);

        let screen = gdi.get_screen_dc(screen_target()).unwrap();
        let before_wrong_target = gdi.clone();
        gdi.delete_compatible_dc(screen, screen_target().owner_thread_id)
            .unwrap_err();
        assert_eq!(gdi, before_wrong_target);

        gdi.delete_compatible_dc(first, screen_target().owner_thread_id)
            .unwrap();
        assert!(
            gdi.delete_compatible_dc(first, screen_target().owner_thread_id)
                .is_err()
        );
        assert!(gdi.device_context_by_handle(first).is_err());
        assert!(gdi.device_context_by_handle(second).is_ok());
        assert_eq!(gdi.objects.len(), 2);
    }

    #[test]
    fn compatible_memory_dc_multi_object_preflight_preserves_all_allocators() {
        let mut gdi = RuntimeGdi {
            next_object_handle: u32::MAX - 1,
            ..RuntimeGdi::default()
        };
        let before = gdi.clone();
        assert!(gdi.create_compatible_dc(screen_target()).unwrap().is_none());
        assert_eq!(gdi, before);
    }

    #[test]
    fn selected_font_query_requires_same_thread_memory_dc_and_created_font() {
        let mut gdi = RuntimeGdi::default();
        let device_context = gdi.create_compatible_dc(screen_target()).unwrap().unwrap();
        let before_stock = gdi.clone();
        gdi.selected_font_a(device_context, screen_target().owner_thread_id)
            .unwrap_err();
        assert_eq!(gdi, before_stock);
        gdi.selected_font_a(device_context, RuntimeThreadId(8193))
            .unwrap_err();
        assert_eq!(gdi, before_stock);

        let created = gdi.create_font_a(font()).unwrap().unwrap();
        gdi.select_object(device_context, created, screen_target().owner_thread_id)
            .unwrap()
            .unwrap();
        let before_query = gdi.clone();
        assert_eq!(
            gdi.selected_font_a(device_context, screen_target().owner_thread_id)
                .unwrap(),
            &font()
        );
        assert_eq!(gdi, before_query);
    }

    #[test]
    fn select_object_checks_owner_before_mutation_and_does_not_advance_on_reselection() {
        let mut gdi = RuntimeGdi::default();
        let dc = gdi.create_compatible_dc(screen_target()).unwrap().unwrap();
        let font = gdi.create_font_a(font()).unwrap().unwrap();
        let before = gdi.clone();
        assert!(
            gdi.select_object(dc, font, RuntimeThreadId(8193))
                .unwrap()
                .is_none()
        );
        assert!(
            gdi.select_object(dc, u32::MAX, screen_target().owner_thread_id)
                .unwrap()
                .is_none()
        );
        assert_eq!(gdi, before);

        let selected = gdi
            .select_object(dc, font, screen_target().owner_thread_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            gdi.device_context_by_handle(dc).unwrap().selected_font_id,
            Some(gdi.object_by_handle(font).unwrap().id)
        );
        assert_ne!(selected, font);
        let after = gdi.clone();
        let repeated = gdi
            .select_object(dc, font, screen_target().owner_thread_id)
            .unwrap()
            .unwrap();
        assert_eq!(repeated, font);
        assert_eq!(gdi, after);
    }

    #[test]
    fn ordinary_object_failures_preserve_state_but_corrupt_indexes_fault() {
        let mut gdi = RuntimeGdi::default();
        let thread = screen_target().owner_thread_id;
        let dc = gdi.create_compatible_dc(screen_target()).unwrap().unwrap();
        let font = gdi.create_font_a(font()).unwrap().unwrap();
        let stock = gdi
            .get_stock_object(RUNTIME_GDI_BLACK_BRUSH_INDEX)
            .unwrap()
            .unwrap();
        gdi.select_object(dc, font, thread).unwrap().unwrap();
        let before = gdi.clone();
        for (target_dc, target_font, owner) in [
            (0, font, thread),
            (dc, 0, thread),
            (dc, stock, thread),
            (dc, font, RuntimeThreadId(thread.0 + 1)),
        ] {
            assert!(
                gdi.select_object(target_dc, target_font, owner)
                    .unwrap()
                    .is_none()
            );
        }
        for handle in [0, stock, font] {
            assert!(gdi.delete_object(handle, |_| Ok(())).unwrap().is_none());
        }
        assert!(gdi.object_bytes(font, 1).unwrap().is_none());
        assert!(
            gdi.object_bytes(0, RUNTIME_LOG_FONT_A_BYTE_LENGTH as u32)
                .unwrap()
                .is_none()
        );
        assert!(gdi.text_raster_target(0, thread).unwrap().is_none());
        assert_eq!(gdi, before);

        let alias = font + 1;
        let font_id = gdi.object_by_handle(font).unwrap().id;
        gdi.object_handle_index.insert(alias, font_id);
        let corrupt_before = gdi.clone();
        assert!(gdi.select_object(dc, alias, thread).is_err());
        assert!(gdi.delete_object(alias, |_| Ok(())).is_err());
        assert!(
            gdi.object_bytes(alias, RUNTIME_LOG_FONT_A_BYTE_LENGTH as u32)
                .is_err()
        );
        assert_eq!(gdi, corrupt_before);

        gdi.object_handle_index.remove(&alias);
        let dc_id = gdi.device_context_by_handle(dc).unwrap().id;
        gdi.device_context_handle_index.insert(dc + 1, dc_id);
        let corrupt_before = gdi.clone();
        assert!(gdi.select_object(dc + 1, font, thread).is_err());
        assert!(gdi.text_raster_target(dc + 1, thread).is_err());
        assert_eq!(gdi, corrupt_before);
    }

    #[test]
    fn dib_section_acquisition_validates_before_publishing_objects() {
        let mut gdi = RuntimeGdi::default();
        let dc = gdi.create_compatible_dc(screen_target()).unwrap().unwrap();
        let backing = RuntimeGdiDibSectionBacking {
            address_allocation_id: ProcessAddressAllocationId(1),
            memory_owner: MemoryOwnerId(1),
            range: GuestRange::new(GuestAddress(0x7200_0000), 0x3000).unwrap(),
            content_generation: 0,
        };
        let before = gdi.clone();
        assert!(
            gdi.create_dib_section(
                0,
                screen_target().owner_thread_id,
                dib_descriptor(),
                backing.address_allocation_id,
                backing.range,
                || panic!("invalid HDC must be rejected before allocating memory"),
            )
            .is_err()
        );
        assert_eq!(gdi, before);
        assert!(
            gdi.create_dib_section(
                dc,
                screen_target().owner_thread_id,
                dib_descriptor(),
                backing.address_allocation_id,
                backing.range,
                || Err(RuntimeStateError::new("memory installation failed")),
            )
            .is_err()
        );
        assert_eq!(gdi, before);
        for mut exhausted in [
            RuntimeGdi {
                next_object_id: u64::MAX,
                ..gdi.clone()
            },
            RuntimeGdi {
                next_object_handle: u32::MAX,
                ..gdi.clone()
            },
        ] {
            let before = exhausted.clone();
            assert!(
                exhausted
                    .create_dib_section(
                        dc,
                        screen_target().owner_thread_id,
                        dib_descriptor(),
                        backing.address_allocation_id,
                        backing.range,
                        || panic!("exhausted GDI capacity must not allocate memory"),
                    )
                    .unwrap()
                    .is_none()
            );
            assert_eq!(exhausted, before);
        }
        gdi.create_dib_section(
            dc,
            screen_target().owner_thread_id,
            dib_descriptor(),
            backing.address_allocation_id,
            backing.range,
            || Ok(Some(backing.memory_owner)),
        )
        .unwrap();
        let before_duplicate = gdi.clone();
        assert!(
            gdi.create_dib_section(
                dc,
                screen_target().owner_thread_id,
                dib_descriptor(),
                backing.address_allocation_id,
                backing.range,
                || panic!("duplicate backing must be rejected before allocating memory"),
            )
            .is_err()
        );
        assert_eq!(gdi, before_duplicate);
    }

    #[test]
    fn text_raster_plan_requires_exact_font_dib_selection_and_advances_content_once() {
        let mut gdi = RuntimeGdi::default();
        let device_context = gdi.create_compatible_dc(screen_target()).unwrap().unwrap();
        let created_font = gdi.create_font_a(font()).unwrap().unwrap();
        let created_dib = gdi
            .create_dib_section(
                device_context,
                screen_target().owner_thread_id,
                dib_descriptor(),
                ProcessAddressAllocationId(1),
                GuestRange::new(GuestAddress(0x7200_0000), 0x3000).unwrap(),
                || Ok(Some(MemoryOwnerId(1))),
            )
            .unwrap()
            .unwrap();

        let before_stock_font = gdi.clone();
        gdi.text_raster_target(device_context, screen_target().owner_thread_id)
            .unwrap_err();
        assert_eq!(gdi, before_stock_font);
        gdi.select_object(
            device_context,
            created_font,
            screen_target().owner_thread_id,
        )
        .unwrap()
        .unwrap();
        let before_stock_bitmap = gdi.clone();
        gdi.text_raster_target(device_context, screen_target().owner_thread_id)
            .unwrap_err();
        assert_eq!(gdi, before_stock_bitmap);
        gdi.select_object(device_context, created_dib, screen_target().owner_thread_id)
            .unwrap()
            .unwrap();

        let target = gdi
            .text_raster_target(device_context, screen_target().owner_thread_id)
            .unwrap()
            .unwrap();
        assert_eq!(target.log_font, font());
        assert_eq!(target.descriptor, dib_descriptor());
        assert_eq!(target.backing.content_generation, 0);
        gdi.validate_text_raster_completion(&target).unwrap();
        gdi.publish_text_raster_completion(&target);
        assert_eq!(
            gdi.device_context_by_handle(device_context)
                .unwrap()
                .selection_generation,
            2
        );
        assert_eq!(
            gdi.text_raster_target(device_context, screen_target().owner_thread_id)
                .unwrap()
                .unwrap()
                .backing
                .content_generation,
            1
        );
        gdi.validate_text_raster_completion(&target)
            .expect_err("the previous content generation cannot complete twice");
        assert_eq!(
            gdi.text_raster_target(device_context, screen_target().owner_thread_id)
                .unwrap()
                .unwrap()
                .backing
                .content_generation,
            1
        );
        assert!(
            gdi.text_raster_target(device_context, RuntimeThreadId(8193))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn window_dc_acquisition_checks_capacity_and_surface_identity_before_publication() {
        for (exhausted, mut gdi) in [
            RuntimeGdi {
                next_surface_id: u64::MAX,
                ..RuntimeGdi::default()
            },
            RuntimeGdi {
                next_device_context_id: u64::MAX,
                ..RuntimeGdi::default()
            },
            RuntimeGdi {
                next_device_context_handle: u32::MAX,
                ..RuntimeGdi::default()
            },
        ]
        .into_iter()
        .enumerate()
        {
            assert!(gdi.preview_window_dc(target()).is_err());
            assert!(gdi.get_window_dc(target()).is_err());
            assert!(gdi.surfaces.is_empty());
            assert!(gdi.window_surface_index.is_empty());
            assert!(gdi.device_contexts.is_empty());
            assert!(gdi.device_context_handle_index.is_empty());
            match exhausted {
                0 => gdi.next_surface_id = 1,
                1 => gdi.next_device_context_id = 1,
                2 => gdi.next_device_context_handle = RUNTIME_DEVICE_CONTEXT_HANDLE_BASE,
                _ => unreachable!(),
            }
            let acquired = gdi.get_window_dc(target()).unwrap();
            assert_eq!(acquired.surface_id.0, 1);
            assert_eq!(acquired.device_context_id.0, 1);
            assert_eq!(acquired.handle, RUNTIME_DEVICE_CONTEXT_HANDLE_BASE);
        }

        let mut gdi = RuntimeGdi::default();
        let first = gdi.get_window_dc(target()).unwrap();
        let corrupt_id = RuntimeGdiSurfaceId(first.surface_id.0 + 1);
        gdi.surfaces.get_mut(&first.surface_id).unwrap().id = corrupt_id;
        gdi.get_window_dc(target())
            .expect_err("a live surface must agree with its index");
        assert_eq!(gdi.surfaces[&first.surface_id].id, corrupt_id);
        assert_eq!(gdi.surfaces.len(), 1);
        assert_eq!(
            gdi.window_surface_index[&target().window_id],
            first.surface_id
        );
        assert_eq!(gdi.device_contexts.len(), 1);
        assert_eq!(
            gdi.device_context_by_handle(first.handle).unwrap().id,
            first.device_context_id
        );
        gdi.surfaces.get_mut(&first.surface_id).unwrap().id = first.surface_id;
        let second = gdi.get_window_dc(target()).unwrap();
        assert_eq!(second.surface_id, first.surface_id);
        assert_eq!(second.device_context_id.0, first.device_context_id.0 + 1);
        assert_eq!(second.handle, first.handle + 1);
    }

    #[test]
    fn window_dc_rejects_missing_surface_index_without_consuming_identity() {
        let mut gdi = RuntimeGdi::default();
        let first = gdi.get_window_dc(target()).unwrap();
        gdi.window_surface_index.remove(&target().window_id);
        let surface = gdi.surfaces[&first.surface_id].clone();
        gdi.get_window_dc(target())
            .expect_err("a live unindexed surface is corruption, not absence");
        assert_eq!(gdi.surfaces[&first.surface_id], surface);
        assert_eq!(gdi.device_contexts.len(), 1);
        gdi.window_surface_index
            .insert(target().window_id, first.surface_id);
        let retry = gdi.get_window_dc(target()).unwrap();
        assert_eq!(retry.device_context_id.0, first.device_context_id.0 + 1);
        assert_eq!(retry.handle, first.handle + 1);
        assert_eq!(retry.surface_id, first.surface_id);
        assert_eq!(gdi.surfaces.len(), 1);
    }

    #[test]
    fn device_context_acquisitions_are_distinct_and_release_is_relation_checked() {
        let mut gdi = RuntimeGdi::default();
        let preview = gdi.preview_window_dc(target()).unwrap();
        assert!(gdi.device_contexts.is_empty());
        assert!(gdi.device_context_handle_index.is_empty());
        assert!(gdi.surfaces.is_empty());
        assert!(gdi.window_surface_index.is_empty());
        let first = gdi.get_window_dc(target()).unwrap();
        assert_eq!(first, preview);
        assert_eq!(gdi.surfaces.len(), 1);
        assert_eq!(first.handle, RUNTIME_DEVICE_CONTEXT_HANDLE_BASE);

        let second = gdi.get_window_dc(target()).unwrap();
        assert_ne!(first.handle, second.handle);
        assert_eq!(first.surface_id, second.surface_id);
        assert_eq!(gdi.surfaces.len(), 1);
        let surface = gdi.surfaces[&first.surface_id].clone();
        for (window_handle, owner) in [
            (0x4801_0002, target().owner_thread_id),
            (target().window_handle, RuntimeThreadId(8193)),
        ] {
            gdi.release_window_dc(window_handle, first.handle, owner)
                .expect_err("ReleaseDC requires the original window and acquiring thread");
            assert_eq!(
                gdi.device_context_by_handle(first.handle).unwrap().id,
                first.device_context_id
            );
            assert_eq!(
                gdi.device_context_by_handle(second.handle).unwrap().id,
                second.device_context_id
            );
            assert_eq!(gdi.device_contexts.len(), 2);
            assert_eq!(gdi.surfaces[&first.surface_id], surface);
        }

        gdi.release_window_dc(
            target().window_handle,
            first.handle,
            target().owner_thread_id,
        )
        .unwrap();
        assert!(
            gdi.release_window_dc(
                target().window_handle,
                first.handle,
                target().owner_thread_id
            )
            .is_err()
        );
        assert!(gdi.device_context_by_handle(first.handle).is_err());
        assert!(gdi.device_context_by_handle(second.handle).is_ok());
        assert_eq!(gdi.surfaces[&first.surface_id], surface);
        gdi.release_window_dc(
            target().window_handle,
            second.handle,
            target().owner_thread_id,
        )
        .unwrap();
        assert!(gdi.device_contexts.is_empty());
        assert!(gdi.device_context_handle_index.is_empty());
        let reacquired = gdi.get_window_dc(target()).unwrap();
        assert_eq!(reacquired.surface_id, first.surface_id);
        assert_eq!(
            reacquired.device_context_id.0,
            second.device_context_id.0 + 1
        );
        assert_eq!(reacquired.handle, second.handle + 1);
        assert_eq!(gdi.surfaces[&first.surface_id], surface);
    }

    #[test]
    fn stock_reuse_rejects_corrupt_live_identity_and_memory_dc_defaults() {
        for damage in 0..4 {
            let mut gdi = RuntimeGdi::default();
            let handle = gdi
                .get_stock_object(RUNTIME_GDI_BLACK_BRUSH_INDEX)
                .unwrap()
                .unwrap();
            let id = gdi.object_by_handle(handle).unwrap().id;
            let object = gdi.objects.get_mut(&id).unwrap();
            match damage {
                0 => object.id.0 += 1,
                1 => object.shared = false,
                2 => object.object = RuntimeGdiObjectKind::FontA { log_font: font() },
                3 => {
                    object.object = RuntimeGdiObjectKind::StockBrush {
                        index: RUNTIME_GDI_BLACK_BRUSH_INDEX + 1,
                        color_ref: 0,
                    }
                }
                _ => unreachable!(),
            }
            let before = gdi.clone();
            gdi.get_stock_object(RUNTIME_GDI_BLACK_BRUSH_INDEX)
                .unwrap_err();
            assert_eq!(gdi, before);
        }

        let mut gdi = RuntimeGdi::default();
        let dc = gdi.create_compatible_dc(screen_target()).unwrap().unwrap();
        let font_id = gdi
            .device_context_by_handle(dc)
            .unwrap()
            .selected_font_id
            .unwrap();
        gdi.objects.get_mut(&font_id).unwrap().id.0 += 1;
        let before = gdi.clone();
        gdi.create_compatible_dc(screen_target()).unwrap_err();
        assert_eq!(gdi, before);
    }

    #[test]
    fn forged_brush_handle_cannot_fill_with_the_original_object() {
        let mut gdi = RuntimeGdi::default();
        let dc = gdi.get_window_dc(target()).unwrap();
        let brush = gdi
            .get_stock_object(RUNTIME_GDI_BLACK_BRUSH_INDEX)
            .unwrap()
            .unwrap();
        let alias = brush + 1;
        let brush_id = gdi.object_by_handle(brush).unwrap().id;
        gdi.object_handle_index.insert(alias, brush_id);
        let before = gdi.clone();
        gdi.object_option(alias).unwrap_err();
        assert!(gdi.object_option(0x4702_ffff).unwrap().is_none());
        gdi.fill_rect(
            dc.handle,
            RuntimeRect {
                left: 0,
                top: 0,
                right: 10,
                bottom: 10,
            },
            alias,
            target().owner_thread_id,
        )
        .unwrap_err();
        assert_eq!(gdi, before);
        assert!(gdi.object_by_handle(brush).is_ok());
    }

    #[test]
    fn stock_brush_and_surface_fill_preserve_failure_atomicity() {
        let mut gdi = RuntimeGdi::default();
        let dc = gdi.get_window_dc(target()).unwrap();
        let invalid_before = gdi.clone();
        for index in [-1, i32::MAX] {
            assert!(gdi.get_stock_object(index).unwrap().is_none());
        }
        gdi.get_stock_object(1).unwrap_err();
        assert_eq!(gdi, invalid_before);

        let brush = gdi
            .get_stock_object(RUNTIME_GDI_BLACK_BRUSH_INDEX)
            .unwrap()
            .unwrap();
        let before_repeat = gdi.clone();
        let repeated = gdi
            .get_stock_object(RUNTIME_GDI_BLACK_BRUSH_INDEX)
            .unwrap()
            .unwrap();
        assert_eq!(brush, repeated);
        assert_eq!(gdi, before_repeat);

        gdi.fill_rect(
            dc.handle,
            RuntimeRect {
                left: 0,
                top: 0,
                right: 800,
                bottom: 600,
            },
            brush,
            target().owner_thread_id,
        )
        .unwrap();
        let surface = &gdi.surfaces[&dc.surface_id];
        assert_eq!(surface.fills[0].color_ref, 0);
        let before_empty = gdi.clone();
        gdi.fill_rect(
            dc.handle,
            RuntimeRect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            brush,
            target().owner_thread_id,
        )
        .unwrap();
        assert_eq!(gdi, before_empty);

        let before_invalid = gdi.clone();
        gdi.fill_rect(
            dc.handle,
            RuntimeRect {
                left: -1,
                top: 0,
                right: 1,
                bottom: 1,
            },
            brush,
            target().owner_thread_id,
        )
        .unwrap_err();
        assert_eq!(gdi, before_invalid);

        let surface = gdi.surfaces.get_mut(&dc.surface_id).unwrap();
        surface
            .fills
            .resize(RUNTIME_GDI_SURFACE_FILL_LIMIT, surface.fills[0]);
        let before_capacity_exhaustion = gdi.clone();
        assert!(
            gdi.fill_rect(
                dc.handle,
                RuntimeRect {
                    left: 0,
                    top: 0,
                    right: 1,
                    bottom: 1,
                },
                brush,
                target().owner_thread_id,
            )
            .is_err()
        );
        assert_eq!(gdi, before_capacity_exhaustion);
        gdi.fill_rect(
            dc.handle,
            RuntimeRect {
                left: 0,
                top: 0,
                right: 1,
                bottom: 1,
            },
            brush,
            RuntimeThreadId(8193),
        )
        .unwrap_err();
        assert_eq!(gdi, before_capacity_exhaustion);
    }

    #[test]
    fn window_surface_resize_preserves_identity_clears_content_and_rejects_live_dc() {
        let mut absent = RuntimeGdi::default();
        absent
            .resize_window_surface(target().window_id, 800, 600, 1_280, 720)
            .unwrap();
        assert!(absent.surfaces.is_empty());
        assert!(absent.window_surface_index.is_empty());

        let mut gdi = RuntimeGdi::default();
        let dc = gdi.get_window_dc(target()).unwrap();
        let brush = gdi
            .get_stock_object(RUNTIME_GDI_BLACK_BRUSH_INDEX)
            .unwrap()
            .unwrap();
        gdi.fill_rect(
            dc.handle,
            RuntimeRect {
                left: 0,
                top: 0,
                right: 800,
                bottom: 600,
            },
            brush,
            target().owner_thread_id,
        )
        .unwrap();
        let surface_before = gdi.surfaces[&dc.surface_id].clone();
        gdi.resize_window_surface(target().window_id, 800, 600, 1_280, 720)
            .expect_err("a live window DC prevents resize");
        assert_eq!(gdi.surfaces[&dc.surface_id], surface_before);
        assert_eq!(gdi.device_contexts[&dc.device_context_id].handle, dc.handle);
        gdi.release_window_dc(target().window_handle, dc.handle, target().owner_thread_id)
            .unwrap();
        gdi.resize_window_surface(target().window_id, 800, 600, 1_280, 720)
            .expect("resize succeeds after releasing the live DC");

        let surface = &gdi.surfaces[&dc.surface_id];
        assert_eq!(surface.id, dc.surface_id);
        assert_eq!(surface.window_id, target().window_id);
        assert_eq!((surface.width, surface.height), (1_280, 720));
        assert!(surface.fills.is_empty());
    }

    #[test]
    fn font_creation_owns_distinct_identity_and_rejects_invalid_inputs() {
        let mut gdi = RuntimeGdi::default();
        let mut invalid = font();
        invalid.face_name_windows_932 = vec![b'x'; RUNTIME_LOG_FONT_A_FACE_CAPACITY];
        let before_invalid = gdi.clone();
        gdi.create_font_a(invalid).unwrap_err();
        assert_eq!(gdi, before_invalid);

        let first = gdi.create_font_a(font()).unwrap().unwrap();
        assert_eq!(
            gdi.object_by_handle(first).unwrap().id,
            RuntimeGdiObjectId(1)
        );
        assert_eq!(first, RUNTIME_GDI_OBJECT_HANDLE_BASE);
        let second = gdi.create_font_a(font()).unwrap().unwrap();
        assert_eq!(
            gdi.object_by_handle(second).unwrap().id,
            RuntimeGdiObjectId(2)
        );
        assert_eq!(second, RUNTIME_GDI_OBJECT_HANDLE_BASE + 1);

        let object = gdi.object_by_handle(first).unwrap();
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

        let dc = gdi.get_window_dc(target()).unwrap();
        let before_wrong_kind = gdi.clone();
        gdi.fill_rect(
            dc.handle,
            RuntimeRect {
                left: 0,
                top: 0,
                right: 1,
                bottom: 1,
            },
            first,
            target().owner_thread_id,
        )
        .unwrap_err();
        assert_eq!(gdi, before_wrong_kind);
    }

    #[test]
    fn graphics_surface_dc_is_single_outstanding_and_guards_com_retirement() {
        let mut gdi = RuntimeGdi::default();
        let target = graphics_target(1, 9);
        let acquired = gdi.get_graphics_surface_dc(target).unwrap().unwrap();
        assert_eq!(acquired, RUNTIME_DEVICE_CONTEXT_HANDLE_BASE);
        assert_eq!(
            gdi.device_context_by_handle(acquired).unwrap().target,
            RuntimeDeviceContextTarget::GraphicsSurface {
                surface_id: target.surface_id,
                surface_com_object_id: target.surface_com_object_id,
                width: target.width,
                height: target.height,
                color_format: target.color_format,
            }
        );
        assert_eq!(gdi.device_contexts.len(), 1);

        let before_repeat = gdi.clone();
        assert!(gdi.get_graphics_surface_dc(target).unwrap().is_none());
        assert_eq!(gdi, before_repeat);

        let before_retirement = gdi.clone();
        gdi.validate_com_object_retirements(&[target.surface_com_object_id].into_iter().collect())
            .unwrap_err();
        assert_eq!(gdi, before_retirement);

        let before_wrong_release = gdi.clone();
        assert!(
            !gdi.release_graphics_surface_dc(
                graphics_target(1, 10),
                acquired,
                target.owner_thread_id,
            )
            .unwrap()
        );
        assert!(
            !gdi.release_graphics_surface_dc(target, 0, target.owner_thread_id)
                .unwrap()
        );
        assert_eq!(gdi, before_wrong_release);

        assert!(
            gdi.release_graphics_surface_dc(target, acquired, target.owner_thread_id)
                .unwrap()
        );
        assert!(gdi.device_contexts.is_empty());
        assert!(
            !gdi.release_graphics_surface_dc(target, acquired, target.owner_thread_id)
                .unwrap()
        );

        gdi.validate_com_object_retirements(&[target.surface_com_object_id].into_iter().collect())
            .unwrap();
        // An unrelated surface DC does not prevent retirement of the released surface.
        gdi.get_graphics_surface_dc(graphics_target(2, 10))
            .unwrap()
            .unwrap();
        gdi.validate_com_object_retirements(&[target.surface_com_object_id].into_iter().collect())
            .unwrap();
    }

    #[test]
    fn graphics_surface_dc_capacity_failure_preserves_allocator_state() {
        let mut gdi = RuntimeGdi::default();
        for index in 0..RUNTIME_GDI_DEVICE_CONTEXT_LIMIT {
            let target = graphics_target(index as u64 + 1, index as u64 + 1);
            gdi.get_graphics_surface_dc(target).unwrap().unwrap();
        }
        let before = gdi.clone();
        gdi.get_graphics_surface_dc(graphics_target(
            RUNTIME_GDI_DEVICE_CONTEXT_LIMIT as u64 + 1,
            RUNTIME_GDI_DEVICE_CONTEXT_LIMIT as u64 + 1,
        ))
        .unwrap_err();
        assert_eq!(gdi, before);
    }
}

use std::collections::{BTreeMap, BTreeSet};

use serde::{Serialize, Serializer, ser::SerializeStruct};

use super::com::RuntimeComObjectId;
use super::graphics::{RuntimeGraphicsColorFormat, RuntimeGraphicsSurfaceId};
use super::{RuntimeRect, RuntimeStateError, RuntimeThreadId, RuntimeWindowId};

pub const LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE: &str = "vine.logical-gdi-dc.v1";
pub const LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE: &str = "vine.logical-gdi-object.v1";
pub const RUNTIME_DEVICE_CONTEXT_HANDLE_BASE: u32 = 0x4701_0001;
pub const RUNTIME_GDI_OBJECT_HANDLE_BASE: u32 = 0x4702_0001;
pub const RUNTIME_GDI_BLACK_BRUSH_INDEX: i32 = 4;
pub const RUNTIME_GDI_DEVICE_CONTEXT_LIMIT: usize = 4_096;
pub const RUNTIME_GDI_OBJECT_LIMIT: usize = 4_096;
pub const RUNTIME_GDI_SURFACE_LIMIT: usize = 4_096;
pub const RUNTIME_GDI_SURFACE_FILL_LIMIT: usize = 16_384;
pub const RUNTIME_LOG_FONT_A_BYTE_LENGTH: usize = 60;
pub const RUNTIME_LOG_FONT_A_FACE_CAPACITY: usize = 32;

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
}

impl RuntimeDeviceContext {
    pub fn window_surface_id(&self) -> Option<RuntimeGdiSurfaceId> {
        match self.target {
            RuntimeDeviceContextTarget::Window { surface_id, .. } => Some(surface_id),
            RuntimeDeviceContextTarget::Screen { .. }
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
    StockBrush { index: i32, color_ref: u32 },
    FontA { log_font: RuntimeLogFontA },
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

    fn plan_object_allocation(&self) -> Result<RuntimeGdiObjectAllocation, RuntimeStateError> {
        if self.objects.len() >= RUNTIME_GDI_OBJECT_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-gdi",
                "gdi-object-capacity-exhausted",
                "logical GDI object capacity is exhausted",
            ));
        }
        let id = RuntimeGdiObjectId(self.next_object_id);
        let next_id = checked_next_identity(self.next_object_id, "gdi-object-identity-exhausted")?;
        let handle = self.next_object_handle;
        if handle < RUNTIME_GDI_OBJECT_HANDLE_BASE || self.object_handle_index.contains_key(&handle)
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "gdi-object-handle-allocator-invalid",
                "logical GDI object handle allocator is not at a free namespace value",
            ));
        }
        let next_handle = handle.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-gdi",
                "gdi-object-handle-exhausted",
                "logical GDI object handle namespace is exhausted",
            )
        })?;
        Ok(RuntimeGdiObjectAllocation {
            id,
            handle,
            next_id,
            next_handle,
        })
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

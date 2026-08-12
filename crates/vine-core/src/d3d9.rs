use serde::Serialize;

use crate::api::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiCallFrame, ApiContract, ApiDirection,
    ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiReturnSchema, ApiReturnValue,
    ApiSymbol, ApiValueSchema, CallbackPolicy, CallingConvention, ExecutionSession,
    LastErrorPolicy, ReentrancyPolicy, StackCleanup,
};
use crate::com::{
    COM_S_OK, ComCallTrap, ComDispatchOutcome, ComFaultClass, ComFrontendCall,
    ComGuestWriteReceipt, ComHResult, ComInterfaceSpec, ComMethodSpec, ComObjectSpec,
    ComReturnValue,
};
use crate::memory::{GuestAddress, MemoryAccess};
use crate::runtime::{
    RuntimeComIid, RuntimeComReturnKind, RuntimeGraphicsBackBufferClearRequest,
    RuntimeGraphicsColorFormat, RuntimeGraphicsCullMode, RuntimeGraphicsDeviceCreateRequest,
    RuntimeGraphicsDrawCaptureRequest, RuntimeGraphicsFlexibleVertexFormat,
    RuntimeGraphicsFlexibleVertexFormatUpdateRequest, RuntimeGraphicsFrontend,
    RuntimeGraphicsPresentMode, RuntimeGraphicsPresentRequest, RuntimeGraphicsPrimitiveTopology,
    RuntimeGraphicsRenderStateUpdate, RuntimeGraphicsRenderStateUpdateRequest,
    RuntimeGraphicsSamplerStateUpdate, RuntimeGraphicsSamplerStateUpdateRequest,
    RuntimeGraphicsSceneBeginRequest, RuntimeGraphicsSceneEndRequest,
    RuntimeGraphicsStreamSourceUpdateRequest, RuntimeGraphicsSurfaceCreateRequest,
    RuntimeGraphicsTextureArgument, RuntimeGraphicsTextureBindingUpdateRequest,
    RuntimeGraphicsTextureCreateRequest, RuntimeGraphicsTextureDescriptor,
    RuntimeGraphicsTextureDirtyRegion, RuntimeGraphicsTextureDirtyRegionMarkRequest,
    RuntimeGraphicsTextureFilter, RuntimeGraphicsTextureLockRequest,
    RuntimeGraphicsTextureOperation, RuntimeGraphicsTexturePool,
    RuntimeGraphicsTextureStageStateUpdate, RuntimeGraphicsTextureStageStateUpdateRequest,
    RuntimeGraphicsTextureUpdateRequest, RuntimeGraphicsTextureUsage,
    RuntimeGraphicsVertexBufferCreateRequest, RuntimeGraphicsVertexBufferDescriptor,
    RuntimeGraphicsVertexBufferLockRequest, RuntimeGraphicsVertexBufferPool,
    RuntimeGraphicsVertexBufferUsage, RuntimeRect, RuntimeStateError, RuntimeStateErrorClass,
};

pub const D3D9_SDK_VERSION: u32 = 32;
pub const D3D9_DIRECT3D_CREATE9_IMPLEMENTATION: &str = "d3d9.direct3d-create9.v1";
pub const D3D9_IDIRECT3D9_CLASS_IDENTITY: &str = "d3d9.idirect3d9-root.v1";
pub const D3D9_COMPATIBILITY_POLICY_SCHEMA: &str = "vine.d3d9-compatibility-policy";
pub const D3D9_COMPATIBILITY_POLICY_VERSION: u32 = 33;
pub const D3D9_D3DADAPTER_IDENTIFIER9_BYTE_LENGTH: u32 = 1_100;
pub const D3D9_D3DCAPS9_BYTE_LENGTH: u32 = 304;
pub const D3D9_D3DDISPLAYMODE_BYTE_LENGTH: u32 = 16;
pub const D3D9_D3DPRESENT_PARAMETERS_BYTE_LENGTH: u32 = 56;
pub const D3D9_D3DRASTER_STATUS_BYTE_LENGTH: u32 = 8;
pub const D3D9_D3DDEVTYPE_HAL: u32 = 1;
pub const D3D9_D3DCAPS_READ_SCANLINE: u32 = 0x0002_0000;
pub const D3D9_D3DCAPS2_DYNAMICTEXTURES: u32 = 0x2000_0000;
pub const D3D9_D3DPS_VERSION_3_0: u32 = 0xffff_0300;
pub const D3D9_D3DFVF_XYZRHW: u32 = 0x004;
pub const D3D9_D3DFVF_DIFFUSE: u32 = 0x040;
pub const D3D9_D3DFVF_TEX1: u32 = 0x100;
pub const D3D9_D3DPT_TRIANGLESTRIP: u32 = 5;
pub const D3D9_D3DFMT_X8R8G8B8: u32 = 22;
pub const D3D9_D3DRTYPE_TEXTURE: u32 = 3;
pub const D3D9_D3DUSAGE_DYNAMIC: u32 = 0x0000_0200;
pub const D3D9_D3DUSAGE_WRITEONLY: u32 = 0x0000_0008;
pub const D3D9_D3DLOCK_NO_DIRTY_UPDATE: u32 = 0x0000_8000;
pub const D3D9_D3DCLEAR_TARGET: u32 = 0x0000_0001;
pub const D3D9_D3DRS_ZENABLE: u32 = 7;
pub const D3D9_D3DRS_CULLMODE: u32 = 22;
pub const D3D9_D3DRS_ALPHABLENDENABLE: u32 = 27;
pub const D3D9_D3DRS_LIGHTING: u32 = 137;
pub const D3D9_D3DCULL_NONE: u32 = 1;
pub const D3D9_D3DSAMP_MAGFILTER: u32 = 5;
pub const D3D9_D3DSAMP_MINFILTER: u32 = 6;
pub const D3D9_D3DTEXF_LINEAR: u32 = 2;
pub const D3D9_D3DTSS_COLOROP: u32 = 1;
pub const D3D9_D3DTSS_COLORARG1: u32 = 2;
pub const D3D9_D3DTSS_COLORARG2: u32 = 3;
pub const D3D9_D3DTSS_ALPHAOP: u32 = 4;
pub const D3D9_D3DTSS_ALPHAARG1: u32 = 5;
pub const D3D9_D3DTSS_ALPHAARG2: u32 = 6;
pub const D3D9_D3DTOP_MODULATE: u32 = 4;
pub const D3D9_D3DTOP_SELECTARG1: u32 = 2;
pub const D3D9_D3DTA_DIFFUSE: u32 = 0;
pub const D3D9_D3DTA_TEXTURE: u32 = 2;
pub const D3D9_D3DENUM_WHQL_LEVEL: u32 = 0x0000_0002;
pub const D3D9_CREATE_HARDWARE_VERTEX_PROCESSING_MULTITHREADED: u32 = 0x0000_0044;
pub const D3D9_D3DFMT_UNKNOWN: u32 = 0;
pub const D3D9_D3DFMT_D16: u32 = 80;
pub const D3D9_D3DMULTISAMPLE_NONE: u32 = 0;
pub const D3D9_D3DSWAPEFFECT_DISCARD: u32 = 3;
pub const D3D9_D3DPOOL_DEFAULT: u32 = 0;
pub const D3D9_D3DPOOL_MANAGED: u32 = 1;
pub const D3D9_D3DPOOL_SYSTEMMEM: u32 = 2;
pub const D3D9_D3DERR_NOTAVAILABLE: ComHResult = ComHResult(0x8876_086a_u32 as i32);
pub const D3D9_D3DERR_INVALIDCALL: ComHResult = ComHResult(0x8876_086c_u32 as i32);
pub const D3D9_VIRTUAL_DRIVER: &str = "vine-d3d9.dll";
pub const D3D9_VIRTUAL_DESCRIPTION: &str = "Vine Virtual Direct3D9 Adapter";
pub const D3D9_VIRTUAL_DEVICE_NAME: &str = "\\\\.\\DISPLAY1";
pub const D3D9_VIRTUAL_DRIVER_VERSION: u64 = 0x0001_0000_0000_0000;
pub const D3D9_VIRTUAL_DEVICE_IDENTIFIER_WINDOWS_BYTES: [u8; 16] = [
    0x45, 0x4e, 0x49, 0x56, 0x33, 0x44, 0x39, 0x44, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
];
pub const D3D9_IDIRECT3D9_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0xca, 0xcb, 0xbd, 0x81, 0xd4, 0x64, 0x6d, 0x42, 0xae, 0x8d, 0xad, 0x01, 0x47, 0xf4, 0x27, 0x5c,
]);
pub const D3D9_IDIRECT3DDEVICE9_CLASS_IDENTITY: &str = "d3d9.idirect3ddevice9.v1";
pub const D3D9_IDIRECT3DDEVICE9_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0x96, 0x3b, 0x22, 0xd0, 0x7a, 0xbf, 0xfd, 0x43, 0x92, 0xbd, 0xa4, 0x3b, 0x0d, 0x82, 0xb9, 0xeb,
]);
pub const D3D9_IDIRECT3DTEXTURE9_CLASS_IDENTITY: &str = "d3d9.idirect3dtexture9.v1";
pub const D3D9_IDIRECT3DTEXTURE9_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0x27, 0x12, 0xc3, 0x85, 0xe5, 0x3d, 0x00, 0x4f, 0x9b, 0x3a, 0xf1, 0x1a, 0xc3, 0x8c, 0x18, 0xb5,
]);
pub const D3D9_IDIRECT3DSURFACE9_CLASS_IDENTITY: &str = "d3d9.idirect3dsurface9.v1";
pub const D3D9_IDIRECT3DSURFACE9_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0x3a, 0xaf, 0xfb, 0x0c, 0xf6, 0x9f, 0x9a, 0x42, 0x99, 0xb3, 0xa2, 0x79, 0x6a, 0xf8, 0xb8, 0x9b,
]);
pub const D3D9_IDIRECT3DVERTEXBUFFER9_CLASS_IDENTITY: &str = "d3d9.idirect3dvertexbuffer9.v1";
pub const D3D9_IDIRECT3DVERTEXBUFFER9_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0xb5, 0xb1, 0x4b, 0xb6, 0x70, 0xfd, 0xf6, 0x4d, 0xbf, 0x91, 0x19, 0xd0, 0xa1, 0x24, 0x55, 0xe3,
]);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9AdapterCountPolicy {
    pub profile_id: String,
    pub display_profile_id: String,
    pub adapter_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9AdapterMonitorPolicy {
    pub profile_id: String,
    pub display_profile_id: String,
    pub adapter: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9AdapterDisplayModePolicy {
    pub profile_id: String,
    pub display_profile_id: String,
    pub adapter: u32,
    pub refresh_rate: u32,
    pub format: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9AdapterIdentifierPolicy {
    pub profile_id: String,
    pub display_profile_id: String,
    pub adapter: u32,
    pub flags: u32,
    pub driver: String,
    pub description: String,
    pub device_name: String,
    pub driver_version: u64,
    pub vendor_id: u32,
    pub device_id: u32,
    pub sub_sys_id: u32,
    pub revision: u32,
    pub device_identifier: [u8; 16],
    pub whql_level: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9DeviceCapsPolicy {
    pub profile_id: String,
    pub display_profile_id: String,
    pub adapter: u32,
    pub device_type: u32,
    pub caps: u32,
    pub caps2: u32,
    pub pixel_shader_version: u32,
    pub number_of_adapters_in_group: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9CheckDeviceTypePolicy {
    pub profile_id: String,
    pub display_profile_id: String,
    pub adapter: u32,
    pub device_type: u32,
    pub adapter_format: u32,
    pub back_buffer_format: u32,
    pub windowed_supported: bool,
    pub fullscreen_supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9CheckDeviceFormatPolicy {
    pub profile_id: String,
    pub display_profile_id: String,
    pub adapter: u32,
    pub device_type: u32,
    pub adapter_format: u32,
    pub resource_type: u32,
    pub check_format: u32,
    pub supported_usages: [u32; 2],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9CreateDevicePolicy {
    pub profile_id: String,
    pub display_profile_id: String,
    pub adapter: u32,
    pub device_type: u32,
    pub behavior_flags: u32,
    pub requested_back_buffer_width: u32,
    pub requested_back_buffer_height: u32,
    pub requested_back_buffer_format: u32,
    pub resolved_back_buffer_format: u32,
    pub back_buffer_count: u32,
    pub multi_sample_type: u32,
    pub multi_sample_quality: u32,
    pub swap_effect: u32,
    pub device_window_matches_focus_window: bool,
    pub windowed: u32,
    pub enable_auto_depth_stencil: u32,
    pub auto_depth_stencil_format: u32,
    pub flags: u32,
    pub full_screen_refresh_rate_in_hz: u32,
    pub presentation_interval: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9CreateTextureVariant {
    pub width: u32,
    pub height: u32,
    pub levels: u32,
    pub usage: u32,
    pub format: u32,
    pub pool: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9CreateTexturePolicy {
    pub profile_id: String,
    pub variants: Vec<D3d9CreateTextureVariant>,
    pub shared_handle_must_be_null: bool,
}

impl D3d9CreateTexturePolicy {
    pub fn restrict_to_system_memory_v1(&mut self) {
        self.profile_id = "primary-system-memory-x8r8g8b8-texture-v1".to_owned();
        self.variants.truncate(1);
    }

    pub fn restrict_to_initial_textures_v1(&mut self) {
        self.profile_id = "primary-initial-x8r8g8b8-textures-v1".to_owned();
        self.variants.truncate(3);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9GetSurfaceLevelPolicy {
    pub profile_id: String,
    pub level: u32,
    pub source_texture_variant_indices: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9SurfaceDcPolicy {
    pub profile_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9InitialVertexBufferPolicy {
    pub profile_id: String,
    pub byte_length: u32,
    pub usage: u32,
    pub flexible_vertex_format: u32,
    pub pool: u32,
    pub shared_handle_must_be_null: bool,
    pub lock_offset: u32,
    pub lock_byte_length: u32,
    pub lock_flags: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9RenderStateSetting {
    pub state: u32,
    pub value: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9InitialRenderStatePolicy {
    pub profile_id: String,
    pub settings: [D3d9RenderStateSetting; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9SamplerStateSetting {
    pub sampler: u32,
    pub state: u32,
    pub value: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9InitialSamplerStatePolicy {
    pub profile_id: String,
    pub settings: [D3d9SamplerStateSetting; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9TextureStageStateSetting {
    pub stage: u32,
    pub state: u32,
    pub value: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9InitialTextureStageStatePolicy {
    pub profile_id: String,
    pub settings: [D3d9TextureStageStateSetting; 6],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9TextureBindingPolicy {
    pub profile_id: String,
    pub stage: u32,
    pub texture_variant_indices: Vec<u32>,
    pub null_unbind_supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9FlexibleVertexFormatPolicy {
    pub profile_id: String,
    pub value: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9StreamSourcePolicy {
    pub profile_id: String,
    pub stream: u32,
    pub offset_in_bytes: u32,
    pub stride: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9DrawPrimitivePolicy {
    pub profile_id: String,
    pub primitive_type: u32,
    pub start_vertex: u32,
    pub primitive_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9RasterStatusPolicy {
    pub profile_id: String,
    pub display_profile_id: String,
    pub in_v_blank: bool,
    pub scan_line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9ClearPolicy {
    pub profile_id: String,
    pub rect_count: u32,
    pub rects_must_be_null: bool,
    pub flags: u32,
    pub color_argb: u32,
    pub depth_bits: u32,
    pub stencil: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9AddDirtyRectPolicy {
    pub profile_id: String,
    pub source_texture_variant_indices: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9TextureLockRectPolicy {
    pub profile_id: String,
    pub source_texture_variant_indices: Vec<u32>,
    pub level: u32,
    pub rect: RuntimeRect,
    pub flags: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9UpdateTexturePolicy {
    pub profile_id: String,
    pub source_texture_variant_indices: Vec<u32>,
    pub destination_texture_variant_indices: Vec<u32>,
    pub partial_dirty_rect: RuntimeRect,
    pub allow_bounded_partial_dirty_rects: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9BeginScenePolicy {
    pub profile_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9EndScenePolicy {
    pub profile_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9PresentPolicy {
    pub profile_id: String,
    pub source_rect_must_be_null: bool,
    pub destination_rect_must_be_null: bool,
    pub destination_window_override_must_be_null: bool,
    pub dirty_region_must_be_null: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9TestCooperativeLevelPolicy {
    pub profile_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9CompatibilityPolicy {
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_cooperative_level: Option<D3d9TestCooperativeLevelPolicy>,
    pub get_adapter_count: Option<D3d9AdapterCountPolicy>,
    pub get_adapter_monitor: Option<D3d9AdapterMonitorPolicy>,
    pub get_adapter_display_mode: Option<D3d9AdapterDisplayModePolicy>,
    pub get_adapter_identifier: Option<D3d9AdapterIdentifierPolicy>,
    pub get_device_caps: Option<D3d9DeviceCapsPolicy>,
    pub check_device_type: Option<D3d9CheckDeviceTypePolicy>,
    pub check_device_format: Option<D3d9CheckDeviceFormatPolicy>,
    pub create_device: Option<D3d9CreateDevicePolicy>,
    pub create_texture: Option<D3d9CreateTexturePolicy>,
    pub get_surface_level: Option<D3d9GetSurfaceLevelPolicy>,
    pub surface_dc: Option<D3d9SurfaceDcPolicy>,
    pub initial_vertex_buffer: Option<D3d9InitialVertexBufferPolicy>,
    pub initial_render_states: Option<D3d9InitialRenderStatePolicy>,
    pub initial_sampler_states: Option<D3d9InitialSamplerStatePolicy>,
    pub initial_texture_stage_states: Option<D3d9InitialTextureStageStatePolicy>,
    pub texture_binding: Option<D3d9TextureBindingPolicy>,
    pub flexible_vertex_format: Option<D3d9FlexibleVertexFormatPolicy>,
    pub stream_source: Option<D3d9StreamSourcePolicy>,
    pub draw_primitive: Option<D3d9DrawPrimitivePolicy>,
    pub raster_status: Option<D3d9RasterStatusPolicy>,
    pub clear: Option<D3d9ClearPolicy>,
    pub add_dirty_rect: Option<D3d9AddDirtyRectPolicy>,
    pub texture_lock_rect: Option<D3d9TextureLockRectPolicy>,
    pub update_texture: Option<D3d9UpdateTexturePolicy>,
    pub begin_scene: Option<D3d9BeginScenePolicy>,
    pub end_scene: Option<D3d9EndScenePolicy>,
    pub present: Option<D3d9PresentPolicy>,
}

impl D3d9CompatibilityPolicy {
    pub fn controlled_v31() -> Self {
        Self {
            schema: D3D9_COMPATIBILITY_POLICY_SCHEMA.to_owned(),
            schema_version: 31,
            id: "d3d9-primary-virtual-adapter-v31".to_owned(),
            test_cooperative_level: None,
            get_adapter_count: Some(D3d9AdapterCountPolicy {
                profile_id: "single-logical-adapter-v1".to_owned(),
                display_profile_id: "single-monitor-1920x1080-work-1920x1032-v2".to_owned(),
                adapter_count: 1,
            }),
            get_adapter_monitor: Some(D3d9AdapterMonitorPolicy {
                profile_id: "primary-adapter-monitor-v1".to_owned(),
                display_profile_id: "single-monitor-1920x1080-work-1920x1032-v2".to_owned(),
                adapter: 0,
            }),
            get_adapter_display_mode: Some(D3d9AdapterDisplayModePolicy {
                profile_id: "primary-adapter-1920x1080-60-x8r8g8b8-v1".to_owned(),
                display_profile_id: "single-monitor-1920x1080-work-1920x1032-v2".to_owned(),
                adapter: 0,
                refresh_rate: 60,
                format: D3D9_D3DFMT_X8R8G8B8,
            }),
            get_adapter_identifier: Some(D3d9AdapterIdentifierPolicy {
                profile_id: "primary-virtual-adapter-identifier-v1".to_owned(),
                display_profile_id: "single-monitor-1920x1080-work-1920x1032-v2".to_owned(),
                adapter: 0,
                flags: 0,
                driver: D3D9_VIRTUAL_DRIVER.to_owned(),
                description: D3D9_VIRTUAL_DESCRIPTION.to_owned(),
                device_name: D3D9_VIRTUAL_DEVICE_NAME.to_owned(),
                driver_version: D3D9_VIRTUAL_DRIVER_VERSION,
                vendor_id: 0,
                device_id: 0,
                sub_sys_id: 0,
                revision: 0,
                device_identifier: D3D9_VIRTUAL_DEVICE_IDENTIFIER_WINDOWS_BYTES,
                whql_level: 0,
            }),
            get_device_caps: Some(D3d9DeviceCapsPolicy {
                profile_id: "primary-hal-readscanline-dynamic-textures-ps3-v1".to_owned(),
                display_profile_id: "single-monitor-1920x1080-work-1920x1032-v2".to_owned(),
                adapter: 0,
                device_type: D3D9_D3DDEVTYPE_HAL,
                caps: D3D9_D3DCAPS_READ_SCANLINE,
                caps2: D3D9_D3DCAPS2_DYNAMICTEXTURES,
                pixel_shader_version: D3D9_D3DPS_VERSION_3_0,
                number_of_adapters_in_group: 1,
            }),
            check_device_type: Some(D3d9CheckDeviceTypePolicy {
                profile_id: "primary-hal-x8r8g8b8-windowed-fullscreen-v1".to_owned(),
                display_profile_id: "single-monitor-1920x1080-work-1920x1032-v2".to_owned(),
                adapter: 0,
                device_type: D3D9_D3DDEVTYPE_HAL,
                adapter_format: D3D9_D3DFMT_X8R8G8B8,
                back_buffer_format: D3D9_D3DFMT_X8R8G8B8,
                windowed_supported: true,
                fullscreen_supported: true,
            }),
            check_device_format: Some(D3d9CheckDeviceFormatPolicy {
                profile_id: "primary-hal-x8r8g8b8-texture-basic-dynamic-v1".to_owned(),
                display_profile_id: "single-monitor-1920x1080-work-1920x1032-v2".to_owned(),
                adapter: 0,
                device_type: D3D9_D3DDEVTYPE_HAL,
                adapter_format: D3D9_D3DFMT_X8R8G8B8,
                resource_type: D3D9_D3DRTYPE_TEXTURE,
                check_format: D3D9_D3DFMT_X8R8G8B8,
                supported_usages: [0, D3D9_D3DUSAGE_DYNAMIC],
            }),
            create_device: Some(D3d9CreateDevicePolicy {
                profile_id: "primary-windowed-hal-device-v1".to_owned(),
                display_profile_id: "single-monitor-1920x1080-work-1920x1032-v2".to_owned(),
                adapter: 0,
                device_type: D3D9_D3DDEVTYPE_HAL,
                behavior_flags: D3D9_CREATE_HARDWARE_VERTEX_PROCESSING_MULTITHREADED,
                requested_back_buffer_width: 0,
                requested_back_buffer_height: 0,
                requested_back_buffer_format: D3D9_D3DFMT_UNKNOWN,
                resolved_back_buffer_format: D3D9_D3DFMT_X8R8G8B8,
                back_buffer_count: 1,
                multi_sample_type: D3D9_D3DMULTISAMPLE_NONE,
                multi_sample_quality: 0,
                swap_effect: D3D9_D3DSWAPEFFECT_DISCARD,
                device_window_matches_focus_window: true,
                windowed: 1,
                enable_auto_depth_stencil: 0,
                auto_depth_stencil_format: D3D9_D3DFMT_D16,
                flags: 0,
                full_screen_refresh_rate_in_hz: 0,
                presentation_interval: 0,
            }),
            create_texture: Some(D3d9CreateTexturePolicy {
                profile_id: "primary-initial-and-recreated-x8r8g8b8-textures-v2".to_owned(),
                variants: controlled_create_texture_variants().into(),
                shared_handle_must_be_null: true,
            }),
            get_surface_level: Some(D3d9GetSurfaceLevelPolicy {
                profile_id: "primary-system-memory-texture-level-zero-surfaces-v2".to_owned(),
                level: 0,
                source_texture_variant_indices: vec![0, 3],
            }),
            surface_dc: Some(D3d9SurfaceDcPolicy {
                profile_id: "primary-surface-gdi-dc-v1".to_owned(),
            }),
            initial_vertex_buffer: Some(D3d9InitialVertexBufferPolicy {
                profile_id: "primary-initial-xyzw-diffuse-tex1-quad-v1".to_owned(),
                byte_length: 112,
                usage: D3D9_D3DUSAGE_WRITEONLY,
                flexible_vertex_format: 0x144,
                pool: D3D9_D3DPOOL_MANAGED,
                shared_handle_must_be_null: true,
                lock_offset: 0,
                lock_byte_length: 0,
                lock_flags: 0,
            }),
            initial_render_states: Some(D3d9InitialRenderStatePolicy {
                profile_id: "primary-initial-fixed-function-render-states-v1".to_owned(),
                settings: controlled_initial_render_state_settings(),
            }),
            initial_sampler_states: Some(D3d9InitialSamplerStatePolicy {
                profile_id: "primary-first-frame-linear-sampler-states-v1".to_owned(),
                settings: controlled_initial_sampler_state_settings(),
            }),
            initial_texture_stage_states: Some(D3d9InitialTextureStageStatePolicy {
                profile_id: "primary-first-frame-fixed-function-texture-stage-zero-v1".to_owned(),
                settings: controlled_initial_texture_stage_state_settings(),
            }),
            texture_binding: Some(D3d9TextureBindingPolicy {
                profile_id: "primary-first-frame-static-default-texture-stage-zero-v1".to_owned(),
                stage: 0,
                texture_variant_indices: vec![4],
                null_unbind_supported: true,
            }),
            flexible_vertex_format: Some(D3d9FlexibleVertexFormatPolicy {
                profile_id: "primary-first-frame-transformed-diffuse-textured-fvf-v1".to_owned(),
                value: D3D9_D3DFVF_XYZRHW | D3D9_D3DFVF_DIFFUSE | D3D9_D3DFVF_TEX1,
            }),
            stream_source: Some(D3d9StreamSourcePolicy {
                profile_id: "primary-first-frame-transformed-diffuse-textured-stream-zero-v1"
                    .to_owned(),
                stream: 0,
                offset_in_bytes: 0,
                stride: 28,
            }),
            draw_primitive: Some(D3d9DrawPrimitivePolicy {
                profile_id: "primary-first-frame-triangle-strip-quad-draw-v1".to_owned(),
                primitive_type: D3D9_D3DPT_TRIANGLESTRIP,
                start_vertex: 0,
                primitive_count: 2,
            }),
            raster_status: Some(D3d9RasterStatusPolicy {
                profile_id: "primary-stable-last-visible-scanline-v2".to_owned(),
                display_profile_id: "single-monitor-1920x1080-work-1920x1032-v2".to_owned(),
                in_v_blank: false,
                scan_line: 1_079,
            }),
            clear: Some(D3d9ClearPolicy {
                profile_id: "primary-full-render-target-black-v1".to_owned(),
                rect_count: 0,
                rects_must_be_null: true,
                flags: D3D9_D3DCLEAR_TARGET,
                color_argb: 0xff00_0000,
                depth_bits: 1.0_f32.to_bits(),
                stencil: 0,
            }),
            add_dirty_rect: Some(D3d9AddDirtyRectPolicy {
                profile_id: "primary-recreated-system-memory-texture-dirty-rect-v1".to_owned(),
                source_texture_variant_indices: vec![3],
            }),
            texture_lock_rect: Some(D3d9TextureLockRectPolicy {
                profile_id: "primary-recreated-system-memory-presentation-lock-v1".to_owned(),
                source_texture_variant_indices: vec![3],
                level: 0,
                rect: RuntimeRect {
                    left: 0,
                    top: 0,
                    right: 1_280,
                    bottom: 720,
                },
                flags: D3D9_D3DLOCK_NO_DIRTY_UPDATE,
            }),
            update_texture: Some(D3d9UpdateTexturePolicy {
                profile_id: "primary-recreated-system-memory-to-static-default-texture-update-v2"
                    .to_owned(),
                source_texture_variant_indices: vec![3],
                destination_texture_variant_indices: vec![4],
                partial_dirty_rect: RuntimeRect {
                    left: 0,
                    top: 0,
                    right: 1_280,
                    bottom: 720,
                },
                allow_bounded_partial_dirty_rects: false,
            }),
            begin_scene: Some(D3d9BeginScenePolicy {
                profile_id: "primary-first-frame-scene-begin-v1".to_owned(),
            }),
            end_scene: Some(D3d9EndScenePolicy {
                profile_id: "primary-first-frame-scene-end-v1".to_owned(),
            }),
            present: Some(D3d9PresentPolicy {
                profile_id: "primary-first-frame-full-back-buffer-present-v1".to_owned(),
                source_rect_must_be_null: true,
                destination_rect_must_be_null: true,
                destination_window_override_must_be_null: true,
                dirty_region_must_be_null: true,
            }),
        }
    }

    pub fn controlled_v32() -> Self {
        let mut policy = Self::controlled_v31();
        policy.schema_version = 32;
        policy.id = "d3d9-primary-virtual-adapter-v32".to_owned();
        policy.test_cooperative_level = Some(D3d9TestCooperativeLevelPolicy {
            profile_id: "primary-live-operational-device-query-v1".to_owned(),
        });
        policy
    }

    pub fn controlled_v33() -> Self {
        let mut policy = Self::controlled_v32();
        policy.schema_version = D3D9_COMPATIBILITY_POLICY_VERSION;
        policy.id = "d3d9-primary-virtual-adapter-v33".to_owned();
        let update = policy
            .update_texture
            .as_mut()
            .expect("controlled D3D9 policy lost UpdateTexture");
        update.profile_id =
            "primary-recreated-system-memory-to-static-default-texture-bounded-update-v3"
                .to_owned();
        update.allow_bounded_partial_dirty_rects = true;
        policy
    }

    pub fn restrict_to_system_memory_v1(&mut self) {
        if let Some(texture) = self.create_texture.as_mut() {
            texture.restrict_to_system_memory_v1();
        }
        if let Some(surface) = self.get_surface_level.as_mut() {
            surface.profile_id = "primary-texture-level-zero-surface-v1".to_owned();
            surface.source_texture_variant_indices = vec![0];
        }
        self.add_dirty_rect = None;
        self.texture_lock_rect = None;
        self.update_texture = None;
        self.texture_binding = None;
        self.flexible_vertex_format = None;
        self.stream_source = None;
        self.draw_primitive = None;
        self.end_scene = None;
        self.present = None;
        self.test_cooperative_level = None;
    }

    pub fn restrict_to_initial_textures_v1(&mut self) {
        if let Some(texture) = self.create_texture.as_mut() {
            texture.restrict_to_initial_textures_v1();
        }
        if let Some(surface) = self.get_surface_level.as_mut() {
            surface.profile_id = "primary-texture-level-zero-surface-v1".to_owned();
            surface.source_texture_variant_indices = vec![0];
        }
        self.add_dirty_rect = None;
        self.texture_lock_rect = None;
        self.update_texture = None;
        self.texture_binding = None;
        self.flexible_vertex_format = None;
        self.stream_source = None;
        self.draw_primitive = None;
        self.end_scene = None;
        self.present = None;
        self.test_cooperative_level = None;
    }

    pub fn validate(&self) -> Result<(), ApiFault> {
        let update_texture_profile = self
            .update_texture
            .as_ref()
            .map(|policy| policy.profile_id.as_str());
        let supported_header = match (self.schema_version, self.id.as_str()) {
            (31, "d3d9-primary-virtual-adapter-v31") => {
                self.test_cooperative_level.is_none()
                    && matches!(
                        update_texture_profile,
                        None | Some(
                            "primary-recreated-system-memory-to-static-default-texture-update-v2"
                        )
                    )
            }
            (32, "d3d9-primary-virtual-adapter-v32") => {
                self.test_cooperative_level.is_some()
                    && matches!(
                        update_texture_profile,
                        None | Some(
                            "primary-recreated-system-memory-to-static-default-texture-update-v2"
                        )
                    )
            }
            (D3D9_COMPATIBILITY_POLICY_VERSION, "d3d9-primary-virtual-adapter-v33") => {
                self.test_cooperative_level.is_some()
                    && matches!(
                        update_texture_profile,
                        None | Some(
                            "primary-recreated-system-memory-to-static-default-texture-bounded-update-v3"
                        )
                    )
            }
            _ => false,
        };
        if self.schema != D3D9_COMPATIBILITY_POLICY_SCHEMA || !supported_header {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-compatibility-policy-header-invalid",
                "D3D9 compatibility policy has an unsupported schema, version, or ID",
            ));
        }
        if let Some(test) = self.test_cooperative_level.as_ref()
            && (test.profile_id != "primary-live-operational-device-query-v1"
                || self.create_device.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-test-cooperative-level-profile-invalid",
                "D3D9 cooperative-level profile must describe a live operational logical device",
            ));
        }
        if let Some(count) = self.get_adapter_count.as_ref()
            && (!valid_policy_id(&count.profile_id)
                || !valid_policy_id(&count.display_profile_id)
                || count.adapter_count != 1)
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-adapter-count-profile-invalid",
                "D3D9 adapter-count profile must describe exactly one logical primary adapter",
            ));
        }
        if let Some(monitor) = self.get_adapter_monitor.as_ref()
            && (!valid_policy_id(&monitor.profile_id)
                || !valid_policy_id(&monitor.display_profile_id)
                || monitor.adapter != 0)
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-adapter-monitor-profile-invalid",
                "D3D9 adapter-monitor profile must map adapter zero to the logical primary monitor",
            ));
        }
        if let Some(mode) = self.get_adapter_display_mode.as_ref()
            && (!valid_policy_id(&mode.profile_id)
                || !valid_policy_id(&mode.display_profile_id)
                || mode.adapter != 0
                || mode.refresh_rate == 0
                || mode.refresh_rate > 1_000
                || mode.format != D3D9_D3DFMT_X8R8G8B8)
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-display-mode-profile-invalid",
                "D3D9 display-mode profile must describe the bounded primary X8R8G8B8 adapter",
            ));
        }
        if let Some(identifier) = self.get_adapter_identifier.as_ref()
            && (!valid_policy_id(&identifier.profile_id)
                || !valid_policy_id(&identifier.display_profile_id)
                || identifier.adapter != 0
                || identifier.flags != 0
                || !valid_fixed_ansi(&identifier.driver, 512)
                || !valid_fixed_ansi(&identifier.description, 512)
                || !valid_fixed_ansi(&identifier.device_name, 32)
                || identifier.device_name != D3D9_VIRTUAL_DEVICE_NAME
                || identifier.driver_version != D3D9_VIRTUAL_DRIVER_VERSION
                || identifier.vendor_id != 0
                || identifier.device_id != 0
                || identifier.sub_sys_id != 0
                || identifier.revision != 0
                || identifier.device_identifier != D3D9_VIRTUAL_DEVICE_IDENTIFIER_WINDOWS_BYTES
                || identifier.whql_level != 0)
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-adapter-identifier-profile-invalid",
                "D3D9 adapter identifier must describe the bounded non-hardware virtual primary adapter",
            ));
        }
        if let Some(caps) = self.get_device_caps.as_ref()
            && (!valid_policy_id(&caps.profile_id)
                || !valid_policy_id(&caps.display_profile_id)
                || caps.adapter != 0
                || caps.device_type != D3D9_D3DDEVTYPE_HAL
                || caps.caps != D3D9_D3DCAPS_READ_SCANLINE
                || caps.caps2 != D3D9_D3DCAPS2_DYNAMICTEXTURES
                || caps.pixel_shader_version != D3D9_D3DPS_VERSION_3_0
                || caps.number_of_adapters_in_group != 1)
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-device-caps-profile-invalid",
                "D3D9 device-caps profile must describe the bounded virtual primary HAL capability band",
            ));
        }
        if let Some(check) = self.check_device_type.as_ref()
            && (!valid_policy_id(&check.profile_id)
                || !valid_policy_id(&check.display_profile_id)
                || check.adapter != 0
                || check.device_type != D3D9_D3DDEVTYPE_HAL
                || check.adapter_format != D3D9_D3DFMT_X8R8G8B8
                || check.back_buffer_format != D3D9_D3DFMT_X8R8G8B8
                || !check.windowed_supported
                || !check.fullscreen_supported)
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-check-device-type-profile-invalid",
                "D3D9 device-type check must describe the bounded primary HAL format band",
            ));
        }
        if let Some(check) = self.check_device_format.as_ref()
            && (!valid_policy_id(&check.profile_id)
                || !valid_policy_id(&check.display_profile_id)
                || check.adapter != 0
                || check.device_type != D3D9_D3DDEVTYPE_HAL
                || check.adapter_format != D3D9_D3DFMT_X8R8G8B8
                || check.resource_type != D3D9_D3DRTYPE_TEXTURE
                || check.check_format != D3D9_D3DFMT_X8R8G8B8
                || check.supported_usages != [0, D3D9_D3DUSAGE_DYNAMIC])
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-check-device-format-profile-invalid",
                "D3D9 device-format check must describe the bounded primary HAL texture band",
            ));
        }
        if let Some(create) = self.create_device.as_ref()
            && (!valid_policy_id(&create.profile_id)
                || !valid_policy_id(&create.display_profile_id)
                || create.adapter != 0
                || create.device_type != D3D9_D3DDEVTYPE_HAL
                || create.behavior_flags != D3D9_CREATE_HARDWARE_VERTEX_PROCESSING_MULTITHREADED
                || create.requested_back_buffer_width != 0
                || create.requested_back_buffer_height != 0
                || create.requested_back_buffer_format != D3D9_D3DFMT_UNKNOWN
                || create.resolved_back_buffer_format != D3D9_D3DFMT_X8R8G8B8
                || create.back_buffer_count != 1
                || create.multi_sample_type != D3D9_D3DMULTISAMPLE_NONE
                || create.multi_sample_quality != 0
                || create.swap_effect != D3D9_D3DSWAPEFFECT_DISCARD
                || !create.device_window_matches_focus_window
                || create.windowed != 1
                || create.enable_auto_depth_stencil != 0
                || create.auto_depth_stencil_format != D3D9_D3DFMT_D16
                || create.flags != 0
                || create.full_screen_refresh_rate_in_hz != 0
                || create.presentation_interval != 0)
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-create-device-profile-invalid",
                "D3D9 device-creation profile must describe the reached bounded windowed HAL presentation",
            ));
        }
        if let Some(create) = self.create_texture.as_ref()
            && (!valid_create_texture_policy(create) || self.create_device.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-create-texture-profile-invalid",
                "D3D9 texture-creation profile must describe either the historical system-memory texture or the reached initial three-texture sequence",
            ));
        }
        if let Some(surface) = self.get_surface_level.as_ref()
            && self
                .create_texture
                .as_ref()
                .is_none_or(|texture| !valid_get_surface_level_policy(surface, texture))
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-get-surface-level-profile-invalid",
                "D3D9 surface-level profile must select the reached system-memory texture level-zero variants",
            ));
        }
        if let Some(surface_dc) = self.surface_dc.as_ref()
            && (!valid_policy_id(&surface_dc.profile_id) || self.get_surface_level.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-surface-dc-profile-invalid",
                "D3D9 surface-DC profile requires the reached logical surface capability",
            ));
        }
        if let Some(vertex_buffer) = self.initial_vertex_buffer.as_ref()
            && (!valid_policy_id(&vertex_buffer.profile_id)
                || vertex_buffer.byte_length != 112
                || vertex_buffer.usage != D3D9_D3DUSAGE_WRITEONLY
                || vertex_buffer.flexible_vertex_format != 0x144
                || vertex_buffer.pool != D3D9_D3DPOOL_MANAGED
                || !vertex_buffer.shared_handle_must_be_null
                || vertex_buffer.lock_offset != 0
                || vertex_buffer.lock_byte_length != 0
                || vertex_buffer.lock_flags != 0
                || self.create_device.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-initial-vertex-buffer-profile-invalid",
                "D3D9 initial vertex-buffer profile must describe the reached managed write-only quad and full lock",
            ));
        }
        if let Some(render_states) = self.initial_render_states.as_ref()
            && (!valid_policy_id(&render_states.profile_id)
                || render_states.settings != controlled_initial_render_state_settings()
                || self.create_device.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-initial-render-state-profile-invalid",
                "D3D9 initial render-state profile must describe the reached fixed-function startup sequence",
            ));
        }
        if let Some(sampler_states) = self.initial_sampler_states.as_ref()
            && (sampler_states.profile_id != "primary-first-frame-linear-sampler-states-v1"
                || sampler_states.settings != controlled_initial_sampler_state_settings()
                || self.create_device.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-initial-sampler-state-profile-invalid",
                "D3D9 initial sampler-state profile must describe the reached sampler-zero linear minification and magnification sequence",
            ));
        }
        if let Some(texture_stage_states) = self.initial_texture_stage_states.as_ref()
            && (texture_stage_states.profile_id
                != "primary-first-frame-fixed-function-texture-stage-zero-v1"
                || texture_stage_states.settings
                    != controlled_initial_texture_stage_state_settings()
                || self.create_device.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-initial-texture-stage-state-profile-invalid",
                "D3D9 initial texture-stage-state profile must describe the reached stage-zero fixed-function color and alpha sequence",
            ));
        }
        if let Some(texture_binding) = self.texture_binding.as_ref()
            && (self
                .create_texture
                .as_ref()
                .is_none_or(|texture| !valid_texture_binding_policy(texture_binding, texture))
                || self.update_texture.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-texture-binding-profile-invalid",
                "D3D9 texture-binding profile must select the reached stage-zero static DEFAULT texture",
            ));
        }
        if let Some(vertex_format) = self.flexible_vertex_format.as_ref()
            && (vertex_format.profile_id
                != "primary-first-frame-transformed-diffuse-textured-fvf-v1"
                || vertex_format.value
                    != (D3D9_D3DFVF_XYZRHW | D3D9_D3DFVF_DIFFUSE | D3D9_D3DFVF_TEX1)
                || self
                    .initial_vertex_buffer
                    .as_ref()
                    .is_none_or(|vertex_buffer| {
                        vertex_buffer.flexible_vertex_format != vertex_format.value
                    }))
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-flexible-vertex-format-profile-invalid",
                "D3D9 flexible vertex format profile must match the reached transformed diffuse textured quad",
            ));
        }
        if let Some(stream_source) = self.stream_source.as_ref()
            && (stream_source.profile_id
                != "primary-first-frame-transformed-diffuse-textured-stream-zero-v1"
                || stream_source.stream != 0
                || stream_source.offset_in_bytes != 0
                || stream_source.stride != 28
                || self
                    .initial_vertex_buffer
                    .as_ref()
                    .is_none_or(|vertex_buffer| {
                        vertex_buffer.profile_id != "primary-initial-xyzw-diffuse-tex1-quad-v1"
                            || vertex_buffer.byte_length != 112
                            || vertex_buffer.flexible_vertex_format
                                != (D3D9_D3DFVF_XYZRHW | D3D9_D3DFVF_DIFFUSE | D3D9_D3DFVF_TEX1)
                    })
                || self.flexible_vertex_format.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-stream-source-profile-invalid",
                "D3D9 stream-source profile must bind the reached transformed diffuse textured quad to stream zero",
            ));
        }
        if let Some(draw) = self.draw_primitive.as_ref()
            && (draw.profile_id != "primary-first-frame-triangle-strip-quad-draw-v1"
                || draw.primitive_type != D3D9_D3DPT_TRIANGLESTRIP
                || draw.start_vertex != 0
                || draw.primitive_count != 2
                || self.begin_scene.is_none()
                || self.clear.is_none()
                || self.initial_render_states.is_none()
                || self.initial_sampler_states.is_none()
                || self.initial_texture_stage_states.is_none()
                || self.texture_binding.is_none()
                || self.flexible_vertex_format.is_none()
                || self.stream_source.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-draw-primitive-profile-invalid",
                "D3D9 DrawPrimitive profile must describe the reached prepared triangle-strip quad submission",
            ));
        }
        if let Some(raster_status) = self.raster_status.as_ref()
            && (raster_status.profile_id != "primary-stable-last-visible-scanline-v2"
                || !valid_policy_id(&raster_status.display_profile_id)
                || raster_status.in_v_blank
                || raster_status.scan_line != 1_079
                || self.create_device.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-raster-status-profile-invalid",
                "D3D9 raster-status profile must describe the deterministic last visible scanline of the primary logical display",
            ));
        }
        if let Some(clear) = self.clear.as_ref()
            && (!valid_policy_id(&clear.profile_id)
                || clear.rect_count != 0
                || !clear.rects_must_be_null
                || clear.flags != D3D9_D3DCLEAR_TARGET
                || clear.color_argb != 0xff00_0000
                || clear.depth_bits != 1.0_f32.to_bits()
                || clear.stencil != 0
                || self.create_device.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-clear-profile-invalid",
                "D3D9 clear profile must describe the reached full-render-target opaque black clear",
            ));
        }
        if let Some(add_dirty_rect) = self.add_dirty_rect.as_ref()
            && self
                .create_texture
                .as_ref()
                .is_none_or(|texture| !valid_add_dirty_rect_policy(add_dirty_rect, texture))
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-add-dirty-rect-profile-invalid",
                "D3D9 AddDirtyRect profile must select the reached recreated system-memory texture",
            ));
        }
        if let Some(texture_lock_rect) = self.texture_lock_rect.as_ref()
            && self
                .create_texture
                .as_ref()
                .is_none_or(|texture| !valid_texture_lock_rect_policy(texture_lock_rect, texture))
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-texture-lock-rect-profile-invalid",
                "D3D9 LockRect profile must select the reached recreated system-memory presentation rectangle",
            ));
        }
        if let Some(update_texture) = self.update_texture.as_ref()
            && self
                .create_texture
                .as_ref()
                .is_none_or(|texture| !valid_update_texture_policy(update_texture, texture))
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-update-texture-profile-invalid",
                "D3D9 UpdateTexture profile must select the reached recreated SYSTEMMEM and static DEFAULT pair",
            ));
        }
        if let Some(begin_scene) = self.begin_scene.as_ref()
            && (begin_scene.profile_id != "primary-first-frame-scene-begin-v1"
                || self.create_device.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-begin-scene-profile-invalid",
                "D3D9 BeginScene profile must describe the reached first-frame device scene transition",
            ));
        }
        if let Some(end_scene) = self.end_scene.as_ref()
            && (end_scene.profile_id != "primary-first-frame-scene-end-v1"
                || self.begin_scene.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-end-scene-profile-invalid",
                "D3D9 EndScene profile must close the reached first-frame logical scene",
            ));
        }
        if let Some(present) = self.present.as_ref()
            && (present.profile_id != "primary-first-frame-full-back-buffer-present-v1"
                || !present.source_rect_must_be_null
                || !present.destination_rect_must_be_null
                || !present.destination_window_override_must_be_null
                || !present.dirty_region_must_be_null
                || self.end_scene.is_none()
                || self.draw_primitive.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-present-profile-invalid",
                "D3D9 Present profile must submit the reached closed first-frame full back buffer",
            ));
        }
        let coherent_profiles = [
            self.get_adapter_count
                .as_ref()
                .map(|profile| (0, profile.display_profile_id.as_str())),
            self.get_adapter_monitor
                .as_ref()
                .map(|profile| (profile.adapter, profile.display_profile_id.as_str())),
            self.get_adapter_display_mode
                .as_ref()
                .map(|profile| (profile.adapter, profile.display_profile_id.as_str())),
            self.get_adapter_identifier
                .as_ref()
                .map(|profile| (profile.adapter, profile.display_profile_id.as_str())),
            self.get_device_caps
                .as_ref()
                .map(|profile| (profile.adapter, profile.display_profile_id.as_str())),
            self.check_device_type
                .as_ref()
                .map(|profile| (profile.adapter, profile.display_profile_id.as_str())),
            self.check_device_format
                .as_ref()
                .map(|profile| (profile.adapter, profile.display_profile_id.as_str())),
            self.create_device
                .as_ref()
                .map(|profile| (profile.adapter, profile.display_profile_id.as_str())),
            self.raster_status
                .as_ref()
                .map(|profile| (0, profile.display_profile_id.as_str())),
        ];
        let mut coherent_profiles = coherent_profiles.into_iter().flatten();
        if let Some(primary) = coherent_profiles.next()
            && coherent_profiles.any(|profile| profile != primary)
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-primary-adapter-profile-mismatch",
                "D3D9 root query profiles must select one coherent logical primary adapter",
            ));
        }
        let coherent_adapter_formats = [
            self.get_adapter_display_mode
                .as_ref()
                .map(|profile| profile.format),
            self.check_device_type
                .as_ref()
                .map(|profile| profile.adapter_format),
            self.check_device_format
                .as_ref()
                .map(|profile| profile.adapter_format),
            self.create_device
                .as_ref()
                .map(|profile| profile.resolved_back_buffer_format),
            self.create_texture
                .as_ref()
                .and_then(|profile| profile.variants.first())
                .map(|variant| variant.format),
        ];
        let mut coherent_adapter_formats = coherent_adapter_formats.into_iter().flatten();
        if let Some(primary) = coherent_adapter_formats.next()
            && coherent_adapter_formats.any(|format| format != primary)
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-primary-adapter-format-profile-mismatch",
                "D3D9 root query profiles must select one coherent adapter format",
            ));
        }
        Ok(())
    }
}

fn controlled_create_texture_variants() -> [D3d9CreateTextureVariant; 6] {
    [
        D3d9CreateTextureVariant {
            width: 1_024,
            height: 1_024,
            levels: 1,
            usage: 0,
            format: D3D9_D3DFMT_X8R8G8B8,
            pool: D3D9_D3DPOOL_SYSTEMMEM,
        },
        D3d9CreateTextureVariant {
            width: 1_024,
            height: 1_024,
            levels: 1,
            usage: 0,
            format: D3D9_D3DFMT_X8R8G8B8,
            pool: D3D9_D3DPOOL_DEFAULT,
        },
        D3d9CreateTextureVariant {
            width: 1_024,
            height: 1_024,
            levels: 1,
            usage: D3D9_D3DUSAGE_DYNAMIC,
            format: D3D9_D3DFMT_X8R8G8B8,
            pool: D3D9_D3DPOOL_DEFAULT,
        },
        D3d9CreateTextureVariant {
            width: 2_048,
            height: 1_024,
            levels: 1,
            usage: 0,
            format: D3D9_D3DFMT_X8R8G8B8,
            pool: D3D9_D3DPOOL_SYSTEMMEM,
        },
        D3d9CreateTextureVariant {
            width: 2_048,
            height: 1_024,
            levels: 1,
            usage: 0,
            format: D3D9_D3DFMT_X8R8G8B8,
            pool: D3D9_D3DPOOL_DEFAULT,
        },
        D3d9CreateTextureVariant {
            width: 2_048,
            height: 1_024,
            levels: 1,
            usage: D3D9_D3DUSAGE_DYNAMIC,
            format: D3D9_D3DFMT_X8R8G8B8,
            pool: D3D9_D3DPOOL_DEFAULT,
        },
    ]
}

fn controlled_initial_render_state_settings() -> [D3d9RenderStateSetting; 4] {
    [
        D3d9RenderStateSetting {
            state: D3D9_D3DRS_CULLMODE,
            value: D3D9_D3DCULL_NONE,
        },
        D3d9RenderStateSetting {
            state: D3D9_D3DRS_ZENABLE,
            value: 0,
        },
        D3d9RenderStateSetting {
            state: D3D9_D3DRS_LIGHTING,
            value: 0,
        },
        D3d9RenderStateSetting {
            state: D3D9_D3DRS_ALPHABLENDENABLE,
            value: 1,
        },
    ]
}

fn controlled_initial_sampler_state_settings() -> [D3d9SamplerStateSetting; 2] {
    [
        D3d9SamplerStateSetting {
            sampler: 0,
            state: D3D9_D3DSAMP_MINFILTER,
            value: D3D9_D3DTEXF_LINEAR,
        },
        D3d9SamplerStateSetting {
            sampler: 0,
            state: D3D9_D3DSAMP_MAGFILTER,
            value: D3D9_D3DTEXF_LINEAR,
        },
    ]
}

fn controlled_initial_texture_stage_state_settings() -> [D3d9TextureStageStateSetting; 6] {
    [
        D3d9TextureStageStateSetting {
            stage: 0,
            state: D3D9_D3DTSS_COLOROP,
            value: D3D9_D3DTOP_MODULATE,
        },
        D3d9TextureStageStateSetting {
            stage: 0,
            state: D3D9_D3DTSS_COLORARG1,
            value: D3D9_D3DTA_TEXTURE,
        },
        D3d9TextureStageStateSetting {
            stage: 0,
            state: D3D9_D3DTSS_COLORARG2,
            value: D3D9_D3DTA_DIFFUSE,
        },
        D3d9TextureStageStateSetting {
            stage: 0,
            state: D3D9_D3DTSS_ALPHAOP,
            value: D3D9_D3DTOP_SELECTARG1,
        },
        D3d9TextureStageStateSetting {
            stage: 0,
            state: D3D9_D3DTSS_ALPHAARG1,
            value: D3D9_D3DTA_TEXTURE,
        },
        D3d9TextureStageStateSetting {
            stage: 0,
            state: D3D9_D3DTSS_ALPHAARG2,
            value: D3D9_D3DTA_DIFFUSE,
        },
    ]
}

fn valid_create_texture_policy(policy: &D3d9CreateTexturePolicy) -> bool {
    if !valid_policy_id(&policy.profile_id) || !policy.shared_handle_must_be_null {
        return false;
    }
    let expected = controlled_create_texture_variants();
    match policy.variants.as_slice() {
        [system_memory] if policy.profile_id == "primary-system-memory-x8r8g8b8-texture-v1" => {
            system_memory == &expected[0]
        }
        [system_memory, default, dynamic]
            if policy.profile_id == "primary-initial-x8r8g8b8-textures-v1" =>
        {
            [system_memory, default, dynamic]
                .into_iter()
                .zip(expected.iter())
                .all(|(actual, expected)| actual == expected)
        }
        [
            initial_system_memory,
            initial_default,
            initial_dynamic,
            recreated_system_memory,
            recreated_default,
            recreated_dynamic,
        ] if policy.profile_id == "primary-initial-and-recreated-x8r8g8b8-textures-v2" => [
            initial_system_memory,
            initial_default,
            initial_dynamic,
            recreated_system_memory,
            recreated_default,
            recreated_dynamic,
        ]
        .into_iter()
        .zip(expected.iter())
        .all(|(actual, expected)| actual == expected),
        _ => false,
    }
}

fn valid_get_surface_level_policy(
    surface: &D3d9GetSurfaceLevelPolicy,
    texture: &D3d9CreateTexturePolicy,
) -> bool {
    if !valid_policy_id(&surface.profile_id) || surface.level != 0 {
        return false;
    }
    let expected_indices: &[u32] = match texture.profile_id.as_str() {
        "primary-system-memory-x8r8g8b8-texture-v1" => {
            if surface.profile_id != "primary-texture-level-zero-surface-v1" {
                return false;
            }
            &[0]
        }
        "primary-initial-x8r8g8b8-textures-v1" => {
            if surface.profile_id != "primary-texture-level-zero-surface-v1" {
                return false;
            }
            &[0]
        }
        "primary-initial-and-recreated-x8r8g8b8-textures-v2" => {
            if surface.profile_id != "primary-system-memory-texture-level-zero-surfaces-v2" {
                return false;
            }
            &[0, 3]
        }
        _ => return false,
    };
    surface.source_texture_variant_indices == expected_indices
        && surface.source_texture_variant_indices.iter().all(|index| {
            texture
                .variants
                .get(*index as usize)
                .is_some_and(|variant| {
                    surface.level < variant.levels
                        && variant.usage == 0
                        && variant.pool == D3D9_D3DPOOL_SYSTEMMEM
                })
        })
}

fn valid_add_dirty_rect_policy(
    policy: &D3d9AddDirtyRectPolicy,
    texture: &D3d9CreateTexturePolicy,
) -> bool {
    policy.profile_id == "primary-recreated-system-memory-texture-dirty-rect-v1"
        && policy.source_texture_variant_indices == [3]
        && policy.source_texture_variant_indices.iter().all(|index| {
            texture
                .variants
                .get(*index as usize)
                .is_some_and(|variant| {
                    variant.width == 2_048
                        && variant.height == 1_024
                        && variant.levels == 1
                        && variant.usage == 0
                        && variant.format == D3D9_D3DFMT_X8R8G8B8
                        && variant.pool == D3D9_D3DPOOL_SYSTEMMEM
                })
        })
}

fn valid_texture_lock_rect_policy(
    policy: &D3d9TextureLockRectPolicy,
    texture: &D3d9CreateTexturePolicy,
) -> bool {
    policy.profile_id == "primary-recreated-system-memory-presentation-lock-v1"
        && policy.source_texture_variant_indices == [3]
        && policy.level == 0
        && policy.rect
            == RuntimeRect {
                left: 0,
                top: 0,
                right: 1_280,
                bottom: 720,
            }
        && policy.flags == D3D9_D3DLOCK_NO_DIRTY_UPDATE
        && policy.source_texture_variant_indices.iter().all(|index| {
            texture
                .variants
                .get(*index as usize)
                .is_some_and(|variant| {
                    variant.width == 2_048
                        && variant.height == 1_024
                        && variant.levels == 1
                        && variant.usage == 0
                        && variant.format == D3D9_D3DFMT_X8R8G8B8
                        && variant.pool == D3D9_D3DPOOL_SYSTEMMEM
                })
        })
}

fn valid_update_texture_policy(
    policy: &D3d9UpdateTexturePolicy,
    texture: &D3d9CreateTexturePolicy,
) -> bool {
    let valid_dirty_rect_profile = match policy.profile_id.as_str() {
        "primary-recreated-system-memory-to-static-default-texture-update-v2" => {
            !policy.allow_bounded_partial_dirty_rects
        }
        "primary-recreated-system-memory-to-static-default-texture-bounded-update-v3" => {
            policy.allow_bounded_partial_dirty_rects
        }
        _ => false,
    };
    if !valid_dirty_rect_profile
        || policy.source_texture_variant_indices != [3]
        || policy.destination_texture_variant_indices != [4]
        || policy.partial_dirty_rect
            != (RuntimeRect {
                left: 0,
                top: 0,
                right: 1_280,
                bottom: 720,
            })
    {
        return false;
    }
    let Some(source) = texture.variants.get(3) else {
        return false;
    };
    let Some(destination) = texture.variants.get(4) else {
        return false;
    };
    source.width == 2_048
        && source.height == 1_024
        && source.levels == 1
        && source.usage == 0
        && source.format == D3D9_D3DFMT_X8R8G8B8
        && source.pool == D3D9_D3DPOOL_SYSTEMMEM
        && destination.width == source.width
        && destination.height == source.height
        && destination.levels == source.levels
        && destination.usage == 0
        && destination.format == source.format
        && destination.pool == D3D9_D3DPOOL_DEFAULT
}

fn valid_texture_binding_policy(
    policy: &D3d9TextureBindingPolicy,
    texture: &D3d9CreateTexturePolicy,
) -> bool {
    policy.profile_id == "primary-first-frame-static-default-texture-stage-zero-v1"
        && policy.stage == 0
        && policy.texture_variant_indices == [4]
        && policy.null_unbind_supported
        && policy.texture_variant_indices.iter().all(|index| {
            texture
                .variants
                .get(*index as usize)
                .is_some_and(|variant| {
                    variant.width == 2_048
                        && variant.height == 1_024
                        && variant.levels == 1
                        && variant.usage == 0
                        && variant.format == D3D9_D3DFMT_X8R8G8B8
                        && variant.pool == D3D9_D3DPOOL_DEFAULT
                })
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9PresentParameters {
    pub back_buffer_width: u32,
    pub back_buffer_height: u32,
    pub back_buffer_format: u32,
    pub back_buffer_count: u32,
    pub multi_sample_type: u32,
    pub multi_sample_quality: u32,
    pub swap_effect: u32,
    pub device_window: u32,
    pub windowed: u32,
    pub enable_auto_depth_stencil: u32,
    pub auto_depth_stencil_format: u32,
    pub flags: u32,
    pub full_screen_refresh_rate_in_hz: u32,
    pub presentation_interval: u32,
}

impl D3d9PresentParameters {
    pub fn from_x86_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != D3D9_D3DPRESENT_PARAMETERS_BYTE_LENGTH as usize {
            return None;
        }
        let mut words = [0_u32; 14];
        for (word, bytes) in words.iter_mut().zip(bytes.chunks_exact(4)) {
            *word = u32::from_le_bytes(bytes.try_into().ok()?);
        }
        Some(Self {
            back_buffer_width: words[0],
            back_buffer_height: words[1],
            back_buffer_format: words[2],
            back_buffer_count: words[3],
            multi_sample_type: words[4],
            multi_sample_quality: words[5],
            swap_effect: words[6],
            device_window: words[7],
            windowed: words[8],
            enable_auto_depth_stencil: words[9],
            auto_depth_stencil_format: words[10],
            flags: words[11],
            full_screen_refresh_rate_in_hz: words[12],
            presentation_interval: words[13],
        })
    }

    fn matches_policy(self, policy: &D3d9CreateDevicePolicy, focus_window: u32) -> bool {
        self.back_buffer_width == policy.requested_back_buffer_width
            && self.back_buffer_height == policy.requested_back_buffer_height
            && self.back_buffer_format == policy.requested_back_buffer_format
            && self.back_buffer_count == policy.back_buffer_count
            && self.multi_sample_type == policy.multi_sample_type
            && self.multi_sample_quality == policy.multi_sample_quality
            && self.swap_effect == policy.swap_effect
            && (!policy.device_window_matches_focus_window || self.device_window == focus_window)
            && self.windowed == policy.windowed
            && self.enable_auto_depth_stencil == policy.enable_auto_depth_stencil
            && self.auto_depth_stencil_format == policy.auto_depth_stencil_format
            && self.flags == policy.flags
            && self.full_screen_refresh_rate_in_hz == policy.full_screen_refresh_rate_in_hz
            && self.presentation_interval == policy.presentation_interval
    }

    fn resolved_prefix(self, width: u32, height: u32, format: u32) -> Vec<u8> {
        [width, height, format]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9DisplayMode {
    pub width: u32,
    pub height: u32,
    pub refresh_rate: u32,
    pub format: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9RasterStatus {
    pub in_v_blank: bool,
    pub scan_line: u32,
}

impl D3d9RasterStatus {
    fn to_x86_bytes(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(D3D9_D3DRASTER_STATUS_BYTE_LENGTH as usize);
        bytes.extend_from_slice(&u32::from(self.in_v_blank).to_le_bytes());
        bytes.extend_from_slice(&self.scan_line.to_le_bytes());
        bytes
    }
}

impl D3d9DisplayMode {
    pub fn to_x86_bytes(self) -> Vec<u8> {
        [self.width, self.height, self.refresh_rate, self.format]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9AdapterIdentifier {
    pub driver: String,
    pub description: String,
    pub device_name: String,
    pub driver_version: u64,
    pub vendor_id: u32,
    pub device_id: u32,
    pub sub_sys_id: u32,
    pub revision: u32,
    pub device_identifier: [u8; 16],
    pub whql_level: u32,
}

impl D3d9AdapterIdentifier {
    fn from_policy(policy: &D3d9AdapterIdentifierPolicy) -> Self {
        Self {
            driver: policy.driver.clone(),
            description: policy.description.clone(),
            device_name: policy.device_name.clone(),
            driver_version: policy.driver_version,
            vendor_id: policy.vendor_id,
            device_id: policy.device_id,
            sub_sys_id: policy.sub_sys_id,
            revision: policy.revision,
            device_identifier: policy.device_identifier,
            whql_level: policy.whql_level,
        }
    }

    pub fn to_x86_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![0; D3D9_D3DADAPTER_IDENTIFIER9_BYTE_LENGTH as usize];
        write_fixed_ansi(&mut bytes, 0, 512, &self.driver);
        write_fixed_ansi(&mut bytes, 512, 512, &self.description);
        write_fixed_ansi(&mut bytes, 1_024, 32, &self.device_name);
        bytes[1_056..1_064].copy_from_slice(&self.driver_version.to_le_bytes());
        for (offset, value) in [
            (1_064, self.vendor_id),
            (1_068, self.device_id),
            (1_072, self.sub_sys_id),
            (1_076, self.revision),
            (1_096, self.whql_level),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[1_080..1_096].copy_from_slice(&self.device_identifier);
        bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9DeviceCaps {
    pub device_type: u32,
    pub adapter_ordinal: u32,
    pub caps: u32,
    pub caps2: u32,
    pub pixel_shader_version: u32,
    pub number_of_adapters_in_group: u32,
}

impl D3d9DeviceCaps {
    fn from_policy(policy: &D3d9DeviceCapsPolicy) -> Self {
        Self {
            device_type: policy.device_type,
            adapter_ordinal: policy.adapter,
            caps: policy.caps,
            caps2: policy.caps2,
            pixel_shader_version: policy.pixel_shader_version,
            number_of_adapters_in_group: policy.number_of_adapters_in_group,
        }
    }

    pub fn to_x86_bytes(self) -> Vec<u8> {
        let mut words = [0_u32; 76];
        words[0] = self.device_type;
        words[1] = self.adapter_ordinal;
        words[2] = self.caps;
        words[3] = self.caps2;
        words[51] = self.pixel_shader_version;
        words[58] = self.number_of_adapters_in_group;
        words.into_iter().flat_map(u32::to_le_bytes).collect()
    }
}

fn valid_policy_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && value.is_ascii() && !value.as_bytes().contains(&0)
}

fn valid_fixed_ansi(value: &str, capacity: usize) -> bool {
    !value.is_empty()
        && value.len() < capacity
        && value.is_ascii()
        && !value.as_bytes().contains(&0)
}

fn write_fixed_ansi(bytes: &mut [u8], offset: usize, capacity: usize, value: &str) {
    debug_assert!(valid_fixed_ansi(value, capacity));
    bytes[offset..offset + value.len()].copy_from_slice(value.as_bytes());
}

fn guest_ranges_overlap(
    left: GuestAddress,
    left_length: usize,
    right: GuestAddress,
    right_length: usize,
) -> bool {
    let left_start = u64::from(left.0);
    let right_start = u64::from(right.0);
    let left_end = left_start + left_length as u64;
    let right_end = right_start + right_length as u64;
    left_start < right_end && right_start < left_end
}

pub fn dispatch_d3d9_api(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    match contract.implementation_id.as_str() {
        D3D9_DIRECT3D_CREATE9_IMPLEMENTATION => handle_direct3d_create9(contract, frame, session),
        implementation_id => ApiHandlerResult::Fault(ApiFault::unsupported(
            "d3d9-api-dispatch",
            "implementation-id-unsupported",
            format!("D3D9 implementation {implementation_id} is not registered"),
        )),
    }
}

pub fn dispatch_d3d9_com_call(
    policy: &D3d9CompatibilityPolicy,
    trap: ComCallTrap,
    session: &mut ExecutionSession<'_>,
) -> ComDispatchOutcome {
    session.dispatch_com_call_with_frontend(trap, |call| dispatch_d3d9_frontend_call(policy, call))
}

fn dispatch_d3d9_frontend_call(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let is_root = call.class_identity() == D3D9_IDIRECT3D9_CLASS_IDENTITY
        && call.frame().iid == D3D9_IDIRECT3D9_IID;
    let is_device = call.class_identity() == D3D9_IDIRECT3DDEVICE9_CLASS_IDENTITY
        && call.frame().iid == D3D9_IDIRECT3DDEVICE9_IID;
    let is_texture = call.class_identity() == D3D9_IDIRECT3DTEXTURE9_CLASS_IDENTITY
        && call.frame().iid == D3D9_IDIRECT3DTEXTURE9_IID;
    let is_surface = call.class_identity() == D3D9_IDIRECT3DSURFACE9_CLASS_IDENTITY
        && call.frame().iid == D3D9_IDIRECT3DSURFACE9_IID;
    let is_vertex_buffer = call.class_identity() == D3D9_IDIRECT3DVERTEXBUFFER9_CLASS_IDENTITY
        && call.frame().iid == D3D9_IDIRECT3DVERTEXBUFFER9_IID;
    if !is_root && !is_device && !is_texture && !is_surface && !is_vertex_buffer {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "d3d9-com-dispatch",
            "d3d9-com-route-mismatch",
            "D3D9 frontend received a COM method outside its registered interface routes",
        );
    }
    if let Err(fault) = policy.validate() {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            fault.stage,
            fault.code,
            fault.message,
        );
    }
    if is_device {
        return dispatch_d3d9_device_frontend_call(policy, call);
    }
    if is_texture {
        return dispatch_d3d9_texture_frontend_call(policy, call);
    }
    if is_surface {
        return dispatch_d3d9_surface_frontend_call(policy, call);
    }
    if is_vertex_buffer {
        return dispatch_d3d9_vertex_buffer_frontend_call(policy, call);
    }
    dispatch_d3d9_root_frontend_call(policy, call)
}

fn dispatch_d3d9_root_frontend_call(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let (expected_argument_words, expected_return_kind) =
        match (call.frame().vtable_slot, call.frame().method.as_str()) {
            (4, "GetAdapterCount") => (0, RuntimeComReturnKind::U32),
            (15, "GetAdapterMonitor") => (1, RuntimeComReturnKind::U32),
            (5, "GetAdapterIdentifier") => (3, RuntimeComReturnKind::HResult),
            (8, "GetAdapterDisplayMode") => (2, RuntimeComReturnKind::HResult),
            (9, "CheckDeviceType") => (5, RuntimeComReturnKind::HResult),
            (10, "CheckDeviceFormat") => (6, RuntimeComReturnKind::HResult),
            (14, "GetDeviceCaps") => (3, RuntimeComReturnKind::HResult),
            (16, "CreateDevice") => (6, RuntimeComReturnKind::HResult),
            _ => {
                return call.fault(
                    ComFaultClass::RuntimeDefect,
                    "d3d9-com-dispatch",
                    "d3d9-com-route-mismatch",
                    "D3D9 frontend received a COM method outside its registered IDirect3D9 route",
                );
            }
        };
    if call.explicit_argument_words() != expected_argument_words
        || call.frame().explicit_arguments.len() != usize::from(expected_argument_words)
        || call.return_kind() != expected_return_kind
    {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "d3d9-com-dispatch",
            "d3d9-com-abi-mismatch",
            "registered IDirect3D9 frontend ABI does not match its selected method",
        );
    }
    match call.frame().vtable_slot {
        4 => dispatch_get_adapter_count(policy, call),
        15 => dispatch_get_adapter_monitor(policy, call),
        5 => dispatch_get_adapter_identifier(policy, call),
        8 => dispatch_get_adapter_display_mode(policy, call),
        9 => dispatch_check_device_type(policy, call),
        10 => dispatch_check_device_format(policy, call),
        14 => dispatch_get_device_caps(policy, call),
        16 => dispatch_create_device(policy, call),
        _ => unreachable!(),
    }
}

fn dispatch_d3d9_device_frontend_call(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let expected = match call.frame().vtable_slot {
        3 => ("TestCooperativeLevel", 0),
        17 => ("Present", 4),
        19 => ("GetRasterStatus", 2),
        23 => ("CreateTexture", 8),
        26 => ("CreateVertexBuffer", 6),
        31 => ("UpdateTexture", 2),
        41 => ("BeginScene", 0),
        42 => ("EndScene", 0),
        43 => ("Clear", 6),
        57 => ("SetRenderState", 2),
        65 => ("SetTexture", 2),
        67 => ("SetTextureStageState", 3),
        69 => ("SetSamplerState", 3),
        81 => ("DrawPrimitive", 3),
        89 => ("SetFVF", 1),
        100 => ("SetStreamSource", 4),
        _ => {
            return call.fault(
                ComFaultClass::RuntimeDefect,
                "d3d9-com-dispatch",
                "d3d9-com-route-mismatch",
                "D3D9 frontend received a COM method outside its registered IDirect3DDevice9 route",
            );
        }
    };
    if call.frame().method != expected.0
        || call.explicit_argument_words() != expected.1
        || call.frame().explicit_arguments.len() != usize::from(expected.1)
        || call.return_kind() != RuntimeComReturnKind::HResult
    {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "d3d9-com-dispatch",
            "d3d9-com-abi-mismatch",
            "registered IDirect3DDevice9 frontend ABI does not match its selected method",
        );
    }
    match call.frame().vtable_slot {
        3 => dispatch_test_cooperative_level(policy, call),
        17 => dispatch_present(policy, call),
        19 => dispatch_get_raster_status(policy, call),
        23 => dispatch_create_texture(policy, call),
        26 => dispatch_create_vertex_buffer(policy, call),
        31 => dispatch_update_texture(policy, call),
        41 => dispatch_begin_scene(policy, call),
        42 => dispatch_end_scene(policy, call),
        43 => dispatch_clear(policy, call),
        57 => dispatch_set_render_state(policy, call),
        65 => dispatch_set_texture(policy, call),
        67 => dispatch_set_texture_stage_state(policy, call),
        69 => dispatch_set_sampler_state(policy, call),
        81 => dispatch_draw_primitive(policy, call),
        89 => dispatch_set_fvf(policy, call),
        100 => dispatch_set_stream_source(policy, call),
        _ => unreachable!(),
    }
}

fn dispatch_test_cooperative_level(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(test_policy) = policy.test_cooperative_level.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-test-cooperative-level",
            "test-cooperative-level-policy-disabled",
            "D3D9 compatibility policy does not enable TestCooperativeLevel",
        );
    };
    debug_assert_eq!(
        test_policy.profile_id,
        "primary-live-operational-device-query-v1"
    );
    if let Err(error) = call
        .runtime()
        .graphics_device_by_com_object(call.frame().object_id)
    {
        return call.runtime_fault(error);
    }
    call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new())
}

fn dispatch_present(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(present_policy) = policy.present.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-present",
            "present-policy-disabled",
            "D3D9 compatibility policy does not enable Present",
        );
    };
    debug_assert_eq!(
        present_policy.profile_id,
        "primary-first-frame-full-back-buffer-present-v1"
    );
    let [
        source_rect,
        destination_rect,
        destination_window_override,
        dirty_region,
    ] = call.frame().explicit_arguments.as_slice()
    else {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "d3d9-present",
            "present-abi-mismatch",
            "D3D9 Present did not decode four explicit arguments",
        );
    };
    if (present_policy.source_rect_must_be_null && *source_rect != 0)
        || (present_policy.destination_rect_must_be_null && *destination_rect != 0)
        || (present_policy.destination_window_override_must_be_null
            && *destination_window_override != 0)
        || (present_policy.dirty_region_must_be_null && *dirty_region != 0)
    {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }
    let plan = match call
        .runtime()
        .plan_graphics_present(RuntimeGraphicsPresentRequest {
            device_com_object_id: call.frame().object_id,
        }) {
        Ok(plan) => plan,
        Err(error)
            if matches!(
                error.class,
                RuntimeStateErrorClass::InvalidInput | RuntimeStateErrorClass::NotFound
            ) =>
        {
            return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
        }
        Err(error) => return call.runtime_fault(error),
    };
    if let Err(error) = call.runtime().preflight_graphics_present(&plan) {
        return call.runtime_fault(error);
    }
    let mut call = call;
    call.runtime_mut().commit_graphics_present(plan);
    call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new())
}

fn dispatch_begin_scene(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(begin_policy) = policy.begin_scene.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-begin-scene",
            "begin-scene-policy-disabled",
            "D3D9 compatibility policy does not enable BeginScene",
        );
    };
    debug_assert_eq!(
        begin_policy.profile_id,
        "primary-first-frame-scene-begin-v1"
    );
    let plan = match call
        .runtime()
        .plan_graphics_scene_begin(RuntimeGraphicsSceneBeginRequest {
            device_com_object_id: call.frame().object_id,
        }) {
        Ok(plan) => plan,
        Err(error)
            if matches!(
                error.class,
                RuntimeStateErrorClass::InvalidInput | RuntimeStateErrorClass::NotFound
            ) =>
        {
            return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
        }
        Err(error) => return call.runtime_fault(error),
    };
    if let Err(error) = call.runtime().preflight_graphics_scene_begin(&plan) {
        return call.runtime_fault(error);
    }
    let mut call = call;
    call.runtime_mut().commit_graphics_scene_begin(plan);
    call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new())
}

fn dispatch_end_scene(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(end_policy) = policy.end_scene.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-end-scene",
            "end-scene-policy-disabled",
            "D3D9 compatibility policy does not enable EndScene",
        );
    };
    debug_assert_eq!(end_policy.profile_id, "primary-first-frame-scene-end-v1");
    let plan = match call
        .runtime()
        .plan_graphics_scene_end(RuntimeGraphicsSceneEndRequest {
            device_com_object_id: call.frame().object_id,
        }) {
        Ok(plan) => plan,
        Err(error)
            if matches!(
                error.class,
                RuntimeStateErrorClass::InvalidInput | RuntimeStateErrorClass::NotFound
            ) =>
        {
            return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
        }
        Err(error) => return call.runtime_fault(error),
    };
    if let Err(error) = call.runtime().preflight_graphics_scene_end(&plan) {
        return call.runtime_fault(error);
    }
    let mut call = call;
    call.runtime_mut().commit_graphics_scene_end(plan);
    call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new())
}

fn dispatch_clear(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(clear_policy) = policy.clear.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-clear",
            "clear-policy-disabled",
            "D3D9 compatibility policy does not enable Clear",
        );
    };
    let rect_count = call.frame().explicit_arguments[0];
    let rects_pointer = call.frame().explicit_arguments[1];
    let flags = call.frame().explicit_arguments[2];
    let color_argb = call.frame().explicit_arguments[3];
    let depth_bits = call.frame().explicit_arguments[4];
    let stencil = call.frame().explicit_arguments[5];
    if rect_count != clear_policy.rect_count
        || (clear_policy.rects_must_be_null && rects_pointer != 0)
        || flags != clear_policy.flags
        || color_argb != clear_policy.color_argb
        || depth_bits != clear_policy.depth_bits
        || stencil != clear_policy.stencil
    {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }
    let plan = match call.runtime().plan_graphics_back_buffer_clear(
        RuntimeGraphicsBackBufferClearRequest {
            device_com_object_id: call.frame().object_id,
            color_argb,
        },
    ) {
        Ok(plan) => plan,
        Err(error) => return call.runtime_fault(error),
    };
    if let Err(error) = call.runtime().preflight_graphics_back_buffer_clear(&plan) {
        return call.runtime_fault(error);
    }
    let mut call = call;
    let result = call.runtime_mut().commit_graphics_back_buffer_clear(plan);
    debug_assert_eq!(result.clear.color_argb, color_argb);
    call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new())
}

fn dispatch_get_raster_status(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(raster_policy) = policy.raster_status.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-raster-status",
            "raster-status-policy-disabled",
            "D3D9 compatibility policy does not enable GetRasterStatus",
        );
    };
    let swap_chain = call.frame().explicit_arguments[0];
    let output_pointer = call.frame().explicit_arguments[1];
    if output_pointer == 0 {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }
    let back_buffer_count = match call
        .runtime()
        .graphics_device_by_com_object(call.frame().object_id)
    {
        Ok(device) => device.presentation.back_buffer_count,
        Err(error) => return call.runtime_fault(error),
    };
    if swap_chain >= back_buffer_count {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }
    if let Some((code, message)) =
        primary_display_authority_fault(&call, &raster_policy.display_profile_id)
    {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "d3d9-raster-status",
            code,
            message,
        );
    }
    let status = D3d9RasterStatus {
        in_v_blank: raster_policy.in_v_blank,
        scan_line: raster_policy.scan_line,
    };
    let bytes = status.to_x86_bytes();
    debug_assert_eq!(bytes.len(), D3D9_D3DRASTER_STATUS_BYTE_LENGTH as usize);
    complete_d3d9_write(call, GuestAddress(output_pointer), bytes)
}

fn dispatch_d3d9_texture_frontend_call(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let expected = match call.frame().vtable_slot {
        18 => ("GetSurfaceLevel", 2),
        19 => ("LockRect", 4),
        20 => ("UnlockRect", 1),
        21 => ("AddDirtyRect", 1),
        _ => {
            return call.fault(
                ComFaultClass::RuntimeDefect,
                "d3d9-com-dispatch",
                "d3d9-com-route-mismatch",
                "D3D9 frontend received a COM method outside its registered IDirect3DTexture9 route",
            );
        }
    };
    if call.frame().method != expected.0
        || call.explicit_argument_words() != expected.1
        || call.frame().explicit_arguments.len() != usize::from(expected.1)
        || call.return_kind() != RuntimeComReturnKind::HResult
    {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "d3d9-com-dispatch",
            "d3d9-com-abi-mismatch",
            "registered IDirect3DTexture9 frontend ABI does not match its selected method",
        );
    }
    match call.frame().vtable_slot {
        18 => dispatch_get_surface_level(policy, call),
        19 => dispatch_texture_lock_rect(policy, call),
        20 => dispatch_texture_unlock_rect(policy, call),
        21 => dispatch_add_dirty_rect(policy, call),
        _ => unreachable!(),
    }
}

fn dispatch_d3d9_surface_frontend_call(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let expected_method = match call.frame().vtable_slot {
        15 => "GetDC",
        16 => "ReleaseDC",
        _ => {
            return call.fault(
                ComFaultClass::RuntimeDefect,
                "d3d9-com-dispatch",
                "d3d9-com-route-mismatch",
                "D3D9 frontend received a COM method outside its registered IDirect3DSurface9 route",
            );
        }
    };
    if call.frame().method != expected_method
        || call.explicit_argument_words() != 1
        || call.frame().explicit_arguments.len() != 1
        || call.return_kind() != RuntimeComReturnKind::HResult
    {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "d3d9-com-dispatch",
            "d3d9-com-abi-mismatch",
            "registered IDirect3DSurface9 frontend ABI does not match GetDC or ReleaseDC",
        );
    }
    match call.frame().vtable_slot {
        15 => dispatch_surface_get_dc(policy, call),
        16 => dispatch_surface_release_dc(policy, call),
        _ => unreachable!(),
    }
}

fn dispatch_d3d9_vertex_buffer_frontend_call(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let expected = match call.frame().vtable_slot {
        11 => ("Lock", 4),
        12 => ("Unlock", 0),
        _ => {
            return call.fault(
                ComFaultClass::RuntimeDefect,
                "d3d9-com-dispatch",
                "d3d9-com-route-mismatch",
                "D3D9 frontend received a COM method outside its registered IDirect3DVertexBuffer9 route",
            );
        }
    };
    if call.frame().method != expected.0
        || call.explicit_argument_words() != expected.1
        || call.frame().explicit_arguments.len() != usize::from(expected.1)
        || call.return_kind() != RuntimeComReturnKind::HResult
    {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "d3d9-com-dispatch",
            "d3d9-com-abi-mismatch",
            "registered IDirect3DVertexBuffer9 frontend ABI does not match its selected method",
        );
    }
    match call.frame().vtable_slot {
        11 => dispatch_vertex_buffer_lock(policy, call),
        12 => dispatch_vertex_buffer_unlock(policy, call),
        _ => unreachable!(),
    }
}

fn dispatch_get_adapter_count(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(count_policy) = policy.get_adapter_count.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-adapter-count",
            "get-adapter-count-policy-disabled",
            "D3D9 compatibility policy does not enable GetAdapterCount",
        );
    };
    if let Some((code, message)) =
        primary_display_authority_fault(&call, &count_policy.display_profile_id)
    {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "d3d9-adapter-count",
            code,
            message,
        );
    }
    call.complete(ComReturnValue::U32(count_policy.adapter_count), Vec::new())
}

fn dispatch_get_adapter_monitor(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(monitor_policy) = policy.get_adapter_monitor.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-adapter-monitor",
            "get-adapter-monitor-policy-disabled",
            "D3D9 compatibility policy does not enable GetAdapterMonitor",
        );
    };
    let adapter = call.frame().explicit_arguments[0];
    if adapter != monitor_policy.adapter {
        return call.complete(ComReturnValue::U32(0), Vec::new());
    }
    if let Some((code, message)) =
        primary_display_authority_fault(&call, &monitor_policy.display_profile_id)
    {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "d3d9-adapter-monitor",
            code,
            message,
        );
    }
    let monitor_handle = call
        .runtime()
        .primary_monitor()
        .expect("display authority preflight requires a primary monitor")
        .handle;
    call.complete(ComReturnValue::U32(monitor_handle), Vec::new())
}

fn dispatch_create_device(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(create_policy) = policy.create_device.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-create-device",
            "create-device-policy-disabled",
            "D3D9 compatibility policy does not enable CreateDevice",
        );
    };
    let adapter = call.frame().explicit_arguments[0];
    let device_type = call.frame().explicit_arguments[1];
    let focus_window = call.frame().explicit_arguments[2];
    let behavior_flags = call.frame().explicit_arguments[3];
    let presentation_pointer = call.frame().explicit_arguments[4];
    let output_pointer = call.frame().explicit_arguments[5];
    if adapter != create_policy.adapter
        || device_type != create_policy.device_type
        || focus_window == 0
        || presentation_pointer == 0
        || output_pointer == 0
    {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }
    if behavior_flags != create_policy.behavior_flags {
        return call.complete(
            ComReturnValue::HResult(D3D9_D3DERR_NOTAVAILABLE),
            Vec::new(),
        );
    }
    if let Some((code, message)) =
        primary_display_authority_fault(&call, &create_policy.display_profile_id)
    {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "d3d9-create-device",
            code,
            message,
        );
    }

    let presentation_address = GuestAddress(presentation_pointer);
    let output_address = GuestAddress(output_pointer);
    if guest_ranges_overlap(
        presentation_address,
        D3D9_D3DPRESENT_PARAMETERS_BYTE_LENGTH as usize,
        output_address,
        4,
    ) {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }
    if let Err(error) = call.memory().validate_access(
        presentation_address,
        D3D9_D3DPRESENT_PARAMETERS_BYTE_LENGTH as usize,
        MemoryAccess::Read,
    ) {
        return call.memory_fault(error);
    }
    if let Err(error) = call
        .memory()
        .validate_access(presentation_address, 12, MemoryAccess::Write)
    {
        return call.memory_fault(error);
    }
    if let Err(error) = call
        .memory()
        .validate_access(output_address, 4, MemoryAccess::Write)
    {
        return call.memory_fault(error);
    }
    let presentation_bytes = match call.memory().read(
        presentation_address,
        D3D9_D3DPRESENT_PARAMETERS_BYTE_LENGTH as usize,
    ) {
        Ok(bytes) => bytes,
        Err(error) => return call.memory_fault(error),
    };
    let presentation = D3d9PresentParameters::from_x86_bytes(&presentation_bytes)
        .expect("validated D3DPRESENT_PARAMETERS bytes must decode");
    if presentation.device_window != focus_window {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }
    if !presentation.matches_policy(create_policy, focus_window) {
        return call.complete(
            ComReturnValue::HResult(D3D9_D3DERR_NOTAVAILABLE),
            Vec::new(),
        );
    }

    let graphics_plan =
        match call
            .runtime()
            .plan_graphics_device_creation(RuntimeGraphicsDeviceCreateRequest {
                frontend: RuntimeGraphicsFrontend::D3d9,
                window_handle: focus_window,
                color_format: RuntimeGraphicsColorFormat::B8g8r8x8Unorm,
                back_buffer_count: presentation.back_buffer_count,
                sample_count: 1,
                present_mode: RuntimeGraphicsPresentMode::Discard,
                windowed: true,
            }) {
            Ok(plan) => plan,
            Err(error)
                if matches!(
                    error.class,
                    RuntimeStateErrorClass::InvalidInput
                        | RuntimeStateErrorClass::NotFound
                        | RuntimeStateErrorClass::Conflict
                ) =>
            {
                return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
            }
            Err(error) => return call.runtime_fault(error),
        };
    if let Err(error) = call
        .runtime()
        .preflight_graphics_device_creation(&graphics_plan)
    {
        return call.runtime_fault(error);
    }
    let planned_device_id = graphics_plan.id();
    let logical_presentation = graphics_plan.presentation();
    let presentation_prefix = presentation.resolved_prefix(
        logical_presentation.width,
        logical_presentation.height,
        create_policy.resolved_back_buffer_format,
    );

    let mut call = call;
    let (installed, guest_writes) = match call.install_child_com_object_with_guest_writes(
        direct3ddevice9_object_spec(),
        |object| {
            let interface = object
                .interfaces
                .iter()
                .find(|interface| interface.iid == D3D9_IDIRECT3DDEVICE9_IID)
                .ok_or_else(|| {
                    RuntimeStateError::runtime_defect(
                        "d3d9-create-device",
                        "idirect3ddevice9-installation-missing",
                        "installed D3D9 device object has no IDirect3DDevice9 interface",
                    )
                })?;
            Ok(vec![
                (presentation_address, presentation_prefix),
                (
                    output_address,
                    interface.guest_pointer.0.to_le_bytes().to_vec(),
                ),
            ])
        },
    ) {
        Ok(result) => result,
        Err(error) => return call.runtime_fault(error),
    };
    let device = call
        .runtime_mut()
        .commit_graphics_device_creation(graphics_plan, installed.object.id);
    debug_assert_eq!(device.id, planned_device_id);
    debug_assert_eq!(device.presentation, logical_presentation);
    call.complete(ComReturnValue::HResult(COM_S_OK), guest_writes)
}

fn dispatch_create_texture(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(create_policy) = policy.create_texture.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-create-texture",
            "create-texture-policy-disabled",
            "D3D9 compatibility policy does not enable CreateTexture",
        );
    };
    let width = call.frame().explicit_arguments[0];
    let height = call.frame().explicit_arguments[1];
    let levels = call.frame().explicit_arguments[2];
    let usage = call.frame().explicit_arguments[3];
    let format = call.frame().explicit_arguments[4];
    let pool = call.frame().explicit_arguments[5];
    let output_pointer = call.frame().explicit_arguments[6];
    let shared_handle_pointer = call.frame().explicit_arguments[7];
    if width == 0
        || height == 0
        || output_pointer == 0
        || (create_policy.shared_handle_must_be_null && shared_handle_pointer != 0)
    {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }
    let Some(create_variant) = create_policy.variants.iter().find(|variant| {
        width == variant.width
            && height == variant.height
            && levels == variant.levels
            && usage == variant.usage
            && format == variant.format
            && pool == variant.pool
    }) else {
        return call.complete(
            ComReturnValue::HResult(D3D9_D3DERR_NOTAVAILABLE),
            Vec::new(),
        );
    };
    let logical_usage = match create_variant.usage {
        0 => RuntimeGraphicsTextureUsage::None,
        D3D9_D3DUSAGE_DYNAMIC => RuntimeGraphicsTextureUsage::Dynamic,
        _ => unreachable!("validated D3D9 texture policy has an unknown usage"),
    };
    let logical_pool = match create_variant.pool {
        D3D9_D3DPOOL_DEFAULT => RuntimeGraphicsTexturePool::Default,
        D3D9_D3DPOOL_SYSTEMMEM => RuntimeGraphicsTexturePool::SystemMemory,
        _ => unreachable!("validated D3D9 texture policy has an unknown pool"),
    };

    let output_address = GuestAddress(output_pointer);
    if let Err(error) = call
        .memory()
        .validate_access(output_address, 4, MemoryAccess::Write)
    {
        return call.memory_fault(error);
    }
    let graphics_plan =
        match call
            .runtime()
            .plan_graphics_texture_creation(RuntimeGraphicsTextureCreateRequest {
                device_com_object_id: call.frame().object_id,
                descriptor: RuntimeGraphicsTextureDescriptor {
                    width,
                    height,
                    levels,
                    usage: logical_usage,
                    color_format: RuntimeGraphicsColorFormat::B8g8r8x8Unorm,
                    pool: logical_pool,
                },
            }) {
            Ok(plan) => plan,
            Err(error) => return call.runtime_fault(error),
        };
    if let Err(error) = call
        .runtime()
        .preflight_graphics_texture_creation(&graphics_plan)
    {
        return call.runtime_fault(error);
    }
    let planned_texture_id = graphics_plan.id();
    let planned_device_id = graphics_plan.device_id();
    let planned_descriptor = graphics_plan.descriptor();

    let mut call = call;
    let (installed, guest_writes) = match call.install_child_com_object_with_guest_writes(
        direct3dtexture9_object_spec(),
        |object| {
            let interface = object
                .interfaces
                .iter()
                .find(|interface| interface.iid == D3D9_IDIRECT3DTEXTURE9_IID)
                .ok_or_else(|| {
                    RuntimeStateError::runtime_defect(
                        "d3d9-create-texture",
                        "idirect3dtexture9-installation-missing",
                        "installed D3D9 texture object has no IDirect3DTexture9 interface",
                    )
                })?;
            Ok(vec![(
                output_address,
                interface.guest_pointer.0.to_le_bytes().to_vec(),
            )])
        },
    ) {
        Ok(result) => result,
        Err(error) => return call.runtime_fault(error),
    };
    let texture = call
        .runtime_mut()
        .commit_graphics_texture_creation(graphics_plan, installed.object.id);
    debug_assert_eq!(texture.id, planned_texture_id);
    debug_assert_eq!(texture.device_id, planned_device_id);
    debug_assert_eq!(texture.descriptor, planned_descriptor);
    call.complete(ComReturnValue::HResult(COM_S_OK), guest_writes)
}

fn dispatch_create_vertex_buffer(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(vertex_buffer_policy) = policy.initial_vertex_buffer.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-create-vertex-buffer",
            "create-vertex-buffer-policy-disabled",
            "D3D9 compatibility policy does not enable CreateVertexBuffer",
        );
    };
    let byte_length = call.frame().explicit_arguments[0];
    let usage = call.frame().explicit_arguments[1];
    let flexible_vertex_format = call.frame().explicit_arguments[2];
    let pool = call.frame().explicit_arguments[3];
    let output_pointer = call.frame().explicit_arguments[4];
    let shared_handle_pointer = call.frame().explicit_arguments[5];
    if byte_length == 0
        || output_pointer == 0
        || (vertex_buffer_policy.shared_handle_must_be_null && shared_handle_pointer != 0)
    {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }
    if byte_length != vertex_buffer_policy.byte_length
        || usage != vertex_buffer_policy.usage
        || flexible_vertex_format != vertex_buffer_policy.flexible_vertex_format
        || pool != vertex_buffer_policy.pool
    {
        return call.complete(
            ComReturnValue::HResult(D3D9_D3DERR_NOTAVAILABLE),
            Vec::new(),
        );
    }

    let mut call = call;
    let result = call.install_graphics_vertex_buffer_with_guest_pointer_write(
        direct3dvertexbuffer9_object_spec(),
        D3D9_IDIRECT3DVERTEXBUFFER9_IID,
        RuntimeGraphicsVertexBufferCreateRequest {
            device_com_object_id: call.frame().object_id,
            descriptor: RuntimeGraphicsVertexBufferDescriptor {
                byte_length,
                usage: RuntimeGraphicsVertexBufferUsage::WriteOnly,
                flexible_vertex_format,
                pool: RuntimeGraphicsVertexBufferPool::Managed,
            },
        },
        GuestAddress(output_pointer),
    );
    let (installed, vertex_buffer, guest_writes) = match result {
        Ok(result) => result,
        Err(error) => return call.runtime_fault(error),
    };
    debug_assert_eq!(vertex_buffer.com_object_id, installed.object.id);
    debug_assert_eq!(vertex_buffer.descriptor.byte_length, byte_length);
    call.complete(ComReturnValue::HResult(COM_S_OK), guest_writes)
}

fn dispatch_set_render_state(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(render_state_policy) = policy.initial_render_states.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-set-render-state",
            "set-render-state-policy-disabled",
            "D3D9 compatibility policy does not enable SetRenderState",
        );
    };
    let state = call.frame().explicit_arguments[0];
    let value = call.frame().explicit_arguments[1];
    if !render_state_policy
        .settings
        .contains(&D3d9RenderStateSetting { state, value })
    {
        return call.complete(
            ComReturnValue::HResult(D3D9_D3DERR_NOTAVAILABLE),
            Vec::new(),
        );
    }
    let update = match (state, value) {
        (D3D9_D3DRS_CULLMODE, D3D9_D3DCULL_NONE) => {
            RuntimeGraphicsRenderStateUpdate::CullMode(RuntimeGraphicsCullMode::None)
        }
        (D3D9_D3DRS_ZENABLE, 0) => RuntimeGraphicsRenderStateUpdate::DepthTestEnabled(false),
        (D3D9_D3DRS_LIGHTING, 0) => RuntimeGraphicsRenderStateUpdate::LightingEnabled(false),
        (D3D9_D3DRS_ALPHABLENDENABLE, 1) => {
            RuntimeGraphicsRenderStateUpdate::AlphaBlendEnabled(true)
        }
        _ => unreachable!("validated D3D9 render-state policy has an unknown setting"),
    };
    let plan = match call.runtime().plan_graphics_render_state_update(
        RuntimeGraphicsRenderStateUpdateRequest {
            device_com_object_id: call.frame().object_id,
            update,
        },
    ) {
        Ok(plan) => plan,
        Err(error) => return call.runtime_fault(error),
    };
    if let Err(error) = call.runtime().preflight_graphics_render_state_update(&plan) {
        return call.runtime_fault(error);
    }
    let mut call = call;
    let result = call.runtime_mut().commit_graphics_render_state_update(plan);
    debug_assert_eq!(result.update, update);
    call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new())
}

fn dispatch_set_sampler_state(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(sampler_state_policy) = policy.initial_sampler_states.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-set-sampler-state",
            "set-sampler-state-policy-disabled",
            "D3D9 compatibility policy does not enable SetSamplerState",
        );
    };
    let sampler = call.frame().explicit_arguments[0];
    let state = call.frame().explicit_arguments[1];
    let value = call.frame().explicit_arguments[2];
    if !sampler_state_policy
        .settings
        .contains(&D3d9SamplerStateSetting {
            sampler,
            state,
            value,
        })
    {
        return call.complete(
            ComReturnValue::HResult(D3D9_D3DERR_NOTAVAILABLE),
            Vec::new(),
        );
    }
    let update = match (state, value) {
        (D3D9_D3DSAMP_MINFILTER, D3D9_D3DTEXF_LINEAR) => {
            RuntimeGraphicsSamplerStateUpdate::MinFilter {
                sampler,
                filter: RuntimeGraphicsTextureFilter::Linear,
            }
        }
        (D3D9_D3DSAMP_MAGFILTER, D3D9_D3DTEXF_LINEAR) => {
            RuntimeGraphicsSamplerStateUpdate::MagFilter {
                sampler,
                filter: RuntimeGraphicsTextureFilter::Linear,
            }
        }
        _ => unreachable!("validated D3D9 sampler-state policy has an unknown setting"),
    };
    let plan = match call.runtime().plan_graphics_sampler_state_update(
        RuntimeGraphicsSamplerStateUpdateRequest {
            device_com_object_id: call.frame().object_id,
            update,
        },
    ) {
        Ok(plan) => plan,
        Err(error) => return call.runtime_fault(error),
    };
    if let Err(error) = call
        .runtime()
        .preflight_graphics_sampler_state_update(&plan)
    {
        return call.runtime_fault(error);
    }
    let mut call = call;
    let result = call
        .runtime_mut()
        .commit_graphics_sampler_state_update(plan);
    debug_assert_eq!(result.update, update);
    call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new())
}

fn dispatch_set_texture(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(binding_policy) = policy.texture_binding.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-set-texture",
            "set-texture-policy-disabled",
            "D3D9 compatibility policy does not enable SetTexture",
        );
    };
    let stage = call.frame().explicit_arguments[0];
    let texture_pointer = call.frame().explicit_arguments[1];
    if stage != binding_policy.stage {
        return call.complete(
            ComReturnValue::HResult(D3D9_D3DERR_NOTAVAILABLE),
            Vec::new(),
        );
    }
    let texture_com_object_id = if texture_pointer == 0 {
        if !binding_policy.null_unbind_supported {
            return call.complete(
                ComReturnValue::HResult(D3D9_D3DERR_NOTAVAILABLE),
                Vec::new(),
            );
        }
        None
    } else {
        let interface = match call
            .runtime()
            .com_interface_for_pointer(GuestAddress(texture_pointer))
        {
            Ok(interface) if interface.iid == D3D9_IDIRECT3DTEXTURE9_IID => interface,
            Ok(_) => {
                return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
            }
            Err(error) if error.class == RuntimeStateErrorClass::NotFound => {
                return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
            }
            Err(error) => return call.runtime_fault(error),
        };
        let texture = match call
            .runtime()
            .graphics_texture_for_com_object(interface.object_id)
        {
            Ok(texture) => texture,
            Err(error) if error.class == RuntimeStateErrorClass::NotFound => {
                return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
            }
            Err(error) => return call.runtime_fault(error),
        };
        let texture_policy = policy
            .create_texture
            .as_ref()
            .expect("validated D3D9 SetTexture policy lost its texture profile");
        let allowed = binding_policy
            .texture_variant_indices
            .iter()
            .filter_map(|index| texture_policy.variants.get(*index as usize))
            .any(|variant| logical_texture_matches_variant(&texture.descriptor, variant));
        if !allowed {
            return call.complete(
                ComReturnValue::HResult(D3D9_D3DERR_NOTAVAILABLE),
                Vec::new(),
            );
        }
        Some(interface.object_id)
    };
    let plan = match call.runtime().plan_graphics_texture_binding_update(
        RuntimeGraphicsTextureBindingUpdateRequest {
            device_com_object_id: call.frame().object_id,
            stage,
            texture_com_object_id,
        },
    ) {
        Ok(plan) => plan,
        Err(error)
            if matches!(
                error.class,
                RuntimeStateErrorClass::InvalidInput | RuntimeStateErrorClass::NotFound
            ) =>
        {
            return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
        }
        Err(error) => return call.runtime_fault(error),
    };
    if let Err(error) = call
        .runtime()
        .preflight_graphics_texture_binding_update(&plan)
    {
        return call.runtime_fault(error);
    }
    let mut call = call;
    let result = call
        .runtime_mut()
        .commit_graphics_texture_binding_update(plan);
    debug_assert_eq!(result.stage, stage);
    call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new())
}

fn dispatch_set_fvf(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(vertex_format_policy) = policy.flexible_vertex_format.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-set-fvf",
            "set-fvf-policy-disabled",
            "D3D9 compatibility policy does not enable SetFVF",
        );
    };
    let value = call.frame().explicit_arguments[0];
    if value != vertex_format_policy.value {
        return call.complete(
            ComReturnValue::HResult(D3D9_D3DERR_NOTAVAILABLE),
            Vec::new(),
        );
    }
    let format = RuntimeGraphicsFlexibleVertexFormat::XyzRhwDiffuseTexture1;
    let plan = match call.runtime().plan_graphics_flexible_vertex_format_update(
        RuntimeGraphicsFlexibleVertexFormatUpdateRequest {
            device_com_object_id: call.frame().object_id,
            format,
        },
    ) {
        Ok(plan) => plan,
        Err(error)
            if matches!(
                error.class,
                RuntimeStateErrorClass::InvalidInput | RuntimeStateErrorClass::NotFound
            ) =>
        {
            return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
        }
        Err(error) => return call.runtime_fault(error),
    };
    if let Err(error) = call
        .runtime()
        .preflight_graphics_flexible_vertex_format_update(&plan)
    {
        return call.runtime_fault(error);
    }
    let mut call = call;
    let result = call
        .runtime_mut()
        .commit_graphics_flexible_vertex_format_update(plan);
    debug_assert_eq!(result.format, format);
    call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new())
}

fn dispatch_set_stream_source(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(stream_source_policy) = policy.stream_source.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-set-stream-source",
            "set-stream-source-policy-disabled",
            "D3D9 compatibility policy does not enable SetStreamSource",
        );
    };
    let stream = call.frame().explicit_arguments[0];
    let vertex_buffer_pointer = call.frame().explicit_arguments[1];
    let offset_in_bytes = call.frame().explicit_arguments[2];
    let stride = call.frame().explicit_arguments[3];
    if stream != stream_source_policy.stream
        || vertex_buffer_pointer == 0
        || offset_in_bytes != stream_source_policy.offset_in_bytes
        || stride != stream_source_policy.stride
    {
        return call.complete(
            ComReturnValue::HResult(D3D9_D3DERR_NOTAVAILABLE),
            Vec::new(),
        );
    }
    let interface = match call
        .runtime()
        .com_interface_for_pointer(GuestAddress(vertex_buffer_pointer))
    {
        Ok(interface) if interface.iid == D3D9_IDIRECT3DVERTEXBUFFER9_IID => interface,
        Ok(_) => {
            return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
        }
        Err(error) if error.class == RuntimeStateErrorClass::NotFound => {
            return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
        }
        Err(error) => return call.runtime_fault(error),
    };
    let plan = match call.runtime().plan_graphics_stream_source_update(
        RuntimeGraphicsStreamSourceUpdateRequest {
            device_com_object_id: call.frame().object_id,
            stream,
            vertex_buffer_com_object_id: interface.object_id,
            offset_in_bytes,
            stride,
        },
    ) {
        Ok(plan) => plan,
        Err(error)
            if matches!(
                error.class,
                RuntimeStateErrorClass::InvalidInput | RuntimeStateErrorClass::NotFound
            ) =>
        {
            return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
        }
        Err(error) => return call.runtime_fault(error),
    };
    if let Err(error) = call
        .runtime()
        .preflight_graphics_stream_source_update(&plan)
    {
        return call.runtime_fault(error);
    }
    let mut call = call;
    let result = call
        .runtime_mut()
        .commit_graphics_stream_source_update(plan);
    debug_assert_eq!(result.stream, stream);
    debug_assert_eq!(result.binding.offset_in_bytes, offset_in_bytes);
    debug_assert_eq!(result.binding.stride, stride);
    call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new())
}

fn dispatch_draw_primitive(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(draw_policy) = policy.draw_primitive.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-draw-primitive",
            "draw-primitive-policy-disabled",
            "D3D9 compatibility policy does not enable DrawPrimitive",
        );
    };
    let primitive_type = call.frame().explicit_arguments[0];
    let start_vertex = call.frame().explicit_arguments[1];
    let primitive_count = call.frame().explicit_arguments[2];
    if primitive_type != draw_policy.primitive_type
        || start_vertex != draw_policy.start_vertex
        || primitive_count != draw_policy.primitive_count
    {
        return call.complete(
            ComReturnValue::HResult(D3D9_D3DERR_NOTAVAILABLE),
            Vec::new(),
        );
    }
    let capture =
        match call
            .runtime()
            .plan_graphics_draw_capture(RuntimeGraphicsDrawCaptureRequest {
                device_com_object_id: call.frame().object_id,
                primitive_topology: RuntimeGraphicsPrimitiveTopology::TriangleStrip,
                start_vertex,
                primitive_count,
            }) {
            Ok(capture) => capture,
            Err(error)
                if matches!(
                    error.class,
                    RuntimeStateErrorClass::InvalidInput | RuntimeStateErrorClass::NotFound
                ) =>
            {
                return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
            }
            Err(error) => return call.runtime_fault(error),
        };
    let vertex_range = capture.vertex_range();
    let vertex_byte_length = match usize::try_from(vertex_range.byte_length()) {
        Ok(byte_length) => byte_length,
        Err(_) => {
            return call.fault(
                ComFaultClass::RuntimeDefect,
                "d3d9-draw-primitive",
                "draw-primitive-vertex-byte-length-invalid",
                "planned D3D9 draw vertex byte length does not fit host accounting",
            );
        }
    };
    let vertex_data = match call.memory().read(vertex_range.start(), vertex_byte_length) {
        Ok(bytes) => bytes,
        Err(error) => return call.memory_fault(error),
    };
    let plan = match call
        .runtime()
        .prepare_graphics_draw_submission(capture, vertex_data)
    {
        Ok(plan) => plan,
        Err(error) if error.class == RuntimeStateErrorClass::InvalidInput => {
            return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
        }
        Err(error) => return call.runtime_fault(error),
    };
    if let Err(error) = call.runtime().preflight_graphics_draw_submission(&plan) {
        return call.runtime_fault(error);
    }
    let mut call = call;
    let result = call.runtime_mut().commit_graphics_draw_submission(plan);
    debug_assert_eq!(
        result.submission.primitive_topology,
        RuntimeGraphicsPrimitiveTopology::TriangleStrip
    );
    debug_assert_eq!(result.submission.start_vertex, start_vertex);
    debug_assert_eq!(result.submission.primitive_count, primitive_count);
    call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new())
}

fn dispatch_set_texture_stage_state(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(texture_stage_state_policy) = policy.initial_texture_stage_states.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-set-texture-stage-state",
            "set-texture-stage-state-policy-disabled",
            "D3D9 compatibility policy does not enable SetTextureStageState",
        );
    };
    let stage = call.frame().explicit_arguments[0];
    let state = call.frame().explicit_arguments[1];
    let value = call.frame().explicit_arguments[2];
    if !texture_stage_state_policy
        .settings
        .contains(&D3d9TextureStageStateSetting {
            stage,
            state,
            value,
        })
    {
        return call.complete(
            ComReturnValue::HResult(D3D9_D3DERR_NOTAVAILABLE),
            Vec::new(),
        );
    }
    let update = match (state, value) {
        (D3D9_D3DTSS_COLOROP, D3D9_D3DTOP_MODULATE) => {
            RuntimeGraphicsTextureStageStateUpdate::ColorOperation {
                stage,
                operation: RuntimeGraphicsTextureOperation::Modulate,
            }
        }
        (D3D9_D3DTSS_COLORARG1, D3D9_D3DTA_TEXTURE) => {
            RuntimeGraphicsTextureStageStateUpdate::ColorArgumentOne {
                stage,
                argument: RuntimeGraphicsTextureArgument::Texture,
            }
        }
        (D3D9_D3DTSS_COLORARG2, D3D9_D3DTA_DIFFUSE) => {
            RuntimeGraphicsTextureStageStateUpdate::ColorArgumentTwo {
                stage,
                argument: RuntimeGraphicsTextureArgument::Diffuse,
            }
        }
        (D3D9_D3DTSS_ALPHAOP, D3D9_D3DTOP_SELECTARG1) => {
            RuntimeGraphicsTextureStageStateUpdate::AlphaOperation {
                stage,
                operation: RuntimeGraphicsTextureOperation::SelectArgumentOne,
            }
        }
        (D3D9_D3DTSS_ALPHAARG1, D3D9_D3DTA_TEXTURE) => {
            RuntimeGraphicsTextureStageStateUpdate::AlphaArgumentOne {
                stage,
                argument: RuntimeGraphicsTextureArgument::Texture,
            }
        }
        (D3D9_D3DTSS_ALPHAARG2, D3D9_D3DTA_DIFFUSE) => {
            RuntimeGraphicsTextureStageStateUpdate::AlphaArgumentTwo {
                stage,
                argument: RuntimeGraphicsTextureArgument::Diffuse,
            }
        }
        _ => unreachable!("validated D3D9 texture-stage-state policy has an unknown setting"),
    };
    let plan = match call.runtime().plan_graphics_texture_stage_state_update(
        RuntimeGraphicsTextureStageStateUpdateRequest {
            device_com_object_id: call.frame().object_id,
            update,
        },
    ) {
        Ok(plan) => plan,
        Err(error) => return call.runtime_fault(error),
    };
    if let Err(error) = call
        .runtime()
        .preflight_graphics_texture_stage_state_update(&plan)
    {
        return call.runtime_fault(error);
    }
    let mut call = call;
    let result = call
        .runtime_mut()
        .commit_graphics_texture_stage_state_update(plan);
    debug_assert_eq!(result.update, update);
    call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new())
}

fn dispatch_vertex_buffer_lock(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(vertex_buffer_policy) = policy.initial_vertex_buffer.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-vertex-buffer-lock",
            "vertex-buffer-lock-policy-disabled",
            "D3D9 compatibility policy does not enable vertex-buffer Lock",
        );
    };
    let offset = call.frame().explicit_arguments[0];
    let byte_length = call.frame().explicit_arguments[1];
    let output_pointer = call.frame().explicit_arguments[2];
    let flags = call.frame().explicit_arguments[3];
    if output_pointer == 0 {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }
    if offset != vertex_buffer_policy.lock_offset
        || byte_length != vertex_buffer_policy.lock_byte_length
        || flags != vertex_buffer_policy.lock_flags
    {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }
    let mut call = call;
    let result = call.lock_graphics_vertex_buffer_with_guest_pointer_write(
        RuntimeGraphicsVertexBufferLockRequest {
            vertex_buffer_com_object_id: call.frame().object_id,
            offset,
            byte_length,
            flags,
        },
        GuestAddress(output_pointer),
    );
    match result {
        Ok((lock, guest_writes)) => {
            debug_assert_eq!(lock.byte_length, vertex_buffer_policy.byte_length);
            call.complete(ComReturnValue::HResult(COM_S_OK), guest_writes)
        }
        Err(error)
            if matches!(
                error.code,
                "graphics-vertex-buffer-already-locked"
                    | "graphics-vertex-buffer-lock-variant-unsupported"
            ) =>
        {
            call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new())
        }
        Err(error) => call.runtime_fault(error),
    }
}

fn dispatch_vertex_buffer_unlock(
    policy: &D3d9CompatibilityPolicy,
    mut call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    if policy.initial_vertex_buffer.is_none() {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-vertex-buffer-unlock",
            "vertex-buffer-unlock-policy-disabled",
            "D3D9 compatibility policy does not enable vertex-buffer Unlock",
        );
    }
    match call.unlock_graphics_vertex_buffer(call.frame().object_id) {
        Ok(result) => {
            debug_assert_ne!(result.content_generation, 0);
            call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new())
        }
        Err(error) if error.code == "graphics-vertex-buffer-not-locked" => {
            call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new())
        }
        Err(error) => call.runtime_fault(error),
    }
}

fn dispatch_texture_lock_rect(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(lock_policy) = policy.texture_lock_rect.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-texture-lock-rect",
            "texture-lock-rect-policy-disabled",
            "D3D9 compatibility policy does not enable texture LockRect",
        );
    };
    let level = call.frame().explicit_arguments[0];
    let output_pointer = call.frame().explicit_arguments[1];
    let rect_pointer = call.frame().explicit_arguments[2];
    let flags = call.frame().explicit_arguments[3];
    if output_pointer == 0
        || rect_pointer == 0
        || level != lock_policy.level
        || flags != lock_policy.flags
    {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }
    let texture = match call
        .runtime()
        .graphics_texture_for_com_object(call.frame().object_id)
    {
        Ok(texture) => texture,
        Err(error) => return call.runtime_fault(error),
    };
    let allowed = policy
        .create_texture
        .as_ref()
        .map(|texture_policy| {
            lock_policy
                .source_texture_variant_indices
                .iter()
                .filter_map(|index| texture_policy.variants.get(*index as usize))
                .any(|variant| logical_texture_matches_variant(&texture.descriptor, variant))
        })
        .unwrap_or(false);
    if !allowed {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-texture-lock-rect",
            "texture-lock-rect-texture-variant-unsupported",
            "D3D9 compatibility policy does not enable LockRect for this texture variant",
        );
    }
    let rect_address = GuestAddress(rect_pointer);
    if let Err(error) = call
        .memory()
        .validate_access(rect_address, 16, MemoryAccess::Read)
    {
        return call.memory_fault(error);
    }
    let rect_bytes = match call.memory().read(rect_address, 16) {
        Ok(bytes) => bytes,
        Err(error) => return call.memory_fault(error),
    };
    let rect = RuntimeRect {
        left: i32::from_le_bytes(rect_bytes[0..4].try_into().expect("validated RECT left")),
        top: i32::from_le_bytes(rect_bytes[4..8].try_into().expect("validated RECT top")),
        right: i32::from_le_bytes(rect_bytes[8..12].try_into().expect("validated RECT right")),
        bottom: i32::from_le_bytes(
            rect_bytes[12..16]
                .try_into()
                .expect("validated RECT bottom"),
        ),
    };
    if rect != lock_policy.rect {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }
    let mut call = call;
    match call.lock_graphics_texture_with_guest_locked_rect_write(
        RuntimeGraphicsTextureLockRequest {
            texture_com_object_id: call.frame().object_id,
            level,
            rect,
            flags,
        },
        GuestAddress(output_pointer),
    ) {
        Ok((lock, guest_writes)) => {
            debug_assert_eq!(lock.level, level);
            debug_assert_eq!(lock.rect, rect);
            debug_assert_eq!(lock.flags, flags);
            call.complete(ComReturnValue::HResult(COM_S_OK), guest_writes)
        }
        Err(error)
            if matches!(
                error.code,
                "graphics-texture-already-locked"
                    | "graphics-texture-lock-variant-unsupported"
                    | "graphics-texture-dirty-rect-out-of-bounds"
            ) =>
        {
            call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new())
        }
        Err(error) => call.runtime_fault(error),
    }
}

fn dispatch_texture_unlock_rect(
    policy: &D3d9CompatibilityPolicy,
    mut call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(lock_policy) = policy.texture_lock_rect.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-texture-unlock-rect",
            "texture-unlock-rect-policy-disabled",
            "D3D9 compatibility policy does not enable texture UnlockRect",
        );
    };
    let level = call.frame().explicit_arguments[0];
    if level != lock_policy.level {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }
    match call.unlock_graphics_texture(call.frame().object_id) {
        Ok(result) => {
            debug_assert_ne!(result.content_generation, 0);
            call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new())
        }
        Err(error) if error.code == "graphics-texture-not-locked" => {
            call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new())
        }
        Err(error) => call.runtime_fault(error),
    }
}

fn dispatch_get_surface_level(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(surface_policy) = policy.get_surface_level.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-get-surface-level",
            "get-surface-level-policy-disabled",
            "D3D9 compatibility policy does not enable GetSurfaceLevel",
        );
    };
    let level = call.frame().explicit_arguments[0];
    let output_pointer = call.frame().explicit_arguments[1];
    if output_pointer == 0 || level != surface_policy.level {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }

    let source_variants = policy
        .create_texture
        .as_ref()
        .map(|texture| {
            surface_policy
                .source_texture_variant_indices
                .iter()
                .map(|index| {
                    texture
                        .variants
                        .get(*index as usize)
                        .expect("validated D3D9 surface policy lost a source texture variant")
                })
                .collect::<Vec<_>>()
        })
        .expect("validated D3D9 surface policy lost its texture profile");
    let texture = match call
        .runtime()
        .graphics_texture_for_com_object(call.frame().object_id)
    {
        Ok(texture) => texture,
        Err(error) => return call.runtime_fault(error),
    };
    if !source_variants
        .iter()
        .any(|variant| logical_texture_matches_variant(&texture.descriptor, variant))
    {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-get-surface-level",
            "get-surface-level-texture-variant-unsupported",
            "D3D9 compatibility policy does not enable GetSurfaceLevel for this texture variant",
        );
    }

    let output_address = GuestAddress(output_pointer);
    if let Err(error) = call
        .memory()
        .validate_access(output_address, 4, MemoryAccess::Write)
    {
        return call.memory_fault(error);
    }
    let existing_surface = match call
        .runtime()
        .graphics_surface_for_texture_level(call.frame().object_id, level)
    {
        Ok(surface) => surface.cloned(),
        Err(error) if error.class == RuntimeStateErrorClass::InvalidInput => {
            return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
        }
        Err(error) => return call.runtime_fault(error),
    };
    if let Some(surface) = existing_surface {
        let interface = match call
            .runtime()
            .com_interface_for_object_iid(surface.com_object_id, D3D9_IDIRECT3DSURFACE9_IID)
        {
            Ok(interface) => interface.clone(),
            Err(error) => return call.runtime_fault(error),
        };
        let mut call = call;
        let (_, guest_writes) =
            match call.add_ref_interface_with_guest_pointer_write(interface.id, output_address) {
                Ok(result) => result,
                Err(error) => return call.runtime_fault(error),
            };
        return call.complete(ComReturnValue::HResult(COM_S_OK), guest_writes);
    }

    let graphics_plan =
        match call
            .runtime()
            .plan_graphics_surface_creation(RuntimeGraphicsSurfaceCreateRequest {
                texture_com_object_id: call.frame().object_id,
                level,
            }) {
            Ok(plan) => plan,
            Err(error) if error.class == RuntimeStateErrorClass::InvalidInput => {
                return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
            }
            Err(error) => return call.runtime_fault(error),
        };
    if let Err(error) = call
        .runtime()
        .preflight_graphics_surface_creation(&graphics_plan)
    {
        return call.runtime_fault(error);
    }
    let planned_surface_id = graphics_plan.id();
    let planned_texture_id = graphics_plan.texture_id();
    let planned_descriptor = graphics_plan.descriptor();

    let mut call = call;
    let (installed, guest_writes) = match call.install_child_com_object_with_guest_writes(
        direct3dsurface9_object_spec(),
        |object| {
            let interface = object
                .interfaces
                .iter()
                .find(|interface| interface.iid == D3D9_IDIRECT3DSURFACE9_IID)
                .ok_or_else(|| {
                    RuntimeStateError::runtime_defect(
                        "d3d9-get-surface-level",
                        "idirect3dsurface9-installation-missing",
                        "installed D3D9 surface object has no IDirect3DSurface9 interface",
                    )
                })?;
            Ok(vec![(
                output_address,
                interface.guest_pointer.0.to_le_bytes().to_vec(),
            )])
        },
    ) {
        Ok(result) => result,
        Err(error) => return call.runtime_fault(error),
    };
    let surface = call
        .runtime_mut()
        .commit_graphics_surface_creation(graphics_plan, installed.object.id);
    debug_assert_eq!(surface.id, planned_surface_id);
    debug_assert_eq!(surface.texture_id, planned_texture_id);
    debug_assert_eq!(surface.descriptor, planned_descriptor);
    call.complete(ComReturnValue::HResult(COM_S_OK), guest_writes)
}

fn dispatch_update_texture(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(update_policy) = policy.update_texture.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-update-texture",
            "update-texture-policy-disabled",
            "D3D9 compatibility policy does not enable UpdateTexture",
        );
    };
    let source_pointer = call.frame().explicit_arguments[0];
    let destination_pointer = call.frame().explicit_arguments[1];
    if source_pointer == 0 || destination_pointer == 0 || source_pointer == destination_pointer {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }
    let source_com_object_id = match call
        .runtime()
        .com_interface_for_pointer(GuestAddress(source_pointer))
    {
        Ok(interface) if interface.iid == D3D9_IDIRECT3DTEXTURE9_IID => interface.object_id,
        Ok(_) => {
            return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
        }
        Err(error) if error.class == RuntimeStateErrorClass::NotFound => {
            return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
        }
        Err(error) => return call.runtime_fault(error),
    };
    let destination_com_object_id = match call
        .runtime()
        .com_interface_for_pointer(GuestAddress(destination_pointer))
    {
        Ok(interface) if interface.iid == D3D9_IDIRECT3DTEXTURE9_IID => interface.object_id,
        Ok(_) => {
            return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
        }
        Err(error) if error.class == RuntimeStateErrorClass::NotFound => {
            return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
        }
        Err(error) => return call.runtime_fault(error),
    };
    if source_com_object_id == destination_com_object_id {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }
    let source = match call
        .runtime()
        .graphics_texture_for_com_object(source_com_object_id)
    {
        Ok(texture) => texture.clone(),
        Err(error) if error.class == RuntimeStateErrorClass::NotFound => {
            return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
        }
        Err(error) => return call.runtime_fault(error),
    };
    let destination = match call
        .runtime()
        .graphics_texture_for_com_object(destination_com_object_id)
    {
        Ok(texture) => texture.clone(),
        Err(error) if error.class == RuntimeStateErrorClass::NotFound => {
            return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
        }
        Err(error) => return call.runtime_fault(error),
    };
    let texture_policy = policy
        .create_texture
        .as_ref()
        .expect("validated D3D9 UpdateTexture policy lost its texture profile");
    let source_allowed = update_policy
        .source_texture_variant_indices
        .iter()
        .filter_map(|index| texture_policy.variants.get(*index as usize))
        .any(|variant| logical_texture_matches_variant(&source.descriptor, variant));
    if !source_allowed {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }
    let destination_allowed = update_policy
        .destination_texture_variant_indices
        .iter()
        .filter_map(|index| texture_policy.variants.get(*index as usize))
        .any(|variant| logical_texture_matches_variant(&destination.descriptor, variant));
    if !destination_allowed {
        let compatible_default_variant = texture_policy.variants.iter().any(|variant| {
            variant.pool == D3D9_D3DPOOL_DEFAULT
                && variant.width == source.descriptor.width
                && variant.height == source.descriptor.height
                && variant.levels == source.descriptor.levels
                && variant.format == D3D9_D3DFMT_X8R8G8B8
                && logical_texture_matches_variant(&destination.descriptor, variant)
        });
        if compatible_default_variant {
            return call.fault(
                ComFaultClass::Unsupported,
                "d3d9-update-texture",
                "update-texture-destination-variant-unsupported",
                "D3D9 compatibility policy does not enable the reached UpdateTexture call for this DEFAULT destination variant",
            );
        }
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }
    if matches!(source.dirty_region, RuntimeGraphicsTextureDirtyRegion::Rect(rect)
        if rect != update_policy.partial_dirty_rect
            && !update_policy.allow_bounded_partial_dirty_rects)
    {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-update-texture",
            "update-texture-partial-dirty-region-variant-unsupported",
            "D3D9 compatibility policy does not enable this partial dirty-region variant",
        );
    }
    let plan =
        match call
            .runtime()
            .plan_graphics_texture_update(RuntimeGraphicsTextureUpdateRequest {
                device_com_object_id: call.frame().object_id,
                source_texture_com_object_id: source_com_object_id,
                destination_texture_com_object_id: destination_com_object_id,
            }) {
            Ok(plan) => plan,
            Err(error)
                if matches!(
                    error.class,
                    RuntimeStateErrorClass::InvalidInput | RuntimeStateErrorClass::NotFound
                ) =>
            {
                return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
            }
            Err(error) => return call.runtime_fault(error),
        };
    if let Err(error) = call.runtime().preflight_graphics_texture_update(&plan) {
        return call.runtime_fault(error);
    }
    let mut call = call;
    call.runtime_mut().commit_graphics_texture_update(plan);
    call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new())
}

fn dispatch_add_dirty_rect(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(add_dirty_rect_policy) = policy.add_dirty_rect.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-add-dirty-rect",
            "add-dirty-rect-policy-disabled",
            "D3D9 compatibility policy does not enable AddDirtyRect",
        );
    };
    let source_variants = policy
        .create_texture
        .as_ref()
        .map(|texture| {
            add_dirty_rect_policy
                .source_texture_variant_indices
                .iter()
                .map(|index| {
                    texture
                        .variants
                        .get(*index as usize)
                        .expect("validated D3D9 AddDirtyRect policy lost a source texture variant")
                })
                .collect::<Vec<_>>()
        })
        .expect("validated D3D9 AddDirtyRect policy lost its texture profile");
    let texture = match call
        .runtime()
        .graphics_texture_for_com_object(call.frame().object_id)
    {
        Ok(texture) => texture,
        Err(error) => return call.runtime_fault(error),
    };
    if !source_variants
        .iter()
        .any(|variant| logical_texture_matches_variant(&texture.descriptor, variant))
    {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-add-dirty-rect",
            "add-dirty-rect-texture-variant-unsupported",
            "D3D9 compatibility policy does not enable AddDirtyRect for this texture variant",
        );
    }

    let rect_pointer = call.frame().explicit_arguments[0];
    let rect = if rect_pointer == 0 {
        None
    } else {
        let address = GuestAddress(rect_pointer);
        if let Err(error) = call
            .memory()
            .validate_access(address, 16, MemoryAccess::Read)
        {
            return call.memory_fault(error);
        }
        let bytes = match call.memory().read(address, 16) {
            Ok(bytes) => bytes,
            Err(error) => return call.memory_fault(error),
        };
        Some(RuntimeRect {
            left: i32::from_le_bytes(bytes[0..4].try_into().expect("validated RECT left")),
            top: i32::from_le_bytes(bytes[4..8].try_into().expect("validated RECT top")),
            right: i32::from_le_bytes(bytes[8..12].try_into().expect("validated RECT right")),
            bottom: i32::from_le_bytes(bytes[12..16].try_into().expect("validated RECT bottom")),
        })
    };
    let plan = match call.runtime().plan_graphics_texture_dirty_region_mark(
        RuntimeGraphicsTextureDirtyRegionMarkRequest {
            texture_com_object_id: call.frame().object_id,
            rect,
        },
    ) {
        Ok(plan) => plan,
        Err(error) if error.class == RuntimeStateErrorClass::InvalidInput => {
            return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
        }
        Err(error) => return call.runtime_fault(error),
    };
    if let Err(error) = call
        .runtime()
        .preflight_graphics_texture_dirty_region_mark(&plan)
    {
        return call.runtime_fault(error);
    }
    let mut call = call;
    call.runtime_mut()
        .commit_graphics_texture_dirty_region_mark(plan);
    call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new())
}

fn logical_texture_matches_variant(
    descriptor: &RuntimeGraphicsTextureDescriptor,
    variant: &D3d9CreateTextureVariant,
) -> bool {
    descriptor.width == variant.width
        && descriptor.height == variant.height
        && descriptor.levels == variant.levels
        && descriptor.color_format == RuntimeGraphicsColorFormat::B8g8r8x8Unorm
        && matches!(
            (descriptor.usage, variant.usage),
            (RuntimeGraphicsTextureUsage::None, 0)
                | (RuntimeGraphicsTextureUsage::Dynamic, D3D9_D3DUSAGE_DYNAMIC)
        )
        && matches!(
            (descriptor.pool, variant.pool),
            (RuntimeGraphicsTexturePool::Default, D3D9_D3DPOOL_DEFAULT)
                | (
                    RuntimeGraphicsTexturePool::SystemMemory,
                    D3D9_D3DPOOL_SYSTEMMEM
                )
        )
        && variant.format == D3D9_D3DFMT_X8R8G8B8
}

fn dispatch_surface_get_dc(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(_) = policy.surface_dc.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-surface-get-dc",
            "surface-dc-policy-disabled",
            "D3D9 compatibility policy does not enable surface GetDC or ReleaseDC",
        );
    };
    let output_pointer = call.frame().explicit_arguments[0];
    if output_pointer == 0 {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }
    let output_address = GuestAddress(output_pointer);
    if let Err(error) = call
        .memory()
        .validate_access(output_address, 4, MemoryAccess::Write)
    {
        return call.memory_fault(error);
    }
    let plan = match call
        .runtime()
        .plan_graphics_surface_dc_acquisition(call.frame().object_id)
    {
        Ok(plan) => plan,
        Err(error) if error.code == "graphics-surface-dc-already-acquired" => {
            return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
        }
        Err(error) => return call.runtime_fault(error),
    };

    let mut call = call;
    let (_, guest_writes) =
        match call.acquire_graphics_surface_dc_with_guest_handle_write(plan, output_address) {
            Ok(result) => result,
            Err(error) => return call.runtime_fault(error),
        };
    call.complete(ComReturnValue::HResult(COM_S_OK), guest_writes)
}

fn dispatch_surface_release_dc(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(_) = policy.surface_dc.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-surface-release-dc",
            "surface-dc-policy-disabled",
            "D3D9 compatibility policy does not enable surface GetDC or ReleaseDC",
        );
    };
    let device_context_handle = call.frame().explicit_arguments[0];
    let surface_com_object_id = call.frame().object_id;
    let mut call = call;
    match call
        .runtime_mut()
        .release_graphics_surface_dc(surface_com_object_id, device_context_handle)
    {
        Ok(()) => call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new()),
        Err(error)
            if matches!(
                error.code,
                "device-context-handle-unknown" | "graphics-surface-dc-relation-mismatch"
            ) =>
        {
            call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new())
        }
        Err(error) => call.runtime_fault(error),
    }
}

fn dispatch_check_device_format(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(check_policy) = policy.check_device_format.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-device-format",
            "check-device-format-policy-disabled",
            "D3D9 compatibility policy does not enable CheckDeviceFormat",
        );
    };
    let adapter = call.frame().explicit_arguments[0];
    let device_type = call.frame().explicit_arguments[1];
    let adapter_format = call.frame().explicit_arguments[2];
    let usage = call.frame().explicit_arguments[3];
    let resource_type = call.frame().explicit_arguments[4];
    let check_format = call.frame().explicit_arguments[5];
    if adapter != check_policy.adapter || device_type != check_policy.device_type {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }
    if let Some((code, message)) =
        primary_display_authority_fault(&call, &check_policy.display_profile_id)
    {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "d3d9-device-format",
            code,
            message,
        );
    }

    let result = if adapter_format == check_policy.adapter_format
        && resource_type == check_policy.resource_type
        && check_format == check_policy.check_format
        && check_policy.supported_usages.contains(&usage)
    {
        COM_S_OK
    } else {
        D3D9_D3DERR_NOTAVAILABLE
    };
    call.complete(ComReturnValue::HResult(result), Vec::new())
}

fn dispatch_check_device_type(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(check_policy) = policy.check_device_type.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-device-type",
            "check-device-type-policy-disabled",
            "D3D9 compatibility policy does not enable CheckDeviceType",
        );
    };
    let adapter = call.frame().explicit_arguments[0];
    let device_type = call.frame().explicit_arguments[1];
    let adapter_format = call.frame().explicit_arguments[2];
    let back_buffer_format = call.frame().explicit_arguments[3];
    let windowed = call.frame().explicit_arguments[4] != 0;
    if adapter != check_policy.adapter || device_type != check_policy.device_type {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }
    if let Some((code, message)) =
        primary_display_authority_fault(&call, &check_policy.display_profile_id)
    {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "d3d9-device-type",
            code,
            message,
        );
    }

    let mode_supported = if windowed {
        check_policy.windowed_supported
    } else {
        check_policy.fullscreen_supported
    };
    let result = if adapter_format == check_policy.adapter_format
        && back_buffer_format == check_policy.back_buffer_format
        && mode_supported
    {
        COM_S_OK
    } else {
        D3D9_D3DERR_NOTAVAILABLE
    };
    call.complete(ComReturnValue::HResult(result), Vec::new())
}

fn primary_display_authority_fault(
    call: &ComFrontendCall<'_>,
    expected_profile_id: &str,
) -> Option<(&'static str, &'static str)> {
    let Some(monitor) = call.runtime().primary_monitor() else {
        return Some((
            "logical-primary-monitor-missing",
            "D3D9 compatibility policy is enabled but RuntimeDisplays has no primary monitor",
        ));
    };
    if call.runtime().display_profile_id() != Some(expected_profile_id) || !monitor.primary {
        return Some((
            "logical-display-profile-mismatch",
            "Rust display authority does not match the selected D3D9 compatibility profile",
        ));
    }
    None
}

fn dispatch_get_device_caps(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(caps_policy) = policy.get_device_caps.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-device-caps",
            "get-device-caps-policy-disabled",
            "D3D9 compatibility policy does not enable GetDeviceCaps",
        );
    };
    let adapter = call.frame().explicit_arguments[0];
    let device_type = call.frame().explicit_arguments[1];
    let output_pointer = call.frame().explicit_arguments[2];
    if adapter != caps_policy.adapter
        || device_type != caps_policy.device_type
        || output_pointer == 0
    {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }

    if let Some((code, message)) =
        primary_display_authority_fault(&call, &caps_policy.display_profile_id)
    {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "d3d9-device-caps",
            code,
            message,
        );
    }

    let bytes = D3d9DeviceCaps::from_policy(caps_policy).to_x86_bytes();
    debug_assert_eq!(bytes.len(), D3D9_D3DCAPS9_BYTE_LENGTH as usize);
    complete_d3d9_write(call, GuestAddress(output_pointer), bytes)
}

fn dispatch_get_adapter_identifier(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(identifier_policy) = policy.get_adapter_identifier.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-adapter-identifier",
            "get-adapter-identifier-policy-disabled",
            "D3D9 compatibility policy does not enable GetAdapterIdentifier",
        );
    };
    let adapter = call.frame().explicit_arguments[0];
    let flags = call.frame().explicit_arguments[1];
    let output_pointer = call.frame().explicit_arguments[2];
    if flags == D3D9_D3DENUM_WHQL_LEVEL {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-adapter-identifier",
            "d3denum-whql-level-unsupported",
            "D3DENUM_WHQL_LEVEL may perform host certificate discovery and is outside the bounded adapter profile",
        );
    }
    if adapter != identifier_policy.adapter
        || flags != identifier_policy.flags
        || output_pointer == 0
    {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }

    if let Some((code, message)) =
        primary_display_authority_fault(&call, &identifier_policy.display_profile_id)
    {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "d3d9-adapter-identifier",
            code,
            message,
        );
    }

    let identifier = D3d9AdapterIdentifier::from_policy(identifier_policy);
    let bytes = identifier.to_x86_bytes();
    debug_assert_eq!(
        bytes.len(),
        D3D9_D3DADAPTER_IDENTIFIER9_BYTE_LENGTH as usize
    );
    complete_d3d9_write(call, GuestAddress(output_pointer), bytes)
}

fn dispatch_get_adapter_display_mode(
    policy: &D3d9CompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let Some(mode_policy) = policy.get_adapter_display_mode.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "d3d9-display",
            "get-adapter-display-mode-policy-disabled",
            "D3D9 compatibility policy does not enable GetAdapterDisplayMode",
        );
    };
    let adapter = call.frame().explicit_arguments[0];
    let output_pointer = call.frame().explicit_arguments[1];
    if adapter != mode_policy.adapter || output_pointer == 0 {
        return call.complete(ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL), Vec::new());
    }

    if let Some((code, message)) =
        primary_display_authority_fault(&call, &mode_policy.display_profile_id)
    {
        return call.fault(ComFaultClass::RuntimeDefect, "d3d9-display", code, message);
    }
    let monitor = call
        .runtime()
        .primary_monitor()
        .expect("display authority preflight requires a primary monitor");
    let width = monitor
        .rect
        .right
        .checked_sub(monitor.rect.left)
        .and_then(|value| u32::try_from(value).ok());
    let height = monitor
        .rect
        .bottom
        .checked_sub(monitor.rect.top)
        .and_then(|value| u32::try_from(value).ok());
    let (Some(width), Some(height)) = (width, height) else {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "d3d9-display",
            "logical-display-geometry-invalid",
            "validated RuntimeDisplays geometry cannot be represented as D3DDISPLAYMODE",
        );
    };
    let mode = D3d9DisplayMode {
        width,
        height,
        refresh_rate: mode_policy.refresh_rate,
        format: mode_policy.format,
    };
    let bytes = mode.to_x86_bytes();
    debug_assert_eq!(bytes.len(), D3D9_D3DDISPLAYMODE_BYTE_LENGTH as usize);
    complete_d3d9_write(call, GuestAddress(output_pointer), bytes)
}

fn complete_d3d9_write(
    call: ComFrontendCall<'_>,
    output: GuestAddress,
    bytes: Vec<u8>,
) -> ComDispatchOutcome {
    if let Err(error) = call
        .memory()
        .validate_access(output, bytes.len(), MemoryAccess::Write)
    {
        return call.memory_fault(error);
    }

    let mut call = call;
    let mut transaction = call.memory_mut().begin_transaction();
    let write_result = match transaction.write(output, &bytes) {
        Ok(result) => result,
        Err(error) => {
            drop(transaction);
            return call.memory_fault(error);
        }
    };
    transaction.commit_transaction();
    call.complete(
        ComReturnValue::HResult(COM_S_OK),
        vec![ComGuestWriteReceipt {
            address: output,
            bytes,
            result: write_result,
        }],
    )
}

fn handle_direct3d_create9(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    let Some(sdk_version) = frame.argument_u32(0) else {
        return ApiHandlerResult::Fault(ApiFault::runtime_defect(
            "d3d9-api-dispatch",
            "decoded-argument-missing",
            "decoded Direct3DCreate9 frame is missing its SDK version",
        ));
    };
    if sdk_version != D3D9_SDK_VERSION {
        return ApiHandlerResult::Immediate(ApiImmediate::new(ApiReturnValue::OpaquePointer(0)));
    }

    let installed = match session.install_com_object(direct3d9_object_spec()) {
        Ok(installed) => installed,
        Err(error) => {
            return ApiHandlerResult::Fault(ApiFault::from_runtime(error, &contract.endpoint));
        }
    };
    let Some(interface) = installed
        .interfaces
        .iter()
        .find(|interface| interface.iid == D3D9_IDIRECT3D9_IID)
    else {
        return ApiHandlerResult::Fault(ApiFault::runtime_defect(
            "d3d9-api-dispatch",
            "idirect3d9-installation-missing",
            "installed D3D9 root object has no IDirect3D9 interface",
        ));
    };
    ApiHandlerResult::Immediate(ApiImmediate::new(ApiReturnValue::OpaquePointer(
        interface.guest_pointer.0,
    )))
}

pub fn d3d9_contract_for_endpoint(
    endpoint: ApiEndpointIdentity,
) -> Result<Option<ApiContract>, ApiFault> {
    if !endpoint.module.eq_ignore_ascii_case("d3d9.dll") {
        return Ok(None);
    }
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return Ok(None);
    };
    match name.as_str() {
        "Direct3DCreate9" => Ok(Some(direct3d_create9_contract(endpoint)?)),
        _ => Ok(None),
    }
}

pub fn direct3d_create9_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![ApiArgumentSchema {
                name: "sdkVersion".to_owned(),
                direction: ApiDirection::In,
                value: ApiValueSchema::U32,
            }],
            return_value: ApiReturnSchema::OpaquePointer { nullable: true },
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Disallowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: D3D9_DIRECT3D_CREATE9_IMPLEMENTATION.to_owned(),
    })
}

pub fn direct3d9_object_spec() -> ComObjectSpec {
    ComObjectSpec {
        class_identity: D3D9_IDIRECT3D9_CLASS_IDENTITY.to_owned(),
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            ComInterfaceSpec {
                iid: D3D9_IDIRECT3D9_IID,
                name: "IDirect3D9".to_owned(),
                methods: vec![
                    ComMethodSpec::query_interface(),
                    ComMethodSpec::add_ref(),
                    ComMethodSpec::release(),
                    ComMethodSpec::unsupported_hresult("RegisterSoftwareDevice", 1),
                    ComMethodSpec::frontend_u32("GetAdapterCount", 0),
                    ComMethodSpec::frontend_hresult("GetAdapterIdentifier", 3),
                    ComMethodSpec::unsupported_u32("GetAdapterModeCount", 2),
                    ComMethodSpec::unsupported_hresult("EnumAdapterModes", 4),
                    ComMethodSpec::frontend_hresult("GetAdapterDisplayMode", 2),
                    ComMethodSpec::frontend_hresult("CheckDeviceType", 5),
                    ComMethodSpec::frontend_hresult("CheckDeviceFormat", 6),
                    ComMethodSpec::unsupported_hresult("CheckDeviceMultiSampleType", 6),
                    ComMethodSpec::unsupported_hresult("CheckDepthStencilMatch", 5),
                    ComMethodSpec::unsupported_hresult("CheckDeviceFormatConversion", 4),
                    ComMethodSpec::frontend_hresult("GetDeviceCaps", 3),
                    ComMethodSpec::frontend_u32("GetAdapterMonitor", 1),
                    ComMethodSpec::frontend_hresult("CreateDevice", 6),
                ],
            },
        ],
    }
}

const D3D9_IDIRECT3DDEVICE9_ABI: [(&str, u16); 119] = [
    ("QueryInterface", 2),
    ("AddRef", 0),
    ("Release", 0),
    ("TestCooperativeLevel", 0),
    ("GetAvailableTextureMem", 0),
    ("EvictManagedResources", 0),
    ("GetDirect3D", 1),
    ("GetDeviceCaps", 1),
    ("GetDisplayMode", 2),
    ("GetCreationParameters", 1),
    ("SetCursorProperties", 3),
    ("SetCursorPosition", 3),
    ("ShowCursor", 1),
    ("CreateAdditionalSwapChain", 2),
    ("GetSwapChain", 2),
    ("GetNumberOfSwapChains", 0),
    ("Reset", 1),
    ("Present", 4),
    ("GetBackBuffer", 4),
    ("GetRasterStatus", 2),
    ("SetDialogBoxMode", 1),
    ("SetGammaRamp", 3),
    ("GetGammaRamp", 2),
    ("CreateTexture", 8),
    ("CreateVolumeTexture", 9),
    ("CreateCubeTexture", 7),
    ("CreateVertexBuffer", 6),
    ("CreateIndexBuffer", 6),
    ("CreateRenderTarget", 8),
    ("CreateDepthStencilSurface", 8),
    ("UpdateSurface", 4),
    ("UpdateTexture", 2),
    ("GetRenderTargetData", 2),
    ("GetFrontBufferData", 2),
    ("StretchRect", 5),
    ("ColorFill", 3),
    ("CreateOffscreenPlainSurface", 6),
    ("SetRenderTarget", 2),
    ("GetRenderTarget", 2),
    ("SetDepthStencilSurface", 1),
    ("GetDepthStencilSurface", 1),
    ("BeginScene", 0),
    ("EndScene", 0),
    ("Clear", 6),
    ("SetTransform", 2),
    ("GetTransform", 2),
    ("MultiplyTransform", 2),
    ("SetViewport", 1),
    ("GetViewport", 1),
    ("SetMaterial", 1),
    ("GetMaterial", 1),
    ("SetLight", 2),
    ("GetLight", 2),
    ("LightEnable", 2),
    ("GetLightEnable", 2),
    ("SetClipPlane", 2),
    ("GetClipPlane", 2),
    ("SetRenderState", 2),
    ("GetRenderState", 2),
    ("CreateStateBlock", 2),
    ("BeginStateBlock", 0),
    ("EndStateBlock", 1),
    ("SetClipStatus", 1),
    ("GetClipStatus", 1),
    ("GetTexture", 2),
    ("SetTexture", 2),
    ("GetTextureStageState", 3),
    ("SetTextureStageState", 3),
    ("GetSamplerState", 3),
    ("SetSamplerState", 3),
    ("ValidateDevice", 1),
    ("SetPaletteEntries", 2),
    ("GetPaletteEntries", 2),
    ("SetCurrentTexturePalette", 1),
    ("GetCurrentTexturePalette", 1),
    ("SetScissorRect", 1),
    ("GetScissorRect", 1),
    ("SetSoftwareVertexProcessing", 1),
    ("GetSoftwareVertexProcessing", 0),
    ("SetNPatchMode", 1),
    ("GetNPatchMode", 0),
    ("DrawPrimitive", 3),
    ("DrawIndexedPrimitive", 6),
    ("DrawPrimitiveUP", 4),
    ("DrawIndexedPrimitiveUP", 8),
    ("ProcessVertices", 6),
    ("CreateVertexDeclaration", 2),
    ("SetVertexDeclaration", 1),
    ("GetVertexDeclaration", 1),
    ("SetFVF", 1),
    ("GetFVF", 1),
    ("CreateVertexShader", 2),
    ("SetVertexShader", 1),
    ("GetVertexShader", 1),
    ("SetVertexShaderConstantF", 3),
    ("GetVertexShaderConstantF", 3),
    ("SetVertexShaderConstantI", 3),
    ("GetVertexShaderConstantI", 3),
    ("SetVertexShaderConstantB", 3),
    ("GetVertexShaderConstantB", 3),
    ("SetStreamSource", 4),
    ("GetStreamSource", 4),
    ("SetStreamSourceFreq", 2),
    ("GetStreamSourceFreq", 2),
    ("SetIndices", 1),
    ("GetIndices", 1),
    ("CreatePixelShader", 2),
    ("SetPixelShader", 1),
    ("GetPixelShader", 1),
    ("SetPixelShaderConstantF", 3),
    ("GetPixelShaderConstantF", 3),
    ("SetPixelShaderConstantI", 3),
    ("GetPixelShaderConstantI", 3),
    ("SetPixelShaderConstantB", 3),
    ("GetPixelShaderConstantB", 3),
    ("DrawRectPatch", 3),
    ("DrawTriPatch", 3),
    ("DeletePatch", 1),
    ("CreateQuery", 2),
];

pub fn direct3ddevice9_object_spec() -> ComObjectSpec {
    let methods = D3D9_IDIRECT3DDEVICE9_ABI
        .iter()
        .enumerate()
        .map(|(slot, (name, argument_words))| match slot {
            0 => ComMethodSpec::query_interface(),
            1 => ComMethodSpec::add_ref(),
            2 => ComMethodSpec::release(),
            3 | 17 | 19 | 23 | 26 | 31 | 41 | 42 | 43 | 57 | 65 | 67 | 69 | 81 | 89 | 100 => {
                ComMethodSpec::frontend_hresult(*name, *argument_words)
            }
            4 | 12 | 15 | 78 => ComMethodSpec::unsupported_u32(*name, *argument_words),
            11 | 21 | 22 => ComMethodSpec::unsupported_void(*name, *argument_words),
            80 => ComMethodSpec::unsupported_x87_f32(*name, *argument_words),
            _ => ComMethodSpec::unsupported_hresult(*name, *argument_words),
        })
        .collect();
    ComObjectSpec {
        class_identity: D3D9_IDIRECT3DDEVICE9_CLASS_IDENTITY.to_owned(),
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            ComInterfaceSpec {
                iid: D3D9_IDIRECT3DDEVICE9_IID,
                name: "IDirect3DDevice9".to_owned(),
                methods,
            },
        ],
    }
}

const D3D9_IDIRECT3DTEXTURE9_ABI: [(&str, u16); 22] = [
    ("QueryInterface", 2),
    ("AddRef", 0),
    ("Release", 0),
    ("GetDevice", 1),
    ("SetPrivateData", 4),
    ("GetPrivateData", 3),
    ("FreePrivateData", 1),
    ("SetPriority", 1),
    ("GetPriority", 0),
    ("PreLoad", 0),
    ("GetType", 0),
    ("SetLOD", 1),
    ("GetLOD", 0),
    ("GetLevelCount", 0),
    ("SetAutoGenFilterType", 1),
    ("GetAutoGenFilterType", 0),
    ("GenerateMipSubLevels", 0),
    ("GetLevelDesc", 2),
    ("GetSurfaceLevel", 2),
    ("LockRect", 4),
    ("UnlockRect", 1),
    ("AddDirtyRect", 1),
];

pub fn direct3dtexture9_object_spec() -> ComObjectSpec {
    let methods = D3D9_IDIRECT3DTEXTURE9_ABI
        .iter()
        .enumerate()
        .map(|(slot, (name, argument_words))| match slot {
            0 => ComMethodSpec::query_interface(),
            1 => ComMethodSpec::add_ref(),
            2 => ComMethodSpec::release(),
            18..=21 => ComMethodSpec::frontend_hresult(*name, *argument_words),
            7 | 8 | 10 | 11 | 12 | 13 | 15 => {
                ComMethodSpec::unsupported_u32(*name, *argument_words)
            }
            9 | 16 => ComMethodSpec::unsupported_void(*name, *argument_words),
            _ => ComMethodSpec::unsupported_hresult(*name, *argument_words),
        })
        .collect();
    ComObjectSpec {
        class_identity: D3D9_IDIRECT3DTEXTURE9_CLASS_IDENTITY.to_owned(),
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            ComInterfaceSpec {
                iid: D3D9_IDIRECT3DTEXTURE9_IID,
                name: "IDirect3DTexture9".to_owned(),
                methods,
            },
        ],
    }
}

const D3D9_IDIRECT3DVERTEXBUFFER9_ABI: [(&str, u16); 14] = [
    ("QueryInterface", 2),
    ("AddRef", 0),
    ("Release", 0),
    ("GetDevice", 1),
    ("SetPrivateData", 4),
    ("GetPrivateData", 3),
    ("FreePrivateData", 1),
    ("SetPriority", 1),
    ("GetPriority", 0),
    ("PreLoad", 0),
    ("GetType", 0),
    ("Lock", 4),
    ("Unlock", 0),
    ("GetDesc", 1),
];

pub fn direct3dvertexbuffer9_object_spec() -> ComObjectSpec {
    let methods = D3D9_IDIRECT3DVERTEXBUFFER9_ABI
        .iter()
        .enumerate()
        .map(|(slot, (name, argument_words))| match slot {
            0 => ComMethodSpec::query_interface(),
            1 => ComMethodSpec::add_ref(),
            2 => ComMethodSpec::release(),
            11 | 12 => ComMethodSpec::frontend_hresult(*name, *argument_words),
            7 | 8 | 10 => ComMethodSpec::unsupported_u32(*name, *argument_words),
            9 => ComMethodSpec::unsupported_void(*name, *argument_words),
            _ => ComMethodSpec::unsupported_hresult(*name, *argument_words),
        })
        .collect();
    ComObjectSpec {
        class_identity: D3D9_IDIRECT3DVERTEXBUFFER9_CLASS_IDENTITY.to_owned(),
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            ComInterfaceSpec {
                iid: D3D9_IDIRECT3DVERTEXBUFFER9_IID,
                name: "IDirect3DVertexBuffer9".to_owned(),
                methods,
            },
        ],
    }
}

const D3D9_IDIRECT3DSURFACE9_ABI: [(&str, u16); 17] = [
    ("QueryInterface", 2),
    ("AddRef", 0),
    ("Release", 0),
    ("GetDevice", 1),
    ("SetPrivateData", 4),
    ("GetPrivateData", 3),
    ("FreePrivateData", 1),
    ("SetPriority", 1),
    ("GetPriority", 0),
    ("PreLoad", 0),
    ("GetType", 0),
    ("GetContainer", 2),
    ("GetDesc", 1),
    ("LockRect", 3),
    ("UnlockRect", 0),
    ("GetDC", 1),
    ("ReleaseDC", 1),
];

pub fn direct3dsurface9_object_spec() -> ComObjectSpec {
    let methods = D3D9_IDIRECT3DSURFACE9_ABI
        .iter()
        .enumerate()
        .map(|(slot, (name, argument_words))| match slot {
            0 => ComMethodSpec::query_interface(),
            1 => ComMethodSpec::add_ref(),
            2 => ComMethodSpec::release(),
            15 | 16 => ComMethodSpec::frontend_hresult(*name, *argument_words),
            7 | 8 | 10 => ComMethodSpec::unsupported_u32(*name, *argument_words),
            9 => ComMethodSpec::unsupported_void(*name, *argument_words),
            _ => ComMethodSpec::unsupported_hresult(*name, *argument_words),
        })
        .collect();
    ComObjectSpec {
        class_identity: D3D9_IDIRECT3DSURFACE9_CLASS_IDENTITY.to_owned(),
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            ComInterfaceSpec {
                iid: D3D9_IDIRECT3DSURFACE9_IID,
                name: "IDirect3DSurface9".to_owned(),
                methods,
            },
        ],
    }
}

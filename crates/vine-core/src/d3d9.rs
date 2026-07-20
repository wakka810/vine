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
    RuntimeComIid, RuntimeComReturnKind, RuntimeGraphicsColorFormat,
    RuntimeGraphicsDeviceCreateRequest, RuntimeGraphicsFrontend, RuntimeGraphicsPresentMode,
    RuntimeGraphicsTextureCreateRequest, RuntimeGraphicsTextureDescriptor,
    RuntimeGraphicsTexturePool, RuntimeGraphicsTextureUsage, RuntimeStateError,
    RuntimeStateErrorClass,
};

pub const D3D9_SDK_VERSION: u32 = 32;
pub const D3D9_DIRECT3D_CREATE9_IMPLEMENTATION: &str = "d3d9.direct3d-create9.v1";
pub const D3D9_IDIRECT3D9_CLASS_IDENTITY: &str = "d3d9.idirect3d9-root.v1";
pub const D3D9_COMPATIBILITY_POLICY_SCHEMA: &str = "vine.d3d9-compatibility-policy";
pub const D3D9_COMPATIBILITY_POLICY_VERSION: u32 = 9;
pub const D3D9_D3DADAPTER_IDENTIFIER9_BYTE_LENGTH: u32 = 1_100;
pub const D3D9_D3DCAPS9_BYTE_LENGTH: u32 = 304;
pub const D3D9_D3DDISPLAYMODE_BYTE_LENGTH: u32 = 16;
pub const D3D9_D3DPRESENT_PARAMETERS_BYTE_LENGTH: u32 = 56;
pub const D3D9_D3DDEVTYPE_HAL: u32 = 1;
pub const D3D9_D3DCAPS_READ_SCANLINE: u32 = 0x0002_0000;
pub const D3D9_D3DCAPS2_DYNAMICTEXTURES: u32 = 0x2000_0000;
pub const D3D9_D3DPS_VERSION_3_0: u32 = 0xffff_0300;
pub const D3D9_D3DFMT_X8R8G8B8: u32 = 22;
pub const D3D9_D3DRTYPE_TEXTURE: u32 = 3;
pub const D3D9_D3DUSAGE_DYNAMIC: u32 = 0x0000_0200;
pub const D3D9_D3DENUM_WHQL_LEVEL: u32 = 0x0000_0002;
pub const D3D9_CREATE_HARDWARE_VERTEX_PROCESSING_MULTITHREADED: u32 = 0x0000_0044;
pub const D3D9_D3DFMT_UNKNOWN: u32 = 0;
pub const D3D9_D3DFMT_D16: u32 = 80;
pub const D3D9_D3DMULTISAMPLE_NONE: u32 = 0;
pub const D3D9_D3DSWAPEFFECT_DISCARD: u32 = 3;
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
pub struct D3d9CreateTexturePolicy {
    pub profile_id: String,
    pub width: u32,
    pub height: u32,
    pub levels: u32,
    pub usage: u32,
    pub format: u32,
    pub pool: u32,
    pub shared_handle_must_be_null: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9CompatibilityPolicy {
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub get_adapter_count: Option<D3d9AdapterCountPolicy>,
    pub get_adapter_monitor: Option<D3d9AdapterMonitorPolicy>,
    pub get_adapter_display_mode: Option<D3d9AdapterDisplayModePolicy>,
    pub get_adapter_identifier: Option<D3d9AdapterIdentifierPolicy>,
    pub get_device_caps: Option<D3d9DeviceCapsPolicy>,
    pub check_device_type: Option<D3d9CheckDeviceTypePolicy>,
    pub check_device_format: Option<D3d9CheckDeviceFormatPolicy>,
    pub create_device: Option<D3d9CreateDevicePolicy>,
    pub create_texture: Option<D3d9CreateTexturePolicy>,
}

impl D3d9CompatibilityPolicy {
    pub fn controlled_v9() -> Self {
        Self {
            schema: D3D9_COMPATIBILITY_POLICY_SCHEMA.to_owned(),
            schema_version: D3D9_COMPATIBILITY_POLICY_VERSION,
            id: "d3d9-primary-virtual-adapter-v9".to_owned(),
            get_adapter_count: Some(D3d9AdapterCountPolicy {
                profile_id: "single-logical-adapter-v1".to_owned(),
                display_profile_id: "single-monitor-1920x1080-v1".to_owned(),
                adapter_count: 1,
            }),
            get_adapter_monitor: Some(D3d9AdapterMonitorPolicy {
                profile_id: "primary-adapter-monitor-v1".to_owned(),
                display_profile_id: "single-monitor-1920x1080-v1".to_owned(),
                adapter: 0,
            }),
            get_adapter_display_mode: Some(D3d9AdapterDisplayModePolicy {
                profile_id: "primary-adapter-1920x1080-60-x8r8g8b8-v1".to_owned(),
                display_profile_id: "single-monitor-1920x1080-v1".to_owned(),
                adapter: 0,
                refresh_rate: 60,
                format: D3D9_D3DFMT_X8R8G8B8,
            }),
            get_adapter_identifier: Some(D3d9AdapterIdentifierPolicy {
                profile_id: "primary-virtual-adapter-identifier-v1".to_owned(),
                display_profile_id: "single-monitor-1920x1080-v1".to_owned(),
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
                display_profile_id: "single-monitor-1920x1080-v1".to_owned(),
                adapter: 0,
                device_type: D3D9_D3DDEVTYPE_HAL,
                caps: D3D9_D3DCAPS_READ_SCANLINE,
                caps2: D3D9_D3DCAPS2_DYNAMICTEXTURES,
                pixel_shader_version: D3D9_D3DPS_VERSION_3_0,
                number_of_adapters_in_group: 1,
            }),
            check_device_type: Some(D3d9CheckDeviceTypePolicy {
                profile_id: "primary-hal-x8r8g8b8-windowed-fullscreen-v1".to_owned(),
                display_profile_id: "single-monitor-1920x1080-v1".to_owned(),
                adapter: 0,
                device_type: D3D9_D3DDEVTYPE_HAL,
                adapter_format: D3D9_D3DFMT_X8R8G8B8,
                back_buffer_format: D3D9_D3DFMT_X8R8G8B8,
                windowed_supported: true,
                fullscreen_supported: true,
            }),
            check_device_format: Some(D3d9CheckDeviceFormatPolicy {
                profile_id: "primary-hal-x8r8g8b8-texture-basic-dynamic-v1".to_owned(),
                display_profile_id: "single-monitor-1920x1080-v1".to_owned(),
                adapter: 0,
                device_type: D3D9_D3DDEVTYPE_HAL,
                adapter_format: D3D9_D3DFMT_X8R8G8B8,
                resource_type: D3D9_D3DRTYPE_TEXTURE,
                check_format: D3D9_D3DFMT_X8R8G8B8,
                supported_usages: [0, D3D9_D3DUSAGE_DYNAMIC],
            }),
            create_device: Some(D3d9CreateDevicePolicy {
                profile_id: "primary-windowed-hal-device-v1".to_owned(),
                display_profile_id: "single-monitor-1920x1080-v1".to_owned(),
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
                profile_id: "primary-system-memory-x8r8g8b8-texture-v1".to_owned(),
                width: 1_024,
                height: 1_024,
                levels: 1,
                usage: 0,
                format: D3D9_D3DFMT_X8R8G8B8,
                pool: D3D9_D3DPOOL_SYSTEMMEM,
                shared_handle_must_be_null: true,
            }),
        }
    }

    pub fn validate(&self) -> Result<(), ApiFault> {
        if self.schema != D3D9_COMPATIBILITY_POLICY_SCHEMA
            || self.schema_version != D3D9_COMPATIBILITY_POLICY_VERSION
            || !valid_policy_id(&self.id)
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-compatibility-policy-header-invalid",
                "D3D9 compatibility policy has an unsupported schema, version, or ID",
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
            && (!valid_policy_id(&create.profile_id)
                || create.width != 1_024
                || create.height != 1_024
                || create.levels != 1
                || create.usage != 0
                || create.format != D3D9_D3DFMT_X8R8G8B8
                || create.pool != D3D9_D3DPOOL_SYSTEMMEM
                || !create.shared_handle_must_be_null
                || self.create_device.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "d3d9-compatibility-policy",
                "d3d9-create-texture-profile-invalid",
                "D3D9 texture-creation profile must describe the reached device-owned system-memory texture",
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
            self.create_texture.as_ref().map(|profile| profile.format),
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
    if !is_root && !is_device {
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
    if call.frame().vtable_slot != 23
        || call.frame().method != "CreateTexture"
        || call.explicit_argument_words() != 8
        || call.frame().explicit_arguments.len() != 8
        || call.return_kind() != RuntimeComReturnKind::HResult
    {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "d3d9-com-dispatch",
            "d3d9-com-abi-mismatch",
            "registered IDirect3DDevice9 frontend ABI does not match CreateTexture",
        );
    }
    dispatch_create_texture(policy, call)
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
    if width != create_policy.width
        || height != create_policy.height
        || levels != create_policy.levels
        || usage != create_policy.usage
        || format != create_policy.format
        || pool != create_policy.pool
    {
        return call.complete(
            ComReturnValue::HResult(D3D9_D3DERR_NOTAVAILABLE),
            Vec::new(),
        );
    }

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
                    usage: RuntimeGraphicsTextureUsage::None,
                    color_format: RuntimeGraphicsColorFormat::B8g8r8x8Unorm,
                    pool: RuntimeGraphicsTexturePool::SystemMemory,
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
            23 => ComMethodSpec::frontend_hresult(*name, *argument_words),
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

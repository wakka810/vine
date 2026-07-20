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
use crate::runtime::{RuntimeComIid, RuntimeComReturnKind};

pub const D3D9_SDK_VERSION: u32 = 32;
pub const D3D9_DIRECT3D_CREATE9_IMPLEMENTATION: &str = "d3d9.direct3d-create9.v1";
pub const D3D9_IDIRECT3D9_CLASS_IDENTITY: &str = "d3d9.idirect3d9-root.v1";
pub const D3D9_COMPATIBILITY_POLICY_SCHEMA: &str = "vine.d3d9-compatibility-policy";
pub const D3D9_COMPATIBILITY_POLICY_VERSION: u32 = 6;
pub const D3D9_D3DADAPTER_IDENTIFIER9_BYTE_LENGTH: u32 = 1_100;
pub const D3D9_D3DCAPS9_BYTE_LENGTH: u32 = 304;
pub const D3D9_D3DDISPLAYMODE_BYTE_LENGTH: u32 = 16;
pub const D3D9_D3DDEVTYPE_HAL: u32 = 1;
pub const D3D9_D3DCAPS_READ_SCANLINE: u32 = 0x0002_0000;
pub const D3D9_D3DCAPS2_DYNAMICTEXTURES: u32 = 0x2000_0000;
pub const D3D9_D3DPS_VERSION_3_0: u32 = 0xffff_0300;
pub const D3D9_D3DFMT_X8R8G8B8: u32 = 22;
pub const D3D9_D3DRTYPE_TEXTURE: u32 = 3;
pub const D3D9_D3DUSAGE_DYNAMIC: u32 = 0x0000_0200;
pub const D3D9_D3DENUM_WHQL_LEVEL: u32 = 0x0000_0002;
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct D3d9AdapterCountPolicy {
    pub profile_id: String,
    pub display_profile_id: String,
    pub adapter_count: u32,
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
pub struct D3d9CompatibilityPolicy {
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub get_adapter_count: Option<D3d9AdapterCountPolicy>,
    pub get_adapter_display_mode: Option<D3d9AdapterDisplayModePolicy>,
    pub get_adapter_identifier: Option<D3d9AdapterIdentifierPolicy>,
    pub get_device_caps: Option<D3d9DeviceCapsPolicy>,
    pub check_device_type: Option<D3d9CheckDeviceTypePolicy>,
    pub check_device_format: Option<D3d9CheckDeviceFormatPolicy>,
}

impl D3d9CompatibilityPolicy {
    pub fn controlled_v6() -> Self {
        Self {
            schema: D3D9_COMPATIBILITY_POLICY_SCHEMA.to_owned(),
            schema_version: D3D9_COMPATIBILITY_POLICY_VERSION,
            id: "d3d9-primary-virtual-adapter-v6".to_owned(),
            get_adapter_count: Some(D3d9AdapterCountPolicy {
                profile_id: "single-logical-adapter-v1".to_owned(),
                display_profile_id: "single-monitor-1920x1080-v1".to_owned(),
                adapter_count: 1,
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
        let coherent_profiles = [
            self.get_adapter_count
                .as_ref()
                .map(|profile| (0, profile.display_profile_id.as_str())),
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
    if call.class_identity() != D3D9_IDIRECT3D9_CLASS_IDENTITY
        || call.frame().iid != D3D9_IDIRECT3D9_IID
    {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "d3d9-com-dispatch",
            "d3d9-com-route-mismatch",
            "D3D9 frontend received a COM method outside its registered IDirect3D9 route",
        );
    }
    let (expected_argument_words, expected_return_kind) =
        match (call.frame().vtable_slot, call.frame().method.as_str()) {
            (4, "GetAdapterCount") => (0, RuntimeComReturnKind::U32),
            (5, "GetAdapterIdentifier") => (3, RuntimeComReturnKind::HResult),
            (8, "GetAdapterDisplayMode") => (2, RuntimeComReturnKind::HResult),
            (9, "CheckDeviceType") => (5, RuntimeComReturnKind::HResult),
            (10, "CheckDeviceFormat") => (6, RuntimeComReturnKind::HResult),
            (14, "GetDeviceCaps") => (3, RuntimeComReturnKind::HResult),
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
    if let Err(fault) = policy.validate() {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            fault.stage,
            fault.code,
            fault.message,
        );
    }
    match call.frame().vtable_slot {
        4 => dispatch_get_adapter_count(policy, call),
        5 => dispatch_get_adapter_identifier(policy, call),
        8 => dispatch_get_adapter_display_mode(policy, call),
        9 => dispatch_check_device_type(policy, call),
        10 => dispatch_check_device_format(policy, call),
        14 => dispatch_get_device_caps(policy, call),
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
                    ComMethodSpec::unsupported_u32("GetAdapterMonitor", 1),
                    ComMethodSpec::unsupported_hresult("CreateDevice", 6),
                ],
            },
        ],
    }
}

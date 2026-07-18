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
pub const D3D9_COMPATIBILITY_POLICY_VERSION: u32 = 1;
pub const D3D9_D3DDISPLAYMODE_BYTE_LENGTH: u32 = 16;
pub const D3D9_D3DFMT_X8R8G8B8: u32 = 22;
pub const D3D9_D3DERR_INVALIDCALL: ComHResult = ComHResult(0x8876_086c_u32 as i32);
pub const D3D9_IDIRECT3D9_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0xca, 0xcb, 0xbd, 0x81, 0xd4, 0x64, 0x6d, 0x42, 0xae, 0x8d, 0xad, 0x01, 0x47, 0xf4, 0x27, 0x5c,
]);

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
pub struct D3d9CompatibilityPolicy {
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub get_adapter_display_mode: Option<D3d9AdapterDisplayModePolicy>,
}

impl D3d9CompatibilityPolicy {
    pub fn controlled_v1() -> Self {
        Self {
            schema: D3D9_COMPATIBILITY_POLICY_SCHEMA.to_owned(),
            schema_version: D3D9_COMPATIBILITY_POLICY_VERSION,
            id: "d3d9-primary-adapter-display-mode-v1".to_owned(),
            get_adapter_display_mode: Some(D3d9AdapterDisplayModePolicy {
                profile_id: "primary-adapter-1920x1080-60-x8r8g8b8-v1".to_owned(),
                display_profile_id: "single-monitor-1920x1080-v1".to_owned(),
                adapter: 0,
                refresh_rate: 60,
                format: D3D9_D3DFMT_X8R8G8B8,
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

fn valid_policy_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && value.is_ascii() && !value.as_bytes().contains(&0)
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
        || call.frame().vtable_slot != 8
        || call.frame().method != "GetAdapterDisplayMode"
    {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "d3d9-com-dispatch",
            "d3d9-com-route-mismatch",
            "D3D9 frontend received a COM method outside its registered IDirect3D9 route",
        );
    }
    if call.explicit_argument_words() != 2
        || call.frame().explicit_arguments.len() != 2
        || call.return_kind() != RuntimeComReturnKind::HResult
    {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "d3d9-com-dispatch",
            "d3d9-com-abi-mismatch",
            "registered IDirect3D9 frontend ABI does not match GetAdapterDisplayMode",
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

    let runtime_profile_id = call.runtime().display_profile_id();
    let monitor = call.runtime().primary_monitor();
    let Some(monitor) = monitor else {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "d3d9-display",
            "logical-primary-monitor-missing",
            "D3D9 display-mode policy is enabled but RuntimeDisplays has no primary monitor",
        );
    };
    if runtime_profile_id != Some(mode_policy.display_profile_id.as_str()) || !monitor.primary {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "d3d9-display",
            "logical-display-profile-mismatch",
            "Rust display authority does not match the selected D3D9 display-mode profile",
        );
    }
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
    let output = GuestAddress(output_pointer);
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
                    ComMethodSpec::unsupported_u32("GetAdapterCount", 0),
                    ComMethodSpec::unsupported_hresult("GetAdapterIdentifier", 3),
                    ComMethodSpec::unsupported_u32("GetAdapterModeCount", 2),
                    ComMethodSpec::unsupported_hresult("EnumAdapterModes", 4),
                    ComMethodSpec::frontend_hresult("GetAdapterDisplayMode", 2),
                    ComMethodSpec::unsupported_hresult("CheckDeviceType", 5),
                    ComMethodSpec::unsupported_hresult("CheckDeviceFormat", 6),
                    ComMethodSpec::unsupported_hresult("CheckDeviceMultiSampleType", 6),
                    ComMethodSpec::unsupported_hresult("CheckDepthStencilMatch", 5),
                    ComMethodSpec::unsupported_hresult("CheckDeviceFormatConversion", 4),
                    ComMethodSpec::unsupported_hresult("GetDeviceCaps", 3),
                    ComMethodSpec::unsupported_u32("GetAdapterMonitor", 1),
                    ComMethodSpec::unsupported_hresult("CreateDevice", 6),
                ],
            },
        ],
    }
}

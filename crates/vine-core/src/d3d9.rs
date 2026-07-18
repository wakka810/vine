use crate::api::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiCallFrame, ApiContract, ApiDirection,
    ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiReturnSchema, ApiReturnValue,
    ApiSymbol, ApiValueSchema, CallbackPolicy, CallingConvention, ExecutionSession,
    LastErrorPolicy, ReentrancyPolicy, StackCleanup,
};
use crate::com::{ComInterfaceSpec, ComMethodSpec, ComObjectSpec};
use crate::runtime::RuntimeComIid;

pub const D3D9_SDK_VERSION: u32 = 32;
pub const D3D9_DIRECT3D_CREATE9_IMPLEMENTATION: &str = "d3d9.direct3d-create9.v1";
pub const D3D9_IDIRECT3D9_CLASS_IDENTITY: &str = "d3d9.idirect3d9-root.v1";
pub const D3D9_IDIRECT3D9_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0xca, 0xcb, 0xbd, 0x81, 0xd4, 0x64, 0x6d, 0x42, 0xae, 0x8d, 0xad, 0x01, 0x47, 0xf4, 0x27, 0x5c,
]);

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
                    ComMethodSpec::unsupported_hresult("GetAdapterDisplayMode", 2),
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

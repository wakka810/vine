use crate::api::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiCallFrame, ApiContract, ApiDirection,
    ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiPointeeSchema,
    ApiPointerConstraint, ApiReturnSchema, ApiReturnValue, ApiSymbol, ApiValueSchema,
    ApiWriteRequirement, CallbackPolicy, CallingConvention, ExecutionSession, LastErrorPolicy,
    ReentrancyPolicy, StackCleanup,
};
use crate::com::{
    COM_E_NOINTERFACE, COM_S_OK, ComHResult, ComInterfaceSpec, ComMethodSpec, ComObjectSpec,
};
use crate::memory::GuestAddress;
use crate::runtime::{RUNTIME_COM_IUNKNOWN_IID, RuntimeComIid, RuntimeStateError};

pub const DINPUT8_VERSION: u32 = 0x0800;
pub const DINPUT8_DIRECT_INPUT_8_CREATE_IMPLEMENTATION: &str = "dinput8.direct-input-8-create.v1";
pub const DINPUT8_IDIRECTINPUT8A_CLASS_IDENTITY: &str = "dinput8.idirectinput8a-root.v1";
pub const DINPUT8_IDIRECTINPUT8A_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0x30, 0x80, 0x79, 0xbf, 0x3a, 0x48, 0xa2, 0x4d, 0xaa, 0x99, 0x5d, 0x64, 0xed, 0x36, 0x97, 0x00,
]);
pub const DINPUT8_DIERR_BETADIRECTINPUTVERSION: ComHResult = ComHResult(0x8007_0481_u32 as i32);
pub const DINPUT8_DIERR_OLDDIRECTINPUTVERSION: ComHResult = ComHResult(0x8007_047e_u32 as i32);
pub const DINPUT8_CLASS_E_NOAGGREGATION: ComHResult = ComHResult(0x8004_0110_u32 as i32);

pub fn dispatch_dinput8_api(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    match contract.implementation_id.as_str() {
        DINPUT8_DIRECT_INPUT_8_CREATE_IMPLEMENTATION => {
            handle_direct_input_8_create(contract, frame, session)
        }
        implementation_id => ApiHandlerResult::Fault(ApiFault::unsupported(
            "dinput8-api-dispatch",
            "implementation-id-unsupported",
            format!("DirectInput implementation {implementation_id} is not registered"),
        )),
    }
}

fn handle_direct_input_8_create(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    let Some(_instance) = frame.argument_u32(0) else {
        return decoded_argument_missing();
    };
    let Some(version) = frame.argument_u32(1) else {
        return decoded_argument_missing();
    };
    let Some(iid_address) = frame.argument_u32(2).map(GuestAddress) else {
        return decoded_argument_missing();
    };
    let Some(output_address) = frame.argument_u32(3).map(GuestAddress) else {
        return decoded_argument_missing();
    };
    let Some(outer_unknown) = frame.argument_u32(4) else {
        return decoded_argument_missing();
    };

    let iid_bytes = match session.memory().read(iid_address, 16) {
        Ok(bytes) => bytes,
        Err(error) => {
            return ApiHandlerResult::Fault(ApiFault::from_memory(error, &contract.endpoint));
        }
    };
    let iid = RuntimeComIid::from_windows_bytes(
        iid_bytes
            .try_into()
            .expect("the fixed IID read must contain exactly 16 bytes"),
    );

    if outer_unknown != 0 {
        return failed_factory(output_address, DINPUT8_CLASS_E_NOAGGREGATION);
    }
    if version < DINPUT8_VERSION {
        return failed_factory(output_address, DINPUT8_DIERR_BETADIRECTINPUTVERSION);
    }
    if version > DINPUT8_VERSION {
        return failed_factory(output_address, DINPUT8_DIERR_OLDDIRECTINPUTVERSION);
    }
    if iid != DINPUT8_IDIRECTINPUT8A_IID && iid != RUNTIME_COM_IUNKNOWN_IID {
        return failed_factory(output_address, COM_E_NOINTERFACE);
    }

    let installed = match session.install_com_object_with_guest_writes(
        direct_input8a_object_spec(),
        |planned| {
            let interface = planned
                .interfaces
                .iter()
                .find(|interface| interface.iid == iid)
                .ok_or_else(|| {
                    RuntimeStateError::runtime_defect(
                        "dinput8-api-dispatch",
                        "requested-interface-installation-missing",
                        "installed DirectInput root object omitted the requested interface",
                    )
                })?;
            Ok(vec![(
                output_address,
                interface.guest_pointer.0.to_le_bytes().to_vec(),
            )])
        },
    ) {
        Ok((installed, _guest_writes)) => installed,
        Err(error) => {
            return ApiHandlerResult::Fault(ApiFault::from_runtime(error, &contract.endpoint));
        }
    };
    let interface = installed
        .interfaces
        .iter()
        .find(|interface| interface.iid == iid)
        .expect("atomic installation already validated the requested interface");

    ApiHandlerResult::Immediate(
        ApiImmediate::new(ApiReturnValue::HResult(COM_S_OK)).with_guest_write(
            output_address,
            interface.guest_pointer.0.to_le_bytes().to_vec(),
        ),
    )
}

fn decoded_argument_missing() -> ApiHandlerResult {
    ApiHandlerResult::Fault(ApiFault::runtime_defect(
        "dinput8-api-dispatch",
        "decoded-argument-missing",
        "decoded DirectInput8Create frame is missing an argument",
    ))
}

fn failed_factory(output_address: GuestAddress, result: ComHResult) -> ApiHandlerResult {
    ApiHandlerResult::Immediate(
        ApiImmediate::new(ApiReturnValue::HResult(result))
            .with_guest_write(output_address, 0_u32.to_le_bytes().to_vec()),
    )
}

pub fn dinput8_contract_for_endpoint(
    endpoint: ApiEndpointIdentity,
) -> Result<Option<ApiContract>, ApiFault> {
    if !endpoint.module.eq_ignore_ascii_case("dinput8.dll") {
        return Ok(None);
    }
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return Ok(None);
    };
    match name.as_str() {
        "DirectInput8Create" => Ok(Some(direct_input_8_create_contract(endpoint)?)),
        _ => Ok(None),
    }
}

pub fn direct_input_8_create_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                opaque_pointer("instance", true),
                u32_argument("version"),
                input_struct("iid", "IID", 16),
                output_struct("interface", "COM_INTERFACE_POINTER", 4),
                opaque_pointer("outerUnknown", true),
            ],
            return_value: ApiReturnSchema::HResult,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Disallowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: DINPUT8_DIRECT_INPUT_8_CREATE_IMPLEMENTATION.to_owned(),
    })
}

pub fn direct_input8a_object_spec() -> ComObjectSpec {
    ComObjectSpec {
        class_identity: DINPUT8_IDIRECTINPUT8A_CLASS_IDENTITY.to_owned(),
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            ComInterfaceSpec {
                iid: DINPUT8_IDIRECTINPUT8A_IID,
                name: "IDirectInput8A".to_owned(),
                methods: vec![
                    ComMethodSpec::query_interface(),
                    ComMethodSpec::add_ref(),
                    ComMethodSpec::release(),
                    ComMethodSpec::unsupported_hresult("CreateDevice", 3),
                    ComMethodSpec::unsupported_hresult("EnumDevices", 4),
                    ComMethodSpec::unsupported_hresult("GetDeviceStatus", 1),
                    ComMethodSpec::unsupported_hresult("RunControlPanel", 2),
                    ComMethodSpec::unsupported_hresult("Initialize", 2),
                    ComMethodSpec::unsupported_hresult("FindDevice", 3),
                    ComMethodSpec::unsupported_hresult("EnumDevicesBySemantics", 5),
                    ComMethodSpec::unsupported_hresult("ConfigureDevices", 4),
                ],
            },
        ],
    }
}

fn u32_argument(name: &str) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::U32,
    }
}

fn opaque_pointer(name: &str, nullable: bool) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::OpaquePointer {
            nullable,
            alignment: 4,
        },
    }
}

fn input_struct(name: &str, type_id: &str, byte_length: u32) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::Pointer {
            constraint: ApiPointerConstraint {
                nullable: false,
                alignment: 4,
                byte_length,
                pointee: ApiPointeeSchema::Struct {
                    type_id: type_id.to_owned(),
                },
                write_requirement: ApiWriteRequirement::None,
            },
        },
    }
}

fn output_struct(name: &str, type_id: &str, byte_length: u32) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::Out,
        value: ApiValueSchema::Pointer {
            constraint: ApiPointerConstraint {
                nullable: false,
                alignment: 4,
                byte_length,
                pointee: ApiPointeeSchema::Struct {
                    type_id: type_id.to_owned(),
                },
                write_requirement: ApiWriteRequirement::Required,
            },
        },
    }
}

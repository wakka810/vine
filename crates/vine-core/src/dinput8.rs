use crate::api::{
    ApiCallFrame, ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiSymbol,
};
use crate::com::{
    COM_E_NOINTERFACE, COM_S_OK, ComDispatchOutcome, ComFrontendCall, ComHResult, ComInterfaceSpec,
    ComMethodSpec, ComObjectSpec,
};
use crate::memory::{GuestAddress, MemoryAccess};
use crate::runtime::{RUNTIME_COM_IUNKNOWN_IID, RuntimeComClass, RuntimeComIid, RuntimeStateError};

pub const DINPUT8_VERSION: u32 = 0x0800;
pub const DINPUT8_IDIRECTINPUT8A_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0x30, 0x80, 0x79, 0xbf, 0x3a, 0x48, 0xa2, 0x4d, 0xaa, 0x99, 0x5d, 0x64, 0xed, 0x36, 0x97, 0x00,
]);
pub const DINPUT8_DIERR_BETADIRECTINPUTVERSION: ComHResult = ComHResult(0x8007_0481_u32 as i32);
pub const DINPUT8_DIERR_OLDDIRECTINPUTVERSION: ComHResult = ComHResult(0x8007_047e_u32 as i32);
pub const DINPUT8_CLASS_E_NOAGGREGATION: ComHResult = ComHResult(0x8004_0110_u32 as i32);
pub const DINPUT8_E_INVALIDARG: ComHResult = ComHResult(0x8007_0057_u32 as i32);
pub const DINPUT8_DI8DEVCLASS_GAMECTRL: u32 = 4;
pub const DINPUT8_DIEDFL_ATTACHEDONLY: u32 = 0x0000_0001;
pub const DINPUT8_DIEDFL_VALID_MASK: u32 = 0x0007_0101;
pub(crate) fn dispatch_dinput8_api(
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let endpoint = &frame.endpoint;
    match (&*endpoint.module.to_ascii_lowercase(), &endpoint.symbol) {
        ("dinput8.dll", ApiSymbol::Name { name }) if name == "DirectInput8Create" => {
            handle_direct_input_8_create(endpoint, frame, runtime)
        }
        _ => ApiHandlerResult::Fault(ApiFault::new("DirectInput endpoint is not implemented")),
    }
}

pub(crate) fn dispatch_dinput8_com(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    if call.class() != RuntimeComClass::DirectInput8A
        || call.frame().iid != DINPUT8_IDIRECTINPUT8A_IID
    {
        return call.fault(
            "DirectInput 8 frontend received a COM object outside its owned root interface",
        );
    }
    match call.frame().vtable_slot {
        4 => handle_enum_devices(call),
        _ => call.fault("DirectInput 8 frontend method is outside the reached compatibility band"),
    }
}

fn handle_enum_devices(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let (device_class, callback, _reference, flags) =
        match call.frame().explicit_arguments.as_slice() {
            [device_class, callback, reference, flags] => {
                (*device_class, *callback, *reference, *flags)
            }
            _ => {
                return call.fault(
                    "decoded IDirectInput8A::EnumDevices frame does not contain four arguments",
                );
            }
        };

    if callback == 0
        || device_class > DINPUT8_DI8DEVCLASS_GAMECTRL
        || flags & !DINPUT8_DIEDFL_VALID_MASK != 0
    {
        return call.complete(DINPUT8_E_INVALIDARG.bits());
    }

    if device_class != DINPUT8_DI8DEVCLASS_GAMECTRL || flags != DINPUT8_DIEDFL_ATTACHEDONLY {
        return call.fault(
            "DirectInput 8 device enumeration variant requires device discovery or callbacks",
        );
    }
    if let Err(error) =
        call.runtime
            .memory
            .validate_access(GuestAddress(callback), 1, MemoryAccess::Execute)
    {
        return call.memory_fault(error);
    }

    call.complete(COM_S_OK.bits())
}

fn handle_direct_input_8_create(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
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

    let iid_bytes = match runtime.memory.read(iid_address, 16) {
        Ok(bytes) => bytes,
        Err(error) => {
            return ApiHandlerResult::Fault(ApiFault::from_memory(error, endpoint));
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

    match runtime.install_com_object_with_guest_writes(
        None,
        direct_input8a_object_spec(),
        |planned| {
            let interface = planned
                .interfaces
                .iter()
                .find(|(candidate_iid, _)| *candidate_iid == iid)
                .ok_or_else(|| {
                    RuntimeStateError::new(
                        "installed DirectInput root object omitted the requested interface",
                    )
                })?;
            Ok(vec![(output_address, interface.1.0.to_le_bytes().to_vec())])
        },
    ) {
        Ok(_) => {}
        Err(error) => {
            return ApiHandlerResult::Fault(ApiFault::from_runtime(error, endpoint));
        }
    };
    ApiHandlerResult::Immediate(ApiImmediate::new(Some((COM_S_OK).bits())))
}

fn decoded_argument_missing() -> ApiHandlerResult {
    ApiHandlerResult::Fault(ApiFault::new(
        "decoded DirectInput8Create frame is missing an argument",
    ))
}

fn failed_factory(output_address: GuestAddress, result: ComHResult) -> ApiHandlerResult {
    ApiHandlerResult::Immediate(
        ApiImmediate::new(Some((result).bits()))
            .with_guest_write(output_address, 0_u32.to_le_bytes().to_vec()),
    )
}

pub(crate) fn dinput8_argument_words(endpoint: &ApiEndpointIdentity) -> Option<u8> {
    if !endpoint.module.eq_ignore_ascii_case("dinput8.dll") {
        return None;
    }
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return None;
    };
    match name.as_str() {
        "DirectInput8Create" => Some(5),
        _ => None,
    }
}

pub(crate) fn direct_input8a_object_spec() -> ComObjectSpec {
    ComObjectSpec {
        class: RuntimeComClass::DirectInput8A,
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            ComInterfaceSpec {
                iid: DINPUT8_IDIRECTINPUT8A_IID,
                name: "IDirectInput8A".to_owned(),
                methods: vec![
                    ComMethodSpec::query_interface(),
                    ComMethodSpec::add_ref(),
                    ComMethodSpec::release(),
                    ComMethodSpec::unsupported("CreateDevice"),
                    ComMethodSpec::frontend("EnumDevices", 4),
                    ComMethodSpec::unsupported("GetDeviceStatus"),
                    ComMethodSpec::unsupported("RunControlPanel"),
                    ComMethodSpec::unsupported("Initialize"),
                    ComMethodSpec::unsupported("FindDevice"),
                    ComMethodSpec::unsupported("EnumDevicesBySemantics"),
                    ComMethodSpec::unsupported("ConfigureDevices"),
                ],
            },
        ],
    }
}

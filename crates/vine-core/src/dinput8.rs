use crate::api::{
    ApiCallFrame, ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiSymbol,
};
use crate::com::{
    COM_E_NOINTERFACE, COM_S_OK, ComDispatchOutcome, ComFrontendCall, ComHResult, ComInterfaceSpec,
    ComMethodSpec, ComObjectSpec,
};
use crate::memory::{GuestAddress, MemoryAccess};
use crate::runtime::{RUNTIME_COM_IUNKNOWN_IID, RuntimeComClass, RuntimeComIid, RuntimeStateError};

pub const DINPUT_IDIRECTINPUTA_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0x60, 0x13, 0x52, 0x89, 0x8a, 0xaa, 0xcf, 0x11, 0xbf, 0xc7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00,
]);
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
const DEVICE8_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0x80, 0x10, 0xd4, 0x54, 0x15, 0xdc, 0x33, 0x48, 0xa4, 0x1b, 0x74, 0x8f, 0x73, 0xa3, 0x81, 0x79,
]);
const DEVICE_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0x80, 0xe6, 0x44, 0x59, 0x2e, 0xc9, 0xcf, 0x11, 0xbf, 0xc7, 0x44, 0x45, 0x53, 0x54, 0, 0,
]);
const KEYBOARD_GUID: [u8; 16] = [
    0x61, 0x2b, 0x1d, 0x6f, 0xa0, 0xd5, 0xcf, 0x11, 0xbf, 0xc7, 0x44, 0x45, 0x53, 0x54, 0, 0,
];
const KEY_GUID: [u8; 16] = [
    0x20, 0x82, 0x72, 0x55, 0x3c, 0xd3, 0xcf, 0x11, 0xbf, 0xc7, 0x44, 0x45, 0x53, 0x54, 0, 0,
];
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct KeyboardDevice {
    pub acquired: bool,
    pub lost: bool,
    pub format: Option<(u32, Vec<(u32, u8)>)>,
    pub window: Option<(crate::runtime::RuntimeWindowId, u32)>,
    pub cooperative_flags: u32,
}
pub(crate) fn dispatch_dinput8_api(
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let endpoint = &frame.endpoint;
    match (&*endpoint.module.to_ascii_lowercase(), &endpoint.symbol) {
        ("dinput8.dll", ApiSymbol::Name { name }) if name == "DirectInput8Create" => {
            handle_direct_input_8_create(endpoint, frame, runtime)
        }
        ("dinput.dll", ApiSymbol::Name { name }) if name == "DirectInputCreateA" => {
            handle_direct_input_create(endpoint, frame, runtime)
        }
        _ => ApiHandlerResult::Fault(ApiFault::new("DirectInput endpoint is not implemented")),
    }
}

pub(crate) fn dispatch_dinput8_com(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    if call.class() == RuntimeComClass::DirectInputKeyboard {
        return keyboard_call(call);
    }
    if call.class() != RuntimeComClass::DirectInput8A
        || !matches!(
            call.frame().iid,
            DINPUT8_IDIRECTINPUT8A_IID | DINPUT_IDIRECTINPUTA_IID
        )
    {
        return call.fault(
            "DirectInput 8 frontend received a COM object outside its owned root interface",
        );
    }
    match call.frame().vtable_slot {
        3 => create_keyboard(call),
        4 => handle_enum_devices(call),
        _ => call.fault("DirectInput 8 frontend method is outside the reached compatibility band"),
    }
}

fn create_keyboard(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let args = call.frame().explicit_arguments.clone();
    if args[0] == 0 || args[1] == 0 {
        return call.complete(DINPUT8_E_INVALIDARG.bits());
    }
    if args[2] != 0 {
        return call.complete(DINPUT8_CLASS_E_NOAGGREGATION.bits());
    }
    let guid = match call.runtime.memory.read(GuestAddress(args[0]), 16) {
        Ok(b) => b,
        Err(e) => return call.memory_fault(e),
    };
    if guid != KEYBOARD_GUID {
        return call.complete(0x80040154);
    } // DIERR_DEVICENOTREG
    let iid = if call.frame().iid == DINPUT8_IDIRECTINPUT8A_IID {
        DEVICE8_IID
    } else {
        DEVICE_IID
    };
    let parent = call.frame().interface_id;
    let object = match call.runtime.install_com_object_with_guest_writes(
        Some(parent),
        keyboard_spec(),
        |object| {
            let pointer = object
                .interfaces
                .iter()
                .find(|(i, _)| *i == iid)
                .ok_or_else(|| RuntimeStateError::new("keyboard interface missing"))?
                .1;
            Ok(vec![(
                GuestAddress(args[1]),
                pointer.0.to_le_bytes().to_vec(),
            )])
        },
    ) {
        Ok(o) => o,
        Err(e) => return call.runtime_fault(e),
    };
    call.runtime.insert_direct_input_device(object.object_id);
    call.complete(0)
}

fn keyboard_spec() -> ComObjectSpec {
    let names = [
        "GetCapabilities",
        "EnumObjects",
        "GetProperty",
        "SetProperty",
        "Acquire",
        "Unacquire",
        "GetDeviceState",
        "GetDeviceData",
        "SetDataFormat",
        "SetEventNotification",
        "SetCooperativeLevel",
        "GetObjectInfo",
        "GetDeviceInfo",
        "RunControlPanel",
        "Initialize",
        "CreateEffect",
        "EnumEffects",
        "GetEffectInfo",
        "GetForceFeedbackState",
        "SendForceFeedbackCommand",
        "EnumCreatedEffectObjects",
        "Escape",
        "Poll",
        "SendDeviceData",
        "EnumEffectsInFile",
        "WriteEffectToFile",
        "BuildActionMap",
        "SetActionMap",
        "GetImageInfo",
    ];
    let mut methods = vec![
        ComMethodSpec::query_interface(),
        ComMethodSpec::add_ref(),
        ComMethodSpec::release(),
    ];
    methods.extend(names.into_iter().enumerate().map(|(i, n)| match i + 3 {
        7 | 8 | 25 => ComMethodSpec::frontend(n, 0),
        9 | 13 => ComMethodSpec::frontend(n, 2),
        11 => ComMethodSpec::frontend(n, 1),
        _ => ComMethodSpec::unsupported(n),
    }));
    let base = methods[..18].to_vec();
    ComObjectSpec {
        class: RuntimeComClass::DirectInputKeyboard,
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            ComInterfaceSpec {
                iid: DEVICE8_IID,
                name: "IDirectInputDevice8A".to_owned(),
                methods,
            },
            ComInterfaceSpec {
                iid: DEVICE_IID,
                name: "IDirectInputDeviceA".to_owned(),
                methods: base,
            },
        ],
    }
}

fn keyboard_call(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let object = call.frame().object_id;
    let state = match call.runtime.direct_input_device(object) {
        Ok(s) => s.clone(),
        Err(e) => return call.runtime_fault(e),
    };
    let args = call.frame().explicit_arguments.clone();
    let foreground_ready = state
        .window
        .is_some_and(|(id, handle)| call.runtime.window(handle).is_ok_and(|w| w.id == id))
        && (state.cooperative_flags & 4 == 0
            || (call
                .runtime
                .keyboard_state()
                .is_some_and(|k| k.source_focused)
                && call.runtime.active_window_handle().ok().flatten()
                    == state.window.map(|(_, h)| h)));
    match call.frame().vtable_slot {
        7 => {
            if state.format.is_none() || state.window.is_none() {
                return call.complete(0x80070015);
            }
            if !foreground_ready {
                return call.complete(0x80070005);
            }
            let device = call
                .runtime
                .direct_input_device_mut(object)
                .expect("live keyboard");
            device.acquired = true;
            device.lost = false;
            call.complete(u32::from(state.acquired))
        }
        8 => {
            let device = call
                .runtime
                .direct_input_device_mut(object)
                .expect("live keyboard");
            device.acquired = false;
            device.lost = false;
            call.complete(u32::from(!state.acquired))
        }
        9 => {
            let Some((length, offsets)) = state.format else {
                return call.complete(0x80070015);
            };
            if args[0] != length || args[1] == 0 {
                return call.complete(DINPUT8_E_INVALIDARG.bits());
            }
            if let Err(e) = call.runtime.memory.validate_access(
                GuestAddress(args[1]),
                length as usize,
                MemoryAccess::Write,
            ) {
                return call.memory_fault(e);
            }
            if !state.acquired {
                return call.complete(if state.lost { 0x8007001e } else { 0x8007000c });
            }
            if !foreground_ready {
                let device = call
                    .runtime
                    .direct_input_device_mut(object)
                    .expect("live keyboard");
                device.acquired = false;
                device.lost = true;
                return call.complete(0x8007001e);
            }
            let Some(keys) = call.runtime.direct_input_keys() else {
                return call.fault("DirectInput keyboard requires a configured input source");
            };
            let mut bytes = vec![0; length as usize];
            for (offset, scan) in offsets {
                bytes[offset as usize] = keys[scan as usize];
            }
            if let Err(e) = call.runtime.memory.write(GuestAddress(args[1]), &bytes) {
                return call.memory_fault(e);
            }
            call.complete(0)
        }
        11 => {
            if state.acquired {
                return call.complete(0x800700aa);
            }
            let bytes = match call.runtime.memory.read(GuestAddress(args[0]), 24) {
                Ok(b) => b,
                Err(e) => return call.memory_fault(e),
            };
            let words: Vec<_> = bytes
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .collect();
            if words[0] != 24
                || words[1] != 16
                || words[2] != 2
                || words[3] == 0
                || words[3] > 4096
                || words[4] == 0
                || words[4] > 256
                || words[5] == 0
            {
                return call.complete(DINPUT8_E_INVALIDARG.bits());
            }
            let objects = match call
                .runtime
                .memory
                .read(GuestAddress(words[5]), words[4] as usize * 16)
            {
                Ok(b) => b,
                Err(e) => return call.memory_fault(e),
            };
            let mut offsets = Vec::new();
            for entry in objects.chunks_exact(16) {
                let w: Vec<_> = entry
                    .chunks_exact(4)
                    .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                    .collect();
                if w[1] >= words[3]
                    || w[2] & 0x7f0000ff != 0x0c
                    || (w[2] >> 8) & 0xffff > 255
                    || w[3] != 0
                    || offsets.iter().any(|(offset, _)| *offset == w[1])
                {
                    return call.complete(DINPUT8_E_INVALIDARG.bits());
                }
                if w[0] != 0 {
                    let guid = match call.runtime.memory.read(GuestAddress(w[0]), 16) {
                        Ok(b) => b,
                        Err(e) => return call.memory_fault(e),
                    };
                    if guid != KEY_GUID {
                        return call.complete(DINPUT8_E_INVALIDARG.bits());
                    }
                }
                offsets.push((w[1], (w[2] >> 8) as u8));
            }
            call.runtime
                .direct_input_device_mut(object)
                .expect("live keyboard")
                .format = Some((words[3], offsets));
            call.complete(0)
        }
        13 => {
            if state.acquired {
                return call.complete(0x800700aa);
            }
            let flags = args[1];
            if flags & !0x1f != 0
                || !matches!(flags & 3, 1 | 2)
                || !matches!(flags & 12, 4 | 8)
                || (flags & 0x10 != 0 && flags & 0xf != 6)
            {
                return call.complete(DINPUT8_E_INVALIDARG.bits());
            }
            if flags & 1 != 0 {
                return call.fault("exclusive keyboard routing is not implemented");
            }
            let window = match call.runtime.window(args[0]) {
                Ok(w) => w,
                Err(_) => return call.complete(0x80070006),
            };
            if window.creation.owner_thread_id != call.runtime.current_thread_id()
                || window.creation.parent != 0
            {
                return call.complete(0x80070006);
            }
            let id = window.id;
            let s = call
                .runtime
                .direct_input_device_mut(object)
                .expect("live keyboard");
            s.window = Some((id, args[0]));
            s.cooperative_flags = flags;
            call.complete(0)
        }
        // The system keyboard does not require polling (DI_NOEFFECT).
        25 => call.complete(if state.acquired {
            1
        } else if state.lost {
            0x8007001e
        } else {
            0x8007000c
        }),
        _ => call.fault("DirectInput keyboard method is not implemented"),
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

    install_direct_input(
        endpoint,
        runtime,
        output_address,
        iid,
        direct_input8a_object_spec(),
    )
}

fn handle_direct_input_create(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let (Some(_instance), Some(version), Some(output), Some(outer)) = (
        frame.argument_u32(0),
        frame.argument_u32(1),
        frame.argument_u32(2),
        frame.argument_u32(3),
    ) else {
        return decoded_argument_missing();
    };
    let output = GuestAddress(output);
    if outer != 0 {
        return failed_factory(output, DINPUT8_CLASS_E_NOAGGREGATION);
    }
    if version < 0x0300 {
        return failed_factory(output, DINPUT8_DIERR_BETADIRECTINPUTVERSION);
    }
    if version > 0x0700 {
        return failed_factory(output, DINPUT8_DIERR_OLDDIRECTINPUTVERSION);
    }
    if !matches!(version, 0x0300 | 0x0500 | 0x0700) {
        return failed_factory(output, DINPUT8_E_INVALIDARG);
    }
    let mut spec = direct_input8a_object_spec();
    spec.interfaces[1].iid = DINPUT_IDIRECTINPUTA_IID;
    spec.interfaces[1].name = "IDirectInputA".to_owned();
    spec.interfaces[1].methods.truncate(8);
    install_direct_input(endpoint, runtime, output, DINPUT_IDIRECTINPUTA_IID, spec)
}

fn install_direct_input(
    endpoint: &ApiEndpointIdentity,
    runtime: &mut crate::runtime::Runtime,
    output_address: GuestAddress,
    iid: RuntimeComIid,
    spec: ComObjectSpec,
) -> ApiHandlerResult {
    match runtime.install_com_object_with_guest_writes(None, spec, |planned| {
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
    }) {
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
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return None;
    };
    match (endpoint.module.to_ascii_lowercase().as_str(), name.as_str()) {
        ("dinput8.dll", "DirectInput8Create") => Some(5),
        ("dinput.dll", "DirectInputCreateA") => Some(4),
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
                    ComMethodSpec::frontend("CreateDevice", 3),
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

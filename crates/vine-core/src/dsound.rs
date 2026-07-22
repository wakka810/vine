use crate::api::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiCallFrame, ApiContract, ApiDirection,
    ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiPointeeSchema,
    ApiPointerConstraint, ApiReturnSchema, ApiReturnValue, ApiSymbol, ApiValueSchema,
    ApiWriteRequirement, CallbackPolicy, CallingConvention, ExecutionSession, LastErrorPolicy,
    ReentrancyPolicy, StackCleanup,
};
use crate::com::{
    COM_S_OK, ComCallTrap, ComDispatchOutcome, ComFaultClass, ComFrontendCall, ComHResult,
    ComInterfaceSpec, ComMethodSpec, ComObjectSpec, ComReturnValue,
};
use crate::memory::GuestAddress;
use crate::runtime::{
    RuntimeAudioCooperativeLevel, RuntimeAudioCooperativeLevelRequest,
    RuntimeAudioDeviceCreateRequest, RuntimeAudioDeviceSelector, RuntimeAudioFrontend,
    RuntimeComIid, RuntimeComReturnKind, RuntimeStateError, RuntimeStateErrorClass,
};
use serde::Serialize;

pub const DSOUND_DIRECT_SOUND_CREATE_8_ORDINAL: u16 = 11;
pub const DSOUND_DIRECT_SOUND_CREATE_8_IMPLEMENTATION: &str = "dsound.direct-sound-create-8.v1";
pub const DSOUND_IDIRECTSOUND8_CLASS_IDENTITY: &str = "dsound.idirectsound8-root.v1";
pub const DSOUND_IDIRECTSOUND_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0x83, 0xfa, 0x9a, 0x27, 0x81, 0x49, 0xce, 0x11, 0xa5, 0x21, 0x00, 0x20, 0xaf, 0x0b, 0xe5, 0x60,
]);
pub const DSOUND_IDIRECTSOUND8_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0x93, 0x7e, 0x0a, 0xc5, 0x95, 0xf3, 0x34, 0x48, 0x9e, 0xf6, 0x7f, 0xa9, 0x9d, 0xe5, 0x09, 0x66,
]);
pub const DSOUND_CLASS_E_NOAGGREGATION: ComHResult = ComHResult(0x8004_0110_u32 as i32);
pub const DSOUND_E_INVALIDARG: ComHResult = ComHResult(0x8007_0057_u32 as i32);
pub const DSOUND_DSSCL_NORMAL: u32 = 1;
pub const DSOUND_DSSCL_PRIORITY: u32 = 2;
pub const DSOUND_DSSCL_EXCLUSIVE: u32 = 3;
pub const DSOUND_DSSCL_WRITEPRIMARY: u32 = 4;
pub const DSOUND_COMPATIBILITY_POLICY_SCHEMA: &str = "vine.dsound-compatibility-policy";
pub const DSOUND_COMPATIBILITY_POLICY_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DsoundCooperativeLevelPolicy {
    pub profile_id: String,
    pub level: u32,
    pub require_live_top_level_window: bool,
    pub require_window_owner_thread: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DsoundCompatibilityPolicy {
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub set_cooperative_level: Option<DsoundCooperativeLevelPolicy>,
}

impl DsoundCompatibilityPolicy {
    pub fn controlled_v1() -> Self {
        Self {
            schema: DSOUND_COMPATIBILITY_POLICY_SCHEMA.to_owned(),
            schema_version: DSOUND_COMPATIBILITY_POLICY_VERSION,
            id: "dsound-primary-default-device-v1".to_owned(),
            set_cooperative_level: Some(DsoundCooperativeLevelPolicy {
                profile_id: "primary-live-window-priority-v1".to_owned(),
                level: DSOUND_DSSCL_PRIORITY,
                require_live_top_level_window: true,
                require_window_owner_thread: true,
            }),
        }
    }

    pub fn validate(&self) -> Result<(), ApiFault> {
        if self.schema != DSOUND_COMPATIBILITY_POLICY_SCHEMA
            || self.schema_version != DSOUND_COMPATIBILITY_POLICY_VERSION
            || !valid_policy_id(&self.id)
        {
            return Err(ApiFault::runtime_defect(
                "dsound-compatibility-policy",
                "dsound-compatibility-policy-header-invalid",
                "DirectSound compatibility policy has an unsupported schema, version, or ID",
            ));
        }
        if let Some(cooperative) = self.set_cooperative_level.as_ref()
            && (!valid_policy_id(&cooperative.profile_id)
                || cooperative.level != DSOUND_DSSCL_PRIORITY
                || !cooperative.require_live_top_level_window
                || !cooperative.require_window_owner_thread)
        {
            return Err(ApiFault::runtime_defect(
                "dsound-compatibility-policy",
                "dsound-cooperative-level-profile-invalid",
                "DirectSound cooperative policy must describe the evidenced live top-level priority relation",
            ));
        }
        Ok(())
    }
}

fn valid_policy_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

pub fn dispatch_dsound_api(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    match contract.implementation_id.as_str() {
        DSOUND_DIRECT_SOUND_CREATE_8_IMPLEMENTATION => {
            handle_direct_sound_create_8(contract, frame, session)
        }
        implementation_id => ApiHandlerResult::Fault(ApiFault::unsupported(
            "dsound-api-dispatch",
            "implementation-id-unsupported",
            format!("DirectSound implementation {implementation_id} is not registered"),
        )),
    }
}

pub fn dispatch_dsound_com_call(
    policy: &DsoundCompatibilityPolicy,
    trap: ComCallTrap,
    session: &mut ExecutionSession<'_>,
) -> ComDispatchOutcome {
    session
        .dispatch_com_call_with_frontend(trap, |call| dispatch_dsound_frontend_call(policy, call))
}

fn dispatch_dsound_frontend_call(
    policy: &DsoundCompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    if call.class_identity() != DSOUND_IDIRECTSOUND8_CLASS_IDENTITY
        || call.frame().iid != DSOUND_IDIRECTSOUND8_IID
    {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "dsound-com-dispatch",
            "dsound-com-target-mismatch",
            "DirectSound frontend received a COM object outside its owned root interface",
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
    if call.frame().vtable_slot != 6
        || call.frame().method != "SetCooperativeLevel"
        || call.explicit_argument_words() != 2
        || call.frame().explicit_arguments.len() != 2
        || call.return_kind() != RuntimeComReturnKind::HResult
    {
        return call.fault(
            ComFaultClass::RuntimeDefect,
            "dsound-com-dispatch",
            "dsound-com-abi-mismatch",
            "registered IDirectSound8 frontend ABI does not match SetCooperativeLevel",
        );
    }
    handle_set_cooperative_level(policy, call)
}

fn handle_set_cooperative_level(
    policy: &DsoundCompatibilityPolicy,
    call: ComFrontendCall<'_>,
) -> ComDispatchOutcome {
    let [window_handle, level] = call.frame().explicit_arguments.as_slice() else {
        unreachable!("frontend ABI preflight fixed the DirectSound argument count");
    };
    let (window_handle, level) = (*window_handle, *level);
    if window_handle == 0 || !(DSOUND_DSSCL_NORMAL..=DSOUND_DSSCL_WRITEPRIMARY).contains(&level) {
        return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
    }
    let Some(cooperative) = policy.set_cooperative_level.as_ref() else {
        return call.fault(
            ComFaultClass::Unsupported,
            "dsound-set-cooperative-level",
            "dsound-cooperative-level-policy-disabled",
            "DirectSound cooperative-level support is disabled by the active policy",
        );
    };
    if level != cooperative.level {
        return call.fault(
            ComFaultClass::Unsupported,
            "dsound-set-cooperative-level",
            "dsound-cooperative-level-variant-unsupported",
            "DirectSound cooperative level is outside the evidenced priority policy",
        );
    }

    let plan =
        match call
            .runtime()
            .plan_audio_cooperative_level(RuntimeAudioCooperativeLevelRequest {
                device_com_object_id: call.frame().object_id,
                window_handle,
                level: RuntimeAudioCooperativeLevel::Priority,
            }) {
            Ok(plan) => plan,
            Err(error) if is_invalid_window_relation(&error) => {
                return call.complete(ComReturnValue::HResult(DSOUND_E_INVALIDARG), Vec::new());
            }
            Err(error) => return call.runtime_fault(error),
        };
    let mut call = call;
    if let Err(error) = call.runtime_mut().commit_audio_cooperative_level(plan) {
        return call.runtime_fault(error);
    }
    call.complete(ComReturnValue::HResult(COM_S_OK), Vec::new())
}

fn is_invalid_window_relation(error: &RuntimeStateError) -> bool {
    matches!(
        (error.class, error.code),
        (
            RuntimeStateErrorClass::InvalidInput
                | RuntimeStateErrorClass::NotFound
                | RuntimeStateErrorClass::Conflict,
            "audio-window-handle-unknown"
                | "audio-window-lifecycle-state-invalid"
                | "audio-window-not-top-level"
                | "audio-window-thread-mismatch"
        )
    )
}

fn handle_direct_sound_create_8(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    let Some(device_guid) = frame.argument_u32(0) else {
        return decoded_argument_missing();
    };
    let Some(output_pointer) = frame.argument_u32(1) else {
        return decoded_argument_missing();
    };
    let Some(outer_unknown) = frame.argument_u32(2) else {
        return decoded_argument_missing();
    };

    if output_pointer == 0 {
        return immediate_hresult(DSOUND_E_INVALIDARG);
    }
    if outer_unknown != 0 {
        return immediate_hresult(DSOUND_CLASS_E_NOAGGREGATION);
    }
    if device_guid != 0 {
        return ApiHandlerResult::Fault(ApiFault::unsupported(
            "dsound-direct-sound-create-8",
            "dsound-device-guid-unsupported",
            "DirectSound device GUID selection is outside the evidenced default-device band",
        ));
    }

    let output_address = GuestAddress(output_pointer);
    let installed = match session.install_audio_device_com_object_with_guest_writes(
        direct_sound8_object_spec(),
        RuntimeAudioDeviceCreateRequest {
            frontend: RuntimeAudioFrontend::DirectSound8,
            selector: RuntimeAudioDeviceSelector::DefaultPlayback,
        },
        |planned| {
            let interface = planned
                .interfaces
                .iter()
                .find(|interface| interface.iid == DSOUND_IDIRECTSOUND8_IID)
                .ok_or_else(|| {
                    RuntimeStateError::runtime_defect(
                        "dsound-api-dispatch",
                        "requested-interface-installation-missing",
                        "installed DirectSound root object omitted IDirectSound8",
                    )
                })?;
            Ok(vec![(
                output_address,
                interface.guest_pointer.0.to_le_bytes().to_vec(),
            )])
        },
    ) {
        Ok((installed, device, _guest_writes)) => {
            debug_assert_eq!(device.com_object_id, installed.object.id);
            installed
        }
        Err(error) => {
            return ApiHandlerResult::Fault(ApiFault::from_runtime(error, &contract.endpoint));
        }
    };
    let interface = installed
        .interfaces
        .iter()
        .find(|interface| interface.iid == DSOUND_IDIRECTSOUND8_IID)
        .expect("atomic DirectSound installation validated IDirectSound8");
    ApiHandlerResult::Immediate(
        ApiImmediate::new(ApiReturnValue::HResult(COM_S_OK)).with_guest_write(
            output_address,
            interface.guest_pointer.0.to_le_bytes().to_vec(),
        ),
    )
}

fn decoded_argument_missing() -> ApiHandlerResult {
    ApiHandlerResult::Fault(ApiFault::runtime_defect(
        "dsound-api-dispatch",
        "decoded-argument-missing",
        "decoded DirectSoundCreate8 frame is missing an argument",
    ))
}

fn immediate_hresult(result: ComHResult) -> ApiHandlerResult {
    ApiHandlerResult::Immediate(ApiImmediate::new(ApiReturnValue::HResult(result)))
}

pub fn dsound_contract_for_endpoint(
    endpoint: ApiEndpointIdentity,
) -> Result<Option<ApiContract>, ApiFault> {
    if !endpoint.module.eq_ignore_ascii_case("dsound.dll") {
        return Ok(None);
    }
    match endpoint.symbol {
        ApiSymbol::Ordinal {
            ordinal: DSOUND_DIRECT_SOUND_CREATE_8_ORDINAL,
        } => Ok(Some(direct_sound_create_8_contract(endpoint)?)),
        _ => Ok(None),
    }
}

pub fn direct_sound_create_8_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                optional_input_struct("deviceGuid", "GUID", 16),
                optional_output_struct("interface", "COM_INTERFACE_POINTER", 4),
                opaque_pointer("outerUnknown", true),
            ],
            return_value: ApiReturnSchema::HResult,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Disallowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: DSOUND_DIRECT_SOUND_CREATE_8_IMPLEMENTATION.to_owned(),
    })
}

pub fn direct_sound8_object_spec() -> ComObjectSpec {
    ComObjectSpec {
        class_identity: DSOUND_IDIRECTSOUND8_CLASS_IDENTITY.to_owned(),
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            ComInterfaceSpec {
                iid: DSOUND_IDIRECTSOUND8_IID,
                name: "IDirectSound8".to_owned(),
                methods: vec![
                    ComMethodSpec::query_interface(),
                    ComMethodSpec::add_ref(),
                    ComMethodSpec::release(),
                    ComMethodSpec::unsupported_hresult("CreateSoundBuffer", 3),
                    ComMethodSpec::unsupported_hresult("GetCaps", 1),
                    ComMethodSpec::unsupported_hresult("DuplicateSoundBuffer", 2),
                    ComMethodSpec::frontend_hresult("SetCooperativeLevel", 2),
                    ComMethodSpec::unsupported_hresult("Compact", 0),
                    ComMethodSpec::unsupported_hresult("GetSpeakerConfig", 1),
                    ComMethodSpec::unsupported_hresult("SetSpeakerConfig", 1),
                    ComMethodSpec::unsupported_hresult("Initialize", 1),
                    ComMethodSpec::unsupported_hresult("VerifyCertification", 1),
                ],
            },
        ],
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

fn optional_input_struct(name: &str, type_id: &str, byte_length: u32) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::Pointer {
            constraint: ApiPointerConstraint {
                nullable: true,
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

fn optional_output_struct(name: &str, type_id: &str, byte_length: u32) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::Out,
        value: ApiValueSchema::Pointer {
            constraint: ApiPointerConstraint {
                nullable: true,
                alignment: 4,
                byte_length,
                pointee: ApiPointeeSchema::Struct {
                    type_id: type_id.to_owned(),
                },
                write_requirement: ApiWriteRequirement::Optional,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_policy_rejects_unmeasured_cooperative_claims() {
        let mut policy = DsoundCompatibilityPolicy::controlled_v1();
        policy
            .set_cooperative_level
            .as_mut()
            .expect("cooperative policy")
            .level = DSOUND_DSSCL_NORMAL;
        assert_eq!(
            policy.validate().unwrap_err().code,
            "dsound-cooperative-level-profile-invalid"
        );

        let mut header = DsoundCompatibilityPolicy::controlled_v1();
        header.schema_version += 1;
        assert_eq!(
            header.validate().unwrap_err().code,
            "dsound-compatibility-policy-header-invalid"
        );
    }

    #[test]
    fn root_spec_keeps_legacy_alias_outside_the_reached_band() {
        let spec = direct_sound8_object_spec();
        assert_eq!(spec.interfaces.len(), 2);
        assert_eq!(
            spec.interfaces[0].iid,
            crate::runtime::RUNTIME_COM_IUNKNOWN_IID
        );
        assert_eq!(spec.interfaces[1].iid, DSOUND_IDIRECTSOUND8_IID);
        assert!(
            spec.interfaces
                .iter()
                .all(|interface| interface.iid != DSOUND_IDIRECTSOUND_IID)
        );
        assert_eq!(spec.interfaces[1].methods.len(), 12);
        assert_eq!(spec.interfaces[1].methods[6].name, "SetCooperativeLevel");
    }

    #[test]
    fn native_hresult_constants_are_fixed() {
        assert_eq!(DSOUND_CLASS_E_NOAGGREGATION.bits(), 0x8004_0110);
        assert_eq!(DSOUND_E_INVALIDARG.bits(), 0x8007_0057);
        assert_eq!(crate::com::COM_E_NOINTERFACE.bits(), 0x8000_4002);
    }
}

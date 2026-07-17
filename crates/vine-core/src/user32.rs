use serde::Serialize;

use crate::api::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiCallFrame, ApiContract, ApiDirection,
    ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiPointeeSchema,
    ApiPointerConstraint, ApiReturnSchema, ApiReturnValue, ApiSymbol, ApiValueSchema,
    ApiWriteRequirement, CallbackPolicy, CallingConvention, ExecutionSession,
    GuestCallbackArgument, GuestCallbackCompletionPolicy, GuestCallbackRequest, LastErrorPolicy,
    ReentrancyPolicy, StackCleanup,
};
use crate::memory::{GuestAddress, MemoryAccess};
use crate::pe32::{ResourceDataEntryPlan, ResourceIdentifier};
use crate::runtime::{
    RUNTIME_SYSTEM_CURSOR_ARROW_RESOURCE_ID, RuntimeCursorSource, RuntimeDisplayProfile,
    RuntimeIconResource, RuntimeInputProfile, RuntimeRect, RuntimeSystemCursorKind,
};

pub const USER32_COMPATIBILITY_POLICY_SCHEMA: &str = "vine.user32-compatibility-policy";
pub const USER32_COMPATIBILITY_POLICY_VERSION: u32 = 4;
pub const ENUM_DISPLAY_MONITORS_IMPLEMENTATION: &str = "user32.enum-display-monitors.v1";
pub const SWAP_MOUSE_BUTTON_IMPLEMENTATION: &str = "user32.swap-mouse-button.v1";
pub const LOAD_ICON_A_IMPLEMENTATION: &str = "user32.load-icon-a.v1";
pub const LOAD_CURSOR_A_IMPLEMENTATION: &str = "user32.load-cursor-a.v1";

const RT_ICON: u32 = 3;
const RT_GROUP_ICON: u32 = 14;
const ERROR_SUCCESS: u32 = 0;
const ERROR_INVALID_HANDLE: u32 = 6;
const ERROR_RESOURCE_DATA_NOT_FOUND: u32 = 1812;
const ERROR_RESOURCE_TYPE_NOT_FOUND: u32 = 1813;
const ERROR_RESOURCE_NAME_NOT_FOUND: u32 = 1814;
const ERROR_RESOURCE_LANG_NOT_FOUND: u32 = 1815;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32LoadIconAPolicy {
    pub profile_id: String,
    pub target_width: u16,
    pub target_height: u16,
    pub maximum_group_entries: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32LoadCursorAPolicy {
    pub profile_id: String,
    pub arrow_resource_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32CompatibilityPolicy {
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub enum_display_monitors: Option<RuntimeDisplayProfile>,
    pub swap_mouse_button: Option<RuntimeInputProfile>,
    pub load_icon_a: Option<User32LoadIconAPolicy>,
    pub load_cursor_a: Option<User32LoadCursorAPolicy>,
}

impl User32CompatibilityPolicy {
    pub fn controlled_v4() -> Self {
        Self {
            schema: USER32_COMPATIBILITY_POLICY_SCHEMA.to_owned(),
            schema_version: USER32_COMPATIBILITY_POLICY_VERSION,
            id: "user32-single-monitor-normal-mouse-icon-cursor-v4".to_owned(),
            enum_display_monitors: Some(RuntimeDisplayProfile {
                id: "single-monitor-1920x1080-v1".to_owned(),
                primary_monitor_rect: RuntimeRect {
                    left: 0,
                    top: 0,
                    right: 1920,
                    bottom: 1080,
                },
            }),
            swap_mouse_button: Some(RuntimeInputProfile {
                id: "normal-mouse-buttons-v1".to_owned(),
                initial_mouse_buttons_swapped: false,
            }),
            load_icon_a: Some(User32LoadIconAPolicy {
                profile_id: "module-shared-large-icon-32x32-v1".to_owned(),
                target_width: 32,
                target_height: 32,
                maximum_group_entries: 64,
            }),
            load_cursor_a: Some(User32LoadCursorAPolicy {
                profile_id: "system-arrow-shared-cursor-v1".to_owned(),
                arrow_resource_id: RUNTIME_SYSTEM_CURSOR_ARROW_RESOURCE_ID,
            }),
        }
    }

    pub fn validate(&self) -> Result<(), ApiFault> {
        if self.schema != USER32_COMPATIBILITY_POLICY_SCHEMA
            || self.schema_version != USER32_COMPATIBILITY_POLICY_VERSION
            || self.id.is_empty()
            || self.id.len() > 128
            || !self.id.is_ascii()
            || self.id.as_bytes().contains(&0)
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-policy-identity-invalid",
                "User32 compatibility policy identity or schema is invalid",
            ));
        }
        if let Some(profile) = self.enum_display_monitors.as_ref() {
            let rect = profile.primary_monitor_rect;
            if profile.id.is_empty()
                || profile.id.len() > 128
                || !profile.id.is_ascii()
                || profile.id.as_bytes().contains(&0)
                || rect.left != 0
                || rect.top != 0
                || rect.right <= 0
                || rect.bottom <= 0
                || rect.right > 32_768
                || rect.bottom > 32_768
            {
                return Err(ApiFault::runtime_defect(
                    "user32-compatibility-policy",
                    "user32-monitor-profile-invalid",
                    "User32 single-monitor profile requires a bounded positive rectangle at the virtual origin",
                ));
            }
        }
        if let Some(profile) = self.swap_mouse_button.as_ref()
            && (profile.id.is_empty()
                || profile.id.len() > 128
                || !profile.id.is_ascii()
                || profile.id.as_bytes().contains(&0))
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-input-profile-invalid",
                "User32 logical input profile requires a bounded nonempty ASCII identity",
            ));
        }
        if let Some(profile) = self.load_icon_a.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.target_width == 0
                || profile.target_width > 256
                || profile.target_height == 0
                || profile.target_height > 256
                || profile.maximum_group_entries == 0
                || profile.maximum_group_entries > 256)
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-load-icon-profile-invalid",
                "User32 module icon profile requires bounded target dimensions and group entries",
            ));
        }
        if let Some(profile) = self.load_cursor_a.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.arrow_resource_id != RUNTIME_SYSTEM_CURSOR_ARROW_RESOURCE_ID)
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-load-cursor-profile-invalid",
                "User32 system cursor profile requires the predefined arrow identity",
            ));
        }
        Ok(())
    }

    pub fn display_profile(&self) -> Option<RuntimeDisplayProfile> {
        self.enum_display_monitors.clone()
    }

    pub fn input_profile(&self) -> Option<RuntimeInputProfile> {
        self.swap_mouse_button.clone()
    }
}

pub fn dispatch_user32_api_with_policy(
    policy: &User32CompatibilityPolicy,
    callback_return_sentinel: GuestAddress,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    if let Err(fault) = policy.validate() {
        return ApiHandlerResult::Fault(fault);
    }
    match contract.implementation_id.as_str() {
        ENUM_DISPLAY_MONITORS_IMPLEMENTATION => {
            handle_enum_display_monitors(policy, callback_return_sentinel, contract, frame, session)
        }
        SWAP_MOUSE_BUTTON_IMPLEMENTATION => {
            match handle_swap_mouse_button(policy, contract, frame, session) {
                Ok(immediate) => ApiHandlerResult::Immediate(immediate),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        LOAD_ICON_A_IMPLEMENTATION => match handle_load_icon_a(policy, contract, frame, session) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        LOAD_CURSOR_A_IMPLEMENTATION => {
            match handle_load_cursor_a(policy, contract, frame, session) {
                Ok(immediate) => ApiHandlerResult::Immediate(immediate),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        implementation_id => ApiHandlerResult::Fault(ApiFault::unsupported(
            "user32-api-dispatch",
            "implementation-id-unsupported",
            format!("User32 implementation {implementation_id} is not registered"),
        )),
    }
}

fn handle_swap_mouse_button(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let profile = policy.swap_mouse_button.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-input",
            "swap-mouse-button-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical mouse button mapping",
                policy.id
            ),
        )
    })?;
    let swapped = required_argument(contract, frame, 0, "mouse button swap flag")? != 0;
    let mapping = session.runtime().mouse_button_mapping().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-input",
            "logical-mouse-button-profile-missing",
            "User32 policy is enabled but the Rust input subsystem has no mouse button profile",
        )
    })?;
    if mapping.profile_id != profile.id
        || mapping.initial_swapped != profile.initial_mouse_buttons_swapped
    {
        return Err(ApiFault::runtime_defect(
            "user32-input",
            "logical-mouse-button-profile-mismatch",
            "Rust input authority does not match the selected User32 compatibility profile",
        ));
    }
    let result = session
        .runtime_mut()
        .swap_mouse_buttons(swapped)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(
        result.previous_swapped,
    )))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SelectedGroupIcon {
    resource_id: u32,
    bytes_in_resource: u32,
    width: u16,
    height: u16,
    planes: u16,
    bits_per_pixel: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupIconSelectionError {
    Malformed,
    ExactSizeUnavailable,
}

fn handle_load_icon_a(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let icon_policy = policy.load_icon_a.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-icons",
            "load-icon-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable module icon loading",
                policy.id
            ),
        )
    })?;
    let instance = required_argument(contract, frame, 0, "module instance")?;
    let icon_name = required_argument(contract, frame, 1, "icon resource name")?;
    if instance == 0 {
        return Err(ApiFault::unsupported(
            "user32-icons",
            "load-icon-a-system-resource-unsupported",
            "LoadIconA currently supports registered module resources, not predefined system icons",
        ));
    }
    if icon_name > u32::from(u16::MAX) {
        return Err(ApiFault::unsupported(
            "user32-icons",
            "load-icon-a-string-resource-unsupported",
            "LoadIconA currently supports only integer module resource identifiers",
        ));
    }

    let image = match session
        .runtime()
        .module_image_by_load_base(GuestAddress(instance))
    {
        Ok(image) => image,
        Err(error) if error.code == "module-image-load-base-unknown" => {
            return Ok(load_icon_failure(ERROR_INVALID_HANDLE));
        }
        Err(error) => {
            return Err(ApiFault::runtime_defect(
                "user32-icons",
                error.code,
                error.message,
            ));
        }
    };
    let Some(resources) = image.resources.as_ref() else {
        return Ok(load_icon_failure(ERROR_RESOURCE_TYPE_NOT_FOUND));
    };
    let group_candidates = resources
        .integer_entries(RT_GROUP_ICON, icon_name)
        .filter_map(|entry| integer_language_id(entry).map(|language| (entry, language)))
        .collect::<Vec<_>>();
    let [(group_resource, language_id)] = group_candidates.as_slice() else {
        return Ok(load_icon_failure(if group_candidates.is_empty() {
            ERROR_RESOURCE_TYPE_NOT_FOUND
        } else {
            ERROR_RESOURCE_LANG_NOT_FOUND
        }));
    };
    let group_limit = 6_u32 + 14_u32 * u32::from(icon_policy.maximum_group_entries);
    if group_resource.size > group_limit {
        return Ok(load_icon_failure(ERROR_RESOURCE_DATA_NOT_FOUND));
    }
    let group_address = resource_address(instance, group_resource.data_rva.0, contract)?;
    let group_bytes = session
        .memory()
        .read(group_address, group_resource.size as usize)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let selected = match select_group_icon(&group_bytes, icon_policy) {
        Ok(selected) => selected,
        Err(GroupIconSelectionError::Malformed) => {
            return Ok(load_icon_failure(ERROR_RESOURCE_DATA_NOT_FOUND));
        }
        Err(GroupIconSelectionError::ExactSizeUnavailable) => {
            return Err(ApiFault::unsupported(
                "user32-icons",
                "load-icon-a-size-selection-unsupported",
                "module group icon has no exact entry for the controlled logical large-icon size",
            ));
        }
    };
    let icon_candidates = resources
        .integer_entries(RT_ICON, selected.resource_id)
        .filter(|entry| integer_language_id(entry) == Some(*language_id))
        .collect::<Vec<_>>();
    let [icon_resource] = icon_candidates.as_slice() else {
        return Ok(load_icon_failure(if icon_candidates.is_empty() {
            ERROR_RESOURCE_TYPE_NOT_FOUND
        } else {
            ERROR_RESOURCE_LANG_NOT_FOUND
        }));
    };
    if icon_resource.size != selected.bytes_in_resource || icon_resource.size < 40 {
        return Ok(load_icon_failure(ERROR_RESOURCE_DATA_NOT_FOUND));
    }
    let icon_address = resource_address(instance, icon_resource.data_rva.0, contract)?;
    let icon_header = session
        .memory()
        .read(icon_address, 40)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    if icon_header.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Err(ApiFault::unsupported(
            "user32-icons",
            "load-icon-a-selected-png-unsupported",
            "selected module icon uses PNG payload; the reached vertical slice validates DIB icons",
        ));
    }
    if !validate_selected_icon_dib(&icon_header, &selected, icon_resource.size) {
        return Ok(load_icon_failure(ERROR_RESOURCE_DATA_NOT_FOUND));
    }

    let icon_resource = RuntimeIconResource {
        module_id: image.module_id,
        module_load_base: image.load_base,
        module_source_sha256: image.source.sha256.clone(),
        group_resource_id: icon_name,
        language_id: *language_id,
        icon_resource_id: selected.resource_id,
        group_data_rva: group_resource.data_rva,
        group_data_size: group_resource.size,
        icon_data_rva: icon_resource.data_rva,
        icon_data_size: icon_resource.size,
        width: selected.width,
        height: selected.height,
        planes: selected.planes,
        bits_per_pixel: selected.bits_per_pixel,
    };
    let result = session
        .runtime_mut()
        .load_shared_icon(icon_resource)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::U32(result.handle)).with_last_error(ERROR_SUCCESS))
}

fn handle_load_cursor_a(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let cursor_policy = policy.load_cursor_a.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-cursors",
            "load-cursor-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable system cursor loading",
                policy.id
            ),
        )
    })?;
    let instance = required_argument(contract, frame, 0, "module instance")?;
    let cursor_name = required_argument(contract, frame, 1, "cursor resource name")?;
    if instance != 0 {
        return Err(ApiFault::unsupported(
            "user32-cursors",
            "load-cursor-a-module-resource-unsupported",
            "LoadCursorA currently supports the predefined system arrow, not module cursors",
        ));
    }
    if cursor_name > u32::from(u16::MAX) {
        return Err(ApiFault::unsupported(
            "user32-cursors",
            "load-cursor-a-string-resource-unsupported",
            "LoadCursorA currently supports only a predefined integer system cursor",
        ));
    }
    if cursor_name != cursor_policy.arrow_resource_id {
        if cursor_name == u32::from(u16::MAX) {
            return Ok(load_cursor_failure(ERROR_RESOURCE_NAME_NOT_FOUND));
        }
        return Err(ApiFault::unsupported(
            "user32-cursors",
            "load-cursor-a-predefined-variant-unsupported",
            "LoadCursorA currently supports only IDC_ARROW from the controlled policy",
        ));
    }

    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let result = session
        .runtime_mut()
        .load_shared_system_cursor(RuntimeCursorSource::SystemPredefined {
            resource_id: cursor_name,
            cursor: RuntimeSystemCursorKind::Arrow,
        })
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::U32(result.handle)).with_last_error(preserved_last_error))
}

fn integer_language_id(entry: &ResourceDataEntryPlan) -> Option<u32> {
    match &entry.language {
        ResourceIdentifier::Id { id } => Some(*id),
        ResourceIdentifier::Name { .. } => None,
    }
}

fn resource_address(
    load_base: u32,
    rva: u32,
    _contract: &ApiContract,
) -> Result<GuestAddress, ApiFault> {
    load_base.checked_add(rva).map(GuestAddress).ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-icons",
            "module-resource-address-overflow",
            "registered module load base plus resource RVA overflowed x86 accounting",
        )
    })
}

fn select_group_icon(
    bytes: &[u8],
    policy: &User32LoadIconAPolicy,
) -> Result<SelectedGroupIcon, GroupIconSelectionError> {
    if bytes.len() < 6 || read_u16(bytes, 0) != Some(0) || read_u16(bytes, 2) != Some(1) {
        return Err(GroupIconSelectionError::Malformed);
    }
    let count = read_u16(bytes, 4).ok_or(GroupIconSelectionError::Malformed)?;
    if count == 0 || count > policy.maximum_group_entries {
        return Err(GroupIconSelectionError::Malformed);
    }
    let expected_size = 6_usize
        .checked_add(usize::from(count) * 14)
        .ok_or(GroupIconSelectionError::Malformed)?;
    if bytes.len() != expected_size {
        return Err(GroupIconSelectionError::Malformed);
    }
    let mut selected = None;
    for index in 0..usize::from(count) {
        let offset = 6 + index * 14;
        let width = match bytes[offset] {
            0 => 256,
            value => u16::from(value),
        };
        let height = match bytes[offset + 1] {
            0 => 256,
            value => u16::from(value),
        };
        let planes = read_u16(bytes, offset + 4).ok_or(GroupIconSelectionError::Malformed)?;
        let bits_per_pixel =
            read_u16(bytes, offset + 6).ok_or(GroupIconSelectionError::Malformed)?;
        let bytes_in_resource =
            read_u32(bytes, offset + 8).ok_or(GroupIconSelectionError::Malformed)?;
        let resource_id =
            u32::from(read_u16(bytes, offset + 12).ok_or(GroupIconSelectionError::Malformed)?);
        if bytes[offset + 3] != 0
            || planes == 0
            || bits_per_pixel == 0
            || bytes_in_resource == 0
            || resource_id == 0
        {
            return Err(GroupIconSelectionError::Malformed);
        }
        if width != policy.target_width || height != policy.target_height {
            continue;
        }
        let candidate = SelectedGroupIcon {
            resource_id,
            bytes_in_resource,
            width,
            height,
            planes,
            bits_per_pixel,
        };
        if selected.as_ref().is_none_or(|current: &SelectedGroupIcon| {
            candidate.bits_per_pixel > current.bits_per_pixel
        }) {
            selected = Some(candidate);
        }
    }
    selected.ok_or(GroupIconSelectionError::ExactSizeUnavailable)
}

fn validate_selected_icon_dib(header: &[u8], selected: &SelectedGroupIcon, data_size: u32) -> bool {
    let Some(header_size) = read_u32(header, 0) else {
        return false;
    };
    let Some(width) = read_i32(header, 4) else {
        return false;
    };
    let Some(height) = read_i32(header, 8) else {
        return false;
    };
    header_size >= 40
        && header_size <= data_size
        && width == i32::from(selected.width)
        && height == i32::from(selected.height) * 2
        && read_u16(header, 12) == Some(selected.planes)
        && read_u16(header, 14) == Some(selected.bits_per_pixel)
}

fn load_icon_failure(last_error: u32) -> ApiImmediate {
    ApiImmediate::new(ApiReturnValue::U32(0)).with_last_error(last_error)
}

fn load_cursor_failure(last_error: u32) -> ApiImmediate {
    ApiImmediate::new(ApiReturnValue::U32(0)).with_last_error(last_error)
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn read_i32(bytes: &[u8], offset: usize) -> Option<i32> {
    Some(i32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn handle_enum_display_monitors(
    policy: &User32CompatibilityPolicy,
    callback_return_sentinel: GuestAddress,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> ApiHandlerResult {
    let Some(profile) = policy.enum_display_monitors.as_ref() else {
        return ApiHandlerResult::Fault(ApiFault::unsupported(
            "user32-display",
            "enum-display-monitors-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical monitor enumeration",
                policy.id
            ),
        ));
    };
    let hdc = match required_argument(contract, frame, 0, "HDC") {
        Ok(value) => value,
        Err(fault) => return ApiHandlerResult::Fault(fault),
    };
    let clip = match required_argument(contract, frame, 1, "clip rectangle") {
        Ok(value) => value,
        Err(fault) => return ApiHandlerResult::Fault(fault),
    };
    let callback = match required_argument(contract, frame, 2, "monitor callback") {
        Ok(value) => value,
        Err(fault) => return ApiHandlerResult::Fault(fault),
    };
    let data = match required_argument(contract, frame, 3, "callback data") {
        Ok(value) => value,
        Err(fault) => return ApiHandlerResult::Fault(fault),
    };
    if hdc != 0 || clip != 0 {
        return ApiHandlerResult::Fault(ApiFault::unsupported(
            "user32-display",
            "enum-display-monitors-variant-unsupported",
            "EnumDisplayMonitors currently supports only NULL HDC and NULL clip rectangle",
        ));
    }
    if let Err(error) =
        session
            .memory()
            .validate_access(GuestAddress(callback), 1, MemoryAccess::Execute)
    {
        return ApiHandlerResult::Fault(ApiFault::from_memory(error, &contract.endpoint));
    }
    let Some(monitor) = session.runtime().primary_monitor() else {
        return ApiHandlerResult::Fault(ApiFault::runtime_defect(
            "user32-display",
            "logical-monitor-profile-missing",
            "User32 policy is enabled but the Rust display subsystem has no logical monitor",
        ));
    };
    if session.runtime().display_profile_id() != Some(profile.id.as_str())
        || monitor.rect != profile.primary_monitor_rect
    {
        return ApiHandlerResult::Fault(ApiFault::runtime_defect(
            "user32-display",
            "logical-monitor-profile-mismatch",
            "Rust display authority does not match the selected User32 compatibility profile",
        ));
    }

    ApiHandlerResult::GuestCallback(GuestCallbackRequest {
        target: GuestAddress(callback),
        arguments: vec![
            GuestCallbackArgument::U32(monitor.handle),
            GuestCallbackArgument::U32(0),
            GuestCallbackArgument::ScratchPointer { offset: 0 },
            GuestCallbackArgument::U32(data),
        ],
        scratch: monitor.rect.to_x86_bytes(),
        return_sentinel: callback_return_sentinel,
        completion_policy: GuestCallbackCompletionPolicy::ReturnBool32,
    })
}

pub fn user32_contract_for_endpoint(
    endpoint: ApiEndpointIdentity,
) -> Result<Option<ApiContract>, ApiFault> {
    if !endpoint.module.eq_ignore_ascii_case("user32.dll") {
        return Ok(None);
    }
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return Ok(None);
    };
    match name.as_str() {
        "EnumDisplayMonitors" => Ok(Some(enum_display_monitors_contract(endpoint)?)),
        "SwapMouseButton" => Ok(Some(swap_mouse_button_contract(endpoint)?)),
        "LoadIconA" => Ok(Some(load_icon_a_contract(endpoint)?)),
        "LoadCursorA" => Ok(Some(load_cursor_a_contract(endpoint)?)),
        _ => Ok(None),
    }
}

pub fn swap_mouse_button_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![u32_argument("swap")],
            return_value: ApiReturnSchema::Bool32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: SWAP_MOUSE_BUTTON_IMPLEMENTATION.to_owned(),
    })
}

pub fn enum_display_monitors_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    let contract = ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                u32_argument("hdc"),
                optional_in_struct("clipRect", "RECT", 16),
                opaque_pointer("monitorCallback", false, 1),
                u32_argument("data"),
            ],
            return_value: ApiReturnSchema::Bool32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Required,
        implementation_id: ENUM_DISPLAY_MONITORS_IMPLEMENTATION.to_owned(),
    };
    Ok(contract)
}

pub fn load_icon_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![u32_argument("instance"), u32_argument("iconName")],
            return_value: ApiReturnSchema::U32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: LOAD_ICON_A_IMPLEMENTATION.to_owned(),
    })
}

pub fn load_cursor_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![u32_argument("instance"), u32_argument("cursorName")],
            return_value: ApiReturnSchema::U32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: LOAD_CURSOR_A_IMPLEMENTATION.to_owned(),
    })
}

fn required_argument(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    index: usize,
    label: &str,
) -> Result<u32, ApiFault> {
    frame.argument_u32(index).ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-api-dispatch",
            "decoded-argument-missing",
            format!(
                "decoded {} frame is missing {label} argument {index}",
                contract.implementation_id
            ),
        )
    })
}

fn u32_argument(name: &str) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::U32,
    }
}

fn valid_profile_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && value.is_ascii() && !value.as_bytes().contains(&0)
}

fn opaque_pointer(name: &str, nullable: bool, alignment: u32) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::OpaquePointer {
            nullable,
            alignment,
        },
    }
}

fn optional_in_struct(name: &str, type_id: &str, byte_length: u32) -> ApiArgumentSchema {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controlled_policy_is_host_independent_and_bounded() {
        let policy = User32CompatibilityPolicy::controlled_v4();
        policy.validate().unwrap();
        let profile = policy.display_profile().unwrap();
        assert_eq!(profile.primary_monitor_rect.right, 1920);
        assert_eq!(profile.primary_monitor_rect.bottom, 1080);

        let mut invalid = policy;
        invalid
            .enum_display_monitors
            .as_mut()
            .unwrap()
            .primary_monitor_rect
            .right = 0;
        assert_eq!(
            invalid.validate().unwrap_err().code,
            "user32-monitor-profile-invalid"
        );

        let mut invalid_input = User32CompatibilityPolicy::controlled_v4();
        invalid_input.swap_mouse_button.as_mut().unwrap().id.clear();
        assert_eq!(
            invalid_input.validate().unwrap_err().code,
            "user32-input-profile-invalid"
        );

        let mut invalid_icon = User32CompatibilityPolicy::controlled_v4();
        invalid_icon
            .load_icon_a
            .as_mut()
            .unwrap()
            .maximum_group_entries = 0;
        assert_eq!(
            invalid_icon.validate().unwrap_err().code,
            "user32-load-icon-profile-invalid"
        );

        let mut invalid_cursor = User32CompatibilityPolicy::controlled_v4();
        invalid_cursor
            .load_cursor_a
            .as_mut()
            .unwrap()
            .arrow_resource_id = 0x7f01;
        assert_eq!(
            invalid_cursor.validate().unwrap_err().code,
            "user32-load-cursor-profile-invalid"
        );
    }
}

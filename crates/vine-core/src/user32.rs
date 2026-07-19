use serde::Serialize;

use crate::api::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiCallFrame, ApiContract, ApiDirection,
    ApiEndpointIdentity, ApiFault, ApiGuestCallbackCall, ApiHandlerResult, ApiImmediate,
    ApiPointeeSchema, ApiPointerConstraint, ApiReturnSchema, ApiReturnValue,
    ApiScalarOrPointerConstraint, ApiSymbol, ApiValueSchema, ApiWriteRequirement, CallbackPolicy,
    CallingConvention, ExecutionSession, GuestCallbackArgument, GuestCallbackCompletionPolicy,
    GuestCallbackRequest, GuestCallbackScratchPointerRelocation, LastErrorPolicy, ReentrancyPolicy,
    StackCleanup,
};
use crate::memory::{GuestAddress, GuestMemory, MemoryAccess};
use crate::pe32::{ResourceDataEntryPlan, ResourceIdentifier};
use crate::runtime::{
    LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE, LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE,
    LOGICAL_WINDOW_HANDLE_NAMESPACE, RUNTIME_SYSTEM_CURSOR_ARROW_RESOURCE_ID,
    RUNTIME_WINDOW_CLASS_NAME_A_ENCODING, RUNTIME_WINDOW_CLASS_NAME_A_LIMIT,
    RUNTIME_WINDOW_NAME_A_ENCODING, RUNTIME_WINDOW_NAME_A_LIMIT,
    RUNTIME_WINDOW_PROCEDURE_ARGUMENT_BYTES, RUNTIME_WINDOW_STYLE_VISIBLE, RuntimeCursorResource,
    RuntimeCursorSource, RuntimeDisplayProfile, RuntimeIconResource, RuntimeInputProfile,
    RuntimeRect, RuntimeSetWindowPosCallbackAction, RuntimeSetWindowPosCallbackMessage,
    RuntimeSetWindowPosOperation, RuntimeSetWindowPosRequest, RuntimeSetWindowPosVariant,
    RuntimeStateErrorClass, RuntimeSystemCursorKind, RuntimeWindow,
    RuntimeWindowClassCursorReference, RuntimeWindowClassIconReference, RuntimeWindowClassModule,
    RuntimeWindowClassNameA, RuntimeWindowClassReference, RuntimeWindowClassRegistration,
    RuntimeWindowCreationCallbackAction, RuntimeWindowCreationMessage,
    RuntimeWindowCreationRequest, RuntimeWindowGeometry, RuntimeWindowLifecycle,
    RuntimeWindowNameA, RuntimeWindowProcedure, RuntimeWindowProcedureCallingConvention,
};

pub const USER32_COMPATIBILITY_POLICY_SCHEMA: &str = "vine.user32-compatibility-policy";
pub const USER32_COMPATIBILITY_POLICY_VERSION: u32 = 14;
pub const ENUM_DISPLAY_MONITORS_IMPLEMENTATION: &str = "user32.enum-display-monitors.v1";
pub const SWAP_MOUSE_BUTTON_IMPLEMENTATION: &str = "user32.swap-mouse-button.v1";
pub const LOAD_ICON_A_IMPLEMENTATION: &str = "user32.load-icon-a.v1";
pub const LOAD_IMAGE_A_IMPLEMENTATION: &str = "user32.load-image-a.v1";
pub const SEND_MESSAGE_A_IMPLEMENTATION: &str = "user32.send-message-a.v1";
pub const SET_WINDOW_POS_IMPLEMENTATION: &str = "user32.set-window-pos.v1";
pub const SET_WINDOW_LONG_A_IMPLEMENTATION: &str = "user32.set-window-long-a.gwl-style.v1";
pub const LOAD_CURSOR_A_IMPLEMENTATION: &str = "user32.load-cursor-a.v1";
pub const REGISTER_CLASS_EX_A_IMPLEMENTATION: &str = "user32.register-class-ex-a.v1";
pub const GET_SYSTEM_METRICS_IMPLEMENTATION: &str = "user32.get-system-metrics.v1";
pub const GET_CLIENT_RECT_IMPLEMENTATION: &str = "user32.get-client-rect.v1";
pub const GET_DC_IMPLEMENTATION: &str = "user32.get-dc.window-acquisition.v1";
pub const FILL_RECT_IMPLEMENTATION: &str = "user32.fill-rect.logical-surface.v1";
pub const RELEASE_DC_IMPLEMENTATION: &str = "user32.release-dc.window-acquisition.v1";
pub const CREATE_WINDOW_EX_A_IMPLEMENTATION: &str = "user32.create-window-ex-a.v1";
pub const DEF_WINDOW_PROC_A_IMPLEMENTATION: &str = "user32.def-window-proc-a.v1";
pub const GET_SYSTEM_MENU_IMPLEMENTATION: &str = "user32.get-system-menu.invalid-window.v1";
pub const ENABLE_MENU_ITEM_IMPLEMENTATION: &str = "user32.enable-menu-item.invalid-menu.v1";

const RT_ICON: u32 = 3;
const RT_CURSOR: u32 = 1;
const RT_GROUP_CURSOR: u32 = 12;
const RT_GROUP_ICON: u32 = 14;
const IMAGE_ICON: u32 = 1;
const LR_SHARED: u32 = 0x0000_8000;
const WM_SETICON: u32 = 0x0080;
const WM_MOVE: u32 = 0x0003;
const WM_WINDOWPOSCHANGED: u32 = 0x0047;
const WM_GETICON: u32 = 0x007f;
const ICON_SMALL: u32 = 0;
const ICON_SMALL2: u32 = 2;
const HWND_NOTOPMOST: u32 = 0xffff_fffe;
const ERROR_SUCCESS: u32 = 0;
const ERROR_INVALID_HANDLE: u32 = 6;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_INVALID_WINDOW_HANDLE: u32 = 1400;
const ERROR_DC_NOT_FOUND: u32 = 1425;
const ERROR_INVALID_INDEX: u32 = 1413;
const ERROR_INVALID_MENU_HANDLE: u32 = 1401;
const ERROR_CLASS_ALREADY_EXISTS: u32 = 1410;
const ERROR_RESOURCE_DATA_NOT_FOUND: u32 = 1812;
const ERROR_RESOURCE_TYPE_NOT_FOUND: u32 = 1813;
const ERROR_RESOURCE_NAME_NOT_FOUND: u32 = 1814;
const GWL_STYLE: i32 = -16;
const INVALID_WINDOW_LONG_INDEX: i32 = -100;
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
pub struct User32LoadImageAPolicy {
    pub profile_id: String,
    pub image_type: u32,
    pub target_width: u16,
    pub target_height: u16,
    pub flags: u32,
    pub maximum_group_entries: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32SendMessageAPolicy {
    pub profile_id: String,
    pub message: u32,
    pub w_param: u32,
    pub icon_width: u16,
    pub icon_height: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32SetWindowPosPolicy {
    pub profile_id: String,
    pub insert_after: u32,
    pub initial_show_screen_multiplier: u32,
    pub initial_show_flags: u32,
    pub initial_hide_flags: u32,
    pub initial_show_callback_flags: u32,
    pub initial_hide_callback_flags: u32,
    pub get_icon_w_param: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32SetWindowLongAPolicy {
    pub profile_id: String,
    pub style_index: i32,
    pub requested_style: u32,
    pub retained_style_mask: u32,
    pub invalid_index: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32LoadCursorAPolicy {
    pub profile_id: String,
    pub arrow_resource_id: u32,
    pub target_width: u16,
    pub target_height: u16,
    pub maximum_group_entries: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32RegisterClassExAPolicy {
    pub profile_id: String,
    pub structure_byte_length: u32,
    pub maximum_class_name_bytes: u32,
    pub allowed_styles: Vec<u32>,
    pub require_zero_class_extra_bytes: bool,
    pub require_zero_window_extra_bytes: bool,
    pub require_live_icon: bool,
    pub require_live_cursor: bool,
    pub require_null_background_brush: bool,
    pub require_null_menu_name: bool,
    pub require_null_small_icon: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32GetSystemMetricsPolicy {
    pub profile_id: String,
    pub caption_height: u32,
    pub fixed_frame_width: u32,
    pub fixed_frame_height: u32,
    pub small_icon_width: Option<u32>,
    pub small_icon_height: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32CreateWindowExAPolicy {
    pub profile_id: String,
    pub maximum_class_name_bytes: u32,
    pub maximum_window_name_bytes: u32,
    pub extended_style: u32,
    pub effective_extended_style: u32,
    pub style: u32,
    pub effective_style: u32,
    pub client_width: u32,
    pub client_height: u32,
    pub outer_width: u32,
    pub outer_height: u32,
    pub maximum_position: i32,
    pub require_null_parent: bool,
    pub require_null_menu: bool,
    pub require_null_create_parameter: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32InvalidMenuChainPolicy {
    pub profile_id: String,
    pub revert: u32,
    pub item: u32,
    pub flags: u32,
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
    pub load_image_a: Option<User32LoadImageAPolicy>,
    pub send_message_a: Option<User32SendMessageAPolicy>,
    pub set_window_pos: Option<User32SetWindowPosPolicy>,
    pub set_window_long_a: Option<User32SetWindowLongAPolicy>,
    pub load_cursor_a: Option<User32LoadCursorAPolicy>,
    pub register_class_ex_a: Option<User32RegisterClassExAPolicy>,
    pub get_system_metrics: Option<User32GetSystemMetricsPolicy>,
    pub create_window_ex_a: Option<User32CreateWindowExAPolicy>,
    pub invalid_menu_chain: Option<User32InvalidMenuChainPolicy>,
}

impl User32CompatibilityPolicy {
    pub fn controlled_v14() -> Self {
        Self {
            schema: USER32_COMPATIBILITY_POLICY_SCHEMA.to_owned(),
            schema_version: USER32_COMPATIBILITY_POLICY_VERSION,
            id: "user32-single-monitor-normal-mouse-icon-cursor-class-metrics-window-menu-small-icon-position-style-v14"
                .to_owned(),
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
            load_image_a: Some(User32LoadImageAPolicy {
                profile_id: "module-shared-small-icon-16x16-v1".to_owned(),
                image_type: IMAGE_ICON,
                target_width: 16,
                target_height: 16,
                flags: LR_SHARED,
                maximum_group_entries: 64,
            }),
            send_message_a: Some(User32SendMessageAPolicy {
                profile_id: "same-thread-wm-seticon-small-v1".to_owned(),
                message: WM_SETICON,
                w_param: ICON_SMALL,
                icon_width: 16,
                icon_height: 16,
            }),
            set_window_pos: Some(User32SetWindowPosPolicy {
                profile_id: "initial-offscreen-show-hide-v1".to_owned(),
                insert_after: HWND_NOTOPMOST,
                initial_show_screen_multiplier: 8,
                initial_show_flags: 0x0000_0559,
                initial_hide_flags: 0x0000_059b,
                initial_show_callback_flags: 0x1000_0d59,
                initial_hide_callback_flags: 0x0000_1d9f,
                get_icon_w_param: ICON_SMALL2,
            }),
            set_window_long_a: Some(User32SetWindowLongAPolicy {
                profile_id: "main-window-gwl-style-visible-v1".to_owned(),
                style_index: GWL_STYLE,
                requested_style: 0x90ca_0000,
                retained_style_mask: 0x0400_0000,
                invalid_index: INVALID_WINDOW_LONG_INDEX,
            }),
            load_cursor_a: Some(User32LoadCursorAPolicy {
                profile_id: "system-arrow-and-module-shared-cursor-v2".to_owned(),
                arrow_resource_id: RUNTIME_SYSTEM_CURSOR_ARROW_RESOURCE_ID,
                target_width: 32,
                target_height: 32,
                maximum_group_entries: 64,
            }),
            register_class_ex_a: Some(User32RegisterClassExAPolicy {
                profile_id: "local-ascii-module-class-v1".to_owned(),
                structure_byte_length: 48,
                maximum_class_name_bytes: 64,
                allowed_styles: vec![0x28, 0x3020],
                require_zero_class_extra_bytes: true,
                require_zero_window_extra_bytes: true,
                require_live_icon: true,
                require_live_cursor: true,
                require_null_background_brush: true,
                require_null_menu_name: true,
                require_null_small_icon: true,
            }),
            get_system_metrics: Some(User32GetSystemMetricsPolicy {
                profile_id: "initial-window-metrics-1920x1080-small-icon-16x16-v3"
                    .to_owned(),
                caption_height: 23,
                fixed_frame_width: 3,
                fixed_frame_height: 3,
                small_icon_width: Some(16),
                small_icon_height: Some(16),
            }),
            create_window_ex_a: Some(User32CreateWindowExAPolicy {
                profile_id: "main-local-window-800x600-v1".to_owned(),
                maximum_class_name_bytes: 64,
                maximum_window_name_bytes: 256,
                extended_style: 0x0004_0000,
                effective_extended_style: 0x0004_0100,
                style: 0x80ca_0000,
                effective_style: 0x84ca_0000,
                client_width: 800,
                client_height: 600,
                outer_width: 806,
                outer_height: 629,
                maximum_position: 32_767,
                require_null_parent: true,
                require_null_menu: true,
                require_null_create_parameter: true,
            }),
            invalid_menu_chain: Some(User32InvalidMenuChainPolicy {
                profile_id: "null-window-system-menu-close-item-v1".to_owned(),
                revert: 0,
                item: 0x0000_f060,
                flags: 0,
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
        if let Some(profile) = self.load_image_a.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.image_type != IMAGE_ICON
                || profile.target_width == 0
                || profile.target_width > 256
                || profile.target_height == 0
                || profile.target_height > 256
                || profile.flags != LR_SHARED
                || profile.maximum_group_entries == 0
                || profile.maximum_group_entries > 256
                || self.load_icon_a.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-load-image-profile-invalid",
                "User32 LoadImageA profile requires the reached shared 16x16 module icon and matching system metrics",
            ));
        }
        if let Some(profile) = self.send_message_a.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.message != WM_SETICON
                || profile.w_param != ICON_SMALL
                || profile.icon_width == 0
                || profile.icon_height == 0
                || self.create_window_ex_a.is_none()
                || self.load_image_a.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-send-message-a-profile-invalid",
                "User32 SendMessageA profile requires the reached same-thread WM_SETICON small-icon window callback",
            ));
        }
        if let Some(profile) = self.set_window_pos.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.insert_after != HWND_NOTOPMOST
                || profile.initial_show_screen_multiplier == 0
                || profile.initial_show_flags == 0
                || profile.initial_hide_flags == 0
                || profile.initial_show_flags == profile.initial_hide_flags
                || profile.initial_show_callback_flags == 0
                || profile.initial_hide_callback_flags == 0
                || profile.initial_show_callback_flags == profile.initial_hide_callback_flags
                || profile.get_icon_w_param != ICON_SMALL2
                || self.send_message_a.is_none()
                || self.create_window_ex_a.is_none()
                || self.get_system_metrics.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-set-window-pos-profile-invalid",
                "User32 SetWindowPos profile requires the reached logical window, small-icon message, display, and show/hide callback sequence",
            ));
        }
        if let Some(profile) = self.set_window_long_a.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.style_index != GWL_STYLE
                || profile.requested_style == 0
                || profile.requested_style & RUNTIME_WINDOW_STYLE_VISIBLE == 0
                || profile.retained_style_mask == 0
                || profile.retained_style_mask & RUNTIME_WINDOW_STYLE_VISIBLE != 0
                || profile.requested_style & profile.retained_style_mask != 0
                || profile.invalid_index == profile.style_index
                || self.create_window_ex_a.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-set-window-long-a-profile-invalid",
                "User32 SetWindowLongA profile requires the reached GWL_STYLE visibility transition and retained native style bits",
            ));
        }
        if let Some(profile) = self.load_cursor_a.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.arrow_resource_id != RUNTIME_SYSTEM_CURSOR_ARROW_RESOURCE_ID
                || profile.target_width == 0
                || profile.target_width > 256
                || profile.target_height == 0
                || profile.target_height > 256
                || profile.maximum_group_entries == 0
                || profile.maximum_group_entries > 256)
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-load-cursor-profile-invalid",
                "User32 cursor profile requires the predefined arrow and bounded module resource dimensions",
            ));
        }
        if let Some(profile) = self.register_class_ex_a.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.structure_byte_length != 48
                || profile.maximum_class_name_bytes == 0
                || profile.maximum_class_name_bytes
                    > u32::try_from(RUNTIME_WINDOW_CLASS_NAME_A_LIMIT)
                        .expect("window class name limit fits u32")
                || profile.allowed_styles.is_empty()
                || profile.allowed_styles.len() > 16
                || profile.allowed_styles.contains(&0)
                || profile
                    .allowed_styles
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                || !profile.require_zero_class_extra_bytes
                || !profile.require_zero_window_extra_bytes
                || !profile.require_live_icon
                || !profile.require_live_cursor
                || !profile.require_null_background_brush
                || !profile.require_null_menu_name
                || !profile.require_null_small_icon)
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-register-class-ex-a-profile-invalid",
                "User32 class registration profile requires the bounded reached local class variants",
            ));
        }
        if let Some(profile) = self.get_system_metrics.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.caption_height == 0
                || profile.caption_height > 1_024
                || profile.fixed_frame_width == 0
                || profile.fixed_frame_width > 1_024
                || profile.fixed_frame_height == 0
                || profile.fixed_frame_height > 1_024
                || profile
                    .small_icon_width
                    .is_some_and(|width| width == 0 || width > 1_024)
                || profile
                    .small_icon_height
                    .is_some_and(|height| height == 0 || height > 1_024)
                || self.enum_display_monitors.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-system-metrics-profile-invalid",
                "User32 system metrics require bounded non-client values and a logical display profile",
            ));
        }
        if let Some(profile) = self.load_image_a.as_ref()
            && (self
                .get_system_metrics
                .as_ref()
                .and_then(|metrics| metrics.small_icon_width)
                != Some(u32::from(profile.target_width))
                || self
                    .get_system_metrics
                    .as_ref()
                    .and_then(|metrics| metrics.small_icon_height)
                    != Some(u32::from(profile.target_height)))
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-load-image-profile-invalid",
                "User32 LoadImageA dimensions must match the controlled small-icon system metrics",
            ));
        }
        if let Some(profile) = self.send_message_a.as_ref()
            && self.load_image_a.as_ref().is_none_or(|image| {
                profile.icon_width != image.target_width
                    || profile.icon_height != image.target_height
            })
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-send-message-a-profile-invalid",
                "User32 SendMessageA icon dimensions must match the controlled LoadImageA profile",
            ));
        }
        if let Some(profile) = self.set_window_pos.as_ref() {
            let display = self.enum_display_monitors.as_ref().ok_or_else(|| {
                ApiFault::runtime_defect(
                    "user32-compatibility-policy",
                    "user32-set-window-pos-profile-invalid",
                    "User32 SetWindowPos profile requires a logical display profile",
                )
            })?;
            let width = u32::try_from(display.primary_monitor_rect.right).unwrap_or(u32::MAX);
            let height = u32::try_from(display.primary_monitor_rect.bottom).unwrap_or(u32::MAX);
            if width
                .checked_mul(profile.initial_show_screen_multiplier)
                .and_then(|value| i32::try_from(value).ok())
                .is_none()
                || height
                    .checked_mul(profile.initial_show_screen_multiplier)
                    .and_then(|value| i32::try_from(value).ok())
                    .is_none()
            {
                return Err(ApiFault::runtime_defect(
                    "user32-compatibility-policy",
                    "user32-set-window-pos-profile-invalid",
                    "User32 SetWindowPos offscreen placement must fit signed x86 coordinates",
                ));
            }
        }
        if let Some(profile) = self.create_window_ex_a.as_ref() {
            let Some(metrics) = self.get_system_metrics.as_ref() else {
                return Err(ApiFault::runtime_defect(
                    "user32-compatibility-policy",
                    "user32-create-window-metrics-profile-missing",
                    "User32 window creation requires the bounded non-client metrics profile",
                ));
            };
            if !valid_profile_id(&profile.profile_id)
                || profile.maximum_class_name_bytes == 0
                || profile.maximum_class_name_bytes
                    > u32::try_from(RUNTIME_WINDOW_CLASS_NAME_A_LIMIT)
                        .expect("window class limit fits u32")
                || profile.maximum_window_name_bytes == 0
                || profile.maximum_window_name_bytes
                    > u32::try_from(RUNTIME_WINDOW_NAME_A_LIMIT)
                        .expect("window title limit fits u32")
                || profile.extended_style == 0
                || profile.effective_extended_style & profile.extended_style
                    != profile.extended_style
                || profile.style == 0
                || profile.effective_style & profile.style != profile.style
                || profile.effective_style & RUNTIME_WINDOW_STYLE_VISIBLE != 0
                || profile.client_width == 0
                || profile.client_height == 0
                || profile.outer_width
                    != profile
                        .client_width
                        .checked_add(metrics.fixed_frame_width.saturating_mul(2))
                        .unwrap_or(0)
                || profile.outer_height
                    != profile
                        .client_height
                        .checked_add(metrics.fixed_frame_height.saturating_mul(2))
                        .and_then(|height| height.checked_add(metrics.caption_height))
                        .unwrap_or(0)
                || profile.maximum_position <= 0
                || !profile.require_null_parent
                || !profile.require_null_menu
                || !profile.require_null_create_parameter
            {
                return Err(ApiFault::runtime_defect(
                    "user32-compatibility-policy",
                    "user32-create-window-profile-invalid",
                    "User32 window creation profile requires the reached styles, bounded copied names, contained 800x600 client geometry, and NULL unowned relations",
                ));
            }
        }
        if let Some(profile) = self.set_window_long_a.as_ref() {
            let create = self.create_window_ex_a.as_ref().ok_or_else(|| {
                ApiFault::runtime_defect(
                    "user32-compatibility-policy",
                    "user32-set-window-long-a-profile-invalid",
                    "User32 SetWindowLongA profile requires a logical window creation profile",
                )
            })?;
            let next_effective_style =
                profile.requested_style | (create.effective_style & profile.retained_style_mask);
            if profile.requested_style & create.style != create.style
                || create.effective_style & profile.retained_style_mask == 0
                || next_effective_style & RUNTIME_WINDOW_STYLE_VISIBLE == 0
            {
                return Err(ApiFault::runtime_defect(
                    "user32-compatibility-policy",
                    "user32-set-window-long-a-profile-invalid",
                    "User32 SetWindowLongA style transition must retain the created window style and make the effective style visible",
                ));
            }
        }
        if let Some(profile) = self.invalid_menu_chain.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.revert != 0
                || profile.item != 0x0000_f060
                || profile.flags != 0
                || self.create_window_ex_a.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-invalid-menu-chain-profile-invalid",
                "User32 invalid menu chain requires the reached NULL-window system-menu close-item variant",
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
        LOAD_IMAGE_A_IMPLEMENTATION => {
            match handle_load_image_a(policy, contract, frame, session) {
                Ok(immediate) => ApiHandlerResult::Immediate(immediate),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        SEND_MESSAGE_A_IMPLEMENTATION => {
            match handle_send_message_a(policy, callback_return_sentinel, contract, frame, session)
            {
                Ok(result) => result,
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        SET_WINDOW_POS_IMPLEMENTATION => {
            match handle_set_window_pos(policy, callback_return_sentinel, contract, frame, session)
            {
                Ok(result) => result,
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        SET_WINDOW_LONG_A_IMPLEMENTATION => {
            match handle_set_window_long_a(policy, contract, frame, session) {
                Ok(immediate) => ApiHandlerResult::Immediate(immediate),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        LOAD_CURSOR_A_IMPLEMENTATION => {
            match handle_load_cursor_a(policy, contract, frame, session) {
                Ok(immediate) => ApiHandlerResult::Immediate(immediate),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        REGISTER_CLASS_EX_A_IMPLEMENTATION => {
            match handle_register_class_ex_a(policy, contract, frame, session) {
                Ok(immediate) => ApiHandlerResult::Immediate(immediate),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        GET_SYSTEM_METRICS_IMPLEMENTATION => {
            match handle_get_system_metrics(policy, contract, frame, session) {
                Ok(immediate) => ApiHandlerResult::Immediate(immediate),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        GET_CLIENT_RECT_IMPLEMENTATION => match handle_get_client_rect(contract, frame, session) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        GET_DC_IMPLEMENTATION => match handle_get_dc(contract, frame, session) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        FILL_RECT_IMPLEMENTATION => match handle_fill_rect(contract, frame, session) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        RELEASE_DC_IMPLEMENTATION => match handle_release_dc(contract, frame, session) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        CREATE_WINDOW_EX_A_IMPLEMENTATION => {
            match handle_create_window_ex_a(
                policy,
                callback_return_sentinel,
                contract,
                frame,
                session,
            ) {
                Ok(result) => result,
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        DEF_WINDOW_PROC_A_IMPLEMENTATION => {
            match handle_def_window_proc_a(
                policy,
                callback_return_sentinel,
                contract,
                frame,
                session,
            ) {
                Ok(result) => result,
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        GET_SYSTEM_MENU_IMPLEMENTATION => match handle_get_system_menu(policy, contract, frame) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        ENABLE_MENU_ITEM_IMPLEMENTATION => match handle_enable_menu_item(policy, contract, frame) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        implementation_id => ApiHandlerResult::Fault(ApiFault::unsupported(
            "user32-api-dispatch",
            "implementation-id-unsupported",
            format!("User32 implementation {implementation_id} is not registered"),
        )),
    }
}

fn handle_get_system_menu(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
) -> Result<ApiImmediate, ApiFault> {
    let profile = policy.invalid_menu_chain.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-menus",
            "get-system-menu-policy-disabled",
            format!(
                "compatibility policy {} does not enable the reached invalid-window system-menu query",
                policy.id
            ),
        )
    })?;
    let window = required_argument(contract, frame, 0, "window")?;
    let revert = required_argument(contract, frame, 1, "revert")?;
    if window != 0 || revert != profile.revert {
        return Err(ApiFault::unsupported(
            "user32-menus",
            "get-system-menu-variant-unsupported",
            "GetSystemMenu currently supports only the reached NULL-window, FALSE-revert failure",
        ));
    }
    Ok(ApiImmediate::new(ApiReturnValue::U32(0)).with_last_error(ERROR_INVALID_WINDOW_HANDLE))
}

fn handle_enable_menu_item(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
) -> Result<ApiImmediate, ApiFault> {
    let profile = policy.invalid_menu_chain.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-menus",
            "enable-menu-item-policy-disabled",
            format!(
                "compatibility policy {} does not enable the reached invalid-menu item update",
                policy.id
            ),
        )
    })?;
    let menu = required_argument(contract, frame, 0, "menu")?;
    let item = required_argument(contract, frame, 1, "item")?;
    let flags = required_argument(contract, frame, 2, "flags")?;
    if menu != 0 || item != profile.item || flags != profile.flags {
        return Err(ApiFault::unsupported(
            "user32-menus",
            "enable-menu-item-variant-unsupported",
            "EnableMenuItem currently supports only the reached NULL-menu close-item failure",
        ));
    }
    Ok(ApiImmediate::new(ApiReturnValue::U32(u32::MAX)).with_last_error(ERROR_INVALID_MENU_HANDLE))
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

fn handle_get_system_metrics(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let metrics = policy.get_system_metrics.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-display",
            "get-system-metrics-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical system metrics",
                policy.id
            ),
        )
    })?;
    let display = policy.enum_display_monitors.as_ref().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-display",
            "get-system-metrics-display-policy-missing",
            "validated system metrics policy has no logical display profile",
        )
    })?;
    let metric_index = required_argument(contract, frame, 0, "system metric index")?;
    let runtime_profile_id = session.runtime().display_profile_id().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-display",
            "logical-display-profile-missing",
            "User32 system metrics policy is enabled but RuntimeDisplays is not configured",
        )
    })?;
    let monitor = session.runtime().primary_monitor().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-display",
            "logical-primary-monitor-missing",
            "configured RuntimeDisplays has no logical primary monitor",
        )
    })?;
    if runtime_profile_id != display.id
        || !monitor.primary
        || monitor.rect != display.primary_monitor_rect
    {
        return Err(ApiFault::runtime_defect(
            "user32-display",
            "logical-display-profile-mismatch",
            "Rust display authority does not match the selected User32 compatibility profile",
        ));
    }
    let value = match metric_index {
        0 => u32::try_from(monitor.rect.right - monitor.rect.left),
        1 => u32::try_from(monitor.rect.bottom - monitor.rect.top),
        4 => Ok(metrics.caption_height),
        7 => Ok(metrics.fixed_frame_width),
        8 => Ok(metrics.fixed_frame_height),
        49 => match metrics.small_icon_width {
            Some(width) => Ok(width),
            None => {
                return Err(ApiFault::unsupported(
                    "user32-display",
                    "get-system-metrics-index-unsupported",
                    "GetSystemMetrics index 49 is disabled in the selected controlled profile",
                ));
            }
        },
        50 => match metrics.small_icon_height {
            Some(height) => Ok(height),
            None => {
                return Err(ApiFault::unsupported(
                    "user32-display",
                    "get-system-metrics-index-unsupported",
                    "GetSystemMetrics index 50 is disabled in the selected controlled profile",
                ));
            }
        },
        _ => {
            return Err(ApiFault::unsupported(
                "user32-display",
                "get-system-metrics-index-unsupported",
                format!(
                    "GetSystemMetrics index {metric_index} is outside the reached controlled profile"
                ),
            ));
        }
    }
    .map_err(|_| {
        ApiFault::runtime_defect(
            "user32-display",
            "logical-display-dimension-invalid",
            "logical display dimensions do not fit the x86 GetSystemMetrics result",
        )
    })?;
    Ok(ApiImmediate::new(ApiReturnValue::U32(value)))
}

fn handle_get_client_rect(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let window_handle = required_argument(contract, frame, 0, "window handle")?;
    let output = required_argument(contract, frame, 1, "client rectangle")?;
    let window = match session.runtime().window(window_handle) {
        Ok(window) => window,
        Err(error) if error.class == RuntimeStateErrorClass::NotFound => {
            return Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
                .with_last_error(ERROR_INVALID_WINDOW_HANDLE));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, &contract.endpoint)),
    };
    let client_right = i32::try_from(window.creation.geometry.client_width).map_err(|_| {
        ApiFault::runtime_defect(
            "user32-windows",
            "get-client-rect-width-invalid",
            "logical client width does not fit an x86 RECT",
        )
    })?;
    let client_bottom = i32::try_from(window.creation.geometry.client_height).map_err(|_| {
        ApiFault::runtime_defect(
            "user32-windows",
            "get-client-rect-height-invalid",
            "logical client height does not fit an x86 RECT",
        )
    })?;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let bytes = [0_i32, 0, client_right, client_bottom]
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect();
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
        .with_last_error(preserved_last_error)
        .with_guest_write(GuestAddress(output), bytes))
}

fn handle_get_dc(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let window_handle = required_argument(contract, frame, 0, "window handle")?;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    match session.runtime_mut().acquire_window_dc(window_handle) {
        Ok(result) => Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
            namespace: LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE.to_owned(),
            value: result.handle,
        })
        .with_last_error(preserved_last_error)),
        Err(error) if error.code == "window-handle-unknown" => {
            Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
                namespace: LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE.to_owned(),
                value: 0,
            })
            .with_last_error(ERROR_INVALID_WINDOW_HANDLE))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_fill_rect(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let device_context = required_argument(contract, frame, 0, "device context")?;
    let rect_address = required_argument(contract, frame, 1, "rectangle")?;
    let brush = required_argument(contract, frame, 2, "brush")?;
    let bytes = session
        .memory()
        .read(GuestAddress(rect_address), 16)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let rect = RuntimeRect {
        left: read_i32(&bytes, 0).ok_or_else(|| malformed_rect_fault(contract))?,
        top: read_i32(&bytes, 4).ok_or_else(|| malformed_rect_fault(contract))?,
        right: read_i32(&bytes, 8).ok_or_else(|| malformed_rect_fault(contract))?,
        bottom: read_i32(&bytes, 12).ok_or_else(|| malformed_rect_fault(contract))?,
    };
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    match session
        .runtime_mut()
        .fill_gdi_rect(device_context, rect, brush)
    {
        Ok(_) => {
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
                .with_last_error(preserved_last_error))
        }
        Err(error) if error.class == RuntimeStateErrorClass::NotFound => {
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
                .with_last_error(ERROR_INVALID_HANDLE))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_release_dc(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let window_handle = required_argument(contract, frame, 0, "window handle")?;
    let device_context = required_argument(contract, frame, 1, "device context")?;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    match session
        .runtime_mut()
        .release_window_dc(window_handle, device_context)
    {
        Ok(()) => {
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
                .with_last_error(preserved_last_error))
        }
        Err(error) if error.code == "window-handle-unknown" => {
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
                .with_last_error(ERROR_INVALID_WINDOW_HANDLE))
        }
        Err(error)
            if error.code == "device-context-handle-unknown"
                || error.code == "device-context-relation-mismatch"
                || error.code == "device-context-window-target-mismatch" =>
        {
            Ok(
                ApiImmediate::new(ApiReturnValue::Bool32(false))
                    .with_last_error(ERROR_DC_NOT_FOUND),
            )
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn malformed_rect_fault(contract: &ApiContract) -> ApiFault {
    ApiFault::runtime_defect(
        "user32-gdi",
        "decoded-rect-missing",
        format!(
            "decoded {} frame did not retain one complete x86 RECT",
            contract.implementation_id
        ),
    )
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ModuleIconSelectionProfile {
    target_width: u16,
    target_height: u16,
    maximum_group_entries: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SelectedGroupCursor {
    resource_id: u32,
    bytes_in_resource: u32,
    width: u16,
    height: u16,
    planes: u16,
    bits_per_pixel: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupCursorSelectionError {
    Malformed,
    ExactSizeUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ModuleCursorSelectionProfile {
    target_width: u16,
    target_height: u16,
    maximum_group_entries: u16,
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

    load_module_shared_icon(
        "load-icon-a-size-selection-unsupported",
        "load-icon-a-selected-png-unsupported",
        ModuleIconSelectionProfile {
            target_width: icon_policy.target_width,
            target_height: icon_policy.target_height,
            maximum_group_entries: icon_policy.maximum_group_entries,
        },
        instance,
        icon_name,
        contract,
        session,
    )
}

fn handle_load_image_a(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let image_policy = policy.load_image_a.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-icons",
            "load-image-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable bounded module image loading",
                policy.id
            ),
        )
    })?;
    let instance = required_argument(contract, frame, 0, "module instance")?;
    let name = required_argument(contract, frame, 1, "image resource name")?;
    let image_type = required_argument(contract, frame, 2, "image type")?;
    let width = required_argument(contract, frame, 3, "image width")?;
    let height = required_argument(contract, frame, 4, "image height")?;
    let flags = required_argument(contract, frame, 5, "image flags")?;
    if instance == 0 {
        return Err(ApiFault::unsupported(
            "user32-icons",
            "load-image-a-system-resource-unsupported",
            "LoadImageA currently supports the reached registered module icon, not system images",
        ));
    }
    if name > u32::from(u16::MAX) {
        return Err(ApiFault::unsupported(
            "user32-icons",
            "load-image-a-string-resource-unsupported",
            "LoadImageA currently supports only integer module resource identifiers",
        ));
    }
    if image_type != image_policy.image_type {
        return Err(ApiFault::unsupported(
            "user32-icons",
            "load-image-a-type-unsupported",
            "LoadImageA currently supports only the reached IMAGE_ICON variant",
        ));
    }
    if width != u32::from(image_policy.target_width)
        || height != u32::from(image_policy.target_height)
    {
        return Err(ApiFault::unsupported(
            "user32-icons",
            "load-image-a-size-unsupported",
            "LoadImageA dimensions are outside the reached controlled small-icon profile",
        ));
    }
    if flags != image_policy.flags {
        return Err(ApiFault::unsupported(
            "user32-icons",
            "load-image-a-flags-unsupported",
            "LoadImageA currently supports only the reached LR_SHARED flags",
        ));
    }

    load_module_shared_icon(
        "load-image-a-size-selection-unsupported",
        "load-image-a-selected-png-unsupported",
        ModuleIconSelectionProfile {
            target_width: image_policy.target_width,
            target_height: image_policy.target_height,
            maximum_group_entries: image_policy.maximum_group_entries,
        },
        instance,
        name,
        contract,
        session,
    )
}

fn handle_send_message_a(
    policy: &User32CompatibilityPolicy,
    callback_return_sentinel: GuestAddress,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiHandlerResult, ApiFault> {
    let message_policy = policy.send_message_a.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-windows",
            "send-message-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable synchronous logical window messages",
                policy.id
            ),
        )
    })?;
    let window_handle = required_argument(contract, frame, 0, "window handle")?;
    let message = required_argument(contract, frame, 1, "message")?;
    let w_param = required_argument(contract, frame, 2, "wParam")?;
    let l_param = required_argument(contract, frame, 3, "lParam")?;
    if message != message_policy.message {
        return Err(ApiFault::unsupported(
            "user32-windows",
            "send-message-a-message-unsupported",
            "SendMessageA currently supports only the reached WM_SETICON message",
        ));
    }
    if w_param != message_policy.w_param {
        return Err(ApiFault::unsupported(
            "user32-windows",
            "send-message-a-wparam-unsupported",
            "SendMessageA WM_SETICON currently supports only ICON_SMALL",
        ));
    }
    let window_procedure = session
        .runtime()
        .resolve_small_icon_window_procedure(
            window_handle,
            l_param,
            message_policy.icon_width,
            message_policy.icon_height,
        )
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let request = GuestCallbackRequest {
        target: window_procedure.address,
        arguments: vec![
            GuestCallbackArgument::U32(window_handle),
            GuestCallbackArgument::U32(message),
            GuestCallbackArgument::U32(w_param),
            GuestCallbackArgument::U32(l_param),
        ],
        scratch: Vec::new(),
        scratch_pointer_relocations: Vec::new(),
        return_sentinel: callback_return_sentinel,
        completion_policy: GuestCallbackCompletionPolicy::ReturnU32,
    };
    session.preflight_guest_callback(&ApiGuestCallbackCall {
        request: request.clone(),
        contract: contract.clone(),
        frame: frame.clone(),
    })?;
    Ok(ApiHandlerResult::GuestCallback(request))
}

fn handle_set_window_long_a(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let style_policy = policy.set_window_long_a.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-windows",
            "set-window-long-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical window style mutation",
                policy.id
            ),
        )
    })?;
    let window_handle = required_argument(contract, frame, 0, "window handle")?;
    let index = required_argument(contract, frame, 1, "window-long index")? as i32;
    let requested_style = required_argument(contract, frame, 2, "new window style")?;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;

    if index == style_policy.invalid_index {
        match session.runtime().window(window_handle) {
            Ok(_) => {
                return Ok(
                    ApiImmediate::new(ApiReturnValue::U32(0)).with_last_error(ERROR_INVALID_INDEX)
                );
            }
            Err(error) if error.code == "window-handle-unknown" => {
                return Ok(ApiImmediate::new(ApiReturnValue::U32(0))
                    .with_last_error(ERROR_INVALID_WINDOW_HANDLE));
            }
            Err(error) => return Err(ApiFault::from_runtime(error, &contract.endpoint)),
        }
    }
    if index != style_policy.style_index || requested_style != style_policy.requested_style {
        return Err(ApiFault::unsupported(
            "user32-windows",
            "set-window-long-a-variant-unsupported",
            "SetWindowLongA arguments are outside the reached GWL_STYLE transition",
        ));
    }
    match session.runtime_mut().set_window_style(
        window_handle,
        requested_style,
        style_policy.retained_style_mask,
    ) {
        Ok(result) => Ok(
            ApiImmediate::new(ApiReturnValue::U32(result.previous_effective_style))
                .with_last_error(preserved_last_error),
        ),
        Err(error) if error.code == "window-handle-unknown" => {
            Ok(ApiImmediate::new(ApiReturnValue::U32(0))
                .with_last_error(ERROR_INVALID_WINDOW_HANDLE))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_set_window_pos(
    policy: &User32CompatibilityPolicy,
    callback_return_sentinel: GuestAddress,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiHandlerResult, ApiFault> {
    let position_policy = policy.set_window_pos.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-windows",
            "set-window-pos-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical window positioning",
                policy.id
            ),
        )
    })?;
    let window_handle = required_argument(contract, frame, 0, "window handle")?;
    let insert_after = required_argument(contract, frame, 1, "insert-after handle")?;
    let x = required_argument(contract, frame, 2, "x")? as i32;
    let y = required_argument(contract, frame, 3, "y")? as i32;
    let width = required_argument(contract, frame, 4, "width")?;
    let height = required_argument(contract, frame, 5, "height")?;
    let flags = required_argument(contract, frame, 6, "flags")?;
    if insert_after != position_policy.insert_after {
        return Err(ApiFault::unsupported(
            "user32-windows",
            "set-window-pos-insert-after-unsupported",
            "SetWindowPos currently supports only the reached HWND_NOTOPMOST ordering",
        ));
    }
    if width != 0 || height != 0 {
        return Err(ApiFault::unsupported(
            "user32-windows",
            "set-window-pos-size-unsupported",
            "SetWindowPos currently supports only the reached SWP_NOSIZE variants",
        ));
    }
    let display = policy.display_profile().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-windows",
            "set-window-pos-display-policy-missing",
            "validated SetWindowPos policy has no logical display profile",
        )
    })?;
    let monitor = session.runtime().primary_monitor().ok_or_else(|| {
        ApiFault::invalid_call(
            "user32-windows",
            "set-window-pos-display-authority-missing",
            "logical display authority is not configured",
        )
    })?;
    if session.runtime().display_profile_id() != Some(display.id.as_str())
        || monitor.rect != display.primary_monitor_rect
        || !monitor.primary
    {
        return Err(ApiFault::invalid_call(
            "user32-windows",
            "set-window-pos-display-authority-mismatch",
            "Rust logical display authority does not match the selected SetWindowPos profile",
        ));
    }
    let screen_width = monitor
        .rect
        .right
        .checked_sub(monitor.rect.left)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| {
            ApiFault::runtime_defect(
                "user32-windows",
                "set-window-pos-display-geometry-invalid",
                "validated logical display width is not a positive x86 value",
            )
        })?;
    let screen_height = monitor
        .rect
        .bottom
        .checked_sub(monitor.rect.top)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| {
            ApiFault::runtime_defect(
                "user32-windows",
                "set-window-pos-display-geometry-invalid",
                "validated logical display height is not a positive x86 value",
            )
        })?;
    let show_x = screen_width
        .checked_mul(position_policy.initial_show_screen_multiplier)
        .and_then(|value| i32::try_from(value).ok())
        .ok_or_else(|| {
            ApiFault::runtime_defect(
                "user32-windows",
                "set-window-pos-show-x-overflow",
                "validated initial offscreen x position overflowed",
            )
        })?;
    let show_y = screen_height
        .checked_mul(position_policy.initial_show_screen_multiplier)
        .and_then(|value| i32::try_from(value).ok())
        .ok_or_else(|| {
            ApiFault::runtime_defect(
                "user32-windows",
                "set-window-pos-show-y-overflow",
                "validated initial offscreen y position overflowed",
            )
        })?;
    let window = session
        .runtime()
        .window(window_handle)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?
        .clone();
    let (variant, next_geometry, next_visible, callback_flags) =
        if flags == position_policy.initial_show_flags && x == show_x && y == show_y {
            let offset_x = window
                .creation
                .geometry
                .client_x
                .checked_sub(window.creation.geometry.x)
                .ok_or_else(|| {
                    ApiFault::runtime_defect(
                        "user32-windows",
                        "set-window-pos-client-offset-x-invalid",
                        "validated logical client x offset underflowed",
                    )
                })?;
            let offset_y = window
                .creation
                .geometry
                .client_y
                .checked_sub(window.creation.geometry.y)
                .ok_or_else(|| {
                    ApiFault::runtime_defect(
                        "user32-windows",
                        "set-window-pos-client-offset-y-invalid",
                        "validated logical client y offset underflowed",
                    )
                })?;
            let mut geometry = window.creation.geometry;
            geometry.x = show_x;
            geometry.y = show_y;
            geometry.client_x = show_x.checked_add(offset_x).ok_or_else(|| {
                ApiFault::runtime_defect(
                    "user32-windows",
                    "set-window-pos-client-x-overflow",
                    "validated logical client x position overflowed",
                )
            })?;
            geometry.client_y = show_y.checked_add(offset_y).ok_or_else(|| {
                ApiFault::runtime_defect(
                    "user32-windows",
                    "set-window-pos-client-y-overflow",
                    "validated logical client y position overflowed",
                )
            })?;
            (
                RuntimeSetWindowPosVariant::InitialShow,
                geometry,
                true,
                position_policy.initial_show_callback_flags,
            )
        } else if flags == position_policy.initial_hide_flags && x == 0 && y == 0 {
            (
                RuntimeSetWindowPosVariant::InitialHide,
                window.creation.geometry,
                false,
                position_policy.initial_hide_callback_flags,
            )
        } else {
            return Err(ApiFault::unsupported(
                "user32-windows",
                "set-window-pos-variant-unsupported",
                "SetWindowPos arguments are outside the reached initial show/hide profile",
            ));
        };
    let operation = session
        .runtime()
        .preview_set_window_pos(RuntimeSetWindowPosRequest {
            window_handle,
            variant,
            next_geometry,
            next_visible,
            next_topmost: false,
            callback_flags,
        })
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let class = session
        .runtime()
        .window_class(window.creation.class.atom)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let request = set_window_pos_callback_request(
        operation,
        class,
        RuntimeSetWindowPosCallbackMessage::WindowPositionChanged,
        callback_return_sentinel,
    )?;
    session.preflight_guest_callback(&ApiGuestCallbackCall {
        request: request.clone(),
        contract: contract.clone(),
        frame: frame.clone(),
    })?;
    Ok(ApiHandlerResult::GuestCallback(request))
}

pub(crate) fn resolve_set_window_pos_callback(
    call: &ApiGuestCallbackCall,
    result: u32,
    session: &ExecutionSession<'_>,
) -> ApiHandlerResult {
    let GuestCallbackCompletionPolicy::SetWindowPos { operation, message } =
        call.request.completion_policy
    else {
        return ApiHandlerResult::Fault(ApiFault::runtime_defect(
            "user32-windows",
            "set-window-pos-callback-policy-mismatch",
            "SetWindowPos callback continuation has the wrong completion policy",
        ));
    };
    let action = match session
        .runtime()
        .preview_set_window_pos_callback(operation, message, result)
    {
        Ok(action) => action,
        Err(error) => {
            return ApiHandlerResult::Fault(ApiFault::from_runtime(error, &call.frame.endpoint));
        }
    };
    match action {
        RuntimeSetWindowPosCallbackAction::Continue {
            window,
            operation,
            next_message,
        } => {
            let class = match session.runtime().window_class(window.creation.class.atom) {
                Ok(class) if class.id == window.creation.class.class_id => class,
                Ok(_) => {
                    return ApiHandlerResult::Fault(ApiFault::runtime_defect(
                        "user32-windows",
                        "set-window-pos-class-identity-mismatch",
                        "logical window class ID no longer matches its ATOM",
                    ));
                }
                Err(error) => {
                    return ApiHandlerResult::Fault(ApiFault::from_runtime(
                        error,
                        &call.frame.endpoint,
                    ));
                }
            };
            match set_window_pos_callback_request(
                operation,
                class,
                next_message,
                call.request.return_sentinel,
            ) {
                Ok(request) => ApiHandlerResult::GuestCallback(request),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        RuntimeSetWindowPosCallbackAction::Completed { .. } => {
            ApiHandlerResult::Immediate(ApiImmediate::new(ApiReturnValue::Bool32(true)))
        }
    }
}

fn set_window_pos_callback_request(
    operation: RuntimeSetWindowPosOperation,
    class: &crate::runtime::RuntimeWindowClass,
    message: RuntimeSetWindowPosCallbackMessage,
    return_sentinel: GuestAddress,
) -> Result<GuestCallbackRequest, ApiFault> {
    if class.registration.window_procedure.argument_bytes != RUNTIME_WINDOW_PROCEDURE_ARGUMENT_BYTES
    {
        return Err(ApiFault::runtime_defect(
            "user32-windows",
            "set-window-pos-window-procedure-abi-invalid",
            "logical window procedure does not use the required four-u32 stdcall ABI",
        ));
    }
    let (message_id, w_param, l_param, scratch) = match message {
        RuntimeSetWindowPosCallbackMessage::WindowPositionChanged => {
            let width = i32::try_from(operation.next_geometry.width).map_err(|_| {
                ApiFault::runtime_defect(
                    "user32-windows",
                    "set-window-pos-callback-width-overflow",
                    "validated logical window width does not fit WINDOWPOS",
                )
            })?;
            let height = i32::try_from(operation.next_geometry.height).map_err(|_| {
                ApiFault::runtime_defect(
                    "user32-windows",
                    "set-window-pos-callback-height-overflow",
                    "validated logical window height does not fit WINDOWPOS",
                )
            })?;
            let scratch = [
                operation.window_handle.to_le_bytes(),
                HWND_NOTOPMOST.to_le_bytes(),
                operation.next_geometry.x.to_le_bytes(),
                operation.next_geometry.y.to_le_bytes(),
                width.to_le_bytes(),
                height.to_le_bytes(),
                operation.callback_flags.to_le_bytes(),
            ]
            .concat();
            (
                WM_WINDOWPOSCHANGED,
                0,
                GuestCallbackArgument::ScratchPointer { offset: 0 },
                scratch,
            )
        }
        RuntimeSetWindowPosCallbackMessage::GetSmallIcon => (
            WM_GETICON,
            ICON_SMALL2,
            GuestCallbackArgument::U32(0),
            Vec::new(),
        ),
    };
    Ok(GuestCallbackRequest {
        target: class.registration.window_procedure.address,
        arguments: vec![
            GuestCallbackArgument::U32(operation.window_handle),
            GuestCallbackArgument::U32(message_id),
            GuestCallbackArgument::U32(w_param),
            l_param,
        ],
        scratch,
        scratch_pointer_relocations: Vec::new(),
        return_sentinel,
        completion_policy: GuestCallbackCompletionPolicy::SetWindowPos { operation, message },
    })
}

fn load_module_shared_icon(
    size_selection_fault: &'static str,
    selected_png_fault: &'static str,
    selection_profile: ModuleIconSelectionProfile,
    instance: u32,
    icon_name: u32,
    contract: &ApiContract,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
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
    let group_limit = 6_u32 + 14_u32 * u32::from(selection_profile.maximum_group_entries);
    if group_resource.size > group_limit {
        return Ok(load_icon_failure(ERROR_RESOURCE_DATA_NOT_FOUND));
    }
    let group_address = resource_address(
        "user32-icons",
        instance,
        group_resource.data_rva.0,
        contract,
    )?;
    let group_bytes = session
        .memory()
        .read(group_address, group_resource.size as usize)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let selected = match select_group_icon(&group_bytes, selection_profile) {
        Ok(selected) => selected,
        Err(GroupIconSelectionError::Malformed) => {
            return Ok(load_icon_failure(ERROR_RESOURCE_DATA_NOT_FOUND));
        }
        Err(GroupIconSelectionError::ExactSizeUnavailable) => {
            return Err(ApiFault::unsupported(
                "user32-icons",
                size_selection_fault,
                "module group icon has no exact entry for the controlled dimensions",
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
    let icon_address =
        resource_address("user32-icons", instance, icon_resource.data_rva.0, contract)?;
    let icon_header = session
        .memory()
        .read(icon_address, 40)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    if icon_header.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Err(ApiFault::unsupported(
            "user32-icons",
            selected_png_fault,
            "selected module icon uses PNG payload; the reached slice validates DIB icons",
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

fn load_module_shared_cursor(
    selection_profile: ModuleCursorSelectionProfile,
    instance: u32,
    cursor_name: u32,
    contract: &ApiContract,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let image = match session
        .runtime()
        .module_image_by_load_base(GuestAddress(instance))
    {
        Ok(image) => image,
        Err(error) if error.code == "module-image-load-base-unknown" => {
            return Ok(load_cursor_failure(ERROR_INVALID_HANDLE));
        }
        Err(error) => {
            return Err(ApiFault::runtime_defect(
                "user32-cursors",
                error.code,
                error.message,
            ));
        }
    };
    let Some(resources) = image.resources.as_ref() else {
        return Ok(load_cursor_failure(ERROR_RESOURCE_TYPE_NOT_FOUND));
    };
    let group_candidates = resources
        .integer_entries(RT_GROUP_CURSOR, cursor_name)
        .filter_map(|entry| integer_language_id(entry).map(|language| (entry, language)))
        .collect::<Vec<_>>();
    let [(group_resource, language_id)] = group_candidates.as_slice() else {
        return Ok(load_cursor_failure(if group_candidates.is_empty() {
            ERROR_RESOURCE_TYPE_NOT_FOUND
        } else {
            ERROR_RESOURCE_LANG_NOT_FOUND
        }));
    };
    let group_limit = 6_u32 + 14_u32 * u32::from(selection_profile.maximum_group_entries);
    if group_resource.size > group_limit {
        return Ok(load_cursor_failure(ERROR_RESOURCE_DATA_NOT_FOUND));
    }
    let group_address = resource_address(
        "user32-cursors",
        instance,
        group_resource.data_rva.0,
        contract,
    )?;
    let group_bytes = session
        .memory()
        .read(group_address, group_resource.size as usize)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let selected = match select_group_cursor(&group_bytes, selection_profile) {
        Ok(selected) => selected,
        Err(GroupCursorSelectionError::Malformed) => {
            return Ok(load_cursor_failure(ERROR_RESOURCE_DATA_NOT_FOUND));
        }
        Err(GroupCursorSelectionError::ExactSizeUnavailable) => {
            return Err(ApiFault::unsupported(
                "user32-cursors",
                "load-cursor-a-module-size-selection-unsupported",
                "module group cursor has no exact entry for the controlled dimensions",
            ));
        }
    };
    let cursor_candidates = resources
        .integer_entries(RT_CURSOR, selected.resource_id)
        .filter(|entry| integer_language_id(entry) == Some(*language_id))
        .collect::<Vec<_>>();
    let [cursor_resource] = cursor_candidates.as_slice() else {
        return Ok(load_cursor_failure(if cursor_candidates.is_empty() {
            ERROR_RESOURCE_TYPE_NOT_FOUND
        } else {
            ERROR_RESOURCE_LANG_NOT_FOUND
        }));
    };
    if cursor_resource.size != selected.bytes_in_resource || cursor_resource.size < 44 {
        return Ok(load_cursor_failure(ERROR_RESOURCE_DATA_NOT_FOUND));
    }
    let cursor_address = resource_address(
        "user32-cursors",
        instance,
        cursor_resource.data_rva.0,
        contract,
    )?;
    let cursor_header = session
        .memory()
        .read(cursor_address, 44)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    if cursor_header[4..].starts_with(&[0x89, b'P', b'N', b'G']) {
        return Err(ApiFault::unsupported(
            "user32-cursors",
            "load-cursor-a-module-png-unsupported",
            "selected module cursor uses PNG payload; the reached slice validates DIB cursors",
        ));
    }
    let hotspot_x = read_u16(&cursor_header, 0).expect("cursor hotspot prefix");
    let hotspot_y = read_u16(&cursor_header, 2).expect("cursor hotspot prefix");
    if hotspot_x >= selected.width
        || hotspot_y >= selected.height
        || !validate_selected_cursor_dib(&cursor_header[4..], &selected, cursor_resource.size - 4)
    {
        return Ok(load_cursor_failure(ERROR_RESOURCE_DATA_NOT_FOUND));
    }

    let source = RuntimeCursorSource::ModuleResource {
        resource: RuntimeCursorResource {
            module_id: image.module_id,
            module_load_base: image.load_base,
            module_source_sha256: image.source.sha256.clone(),
            group_resource_id: cursor_name,
            language_id: *language_id,
            cursor_resource_id: selected.resource_id,
            group_data_rva: group_resource.data_rva,
            group_data_size: group_resource.size,
            cursor_data_rva: cursor_resource.data_rva,
            cursor_data_size: cursor_resource.size,
            width: selected.width,
            height: selected.height,
            planes: selected.planes,
            bits_per_pixel: selected.bits_per_pixel,
            hotspot_x,
            hotspot_y,
        },
    };
    let result = session
        .runtime_mut()
        .load_shared_cursor(source)
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
    if cursor_name > u32::from(u16::MAX) {
        return Err(ApiFault::unsupported(
            "user32-cursors",
            "load-cursor-a-string-resource-unsupported",
            "LoadCursorA currently supports only integer system and module cursor identifiers",
        ));
    }
    if instance != 0 {
        return load_module_shared_cursor(
            ModuleCursorSelectionProfile {
                target_width: cursor_policy.target_width,
                target_height: cursor_policy.target_height,
                maximum_group_entries: cursor_policy.maximum_group_entries,
            },
            instance,
            cursor_name,
            contract,
            session,
        );
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
        .load_shared_cursor(RuntimeCursorSource::SystemPredefined {
            resource_id: cursor_name,
            cursor: RuntimeSystemCursorKind::Arrow,
        })
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::U32(result.handle)).with_last_error(preserved_last_error))
}

fn handle_register_class_ex_a(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let class_policy = policy.register_class_ex_a.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-window-classes",
            "register-class-ex-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable local class registration",
                policy.id
            ),
        )
    })?;
    let class_address = GuestAddress(required_argument(
        contract,
        frame,
        0,
        "WNDCLASSEXA pointer",
    )?);
    let class_bytes = session
        .memory()
        .read(
            class_address,
            usize::try_from(class_policy.structure_byte_length).map_err(|_| {
                ApiFault::runtime_defect(
                    "user32-window-classes",
                    "register-class-ex-a-structure-size-invalid",
                    "validated class policy structure length does not fit host accounting",
                )
            })?,
        )
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let field = |offset| {
        read_u32(&class_bytes, offset).expect("validated WNDCLASSEXA contains every x86 field")
    };
    let cb_size = field(0);
    if cb_size != class_policy.structure_byte_length {
        return Ok(register_class_failure(ERROR_INVALID_PARAMETER));
    }

    let style = field(4);
    let window_procedure_address = GuestAddress(field(8));
    let class_extra_bytes = field(12);
    let window_extra_bytes = field(16);
    let instance = GuestAddress(field(20));
    let icon_handle = field(24);
    let cursor_handle = field(28);
    let background_brush = field(32);
    let menu_name_address = field(36);
    let class_name_address = field(40);
    let small_icon_handle = field(44);

    if !class_policy.allowed_styles.contains(&style) {
        return Err(ApiFault::unsupported(
            "user32-window-classes",
            "register-class-ex-a-style-unsupported",
            "RegisterClassExA style is outside the reached controlled profile",
        ));
    }
    if (class_policy.require_zero_class_extra_bytes && class_extra_bytes != 0)
        || (class_policy.require_zero_window_extra_bytes && window_extra_bytes != 0)
    {
        return Err(ApiFault::unsupported(
            "user32-window-classes",
            "register-class-ex-a-extra-bytes-unsupported",
            "RegisterClassExA currently supports zero class and window extra bytes",
        ));
    }
    if (class_policy.require_live_icon && icon_handle == 0)
        || (class_policy.require_live_cursor && cursor_handle == 0)
    {
        return Err(ApiFault::unsupported(
            "user32-window-classes",
            "register-class-ex-a-null-resource-unsupported",
            "RegisterClassExA reached profile requires live logical icon and cursor identities",
        ));
    }
    if (class_policy.require_null_background_brush && background_brush != 0)
        || (class_policy.require_null_menu_name && menu_name_address != 0)
        || (class_policy.require_null_small_icon && small_icon_handle != 0)
    {
        return Err(ApiFault::unsupported(
            "user32-window-classes",
            "register-class-ex-a-optional-field-unsupported",
            "RegisterClassExA currently supports NULL brush, menu name, and small icon",
        ));
    }
    if instance.0 == 0 || window_procedure_address.0 == 0 {
        return Ok(register_class_failure(ERROR_INVALID_PARAMETER));
    }
    if class_name_address == 0 {
        return Ok(register_class_failure(ERROR_INVALID_PARAMETER));
    }
    if class_name_address <= u32::from(u16::MAX) {
        return Err(ApiFault::unsupported(
            "user32-window-classes",
            "register-class-ex-a-integer-name-unsupported",
            "RegisterClassExA currently supports copied local ASCII names, not integer atoms",
        ));
    }
    let class_name = read_bounded_ascii_nul(
        session.memory(),
        GuestAddress(class_name_address),
        usize::try_from(class_policy.maximum_class_name_bytes).map_err(|_| {
            ApiFault::runtime_defect(
                "user32-window-classes",
                "register-class-ex-a-name-limit-invalid",
                "validated class name limit does not fit host accounting",
            )
        })?,
        contract,
    )?;
    if class_name.is_empty() {
        return Ok(register_class_failure(ERROR_INVALID_PARAMETER));
    }

    let module = match session.runtime().module_image_by_load_base(instance) {
        Ok(image) => {
            let procedure = u64::from(window_procedure_address.0);
            if procedure < u64::from(image.image_range.start().0)
                || procedure >= image.image_range.end()
            {
                return Ok(register_class_failure(ERROR_INVALID_PARAMETER));
            }
            let Some(procedure_rva) = window_procedure_address.0.checked_sub(image.load_base.0)
            else {
                return Ok(register_class_failure(ERROR_INVALID_PARAMETER));
            };
            (
                RuntimeWindowClassModule {
                    module_id: image.module_id,
                    load_base: image.load_base,
                    source_sha256: image.source.sha256.clone(),
                },
                procedure_rva,
            )
        }
        Err(error) if error.code == "module-image-load-base-unknown" => {
            return Ok(register_class_failure(ERROR_INVALID_PARAMETER));
        }
        Err(error) => {
            return Err(ApiFault::runtime_defect(
                "user32-window-classes",
                error.code,
                error.message,
            ));
        }
    };
    if session
        .memory()
        .validate_access(window_procedure_address, 1, MemoryAccess::Execute)
        .is_err()
    {
        return Ok(register_class_failure(ERROR_INVALID_PARAMETER));
    }
    let icon = match session.runtime().icon(icon_handle) {
        Ok(icon) => RuntimeWindowClassIconReference {
            icon_id: icon.id,
            handle: icon.handle,
        },
        Err(error) if error.code == "icon-handle-unknown" => {
            return Ok(register_class_failure(ERROR_INVALID_PARAMETER));
        }
        Err(error) => {
            return Err(ApiFault::runtime_defect(
                "user32-windows",
                error.code,
                error.message,
            ));
        }
    };
    let cursor = match session.runtime().cursor(cursor_handle) {
        Ok(cursor) => RuntimeWindowClassCursorReference {
            cursor_id: cursor.id,
            handle: cursor.handle,
        },
        Err(error) if error.code == "cursor-handle-unknown" => {
            return Ok(register_class_failure(ERROR_INVALID_PARAMETER));
        }
        Err(error) => {
            return Err(ApiFault::runtime_defect(
                "user32-windows",
                error.code,
                error.message,
            ));
        }
    };
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let registration = RuntimeWindowClassRegistration {
        module: module.0.clone(),
        style,
        window_procedure: RuntimeWindowProcedure {
            module_id: module.0.module_id,
            address: window_procedure_address,
            rva: crate::pe32::Rva(module.1),
            calling_convention: RuntimeWindowProcedureCallingConvention::Stdcall,
            argument_bytes: RUNTIME_WINDOW_PROCEDURE_ARGUMENT_BYTES,
        },
        class_extra_bytes,
        window_extra_bytes,
        icon: Some(icon),
        cursor: Some(cursor),
        background_brush,
        menu_name: None,
        class_name: RuntimeWindowClassNameA {
            value: class_name,
            encoding: RUNTIME_WINDOW_CLASS_NAME_A_ENCODING.to_owned(),
        },
        small_icon: None,
    };
    let result = session
        .runtime_mut()
        .register_window_class(registration)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    if result.already_registered {
        return Ok(register_class_failure(ERROR_CLASS_ALREADY_EXISTS));
    }
    Ok(ApiImmediate::new(ApiReturnValue::U16(result.atom)).with_last_error(preserved_last_error))
}

fn handle_create_window_ex_a(
    policy: &User32CompatibilityPolicy,
    callback_return_sentinel: GuestAddress,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiHandlerResult, ApiFault> {
    let window_policy = policy.create_window_ex_a.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-windows",
            "create-window-ex-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical window creation",
                policy.id
            ),
        )
    })?;
    let extended_style = required_argument(contract, frame, 0, "extended style")?;
    let class_name_address = required_argument(contract, frame, 1, "class name")?;
    let window_name_address = required_argument(contract, frame, 2, "window name")?;
    let style = required_argument(contract, frame, 3, "style")?;
    let x = required_argument(contract, frame, 4, "x coordinate")? as i32;
    let y = required_argument(contract, frame, 5, "y coordinate")? as i32;
    let width = required_argument(contract, frame, 6, "outer width")?;
    let height = required_argument(contract, frame, 7, "outer height")?;
    let parent = required_argument(contract, frame, 8, "parent window")?;
    let menu = required_argument(contract, frame, 9, "menu")?;
    let instance = GuestAddress(required_argument(contract, frame, 10, "instance")?);
    let create_parameter = required_argument(contract, frame, 11, "creation parameter")?;

    if extended_style != window_policy.extended_style || style != window_policy.style {
        return Err(ApiFault::unsupported(
            "user32-windows",
            "create-window-ex-a-style-unsupported",
            "CreateWindowExA styles are outside the reached controlled profile",
        ));
    }
    if width != window_policy.outer_width || height != window_policy.outer_height {
        return Err(ApiFault::unsupported(
            "user32-windows",
            "create-window-ex-a-geometry-unsupported",
            "CreateWindowExA outer dimensions are outside the reached 800x600 client profile",
        ));
    }
    if x < 0 || y < 0 || x > window_policy.maximum_position || y > window_policy.maximum_position {
        return Err(ApiFault::unsupported(
            "user32-windows",
            "create-window-ex-a-position-unsupported",
            "CreateWindowExA position is outside the bounded explicit logical display profile",
        ));
    }
    if (window_policy.require_null_parent && parent != 0)
        || (window_policy.require_null_menu && menu != 0)
        || (window_policy.require_null_create_parameter && create_parameter != 0)
    {
        return Err(ApiFault::unsupported(
            "user32-windows",
            "create-window-ex-a-relation-unsupported",
            "CreateWindowExA currently supports a top-level window with NULL menu and creation parameter",
        ));
    }
    if class_name_address == 0 || window_name_address == 0 || instance.0 == 0 {
        return Ok(ApiHandlerResult::Immediate(create_window_failure(
            ERROR_INVALID_PARAMETER,
        )));
    }
    if class_name_address <= u32::from(u16::MAX) {
        return Err(ApiFault::unsupported(
            "user32-windows",
            "create-window-ex-a-integer-class-unsupported",
            "CreateWindowExA currently supports a copied process-local class name, not an integer atom",
        ));
    }
    let class_name = read_bounded_create_window_text_a(
        session.memory(),
        GuestAddress(class_name_address),
        usize::try_from(window_policy.maximum_class_name_bytes).map_err(|_| {
            ApiFault::runtime_defect(
                "user32-windows",
                "create-window-class-limit-invalid",
                "validated CreateWindowExA class-name bound does not fit host accounting",
            )
        })?,
        "class",
        contract,
    )?;
    let window_name = read_bounded_create_window_text_a(
        session.memory(),
        GuestAddress(window_name_address),
        usize::try_from(window_policy.maximum_window_name_bytes).map_err(|_| {
            ApiFault::runtime_defect(
                "user32-windows",
                "create-window-title-limit-invalid",
                "validated CreateWindowExA title bound does not fit host accounting",
            )
        })?,
        "title",
        contract,
    )?;
    if class_name.is_empty() || window_name.is_empty() {
        return Ok(ApiHandlerResult::Immediate(create_window_failure(
            ERROR_INVALID_PARAMETER,
        )));
    }

    let module = match session.runtime().module_image_by_load_base(instance) {
        Ok(image) => RuntimeWindowClassModule {
            module_id: image.module_id,
            load_base: image.load_base,
            source_sha256: image.source.sha256.clone(),
        },
        Err(error) if error.code == "module-image-load-base-unknown" => {
            return Ok(ApiHandlerResult::Immediate(create_window_failure(
                ERROR_INVALID_PARAMETER,
            )));
        }
        Err(error) => {
            return Err(ApiFault::runtime_defect(
                "user32-windows",
                error.code,
                error.message,
            ));
        }
    };
    let class = match session
        .runtime()
        .window_class_by_name(module.module_id, &class_name)
    {
        Ok(class) => class.clone(),
        Err(error) if error.code == "window-class-name-unknown" => {
            return Ok(ApiHandlerResult::Immediate(create_window_failure(
                ERROR_INVALID_PARAMETER,
            )));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, &contract.endpoint)),
    };
    let metrics = policy.get_system_metrics.as_ref().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-windows",
            "create-window-metrics-profile-missing",
            "validated CreateWindowExA policy has no non-client metrics",
        )
    })?;
    let client_x = x
        .checked_add(i32::try_from(metrics.fixed_frame_width).map_err(|_| {
            ApiFault::runtime_defect(
                "user32-windows",
                "create-window-frame-width-invalid",
                "validated frame width does not fit logical window coordinates",
            )
        })?)
        .ok_or_else(|| {
            ApiFault::invalid_call(
                "user32-windows",
                "create-window-client-x-overflow",
                "logical client x coordinate overflowed",
            )
        })?;
    let top_inset = metrics
        .fixed_frame_height
        .checked_add(metrics.caption_height)
        .ok_or_else(|| {
            ApiFault::runtime_defect(
                "user32-windows",
                "create-window-top-inset-overflow",
                "validated non-client top inset overflowed",
            )
        })?;
    let client_y = y
        .checked_add(i32::try_from(top_inset).map_err(|_| {
            ApiFault::runtime_defect(
                "user32-windows",
                "create-window-top-inset-invalid",
                "validated non-client top inset does not fit logical coordinates",
            )
        })?)
        .ok_or_else(|| {
            ApiFault::invalid_call(
                "user32-windows",
                "create-window-client-y-overflow",
                "logical client y coordinate overflowed",
            )
        })?;
    let entry_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let request = RuntimeWindowCreationRequest {
        owner_thread_id: session.runtime().current_thread_id(),
        class: RuntimeWindowClassReference {
            class_id: class.id,
            atom: class.atom,
            module_id: class.registration.module.module_id,
        },
        instance: module,
        name: RuntimeWindowNameA {
            value: window_name,
            encoding: RUNTIME_WINDOW_NAME_A_ENCODING.to_owned(),
        },
        extended_style,
        effective_extended_style: window_policy.effective_extended_style,
        style,
        effective_style: window_policy.effective_style,
        geometry: RuntimeWindowGeometry {
            x,
            y,
            width,
            height,
            client_x,
            client_y,
            client_width: window_policy.client_width,
            client_height: window_policy.client_height,
        },
        parent,
        menu,
        create_parameter,
        entry_last_error,
    };
    let preview = session
        .runtime()
        .preview_window_creation(&request)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let callback = create_window_callback_request(
        &preview,
        &class,
        RuntimeWindowCreationMessage::NonClientCreate,
        callback_return_sentinel,
        contract,
    )?;
    let callback_call = ApiGuestCallbackCall {
        request: callback.clone(),
        contract: contract.clone(),
        frame: frame.clone(),
    };
    session.preflight_guest_callback(&callback_call)?;
    let created = session
        .runtime_mut()
        .create_window(request)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    if created.window_id != preview.id || created.handle != preview.handle {
        return Err(ApiFault::runtime_defect(
            "user32-windows",
            "create-window-preview-commit-mismatch",
            "logical window identity changed between preflight and commit",
        ));
    }
    Ok(ApiHandlerResult::GuestCallback(callback))
}

pub(crate) fn resolve_create_window_ex_a_callback(
    call: &ApiGuestCallbackCall,
    result: u32,
    session: &ExecutionSession<'_>,
) -> ApiHandlerResult {
    let GuestCallbackCompletionPolicy::CreateWindowExA {
        window_id,
        window_handle,
        message,
    } = call.request.completion_policy
    else {
        return ApiHandlerResult::Fault(ApiFault::runtime_defect(
            "user32-windows",
            "create-window-callback-policy-mismatch",
            "CreateWindowExA callback continuation has the wrong completion policy",
        ));
    };
    let action = match session
        .runtime()
        .preview_window_creation_callback(window_id, message, result)
    {
        Ok(action) => action,
        Err(error) => {
            return ApiHandlerResult::Fault(ApiFault::from_runtime(error, &call.frame.endpoint));
        }
    };
    let action_window = match &action {
        RuntimeWindowCreationCallbackAction::Continue { window, .. }
        | RuntimeWindowCreationCallbackAction::Created { window }
        | RuntimeWindowCreationCallbackAction::Aborted { window } => window,
    };
    if action_window.handle != window_handle {
        return ApiHandlerResult::Fault(ApiFault::invalid_call(
            "user32-windows",
            "create-window-callback-handle-mismatch",
            "CreateWindowExA callback continuation does not match the pending logical HWND",
        ));
    }
    match action {
        RuntimeWindowCreationCallbackAction::Continue {
            window,
            next_message,
        } => {
            let class = match session.runtime().window_class(window.creation.class.atom) {
                Ok(class) if class.id == window.creation.class.class_id => class,
                Ok(_) => {
                    return ApiHandlerResult::Fault(ApiFault::runtime_defect(
                        "user32-windows",
                        "create-window-class-identity-mismatch",
                        "pending logical window class ID no longer matches its ATOM",
                    ));
                }
                Err(error) => {
                    return ApiHandlerResult::Fault(ApiFault::from_runtime(
                        error,
                        &call.frame.endpoint,
                    ));
                }
            };
            match create_window_callback_request(
                &window,
                class,
                next_message,
                call.request.return_sentinel,
                &call.contract,
            ) {
                Ok(request) => ApiHandlerResult::GuestCallback(request),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        RuntimeWindowCreationCallbackAction::Created { window } => ApiHandlerResult::Immediate(
            ApiImmediate::new(ApiReturnValue::LogicalHandle {
                namespace: LOGICAL_WINDOW_HANDLE_NAMESPACE.to_owned(),
                value: window.handle,
            })
            .with_last_error(window.creation.entry_last_error),
        ),
        RuntimeWindowCreationCallbackAction::Aborted { .. } => {
            ApiHandlerResult::Immediate(create_window_failure(ERROR_INVALID_PARAMETER))
        }
    }
}

fn handle_def_window_proc_a(
    policy: &User32CompatibilityPolicy,
    callback_return_sentinel: GuestAddress,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiHandlerResult, ApiFault> {
    let message = required_argument(contract, frame, 1, "message")?;
    if message == WM_WINDOWPOSCHANGED && policy.set_window_pos.is_some() {
        return handle_def_window_proc_window_position(
            policy,
            callback_return_sentinel,
            contract,
            frame,
            session,
        );
    }
    handle_def_window_proc_a_immediate(policy, contract, frame, session)
        .map(ApiHandlerResult::Immediate)
}

fn handle_def_window_proc_window_position(
    policy: &User32CompatibilityPolicy,
    callback_return_sentinel: GuestAddress,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiHandlerResult, ApiFault> {
    let position_policy = policy.set_window_pos.as_ref().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-windows",
            "def-window-proc-position-policy-missing",
            "validated window-position default processing has no policy",
        )
    })?;
    let handle = required_argument(contract, frame, 0, "window handle")?;
    let w_param = required_argument(contract, frame, 2, "wParam")?;
    let l_param = required_argument(contract, frame, 3, "lParam")?;
    if w_param != 0 {
        return Err(ApiFault::unsupported(
            "user32-windows",
            "def-window-proc-position-wparam-unsupported",
            "DefWindowProcA WM_WINDOWPOSCHANGED requires the reached zero wParam",
        ));
    }
    let window = session
        .runtime()
        .window(handle)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?
        .clone();
    if window.creation.owner_thread_id != session.runtime().current_thread_id()
        || window.lifecycle != RuntimeWindowLifecycle::Created
    {
        return Err(ApiFault::invalid_call(
            "user32-windows",
            "def-window-proc-position-window-state-invalid",
            "window-position default processing requires the created owner-thread window",
        ));
    }
    let words = read_window_pos_words(session.memory(), GuestAddress(l_param), &contract.endpoint)?;
    if words[0] != handle || words[1] != position_policy.insert_after {
        return Err(ApiFault::invalid_call(
            "user32-windows",
            "def-window-proc-position-identity-mismatch",
            "WINDOWPOS logical HWND or insert-after identity does not match the active window",
        ));
    }
    let callback_geometry = RuntimeWindowGeometry {
        x: words[2] as i32,
        y: words[3] as i32,
        width: words[4],
        height: words[5],
        client_x: 0,
        client_y: 0,
        client_width: window.creation.geometry.client_width,
        client_height: window.creation.geometry.client_height,
    };
    let show_geometry =
        initial_set_window_pos_show_geometry(policy, session, &window, &contract.endpoint)?;
    let (variant, next_geometry, next_visible, notify_move) =
        if words[6] == position_policy.initial_show_callback_flags {
            if callback_geometry.x != show_geometry.x
                || callback_geometry.y != show_geometry.y
                || callback_geometry.width != show_geometry.width
                || callback_geometry.height != show_geometry.height
            {
                return Err(ApiFault::invalid_call(
                    "user32-windows",
                    "def-window-proc-position-show-payload-mismatch",
                    "initial show WINDOWPOS payload does not match the controlled placement",
                ));
            }
            (
                RuntimeSetWindowPosVariant::InitialShow,
                show_geometry,
                true,
                true,
            )
        } else if words[6] == position_policy.initial_hide_callback_flags {
            if callback_geometry.x != window.creation.geometry.x
                || callback_geometry.y != window.creation.geometry.y
                || callback_geometry.width != window.creation.geometry.width
                || callback_geometry.height != window.creation.geometry.height
            {
                return Err(ApiFault::invalid_call(
                    "user32-windows",
                    "def-window-proc-position-hide-payload-mismatch",
                    "initial hide WINDOWPOS payload does not preserve the controlled placement",
                ));
            }
            (
                RuntimeSetWindowPosVariant::InitialHide,
                window.creation.geometry,
                false,
                false,
            )
        } else {
            return Err(ApiFault::unsupported(
                "user32-windows",
                "def-window-proc-position-flags-unsupported",
                "WINDOWPOS flags are outside the reached initial show/hide callback profile",
            ));
        };
    session
        .runtime()
        .preview_set_window_pos(RuntimeSetWindowPosRequest {
            window_handle: handle,
            variant,
            next_geometry,
            next_visible,
            next_topmost: false,
            callback_flags: words[6],
        })
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    if !notify_move {
        return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(
            ApiReturnValue::U32(0),
        )));
    }
    let procedure = session
        .runtime()
        .resolve_window_procedure(handle)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let request = GuestCallbackRequest {
        target: procedure.address,
        arguments: vec![
            GuestCallbackArgument::U32(handle),
            GuestCallbackArgument::U32(WM_MOVE),
            GuestCallbackArgument::U32(0),
            GuestCallbackArgument::U32(window_creation_move_l_param(&next_geometry)?),
        ],
        scratch: Vec::new(),
        scratch_pointer_relocations: Vec::new(),
        return_sentinel: callback_return_sentinel,
        completion_policy: GuestCallbackCompletionPolicy::ReturnConstantU32 { value: 0 },
    };
    session.preflight_guest_callback(&ApiGuestCallbackCall {
        request: request.clone(),
        contract: contract.clone(),
        frame: frame.clone(),
    })?;
    Ok(ApiHandlerResult::GuestCallback(request))
}

fn handle_def_window_proc_a_immediate(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    if policy.create_window_ex_a.is_none()
        && policy.send_message_a.is_none()
        && policy.set_window_pos.is_none()
    {
        return Err(ApiFault::unsupported(
            "user32-windows",
            "def-window-proc-a-policy-disabled",
            "DefWindowProcA requires a logical window callback profile",
        ));
    }
    let handle = required_argument(contract, frame, 0, "window handle")?;
    let message = required_argument(contract, frame, 1, "message")?;
    let w_param = required_argument(contract, frame, 2, "wParam")?;
    let l_param = required_argument(contract, frame, 3, "lParam")?;
    let window = session
        .runtime()
        .window(handle)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?
        .clone();
    if window.creation.owner_thread_id != session.runtime().current_thread_id() {
        return Err(ApiFault::unsupported(
            "user32-windows",
            "def-window-proc-cross-thread-unsupported",
            "DefWindowProcA currently supports the creating owner thread only",
        ));
    }
    match (message, w_param, window.lifecycle) {
        (WM_SETICON, ICON_SMALL, RuntimeWindowLifecycle::Created)
            if policy.send_message_a.is_some() =>
        {
            let profile = policy.send_message_a.as_ref().ok_or_else(|| {
                ApiFault::runtime_defect(
                    "user32-windows",
                    "def-window-proc-a-message-policy-missing",
                    "validated WM_SETICON default processing has no message profile",
                )
            })?;
            let previous = session
                .runtime_mut()
                .set_window_small_icon(handle, l_param, profile.icon_width, profile.icon_height)
                .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
            Ok(ApiImmediate::new(ApiReturnValue::U32(previous)))
        }
        (
            0x0081,
            0,
            RuntimeWindowLifecycle::Creating {
                message: RuntimeWindowCreationMessage::NonClientCreate,
            },
        ) => {
            session
                .memory()
                .validate_access(GuestAddress(l_param), 48, MemoryAccess::Read)
                .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
            Ok(ApiImmediate::new(ApiReturnValue::U32(1)))
        }
        (
            0x0083,
            0,
            RuntimeWindowLifecycle::Creating {
                message: RuntimeWindowCreationMessage::NonClientCalculate,
            },
        ) => {
            let before = session
                .memory()
                .read(GuestAddress(l_param), 16)
                .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
            let geometry = window.creation.geometry;
            let outer_right = geometry
                .x
                .checked_add_unsigned(geometry.width)
                .ok_or_else(|| {
                    ApiFault::runtime_defect(
                        "user32-windows",
                        "def-window-proc-outer-right-overflow",
                        "validated logical outer rectangle overflowed",
                    )
                })?;
            let outer_bottom = geometry
                .y
                .checked_add_unsigned(geometry.height)
                .ok_or_else(|| {
                    ApiFault::runtime_defect(
                        "user32-windows",
                        "def-window-proc-outer-bottom-overflow",
                        "validated logical outer rectangle overflowed",
                    )
                })?;
            let expected = [geometry.x, geometry.y, outer_right, outer_bottom]
                .into_iter()
                .flat_map(i32::to_le_bytes)
                .collect::<Vec<_>>();
            if before != expected {
                let actual = before
                    .chunks_exact(4)
                    .map(|word| i32::from_le_bytes(word.try_into().expect("RECT word")))
                    .collect::<Vec<_>>();
                return Err(ApiFault::invalid_call(
                    "user32-windows",
                    "def-window-proc-nccalcsize-rect-mismatch",
                    format!(
                        "DefWindowProcA WM_NCCALCSIZE input RECT {actual:?} does not match pending outer RECT [{}, {}, {}, {}]",
                        geometry.x, geometry.y, outer_right, outer_bottom
                    ),
                ));
            }
            let client_right = geometry
                .client_x
                .checked_add_unsigned(geometry.client_width)
                .ok_or_else(|| {
                    ApiFault::runtime_defect(
                        "user32-windows",
                        "def-window-proc-client-right-overflow",
                        "validated logical client rectangle overflowed",
                    )
                })?;
            let client_bottom = geometry
                .client_y
                .checked_add_unsigned(geometry.client_height)
                .ok_or_else(|| {
                    ApiFault::runtime_defect(
                        "user32-windows",
                        "def-window-proc-client-bottom-overflow",
                        "validated logical client rectangle overflowed",
                    )
                })?;
            let bytes = [
                geometry.client_x,
                geometry.client_y,
                client_right,
                client_bottom,
            ]
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect();
            Ok(ApiImmediate::new(ApiReturnValue::U32(0))
                .with_guest_write(GuestAddress(l_param), bytes))
        }
        (
            0x0005,
            0,
            RuntimeWindowLifecycle::Creating {
                message: RuntimeWindowCreationMessage::Size,
            },
        ) => {
            let expected = window_creation_size_l_param(&window.creation.geometry)?;
            if l_param != expected {
                return Err(ApiFault::invalid_call(
                    "user32-windows",
                    "def-window-proc-size-lparam-mismatch",
                    format!(
                        "DefWindowProcA WM_SIZE lParam {l_param:#010x} does not match logical client size {expected:#010x}"
                    ),
                ));
            }
            Ok(ApiImmediate::new(ApiReturnValue::U32(0)))
        }
        (
            WM_MOVE,
            0,
            RuntimeWindowLifecycle::Creating {
                message: RuntimeWindowCreationMessage::Move,
            },
        ) => {
            let expected = window_creation_move_l_param(&window.creation.geometry)?;
            if l_param != expected {
                return Err(ApiFault::invalid_call(
                    "user32-windows",
                    "def-window-proc-move-lparam-mismatch",
                    format!(
                        "DefWindowProcA WM_MOVE lParam {l_param:#010x} does not match logical client position {expected:#010x}"
                    ),
                ));
            }
            Ok(ApiImmediate::new(ApiReturnValue::U32(0)))
        }
        (WM_MOVE, 0, RuntimeWindowLifecycle::Created) if policy.set_window_pos.is_some() => {
            let expected_geometry =
                initial_set_window_pos_show_geometry(policy, session, &window, &contract.endpoint)?;
            let expected = window_creation_move_l_param(&expected_geometry)?;
            if l_param != expected {
                return Err(ApiFault::invalid_call(
                    "user32-windows",
                    "def-window-proc-position-move-lparam-mismatch",
                    format!(
                        "DefWindowProcA positioned WM_MOVE lParam {l_param:#010x} does not match controlled client position {expected:#010x}"
                    ),
                ));
            }
            Ok(ApiImmediate::new(ApiReturnValue::U32(0)))
        }
        _ => Err(ApiFault::unsupported(
            "user32-windows",
            "def-window-proc-a-message-unsupported",
            "DefWindowProcA message is outside the reached synchronous window profile",
        )),
    }
}

fn read_window_pos_words(
    memory: &GuestMemory,
    address: GuestAddress,
    endpoint: &ApiEndpointIdentity,
) -> Result<[u32; 7], ApiFault> {
    let bytes = memory
        .read(address, 28)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let mut words = [0_u32; 7];
    for (index, word) in bytes.chunks_exact(4).enumerate() {
        words[index] = u32::from_le_bytes(word.try_into().expect("WINDOWPOS word"));
    }
    Ok(words)
}

fn initial_set_window_pos_show_geometry(
    policy: &User32CompatibilityPolicy,
    session: &ExecutionSession<'_>,
    window: &RuntimeWindow,
    _endpoint: &ApiEndpointIdentity,
) -> Result<RuntimeWindowGeometry, ApiFault> {
    let position_policy = policy.set_window_pos.as_ref().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-windows",
            "set-window-pos-policy-missing",
            "validated initial placement has no SetWindowPos policy",
        )
    })?;
    let display = policy.display_profile().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-windows",
            "set-window-pos-display-policy-missing",
            "validated initial placement has no display profile",
        )
    })?;
    let monitor = session.runtime().primary_monitor().ok_or_else(|| {
        ApiFault::invalid_call(
            "user32-windows",
            "set-window-pos-display-authority-missing",
            "logical display authority is not configured",
        )
    })?;
    if session.runtime().display_profile_id() != Some(display.id.as_str())
        || monitor.rect != display.primary_monitor_rect
        || !monitor.primary
    {
        return Err(ApiFault::invalid_call(
            "user32-windows",
            "set-window-pos-display-authority-mismatch",
            "Rust logical display authority does not match the initial placement profile",
        ));
    }
    let width = u32::try_from(monitor.rect.right - monitor.rect.left).map_err(|_| {
        ApiFault::runtime_defect(
            "user32-windows",
            "set-window-pos-display-width-invalid",
            "validated logical display width is invalid",
        )
    })?;
    let height = u32::try_from(monitor.rect.bottom - monitor.rect.top).map_err(|_| {
        ApiFault::runtime_defect(
            "user32-windows",
            "set-window-pos-display-height-invalid",
            "validated logical display height is invalid",
        )
    })?;
    let x = width
        .checked_mul(position_policy.initial_show_screen_multiplier)
        .and_then(|value| i32::try_from(value).ok())
        .ok_or_else(|| {
            ApiFault::runtime_defect(
                "user32-windows",
                "set-window-pos-show-x-overflow",
                "validated initial offscreen x position overflowed",
            )
        })?;
    let y = height
        .checked_mul(position_policy.initial_show_screen_multiplier)
        .and_then(|value| i32::try_from(value).ok())
        .ok_or_else(|| {
            ApiFault::runtime_defect(
                "user32-windows",
                "set-window-pos-show-y-overflow",
                "validated initial offscreen y position overflowed",
            )
        })?;
    let offset_x = window.creation.geometry.client_x - window.creation.geometry.x;
    let offset_y = window.creation.geometry.client_y - window.creation.geometry.y;
    let mut geometry = window.creation.geometry;
    geometry.x = x;
    geometry.y = y;
    geometry.client_x = x.checked_add(offset_x).ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-windows",
            "set-window-pos-client-x-overflow",
            "validated positioned client x overflowed",
        )
    })?;
    geometry.client_y = y.checked_add(offset_y).ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-windows",
            "set-window-pos-client-y-overflow",
            "validated positioned client y overflowed",
        )
    })?;
    Ok(geometry)
}

fn create_window_callback_request(
    window: &RuntimeWindow,
    class: &crate::runtime::RuntimeWindowClass,
    message: RuntimeWindowCreationMessage,
    return_sentinel: GuestAddress,
    _contract: &ApiContract,
) -> Result<GuestCallbackRequest, ApiFault> {
    if class.id != window.creation.class.class_id
        || class.atom != window.creation.class.atom
        || class.registration.module.module_id != window.creation.class.module_id
    {
        return Err(ApiFault::runtime_defect(
            "user32-windows",
            "create-window-callback-class-mismatch",
            "logical window callback class relation is inconsistent",
        ));
    }
    let geometry = window.creation.geometry;
    let (message_id, scratch, scratch_pointer_relocations, l_param) = match message {
        RuntimeWindowCreationMessage::NonClientCreate | RuntimeWindowCreationMessage::Create => {
            let mut scratch = Vec::with_capacity(256);
            for value in [
                window.creation.create_parameter,
                window.creation.instance.load_base.0,
                window.creation.menu,
                window.creation.parent,
                geometry.height,
                geometry.width,
                geometry.y as u32,
                geometry.x as u32,
                window.creation.style,
                0,
                0,
                window.creation.effective_extended_style,
            ] {
                scratch.extend_from_slice(&value.to_le_bytes());
            }
            let title_offset = u32::try_from(scratch.len()).expect("CREATESTRUCTA size fits u32");
            scratch.extend_from_slice(window.creation.name.value.as_bytes());
            scratch.push(0);
            while scratch.len() % 4 != 0 {
                scratch.push(0);
            }
            let class_offset = u32::try_from(scratch.len()).map_err(|_| {
                ApiFault::resource_limit(
                    "user32-windows",
                    "create-window-callback-scratch-unrepresentable",
                    "CreateWindowExA callback scratch does not fit x86 accounting",
                )
            })?;
            scratch.extend_from_slice(class.registration.class_name.value.as_bytes());
            scratch.push(0);
            (
                if message == RuntimeWindowCreationMessage::NonClientCreate {
                    0x0081
                } else {
                    0x0001
                },
                scratch,
                vec![
                    GuestCallbackScratchPointerRelocation {
                        write_offset: 36,
                        target_offset: title_offset,
                    },
                    GuestCallbackScratchPointerRelocation {
                        write_offset: 40,
                        target_offset: class_offset,
                    },
                ],
                GuestCallbackArgument::ScratchPointer { offset: 0 },
            )
        }
        RuntimeWindowCreationMessage::NonClientCalculate => {
            let right = geometry
                .x
                .checked_add_unsigned(geometry.width)
                .ok_or_else(|| {
                    ApiFault::runtime_defect(
                        "user32-windows",
                        "create-window-outer-right-overflow",
                        "validated logical outer rectangle overflowed",
                    )
                })?;
            let bottom = geometry
                .y
                .checked_add_unsigned(geometry.height)
                .ok_or_else(|| {
                    ApiFault::runtime_defect(
                        "user32-windows",
                        "create-window-outer-bottom-overflow",
                        "validated logical outer rectangle overflowed",
                    )
                })?;
            (
                0x0083,
                [geometry.x, geometry.y, right, bottom]
                    .into_iter()
                    .flat_map(i32::to_le_bytes)
                    .collect(),
                Vec::new(),
                GuestCallbackArgument::ScratchPointer { offset: 0 },
            )
        }
        RuntimeWindowCreationMessage::Size => (
            0x0005,
            Vec::new(),
            Vec::new(),
            GuestCallbackArgument::U32(window_creation_size_l_param(&geometry)?),
        ),
        RuntimeWindowCreationMessage::Move => (
            0x0003,
            Vec::new(),
            Vec::new(),
            GuestCallbackArgument::U32(window_creation_move_l_param(&geometry)?),
        ),
    };
    Ok(GuestCallbackRequest {
        target: class.registration.window_procedure.address,
        arguments: vec![
            GuestCallbackArgument::U32(window.handle),
            GuestCallbackArgument::U32(message_id),
            GuestCallbackArgument::U32(0),
            l_param,
        ],
        scratch,
        scratch_pointer_relocations,
        return_sentinel,
        completion_policy: GuestCallbackCompletionPolicy::CreateWindowExA {
            window_id: window.id,
            window_handle: window.handle,
            message,
        },
    })
}

fn window_creation_size_l_param(geometry: &RuntimeWindowGeometry) -> Result<u32, ApiFault> {
    let width = u16::try_from(geometry.client_width).map_err(|_| {
        ApiFault::runtime_defect(
            "user32-windows",
            "create-window-client-width-unrepresentable",
            "logical client width does not fit WM_SIZE lParam",
        )
    })?;
    let height = u16::try_from(geometry.client_height).map_err(|_| {
        ApiFault::runtime_defect(
            "user32-windows",
            "create-window-client-height-unrepresentable",
            "logical client height does not fit WM_SIZE lParam",
        )
    })?;
    Ok(u32::from(width) | (u32::from(height) << 16))
}

fn window_creation_move_l_param(geometry: &RuntimeWindowGeometry) -> Result<u32, ApiFault> {
    let x = i16::try_from(geometry.client_x).map_err(|_| {
        ApiFault::runtime_defect(
            "user32-windows",
            "create-window-client-x-unrepresentable",
            "logical client x coordinate does not fit WM_MOVE lParam",
        )
    })?;
    let y = i16::try_from(geometry.client_y).map_err(|_| {
        ApiFault::runtime_defect(
            "user32-windows",
            "create-window-client-y-unrepresentable",
            "logical client y coordinate does not fit WM_MOVE lParam",
        )
    })?;
    Ok(u32::from(x as u16) | (u32::from(y as u16) << 16))
}

fn read_bounded_create_window_text_a(
    memory: &GuestMemory,
    address: GuestAddress,
    maximum_bytes: usize,
    kind: &'static str,
    contract: &ApiContract,
) -> Result<String, ApiFault> {
    let mut bytes = Vec::with_capacity(maximum_bytes);
    for offset in 0..maximum_bytes {
        let offset = u32::try_from(offset).map_err(|_| {
            ApiFault::runtime_defect(
                "user32-windows",
                "create-window-text-offset-invalid",
                "validated CreateWindowExA text bound does not fit x86 accounting",
            )
        })?;
        let byte_address = address
            .0
            .checked_add(offset)
            .map(GuestAddress)
            .ok_or_else(|| {
                ApiFault::invalid_call(
                    "user32-windows",
                    "create-window-text-address-overflow",
                    "CreateWindowExA text read overflowed x86 address accounting",
                )
            })?;
        let byte = memory
            .read(byte_address, 1)
            .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?[0];
        if byte == 0 {
            return String::from_utf8(bytes).map_err(|_| {
                ApiFault::runtime_defect(
                    "user32-windows",
                    "create-window-text-decoding-defect",
                    "validated ASCII CreateWindowExA text failed UTF-8 decoding",
                )
            });
        }
        if !byte.is_ascii() {
            return Err(ApiFault::unsupported(
                "user32-windows",
                "create-window-non-ascii-text-unsupported",
                format!(
                    "CreateWindowExA {kind} currently supports the reached ASCII subset of Windows-932"
                ),
            ));
        }
        bytes.push(byte);
    }
    Err(ApiFault::unsupported(
        "user32-windows",
        "create-window-text-bound-exceeded",
        format!("CreateWindowExA {kind} is not NUL-terminated within the controlled bound"),
    ))
}

fn create_window_failure(last_error: u32) -> ApiImmediate {
    ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: LOGICAL_WINDOW_HANDLE_NAMESPACE.to_owned(),
        value: 0,
    })
    .with_last_error(last_error)
}

fn read_bounded_ascii_nul(
    memory: &GuestMemory,
    address: GuestAddress,
    maximum_bytes: usize,
    contract: &ApiContract,
) -> Result<String, ApiFault> {
    let mut bytes = Vec::with_capacity(maximum_bytes);
    for offset in 0..maximum_bytes {
        let offset = u32::try_from(offset).map_err(|_| {
            ApiFault::runtime_defect(
                "user32-window-classes",
                "register-class-ex-a-name-offset-invalid",
                "validated class name bound does not fit x86 accounting",
            )
        })?;
        let byte_address = address
            .0
            .checked_add(offset)
            .map(GuestAddress)
            .ok_or_else(|| {
                ApiFault::invalid_call(
                    "user32-window-classes",
                    "register-class-ex-a-name-address-overflow",
                    "class name read overflowed x86 address accounting",
                )
            })?;
        let byte = memory
            .read(byte_address, 1)
            .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?[0];
        if byte == 0 {
            return String::from_utf8(bytes).map_err(|_| {
                ApiFault::runtime_defect(
                    "user32-window-classes",
                    "register-class-ex-a-name-decoding-defect",
                    "validated ASCII class name failed UTF-8 decoding",
                )
            });
        }
        if !byte.is_ascii() {
            return Err(ApiFault::unsupported(
                "user32-window-classes",
                "register-class-ex-a-non-ascii-name-unsupported",
                "RegisterClassExA currently supports the reached ASCII subset of Windows-932",
            ));
        }
        bytes.push(byte);
    }
    Err(ApiFault::unsupported(
        "user32-window-classes",
        "register-class-ex-a-name-bound-exceeded",
        "RegisterClassExA class name is not NUL-terminated within the controlled bound",
    ))
}

fn integer_language_id(entry: &ResourceDataEntryPlan) -> Option<u32> {
    match &entry.language {
        ResourceIdentifier::Id { id } => Some(*id),
        ResourceIdentifier::Name { .. } => None,
    }
}

fn resource_address(
    subsystem: &'static str,
    load_base: u32,
    rva: u32,
    _contract: &ApiContract,
) -> Result<GuestAddress, ApiFault> {
    load_base.checked_add(rva).map(GuestAddress).ok_or_else(|| {
        ApiFault::runtime_defect(
            subsystem,
            "module-resource-address-overflow",
            "registered module load base plus resource RVA overflowed x86 accounting",
        )
    })
}

fn select_group_icon(
    bytes: &[u8],
    policy: ModuleIconSelectionProfile,
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

fn select_group_cursor(
    bytes: &[u8],
    policy: ModuleCursorSelectionProfile,
) -> Result<SelectedGroupCursor, GroupCursorSelectionError> {
    if bytes.len() < 6 || read_u16(bytes, 0) != Some(0) || read_u16(bytes, 2) != Some(2) {
        return Err(GroupCursorSelectionError::Malformed);
    }
    let count = read_u16(bytes, 4).ok_or(GroupCursorSelectionError::Malformed)?;
    if count == 0 || count > policy.maximum_group_entries {
        return Err(GroupCursorSelectionError::Malformed);
    }
    let expected_size = 6_usize
        .checked_add(usize::from(count) * 14)
        .ok_or(GroupCursorSelectionError::Malformed)?;
    if bytes.len() != expected_size {
        return Err(GroupCursorSelectionError::Malformed);
    }
    let mut selected = None;
    for index in 0..usize::from(count) {
        let offset = 6 + index * 14;
        let width = read_u16(bytes, offset).ok_or(GroupCursorSelectionError::Malformed)?;
        let stored_height =
            read_u16(bytes, offset + 2).ok_or(GroupCursorSelectionError::Malformed)?;
        let planes = read_u16(bytes, offset + 4).ok_or(GroupCursorSelectionError::Malformed)?;
        let bits_per_pixel =
            read_u16(bytes, offset + 6).ok_or(GroupCursorSelectionError::Malformed)?;
        let bytes_in_resource =
            read_u32(bytes, offset + 8).ok_or(GroupCursorSelectionError::Malformed)?;
        let resource_id =
            u32::from(read_u16(bytes, offset + 12).ok_or(GroupCursorSelectionError::Malformed)?);
        if width == 0
            || width > 256
            || stored_height == 0
            || !stored_height.is_multiple_of(2)
            || stored_height / 2 > 256
            || planes == 0
            || bits_per_pixel == 0
            || bytes_in_resource < 44
            || resource_id == 0
        {
            return Err(GroupCursorSelectionError::Malformed);
        }
        let height = stored_height / 2;
        if width != policy.target_width || height != policy.target_height {
            continue;
        }
        let candidate = SelectedGroupCursor {
            resource_id,
            bytes_in_resource,
            width,
            height,
            planes,
            bits_per_pixel,
        };
        if selected
            .as_ref()
            .is_none_or(|current: &SelectedGroupCursor| {
                candidate.bits_per_pixel > current.bits_per_pixel
            })
        {
            selected = Some(candidate);
        }
    }
    selected.ok_or(GroupCursorSelectionError::ExactSizeUnavailable)
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

fn validate_selected_cursor_dib(
    header: &[u8],
    selected: &SelectedGroupCursor,
    data_size: u32,
) -> bool {
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

fn register_class_failure(last_error: u32) -> ApiImmediate {
    ApiImmediate::new(ApiReturnValue::U16(0)).with_last_error(last_error)
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
        scratch_pointer_relocations: Vec::new(),
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
        "LoadImageA" => Ok(Some(load_image_a_contract(endpoint)?)),
        "SendMessageA" => Ok(Some(send_message_a_contract(endpoint)?)),
        "SetWindowPos" => Ok(Some(set_window_pos_contract(endpoint)?)),
        "SetWindowLongA" => Ok(Some(set_window_long_a_contract(endpoint)?)),
        "LoadCursorA" => Ok(Some(load_cursor_a_contract(endpoint)?)),
        "RegisterClassExA" => Ok(Some(register_class_ex_a_contract(endpoint)?)),
        "GetSystemMetrics" => Ok(Some(get_system_metrics_contract(endpoint)?)),
        "GetClientRect" => Ok(Some(get_client_rect_contract(endpoint)?)),
        "GetDC" => Ok(Some(get_dc_contract(endpoint)?)),
        "FillRect" => Ok(Some(fill_rect_contract(endpoint)?)),
        "ReleaseDC" => Ok(Some(release_dc_contract(endpoint)?)),
        "CreateWindowExA" => Ok(Some(create_window_ex_a_contract(endpoint)?)),
        "DefWindowProcA" => Ok(Some(def_window_proc_a_contract(endpoint)?)),
        "GetSystemMenu" => Ok(Some(get_system_menu_contract(endpoint)?)),
        "EnableMenuItem" => Ok(Some(enable_menu_item_contract(endpoint)?)),
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

pub fn load_image_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                u32_argument("instance"),
                u32_argument("name"),
                u32_argument("type"),
                u32_argument("width"),
                u32_argument("height"),
                u32_argument("flags"),
            ],
            return_value: ApiReturnSchema::U32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: LOAD_IMAGE_A_IMPLEMENTATION.to_owned(),
    })
}

pub fn send_message_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                logical_window_handle_argument("window", false),
                u32_argument("message"),
                u32_argument("wParam"),
                scalar_or_pointer_l_param_argument("lParam"),
            ],
            return_value: ApiReturnSchema::U32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Required,
        implementation_id: SEND_MESSAGE_A_IMPLEMENTATION.to_owned(),
    })
}

pub fn set_window_pos_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                logical_window_handle_argument("window", false),
                u32_argument("insertAfter"),
                u32_argument("x"),
                u32_argument("y"),
                u32_argument("width"),
                u32_argument("height"),
                u32_argument("flags"),
            ],
            return_value: ApiReturnSchema::Bool32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Required,
        implementation_id: SET_WINDOW_POS_IMPLEMENTATION.to_owned(),
    })
}

pub fn set_window_long_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                logical_window_handle_argument("window", false),
                u32_argument("index"),
                u32_argument("newValue"),
            ],
            return_value: ApiReturnSchema::U32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: SET_WINDOW_LONG_A_IMPLEMENTATION.to_owned(),
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

pub fn register_class_ex_a_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![required_in_struct("windowClass", "WNDCLASSEXA", 48)],
            return_value: ApiReturnSchema::U16,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: REGISTER_CLASS_EX_A_IMPLEMENTATION.to_owned(),
    })
}

pub fn get_system_metrics_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![u32_argument("metricIndex")],
            return_value: ApiReturnSchema::U32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: GET_SYSTEM_METRICS_IMPLEMENTATION.to_owned(),
    })
}

pub fn get_client_rect_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                logical_window_handle_argument("window", true),
                conditional_out_struct("clientRect", "RECT", 16),
            ],
            return_value: ApiReturnSchema::Bool32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: GET_CLIENT_RECT_IMPLEMENTATION.to_owned(),
    })
}

pub fn get_dc_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![logical_window_handle_argument("window", false)],
            return_value: ApiReturnSchema::LogicalHandle {
                namespace: LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE.to_owned(),
                nullable: true,
            },
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: GET_DC_IMPLEMENTATION.to_owned(),
    })
}

pub fn fill_rect_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                logical_handle_argument(
                    "deviceContext",
                    LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE,
                    false,
                ),
                required_in_struct("rect", "RECT", 16),
                logical_handle_argument("brush", LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE, false),
            ],
            return_value: ApiReturnSchema::Bool32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: FILL_RECT_IMPLEMENTATION.to_owned(),
    })
}

pub fn release_dc_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                logical_window_handle_argument("window", false),
                logical_handle_argument(
                    "deviceContext",
                    LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE,
                    false,
                ),
            ],
            return_value: ApiReturnSchema::Bool32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: RELEASE_DC_IMPLEMENTATION.to_owned(),
    })
}

pub fn create_window_ex_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                u32_argument("extendedStyle"),
                windows_932_name_or_ordinal("className", 256),
                windows_932_name_or_ordinal("windowName", 512),
                u32_argument("style"),
                u32_argument("x"),
                u32_argument("y"),
                u32_argument("width"),
                u32_argument("height"),
                logical_window_handle_argument("parent", true),
                u32_argument("menu"),
                opaque_pointer_argument("instance", false, 4),
                opaque_pointer_argument("creationParameter", true, 1),
            ],
            return_value: ApiReturnSchema::LogicalHandle {
                namespace: LOGICAL_WINDOW_HANDLE_NAMESPACE.to_owned(),
                nullable: true,
            },
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Optional,
        implementation_id: CREATE_WINDOW_EX_A_IMPLEMENTATION.to_owned(),
    })
}

pub fn def_window_proc_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                logical_window_handle_argument("window", false),
                u32_argument("message"),
                u32_argument("wParam"),
                scalar_or_pointer_l_param_argument("lParam"),
            ],
            return_value: ApiReturnSchema::U32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Optional,
        implementation_id: DEF_WINDOW_PROC_A_IMPLEMENTATION.to_owned(),
    })
}

pub fn get_system_menu_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                logical_window_handle_argument("window", true),
                u32_argument("revert"),
            ],
            return_value: ApiReturnSchema::U32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: GET_SYSTEM_MENU_IMPLEMENTATION.to_owned(),
    })
}

pub fn enable_menu_item_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                u32_argument("menu"),
                u32_argument("item"),
                u32_argument("flags"),
            ],
            return_value: ApiReturnSchema::U32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: ENABLE_MENU_ITEM_IMPLEMENTATION.to_owned(),
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

fn windows_932_name_or_ordinal(name: &str, maximum_code_units: u32) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::NameOrOrdinal {
            encoding: crate::api::ApiStringEncoding::Windows932,
            maximum_code_units,
        },
    }
}

fn logical_window_handle_argument(name: &str, nullable: bool) -> ApiArgumentSchema {
    logical_handle_argument(name, LOGICAL_WINDOW_HANDLE_NAMESPACE, nullable)
}

fn logical_handle_argument(name: &str, namespace: &str, nullable: bool) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::LogicalHandle {
            namespace: namespace.to_owned(),
            nullable,
        },
    }
}

fn opaque_pointer_argument(name: &str, nullable: bool, alignment: u32) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::OpaquePointer {
            nullable,
            alignment,
        },
    }
}

fn scalar_or_pointer_l_param_argument(name: &str) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::InOut,
        value: ApiValueSchema::ScalarOrPointer {
            constraint: ApiScalarOrPointerConstraint {
                alignment: 4,
                minimum_byte_length: 16,
                maximum_byte_length: 48,
                pointee: ApiPointeeSchema::Bytes,
                write_requirement: ApiWriteRequirement::Optional,
            },
        },
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

fn required_in_struct(name: &str, type_id: &str, byte_length: u32) -> ApiArgumentSchema {
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

fn conditional_out_struct(name: &str, type_id: &str, byte_length: u32) -> ApiArgumentSchema {
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
                write_requirement: ApiWriteRequirement::Optional,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controlled_policy_is_host_independent_and_bounded() {
        let policy = User32CompatibilityPolicy::controlled_v14();
        policy.validate().unwrap();
        let profile = policy.display_profile().unwrap();
        assert_eq!(profile.primary_monitor_rect.right, 1920);
        assert_eq!(profile.primary_monitor_rect.bottom, 1080);
        assert_eq!(
            policy.get_system_metrics.as_ref().unwrap().small_icon_width,
            Some(16)
        );
        assert_eq!(
            policy
                .get_system_metrics
                .as_ref()
                .unwrap()
                .small_icon_height,
            Some(16)
        );

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

        let mut invalid_input = User32CompatibilityPolicy::controlled_v14();
        invalid_input.swap_mouse_button.as_mut().unwrap().id.clear();
        assert_eq!(
            invalid_input.validate().unwrap_err().code,
            "user32-input-profile-invalid"
        );

        let mut invalid_icon = User32CompatibilityPolicy::controlled_v14();
        invalid_icon
            .load_icon_a
            .as_mut()
            .unwrap()
            .maximum_group_entries = 0;
        assert_eq!(
            invalid_icon.validate().unwrap_err().code,
            "user32-load-icon-profile-invalid"
        );

        let mut invalid_image = User32CompatibilityPolicy::controlled_v14();
        invalid_image.load_image_a.as_mut().unwrap().flags = 0;
        assert_eq!(
            invalid_image.validate().unwrap_err().code,
            "user32-load-image-profile-invalid"
        );

        let mut invalid_message = User32CompatibilityPolicy::controlled_v14();
        invalid_message.send_message_a.as_mut().unwrap().message = 0x0081;
        assert_eq!(
            invalid_message.validate().unwrap_err().code,
            "user32-send-message-a-profile-invalid"
        );

        let mut invalid_window_style = User32CompatibilityPolicy::controlled_v14();
        invalid_window_style
            .set_window_long_a
            .as_mut()
            .unwrap()
            .retained_style_mask = RUNTIME_WINDOW_STYLE_VISIBLE;
        assert_eq!(
            invalid_window_style.validate().unwrap_err().code,
            "user32-set-window-long-a-profile-invalid"
        );

        let mut invalid_cursor = User32CompatibilityPolicy::controlled_v14();
        invalid_cursor
            .load_cursor_a
            .as_mut()
            .unwrap()
            .arrow_resource_id = 0x7f01;
        assert_eq!(
            invalid_cursor.validate().unwrap_err().code,
            "user32-load-cursor-profile-invalid"
        );

        let mut invalid_class = User32CompatibilityPolicy::controlled_v14();
        invalid_class
            .register_class_ex_a
            .as_mut()
            .unwrap()
            .allowed_styles
            .reverse();
        assert_eq!(
            invalid_class.validate().unwrap_err().code,
            "user32-register-class-ex-a-profile-invalid"
        );

        let mut invalid_metrics = User32CompatibilityPolicy::controlled_v14();
        invalid_metrics
            .get_system_metrics
            .as_mut()
            .unwrap()
            .caption_height = 0;
        assert_eq!(
            invalid_metrics.validate().unwrap_err().code,
            "user32-system-metrics-profile-invalid"
        );

        let mut invalid_small_icon_width = User32CompatibilityPolicy::controlled_v14();
        invalid_small_icon_width
            .get_system_metrics
            .as_mut()
            .unwrap()
            .small_icon_width = Some(0);
        assert_eq!(
            invalid_small_icon_width.validate().unwrap_err().code,
            "user32-system-metrics-profile-invalid"
        );

        let mut invalid_small_icon = User32CompatibilityPolicy::controlled_v14();
        invalid_small_icon
            .get_system_metrics
            .as_mut()
            .unwrap()
            .small_icon_height = Some(0);
        assert_eq!(
            invalid_small_icon.validate().unwrap_err().code,
            "user32-system-metrics-profile-invalid"
        );

        let mut invalid_menu = User32CompatibilityPolicy::controlled_v14();
        invalid_menu.invalid_menu_chain.as_mut().unwrap().item = 0xf061;
        assert_eq!(
            invalid_menu.validate().unwrap_err().code,
            "user32-invalid-menu-chain-profile-invalid"
        );
    }
}

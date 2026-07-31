use serde::Serialize;

use crate::api::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiCallFrame, ApiContract, ApiDirection,
    ApiDynamicPointerConstraint, ApiEndpointIdentity, ApiFault, ApiGuestCallbackCall,
    ApiGuestWrite, ApiGuestWriteReceipt, ApiHandlerResult, ApiImmediate, ApiPointeeSchema,
    ApiPointerConstraint, ApiReturnSchema, ApiReturnValue, ApiScalarOrPointerConstraint, ApiSymbol,
    ApiValueSchema, ApiWriteRequirement, CallbackPolicy, CallingConvention, ExecutionSession,
    GuestCallbackArgument, GuestCallbackCompletionPolicy, GuestCallbackRequest,
    GuestCallbackScratchPointerRelocation, LastErrorPolicy, ReentrancyPolicy, StackCleanup,
};
use crate::memory::{GuestAddress, GuestMemory, MemoryAccess};
use crate::pe32::{ResourceDataEntryPlan, ResourceIdentifier};
use crate::runtime::{
    LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE, LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE,
    LOGICAL_MENU_HANDLE_NAMESPACE, LOGICAL_MONITOR_HANDLE_NAMESPACE,
    LOGICAL_WINDOW_HANDLE_NAMESPACE, RUNTIME_SYSTEM_CURSOR_ARROW_RESOURCE_ID,
    RUNTIME_USER_TIMER_MINIMUM_MILLISECONDS, RUNTIME_WINDOW_CLASS_NAME_A_ENCODING,
    RUNTIME_WINDOW_CLASS_NAME_A_LIMIT, RUNTIME_WINDOW_NAME_A_ENCODING, RUNTIME_WINDOW_NAME_A_LIMIT,
    RUNTIME_WINDOW_PROCEDURE_ARGUMENT_BYTES, RUNTIME_WINDOW_STYLE_MINIMIZED,
    RUNTIME_WINDOW_STYLE_VISIBLE, RUNTIME_X86_PAINT_STRUCT_BYTE_LENGTH, RuntimeBeginPaintRequest,
    RuntimeCursorPositionProfile, RuntimeCursorResource, RuntimeCursorSource,
    RuntimeDisplayProfile, RuntimeEndPaintRequest, RuntimeIconResource,
    RuntimeImeStatusWindowRequest, RuntimeInputProfile, RuntimeKeyboardProfile,
    RuntimeMessagePeekDisposition, RuntimePointerButtonMessageProfile, RuntimeRect,
    RuntimeSetWindowPosCallbackAction, RuntimeSetWindowPosCallbackMessage,
    RuntimeSetWindowPosDefaultCallbackAction, RuntimeSetWindowPosDefaultCallbackMessage,
    RuntimeSetWindowPosDefaultOperation, RuntimeSetWindowPosOperation, RuntimeSetWindowPosRequest,
    RuntimeSetWindowPosVariant, RuntimeStateErrorClass, RuntimeSystemCursorKind,
    RuntimeUpdateWindowCallbackAction, RuntimeUpdateWindowOperation, RuntimeUpdateWindowRequest,
    RuntimeWindow, RuntimeWindowClassCursorReference, RuntimeWindowClassIconReference,
    RuntimeWindowClassModule, RuntimeWindowClassNameA, RuntimeWindowClassReference,
    RuntimeWindowClassRegistration, RuntimeWindowCreationCallbackAction,
    RuntimeWindowCreationMessage, RuntimeWindowCreationRequest, RuntimeWindowGeometry,
    RuntimeWindowLifecycle, RuntimeWindowNameA, RuntimeWindowProcedure,
    RuntimeWindowProcedureCallingConvention,
};

pub const USER32_COMPATIBILITY_POLICY_SCHEMA: &str = "vine.user32-compatibility-policy";
pub const USER32_COMPATIBILITY_POLICY_VERSION: u32 = 35;
const USER32_COMPATIBILITY_POLICY_POINTER_DOWN_VERSION: u32 = 34;
const USER32_COMPATIBILITY_POLICY_SET_FOCUS_VERSION: u32 = 33;
const USER32_COMPATIBILITY_POLICY_POINTER_VERSION: u32 = 32;
const USER32_COMPATIBILITY_POLICY_TIMER_VERSION: u32 = 31;
const USER32_COMPATIBILITY_POLICY_BASE_VERSION: u32 = 30;
pub const ENUM_DISPLAY_MONITORS_IMPLEMENTATION: &str = "user32.enum-display-monitors.v1";
pub const MONITOR_FROM_WINDOW_IMPLEMENTATION: &str = "user32.monitor-from-window.v1";
pub const GET_MONITOR_INFO_A_IMPLEMENTATION: &str = "user32.get-monitor-info-a.v1";
pub const SWAP_MOUSE_BUTTON_IMPLEMENTATION: &str = "user32.swap-mouse-button.v1";
pub const LOAD_ICON_A_IMPLEMENTATION: &str = "user32.load-icon-a.v1";
pub const LOAD_IMAGE_A_IMPLEMENTATION: &str = "user32.load-image-a.v1";
pub const SEND_MESSAGE_A_IMPLEMENTATION: &str = "user32.send-message-a.v1";
pub const SET_WINDOW_POS_IMPLEMENTATION: &str = "user32.set-window-pos.v5";
pub const SHOW_WINDOW_IMPLEMENTATION: &str = "user32.show-window.visible-normal-noop.v1";
pub const UPDATE_WINDOW_IMPLEMENTATION: &str = "user32.update-window.full-client-paint.v1";
pub const BEGIN_PAINT_IMPLEMENTATION: &str = "user32.begin-paint.full-client-logical-dc.v1";
pub const END_PAINT_IMPLEMENTATION: &str = "user32.end-paint.matching-session.v1";
pub const SET_WINDOW_LONG_A_IMPLEMENTATION: &str = "user32.set-window-long-a.gwl-style.v1";
pub const SET_TIMER_IMPLEMENTATION: &str = "user32.set-timer.live-window-callback-registration.v1";
pub const LOAD_CURSOR_A_IMPLEMENTATION: &str = "user32.load-cursor-a.v1";
pub const REGISTER_CLASS_EX_A_IMPLEMENTATION: &str = "user32.register-class-ex-a.v1";
pub const GET_SYSTEM_METRICS_IMPLEMENTATION: &str = "user32.get-system-metrics.v1";
pub const GET_CLIENT_RECT_IMPLEMENTATION: &str = "user32.get-client-rect.v1";
pub const GET_WINDOW_RECT_IMPLEMENTATION: &str = "user32.get-window-rect.v1";
pub const GET_CURSOR_POS_IMPLEMENTATION: &str = "user32.get-cursor-pos.v1";
pub const PEEK_MESSAGE_A_IMPLEMENTATION: &str = "user32.peek-message-a.current-thread.v1";
pub const TRANSLATE_MESSAGE_IMPLEMENTATION: &str =
    "user32.translate-message.window-timer-pointer-noop.v2";
pub const DISPATCH_MESSAGE_A_IMPLEMENTATION: &str =
    "user32.dispatch-message-a.window-timer-pointer-callback.v2";
pub const IS_ICONIC_IMPLEMENTATION: &str = "user32.is-iconic.logical-window-style.v1";
pub const GET_FOREGROUND_WINDOW_IMPLEMENTATION: &str = "user32.get-foreground-window.v1";
pub const GET_ASYNC_KEY_STATE_IMPLEMENTATION: &str = "user32.get-async-key-state.v1";
pub const SET_FOCUS_IMPLEMENTATION: &str = "user32.set-focus.same-current-window.v1";
pub const SET_RECT_IMPLEMENTATION: &str = "user32.set-rect.v1";
pub const GET_DC_IMPLEMENTATION: &str = "user32.get-dc.window-or-screen-acquisition.v2";
pub const FILL_RECT_IMPLEMENTATION: &str = "user32.fill-rect.logical-surface.v1";
pub const RELEASE_DC_IMPLEMENTATION: &str = "user32.release-dc.window-or-screen-acquisition.v2";
pub const CREATE_WINDOW_EX_A_IMPLEMENTATION: &str = "user32.create-window-ex-a.v1";
pub const DEF_WINDOW_PROC_A_IMPLEMENTATION: &str = "user32.def-window-proc-a.v1";
pub const GET_SYSTEM_MENU_IMPLEMENTATION: &str = "user32.get-system-menu.logical-system-menu.v2";
pub const ENABLE_MENU_ITEM_IMPLEMENTATION: &str = "user32.enable-menu-item.logical-system-menu.v2";

const RT_ICON: u32 = 3;
const RT_CURSOR: u32 = 1;
const RT_GROUP_CURSOR: u32 = 12;
const RT_GROUP_ICON: u32 = 14;
const IMAGE_ICON: u32 = 1;
const LR_SHARED: u32 = 0x0000_8000;
const WM_SETICON: u32 = 0x0080;
const WM_PAINT: u32 = 0x000f;
const WM_TIMER: u32 = 0x0113;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_RBUTTONDOWN: u32 = 0x0204;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_MOVE: u32 = 0x0003;
const WM_SIZE: u32 = 0x0005;
const WM_ACTIVATE: u32 = 0x0006;
const WM_SETFOCUS: u32 = 0x0007;
const WM_ACTIVATEAPP: u32 = 0x001c;
const WM_WINDOWPOSCHANGING: u32 = 0x0046;
const WM_WINDOWPOSCHANGED: u32 = 0x0047;
const WM_NCCALCSIZE: u32 = 0x0083;
const WM_NCACTIVATE: u32 = 0x0086;
const WM_GETICON: u32 = 0x007f;
const WM_IME_SETCONTEXT: u32 = 0x0281;
const WM_IME_NOTIFY: u32 = 0x0282;
const PM_NOREMOVE: u32 = 0;
const PM_REMOVE: u32 = 1;
const X86_MSG_BYTE_LENGTH: u32 = 28;
const MK_LBUTTON: u32 = 0x0001;
const MK_RBUTTON: u32 = 0x0002;
const MK_SHIFT: u32 = 0x0004;
const MK_CONTROL: u32 = 0x0008;
pub const USER32_WM_IME_CONTROL: u32 = 0x0283;
pub const USER32_IMC_CLOSESTATUSWINDOW: u32 = 0x0021;
pub const USER32_IMC_OPENSTATUSWINDOW: u32 = 0x0022;
const ICON_SMALL: u32 = 0;
const ICON_SMALL2: u32 = 2;
const HWND_TOP: u32 = 0;
const HWND_NOTOPMOST: u32 = 0xffff_fffe;
const MONITOR_DEFAULTTOPRIMARY: u32 = 1;
const ERROR_SUCCESS: u32 = 0;
const ERROR_INVALID_HANDLE: u32 = 6;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_INVALID_WINDOW_HANDLE: u32 = 1400;
const ERROR_NOACCESS: u32 = 998;
const ERROR_INVALID_MONITOR_HANDLE: u32 = 1461;
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
const MONITORINFO_BYTE_LENGTH: u32 = 40;
const MONITORINFOEXA_BYTE_LENGTH: u32 = 72;
const MONITORINFOF_PRIMARY: u32 = 1;

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
pub struct User32SendMessageAImeControlPolicy {
    pub profile_id: String,
    pub message: u32,
    pub close_status_window_command: u32,
    pub open_status_window_command: u32,
    pub l_param: u32,
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
    pub activation_flags: u32,
    pub activation_internal_flags: u32,
    pub activation_insert_after: u32,
    pub resize_outer_width: u32,
    pub resize_outer_height: u32,
    pub resize_client_width: u32,
    pub resize_client_height: u32,
    pub resize_nccalc_window_pos_flags: u32,
    pub resize_changed_flags: u32,
    pub frame_change_insert_after: u32,
    pub frame_change_move_x: i32,
    pub frame_change_move_y: i32,
    pub frame_change_move_flags: u32,
    pub frame_change_nccalc_window_pos_flags: u32,
    pub frame_change_changed_flags: u32,
    pub activate_app_other_thread_id: u32,
    pub ime_set_context_flags: u32,
    pub ime_notify_command: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32ShowWindowPolicy {
    pub profile_id: String,
    pub normal_command: u32,
    pub require_visible: bool,
    pub require_active: bool,
    pub require_focused: bool,
    pub require_foreground: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32IsIconicPolicy {
    pub profile_id: String,
    pub minimized_style_mask: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32UpdateWindowPolicy {
    pub profile_id: String,
    pub paint_message: u32,
    pub callback_result: u32,
    pub paint_struct_byte_length: u32,
    pub erase_pending: bool,
    pub require_visible: bool,
    pub require_active: bool,
    pub require_focused: bool,
    pub require_foreground: bool,
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
pub struct User32SetTimerPolicy {
    pub profile_id: String,
    pub timer_id: u32,
    pub interval_milliseconds: u32,
    pub message: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32WindowTimerDeliveryPolicy {
    pub profile_id: String,
    pub message: u32,
    pub callback_result: u32,
    pub require_empty_queue: bool,
    pub coalesce_missed_periods: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32MonitorFromWindowPolicy {
    pub profile_id: String,
    pub flags: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32GetMonitorInfoAPolicy {
    pub profile_id: String,
    pub structure_byte_length: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32ScreenDcPolicy {
    pub profile_id: String,
    pub require_primary_monitor: bool,
    pub require_same_thread_release: bool,
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
pub struct User32SystemMenuPolicy {
    pub profile_id: String,
    pub revert: u32,
    pub item: u32,
    pub flags: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32GetAsyncKeyStatePolicy {
    pub profile_id: String,
    pub minimum_virtual_key: u16,
    pub maximum_virtual_key: u16,
    pub require_source_focus: bool,
    pub require_focused_foreground_window: bool,
    pub consume_pressed_since_last_query: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32SetFocusPolicy {
    pub profile_id: String,
    pub require_current_thread_active_focused_foreground_window: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32DefWindowProcAPointerDownPolicy {
    pub profile_id: String,
    pub message: u32,
    pub w_param: u32,
    pub result: u32,
    pub require_current_thread_visible_active_focused_foreground_window: bool,
    pub require_cursor_derived_client_l_param: bool,
}

pub type User32DefWindowProcAPointerReleasePolicy = User32DefWindowProcAPointerDownPolicy;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32SwapMouseButtonPolicy {
    pub profile_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32PeekMessageAPolicy {
    pub profile_id: String,
    pub window: u32,
    pub minimum_message: u32,
    pub maximum_message: u32,
    pub no_remove_flag: u32,
    pub remove_flag: u32,
    pub message_byte_length: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32CompatibilityPolicy {
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub enum_display_monitors: Option<RuntimeDisplayProfile>,
    pub input: RuntimeInputProfile,
    pub swap_mouse_button: Option<User32SwapMouseButtonPolicy>,
    pub get_async_key_state: Option<User32GetAsyncKeyStatePolicy>,
    pub peek_message_a: Option<User32PeekMessageAPolicy>,
    pub is_iconic: Option<User32IsIconicPolicy>,
    pub load_icon_a: Option<User32LoadIconAPolicy>,
    pub load_image_a: Option<User32LoadImageAPolicy>,
    pub send_message_a: Option<User32SendMessageAPolicy>,
    pub send_message_a_ime_control: Option<User32SendMessageAImeControlPolicy>,
    pub set_window_pos: Option<User32SetWindowPosPolicy>,
    pub show_window: Option<User32ShowWindowPolicy>,
    pub update_window: Option<User32UpdateWindowPolicy>,
    pub set_window_long_a: Option<User32SetWindowLongAPolicy>,
    pub set_timer: Option<User32SetTimerPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_timer_delivery: Option<User32WindowTimerDeliveryPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pointer_button_messages: Option<RuntimePointerButtonMessageProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_focus: Option<User32SetFocusPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub def_window_proc_a_pointer_down: Option<User32DefWindowProcAPointerDownPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub def_window_proc_a_pointer_release: Option<User32DefWindowProcAPointerReleasePolicy>,
    pub monitor_from_window: Option<User32MonitorFromWindowPolicy>,
    pub get_monitor_info_a: Option<User32GetMonitorInfoAPolicy>,
    pub screen_dc: Option<User32ScreenDcPolicy>,
    pub load_cursor_a: Option<User32LoadCursorAPolicy>,
    pub register_class_ex_a: Option<User32RegisterClassExAPolicy>,
    pub get_system_metrics: Option<User32GetSystemMetricsPolicy>,
    pub create_window_ex_a: Option<User32CreateWindowExAPolicy>,
    pub system_menu: Option<User32SystemMenuPolicy>,
}

impl User32CompatibilityPolicy {
    pub fn controlled_v30() -> Self {
        Self {
            schema: USER32_COMPATIBILITY_POLICY_SCHEMA.to_owned(),
            schema_version: USER32_COMPATIBILITY_POLICY_BASE_VERSION,
            id: "user32-single-monitor-window-frame-change-show-update-paint-window-state-v30"
                .to_owned(),
            enum_display_monitors: Some(RuntimeDisplayProfile {
                id: "single-monitor-1920x1080-work-1920x1032-v2".to_owned(),
                primary_monitor_rect: RuntimeRect {
                    left: 0,
                    top: 0,
                    right: 1920,
                    bottom: 1080,
                },
                primary_work_rect: RuntimeRect {
                    left: 0,
                    top: 0,
                    right: 1920,
                    bottom: 1032,
                },
            }),
            input: RuntimeInputProfile {
                id: "normal-mouse-buttons-v1".to_owned(),
                initial_mouse_buttons_swapped: false,
                keyboard: Some(RuntimeKeyboardProfile {
                    id: "foreground-focused-virtual-key-state-v1".to_owned(),
                    initial_source_focused: true,
                    maximum_virtual_key: 0xff,
                    aggregate_modifier_keys: true,
                    clear_on_source_focus_loss: true,
                }),
                cursor_position: Some(RuntimeCursorPositionProfile {
                    id: "primary-display-center-cursor-v1".to_owned(),
                    initial_screen_x: 960,
                    initial_screen_y: 540,
                }),
            },
            swap_mouse_button: Some(User32SwapMouseButtonPolicy {
                profile_id: "normal-mouse-buttons-v1".to_owned(),
            }),
            get_async_key_state: Some(User32GetAsyncKeyStatePolicy {
                profile_id: "foreground-focused-virtual-key-state-v1".to_owned(),
                minimum_virtual_key: 0,
                maximum_virtual_key: 0xff,
                require_source_focus: true,
                require_focused_foreground_window: true,
                consume_pressed_since_last_query: true,
            }),
            peek_message_a: Some(User32PeekMessageAPolicy {
                profile_id: "current-thread-unfiltered-peek-message-a-v1".to_owned(),
                window: 0,
                minimum_message: 0,
                maximum_message: 0,
                no_remove_flag: PM_NOREMOVE,
                remove_flag: PM_REMOVE,
                message_byte_length: X86_MSG_BYTE_LENGTH,
            }),
            is_iconic: Some(User32IsIconicPolicy {
                profile_id: "logical-window-effective-style-is-iconic-v1".to_owned(),
                minimized_style_mask: RUNTIME_WINDOW_STYLE_MINIMIZED,
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
            send_message_a_ime_control: Some(User32SendMessageAImeControlPolicy {
                profile_id: "same-thread-default-ime-status-window-control-v1".to_owned(),
                message: USER32_WM_IME_CONTROL,
                close_status_window_command: USER32_IMC_CLOSESTATUSWINDOW,
                open_status_window_command: USER32_IMC_OPENSTATUSWINDOW,
                l_param: 0,
            }),
            set_window_pos: Some(User32SetWindowPosPolicy {
                profile_id:
                    "initial-offscreen-show-hide-same-geometry-activation-resize-frame-change-move-v4".to_owned(),
                insert_after: HWND_NOTOPMOST,
                initial_show_screen_multiplier: 8,
                initial_show_flags: 0x0000_0559,
                initial_hide_flags: 0x0000_059b,
                initial_show_callback_flags: 0x1000_0d59,
                initial_hide_callback_flags: 0x0000_1d9f,
                get_icon_w_param: ICON_SMALL2,
                activation_flags: 0x0000_010a,
                activation_internal_flags: 0x0000_0003,
                activation_insert_after: 0,
                resize_outer_width: 1_286,
                resize_outer_height: 749,
                resize_client_width: 1_280,
                resize_client_height: 720,
                resize_nccalc_window_pos_flags: 0x0000_190e,
                resize_changed_flags: 0x0000_110e,
                frame_change_insert_after: HWND_TOP,
                frame_change_move_x: 317,
                frame_change_move_y: 154,
                frame_change_move_flags: 0x0000_0025,
                frame_change_nccalc_window_pos_flags: 0x0000_1825,
                frame_change_changed_flags: 0x0000_0825,
                activate_app_other_thread_id: 0,
                ime_set_context_flags: 0xc000_000f,
                ime_notify_command: 2,
            }),
            show_window: Some(User32ShowWindowPolicy {
                profile_id: "already-visible-active-normal-noop-v1".to_owned(),
                normal_command: 1,
                require_visible: true,
                require_active: true,
                require_focused: true,
                require_foreground: true,
            }),
            update_window: Some(User32UpdateWindowPolicy {
                profile_id: "visible-active-full-client-paint-v1".to_owned(),
                paint_message: WM_PAINT,
                callback_result: 0,
                paint_struct_byte_length: RUNTIME_X86_PAINT_STRUCT_BYTE_LENGTH as u32,
                erase_pending: true,
                require_visible: true,
                require_active: true,
                require_focused: true,
                require_foreground: true,
            }),
            set_window_long_a: Some(User32SetWindowLongAPolicy {
                profile_id: "main-window-gwl-style-visible-v1".to_owned(),
                style_index: GWL_STYLE,
                requested_style: 0x90ca_0000,
                retained_style_mask: 0x0400_0000,
                invalid_index: INVALID_WINDOW_LONG_INDEX,
            }),
            set_timer: Some(User32SetTimerPolicy {
                profile_id: "main-window-id-1-20ms-timerproc-v1".to_owned(),
                timer_id: 1,
                interval_milliseconds: 20,
                message: WM_TIMER,
            }),
            window_timer_delivery: None,
            pointer_button_messages: None,
            set_focus: None,
            def_window_proc_a_pointer_down: None,
            def_window_proc_a_pointer_release: None,
            monitor_from_window: Some(User32MonitorFromWindowPolicy {
                profile_id: "live-window-default-to-primary-v1".to_owned(),
                flags: MONITOR_DEFAULTTOPRIMARY,
            }),
            get_monitor_info_a: Some(User32GetMonitorInfoAPolicy {
                profile_id: "logical-primary-monitor-info-40-v1".to_owned(),
                structure_byte_length: MONITORINFO_BYTE_LENGTH,
            }),
            screen_dc: Some(User32ScreenDcPolicy {
                profile_id: "null-window-primary-screen-same-thread-v1".to_owned(),
                require_primary_monitor: true,
                require_same_thread_release: true,
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
                profile_id: "initial-window-metrics-1920x1080-small-icon-16x16-v3".to_owned(),
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
            system_menu: Some(User32SystemMenuPolicy {
                profile_id: "live-window-system-menu-close-item-enabled-v1".to_owned(),
                revert: 0,
                item: 0x0000_f060,
                flags: 0,
            }),
        }
    }

    pub fn controlled_v31() -> Self {
        let mut policy = Self::controlled_v30();
        policy.schema_version = USER32_COMPATIBILITY_POLICY_TIMER_VERSION;
        policy.id =
            "user32-single-monitor-window-frame-change-show-update-paint-window-timer-delivery-v31"
                .to_owned();
        policy.window_timer_delivery = Some(User32WindowTimerDeliveryPolicy {
            profile_id: "current-thread-empty-queue-coalesced-window-timer-callback-v1".to_owned(),
            message: WM_TIMER,
            callback_result: 0,
            require_empty_queue: true,
            coalesce_missed_periods: true,
        });
        policy
    }

    pub fn controlled_v32() -> Self {
        let mut policy = Self::controlled_v31();
        policy.schema_version = USER32_COMPATIBILITY_POLICY_POINTER_VERSION;
        policy.id =
            "user32-single-monitor-window-frame-change-show-update-paint-window-timer-pointer-message-v32"
                .to_owned();
        policy.pointer_button_messages = Some(RuntimePointerButtonMessageProfile {
            id: "normalized-primary-secondary-foreground-client-message-v1".to_owned(),
            normalized_position_denominator: 65_536,
            left_button_down_message: WM_LBUTTONDOWN,
            left_button_up_message: WM_LBUTTONUP,
            right_button_down_message: WM_RBUTTONDOWN,
            right_button_up_message: WM_RBUTTONUP,
            left_button_key_mask: MK_LBUTTON,
            right_button_key_mask: MK_RBUTTON,
            shift_key_mask: MK_SHIFT,
            control_key_mask: MK_CONTROL,
            require_visible_active_focused_foreground_window: true,
        });
        policy
    }

    pub fn controlled_v33() -> Self {
        let mut policy = Self::controlled_v32();
        policy.schema_version = USER32_COMPATIBILITY_POLICY_SET_FOCUS_VERSION;
        policy.id =
            "user32-single-monitor-window-frame-change-show-update-paint-window-timer-pointer-message-set-focus-v33"
                .to_owned();
        policy.set_focus = Some(User32SetFocusPolicy {
            profile_id: "same-current-thread-active-focused-foreground-window-v1".to_owned(),
            require_current_thread_active_focused_foreground_window: true,
        });
        policy
    }

    pub fn controlled_v34() -> Self {
        let mut policy = Self::controlled_v33();
        policy.schema_version = USER32_COMPATIBILITY_POLICY_POINTER_DOWN_VERSION;
        policy.id =
            "user32-single-monitor-window-timer-pointer-set-focus-def-window-proc-pointer-down-v34"
                .to_owned();
        policy.def_window_proc_a_pointer_down = Some(User32DefWindowProcAPointerDownPolicy {
            profile_id:
                "same-current-thread-visible-active-focused-foreground-cursor-wm-lbuttondown-v1"
                    .to_owned(),
            message: WM_LBUTTONDOWN,
            w_param: MK_LBUTTON,
            result: 0,
            require_current_thread_visible_active_focused_foreground_window: true,
            require_cursor_derived_client_l_param: true,
        });
        policy
    }

    pub fn controlled_v35() -> Self {
        let mut policy = Self::controlled_v34();
        policy.schema_version = USER32_COMPATIBILITY_POLICY_VERSION;
        policy.id =
            "user32-single-monitor-window-timer-pointer-set-focus-def-window-proc-pointer-down-release-v35"
                .to_owned();
        policy.def_window_proc_a_pointer_release = Some(User32DefWindowProcAPointerDownPolicy {
            profile_id:
                "same-current-thread-visible-active-focused-foreground-cursor-wm-lbuttonup-v1"
                    .to_owned(),
            message: WM_LBUTTONUP,
            w_param: 0,
            result: 0,
            require_current_thread_visible_active_focused_foreground_window: true,
            require_cursor_derived_client_l_param: true,
        });
        policy
    }

    pub fn validate(&self) -> Result<(), ApiFault> {
        if self.schema != USER32_COMPATIBILITY_POLICY_SCHEMA
            || !matches!(
                (
                    self.schema_version,
                    self.window_timer_delivery.is_some(),
                    self.pointer_button_messages.is_some(),
                    self.set_focus.is_some(),
                    self.def_window_proc_a_pointer_down.is_some(),
                    self.def_window_proc_a_pointer_release.is_some()
                ),
                (
                    USER32_COMPATIBILITY_POLICY_BASE_VERSION,
                    false,
                    false,
                    false,
                    false,
                    false
                ) | (
                    USER32_COMPATIBILITY_POLICY_TIMER_VERSION,
                    true,
                    false,
                    false,
                    false,
                    false
                ) | (
                    USER32_COMPATIBILITY_POLICY_POINTER_VERSION,
                    true,
                    true,
                    false,
                    false,
                    false
                ) | (
                    USER32_COMPATIBILITY_POLICY_SET_FOCUS_VERSION,
                    true,
                    true,
                    true,
                    false,
                    false
                ) | (
                    USER32_COMPATIBILITY_POLICY_POINTER_DOWN_VERSION,
                    true,
                    true,
                    true,
                    true,
                    false
                ) | (
                    USER32_COMPATIBILITY_POLICY_VERSION,
                    true,
                    true,
                    true,
                    true,
                    true
                )
            )
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
            let work_rect = profile.primary_work_rect;
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
                || work_rect.left < rect.left
                || work_rect.top < rect.top
                || work_rect.right <= work_rect.left
                || work_rect.bottom <= work_rect.top
                || work_rect.right > rect.right
                || work_rect.bottom > rect.bottom
            {
                return Err(ApiFault::runtime_defect(
                    "user32-compatibility-policy",
                    "user32-monitor-profile-invalid",
                    "User32 single-monitor profile requires bounded monitor and contained work rectangles",
                ));
            }
        }
        if !valid_profile_id(&self.input.id) {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-input-profile-invalid",
                "User32 logical input profile requires a bounded nonempty ASCII identity",
            ));
        }
        let cursor_position = self.input.cursor_position.as_ref();
        let monitor_rect = self
            .enum_display_monitors
            .as_ref()
            .map(|profile| profile.primary_monitor_rect);
        if cursor_position.is_none_or(|cursor| {
            !valid_profile_id(&cursor.id)
                || monitor_rect.is_none_or(|rect| {
                    cursor.initial_screen_x < rect.left
                        || cursor.initial_screen_x >= rect.right
                        || cursor.initial_screen_y < rect.top
                        || cursor.initial_screen_y >= rect.bottom
                })
        }) {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-cursor-position-profile-invalid",
                "User32 cursor position requires a bounded initial point inside the logical primary monitor",
            ));
        }
        if let Some(profile) = self.swap_mouse_button.as_ref()
            && (!valid_profile_id(&profile.profile_id) || profile.profile_id != self.input.id)
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-swap-mouse-button-profile-invalid",
                "User32 SwapMouseButton policy must reference the configured logical input profile",
            ));
        }
        if let Some(profile) = self.get_async_key_state.as_ref() {
            let keyboard = self.input.keyboard.as_ref();
            if !valid_profile_id(&profile.profile_id)
                || profile.minimum_virtual_key != 0
                || profile.maximum_virtual_key != 0xff
                || !profile.require_source_focus
                || !profile.require_focused_foreground_window
                || !profile.consume_pressed_since_last_query
                || keyboard.is_none_or(|keyboard| {
                    keyboard.id != profile.profile_id
                        || !keyboard.initial_source_focused
                        || keyboard.maximum_virtual_key != profile.maximum_virtual_key
                        || !keyboard.aggregate_modifier_keys
                        || !keyboard.clear_on_source_focus_loss
                })
            {
                return Err(ApiFault::runtime_defect(
                    "user32-compatibility-policy",
                    "user32-get-async-key-state-profile-invalid",
                    "User32 GetAsyncKeyState policy requires the bounded focused 8-bit virtual-key input profile",
                ));
            }
        }
        if let Some(profile) = self.peek_message_a.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.window != 0
                || profile.minimum_message != 0
                || profile.maximum_message != 0
                || profile.no_remove_flag != PM_NOREMOVE
                || profile.remove_flag != PM_REMOVE
                || profile.message_byte_length != X86_MSG_BYTE_LENGTH
                || self.input.cursor_position.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-peek-message-a-profile-invalid",
                "User32 PeekMessageA policy requires the reached current-thread unfiltered PM_NOREMOVE/PM_REMOVE MSG profile",
            ));
        }
        if let Some(profile) = self.is_iconic.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.minimized_style_mask != RUNTIME_WINDOW_STYLE_MINIMIZED)
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-is-iconic-profile-invalid",
                "User32 IsIconic policy requires the x86 WS_MINIMIZE effective-style bit",
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
        if let Some(profile) = self.send_message_a_ime_control.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.message != USER32_WM_IME_CONTROL
                || profile.close_status_window_command != USER32_IMC_CLOSESTATUSWINDOW
                || profile.open_status_window_command != USER32_IMC_OPENSTATUSWINDOW
                || profile.close_status_window_command == profile.open_status_window_command
                || profile.l_param != 0
                || self.create_window_ex_a.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-send-message-a-ime-control-profile-invalid",
                "User32 IME control profile requires the reached same-thread default IME status-window commands",
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
                || profile.activation_flags != 0x0000_010a
                || profile.activation_internal_flags != 0x0000_0003
                || profile.activation_insert_after != 0
                || profile.resize_outer_width == 0
                || profile.resize_outer_height == 0
                || profile.resize_client_width == 0
                || profile.resize_client_height == 0
                || profile.resize_client_width >= profile.resize_outer_width
                || profile.resize_client_height >= profile.resize_outer_height
                || profile.resize_nccalc_window_pos_flags != 0x0000_190e
                || profile.resize_changed_flags != 0x0000_110e
                || profile.frame_change_insert_after != HWND_TOP
                || profile.frame_change_move_x != 317
                || profile.frame_change_move_y != 154
                || profile.frame_change_move_flags != 0x0000_0025
                || profile.frame_change_nccalc_window_pos_flags != 0x0000_1825
                || profile.frame_change_changed_flags != 0x0000_0825
                || profile.activate_app_other_thread_id != 0
                || profile.ime_set_context_flags != 0xc000_000f
                || profile.ime_notify_command != 2
                || self.send_message_a.is_none()
                || self.create_window_ex_a.is_none()
                || self.get_system_metrics.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-set-window-pos-profile-invalid",
                "User32 SetWindowPos profile requires the reached logical window, small-icon message, display, show/hide, resize, and frame-change move callback sequences",
            ));
        }
        if let Some(profile) = self.show_window.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.normal_command != 1
                || !profile.require_visible
                || !profile.require_active
                || !profile.require_focused
                || !profile.require_foreground
                || self.set_window_pos.is_none()
                || self.create_window_ex_a.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-show-window-profile-invalid",
                "User32 ShowWindow profile requires the reached already-visible active SW_SHOWNORMAL no-op variant",
            ));
        }
        if let Some(profile) = self.update_window.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.paint_message != WM_PAINT
                || profile.callback_result != 0
                || profile.paint_struct_byte_length != RUNTIME_X86_PAINT_STRUCT_BYTE_LENGTH as u32
                || !profile.erase_pending
                || !profile.require_visible
                || !profile.require_active
                || !profile.require_focused
                || !profile.require_foreground
                || self.show_window.is_none()
                || self.set_window_pos.is_none()
                || self.create_window_ex_a.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-update-window-profile-invalid",
                "User32 UpdateWindow profile requires the reached visible active full-client WM_PAINT, BeginPaint, and EndPaint variant",
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
        if let Some(profile) = self.set_timer.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.timer_id == 0
                || profile.interval_milliseconds < RUNTIME_USER_TIMER_MINIMUM_MILLISECONDS
                || profile.message != WM_TIMER
                || self.create_window_ex_a.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-set-timer-profile-invalid",
                "User32 SetTimer profile requires a live logical window, a nonzero timer identity, a bounded interval, and WM_TIMER delivery semantics",
            ));
        }
        if let Some(profile) = self.window_timer_delivery.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.message != WM_TIMER
                || profile.callback_result != 0
                || !profile.require_empty_queue
                || !profile.coalesce_missed_periods
                || self.peek_message_a.is_none()
                || self
                    .set_timer
                    .as_ref()
                    .is_none_or(|timer| timer.message != profile.message || timer.timer_id == 0))
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-window-timer-delivery-profile-invalid",
                "User32 window timer delivery requires the reached empty current-thread queue, coalesced WM_TIMER, and live SetTimer profile",
            ));
        }
        if let Some(profile) = self.pointer_button_messages.as_ref()
            && (!valid_profile_id(&profile.id)
                || profile.normalized_position_denominator != 65_536
                || profile.left_button_down_message != WM_LBUTTONDOWN
                || profile.left_button_up_message != WM_LBUTTONUP
                || profile.right_button_down_message != WM_RBUTTONDOWN
                || profile.right_button_up_message != WM_RBUTTONUP
                || profile.left_button_key_mask != MK_LBUTTON
                || profile.right_button_key_mask != MK_RBUTTON
                || profile.shift_key_mask != MK_SHIFT
                || profile.control_key_mask != MK_CONTROL
                || !profile.require_visible_active_focused_foreground_window
                || self.input.keyboard.is_none()
                || self.input.cursor_position.is_none()
                || self.peek_message_a.is_none()
                || self.create_window_ex_a.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-pointer-button-message-profile-invalid",
                "User32 pointer messages require normalized primary/secondary input for the visible active focused foreground logical window",
            ));
        }
        if let Some(profile) = self.set_focus.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.profile_id != "same-current-thread-active-focused-foreground-window-v1"
                || !profile.require_current_thread_active_focused_foreground_window
                || self.pointer_button_messages.is_none()
                || self.create_window_ex_a.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-set-focus-profile-invalid",
                "User32 SetFocus requires the reached same-current-thread active, focused, foreground logical window profile",
            ));
        }
        if let Some(profile) = self.def_window_proc_a_pointer_down.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.profile_id
                    != "same-current-thread-visible-active-focused-foreground-cursor-wm-lbuttondown-v1"
                || profile.message != WM_LBUTTONDOWN
                || profile.w_param != MK_LBUTTON
                || profile.result != 0
                || !profile.require_current_thread_visible_active_focused_foreground_window
                || !profile.require_cursor_derived_client_l_param
                || self.pointer_button_messages.as_ref().is_none_or(|pointer| {
                    pointer.left_button_down_message != profile.message
                        || pointer.left_button_key_mask != profile.w_param
                        || !pointer.require_visible_active_focused_foreground_window
                })
                || self.set_focus.is_none()
                || self.create_window_ex_a.is_none()
                || self.input.cursor_position.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-def-window-proc-a-pointer-down-profile-invalid",
                "User32 DefWindowProcA pointer-down default processing requires the reached same-current-thread visible active focused foreground cursor-derived WM_LBUTTONDOWN profile",
            ));
        }
        if let Some(profile) = self.def_window_proc_a_pointer_release.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.profile_id
                    != "same-current-thread-visible-active-focused-foreground-cursor-wm-lbuttonup-v1"
                || profile.message != WM_LBUTTONUP
                || profile.w_param != 0
                || profile.result != 0
                || !profile.require_current_thread_visible_active_focused_foreground_window
                || !profile.require_cursor_derived_client_l_param
                || self.pointer_button_messages.as_ref().is_none_or(|pointer| {
                    pointer.left_button_up_message != profile.message
                        || !pointer.require_visible_active_focused_foreground_window
                })
                || self.def_window_proc_a_pointer_down.is_none()
                || self.set_focus.is_none()
                || self.create_window_ex_a.is_none()
                || self.input.cursor_position.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-def-window-proc-a-pointer-release-profile-invalid",
                "User32 DefWindowProcA pointer-release default processing requires the reached same-current-thread visible active focused foreground cursor-derived WM_LBUTTONUP profile",
            ));
        }
        if let Some(profile) = self.monitor_from_window.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.flags != MONITOR_DEFAULTTOPRIMARY
                || self.enum_display_monitors.is_none()
                || self.create_window_ex_a.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-monitor-from-window-profile-invalid",
                "User32 MonitorFromWindow profile requires the reached live-window primary-monitor fallback",
            ));
        }
        if let Some(profile) = self.get_monitor_info_a.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.structure_byte_length != MONITORINFO_BYTE_LENGTH
                || self.enum_display_monitors.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-get-monitor-info-a-profile-invalid",
                "User32 GetMonitorInfoA profile requires the reached 40-byte logical primary monitor structure",
            ));
        }
        if let Some(profile) = self.screen_dc.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || !profile.require_primary_monitor
                || !profile.require_same_thread_release
                || self.enum_display_monitors.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-screen-dc-profile-invalid",
                "User32 screen DC profile requires the configured primary monitor and same-thread release relation",
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
            let create = self.create_window_ex_a.as_ref().ok_or_else(|| {
                ApiFault::runtime_defect(
                    "user32-compatibility-policy",
                    "user32-set-window-pos-profile-invalid",
                    "User32 SetWindowPos resize requires the controlled window creation profile",
                )
            })?;
            if profile.resize_outer_width == create.outer_width
                || profile.resize_outer_height == create.outer_height
                || profile
                    .resize_outer_width
                    .checked_sub(profile.resize_client_width)
                    != create.outer_width.checked_sub(create.client_width)
                || profile
                    .resize_outer_height
                    .checked_sub(profile.resize_client_height)
                    != create.outer_height.checked_sub(create.client_height)
            {
                return Err(ApiFault::runtime_defect(
                    "user32-compatibility-policy",
                    "user32-set-window-pos-profile-invalid",
                    "User32 SetWindowPos resize must change both outer dimensions while preserving the controlled non-client extents",
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
        if let Some(profile) = self.system_menu.as_ref()
            && (!valid_profile_id(&profile.profile_id)
                || profile.revert != 0
                || profile.item != 0x0000_f060
                || profile.flags != 0
                || self.create_window_ex_a.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-system-menu-profile-invalid",
                "User32 system-menu policy requires the reached FALSE-revert, SC_CLOSE, MF_BYCOMMAND|MF_ENABLED variant and live-window capability",
            ));
        }
        Ok(())
    }

    pub fn display_profile(&self) -> Option<RuntimeDisplayProfile> {
        self.enum_display_monitors.clone()
    }

    pub fn input_profile(&self) -> Option<RuntimeInputProfile> {
        Some(self.input.clone())
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
        SHOW_WINDOW_IMPLEMENTATION => match handle_show_window(policy, contract, frame, session) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        UPDATE_WINDOW_IMPLEMENTATION => {
            match handle_update_window(policy, callback_return_sentinel, contract, frame, session) {
                Ok(result) => result,
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        BEGIN_PAINT_IMPLEMENTATION => match handle_begin_paint(policy, contract, frame, session) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        END_PAINT_IMPLEMENTATION => match handle_end_paint(policy, contract, frame, session) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        SET_WINDOW_LONG_A_IMPLEMENTATION => {
            match handle_set_window_long_a(policy, contract, frame, session) {
                Ok(immediate) => ApiHandlerResult::Immediate(immediate),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        SET_TIMER_IMPLEMENTATION => match handle_set_timer(policy, contract, frame, session) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        MONITOR_FROM_WINDOW_IMPLEMENTATION => {
            match handle_monitor_from_window(policy, contract, frame, session) {
                Ok(immediate) => ApiHandlerResult::Immediate(immediate),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        GET_MONITOR_INFO_A_IMPLEMENTATION => {
            match handle_get_monitor_info_a(policy, contract, frame, session) {
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
        GET_WINDOW_RECT_IMPLEMENTATION => match handle_get_window_rect(contract, frame, session) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        GET_CURSOR_POS_IMPLEMENTATION => match handle_get_cursor_pos(contract, frame, session) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        PEEK_MESSAGE_A_IMPLEMENTATION => {
            match handle_peek_message_a(policy, contract, frame, session) {
                Ok(immediate) => ApiHandlerResult::Immediate(immediate),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        TRANSLATE_MESSAGE_IMPLEMENTATION => {
            match handle_translate_message(policy, contract, frame, session) {
                Ok(immediate) => ApiHandlerResult::Immediate(immediate),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        DISPATCH_MESSAGE_A_IMPLEMENTATION => {
            match handle_dispatch_message_a(
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
        IS_ICONIC_IMPLEMENTATION => match handle_is_iconic(policy, contract, frame, session) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        GET_FOREGROUND_WINDOW_IMPLEMENTATION => {
            match handle_get_foreground_window(contract, session) {
                Ok(immediate) => ApiHandlerResult::Immediate(immediate),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        GET_ASYNC_KEY_STATE_IMPLEMENTATION => {
            match handle_get_async_key_state(policy, contract, frame, session) {
                Ok(immediate) => ApiHandlerResult::Immediate(immediate),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        SET_FOCUS_IMPLEMENTATION => match handle_set_focus(policy, contract, frame, session) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        SET_RECT_IMPLEMENTATION => match handle_set_rect(contract, frame) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        GET_DC_IMPLEMENTATION => match handle_get_dc(policy, contract, frame, session) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        FILL_RECT_IMPLEMENTATION => match handle_fill_rect(contract, frame, session) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        RELEASE_DC_IMPLEMENTATION => match handle_release_dc(policy, contract, frame, session) {
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
        GET_SYSTEM_MENU_IMPLEMENTATION => {
            match handle_get_system_menu(policy, contract, frame, session) {
                Ok(immediate) => ApiHandlerResult::Immediate(immediate),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        ENABLE_MENU_ITEM_IMPLEMENTATION => {
            match handle_enable_menu_item(policy, contract, frame, session) {
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

fn handle_get_system_menu(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let profile = policy.system_menu.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-menus",
            "get-system-menu-policy-disabled",
            format!(
                "compatibility policy {} does not enable the reached logical system-menu query",
                policy.id
            ),
        )
    })?;
    let window = required_argument(contract, frame, 0, "window")?;
    let revert = required_argument(contract, frame, 1, "revert")?;
    if revert != profile.revert {
        return Err(ApiFault::unsupported(
            "user32-menus",
            "get-system-menu-variant-unsupported",
            "GetSystemMenu currently supports only the reached FALSE-revert query",
        ));
    }
    if window == 0 {
        return Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
            namespace: LOGICAL_MENU_HANDLE_NAMESPACE.to_owned(),
            value: 0,
        })
        .with_last_error(ERROR_INVALID_WINDOW_HANDLE));
    }
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    match session.runtime_mut().get_system_menu(window, profile.item) {
        Ok(result) => Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
            namespace: LOGICAL_MENU_HANDLE_NAMESPACE.to_owned(),
            value: result.handle,
        })
        .with_last_error(preserved_last_error)),
        Err(error) if error.class == RuntimeStateErrorClass::NotFound => {
            Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
                namespace: LOGICAL_MENU_HANDLE_NAMESPACE.to_owned(),
                value: 0,
            })
            .with_last_error(ERROR_INVALID_WINDOW_HANDLE))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_enable_menu_item(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let profile = policy.system_menu.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-menus",
            "enable-menu-item-policy-disabled",
            format!(
                "compatibility policy {} does not enable the reached logical system-menu item update",
                policy.id
            ),
        )
    })?;
    let menu = required_argument(contract, frame, 0, "menu")?;
    let item = required_argument(contract, frame, 1, "item")?;
    let flags = required_argument(contract, frame, 2, "flags")?;
    if item != profile.item || flags != profile.flags {
        return Err(ApiFault::unsupported(
            "user32-menus",
            "enable-menu-item-variant-unsupported",
            "EnableMenuItem currently supports only the reached SC_CLOSE MF_BYCOMMAND|MF_ENABLED update",
        ));
    }
    if menu == 0 {
        return Ok(ApiImmediate::new(ApiReturnValue::U32(u32::MAX))
            .with_last_error(ERROR_INVALID_MENU_HANDLE));
    }
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    match session
        .runtime_mut()
        .enable_system_menu_close_item(menu, item)
    {
        Ok(result) => Ok(
            ApiImmediate::new(ApiReturnValue::U32(result.previous_state))
                .with_last_error(preserved_last_error),
        ),
        Err(error) if error.class == RuntimeStateErrorClass::NotFound => {
            Ok(ApiImmediate::new(ApiReturnValue::U32(u32::MAX))
                .with_last_error(ERROR_INVALID_MENU_HANDLE))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
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
    if mapping.profile_id != profile.profile_id
        || mapping.profile_id != policy.input.id
        || mapping.initial_swapped != policy.input.initial_mouse_buttons_swapped
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
        || monitor.work_rect != display.primary_work_rect
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

fn handle_get_window_rect(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let window_handle = required_argument(contract, frame, 0, "window handle")?;
    let output = required_argument(contract, frame, 1, "window rectangle")?;
    let window = match session.runtime().window(window_handle) {
        Ok(window) => window,
        Err(error) if error.class == RuntimeStateErrorClass::NotFound => {
            return Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
                .with_last_error(ERROR_INVALID_WINDOW_HANDLE));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, &contract.endpoint)),
    };
    let geometry = window.creation.geometry;
    let width = i32::try_from(geometry.width).map_err(|_| {
        ApiFault::runtime_defect(
            "user32-windows",
            "get-window-rect-width-invalid",
            "logical outer width does not fit an x86 RECT",
        )
    })?;
    let height = i32::try_from(geometry.height).map_err(|_| {
        ApiFault::runtime_defect(
            "user32-windows",
            "get-window-rect-height-invalid",
            "logical outer height does not fit an x86 RECT",
        )
    })?;
    let right = geometry.x.checked_add(width).ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-windows",
            "get-window-rect-right-invalid",
            "logical outer right coordinate does not fit an x86 RECT",
        )
    })?;
    let bottom = geometry.y.checked_add(height).ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-windows",
            "get-window-rect-bottom-invalid",
            "logical outer bottom coordinate does not fit an x86 RECT",
        )
    })?;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let bytes = [geometry.x, geometry.y, right, bottom]
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect();
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
        .with_last_error(preserved_last_error)
        .with_guest_write(GuestAddress(output), bytes))
}

fn handle_get_cursor_pos(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let output = required_argument(contract, frame, 0, "cursor position")?;
    if output == 0 {
        return Ok(ApiImmediate::new(ApiReturnValue::Bool32(false)).with_last_error(ERROR_NOACCESS));
    }
    let cursor = session.runtime().cursor_position().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-input",
            "cursor-position-profile-unconfigured",
            "logical cursor position must be configured before GetCursorPos is queried",
        )
    })?;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let bytes = [cursor.screen_x, cursor.screen_y]
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect();
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
        .with_last_error(preserved_last_error)
        .with_guest_write(GuestAddress(output), bytes))
}

fn handle_peek_message_a(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let profile = policy.peek_message_a.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-messages",
            "peek-message-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable current-thread message peeking",
                policy.id
            ),
        )
    })?;
    let output = required_argument(contract, frame, 0, "message output")?;
    let window = required_argument(contract, frame, 1, "window filter")?;
    let minimum_message = required_argument(contract, frame, 2, "minimum message")?;
    let maximum_message = required_argument(contract, frame, 3, "maximum message")?;
    let remove_flag = required_argument(contract, frame, 4, "remove flag")?;
    if window != profile.window
        || minimum_message != profile.minimum_message
        || maximum_message != profile.maximum_message
        || ![profile.no_remove_flag, profile.remove_flag].contains(&remove_flag)
    {
        return Err(ApiFault::unsupported(
            "user32-messages",
            "peek-message-a-variant-unsupported",
            "PeekMessageA currently supports only the controlled current-thread unfiltered PM_NOREMOVE/PM_REMOVE variants",
        ));
    }
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let remove = remove_flag == profile.remove_flag;
    let window_timer_message = policy
        .window_timer_delivery
        .as_ref()
        .map(|delivery| delivery.message);
    let (disposition, write) = session
        .peek_current_thread_message(GuestAddress(output), remove, window_timer_message)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    match disposition {
        RuntimeMessagePeekDisposition::Empty => {
            if write.is_some() {
                return Err(ApiFault::runtime_defect(
                    "user32-messages",
                    "peek-message-a-empty-write-unexpected",
                    "empty message peek unexpectedly committed guest output",
                ));
            }
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
                .with_last_error(preserved_last_error))
        }
        RuntimeMessagePeekDisposition::Message { message, removed } => {
            if removed != remove {
                return Err(ApiFault::runtime_defect(
                    "user32-messages",
                    "peek-message-a-remove-disposition-mismatch",
                    "message queue commit did not match the requested remove disposition",
                ));
            }
            let bytes = message.to_x86_msg_bytes();
            let write = write.ok_or_else(|| {
                ApiFault::runtime_defect(
                    "user32-messages",
                    "peek-message-a-message-write-missing",
                    "nonempty message peek did not commit the MSG output",
                )
            })?;
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
                .with_committed_guest_write(ApiGuestWriteReceipt {
                    write: ApiGuestWrite {
                        address: GuestAddress(output),
                        bytes,
                    },
                    result: write,
                })
                .with_last_error(preserved_last_error))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct X86Message {
    window: u32,
    message: u32,
    w_param: u32,
    l_param: u32,
    time: u32,
}

fn read_x86_message(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<X86Message, ApiFault> {
    let address = required_argument(contract, frame, 0, "message")?;
    let bytes = session
        .memory()
        .read(GuestAddress(address), X86_MSG_BYTE_LENGTH as usize)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let word = |offset, label| {
        read_u32(&bytes, offset).ok_or_else(|| {
            ApiFault::runtime_defect(
                "user32-messages",
                "message-structure-read-incomplete",
                format!("validated MSG32 omitted its {label} field"),
            )
        })
    };
    Ok(X86Message {
        window: word(0, "window")?,
        message: word(4, "message")?,
        w_param: word(8, "wParam")?,
        l_param: word(12, "lParam")?,
        time: word(16, "time")?,
    })
}

fn validate_window_timer_message(
    policy: &User32CompatibilityPolicy,
    message: X86Message,
    contract: &ApiContract,
    session: &ExecutionSession<'_>,
) -> Result<GuestAddress, ApiFault> {
    let delivery_policy = policy.window_timer_delivery.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-timers",
            "window-timer-delivery-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical window timer delivery",
                policy.id
            ),
        )
    })?;
    let timer_policy = policy.set_timer.as_ref().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-timers",
            "window-timer-registration-policy-missing",
            "validated window timer delivery policy has no SetTimer registration profile",
        )
    })?;
    if message.message != delivery_policy.message || message.w_param != timer_policy.timer_id {
        return Err(ApiFault::unsupported(
            "user32-timers",
            "window-timer-message-variant-unsupported",
            "message is outside the reached live-window WM_TIMER callback profile",
        ));
    }
    session
        .runtime()
        .resolve_window_timer_callback(
            message.window,
            message.w_param,
            GuestAddress(message.l_param),
        )
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueuedMessageDispatch {
    WindowTimer { callback: GuestAddress, result: u32 },
    WindowProcedure { callback: GuestAddress },
}

fn validate_queued_message_dispatch(
    policy: &User32CompatibilityPolicy,
    message: X86Message,
    contract: &ApiContract,
    session: &ExecutionSession<'_>,
) -> Result<QueuedMessageDispatch, ApiFault> {
    if policy
        .window_timer_delivery
        .as_ref()
        .is_some_and(|delivery| message.message == delivery.message)
    {
        let callback = validate_window_timer_message(policy, message, contract, session)?;
        return Ok(QueuedMessageDispatch::WindowTimer {
            callback,
            result: policy
                .window_timer_delivery
                .as_ref()
                .expect("validated window timer delivery policy")
                .callback_result,
        });
    }
    let profile = policy.pointer_button_messages.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-messages",
            "queued-window-message-variant-unsupported",
            "message is outside the reached window-timer and pointer-button profiles",
        )
    })?;
    let expected_button_state = match message.message {
        value if value == profile.left_button_down_message => {
            Some((profile.left_button_key_mask, true))
        }
        value if value == profile.left_button_up_message => {
            Some((profile.left_button_key_mask, false))
        }
        value if value == profile.right_button_down_message => {
            Some((profile.right_button_key_mask, true))
        }
        value if value == profile.right_button_up_message => {
            Some((profile.right_button_key_mask, false))
        }
        _ => None,
    }
    .ok_or_else(|| {
        ApiFault::unsupported(
            "user32-messages",
            "queued-window-message-variant-unsupported",
            "message is outside the reached window-timer and pointer-button profiles",
        )
    })?;
    let allowed_key_state = profile.left_button_key_mask
        | profile.right_button_key_mask
        | profile.shift_key_mask
        | profile.control_key_mask;
    if message.w_param & !allowed_key_state != 0
        || (message.w_param & expected_button_state.0 != 0) != expected_button_state.1
    {
        return Err(ApiFault::invalid_call(
            "user32-messages",
            "pointer-button-message-key-state-invalid",
            "queued pointer message has an invalid button or modifier key-state mask",
        ));
    }
    let callback = session
        .runtime()
        .resolve_window_procedure(message.window)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?
        .address;
    Ok(QueuedMessageDispatch::WindowProcedure { callback })
}

fn handle_translate_message(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let message = read_x86_message(contract, frame, session)?;
    validate_queued_message_dispatch(policy, message, contract, session)?;
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(false)))
}

fn handle_dispatch_message_a(
    policy: &User32CompatibilityPolicy,
    callback_return_sentinel: GuestAddress,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiHandlerResult, ApiFault> {
    let message = read_x86_message(contract, frame, session)?;
    let dispatch = validate_queued_message_dispatch(policy, message, contract, session)?;
    let (callback, l_param, completion_policy) = match dispatch {
        QueuedMessageDispatch::WindowTimer { callback, result } => (
            callback,
            message.time,
            GuestCallbackCompletionPolicy::ReturnConstantU32 { value: result },
        ),
        QueuedMessageDispatch::WindowProcedure { callback } => (
            callback,
            message.l_param,
            GuestCallbackCompletionPolicy::ReturnU32,
        ),
    };
    let request = GuestCallbackRequest {
        target: callback,
        arguments: vec![
            GuestCallbackArgument::U32(message.window),
            GuestCallbackArgument::U32(message.message),
            GuestCallbackArgument::U32(message.w_param),
            GuestCallbackArgument::U32(l_param),
        ],
        scratch: Vec::new(),
        scratch_pointer_relocations: Vec::new(),
        return_sentinel: callback_return_sentinel,
        completion_policy,
    };
    session.preflight_guest_callback(&ApiGuestCallbackCall {
        request: Box::new(request.clone()),
        contract: contract.clone(),
        frame: frame.clone(),
    })?;
    Ok(ApiHandlerResult::GuestCallback(request))
}

fn handle_is_iconic(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let profile = policy.is_iconic.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-windows",
            "is-iconic-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical iconic-state queries",
                policy.id
            ),
        )
    })?;
    if profile.minimized_style_mask != RUNTIME_WINDOW_STYLE_MINIMIZED {
        return Err(ApiFault::runtime_defect(
            "user32-windows",
            "is-iconic-style-mask-invalid",
            "validated IsIconic policy changed its minimized style bit",
        ));
    }
    let window_handle = required_argument(contract, frame, 0, "window handle")?;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    match session.runtime().window_is_iconic(window_handle) {
        Ok(iconic) => {
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(iconic))
                .with_last_error(preserved_last_error))
        }
        Err(error) if error.code == "window-handle-unknown" => {
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
                .with_last_error(ERROR_INVALID_WINDOW_HANDLE))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_get_foreground_window(
    contract: &ApiContract,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let handle = session
        .runtime()
        .foreground_window_handle()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?
        .unwrap_or(0);
    Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: LOGICAL_WINDOW_HANDLE_NAMESPACE.to_owned(),
        value: handle,
    }))
}

fn handle_set_focus(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let profile = policy.set_focus.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-windows",
            "set-focus-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical focus assignment",
                policy.id
            ),
        )
    })?;
    if profile.profile_id != "same-current-thread-active-focused-foreground-window-v1"
        || !profile.require_current_thread_active_focused_foreground_window
    {
        return Err(ApiFault::runtime_defect(
            "user32-windows",
            "set-focus-profile-invalid",
            "validated SetFocus policy changed its bounded current-window relation",
        ));
    }
    let window_handle = required_argument(contract, frame, 0, "window handle")?;
    let previous_window = session
        .runtime()
        .set_focus_same_window(window_handle)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: LOGICAL_WINDOW_HANDLE_NAMESPACE.to_owned(),
        value: previous_window,
    }))
}

fn handle_get_async_key_state(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let profile = policy.get_async_key_state.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-input",
            "get-async-key-state-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical asynchronous key state",
                policy.id
            ),
        )
    })?;
    let virtual_key = required_argument(contract, frame, 0, "virtual key")?;
    let keyboard = session.runtime().keyboard_state().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-input",
            "logical-keyboard-profile-missing",
            "User32 policy is enabled but the Rust input subsystem has no keyboard profile",
        )
    })?;
    if keyboard.profile_id != profile.profile_id
        || keyboard.maximum_virtual_key != profile.maximum_virtual_key
        || !keyboard.aggregate_modifier_keys
        || !keyboard.clear_on_source_focus_loss
    {
        return Err(ApiFault::runtime_defect(
            "user32-input",
            "logical-keyboard-profile-mismatch",
            "Rust input authority does not match the selected User32 keyboard profile",
        ));
    }
    if virtual_key < u32::from(profile.minimum_virtual_key)
        || virtual_key > u32::from(profile.maximum_virtual_key)
    {
        return Ok(ApiImmediate::new(ApiReturnValue::I16(0)));
    }
    if profile.require_source_focus && !keyboard.source_focused {
        return Ok(ApiImmediate::new(ApiReturnValue::I16(0)));
    }
    if profile.require_focused_foreground_window {
        let focused = session
            .runtime()
            .focused_window_handle()
            .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
        let foreground = session
            .runtime()
            .foreground_window_handle()
            .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
        if focused.is_none() || focused != foreground {
            return Ok(ApiImmediate::new(ApiReturnValue::I16(0)));
        }
    }
    let result = session
        .runtime_mut()
        .query_async_key_state(virtual_key)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?
        .ok_or_else(|| {
            ApiFault::runtime_defect(
                "user32-input",
                "logical-virtual-key-range-mismatch",
                "validated User32 virtual key is outside the Rust input authority",
            )
        })?;
    Ok(ApiImmediate::new(ApiReturnValue::I16(result.value)))
}

fn handle_set_rect(contract: &ApiContract, frame: &ApiCallFrame) -> Result<ApiImmediate, ApiFault> {
    let output = required_argument(contract, frame, 0, "rectangle")?;
    let left = required_argument(contract, frame, 1, "left")? as i32;
    let top = required_argument(contract, frame, 2, "top")? as i32;
    let right = required_argument(contract, frame, 3, "right")? as i32;
    let bottom = required_argument(contract, frame, 4, "bottom")? as i32;
    if output == 0 {
        return Ok(ApiImmediate::new(ApiReturnValue::Bool32(false)));
    }
    let bytes = [left, top, right, bottom]
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect();
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
        .with_guest_write(GuestAddress(output), bytes))
}

fn handle_monitor_from_window(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let monitor_policy = policy.monitor_from_window.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-display",
            "monitor-from-window-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical monitor lookup from a window",
                policy.id
            ),
        )
    })?;
    let window_handle = required_argument(contract, frame, 0, "window handle")?;
    let flags = required_argument(contract, frame, 1, "monitor fallback flags")?;
    if flags != monitor_policy.flags {
        return Err(ApiFault::unsupported(
            "user32-display",
            "monitor-from-window-flags-unsupported",
            "MonitorFromWindow currently supports only the reached MONITOR_DEFAULTTOPRIMARY flag",
        ));
    }
    match session.runtime().window(window_handle) {
        Ok(_) => {}
        Err(error) if error.class == RuntimeStateErrorClass::NotFound => {
            return Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
                namespace: LOGICAL_MONITOR_HANDLE_NAMESPACE.to_owned(),
                value: 0,
            })
            .with_last_error(ERROR_INVALID_WINDOW_HANDLE));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
    let display = policy.enum_display_monitors.as_ref().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-display",
            "monitor-from-window-display-policy-missing",
            "validated MonitorFromWindow policy has no logical display profile",
        )
    })?;
    let monitor = session.runtime().primary_monitor().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-display",
            "logical-primary-monitor-missing",
            "MonitorFromWindow policy is enabled but RuntimeDisplays has no primary monitor",
        )
    })?;
    if session.runtime().display_profile_id() != Some(display.id.as_str())
        || !monitor.primary
        || monitor.rect != display.primary_monitor_rect
        || monitor.work_rect != display.primary_work_rect
    {
        return Err(ApiFault::runtime_defect(
            "user32-display",
            "logical-display-profile-mismatch",
            "Rust display authority does not match the selected User32 compatibility profile",
        ));
    }
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: LOGICAL_MONITOR_HANDLE_NAMESPACE.to_owned(),
        value: monitor.handle,
    })
    .with_last_error(preserved_last_error))
}

fn handle_get_monitor_info_a(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let monitor_info_policy = policy.get_monitor_info_a.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-display",
            "get-monitor-info-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical monitor information",
                policy.id
            ),
        )
    })?;
    let monitor_handle = required_argument(contract, frame, 0, "monitor handle")?;
    let output = required_argument(contract, frame, 1, "monitor information")?;
    let display = policy.enum_display_monitors.as_ref().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-display",
            "get-monitor-info-a-display-policy-missing",
            "validated GetMonitorInfoA policy has no logical display profile",
        )
    })?;
    let monitor = match session.runtime().monitor(monitor_handle) {
        Ok(monitor) => monitor,
        Err(error) if error.class == RuntimeStateErrorClass::NotFound => {
            return Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
                .with_last_error(ERROR_INVALID_MONITOR_HANDLE));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, &contract.endpoint)),
    };
    if session.runtime().display_profile_id() != Some(display.id.as_str())
        || !monitor.primary
        || monitor.rect != display.primary_monitor_rect
        || monitor.work_rect != display.primary_work_rect
    {
        return Err(ApiFault::runtime_defect(
            "user32-display",
            "logical-display-profile-mismatch",
            "Rust display authority does not match the selected User32 compatibility profile",
        ));
    }
    let rect = monitor.rect;
    let work_rect = monitor.work_rect;
    let size_bytes = session
        .memory()
        .read(GuestAddress(output), 4)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let structure_byte_length = read_u32(&size_bytes, 0).ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-display",
            "get-monitor-info-a-size-read-incomplete",
            "validated MONITORINFO size field could not be decoded",
        )
    })?;
    if structure_byte_length == MONITORINFOEXA_BYTE_LENGTH {
        return Err(ApiFault::unsupported(
            "user32-display",
            "get-monitor-info-a-ex-variant-unsupported",
            "GetMonitorInfoA currently supports only the target-reached 40-byte MONITORINFO variant",
        ));
    }
    if structure_byte_length != monitor_info_policy.structure_byte_length {
        return Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
            .with_last_error(ERROR_INVALID_PARAMETER));
    }
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let mut bytes = Vec::with_capacity(MONITORINFO_BYTE_LENGTH as usize);
    bytes.extend_from_slice(&MONITORINFO_BYTE_LENGTH.to_le_bytes());
    bytes.extend_from_slice(&rect.to_x86_bytes());
    bytes.extend_from_slice(&work_rect.to_x86_bytes());
    bytes.extend_from_slice(&MONITORINFOF_PRIMARY.to_le_bytes());
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
        .with_last_error(preserved_last_error)
        .with_guest_write(GuestAddress(output), bytes))
}

fn handle_get_dc(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let window_handle = required_argument(contract, frame, 0, "window handle")?;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let acquisition = if window_handle == 0 {
        validate_screen_dc_authority(policy, session)?;
        session
            .runtime_mut()
            .acquire_screen_dc()
            .map(|result| result.handle)
    } else {
        session
            .runtime_mut()
            .acquire_window_dc(window_handle)
            .map(|result| result.handle)
    };
    match acquisition {
        Ok(result) => Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
            namespace: LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE.to_owned(),
            value: result,
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
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let window_handle = required_argument(contract, frame, 0, "window handle")?;
    let device_context = required_argument(contract, frame, 1, "device context")?;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    if window_handle == 0 {
        validate_screen_dc_authority(policy, session)?;
        return match session.runtime_mut().release_screen_dc(device_context) {
            Ok(()) => Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
                .with_last_error(preserved_last_error)),
            Err(error)
                if error.code == "device-context-handle-unknown"
                    || error.code == "screen-device-context-relation-mismatch"
                    || error.code == "device-context-screen-target-mismatch" =>
            {
                Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
                    .with_last_error(preserved_last_error))
            }
            Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
        };
    }
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

fn validate_screen_dc_authority(
    policy: &User32CompatibilityPolicy,
    session: &ExecutionSession<'_>,
) -> Result<(), ApiFault> {
    let screen_dc = policy.screen_dc.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-gdi",
            "screen-dc-policy-disabled",
            format!(
                "compatibility policy {} does not enable NULL-window screen DC access",
                policy.id
            ),
        )
    })?;
    let display = policy.display_profile().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-gdi",
            "screen-dc-display-policy-missing",
            "validated screen DC policy has no logical display profile",
        )
    })?;
    let monitor = session.runtime().primary_monitor().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-gdi",
            "screen-dc-display-authority-missing",
            "screen DC policy is enabled but RuntimeDisplays has no primary monitor",
        )
    })?;
    if session.runtime().display_profile_id() != Some(display.id.as_str())
        || monitor.rect != display.primary_monitor_rect
        || monitor.work_rect != display.primary_work_rect
        || (screen_dc.require_primary_monitor && !monitor.primary)
    {
        return Err(ApiFault::runtime_defect(
            "user32-gdi",
            "screen-dc-display-authority-mismatch",
            "screen DC policy and Rust display authority do not identify the same primary monitor",
        ));
    }
    Ok(())
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
    let window_handle = required_argument(contract, frame, 0, "window handle")?;
    let message = required_argument(contract, frame, 1, "message")?;
    let w_param = required_argument(contract, frame, 2, "wParam")?;
    let l_param = required_argument(contract, frame, 3, "lParam")?;

    if let Some(ime_policy) = policy.send_message_a_ime_control.as_ref()
        && message == ime_policy.message
    {
        if l_param != ime_policy.l_param {
            return Err(ApiFault::unsupported(
                "user32-windows",
                "send-message-a-ime-control-lparam-unsupported",
                "SendMessageA WM_IME_CONTROL requires the evidenced zero lParam",
            ));
        }
        let request = if w_param == ime_policy.close_status_window_command {
            RuntimeImeStatusWindowRequest::Close
        } else if w_param == ime_policy.open_status_window_command {
            RuntimeImeStatusWindowRequest::Open
        } else {
            return Err(ApiFault::unsupported(
                "user32-windows",
                "send-message-a-ime-control-command-unsupported",
                "SendMessageA WM_IME_CONTROL supports only the evidenced close/open status-window commands",
            ));
        };
        session
            .runtime_mut()
            .control_ime_status_window(window_handle, request)
            .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
        return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(
            ApiReturnValue::U32(0),
        )));
    }

    let message_policy = policy.send_message_a.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-windows",
            "send-message-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable synchronous app-window messages",
                policy.id
            ),
        )
    })?;
    if message != message_policy.message {
        return Err(ApiFault::unsupported(
            "user32-windows",
            "send-message-a-message-unsupported",
            "SendMessageA message is outside the selected app-window and IME control profiles",
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
        request: Box::new(request.clone()),
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

fn handle_set_timer(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let timer_policy = policy.set_timer.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-timers",
            "set-timer-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical window timer registration",
                policy.id
            ),
        )
    })?;
    let window_handle = required_argument(contract, frame, 0, "window handle")?;
    let timer_id = required_argument(contract, frame, 1, "timer ID")?;
    let interval_milliseconds = required_argument(contract, frame, 2, "interval")?;
    let callback = required_argument(contract, frame, 3, "timer callback")?;
    if timer_id != timer_policy.timer_id
        || interval_milliseconds != timer_policy.interval_milliseconds
        || callback == 0
    {
        return Err(ApiFault::unsupported(
            "user32-timers",
            "set-timer-variant-unsupported",
            "SetTimer arguments are outside the reached live-window ID 1, 20 ms TIMERPROC registration",
        ));
    }
    session
        .memory()
        .validate_access(GuestAddress(callback), 1, MemoryAccess::Execute)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    match session.runtime_mut().register_window_timer(
        window_handle,
        timer_id,
        interval_milliseconds,
        GuestAddress(callback),
    ) {
        Ok(result) if result.timer_id == timer_id => {
            Ok(ApiImmediate::new(ApiReturnValue::U32(result.timer_id))
                .with_last_error(preserved_last_error))
        }
        Ok(_) => Err(ApiFault::runtime_defect(
            "user32-timers",
            "set-timer-result-identity-mismatch",
            "logical timer authority returned a different window timer identity",
        )),
        Err(error) if error.code == "window-handle-unknown" => {
            Ok(ApiImmediate::new(ApiReturnValue::U32(0))
                .with_last_error(ERROR_INVALID_WINDOW_HANDLE))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_show_window(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let show_policy = policy.show_window.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-windows",
            "show-window-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical window visibility queries",
                policy.id
            ),
        )
    })?;
    let window_handle = required_argument(contract, frame, 0, "window handle")?;
    let command = required_argument(contract, frame, 1, "show command")?;
    if command != show_policy.normal_command {
        return Err(ApiFault::unsupported(
            "user32-windows",
            "show-window-command-unsupported",
            "ShowWindow command is outside the reached SW_SHOWNORMAL already-visible variant",
        ));
    }
    let entry_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let window = match session.runtime().window(window_handle) {
        Ok(window) => window,
        Err(error) if error.code == "window-handle-unknown" => {
            return Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
                .with_last_error(ERROR_INVALID_WINDOW_HANDLE));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, &contract.endpoint)),
    };
    if window.creation.owner_thread_id != session.runtime().current_thread_id()
        || window.lifecycle != RuntimeWindowLifecycle::Created
    {
        return Err(ApiFault::invalid_call(
            "user32-windows",
            "show-window-owner-lifecycle-mismatch",
            "reached ShowWindow variant requires the selected owner thread and a created logical window",
        ));
    }
    let position_policy = policy.set_window_pos.as_ref().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-windows",
            "show-window-position-policy-missing",
            "validated ShowWindow policy has no SetWindowPos state profile",
        )
    })?;
    let geometry = window.creation.geometry;
    if window.visible != show_policy.require_visible
        || window.creation.effective_style & RUNTIME_WINDOW_STYLE_VISIBLE == 0
        || window.topmost
        || geometry.x != position_policy.frame_change_move_x
        || geometry.y != position_policy.frame_change_move_y
        || geometry.width != position_policy.resize_outer_width
        || geometry.height != position_policy.resize_outer_height
        || geometry.client_width != position_policy.resize_client_width
        || geometry.client_height != position_policy.resize_client_height
    {
        return Err(ApiFault::invalid_call(
            "user32-windows",
            "show-window-state-mismatch",
            "logical window state is outside the reached already-visible normal no-op variant",
        ));
    }
    let active = session
        .runtime()
        .active_window_handle()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let focused = session
        .runtime()
        .focused_window_handle()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let foreground = session
        .runtime()
        .foreground_window_handle()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    if active == show_policy.require_active.then_some(window_handle)
        && focused == show_policy.require_focused.then_some(window_handle)
        && foreground == show_policy.require_foreground.then_some(window_handle)
    {
        return Ok(ApiImmediate::new(ApiReturnValue::Bool32(window.visible))
            .with_last_error(entry_last_error));
    }
    Err(ApiFault::invalid_call(
        "user32-windows",
        "show-window-relation-mismatch",
        "logical active, focus, or foreground relation is outside the reached ShowWindow variant",
    ))
}

fn handle_update_window(
    policy: &User32CompatibilityPolicy,
    callback_return_sentinel: GuestAddress,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiHandlerResult, ApiFault> {
    let update_policy = policy.update_window.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-windows",
            "update-window-policy-disabled",
            format!(
                "compatibility policy {} does not enable synchronous logical window painting",
                policy.id
            ),
        )
    })?;
    let window_handle = required_argument(contract, frame, 0, "window handle")?;
    let entry_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let window = match session.runtime().window(window_handle) {
        Ok(window) => window,
        Err(error) if error.code == "window-handle-unknown" => {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(ApiReturnValue::Bool32(false))
                    .with_last_error(ERROR_INVALID_WINDOW_HANDLE),
            ));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, &contract.endpoint)),
    };
    let active = session
        .runtime()
        .active_window_handle()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let focused = session
        .runtime()
        .focused_window_handle()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let foreground = session
        .runtime()
        .foreground_window_handle()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    if window.visible != update_policy.require_visible
        || active != update_policy.require_active.then_some(window_handle)
        || focused != update_policy.require_focused.then_some(window_handle)
        || foreground != update_policy.require_foreground.then_some(window_handle)
    {
        return Err(ApiFault::invalid_call(
            "user32-windows",
            "update-window-relation-mismatch",
            "logical visibility, active, focus, or foreground relation is outside the reached synchronous paint variant",
        ));
    }
    let operation = session
        .runtime()
        .preview_update_window(RuntimeUpdateWindowRequest {
            window_handle,
            entry_last_error,
            expected_callback_result: update_policy.callback_result,
        })
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let procedure = session
        .runtime()
        .resolve_window_procedure(window_handle)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let request = update_window_callback_request(
        operation,
        procedure,
        update_policy.paint_message,
        callback_return_sentinel,
    )?;
    session.preflight_guest_callback(&ApiGuestCallbackCall {
        request: Box::new(request.clone()),
        contract: contract.clone(),
        frame: frame.clone(),
    })?;
    Ok(ApiHandlerResult::GuestCallback(request))
}

fn handle_begin_paint(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let paint_policy = policy.update_window.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-paint",
            "begin-paint-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical paint sessions",
                policy.id
            ),
        )
    })?;
    let window_handle = required_argument(contract, frame, 0, "window handle")?;
    let paint_struct_address =
        GuestAddress(required_argument(contract, frame, 1, "PAINTSTRUCT output")?);
    let entry_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    match session.runtime().window(window_handle) {
        Ok(_) => {}
        Err(error) if error.code == "window-handle-unknown" => {
            return Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
                namespace: LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE.to_owned(),
                value: 0,
            })
            .with_last_error(ERROR_INVALID_WINDOW_HANDLE));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
    let (paint_session, device_context, write_result) = session
        .begin_window_paint(RuntimeBeginPaintRequest {
            window_handle,
            paint_struct_address,
            erase: paint_policy.erase_pending,
        })
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    debug_assert_eq!(
        paint_session.paint_struct_bytes.len(),
        paint_policy.paint_struct_byte_length as usize
    );
    debug_assert_eq!(
        paint_session.device_context_id,
        device_context.device_context_id
    );
    debug_assert_eq!(paint_session.device_context_handle, device_context.handle);
    Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE.to_owned(),
        value: device_context.handle,
    })
    .with_committed_guest_write(ApiGuestWriteReceipt {
        write: ApiGuestWrite {
            address: paint_struct_address,
            bytes: paint_session.paint_struct_bytes,
        },
        result: write_result,
    })
    .with_last_error(entry_last_error))
}

fn handle_end_paint(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let paint_policy = policy.update_window.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-paint",
            "end-paint-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical paint sessions",
                policy.id
            ),
        )
    })?;
    let window_handle = required_argument(contract, frame, 0, "window handle")?;
    let paint_struct_address =
        GuestAddress(required_argument(contract, frame, 1, "PAINTSTRUCT input")?);
    let entry_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    match session.runtime().window(window_handle) {
        Ok(_) => {}
        Err(error) if error.code == "window-handle-unknown" => {
            return Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
                .with_last_error(ERROR_INVALID_WINDOW_HANDLE));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
    let paint_struct_bytes = session
        .memory()
        .read(
            paint_struct_address,
            paint_policy.paint_struct_byte_length as usize,
        )
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    session
        .end_window_paint(RuntimeEndPaintRequest {
            window_handle,
            paint_struct_address,
            paint_struct_bytes,
        })
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(true)).with_last_error(entry_last_error))
}

pub(crate) fn resolve_update_window_callback(
    call: &ApiGuestCallbackCall,
    result: u32,
    session: &ExecutionSession<'_>,
) -> ApiHandlerResult {
    let GuestCallbackCompletionPolicy::UpdateWindow { operation } = call.request.completion_policy
    else {
        return ApiHandlerResult::Fault(ApiFault::runtime_defect(
            "user32-windows",
            "update-window-callback-policy-mismatch",
            "UpdateWindow callback continuation has the wrong completion policy",
        ));
    };
    match session
        .runtime()
        .preview_update_window_callback(operation, result)
    {
        Ok(RuntimeUpdateWindowCallbackAction::Completed { .. }) => ApiHandlerResult::Immediate(
            ApiImmediate::new(ApiReturnValue::Bool32(true))
                .with_last_error(operation.entry_last_error),
        ),
        Err(error) => ApiHandlerResult::Fault(ApiFault::from_runtime(error, &call.frame.endpoint)),
    }
}

fn update_window_callback_request(
    operation: RuntimeUpdateWindowOperation,
    procedure: RuntimeWindowProcedure,
    paint_message: u32,
    return_sentinel: GuestAddress,
) -> Result<GuestCallbackRequest, ApiFault> {
    if procedure.calling_convention != RuntimeWindowProcedureCallingConvention::Stdcall
        || procedure.argument_bytes != RUNTIME_WINDOW_PROCEDURE_ARGUMENT_BYTES
    {
        return Err(ApiFault::runtime_defect(
            "user32-windows",
            "update-window-procedure-abi-invalid",
            "logical WM_PAINT target does not use the required four-u32 stdcall ABI",
        ));
    }
    Ok(GuestCallbackRequest {
        target: procedure.address,
        arguments: vec![
            GuestCallbackArgument::U32(operation.window_handle),
            GuestCallbackArgument::U32(paint_message),
            GuestCallbackArgument::U32(0),
            GuestCallbackArgument::U32(0),
        ],
        scratch: Vec::new(),
        scratch_pointer_relocations: Vec::new(),
        return_sentinel,
        completion_policy: GuestCallbackCompletionPolicy::UpdateWindow { operation },
    })
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
    let entry_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let frame_change_move_request = insert_after == position_policy.frame_change_insert_after
        && x == position_policy.frame_change_move_x
        && y == position_policy.frame_change_move_y
        && width == 0
        && height == 0
        && flags == position_policy.frame_change_move_flags;
    if insert_after != position_policy.insert_after && !frame_change_move_request {
        return Err(ApiFault::unsupported(
            "user32-windows",
            "set-window-pos-insert-after-unsupported",
            "SetWindowPos insert-after identity is outside the reached HWND_NOTOPMOST and NOZORDER HWND_TOP variants",
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
        || monitor.work_rect != display.primary_work_rect
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
    let window = match session.runtime().window(window_handle) {
        Ok(window) => window.clone(),
        Err(error) if error.code == "window-handle-unknown" => {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(ApiReturnValue::Bool32(false))
                    .with_last_error(ERROR_INVALID_WINDOW_HANDLE),
            ));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, &contract.endpoint)),
    };
    let (variant, next_geometry, next_visible, callback_flags, callback_insert_after) = if width
        == 0
        && height == 0
        && flags == position_policy.initial_show_flags
        && x == show_x
        && y == show_y
    {
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
            position_policy.insert_after,
        )
    } else if width == 0
        && height == 0
        && flags == position_policy.initial_hide_flags
        && x == 0
        && y == 0
    {
        (
            RuntimeSetWindowPosVariant::InitialHide,
            window.creation.geometry,
            false,
            position_policy.initial_hide_callback_flags,
            position_policy.insert_after,
        )
    } else if flags == position_policy.activation_flags
        && x == 0
        && y == 0
        && width == window.creation.geometry.width
        && height == window.creation.geometry.height
    {
        (
            RuntimeSetWindowPosVariant::ActivateSameGeometry,
            window.creation.geometry,
            true,
            position_policy.activation_flags,
            position_policy.insert_after,
        )
    } else if flags == position_policy.activation_flags
        && x == 0
        && y == 0
        && width == position_policy.resize_outer_width
        && height == position_policy.resize_outer_height
    {
        let mut geometry = window.creation.geometry;
        geometry.width = position_policy.resize_outer_width;
        geometry.height = position_policy.resize_outer_height;
        geometry.client_width = position_policy.resize_client_width;
        geometry.client_height = position_policy.resize_client_height;
        (
            RuntimeSetWindowPosVariant::Resize,
            geometry,
            true,
            position_policy.activation_flags,
            position_policy.insert_after,
        )
    } else if frame_change_move_request {
        if window.creation.geometry.width != position_policy.resize_outer_width
            || window.creation.geometry.height != position_policy.resize_outer_height
            || window.creation.geometry.client_width != position_policy.resize_client_width
            || window.creation.geometry.client_height != position_policy.resize_client_height
        {
            return Err(ApiFault::invalid_call(
                "user32-windows",
                "set-window-pos-frame-change-move-geometry-invalid",
                "frame-change move requires the reached resized outer and client geometry",
            ));
        }
        let offset_x = window
            .creation
            .geometry
            .client_x
            .checked_sub(window.creation.geometry.x)
            .ok_or_else(|| {
                ApiFault::runtime_defect(
                    "user32-windows",
                    "set-window-pos-frame-change-client-offset-x-invalid",
                    "validated frame-change client x offset underflowed",
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
                    "set-window-pos-frame-change-client-offset-y-invalid",
                    "validated frame-change client y offset underflowed",
                )
            })?;
        let mut geometry = window.creation.geometry;
        geometry.x = position_policy.frame_change_move_x;
        geometry.y = position_policy.frame_change_move_y;
        geometry.client_x = geometry.x.checked_add(offset_x).ok_or_else(|| {
            ApiFault::runtime_defect(
                "user32-windows",
                "set-window-pos-frame-change-client-x-overflow",
                "validated frame-change client x position overflowed",
            )
        })?;
        geometry.client_y = geometry.y.checked_add(offset_y).ok_or_else(|| {
            ApiFault::runtime_defect(
                "user32-windows",
                "set-window-pos-frame-change-client-y-overflow",
                "validated frame-change client y position overflowed",
            )
        })?;
        (
            RuntimeSetWindowPosVariant::FrameChangeMove,
            geometry,
            true,
            position_policy.frame_change_move_flags,
            position_policy.frame_change_insert_after,
        )
    } else {
        let (code, message) = if width != 0 || height != 0 {
            (
                "set-window-pos-size-unsupported",
                "SetWindowPos size is outside the reached same-geometry activation variant",
            )
        } else {
            (
                "set-window-pos-variant-unsupported",
                "SetWindowPos arguments are outside the reached initial show/hide profile",
            )
        };
        return Err(ApiFault::unsupported("user32-windows", code, message));
    };
    let operation = session
        .runtime()
        .preview_set_window_pos(RuntimeSetWindowPosRequest {
            window_handle,
            variant,
            next_geometry,
            next_visible,
            next_topmost: false,
            entry_last_error,
            callback_flags,
            callback_insert_after,
            activation_insert_after: position_policy.activation_insert_after,
            activation_internal_flags: position_policy.activation_internal_flags,
            resize_nccalc_window_pos_flags: position_policy.resize_nccalc_window_pos_flags,
            resize_changed_flags: position_policy.resize_changed_flags,
            frame_change_nccalc_window_pos_flags: position_policy
                .frame_change_nccalc_window_pos_flags,
            frame_change_changed_flags: position_policy.frame_change_changed_flags,
        })
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let class = session
        .runtime()
        .window_class(window.creation.class.atom)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let first_message = match operation.variant {
        RuntimeSetWindowPosVariant::InitialShow | RuntimeSetWindowPosVariant::InitialHide => {
            RuntimeSetWindowPosCallbackMessage::WindowPositionChanged
        }
        RuntimeSetWindowPosVariant::ActivateSameGeometry => {
            RuntimeSetWindowPosCallbackMessage::WindowPositionChanging
        }
        RuntimeSetWindowPosVariant::RepeatActivation => {
            RuntimeSetWindowPosCallbackMessage::WindowPositionChanging
        }
        RuntimeSetWindowPosVariant::Resize | RuntimeSetWindowPosVariant::FrameChangeMove => {
            RuntimeSetWindowPosCallbackMessage::WindowPositionChanging
        }
    };
    let request =
        set_window_pos_callback_request(operation, class, first_message, callback_return_sentinel)?;
    session.preflight_guest_callback(&ApiGuestCallbackCall {
        request: Box::new(request.clone()),
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
        RuntimeSetWindowPosCallbackAction::Completed { .. } => ApiHandlerResult::Immediate(
            ApiImmediate::new(ApiReturnValue::Bool32(true))
                .with_last_error(operation.entry_last_error),
        ),
    }
}

pub(crate) fn resolve_set_window_pos_default_callback(
    call: &ApiGuestCallbackCall,
    result: u32,
    session: &ExecutionSession<'_>,
) -> ApiHandlerResult {
    let GuestCallbackCompletionPolicy::SetWindowPosDefault { operation, message } =
        call.request.completion_policy
    else {
        return ApiHandlerResult::Fault(ApiFault::runtime_defect(
            "user32-windows",
            "set-window-pos-default-callback-policy-mismatch",
            "activation default callback has the wrong completion policy",
        ));
    };
    let action = match session
        .runtime()
        .preview_set_window_pos_default_callback(operation, message, result)
    {
        Ok(action) => action,
        Err(error) => {
            return ApiHandlerResult::Fault(ApiFault::from_runtime(error, &call.frame.endpoint));
        }
    };
    match action {
        RuntimeSetWindowPosDefaultCallbackAction::Continue {
            operation,
            next_message,
        } => match set_window_pos_default_callback_request(
            operation,
            next_message,
            call.request.return_sentinel,
            0,
            session,
            &call.frame.endpoint,
        ) {
            Ok(request) => ApiHandlerResult::GuestCallback(request),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        RuntimeSetWindowPosDefaultCallbackAction::Completed => {
            ApiHandlerResult::Immediate(ApiImmediate::new(ApiReturnValue::U32(0)))
        }
    }
}

fn set_window_pos_default_callback_request(
    operation: RuntimeSetWindowPosDefaultOperation,
    message: RuntimeSetWindowPosDefaultCallbackMessage,
    return_sentinel: GuestAddress,
    ime_set_context_flags: u32,
    session: &ExecutionSession<'_>,
    endpoint: &ApiEndpointIdentity,
) -> Result<GuestCallbackRequest, ApiFault> {
    let procedure = session
        .runtime()
        .resolve_window_procedure(operation.window_handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let (message_id, w_param, l_param) = match message {
        RuntimeSetWindowPosDefaultCallbackMessage::ImeSetContext => {
            (WM_IME_SETCONTEXT, 1, ime_set_context_flags)
        }
        RuntimeSetWindowPosDefaultCallbackMessage::SetFocus => (WM_SETFOCUS, 0, 0),
    };
    Ok(GuestCallbackRequest {
        target: procedure.address,
        arguments: vec![
            GuestCallbackArgument::U32(operation.window_handle),
            GuestCallbackArgument::U32(message_id),
            GuestCallbackArgument::U32(w_param),
            GuestCallbackArgument::U32(l_param),
        ],
        scratch: Vec::new(),
        scratch_pointer_relocations: Vec::new(),
        return_sentinel,
        completion_policy: GuestCallbackCompletionPolicy::SetWindowPosDefault {
            operation,
            message,
        },
    })
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
    let (message_id, w_param, l_param, scratch, scratch_pointer_relocations) = match message {
        RuntimeSetWindowPosCallbackMessage::WindowPositionChanged => {
            let flags = match operation.variant {
                RuntimeSetWindowPosVariant::Resize => operation.resize_changed_flags,
                RuntimeSetWindowPosVariant::FrameChangeMove => operation.frame_change_changed_flags,
                _ => operation.callback_flags,
            };
            let scratch = window_pos_callback_scratch(
                operation.window_handle,
                operation.callback_insert_after,
                operation.next_geometry.x,
                operation.next_geometry.y,
                operation.next_geometry.width,
                operation.next_geometry.height,
                flags,
            )?;
            (
                WM_WINDOWPOSCHANGED,
                0,
                GuestCallbackArgument::ScratchPointer { offset: 0 },
                scratch,
                Vec::new(),
            )
        }
        RuntimeSetWindowPosCallbackMessage::GetSmallIcon => (
            WM_GETICON,
            ICON_SMALL2,
            GuestCallbackArgument::U32(0),
            Vec::new(),
            Vec::new(),
        ),
        RuntimeSetWindowPosCallbackMessage::WindowPositionChanging => {
            let (x, y, width, height) =
                if operation.variant == RuntimeSetWindowPosVariant::FrameChangeMove {
                    (operation.next_geometry.x, operation.next_geometry.y, 0, 0)
                } else {
                    (
                        0,
                        0,
                        operation.next_geometry.width,
                        operation.next_geometry.height,
                    )
                };
            (
                WM_WINDOWPOSCHANGING,
                0,
                GuestCallbackArgument::ScratchPointer { offset: 0 },
                window_pos_callback_scratch(
                    operation.window_handle,
                    operation.callback_insert_after,
                    x,
                    y,
                    width,
                    height,
                    operation.callback_flags,
                )?,
                Vec::new(),
            )
        }
        RuntimeSetWindowPosCallbackMessage::NonClientCalculate => (
            WM_NCCALCSIZE,
            1,
            GuestCallbackArgument::ScratchPointer { offset: 0 },
            set_window_pos_nccalc_callback_scratch(operation)?,
            vec![GuestCallbackScratchPointerRelocation {
                write_offset: 48,
                target_offset: 52,
            }],
        ),
        RuntimeSetWindowPosCallbackMessage::Size => (
            WM_SIZE,
            0,
            GuestCallbackArgument::U32(window_creation_size_l_param(&operation.next_geometry)?),
            Vec::new(),
            Vec::new(),
        ),
        RuntimeSetWindowPosCallbackMessage::ActivationWindowPositionChanging => (
            WM_WINDOWPOSCHANGING,
            0,
            GuestCallbackArgument::ScratchPointer { offset: 0 },
            window_pos_callback_scratch(
                operation.window_handle,
                operation.activation_insert_after,
                0,
                0,
                0,
                0,
                operation.activation_internal_flags,
            )?,
            Vec::new(),
        ),
        RuntimeSetWindowPosCallbackMessage::ActivateApplication => (
            WM_ACTIVATEAPP,
            1,
            GuestCallbackArgument::U32(0),
            Vec::new(),
            Vec::new(),
        ),
        RuntimeSetWindowPosCallbackMessage::NonClientActivate => (
            WM_NCACTIVATE,
            1,
            GuestCallbackArgument::U32(0),
            Vec::new(),
            Vec::new(),
        ),
        RuntimeSetWindowPosCallbackMessage::Activate => (
            WM_ACTIVATE,
            1,
            GuestCallbackArgument::U32(0),
            Vec::new(),
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
        scratch_pointer_relocations,
        return_sentinel,
        completion_policy: GuestCallbackCompletionPolicy::SetWindowPos { operation, message },
    })
}

fn set_window_pos_nccalc_callback_scratch(
    operation: RuntimeSetWindowPosOperation,
) -> Result<Vec<u8>, ApiFault> {
    let rect = |geometry: RuntimeWindowGeometry, client: bool| -> Result<[i32; 4], ApiFault> {
        let (x, y, width, height) = if client {
            (
                geometry.client_x,
                geometry.client_y,
                geometry.client_width,
                geometry.client_height,
            )
        } else {
            (geometry.x, geometry.y, geometry.width, geometry.height)
        };
        let right = x.checked_add_unsigned(width).ok_or_else(|| {
            ApiFault::runtime_defect(
                "user32-windows",
                "set-window-pos-nccalc-right-overflow",
                "validated resize rectangle right edge overflowed",
            )
        })?;
        let bottom = y.checked_add_unsigned(height).ok_or_else(|| {
            ApiFault::runtime_defect(
                "user32-windows",
                "set-window-pos-nccalc-bottom-overflow",
                "validated resize rectangle bottom edge overflowed",
            )
        })?;
        Ok([x, y, right, bottom])
    };
    let mut scratch = Vec::with_capacity(80);
    for rectangle in [
        rect(operation.next_geometry, false)?,
        rect(operation.previous_geometry, false)?,
        rect(operation.previous_geometry, true)?,
    ] {
        for value in rectangle {
            scratch.extend_from_slice(&value.to_le_bytes());
        }
    }
    scratch.extend_from_slice(&0_u32.to_le_bytes());
    let nccalc_window_pos_flags = match operation.variant {
        RuntimeSetWindowPosVariant::Resize => operation.resize_nccalc_window_pos_flags,
        RuntimeSetWindowPosVariant::FrameChangeMove => {
            operation.frame_change_nccalc_window_pos_flags
        }
        _ => {
            return Err(ApiFault::runtime_defect(
                "user32-windows",
                "set-window-pos-nccalc-variant-invalid",
                "NCCALCSIZE scratch requires a resize or frame-change move operation",
            ));
        }
    };
    scratch.extend_from_slice(&window_pos_callback_scratch(
        operation.window_handle,
        operation.callback_insert_after,
        operation.next_geometry.x,
        operation.next_geometry.y,
        operation.next_geometry.width,
        operation.next_geometry.height,
        nccalc_window_pos_flags,
    )?);
    debug_assert_eq!(scratch.len(), 80);
    Ok(scratch)
}

fn window_pos_callback_scratch(
    window_handle: u32,
    insert_after: u32,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    flags: u32,
) -> Result<Vec<u8>, ApiFault> {
    let width = i32::try_from(width).map_err(|_| {
        ApiFault::runtime_defect(
            "user32-windows",
            "set-window-pos-callback-width-overflow",
            "validated logical window width does not fit WINDOWPOS",
        )
    })?;
    let height = i32::try_from(height).map_err(|_| {
        ApiFault::runtime_defect(
            "user32-windows",
            "set-window-pos-callback-height-overflow",
            "validated logical window height does not fit WINDOWPOS",
        )
    })?;
    Ok([
        window_handle.to_le_bytes(),
        insert_after.to_le_bytes(),
        x.to_le_bytes(),
        y.to_le_bytes(),
        width.to_le_bytes(),
        height.to_le_bytes(),
        flags.to_le_bytes(),
    ]
    .concat())
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
        request: Box::new(callback.clone()),
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
    if policy.set_window_pos.is_some() {
        if message == WM_WINDOWPOSCHANGED {
            return handle_def_window_proc_window_position(
                policy,
                callback_return_sentinel,
                contract,
                frame,
                session,
            );
        }
        if matches!(message, WM_NCCALCSIZE | WM_SIZE) {
            return handle_def_window_proc_resize(policy, contract, frame, session);
        }
        if matches!(
            message,
            WM_WINDOWPOSCHANGING
                | WM_ACTIVATEAPP
                | WM_NCACTIVATE
                | WM_ACTIVATE
                | WM_IME_SETCONTEXT
                | WM_IME_NOTIFY
                | WM_SETFOCUS
        ) {
            return handle_def_window_proc_activation(
                policy,
                callback_return_sentinel,
                contract,
                frame,
                session,
            );
        }
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
    if words[0] != handle {
        return Err(ApiFault::invalid_call(
            "user32-windows",
            "def-window-proc-position-identity-mismatch",
            "WINDOWPOS logical HWND does not match the active window",
        ));
    }
    if words[6] == position_policy.resize_changed_flags {
        let geometry = window.creation.geometry;
        if words
            != [
                handle,
                position_policy.insert_after,
                geometry.x as u32,
                geometry.y as u32,
                position_policy.resize_outer_width,
                position_policy.resize_outer_height,
                position_policy.resize_changed_flags,
            ]
            || geometry.width != position_policy.resize_outer_width
            || geometry.height != position_policy.resize_outer_height
            || geometry.client_width != position_policy.resize_client_width
            || geometry.client_height != position_policy.resize_client_height
        {
            return Err(ApiFault::invalid_call(
                "user32-windows",
                "def-window-proc-resize-changed-payload-mismatch",
                "resize WM_WINDOWPOSCHANGED does not match the committed logical geometry",
            ));
        }
        require_window_activation_relations(session, handle, true, true, true, contract)?;
        return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(
            ApiReturnValue::U32(0),
        )));
    }
    if words[6] == position_policy.frame_change_changed_flags {
        let geometry = window.creation.geometry;
        if words
            != [
                handle,
                position_policy.frame_change_insert_after,
                position_policy.frame_change_move_x as u32,
                position_policy.frame_change_move_y as u32,
                geometry.width,
                geometry.height,
                position_policy.frame_change_changed_flags,
            ]
            || geometry.x != position_policy.frame_change_move_x
            || geometry.y != position_policy.frame_change_move_y
            || geometry.width != position_policy.resize_outer_width
            || geometry.height != position_policy.resize_outer_height
            || geometry.client_width != position_policy.resize_client_width
            || geometry.client_height != position_policy.resize_client_height
        {
            return Err(ApiFault::invalid_call(
                "user32-windows",
                "def-window-proc-frame-change-changed-payload-mismatch",
                "frame-change WM_WINDOWPOSCHANGED does not match the committed logical geometry",
            ));
        }
        require_window_activation_relations(session, handle, true, true, true, contract)?;
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
                GuestCallbackArgument::U32(window_creation_move_l_param(&geometry)?),
            ],
            scratch: Vec::new(),
            scratch_pointer_relocations: Vec::new(),
            return_sentinel: callback_return_sentinel,
            completion_policy: GuestCallbackCompletionPolicy::ReturnConstantU32 { value: 0 },
        };
        session.preflight_guest_callback(&ApiGuestCallbackCall {
            request: Box::new(request.clone()),
            contract: contract.clone(),
            frame: frame.clone(),
        })?;
        return Ok(ApiHandlerResult::GuestCallback(request));
    }
    if words[1] != position_policy.insert_after {
        return Err(ApiFault::invalid_call(
            "user32-windows",
            "def-window-proc-position-identity-mismatch",
            "WINDOWPOS insert-after identity does not match the reached initial or resize variant",
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
            entry_last_error: session
                .last_error()
                .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?,
            callback_flags: words[6],
            callback_insert_after: position_policy.insert_after,
            activation_insert_after: position_policy.activation_insert_after,
            activation_internal_flags: position_policy.activation_internal_flags,
            resize_nccalc_window_pos_flags: position_policy.resize_nccalc_window_pos_flags,
            resize_changed_flags: position_policy.resize_changed_flags,
            frame_change_nccalc_window_pos_flags: position_policy
                .frame_change_nccalc_window_pos_flags,
            frame_change_changed_flags: position_policy.frame_change_changed_flags,
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
        request: Box::new(request.clone()),
        contract: contract.clone(),
        frame: frame.clone(),
    })?;
    Ok(ApiHandlerResult::GuestCallback(request))
}

fn handle_def_window_proc_resize(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiHandlerResult, ApiFault> {
    let position_policy = policy.set_window_pos.as_ref().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-windows",
            "def-window-proc-resize-policy-missing",
            "validated resize default processing has no SetWindowPos policy",
        )
    })?;
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
        return Err(ApiFault::invalid_call(
            "user32-windows",
            "def-window-proc-resize-window-state-invalid",
            "resize default processing requires the created owner-thread window",
        ));
    }
    if window.lifecycle != RuntimeWindowLifecycle::Created {
        return handle_def_window_proc_a_immediate(policy, contract, frame, session)
            .map(ApiHandlerResult::Immediate);
    }
    require_window_activation_relations(session, handle, true, true, true, contract)?;

    match message {
        WM_NCCALCSIZE => {
            if w_param != 1 {
                return Err(ApiFault::unsupported(
                    "user32-windows",
                    "def-window-proc-resize-nccalc-wparam-unsupported",
                    "resize WM_NCCALCSIZE requires the reached TRUE wParam",
                ));
            }
            let flags_address = l_param.checked_add(76).ok_or_else(|| {
                ApiFault::invalid_call(
                    "user32-windows",
                    "def-window-proc-nccalc-pointer-overflow",
                    "NCCALCSIZE_PARAMS WINDOWPOS flags pointer overflowed x86 address space",
                )
            })?;
            let flags_bytes = session
                .memory()
                .read(GuestAddress(flags_address), 4)
                .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
            let nccalc_flags =
                u32::from_le_bytes(flags_bytes.try_into().expect("WINDOWPOS flags word"));
            let (variant, next_geometry, callback_flags, callback_insert_after) = if nccalc_flags
                == position_policy.resize_nccalc_window_pos_flags
            {
                let mut geometry = window.creation.geometry;
                geometry.width = position_policy.resize_outer_width;
                geometry.height = position_policy.resize_outer_height;
                geometry.client_width = position_policy.resize_client_width;
                geometry.client_height = position_policy.resize_client_height;
                (
                    RuntimeSetWindowPosVariant::Resize,
                    geometry,
                    position_policy.activation_flags,
                    position_policy.insert_after,
                )
            } else if nccalc_flags == position_policy.frame_change_nccalc_window_pos_flags {
                if window.creation.geometry.width != position_policy.resize_outer_width
                    || window.creation.geometry.height != position_policy.resize_outer_height
                    || window.creation.geometry.client_width != position_policy.resize_client_width
                    || window.creation.geometry.client_height
                        != position_policy.resize_client_height
                {
                    return Err(ApiFault::invalid_call(
                        "user32-windows",
                        "def-window-proc-frame-change-geometry-invalid",
                        "frame-change NCCALCSIZE requires the reached resized outer and client geometry",
                    ));
                }
                let offset_x = window
                    .creation
                    .geometry
                    .client_x
                    .checked_sub(window.creation.geometry.x)
                    .ok_or_else(|| {
                        ApiFault::runtime_defect(
                            "user32-windows",
                            "def-window-proc-frame-change-client-offset-x-invalid",
                            "validated frame-change client x offset underflowed",
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
                            "def-window-proc-frame-change-client-offset-y-invalid",
                            "validated frame-change client y offset underflowed",
                        )
                    })?;
                let mut geometry = window.creation.geometry;
                geometry.x = position_policy.frame_change_move_x;
                geometry.y = position_policy.frame_change_move_y;
                geometry.client_x = geometry.x.checked_add(offset_x).ok_or_else(|| {
                    ApiFault::runtime_defect(
                        "user32-windows",
                        "def-window-proc-frame-change-client-x-overflow",
                        "validated frame-change client x position overflowed",
                    )
                })?;
                geometry.client_y = geometry.y.checked_add(offset_y).ok_or_else(|| {
                    ApiFault::runtime_defect(
                        "user32-windows",
                        "def-window-proc-frame-change-client-y-overflow",
                        "validated frame-change client y position overflowed",
                    )
                })?;
                (
                    RuntimeSetWindowPosVariant::FrameChangeMove,
                    geometry,
                    position_policy.frame_change_move_flags,
                    position_policy.frame_change_insert_after,
                )
            } else {
                return Err(ApiFault::unsupported(
                    "user32-windows",
                    "def-window-proc-nccalc-flags-unsupported",
                    "NCCALCSIZE WINDOWPOS flags are outside the reached resize and frame-change move variants",
                ));
            };
            let operation = session
                .runtime()
                .preview_set_window_pos(RuntimeSetWindowPosRequest {
                    window_handle: handle,
                    variant,
                    next_geometry,
                    next_visible: true,
                    next_topmost: false,
                    entry_last_error: session
                        .last_error()
                        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?,
                    callback_flags,
                    callback_insert_after,
                    activation_insert_after: position_policy.activation_insert_after,
                    activation_internal_flags: position_policy.activation_internal_flags,
                    resize_nccalc_window_pos_flags: position_policy.resize_nccalc_window_pos_flags,
                    resize_changed_flags: position_policy.resize_changed_flags,
                    frame_change_nccalc_window_pos_flags: position_policy
                        .frame_change_nccalc_window_pos_flags,
                    frame_change_changed_flags: position_policy.frame_change_changed_flags,
                })
                .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
            let mut expected = set_window_pos_nccalc_callback_scratch(operation)?;
            let window_pos_address = l_param.checked_add(52).ok_or_else(|| {
                ApiFault::invalid_call(
                    "user32-windows",
                    "def-window-proc-nccalc-pointer-overflow",
                    "NCCALCSIZE_PARAMS WINDOWPOS pointer overflowed x86 address space",
                )
            })?;
            expected[48..52].copy_from_slice(&window_pos_address.to_le_bytes());
            let before = session
                .memory()
                .read(GuestAddress(l_param), expected.len())
                .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
            if before != expected {
                return Err(ApiFault::invalid_call(
                    "user32-windows",
                    "def-window-proc-nccalc-payload-mismatch",
                    "NCCALCSIZE_PARAMS does not match the pending logical geometry",
                ));
            }
            let geometry = operation.next_geometry;
            let client_right = geometry
                .client_x
                .checked_add_unsigned(geometry.client_width)
                .ok_or_else(|| {
                    ApiFault::runtime_defect(
                        "user32-windows",
                        "def-window-proc-nccalc-client-right-overflow",
                        "validated client right edge overflowed",
                    )
                })?;
            let client_bottom = geometry
                .client_y
                .checked_add_unsigned(geometry.client_height)
                .ok_or_else(|| {
                    ApiFault::runtime_defect(
                        "user32-windows",
                        "def-window-proc-nccalc-client-bottom-overflow",
                        "validated client bottom edge overflowed",
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
            Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(ApiReturnValue::U32(0))
                    .with_guest_write(GuestAddress(l_param), bytes),
            ))
        }
        WM_SIZE => {
            let geometry = window.creation.geometry;
            let expected = window_creation_size_l_param(&geometry)?;
            if w_param != 0
                || l_param != expected
                || geometry.width != position_policy.resize_outer_width
                || geometry.height != position_policy.resize_outer_height
                || geometry.client_width != position_policy.resize_client_width
                || geometry.client_height != position_policy.resize_client_height
            {
                return Err(ApiFault::invalid_call(
                    "user32-windows",
                    "def-window-proc-resize-size-payload-mismatch",
                    "resize WM_SIZE does not match the committed logical client geometry",
                ));
            }
            Ok(ApiHandlerResult::Immediate(ApiImmediate::new(
                ApiReturnValue::U32(0),
            )))
        }
        _ => Err(ApiFault::runtime_defect(
            "user32-windows",
            "def-window-proc-resize-message-invalid",
            "resize default handler received an unrelated message",
        )),
    }
}

fn handle_def_window_proc_activation(
    policy: &User32CompatibilityPolicy,
    callback_return_sentinel: GuestAddress,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiHandlerResult, ApiFault> {
    let position_policy = policy.set_window_pos.as_ref().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-windows",
            "def-window-proc-activation-policy-missing",
            "validated activation default processing has no SetWindowPos policy",
        )
    })?;
    let handle = required_argument(contract, frame, 0, "window handle")?;
    let message = required_argument(contract, frame, 1, "message")?;
    let w_param = required_argument(contract, frame, 2, "wParam")?;
    let l_param = required_argument(contract, frame, 3, "lParam")?;
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
            "def-window-proc-activation-window-state-invalid",
            "activation default processing requires the created owner-thread window",
        ));
    }

    match message {
        WM_WINDOWPOSCHANGING => {
            if w_param != 0 {
                return Err(ApiFault::unsupported(
                    "user32-windows",
                    "def-window-proc-activation-wparam-unsupported",
                    "WM_WINDOWPOSCHANGING requires the reached zero wParam",
                ));
            }
            let words =
                read_window_pos_words(session.memory(), GuestAddress(l_param), &contract.endpoint)?;
            let first = words
                == [
                    handle,
                    position_policy.insert_after,
                    0,
                    0,
                    window.creation.geometry.width,
                    window.creation.geometry.height,
                    position_policy.activation_flags,
                ];
            let internal = words
                == [
                    handle,
                    position_policy.activation_insert_after,
                    0,
                    0,
                    0,
                    0,
                    position_policy.activation_internal_flags,
                ];
            let resize = words
                == [
                    handle,
                    position_policy.insert_after,
                    0,
                    0,
                    position_policy.resize_outer_width,
                    position_policy.resize_outer_height,
                    position_policy.activation_flags,
                ];
            let frame_change_move = words
                == [
                    handle,
                    position_policy.frame_change_insert_after,
                    position_policy.frame_change_move_x as u32,
                    position_policy.frame_change_move_y as u32,
                    0,
                    0,
                    position_policy.frame_change_move_flags,
                ];
            if first {
                let already_focused = session
                    .runtime()
                    .focused_window_handle()
                    .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?
                    == Some(handle);
                require_window_activation_relations(
                    session,
                    handle,
                    already_focused,
                    already_focused,
                    already_focused,
                    contract,
                )?;
            } else if internal {
                require_window_activation_relations(session, handle, true, false, true, contract)?;
            } else if resize || frame_change_move {
                require_window_activation_relations(session, handle, true, true, true, contract)?;
            } else {
                return Err(ApiFault::invalid_call(
                    "user32-windows",
                    "def-window-proc-activation-window-pos-mismatch",
                    "WM_WINDOWPOSCHANGING payload is outside the reached activation, resize, and frame-change variants",
                ));
            }
            Ok(ApiHandlerResult::Immediate(ApiImmediate::new(
                ApiReturnValue::U32(0),
            )))
        }
        WM_ACTIVATEAPP => {
            if w_param != 1 || l_param != position_policy.activate_app_other_thread_id {
                return Err(ApiFault::unsupported(
                    "user32-windows",
                    "def-window-proc-activate-app-variant-unsupported",
                    "WM_ACTIVATEAPP arguments are outside the reached logical activation variant",
                ));
            }
            require_window_activation_relations(session, handle, true, false, true, contract)?;
            Ok(ApiHandlerResult::Immediate(ApiImmediate::new(
                ApiReturnValue::U32(0),
            )))
        }
        WM_NCACTIVATE => {
            if w_param != 1 || l_param != 0 {
                return Err(ApiFault::unsupported(
                    "user32-windows",
                    "def-window-proc-nc-activate-variant-unsupported",
                    "WM_NCACTIVATE arguments are outside the reached activation variant",
                ));
            }
            require_window_activation_relations(session, handle, true, false, true, contract)?;
            Ok(ApiHandlerResult::Immediate(ApiImmediate::new(
                ApiReturnValue::U32(1),
            )))
        }
        WM_ACTIVATE => {
            if w_param != 1 || l_param != 0 {
                return Err(ApiFault::unsupported(
                    "user32-windows",
                    "def-window-proc-activate-variant-unsupported",
                    "WM_ACTIVATE arguments are outside the reached active non-minimized variant",
                ));
            }
            require_window_activation_relations(session, handle, true, false, true, contract)?;
            let preview = session
                .runtime()
                .preview_set_window_pos_focus(handle)
                .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
            let request = set_window_pos_default_callback_request(
                preview,
                RuntimeSetWindowPosDefaultCallbackMessage::ImeSetContext,
                callback_return_sentinel,
                position_policy.ime_set_context_flags,
                session,
                &contract.endpoint,
            )?;
            session.preflight_guest_callback(&ApiGuestCallbackCall {
                request: Box::new(request.clone()),
                contract: contract.clone(),
                frame: frame.clone(),
            })?;
            let committed = session
                .runtime_mut()
                .focus_set_window_pos_window(handle)
                .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
            if committed != preview {
                return Err(ApiFault::runtime_defect(
                    "user32-windows",
                    "set-window-pos-focus-preview-commit-mismatch",
                    "logical focus identity changed between callback preflight and commit",
                ));
            }
            Ok(ApiHandlerResult::GuestCallback(request))
        }
        WM_IME_SETCONTEXT => {
            if w_param != 1 || l_param != position_policy.ime_set_context_flags {
                return Err(ApiFault::unsupported(
                    "user32-windows",
                    "def-window-proc-ime-context-variant-unsupported",
                    "WM_IME_SETCONTEXT arguments are outside the reached activation variant",
                ));
            }
            require_window_activation_relations(session, handle, true, true, true, contract)?;
            let procedure = session
                .runtime()
                .resolve_window_procedure(handle)
                .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
            let request = GuestCallbackRequest {
                target: procedure.address,
                arguments: vec![
                    GuestCallbackArgument::U32(handle),
                    GuestCallbackArgument::U32(WM_IME_NOTIFY),
                    GuestCallbackArgument::U32(position_policy.ime_notify_command),
                    GuestCallbackArgument::U32(0),
                ],
                scratch: Vec::new(),
                scratch_pointer_relocations: Vec::new(),
                return_sentinel: callback_return_sentinel,
                completion_policy: GuestCallbackCompletionPolicy::ReturnConstantU32 { value: 0 },
            };
            session.preflight_guest_callback(&ApiGuestCallbackCall {
                request: Box::new(request.clone()),
                contract: contract.clone(),
                frame: frame.clone(),
            })?;
            Ok(ApiHandlerResult::GuestCallback(request))
        }
        WM_IME_NOTIFY => {
            if w_param != position_policy.ime_notify_command || l_param != 0 {
                return Err(ApiFault::unsupported(
                    "user32-windows",
                    "def-window-proc-ime-notify-variant-unsupported",
                    "WM_IME_NOTIFY arguments are outside the reached activation variant",
                ));
            }
            require_window_activation_relations(session, handle, true, true, true, contract)?;
            Ok(ApiHandlerResult::Immediate(ApiImmediate::new(
                ApiReturnValue::U32(0),
            )))
        }
        WM_SETFOCUS => {
            if w_param != 0 || l_param != 0 {
                return Err(ApiFault::unsupported(
                    "user32-windows",
                    "def-window-proc-set-focus-variant-unsupported",
                    "WM_SETFOCUS arguments are outside the reached no-previous-focus variant",
                ));
            }
            require_window_activation_relations(session, handle, true, true, true, contract)?;
            Ok(ApiHandlerResult::Immediate(ApiImmediate::new(
                ApiReturnValue::U32(0),
            )))
        }
        _ => Err(ApiFault::runtime_defect(
            "user32-windows",
            "def-window-proc-activation-message-invalid",
            "activation default handler received an unrelated message",
        )),
    }
}

fn require_window_activation_relations(
    session: &ExecutionSession<'_>,
    handle: u32,
    active: bool,
    focused: bool,
    foreground: bool,
    contract: &ApiContract,
) -> Result<(), ApiFault> {
    let active_handle = session
        .runtime()
        .active_window_handle()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let focused_handle = session
        .runtime()
        .focused_window_handle()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let foreground_handle = session
        .runtime()
        .foreground_window_handle()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    if active_handle == active.then_some(handle)
        && focused_handle == focused.then_some(handle)
        && foreground_handle == foreground.then_some(handle)
    {
        return Ok(());
    }
    Err(ApiFault::invalid_call(
        "user32-windows",
        "def-window-proc-activation-relation-mismatch",
        "logical active, focused, or foreground relation does not match callback timing",
    ))
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
    if matches!(message, WM_LBUTTONDOWN | WM_LBUTTONUP) {
        return handle_def_window_proc_pointer_button(
            policy, contract, message, w_param, l_param, &window, session,
        );
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
            let current = window_creation_move_l_param(&window.creation.geometry)?;
            let initial_show =
                window_creation_move_l_param(&initial_set_window_pos_show_geometry(
                    policy,
                    session,
                    &window,
                    &contract.endpoint,
                )?)?;
            if l_param != current && l_param != initial_show {
                return Err(ApiFault::invalid_call(
                    "user32-windows",
                    "def-window-proc-position-move-lparam-mismatch",
                    format!(
                        "DefWindowProcA positioned WM_MOVE lParam {l_param:#010x} does not match current {current:#010x} or initial-show {initial_show:#010x} client position"
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

fn handle_def_window_proc_pointer_button(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    message: u32,
    w_param: u32,
    l_param: u32,
    window: &RuntimeWindow,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let (default_profile, expected_profile_id, expected_w_param, phase) = match message {
        WM_LBUTTONDOWN => (
            policy
                .def_window_proc_a_pointer_down
                .as_ref()
                .ok_or_else(|| {
                    ApiFault::unsupported(
                        "user32-windows",
                        "def-window-proc-pointer-policy-disabled",
                        "DefWindowProcA WM_LBUTTONDOWN requires the reached pointer-down default-processing profile",
                    )
                })?,
            "same-current-thread-visible-active-focused-foreground-cursor-wm-lbuttondown-v1",
            MK_LBUTTON,
            "pointer-down",
        ),
        WM_LBUTTONUP => (
            policy
                .def_window_proc_a_pointer_release
                .as_ref()
                .ok_or_else(|| {
                    ApiFault::unsupported(
                        "user32-windows",
                        "def-window-proc-pointer-release-policy-disabled",
                        "DefWindowProcA WM_LBUTTONUP requires the reached pointer-release default-processing profile",
                    )
                })?,
            "same-current-thread-visible-active-focused-foreground-cursor-wm-lbuttonup-v1",
            0,
            "pointer-release",
        ),
        _ => {
            return Err(ApiFault::runtime_defect(
                "user32-windows",
                "def-window-proc-pointer-message-invalid",
                "pointer-button default processing received an unsupported message",
            ));
        }
    };
    let pointer_profile = policy.pointer_button_messages.as_ref().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-windows",
            "def-window-proc-pointer-dependency-missing",
            format!("validated {phase} default processing has no pointer-button message profile"),
        )
    })?;
    if default_profile.profile_id != expected_profile_id
        || default_profile.message != message
        || default_profile.w_param != expected_w_param
        || default_profile.result != 0
        || !default_profile.require_current_thread_visible_active_focused_foreground_window
        || !default_profile.require_cursor_derived_client_l_param
        || (message == WM_LBUTTONDOWN
            && (pointer_profile.left_button_down_message != default_profile.message
                || pointer_profile.left_button_key_mask != default_profile.w_param))
        || (message == WM_LBUTTONUP
            && pointer_profile.left_button_up_message != default_profile.message)
        || !pointer_profile.require_visible_active_focused_foreground_window
    {
        return Err(ApiFault::runtime_defect(
            "user32-windows",
            "def-window-proc-pointer-profile-invalid",
            format!(
                "validated {phase} default-processing policy changed the reached message relation"
            ),
        ));
    }
    if w_param != default_profile.w_param {
        return Err(ApiFault::invalid_call(
            "user32-windows",
            "def-window-proc-pointer-wparam-mismatch",
            format!(
                "DefWindowProcA message {message:#06x} requires wParam {expected_w_param:#010x}"
            ),
        ));
    }
    if policy.set_focus.is_none() || policy.create_window_ex_a.is_none() {
        return Err(ApiFault::runtime_defect(
            "user32-windows",
            "def-window-proc-pointer-window-dependency-missing",
            format!(
                "validated {phase} default processing has no SetFocus or CreateWindowExA dependency"
            ),
        ));
    }
    let handle = window.handle;
    let active_handle = session
        .runtime()
        .active_window_handle()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let focused_handle = session
        .runtime()
        .focused_window_handle()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let foreground_handle = session
        .runtime()
        .foreground_window_handle()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    if window.lifecycle != RuntimeWindowLifecycle::Created
        || !window.visible
        || window.creation.effective_style & RUNTIME_WINDOW_STYLE_VISIBLE == 0
        || active_handle != Some(handle)
        || focused_handle != Some(handle)
        || foreground_handle != Some(handle)
    {
        return Err(ApiFault::invalid_call(
            "user32-windows",
            "def-window-proc-pointer-window-relation-mismatch",
            format!(
                "DefWindowProcA message {message:#06x} requires the same visible active focused foreground logical window"
            ),
        ));
    }
    let cursor = session.runtime().cursor_position().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-windows",
            "def-window-proc-pointer-cursor-missing",
            "validated pointer-button policy has no logical cursor position",
        )
    })?;
    let geometry = window.creation.geometry;
    let client_x = cursor
        .screen_x
        .checked_sub(geometry.client_x)
        .ok_or_else(|| {
            ApiFault::invalid_call(
                "user32-windows",
                "def-window-proc-pointer-client-x-overflow",
                "logical cursor x cannot be represented relative to the target client area",
            )
        })?;
    let client_y = cursor
        .screen_y
        .checked_sub(geometry.client_y)
        .ok_or_else(|| {
            ApiFault::invalid_call(
                "user32-windows",
                "def-window-proc-pointer-client-y-overflow",
                "logical cursor y cannot be represented relative to the target client area",
            )
        })?;
    let client_width = i32::try_from(geometry.client_width).map_err(|_| {
        ApiFault::runtime_defect(
            "user32-windows",
            "def-window-proc-pointer-client-width-unrepresentable",
            "validated logical client width does not fit signed pointer coordinates",
        )
    })?;
    let client_height = i32::try_from(geometry.client_height).map_err(|_| {
        ApiFault::runtime_defect(
            "user32-windows",
            "def-window-proc-pointer-client-height-unrepresentable",
            "validated logical client height does not fit signed pointer coordinates",
        )
    })?;
    let client_x_i16 = i16::try_from(client_x).ok();
    let client_y_i16 = i16::try_from(client_y).ok();
    if client_x < 0
        || client_x >= client_width
        || client_y < 0
        || client_y >= client_height
        || client_x_i16.is_none()
        || client_y_i16.is_none()
    {
        return Err(ApiFault::invalid_call(
            "user32-windows",
            "def-window-proc-pointer-client-position-invalid",
            "logical cursor is outside the reached signed target client coordinates",
        ));
    }
    let expected_l_param = u32::from(client_x_i16.expect("validated client x") as u16)
        | (u32::from(client_y_i16.expect("validated client y") as u16) << 16);
    if l_param != expected_l_param {
        return Err(ApiFault::invalid_call(
            "user32-windows",
            "def-window-proc-pointer-lparam-mismatch",
            format!(
                "DefWindowProcA message {message:#06x} lParam {l_param:#010x} does not match the logical cursor-derived client position {expected_l_param:#010x}"
            ),
        ));
    }
    Ok(ApiImmediate::new(ApiReturnValue::U32(
        default_profile.result,
    )))
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
        || monitor.work_rect != display.primary_work_rect
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
        || monitor.work_rect != profile.primary_work_rect
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
        "ShowWindow" => Ok(Some(show_window_contract(endpoint)?)),
        "UpdateWindow" => Ok(Some(update_window_contract(endpoint)?)),
        "BeginPaint" => Ok(Some(begin_paint_contract(endpoint)?)),
        "EndPaint" => Ok(Some(end_paint_contract(endpoint)?)),
        "SetWindowLongA" => Ok(Some(set_window_long_a_contract(endpoint)?)),
        "SetTimer" => Ok(Some(set_timer_contract(endpoint)?)),
        "LoadCursorA" => Ok(Some(load_cursor_a_contract(endpoint)?)),
        "RegisterClassExA" => Ok(Some(register_class_ex_a_contract(endpoint)?)),
        "GetSystemMetrics" => Ok(Some(get_system_metrics_contract(endpoint)?)),
        "GetClientRect" => Ok(Some(get_client_rect_contract(endpoint)?)),
        "GetWindowRect" => Ok(Some(get_window_rect_contract(endpoint)?)),
        "GetCursorPos" => Ok(Some(get_cursor_pos_contract(endpoint)?)),
        "PeekMessageA" => Ok(Some(peek_message_a_contract(endpoint)?)),
        "TranslateMessage" => Ok(Some(translate_message_contract(endpoint)?)),
        "DispatchMessageA" => Ok(Some(dispatch_message_a_contract(endpoint)?)),
        "IsIconic" => Ok(Some(is_iconic_contract(endpoint)?)),
        "GetForegroundWindow" => Ok(Some(get_foreground_window_contract(endpoint)?)),
        "GetAsyncKeyState" => Ok(Some(get_async_key_state_contract(endpoint)?)),
        "SetFocus" => Ok(Some(set_focus_contract(endpoint)?)),
        "SetRect" => Ok(Some(set_rect_contract(endpoint)?)),
        "MonitorFromWindow" => Ok(Some(monitor_from_window_contract(endpoint)?)),
        "GetMonitorInfoA" => Ok(Some(get_monitor_info_a_contract(endpoint)?)),
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
        callback_policy: CallbackPolicy::Optional,
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
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Optional,
        implementation_id: SET_WINDOW_POS_IMPLEMENTATION.to_owned(),
    })
}

pub fn show_window_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                logical_window_handle_argument("window", false),
                u32_argument("showCommand"),
            ],
            return_value: ApiReturnSchema::Bool32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: SHOW_WINDOW_IMPLEMENTATION.to_owned(),
    })
}

pub fn update_window_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![logical_window_handle_argument("window", false)],
            return_value: ApiReturnSchema::Bool32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Optional,
        implementation_id: UPDATE_WINDOW_IMPLEMENTATION.to_owned(),
    })
}

pub fn begin_paint_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                logical_window_handle_argument("window", false),
                conditional_out_struct(
                    "paintStruct",
                    "PAINTSTRUCT32",
                    RUNTIME_X86_PAINT_STRUCT_BYTE_LENGTH as u32,
                ),
            ],
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
        implementation_id: BEGIN_PAINT_IMPLEMENTATION.to_owned(),
    })
}

pub fn end_paint_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                logical_window_handle_argument("window", false),
                required_in_struct(
                    "paintStruct",
                    "PAINTSTRUCT32",
                    RUNTIME_X86_PAINT_STRUCT_BYTE_LENGTH as u32,
                ),
            ],
            return_value: ApiReturnSchema::Bool32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: END_PAINT_IMPLEMENTATION.to_owned(),
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

pub fn set_timer_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                logical_window_handle_argument("window", false),
                u32_argument("timerId"),
                u32_argument("intervalMilliseconds"),
                opaque_pointer_argument("timerProcedure", false, 1),
            ],
            return_value: ApiReturnSchema::U32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: SET_TIMER_IMPLEMENTATION.to_owned(),
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

pub fn get_window_rect_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                logical_window_handle_argument("window", true),
                conditional_out_struct("windowRect", "RECT", 16),
            ],
            return_value: ApiReturnSchema::Bool32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: GET_WINDOW_RECT_IMPLEMENTATION.to_owned(),
    })
}

pub fn get_cursor_pos_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![nullable_out_struct("point", "POINT", 8)],
            return_value: ApiReturnSchema::Bool32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: GET_CURSOR_POS_IMPLEMENTATION.to_owned(),
    })
}

pub fn peek_message_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                conditional_out_struct("message", "MSG32", X86_MSG_BYTE_LENGTH),
                u32_argument("window"),
                u32_argument("minimumMessage"),
                u32_argument("maximumMessage"),
                u32_argument("removeMessage"),
            ],
            return_value: ApiReturnSchema::Bool32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: PEEK_MESSAGE_A_IMPLEMENTATION.to_owned(),
    })
}

pub fn translate_message_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![required_in_struct("message", "MSG32", X86_MSG_BYTE_LENGTH)],
            return_value: ApiReturnSchema::Bool32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: TRANSLATE_MESSAGE_IMPLEMENTATION.to_owned(),
    })
}

pub fn dispatch_message_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![required_in_struct("message", "MSG32", X86_MSG_BYTE_LENGTH)],
            return_value: ApiReturnSchema::U32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Optional,
        implementation_id: DISPATCH_MESSAGE_A_IMPLEMENTATION.to_owned(),
    })
}

pub fn is_iconic_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![logical_window_handle_argument("window", false)],
            return_value: ApiReturnSchema::Bool32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: IS_ICONIC_IMPLEMENTATION.to_owned(),
    })
}

pub fn get_foreground_window_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: Vec::new(),
            return_value: ApiReturnSchema::LogicalHandle {
                namespace: LOGICAL_WINDOW_HANDLE_NAMESPACE.to_owned(),
                nullable: true,
            },
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: GET_FOREGROUND_WINDOW_IMPLEMENTATION.to_owned(),
    })
}

pub fn get_async_key_state_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![u32_argument("virtualKey")],
            return_value: ApiReturnSchema::I16,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: GET_ASYNC_KEY_STATE_IMPLEMENTATION.to_owned(),
    })
}

pub fn set_focus_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![logical_window_handle_argument("window", true)],
            return_value: ApiReturnSchema::LogicalHandle {
                namespace: LOGICAL_WINDOW_HANDLE_NAMESPACE.to_owned(),
                nullable: true,
            },
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: SET_FOCUS_IMPLEMENTATION.to_owned(),
    })
}

pub fn set_rect_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                nullable_out_struct("rectangle", "RECT", 16),
                u32_argument("left"),
                u32_argument("top"),
                u32_argument("right"),
                u32_argument("bottom"),
            ],
            return_value: ApiReturnSchema::Bool32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: SET_RECT_IMPLEMENTATION.to_owned(),
    })
}

pub fn monitor_from_window_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                logical_window_handle_argument("window", false),
                u32_argument("flags"),
            ],
            return_value: ApiReturnSchema::LogicalHandle {
                namespace: LOGICAL_MONITOR_HANDLE_NAMESPACE.to_owned(),
                nullable: true,
            },
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: MONITOR_FROM_WINDOW_IMPLEMENTATION.to_owned(),
    })
}

pub fn get_monitor_info_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                logical_handle_argument("monitor", LOGICAL_MONITOR_HANDLE_NAMESPACE, true),
                inout_dynamic_struct("monitorInfo", "MONITORINFOA", 4, MONITORINFOEXA_BYTE_LENGTH),
            ],
            return_value: ApiReturnSchema::Bool32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: GET_MONITOR_INFO_A_IMPLEMENTATION.to_owned(),
    })
}

pub fn get_dc_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![logical_window_handle_argument("window", true)],
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
                logical_window_handle_argument("window", true),
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
                logical_handle_argument("menu", LOGICAL_MENU_HANDLE_NAMESPACE, true),
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
            return_value: ApiReturnSchema::LogicalHandle {
                namespace: LOGICAL_MENU_HANDLE_NAMESPACE.to_owned(),
                nullable: true,
            },
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
                logical_handle_argument("menu", LOGICAL_MENU_HANDLE_NAMESPACE, true),
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

fn nullable_out_struct(name: &str, type_id: &str, byte_length: u32) -> ApiArgumentSchema {
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

fn inout_dynamic_struct(
    name: &str,
    type_id: &str,
    minimum_byte_length: u32,
    maximum_byte_length: u32,
) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::InOut,
        value: ApiValueSchema::DynamicPointer {
            constraint: ApiDynamicPointerConstraint {
                nullable: false,
                alignment: 4,
                minimum_byte_length,
                maximum_byte_length,
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
        let policy = User32CompatibilityPolicy::controlled_v30();
        policy.validate().unwrap();
        let profile = policy.display_profile().unwrap();
        assert_eq!(profile.primary_monitor_rect.right, 1920);
        assert_eq!(profile.primary_monitor_rect.bottom, 1080);
        assert_eq!(profile.primary_work_rect.right, 1920);
        assert_eq!(profile.primary_work_rect.bottom, 1032);
        assert_eq!(
            policy
                .get_monitor_info_a
                .as_ref()
                .unwrap()
                .structure_byte_length,
            40
        );
        let screen_dc = policy.screen_dc.as_ref().unwrap();
        assert!(screen_dc.require_primary_monitor);
        assert!(screen_dc.require_same_thread_release);
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
        let timer = policy.set_timer.as_ref().unwrap();
        assert_eq!(timer.timer_id, 1);
        assert_eq!(timer.interval_milliseconds, 20);
        assert_eq!(timer.message, WM_TIMER);
        assert_eq!(policy.schema_version, 30);
        assert!(policy.window_timer_delivery.is_none());
        let update_window = policy.update_window.as_ref().unwrap();
        assert_eq!(update_window.paint_message, WM_PAINT);
        assert_eq!(update_window.callback_result, 0);
        let keyboard = policy
            .input_profile()
            .unwrap()
            .keyboard
            .expect("keyboard profile");
        assert_eq!(keyboard.id, "foreground-focused-virtual-key-state-v1");
        assert!(keyboard.initial_source_focused);
        assert_eq!(keyboard.maximum_virtual_key, 0xff);
        assert!(keyboard.aggregate_modifier_keys);
        assert!(keyboard.clear_on_source_focus_loss);
        let cursor_position = policy
            .input_profile()
            .unwrap()
            .cursor_position
            .expect("cursor position profile");
        assert_eq!(cursor_position.id, "primary-display-center-cursor-v1");
        assert_eq!(cursor_position.initial_screen_x, 960);
        assert_eq!(cursor_position.initial_screen_y, 540);
        let get_async_key_state = policy.get_async_key_state.as_ref().unwrap();
        assert_eq!(get_async_key_state.profile_id, keyboard.id);
        assert_eq!(get_async_key_state.minimum_virtual_key, 0);
        assert_eq!(get_async_key_state.maximum_virtual_key, 0xff);
        assert!(get_async_key_state.require_source_focus);
        assert!(get_async_key_state.require_focused_foreground_window);
        assert!(get_async_key_state.consume_pressed_since_last_query);
        let peek_message_a = policy.peek_message_a.as_ref().unwrap();
        assert_eq!(peek_message_a.window, 0);
        assert_eq!(peek_message_a.minimum_message, 0);
        assert_eq!(peek_message_a.maximum_message, 0);
        assert_eq!(peek_message_a.no_remove_flag, PM_NOREMOVE);
        assert_eq!(peek_message_a.remove_flag, PM_REMOVE);
        assert_eq!(peek_message_a.message_byte_length, X86_MSG_BYTE_LENGTH);
        let is_iconic = policy.is_iconic.as_ref().unwrap();
        assert_eq!(
            is_iconic.profile_id,
            "logical-window-effective-style-is-iconic-v1"
        );
        assert_eq!(
            is_iconic.minimized_style_mask,
            RUNTIME_WINDOW_STYLE_MINIMIZED
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

        let mut invalid_work = User32CompatibilityPolicy::controlled_v30();
        invalid_work
            .enum_display_monitors
            .as_mut()
            .unwrap()
            .primary_work_rect
            .bottom = 1081;
        assert_eq!(
            invalid_work.validate().unwrap_err().code,
            "user32-monitor-profile-invalid"
        );

        let mut invalid_input = User32CompatibilityPolicy::controlled_v30();
        invalid_input.input.id.clear();
        assert_eq!(
            invalid_input.validate().unwrap_err().code,
            "user32-input-profile-invalid"
        );

        let mut invalid_cursor_position = User32CompatibilityPolicy::controlled_v30();
        invalid_cursor_position
            .input
            .cursor_position
            .as_mut()
            .unwrap()
            .initial_screen_x = 1920;
        assert_eq!(
            invalid_cursor_position.validate().unwrap_err().code,
            "user32-cursor-position-profile-invalid"
        );

        let mut invalid_keyboard = User32CompatibilityPolicy::controlled_v30();
        invalid_keyboard
            .input
            .keyboard
            .as_mut()
            .unwrap()
            .aggregate_modifier_keys = false;
        assert_eq!(
            invalid_keyboard.validate().unwrap_err().code,
            "user32-get-async-key-state-profile-invalid"
        );

        let mut invalid_async_key = User32CompatibilityPolicy::controlled_v30();
        invalid_async_key
            .get_async_key_state
            .as_mut()
            .unwrap()
            .maximum_virtual_key = 0xfe;
        assert_eq!(
            invalid_async_key.validate().unwrap_err().code,
            "user32-get-async-key-state-profile-invalid"
        );

        let mut invalid_peek_message = User32CompatibilityPolicy::controlled_v30();
        invalid_peek_message
            .peek_message_a
            .as_mut()
            .unwrap()
            .remove_flag = 2;
        assert_eq!(
            invalid_peek_message.validate().unwrap_err().code,
            "user32-peek-message-a-profile-invalid"
        );

        let mut invalid_is_iconic = User32CompatibilityPolicy::controlled_v30();
        invalid_is_iconic
            .is_iconic
            .as_mut()
            .unwrap()
            .minimized_style_mask = 0;
        assert_eq!(
            invalid_is_iconic.validate().unwrap_err().code,
            "user32-is-iconic-profile-invalid"
        );

        let mut invalid_icon = User32CompatibilityPolicy::controlled_v30();
        invalid_icon
            .load_icon_a
            .as_mut()
            .unwrap()
            .maximum_group_entries = 0;
        assert_eq!(
            invalid_icon.validate().unwrap_err().code,
            "user32-load-icon-profile-invalid"
        );

        let mut invalid_image = User32CompatibilityPolicy::controlled_v30();
        invalid_image.load_image_a.as_mut().unwrap().flags = 0;
        assert_eq!(
            invalid_image.validate().unwrap_err().code,
            "user32-load-image-profile-invalid"
        );

        let mut invalid_message = User32CompatibilityPolicy::controlled_v30();
        invalid_message.send_message_a.as_mut().unwrap().message = 0x0081;
        assert_eq!(
            invalid_message.validate().unwrap_err().code,
            "user32-send-message-a-profile-invalid"
        );

        let mut invalid_update = User32CompatibilityPolicy::controlled_v30();
        invalid_update.update_window.as_mut().unwrap().paint_message = WM_PAINT + 1;
        assert_eq!(
            invalid_update.validate().unwrap_err().code,
            "user32-update-window-profile-invalid"
        );

        let mut invalid_window_style = User32CompatibilityPolicy::controlled_v30();
        invalid_window_style
            .set_window_long_a
            .as_mut()
            .unwrap()
            .retained_style_mask = RUNTIME_WINDOW_STYLE_VISIBLE;
        assert_eq!(
            invalid_window_style.validate().unwrap_err().code,
            "user32-set-window-long-a-profile-invalid"
        );

        let mut invalid_timer = User32CompatibilityPolicy::controlled_v30();
        invalid_timer
            .set_timer
            .as_mut()
            .unwrap()
            .interval_milliseconds = 9;
        assert_eq!(
            invalid_timer.validate().unwrap_err().code,
            "user32-set-timer-profile-invalid"
        );

        let delivery = User32CompatibilityPolicy::controlled_v31();
        delivery.validate().unwrap();
        assert_eq!(
            delivery.schema_version,
            USER32_COMPATIBILITY_POLICY_TIMER_VERSION
        );
        let delivery_profile = delivery.window_timer_delivery.as_ref().unwrap();
        assert_eq!(delivery_profile.message, WM_TIMER);
        assert_eq!(delivery_profile.callback_result, 0);
        assert!(delivery_profile.require_empty_queue);
        assert!(delivery_profile.coalesce_missed_periods);

        let mut invalid_delivery = User32CompatibilityPolicy::controlled_v31();
        invalid_delivery
            .window_timer_delivery
            .as_mut()
            .unwrap()
            .coalesce_missed_periods = false;
        assert_eq!(
            invalid_delivery.validate().unwrap_err().code,
            "user32-window-timer-delivery-profile-invalid"
        );

        let pointer = User32CompatibilityPolicy::controlled_v32();
        pointer.validate().unwrap();
        assert_eq!(
            pointer.schema_version,
            USER32_COMPATIBILITY_POLICY_POINTER_VERSION
        );
        let pointer_profile = pointer.pointer_button_messages.as_ref().unwrap();
        assert_eq!(pointer_profile.normalized_position_denominator, 65_536);
        assert_eq!(pointer_profile.left_button_down_message, WM_LBUTTONDOWN);
        assert_eq!(pointer_profile.left_button_up_message, WM_LBUTTONUP);
        assert_eq!(pointer_profile.left_button_key_mask, MK_LBUTTON);
        assert!(pointer_profile.require_visible_active_focused_foreground_window);

        let mut invalid_pointer = User32CompatibilityPolicy::controlled_v32();
        invalid_pointer
            .pointer_button_messages
            .as_mut()
            .unwrap()
            .left_button_key_mask = 0;
        assert_eq!(
            invalid_pointer.validate().unwrap_err().code,
            "user32-pointer-button-message-profile-invalid"
        );

        let focus = User32CompatibilityPolicy::controlled_v33();
        focus.validate().unwrap();
        assert_eq!(
            focus.schema_version,
            USER32_COMPATIBILITY_POLICY_SET_FOCUS_VERSION
        );
        assert!(focus.def_window_proc_a_pointer_down.is_none());
        let focus_profile = focus.set_focus.as_ref().unwrap();
        assert_eq!(
            focus_profile.profile_id,
            "same-current-thread-active-focused-foreground-window-v1"
        );
        assert!(focus_profile.require_current_thread_active_focused_foreground_window);

        let mut invalid_focus = User32CompatibilityPolicy::controlled_v33();
        invalid_focus
            .set_focus
            .as_mut()
            .unwrap()
            .require_current_thread_active_focused_foreground_window = false;
        assert_eq!(
            invalid_focus.validate().unwrap_err().code,
            "user32-set-focus-profile-invalid"
        );

        let pointer_default = User32CompatibilityPolicy::controlled_v34();
        pointer_default.validate().unwrap();
        assert_eq!(
            pointer_default.schema_version,
            USER32_COMPATIBILITY_POLICY_POINTER_DOWN_VERSION
        );
        assert!(pointer_default.def_window_proc_a_pointer_release.is_none());
        let pointer_default_profile = pointer_default
            .def_window_proc_a_pointer_down
            .as_ref()
            .unwrap();
        assert_eq!(pointer_default_profile.message, WM_LBUTTONDOWN);
        assert_eq!(pointer_default_profile.w_param, MK_LBUTTON);
        assert_eq!(pointer_default_profile.result, 0);
        assert!(
            pointer_default_profile.require_current_thread_visible_active_focused_foreground_window
        );
        assert!(pointer_default_profile.require_cursor_derived_client_l_param);

        let mut invalid_pointer_default = User32CompatibilityPolicy::controlled_v34();
        invalid_pointer_default
            .def_window_proc_a_pointer_down
            .as_mut()
            .unwrap()
            .result = 1;
        assert_eq!(
            invalid_pointer_default.validate().unwrap_err().code,
            "user32-def-window-proc-a-pointer-down-profile-invalid"
        );

        let pointer_release = User32CompatibilityPolicy::controlled_v35();
        pointer_release.validate().unwrap();
        assert_eq!(
            pointer_release.schema_version,
            USER32_COMPATIBILITY_POLICY_VERSION
        );
        let pointer_release_profile = pointer_release
            .def_window_proc_a_pointer_release
            .as_ref()
            .unwrap();
        assert_eq!(pointer_release_profile.message, WM_LBUTTONUP);
        assert_eq!(pointer_release_profile.w_param, 0);
        assert_eq!(pointer_release_profile.result, 0);
        assert!(
            pointer_release_profile.require_current_thread_visible_active_focused_foreground_window
        );
        assert!(pointer_release_profile.require_cursor_derived_client_l_param);

        let mut invalid_pointer_release = User32CompatibilityPolicy::controlled_v35();
        invalid_pointer_release
            .def_window_proc_a_pointer_release
            .as_mut()
            .unwrap()
            .w_param = MK_LBUTTON;
        assert_eq!(
            invalid_pointer_release.validate().unwrap_err().code,
            "user32-def-window-proc-a-pointer-release-profile-invalid"
        );

        let mut invalid_monitor = User32CompatibilityPolicy::controlled_v30();
        invalid_monitor.monitor_from_window.as_mut().unwrap().flags = 2;
        assert_eq!(
            invalid_monitor.validate().unwrap_err().code,
            "user32-monitor-from-window-profile-invalid"
        );

        let mut invalid_monitor_info = User32CompatibilityPolicy::controlled_v30();
        invalid_monitor_info
            .get_monitor_info_a
            .as_mut()
            .unwrap()
            .structure_byte_length = 72;
        assert_eq!(
            invalid_monitor_info.validate().unwrap_err().code,
            "user32-get-monitor-info-a-profile-invalid"
        );

        let mut invalid_screen_dc = User32CompatibilityPolicy::controlled_v30();
        invalid_screen_dc
            .screen_dc
            .as_mut()
            .unwrap()
            .require_same_thread_release = false;
        assert_eq!(
            invalid_screen_dc.validate().unwrap_err().code,
            "user32-screen-dc-profile-invalid"
        );

        let mut invalid_cursor = User32CompatibilityPolicy::controlled_v30();
        invalid_cursor
            .load_cursor_a
            .as_mut()
            .unwrap()
            .arrow_resource_id = 0x7f01;
        assert_eq!(
            invalid_cursor.validate().unwrap_err().code,
            "user32-load-cursor-profile-invalid"
        );

        let mut invalid_class = User32CompatibilityPolicy::controlled_v30();
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

        let mut invalid_metrics = User32CompatibilityPolicy::controlled_v30();
        invalid_metrics
            .get_system_metrics
            .as_mut()
            .unwrap()
            .caption_height = 0;
        assert_eq!(
            invalid_metrics.validate().unwrap_err().code,
            "user32-system-metrics-profile-invalid"
        );

        let mut invalid_small_icon_width = User32CompatibilityPolicy::controlled_v30();
        invalid_small_icon_width
            .get_system_metrics
            .as_mut()
            .unwrap()
            .small_icon_width = Some(0);
        assert_eq!(
            invalid_small_icon_width.validate().unwrap_err().code,
            "user32-system-metrics-profile-invalid"
        );

        let mut invalid_small_icon = User32CompatibilityPolicy::controlled_v30();
        invalid_small_icon
            .get_system_metrics
            .as_mut()
            .unwrap()
            .small_icon_height = Some(0);
        assert_eq!(
            invalid_small_icon.validate().unwrap_err().code,
            "user32-system-metrics-profile-invalid"
        );

        let mut invalid_menu = User32CompatibilityPolicy::controlled_v30();
        invalid_menu.system_menu.as_mut().unwrap().item = 0xf061;
        assert_eq!(
            invalid_menu.validate().unwrap_err().code,
            "user32-system-menu-profile-invalid"
        );
    }
}

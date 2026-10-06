use encoding_rs::SHIFT_JIS;

use crate::api::{
    ApiCallFrame, ApiDeferredCall, ApiDispatcher, ApiEndpointIdentity, ApiFault,
    ApiGuestCallbackCall, ApiHandlerResult, ApiImmediate, ApiSymbol, DispatchOutcome,
    GuestCallbackArgument, GuestCallbackCompletionPolicy, GuestCallbackRequest,
    GuestCallbackScratchPointerRelocation, HostCompletion,
};
use crate::memory::{GuestAddress, GuestMemory, MemoryAccess};
use crate::pe32::{ResourceDataEntryPlan, ResourceIdentifier};
use crate::runtime::{
    RUNTIME_SYSTEM_CURSOR_ARROW_RESOURCE_ID, RUNTIME_WINDOW_CLASS_NAME_A_LIMIT,
    RUNTIME_WINDOW_NAME_A_LIMIT, RUNTIME_WINDOW_STYLE_VISIBLE,
    RUNTIME_X86_PAINT_STRUCT_BYTE_LENGTH, RuntimeCursorResource, RuntimeCursorSource,
    RuntimeDeviceContextTarget, RuntimeIconResource, RuntimeImeStatusWindowRequest, RuntimeRect,
    RuntimeSetWindowPosCallbackAction, RuntimeSetWindowPosCallbackMessage,
    RuntimeSetWindowPosDefaultCallbackAction, RuntimeSetWindowPosDefaultCallbackMessage,
    RuntimeSetWindowPosDefaultOperation, RuntimeSetWindowPosOperation, RuntimeSetWindowPosRequest,
    RuntimeSetWindowPosVariant, RuntimeSystemCursorKind, RuntimeUpdateWindowOperation,
    RuntimeWindow, RuntimeWindowClassCursorReference, RuntimeWindowClassIconReference,
    RuntimeWindowClassModule, RuntimeWindowClassReference, RuntimeWindowClassRegistration,
    RuntimeWindowCreationCallbackAction, RuntimeWindowCreationMessage,
    RuntimeWindowCreationRequest, RuntimeWindowGeometry, RuntimeWindowLifecycle,
    RuntimeWindowNameA, RuntimeWindowProcedure,
};

const RT_ICON: u32 = 3;
const RT_CURSOR: u32 = 1;
const RT_GROUP_CURSOR: u32 = 12;
const RT_GROUP_ICON: u32 = 14;
const MAX_GROUP_IMAGE_ENTRIES: u16 = 64;
const WM_SETICON: u32 = 0x0080;
const WM_SETTEXT: u32 = 0x000c;
const WINDOW_TITLE_A_MAX_BYTES: usize = 256;
const WM_PAINT: u32 = 0x000f;
const WM_TIMER: u32 = 0x0113;
const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_CHAR: u32 = 0x0102;
const WM_SYSKEYDOWN: u32 = 0x0104;
const WM_SYSKEYUP: u32 = 0x0105;
const WM_SYSCHAR: u32 = 0x0106;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_LBUTTONDBLCLK: u32 = 0x0203;
const WM_RBUTTONDOWN: u32 = 0x0204;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_RBUTTONDBLCLK: u32 = 0x0206;
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
const ERROR_SUCCESS: u32 = 0;
const ERROR_INVALID_HANDLE: u32 = 6;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_INVALID_FLAGS: u32 = 1004;
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
const CURSORINFO_BYTE_LENGTH: u32 = 20;
const CURSOR_SHOWING: u32 = 1;
const MONITORINFOEXA_BYTE_LENGTH: u32 = 72;
const MONITORINFOF_PRIMARY: u32 = 1;

fn handle_get_system_menu(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let window = required_argument(endpoint, frame, 0, "window")?;
    let revert = required_argument(endpoint, frame, 1, "revert")?;
    if revert != 0 {
        return Err(ApiFault::new(
            "system menu revert requires restoring its previous state",
        ));
    }
    if window == 0 {
        return Ok(ApiImmediate::new(Some(0)).with_last_error(ERROR_INVALID_WINDOW_HANDLE));
    }
    match runtime
        .get_system_menu(window, 0xf060)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
    {
        Some(handle) => Ok(ApiImmediate::new(Some(handle))),
        None => Ok(ApiImmediate::new(Some(0)).with_last_error(ERROR_INVALID_WINDOW_HANDLE)),
    }
}

fn handle_enable_menu_item(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let menu = required_argument(endpoint, frame, 0, "menu")?;
    let item = required_argument(endpoint, frame, 1, "item")?;
    let flags = required_argument(endpoint, frame, 2, "flags")?;
    if item != 0xf060 || flags != 0 {
        return Err(ApiFault::new(
            "only enabling the system menu close command is implemented",
        ));
    }
    if menu == 0 {
        return Ok(ApiImmediate::new(Some(u32::MAX)).with_last_error(ERROR_INVALID_MENU_HANDLE));
    }
    match runtime
        .enable_system_menu_close_item(menu, item)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
    {
        Some(result) => Ok(ApiImmediate::new(Some(result))),
        None => Ok(ApiImmediate::new(Some(u32::MAX)).with_last_error(ERROR_INVALID_MENU_HANDLE)),
    }
}

fn handle_get_system_metrics(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let index = required_argument(endpoint, frame, 0, "system metric index")?;
    let value = match index {
        0 | 1 => {
            let monitor = runtime
                .primary_monitor()
                .ok_or_else(|| ApiFault::new("the primary monitor is not configured"))?;
            u32::try_from(if index == 0 {
                monitor.rect.right - monitor.rect.left
            } else {
                monitor.rect.bottom - monitor.rect.top
            })
            .map_err(|_| {
                ApiFault::new(
                    "primary display dimensions cannot be represented as a Windows metric",
                )
            })?
        }
        4 => 23,       // SM_CYCAPTION
        7 | 8 => 3,    // SM_CXFIXEDFRAME, SM_CYFIXEDFRAME
        49 | 50 => 16, // SM_CXSMICON, SM_CYSMICON
        _ => {
            return Err(ApiFault::new(format!(
                "GetSystemMetrics index {index} is not implemented"
            )));
        }
    };
    Ok(ApiImmediate::new(Some(value)))
}

fn handle_get_client_rect(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let window_handle = required_argument(endpoint, frame, 0, "window handle")?;
    let output = required_argument(endpoint, frame, 1, "client rectangle")?;
    let window = match runtime.window_option(window_handle) {
        Ok(Some(window)) => window,
        Ok(None) => {
            return Ok(ApiImmediate::new(Some(u32::from(false)))
                .with_last_error(ERROR_INVALID_WINDOW_HANDLE));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, endpoint)),
    };
    let client_right = i32::try_from(window.creation.geometry.client_width)
        .map_err(|_| ApiFault::new("logical client width does not fit an x86 RECT"))?;
    let client_bottom = i32::try_from(window.creation.geometry.client_height)
        .map_err(|_| ApiFault::new("logical client height does not fit an x86 RECT"))?;
    let bytes = [0_i32, 0, client_right, client_bottom]
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect();
    Ok(ApiImmediate::new(Some(u32::from(true))).with_guest_write(GuestAddress(output), bytes))
}

fn handle_get_window_rect(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let window_handle = required_argument(endpoint, frame, 0, "window handle")?;
    let output = required_argument(endpoint, frame, 1, "window rectangle")?;
    let window = match runtime.window_option(window_handle) {
        Ok(Some(window)) => window,
        Ok(None) => {
            return Ok(ApiImmediate::new(Some(u32::from(false)))
                .with_last_error(ERROR_INVALID_WINDOW_HANDLE));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, endpoint)),
    };
    let geometry = window.creation.geometry;
    let width = i32::try_from(geometry.width)
        .map_err(|_| ApiFault::new("logical outer width does not fit an x86 RECT"))?;
    let height = i32::try_from(geometry.height)
        .map_err(|_| ApiFault::new("logical outer height does not fit an x86 RECT"))?;
    let right = geometry
        .x
        .checked_add(width)
        .ok_or_else(|| ApiFault::new("logical outer right coordinate does not fit an x86 RECT"))?;
    let bottom = geometry
        .y
        .checked_add(height)
        .ok_or_else(|| ApiFault::new("logical outer bottom coordinate does not fit an x86 RECT"))?;
    let bytes = [geometry.x, geometry.y, right, bottom]
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect();
    Ok(ApiImmediate::new(Some(u32::from(true))).with_guest_write(GuestAddress(output), bytes))
}

fn handle_get_cursor_pos(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let output = required_argument(endpoint, frame, 0, "cursor position")?;
    if output == 0 {
        return Ok(ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_NOACCESS));
    }
    let cursor = runtime.cursor_position().ok_or_else(|| {
        ApiFault::new("logical cursor position must be configured before GetCursorPos is queried")
    })?;
    let bytes = [cursor.screen_x, cursor.screen_y]
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect();
    Ok(ApiImmediate::new(Some(u32::from(true))).with_guest_write(GuestAddress(output), bytes))
}

fn handle_set_cursor_pos(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let screen_x = required_argument(endpoint, frame, 0, "screen x")? as i32;
    let screen_y = required_argument(endpoint, frame, 1, "screen y")? as i32;
    runtime
        .set_cursor_position(screen_x, screen_y)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(u32::from(true))))
}

fn handle_set_cursor(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let handle = required_argument(endpoint, frame, 0, "cursor handle")?;
    if handle != 0 {
        match runtime.cursor_option(handle) {
            Ok(Some(_)) => {}
            Ok(None) => return Ok(ApiImmediate::new(Some(0))),
            Err(error) => return Err(ApiFault::from_runtime(error, endpoint)),
        }
    }
    let previous = runtime
        .set_current_cursor((handle != 0).then_some(handle))
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(previous)))
}

fn handle_get_cursor_info(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let output = required_argument(endpoint, frame, 0, "cursor information")?;
    if output == 0 {
        return Ok(ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_NOACCESS));
    }
    let size_bytes = runtime
        .memory
        .read(GuestAddress(output), 4)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let structure_byte_length = read_u32(&size_bytes, 0)
        .ok_or_else(|| ApiFault::new("validated CURSORINFO size field could not be decoded"))?;
    if structure_byte_length != CURSORINFO_BYTE_LENGTH {
        return Ok(
            ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_INVALID_PARAMETER)
        );
    }
    let current_cursor = runtime
        .current_cursor()
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let cursor_handle = current_cursor.map_or(0, |cursor| cursor.handle);
    let (screen_x, screen_y) = runtime
        .cursor_position()
        .map(|cursor| (cursor.screen_x, cursor.screen_y))
        .ok_or_else(|| {
            ApiFault::new(
                "logical cursor position must be configured before GetCursorInfo is queried",
            )
        })?;
    // ShowCursor keeps a per-thread display count; the cursor is visible only while it is
    // non-negative, so the reported flags must follow that authority instead of a constant.
    let display_count = runtime
        .current_thread_cursor_display_count()
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let flags = if display_count >= 0 && current_cursor.is_some() {
        CURSOR_SHOWING
    } else {
        0
    };
    let mut bytes = Vec::with_capacity(CURSORINFO_BYTE_LENGTH as usize);
    // cbSize is an input the caller owns; echo the validated length rather than clearing it.
    bytes.extend_from_slice(&CURSORINFO_BYTE_LENGTH.to_le_bytes());
    bytes.extend_from_slice(&flags.to_le_bytes());
    bytes.extend_from_slice(&cursor_handle.to_le_bytes());
    bytes.extend_from_slice(&screen_x.to_le_bytes());
    bytes.extend_from_slice(&screen_y.to_le_bytes());
    Ok(ApiImmediate::new(Some(u32::from(true))).with_guest_write(GuestAddress(output), bytes))
}

fn handle_show_cursor(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let show = required_argument(endpoint, frame, 0, "show flag")? != 0;
    let count = runtime
        .show_cursor(show)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some((count) as u32)))
}

fn handle_get_queue_status(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let flags = required_argument(endpoint, frame, 0, "queue status flags")?;
    if flags & !crate::runtime::RUNTIME_QUEUE_STATUS_VALID_FLAGS != 0 {
        return Ok(ApiImmediate::new(Some(0)).with_last_error(ERROR_INVALID_FLAGS));
    }
    let status = runtime
        .get_current_thread_queue_status(flags)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(status)))
}

fn handle_peek_message_a(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let output = required_argument(endpoint, frame, 0, "message output")?;
    let window = required_argument(endpoint, frame, 1, "window filter")?;
    let minimum = required_argument(endpoint, frame, 2, "minimum message")?;
    let maximum = required_argument(endpoint, frame, 3, "maximum message")?;
    let flag = required_argument(endpoint, frame, 4, "remove flag")?;
    if window != 0 || minimum != 0 || maximum != 0 || !matches!(flag, PM_NOREMOVE | PM_REMOVE) {
        return Err(ApiFault::new(
            "only unfiltered current-thread PM_NOREMOVE/PM_REMOVE peeking is implemented",
        ));
    }
    peek_message_a_unfiltered(endpoint, runtime, output, flag == PM_REMOVE)
}

fn peek_message_a_unfiltered(
    endpoint: &ApiEndpointIdentity,
    runtime: &mut crate::runtime::Runtime,
    output: u32,
    remove: bool,
) -> Result<ApiImmediate, ApiFault> {
    let found = runtime
        .peek_current_thread_message(GuestAddress(output), remove)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(u32::from(found))))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct X86Message {
    window: u32,
    message: u32,
    w_param: u32,
    l_param: u32,
    time: u32,
    point_x: i32,
    point_y: i32,
}

fn read_x86_message(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    memory: &crate::memory::GuestMemory,
) -> Result<X86Message, ApiFault> {
    let address = required_argument(endpoint, frame, 0, "message")?;
    let bytes = memory
        .read(GuestAddress(address), X86_MSG_BYTE_LENGTH as usize)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let word = |offset, label| {
        read_u32(&bytes, offset)
            .ok_or_else(|| ApiFault::new(format!("validated MSG32 omitted its {label} field")))
    };
    Ok(X86Message {
        window: word(0, "window")?,
        message: word(4, "message")?,
        w_param: word(8, "wParam")?,
        l_param: word(12, "lParam")?,
        time: word(16, "time")?,
        point_x: word(20, "point x")? as i32,
        point_y: word(24, "point y")? as i32,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueuedMessageDispatch {
    WindowTimer { callback: GuestAddress, result: u32 },
    WindowProcedure { callback: GuestAddress },
}

fn validate_queued_pointer_message(
    message: X86Message,
    endpoint: &ApiEndpointIdentity,
    runtime: &crate::runtime::Runtime,
) -> Result<QueuedMessageDispatch, ApiFault> {
    let expected_button_state = match message.message {
        WM_MOUSEMOVE => None,
        value if value == WM_LBUTTONDOWN => Some((MK_LBUTTON, true)),
        value if value == WM_LBUTTONDBLCLK => Some((MK_LBUTTON, true)),
        value if value == WM_LBUTTONUP => Some((MK_LBUTTON, false)),
        value if value == WM_RBUTTONDOWN => Some((MK_RBUTTON, true)),
        value if value == WM_RBUTTONDBLCLK => Some((MK_RBUTTON, true)),
        value if value == WM_RBUTTONUP => Some((MK_RBUTTON, false)),
        _ => {
            return Err(ApiFault::new(
                "message is outside the reached window-timer and pointer profiles",
            ));
        }
    };
    let allowed_key_state = MK_LBUTTON | MK_RBUTTON | MK_SHIFT | MK_CONTROL;
    if message.w_param & !allowed_key_state != 0
        || expected_button_state.is_some_and(|(mask, down)| (message.w_param & mask != 0) != down)
    {
        return Err(ApiFault::new(
            "queued pointer message has an invalid button or modifier key-state mask",
        ));
    }
    let callback = runtime
        .resolve_window_procedure(message.window)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
        .address;
    Ok(QueuedMessageDispatch::WindowProcedure { callback })
}

fn validate_queued_message_dispatch(
    message: X86Message,
    endpoint: &ApiEndpointIdentity,
    runtime: &crate::runtime::Runtime,
) -> Result<QueuedMessageDispatch, ApiFault> {
    if matches!(
        message.message,
        WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP | WM_CHAR | WM_SYSCHAR
    ) {
        if message.w_param > 0xff
            || message.l_param & 0xffff == 0
            || message.l_param & 0x1e00_0000 != 0
        {
            return Err(ApiFault::new(
                "queued keyboard message has invalid key or lParam fields",
            ));
        }
        if matches!(
            message.message,
            WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP
        ) {
            let up = matches!(message.message, WM_KEYUP | WM_SYSKEYUP);
            if (message.l_param & 0x8000_0000 != 0) != up
                || (up && message.l_param & 0x4000_0000 == 0)
                || (matches!(message.message, WM_KEYDOWN | WM_KEYUP)
                    && message.l_param & 0x2000_0000 != 0)
            {
                return Err(ApiFault::new(
                    "queued keyboard transition flags do not match its message",
                ));
            }
        }
        let callback = runtime
            .resolve_window_procedure(message.window)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?
            .address;
        return Ok(QueuedMessageDispatch::WindowProcedure { callback });
    }
    if message.message != WM_TIMER {
        if matches!(
            message.message,
            WM_MOUSEMOVE
                | WM_LBUTTONDOWN
                | WM_LBUTTONDBLCLK
                | WM_LBUTTONUP
                | WM_RBUTTONDOWN
                | WM_RBUTTONDBLCLK
                | WM_RBUTTONUP
        ) {
            return validate_queued_pointer_message(message, endpoint, runtime);
        }
        let callback = runtime
            .resolve_window_procedure(message.window)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?
            .address;
        return Ok(QueuedMessageDispatch::WindowProcedure { callback });
    }
    let callback = runtime
        .resolve_window_timer_callback(
            message.window,
            message.w_param,
            GuestAddress(message.l_param),
        )
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(QueuedMessageDispatch::WindowTimer {
        callback,
        result: 0,
    })
}

fn handle_translate_message(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let message = read_x86_message(endpoint, frame, &runtime.memory)?;
    if matches!(
        message.message,
        WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP
    ) {
        validate_queued_message_dispatch(message, endpoint, runtime)?;
        runtime
            .translate_keyboard_message(crate::runtime::RuntimeThreadMessage {
                target_thread_id: runtime.current_thread_id(),
                hwnd: message.window,
                message: message.message,
                w_param: message.w_param,
                l_param: message.l_param,
                time: message.time,
                point_x: message.point_x,
                point_y: message.point_y,
            })
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        return Ok(ApiImmediate::new(Some(1)));
    }
    Ok(ApiImmediate::new(Some(u32::from(false))))
}

fn handle_dispatch_message_a(
    callback_return_sentinel: GuestAddress,
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiHandlerResult, ApiFault> {
    let message = read_x86_message(endpoint, frame, &runtime.memory)?;
    let dispatch = validate_queued_message_dispatch(message, endpoint, runtime)?;
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
    Ok(ApiHandlerResult::GuestCallback(Box::new(request)))
}

fn handle_is_iconic(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let window_handle = required_argument(endpoint, frame, 0, "window handle")?;
    match runtime.window_is_iconic(window_handle) {
        Ok(Some(iconic)) => Ok(ApiImmediate::new(Some(u32::from(iconic)))),
        Ok(None) => {
            Ok(ApiImmediate::new(Some(u32::from(false)))
                .with_last_error(ERROR_INVALID_WINDOW_HANDLE))
        }
        Err(error) => Err(ApiFault::from_runtime(error, endpoint)),
    }
}

fn handle_get_foreground_window(
    endpoint: &ApiEndpointIdentity,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let handle = runtime
        .foreground_window_handle()
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
        .unwrap_or(0);
    Ok(ApiImmediate::new(Some(handle)))
}

fn handle_set_focus(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let window_handle = required_argument(endpoint, frame, 0, "window handle")?;
    let previous_window = runtime
        .set_focus_same_window(window_handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(previous_window)))
}

fn handle_get_async_key_state(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let virtual_key = required_argument(endpoint, frame, 0, "virtual key")?;
    if virtual_key > 0xff {
        return Ok(ApiImmediate::new(Some(0)));
    }
    let keyboard = runtime
        .keyboard_state()
        .ok_or_else(|| ApiFault::new("the active process has no keyboard state"))?;
    if !keyboard.source_focused {
        return Ok(ApiImmediate::new(Some(0)));
    }
    let state = runtime
        .query_async_key_state(virtual_key)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
        .ok_or_else(|| ApiFault::new("the validated virtual key is absent from keyboard state"))?;
    Ok(ApiImmediate::new(Some((state) as i32 as u32)))
}

fn handle_set_rect(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
) -> Result<ApiImmediate, ApiFault> {
    let output = required_argument(endpoint, frame, 0, "rectangle")?;
    let left = required_argument(endpoint, frame, 1, "left")? as i32;
    let top = required_argument(endpoint, frame, 2, "top")? as i32;
    let right = required_argument(endpoint, frame, 3, "right")? as i32;
    let bottom = required_argument(endpoint, frame, 4, "bottom")? as i32;
    if output == 0 {
        return Ok(ApiImmediate::new(Some(u32::from(false))));
    }
    let bytes = [left, top, right, bottom]
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect();
    Ok(ApiImmediate::new(Some(u32::from(true))).with_guest_write(GuestAddress(output), bytes))
}

fn handle_monitor_from_window(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let window_handle = required_argument(endpoint, frame, 0, "window handle")?;
    let flags = required_argument(endpoint, frame, 1, "monitor fallback flags")?;
    if flags != 1 {
        return Err(ApiFault::new(
            "MonitorFromWindow currently supports MONITOR_DEFAULTTOPRIMARY",
        ));
    }
    match runtime.window_option(window_handle) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Ok(ApiImmediate::new(Some(0)).with_last_error(ERROR_INVALID_WINDOW_HANDLE));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, endpoint)),
    }
    let monitor = runtime.primary_monitor().ok_or_else(|| {
        ApiFault::new("MonitorFromWindow requires the configured primary display")
    })?;
    Ok(ApiImmediate::new(Some(monitor.handle)))
}

fn handle_get_monitor_info_a(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let monitor_handle = required_argument(endpoint, frame, 0, "monitor handle")?;
    let output = required_argument(endpoint, frame, 1, "monitor information")?;
    let Some(monitor) = runtime.monitor(monitor_handle) else {
        return Ok(
            ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_INVALID_MONITOR_HANDLE)
        );
    };
    let size = runtime
        .memory
        .read(GuestAddress(output), 4)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let length = u32::from_le_bytes(size.try_into().expect("four-byte MONITORINFO length"));
    if length == MONITORINFOEXA_BYTE_LENGTH {
        return Err(ApiFault::new(
            "GetMonitorInfoA MONITORINFOEXA is not implemented",
        ));
    }
    if length != MONITORINFO_BYTE_LENGTH {
        return Ok(
            ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_INVALID_PARAMETER)
        );
    }
    let mut bytes = Vec::with_capacity(MONITORINFO_BYTE_LENGTH as usize);
    bytes.extend_from_slice(&MONITORINFO_BYTE_LENGTH.to_le_bytes());
    bytes.extend_from_slice(&monitor.rect.to_x86_bytes());
    bytes.extend_from_slice(&monitor.work_rect.to_x86_bytes());
    bytes.extend_from_slice(&MONITORINFOF_PRIMARY.to_le_bytes());
    Ok(ApiImmediate::new(Some(u32::from(true))).with_guest_write(GuestAddress(output), bytes))
}

fn handle_get_dc(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let window_handle = required_argument(endpoint, frame, 0, "window handle")?;
    if window_handle != 0 {
        match runtime.window_option(window_handle) {
            Ok(Some(_)) => {}
            Ok(None) => {
                return Ok(ApiImmediate::new(Some(0)).with_last_error(ERROR_INVALID_WINDOW_HANDLE));
            }
            Err(error) => return Err(ApiFault::from_runtime(error, endpoint)),
        }
    }
    let handle = if window_handle == 0 {
        runtime.acquire_screen_dc()
    } else {
        runtime
            .acquire_window_dc(window_handle)
            .map(|result| result.handle)
    }
    .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(handle)))
}

fn handle_fill_rect(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let device_context = required_argument(endpoint, frame, 0, "device context")?;
    let rect_address = required_argument(endpoint, frame, 1, "rectangle")?;
    let brush = required_argument(endpoint, frame, 2, "brush")?;
    let bytes = runtime
        .memory
        .read(GuestAddress(rect_address), 16)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let rect = RuntimeRect {
        left: read_i32(&bytes, 0).ok_or_else(|| malformed_rect_fault(endpoint))?,
        top: read_i32(&bytes, 4).ok_or_else(|| malformed_rect_fault(endpoint))?,
        right: read_i32(&bytes, 8).ok_or_else(|| malformed_rect_fault(endpoint))?,
        bottom: read_i32(&bytes, 12).ok_or_else(|| malformed_rect_fault(endpoint))?,
    };
    match runtime.fill_gdi_rect(device_context, rect, brush) {
        Ok(true) => Ok(ApiImmediate::new(Some(u32::from(true)))),
        Ok(false) => {
            Ok(ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_INVALID_HANDLE))
        }
        Err(error) => Err(ApiFault::from_runtime(error, endpoint)),
    }
}

fn handle_release_dc(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let window_handle = required_argument(endpoint, frame, 0, "window handle")?;
    let device_context = required_argument(endpoint, frame, 1, "device context")?;
    let window = if window_handle == 0 {
        None
    } else {
        match runtime.window_option(window_handle) {
            Ok(Some(window)) => Some(window),
            Ok(None) => {
                return Ok(ApiImmediate::new(Some(u32::from(false)))
                    .with_last_error(ERROR_INVALID_WINDOW_HANDLE));
            }
            Err(error) => return Err(ApiFault::from_runtime(error, endpoint)),
        }
    };
    let current_thread = runtime.current_thread_id();
    let hdc = runtime
        .gdi_device_context_option(device_context)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let belongs_to_target = if let Some(window) = window {
        hdc.is_some_and(|hdc| {
            hdc.owner_thread_id == current_thread
                && hdc.owner_thread_id == window.creation.owner_thread_id
                && matches!(hdc.target, RuntimeDeviceContextTarget::Window { window_id, window_handle: acquired, .. }
                    if window_id == window.id && acquired == window_handle)
        })
    } else {
        let monitor = runtime.primary_monitor().ok_or_else(|| {
            ApiFault::new("screen DC release requires a configured primary monitor")
        })?;
        hdc.is_some_and(|hdc| {
            hdc.owner_thread_id == current_thread
                && matches!(hdc.target, RuntimeDeviceContextTarget::Screen { monitor_handle, rect }
                    if monitor_handle == monitor.handle && rect == monitor.rect)
        })
    };
    if !belongs_to_target {
        let mut result = ApiImmediate::new(Some(u32::from(false)));
        if window_handle != 0 {
            result.last_error = Some(ERROR_DC_NOT_FOUND);
        }
        return Ok(result);
    }
    if window_handle == 0 {
        runtime.release_screen_dc(device_context)
    } else {
        runtime.release_window_dc(window_handle, device_context)
    }
    .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(u32::from(true))))
}

fn malformed_rect_fault(endpoint: &ApiEndpointIdentity) -> ApiFault {
    ApiFault::new(format!(
        "decoded {:?} frame did not retain one complete x86 RECT",
        endpoint.symbol
    ))
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

fn handle_load_icon_a(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let instance = required_argument(endpoint, frame, 0, "module instance")?;
    let icon = required_argument(endpoint, frame, 1, "icon resource name")?;
    if instance == 0 || icon > u32::from(u16::MAX) {
        return Err(ApiFault::new(
            "only integer module icon resources are supported",
        ));
    }
    load_module_shared_icon((32, 32), instance, icon, endpoint, runtime)
}

fn handle_load_image_a(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let instance = required_argument(endpoint, frame, 0, "module instance")?;
    let name = required_argument(endpoint, frame, 1, "resource name")?;
    let image_type = required_argument(endpoint, frame, 2, "image type")?;
    let width = required_argument(endpoint, frame, 3, "width")?;
    let height = required_argument(endpoint, frame, 4, "height")?;
    let flags = required_argument(endpoint, frame, 5, "flags")?;
    if instance == 0
        || name > u32::from(u16::MAX)
        || image_type != 1
        || !(1..=256).contains(&width)
        || !(1..=256).contains(&height)
        || flags != 0x8000
    {
        return Err(ApiFault::new(
            "LoadImageA supports bounded shared module icon resources by integer ID",
        ));
    }
    load_module_shared_icon(
        (width as u16, height as u16),
        instance,
        name,
        endpoint,
        runtime,
    )
}

fn handle_set_window_text_a(
    callback_return_sentinel: GuestAddress,
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiHandlerResult, ApiFault> {
    let window_handle = required_argument(endpoint, frame, 0, "window handle")?;
    let title_address = GuestAddress(required_argument(endpoint, frame, 1, "window title")?);
    let entry_last_error = runtime
        .current_last_error()
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    match runtime.window_option(window_handle) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(Some(u32::from(false)))
                    .with_last_error(ERROR_INVALID_WINDOW_HANDLE),
            ));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, endpoint)),
    }
    let window_procedure = runtime
        .resolve_window_procedure(window_handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    // lpString is optional and an empty title is an ordinary request; both are simply the text
    // the window procedure receives through WM_SETTEXT. A present pointer is still validated
    // against the reached title bound before it reaches the guest procedure.
    if title_address.0 != 0 {
        read_bounded_window_title_a(
            &runtime.memory,
            title_address,
            WINDOW_TITLE_A_MAX_BYTES,
            endpoint,
        )?;
    }
    let request = GuestCallbackRequest {
        target: window_procedure.address,
        arguments: vec![
            GuestCallbackArgument::U32(window_handle),
            GuestCallbackArgument::U32(WM_SETTEXT),
            GuestCallbackArgument::U32(0),
            GuestCallbackArgument::U32(title_address.0),
        ],
        scratch: Vec::new(),
        scratch_pointer_relocations: Vec::new(),
        return_sentinel: callback_return_sentinel,
        completion_policy: GuestCallbackCompletionPolicy::ReturnBool32WithLastError {
            last_error: entry_last_error,
        },
    };
    Ok(ApiHandlerResult::GuestCallback(Box::new(request)))
}

fn handle_set_window_long_a(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let window_handle = required_argument(endpoint, frame, 0, "window handle")?;
    let index = required_argument(endpoint, frame, 1, "window-long index")? as i32;
    let style = required_argument(endpoint, frame, 2, "new window style")?;
    match runtime.window_option(window_handle) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Ok(ApiImmediate::new(Some(0)).with_last_error(ERROR_INVALID_WINDOW_HANDLE));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, endpoint)),
    }
    if index == INVALID_WINDOW_LONG_INDEX {
        return Ok(ApiImmediate::new(Some(0)).with_last_error(ERROR_INVALID_INDEX));
    }
    if index != GWL_STYLE {
        return Err(ApiFault::new(
            "SetWindowLongA currently implements only GWL_STYLE",
        ));
    }
    let previous_style = runtime
        .set_window_style(window_handle, style)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(previous_style)))
}

fn handle_set_timer(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let window_handle = required_argument(endpoint, frame, 0, "window handle")?;
    let timer_id = required_argument(endpoint, frame, 1, "timer ID")?;
    let interval = required_argument(endpoint, frame, 2, "interval")?;
    let callback = required_argument(endpoint, frame, 3, "timer callback")?;
    match runtime.window_option(window_handle) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Ok(ApiImmediate::new(Some(0)).with_last_error(ERROR_INVALID_WINDOW_HANDLE));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, endpoint)),
    }
    if timer_id == 0 || callback == 0 {
        return Err(ApiFault::new(
            "SetTimer automatic IDs and message-only timers are not implemented",
        ));
    }
    runtime
        .memory
        .validate_access(GuestAddress(callback), 1, MemoryAccess::Execute)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let timer_id = runtime
        .register_window_timer(window_handle, timer_id, interval, GuestAddress(callback))
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(timer_id)))
}

pub(crate) fn resolve_update_window_callback(
    call: &ApiGuestCallbackCall,
    runtime: &crate::runtime::Runtime,
) -> ApiHandlerResult {
    let GuestCallbackCompletionPolicy::UpdateWindow { operation } = call.request.completion_policy
    else {
        return ApiHandlerResult::Fault(ApiFault::new(
            "UpdateWindow callback continuation has the wrong completion policy",
        ));
    };
    match runtime.validate_update_window_callback(operation) {
        Ok(()) => ApiHandlerResult::Immediate(
            ApiImmediate::new(Some(u32::from(true))).with_last_error(operation.entry_last_error),
        ),
        Err(error) => ApiHandlerResult::Fault(ApiFault::from_runtime(error, &call.frame.endpoint)),
    }
}

fn update_window_callback_request(
    operation: RuntimeUpdateWindowOperation,
    procedure: RuntimeWindowProcedure,
    paint_message: u32,
    return_sentinel: GuestAddress,
) -> GuestCallbackRequest {
    GuestCallbackRequest {
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
    }
}

fn begin_set_window_pos(
    callback_return_sentinel: GuestAddress,
    endpoint: &ApiEndpointIdentity,
    runtime: &mut crate::runtime::Runtime,
    request: RuntimeSetWindowPosRequest,
) -> Result<ApiHandlerResult, ApiFault> {
    let operation = runtime
        .preview_set_window_pos(request)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let window = runtime
        .window(operation.window_handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let class = runtime
        .window_class(window.creation.class.atom)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
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
    Ok(ApiHandlerResult::GuestCallback(Box::new(request)))
}

pub(crate) fn resolve_set_window_pos_callback(
    call: &ApiGuestCallbackCall,
    result: u32,
    runtime: &crate::runtime::Runtime,
) -> ApiHandlerResult {
    let GuestCallbackCompletionPolicy::SetWindowPos { operation, message } =
        call.request.completion_policy
    else {
        return ApiHandlerResult::Fault(ApiFault::new(
            "SetWindowPos callback continuation has the wrong completion policy",
        ));
    };
    let action = match runtime.preview_set_window_pos_callback(operation, message, result) {
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
            let class = match runtime.window_class(window.creation.class.atom) {
                Ok(class) if class.id == window.creation.class.class_id => class,
                Ok(_) => {
                    return ApiHandlerResult::Fault(ApiFault::new(
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
                Ok(request) => ApiHandlerResult::GuestCallback(Box::new(request)),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        RuntimeSetWindowPosCallbackAction::Completed { .. } => ApiHandlerResult::Immediate(
            ApiImmediate::new(Some(u32::from(true))).with_last_error(operation.entry_last_error),
        ),
    }
}

pub(crate) fn resolve_set_window_pos_default_callback(
    call: &ApiGuestCallbackCall,
    result: u32,
    runtime: &crate::runtime::Runtime,
) -> ApiHandlerResult {
    let GuestCallbackCompletionPolicy::SetWindowPosDefault { operation, message } =
        call.request.completion_policy
    else {
        return ApiHandlerResult::Fault(ApiFault::new(
            "activation default callback has the wrong completion policy",
        ));
    };
    let action = match runtime.preview_set_window_pos_default_callback(operation, message, result) {
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
            runtime,
            &call.frame.endpoint,
        ) {
            Ok(request) => ApiHandlerResult::GuestCallback(Box::new(request)),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        RuntimeSetWindowPosDefaultCallbackAction::Completed => {
            ApiHandlerResult::Immediate(ApiImmediate::new(Some(0)))
        }
    }
}

fn set_window_pos_default_callback_request(
    operation: RuntimeSetWindowPosDefaultOperation,
    message: RuntimeSetWindowPosDefaultCallbackMessage,
    return_sentinel: GuestAddress,
    ime_set_context_flags: u32,
    runtime: &crate::runtime::Runtime,
    endpoint: &ApiEndpointIdentity,
) -> Result<GuestCallbackRequest, ApiFault> {
    let procedure = runtime
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
        let right = x
            .checked_add_unsigned(width)
            .ok_or_else(|| ApiFault::new("validated resize rectangle right edge overflowed"))?;
        let bottom = y
            .checked_add_unsigned(height)
            .ok_or_else(|| ApiFault::new("validated resize rectangle bottom edge overflowed"))?;
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
            return Err(ApiFault::new(
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
    let width = i32::try_from(width)
        .map_err(|_| ApiFault::new("validated logical window width does not fit WINDOWPOS"))?;
    let height = i32::try_from(height)
        .map_err(|_| ApiFault::new("validated logical window height does not fit WINDOWPOS"))?;
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
    target_size: (u16, u16),
    instance: u32,
    icon_name: u32,
    endpoint: &ApiEndpointIdentity,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let image = match runtime.module_image_by_load_base(GuestAddress(instance)) {
        Ok(Some(image)) => image,
        Ok(None) => return Ok(load_icon_failure(ERROR_INVALID_HANDLE)),
        Err(error) => {
            return Err(ApiFault::new(error.to_string()));
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
    let group_limit = 6_u32 + 14_u32 * u32::from(MAX_GROUP_IMAGE_ENTRIES);
    if group_resource.size > group_limit {
        return Ok(load_icon_failure(ERROR_RESOURCE_DATA_NOT_FOUND));
    }
    let group_address = resource_address(instance, group_resource.data_rva.0)?;
    let group_bytes = runtime
        .memory
        .read(group_address, group_resource.size as usize)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let selected = match select_group_icon(&group_bytes, target_size.0, target_size.1) {
        Ok(selected) => selected,
        Err(GroupIconSelectionError::Malformed) => {
            return Ok(load_icon_failure(ERROR_RESOURCE_DATA_NOT_FOUND));
        }
        Err(GroupIconSelectionError::ExactSizeUnavailable) => {
            return Err(ApiFault::new(
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
    let icon_address = resource_address(instance, icon_resource.data_rva.0)?;
    let icon_header = runtime
        .memory
        .read(icon_address, 40)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    if icon_header.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Err(ApiFault::new(
            "selected module icon uses PNG payload; the reached slice validates DIB icons",
        ));
    }
    if !validate_selected_icon_dib(&icon_header, &selected, icon_resource.size) {
        return Ok(load_icon_failure(ERROR_RESOURCE_DATA_NOT_FOUND));
    }

    let icon_resource = RuntimeIconResource {
        module_id: image.module_id,
        module_load_base: image.image_range.start(),
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
    let handle = runtime
        .load_shared_icon(icon_resource)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(handle)).with_last_error(ERROR_SUCCESS))
}

fn load_module_shared_cursor(
    instance: u32,
    cursor_name: u32,
    endpoint: &ApiEndpointIdentity,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let image = match runtime.module_image_by_load_base(GuestAddress(instance)) {
        Ok(Some(image)) => image,
        Ok(None) => return Ok(load_cursor_failure(ERROR_INVALID_HANDLE)),
        Err(error) => {
            return Err(ApiFault::new(error.to_string()));
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
    let group_limit = 6_u32 + 14_u32 * u32::from(MAX_GROUP_IMAGE_ENTRIES);
    if group_resource.size > group_limit {
        return Ok(load_cursor_failure(ERROR_RESOURCE_DATA_NOT_FOUND));
    }
    let group_address = resource_address(instance, group_resource.data_rva.0)?;
    let group_bytes = runtime
        .memory
        .read(group_address, group_resource.size as usize)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let selected = match select_group_cursor(&group_bytes, 32, 32) {
        Ok(selected) => selected,
        Err(GroupCursorSelectionError::Malformed) => {
            return Ok(load_cursor_failure(ERROR_RESOURCE_DATA_NOT_FOUND));
        }
        Err(GroupCursorSelectionError::ExactSizeUnavailable) => {
            return Err(ApiFault::new(
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
    let cursor_address = resource_address(instance, cursor_resource.data_rva.0)?;
    let cursor_header = runtime
        .memory
        .read(cursor_address, 44)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    if cursor_header[4..].starts_with(&[0x89, b'P', b'N', b'G']) {
        return Err(ApiFault::new(
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
            module_load_base: image.image_range.start(),
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
    let handle = runtime
        .load_shared_cursor(source)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(handle)).with_last_error(ERROR_SUCCESS))
}

fn handle_load_cursor_a(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let instance = required_argument(endpoint, frame, 0, "module instance")?;
    let name = required_argument(endpoint, frame, 1, "cursor resource name")?;
    if name > u32::from(u16::MAX) {
        return Err(ApiFault::new(
            "string cursor resource names are not yet supported",
        ));
    }
    if instance != 0 {
        return load_module_shared_cursor(instance, name, endpoint, runtime);
    }
    if name == u32::from(u16::MAX) {
        return Ok(load_cursor_failure(ERROR_RESOURCE_NAME_NOT_FOUND));
    }
    if name != RUNTIME_SYSTEM_CURSOR_ARROW_RESOURCE_ID {
        return Err(ApiFault::new(
            "only the predefined arrow cursor is supported",
        ));
    }
    let cursor = runtime
        .load_shared_cursor(RuntimeCursorSource::SystemPredefined {
            resource_id: name,
            cursor: RuntimeSystemCursorKind::Arrow,
        })
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(cursor)))
}

fn handle_register_class_ex_a(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let address = GuestAddress(required_argument(
        endpoint,
        frame,
        0,
        "WNDCLASSEXA pointer",
    )?);
    let bytes = runtime
        .memory
        .read(address, 48)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let field = |offset| read_u32(&bytes, offset).expect("48-byte WNDCLASSEXA");
    if field(0) != 48 {
        return Ok(register_class_failure(ERROR_INVALID_PARAMETER));
    }
    if field(12) != 0 || field(16) != 0 {
        return Err(ApiFault::new(
            "class and window extra-byte storage is not yet supported",
        ));
    }
    if field(32) != 0 || field(36) != 0 || field(44) != 0 {
        return Err(ApiFault::new(
            "background brush, menu resource, and small icon registration are not yet supported",
        ));
    }
    if field(40) == 0 {
        return Ok(register_class_failure(ERROR_INVALID_PARAMETER));
    }
    if field(40) <= u32::from(u16::MAX) {
        return Err(ApiFault::new("integer class names are not yet supported"));
    }
    let class_name = read_bounded_ascii_nul(
        &runtime.memory,
        GuestAddress(field(40)),
        RUNTIME_WINDOW_CLASS_NAME_A_LIMIT,
        endpoint,
    )?;
    if class_name.is_empty() {
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
    if instance.0 == 0 || window_procedure_address.0 == 0 {
        return Ok(register_class_failure(ERROR_INVALID_PARAMETER));
    }
    let module = match runtime.module_image_by_load_base(instance) {
        Ok(Some(image)) => {
            let procedure = u64::from(window_procedure_address.0);
            if procedure < u64::from(image.image_range.start().0)
                || procedure >= image.image_range.end()
            {
                return Ok(register_class_failure(ERROR_INVALID_PARAMETER));
            }
            let Some(procedure_rva) = window_procedure_address
                .0
                .checked_sub(image.image_range.start().0)
            else {
                return Ok(register_class_failure(ERROR_INVALID_PARAMETER));
            };
            (
                RuntimeWindowClassModule {
                    module_id: image.module_id,
                    load_base: image.image_range.start(),
                },
                procedure_rva,
            )
        }
        Ok(None) => return Ok(register_class_failure(ERROR_INVALID_PARAMETER)),
        Err(error) => {
            return Err(ApiFault::new(error.to_string()));
        }
    };
    if runtime
        .memory
        .validate_access(window_procedure_address, 1, MemoryAccess::Execute)
        .is_err()
    {
        return Ok(register_class_failure(ERROR_INVALID_PARAMETER));
    }
    let icon = match runtime.icon_option(icon_handle) {
        Ok(Some(icon)) => RuntimeWindowClassIconReference {
            icon_id: icon.id,
            handle: icon.handle,
        },
        Ok(None) => return Ok(register_class_failure(ERROR_INVALID_PARAMETER)),
        Err(error) => return Err(ApiFault::from_runtime(error, endpoint)),
    };
    let cursor = match runtime.cursor_option(cursor_handle) {
        Ok(Some(cursor)) => RuntimeWindowClassCursorReference {
            cursor_id: cursor.id,
            handle: cursor.handle,
        },
        Ok(None) => return Ok(register_class_failure(ERROR_INVALID_PARAMETER)),
        Err(error) => return Err(ApiFault::from_runtime(error, endpoint)),
    };
    let registration = RuntimeWindowClassRegistration {
        module: module.0.clone(),
        style,
        window_procedure: RuntimeWindowProcedure {
            module_id: module.0.module_id,
            address: window_procedure_address,
            rva: crate::pe32::Rva(module.1),
        },
        class_extra_bytes,
        window_extra_bytes,
        icon: Some(icon),
        cursor: Some(cursor),
        background_brush,
        menu_name: None,
        class_name,
        small_icon: None,
    };
    let atom = runtime
        .register_window_class(registration)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    if atom == 0 {
        return Ok(register_class_failure(ERROR_CLASS_ALREADY_EXISTS));
    }
    Ok(ApiImmediate::new(Some(u32::from(atom))))
}

fn handle_create_window_ex_a(
    callback_return_sentinel: GuestAddress,
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiHandlerResult, ApiFault> {
    let extended_style = required_argument(endpoint, frame, 0, "extended style")?;
    let class_address = required_argument(endpoint, frame, 1, "class name")?;
    let title_address = required_argument(endpoint, frame, 2, "window title")?;
    let style = required_argument(endpoint, frame, 3, "style")?;
    let x = required_argument(endpoint, frame, 4, "x coordinate")? as i32;
    let y = required_argument(endpoint, frame, 5, "y coordinate")? as i32;
    let width = required_argument(endpoint, frame, 6, "outer width")?;
    let height = required_argument(endpoint, frame, 7, "outer height")?;
    let parent = required_argument(endpoint, frame, 8, "parent window")?;
    let menu = required_argument(endpoint, frame, 9, "menu")?;
    let instance = GuestAddress(required_argument(endpoint, frame, 10, "instance")?);
    let create_parameter = required_argument(endpoint, frame, 11, "creation parameter")?;
    // This is a fixed-frame top-level window, not a title- or callsite-specific size.
    // Other non-client layouts must not silently acquire incorrect client coordinates.
    if extended_style & !0x0004_0000 != 0
        || style & (0x4000_0000 | 0x1000_0000 | 0x0800_0000 | 0x0004_0000) != 0
        || style & 0x00c0_0000 != 0x00c0_0000
        || parent != 0
        || menu != 0
        || create_parameter != 0
    {
        return Err(ApiFault::new(
            "CreateWindowExA requires a hidden fixed-frame top-level window without parent, menu, or creation parameter",
        ));
    }
    if x < 0
        || y < 0
        || x > 32_767
        || y > 32_767
        || !(7..=32_768).contains(&width)
        || !(30..=32_768).contains(&height)
    {
        return Err(ApiFault::new(
            "CreateWindowExA requires bounded positive window geometry",
        ));
    }
    if class_address == 0 || title_address == 0 || instance.0 == 0 {
        return Ok(ApiHandlerResult::Immediate(create_window_failure(
            ERROR_INVALID_PARAMETER,
        )));
    }
    if class_address <= u32::from(u16::MAX) {
        return Err(ApiFault::new(
            "integer class atoms need their own class lookup",
        ));
    }
    let class_name = read_bounded_windows_932_text_a(
        &runtime.memory,
        GuestAddress(class_address),
        RUNTIME_WINDOW_CLASS_NAME_A_LIMIT,
        "class",
        endpoint,
    )?;
    let window_name = read_bounded_windows_932_text_a(
        &runtime.memory,
        GuestAddress(title_address),
        RUNTIME_WINDOW_NAME_A_LIMIT,
        "title",
        endpoint,
    )?;
    if class_name.is_empty() || window_name.is_empty() {
        return Ok(ApiHandlerResult::Immediate(create_window_failure(
            ERROR_INVALID_PARAMETER,
        )));
    }
    let module = match runtime.module_image_by_load_base(instance) {
        Ok(Some(image)) => RuntimeWindowClassModule {
            module_id: image.module_id,
            load_base: image.image_range.start(),
        },
        Ok(None) => {
            return Ok(ApiHandlerResult::Immediate(create_window_failure(
                ERROR_INVALID_PARAMETER,
            )));
        }
        Err(error) => {
            return Err(ApiFault::new(error.to_string()));
        }
    };
    let class = match runtime.window_class_by_name_option(module.module_id, &class_name) {
        Ok(Some(class)) => class,
        Ok(None) => {
            return Ok(ApiHandlerResult::Immediate(create_window_failure(
                ERROR_INVALID_PARAMETER,
            )));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, endpoint)),
    };
    let class_reference = RuntimeWindowClassReference {
        class_id: class.id,
        atom: class.atom,
        module_id: class.registration.module.module_id,
    };
    let request = RuntimeWindowCreationRequest {
        owner_thread_id: runtime.current_thread_id(),
        class: class_reference,
        instance: module,
        name: RuntimeWindowNameA { value: window_name },
        extended_style,
        effective_extended_style: extended_style
            | if extended_style & 0x0004_0000 != 0 {
                0x100
            } else {
                0
            },
        style,
        effective_style: style
            | if style & 0x8000_0000 != 0 {
                0x0400_0000
            } else {
                0
            },
        geometry: RuntimeWindowGeometry {
            x,
            y,
            width,
            height,
            client_x: x + 3,
            client_y: y + 26,
            client_width: width - 6,
            client_height: height - 29,
        },
        parent,
        menu,
        create_parameter,
        entry_last_error: runtime
            .current_last_error()
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?,
    };
    begin_window_creation(callback_return_sentinel, endpoint, frame, runtime, request)
}

fn begin_window_creation(
    callback_return_sentinel: GuestAddress,
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
    request: RuntimeWindowCreationRequest,
) -> Result<ApiHandlerResult, ApiFault> {
    let preview = runtime
        .checked_window_creation(request)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let class = runtime
        .window_class(preview.creation.class.atom)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let callback = create_window_callback_request(
        &preview,
        class,
        RuntimeWindowCreationMessage::NonClientCreate,
        callback_return_sentinel,
        endpoint,
    )?;
    runtime.preflight_guest_callback(frame, &callback)?;
    runtime.publish_window_creation(preview);
    Ok(ApiHandlerResult::GuestCallback(Box::new(callback)))
}

pub(crate) fn resolve_create_window_ex_a_callback(
    endpoint: &ApiEndpointIdentity,
    call: &ApiGuestCallbackCall,
    result: u32,
    runtime: &crate::runtime::Runtime,
) -> ApiHandlerResult {
    let GuestCallbackCompletionPolicy::CreateWindowExA {
        window_id,
        window_handle,
        message,
    } = call.request.completion_policy
    else {
        return ApiHandlerResult::Fault(ApiFault::new(
            "CreateWindowExA callback continuation has the wrong completion policy",
        ));
    };
    let action = match runtime.preview_window_creation_callback(window_id, message, result) {
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
        return ApiHandlerResult::Fault(ApiFault::new(
            "CreateWindowExA callback continuation does not match the pending logical HWND",
        ));
    }
    match action {
        RuntimeWindowCreationCallbackAction::Continue {
            window,
            next_message,
        } => {
            let class = match runtime.window_class(window.creation.class.atom) {
                Ok(class) if class.id == window.creation.class.class_id => class,
                Ok(_) => {
                    return ApiHandlerResult::Fault(ApiFault::new(
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
                endpoint,
            ) {
                Ok(request) => ApiHandlerResult::GuestCallback(Box::new(request)),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        RuntimeWindowCreationCallbackAction::Created { window } => ApiHandlerResult::Immediate(
            ApiImmediate::new(Some(window.handle))
                .with_last_error(window.creation.entry_last_error),
        ),
        RuntimeWindowCreationCallbackAction::Aborted { .. } => {
            ApiHandlerResult::Immediate(create_window_failure(ERROR_INVALID_PARAMETER))
        }
    }
}

fn require_window_activation_relations(
    runtime: &crate::runtime::Runtime,
    handle: u32,
    active: bool,
    focused: bool,
    foreground: bool,
    endpoint: &ApiEndpointIdentity,
) -> Result<(), ApiFault> {
    let active_handle = runtime
        .active_window_handle()
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let focused_handle = runtime
        .focused_window_handle()
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let foreground_handle = runtime
        .foreground_window_handle()
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    if active_handle == active.then_some(handle)
        && focused_handle == focused.then_some(handle)
        && foreground_handle == foreground.then_some(handle)
    {
        return Ok(());
    }
    Err(ApiFault::new(
        "logical active, focused, or foreground relation does not match callback timing",
    ))
}

fn handle_def_window_proc_pointer(
    endpoint: &ApiEndpointIdentity,
    runtime: &crate::runtime::Runtime,
    window: &RuntimeWindow,
    message: u32,
    w_param: u32,
    l_param: u32,
) -> Result<ApiImmediate, ApiFault> {
    let handle = window.handle;
    if window.lifecycle != RuntimeWindowLifecycle::Created
        || !window.visible
        || runtime
            .active_window_handle()
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?
            != Some(handle)
        || runtime
            .focused_window_handle()
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?
            != Some(handle)
        || runtime
            .foreground_window_handle()
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?
            != Some(handle)
    {
        return Err(ApiFault::new(
            "pointer default processing requires the visible active focused foreground window",
        ));
    }
    let allowed = MK_LBUTTON | MK_RBUTTON | MK_SHIFT | MK_CONTROL;
    let required_button = match message {
        WM_LBUTTONDOWN | WM_LBUTTONDBLCLK | WM_LBUTTONUP => MK_LBUTTON,
        WM_RBUTTONDOWN | WM_RBUTTONDBLCLK | WM_RBUTTONUP => MK_RBUTTON,
        _ => 0,
    };
    let pressed = matches!(
        message,
        WM_LBUTTONDOWN | WM_LBUTTONDBLCLK | WM_RBUTTONDOWN | WM_RBUTTONDBLCLK
    );
    if w_param & !allowed != 0
        || (required_button != 0 && (w_param & required_button != 0) != pressed)
    {
        return Err(ApiFault::new(
            "pointer message has inconsistent Win32 button key state",
        ));
    }
    let x = i32::from(l_param as i16);
    let y = i32::from((l_param >> 16) as i16);
    if x < 0
        || y < 0
        || u32::try_from(x)
            .ok()
            .is_none_or(|x| x >= window.creation.geometry.client_width)
        || u32::try_from(y)
            .ok()
            .is_none_or(|y| y >= window.creation.geometry.client_height)
    {
        return Err(ApiFault::new(
            "pointer message position is outside the client area",
        ));
    }
    Ok(ApiImmediate::new(Some(0)))
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

fn create_window_callback_request(
    window: &RuntimeWindow,
    class: &crate::runtime::RuntimeWindowClass,
    message: RuntimeWindowCreationMessage,
    return_sentinel: GuestAddress,
    endpoint: &ApiEndpointIdentity,
) -> Result<GuestCallbackRequest, ApiFault> {
    if class.id != window.creation.class.class_id
        || class.atom != window.creation.class.atom
        || class.registration.module.module_id != window.creation.class.module_id
    {
        return Err(ApiFault::new(
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
            let title = window
                .creation
                .name
                .windows_932_bytes()
                .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
            scratch.extend_from_slice(&title);
            scratch.push(0);
            while scratch.len() % 4 != 0 {
                scratch.push(0);
            }
            let class_offset = u32::try_from(scratch.len()).map_err(|_| {
                ApiFault::new("CreateWindowExA callback scratch does not fit x86 accounting")
            })?;
            scratch.extend_from_slice(class.registration.class_name.as_bytes());
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
                .ok_or_else(|| ApiFault::new("validated logical outer rectangle overflowed"))?;
            let bottom = geometry
                .y
                .checked_add_unsigned(geometry.height)
                .ok_or_else(|| ApiFault::new("validated logical outer rectangle overflowed"))?;
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
    let width = u16::try_from(geometry.client_width)
        .map_err(|_| ApiFault::new("logical client width does not fit WM_SIZE lParam"))?;
    let height = u16::try_from(geometry.client_height)
        .map_err(|_| ApiFault::new("logical client height does not fit WM_SIZE lParam"))?;
    Ok(u32::from(width) | (u32::from(height) << 16))
}

fn window_creation_move_l_param(geometry: &RuntimeWindowGeometry) -> Result<u32, ApiFault> {
    let x = i16::try_from(geometry.client_x)
        .map_err(|_| ApiFault::new("logical client x coordinate does not fit WM_MOVE lParam"))?;
    let y = i16::try_from(geometry.client_y)
        .map_err(|_| ApiFault::new("logical client y coordinate does not fit WM_MOVE lParam"))?;
    Ok(u32::from(x as u16) | (u32::from(y as u16) << 16))
}

fn read_bounded_windows_932_text_a(
    memory: &GuestMemory,
    address: GuestAddress,
    maximum_bytes: usize,
    kind: &'static str,
    endpoint: &ApiEndpointIdentity,
) -> Result<String, ApiFault> {
    let mut bytes = Vec::with_capacity(maximum_bytes);
    for offset in 0..maximum_bytes {
        let offset = u32::try_from(offset).map_err(|_| {
            ApiFault::new("validated Windows-932 text bound does not fit x86 accounting")
        })?;
        let byte_address = address
            .0
            .checked_add(offset)
            .map(GuestAddress)
            .ok_or_else(|| {
                ApiFault::new("Windows-932 text read overflowed x86 address accounting")
            })?;
        let byte = memory
            .read(byte_address, 1)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?[0];
        if byte == 0 {
            let (decoded, had_errors) = SHIFT_JIS.decode_without_bom_handling(&bytes);
            if had_errors {
                return Err(ApiFault::new(format!("{kind} is not valid Windows-932")));
            }
            return Ok(decoded.into_owned());
        }
        bytes.push(byte);
    }
    Err(ApiFault::new(format!(
        "{kind} is not NUL-terminated within the controlled bound"
    )))
}

fn set_window_text_default(
    endpoint: &ApiEndpointIdentity,
    runtime: &mut crate::runtime::Runtime,
    handle: u32,
    title_address: u32,
) -> Result<ApiImmediate, ApiFault> {
    let title = if title_address == 0 {
        RuntimeWindowNameA {
            value: String::new(),
        }
    } else {
        read_bounded_window_title_a(
            &runtime.memory,
            GuestAddress(title_address),
            WINDOW_TITLE_A_MAX_BYTES,
            endpoint,
        )?
    };
    runtime
        .set_window_title(handle, title)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(1)))
}

fn read_bounded_window_title_a(
    memory: &GuestMemory,
    address: GuestAddress,
    maximum_bytes: usize,
    endpoint: &ApiEndpointIdentity,
) -> Result<RuntimeWindowNameA, ApiFault> {
    let mut bytes = Vec::with_capacity(maximum_bytes);
    for offset in 0..maximum_bytes {
        let offset = u32::try_from(offset).map_err(|_| {
            ApiFault::new("validated window-title bound does not fit x86 accounting")
        })?;
        let byte_address = address
            .0
            .checked_add(offset)
            .map(GuestAddress)
            .ok_or_else(|| ApiFault::new("window-title read overflowed x86 address accounting"))?;
        let byte = memory
            .read(byte_address, 1)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?[0];
        if byte == 0 {
            let (decoded, had_errors) = SHIFT_JIS.decode_without_bom_handling(&bytes);
            if had_errors {
                return Err(ApiFault::new(
                    "window title is not a valid bounded Windows-932 byte sequence",
                ));
            }
            return Ok(RuntimeWindowNameA {
                value: decoded.into_owned(),
            });
        }
        bytes.push(byte);
    }
    Err(ApiFault::new(
        "window title is not NUL-terminated within the controlled Windows-932 bound",
    ))
}

fn create_window_failure(last_error: u32) -> ApiImmediate {
    ApiImmediate::new(Some(0)).with_last_error(last_error)
}

fn read_bounded_ascii_nul(
    memory: &GuestMemory,
    address: GuestAddress,
    maximum_bytes: usize,
    endpoint: &ApiEndpointIdentity,
) -> Result<String, ApiFault> {
    let mut bytes = Vec::with_capacity(maximum_bytes);
    for offset in 0..maximum_bytes {
        let offset = u32::try_from(offset)
            .map_err(|_| ApiFault::new("validated class name bound does not fit x86 accounting"))?;
        let byte_address = address
            .0
            .checked_add(offset)
            .map(GuestAddress)
            .ok_or_else(|| ApiFault::new("class name read overflowed x86 address accounting"))?;
        let byte = memory
            .read(byte_address, 1)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?[0];
        if byte == 0 {
            let (decoded, had_errors) = SHIFT_JIS.decode_without_bom_handling(&bytes);
            if had_errors {
                return Err(ApiFault::new(
                    "RegisterClassExA class name is not valid Windows-932",
                ));
            }
            return Ok(decoded.into_owned());
        }
        bytes.push(byte);
    }
    Err(ApiFault::new(
        "RegisterClassExA class name is not NUL-terminated within the controlled bound",
    ))
}

fn integer_language_id(entry: &ResourceDataEntryPlan) -> Option<u32> {
    match &entry.language {
        ResourceIdentifier::Id { id } => Some(*id),
        ResourceIdentifier::Name { .. } => None,
    }
}

fn resource_address(load_base: u32, rva: u32) -> Result<GuestAddress, ApiFault> {
    load_base.checked_add(rva).map(GuestAddress).ok_or_else(|| {
        ApiFault::new("registered module load base plus resource RVA overflowed x86 accounting")
    })
}

fn select_group_icon(
    bytes: &[u8],
    target_width: u16,
    target_height: u16,
) -> Result<SelectedGroupIcon, GroupIconSelectionError> {
    if bytes.len() < 6 || read_u16(bytes, 0) != Some(0) || read_u16(bytes, 2) != Some(1) {
        return Err(GroupIconSelectionError::Malformed);
    }
    let count = read_u16(bytes, 4).ok_or(GroupIconSelectionError::Malformed)?;
    if count == 0 || count > MAX_GROUP_IMAGE_ENTRIES {
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
        if width != target_width || height != target_height {
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
    target_width: u16,
    target_height: u16,
) -> Result<SelectedGroupCursor, GroupCursorSelectionError> {
    if bytes.len() < 6 || read_u16(bytes, 0) != Some(0) || read_u16(bytes, 2) != Some(2) {
        return Err(GroupCursorSelectionError::Malformed);
    }
    let count = read_u16(bytes, 4).ok_or(GroupCursorSelectionError::Malformed)?;
    if count == 0 || count > MAX_GROUP_IMAGE_ENTRIES {
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
        if width != target_width || height != target_height {
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
    ApiImmediate::new(Some(0)).with_last_error(last_error)
}

fn load_cursor_failure(last_error: u32) -> ApiImmediate {
    ApiImmediate::new(Some(0)).with_last_error(last_error)
}

fn register_class_failure(last_error: u32) -> ApiImmediate {
    ApiImmediate::new(Some(0)).with_last_error(last_error)
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

fn handle_set_window_pos(
    callback_return_sentinel: GuestAddress,
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiHandlerResult, ApiFault> {
    let window_handle = required_argument(endpoint, frame, 0, "window handle")?;
    let insert_after = required_argument(endpoint, frame, 1, "insert-after")?;
    let x = required_argument(endpoint, frame, 2, "x")? as i32;
    let y = required_argument(endpoint, frame, 3, "y")? as i32;
    let width = required_argument(endpoint, frame, 4, "width")?;
    let height = required_argument(endpoint, frame, 5, "height")?;
    let flags = required_argument(endpoint, frame, 6, "flags")?;
    // Show/move has guest-provided coordinates; hide ignores the position under SWP_NOMOVE.
    let activate_or_resize = flags == 0x010a && width != 0 && height != 0 && x == 0 && y == 0;
    let frame_change_move = flags == 0x25 && insert_after == HWND_TOP && width == 0 && height == 0;
    if !frame_change_move
        && (insert_after != HWND_NOTOPMOST
            || (!activate_or_resize
                && (width != 0 || height != 0 || !matches!(flags, 0x0559 | 0x059b))))
    {
        return Err(ApiFault::new(
            "only initial show/hide and bounded same-position activation/resize are implemented",
        ));
    }
    let window = match runtime.window_option(window_handle) {
        Ok(Some(window)) => window,
        Ok(None) => {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(Some(u32::from(false)))
                    .with_last_error(ERROR_INVALID_WINDOW_HANDLE),
            ));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, endpoint)),
    };
    if frame_change_move {
        let mut geometry = window.creation.geometry;
        let (Some(client_offset_x), Some(client_offset_y)) = (
            geometry.client_x.checked_sub(geometry.x),
            geometry.client_y.checked_sub(geometry.y),
        ) else {
            return Err(ApiFault::new("existing client offset overflowed"));
        };
        let (Some(client_x), Some(client_y)) = (
            x.checked_add(client_offset_x),
            y.checked_add(client_offset_y),
        ) else {
            return Err(ApiFault::new("moved client position overflowed"));
        };
        geometry.x = x;
        geometry.y = y;
        geometry.client_x = client_x;
        geometry.client_y = client_y;
        window_creation_move_l_param(&geometry)?;
        let entry_last_error = runtime
            .current_last_error()
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        return begin_set_window_pos(
            callback_return_sentinel,
            endpoint,
            runtime,
            RuntimeSetWindowPosRequest {
                window_handle,
                variant: RuntimeSetWindowPosVariant::FrameChangeMove,
                next_geometry: geometry,
                next_visible: true,
                next_topmost: false,
                entry_last_error,
                callback_flags: flags,
                callback_insert_after: insert_after,
                activation_insert_after: 0,
                activation_internal_flags: 3,
                resize_nccalc_window_pos_flags: 0,
                resize_changed_flags: 0,
                frame_change_nccalc_window_pos_flags: 0x1825,
                frame_change_changed_flags: 0x0825,
            },
        );
    }
    if activate_or_resize {
        let mut geometry = window.creation.geometry;
        let same_size = geometry.width == width && geometry.height == height;
        if !same_size {
            // Preserve the current non-client extents, rather than using a title-sized
            // profile or making the browser canvas authoritative for Windows geometry.
            let frame_width = geometry.width.checked_sub(geometry.client_width);
            let frame_height = geometry.height.checked_sub(geometry.client_height);
            let (Some(client_width), Some(client_height)) = (
                frame_width.and_then(|frame| width.checked_sub(frame)),
                frame_height.and_then(|frame| height.checked_sub(frame)),
            ) else {
                return Err(ApiFault::new(
                    "outer size cannot contain the window's non-client frame",
                ));
            };
            geometry.width = width;
            geometry.height = height;
            geometry.client_width = client_width;
            geometry.client_height = client_height;
        }
        let entry_last_error = runtime
            .current_last_error()
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        return begin_set_window_pos(
            callback_return_sentinel,
            endpoint,
            runtime,
            RuntimeSetWindowPosRequest {
                window_handle,
                variant: if same_size {
                    RuntimeSetWindowPosVariant::ActivateSameGeometry
                } else {
                    RuntimeSetWindowPosVariant::Resize
                },
                next_geometry: geometry,
                next_visible: true,
                next_topmost: false,
                entry_last_error,
                callback_flags: flags,
                callback_insert_after: insert_after,
                activation_insert_after: 0,
                activation_internal_flags: 3,
                resize_nccalc_window_pos_flags: 0x190e,
                resize_changed_flags: 0x110e,
                frame_change_nccalc_window_pos_flags: 0,
                frame_change_changed_flags: 0,
            },
        );
    }
    let showing = flags == 0x0559;
    let mut geometry = window.creation.geometry;
    if showing {
        let offset_x = geometry.client_x - geometry.x;
        let offset_y = geometry.client_y - geometry.y;
        let (Some(client_x), Some(client_y)) = (x.checked_add(offset_x), y.checked_add(offset_y))
        else {
            return Err(ApiFault::new(
                "client position is outside signed Windows coordinates",
            ));
        };
        geometry.x = x;
        geometry.y = y;
        geometry.client_x = client_x;
        geometry.client_y = client_y;
        if window_creation_move_l_param(&geometry).is_err() {
            return Err(ApiFault::new(
                "window move notification requires signed 16-bit client coordinates",
            ));
        }
    }
    let entry_last_error = runtime
        .current_last_error()
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    begin_set_window_pos(
        callback_return_sentinel,
        endpoint,
        runtime,
        RuntimeSetWindowPosRequest {
            window_handle,
            variant: if showing {
                RuntimeSetWindowPosVariant::InitialShow
            } else {
                RuntimeSetWindowPosVariant::InitialHide
            },
            next_geometry: geometry,
            next_visible: showing,
            next_topmost: false,
            entry_last_error,
            callback_flags: if showing {
                flags | 0x1000_0800
            } else {
                flags | 0x1804
            },
            callback_insert_after: insert_after,
            activation_insert_after: 0,
            activation_internal_flags: 3,
            resize_nccalc_window_pos_flags: 0,
            resize_changed_flags: 0,
            frame_change_nccalc_window_pos_flags: 0,
            frame_change_changed_flags: 0,
        },
    )
}

fn handle_send_message_a(
    callback_return_sentinel: GuestAddress,
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiHandlerResult, ApiFault> {
    let window = required_argument(endpoint, frame, 0, "window handle")?;
    let message = required_argument(endpoint, frame, 1, "message")?;
    let w_param = required_argument(endpoint, frame, 2, "wParam")?;
    let l_param = required_argument(endpoint, frame, 3, "lParam")?;
    if message == USER32_WM_IME_CONTROL {
        let request = match w_param {
            USER32_IMC_CLOSESTATUSWINDOW if l_param == 0 => RuntimeImeStatusWindowRequest::Close,
            USER32_IMC_OPENSTATUSWINDOW if l_param == 0 => RuntimeImeStatusWindowRequest::Open,
            _ => {
                return Err(ApiFault::new(
                    "WM_IME_CONTROL currently supports opening and closing the status window",
                ));
            }
        };
        runtime
            .control_ime_status_window(window, request)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(0))));
    }
    if message != WM_SETICON || w_param != ICON_SMALL {
        return Err(ApiFault::new(
            "SendMessageA currently forwards synchronous WM_SETICON for a small icon",
        ));
    }
    let procedure = runtime
        .resolve_small_icon_window_procedure(window, l_param)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let request = GuestCallbackRequest {
        target: procedure.address,
        arguments: [window, message, w_param, l_param]
            .into_iter()
            .map(GuestCallbackArgument::U32)
            .collect(),
        scratch: Vec::new(),
        scratch_pointer_relocations: Vec::new(),
        return_sentinel: callback_return_sentinel,
        completion_policy: GuestCallbackCompletionPolicy::ReturnU32,
    };
    Ok(ApiHandlerResult::GuestCallback(Box::new(request)))
}

fn handle_def_window_proc_position(
    callback_return_sentinel: GuestAddress,
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiHandlerResult, ApiFault> {
    let handle = required_argument(endpoint, frame, 0, "window handle")?;
    let w_param = required_argument(endpoint, frame, 2, "wParam")?;
    let l_param = GuestAddress(required_argument(endpoint, frame, 3, "WINDOWPOS pointer")?);
    let window = runtime
        .window(handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    if w_param != 0
        || window.creation.owner_thread_id != runtime.current_thread_id()
        || window.lifecycle != RuntimeWindowLifecycle::Created
    {
        return Err(ApiFault::new(
            "default WINDOWPOSCHANGED processing requires a created owner-thread window",
        ));
    }
    let words = read_window_pos_words(&runtime.memory, l_param, endpoint)?;
    let resized = &window.creation.geometry;
    if words
        == [
            handle,
            HWND_NOTOPMOST,
            resized.x as u32,
            resized.y as u32,
            resized.width,
            resized.height,
            0x110e,
        ]
    {
        require_window_activation_relations(runtime, handle, true, true, true, endpoint)?;
        return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(0))));
    }
    if words
        == [
            handle,
            HWND_TOP,
            resized.x as u32,
            resized.y as u32,
            resized.width,
            resized.height,
            0x0825,
        ]
    {
        require_window_activation_relations(runtime, handle, true, true, true, endpoint)?;
        let procedure = runtime
            .resolve_window_procedure(handle)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        let request = GuestCallbackRequest {
            target: procedure.address,
            arguments: [handle, WM_MOVE, 0, window_creation_move_l_param(resized)?]
                .into_iter()
                .map(GuestCallbackArgument::U32)
                .collect(),
            scratch: Vec::new(),
            scratch_pointer_relocations: Vec::new(),
            return_sentinel: callback_return_sentinel,
            completion_policy: GuestCallbackCompletionPolicy::ReturnConstantU32 { value: 0 },
        };
        return Ok(ApiHandlerResult::GuestCallback(Box::new(request)));
    }
    let showing = !window.visible && words[6] == 0x1000_0d59;
    let hiding = window.visible && words[6] == 0x0000_1d9f;
    if words[0] != handle
        || words[1] != HWND_NOTOPMOST
        || words[4] != window.creation.geometry.width
        || words[5] != window.creation.geometry.height
        || !(showing || hiding)
        || (hiding
            && (words[2] != window.creation.geometry.x as u32
                || words[3] != window.creation.geometry.y as u32))
    {
        return Err(ApiFault::new(
            "WINDOWPOSCHANGED does not match this window's pending show or hide",
        ));
    }
    let mut geometry = window.creation.geometry;
    if showing {
        let (x, y) = (words[2] as i32, words[3] as i32);
        geometry.x = x;
        geometry.y = y;
        geometry.client_x = x
            .checked_add(geometry.client_x - window.creation.geometry.x)
            .ok_or_else(|| ApiFault::new("client x overflowed"))?;
        geometry.client_y = y
            .checked_add(geometry.client_y - window.creation.geometry.y)
            .ok_or_else(|| ApiFault::new("client y overflowed"))?;
    }
    runtime
        .preview_set_window_pos(RuntimeSetWindowPosRequest {
            window_handle: handle,
            variant: if showing {
                RuntimeSetWindowPosVariant::InitialShow
            } else {
                RuntimeSetWindowPosVariant::InitialHide
            },
            next_geometry: geometry,
            next_visible: showing,
            next_topmost: false,
            entry_last_error: runtime
                .current_last_error()
                .map_err(|error| ApiFault::from_runtime(error, endpoint))?,
            callback_flags: words[6],
            callback_insert_after: words[1],
            activation_insert_after: 0,
            activation_internal_flags: 3,
            resize_nccalc_window_pos_flags: 0,
            resize_changed_flags: 0,
            frame_change_nccalc_window_pos_flags: 0,
            frame_change_changed_flags: 0,
        })
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    if hiding {
        return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(0))));
    }
    let procedure = runtime
        .resolve_window_procedure(handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let request = GuestCallbackRequest {
        target: procedure.address,
        arguments: [handle, WM_MOVE, 0, window_creation_move_l_param(&geometry)?]
            .into_iter()
            .map(GuestCallbackArgument::U32)
            .collect(),
        scratch: Vec::new(),
        scratch_pointer_relocations: Vec::new(),
        return_sentinel: callback_return_sentinel,
        completion_policy: GuestCallbackCompletionPolicy::ReturnConstantU32 { value: 0 },
    };
    Ok(ApiHandlerResult::GuestCallback(Box::new(request)))
}

fn handle_def_window_proc_activation(
    return_sentinel: GuestAddress,
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiHandlerResult, ApiFault> {
    let handle = required_argument(endpoint, frame, 0, "window handle")?;
    let message = required_argument(endpoint, frame, 1, "message")?;
    let w_param = required_argument(endpoint, frame, 2, "wParam")?;
    let l_param = required_argument(endpoint, frame, 3, "lParam")?;
    let window = runtime
        .window(handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    if window.lifecycle != RuntimeWindowLifecycle::Created
        || window.creation.owner_thread_id != runtime.current_thread_id()
        || w_param != 1
        || (message == WM_ACTIVATE && l_param != 0)
        || (message == WM_IME_SETCONTEXT && l_param != 0xc000_000f)
    {
        return Err(ApiFault::new(
            "DefWindowProcA activation requires the created owner-thread window and bounded message parameters",
        ));
    }
    if message == WM_ACTIVATE {
        require_window_activation_relations(runtime, handle, true, false, true, endpoint)?;
        let preview = runtime
            .preview_set_window_pos_focus(handle)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        let request = set_window_pos_default_callback_request(
            preview,
            RuntimeSetWindowPosDefaultCallbackMessage::ImeSetContext,
            return_sentinel,
            0xc000_000f,
            runtime,
            endpoint,
        )?;
        runtime.preflight_guest_callback(frame, &request)?;
        runtime
            .focus_set_window_pos_window(handle)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        Ok(ApiHandlerResult::GuestCallback(Box::new(request)))
    } else {
        require_window_activation_relations(runtime, handle, true, true, true, endpoint)?;
        let procedure = runtime
            .resolve_window_procedure(handle)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        let request = GuestCallbackRequest {
            target: procedure.address,
            arguments: [handle, WM_IME_NOTIFY, 2, 0]
                .into_iter()
                .map(GuestCallbackArgument::U32)
                .collect(),
            scratch: Vec::new(),
            scratch_pointer_relocations: Vec::new(),
            return_sentinel,
            completion_policy: GuestCallbackCompletionPolicy::ReturnConstantU32 { value: 0 },
        };
        Ok(ApiHandlerResult::GuestCallback(Box::new(request)))
    }
}

fn handle_update_window(
    callback_return_sentinel: GuestAddress,
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiHandlerResult, ApiFault> {
    let handle = required_argument(endpoint, frame, 0, "window handle")?;
    match runtime.window_option(handle) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(Some(u32::from(false)))
                    .with_last_error(ERROR_INVALID_WINDOW_HANDLE),
            ));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, endpoint)),
    }
    let last_error = runtime
        .current_last_error()
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let operation = runtime
        .preview_update_window(handle, last_error)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let procedure = runtime
        .resolve_window_procedure(handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let request =
        update_window_callback_request(operation, procedure, WM_PAINT, callback_return_sentinel);
    Ok(ApiHandlerResult::GuestCallback(Box::new(request)))
}

fn handle_begin_paint(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let handle = required_argument(endpoint, frame, 0, "window handle")?;
    let output = GuestAddress(required_argument(endpoint, frame, 1, "PAINTSTRUCT output")?);
    match runtime.window_option(handle) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Ok(ApiImmediate::new(Some(0)).with_last_error(ERROR_INVALID_WINDOW_HANDLE));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, endpoint)),
    }
    let device_context_handle = runtime
        .begin_window_paint(handle, output)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(device_context_handle)))
}

fn handle_end_paint(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let handle = required_argument(endpoint, frame, 0, "window handle")?;
    let input = GuestAddress(required_argument(endpoint, frame, 1, "PAINTSTRUCT input")?);
    match runtime.window_option(handle) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Ok(ApiImmediate::new(Some(u32::from(false)))
                .with_last_error(ERROR_INVALID_WINDOW_HANDLE));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, endpoint)),
    }
    let bytes = runtime
        .memory
        .read(input, RUNTIME_X86_PAINT_STRUCT_BYTE_LENGTH)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    runtime
        .end_window_paint(handle, input, &bytes)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(u32::from(true))))
}

fn handle_show_window(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let handle = required_argument(endpoint, frame, 0, "window handle")?;
    let command = required_argument(endpoint, frame, 1, "show command")?;
    if command != 1 {
        return Err(ApiFault::new(
            "ShowWindow currently supports SW_SHOWNORMAL on an already-visible window",
        ));
    }
    let window = match runtime.window_option(handle) {
        Ok(Some(window)) => window,
        Ok(None) => {
            return Ok(ApiImmediate::new(Some(u32::from(false)))
                .with_last_error(ERROR_INVALID_WINDOW_HANDLE));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, endpoint)),
    };
    if window.creation.owner_thread_id != runtime.current_thread_id()
        || window.lifecycle != RuntimeWindowLifecycle::Created
        || !window.visible
        || window.creation.effective_style & RUNTIME_WINDOW_STYLE_VISIBLE == 0
        || window.topmost
    {
        return Err(ApiFault::new(
            "SW_SHOWNORMAL requires a visible created owner-thread non-topmost window",
        ));
    }
    require_window_activation_relations(runtime, handle, true, true, true, endpoint)?;
    Ok(ApiImmediate::new(Some(u32::from(true))))
}

fn handle_nccalc_resize(
    endpoint: &ApiEndpointIdentity,
    memory: &crate::memory::GuestMemory,
    window: &RuntimeWindow,
    address: u32,
) -> Result<ApiImmediate, ApiFault> {
    let position_address = address
        .checked_add(52)
        .ok_or_else(|| ApiFault::new("NCCALCSIZE_PARAMS crosses the x86 address space"))?;
    let bytes = memory
        .read(GuestAddress(address), 80)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let position = read_window_pos_words(memory, GuestAddress(position_address), endpoint)?;
    let geometry = window.creation.geometry;
    let old_outer = [
        geometry.x,
        geometry.y,
        geometry
            .x
            .checked_add_unsigned(geometry.width)
            .ok_or_else(|| ApiFault::new("outer rectangle overflowed"))?,
        geometry
            .y
            .checked_add_unsigned(geometry.height)
            .ok_or_else(|| ApiFault::new("outer rectangle overflowed"))?,
    ];
    let old_client = [
        geometry.client_x,
        geometry.client_y,
        geometry
            .client_x
            .checked_add_unsigned(geometry.client_width)
            .ok_or_else(|| ApiFault::new("client rectangle overflowed"))?,
        geometry
            .client_y
            .checked_add_unsigned(geometry.client_height)
            .ok_or_else(|| ApiFault::new("client rectangle overflowed"))?,
    ];
    let (width, height) = (i32::try_from(position[4]), i32::try_from(position[5]));
    let (Ok(width), Ok(height)) = (width, height) else {
        return Err(ApiFault::new(
            "NCCALCSIZE requires signed window dimensions",
        ));
    };
    let new_outer = [
        position[2] as i32,
        position[3] as i32,
        (position[2] as i32)
            .checked_add(width)
            .ok_or_else(|| ApiFault::new("resized outer rectangle overflowed"))?,
        (position[3] as i32)
            .checked_add(height)
            .ok_or_else(|| ApiFault::new("resized outer rectangle overflowed"))?,
    ];
    let rect_matches = |offset: usize, expected: [i32; 4]| {
        expected
            .iter()
            .enumerate()
            .all(|(i, value)| read_i32(&bytes, offset + i * 4) == Some(*value))
    };
    let resize = position[1] == HWND_NOTOPMOST
        && position[6] == 0x190e
        && position[2] == geometry.x as u32
        && position[3] == geometry.y as u32;
    let frame_move = position[1] == HWND_TOP
        && position[6] == 0x1825
        && position[4] == geometry.width
        && position[5] == geometry.height;
    if position[0] != window.handle
        || !(resize || frame_move)
        || width <= 0
        || height <= 0
        || read_u32(&bytes, 48) != Some(position_address)
        || !rect_matches(0, new_outer)
        || !rect_matches(16, old_outer)
        || !rect_matches(32, old_client)
    {
        return Err(ApiFault::new(
            "NCCALCSIZE_PARAMS does not describe a supported owner-window resize",
        ));
    }
    let new_client = [
        old_client[0]
            .checked_sub(old_outer[0])
            .and_then(|offset| new_outer[0].checked_add(offset)),
        old_client[1]
            .checked_sub(old_outer[1])
            .and_then(|offset| new_outer[1].checked_add(offset)),
        old_outer[2]
            .checked_sub(old_client[2])
            .and_then(|offset| new_outer[2].checked_sub(offset)),
        old_outer[3]
            .checked_sub(old_client[3])
            .and_then(|offset| new_outer[3].checked_sub(offset)),
    ];
    let [Some(left), Some(top), Some(right), Some(bottom)] = new_client else {
        return Err(ApiFault::new("resized client rectangle overflowed"));
    };
    if right < left || bottom < top {
        return Err(ApiFault::new(
            "resized outer rectangle cannot contain the non-client frame",
        ));
    }
    Ok(ApiImmediate::new(Some(0)).with_guest_write(
        GuestAddress(address),
        [left, top, right, bottom]
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect(),
    ))
}

fn handle_def_window_proc_a(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let handle = required_argument(endpoint, frame, 0, "window handle")?;
    let message = required_argument(endpoint, frame, 1, "message")?;
    let w_param = required_argument(endpoint, frame, 2, "wParam")?;
    let l_param = required_argument(endpoint, frame, 3, "lParam")?;
    let window = runtime
        .window(handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
        .clone();
    if window.creation.owner_thread_id != runtime.current_thread_id() {
        return Err(ApiFault::new(
            "default processing requires the creating thread",
        ));
    }
    let geometry = window.creation.geometry;
    if matches!(message, WM_KEYDOWN | WM_KEYUP | WM_CHAR) {
        validate_queued_message_dispatch(
            X86Message {
                window: handle,
                message,
                w_param,
                l_param,
                time: 0,
                point_x: 0,
                point_y: 0,
            },
            endpoint,
            runtime,
        )?;
        return Ok(ApiImmediate::new(Some(0)));
    }
    if matches!(
        message,
        WM_MOUSEMOVE
            | WM_LBUTTONDOWN
            | WM_LBUTTONUP
            | WM_LBUTTONDBLCLK
            | WM_RBUTTONDOWN
            | WM_RBUTTONUP
            | WM_RBUTTONDBLCLK
    ) {
        return handle_def_window_proc_pointer(
            endpoint, runtime, &window, message, w_param, l_param,
        );
    }
    let result = match (message, w_param, window.lifecycle) {
        // Private-class and application messages have no default window action.
        (0x0400..=0xbfff, _, RuntimeWindowLifecycle::Created) => ApiImmediate::new(Some(0)),
        (WM_GETICON, ICON_SMALL | ICON_SMALL2, RuntimeWindowLifecycle::Created) if l_param == 0 => {
            ApiImmediate::new(Some(window.small_icon.map_or(0, |icon| icon.handle)))
        }
        (WM_SETICON, ICON_SMALL, RuntimeWindowLifecycle::Created) => {
            let previous = runtime
                .set_window_small_icon(handle, l_param)
                .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
            ApiImmediate::new(Some(previous))
        }
        (WM_SETTEXT, 0, RuntimeWindowLifecycle::Created) => {
            set_window_text_default(endpoint, runtime, handle, l_param)?
        }
        (WM_MOVE, 0, RuntimeWindowLifecycle::Created) => ApiImmediate::new(Some(0)),
        (WM_NCCALCSIZE, 1, RuntimeWindowLifecycle::Created) => {
            handle_nccalc_resize(endpoint, &runtime.memory, &window, l_param)?
        }
        (WM_SIZE, 0, RuntimeWindowLifecycle::Created)
            if l_param == window_creation_size_l_param(&geometry)? =>
        {
            ApiImmediate::new(Some(0))
        }
        (WM_ACTIVATEAPP, 1, RuntimeWindowLifecycle::Created) if l_param == 0 => {
            require_window_activation_relations(runtime, handle, true, false, true, endpoint)?;
            ApiImmediate::new(Some(0))
        }
        (WM_NCACTIVATE, 1, RuntimeWindowLifecycle::Created) if l_param == 0 => {
            require_window_activation_relations(runtime, handle, true, false, true, endpoint)?;
            ApiImmediate::new(Some(1))
        }
        (WM_IME_NOTIFY, 2, RuntimeWindowLifecycle::Created) if l_param == 0 => {
            require_window_activation_relations(runtime, handle, true, true, true, endpoint)?;
            ApiImmediate::new(Some(0))
        }
        (WM_SETFOCUS, 0, RuntimeWindowLifecycle::Created) if l_param == 0 => {
            require_window_activation_relations(runtime, handle, true, true, true, endpoint)?;
            ApiImmediate::new(Some(0))
        }
        (WM_WINDOWPOSCHANGING, 0, RuntimeWindowLifecycle::Created) => {
            let words = read_window_pos_words(&runtime.memory, GuestAddress(l_param), endpoint)?;
            let external = words[1] == HWND_NOTOPMOST
                && words[2] == 0
                && words[3] == 0
                && words[4] != 0
                && words[5] != 0
                && words[6] == 0x010a;
            let activation = words[1] == 0 && words[2..] == [0, 0, 0, 0, 3];
            let frame_move =
                words[1] == HWND_TOP && words[4] == 0 && words[5] == 0 && words[6] == 0x25;
            if words[0] != handle || !(external || activation || frame_move) {
                return Err(ApiFault::new(
                    "WM_WINDOWPOSCHANGING has an unsupported positioning request",
                ));
            }
            ApiImmediate::new(Some(0))
        }
        (
            0x0081,
            0,
            RuntimeWindowLifecycle::Creating {
                message: RuntimeWindowCreationMessage::NonClientCreate,
            },
        ) => {
            runtime
                .memory
                .validate_access(GuestAddress(l_param), 48, MemoryAccess::Read)
                .map_err(|error| ApiFault::from_memory(error, endpoint))?;
            ApiImmediate::new(Some(1))
        }
        (
            0x0083,
            0,
            RuntimeWindowLifecycle::Creating {
                message: RuntimeWindowCreationMessage::NonClientCalculate,
            },
        ) => {
            let outer_right = geometry
                .x
                .checked_add_unsigned(geometry.width)
                .ok_or_else(|| ApiFault::new("window right edge overflowed"))?;
            let outer_bottom = geometry
                .y
                .checked_add_unsigned(geometry.height)
                .ok_or_else(|| ApiFault::new("window bottom edge overflowed"))?;
            let expected: Vec<u8> = [geometry.x, geometry.y, outer_right, outer_bottom]
                .into_iter()
                .flat_map(i32::to_le_bytes)
                .collect();
            let before = runtime
                .memory
                .read(GuestAddress(l_param), 16)
                .map_err(|error| ApiFault::from_memory(error, endpoint))?;
            if before != expected {
                return Err(ApiFault::new(
                    "WM_NCCALCSIZE RECT does not match the pending outer window",
                ));
            }
            let client_right = geometry
                .client_x
                .checked_add_unsigned(geometry.client_width)
                .ok_or_else(|| ApiFault::new("client right edge overflowed"))?;
            let client_bottom = geometry
                .client_y
                .checked_add_unsigned(geometry.client_height)
                .ok_or_else(|| ApiFault::new("client bottom edge overflowed"))?;
            let client_rect: Vec<u8> = [
                geometry.client_x,
                geometry.client_y,
                client_right,
                client_bottom,
            ]
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect();
            ApiImmediate::new(Some(0)).with_guest_write(GuestAddress(l_param), client_rect)
        }
        (
            0x0001,
            0,
            RuntimeWindowLifecycle::Creating {
                message: RuntimeWindowCreationMessage::Create,
            },
        ) => ApiImmediate::new(Some(0)),
        (
            WM_SIZE,
            0,
            RuntimeWindowLifecycle::Creating {
                message: RuntimeWindowCreationMessage::Size,
            },
        ) if l_param == window_creation_size_l_param(&geometry)? => ApiImmediate::new(Some(0)),
        (
            WM_MOVE,
            0,
            RuntimeWindowLifecycle::Creating {
                message: RuntimeWindowCreationMessage::Move,
            },
        ) if l_param == window_creation_move_l_param(&geometry)? => ApiImmediate::new(Some(0)),
        (
            WM_SIZE,
            0,
            RuntimeWindowLifecycle::Creating {
                message: RuntimeWindowCreationMessage::Size,
            },
        ) => {
            return Err(ApiFault::new(
                "WM_SIZE lParam does not match the pending window client dimensions",
            ));
        }
        (
            WM_MOVE,
            0,
            RuntimeWindowLifecycle::Creating {
                message: RuntimeWindowCreationMessage::Move,
            },
        ) => {
            return Err(ApiFault::new(
                "WM_MOVE lParam does not match the pending window client position",
            ));
        }
        _ => {
            return Err(ApiFault::new(format!(
                "DefWindowProcA does not implement message {message:#x} in this window state"
            )));
        }
    };
    Ok(result)
}

fn handle_message_box_a(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiHandlerResult, ApiFault> {
    let owner = required_argument(endpoint, frame, 0, "owner")?;
    let text = required_argument(endpoint, frame, 1, "text")?;
    let title = required_argument(endpoint, frame, 2, "caption")?;
    let flags = required_argument(endpoint, frame, 3, "type")?;
    if owner != 0
        && !runtime
            .window_option(owner)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?
            .is_some_and(|window| window.lifecycle == RuntimeWindowLifecycle::Created)
    {
        return Ok(ApiHandlerResult::Immediate(
            ApiImmediate::new(Some(0)).with_last_error(ERROR_INVALID_WINDOW_HANDLE),
        ));
    }
    let text = if text == 0 {
        String::new()
    } else {
        read_bounded_windows_932_text_a(
            &runtime.memory,
            GuestAddress(text),
            65536,
            "MessageBox text",
            endpoint,
        )?
    };
    let title = if title == 0 {
        "エラー".to_owned()
    } else {
        read_bounded_windows_932_text_a(
            &runtime.memory,
            GuestAddress(title),
            4096,
            "MessageBox caption",
            endpoint,
        )?
    };
    runtime
        .begin_message_box(owner, title, text, flags)
        .map(ApiHandlerResult::Deferred)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))
}

pub fn complete_user32_message_box(
    dispatcher: &ApiDispatcher,
    call: &ApiDeferredCall,
    button_id: u32,
    runtime: &mut crate::runtime::Runtime,
) -> DispatchOutcome {
    if call.request.service != crate::runtime::RUNTIME_MESSAGE_BOX_SERVICE
        || !call
            .frame
            .endpoint
            .module
            .eq_ignore_ascii_case("user32.dll")
        || !matches!(&call.frame.endpoint.symbol, ApiSymbol::Name { name } if name == "MessageBoxA")
    {
        return DispatchOutcome::Fault(ApiFault::new("completion does not identify MessageBoxA"));
    }
    let request_id = call.request.request_id;
    if let Err(error) = runtime.validate_message_box_result(request_id, button_id) {
        return DispatchOutcome::Fault(ApiFault::from_runtime(error, &call.frame.endpoint));
    }
    let result = dispatcher.complete_deferred(
        call,
        HostCompletion {
            request_id,
            result: ApiImmediate::new(Some(button_id)),
        },
        runtime,
    );
    if matches!(result, DispatchOutcome::Immediate(_)) {
        runtime.commit_message_box_result(request_id);
    }
    result
}

pub(crate) fn dispatch_user32_api(
    callback_return_sentinel: GuestAddress,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let endpoint = &frame.endpoint;
    let result = match &endpoint.symbol {
        ApiSymbol::Name { name } if endpoint.module.eq_ignore_ascii_case("user32.dll") => {
            match name.as_str() {
                "MessageBoxA" => {
                    return match handle_message_box_a(endpoint, frame, runtime) {
                        Ok(result) => result,
                        Err(fault) => ApiHandlerResult::Fault(fault),
                    };
                }
                "EnumDisplayMonitors" => {
                    return enum_display_monitors(
                        callback_return_sentinel,
                        endpoint,
                        frame,
                        runtime,
                    );
                }
                "LoadIconA" => handle_load_icon_a(endpoint, frame, runtime),
                "ShowWindow" => handle_show_window(endpoint, frame, runtime),
                "SetRect" => handle_set_rect(endpoint, frame),
                "UpdateWindow" => {
                    return match handle_update_window(
                        callback_return_sentinel,
                        endpoint,
                        frame,
                        runtime,
                    ) {
                        Ok(result) => result,
                        Err(fault) => ApiHandlerResult::Fault(fault),
                    };
                }
                "BeginPaint" => handle_begin_paint(endpoint, frame, runtime),
                "EndPaint" => handle_end_paint(endpoint, frame, runtime),
                "LoadCursorA" => handle_load_cursor_a(endpoint, frame, runtime),
                "LoadImageA" => handle_load_image_a(endpoint, frame, runtime),
                "RegisterClassExA" => handle_register_class_ex_a(endpoint, frame, runtime),
                "DefWindowProcA" => {
                    if matches!(frame.argument_u32(1), Some(WM_ACTIVATE | WM_IME_SETCONTEXT)) {
                        return match handle_def_window_proc_activation(
                            callback_return_sentinel,
                            endpoint,
                            frame,
                            runtime,
                        ) {
                            Ok(outcome) => outcome,
                            Err(fault) => ApiHandlerResult::Fault(fault),
                        };
                    }
                    if frame.argument_u32(1) == Some(WM_WINDOWPOSCHANGED) {
                        return match handle_def_window_proc_position(
                            callback_return_sentinel,
                            endpoint,
                            frame,
                            runtime,
                        ) {
                            Ok(outcome) => outcome,
                            Err(fault) => ApiHandlerResult::Fault(fault),
                        };
                    }
                    handle_def_window_proc_a(endpoint, frame, runtime)
                }
                "SetWindowTextA" => {
                    return match handle_set_window_text_a(
                        callback_return_sentinel,
                        endpoint,
                        frame,
                        runtime,
                    ) {
                        Ok(result) => result,
                        Err(fault) => ApiHandlerResult::Fault(fault),
                    };
                }
                "SetWindowPos" => {
                    return match handle_set_window_pos(
                        callback_return_sentinel,
                        endpoint,
                        frame,
                        runtime,
                    ) {
                        Ok(outcome) => outcome,
                        Err(fault) => ApiHandlerResult::Fault(fault),
                    };
                }
                "SendMessageA" => {
                    return match handle_send_message_a(
                        callback_return_sentinel,
                        endpoint,
                        frame,
                        runtime,
                    ) {
                        Ok(outcome) => outcome,
                        Err(fault) => ApiHandlerResult::Fault(fault),
                    };
                }
                "GetSystemMenu" => handle_get_system_menu(endpoint, frame, runtime),
                "EnableMenuItem" => handle_enable_menu_item(endpoint, frame, runtime),
                "CreateWindowExA" => {
                    return match handle_create_window_ex_a(
                        callback_return_sentinel,
                        endpoint,
                        frame,
                        runtime,
                    ) {
                        Ok(outcome) => outcome,
                        Err(fault) => ApiHandlerResult::Fault(fault),
                    };
                }
                "GetSystemMetrics" => handle_get_system_metrics(endpoint, frame, runtime),
                "SetFocus" => handle_set_focus(endpoint, frame, runtime),
                "GetForegroundWindow" => handle_get_foreground_window(endpoint, runtime),
                "IsIconic" => handle_is_iconic(endpoint, frame, runtime),
                "PeekMessageA" => handle_peek_message_a(endpoint, frame, runtime),
                "GetQueueStatus" => handle_get_queue_status(endpoint, frame, runtime),
                "TranslateMessage" => handle_translate_message(endpoint, frame, runtime),
                "DispatchMessageA" => {
                    return match handle_dispatch_message_a(
                        callback_return_sentinel,
                        endpoint,
                        frame,
                        runtime,
                    ) {
                        Ok(result) => result,
                        Err(fault) => ApiHandlerResult::Fault(fault),
                    };
                }
                "GetAsyncKeyState" => handle_get_async_key_state(endpoint, frame, runtime),
                "GetCursorPos" => handle_get_cursor_pos(endpoint, frame, runtime),
                "SetCursorPos" => handle_set_cursor_pos(endpoint, frame, runtime),
                "SetCursor" => handle_set_cursor(endpoint, frame, runtime),
                "GetCursorInfo" => handle_get_cursor_info(endpoint, frame, runtime),
                "ShowCursor" => handle_show_cursor(endpoint, frame, runtime),
                "MonitorFromWindow" => handle_monitor_from_window(endpoint, frame, runtime),
                "GetMonitorInfoA" => handle_get_monitor_info_a(endpoint, frame, runtime),
                "GetDC" => handle_get_dc(endpoint, frame, runtime),
                "ReleaseDC" => handle_release_dc(endpoint, frame, runtime),
                "FillRect" => handle_fill_rect(endpoint, frame, runtime),
                "SetWindowLongA" => handle_set_window_long_a(endpoint, frame, runtime),
                "SetTimer" => handle_set_timer(endpoint, frame, runtime),
                "GetClientRect" => handle_get_client_rect(endpoint, frame, runtime),
                "GetWindowRect" => handle_get_window_rect(endpoint, frame, runtime),
                "SwapMouseButton" => {
                    let swapped =
                        match required_argument(endpoint, frame, 0, "mouse button swap flag") {
                            Ok(value) => value != 0,
                            Err(error) => return ApiHandlerResult::Fault(error),
                        };
                    runtime
                        .swap_mouse_buttons(swapped)
                        .map(|previous| ApiImmediate::new(Some(u32::from(previous))))
                        .map_err(|error| ApiFault::from_runtime(error, endpoint))
                }
                _ => Err(ApiFault::new(format!(
                    "unsupported imported function {}!{name}",
                    endpoint.module
                ))),
            }
        }
        _ => Err(ApiFault::new(format!(
            "unsupported imported function {}!{:?}",
            endpoint.module, endpoint.symbol
        ))),
    };
    match result {
        Ok(immediate) => ApiHandlerResult::Immediate(immediate),
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn enum_display_monitors(
    callback_return_sentinel: GuestAddress,
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> ApiHandlerResult {
    let hdc = match required_argument(endpoint, frame, 0, "HDC") {
        Ok(value) => value,
        Err(fault) => return ApiHandlerResult::Fault(fault),
    };
    let clip = match required_argument(endpoint, frame, 1, "clip rectangle") {
        Ok(value) => value,
        Err(fault) => return ApiHandlerResult::Fault(fault),
    };
    let callback = match required_argument(endpoint, frame, 2, "monitor callback") {
        Ok(value) => value,
        Err(fault) => return ApiHandlerResult::Fault(fault),
    };
    let data = match required_argument(endpoint, frame, 3, "callback data") {
        Ok(value) => value,
        Err(fault) => return ApiHandlerResult::Fault(fault),
    };
    if hdc != 0 || clip != 0 {
        return ApiHandlerResult::Fault(ApiFault::new(
            "EnumDisplayMonitors currently supports only NULL HDC and NULL clip rectangle",
        ));
    }
    if callback == 0 {
        return ApiHandlerResult::Fault(ApiFault::new("EnumDisplayMonitors requires a callback"));
    }
    if let Err(error) =
        runtime
            .memory
            .validate_access(GuestAddress(callback), 1, MemoryAccess::Execute)
    {
        return ApiHandlerResult::Fault(ApiFault::from_memory(error, endpoint));
    }
    let Some(monitor) = runtime.primary_monitor() else {
        return ApiHandlerResult::Fault(ApiFault::new(
            "Rust display subsystem has no logical monitor",
        ));
    };
    ApiHandlerResult::GuestCallback(Box::new(GuestCallbackRequest {
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
    }))
}

pub(crate) fn user32_argument_words(endpoint: &ApiEndpointIdentity) -> Option<u8> {
    if !endpoint.module.eq_ignore_ascii_case("user32.dll") {
        return None;
    }
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return None;
    };
    Some(match name.as_str() {
        "GetForegroundWindow" => 0,
        "SwapMouseButton" | "UpdateWindow" | "RegisterClassExA" | "GetSystemMetrics"
        | "GetCursorPos" | "SetCursor" | "GetCursorInfo" | "ShowCursor" | "TranslateMessage"
        | "DispatchMessageA" | "IsIconic" | "GetAsyncKeyState" | "SetFocus" | "GetDC"
        | "GetQueueStatus" => 1,
        "LoadIconA" | "SetWindowTextA" | "ShowWindow" | "BeginPaint" | "EndPaint"
        | "LoadCursorA" | "GetClientRect" | "GetWindowRect" | "SetCursorPos"
        | "MonitorFromWindow" | "GetMonitorInfoA" | "ReleaseDC" | "GetSystemMenu" => 2,
        "SetWindowLongA" | "FillRect" | "EnableMenuItem" => 3,
        "MessageBoxA" | "EnumDisplayMonitors" | "SendMessageA" | "SetTimer" | "DefWindowProcA" => 4,
        "PeekMessageA" | "SetRect" => 5,
        "LoadImageA" => 6,
        "SetWindowPos" => 7,
        "CreateWindowExA" => 12,
        _ => return None,
    })
}

fn required_argument(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    index: usize,
    label: &str,
) -> Result<u32, ApiFault> {
    frame.argument_u32(index).ok_or_else(|| {
        ApiFault::new(format!(
            "decoded {:?} frame is missing {label} argument {index}",
            endpoint.symbol
        ))
    })
}

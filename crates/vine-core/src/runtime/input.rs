use super::RuntimeStateError;

const VIRTUAL_KEY_COUNT: usize = 256;
const VIRTUAL_KEY_WORD_COUNT: usize = VIRTUAL_KEY_COUNT / u32::BITS as usize;

const VK_SHIFT: u16 = 0x10;
const VK_CONTROL: u16 = 0x11;
const VK_MENU: u16 = 0x12;
const VK_LBUTTON: u16 = 0x01;
const VK_RBUTTON: u16 = 0x02;
const VK_LSHIFT: u16 = 0xa0;
const VK_RSHIFT: u16 = 0xa1;
const VK_LCONTROL: u16 = 0xa2;
const VK_RCONTROL: u16 = 0xa3;
const VK_LMENU: u16 = 0xa4;
const VK_RMENU: u16 = 0xa5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeInputProfile {
    pub initial_mouse_buttons_swapped: bool,
    pub initial_keyboard_source_focused: Option<bool>,
    pub cursor_position: Option<RuntimeCursorPositionProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCursorPositionProfile {
    pub initial_screen_x: i32,
    pub initial_screen_y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKeyboardSourceEventKind {
    FocusGained,
    FocusLost,
    KeyDown,
    KeyUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeKeyboardSourceEvent {
    pub sequence: u64,
    pub kind: RuntimeKeyboardSourceEventKind,
    pub virtual_key: Option<u16>,
    pub repeat: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePointerButtonSourceEventKind {
    ButtonDown,
    ButtonUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePointerButton {
    Primary,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeLogicalMouseButton {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimePointerButtonSourceEvent {
    pub sequence: u64,
    pub kind: RuntimePointerButtonSourceEventKind,
    pub button: RuntimePointerButton,
    pub normalized_x: u16,
    pub normalized_y: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimePointerPositionSourceEvent {
    pub sequence: u64,
    pub normalized_x: u16,
    pub normalized_y: u16,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub struct RuntimeKeyboardState {
    pub source_focused: bool,
    pub last_event_sequence: u64,
    down_words: [u32; VIRTUAL_KEY_WORD_COUNT],
    pressed_words: [u32; VIRTUAL_KEY_WORD_COUNT],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCursorPosition {
    pub screen_x: i32,
    pub screen_y: i32,
}

#[derive(Debug, Default)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub(super) struct RuntimeInput {
    mouse_buttons_swapped: Option<bool>,
    keyboard: Option<RuntimeKeyboardState>,
    cursor_position: Option<RuntimeCursorPosition>,
    last_pointer_click: Option<RuntimePointerClickRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimePointerClickRecord {
    target_window_id: u64,
    target_window_handle: u32,
    logical_button: RuntimeLogicalMouseButton,
    screen_x: i32,
    screen_y: i32,
    tick_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimePointerButtonEventTarget {
    pub window_id: u64,
    pub window_handle: u32,
    pub accepts_double_clicks: bool,
    pub tick_count: u32,
}

pub(super) struct RuntimePointerButtonEventUpdate {
    next_cursor: RuntimeCursorPosition,
    next_last_pointer_click: Option<RuntimePointerClickRecord>,
    keyboard_event: RuntimeKeyboardSourceEvent,
    key_state: u32,
    event: RuntimePointerButtonSourceEvent,
    logical_button: RuntimeLogicalMouseButton,
    double_click: bool,
}

impl RuntimePointerButtonEventUpdate {
    pub(super) fn message_and_key_state(&self) -> (u32, u32) {
        let message = match (self.logical_button, self.event.kind, self.double_click) {
            (
                RuntimeLogicalMouseButton::Left,
                RuntimePointerButtonSourceEventKind::ButtonDown,
                true,
            ) => 0x0203,
            (
                RuntimeLogicalMouseButton::Left,
                RuntimePointerButtonSourceEventKind::ButtonDown,
                false,
            ) => 0x0201,
            (RuntimeLogicalMouseButton::Left, RuntimePointerButtonSourceEventKind::ButtonUp, _) => {
                0x0202
            }
            (
                RuntimeLogicalMouseButton::Right,
                RuntimePointerButtonSourceEventKind::ButtonDown,
                true,
            ) => 0x0206,
            (
                RuntimeLogicalMouseButton::Right,
                RuntimePointerButtonSourceEventKind::ButtonDown,
                false,
            ) => 0x0204,
            (
                RuntimeLogicalMouseButton::Right,
                RuntimePointerButtonSourceEventKind::ButtonUp,
                _,
            ) => 0x0205,
        };
        (message, self.key_state)
    }
}

impl RuntimeInput {
    pub(super) fn configure(
        &mut self,
        profile: RuntimeInputProfile,
    ) -> Result<(), RuntimeStateError> {
        if self.mouse_buttons_swapped.is_some()
            || self.keyboard.is_some()
            || self.cursor_position.is_some()
        {
            return Err(RuntimeStateError::new(
                "logical input profile may be configured only once",
            ));
        }
        let keyboard = profile
            .initial_keyboard_source_focused
            .map(RuntimeKeyboardState::new);
        let cursor_position = profile.cursor_position.map(|cursor| RuntimeCursorPosition {
            screen_x: cursor.initial_screen_x,
            screen_y: cursor.initial_screen_y,
        });
        self.mouse_buttons_swapped = Some(profile.initial_mouse_buttons_swapped);
        self.keyboard = keyboard;
        self.cursor_position = cursor_position;
        Ok(())
    }

    pub(super) fn swap_mouse_buttons(&mut self, swapped: bool) -> Result<bool, RuntimeStateError> {
        let mapping = self.mouse_buttons_swapped.as_mut().ok_or_else(|| {
            RuntimeStateError::new(
                "logical mouse button mapping must be configured before it is changed",
            )
        })?;
        Ok(std::mem::replace(mapping, swapped))
    }

    pub(super) fn keyboard_state(&self) -> Option<&RuntimeKeyboardState> {
        self.keyboard.as_ref()
    }

    pub(super) fn cursor_position(&self) -> Option<&RuntimeCursorPosition> {
        self.cursor_position.as_ref()
    }

    pub(super) fn set_cursor_position(
        &mut self,
        screen_x: i32,
        screen_y: i32,
    ) -> Result<(), RuntimeStateError> {
        let cursor = self.cursor_position.as_mut().ok_or_else(|| {
            RuntimeStateError::new(
                "logical cursor position must be configured before it is changed",
            )
        })?;
        cursor.screen_x = screen_x;
        cursor.screen_y = screen_y;
        Ok(())
    }

    fn validate_keyboard_event(
        &self,
        event: RuntimeKeyboardSourceEvent,
    ) -> Result<(), RuntimeStateError> {
        let keyboard = self.keyboard.as_ref().ok_or_else(|| {
            RuntimeStateError::new(
                "logical keyboard profile must be configured before events are ingested",
            )
        })?;
        let expected_sequence = keyboard.last_event_sequence.checked_add(1).ok_or_else(|| {
            RuntimeStateError::new("logical keyboard event sequence is exhausted")
        })?;
        if event.sequence != expected_sequence {
            return Err(RuntimeStateError::new(
                "logical keyboard event sequence is duplicate, stale, or out of order",
            ));
        }

        match event.kind {
            RuntimeKeyboardSourceEventKind::FocusGained => {
                validate_focus_event(event)?;
            }
            RuntimeKeyboardSourceEventKind::FocusLost => {
                validate_focus_event(event)?;
            }
            RuntimeKeyboardSourceEventKind::KeyDown | RuntimeKeyboardSourceEventKind::KeyUp => {
                if !keyboard.source_focused {
                    return Err(RuntimeStateError::new(
                        "logical keyboard events require an active browser input source",
                    ));
                }
                let virtual_key = event.virtual_key.ok_or_else(|| {
                    RuntimeStateError::new("logical key events require a virtual-key code")
                })?;
                if virtual_key == 0 || virtual_key as usize >= VIRTUAL_KEY_COUNT {
                    return Err(RuntimeStateError::new(
                        "logical key event virtual-key code is outside the configured range",
                    ));
                }
                if event.kind == RuntimeKeyboardSourceEventKind::KeyUp && event.repeat {
                    return Err(RuntimeStateError::new(
                        "logical key-up events cannot be repeat events",
                    ));
                }
                let was_down = keyboard.key_down(virtual_key);
                if event.kind == RuntimeKeyboardSourceEventKind::KeyDown
                    && event.repeat
                    && !was_down
                {
                    return Err(RuntimeStateError::new(
                        "logical repeat event requires an existing down state",
                    ));
                }
                if event.kind == RuntimeKeyboardSourceEventKind::KeyDown
                    && !event.repeat
                    && was_down
                {
                    return Err(RuntimeStateError::new(
                        "logical duplicate key-down must be marked as a repeat",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn publish_keyboard_event(&mut self, event: RuntimeKeyboardSourceEvent) {
        let keyboard = self.keyboard.as_mut().expect("validated keyboard event");
        match event.kind {
            RuntimeKeyboardSourceEventKind::FocusGained => keyboard.source_focused = true,
            RuntimeKeyboardSourceEventKind::FocusLost => {
                keyboard.source_focused = false;
                keyboard.down_words.fill(0);
                keyboard.pressed_words.fill(0);
            }
            RuntimeKeyboardSourceEventKind::KeyDown | RuntimeKeyboardSourceEventKind::KeyUp => {
                let virtual_key = event.virtual_key.expect("validated virtual-key code");
                let was_down = keyboard.key_down(virtual_key);
                let down = event.kind == RuntimeKeyboardSourceEventKind::KeyDown;
                keyboard.set_key_down(virtual_key, down);
                if down && !was_down {
                    keyboard.set_key_pressed(virtual_key, true);
                }
                if let Some((generic, left, right)) = modifier_relation(virtual_key) {
                    let generic_was_down = keyboard.key_down(generic);
                    let generic_down = keyboard.key_down(left) || keyboard.key_down(right);
                    keyboard.set_key_down(generic, generic_down);
                    if generic_down && !generic_was_down {
                        keyboard.set_key_pressed(generic, true);
                    }
                }
            }
        }
        keyboard.last_event_sequence = event.sequence;
    }

    pub(super) fn skip_unroutable_pointer_event(
        &mut self,
        sequence: u64,
    ) -> Result<(), RuntimeStateError> {
        let keyboard = self
            .keyboard
            .as_mut()
            .ok_or_else(|| RuntimeStateError::new("pointer input requires a keyboard state"))?;
        let expected = keyboard
            .last_event_sequence
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("input event sequence is exhausted"))?;
        if sequence != expected {
            return Err(RuntimeStateError::new(
                "unroutable pointer event is duplicate, stale, or out of order",
            ));
        }
        keyboard.last_event_sequence = sequence;
        Ok(())
    }

    pub(super) fn preview_pointer_button_event(
        &self,
        event: RuntimePointerButtonSourceEvent,
        screen_x: i32,
        screen_y: i32,
        target: RuntimePointerButtonEventTarget,
    ) -> Result<RuntimePointerButtonEventUpdate, RuntimeStateError> {
        let mapping = self.mouse_buttons_swapped.as_ref().ok_or_else(|| {
            RuntimeStateError::new("logical mouse button mapping must be configured before pointer events are ingested",
            )
        })?;
        self.cursor_position.as_ref().ok_or_else(|| {
            RuntimeStateError::new(
                "logical cursor position must be configured before pointer events are ingested",
            )
        })?;
        let logical_button =
            match (event.button, *mapping) {
                (RuntimePointerButton::Primary, false)
                | (RuntimePointerButton::Secondary, true) => RuntimeLogicalMouseButton::Left,
                (RuntimePointerButton::Secondary, false)
                | (RuntimePointerButton::Primary, true) => RuntimeLogicalMouseButton::Right,
            };
        let virtual_key = match logical_button {
            RuntimeLogicalMouseButton::Left => VK_LBUTTON,
            RuntimeLogicalMouseButton::Right => VK_RBUTTON,
        };
        let (double_click, next_last_pointer_click) = match event.kind {
            RuntimePointerButtonSourceEventKind::ButtonDown => {
                let double_click = target.accepts_double_clicks
                    && self.last_pointer_click.is_some_and(|previous| {
                        previous.target_window_id == target.window_id
                            && previous.target_window_handle == target.window_handle
                            && previous.logical_button == logical_button
                            && target.tick_count.wrapping_sub(previous.tick_count) <= 500
                            && previous.screen_x.abs_diff(screen_x).saturating_mul(2) <= 4
                            && previous.screen_y.abs_diff(screen_y).saturating_mul(2) <= 4
                    });
                let next = if target.accepts_double_clicks && !double_click {
                    Some(RuntimePointerClickRecord {
                        target_window_id: target.window_id,
                        target_window_handle: target.window_handle,
                        logical_button,
                        screen_x,
                        screen_y,
                        tick_count: target.tick_count,
                    })
                } else {
                    None
                };
                (double_click, next)
            }
            RuntimePointerButtonSourceEventKind::ButtonUp => (false, self.last_pointer_click),
        };
        let keyboard_event = RuntimeKeyboardSourceEvent {
            sequence: event.sequence,
            kind: match event.kind {
                RuntimePointerButtonSourceEventKind::ButtonDown => {
                    RuntimeKeyboardSourceEventKind::KeyDown
                }
                RuntimePointerButtonSourceEventKind::ButtonUp => {
                    RuntimeKeyboardSourceEventKind::KeyUp
                }
            },
            virtual_key: Some(virtual_key),
            repeat: false,
        };
        self.validate_keyboard_event(keyboard_event)?;
        let keyboard = self.keyboard.as_ref().expect("validated pointer keyboard");
        let mut key_state = mouse_message_key_state(keyboard);
        let button_mask = match logical_button {
            RuntimeLogicalMouseButton::Left => 0x0001,
            RuntimeLogicalMouseButton::Right => 0x0002,
        };
        if event.kind == RuntimePointerButtonSourceEventKind::ButtonDown {
            key_state |= button_mask;
        } else {
            key_state &= !button_mask;
        }
        Ok(RuntimePointerButtonEventUpdate {
            next_cursor: RuntimeCursorPosition { screen_x, screen_y },
            next_last_pointer_click,
            keyboard_event,
            key_state,
            event,
            logical_button,
            double_click,
        })
    }

    pub(super) fn pointer_position_key_state(
        &self,
        sequence: u64,
    ) -> Result<u32, RuntimeStateError> {
        let keyboard = self.keyboard.as_ref().ok_or_else(|| {
            RuntimeStateError::new(
                "logical keyboard profile must be configured before pointer positions are ingested",
            )
        })?;
        self.cursor_position.as_ref().ok_or_else(|| {
            RuntimeStateError::new(
                "logical cursor position must be configured before pointer positions are ingested",
            )
        })?;
        let expected_sequence = keyboard
            .last_event_sequence
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("logical input event sequence is exhausted"))?;
        if sequence != expected_sequence {
            return Err(RuntimeStateError::new(
                "logical pointer position sequence is duplicate, stale, or out of order",
            ));
        }
        Ok(mouse_message_key_state(keyboard))
    }

    pub(super) fn publish_pointer_position(&mut self, sequence: u64, screen_x: i32, screen_y: i32) {
        self.keyboard
            .as_mut()
            .expect("validated pointer keyboard")
            .last_event_sequence = sequence;
        self.cursor_position = Some(RuntimeCursorPosition { screen_x, screen_y });
    }

    pub(super) fn apply_pointer_button_event(&mut self, update: RuntimePointerButtonEventUpdate) {
        self.publish_keyboard_event(update.keyboard_event);
        self.cursor_position = Some(update.next_cursor);
        self.last_pointer_click = update.next_last_pointer_click;
    }

    #[cfg(test)]
    pub(super) fn ingest_keyboard_event(
        &mut self,
        event: RuntimeKeyboardSourceEvent,
    ) -> Result<(), RuntimeStateError> {
        self.validate_keyboard_event(event)?;
        self.publish_keyboard_event(event);
        Ok(())
    }

    pub(super) fn keyboard_message(
        &self,
        event: RuntimeKeyboardSourceEvent,
    ) -> Result<Option<(u32, u32, u32)>, RuntimeStateError> {
        self.validate_keyboard_event(event)?;
        if !matches!(
            event.kind,
            RuntimeKeyboardSourceEventKind::KeyDown | RuntimeKeyboardSourceEventKind::KeyUp
        ) {
            return Ok(None);
        }
        let key = event.virtual_key.expect("validated keyboard virtual key");
        let keyboard = self.keyboard.as_ref().expect("validated keyboard state");
        let up = event.kind == RuntimeKeyboardSourceEventKind::KeyUp;
        let alt = keyboard.key_down(VK_MENU) || matches!(key, VK_MENU | VK_LMENU | VK_RMENU);
        let system = alt || key == 0x79; // F10
        let message = match (system, up) {
            (false, false) => 0x0100,
            (false, true) => 0x0101,
            (true, false) => 0x0104,
            (true, true) => 0x0105,
        };
        let (scan_code, extended) = keyboard_scan_code(key);
        let l_param = 1
            | u32::from(scan_code) << 16
            | u32::from(extended) << 24
            | u32::from(alt) << 29
            | u32::from(up || keyboard.key_down(key)) << 30
            | u32::from(up) << 31;
        let message_key = modifier_relation(key).map_or(key, |(generic, _, _)| generic);
        Ok(Some((message, u32::from(message_key), l_param)))
    }

    pub(super) fn query_async_key_state(
        &mut self,
        virtual_key: u32,
    ) -> Result<Option<i16>, RuntimeStateError> {
        let keyboard = self.keyboard.as_mut().ok_or_else(|| {
            RuntimeStateError::new(
                "logical keyboard profile must be configured before it is queried",
            )
        })?;
        let Ok(virtual_key) = u16::try_from(virtual_key) else {
            return Ok(None);
        };
        if virtual_key as usize >= VIRTUAL_KEY_COUNT {
            return Ok(None);
        }
        let down = keyboard.key_down(virtual_key);
        let pressed_since_last_query = keyboard.key_pressed(virtual_key);
        if pressed_since_last_query {
            keyboard.set_key_pressed(virtual_key, false);
        }
        let value = ((u16::from(down) << 15) | u16::from(pressed_since_last_query)) as i16;
        Ok(Some(value))
    }
}

fn keyboard_scan_code(key: u16) -> (u8, bool) {
    let scan_code = match key {
        0x08 => 0x0e,
        0x09 => 0x0f,
        0x0d => 0x1c,
        0x13 => 0x45,
        0x14 => 0x3a,
        0x1b => 0x01,
        0x20 => 0x39,
        0x21 => 0x49,
        0x22 => 0x51,
        0x23 => 0x4f,
        0x24 => 0x47,
        0x25 => 0x4b,
        0x26 => 0x48,
        0x27 => 0x4d,
        0x28 => 0x50,
        0x2c => 0x37,
        0x2d => 0x52,
        0x2e => 0x53,
        0x30 => 0x0b,
        0x31..=0x39 => (key - 0x31 + 2) as u8,
        0x41 => 0x1e,
        0x42 => 0x30,
        0x43 => 0x2e,
        0x44 => 0x20,
        0x45 => 0x12,
        0x46 => 0x21,
        0x47 => 0x22,
        0x48 => 0x23,
        0x49 => 0x17,
        0x4a => 0x24,
        0x4b => 0x25,
        0x4c => 0x26,
        0x4d => 0x32,
        0x4e => 0x31,
        0x4f => 0x18,
        0x50 => 0x19,
        0x51 => 0x10,
        0x52 => 0x13,
        0x53 => 0x1f,
        0x54 => 0x14,
        0x55 => 0x16,
        0x56 => 0x2f,
        0x57 => 0x11,
        0x58 => 0x2d,
        0x59 => 0x15,
        0x5a => 0x2c,
        0x5b => 0x5b,
        0x5c => 0x5c,
        0x5d => 0x5d,
        0x60 => 0x52,
        0x61 => 0x4f,
        0x62 => 0x50,
        0x63 => 0x51,
        0x64 => 0x4b,
        0x65 => 0x4c,
        0x66 => 0x4d,
        0x67 => 0x47,
        0x68 => 0x48,
        0x69 => 0x49,
        0x6a => 0x37,
        0x6b => 0x4e,
        0x6c => 0x7e,
        0x6d => 0x4a,
        0x6e => 0x53,
        0x6f => 0x35,
        0x70..=0x79 => (key - 0x70 + 0x3b) as u8,
        0x7a => 0x57,
        0x7b => 0x58,
        0x7c..=0x87 => (key - 0x7c + 0x64) as u8,
        0x90 => 0x45,
        0x91 => 0x46,
        VK_SHIFT | VK_LSHIFT => 0x2a,
        VK_RSHIFT => 0x36,
        VK_CONTROL | VK_LCONTROL | VK_RCONTROL => 0x1d,
        VK_MENU | VK_LMENU | VK_RMENU => 0x38,
        0xba => 0x27,
        0xbb => 0x0d,
        0xbc => 0x33,
        0xbd => 0x0c,
        0xbe => 0x34,
        0xbf => 0x35,
        0xc0 => 0x29,
        0xdb => 0x1a,
        0xdc => 0x2b,
        0xdd => 0x1b,
        0xde => 0x28,
        0xe2 => 0x56,
        _ => 0,
    };
    let extended =
        matches!(key, 0x21..=0x28 | 0x2c..=0x2e | 0x5b..=0x5d | 0x6f | VK_RCONTROL | VK_RMENU);
    (scan_code, extended)
}

impl RuntimeKeyboardState {
    fn new(initial_source_focused: bool) -> Self {
        Self {
            source_focused: initial_source_focused,
            last_event_sequence: 0,
            down_words: [0; VIRTUAL_KEY_WORD_COUNT],
            pressed_words: [0; VIRTUAL_KEY_WORD_COUNT],
        }
    }

    fn key_down(&self, virtual_key: u16) -> bool {
        bit_is_set(&self.down_words, virtual_key)
    }

    fn key_pressed(&self, virtual_key: u16) -> bool {
        bit_is_set(&self.pressed_words, virtual_key)
    }

    fn set_key_down(&mut self, virtual_key: u16, value: bool) {
        set_bit(&mut self.down_words, virtual_key, value);
    }

    fn set_key_pressed(&mut self, virtual_key: u16, value: bool) {
        set_bit(&mut self.pressed_words, virtual_key, value);
    }
}

fn mouse_message_key_state(keyboard: &RuntimeKeyboardState) -> u32 {
    let mut key_state = 0;
    for (virtual_key, mask) in [
        (VK_LBUTTON, 0x0001),
        (VK_RBUTTON, 0x0002),
        (VK_SHIFT, 0x0004),
        (VK_CONTROL, 0x0008),
    ] {
        if keyboard.key_down(virtual_key) {
            key_state |= mask;
        }
    }
    key_state
}

fn validate_focus_event(event: RuntimeKeyboardSourceEvent) -> Result<(), RuntimeStateError> {
    if event.virtual_key.is_some() || event.repeat {
        return Err(RuntimeStateError::new(
            "logical keyboard focus events cannot carry a virtual key or repeat flag",
        ));
    }
    Ok(())
}

fn modifier_relation(virtual_key: u16) -> Option<(u16, u16, u16)> {
    match virtual_key {
        VK_LSHIFT | VK_RSHIFT => Some((VK_SHIFT, VK_LSHIFT, VK_RSHIFT)),
        VK_LCONTROL | VK_RCONTROL => Some((VK_CONTROL, VK_LCONTROL, VK_RCONTROL)),
        VK_LMENU | VK_RMENU => Some((VK_MENU, VK_LMENU, VK_RMENU)),
        _ => None,
    }
}

fn bit_is_set(words: &[u32; VIRTUAL_KEY_WORD_COUNT], virtual_key: u16) -> bool {
    let index = usize::from(virtual_key);
    words[index / u32::BITS as usize] & (1_u32 << (index % u32::BITS as usize)) != 0
}

fn set_bit(words: &mut [u32; VIRTUAL_KEY_WORD_COUNT], virtual_key: u16, value: bool) {
    let index = usize::from(virtual_key);
    let word = &mut words[index / u32::BITS as usize];
    let mask = 1_u32 << (index % u32::BITS as usize);
    if value {
        *word |= mask;
    } else {
        *word &= !mask;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(initial_mouse_buttons_swapped: bool) -> RuntimeInputProfile {
        RuntimeInputProfile {
            initial_mouse_buttons_swapped,
            initial_keyboard_source_focused: Some(true),
            cursor_position: Some(RuntimeCursorPositionProfile {
                initial_screen_x: 960,
                initial_screen_y: 540,
            }),
        }
    }

    #[test]
    fn configuration_and_swaps_track_changes() {
        let mut input = RuntimeInput::default();
        input.configure(profile(false)).unwrap();
        assert_eq!(
            input.cursor_position(),
            Some(&RuntimeCursorPosition {
                screen_x: 960,
                screen_y: 540,
            })
        );
        input.configure(profile(false)).unwrap_err();

        assert!(!input.swap_mouse_buttons(false).unwrap());
        assert_eq!(input.mouse_buttons_swapped, Some(false));

        assert!(!input.swap_mouse_buttons(true).unwrap());
        assert_eq!(input.mouse_buttons_swapped, Some(true));
        assert!(input.swap_mouse_buttons(true).unwrap());
        assert_eq!(input.mouse_buttons_swapped, Some(true));
    }

    #[test]
    fn cursor_position_updates_preserve_coordinates_and_noops() {
        let mut input = RuntimeInput::default();
        input.configure(profile(false)).unwrap();

        input.set_cursor_position(960, 540).unwrap();
        assert_eq!(
            input.cursor_position(),
            Some(&RuntimeCursorPosition {
                screen_x: 960,
                screen_y: 540
            })
        );

        input.set_cursor_position(566, 374).unwrap();
        let moved = RuntimeCursorPosition {
            screen_x: 566,
            screen_y: 374,
        };
        assert_eq!(input.cursor_position(), Some(&moved));
        input.set_cursor_position(566, 374).unwrap();
        assert_eq!(input.cursor_position(), Some(&moved));

        let mut unconfigured = RuntimeInput::default();
        assert!(unconfigured.set_cursor_position(0, 0).is_err());
        assert!(unconfigured.cursor_position().is_none());
    }

    fn key_event(
        sequence: u64,
        kind: RuntimeKeyboardSourceEventKind,
        virtual_key: u16,
        repeat: bool,
    ) -> RuntimeKeyboardSourceEvent {
        RuntimeKeyboardSourceEvent {
            sequence,
            kind,
            virtual_key: Some(virtual_key),
            repeat,
        }
    }

    fn focus_event(
        sequence: u64,
        kind: RuntimeKeyboardSourceEventKind,
    ) -> RuntimeKeyboardSourceEvent {
        RuntimeKeyboardSourceEvent {
            sequence,
            kind,
            virtual_key: None,
            repeat: false,
        }
    }

    fn pointer_event(
        sequence: u64,
        kind: RuntimePointerButtonSourceEventKind,
    ) -> RuntimePointerButtonSourceEvent {
        RuntimePointerButtonSourceEvent {
            sequence,
            kind,
            button: RuntimePointerButton::Primary,
            normalized_x: 32_768,
            normalized_y: 32_768,
        }
    }

    fn pointer_target(
        accepts_double_clicks: bool,
        tick_count: u32,
    ) -> RuntimePointerButtonEventTarget {
        RuntimePointerButtonEventTarget {
            window_id: 7,
            window_handle: 0x4801_0001,
            accepts_double_clicks,
            tick_count,
        }
    }

    #[test]
    fn pointer_events_share_mapping_keyboard_and_cursor_authority() {
        let mut input = RuntimeInput::default();
        input.configure(profile(false)).unwrap();

        let down = input
            .preview_pointer_button_event(
                pointer_event(1, RuntimePointerButtonSourceEventKind::ButtonDown),
                1_600,
                900,
                pointer_target(false, 100),
            )
            .unwrap();
        assert_eq!(input.keyboard_state().unwrap().last_event_sequence, 0);
        assert!(!input.keyboard_state().unwrap().key_down(VK_LBUTTON));
        let cursor = input.cursor_position().unwrap();
        assert_eq!((cursor.screen_x, cursor.screen_y), (960, 540));
        assert!(input.last_pointer_click.is_none());
        assert_eq!(down.message_and_key_state(), (0x0201, 0x0001));
        input.apply_pointer_button_event(down);
        let cursor = input.cursor_position.as_ref().unwrap();
        assert_eq!((cursor.screen_x, cursor.screen_y), (1_600, 900));
        assert!(input.keyboard_state().unwrap().key_down(VK_LBUTTON));

        let up = input
            .preview_pointer_button_event(
                pointer_event(2, RuntimePointerButtonSourceEventKind::ButtonUp),
                1_600,
                900,
                pointer_target(false, 110),
            )
            .unwrap();
        assert_eq!(up.message_and_key_state(), (0x0202, 0));
        input.apply_pointer_button_event(up);
        assert!(!input.keyboard_state().unwrap().key_down(VK_LBUTTON));
        let click_before = input.last_pointer_click;
        assert!(
            input
                .preview_pointer_button_event(
                    pointer_event(2, RuntimePointerButtonSourceEventKind::ButtonDown),
                    1_600,
                    900,
                    pointer_target(false, 120),
                )
                .is_err()
        );
        assert_eq!(input.keyboard_state().unwrap().last_event_sequence, 2);
        assert!(!input.keyboard_state().unwrap().key_down(VK_LBUTTON));
        let cursor = input.cursor_position().unwrap();
        assert_eq!((cursor.screen_x, cursor.screen_y), (1_600, 900));
        assert_eq!(input.last_pointer_click, click_before);

        let mut swapped = RuntimeInput::default();
        swapped.configure(profile(true)).unwrap();
        let right = swapped
            .preview_pointer_button_event(
                pointer_event(1, RuntimePointerButtonSourceEventKind::ButtonDown),
                960,
                540,
                pointer_target(false, 100),
            )
            .unwrap();
        assert_eq!(right.message_and_key_state(), (0x0204, 0x0002));
        swapped.apply_pointer_button_event(right);
        assert!(swapped.keyboard_state().unwrap().key_down(VK_RBUTTON));
        assert!(!swapped.keyboard_state().unwrap().key_down(VK_LBUTTON));
    }

    #[test]
    fn pointer_double_click_requires_matching_class_window_time_and_position() {
        let mut input = RuntimeInput::default();
        input.configure(profile(false)).unwrap();

        let first_down = input
            .preview_pointer_button_event(
                pointer_event(1, RuntimePointerButtonSourceEventKind::ButtonDown),
                640,
                810,
                pointer_target(true, 100),
            )
            .unwrap();
        assert_eq!(first_down.message_and_key_state(), (0x0201, 0x0001));
        input.apply_pointer_button_event(first_down);
        let first_up = input
            .preview_pointer_button_event(
                pointer_event(2, RuntimePointerButtonSourceEventKind::ButtonUp),
                640,
                810,
                pointer_target(true, 110),
            )
            .unwrap();
        input.apply_pointer_button_event(first_up);

        let click_before = input.last_pointer_click;
        let second_down = input
            .preview_pointer_button_event(
                pointer_event(3, RuntimePointerButtonSourceEventKind::ButtonDown),
                642,
                812,
                pointer_target(true, 600),
            )
            .unwrap();
        assert_eq!(input.last_pointer_click, click_before);
        assert_eq!(input.keyboard_state().unwrap().last_event_sequence, 2);
        assert!(!input.keyboard_state().unwrap().key_down(VK_LBUTTON));
        let cursor = input.cursor_position().unwrap();
        assert_eq!((cursor.screen_x, cursor.screen_y), (640, 810));
        assert_eq!(second_down.message_and_key_state(), (0x0203, 0x0001));

        for (screen_x, screen_y, target) in [
            (640, 810, pointer_target(false, 200)),
            (640, 810, pointer_target(true, 601)),
            (643, 810, pointer_target(true, 200)),
            (
                640,
                810,
                RuntimePointerButtonEventTarget {
                    window_id: 8,
                    ..pointer_target(true, 200)
                },
            ),
        ] {
            let ordinary_down = input
                .preview_pointer_button_event(
                    pointer_event(3, RuntimePointerButtonSourceEventKind::ButtonDown),
                    screen_x,
                    screen_y,
                    target,
                )
                .unwrap();
            assert_eq!(ordinary_down.message_and_key_state(), (0x0201, 0x0001));
        }

        input.apply_pointer_button_event(second_down);
        let second_up = input
            .preview_pointer_button_event(
                pointer_event(4, RuntimePointerButtonSourceEventKind::ButtonUp),
                642,
                812,
                pointer_target(true, 610),
            )
            .unwrap();
        input.apply_pointer_button_event(second_up);
        let third_down = input
            .preview_pointer_button_event(
                pointer_event(5, RuntimePointerButtonSourceEventKind::ButtonDown),
                642,
                812,
                pointer_target(true, 620),
            )
            .unwrap();
        assert_eq!(
            third_down.message_and_key_state(),
            (0x0201, 0x0001),
            "a completed double-click must not overlap the next click sequence"
        );
    }

    #[test]
    fn unroutable_pointer_input_advances_shared_sequence_without_keys_or_cursor() {
        let mut input = RuntimeInput::default();
        input.configure(profile(false)).unwrap();
        let cursor = input.cursor_position().cloned();
        input.skip_unroutable_pointer_event(1).unwrap();
        assert_eq!(input.cursor_position(), cursor.as_ref());
        assert_eq!(input.keyboard_state().unwrap().last_event_sequence, 1);
        assert!(!input.keyboard_state().unwrap().key_down(VK_LBUTTON));
        assert!(input.skip_unroutable_pointer_event(1).is_err());
        assert_eq!(input.cursor_position(), cursor.as_ref());
        assert_eq!(input.keyboard_state().unwrap().last_event_sequence, 1);
        assert!(!input.keyboard_state().unwrap().key_down(VK_LBUTTON));
        assert!(input.last_pointer_click.is_none());
        input
            .ingest_keyboard_event(RuntimeKeyboardSourceEvent {
                sequence: 2,
                kind: RuntimeKeyboardSourceEventKind::KeyDown,
                virtual_key: Some(0x41),
                repeat: false,
            })
            .expect("keyboard resumes after an ignored pointer event");
    }

    #[test]
    fn pointer_positions_share_input_sequence_without_changing_button_state() {
        let mut input = RuntimeInput::default();
        input.configure(profile(false)).unwrap();

        assert!(input.pointer_position_key_state(0).is_err());
        assert!(input.pointer_position_key_state(2).is_err());
        let key_state = input.pointer_position_key_state(1).unwrap();
        assert_eq!(input.keyboard_state().unwrap().last_event_sequence, 0);
        assert!(!input.keyboard_state().unwrap().key_down(VK_LBUTTON));
        let cursor = input.cursor_position().unwrap();
        assert_eq!((cursor.screen_x, cursor.screen_y), (960, 540));
        assert!(input.last_pointer_click.is_none());
        assert_eq!(key_state, 0);
        input.publish_pointer_position(1, 640, 810);
        let cursor = input.cursor_position.as_ref().unwrap();
        assert_eq!((cursor.screen_x, cursor.screen_y), (640, 810));
        assert_eq!(input.keyboard_state().unwrap().last_event_sequence, 1);
        assert!(!input.keyboard_state().unwrap().key_down(VK_LBUTTON));

        let down = input
            .preview_pointer_button_event(
                pointer_event(2, RuntimePointerButtonSourceEventKind::ButtonDown),
                640,
                810,
                pointer_target(false, 100),
            )
            .unwrap();
        assert_eq!(down.message_and_key_state(), (0x0201, 0x0001));
        input.apply_pointer_button_event(down);
        assert_eq!(input.keyboard_state().unwrap().last_event_sequence, 2);
        assert!(input.keyboard_state().unwrap().key_down(VK_LBUTTON));
        assert!(input.pointer_position_key_state(2).is_err());
        assert_eq!(input.pointer_position_key_state(3).unwrap(), 0x0001);
        input.publish_pointer_position(3, 800, 900);
        assert_eq!(input.keyboard_state().unwrap().last_event_sequence, 3);
        assert_eq!(
            input.query_async_key_state(VK_LBUTTON as u32).unwrap(),
            Some(0x8001_u16 as i16)
        );
        assert_eq!(
            input.query_async_key_state(VK_LBUTTON as u32).unwrap(),
            Some(0x8000_u16 as i16)
        );
    }

    #[test]
    fn completed_pointer_click_retains_pressed_since_last_query_once() {
        let mut input = RuntimeInput::default();
        input.configure(profile(false)).unwrap();

        let down = input
            .preview_pointer_button_event(
                pointer_event(1, RuntimePointerButtonSourceEventKind::ButtonDown),
                640,
                810,
                pointer_target(false, 100),
            )
            .unwrap();
        input.apply_pointer_button_event(down);
        let up = input
            .preview_pointer_button_event(
                pointer_event(2, RuntimePointerButtonSourceEventKind::ButtonUp),
                640,
                810,
                pointer_target(false, 110),
            )
            .unwrap();
        input.apply_pointer_button_event(up);

        let clicked = input
            .query_async_key_state(u32::from(VK_LBUTTON))
            .unwrap()
            .unwrap();
        assert_eq!(clicked, 0x0001);

        assert_eq!(
            input
                .query_async_key_state(u32::from(VK_LBUTTON))
                .unwrap()
                .unwrap(),
            0
        );
    }

    #[test]
    fn modifier_events_aggregate_and_queries_consume_only_the_requested_pressed_bit() {
        let mut input = RuntimeInput::default();
        input.configure(profile(false)).unwrap();

        input
            .ingest_keyboard_event(key_event(
                1,
                RuntimeKeyboardSourceEventKind::KeyDown,
                VK_LCONTROL,
                false,
            ))
            .unwrap();
        let generic = input
            .query_async_key_state(u32::from(VK_CONTROL))
            .unwrap()
            .unwrap();
        assert_eq!(generic, 0x8001_u16 as i16);
        let generic_repeat = input
            .query_async_key_state(u32::from(VK_CONTROL))
            .unwrap()
            .unwrap();
        assert_eq!(generic_repeat, 0x8000_u16 as i16);

        let side = input
            .query_async_key_state(u32::from(VK_LCONTROL))
            .unwrap()
            .unwrap();
        assert_eq!(side, 0x8001_u16 as i16);

        input
            .ingest_keyboard_event(key_event(
                2,
                RuntimeKeyboardSourceEventKind::KeyDown,
                VK_LCONTROL,
                true,
            ))
            .unwrap();
        assert_eq!(
            input
                .query_async_key_state(u32::from(VK_CONTROL))
                .unwrap()
                .unwrap(),
            0x8000_u16 as i16
        );

        input
            .ingest_keyboard_event(key_event(
                3,
                RuntimeKeyboardSourceEventKind::KeyUp,
                VK_LCONTROL,
                false,
            ))
            .unwrap();
        assert_eq!(
            input
                .query_async_key_state(u32::from(VK_CONTROL))
                .unwrap()
                .unwrap(),
            0
        );
    }

    #[test]
    fn keyboard_sequence_rejection_and_focus_loss_preserve_key_state() {
        let mut input = RuntimeInput::default();
        input.configure(profile(false)).unwrap();

        input
            .ingest_keyboard_event(key_event(
                1,
                RuntimeKeyboardSourceEventKind::KeyDown,
                b'A'.into(),
                false,
            ))
            .unwrap();
        assert!(
            input
                .ingest_keyboard_event(key_event(
                    1,
                    RuntimeKeyboardSourceEventKind::KeyDown,
                    b'A'.into(),
                    false,
                ))
                .is_err()
        );
        let keyboard = input.keyboard_state().unwrap();
        assert_eq!(keyboard.last_event_sequence, 1);
        assert!(keyboard.source_focused);
        assert!(keyboard.key_down(u16::from(b'A')));

        for event in [
            key_event(
                2,
                RuntimeKeyboardSourceEventKind::KeyDown,
                b'A'.into(),
                false,
            ),
            key_event(
                2,
                RuntimeKeyboardSourceEventKind::KeyDown,
                b'B'.into(),
                true,
            ),
            key_event(2, RuntimeKeyboardSourceEventKind::KeyUp, b'A'.into(), true),
            key_event(2, RuntimeKeyboardSourceEventKind::KeyDown, 0, false),
            RuntimeKeyboardSourceEvent {
                sequence: 2,
                kind: RuntimeKeyboardSourceEventKind::FocusLost,
                virtual_key: None,
                repeat: true,
            },
        ] {
            assert!(input.ingest_keyboard_event(event).is_err());
            let keyboard = input.keyboard_state().unwrap();
            assert_eq!(keyboard.last_event_sequence, 1);
            assert!(keyboard.source_focused);
            assert!(keyboard.key_down(u16::from(b'A')));
            assert!(!keyboard.key_down(u16::from(b'B')));
        }

        assert_eq!(
            input
                .query_async_key_state(u32::from(b'A'))
                .unwrap()
                .unwrap(),
            0x8001_u16 as i16
        );
        assert_eq!(
            input
                .query_async_key_state(u32::from(b'A'))
                .unwrap()
                .unwrap(),
            0x8000_u16 as i16
        );

        input
            .ingest_keyboard_event(focus_event(2, RuntimeKeyboardSourceEventKind::FocusLost))
            .unwrap();
        assert!(!input.keyboard_state().unwrap().source_focused);
        assert!(!input.keyboard_state().unwrap().key_down(u16::from(b'A')));
        assert!(
            input
                .ingest_keyboard_event(key_event(
                    3,
                    RuntimeKeyboardSourceEventKind::KeyDown,
                    b'B'.into(),
                    false,
                ))
                .is_err()
        );
        let keyboard = input.keyboard_state().unwrap();
        assert_eq!(keyboard.last_event_sequence, 2);
        assert!(!keyboard.source_focused);
        assert!(!keyboard.key_down(u16::from(b'A')));
        assert!(!keyboard.key_down(u16::from(b'B')));
        assert_eq!(
            input.query_async_key_state(u32::from(b'A')).unwrap(),
            Some(0)
        );
        assert_eq!(
            input.query_async_key_state(u32::from(b'B')).unwrap(),
            Some(0)
        );
        input
            .ingest_keyboard_event(focus_event(3, RuntimeKeyboardSourceEventKind::FocusGained))
            .unwrap();

        assert!(input.query_async_key_state(0xff).unwrap().is_some());
        assert!(input.query_async_key_state(0x100).unwrap().is_none());
    }

    #[test]
    fn missing_input_state_does_not_mutate() {
        let mut input = RuntimeInput::default();
        assert!(input.swap_mouse_buttons(true).is_err());
        assert!(input.mouse_buttons_swapped.is_none());
        assert!(input.keyboard_state().is_none());
        assert!(input.cursor_position.is_none());
        assert!(input.last_pointer_click.is_none());
        input.configure(profile(false)).unwrap();
        assert!(!input.swap_mouse_buttons(true).unwrap());
    }
}

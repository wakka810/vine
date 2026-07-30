use serde::Serialize;

use super::RuntimeStateError;

const MAX_INPUT_PROFILE_ID_BYTES: usize = 128;
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInputProfile {
    pub id: String,
    pub initial_mouse_buttons_swapped: bool,
    pub keyboard: Option<RuntimeKeyboardProfile>,
    pub cursor_position: Option<RuntimeCursorPositionProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCursorPositionProfile {
    pub id: String,
    pub initial_screen_x: i32,
    pub initial_screen_y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeKeyboardProfile {
    pub id: String,
    pub initial_source_focused: bool,
    pub maximum_virtual_key: u16,
    pub aggregate_modifier_keys: bool,
    pub clear_on_source_focus_loss: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeKeyboardSourceEventKind {
    FocusGained,
    FocusLost,
    KeyDown,
    KeyUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeKeyboardSourceEvent {
    pub sequence: u64,
    pub kind: RuntimeKeyboardSourceEventKind,
    pub virtual_key: Option<u16>,
    pub repeat: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimePointerButtonSourceEventKind {
    ButtonDown,
    ButtonUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimePointerButton {
    Primary,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeLogicalMouseButton {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePointerButtonSourceEvent {
    pub sequence: u64,
    pub kind: RuntimePointerButtonSourceEventKind,
    pub button: RuntimePointerButton,
    pub normalized_x: u16,
    pub normalized_y: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePointerButtonMessageProfile {
    pub id: String,
    pub normalized_position_denominator: u32,
    pub left_button_down_message: u32,
    pub left_button_up_message: u32,
    pub right_button_down_message: u32,
    pub right_button_up_message: u32,
    pub left_button_key_mask: u32,
    pub right_button_key_mask: u32,
    pub shift_key_mask: u32,
    pub control_key_mask: u32,
    pub require_visible_active_focused_foreground_window: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeVirtualKeyState {
    pub virtual_key: u16,
    pub down: bool,
    pub pressed_since_last_query: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeKeyboardState {
    pub profile_id: String,
    pub source_focused: bool,
    pub maximum_virtual_key: u16,
    pub aggregate_modifier_keys: bool,
    pub clear_on_source_focus_loss: bool,
    pub generation: u64,
    pub last_event_sequence: u64,
    down_words: [u32; VIRTUAL_KEY_WORD_COUNT],
    pressed_words: [u32; VIRTUAL_KEY_WORD_COUNT],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeKeyboardEventReceipt {
    pub sequence: u64,
    pub kind: RuntimeKeyboardSourceEventKind,
    pub virtual_key: Option<u16>,
    pub repeat: bool,
    pub source_focused: bool,
    pub generation: u64,
    pub affected_keys: Vec<RuntimeVirtualKeyState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePointerButtonEventReceipt {
    pub event: RuntimePointerButtonSourceEvent,
    pub logical_button: RuntimeLogicalMouseButton,
    pub virtual_key: u16,
    pub keyboard: RuntimeKeyboardEventReceipt,
    pub cursor: RuntimeCursorPosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAsyncKeyStateResult {
    pub virtual_key: u16,
    pub down: bool,
    pub pressed_since_last_query: bool,
    pub value: i16,
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeMouseButtonMapping {
    pub profile_id: String,
    pub initial_swapped: bool,
    pub swapped: bool,
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCursorPosition {
    pub profile_id: String,
    pub screen_x: i32,
    pub screen_y: i32,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSwapMouseButtonResult {
    pub previous_swapped: bool,
    pub current_swapped: bool,
    pub generation: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    mouse_button_mapping: Option<RuntimeMouseButtonMapping>,
    #[serde(skip_serializing_if = "Option::is_none")]
    keyboard: Option<RuntimeKeyboardState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor_position: Option<RuntimeCursorPosition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeInputConfigurationPlan {
    mapping: RuntimeMouseButtonMapping,
    keyboard: Option<RuntimeKeyboardState>,
    cursor_position: Option<RuntimeCursorPosition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeMouseButtonSwapPlan {
    expected_generation: u64,
    expected_swapped: bool,
    next_swapped: bool,
    next_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeKeyboardEventPlan {
    expected_generation: u64,
    expected_last_event_sequence: u64,
    next_state: RuntimeKeyboardState,
    event: RuntimeKeyboardSourceEvent,
    affected_virtual_keys: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimePointerButtonEventPlan {
    expected_mapping_generation: u64,
    expected_mapping_swapped: bool,
    expected_cursor_generation: u64,
    expected_cursor_x: i32,
    expected_cursor_y: i32,
    next_cursor: RuntimeCursorPosition,
    keyboard: RuntimeKeyboardEventPlan,
    event: RuntimePointerButtonSourceEvent,
    logical_button: RuntimeLogicalMouseButton,
    virtual_key: u16,
}

impl RuntimePointerButtonEventPlan {
    pub(super) fn message_and_key_state(
        &self,
        profile: &RuntimePointerButtonMessageProfile,
    ) -> (u32, u32) {
        let message = match (self.logical_button, self.event.kind) {
            (RuntimeLogicalMouseButton::Left, RuntimePointerButtonSourceEventKind::ButtonDown) => {
                profile.left_button_down_message
            }
            (RuntimeLogicalMouseButton::Left, RuntimePointerButtonSourceEventKind::ButtonUp) => {
                profile.left_button_up_message
            }
            (RuntimeLogicalMouseButton::Right, RuntimePointerButtonSourceEventKind::ButtonDown) => {
                profile.right_button_down_message
            }
            (RuntimeLogicalMouseButton::Right, RuntimePointerButtonSourceEventKind::ButtonUp) => {
                profile.right_button_up_message
            }
        };
        let mut key_state = 0;
        for (virtual_key, mask) in [
            (VK_LBUTTON, profile.left_button_key_mask),
            (VK_RBUTTON, profile.right_button_key_mask),
            (VK_SHIFT, profile.shift_key_mask),
            (VK_CONTROL, profile.control_key_mask),
        ] {
            if self.keyboard.next_state.key_down(virtual_key) {
                key_state |= mask;
            }
        }
        (message, key_state)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeAsyncKeyStateQueryPlan {
    expected_generation: u64,
    virtual_key: u16,
    expected_down: bool,
    expected_pressed: bool,
    next_generation: u64,
}

impl RuntimeInput {
    pub(super) fn plan_configuration(
        &self,
        profile: RuntimeInputProfile,
    ) -> Result<RuntimeInputConfigurationPlan, RuntimeStateError> {
        if self.mouse_button_mapping.is_some()
            || self.keyboard.is_some()
            || self.cursor_position.is_some()
        {
            return Err(RuntimeStateError::conflict(
                "runtime-input",
                "input-profile-already-configured",
                "logical input profile may be configured only once",
            ));
        }
        if profile.id.is_empty()
            || profile.id.len() > MAX_INPUT_PROFILE_ID_BYTES
            || !profile.id.is_ascii()
            || profile.id.as_bytes().contains(&0)
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-input",
                "input-profile-id-invalid",
                "logical input profile ID must be bounded nonempty ASCII",
            ));
        }
        let keyboard = profile
            .keyboard
            .map(RuntimeKeyboardState::from_profile)
            .transpose()?;
        let cursor_position = profile
            .cursor_position
            .map(|cursor| {
                validate_profile_id(&cursor.id, "cursor-position-profile-id-invalid")?;
                Ok(RuntimeCursorPosition {
                    profile_id: cursor.id,
                    screen_x: cursor.initial_screen_x,
                    screen_y: cursor.initial_screen_y,
                    generation: 0,
                })
            })
            .transpose()?;
        Ok(RuntimeInputConfigurationPlan {
            mapping: RuntimeMouseButtonMapping {
                profile_id: profile.id,
                initial_swapped: profile.initial_mouse_buttons_swapped,
                swapped: profile.initial_mouse_buttons_swapped,
                generation: 0,
            },
            keyboard,
            cursor_position,
        })
    }

    pub(super) fn commit_configuration(
        &mut self,
        plan: RuntimeInputConfigurationPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.mouse_button_mapping.is_some()
            || self.keyboard.is_some()
            || self.cursor_position.is_some()
        {
            return Err(RuntimeStateError::conflict(
                "runtime-input",
                "input-configuration-plan-stale",
                "logical input configuration plan no longer targets an empty owner",
            ));
        }
        self.mouse_button_mapping = Some(plan.mapping);
        self.keyboard = plan.keyboard;
        self.cursor_position = plan.cursor_position;
        Ok(())
    }

    pub(super) fn plan_mouse_button_swap(
        &self,
        swapped: bool,
    ) -> Result<RuntimeMouseButtonSwapPlan, RuntimeStateError> {
        let mapping = self.mouse_button_mapping.as_ref().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-input",
                "mouse-button-mapping-unconfigured",
                "logical mouse button mapping must be configured before it is changed",
            )
        })?;
        let next_generation = if mapping.swapped == swapped {
            mapping.generation
        } else {
            mapping.generation.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-input",
                    "mouse-button-mapping-generation-overflow",
                    "logical mouse button mapping generation is exhausted",
                )
            })?
        };
        Ok(RuntimeMouseButtonSwapPlan {
            expected_generation: mapping.generation,
            expected_swapped: mapping.swapped,
            next_swapped: swapped,
            next_generation,
        })
    }

    pub(super) fn commit_mouse_button_swap(
        &mut self,
        plan: RuntimeMouseButtonSwapPlan,
    ) -> Result<RuntimeSwapMouseButtonResult, RuntimeStateError> {
        let mapping = self.mouse_button_mapping.as_mut().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-input",
                "mouse-button-mapping-unconfigured",
                "logical mouse button mapping disappeared before commit",
            )
        })?;
        if mapping.generation != plan.expected_generation
            || mapping.swapped != plan.expected_swapped
        {
            return Err(RuntimeStateError::conflict(
                "runtime-input",
                "mouse-button-swap-plan-stale",
                "logical mouse button mapping changed after the swap was planned",
            ));
        }
        mapping.swapped = plan.next_swapped;
        mapping.generation = plan.next_generation;
        Ok(RuntimeSwapMouseButtonResult {
            previous_swapped: plan.expected_swapped,
            current_swapped: mapping.swapped,
            generation: mapping.generation,
        })
    }

    pub(super) fn mouse_button_mapping(&self) -> Option<&RuntimeMouseButtonMapping> {
        self.mouse_button_mapping.as_ref()
    }

    pub(super) fn keyboard_state(&self) -> Option<&RuntimeKeyboardState> {
        self.keyboard.as_ref()
    }

    pub(super) fn cursor_position(&self) -> Option<&RuntimeCursorPosition> {
        self.cursor_position.as_ref()
    }

    pub(super) fn plan_keyboard_event(
        &self,
        event: RuntimeKeyboardSourceEvent,
    ) -> Result<RuntimeKeyboardEventPlan, RuntimeStateError> {
        let keyboard = self.keyboard.as_ref().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-input",
                "keyboard-profile-unconfigured",
                "logical keyboard profile must be configured before events are ingested",
            )
        })?;
        let expected_sequence = keyboard.last_event_sequence.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-input",
                "keyboard-event-sequence-overflow",
                "logical keyboard event sequence is exhausted",
            )
        })?;
        if event.sequence != expected_sequence {
            return Err(RuntimeStateError::conflict(
                "runtime-input",
                "keyboard-event-sequence-stale",
                "logical keyboard event sequence is duplicate, stale, or out of order",
            ));
        }

        let mut next_state = keyboard.clone();
        let mut affected_virtual_keys = Vec::new();
        match event.kind {
            RuntimeKeyboardSourceEventKind::FocusGained => {
                validate_focus_event(event)?;
                if !next_state.source_focused {
                    next_state.source_focused = true;
                }
            }
            RuntimeKeyboardSourceEventKind::FocusLost => {
                validate_focus_event(event)?;
                next_state.source_focused = false;
                if next_state.clear_on_source_focus_loss {
                    for virtual_key in 0..=next_state.maximum_virtual_key {
                        if next_state
                            .key_state(virtual_key)
                            .is_some_and(|state| state.down || state.pressed_since_last_query)
                        {
                            affected_virtual_keys.push(virtual_key);
                        }
                    }
                    next_state.down_words.fill(0);
                    next_state.pressed_words.fill(0);
                }
            }
            RuntimeKeyboardSourceEventKind::KeyDown | RuntimeKeyboardSourceEventKind::KeyUp => {
                if !next_state.source_focused {
                    return Err(RuntimeStateError::conflict(
                        "runtime-input",
                        "keyboard-event-source-not-focused",
                        "logical keyboard events require an active browser input source",
                    ));
                }
                let virtual_key = event.virtual_key.ok_or_else(|| {
                    RuntimeStateError::invalid_input(
                        "runtime-input",
                        "keyboard-event-virtual-key-missing",
                        "logical key events require a virtual-key code",
                    )
                })?;
                if virtual_key == 0 || virtual_key > next_state.maximum_virtual_key {
                    return Err(RuntimeStateError::invalid_input(
                        "runtime-input",
                        "keyboard-event-virtual-key-invalid",
                        "logical key event virtual-key code is outside the configured range",
                    ));
                }
                if event.kind == RuntimeKeyboardSourceEventKind::KeyUp && event.repeat {
                    return Err(RuntimeStateError::invalid_input(
                        "runtime-input",
                        "keyboard-event-repeat-invalid",
                        "logical key-up events cannot be repeat events",
                    ));
                }
                let was_down = next_state.key_down(virtual_key);
                if event.kind == RuntimeKeyboardSourceEventKind::KeyDown
                    && event.repeat
                    && !was_down
                {
                    return Err(RuntimeStateError::conflict(
                        "runtime-input",
                        "keyboard-event-repeat-without-key-down",
                        "logical repeat event requires an existing down state",
                    ));
                }
                if event.kind == RuntimeKeyboardSourceEventKind::KeyDown
                    && !event.repeat
                    && was_down
                {
                    return Err(RuntimeStateError::conflict(
                        "runtime-input",
                        "keyboard-event-duplicate-key-down",
                        "logical duplicate key-down must be marked as a repeat",
                    ));
                }
                let down = event.kind == RuntimeKeyboardSourceEventKind::KeyDown;
                next_state.set_key_down(virtual_key, down);
                if down && !was_down {
                    next_state.set_key_pressed(virtual_key, true);
                }
                affected_virtual_keys.push(virtual_key);

                if next_state.aggregate_modifier_keys
                    && let Some((generic, left, right)) = modifier_relation(virtual_key)
                {
                    let generic_was_down = next_state.key_down(generic);
                    let generic_down = next_state.key_down(left) || next_state.key_down(right);
                    next_state.set_key_down(generic, generic_down);
                    if generic_down && !generic_was_down {
                        next_state.set_key_pressed(generic, true);
                    }
                    if generic != virtual_key {
                        affected_virtual_keys.push(generic);
                    }
                }
            }
        }
        next_state.last_event_sequence = event.sequence;
        next_state.generation = keyboard.generation.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-input",
                "keyboard-state-generation-overflow",
                "logical keyboard state generation is exhausted",
            )
        })?;
        affected_virtual_keys.sort_unstable();
        affected_virtual_keys.dedup();
        Ok(RuntimeKeyboardEventPlan {
            expected_generation: keyboard.generation,
            expected_last_event_sequence: keyboard.last_event_sequence,
            next_state,
            event,
            affected_virtual_keys,
        })
    }

    pub(super) fn plan_pointer_button_event(
        &self,
        event: RuntimePointerButtonSourceEvent,
        screen_x: i32,
        screen_y: i32,
    ) -> Result<RuntimePointerButtonEventPlan, RuntimeStateError> {
        let mapping = self.mouse_button_mapping.as_ref().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-input",
                "mouse-button-mapping-unconfigured",
                "logical mouse button mapping must be configured before pointer events are ingested",
            )
        })?;
        let cursor = self.cursor_position.as_ref().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-input",
                "cursor-position-profile-unconfigured",
                "logical cursor position must be configured before pointer events are ingested",
            )
        })?;
        let logical_button =
            match (event.button, mapping.swapped) {
                (RuntimePointerButton::Primary, false)
                | (RuntimePointerButton::Secondary, true) => RuntimeLogicalMouseButton::Left,
                (RuntimePointerButton::Secondary, false)
                | (RuntimePointerButton::Primary, true) => RuntimeLogicalMouseButton::Right,
            };
        let virtual_key = match logical_button {
            RuntimeLogicalMouseButton::Left => VK_LBUTTON,
            RuntimeLogicalMouseButton::Right => VK_RBUTTON,
        };
        let keyboard = self.plan_keyboard_event(RuntimeKeyboardSourceEvent {
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
        })?;
        let next_generation = if cursor.screen_x == screen_x && cursor.screen_y == screen_y {
            cursor.generation
        } else {
            cursor.generation.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-input",
                    "cursor-position-generation-overflow",
                    "logical cursor position generation is exhausted",
                )
            })?
        };
        Ok(RuntimePointerButtonEventPlan {
            expected_mapping_generation: mapping.generation,
            expected_mapping_swapped: mapping.swapped,
            expected_cursor_generation: cursor.generation,
            expected_cursor_x: cursor.screen_x,
            expected_cursor_y: cursor.screen_y,
            next_cursor: RuntimeCursorPosition {
                profile_id: cursor.profile_id.clone(),
                screen_x,
                screen_y,
                generation: next_generation,
            },
            keyboard,
            event,
            logical_button,
            virtual_key,
        })
    }

    pub(super) fn preflight_pointer_button_event(
        &self,
        plan: &RuntimePointerButtonEventPlan,
    ) -> Result<(), RuntimeStateError> {
        let mapping = self.mouse_button_mapping.as_ref().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-input",
                "mouse-button-mapping-unconfigured",
                "logical mouse button mapping disappeared before pointer event commit",
            )
        })?;
        let cursor = self.cursor_position.as_ref().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-input",
                "cursor-position-profile-unconfigured",
                "logical cursor position disappeared before pointer event commit",
            )
        })?;
        if mapping.generation != plan.expected_mapping_generation
            || mapping.swapped != plan.expected_mapping_swapped
            || cursor.generation != plan.expected_cursor_generation
            || cursor.screen_x != plan.expected_cursor_x
            || cursor.screen_y != plan.expected_cursor_y
            || cursor.profile_id != plan.next_cursor.profile_id
        {
            return Err(RuntimeStateError::conflict(
                "runtime-input",
                "pointer-button-event-plan-stale",
                "logical pointer mapping or cursor changed after the event was planned",
            ));
        }
        match self.preflight_keyboard_event(&plan.keyboard) {
            Err(error) if error.code == "keyboard-event-plan-stale" => {
                Err(RuntimeStateError::conflict(
                    "runtime-input",
                    "pointer-button-event-plan-stale",
                    "logical pointer keyboard state changed after the event was planned",
                ))
            }
            result => result,
        }
    }

    pub(super) fn commit_preflighted_pointer_button_event(
        &mut self,
        plan: RuntimePointerButtonEventPlan,
    ) -> RuntimePointerButtonEventReceipt {
        debug_assert!(self.preflight_pointer_button_event(&plan).is_ok());
        let RuntimePointerButtonEventPlan {
            next_cursor,
            keyboard,
            event,
            logical_button,
            virtual_key,
            ..
        } = plan;
        let keyboard = self.commit_preflighted_keyboard_event(keyboard);
        self.cursor_position = Some(next_cursor.clone());
        RuntimePointerButtonEventReceipt {
            event,
            logical_button,
            virtual_key,
            keyboard,
            cursor: next_cursor,
        }
    }

    pub(super) fn commit_keyboard_event(
        &mut self,
        plan: RuntimeKeyboardEventPlan,
    ) -> Result<RuntimeKeyboardEventReceipt, RuntimeStateError> {
        self.preflight_keyboard_event(&plan)?;
        Ok(self.commit_preflighted_keyboard_event(plan))
    }

    fn preflight_keyboard_event(
        &self,
        plan: &RuntimeKeyboardEventPlan,
    ) -> Result<(), RuntimeStateError> {
        let keyboard = self.keyboard.as_ref().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-input",
                "keyboard-profile-unconfigured",
                "logical keyboard profile disappeared before event commit",
            )
        })?;
        if keyboard.generation != plan.expected_generation
            || keyboard.last_event_sequence != plan.expected_last_event_sequence
        {
            return Err(RuntimeStateError::conflict(
                "runtime-input",
                "keyboard-event-plan-stale",
                "logical keyboard state changed after the event was planned",
            ));
        }
        Ok(())
    }

    fn commit_preflighted_keyboard_event(
        &mut self,
        plan: RuntimeKeyboardEventPlan,
    ) -> RuntimeKeyboardEventReceipt {
        debug_assert!(self.preflight_keyboard_event(&plan).is_ok());
        let keyboard = self
            .keyboard
            .as_mut()
            .expect("keyboard event owner was preflighted");
        *keyboard = plan.next_state;
        RuntimeKeyboardEventReceipt {
            sequence: plan.event.sequence,
            kind: plan.event.kind,
            virtual_key: plan.event.virtual_key,
            repeat: plan.event.repeat,
            source_focused: keyboard.source_focused,
            generation: keyboard.generation,
            affected_keys: plan
                .affected_virtual_keys
                .into_iter()
                .filter_map(|virtual_key| keyboard.key_state(virtual_key))
                .collect(),
        }
    }

    pub(super) fn plan_async_key_state_query(
        &self,
        virtual_key: u32,
    ) -> Result<Option<RuntimeAsyncKeyStateQueryPlan>, RuntimeStateError> {
        let keyboard = self.keyboard.as_ref().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-input",
                "keyboard-profile-unconfigured",
                "logical keyboard profile must be configured before it is queried",
            )
        })?;
        let Ok(virtual_key) = u16::try_from(virtual_key) else {
            return Ok(None);
        };
        let Some(state) = keyboard.key_state(virtual_key) else {
            return Ok(None);
        };
        let next_generation = if state.pressed_since_last_query {
            keyboard.generation.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-input",
                    "keyboard-state-generation-overflow",
                    "logical keyboard state generation is exhausted",
                )
            })?
        } else {
            keyboard.generation
        };
        Ok(Some(RuntimeAsyncKeyStateQueryPlan {
            expected_generation: keyboard.generation,
            virtual_key,
            expected_down: state.down,
            expected_pressed: state.pressed_since_last_query,
            next_generation,
        }))
    }

    pub(super) fn commit_async_key_state_query(
        &mut self,
        plan: RuntimeAsyncKeyStateQueryPlan,
    ) -> Result<RuntimeAsyncKeyStateResult, RuntimeStateError> {
        let keyboard = self.keyboard.as_mut().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-input",
                "keyboard-profile-unconfigured",
                "logical keyboard profile disappeared before query commit",
            )
        })?;
        let current = keyboard.key_state(plan.virtual_key).ok_or_else(|| {
            RuntimeStateError::conflict(
                "runtime-input",
                "async-key-state-query-plan-stale",
                "logical virtual-key range changed after the query was planned",
            )
        })?;
        if keyboard.generation != plan.expected_generation
            || current.down != plan.expected_down
            || current.pressed_since_last_query != plan.expected_pressed
        {
            return Err(RuntimeStateError::conflict(
                "runtime-input",
                "async-key-state-query-plan-stale",
                "logical keyboard state changed after the query was planned",
            ));
        }
        if plan.expected_pressed {
            keyboard.set_key_pressed(plan.virtual_key, false);
        }
        keyboard.generation = plan.next_generation;
        let value =
            ((u16::from(plan.expected_down) << 15) | u16::from(plan.expected_pressed)) as i16;
        Ok(RuntimeAsyncKeyStateResult {
            virtual_key: plan.virtual_key,
            down: plan.expected_down,
            pressed_since_last_query: plan.expected_pressed,
            value,
            generation: keyboard.generation,
        })
    }
}

impl RuntimeKeyboardState {
    fn from_profile(profile: RuntimeKeyboardProfile) -> Result<Self, RuntimeStateError> {
        validate_profile_id(&profile.id, "keyboard-profile-id-invalid")?;
        if profile.maximum_virtual_key as usize + 1 != VIRTUAL_KEY_COUNT
            || !profile.aggregate_modifier_keys
            || !profile.clear_on_source_focus_loss
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-input",
                "keyboard-profile-shape-invalid",
                "logical keyboard profile must cover the 8-bit virtual-key range, aggregate modifiers, and clear keys on focus loss",
            ));
        }
        Ok(Self {
            profile_id: profile.id,
            source_focused: profile.initial_source_focused,
            maximum_virtual_key: profile.maximum_virtual_key,
            aggregate_modifier_keys: profile.aggregate_modifier_keys,
            clear_on_source_focus_loss: profile.clear_on_source_focus_loss,
            generation: 0,
            last_event_sequence: 0,
            down_words: [0; VIRTUAL_KEY_WORD_COUNT],
            pressed_words: [0; VIRTUAL_KEY_WORD_COUNT],
        })
    }

    pub fn key_state(&self, virtual_key: u16) -> Option<RuntimeVirtualKeyState> {
        if virtual_key > self.maximum_virtual_key {
            return None;
        }
        Some(RuntimeVirtualKeyState {
            virtual_key,
            down: self.key_down(virtual_key),
            pressed_since_last_query: self.key_pressed(virtual_key),
        })
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

fn validate_profile_id(id: &str, code: &'static str) -> Result<(), RuntimeStateError> {
    if id.is_empty()
        || id.len() > MAX_INPUT_PROFILE_ID_BYTES
        || !id.is_ascii()
        || id.as_bytes().contains(&0)
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-input",
            code,
            "logical input profile ID must be bounded nonempty ASCII",
        ));
    }
    Ok(())
}

fn validate_focus_event(event: RuntimeKeyboardSourceEvent) -> Result<(), RuntimeStateError> {
    if event.virtual_key.is_some() || event.repeat {
        return Err(RuntimeStateError::invalid_input(
            "runtime-input",
            "keyboard-focus-event-shape-invalid",
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
            id: "normal-mouse-buttons-v1".to_owned(),
            initial_mouse_buttons_swapped,
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
        }
    }

    #[test]
    fn configuration_and_swaps_are_planned_and_generation_tracks_only_changes() {
        let mut input = RuntimeInput::default();
        let configuration = input.plan_configuration(profile(false)).unwrap();
        let stale_configuration = configuration.clone();
        input.commit_configuration(configuration).unwrap();
        assert_eq!(
            input.cursor_position(),
            Some(&RuntimeCursorPosition {
                profile_id: "primary-display-center-cursor-v1".to_owned(),
                screen_x: 960,
                screen_y: 540,
                generation: 0,
            })
        );
        assert_eq!(
            input
                .commit_configuration(stale_configuration)
                .unwrap_err()
                .code,
            "input-configuration-plan-stale"
        );

        let no_change = input.plan_mouse_button_swap(false).unwrap();
        let result = input.commit_mouse_button_swap(no_change).unwrap();
        assert!(!result.previous_swapped);
        assert!(!result.current_swapped);
        assert_eq!(result.generation, 0);

        let swap = input.plan_mouse_button_swap(true).unwrap();
        let stale = swap;
        let result = input.commit_mouse_button_swap(swap).unwrap();
        assert!(!result.previous_swapped);
        assert!(result.current_swapped);
        assert_eq!(result.generation, 1);
        assert_eq!(
            input.commit_mouse_button_swap(stale).unwrap_err().code,
            "mouse-button-swap-plan-stale"
        );
        assert_eq!(input.mouse_button_mapping().unwrap().generation, 1);
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

    fn pointer_profile() -> RuntimePointerButtonMessageProfile {
        RuntimePointerButtonMessageProfile {
            id: "normalized-primary-secondary-foreground-client-message-v1".to_owned(),
            normalized_position_denominator: 65_536,
            left_button_down_message: 0x0201,
            left_button_up_message: 0x0202,
            right_button_down_message: 0x0204,
            right_button_up_message: 0x0205,
            left_button_key_mask: 0x0001,
            right_button_key_mask: 0x0002,
            shift_key_mask: 0x0004,
            control_key_mask: 0x0008,
            require_visible_active_focused_foreground_window: true,
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

    #[test]
    fn pointer_events_share_mapping_keyboard_and_cursor_authority_with_stale_safe_commit() {
        let message_profile = pointer_profile();
        let mut input = RuntimeInput::default();
        let configuration = input.plan_configuration(profile(false)).unwrap();
        input.commit_configuration(configuration).unwrap();

        let before = input.clone();
        let down = input
            .plan_pointer_button_event(
                pointer_event(1, RuntimePointerButtonSourceEventKind::ButtonDown),
                1_600,
                900,
            )
            .unwrap();
        assert_eq!(
            input, before,
            "pointer planning must not mutate input state"
        );
        assert_eq!(
            down.message_and_key_state(&message_profile),
            (0x0201, 0x0001)
        );
        input.preflight_pointer_button_event(&down).unwrap();
        let down = input.commit_preflighted_pointer_button_event(down);
        assert_eq!(down.logical_button, RuntimeLogicalMouseButton::Left);
        assert_eq!(down.virtual_key, VK_LBUTTON);
        assert_eq!((down.cursor.screen_x, down.cursor.screen_y), (1_600, 900));
        assert_eq!(down.cursor.generation, 1);
        assert!(down.keyboard.affected_keys[0].down);

        let up = input
            .plan_pointer_button_event(
                pointer_event(2, RuntimePointerButtonSourceEventKind::ButtonUp),
                1_600,
                900,
            )
            .unwrap();
        let stale = up.clone();
        assert_eq!(up.message_and_key_state(&message_profile), (0x0202, 0));
        let up = input.commit_preflighted_pointer_button_event(up);
        assert_eq!(up.cursor.generation, 1);
        assert!(!up.keyboard.affected_keys[0].down);
        assert_eq!(
            input
                .preflight_pointer_button_event(&stale)
                .unwrap_err()
                .code,
            "pointer-button-event-plan-stale"
        );

        let mut swapped = RuntimeInput::default();
        let configuration = swapped.plan_configuration(profile(true)).unwrap();
        swapped.commit_configuration(configuration).unwrap();
        let right = swapped
            .plan_pointer_button_event(
                pointer_event(1, RuntimePointerButtonSourceEventKind::ButtonDown),
                960,
                540,
            )
            .unwrap();
        assert_eq!(
            right.message_and_key_state(&message_profile),
            (0x0204, 0x0002)
        );
        let right = swapped.commit_preflighted_pointer_button_event(right);
        assert_eq!(right.logical_button, RuntimeLogicalMouseButton::Right);
        assert_eq!(right.virtual_key, VK_RBUTTON);

        let mut remapped = RuntimeInput::default();
        let configuration = remapped.plan_configuration(profile(false)).unwrap();
        remapped.commit_configuration(configuration).unwrap();
        let stale_mapping = remapped
            .plan_pointer_button_event(
                pointer_event(1, RuntimePointerButtonSourceEventKind::ButtonDown),
                960,
                540,
            )
            .unwrap();
        let swap = remapped.plan_mouse_button_swap(true).unwrap();
        remapped.commit_mouse_button_swap(swap).unwrap();
        assert_eq!(
            remapped
                .preflight_pointer_button_event(&stale_mapping)
                .unwrap_err()
                .code,
            "pointer-button-event-plan-stale"
        );
    }

    #[test]
    fn modifier_events_aggregate_and_queries_consume_only_the_requested_pressed_bit() {
        let mut input = RuntimeInput::default();
        let configuration = input.plan_configuration(profile(false)).unwrap();
        input.commit_configuration(configuration).unwrap();

        let down = input
            .plan_keyboard_event(key_event(
                1,
                RuntimeKeyboardSourceEventKind::KeyDown,
                VK_LCONTROL,
                false,
            ))
            .unwrap();
        let receipt = input.commit_keyboard_event(down).unwrap();
        assert_eq!(
            receipt
                .affected_keys
                .iter()
                .map(|state| state.virtual_key)
                .collect::<Vec<_>>(),
            vec![VK_CONTROL, VK_LCONTROL]
        );

        let generic = input
            .plan_async_key_state_query(u32::from(VK_CONTROL))
            .unwrap()
            .unwrap();
        let generic = input.commit_async_key_state_query(generic).unwrap();
        assert!(generic.down);
        assert!(generic.pressed_since_last_query);
        assert_eq!(generic.value, 0x8001_u16 as i16);
        let generic_repeat = input
            .plan_async_key_state_query(u32::from(VK_CONTROL))
            .unwrap()
            .unwrap();
        let generic_repeat = input.commit_async_key_state_query(generic_repeat).unwrap();
        assert_eq!(generic_repeat.value, 0x8000_u16 as i16);

        let side = input
            .plan_async_key_state_query(u32::from(VK_LCONTROL))
            .unwrap()
            .unwrap();
        let side = input.commit_async_key_state_query(side).unwrap();
        assert_eq!(side.value, 0x8001_u16 as i16);

        let repeat = input
            .plan_keyboard_event(key_event(
                2,
                RuntimeKeyboardSourceEventKind::KeyDown,
                VK_LCONTROL,
                true,
            ))
            .unwrap();
        input.commit_keyboard_event(repeat).unwrap();
        let repeated_query = input
            .plan_async_key_state_query(u32::from(VK_CONTROL))
            .unwrap()
            .unwrap();
        assert_eq!(
            input
                .commit_async_key_state_query(repeated_query)
                .unwrap()
                .value,
            0x8000_u16 as i16
        );

        let up = input
            .plan_keyboard_event(key_event(
                3,
                RuntimeKeyboardSourceEventKind::KeyUp,
                VK_LCONTROL,
                false,
            ))
            .unwrap();
        input.commit_keyboard_event(up).unwrap();
        let released = input
            .plan_async_key_state_query(u32::from(VK_CONTROL))
            .unwrap()
            .unwrap();
        assert_eq!(
            input.commit_async_key_state_query(released).unwrap().value,
            0
        );
    }

    #[test]
    fn event_and_query_plans_are_stale_safe_and_focus_loss_clears_keys() {
        let mut input = RuntimeInput::default();
        let configuration = input.plan_configuration(profile(false)).unwrap();
        input.commit_configuration(configuration).unwrap();

        let first = input
            .plan_keyboard_event(key_event(
                1,
                RuntimeKeyboardSourceEventKind::KeyDown,
                b'A'.into(),
                false,
            ))
            .unwrap();
        let stale = first.clone();
        input.commit_keyboard_event(first).unwrap();
        assert_eq!(
            input.commit_keyboard_event(stale).unwrap_err().code,
            "keyboard-event-plan-stale"
        );

        let query = input
            .plan_async_key_state_query(u32::from(b'A'))
            .unwrap()
            .unwrap();
        let stale_query = query;
        input.commit_async_key_state_query(query).unwrap();
        assert_eq!(
            input
                .commit_async_key_state_query(stale_query)
                .unwrap_err()
                .code,
            "async-key-state-query-plan-stale"
        );

        let focus_lost = input
            .plan_keyboard_event(focus_event(2, RuntimeKeyboardSourceEventKind::FocusLost))
            .unwrap();
        let receipt = input.commit_keyboard_event(focus_lost).unwrap();
        assert!(!receipt.source_focused);
        assert!(
            !input
                .keyboard_state()
                .unwrap()
                .key_state(u16::from(b'A'))
                .unwrap()
                .down
        );
        let before = input.clone();
        assert_eq!(
            input
                .plan_keyboard_event(key_event(
                    3,
                    RuntimeKeyboardSourceEventKind::KeyDown,
                    b'B'.into(),
                    false,
                ))
                .unwrap_err()
                .code,
            "keyboard-event-source-not-focused"
        );
        assert_eq!(input, before);
        let focus_gained = input
            .plan_keyboard_event(focus_event(3, RuntimeKeyboardSourceEventKind::FocusGained))
            .unwrap();
        input.commit_keyboard_event(focus_gained).unwrap();

        assert!(input.plan_async_key_state_query(0xff).unwrap().is_some());
        assert!(input.plan_async_key_state_query(0x100).unwrap().is_none());
    }

    #[test]
    fn invalid_or_missing_profiles_fail_without_mutation() {
        let input = RuntimeInput::default();
        let before = input.clone();
        let mut invalid = profile(false);
        invalid.id.clear();
        assert_eq!(
            input.plan_configuration(invalid).unwrap_err().code,
            "input-profile-id-invalid"
        );
        assert_eq!(input, before);
        assert_eq!(
            input.plan_mouse_button_swap(true).unwrap_err().code,
            "mouse-button-mapping-unconfigured"
        );
        assert_eq!(input, before);

        let mut invalid_cursor = profile(false);
        invalid_cursor.cursor_position.as_mut().unwrap().id.clear();
        assert_eq!(
            input.plan_configuration(invalid_cursor).unwrap_err().code,
            "cursor-position-profile-id-invalid"
        );
        assert_eq!(input, before);

        let mut invalid_keyboard = profile(false);
        invalid_keyboard
            .keyboard
            .as_mut()
            .unwrap()
            .maximum_virtual_key = 0xfe;
        assert_eq!(
            input.plan_configuration(invalid_keyboard).unwrap_err().code,
            "keyboard-profile-shape-invalid"
        );
        assert_eq!(input, before);
    }
}

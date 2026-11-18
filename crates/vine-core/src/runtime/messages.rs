use std::collections::{BTreeMap, VecDeque};

use super::{RuntimeStateError, RuntimeThreadId};

const MAX_RUNTIME_THREAD_MESSAGES: usize = 4096;
pub(crate) const RUNTIME_QUEUE_STATUS_VALID_FLAGS: u32 = 0x1dff;
const QS_KEY: u16 = 0x0001;
const QS_MOUSEMOVE: u16 = 0x0002;
const QS_MOUSEBUTTON: u16 = 0x0004;
const QS_POSTMESSAGE: u16 = 0x0008;
const QS_TIMER: u16 = 0x0010;
const QS_ALLPOSTMESSAGE: u16 = 0x0100;

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct RuntimeMessageFilter {
    pub window: u32,
    pub minimum: u32,
    pub maximum: u32,
}

impl RuntimeMessageFilter {
    pub(crate) fn includes_message(self, message: u32) -> bool {
        (self.minimum == 0 && self.maximum == 0)
            || (self.minimum <= message && message <= self.maximum)
    }

    fn queue_status_mask(self) -> u16 {
        if self.minimum == 0 && self.maximum == 0 {
            return u16::MAX;
        }
        // Every PeekMessage checks posted messages; input categories are only
        // observed when their message ranges are requested. HWND does not alter
        // these per-thread notification bits.
        let intersects = |first, last| {
            self.minimum <= self.maximum && self.minimum <= last && first <= self.maximum
        };
        let mut mask = QS_POSTMESSAGE | QS_ALLPOSTMESSAGE;
        if intersects(0x0100, 0x0109) {
            mask |= QS_KEY;
        }
        if self.includes_message(0x0200) {
            mask |= QS_MOUSEMOVE;
        }
        if intersects(0x0201, 0x020e) {
            mask |= QS_MOUSEBUTTON;
        }
        if self.includes_message(0x0113) {
            mask |= QS_TIMER;
        }
        mask
    }

    pub(crate) fn matches_window(self, hwnd: u32) -> bool {
        // The currently supported window model contains only top-level windows.
        self.window == 0 || self.window == hwnd || (self.window == u32::MAX && hwnd == 0)
    }

    pub(crate) fn matches(self, hwnd: u32, message: u32) -> bool {
        message == 0x0012 // WM_QUIT bypasses both the HWND and message-range filters.
            || (self.matches_window(hwnd) && self.includes_message(message))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeThreadMessage {
    pub target_thread_id: RuntimeThreadId,
    pub hwnd: u32,
    pub message: u32,
    pub w_param: u32,
    pub l_param: u32,
    pub time: u32,
    pub point_x: i32,
    pub point_y: i32,
}

impl RuntimeThreadMessage {
    pub fn to_x86_msg_bytes(self) -> [u8; 28] {
        let words = [
            self.hwnd,
            self.message,
            self.w_param,
            self.l_param,
            self.time,
            self.point_x as u32,
            self.point_y as u32,
        ];
        let mut bytes = [0; 28];
        for (word, output) in words.into_iter().zip(bytes.chunks_exact_mut(4)) {
            output.copy_from_slice(&word.to_le_bytes());
        }
        bytes
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeMessageReceiveDisposition {
    Message { message: RuntimeThreadMessage },
    Blocked,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub(super) struct RuntimeMessages {
    queues: BTreeMap<RuntimeThreadId, RuntimeMessageQueue>,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
struct RuntimeMessageQueue {
    messages: VecDeque<QueuedMessage>,
    new_status: u16,
    // GetAsyncKeyState follows host transitions; translation follows the
    // keys this thread has actually removed from its Windows message queue.
    keyboard: [u8; 256],
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(test, derive(PartialEq, Eq))]
struct QueuedMessage {
    message: RuntimeThreadMessage,
    status: u16,
}

impl Default for RuntimeMessageQueue {
    fn default() -> Self {
        Self {
            messages: VecDeque::new(),
            new_status: 0,
            keyboard: [0; 256],
        }
    }
}

impl RuntimeMessages {
    pub(super) fn new(initial_thread_id: RuntimeThreadId) -> Self {
        Self {
            queues: BTreeMap::from([(initial_thread_id, RuntimeMessageQueue::default())]),
        }
    }

    pub(super) fn validate_registration(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<(), RuntimeStateError> {
        if self.queues.contains_key(&thread_id) {
            return Err(RuntimeStateError::new(
                "message subsystem already owns a queue for the thread",
            ));
        }
        Ok(())
    }

    pub(super) fn register(&mut self, thread_id: RuntimeThreadId) {
        let previous = self
            .queues
            .insert(thread_id, RuntimeMessageQueue::default());
        assert!(
            previous.is_none(),
            "message queue registration must be validated before insertion"
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn checked_post(
        &self,
        target_thread_id: RuntimeThreadId,
        hwnd: u32,
        message: u32,
        w_param: u32,
        l_param: u32,
        time: u32,
        point_x: i32,
        point_y: i32,
        wake_target: bool,
    ) -> Result<RuntimeThreadMessage, RuntimeStateError> {
        let queue = self.queue(target_thread_id)?;
        if queue.messages.len() >= MAX_RUNTIME_THREAD_MESSAGES {
            return Err(RuntimeStateError::new(
                "runtime thread message queue reached its controlled capacity",
            ));
        }
        if wake_target && queue.new_status != 0 {
            return Err(RuntimeStateError::new(
                "message-waiting thread must have observed its previous queue status",
            ));
        }
        Ok(RuntimeThreadMessage {
            target_thread_id,
            hwnd,
            message,
            w_param,
            l_param,
            time,
            point_x,
            point_y,
        })
    }

    pub(super) fn push(&mut self, message: RuntimeThreadMessage) {
        self.push_with_status(message, QS_POSTMESSAGE | QS_ALLPOSTMESSAGE, false);
    }

    pub(super) fn push_input(&mut self, message: RuntimeThreadMessage) {
        let status = match message.message {
            0x0100 | 0x0101 | 0x0104 | 0x0105 => QS_KEY,
            0x0200 => QS_MOUSEMOVE,
            0x0201..=0x0206 => QS_MOUSEBUTTON,
            _ => unreachable!("input queue message was checked before publication"),
        };
        self.push_with_status(message, status, false);
    }

    pub(super) fn push_window_timer(&mut self, message: RuntimeThreadMessage) {
        self.push_with_status(message, QS_TIMER, false);
    }

    fn push_with_status(&mut self, message: RuntimeThreadMessage, status: u16, front: bool) {
        let queue = self
            .queues
            .get_mut(&message.target_thread_id)
            .expect("message post target was checked");
        let entry = QueuedMessage { message, status };
        if front {
            queue.messages.push_front(entry);
        } else {
            queue.messages.push_back(entry);
        }
        queue.new_status |= status;
    }

    pub(super) fn get_queue_status(
        &mut self,
        thread_id: RuntimeThreadId,
        flags: u32,
    ) -> Result<u32, RuntimeStateError> {
        let result = self.peek_queue_status(thread_id, flags)?;
        self.queues
            .get_mut(&thread_id)
            .expect("live message queue")
            .new_status &= !(flags as u16);
        Ok(result)
    }

    pub(super) fn peek_queue_status(
        &self,
        thread_id: RuntimeThreadId,
        flags: u32,
    ) -> Result<u32, RuntimeStateError> {
        if flags & !RUNTIME_QUEUE_STATUS_VALID_FLAGS != 0 {
            return Err(RuntimeStateError::new("invalid message queue status flags"));
        }
        let queue = self.queues.get(&thread_id).ok_or_else(|| {
            RuntimeStateError::new("live runtime thread has no message subsystem queue")
        })?;
        let current = queue
            .messages
            .iter()
            .fold(0_u16, |bits, entry| bits | entry.status);
        let selected = flags as u16;
        let result = (u32::from(current & selected) << 16)
            | u32::from(queue.new_status & current & selected);
        Ok(result)
    }

    pub(super) fn observe_unfiltered(&mut self, thread_id: RuntimeThreadId) {
        self.observe_filtered(thread_id, RuntimeMessageFilter::default());
    }

    pub(super) fn observe_filtered(
        &mut self,
        thread_id: RuntimeThreadId,
        filter: RuntimeMessageFilter,
    ) {
        self.queues
            .get_mut(&thread_id)
            .expect("message receive owner was checked")
            .new_status &= !filter.queue_status_mask();
    }

    pub(super) fn has_new(&self, thread_id: RuntimeThreadId) -> Result<bool, RuntimeStateError> {
        Ok(self.queue(thread_id)?.new_status != 0)
    }

    #[cfg(test)]
    pub(super) fn front(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<Option<RuntimeThreadMessage>, RuntimeStateError> {
        Ok(self
            .queue(thread_id)?
            .messages
            .front()
            .map(|entry| entry.message))
    }

    pub(super) fn find(
        &self,
        thread_id: RuntimeThreadId,
        filter: RuntimeMessageFilter,
    ) -> Result<Option<(usize, RuntimeThreadMessage)>, RuntimeStateError> {
        Ok(self
            .queue(thread_id)?
            .messages
            .iter()
            .enumerate()
            .find(|(_, entry)| filter.matches(entry.message.hwnd, entry.message.message))
            .map(|(index, entry)| (index, entry.message)))
    }

    #[cfg(test)]
    pub(super) fn pop(&mut self, thread_id: RuntimeThreadId) {
        self.remove_at(thread_id, 0);
    }

    pub(super) fn remove_at(&mut self, thread_id: RuntimeThreadId, index: usize) {
        let queue = self
            .queues
            .get_mut(&thread_id)
            .expect("message receive owner was checked");
        let message = queue
            .messages
            .remove(index)
            .expect("message was checked before receive")
            .message;
        if matches!(message.message, 0x0100 | 0x0101 | 0x0104 | 0x0105) && message.w_param <= 0xff {
            let down = matches!(message.message, 0x0100 | 0x0104);
            let key = message.w_param as usize;
            let side = match key {
                0x10 => {
                    if message.l_param >> 16 & 0xff == 0x36 {
                        0xa1
                    } else {
                        0xa0
                    }
                }
                0x11 => {
                    if message.l_param & 0x0100_0000 != 0 {
                        0xa3
                    } else {
                        0xa2
                    }
                }
                0x12 => {
                    if message.l_param & 0x0100_0000 != 0 {
                        0xa5
                    } else {
                        0xa4
                    }
                }
                _ => key,
            };
            if down && message.l_param & 0x4000_0000 == 0 && matches!(key, 0x14 | 0x90 | 0x91) {
                queue.keyboard[key] ^= 1;
            }
            queue.keyboard[side] = queue.keyboard[side] & 1 | if down { 0x80 } else { 0 };
            if matches!(key, 0x10..=0x12) {
                let left = 0xa0 + (key - 0x10) * 2;
                queue.keyboard[key] = (queue.keyboard[left] | queue.keyboard[left + 1]) & 0x80;
            }
        } else if matches!(message.message, 0x0200..=0x020e) {
            // Mouse messages carry the logical button state at generation time.
            for (key, mask) in [
                (0x01, 1),
                (0x02, 2),
                (0x04, 0x10),
                (0x05, 0x20),
                (0x06, 0x40),
            ] {
                queue.keyboard[key] = if message.w_param & mask != 0 { 0x80 } else { 0 };
            }
        }
    }

    pub(super) fn clear(&mut self, thread_id: RuntimeThreadId) {
        let queue = self
            .queues
            .get_mut(&thread_id)
            .expect("message queue owner was checked before thread exit");
        queue.messages.clear();
        queue.new_status = 0;
    }

    pub(super) fn release_keyboard_source(&mut self) {
        // Source focus loss releases held keys without consuming posted messages
        // or resetting the thread's lock-key toggles.
        for queue in self.queues.values_mut() {
            for key in &mut queue.keyboard {
                *key &= 1;
            }
        }
    }

    #[cfg(test)]
    pub(super) fn snapshot(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<Vec<RuntimeThreadMessage>, RuntimeStateError> {
        Ok(self
            .queue(thread_id)?
            .messages
            .iter()
            .map(|entry| entry.message)
            .collect())
    }

    pub(super) fn is_empty(&self, thread_id: RuntimeThreadId) -> Result<bool, RuntimeStateError> {
        Ok(self.queue(thread_id)?.messages.is_empty())
    }

    pub(super) fn key_state(
        &self,
        thread_id: RuntimeThreadId,
        key: u8,
    ) -> Result<i16, RuntimeStateError> {
        let state = self.queue(thread_id)?.keyboard[usize::from(key)];
        Ok(((u16::from(state & 0x80) << 8) | u16::from(state & 1)) as i16)
    }

    pub(super) fn keyboard_state(
        &self,
        thread: RuntimeThreadId,
    ) -> Result<[u8; 256], RuntimeStateError> {
        Ok(self.queue(thread)?.keyboard)
    }

    pub(super) fn set_keyboard_state(
        &mut self,
        thread: RuntimeThreadId,
        state: [u8; 256],
    ) -> Result<(), RuntimeStateError> {
        self.queues
            .get_mut(&thread)
            .ok_or_else(|| RuntimeStateError::new("thread has no message queue"))?
            .keyboard = state.map(|key| key & 0x81);
        Ok(())
    }

    pub(super) fn keyboard_character(
        &self,
        thread_id: RuntimeThreadId,
        key: u32,
    ) -> Result<Option<u8>, RuntimeStateError> {
        let keyboard = &self.queue(thread_id)?.keyboard;
        let control = keyboard[0x11] & 0x80 != 0;
        let shift = keyboard[0x10] & 0x80 != 0;
        let character = match key {
            0x08 => {
                if control {
                    0x7f
                } else {
                    0x08
                }
            }
            0x09 => 0x09,
            0x0d => {
                if control {
                    0x0a
                } else {
                    0x0d
                }
            }
            0x1b => 0x1b,
            0x20 => {
                if control {
                    0
                } else {
                    0x20
                }
            }
            0x41..=0x5a if control => (key - 0x40) as u8,
            0x41..=0x5a => {
                if shift ^ (keyboard[0x14] & 1 != 0) {
                    key as u8
                } else {
                    (key + 0x20) as u8
                }
            }
            0x30..=0x39 if !shift && !control => key as u8,
            0x10..=0x14
            | 0x21..=0x28
            | 0x2c..=0x2e
            | 0x5b..=0x5d
            | 0x70..=0x87
            | 0x90..=0x91
            | 0xa0..=0xa5 => return Ok(None),
            _ => {
                return Err(RuntimeStateError::new(
                    "keyboard character translation is outside the implemented ASCII and control keys",
                ));
            }
        };
        Ok(Some(character))
    }

    pub(super) fn push_character(&mut self, message: RuntimeThreadMessage) {
        // TranslateMessage's synthesized character is read on the next message
        // loop iteration. Ordinary message posts continue to use FIFO push().
        self.push_with_status(message, QS_POSTMESSAGE | QS_ALLPOSTMESSAGE, true);
    }

    fn queue(&self, thread_id: RuntimeThreadId) -> Result<&RuntimeMessageQueue, RuntimeStateError> {
        self.queues.get(&thread_id).ok_or_else(|| {
            RuntimeStateError::new("live runtime thread has no message subsystem queue")
        })
    }
}

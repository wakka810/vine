use std::collections::{BTreeMap, VecDeque};

use serde::Serialize;

use super::{RuntimeStateError, RuntimeThreadId};

const MAX_RUNTIME_THREAD_MESSAGES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeMessageId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeThreadMessage {
    pub id: RuntimeMessageId,
    pub source_thread_id: RuntimeThreadId,
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
    pub fn to_x86_msg_bytes(self) -> Vec<u8> {
        [
            self.hwnd.to_le_bytes(),
            self.message.to_le_bytes(),
            self.w_param.to_le_bytes(),
            self.l_param.to_le_bytes(),
            self.time.to_le_bytes(),
            self.point_x.to_le_bytes(),
            self.point_y.to_le_bytes(),
        ]
        .concat()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeMessageWake {
    pub thread_id: RuntimeThreadId,
    pub message_id: RuntimeMessageId,
    pub queue_depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum RuntimeMessageReceiveDisposition {
    Message { message: RuntimeThreadMessage },
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum RuntimeMessagePeekDisposition {
    Empty,
    Message {
        message: RuntimeThreadMessage,
        removed: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct RuntimeMessages {
    queues: BTreeMap<RuntimeThreadId, VecDeque<RuntimeThreadMessage>>,
    next_message_id: u64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RuntimeMessagePostPlan {
    pub(super) message: RuntimeThreadMessage,
    pub(super) next_message_id: u64,
    pub(super) queue_depth: usize,
    pub(super) wake_target: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum RuntimeMessageReceivePlan {
    Message { message: RuntimeThreadMessage },
    Blocked,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum RuntimeMessagePeekPlan {
    Empty {
        thread_id: RuntimeThreadId,
    },
    Message {
        message: RuntimeThreadMessage,
        remove: bool,
    },
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RuntimeMessageClearPlan {
    pub(super) thread_id: RuntimeThreadId,
    pub(super) message_count: usize,
}

impl RuntimeMessages {
    pub(super) fn new(initial_thread_id: RuntimeThreadId) -> Self {
        Self {
            queues: BTreeMap::from([(initial_thread_id, VecDeque::new())]),
            next_message_id: 1,
        }
    }

    pub(super) fn preflight_register_thread(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<(), RuntimeStateError> {
        if self.queues.contains_key(&thread_id) {
            return Err(RuntimeStateError::conflict(
                "runtime-message",
                "message-thread-queue-duplicate",
                "message subsystem already owns a queue for the thread",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_register_thread(&mut self, thread_id: RuntimeThreadId) {
        let previous = self.queues.insert(thread_id, VecDeque::new());
        debug_assert!(
            previous.is_none(),
            "message queue registration was not preflighted"
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn plan_post(
        &self,
        source_thread_id: RuntimeThreadId,
        target_thread_id: RuntimeThreadId,
        hwnd: u32,
        message: u32,
        w_param: u32,
        l_param: u32,
        time: u32,
        point_x: i32,
        point_y: i32,
        wake_target: bool,
    ) -> Result<RuntimeMessagePostPlan, RuntimeStateError> {
        let queue = self.queue(target_thread_id)?;
        if queue.len() >= MAX_RUNTIME_THREAD_MESSAGES {
            return Err(RuntimeStateError::resource_limit(
                "runtime-message",
                "message-queue-capacity-exceeded",
                "runtime thread message queue reached its controlled capacity",
            ));
        }
        if wake_target && !queue.is_empty() {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-message",
                "waiting-message-thread-queue-nonempty",
                "message-waiting thread must have an empty queue",
            ));
        }
        let id = RuntimeMessageId(self.next_message_id);
        let next_message_id = self.next_message_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-message",
                "message-id-overflow",
                "runtime message ID space is exhausted",
            )
        })?;
        Ok(RuntimeMessagePostPlan {
            message: RuntimeThreadMessage {
                id,
                source_thread_id,
                target_thread_id,
                hwnd,
                message,
                w_param,
                l_param,
                time,
                point_x,
                point_y,
            },
            next_message_id,
            queue_depth: queue.len() + 1,
            wake_target,
        })
    }

    pub(super) fn commit_post(&mut self, plan: RuntimeMessagePostPlan) {
        let queue = self
            .queues
            .get_mut(&plan.message.target_thread_id)
            .expect("message post target was preflighted");
        debug_assert_eq!(queue.len() + 1, plan.queue_depth);
        debug_assert_eq!(self.next_message_id + 1, plan.next_message_id);
        queue.push_back(plan.message);
        self.next_message_id = plan.next_message_id;
    }

    pub(super) fn plan_receive(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeMessageReceivePlan, RuntimeStateError> {
        Ok(match self.queue(thread_id)?.front().copied() {
            Some(message) => RuntimeMessageReceivePlan::Message { message },
            None => RuntimeMessageReceivePlan::Blocked,
        })
    }

    pub(super) fn commit_receive(
        &mut self,
        thread_id: RuntimeThreadId,
        expected: RuntimeMessageId,
    ) {
        let queue = self
            .queues
            .get_mut(&thread_id)
            .expect("message receive owner was preflighted");
        debug_assert_eq!(queue.front().map(|message| message.id), Some(expected));
        queue.pop_front();
    }

    pub(super) fn plan_peek(
        &self,
        thread_id: RuntimeThreadId,
        remove: bool,
    ) -> Result<RuntimeMessagePeekPlan, RuntimeStateError> {
        Ok(match self.queue(thread_id)?.front().copied() {
            Some(message) => RuntimeMessagePeekPlan::Message { message, remove },
            None => RuntimeMessagePeekPlan::Empty { thread_id },
        })
    }

    pub(super) fn preflight_peek(
        &self,
        plan: &RuntimeMessagePeekPlan,
    ) -> Result<(), RuntimeStateError> {
        let (thread_id, expected) = match plan {
            RuntimeMessagePeekPlan::Empty { thread_id } => (*thread_id, None),
            RuntimeMessagePeekPlan::Message { message, .. } => {
                (message.target_thread_id, Some(message.id))
            }
        };
        if self.queue(thread_id)?.front().map(|message| message.id) != expected {
            return Err(RuntimeStateError::conflict(
                "runtime-message",
                "message-peek-plan-stale",
                "message queue head changed after PeekMessage was planned",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_preflighted_peek(
        &mut self,
        plan: RuntimeMessagePeekPlan,
    ) -> RuntimeMessagePeekDisposition {
        match plan {
            RuntimeMessagePeekPlan::Empty { .. } => RuntimeMessagePeekDisposition::Empty,
            RuntimeMessagePeekPlan::Message { message, remove } => {
                if remove {
                    self.queues
                        .get_mut(&message.target_thread_id)
                        .expect("message peek owner was preflighted")
                        .pop_front();
                }
                RuntimeMessagePeekDisposition::Message {
                    message,
                    removed: remove,
                }
            }
        }
    }

    pub(super) fn plan_clear(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeMessageClearPlan, RuntimeStateError> {
        Ok(RuntimeMessageClearPlan {
            thread_id,
            message_count: self.queue(thread_id)?.len(),
        })
    }

    pub(super) fn commit_clear(&mut self, plan: RuntimeMessageClearPlan) {
        let queue = self
            .queues
            .get_mut(&plan.thread_id)
            .expect("message clear owner was preflighted");
        debug_assert_eq!(queue.len(), plan.message_count);
        queue.clear();
    }

    pub(super) fn snapshot(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<Vec<RuntimeThreadMessage>, RuntimeStateError> {
        Ok(self.queue(thread_id)?.iter().copied().collect())
    }

    pub(super) fn is_empty(&self, thread_id: RuntimeThreadId) -> Result<bool, RuntimeStateError> {
        Ok(self.queue(thread_id)?.is_empty())
    }

    fn queue(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<&VecDeque<RuntimeThreadMessage>, RuntimeStateError> {
        self.queues.get(&thread_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-message",
                "message-thread-queue-missing",
                "live runtime thread has no message subsystem queue",
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNER: RuntimeThreadId = RuntimeThreadId(1);
    const SOURCE: RuntimeThreadId = RuntimeThreadId(2);

    #[test]
    fn queue_plans_do_not_mutate_and_commits_preserve_fifo_identity() {
        let mut messages = RuntimeMessages::new(OWNER);
        messages.preflight_register_thread(SOURCE).unwrap();
        messages.commit_register_thread(SOURCE);

        let before_plan = messages.clone();
        let first = messages
            .plan_post(SOURCE, OWNER, 3, 4, 5, 6, 7, 8, 9, true)
            .unwrap();
        assert_eq!(messages, before_plan);
        assert_eq!(first.message.id, RuntimeMessageId(1));
        assert_eq!(first.queue_depth, 1);
        messages.commit_post(first);

        let second = messages
            .plan_post(SOURCE, OWNER, 10, 11, 12, 13, 14, 15, 16, false)
            .unwrap();
        messages.commit_post(second);
        assert_eq!(
            messages.snapshot(OWNER).unwrap(),
            vec![first.message, second.message]
        );

        let stale_peek = messages.plan_peek(OWNER, true).unwrap();
        let receive = messages.plan_receive(OWNER).unwrap();
        assert!(matches!(
            receive,
            RuntimeMessageReceivePlan::Message { message, .. } if message == first.message
        ));
        messages.commit_receive(OWNER, first.message.id);
        assert_eq!(messages.snapshot(OWNER).unwrap(), vec![second.message]);
        assert_eq!(
            messages.preflight_peek(&stale_peek).unwrap_err().code,
            "message-peek-plan-stale"
        );
        assert_eq!(messages.snapshot(OWNER).unwrap(), vec![second.message]);

        let no_remove = messages.plan_peek(OWNER, false).unwrap();
        messages.preflight_peek(&no_remove).unwrap();
        assert_eq!(
            messages.commit_preflighted_peek(no_remove),
            RuntimeMessagePeekDisposition::Message {
                message: second.message,
                removed: false,
            }
        );
        assert_eq!(messages.snapshot(OWNER).unwrap(), vec![second.message]);

        let remove = messages.plan_peek(OWNER, true).unwrap();
        messages.preflight_peek(&remove).unwrap();
        assert_eq!(
            messages.commit_preflighted_peek(remove),
            RuntimeMessagePeekDisposition::Message {
                message: second.message,
                removed: true,
            }
        );
        assert!(messages.snapshot(OWNER).unwrap().is_empty());

        let empty = messages.plan_peek(OWNER, true).unwrap();
        messages.preflight_peek(&empty).unwrap();
        assert_eq!(
            messages.commit_preflighted_peek(empty),
            RuntimeMessagePeekDisposition::Empty
        );
    }

    #[test]
    fn rejected_plan_preserves_queue_and_allocator_state() {
        let messages = RuntimeMessages::new(OWNER);
        let before = messages.clone();
        assert_eq!(
            messages
                .plan_post(SOURCE, RuntimeThreadId(99), 0, 0, 0, 0, 0, 0, 0, false)
                .unwrap_err()
                .code,
            "message-thread-queue-missing"
        );
        assert_eq!(messages, before);
    }
}

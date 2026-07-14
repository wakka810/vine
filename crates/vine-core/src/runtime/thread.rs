use std::collections::BTreeMap;

use serde::Serialize;

use super::{
    RuntimeCallbackFrame, RuntimeCallbackFrameId, RuntimeStateError, RuntimeThreadId,
    RuntimeThreadLifecycleState, RuntimeThreadStack, RuntimeThreadState, RuntimeTlsValue,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub(super) struct RuntimeThreads {
    states: BTreeMap<RuntimeThreadId, RuntimeThreadState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeThreadMutationPlan {
    previous: Option<RuntimeThreadState>,
    next: RuntimeThreadState,
}

impl RuntimeThreadMutationPlan {
    pub(super) fn next(&self) -> &RuntimeThreadState {
        &self.next
    }

    pub(super) fn terminal_after_request(&self, exit_code: u32) -> Result<Self, RuntimeStateError> {
        if self.next.lifecycle != (RuntimeThreadLifecycleState::ExitRequested { exit_code }) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-thread",
                "thread-terminal-plan-request-mismatch",
                "terminal thread plan requires the matching exit-requested state",
            ));
        }
        let previous = self.next.clone();
        let mut next = previous.clone();
        next.lifecycle = RuntimeThreadLifecycleState::Exited { exit_code };
        Ok(Self {
            previous: Some(previous),
            next,
        })
    }
}

impl RuntimeThreads {
    pub(super) fn new(initial_thread_id: RuntimeThreadId, initial_last_error: u32) -> Self {
        let initial = RuntimeThreadState {
            id: initial_thread_id,
            lifecycle: RuntimeThreadLifecycleState::Initialized,
            last_error: initial_last_error,
            tls_values: BTreeMap::new(),
            fls_storage_initialized: false,
            fls_values: BTreeMap::new(),
            callback_frames: Vec::new(),
            stack: None,
        };
        Self {
            states: BTreeMap::from([(initial_thread_id, initial)]),
        }
    }

    pub(super) fn contains(&self, thread_id: RuntimeThreadId) -> bool {
        self.states.contains_key(&thread_id)
    }

    pub(super) fn state(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<&RuntimeThreadState, RuntimeStateError> {
        self.states.get(&thread_id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-thread-registry",
                "runtime-thread-id-unknown",
                format!("runtime thread ID {} is not registered", thread_id.0),
            )
        })
    }

    pub(super) fn current_state(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<&RuntimeThreadState, RuntimeStateError> {
        self.states.get(&thread_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-thread",
                "current-thread-state-missing",
                "scheduler current thread has no runtime thread state",
            )
        })
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (&RuntimeThreadId, &RuntimeThreadState)> {
        self.states.iter()
    }

    pub(super) fn plan_registration(
        &self,
        thread_id: RuntimeThreadId,
        initial_last_error: u32,
    ) -> Result<RuntimeThreadMutationPlan, RuntimeStateError> {
        if thread_id.0 == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-thread-registry",
                "thread-id-zero",
                "runtime thread IDs must be nonzero",
            ));
        }
        if self.states.contains_key(&thread_id) {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-registry",
                "thread-id-duplicate",
                format!("runtime thread ID {} is already registered", thread_id.0),
            ));
        }
        Ok(RuntimeThreadMutationPlan {
            previous: None,
            next: RuntimeThreadState {
                id: thread_id,
                lifecycle: RuntimeThreadLifecycleState::Running,
                last_error: initial_last_error,
                tls_values: BTreeMap::new(),
                fls_storage_initialized: false,
                fls_values: BTreeMap::new(),
                callback_frames: Vec::new(),
                stack: None,
            },
        })
    }

    pub(super) fn plan_start(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeThreadMutationPlan, RuntimeStateError> {
        let previous = self.current_state(thread_id)?.clone();
        if previous.lifecycle != RuntimeThreadLifecycleState::Initialized {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "initial-thread-start-transition-invalid",
                "initial thread start requires initialized thread state",
            ));
        }
        let mut next = previous.clone();
        next.lifecycle = RuntimeThreadLifecycleState::Running;
        Ok(RuntimeThreadMutationPlan {
            previous: Some(previous),
            next,
        })
    }

    pub(super) fn plan_stack_registration(
        &self,
        thread_id: RuntimeThreadId,
        stack: RuntimeThreadStack,
    ) -> Result<RuntimeThreadMutationPlan, RuntimeStateError> {
        let previous = self.state(thread_id)?.clone();
        if previous.stack.is_some() {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-stack",
                "thread-stack-already-registered",
                "runtime thread stack may be registered only once",
            ));
        }
        if stack.thread_id != thread_id {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-thread-stack",
                "thread-stack-owner-mismatch",
                "runtime thread stack identity does not match its owner thread",
            ));
        }
        let mut next = previous.clone();
        next.stack = Some(stack);
        Ok(RuntimeThreadMutationPlan {
            previous: Some(previous),
            next,
        })
    }

    pub(super) fn plan_stack_growth(
        &self,
        thread_id: RuntimeThreadId,
        next_stack: RuntimeThreadStack,
    ) -> Result<RuntimeThreadMutationPlan, RuntimeStateError> {
        let previous = self.state(thread_id)?.clone();
        let stack = previous.stack.as_ref().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-thread-stack",
                "thread-stack-not-registered",
                format!("runtime thread ID {} has no registered stack", thread_id.0),
            )
        })?;
        if next_stack.thread_id != thread_id
            || next_stack.address_allocation_id != stack.address_allocation_id
            || next_stack.memory_owner != stack.memory_owner
            || next_stack.reserved_range != stack.reserved_range
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-thread-stack",
                "thread-stack-growth-identity-mismatch",
                "thread stack growth cannot replace its allocation identity",
            ));
        }
        let mut next = previous.clone();
        next.stack = Some(next_stack);
        Ok(RuntimeThreadMutationPlan {
            previous: Some(previous),
            next,
        })
    }

    pub(super) fn last_error(&self, thread_id: RuntimeThreadId) -> Result<u32, RuntimeStateError> {
        Ok(self.current_state(thread_id)?.last_error)
    }

    pub(super) fn plan_last_error(
        &self,
        thread_id: RuntimeThreadId,
        value: u32,
    ) -> Result<RuntimeThreadMutationPlan, RuntimeStateError> {
        let previous = self.current_state(thread_id)?.clone();
        let mut next = previous.clone();
        next.last_error = value;
        Ok(RuntimeThreadMutationPlan {
            previous: Some(previous),
            next,
        })
    }

    pub(super) fn plan_initialize_fls_storage(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeThreadMutationPlan, RuntimeStateError> {
        let previous = self.current_state(thread_id)?.clone();
        let mut next = previous.clone();
        next.fls_storage_initialized = true;
        Ok(RuntimeThreadMutationPlan {
            previous: Some(previous),
            next,
        })
    }

    pub(super) fn plan_fls_value(
        &self,
        thread_id: RuntimeThreadId,
        index: u32,
        value: u32,
    ) -> Result<(RuntimeThreadMutationPlan, Option<u32>), RuntimeStateError> {
        let previous = self.current_state(thread_id)?.clone();
        let mut next = previous.clone();
        next.fls_storage_initialized = true;
        let replaced = next.fls_values.insert(index, value);
        Ok((
            RuntimeThreadMutationPlan {
                previous: Some(previous),
                next,
            },
            replaced,
        ))
    }

    pub(super) fn fls_value_or_unset(
        &self,
        thread_id: RuntimeThreadId,
        index: u32,
    ) -> Result<Option<u32>, RuntimeStateError> {
        let thread = self.current_state(thread_id)?;
        if !thread.fls_storage_initialized {
            return Err(RuntimeStateError::not_found(
                "runtime-fls",
                "fls-thread-storage-uninitialized",
                format!("current thread has no FLS storage for logical index {index}"),
            ));
        }
        Ok(thread.fls_values.get(&index).copied())
    }

    pub(super) fn plan_tls_value(
        &self,
        thread_id: RuntimeThreadId,
        index: u32,
        value: RuntimeTlsValue,
    ) -> Result<RuntimeThreadMutationPlan, RuntimeStateError> {
        let previous = self.current_state(thread_id)?.clone();
        let mut next = previous.clone();
        next.tls_values.insert(index, value);
        Ok(RuntimeThreadMutationPlan {
            previous: Some(previous),
            next,
        })
    }

    pub(super) fn tls_value_or_unset(
        &self,
        thread_id: RuntimeThreadId,
        index: u32,
    ) -> Result<Option<&RuntimeTlsValue>, RuntimeStateError> {
        Ok(self.current_state(thread_id)?.tls_values.get(&index))
    }

    pub(super) fn initialized_tls_thread_count(&self, index: u32) -> usize {
        self.states
            .values()
            .filter(|thread| thread.tls_values.contains_key(&index))
            .count()
    }

    pub(super) fn callback_depth(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<usize, RuntimeStateError> {
        Ok(self.current_state(thread_id)?.callback_frames.len())
    }

    pub(super) fn plan_callback_push(
        &self,
        thread_id: RuntimeThreadId,
        frame: RuntimeCallbackFrame,
    ) -> Result<(RuntimeThreadMutationPlan, usize), RuntimeStateError> {
        if frame.thread_id != thread_id {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-callback",
                "callback-frame-thread-mismatch",
                "callback frame identity does not match the selected thread",
            ));
        }
        let previous = self.current_state(thread_id)?.clone();
        let mut next = previous.clone();
        next.callback_frames.push(frame);
        let depth = next.callback_frames.len();
        Ok((
            RuntimeThreadMutationPlan {
                previous: Some(previous),
                next,
            },
            depth,
        ))
    }

    pub(super) fn plan_callback_pop(
        &self,
        thread_id: RuntimeThreadId,
        expected: RuntimeCallbackFrameId,
    ) -> Result<(RuntimeThreadMutationPlan, RuntimeCallbackFrame, usize), RuntimeStateError> {
        let previous = self.current_state(thread_id)?.clone();
        let frame = previous.callback_frames.last().copied().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-callback",
                "callback-frame-stack-empty",
                "current thread has no callback frame to return",
            )
        })?;
        if frame.id != expected {
            return Err(RuntimeStateError::conflict(
                "runtime-callback",
                "callback-frame-return-order-mismatch",
                "callback frame return is not strict LIFO order",
            ));
        }
        let mut next = previous.clone();
        next.callback_frames.pop();
        let depth = next.callback_frames.len();
        Ok((
            RuntimeThreadMutationPlan {
                previous: Some(previous),
                next,
            },
            frame,
            depth,
        ))
    }

    pub(super) fn plan_callback_replace(
        &self,
        thread_id: RuntimeThreadId,
        expected: RuntimeCallbackFrameId,
        frame: RuntimeCallbackFrame,
    ) -> Result<(RuntimeThreadMutationPlan, RuntimeCallbackFrame, usize), RuntimeStateError> {
        if frame.thread_id != thread_id {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-callback",
                "callback-frame-thread-mismatch",
                "callback frame identity does not match the selected thread",
            ));
        }
        let previous = self.current_state(thread_id)?.clone();
        let replaced = previous.callback_frames.last().copied().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-callback",
                "callback-frame-stack-empty",
                "current thread has no callback frame to replace",
            )
        })?;
        if replaced.id != expected {
            return Err(RuntimeStateError::conflict(
                "runtime-callback",
                "callback-frame-return-order-mismatch",
                "callback frame replacement is not strict LIFO order",
            ));
        }
        let mut next = previous.clone();
        *next
            .callback_frames
            .last_mut()
            .expect("validated callback frame disappeared") = frame;
        let depth = next.callback_frames.len();
        Ok((
            RuntimeThreadMutationPlan {
                previous: Some(previous),
                next,
            },
            replaced,
            depth,
        ))
    }

    pub(super) fn plan_exit_request(
        &self,
        thread_id: RuntimeThreadId,
        exit_code: u32,
    ) -> Result<(RuntimeThreadMutationPlan, usize), RuntimeStateError> {
        let previous = self.state(thread_id)?.clone();
        if previous.lifecycle != RuntimeThreadLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "thread-exit-request-transition-invalid",
                "thread exit request requires a running thread",
            ));
        }
        let callback_frames_cancelled = previous.callback_frames.len();
        let mut next = previous.clone();
        next.callback_frames.clear();
        next.lifecycle = RuntimeThreadLifecycleState::ExitRequested { exit_code };
        Ok((
            RuntimeThreadMutationPlan {
                previous: Some(previous),
                next,
            },
            callback_frames_cancelled,
        ))
    }

    pub(super) fn plan_exit_completion(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<(RuntimeThreadMutationPlan, u32), RuntimeStateError> {
        let previous = self.state(thread_id)?.clone();
        let exit_code = match previous.lifecycle {
            RuntimeThreadLifecycleState::ExitRequested { exit_code } => exit_code,
            _ => {
                return Err(RuntimeStateError::conflict(
                    "runtime-lifecycle",
                    "thread-exit-completion-transition-invalid",
                    "thread exit completion requires a prior exit request",
                ));
            }
        };
        let mut next = previous.clone();
        next.lifecycle = RuntimeThreadLifecycleState::Exited { exit_code };
        Ok((
            RuntimeThreadMutationPlan {
                previous: Some(previous),
                next,
            },
            exit_code,
        ))
    }

    pub(super) fn commit(&mut self, plan: RuntimeThreadMutationPlan) -> &RuntimeThreadState {
        debug_assert_eq!(
            self.states.get(&plan.next.id),
            plan.previous.as_ref(),
            "thread plan was not committed against its preflight state"
        );
        let thread_id = plan.next.id;
        let previous = self.states.insert(thread_id, plan.next);
        debug_assert_eq!(previous, plan.previous);
        self.states
            .get(&thread_id)
            .expect("committed runtime thread disappeared")
    }
}

#[cfg(test)]
mod tests {
    use crate::memory::GuestAddress;

    use super::*;

    const INITIAL: RuntimeThreadId = RuntimeThreadId(0x2000);
    const SECOND: RuntimeThreadId = RuntimeThreadId(0x1001);

    #[test]
    fn plans_are_mutation_free_and_thread_local_values_remain_isolated() {
        let mut threads = RuntimeThreads::new(INITIAL, 11);
        let before_start = threads.clone();
        let start = threads.plan_start(INITIAL).unwrap();
        assert_eq!(threads, before_start);
        threads.commit(start);

        let registration = threads.plan_registration(SECOND, 33).unwrap();
        assert!(!threads.contains(SECOND));
        threads.commit(registration);
        let initial_tls = threads
            .plan_tls_value(INITIAL, 0, RuntimeTlsValue::Raw { value: 0x1111 })
            .unwrap();
        let second_tls = threads
            .plan_tls_value(SECOND, 0, RuntimeTlsValue::Raw { value: 0x2222 })
            .unwrap();
        threads.commit(initial_tls);
        threads.commit(second_tls);

        assert_eq!(threads.last_error(INITIAL), Ok(11));
        assert_eq!(threads.last_error(SECOND), Ok(33));
        assert_eq!(
            threads
                .tls_value_or_unset(INITIAL, 0)
                .unwrap()
                .unwrap()
                .raw_value(),
            0x1111
        );
        assert_eq!(
            threads
                .tls_value_or_unset(SECOND, 0)
                .unwrap()
                .unwrap()
                .raw_value(),
            0x2222
        );
    }

    #[test]
    fn callback_and_exit_plans_commit_in_explicit_order() {
        let mut threads = RuntimeThreads::new(INITIAL, 7);
        let start = threads.plan_start(INITIAL).unwrap();
        threads.commit(start);
        let frame = RuntimeCallbackFrame {
            id: RuntimeCallbackFrameId(1),
            thread_id: INITIAL,
            target: GuestAddress(0x1000_1000),
            return_sentinel: GuestAddress(0x7f01_0000),
            entry_last_error: 7,
        };
        let (push, depth) = threads.plan_callback_push(INITIAL, frame).unwrap();
        assert_eq!(depth, 1);
        assert_eq!(threads.callback_depth(INITIAL), Ok(0));
        threads.commit(push);

        let replacement = RuntimeCallbackFrame {
            id: RuntimeCallbackFrameId(2),
            target: GuestAddress(0x1000_2000),
            ..frame
        };
        let (replace, replaced, depth) = threads
            .plan_callback_replace(INITIAL, frame.id, replacement)
            .unwrap();
        assert_eq!(replaced, frame);
        assert_eq!(depth, 1);
        assert_eq!(
            threads.state(INITIAL).unwrap().callback_frames,
            [frame],
            "callback replacement remains mutation-free until commit"
        );
        threads.commit(replace);
        assert_eq!(
            threads.state(INITIAL).unwrap().callback_frames,
            [replacement]
        );

        let (request, cancelled) = threads.plan_exit_request(INITIAL, 23).unwrap();
        let terminal = request.terminal_after_request(23).unwrap();
        assert_eq!(cancelled, 1);
        assert_eq!(threads.state(INITIAL).unwrap().callback_frames.len(), 1);
        threads.commit(request);
        threads.commit(terminal);
        assert_eq!(
            threads.state(INITIAL).unwrap().lifecycle,
            RuntimeThreadLifecycleState::Exited { exit_code: 23 }
        );
    }

    #[test]
    fn rejected_plans_preserve_the_registry_and_allocator_identity() {
        let threads = RuntimeThreads::new(INITIAL, 0);
        let before = threads.clone();
        assert_eq!(
            threads.plan_registration(INITIAL, 1).unwrap_err().code,
            "thread-id-duplicate"
        );
        assert_eq!(
            threads
                .plan_registration(RuntimeThreadId(0), 1)
                .unwrap_err()
                .code,
            "thread-id-zero"
        );
        assert_eq!(threads, before);
    }
}

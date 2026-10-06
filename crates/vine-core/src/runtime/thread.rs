use std::collections::{BTreeMap, BTreeSet};

use super::{ProcessAddressAllocationId, RuntimeStateError};
use crate::com::{COM_RPC_E_CHANGED_MODE, COM_S_FALSE, COM_S_OK, ComHResult};
use crate::memory::{GuestAddress, GuestRange, MemoryOwnerId};

pub(crate) const TLS_OUT_OF_INDEXES: u32 = u32::MAX;
const TLS_INDEX_LIMIT: u32 = 1088;
pub const FLS_OUT_OF_INDEXES: u32 = u32::MAX;
pub const RUNTIME_TLS_MINIMUM_AVAILABLE: u32 = 64;
pub const RUNTIME_FLS_INDEX_LIMIT: u32 = 128;
pub const RUNTIME_GUEST_THREAD_LIMIT: usize = 256;
pub const RUNTIME_GUEST_THREAD_STACK_ARENA_BASE: GuestAddress = GuestAddress(0x5000_0000);
pub const RUNTIME_GUEST_THREAD_STACK_ARENA_SIZE: u64 = 0x1000_0000;
pub const RUNTIME_GUEST_THREAD_ENVIRONMENT_ARENA_BASE: GuestAddress = GuestAddress(0x7f03_0000);
pub const RUNTIME_GUEST_THREAD_ENVIRONMENT_ARENA_SIZE: u64 = 0x00c0_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeComApartmentModel {
    SingleThreaded,
    MultiThreaded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeComApartmentState {
    pub model: RuntimeComApartmentModel,
    pub initialization_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeThreadId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeCallbackFrameId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCallbackScratch {
    pub slot: u16,
    pub address: GuestAddress,
    pub byte_length: u32,
    pub capacity: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCallbackFrame {
    pub id: RuntimeCallbackFrameId,
    pub thread_id: RuntimeThreadId,
    pub target: GuestAddress,
    pub return_sentinel: GuestAddress,
    pub entry_last_error: u32,
    pub scratch: Option<RuntimeCallbackScratch>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeThreadLifecycleState {
    Initialized,
    Running,
    ExitRequested { exit_code: u32 },
    Exited { exit_code: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeThreadPriority {
    Idle,
    Lowest,
    BelowNormal,
    Normal,
    AboveNormal,
    Highest,
    TimeCritical,
}

impl RuntimeThreadPriority {
    pub(super) const fn scheduling_rank(self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Lowest => 1,
            Self::BelowNormal => 2,
            Self::Normal => 3,
            Self::AboveNormal => 4,
            Self::Highest => 5,
            Self::TimeCritical => 6,
        }
    }

    pub const fn from_win32_value(value: i32) -> Option<Self> {
        match value {
            -15 => Some(Self::Idle),
            -2 => Some(Self::Lowest),
            -1 => Some(Self::BelowNormal),
            0 => Some(Self::Normal),
            1 => Some(Self::AboveNormal),
            2 => Some(Self::Highest),
            15 => Some(Self::TimeCritical),
            _ => None,
        }
    }

    pub const fn win32_value(self) -> i32 {
        match self {
            Self::Idle => -15,
            Self::Lowest => -2,
            Self::BelowNormal => -1,
            Self::Normal => 0,
            Self::AboveNormal => 1,
            Self::Highest => 2,
            Self::TimeCritical => 15,
        }
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub struct RuntimeThreadState {
    pub id: RuntimeThreadId,
    pub lifecycle: RuntimeThreadLifecycleState,
    pub last_error: u32,
    pub cursor_display_count: i32,
    pub com_apartment: Option<RuntimeComApartmentState>,
    pub priority: Option<RuntimeThreadPriority>,
    pub tls_values: BTreeMap<u32, u32>,
    pub fls_storage_initialized: bool,
    pub fls_values: BTreeMap<u32, u32>,
    pub callback_frames: Vec<RuntimeCallbackFrame>,
    pub stack: Option<RuntimeThreadStack>,
    pub environment: Option<RuntimeThreadEnvironment>,
    pub guest_bootstrap: Option<RuntimeGuestThreadBootstrap>,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub struct RuntimeThreadStack {
    pub thread_id: RuntimeThreadId,
    pub address_allocation_id: ProcessAddressAllocationId,
    pub memory_owner: MemoryOwnerId,
    pub reserved_range: GuestRange,
    pub committed_range: GuestRange,
    // The process image default does not grow when a running thread consumes its stack guard.
    pub initial_commit_bytes: u64,
    pub guard_page: Option<GuestRange>,
    pub stack_top: GuestAddress,
    pub initial_stack_pointer: GuestAddress,
    pub return_sentinel: GuestAddress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeThreadEnvironment {
    pub thread_id: RuntimeThreadId,
    pub address_allocation_id: ProcessAddressAllocationId,
    pub memory_owner: MemoryOwnerId,
    pub range: GuestRange,
    pub teb_address: GuestAddress,
    pub peb_address: GuestAddress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeGuestThreadBootstrap {
    pub thread_id: RuntimeThreadId,
    pub entry_point: GuestAddress,
    pub parameter: u32,
    pub initial_stack_pointer: GuestAddress,
    pub fs_base: GuestAddress,
    pub return_sentinel: GuestAddress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeCreateThreadRequest {
    pub stack_reserve_bytes: u64,
    pub stack_commit_bytes: u64,
    pub entry_point: GuestAddress,
    pub parameter: u32,
    pub thread_id_output: Option<GuestAddress>,
    pub initial_suspend_count: u32,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub(super) struct RuntimeThreads {
    states: BTreeMap<RuntimeThreadId, RuntimeThreadState>,
    allocated_tls_indices: BTreeSet<u32>,
    fls_slots: BTreeMap<u32, Option<GuestAddress>>,
    next_tls_index: u32,
    next_fls_index: u32,
    next_callback_frame_id: u64,
    next_thread_id: u64,
}

impl RuntimeThreads {
    pub(super) fn new(initial_thread_id: RuntimeThreadId, initial_last_error: u32) -> Self {
        let initial = RuntimeThreadState {
            id: initial_thread_id,
            lifecycle: RuntimeThreadLifecycleState::Initialized,
            last_error: initial_last_error,
            cursor_display_count: 0,
            com_apartment: None,
            priority: None,
            tls_values: BTreeMap::new(),
            fls_storage_initialized: false,
            fls_values: BTreeMap::new(),
            callback_frames: Vec::new(),
            stack: None,
            environment: None,
            guest_bootstrap: None,
        };
        Self {
            states: BTreeMap::from([(initial_thread_id, initial)]),
            allocated_tls_indices: BTreeSet::new(),
            fls_slots: BTreeMap::new(),
            next_tls_index: 0,
            next_fls_index: 0,
            next_callback_frame_id: 1,
            next_thread_id: u64::from(initial_thread_id.0) + 1,
        }
    }

    #[cfg(test)]
    pub(crate) fn allocated_tls_indices(&self) -> &BTreeSet<u32> {
        &self.allocated_tls_indices
    }

    #[cfg(test)]
    const fn next_tls_index(&self) -> u32 {
        self.next_tls_index
    }

    #[cfg(test)]
    const fn next_fls_index(&self) -> u32 {
        self.next_fls_index
    }

    pub(super) fn allocate_tls_index(&mut self) -> Result<u32, RuntimeStateError> {
        if self.next_tls_index >= TLS_INDEX_LIMIT {
            return Ok(TLS_OUT_OF_INDEXES);
        }
        let index = self.next_tls_index;
        if self.allocated_tls_indices.contains(&index) {
            return Err(RuntimeStateError::new(
                "logical TLS allocator selected an already allocated index",
            ));
        }
        self.allocated_tls_indices.insert(index);
        // TlsSetValue performs only minimal validation for the guaranteed low-index range, so a
        // caller may have populated this slot before TlsAlloc formally assigns it. Allocation
        // initializes the slot to NULL for every thread.
        for thread in self.states.values_mut() {
            thread.tls_values.remove(&index);
        }
        self.next_tls_index = index + 1;
        Ok(index)
    }

    pub(super) fn tls_index_available(&self, index: u32) -> bool {
        index < RUNTIME_TLS_MINIMUM_AVAILABLE || self.allocated_tls_indices.contains(&index)
    }

    pub(super) fn require_tls_index(&self, index: u32) -> Result<(), RuntimeStateError> {
        if !self.tls_index_available(index) {
            return Err(RuntimeStateError::new(format!(
                "logical TLS index {index} is not allocated"
            )));
        }
        Ok(())
    }

    pub(super) fn allocate_fls_slot(
        &mut self,
        thread_id: RuntimeThreadId,
        callback: Option<crate::memory::GuestAddress>,
    ) -> Result<u32, RuntimeStateError> {
        if self.next_fls_index >= RUNTIME_FLS_INDEX_LIMIT {
            return Ok(FLS_OUT_OF_INDEXES);
        }
        let index = self.next_fls_index;
        if self.fls_slots.contains_key(&index) {
            return Err(RuntimeStateError::new(
                "logical FLS allocator selected an already allocated index",
            ));
        }
        let next_index = (index + 1..RUNTIME_FLS_INDEX_LIMIT)
            .find(|candidate| !self.fls_slots.contains_key(candidate))
            .unwrap_or(RUNTIME_FLS_INDEX_LIMIT);
        self.current_state(thread_id)?;
        // All fallible checks precede the slot and thread-storage mutations.
        self.states
            .get_mut(&thread_id)
            .expect("validated thread state")
            .fls_storage_initialized = true;
        self.fls_slots.insert(index, callback);
        self.next_fls_index = next_index;
        Ok(index)
    }

    pub(super) fn free_fls_slot(&mut self, index: u32) -> Result<bool, RuntimeStateError> {
        let callback = self.fls_slot(index)?;
        // A non-null callback would require running guest code for each live value.
        // Do not consume the slot before that behavior exists.
        if callback.is_some()
            && self.states.values().any(|thread| {
                thread
                    .fls_values
                    .get(&index)
                    .is_some_and(|value| *value != 0)
            })
        {
            return Ok(false);
        }
        self.fls_slots.remove(&index);
        for thread in self.states.values_mut() {
            thread.fls_values.remove(&index);
        }
        self.next_fls_index = self.next_fls_index.min(index);
        Ok(true)
    }

    pub(super) fn fls_slot(&self, index: u32) -> Result<Option<GuestAddress>, RuntimeStateError> {
        self.fls_slots.get(&index).copied().ok_or_else(|| {
            RuntimeStateError::new(format!("logical FLS index {index} is not allocated"))
        })
    }

    #[cfg(test)]
    pub(super) fn exhaust_callback_ids_for_test(&mut self) {
        self.next_callback_frame_id = u64::MAX;
    }

    pub(super) fn next_callback_frame_id(
        &self,
    ) -> Result<RuntimeCallbackFrameId, RuntimeStateError> {
        self.next_callback_frame_id
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("callback frame ID space is exhausted"))?;
        Ok(RuntimeCallbackFrameId(self.next_callback_frame_id))
    }

    pub(super) fn contains(&self, thread_id: RuntimeThreadId) -> bool {
        self.states.contains_key(&thread_id)
    }

    pub(super) fn state(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<&RuntimeThreadState, RuntimeStateError> {
        self.states.get(&thread_id).ok_or_else(|| {
            RuntimeStateError::new(format!(
                "runtime thread ID {} is not registered",
                thread_id.0
            ))
        })
    }

    pub(super) fn current_state(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<&RuntimeThreadState, RuntimeStateError> {
        self.states.get(&thread_id).ok_or_else(|| {
            RuntimeStateError::new("scheduler current thread has no runtime thread state")
        })
    }

    pub(super) fn validate_registration(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<(), RuntimeStateError> {
        if thread_id.0 == 0 {
            return Err(RuntimeStateError::new("runtime thread IDs must be nonzero"));
        }
        if self.states.contains_key(&thread_id) {
            return Err(RuntimeStateError::new(format!(
                "runtime thread ID {} is already registered",
                thread_id.0
            )));
        }
        Ok(())
    }

    pub(super) fn register(&mut self, thread_id: RuntimeThreadId, initial_last_error: u32) {
        let previous = self.states.insert(
            thread_id,
            Self::new_registration(thread_id, initial_last_error),
        );
        assert!(
            previous.is_none(),
            "thread registration must be validated before insertion"
        );
    }

    fn new_registration(thread_id: RuntimeThreadId, initial_last_error: u32) -> RuntimeThreadState {
        RuntimeThreadState {
            id: thread_id,
            lifecycle: RuntimeThreadLifecycleState::Running,
            last_error: initial_last_error,
            cursor_display_count: 0,
            com_apartment: None,
            priority: None,
            tls_values: BTreeMap::new(),
            fls_storage_initialized: false,
            fls_values: BTreeMap::new(),
            callback_frames: Vec::new(),
            stack: None,
            environment: None,
            guest_bootstrap: None,
        }
    }

    pub(super) fn next_guest_thread_id(
        &self,
    ) -> Result<Option<RuntimeThreadId>, RuntimeStateError> {
        if self.states.len() >= RUNTIME_GUEST_THREAD_LIMIT {
            return Ok(None);
        }
        let mut candidate = self.next_thread_id;
        while candidate <= u64::from(u32::MAX)
            && self.states.contains_key(&RuntimeThreadId(candidate as u32))
        {
            candidate += 1;
        }
        let Ok(thread_id) = u32::try_from(candidate).map(RuntimeThreadId) else {
            return Ok(None);
        };
        self.validate_registration(thread_id)?;
        Ok(Some(thread_id))
    }

    pub(super) fn validate_guest_registration(
        &self,
        thread_id: RuntimeThreadId,
        stack: &RuntimeThreadStack,
        environment: &RuntimeThreadEnvironment,
        bootstrap: &RuntimeGuestThreadBootstrap,
    ) -> Result<(), RuntimeStateError> {
        self.validate_registration(thread_id)?;
        if stack.thread_id != thread_id
            || environment.thread_id != thread_id
            || bootstrap.thread_id != thread_id
            || bootstrap.initial_stack_pointer != stack.initial_stack_pointer
            || bootstrap.return_sentinel != stack.return_sentinel
            || bootstrap.fs_base != environment.teb_address
        {
            return Err(RuntimeStateError::new(
                "guest thread stack, TEB, and execution bootstrap identities must agree",
            ));
        }
        Ok(())
    }

    pub(super) fn register_guest(
        &mut self,
        thread_id: RuntimeThreadId,
        initial_last_error: u32,
        stack: RuntimeThreadStack,
        environment: RuntimeThreadEnvironment,
        bootstrap: RuntimeGuestThreadBootstrap,
    ) {
        let mut state = Self::new_registration(thread_id, initial_last_error);
        state.stack = Some(stack);
        state.environment = Some(environment);
        state.guest_bootstrap = Some(bootstrap);
        self.next_thread_id = u64::from(thread_id.0) + 1;
        let previous = self.states.insert(thread_id, state);
        assert!(
            previous.is_none(),
            "guest thread registration must be validated before insertion"
        );
    }

    pub(super) fn validate_start(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<(), RuntimeStateError> {
        if self.current_state(thread_id)?.lifecycle != RuntimeThreadLifecycleState::Initialized {
            return Err(RuntimeStateError::new(
                "initial thread start requires initialized thread state",
            ));
        }
        Ok(())
    }

    pub(super) fn start(&mut self, thread_id: RuntimeThreadId) -> Result<(), RuntimeStateError> {
        self.validate_start(thread_id)?;
        self.states
            .get_mut(&thread_id)
            .expect("validated initial thread")
            .lifecycle = RuntimeThreadLifecycleState::Running;
        Ok(())
    }

    pub(super) fn register_stack(
        &mut self,
        thread_id: RuntimeThreadId,
        stack: RuntimeThreadStack,
    ) -> Result<(), RuntimeStateError> {
        if self.state(thread_id)?.stack.is_some() {
            return Err(RuntimeStateError::new(
                "runtime thread stack may be registered only once",
            ));
        }
        if stack.thread_id != thread_id {
            return Err(RuntimeStateError::new(
                "runtime thread stack identity does not match its owner thread",
            ));
        }
        self.states
            .get_mut(&thread_id)
            .expect("validated stack owner")
            .stack = Some(stack);
        Ok(())
    }

    pub(super) fn stack_mut(
        &mut self,
        thread_id: RuntimeThreadId,
    ) -> Result<&mut RuntimeThreadStack, RuntimeStateError> {
        let thread = self.states.get_mut(&thread_id).ok_or_else(|| {
            RuntimeStateError::new(format!(
                "runtime thread ID {} is not registered",
                thread_id.0
            ))
        })?;
        let stack = thread.stack.as_mut().ok_or_else(|| {
            RuntimeStateError::new(format!(
                "runtime thread ID {} has no registered stack",
                thread_id.0
            ))
        })?;
        if stack.thread_id != thread_id {
            return Err(RuntimeStateError::new(
                "runtime thread stack identity does not match its owner thread",
            ));
        }
        Ok(stack)
    }

    pub(super) fn last_error(&self, thread_id: RuntimeThreadId) -> Result<u32, RuntimeStateError> {
        Ok(self.current_state(thread_id)?.last_error)
    }

    pub(super) fn last_error_mut(
        &mut self,
        thread_id: RuntimeThreadId,
    ) -> Result<&mut u32, RuntimeStateError> {
        self.current_state(thread_id)?;
        Ok(&mut self
            .states
            .get_mut(&thread_id)
            .expect("validated current thread")
            .last_error)
    }

    pub(super) fn set_last_error(
        &mut self,
        thread_id: RuntimeThreadId,
        value: u32,
    ) -> Result<(), RuntimeStateError> {
        self.current_state(thread_id)?;
        self.states
            .get_mut(&thread_id)
            .expect("validated current thread")
            .last_error = value;
        Ok(())
    }

    pub(super) fn com_apartment(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<Option<&RuntimeComApartmentState>, RuntimeStateError> {
        Ok(self.current_state(thread_id)?.com_apartment.as_ref())
    }

    pub(super) fn initialize_com_apartment(
        &mut self,
        thread_id: RuntimeThreadId,
        requested_model: RuntimeComApartmentModel,
    ) -> Result<ComHResult, RuntimeStateError> {
        let previous = self.current_state(thread_id)?;
        if previous.lifecycle != RuntimeThreadLifecycleState::Running {
            return Err(RuntimeStateError::new(
                "COM initialization requires a running runtime thread",
            ));
        }
        let (apartment, result) = match previous.com_apartment {
            None => {
                let apartment = RuntimeComApartmentState {
                    model: requested_model,
                    initialization_count: 1,
                };
                (apartment, COM_S_OK)
            }
            Some(apartment) if apartment.model == requested_model => {
                let initialization_count = apartment
                    .initialization_count
                    .checked_add(1)
                    .ok_or_else(|| {
                        RuntimeStateError::new("COM apartment initialization count is exhausted")
                    })?;
                let apartment = RuntimeComApartmentState {
                    model: requested_model,
                    initialization_count,
                };
                (apartment, COM_S_FALSE)
            }
            Some(_) => return Ok(COM_RPC_E_CHANGED_MODE),
        };
        self.states
            .get_mut(&thread_id)
            .expect("validated current thread")
            .com_apartment = Some(apartment);
        Ok(result)
    }

    pub(super) fn effective_priority(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeThreadPriority, RuntimeStateError> {
        Ok(self
            .current_state(thread_id)?
            .priority
            .unwrap_or(RuntimeThreadPriority::Normal))
    }

    pub(super) fn set_priority(
        &mut self,
        thread_id: RuntimeThreadId,
        requested_priority: RuntimeThreadPriority,
    ) -> Result<bool, RuntimeStateError> {
        let previous = self.current_state(thread_id)?;
        if previous.lifecycle != RuntimeThreadLifecycleState::Running {
            return Ok(false);
        }
        self.states
            .get_mut(&thread_id)
            .expect("validated current thread")
            .priority = Some(requested_priority);
        Ok(true)
    }

    pub(super) fn cursor_display_count(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<i32, RuntimeStateError> {
        Ok(self.current_state(thread_id)?.cursor_display_count)
    }

    pub(super) fn change_cursor_display_count(
        &mut self,
        thread_id: RuntimeThreadId,
        show: bool,
    ) -> Result<i32, RuntimeStateError> {
        let previous = self.current_state(thread_id)?;
        if previous.lifecycle != RuntimeThreadLifecycleState::Running {
            return Err(RuntimeStateError::new(
                "cursor display count can change only for a running runtime thread",
            ));
        }
        let next_count = if show {
            previous.cursor_display_count.checked_add(1)
        } else {
            previous.cursor_display_count.checked_sub(1)
        }
        .ok_or_else(|| {
            RuntimeStateError::new(
                "logical cursor display counter exceeds the signed Win32 INT range",
            )
        })?;
        self.states
            .get_mut(&thread_id)
            .expect("validated current thread")
            .cursor_display_count = next_count;
        Ok(next_count)
    }

    pub(super) fn set_fls_value(
        &mut self,
        thread_id: RuntimeThreadId,
        index: u32,
        value: u32,
    ) -> Result<(), RuntimeStateError> {
        self.current_state(thread_id)?;
        let thread = self
            .states
            .get_mut(&thread_id)
            .expect("validated thread state");
        thread.fls_storage_initialized = true;
        thread.fls_values.insert(index, value);
        Ok(())
    }

    pub(super) fn fls_value_or_unset(
        &self,
        thread_id: RuntimeThreadId,
        index: u32,
    ) -> Result<Option<u32>, RuntimeStateError> {
        let thread = self.current_state(thread_id)?;
        // A thread that has not stored an FLS value reads the allocated slot as NULL.
        Ok(thread.fls_values.get(&index).copied())
    }

    pub(super) fn set_tls_value(
        &mut self,
        thread_id: RuntimeThreadId,
        index: u32,
        value: u32,
    ) -> Result<(), RuntimeStateError> {
        self.current_state(thread_id)?;
        self.states
            .get_mut(&thread_id)
            .expect("validated current thread")
            .tls_values
            .insert(index, value);
        Ok(())
    }

    pub(super) fn tls_value_or_unset(
        &self,
        thread_id: RuntimeThreadId,
        index: u32,
    ) -> Result<Option<u32>, RuntimeStateError> {
        Ok(self
            .current_state(thread_id)?
            .tls_values
            .get(&index)
            .copied())
    }

    pub(super) fn callback_depth(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<usize, RuntimeStateError> {
        Ok(self.current_state(thread_id)?.callback_frames.len())
    }

    pub(super) fn validate_callback_entry(
        &self,
        thread_id: RuntimeThreadId,
        replacing: Option<RuntimeCallbackFrameId>,
    ) -> Result<usize, RuntimeStateError> {
        let depth = self.callback_depth(thread_id)?;
        if let Some(expected) = replacing {
            self.callback_frame_for_return(thread_id, expected)?;
            return Ok(depth);
        }
        depth
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("callback depth exceeds the host range"))
    }

    pub(super) fn callback_frame_for_return(
        &self,
        thread_id: RuntimeThreadId,
        expected: RuntimeCallbackFrameId,
    ) -> Result<RuntimeCallbackFrame, RuntimeStateError> {
        let thread = self.current_state(thread_id)?;
        let frame = thread.callback_frames.last().copied().ok_or_else(|| {
            RuntimeStateError::new("current thread has no callback frame to return")
        })?;
        if frame.id != expected {
            return Err(RuntimeStateError::new(
                "callback frame return is not strict LIFO order",
            ));
        }
        Ok(frame)
    }

    pub(super) fn publish_callback_frame(
        &mut self,
        frame: RuntimeCallbackFrame,
        replacing: bool,
    ) -> Option<RuntimeCallbackFrame> {
        // Frame identity and entry were checked immediately before the caller's
        // synchronous memory writes. Only publication consumes the next ID.
        self.next_callback_frame_id = frame.id.0 + 1;
        let thread = self
            .states
            .get_mut(&frame.thread_id)
            .expect("validated callback thread");
        if replacing {
            Some(std::mem::replace(
                thread
                    .callback_frames
                    .last_mut()
                    .expect("validated callback frame"),
                frame,
            ))
        } else {
            thread.callback_frames.push(frame);
            None
        }
    }

    pub(super) fn pop_callback_frame(
        &mut self,
        thread_id: RuntimeThreadId,
        expected: RuntimeCallbackFrameId,
    ) -> Result<RuntimeCallbackFrame, RuntimeStateError> {
        let frame = self.callback_frame_for_return(thread_id, expected)?;
        self.states
            .get_mut(&thread_id)
            .expect("validated callback thread")
            .callback_frames
            .pop();
        Ok(frame)
    }

    pub(super) fn validate_exit_request(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<(), RuntimeStateError> {
        let thread = self.state(thread_id)?;
        if thread.lifecycle != RuntimeThreadLifecycleState::Running {
            return Err(RuntimeStateError::new(
                "thread exit request requires a running thread",
            ));
        }
        Ok(())
    }

    pub(super) fn request_exit(
        &mut self,
        thread_id: RuntimeThreadId,
        exit_code: u32,
    ) -> Result<(), RuntimeStateError> {
        self.validate_exit_request(thread_id)?;
        let thread = self
            .states
            .get_mut(&thread_id)
            .expect("validated thread disappeared");
        thread.callback_frames.clear();
        thread.lifecycle = RuntimeThreadLifecycleState::ExitRequested { exit_code };
        Ok(())
    }

    pub(super) fn validate_exit_completion(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<u32, RuntimeStateError> {
        match self.state(thread_id)?.lifecycle {
            RuntimeThreadLifecycleState::ExitRequested { exit_code } => Ok(exit_code),
            _ => Err(RuntimeStateError::new(
                "thread exit completion requires a prior exit request",
            )),
        }
    }

    pub(super) fn complete_exit(
        &mut self,
        thread_id: RuntimeThreadId,
    ) -> Result<u32, RuntimeStateError> {
        let exit_code = self.validate_exit_completion(thread_id)?;
        let thread = self
            .states
            .get_mut(&thread_id)
            .expect("validated thread disappeared");
        thread.lifecycle = RuntimeThreadLifecycleState::Exited { exit_code };
        thread.com_apartment = None;
        thread.cursor_display_count = 0;
        Ok(exit_code)
    }
}

#[cfg(test)]
mod tests {
    use crate::memory::GuestAddress;

    use super::*;

    const INITIAL: RuntimeThreadId = RuntimeThreadId(0x2000);
    const SECOND: RuntimeThreadId = RuntimeThreadId(0x1001);

    #[test]
    fn com_apartment_initialization_is_counted_thread_local_and_failure_atomic() {
        let mut threads = RuntimeThreads::new(INITIAL, 11);
        threads.start(INITIAL).unwrap();
        threads.validate_registration(SECOND).unwrap();
        threads.register(SECOND, 33);

        assert_eq!(threads.com_apartment(INITIAL), Ok(None));
        let first = threads
            .initialize_com_apartment(INITIAL, RuntimeComApartmentModel::SingleThreaded)
            .unwrap();
        assert_eq!(first, COM_S_OK);
        assert_eq!(
            threads
                .com_apartment(INITIAL)
                .unwrap()
                .unwrap()
                .initialization_count,
            1
        );

        let repeat = threads
            .initialize_com_apartment(INITIAL, RuntimeComApartmentModel::SingleThreaded)
            .unwrap();
        assert_eq!(repeat, COM_S_FALSE);
        assert_eq!(
            threads
                .com_apartment(INITIAL)
                .unwrap()
                .unwrap()
                .initialization_count,
            2
        );

        let before_conflict = threads.clone();
        let conflict = threads
            .initialize_com_apartment(INITIAL, RuntimeComApartmentModel::MultiThreaded)
            .unwrap();
        assert_eq!(conflict, COM_RPC_E_CHANGED_MODE);
        assert_eq!(threads, before_conflict);

        threads
            .initialize_com_apartment(SECOND, RuntimeComApartmentModel::MultiThreaded)
            .unwrap();
        assert_eq!(
            threads.com_apartment(SECOND).unwrap().unwrap().model,
            RuntimeComApartmentModel::MultiThreaded
        );
        assert_eq!(
            threads.com_apartment(INITIAL).unwrap().unwrap().model,
            RuntimeComApartmentModel::SingleThreaded
        );

        threads.set_last_error(INITIAL, 12).unwrap();
        assert_eq!(
            threads
                .com_apartment(INITIAL)
                .unwrap()
                .unwrap()
                .initialization_count,
            2
        );
        assert_eq!(
            threads
                .com_apartment(SECOND)
                .unwrap()
                .unwrap()
                .initialization_count,
            1
        );
    }

    #[test]
    fn com_apartment_overflow_and_thread_exit_do_not_leak_state() {
        let mut threads = RuntimeThreads::new(INITIAL, 11);
        threads.start(INITIAL).unwrap();
        threads.states.get_mut(&INITIAL).unwrap().com_apartment = Some(RuntimeComApartmentState {
            model: RuntimeComApartmentModel::SingleThreaded,
            initialization_count: u32::MAX,
        });
        let before_overflow = threads.clone();
        threads
            .initialize_com_apartment(INITIAL, RuntimeComApartmentModel::SingleThreaded)
            .unwrap_err();
        assert_eq!(threads, before_overflow);

        threads.request_exit(INITIAL, 23).unwrap();
        threads.complete_exit(INITIAL).unwrap();
        assert_eq!(threads.state(INITIAL).unwrap().com_apartment, None);
        threads
            .initialize_com_apartment(INITIAL, RuntimeComApartmentModel::SingleThreaded)
            .unwrap_err();
    }

    #[test]
    fn thread_local_values_remain_isolated() {
        let mut threads = RuntimeThreads::new(INITIAL, 11);
        let before_start = threads.clone();
        threads.start(INITIAL).unwrap();
        assert_ne!(threads, before_start);
        let running = threads.clone();
        assert!(threads.start(INITIAL).is_err());
        assert_eq!(threads, running);

        threads.validate_registration(SECOND).unwrap();
        assert!(!threads.contains(SECOND));
        threads.register(SECOND, 33);
        threads.set_tls_value(INITIAL, 0, 0x1111).unwrap();
        threads.set_tls_value(SECOND, 0, 0x2222).unwrap();

        assert_eq!(threads.last_error(INITIAL), Ok(11));
        assert_eq!(threads.last_error(SECOND), Ok(33));
        assert_eq!(
            threads.tls_value_or_unset(INITIAL, 0).unwrap().unwrap(),
            0x1111
        );
        assert_eq!(
            threads.tls_value_or_unset(SECOND, 0).unwrap().unwrap(),
            0x2222
        );
    }

    #[test]
    fn priority_uses_normal_default_and_is_thread_local() {
        let mut threads = RuntimeThreads::new(INITIAL, 11);
        threads.start(INITIAL).unwrap();
        threads.validate_registration(SECOND).unwrap();
        threads.register(SECOND, 33);

        assert_eq!(
            threads.effective_priority(INITIAL),
            Ok(RuntimeThreadPriority::Normal)
        );
        assert_eq!(
            threads.effective_priority(SECOND),
            Ok(RuntimeThreadPriority::Normal)
        );
        assert_eq!(
            threads.set_priority(INITIAL, RuntimeThreadPriority::AboveNormal),
            Ok(true)
        );
        assert_eq!(
            threads.effective_priority(INITIAL),
            Ok(RuntimeThreadPriority::AboveNormal)
        );
        assert_eq!(
            threads.effective_priority(SECOND),
            Ok(RuntimeThreadPriority::Normal)
        );

        assert_eq!(
            threads.set_priority(INITIAL, RuntimeThreadPriority::Normal),
            Ok(true)
        );
        assert_eq!(
            threads.effective_priority(INITIAL),
            Ok(RuntimeThreadPriority::Normal)
        );
    }

    #[test]
    fn cursor_display_count_is_signed_atomic_thread_local_and_terminal() {
        let mut threads = RuntimeThreads::new(INITIAL, 11);
        threads.start(INITIAL).unwrap();
        threads.validate_registration(SECOND).unwrap();
        threads.register(SECOND, 33);

        assert_eq!(threads.cursor_display_count(INITIAL), Ok(0));
        assert_eq!(threads.cursor_display_count(SECOND), Ok(0));
        assert_eq!(threads.change_cursor_display_count(INITIAL, true), Ok(1));
        assert_eq!(threads.cursor_display_count(INITIAL), Ok(1));
        assert_eq!(threads.cursor_display_count(SECOND), Ok(0));

        assert_eq!(threads.change_cursor_display_count(SECOND, false), Ok(-1));
        assert_eq!(threads.cursor_display_count(INITIAL), Ok(1));
        assert_eq!(threads.cursor_display_count(SECOND), Ok(-1));

        threads
            .states
            .get_mut(&INITIAL)
            .unwrap()
            .cursor_display_count = i32::MAX;
        let before_overflow = threads.clone();
        threads
            .change_cursor_display_count(INITIAL, true)
            .unwrap_err();
        assert_eq!(threads, before_overflow);

        threads
            .states
            .get_mut(&INITIAL)
            .unwrap()
            .cursor_display_count = i32::MIN;
        let before_underflow = threads.clone();
        threads
            .change_cursor_display_count(INITIAL, false)
            .unwrap_err();
        assert_eq!(threads, before_underflow);

        threads.request_exit(SECOND, 23).unwrap();
        threads.complete_exit(SECOND).unwrap();
        assert_eq!(threads.cursor_display_count(SECOND), Ok(0));
        threads
            .change_cursor_display_count(SECOND, true)
            .unwrap_err();
    }

    #[test]
    fn direct_exit_rejects_invalid_transitions_without_mutating_threads() {
        let mut threads = RuntimeThreads::new(INITIAL, 7);
        let initialized = threads.clone();
        for id in [INITIAL, SECOND] {
            assert!(threads.request_exit(id, 23).is_err());
            assert!(threads.complete_exit(id).is_err());
            assert_eq!(threads, initialized);
        }
        threads.start(INITIAL).unwrap();
        let running = threads.clone();
        assert!(threads.complete_exit(INITIAL).is_err());
        assert_eq!(threads, running);
        threads.request_exit(INITIAL, 23).unwrap();
        let requested = threads.clone();
        assert!(threads.request_exit(INITIAL, 99).is_err());
        assert_eq!(threads, requested);
        assert_eq!(threads.complete_exit(INITIAL).unwrap(), 23);
        let exited = threads.clone();
        assert!(threads.request_exit(INITIAL, 99).is_err());
        assert!(threads.complete_exit(INITIAL).is_err());
        assert_eq!(threads, exited);
        assert_eq!(threads.last_error(INITIAL).unwrap(), 7);
    }

    #[test]
    fn callback_handoffs_and_direct_exit_keep_explicit_order() {
        let mut threads = RuntimeThreads::new(INITIAL, 7);
        threads.start(INITIAL).unwrap();
        let frame = RuntimeCallbackFrame {
            id: RuntimeCallbackFrameId(1),
            thread_id: INITIAL,
            target: GuestAddress(0x1000_1000),
            return_sentinel: GuestAddress(0x7f01_0000),
            entry_last_error: 7,
            scratch: None,
        };
        let depth = threads.validate_callback_entry(INITIAL, None).unwrap();
        assert_eq!(depth, 1);
        assert_eq!(threads.callback_depth(INITIAL), Ok(0));
        threads.publish_callback_frame(frame, false);

        let replacement = RuntimeCallbackFrame {
            id: RuntimeCallbackFrameId(2),
            target: GuestAddress(0x1000_2000),
            ..frame
        };
        let depth = threads
            .validate_callback_entry(INITIAL, Some(frame.id))
            .unwrap();
        assert_eq!(depth, 1);
        assert_eq!(
            threads.state(INITIAL).unwrap().callback_frames,
            [frame],
            "entry validation preserves the active callback"
        );
        assert_eq!(
            threads.publish_callback_frame(replacement, true),
            Some(frame)
        );
        assert_eq!(
            threads.state(INITIAL).unwrap().callback_frames,
            [replacement]
        );

        threads.validate_exit_request(INITIAL).unwrap();
        assert_eq!(threads.state(INITIAL).unwrap().callback_frames.len(), 1);
        threads.request_exit(INITIAL, 23).unwrap();
        assert!(threads.state(INITIAL).unwrap().callback_frames.is_empty());
        assert_eq!(threads.complete_exit(INITIAL).unwrap(), 23);
        assert_eq!(
            threads.state(INITIAL).unwrap().lifecycle,
            RuntimeThreadLifecycleState::Exited { exit_code: 23 }
        );
    }

    #[test]
    fn callback_stack_transitions_preserve_tls_fls_and_last_error() {
        let mut threads = RuntimeThreads::new(INITIAL, 37);
        threads.start(INITIAL).unwrap();
        let tls = threads.allocate_tls_index().unwrap();
        let fls = threads.allocate_fls_slot(INITIAL, None).unwrap();
        threads.set_tls_value(INITIAL, tls, 0x1234).unwrap();
        threads.set_fls_value(INITIAL, fls, 0x5678).unwrap();
        let assert_values = |threads: &RuntimeThreads| {
            assert_eq!(
                threads.tls_value_or_unset(INITIAL, tls).unwrap(),
                Some(0x1234)
            );
            assert_eq!(
                threads.fls_value_or_unset(INITIAL, fls).unwrap(),
                Some(0x5678)
            );
            assert_eq!(threads.last_error(INITIAL).unwrap(), 37);
            assert_eq!(
                threads.state(INITIAL).unwrap().lifecycle,
                RuntimeThreadLifecycleState::Running
            );
        };
        let frame = RuntimeCallbackFrame {
            id: RuntimeCallbackFrameId(1),
            thread_id: INITIAL,
            target: GuestAddress(0x1000_1000),
            return_sentinel: GuestAddress(0x7f01_0000),
            entry_last_error: 37,
            scratch: None,
        };
        let depth = threads.validate_callback_entry(INITIAL, None).unwrap();
        assert_eq!(depth, 1);
        threads.publish_callback_frame(frame, false);
        assert_values(&threads);
        assert!(
            threads
                .pop_callback_frame(INITIAL, RuntimeCallbackFrameId(99))
                .is_err()
        );
        assert_eq!(threads.state(INITIAL).unwrap().callback_frames, [frame]);
        assert_values(&threads);
        let replacement = RuntimeCallbackFrame {
            id: RuntimeCallbackFrameId(2),
            ..frame
        };
        let depth = threads
            .validate_callback_entry(INITIAL, Some(frame.id))
            .unwrap();
        assert_eq!(depth, 1);
        assert_eq!(
            threads.publish_callback_frame(replacement, true),
            Some(frame)
        );
        assert_values(&threads);
        let returned = threads.pop_callback_frame(INITIAL, replacement.id).unwrap();
        assert_eq!(returned, replacement);
        assert!(threads.state(INITIAL).unwrap().callback_frames.is_empty());
        assert_values(&threads);
    }

    #[test]
    fn stack_access_and_registration_preserve_other_thread_state() {
        let mut threads = RuntimeThreads::new(INITIAL, 37);
        let stack = RuntimeThreadStack {
            thread_id: INITIAL,
            address_allocation_id: ProcessAddressAllocationId(1),
            memory_owner: MemoryOwnerId(1),
            reserved_range: GuestRange::new(GuestAddress(0x1000), 0x2000).unwrap(),
            committed_range: GuestRange::new(GuestAddress(0x2000), 0x1000).unwrap(),
            initial_commit_bytes: 0x1000,
            guard_page: Some(GuestRange::new(GuestAddress(0x1000), 0x1000).unwrap()),
            stack_top: GuestAddress(0x3000),
            initial_stack_pointer: GuestAddress(0x2ffc),
            return_sentinel: GuestAddress(0x4000),
        };
        let mut wrong_owner = stack.clone();
        wrong_owner.thread_id = SECOND;
        assert!(threads.register_stack(INITIAL, wrong_owner).is_err());
        assert!(threads.state(INITIAL).unwrap().stack.is_none());
        assert!(threads.stack_mut(INITIAL).is_err());
        assert!(threads.stack_mut(SECOND).is_err());
        assert_eq!(threads.last_error(INITIAL).unwrap(), 37);
        threads.register_stack(INITIAL, stack.clone()).unwrap();
        assert!(threads.register_stack(INITIAL, stack.clone()).is_err());
        assert_eq!(threads.state(INITIAL).unwrap().stack.as_ref(), Some(&stack));
        {
            let live = threads.stack_mut(INITIAL).unwrap();
            live.committed_range = live.reserved_range;
            live.guard_page = None;
        }
        let grown = threads.state(INITIAL).unwrap().stack.as_ref().unwrap();
        assert_eq!(grown.thread_id, stack.thread_id);
        assert_eq!(grown.address_allocation_id, stack.address_allocation_id);
        assert_eq!(grown.memory_owner, stack.memory_owner);
        assert_eq!(grown.reserved_range, stack.reserved_range);
        assert_eq!(grown.committed_range, stack.reserved_range);
        assert_eq!(grown.initial_commit_bytes, stack.initial_commit_bytes);
        assert_eq!(grown.guard_page, None);
        assert_eq!(threads.last_error(INITIAL).unwrap(), 37);
        threads.stack_mut(INITIAL).unwrap().thread_id = SECOND;
        assert!(threads.stack_mut(INITIAL).is_err());
        assert_eq!(threads.last_error(INITIAL).unwrap(), 37);
        let stack = threads
            .states
            .get_mut(&INITIAL)
            .unwrap()
            .stack
            .as_mut()
            .unwrap();
        assert_eq!(stack.thread_id, SECOND);
        assert_eq!(stack.committed_range, stack.reserved_range);
        stack.thread_id = INITIAL;
        assert_eq!(threads.stack_mut(INITIAL).unwrap().thread_id, INITIAL);
    }

    #[test]
    fn guest_thread_identity_selection_checks_bounds_and_capacity_without_mutation() {
        let mut threads = RuntimeThreads::new(INITIAL, 0);
        threads.next_thread_id = u64::from(INITIAL.0);
        let before = threads.clone();
        assert_eq!(
            threads.next_guest_thread_id().unwrap(),
            Some(RuntimeThreadId(INITIAL.0 + 1))
        );
        assert_eq!(threads, before);

        threads.next_thread_id = u64::from(u32::MAX);
        let before = threads.clone();
        assert_eq!(
            threads.next_guest_thread_id().unwrap(),
            Some(RuntimeThreadId(u32::MAX))
        );
        assert_eq!(threads, before);
        threads.register(RuntimeThreadId(u32::MAX), 0);
        let before = threads.clone();
        assert!(threads.next_guest_thread_id().unwrap().is_none());
        assert_eq!(threads, before);

        let mut threads = RuntimeThreads::new(INITIAL, 0);
        for id in 1..RUNTIME_GUEST_THREAD_LIMIT as u32 {
            threads.validate_registration(RuntimeThreadId(id)).unwrap();
            threads.register(RuntimeThreadId(id), 0);
        }
        let before = threads.clone();
        assert!(threads.next_guest_thread_id().unwrap().is_none());
        assert_eq!(threads, before);
    }

    #[test]
    fn rejected_registration_preserves_the_registry_and_allocator_identity() {
        let threads = RuntimeThreads::new(INITIAL, 0);
        let before = threads.clone();
        threads.validate_registration(INITIAL).unwrap_err();
        threads
            .validate_registration(RuntimeThreadId(0))
            .unwrap_err();
        assert_eq!(threads, before);
    }

    #[test]
    fn resource_allocators_publish_with_their_owned_thread_state() {
        let mut threads = RuntimeThreads::new(INITIAL, 7);
        threads.start(INITIAL).unwrap();

        threads.set_tls_value(INITIAL, 0, 0x1234).unwrap();
        assert!(threads.state(INITIAL).unwrap().tls_values.contains_key(&0));
        assert!(threads.allocated_tls_indices().is_empty());
        assert_eq!(threads.next_tls_index(), 0);
        assert_eq!(threads.allocate_tls_index(), Ok(0));
        assert_eq!(threads.allocated_tls_indices(), &BTreeSet::from([0]));
        assert_eq!(threads.next_tls_index(), 1);
        assert!(!threads.state(INITIAL).unwrap().tls_values.contains_key(&0));

        assert!(threads.fls_slots.is_empty());
        assert!(!threads.state(INITIAL).unwrap().fls_storage_initialized);
        let slot = threads
            .allocate_fls_slot(INITIAL, Some(GuestAddress(0x1000_2000)))
            .unwrap();
        assert_eq!(slot, 0);
        assert_eq!(
            threads.fls_slot(slot).unwrap(),
            Some(GuestAddress(0x1000_2000))
        );
        assert!(threads.state(INITIAL).unwrap().fls_storage_initialized);
        assert_eq!(threads.next_fls_index(), 1);

        let callback_id = threads.next_callback_frame_id().unwrap();
        let frame = RuntimeCallbackFrame {
            id: callback_id,
            thread_id: INITIAL,
            target: GuestAddress(0x1000_3000),
            return_sentinel: GuestAddress(0x7f01_0000),
            entry_last_error: 7,
            scratch: None,
        };
        let depth = threads.validate_callback_entry(INITIAL, None).unwrap();
        assert_eq!(depth, 1);
        assert_eq!(
            threads.next_callback_frame_id().unwrap(),
            RuntimeCallbackFrameId(1)
        );
        assert!(threads.state(INITIAL).unwrap().callback_frames.is_empty());
        threads.publish_callback_frame(frame, false);
        assert_eq!(
            threads.next_callback_frame_id().unwrap(),
            RuntimeCallbackFrameId(2)
        );
        assert_eq!(threads.state(INITIAL).unwrap().callback_frames, [frame]);
    }

    #[test]
    fn fls_free_discards_thread_values_and_reuses_the_index() {
        let mut threads = RuntimeThreads::new(INITIAL, 0);
        assert_eq!(
            threads
                .allocate_fls_slot(INITIAL, Some(GuestAddress(0x1000_2000)))
                .unwrap(),
            0
        );
        threads.set_fls_value(INITIAL, 0, 42).unwrap();
        let before = threads.clone();
        assert!(
            !threads.free_fls_slot(0).unwrap(),
            "callback requires guest execution"
        );
        assert_eq!(threads, before);
        assert_eq!(threads.allocate_fls_slot(INITIAL, None).unwrap(), 1);
        threads.set_fls_value(INITIAL, 1, 99).unwrap();
        assert!(threads.free_fls_slot(1).unwrap());
        assert_eq!(threads.allocate_fls_slot(INITIAL, None).unwrap(), 1);
        assert_eq!(threads.fls_value_or_unset(INITIAL, 1).unwrap(), None);
        threads.set_fls_value(INITIAL, 0, 0).unwrap();
        assert!(threads.free_fls_slot(0).unwrap());
        assert_eq!(threads.allocate_fls_slot(INITIAL, None).unwrap(), 0);
        assert_eq!(threads.fls_value_or_unset(INITIAL, 0).unwrap(), None);
    }

    #[test]
    fn fls_allocation_skips_live_slots_after_nonadjacent_frees() {
        let mut threads = RuntimeThreads::new(INITIAL, 0);
        for index in 0..RUNTIME_FLS_INDEX_LIMIT {
            assert_eq!(threads.allocate_fls_slot(INITIAL, None).unwrap(), index);
            threads.set_fls_value(INITIAL, index, index + 1).unwrap();
        }
        assert!(threads.free_fls_slot(0).unwrap());
        assert!(threads.free_fls_slot(3).unwrap());
        assert_eq!(threads.allocate_fls_slot(INITIAL, None).unwrap(), 0);
        assert_eq!(threads.allocate_fls_slot(INITIAL, None).unwrap(), 3);
        assert_eq!(
            threads.allocate_fls_slot(INITIAL, None).unwrap(),
            FLS_OUT_OF_INDEXES
        );
        assert_eq!(threads.fls_slots.len(), RUNTIME_FLS_INDEX_LIMIT as usize);
        assert_eq!(threads.fls_value_or_unset(INITIAL, 0).unwrap(), None);
        assert_eq!(threads.fls_value_or_unset(INITIAL, 3).unwrap(), None);
        for index in 1..RUNTIME_FLS_INDEX_LIMIT {
            if index != 3 {
                assert_eq!(
                    threads.fls_value_or_unset(INITIAL, index).unwrap(),
                    Some(index + 1)
                );
            }
        }
    }

    #[test]
    fn tls_capacity_failure_preserves_allocated_indices_and_thread_values() {
        let mut threads = RuntimeThreads::new(INITIAL, 0);
        threads.validate_registration(SECOND).unwrap();
        threads.register(SECOND, 0);
        for index in 0..TLS_INDEX_LIMIT {
            assert_eq!(threads.allocate_tls_index().unwrap(), index);
        }
        threads
            .set_tls_value(INITIAL, TLS_INDEX_LIMIT - 1, 0x1234)
            .unwrap();
        threads
            .set_tls_value(SECOND, TLS_INDEX_LIMIT - 1, 0x5678)
            .unwrap();
        for _ in 0..2 {
            assert_eq!(threads.allocate_tls_index().unwrap(), TLS_OUT_OF_INDEXES);
            assert_eq!(
                threads.allocated_tls_indices.len(),
                TLS_INDEX_LIMIT as usize
            );
            assert_eq!(threads.next_tls_index, TLS_INDEX_LIMIT);
            assert_eq!(
                threads
                    .tls_value_or_unset(INITIAL, TLS_INDEX_LIMIT - 1)
                    .unwrap(),
                Some(0x1234)
            );
            assert_eq!(
                threads
                    .tls_value_or_unset(SECOND, TLS_INDEX_LIMIT - 1)
                    .unwrap(),
                Some(0x5678)
            );
        }
        assert!(threads.require_tls_index(TLS_INDEX_LIMIT).is_err());
    }

    #[test]
    fn resource_allocator_rejections_preserve_all_owned_state() {
        let mut threads = RuntimeThreads::new(INITIAL, 0);
        threads.next_tls_index = TLS_OUT_OF_INDEXES;
        let before_tls_exhaustion = threads.clone();
        assert_eq!(threads.allocate_tls_index().unwrap(), TLS_OUT_OF_INDEXES);
        assert_eq!(threads, before_tls_exhaustion);

        threads.next_fls_index = RUNTIME_FLS_INDEX_LIMIT;
        let before_fls_exhaustion = threads.clone();
        assert_eq!(
            threads.allocate_fls_slot(INITIAL, None).unwrap(),
            FLS_OUT_OF_INDEXES
        );
        assert_eq!(threads, before_fls_exhaustion);

        threads.next_callback_frame_id = u64::MAX;
        let before_callback_exhaustion = threads.clone();
        threads.next_callback_frame_id().unwrap_err();
        assert_eq!(threads, before_callback_exhaustion);
    }
}

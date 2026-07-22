use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::{ProcessAddressAllocationId, RuntimeStateError};
use crate::memory::{GuestAddress, GuestRange, GuestWriteResult, MemoryOwnerId};

pub const LOGICAL_TLS_INDEX_NAMESPACE: &str = "vine.logical-tls-index.v1";
pub const LOGICAL_FLS_INDEX_NAMESPACE: &str = "vine.logical-fls-index.v1";
pub const TLS_OUT_OF_INDEXES: u32 = u32::MAX;
pub const FLS_OUT_OF_INDEXES: u32 = u32::MAX;
pub const RUNTIME_FLS_INDEX_LIMIT: u32 = 128;
pub const RUNTIME_GUEST_THREAD_LIMIT: usize = 256;
pub const RUNTIME_GUEST_THREAD_STACK_ARENA_BASE: GuestAddress = GuestAddress(0x5000_0000);
pub const RUNTIME_GUEST_THREAD_STACK_ARENA_SIZE: u64 = 0x1000_0000;
pub const RUNTIME_GUEST_THREAD_ENVIRONMENT_ARENA_BASE: GuestAddress = GuestAddress(0x7f03_0000);
pub const RUNTIME_GUEST_THREAD_ENVIRONMENT_ARENA_SIZE: u64 = 0x00c0_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeComApartmentModel {
    SingleThreaded,
    MultiThreaded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeComApartmentState {
    pub model: RuntimeComApartmentModel,
    pub initialization_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeComInitializationDisposition {
    Initialized,
    AlreadyInitialized,
    ChangedMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeComInitializationOutcome {
    pub thread_id: RuntimeThreadId,
    pub requested_model: RuntimeComApartmentModel,
    pub apartment: RuntimeComApartmentState,
    pub disposition: RuntimeComInitializationDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeComUninitializationOutcome {
    pub thread_id: RuntimeThreadId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apartment: Option<RuntimeComApartmentState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeThreadId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeCallbackFrameId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCallbackScratch {
    pub slot: u16,
    pub address: GuestAddress,
    pub byte_length: u32,
    pub capacity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RuntimeTlsValue {
    Raw {
        value: u32,
    },
    DynamicEndpoint {
        endpoint_id: u32,
        address: GuestAddress,
    },
}

impl RuntimeTlsValue {
    pub fn raw_value(&self) -> u32 {
        match self {
            Self::Raw { value } => *value,
            Self::DynamicEndpoint { address, .. } => address.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFlsSlot {
    pub namespace: String,
    pub index: u32,
    pub callback: Option<GuestAddress>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCallbackFrame {
    pub id: RuntimeCallbackFrameId,
    pub thread_id: RuntimeThreadId,
    pub target: GuestAddress,
    pub return_sentinel: GuestAddress,
    pub entry_last_error: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scratch: Option<RuntimeCallbackScratch>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum RuntimeThreadLifecycleState {
    Initialized,
    Running,
    ExitRequested { exit_code: u32 },
    Exited { exit_code: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCancellation {
    pub thread_id: RuntimeThreadId,
    pub exit_code: u32,
    pub host_requests_cancelled: usize,
    pub callback_frames_cancelled: usize,
    pub wait_registrations_cancelled: usize,
    pub messages_discarded: usize,
    pub scheduler_events_discarded: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeThreadState {
    pub id: RuntimeThreadId,
    pub lifecycle: RuntimeThreadLifecycleState,
    pub last_error: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub com_apartment: Option<RuntimeComApartmentState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affinity_mask: Option<u32>,
    pub tls_values: BTreeMap<u32, RuntimeTlsValue>,
    pub fls_storage_initialized: bool,
    pub fls_values: BTreeMap<u32, u32>,
    pub callback_frames: Vec<RuntimeCallbackFrame>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<RuntimeThreadStack>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<RuntimeThreadEnvironment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guest_bootstrap: Option<RuntimeGuestThreadBootstrap>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeThreadStack {
    pub thread_id: RuntimeThreadId,
    pub address_allocation_id: ProcessAddressAllocationId,
    pub memory_owner: MemoryOwnerId,
    pub reserved_range: GuestRange,
    pub committed_range: GuestRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guard_page: Option<GuestRange>,
    pub committed_pages: usize,
    pub stack_top: GuestAddress,
    pub initial_stack_pointer: GuestAddress,
    pub return_sentinel: GuestAddress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeThreadEnvironment {
    pub thread_id: RuntimeThreadId,
    pub address_allocation_id: ProcessAddressAllocationId,
    pub memory_owner: MemoryOwnerId,
    pub range: GuestRange,
    pub teb_address: GuestAddress,
    pub peb_address: GuestAddress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGuestThreadBootstrap {
    pub thread_id: RuntimeThreadId,
    pub entry_point: GuestAddress,
    pub parameter: u32,
    pub initial_stack_pointer: GuestAddress,
    pub fs_base: GuestAddress,
    pub return_sentinel: GuestAddress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCreateThreadRequest {
    pub stack_reserve_bytes: u64,
    pub stack_commit_bytes: u64,
    pub entry_point: GuestAddress,
    pub parameter: u32,
    pub thread_id_output: Option<GuestAddress>,
    pub initial_suspend_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCreateThreadResult {
    pub thread_id: RuntimeThreadId,
    pub handle: super::RuntimeHandle,
    pub stack: RuntimeThreadStack,
    pub environment: RuntimeThreadEnvironment,
    pub bootstrap: RuntimeGuestThreadBootstrap,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id_output: Option<GuestAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id_write: Option<GuestWriteResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeResumeThreadResult {
    pub thread_id: RuntimeThreadId,
    pub handle: super::RuntimeHandle,
    pub previous_suspend_count: u32,
    pub remaining_suspend_count: u32,
    pub became_runnable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeThreadStackGrowth {
    pub thread_id: RuntimeThreadId,
    pub address_allocation_id: ProcessAddressAllocationId,
    pub memory_owner: MemoryOwnerId,
    pub fault_address: GuestAddress,
    pub page_fault_error_code: u32,
    pub committed_page: GuestRange,
    pub previous_guard_page: GuestRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_guard_page: Option<GuestRange>,
    pub committed_pages: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub(super) struct RuntimeThreads {
    states: BTreeMap<RuntimeThreadId, RuntimeThreadState>,
    #[serde(skip)]
    allocated_tls_indices: BTreeSet<u32>,
    #[serde(skip)]
    fls_slots: BTreeMap<u32, RuntimeFlsSlot>,
    #[serde(skip)]
    next_tls_index: u32,
    #[serde(skip)]
    next_fls_index: u32,
    #[serde(skip)]
    next_callback_frame_id: u64,
    #[serde(skip)]
    next_thread_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeThreadMutationPlan {
    previous: Option<RuntimeThreadState>,
    next: RuntimeThreadState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeThreadIdAllocationPlan {
    expected_next_thread_id: u64,
    thread_id: RuntimeThreadId,
    next_thread_id: u64,
}

impl RuntimeThreadIdAllocationPlan {
    pub(super) const fn thread_id(self) -> RuntimeThreadId {
        self.thread_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeThreadAffinityPlan {
    mutation: RuntimeThreadMutationPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeComInitializationPlan {
    previous: RuntimeThreadState,
    next: Option<RuntimeThreadState>,
    outcome: RuntimeComInitializationOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeComUninitializationPlan {
    previous: RuntimeThreadState,
    next: RuntimeThreadState,
    outcome: RuntimeComUninitializationOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeTlsIndexAllocationPlan {
    previous_next_index: u32,
    index: u32,
    next_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeFlsSlotAllocationPlan {
    previous_next_index: u32,
    slot: RuntimeFlsSlot,
    next_index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeCallbackFrameIdPlan {
    previous_next_id: u64,
    id: RuntimeCallbackFrameId,
    next_id: u64,
}

impl RuntimeCallbackFrameIdPlan {
    pub(super) const fn id(&self) -> RuntimeCallbackFrameId {
        self.id
    }
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
        next.com_apartment = None;
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
            com_apartment: None,
            affinity_mask: None,
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

    pub(super) const fn next_thread_id(&self) -> u64 {
        self.next_thread_id
    }

    pub(super) fn allocated_tls_indices(&self) -> &BTreeSet<u32> {
        &self.allocated_tls_indices
    }

    pub(super) fn fls_slots(&self) -> &BTreeMap<u32, RuntimeFlsSlot> {
        &self.fls_slots
    }

    pub(super) const fn next_tls_index(&self) -> u32 {
        self.next_tls_index
    }

    pub(super) const fn next_fls_index(&self) -> u32 {
        self.next_fls_index
    }

    pub(super) const fn next_callback_frame_id(&self) -> u64 {
        self.next_callback_frame_id
    }

    pub(super) fn plan_tls_index_allocation(
        &self,
    ) -> Result<RuntimeTlsIndexAllocationPlan, RuntimeStateError> {
        if self.next_tls_index == TLS_OUT_OF_INDEXES {
            return Err(RuntimeStateError::resource_limit(
                "runtime-tls",
                "tls-index-space-exhausted",
                "logical TLS index space is exhausted",
            ));
        }
        let index = self.next_tls_index;
        if self.allocated_tls_indices.contains(&index) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-tls",
                "tls-index-allocator-collision",
                "logical TLS allocator selected an already allocated index",
            ));
        }
        let next_index = index.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-tls",
                "tls-index-counter-overflow",
                "logical TLS index counter overflowed",
            )
        })?;
        Ok(RuntimeTlsIndexAllocationPlan {
            previous_next_index: self.next_tls_index,
            index,
            next_index,
        })
    }

    pub(super) fn commit_tls_index_allocation(
        &mut self,
        plan: RuntimeTlsIndexAllocationPlan,
    ) -> u32 {
        debug_assert_eq!(self.next_tls_index, plan.previous_next_index);
        let inserted = self.allocated_tls_indices.insert(plan.index);
        debug_assert!(inserted);
        self.next_tls_index = plan.next_index;
        plan.index
    }

    pub(super) fn require_tls_index(&self, index: u32) -> Result<(), RuntimeStateError> {
        if !self.allocated_tls_indices.contains(&index) {
            return Err(RuntimeStateError::not_found(
                "runtime-tls",
                "tls-index-unallocated",
                format!("logical TLS index {index} is not allocated"),
            ));
        }
        Ok(())
    }

    pub(super) fn plan_fls_slot_allocation(
        &self,
        callback: Option<crate::memory::GuestAddress>,
    ) -> Result<RuntimeFlsSlotAllocationPlan, RuntimeStateError> {
        if self.next_fls_index >= RUNTIME_FLS_INDEX_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-fls",
                "fls-index-space-exhausted",
                format!("logical FLS allocation is limited to {RUNTIME_FLS_INDEX_LIMIT} slots"),
            ));
        }
        let index = self.next_fls_index;
        if self.fls_slots.contains_key(&index) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-fls",
                "fls-index-allocator-collision",
                "logical FLS allocator selected an already allocated index",
            ));
        }
        let next_index = index.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-fls",
                "fls-index-counter-overflow",
                "logical FLS index counter overflowed",
            )
        })?;
        Ok(RuntimeFlsSlotAllocationPlan {
            previous_next_index: self.next_fls_index,
            slot: RuntimeFlsSlot {
                namespace: LOGICAL_FLS_INDEX_NAMESPACE.to_owned(),
                index,
                callback,
            },
            next_index,
        })
    }

    pub(super) fn commit_fls_slot_allocation(
        &mut self,
        plan: RuntimeFlsSlotAllocationPlan,
        thread_plan: RuntimeThreadMutationPlan,
    ) -> &RuntimeFlsSlot {
        debug_assert_eq!(self.next_fls_index, plan.previous_next_index);
        let index = plan.slot.index;
        let previous = self.fls_slots.insert(index, plan.slot);
        debug_assert!(previous.is_none());
        self.next_fls_index = plan.next_index;
        self.commit(thread_plan);
        self.fls_slots
            .get(&index)
            .expect("committed FLS slot disappeared")
    }

    pub(super) fn fls_slot(&self, index: u32) -> Result<&RuntimeFlsSlot, RuntimeStateError> {
        self.fls_slots.get(&index).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-fls",
                "fls-index-unallocated",
                format!("logical FLS index {index} is not allocated"),
            )
        })
    }

    pub(super) fn plan_callback_frame_id(
        &self,
    ) -> Result<RuntimeCallbackFrameIdPlan, RuntimeStateError> {
        let id = RuntimeCallbackFrameId(self.next_callback_frame_id);
        let next_id = self.next_callback_frame_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-callback",
                "callback-frame-id-overflow",
                "callback frame ID space is exhausted",
            )
        })?;
        Ok(RuntimeCallbackFrameIdPlan {
            previous_next_id: self.next_callback_frame_id,
            id,
            next_id,
        })
    }

    pub(super) fn commit_callback_mutation(
        &mut self,
        id_plan: Option<RuntimeCallbackFrameIdPlan>,
        thread_plan: RuntimeThreadMutationPlan,
    ) -> &RuntimeThreadState {
        if let Some(id_plan) = id_plan {
            debug_assert_eq!(self.next_callback_frame_id, id_plan.previous_next_id);
            self.next_callback_frame_id = id_plan.next_id;
        }
        self.commit(thread_plan)
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
                com_apartment: None,
                affinity_mask: None,
                tls_values: BTreeMap::new(),
                fls_storage_initialized: false,
                fls_values: BTreeMap::new(),
                callback_frames: Vec::new(),
                stack: None,
                environment: None,
                guest_bootstrap: None,
            },
        })
    }

    pub(super) fn plan_thread_id_allocation(
        &self,
    ) -> Result<RuntimeThreadIdAllocationPlan, RuntimeStateError> {
        if self.states.len() >= RUNTIME_GUEST_THREAD_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-thread-registry",
                "runtime-thread-capacity-exhausted",
                format!("runtime thread capacity of {RUNTIME_GUEST_THREAD_LIMIT} is exhausted"),
            ));
        }
        let mut candidate = self.next_thread_id;
        while candidate <= u64::from(u32::MAX)
            && self.states.contains_key(&RuntimeThreadId(candidate as u32))
        {
            candidate += 1;
        }
        let thread_id = u32::try_from(candidate).map(RuntimeThreadId).map_err(|_| {
            RuntimeStateError::resource_limit(
                "runtime-thread-registry",
                "runtime-thread-id-space-exhausted",
                "logical runtime thread ID space is exhausted",
            )
        })?;
        let next_thread_id = candidate.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-thread-registry",
                "runtime-thread-id-space-exhausted",
                "logical runtime thread ID space is exhausted",
            )
        })?;
        Ok(RuntimeThreadIdAllocationPlan {
            expected_next_thread_id: self.next_thread_id,
            thread_id,
            next_thread_id,
        })
    }

    pub(super) fn plan_guest_registration(
        &self,
        allocation: RuntimeThreadIdAllocationPlan,
        initial_last_error: u32,
        stack: RuntimeThreadStack,
        environment: RuntimeThreadEnvironment,
        bootstrap: RuntimeGuestThreadBootstrap,
    ) -> Result<RuntimeThreadMutationPlan, RuntimeStateError> {
        let thread_id = allocation.thread_id;
        if stack.thread_id != thread_id
            || environment.thread_id != thread_id
            || bootstrap.thread_id != thread_id
            || bootstrap.initial_stack_pointer != stack.initial_stack_pointer
            || bootstrap.return_sentinel != stack.return_sentinel
            || bootstrap.fs_base != environment.teb_address
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-thread-bootstrap",
                "guest-thread-bootstrap-identity-mismatch",
                "guest thread stack, TEB, and execution bootstrap identities must agree",
            ));
        }
        let mut plan = self.plan_registration(thread_id, initial_last_error)?;
        plan.next.stack = Some(stack);
        plan.next.environment = Some(environment);
        plan.next.guest_bootstrap = Some(bootstrap);
        Ok(plan)
    }

    pub(super) fn preflight_guest_registration_commit(
        &self,
        allocation: &RuntimeThreadIdAllocationPlan,
        registration: &RuntimeThreadMutationPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.next_thread_id != allocation.expected_next_thread_id
            || registration.previous.is_some()
            || registration.next.id != allocation.thread_id
            || self.states.contains_key(&allocation.thread_id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-registry",
                "guest-thread-registration-plan-stale",
                "runtime threads changed after guest thread creation preflight",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_guest_registration(
        &mut self,
        allocation: RuntimeThreadIdAllocationPlan,
        registration: RuntimeThreadMutationPlan,
    ) -> &RuntimeThreadState {
        debug_assert_eq!(self.next_thread_id, allocation.expected_next_thread_id);
        debug_assert_eq!(registration.next.id, allocation.thread_id);
        self.next_thread_id = allocation.next_thread_id;
        self.commit(registration)
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

    pub(super) fn com_apartment(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<Option<&RuntimeComApartmentState>, RuntimeStateError> {
        Ok(self.current_state(thread_id)?.com_apartment.as_ref())
    }

    pub(super) fn plan_com_initialization(
        &self,
        thread_id: RuntimeThreadId,
        requested_model: RuntimeComApartmentModel,
    ) -> Result<RuntimeComInitializationPlan, RuntimeStateError> {
        let previous = self.current_state(thread_id)?.clone();
        if previous.lifecycle != RuntimeThreadLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-com-apartment",
                "com-initialization-lifecycle-state-invalid",
                "COM initialization requires a running runtime thread",
            ));
        }
        let (next, apartment, disposition) = match previous.com_apartment {
            None => {
                let apartment = RuntimeComApartmentState {
                    model: requested_model,
                    initialization_count: 1,
                };
                let mut next = previous.clone();
                next.com_apartment = Some(apartment);
                (
                    Some(next),
                    apartment,
                    RuntimeComInitializationDisposition::Initialized,
                )
            }
            Some(apartment) if apartment.model == requested_model => {
                let initialization_count = apartment
                    .initialization_count
                    .checked_add(1)
                    .ok_or_else(|| {
                        RuntimeStateError::resource_limit(
                            "runtime-com-apartment",
                            "com-initialization-count-overflow",
                            "COM apartment initialization count is exhausted",
                        )
                    })?;
                let apartment = RuntimeComApartmentState {
                    model: requested_model,
                    initialization_count,
                };
                let mut next = previous.clone();
                next.com_apartment = Some(apartment);
                (
                    Some(next),
                    apartment,
                    RuntimeComInitializationDisposition::AlreadyInitialized,
                )
            }
            Some(apartment) => (
                None,
                apartment,
                RuntimeComInitializationDisposition::ChangedMode,
            ),
        };
        Ok(RuntimeComInitializationPlan {
            previous,
            next,
            outcome: RuntimeComInitializationOutcome {
                thread_id,
                requested_model,
                apartment,
                disposition,
            },
        })
    }

    pub(super) fn commit_com_initialization(
        &mut self,
        plan: RuntimeComInitializationPlan,
    ) -> Result<RuntimeComInitializationOutcome, RuntimeStateError> {
        if self.states.get(&plan.previous.id) != Some(&plan.previous) {
            return Err(RuntimeStateError::conflict(
                "runtime-com-apartment",
                "com-initialization-plan-stale",
                "runtime thread changed after COM initialization preflight",
            ));
        }
        if let Some(next) = plan.next {
            self.states.insert(next.id, next);
        }
        Ok(plan.outcome)
    }

    pub(super) fn plan_com_uninitialization(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeComUninitializationPlan, RuntimeStateError> {
        let previous = self.current_state(thread_id)?.clone();
        if previous.lifecycle != RuntimeThreadLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-com-apartment",
                "com-uninitialization-lifecycle-state-invalid",
                "COM uninitialization requires a running runtime thread",
            ));
        }
        let apartment = previous.com_apartment.ok_or_else(|| {
            RuntimeStateError::conflict(
                "runtime-com-apartment",
                "com-apartment-uninitialized",
                "current runtime thread has no initialized COM apartment",
            )
        })?;
        if apartment.initialization_count == 0 {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-com-apartment",
                "com-initialization-count-zero",
                "initialized COM apartment has a zero initialization count",
            ));
        }
        let remaining = apartment.initialization_count - 1;
        let next_apartment = (remaining != 0).then_some(RuntimeComApartmentState {
            model: apartment.model,
            initialization_count: remaining,
        });
        let mut next = previous.clone();
        next.com_apartment = next_apartment;
        Ok(RuntimeComUninitializationPlan {
            previous,
            next,
            outcome: RuntimeComUninitializationOutcome {
                thread_id,
                apartment: next_apartment,
            },
        })
    }

    pub(super) fn commit_com_uninitialization(
        &mut self,
        plan: RuntimeComUninitializationPlan,
    ) -> Result<RuntimeComUninitializationOutcome, RuntimeStateError> {
        if self.states.get(&plan.previous.id) != Some(&plan.previous) {
            return Err(RuntimeStateError::conflict(
                "runtime-com-apartment",
                "com-uninitialization-plan-stale",
                "runtime thread changed after COM uninitialization preflight",
            ));
        }
        self.states.insert(plan.next.id, plan.next);
        Ok(plan.outcome)
    }

    pub(super) fn effective_affinity_mask(
        &self,
        thread_id: RuntimeThreadId,
        process_mask: u32,
    ) -> Result<u32, RuntimeStateError> {
        if process_mask == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-thread-affinity",
                "thread-affinity-process-mask-zero",
                "runtime thread affinity requires a nonzero virtual process mask",
            ));
        }
        let thread = self.current_state(thread_id)?;
        let affinity_mask = thread.affinity_mask.unwrap_or(process_mask);
        if affinity_mask == 0 || affinity_mask & !process_mask != 0 {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-thread-affinity",
                "thread-affinity-state-outside-process-mask",
                "runtime thread affinity state is outside the virtual process mask",
            ));
        }
        Ok(affinity_mask)
    }

    pub(super) fn plan_affinity_mask(
        &self,
        thread_id: RuntimeThreadId,
        process_mask: u32,
        requested_mask: u32,
    ) -> Result<(RuntimeThreadAffinityPlan, u32), RuntimeStateError> {
        let previous = self.current_state(thread_id)?.clone();
        if previous.lifecycle != RuntimeThreadLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-affinity",
                "thread-affinity-lifecycle-state-invalid",
                "thread affinity can change only for a running runtime thread",
            ));
        }
        let previous_mask = self.effective_affinity_mask(thread_id, process_mask)?;
        if requested_mask == 0 || requested_mask & !process_mask != 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-thread-affinity",
                "thread-affinity-mask-outside-process-mask",
                "requested runtime thread affinity must be a nonzero subset of the virtual process mask",
            ));
        }
        let mut next = previous.clone();
        next.affinity_mask = Some(requested_mask);
        Ok((
            RuntimeThreadAffinityPlan {
                mutation: RuntimeThreadMutationPlan {
                    previous: Some(previous),
                    next,
                },
            },
            previous_mask,
        ))
    }

    pub(super) fn commit_affinity_mask(
        &mut self,
        plan: RuntimeThreadAffinityPlan,
    ) -> Result<&RuntimeThreadState, RuntimeStateError> {
        if self.states.get(&plan.mutation.next.id) != plan.mutation.previous.as_ref() {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-affinity",
                "thread-affinity-plan-stale",
                "runtime thread changed after affinity preflight",
            ));
        }
        Ok(self.commit(plan.mutation))
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
        next.com_apartment = None;
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
    fn com_apartment_plans_are_counted_thread_local_and_failure_atomic() {
        let mut threads = RuntimeThreads::new(INITIAL, 11);
        let start = threads.plan_start(INITIAL).unwrap();
        threads.commit(start);
        let registration = threads.plan_registration(SECOND, 33).unwrap();
        threads.commit(registration);

        let first = threads
            .plan_com_initialization(INITIAL, RuntimeComApartmentModel::SingleThreaded)
            .unwrap();
        assert_eq!(threads.com_apartment(INITIAL), Ok(None));
        let first = threads.commit_com_initialization(first).unwrap();
        assert_eq!(
            first.disposition,
            RuntimeComInitializationDisposition::Initialized
        );
        assert_eq!(first.apartment.initialization_count, 1);

        let repeat = threads
            .plan_com_initialization(INITIAL, RuntimeComApartmentModel::SingleThreaded)
            .unwrap();
        assert_eq!(
            threads
                .com_apartment(INITIAL)
                .unwrap()
                .unwrap()
                .initialization_count,
            1
        );
        let repeat = threads.commit_com_initialization(repeat).unwrap();
        assert_eq!(
            repeat.disposition,
            RuntimeComInitializationDisposition::AlreadyInitialized
        );
        assert_eq!(repeat.apartment.initialization_count, 2);

        let before_conflict = threads.clone();
        let conflict = threads
            .plan_com_initialization(INITIAL, RuntimeComApartmentModel::MultiThreaded)
            .unwrap();
        assert_eq!(
            conflict.outcome.disposition,
            RuntimeComInitializationDisposition::ChangedMode
        );
        assert!(conflict.next.is_none());
        let conflict = threads.commit_com_initialization(conflict).unwrap();
        assert_eq!(conflict.apartment.initialization_count, 2);
        assert_eq!(threads, before_conflict);

        let second = threads
            .plan_com_initialization(SECOND, RuntimeComApartmentModel::MultiThreaded)
            .unwrap();
        threads.commit_com_initialization(second).unwrap();
        assert_eq!(
            threads.com_apartment(SECOND).unwrap().unwrap().model,
            RuntimeComApartmentModel::MultiThreaded
        );
        assert_eq!(
            threads.com_apartment(INITIAL).unwrap().unwrap().model,
            RuntimeComApartmentModel::SingleThreaded
        );

        let stale = threads
            .plan_com_initialization(SECOND, RuntimeComApartmentModel::MultiThreaded)
            .unwrap();
        let last_error = threads.plan_last_error(SECOND, 44).unwrap();
        threads.commit(last_error);
        let before_stale = threads.clone();
        assert_eq!(
            threads.commit_com_initialization(stale).unwrap_err().code,
            "com-initialization-plan-stale"
        );
        assert_eq!(threads, before_stale);

        let stale_uninitialize = threads.plan_com_uninitialization(INITIAL).unwrap();
        let last_error = threads.plan_last_error(INITIAL, 12).unwrap();
        threads.commit(last_error);
        let before_stale = threads.clone();
        assert_eq!(
            threads
                .commit_com_uninitialization(stale_uninitialize)
                .unwrap_err()
                .code,
            "com-uninitialization-plan-stale"
        );
        assert_eq!(threads, before_stale);

        let first_uninitialize = threads.plan_com_uninitialization(INITIAL).unwrap();
        let first_uninitialize = threads
            .commit_com_uninitialization(first_uninitialize)
            .unwrap();
        assert_eq!(
            first_uninitialize.apartment.unwrap().initialization_count,
            1
        );
        let final_uninitialize = threads.plan_com_uninitialization(INITIAL).unwrap();
        let final_uninitialize = threads
            .commit_com_uninitialization(final_uninitialize)
            .unwrap();
        assert_eq!(final_uninitialize.apartment, None);
        let before_duplicate = threads.clone();
        assert_eq!(
            threads.plan_com_uninitialization(INITIAL).unwrap_err().code,
            "com-apartment-uninitialized"
        );
        assert_eq!(threads, before_duplicate);
    }

    #[test]
    fn com_apartment_overflow_and_thread_exit_do_not_leak_state() {
        let mut threads = RuntimeThreads::new(INITIAL, 11);
        let start = threads.plan_start(INITIAL).unwrap();
        threads.commit(start);
        threads.states.get_mut(&INITIAL).unwrap().com_apartment = Some(RuntimeComApartmentState {
            model: RuntimeComApartmentModel::SingleThreaded,
            initialization_count: u32::MAX,
        });
        let before_overflow = threads.clone();
        assert_eq!(
            threads
                .plan_com_initialization(INITIAL, RuntimeComApartmentModel::SingleThreaded)
                .unwrap_err()
                .code,
            "com-initialization-count-overflow"
        );
        assert_eq!(threads, before_overflow);

        let (request, _) = threads.plan_exit_request(INITIAL, 23).unwrap();
        let terminal = request.terminal_after_request(23).unwrap();
        threads.commit(request);
        threads.commit(terminal);
        assert_eq!(threads.state(INITIAL).unwrap().com_apartment, None);
        assert_eq!(
            threads
                .plan_com_initialization(INITIAL, RuntimeComApartmentModel::SingleThreaded)
                .unwrap_err()
                .code,
            "com-initialization-lifecycle-state-invalid"
        );
        assert_eq!(
            threads.plan_com_uninitialization(INITIAL).unwrap_err().code,
            "com-uninitialization-lifecycle-state-invalid"
        );
    }

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
    fn affinity_plans_use_the_virtual_process_default_and_commit_per_thread() {
        let mut threads = RuntimeThreads::new(INITIAL, 11);
        let start = threads.plan_start(INITIAL).unwrap();
        threads.commit(start);
        let registration = threads.plan_registration(SECOND, 33).unwrap();
        threads.commit(registration);

        assert_eq!(threads.effective_affinity_mask(INITIAL, 0b111), Ok(0b111));
        assert_eq!(threads.effective_affinity_mask(SECOND, 0b111), Ok(0b111));
        let before = threads.clone();
        let (plan, previous) = threads.plan_affinity_mask(INITIAL, 0b111, 0b001).unwrap();
        assert_eq!(previous, 0b111);
        assert_eq!(threads, before, "affinity planning must not mutate state");
        threads.commit_affinity_mask(plan).unwrap();
        assert_eq!(threads.effective_affinity_mask(INITIAL, 0b111), Ok(0b001));
        assert_eq!(threads.effective_affinity_mask(SECOND, 0b111), Ok(0b111));

        let before_invalid = threads.clone();
        assert_eq!(
            threads
                .plan_affinity_mask(INITIAL, 0b111, 0)
                .unwrap_err()
                .code,
            "thread-affinity-mask-outside-process-mask"
        );
        assert_eq!(
            threads
                .plan_affinity_mask(INITIAL, 0b111, 0b1000)
                .unwrap_err()
                .code,
            "thread-affinity-mask-outside-process-mask"
        );
        assert_eq!(threads, before_invalid);

        let (restore, previous) = threads.plan_affinity_mask(INITIAL, 0b111, 0b111).unwrap();
        assert_eq!(previous, 0b001);
        let stale = threads.plan_affinity_mask(INITIAL, 0b111, 0b010).unwrap().0;
        threads.commit_affinity_mask(restore).unwrap();
        assert_eq!(threads.effective_affinity_mask(INITIAL, 0b111), Ok(0b111));
        let before_stale = threads.clone();
        assert_eq!(
            threads.commit_affinity_mask(stale).unwrap_err().code,
            "thread-affinity-plan-stale"
        );
        assert_eq!(threads, before_stale);
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
            scratch: None,
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

    #[test]
    fn resource_allocator_plans_commit_with_their_owned_thread_state() {
        let mut threads = RuntimeThreads::new(INITIAL, 7);
        let start = threads.plan_start(INITIAL).unwrap();
        threads.commit(start);

        let tls = threads.plan_tls_index_allocation().unwrap();
        assert!(threads.allocated_tls_indices().is_empty());
        assert_eq!(threads.next_tls_index(), 0);
        assert_eq!(threads.commit_tls_index_allocation(tls), 0);
        assert_eq!(threads.allocated_tls_indices(), &BTreeSet::from([0]));
        assert_eq!(threads.next_tls_index(), 1);

        let fls = threads
            .plan_fls_slot_allocation(Some(GuestAddress(0x1000_2000)))
            .unwrap();
        let initialize = threads.plan_initialize_fls_storage(INITIAL).unwrap();
        assert!(threads.fls_slots().is_empty());
        assert!(!threads.state(INITIAL).unwrap().fls_storage_initialized);
        let slot = threads.commit_fls_slot_allocation(fls, initialize).clone();
        assert_eq!(slot.index, 0);
        assert_eq!(slot.callback, Some(GuestAddress(0x1000_2000)));
        assert!(threads.state(INITIAL).unwrap().fls_storage_initialized);
        assert_eq!(threads.next_fls_index(), 1);

        let callback_id = threads.plan_callback_frame_id().unwrap();
        let frame = RuntimeCallbackFrame {
            id: callback_id.id(),
            thread_id: INITIAL,
            target: GuestAddress(0x1000_3000),
            return_sentinel: GuestAddress(0x7f01_0000),
            entry_last_error: 7,
            scratch: None,
        };
        let (push, depth) = threads.plan_callback_push(INITIAL, frame).unwrap();
        assert_eq!(depth, 1);
        assert_eq!(threads.next_callback_frame_id(), 1);
        assert!(threads.state(INITIAL).unwrap().callback_frames.is_empty());
        threads.commit_callback_mutation(Some(callback_id), push);
        assert_eq!(threads.next_callback_frame_id(), 2);
        assert_eq!(threads.state(INITIAL).unwrap().callback_frames, [frame]);
    }

    #[test]
    fn resource_allocator_rejections_preserve_all_owned_state() {
        let mut threads = RuntimeThreads::new(INITIAL, 0);
        threads.allocated_tls_indices.insert(0);
        let before_tls_collision = threads.clone();
        assert_eq!(
            threads.plan_tls_index_allocation().unwrap_err().code,
            "tls-index-allocator-collision"
        );
        assert_eq!(threads, before_tls_collision);

        threads.next_tls_index = TLS_OUT_OF_INDEXES;
        let before_tls_exhaustion = threads.clone();
        assert_eq!(
            threads.plan_tls_index_allocation().unwrap_err().code,
            "tls-index-space-exhausted"
        );
        assert_eq!(threads, before_tls_exhaustion);

        threads.next_fls_index = RUNTIME_FLS_INDEX_LIMIT;
        let before_fls_exhaustion = threads.clone();
        assert_eq!(
            threads.plan_fls_slot_allocation(None).unwrap_err().code,
            "fls-index-space-exhausted"
        );
        assert_eq!(threads, before_fls_exhaustion);

        threads.next_callback_frame_id = u64::MAX;
        let before_callback_exhaustion = threads.clone();
        assert_eq!(
            threads.plan_callback_frame_id().unwrap_err().code,
            "callback-frame-id-overflow"
        );
        assert_eq!(threads, before_callback_exhaustion);
    }
}

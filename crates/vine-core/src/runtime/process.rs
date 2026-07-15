use std::collections::BTreeMap;

use serde::Serialize;

use crate::environment::{
    CONTROLLED_ANSI_COMMAND_LINE_ENCODING, CONTROLLED_WIDE_ENVIRONMENT_ENCODING,
    InitialProcessEnvironment,
};
use crate::memory::GuestAddress;

use super::{
    ProcessAddressAllocationId, ProcessAddressAllocationRecord, ProcessAddressSpaceLayout,
    RuntimeAddressRange, RuntimeCancellation, RuntimeStateError, RuntimeThreadId,
};

pub const LOGICAL_STANDARD_HANDLE_NAMESPACE: &str = "vine.logical-standard-handle.v1";
pub const X86_STARTUP_INFO_W_BYTE_LENGTH: usize = 68;
pub const RUNTIME_STANDARD_INPUT_SELECTOR: u32 = 0xffff_fff6;
pub const RUNTIME_STANDARD_INPUT_HANDLE: u32 = 0x5301_0001;
pub const RUNTIME_STANDARD_OUTPUT_SELECTOR: u32 = 0xffff_fff5;
pub const RUNTIME_STANDARD_OUTPUT_HANDLE: u32 = 0x5301_0002;
pub const RUNTIME_STANDARD_ERROR_SELECTOR: u32 = 0xffff_fff4;
pub const RUNTIME_STANDARD_ERROR_HANDLE: u32 = 0x5301_0003;
pub const RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING: &str = "windows-932-v1";
pub const RUNTIME_PROCESS_IMAGE_PATH_A_MAX_BYTES: usize = 32 * 1024;

const PROCESS_IMAGE_DIRECTORY_AUTHORITY: &str = "runtime-process-image-directory-v1";
const RUNTIME_STANDARD_HANDLE_OWNER: &str = "runtime-process-standard-handles";
const RUNTIME_STANDARD_INPUT_VARIANT: &str = "standard-input-logical-v1";
const RUNTIME_STANDARD_OUTPUT_VARIANT: &str = "standard-output-logical-v1";
const RUNTIME_STANDARD_ERROR_VARIANT: &str = "standard-error-logical-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeProcessId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeClockState {
    pub wall_clock_file_time: u64,
    pub tick_count: u32,
    pub performance_counter: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStartupInfo {
    pub profile_id: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCommandLine {
    pub source_authority: String,
    pub wide_buffer_address: GuestAddress,
    pub wide_length_bytes: u16,
    pub ansi_buffer_address: GuestAddress,
    pub ansi_byte_length_with_nul: u32,
    pub ansi_encoding: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProcessImagePathA {
    pub source_authority: String,
    pub bytes_with_nul: Vec<u8>,
    pub encoding: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEnvironmentStringCopy {
    pub acquisition_id: u64,
    pub buffer_address: GuestAddress,
    pub byte_length_with_double_nul: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEnvironmentStrings {
    pub source_authority: String,
    pub authority_buffer_address: GuestAddress,
    pub copy_buffer_address: GuestAddress,
    pub byte_length_with_double_nul: u32,
    pub encoding: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_copy: Option<RuntimeEnvironmentStringCopy>,
    pub next_acquisition_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProcessEnvironment {
    pub address_allocation_id: ProcessAddressAllocationId,
    pub range: RuntimeAddressRange,
    pub command_line: RuntimeCommandLine,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment_strings: Option<RuntimeEnvironmentStrings>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStandardHandle {
    pub owner: String,
    pub variant_id: String,
    pub namespace: String,
    pub handle: u32,
    pub selector: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum RuntimeProcessLifecycleState {
    Bootstrapping,
    Running,
    ExitRequested {
        exit_code: u32,
        initiating_thread_id: RuntimeThreadId,
    },
    Exited {
        exit_code: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProcessCancellation {
    pub initiating_thread_id: RuntimeThreadId,
    pub exit_code: u32,
    pub thread_cancellations: Vec<RuntimeCancellation>,
    pub host_requests_cancelled: usize,
    pub callback_frames_cancelled: usize,
    pub wait_registrations_cancelled: usize,
    pub messages_discarded: usize,
    pub scheduler_events_discarded: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProcessState {
    id: RuntimeProcessId,
    lifecycle: RuntimeProcessLifecycleState,
    address_space: ProcessAddressSpaceLayout,
    clock: RuntimeClockState,
    startup_info: RuntimeStartupInfo,
    image_path_a: RuntimeProcessImagePathA,
    #[serde(flatten)]
    directories: RuntimeProcessDirectories,
    #[serde(skip_serializing_if = "Option::is_none")]
    environment: Option<RuntimeProcessEnvironment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unhandled_exception_filter: Option<GuestAddress>,
    heap_termination_on_corruption: bool,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    standard_handles: BTreeMap<u32, RuntimeStandardHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCurrentDirectoryA {
    pub source_authority: String,
    pub bytes_with_nul: Vec<u8>,
    pub encoding: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeProcessDirectories {
    current_directory_a: RuntimeCurrentDirectoryA,
    current_directory_generation: u64,
    #[serde(skip)]
    process_image_directory_a: RuntimeCurrentDirectoryA,
    #[serde(skip)]
    process_image_directory_key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSetCurrentDirectoryAPlan {
    previous: RuntimeCurrentDirectoryA,
    previous_generation: u64,
    next: RuntimeCurrentDirectoryA,
    next_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeProcessMutationPlan {
    previous_lifecycle: RuntimeProcessLifecycleState,
    next_lifecycle: RuntimeProcessLifecycleState,
    previous_unhandled_exception_filter: Option<GuestAddress>,
    next_unhandled_exception_filter: Option<GuestAddress>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeTickCountSamplePlan {
    previous_lifecycle: RuntimeProcessLifecycleState,
    previous_tick_count: u32,
    returned_tick_count: u32,
    next_tick_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeProcessEnvironmentPlan {
    previous_lifecycle: RuntimeProcessLifecycleState,
    range: RuntimeAddressRange,
    command_line: RuntimeCommandLine,
    environment_strings: Option<RuntimeEnvironmentStrings>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedDirectoryPath {
    bytes_with_nul: Vec<u8>,
    identity_key: Vec<u8>,
}

impl RuntimeProcessDirectories {
    pub(super) fn new(
        mut initial: RuntimeCurrentDirectoryA,
        image_path: &RuntimeProcessImagePathA,
    ) -> Result<Self, RuntimeStateError> {
        validate_path_authority(&initial.source_authority, &initial.encoding)?;
        let normalized_initial = normalize_absolute_directory_path(&initial.bytes_with_nul)?;
        initial.bytes_with_nul = normalized_initial.bytes_with_nul;

        validate_path_authority(&image_path.source_authority, &image_path.encoding)?;
        let image_directory = process_image_directory(image_path)?;
        let image_directory_key =
            normalize_absolute_directory_path(&image_directory.bytes_with_nul)?.identity_key;
        Ok(Self {
            current_directory_a: initial,
            current_directory_generation: 0,
            process_image_directory_a: image_directory,
            process_image_directory_key: image_directory_key,
        })
    }

    pub(super) fn current_directory_a(&self) -> &RuntimeCurrentDirectoryA {
        &self.current_directory_a
    }

    pub(super) const fn current_directory_generation(&self) -> u64 {
        self.current_directory_generation
    }

    pub(super) fn is_process_image_directory(
        &self,
        requested: &[u8],
    ) -> Result<bool, RuntimeStateError> {
        Ok(normalize_absolute_directory_path(requested)?.identity_key
            == self.process_image_directory_key)
    }

    pub(super) fn plan_process_image_directory(
        &self,
        requested: &[u8],
    ) -> Result<RuntimeSetCurrentDirectoryAPlan, RuntimeStateError> {
        if !self.is_process_image_directory(requested)? {
            return Err(RuntimeStateError::not_found(
                "runtime-process-directory",
                "process-image-directory-not-registered",
                "requested current directory is not the registered process image directory",
            ));
        }
        let next_generation = self
            .current_directory_generation
            .checked_add(1)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-process-directory",
                    "current-directory-generation-overflow",
                    "process current-directory generation overflowed",
                )
            })?;
        Ok(RuntimeSetCurrentDirectoryAPlan {
            previous: self.current_directory_a.clone(),
            previous_generation: self.current_directory_generation,
            next: self.process_image_directory_a.clone(),
            next_generation,
        })
    }

    pub(super) fn commit(
        &mut self,
        plan: RuntimeSetCurrentDirectoryAPlan,
    ) -> Result<&RuntimeCurrentDirectoryA, RuntimeStateError> {
        if self.current_directory_generation != plan.previous_generation
            || self.current_directory_a != plan.previous
        {
            return Err(RuntimeStateError::conflict(
                "runtime-process-directory",
                "current-directory-plan-stale",
                "process current-directory state changed after preflight",
            ));
        }
        self.current_directory_a = plan.next;
        self.current_directory_generation = plan.next_generation;
        Ok(&self.current_directory_a)
    }
}

impl RuntimeProcessState {
    pub(super) fn new(
        id: RuntimeProcessId,
        address_space: ProcessAddressSpaceLayout,
        clock: RuntimeClockState,
        startup_info: RuntimeStartupInfo,
        image_path_a: RuntimeProcessImagePathA,
        directories: RuntimeProcessDirectories,
    ) -> Self {
        Self {
            id,
            lifecycle: RuntimeProcessLifecycleState::Bootstrapping,
            address_space,
            clock,
            startup_info,
            image_path_a,
            directories,
            environment: None,
            unhandled_exception_filter: None,
            heap_termination_on_corruption: false,
            standard_handles: BTreeMap::new(),
        }
    }

    pub fn id(&self) -> super::RuntimeProcessId {
        self.id
    }

    pub fn lifecycle(&self) -> RuntimeProcessLifecycleState {
        self.lifecycle
    }

    pub fn address_space(&self) -> super::ProcessAddressSpaceLayout {
        self.address_space
    }

    pub fn clock(&self) -> &RuntimeClockState {
        &self.clock
    }

    pub(super) fn plan_tick_count_sample(
        &self,
        increment_per_read: u32,
    ) -> Result<RuntimeTickCountSamplePlan, RuntimeStateError> {
        if !self.is_running() {
            return Err(RuntimeStateError::conflict(
                "runtime-process-clock",
                "tick-count-process-state-invalid",
                "tick-count sampling requires a running process",
            ));
        }
        if increment_per_read == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-process-clock",
                "tick-count-increment-zero",
                "advancing tick-count sampling requires a nonzero increment",
            ));
        }
        Ok(RuntimeTickCountSamplePlan {
            previous_lifecycle: self.lifecycle,
            previous_tick_count: self.clock.tick_count,
            returned_tick_count: self.clock.tick_count,
            next_tick_count: self.clock.tick_count.wrapping_add(increment_per_read),
        })
    }

    pub(super) fn commit_tick_count_sample(
        &mut self,
        plan: RuntimeTickCountSamplePlan,
    ) -> Result<u32, RuntimeStateError> {
        if self.lifecycle != plan.previous_lifecycle
            || self.clock.tick_count != plan.previous_tick_count
        {
            return Err(RuntimeStateError::conflict(
                "runtime-process-clock",
                "tick-count-sample-plan-stale",
                "process lifecycle or tick count changed after sampling preflight",
            ));
        }
        self.clock.tick_count = plan.next_tick_count;
        Ok(plan.returned_tick_count)
    }

    pub fn startup_info(&self) -> &RuntimeStartupInfo {
        &self.startup_info
    }

    pub fn image_path_a(&self) -> &RuntimeProcessImagePathA {
        &self.image_path_a
    }

    pub fn environment(&self) -> Option<&RuntimeProcessEnvironment> {
        self.environment.as_ref()
    }

    pub fn unhandled_exception_filter(&self) -> Option<GuestAddress> {
        self.unhandled_exception_filter
    }

    pub fn heap_termination_on_corruption(&self) -> bool {
        self.heap_termination_on_corruption
    }

    pub fn standard_handles(&self) -> &std::collections::BTreeMap<u32, RuntimeStandardHandle> {
        &self.standard_handles
    }

    pub(super) const fn is_bootstrapping(&self) -> bool {
        matches!(self.lifecycle, RuntimeProcessLifecycleState::Bootstrapping)
    }

    pub(super) const fn is_running(&self) -> bool {
        matches!(self.lifecycle, RuntimeProcessLifecycleState::Running)
    }

    pub(super) const fn is_exiting(&self) -> bool {
        matches!(
            self.lifecycle,
            RuntimeProcessLifecycleState::ExitRequested { .. }
                | RuntimeProcessLifecycleState::Exited { .. }
        )
    }

    pub(super) fn plan_start(&self) -> Result<RuntimeProcessMutationPlan, RuntimeStateError> {
        if !self.is_bootstrapping() {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "initial-thread-start-transition-invalid",
                "initial thread start requires a bootstrapping process",
            ));
        }
        Ok(self.plan_mutation(
            RuntimeProcessLifecycleState::Running,
            self.unhandled_exception_filter,
        ))
    }

    pub(super) fn plan_exit_request(
        &self,
        initiating_thread_id: RuntimeThreadId,
        exit_code: u32,
    ) -> Result<RuntimeProcessMutationPlan, RuntimeStateError> {
        if !self.is_running() {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "process-exit-request-transition-invalid",
                "process exit request requires a running process",
            ));
        }
        Ok(self.plan_mutation(
            RuntimeProcessLifecycleState::ExitRequested {
                exit_code,
                initiating_thread_id,
            },
            self.unhandled_exception_filter,
        ))
    }

    pub(super) fn exit_request_code(
        &self,
        initiating_thread_id: RuntimeThreadId,
    ) -> Result<u32, RuntimeStateError> {
        match self.lifecycle {
            RuntimeProcessLifecycleState::ExitRequested {
                exit_code,
                initiating_thread_id: owner,
            } if owner == initiating_thread_id => Ok(exit_code),
            _ => Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "process-exit-completion-transition-invalid",
                "process exit completion requires a matching prior exit request",
            )),
        }
    }

    pub(super) fn plan_exit_completion(
        &self,
        initiating_thread_id: RuntimeThreadId,
    ) -> Result<RuntimeProcessMutationPlan, RuntimeStateError> {
        let exit_code = self.exit_request_code(initiating_thread_id)?;
        Ok(self.plan_mutation(RuntimeProcessLifecycleState::Exited { exit_code }, None))
    }

    pub(super) fn plan_direct_termination(
        &self,
        exit_code: u32,
    ) -> Result<RuntimeProcessMutationPlan, RuntimeStateError> {
        if !self.is_running() {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "process-exit-request-transition-invalid",
                "process termination requires a running process",
            ));
        }
        Ok(self.plan_mutation(RuntimeProcessLifecycleState::Exited { exit_code }, None))
    }

    pub(super) fn preflight_mutation_commit(
        &self,
        plan: &RuntimeProcessMutationPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.lifecycle != plan.previous_lifecycle
            || self.unhandled_exception_filter != plan.previous_unhandled_exception_filter
        {
            return Err(RuntimeStateError::conflict(
                "runtime-process",
                "process-mutation-plan-stale",
                "process lifecycle or policy changed after preflight",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_mutation(&mut self, plan: RuntimeProcessMutationPlan) {
        debug_assert_eq!(self.lifecycle, plan.previous_lifecycle);
        debug_assert_eq!(
            self.unhandled_exception_filter,
            plan.previous_unhandled_exception_filter
        );
        self.lifecycle = plan.next_lifecycle;
        self.unhandled_exception_filter = plan.next_unhandled_exception_filter;
    }

    fn plan_mutation(
        &self,
        next_lifecycle: RuntimeProcessLifecycleState,
        next_unhandled_exception_filter: Option<GuestAddress>,
    ) -> RuntimeProcessMutationPlan {
        RuntimeProcessMutationPlan {
            previous_lifecycle: self.lifecycle,
            next_lifecycle,
            previous_unhandled_exception_filter: self.unhandled_exception_filter,
            next_unhandled_exception_filter,
        }
    }

    pub(super) fn set_unhandled_exception_filter(
        &mut self,
        filter: Option<GuestAddress>,
    ) -> Result<Option<GuestAddress>, RuntimeStateError> {
        if !self.is_running() {
            return Err(RuntimeStateError::conflict(
                "runtime-process-exception-filter",
                "unhandled-exception-filter-process-state-invalid",
                "unhandled-exception filter replacement requires a running process",
            ));
        }
        let previous = self.unhandled_exception_filter;
        self.unhandled_exception_filter = filter;
        Ok(previous)
    }

    pub(super) fn set_heap_termination_on_corruption(&mut self, enabled: bool) {
        self.heap_termination_on_corruption = enabled;
    }

    pub(super) fn current_directory_a(&self) -> &RuntimeCurrentDirectoryA {
        self.directories.current_directory_a()
    }

    pub(super) fn current_directory_generation(&self) -> u64 {
        self.directories.current_directory_generation()
    }

    pub(super) fn is_process_image_directory_a(
        &self,
        requested: &[u8],
    ) -> Result<bool, RuntimeStateError> {
        self.directories.is_process_image_directory(requested)
    }

    pub(super) fn plan_set_current_directory_a(
        &self,
        requested: &[u8],
    ) -> Result<RuntimeSetCurrentDirectoryAPlan, RuntimeStateError> {
        if !self.is_running() {
            return Err(RuntimeStateError::conflict(
                "runtime-process-directory",
                "current-directory-process-state-invalid",
                "current-directory replacement requires a running process",
            ));
        }
        self.directories.plan_process_image_directory(requested)
    }

    pub(super) fn commit_set_current_directory_a(
        &mut self,
        plan: RuntimeSetCurrentDirectoryAPlan,
    ) -> Result<&RuntimeCurrentDirectoryA, RuntimeStateError> {
        if !self.is_running() {
            return Err(RuntimeStateError::conflict(
                "runtime-process-directory",
                "current-directory-process-state-invalid",
                "current-directory replacement requires a running process",
            ));
        }
        self.directories.commit(plan)
    }

    pub(super) fn preflight_environment_registration(
        &self,
        environment: &InitialProcessEnvironment,
    ) -> Result<RuntimeProcessEnvironmentPlan, RuntimeStateError> {
        if self.environment.is_some() {
            return Err(RuntimeStateError::conflict(
                "runtime-process-environment",
                "process-environment-already-registered",
                "runtime process environment may be registered only once",
            ));
        }
        if self.is_exiting() {
            return Err(RuntimeStateError::conflict(
                "runtime-process-environment",
                "process-environment-lifecycle-state-invalid",
                "runtime process environment cannot be registered during process exit",
            ));
        }
        if !environment.owners_sealed {
            return Err(RuntimeStateError::invalid_input(
                "runtime-process-environment",
                "process-environment-owner-unsealed",
                "runtime process environment requires sealed guest-memory owners",
            ));
        }
        let parameters = environment.process_parameters.as_ref().ok_or_else(|| {
            RuntimeStateError::invalid_input(
                "runtime-process-environment",
                "process-parameters-missing",
                "runtime process command line requires initial process parameters",
            )
        })?;
        if !parameters.owners_sealed {
            return Err(RuntimeStateError::invalid_input(
                "runtime-process-environment",
                "process-parameter-owner-unsealed",
                "runtime process parameters require sealed guest-memory owners",
            ));
        }
        let ansi = parameters.ansi.as_ref().ok_or_else(|| {
            RuntimeStateError::invalid_input(
                "runtime-process-environment",
                "ansi-command-line-unavailable",
                "the controlled process command line has no deterministic ANSI view",
            )
        })?;
        if ansi.encoding != CONTROLLED_ANSI_COMMAND_LINE_ENCODING
            || !parameters.command_line.is_ascii()
            || usize::try_from(ansi.byte_length_with_nul).ok()
                != parameters.command_line.len().checked_add(1)
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-process-environment",
                "ansi-command-line-metadata-invalid",
                "initial ANSI command-line metadata does not match the controlled ASCII authority",
            ));
        }
        let ansi_end = u64::from(ansi.buffer_address.0)
            .checked_add(u64::from(ansi.byte_length_with_nul))
            .ok_or_else(|| {
                RuntimeStateError::invalid_input(
                    "runtime-process-environment",
                    "ansi-command-line-range-overflow",
                    "initial ANSI command-line range overflowed x86 accounting",
                )
            })?;
        if ansi.buffer_address == parameters.command_line_buffer_address
            || u64::from(ansi.buffer_address.0) < u64::from(parameters.command_line_range.start().0)
            || ansi_end > parameters.command_line_range.end()
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-process-environment",
                "ansi-command-line-range-invalid",
                "initial ANSI command-line view must be distinct and contained by the command-line allocation",
            ));
        }
        let wide_start = u64::from(parameters.command_line_buffer_address.0);
        let wide_end = wide_start
            .checked_add(u64::from(parameters.command_line_maximum_length_bytes))
            .ok_or_else(|| {
                RuntimeStateError::invalid_input(
                    "runtime-process-environment",
                    "wide-command-line-range-overflow",
                    "initial wide command-line range overflowed x86 accounting",
                )
            })?;
        if wide_start < u64::from(parameters.command_line_range.start().0)
            || wide_end > parameters.command_line_range.end()
            || ranges_overlap(
                wide_start,
                wide_end,
                u64::from(ansi.buffer_address.0),
                ansi_end,
            )
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-process-environment",
                "command-line-view-range-invalid",
                "initial wide and ANSI command-line views must be disjoint and contained",
            ));
        }
        let environment_strings = parameters
            .environment_strings
            .as_ref()
            .map(|environment_strings| {
                if environment_strings.encoding != CONTROLLED_WIDE_ENVIRONMENT_ENCODING
                    || environment_strings.byte_length_with_double_nul != 4
                    || environment_strings.authority_buffer_address
                        == environment_strings.copy_buffer_address
                {
                    return Err(RuntimeStateError::invalid_input(
                        "runtime-process-environment",
                        "wide-environment-metadata-invalid",
                        "initial wide environment metadata does not match the controlled empty block",
                    ));
                }
                let authority_start =
                    u64::from(environment_strings.authority_buffer_address.0);
                let authority_end = authority_start
                    .checked_add(u64::from(
                        environment_strings.byte_length_with_double_nul,
                    ))
                    .ok_or_else(|| {
                        RuntimeStateError::invalid_input(
                            "runtime-process-environment",
                            "wide-environment-authority-range-overflow",
                            "wide environment authority range overflowed x86 accounting",
                        )
                    })?;
                let copy_start = u64::from(environment_strings.copy_buffer_address.0);
                let copy_end = copy_start
                    .checked_add(u64::from(
                        environment_strings.byte_length_with_double_nul,
                    ))
                    .ok_or_else(|| {
                        RuntimeStateError::invalid_input(
                            "runtime-process-environment",
                            "wide-environment-copy-range-overflow",
                            "wide environment copy range overflowed x86 accounting",
                        )
                    })?;
                let command_line_start = u64::from(parameters.command_line_range.start().0);
                let command_line_end = parameters.command_line_range.end();
                if authority_start < command_line_start
                    || authority_end > command_line_end
                    || copy_start < command_line_start
                    || copy_end > command_line_end
                    || ranges_overlap(authority_start, authority_end, copy_start, copy_end)
                    || ranges_overlap(authority_start, authority_end, wide_start, wide_end)
                    || ranges_overlap(
                        authority_start,
                        authority_end,
                        u64::from(ansi.buffer_address.0),
                        ansi_end,
                    )
                    || ranges_overlap(copy_start, copy_end, wide_start, wide_end)
                    || ranges_overlap(
                        copy_start,
                        copy_end,
                        u64::from(ansi.buffer_address.0),
                        ansi_end,
                    )
                {
                    return Err(RuntimeStateError::invalid_input(
                        "runtime-process-environment",
                        "wide-environment-range-invalid",
                        "wide environment authority and copy must be disjoint from all process string views",
                    ));
                }
                Ok(RuntimeEnvironmentStrings {
                    source_authority: "rtl-user-process-parameters-environment".to_owned(),
                    authority_buffer_address: environment_strings.authority_buffer_address,
                    copy_buffer_address: environment_strings.copy_buffer_address,
                    byte_length_with_double_nul: environment_strings.byte_length_with_double_nul,
                    encoding: environment_strings.encoding.clone(),
                    active_copy: None,
                    next_acquisition_id: 1,
                })
            })
            .transpose()?;

        let mut ranges = [
            parameters.process_parameters_range,
            parameters.command_line_range,
            environment.teb_range,
            environment.peb_range,
        ];
        ranges.sort_by_key(|range| range.start().0);
        if ranges
            .windows(2)
            .any(|pair| pair[0].end() != u64::from(pair[1].start().0))
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-process-environment",
                "process-environment-range-discontiguous",
                "initial process-environment pages must form one contiguous allocation",
            ));
        }
        let base = ranges[0].start();
        let byte_length = ranges[3]
            .end()
            .checked_sub(u64::from(base.0))
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-process-environment",
                    "process-environment-range-order-invalid",
                    "sorted process-environment ranges have an invalid order",
                )
            })?;
        let range = RuntimeAddressRange::new(base, byte_length)?;
        Ok(RuntimeProcessEnvironmentPlan {
            previous_lifecycle: self.lifecycle,
            range,
            command_line: RuntimeCommandLine {
                source_authority: "rtl-user-process-parameters-command-line".to_owned(),
                wide_buffer_address: parameters.command_line_buffer_address,
                wide_length_bytes: parameters.command_line_length_bytes,
                ansi_buffer_address: ansi.buffer_address,
                ansi_byte_length_with_nul: ansi.byte_length_with_nul,
                ansi_encoding: ansi.encoding.clone(),
            },
            environment_strings,
        })
    }

    pub(super) fn preflight_environment_commit(
        &self,
        plan: &RuntimeProcessEnvironmentPlan,
        allocation: &ProcessAddressAllocationRecord,
    ) -> Result<(), RuntimeStateError> {
        if self.lifecycle != plan.previous_lifecycle || self.environment.is_some() {
            return Err(RuntimeStateError::conflict(
                "runtime-process-environment",
                "process-environment-plan-stale",
                "process state changed after environment registration preflight",
            ));
        }
        if allocation.range != plan.range {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-process-environment",
                "process-environment-preferred-address-lost",
                "process address allocator did not preserve the materialized environment range",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_environment_registration(
        &mut self,
        plan: RuntimeProcessEnvironmentPlan,
        allocation: ProcessAddressAllocationRecord,
    ) -> &RuntimeProcessEnvironment {
        debug_assert_eq!(self.lifecycle, plan.previous_lifecycle);
        debug_assert!(self.environment.is_none());
        debug_assert_eq!(allocation.range, plan.range);
        self.environment = Some(RuntimeProcessEnvironment {
            address_allocation_id: allocation.id,
            range: plan.range,
            command_line: plan.command_line,
            environment_strings: plan.environment_strings,
        });
        self.environment
            .as_ref()
            .expect("committed process environment disappeared")
    }

    pub(super) fn command_line(&self) -> Result<&RuntimeCommandLine, RuntimeStateError> {
        self.environment
            .as_ref()
            .map(|environment| &environment.command_line)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-process-environment",
                    "process-command-line-unregistered",
                    "runtime process has no registered command-line authority",
                )
            })
    }

    pub(super) fn environment_strings(
        &self,
    ) -> Result<&RuntimeEnvironmentStrings, RuntimeStateError> {
        self.environment
            .as_ref()
            .and_then(|environment| environment.environment_strings.as_ref())
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-process-environment",
                    "wide-environment-strings-unregistered",
                    "runtime process has no registered wide environment-string authority",
                )
            })
    }

    pub(super) fn acquire_environment_strings_w(
        &mut self,
    ) -> Result<RuntimeEnvironmentStringCopy, RuntimeStateError> {
        if self.is_exiting() {
            return Err(RuntimeStateError::conflict(
                "runtime-process-environment",
                "wide-environment-acquire-lifecycle-state-invalid",
                "wide environment strings cannot be acquired during process exit",
            ));
        }
        let environment_strings = self.environment_strings_mut()?;
        if environment_strings.active_copy.is_some() {
            return Err(RuntimeStateError::resource_limit(
                "runtime-process-environment",
                "wide-environment-copy-limit-reached",
                "the controlled profile permits one active wide environment-string copy",
            ));
        }
        let next_acquisition_id = environment_strings
            .next_acquisition_id
            .checked_add(1)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-process-environment",
                    "wide-environment-acquisition-id-exhausted",
                    "wide environment-string acquisition ID space is exhausted",
                )
            })?;
        let copy = RuntimeEnvironmentStringCopy {
            acquisition_id: environment_strings.next_acquisition_id,
            buffer_address: environment_strings.copy_buffer_address,
            byte_length_with_double_nul: environment_strings.byte_length_with_double_nul,
        };
        environment_strings.next_acquisition_id = next_acquisition_id;
        environment_strings.active_copy = Some(copy.clone());
        Ok(copy)
    }

    pub(super) fn free_environment_strings_w(
        &mut self,
        buffer_address: GuestAddress,
    ) -> Result<RuntimeEnvironmentStringCopy, RuntimeStateError> {
        let environment_strings = self.environment_strings_mut()?;
        let copy = environment_strings.active_copy.as_ref().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-process-environment",
                "wide-environment-copy-not-active",
                "no wide environment-string copy is active",
            )
        })?;
        if copy.buffer_address != buffer_address {
            return Err(RuntimeStateError::invalid_input(
                "runtime-process-environment",
                "wide-environment-copy-pointer-invalid",
                "FreeEnvironmentStringsW requires the active process-owned copy pointer",
            ));
        }
        let copy = copy.clone();
        environment_strings.active_copy = None;
        Ok(copy)
    }

    fn environment_strings_mut(
        &mut self,
    ) -> Result<&mut RuntimeEnvironmentStrings, RuntimeStateError> {
        self.environment
            .as_mut()
            .and_then(|environment| environment.environment_strings.as_mut())
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-process-environment",
                    "wide-environment-strings-unregistered",
                    "runtime process has no registered wide environment-string authority",
                )
            })
    }

    pub(super) fn ensure_standard_handle(
        &mut self,
        selector: u32,
    ) -> Result<&RuntimeStandardHandle, RuntimeStateError> {
        if !self.is_running() {
            return Err(RuntimeStateError::conflict(
                "runtime-standard-handle",
                "standard-handle-lifecycle-state-invalid",
                "standard handle lookup requires a running process",
            ));
        }
        let (handle, variant_id) = match selector {
            RUNTIME_STANDARD_INPUT_SELECTOR => (
                RUNTIME_STANDARD_INPUT_HANDLE,
                RUNTIME_STANDARD_INPUT_VARIANT,
            ),
            RUNTIME_STANDARD_OUTPUT_SELECTOR => (
                RUNTIME_STANDARD_OUTPUT_HANDLE,
                RUNTIME_STANDARD_OUTPUT_VARIANT,
            ),
            RUNTIME_STANDARD_ERROR_SELECTOR => (
                RUNTIME_STANDARD_ERROR_HANDLE,
                RUNTIME_STANDARD_ERROR_VARIANT,
            ),
            _ => {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-standard-handle",
                    "standard-handle-selector-unsupported",
                    "only the reached standard input, output, and error selectors are supported",
                ));
            }
        };
        Ok(self
            .standard_handles
            .entry(selector)
            .or_insert_with(|| RuntimeStandardHandle {
                owner: RUNTIME_STANDARD_HANDLE_OWNER.to_owned(),
                variant_id: variant_id.to_owned(),
                namespace: LOGICAL_STANDARD_HANDLE_NAMESPACE.to_owned(),
                handle,
                selector,
            }))
    }

    pub(super) fn standard_handle_by_value(
        &self,
        handle: u32,
    ) -> Result<&RuntimeStandardHandle, RuntimeStateError> {
        self.standard_handles
            .values()
            .find(|standard_handle| standard_handle.handle == handle)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-standard-handle",
                    "standard-handle-unknown",
                    format!("logical standard handle {handle:#010x} is not registered"),
                )
            })
    }
}

fn validate_path_authority(
    source_authority: &str,
    encoding: &str,
) -> Result<(), RuntimeStateError> {
    if source_authority.is_empty() || encoding != RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING {
        return Err(RuntimeStateError::invalid_input(
            "runtime-process-directory",
            "current-directory-authority-invalid",
            "process directory paths require a nonempty authority and Windows-932 encoding",
        ));
    }
    Ok(())
}

fn process_image_directory(
    image_path: &RuntimeProcessImagePathA,
) -> Result<RuntimeCurrentDirectoryA, RuntimeStateError> {
    let normalized = normalize_absolute_directory_path(&image_path.bytes_with_nul)?;
    let body = &normalized.bytes_with_nul[..normalized.bytes_with_nul.len() - 1];
    let separator = last_separator_outside_windows_932(body).ok_or_else(|| {
        RuntimeStateError::invalid_input(
            "runtime-process-directory",
            "process-image-directory-missing",
            "process image path has no DOS directory component",
        )
    })?;
    if separator + 1 >= body.len() {
        return Err(RuntimeStateError::invalid_input(
            "runtime-process-directory",
            "process-image-name-missing",
            "process image path must end in a file name",
        ));
    }
    let directory_end = if separator == 2 { 3 } else { separator };
    let mut bytes_with_nul = body[..directory_end].to_vec();
    bytes_with_nul.push(0);
    let normalized_directory = normalize_absolute_directory_path(&bytes_with_nul)?;
    Ok(RuntimeCurrentDirectoryA {
        source_authority: PROCESS_IMAGE_DIRECTORY_AUTHORITY.to_owned(),
        bytes_with_nul: normalized_directory.bytes_with_nul,
        encoding: RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
    })
}

fn normalize_absolute_directory_path(
    bytes_with_nul: &[u8],
) -> Result<NormalizedDirectoryPath, RuntimeStateError> {
    if bytes_with_nul.len() < 2
        || bytes_with_nul.len() > RUNTIME_PROCESS_IMAGE_PATH_A_MAX_BYTES
        || bytes_with_nul.last() != Some(&0)
        || bytes_with_nul[..bytes_with_nul.len() - 1].contains(&0)
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-process-directory",
            "current-directory-path-shape-invalid",
            "process current directory requires one bounded trailing-NUL ANSI path",
        ));
    }
    let body = &bytes_with_nul[..bytes_with_nul.len() - 1];
    if body.len() < 3 || !body[0].is_ascii_alphabetic() || body[1] != b':' || !is_separator(body[2])
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-process-directory",
            "current-directory-path-not-absolute",
            "process current-directory subset requires an absolute DOS drive path",
        ));
    }

    let mut components = Vec::<Vec<u8>>::new();
    let mut component = Vec::new();
    let mut index = 3;
    while index < body.len() {
        let byte = body[index];
        if is_separator(byte) {
            if !component.is_empty() {
                validate_component(&component)?;
                components.push(std::mem::take(&mut component));
            }
            index += 1;
            continue;
        }
        if is_windows_932_lead_byte(byte) {
            let Some(&trail) = body.get(index + 1) else {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-process-directory",
                    "current-directory-windows-932-sequence-invalid",
                    "process current-directory path ends in a Windows-932 lead byte",
                ));
            };
            if !is_windows_932_trail_byte(trail) {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-process-directory",
                    "current-directory-windows-932-sequence-invalid",
                    "process current-directory path contains an invalid Windows-932 trail byte",
                ));
            }
            component.extend_from_slice(&[byte, trail]);
            index += 2;
            continue;
        }
        if byte < 0x20
            || byte == 0x7f
            || matches!(byte, b'"' | b'<' | b'>' | b'|' | b'?' | b'*' | b':')
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-process-directory",
                "current-directory-character-invalid",
                "process current-directory path contains a disallowed DOS path character",
            ));
        }
        component.push(byte);
        index += 1;
    }
    if !component.is_empty() {
        validate_component(&component)?;
        components.push(component);
    }

    let mut normalized = vec![body[0].to_ascii_uppercase(), b':', b'\\'];
    for (component_index, component) in components.iter().enumerate() {
        if component_index != 0 {
            normalized.push(b'\\');
        }
        normalized.extend_from_slice(component);
    }
    normalized.push(0);
    let identity_key = windows_932_ascii_fold(&normalized[..normalized.len() - 1]);
    Ok(NormalizedDirectoryPath {
        bytes_with_nul: normalized,
        identity_key,
    })
}

fn validate_component(component: &[u8]) -> Result<(), RuntimeStateError> {
    if component == b"." || component == b".." {
        return Err(RuntimeStateError::invalid_input(
            "runtime-process-directory",
            "current-directory-relative-component-unsupported",
            "process current-directory subset does not resolve dot components without a VFS",
        ));
    }
    Ok(())
}

fn last_separator_outside_windows_932(bytes: &[u8]) -> Option<usize> {
    let mut last = None;
    let mut index = 0;
    while index < bytes.len() {
        if is_windows_932_lead_byte(bytes[index])
            && bytes
                .get(index + 1)
                .is_some_and(|byte| is_windows_932_trail_byte(*byte))
        {
            index += 2;
        } else {
            if is_separator(bytes[index]) {
                last = Some(index);
            }
            index += 1;
        }
    }
    last
}

fn windows_932_ascii_fold(bytes: &[u8]) -> Vec<u8> {
    let mut folded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if is_windows_932_lead_byte(byte)
            && bytes
                .get(index + 1)
                .is_some_and(|trail| is_windows_932_trail_byte(*trail))
        {
            folded.extend_from_slice(&bytes[index..index + 2]);
            index += 2;
        } else {
            folded.push(byte.to_ascii_lowercase());
            index += 1;
        }
    }
    folded
}

const fn is_separator(byte: u8) -> bool {
    matches!(byte, b'\\' | b'/')
}

const fn is_windows_932_lead_byte(byte: u8) -> bool {
    matches!(byte, 0x81..=0x9f | 0xe0..=0xfc)
}

const fn is_windows_932_trail_byte(byte: u8) -> bool {
    matches!(byte, 0x40..=0x7e | 0x80..=0xfc)
}

const fn ranges_overlap(left_start: u64, left_end: u64, right_start: u64, right_end: u64) -> bool {
    left_start < right_end && right_start < left_end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn current_directory(bytes: &[u8]) -> RuntimeCurrentDirectoryA {
        RuntimeCurrentDirectoryA {
            source_authority: "process-test-bootstrap".to_owned(),
            bytes_with_nul: bytes.to_vec(),
            encoding: RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        }
    }

    fn image_path(bytes: &[u8]) -> RuntimeProcessImagePathA {
        RuntimeProcessImagePathA {
            source_authority: "process-test-image".to_owned(),
            bytes_with_nul: bytes.to_vec(),
            encoding: RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        }
    }

    fn process_state() -> RuntimeProcessState {
        let image = image_path(b"C:\\Vine\\BGI.exe\0");
        let directories =
            RuntimeProcessDirectories::new(current_directory(b"C:\\Vine\0"), &image).unwrap();
        RuntimeProcessState::new(
            RuntimeProcessId(0x1000),
            ProcessAddressSpaceLayout::vine_v1(),
            RuntimeClockState {
                wall_clock_file_time: 1,
                tick_count: 2,
                performance_counter: 3,
            },
            RuntimeStartupInfo {
                profile_id: "process-unit-test".to_owned(),
                bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
            },
            image,
            directories,
        )
    }

    #[test]
    fn tick_count_sampling_plans_progress_wrap_and_reject_stale_commit() {
        let mut process = process_state();
        let start = process.plan_start().unwrap();
        process.preflight_mutation_commit(&start).unwrap();
        process.commit_mutation(start);

        let before = process.clone();
        let first = process.plan_tick_count_sample(1).unwrap();
        assert_eq!(process, before, "clock sampling plan must be mutation-free");
        assert_eq!(process.commit_tick_count_sample(first), Ok(2));
        assert_eq!(process.clock().tick_count, 3);

        let stale = process.plan_tick_count_sample(1).unwrap();
        let committed = process.plan_tick_count_sample(2).unwrap();
        assert_eq!(process.commit_tick_count_sample(committed), Ok(3));
        assert_eq!(process.clock().tick_count, 5);
        assert_eq!(
            process.commit_tick_count_sample(stale).unwrap_err().code,
            "tick-count-sample-plan-stale"
        );
        assert_eq!(process.clock().tick_count, 5);

        process.clock.tick_count = u32::MAX;
        let wrapping = process.plan_tick_count_sample(1).unwrap();
        assert_eq!(process.commit_tick_count_sample(wrapping), Ok(u32::MAX));
        assert_eq!(process.clock().tick_count, 0);

        let stale_lifecycle = process.plan_tick_count_sample(1).unwrap();
        let exit = process
            .plan_exit_request(RuntimeThreadId(0x2000), 7)
            .unwrap();
        process.preflight_mutation_commit(&exit).unwrap();
        process.commit_mutation(exit);
        let before_stale_lifecycle = process.clone();
        assert_eq!(
            process
                .commit_tick_count_sample(stale_lifecycle)
                .unwrap_err()
                .code,
            "tick-count-sample-plan-stale"
        );
        assert_eq!(process, before_stale_lifecycle);
    }

    #[test]
    fn current_directory_plan_is_copy_owned_and_commits_only_after_preflight() {
        let mut directories = RuntimeProcessDirectories::new(
            current_directory(b"D:\\Launcher\0"),
            &image_path(b"C:\\Vine\\BGI.exe\0"),
        )
        .unwrap();
        let before = directories.clone();
        let mut request = b"c:/VINE/\0".to_vec();
        let plan = directories.plan_process_image_directory(&request).unwrap();
        request.fill(b'X');
        assert_eq!(directories, before);
        assert_eq!(
            directories.commit(plan).unwrap(),
            &current_directory(b"C:\\Vine\0").with_authority(PROCESS_IMAGE_DIRECTORY_AUTHORITY)
        );
        assert_eq!(directories.current_directory_generation(), 1);
    }

    #[test]
    fn rejected_and_stale_plans_preserve_directory_state() {
        let mut directories = RuntimeProcessDirectories::new(
            current_directory(b"C:\\Vine\0"),
            &image_path(b"C:\\Vine\\BGI.exe\0"),
        )
        .unwrap();
        let before = directories.clone();
        assert_eq!(
            directories
                .plan_process_image_directory(b"C:\\Other\0")
                .unwrap_err()
                .code,
            "process-image-directory-not-registered"
        );
        assert_eq!(
            directories
                .plan_process_image_directory(b"..\0")
                .unwrap_err()
                .code,
            "current-directory-path-not-absolute"
        );
        assert_eq!(directories, before);

        let stale = directories
            .plan_process_image_directory(b"C:\\Vine\\\0")
            .unwrap();
        let first = directories
            .plan_process_image_directory(b"C:\\Vine\0")
            .unwrap();
        directories.commit(first).unwrap();
        let after_first = directories.clone();
        assert_eq!(
            directories.commit(stale).unwrap_err().code,
            "current-directory-plan-stale"
        );
        assert_eq!(directories, after_first);
    }

    #[test]
    fn windows_932_trail_backslash_is_not_a_directory_separator() {
        let image = image_path(&[
            b'C', b':', b'\\', 0x81, 0x5c, b'\\', b'B', b'G', b'I', b'.', b'e', b'x', b'e', 0,
        ]);
        let directories = RuntimeProcessDirectories::new(
            current_directory(&[b'C', b':', b'\\', 0x81, 0x5c, 0]),
            &image,
        )
        .unwrap();
        assert!(
            directories
                .is_process_image_directory(&[b'c', b':', b'/', 0x81, 0x5c, b'/', 0])
                .unwrap()
        );
    }

    #[test]
    fn lifecycle_plans_are_mutation_free_and_stale_commits_preserve_process_state() {
        let mut process = process_state();
        let bootstrap = process.clone();
        let start = process.plan_start().unwrap();
        assert_eq!(process, bootstrap);
        process.preflight_mutation_commit(&start).unwrap();
        process.commit_mutation(start);
        assert_eq!(process.lifecycle(), RuntimeProcessLifecycleState::Running);

        let stale_exit = process
            .plan_exit_request(RuntimeThreadId(0x2000), 7)
            .unwrap();
        process
            .set_unhandled_exception_filter(Some(GuestAddress(0x1000_1234)))
            .unwrap();
        let before_stale_commit = process.clone();
        assert_eq!(
            process
                .preflight_mutation_commit(&stale_exit)
                .unwrap_err()
                .code,
            "process-mutation-plan-stale"
        );
        assert_eq!(process, before_stale_commit);

        let exit = process
            .plan_exit_request(RuntimeThreadId(0x2000), 7)
            .unwrap();
        process.preflight_mutation_commit(&exit).unwrap();
        process.commit_mutation(exit);
        let completion = process
            .plan_exit_completion(RuntimeThreadId(0x2000))
            .unwrap();
        process.preflight_mutation_commit(&completion).unwrap();
        process.commit_mutation(completion);
        assert_eq!(
            process.lifecycle(),
            RuntimeProcessLifecycleState::Exited { exit_code: 7 }
        );
        assert_eq!(process.unhandled_exception_filter(), None);
    }

    #[test]
    fn standard_handle_and_environment_copy_collections_are_process_private() {
        let mut process = process_state();
        let start = process.plan_start().unwrap();
        process.preflight_mutation_commit(&start).unwrap();
        process.commit_mutation(start);

        let input = process
            .ensure_standard_handle(RUNTIME_STANDARD_INPUT_SELECTOR)
            .unwrap()
            .clone();
        assert_eq!(input.handle, RUNTIME_STANDARD_INPUT_HANDLE);
        assert_eq!(process.standard_handles().len(), 1);
        let before_rejection = process.clone();
        assert_eq!(
            process.ensure_standard_handle(0).unwrap_err().code,
            "standard-handle-selector-unsupported"
        );
        assert_eq!(process, before_rejection);

        process.environment = Some(RuntimeProcessEnvironment {
            address_allocation_id: ProcessAddressAllocationId(37),
            range: RuntimeAddressRange::new(GuestAddress(0x7ffd_c000), 0x4000).unwrap(),
            command_line: RuntimeCommandLine {
                source_authority: "process-unit-test".to_owned(),
                wide_buffer_address: GuestAddress(0x7ffd_d100),
                wide_length_bytes: 4,
                ansi_buffer_address: GuestAddress(0x7ffd_d200),
                ansi_byte_length_with_nul: 3,
                ansi_encoding: CONTROLLED_ANSI_COMMAND_LINE_ENCODING.to_owned(),
            },
            environment_strings: Some(RuntimeEnvironmentStrings {
                source_authority: "process-unit-test".to_owned(),
                authority_buffer_address: GuestAddress(0x7ffd_d300),
                copy_buffer_address: GuestAddress(0x7ffd_d304),
                byte_length_with_double_nul: 4,
                encoding: CONTROLLED_WIDE_ENVIRONMENT_ENCODING.to_owned(),
                active_copy: None,
                next_acquisition_id: 1,
            }),
        });
        let copy = process.acquire_environment_strings_w().unwrap();
        let acquired = process.clone();
        assert_eq!(
            process.acquire_environment_strings_w().unwrap_err().code,
            "wide-environment-copy-limit-reached"
        );
        assert_eq!(process, acquired);
        assert_eq!(
            process
                .free_environment_strings_w(GuestAddress(copy.buffer_address.0 + 4))
                .unwrap_err()
                .code,
            "wide-environment-copy-pointer-invalid"
        );
        assert_eq!(process, acquired);
        assert_eq!(
            process
                .free_environment_strings_w(copy.buffer_address)
                .unwrap(),
            copy
        );
        assert!(process.environment_strings().unwrap().active_copy.is_none());
    }

    trait WithAuthority {
        fn with_authority(self, authority: &str) -> Self;
    }

    impl WithAuthority for RuntimeCurrentDirectoryA {
        fn with_authority(mut self, authority: &str) -> Self {
            self.source_authority = authority.to_owned();
            self
        }
    }
}

use std::collections::BTreeSet;

use crate::memory::GuestAddress;

use super::RuntimeStateError;
use super::clock::RuntimeClock;

pub const X86_STARTUP_INFO_W_BYTE_LENGTH: usize = 68;
pub const RUNTIME_STANDARD_INPUT_SELECTOR: u32 = 0xffff_fff6;
pub const RUNTIME_STANDARD_INPUT_HANDLE: u32 = 0x5301_0001;
pub const RUNTIME_STANDARD_OUTPUT_SELECTOR: u32 = 0xffff_fff5;
pub const RUNTIME_STANDARD_OUTPUT_HANDLE: u32 = 0x5301_0002;
pub const RUNTIME_STANDARD_ERROR_SELECTOR: u32 = 0xffff_fff4;
pub const RUNTIME_STANDARD_ERROR_HANDLE: u32 = 0x5301_0003;
pub const RUNTIME_PROCESS_IMAGE_PATH_A_MAX_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeProcessId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStartupInfo {
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub struct RuntimeCommandLine {
    pub ansi_buffer_address: GuestAddress,
    pub ansi_byte_length_with_nul: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProcessImagePathA {
    pub bytes_with_nul: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEnvironmentStrings {
    pub authority_buffer_address: GuestAddress,
    pub copy_buffer_address: GuestAddress,
    pub byte_length_with_double_nul: u32,
    pub copy_active: bool,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub struct RuntimeProcessEnvironment {
    pub initial_teb_address: GuestAddress,
    pub peb_address: GuestAddress,
    pub command_line: RuntimeCommandLine,
    pub environment_strings: RuntimeEnvironmentStrings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProcessLifecycleState {
    Bootstrapping,
    Running,
}

#[derive(Debug)]
pub struct RuntimeProcessState {
    id: RuntimeProcessId,
    lifecycle: RuntimeProcessLifecycleState,
    clock: RuntimeClock,
    startup_info: RuntimeStartupInfo,
    image_path_a: RuntimeProcessImagePathA,
    directories: RuntimeProcessDirectories,
    environment: Option<RuntimeProcessEnvironment>,
    unhandled_exception_filter: Option<GuestAddress>,
    heap_termination_on_corruption: bool,
    standard_handles: BTreeSet<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCurrentDirectoryA {
    pub bytes_with_nul: Vec<u8>,
}

#[derive(Debug)]
pub(super) struct RuntimeProcessDirectories {
    current_directory_a: RuntimeCurrentDirectoryA,
    process_image_directory_a: RuntimeCurrentDirectoryA,
    process_image_directory_key: Vec<u8>,
}

#[derive(Debug)]
struct NormalizedDirectoryPath {
    bytes_with_nul: Vec<u8>,
    identity_key: Vec<u8>,
}

impl RuntimeProcessDirectories {
    pub(super) fn new(
        mut initial: RuntimeCurrentDirectoryA,
        image_path: &RuntimeProcessImagePathA,
    ) -> Result<Self, RuntimeStateError> {
        let normalized_initial = normalize_absolute_directory_path(&initial.bytes_with_nul)?;
        initial.bytes_with_nul = normalized_initial.bytes_with_nul;

        let image_directory = process_image_directory(image_path)?;
        let image_directory_key =
            normalize_absolute_directory_path(&image_directory.bytes_with_nul)?.identity_key;
        Ok(Self {
            current_directory_a: initial,
            process_image_directory_a: image_directory,
            process_image_directory_key: image_directory_key,
        })
    }

    pub(super) fn current_directory_a(&self) -> &RuntimeCurrentDirectoryA {
        &self.current_directory_a
    }

    pub(super) fn is_process_image_directory(
        &self,
        requested: &[u8],
    ) -> Result<bool, RuntimeStateError> {
        Ok(normalize_absolute_directory_path(requested)?.identity_key
            == self.process_image_directory_key)
    }

    pub(super) fn is_process_image_drive_root(&self, requested: Option<&[u8]>) -> bool {
        let requested_drive = match requested {
            None => self.current_directory_a.bytes_with_nul.first().copied(),
            Some([drive, b':', separator, 0])
                if drive.is_ascii_alphabetic() && is_separator(*separator) =>
            {
                Some(*drive)
            }
            Some(_) => None,
        };
        requested_drive.is_some_and(|drive| {
            self.process_image_directory_key
                .first()
                .is_some_and(|image_drive| drive.eq_ignore_ascii_case(image_drive))
        })
    }

    pub(super) fn set_process_image_directory(
        &mut self,
        requested: &[u8],
    ) -> Result<(), RuntimeStateError> {
        if !self.is_process_image_directory(requested)? {
            return Err(RuntimeStateError::new(
                "changing directory away from the executable's directory is not yet supported",
            ));
        }
        self.current_directory_a = self.process_image_directory_a.clone();
        Ok(())
    }
}

impl RuntimeProcessState {
    pub(super) fn new(
        id: RuntimeProcessId,
        clock: super::RuntimeClockState,
        startup_info: RuntimeStartupInfo,
        image_path_a: RuntimeProcessImagePathA,
        directories: RuntimeProcessDirectories,
    ) -> Self {
        Self {
            id,
            lifecycle: RuntimeProcessLifecycleState::Bootstrapping,
            clock: RuntimeClock::new(clock).expect("validated runtime clock bootstrap"),
            startup_info,
            image_path_a,
            directories,
            environment: None,
            unhandled_exception_filter: None,
            heap_termination_on_corruption: false,
            standard_handles: BTreeSet::new(),
        }
    }

    pub fn id(&self) -> super::RuntimeProcessId {
        self.id
    }

    #[cfg(test)]
    pub(crate) fn lifecycle(&self) -> RuntimeProcessLifecycleState {
        self.lifecycle
    }

    pub fn clock(&self) -> &super::RuntimeClockState {
        self.clock.state()
    }

    pub fn timer_time_100ns(&self) -> u64 {
        self.clock.timer_time_100ns()
    }

    pub fn synchronized_instruction_count(&self) -> u64 {
        self.clock.synchronized_instruction_count()
    }

    pub(super) fn advance_clock(
        &mut self,
        target_time_100ns: u64,
    ) -> Result<u64, RuntimeStateError> {
        self.clock.advance(target_time_100ns)
    }

    pub(super) fn project_clock_advance(
        &self,
        target_time_100ns: u64,
    ) -> Result<(u64, super::RuntimeClockState), RuntimeStateError> {
        self.clock.project_advance(target_time_100ns)
    }

    pub(super) fn apply_clock_advance(&mut self, projection: (u64, super::RuntimeClockState)) {
        self.clock.apply_advance(projection);
    }

    pub(super) fn advance_scheduled_host_execution_clock(
        &mut self,
        retired_instructions: u64,
        host_elapsed_time_100ns: u64,
    ) -> Result<u64, RuntimeStateError> {
        self.clock
            .advance_scheduled_host_execution(retired_instructions, host_elapsed_time_100ns)
    }

    pub(super) fn sample_tick_count(&self) -> Result<u32, RuntimeStateError> {
        if !self.is_running() {
            return Err(RuntimeStateError::new(
                "tick-count sampling requires a running process",
            ));
        }
        Ok(self.clock.state().tick_count)
    }

    pub(super) fn sample_performance_counter(&self) -> Result<u64, RuntimeStateError> {
        if !self.is_running() {
            return Err(RuntimeStateError::new(
                "performance-counter sampling requires a running process",
            ));
        }
        Ok(self.clock.state().performance_counter)
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

    #[cfg(test)]
    pub fn heap_termination_on_corruption(&self) -> bool {
        self.heap_termination_on_corruption
    }

    #[cfg(test)]
    pub fn standard_handles(&self) -> &BTreeSet<u32> {
        &self.standard_handles
    }

    pub(super) const fn is_bootstrapping(&self) -> bool {
        matches!(self.lifecycle, RuntimeProcessLifecycleState::Bootstrapping)
    }

    pub(super) const fn is_running(&self) -> bool {
        matches!(self.lifecycle, RuntimeProcessLifecycleState::Running)
    }

    pub(super) fn start(&mut self) -> Result<(), RuntimeStateError> {
        if !self.is_bootstrapping() {
            return Err(RuntimeStateError::new(
                "initial thread start requires a bootstrapping process",
            ));
        }
        self.lifecycle = RuntimeProcessLifecycleState::Running;
        Ok(())
    }

    // The aggregate has already checked the exit request or direct termination
    // and validated every subsystem's teardown before reaching this mutation.
    pub(super) fn set_unhandled_exception_filter(
        &mut self,
        filter: Option<GuestAddress>,
    ) -> Result<Option<GuestAddress>, RuntimeStateError> {
        if !self.is_running() {
            return Err(RuntimeStateError::new(
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

    pub(super) fn is_process_image_drive_root_a(&self, requested: Option<&[u8]>) -> bool {
        self.directories.is_process_image_drive_root(requested)
    }

    pub(super) fn set_current_directory_a(
        &mut self,
        requested: &[u8],
    ) -> Result<(), RuntimeStateError> {
        if !self.is_running() {
            return Err(RuntimeStateError::new(
                "current-directory replacement requires a running process",
            ));
        }
        self.directories.set_process_image_directory(requested)
    }

    pub(super) fn install_environment(&mut self, environment: RuntimeProcessEnvironment) {
        self.environment = Some(environment);
    }

    pub(super) fn command_line(&self) -> Result<&RuntimeCommandLine, RuntimeStateError> {
        self.environment
            .as_ref()
            .map(|environment| &environment.command_line)
            .ok_or_else(|| {
                RuntimeStateError::new("runtime process has no registered command-line authority")
            })
    }

    pub(super) fn environment_strings(
        &self,
    ) -> Result<&RuntimeEnvironmentStrings, RuntimeStateError> {
        self.environment
            .as_ref()
            .map(|environment| &environment.environment_strings)
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "runtime process has no registered wide environment-string authority",
                )
            })
    }

    pub(super) fn acquire_environment_strings_w(
        &mut self,
    ) -> Result<GuestAddress, RuntimeStateError> {
        let environment = self.environment_strings_mut()?;
        if environment.copy_active {
            return Err(RuntimeStateError::new(
                "only one wide environment-string copy can be active",
            ));
        }
        environment.copy_active = true;
        Ok(environment.copy_buffer_address)
    }

    pub(super) fn free_environment_strings_w(
        &mut self,
        buffer_address: GuestAddress,
    ) -> Result<(), RuntimeStateError> {
        let environment = self.environment_strings_mut()?;
        if !environment.copy_active {
            return Err(RuntimeStateError::new(
                "no wide environment-string copy is active",
            ));
        }
        if environment.copy_buffer_address != buffer_address {
            return Err(RuntimeStateError::new(
                "FreeEnvironmentStringsW requires the active process-owned copy pointer",
            ));
        }
        environment.copy_active = false;
        Ok(())
    }

    fn environment_strings_mut(
        &mut self,
    ) -> Result<&mut RuntimeEnvironmentStrings, RuntimeStateError> {
        self.environment
            .as_mut()
            .map(|environment| &mut environment.environment_strings)
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "runtime process has no registered wide environment-string authority",
                )
            })
    }

    pub(super) fn ensure_standard_handle(
        &mut self,
        selector: u32,
    ) -> Result<u32, RuntimeStateError> {
        if !self.is_running() {
            return Err(RuntimeStateError::new(
                "standard handle lookup requires a running process",
            ));
        }
        let handle = match selector {
            RUNTIME_STANDARD_INPUT_SELECTOR => RUNTIME_STANDARD_INPUT_HANDLE,
            RUNTIME_STANDARD_OUTPUT_SELECTOR => RUNTIME_STANDARD_OUTPUT_HANDLE,
            RUNTIME_STANDARD_ERROR_SELECTOR => RUNTIME_STANDARD_ERROR_HANDLE,
            _ => return Ok(u32::MAX),
        };
        self.standard_handles.insert(selector);
        Ok(handle)
    }

    pub(super) fn standard_file_type(&self, handle: u32) -> Option<u32> {
        let (selector, file_type) = match handle {
            RUNTIME_STANDARD_INPUT_HANDLE => (RUNTIME_STANDARD_INPUT_SELECTOR, 2),
            RUNTIME_STANDARD_OUTPUT_HANDLE => (RUNTIME_STANDARD_OUTPUT_SELECTOR, 3),
            RUNTIME_STANDARD_ERROR_HANDLE => (RUNTIME_STANDARD_ERROR_SELECTOR, 3),
            _ => return None,
        };
        self.standard_handles
            .contains(&selector)
            .then_some(file_type)
    }
}

fn process_image_directory(
    image_path: &RuntimeProcessImagePathA,
) -> Result<RuntimeCurrentDirectoryA, RuntimeStateError> {
    let normalized = normalize_absolute_directory_path(&image_path.bytes_with_nul)?;
    let body = &normalized.bytes_with_nul[..normalized.bytes_with_nul.len() - 1];
    let separator = last_separator_outside_windows_932(body).ok_or_else(|| {
        RuntimeStateError::new("process image path has no DOS directory component")
    })?;
    if separator + 1 >= body.len() {
        return Err(RuntimeStateError::new(
            "process image path must end in a file name",
        ));
    }
    let directory_end = if separator == 2 { 3 } else { separator };
    let mut bytes_with_nul = body[..directory_end].to_vec();
    bytes_with_nul.push(0);
    let normalized_directory = normalize_absolute_directory_path(&bytes_with_nul)?;
    Ok(RuntimeCurrentDirectoryA {
        bytes_with_nul: normalized_directory.bytes_with_nul,
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
        return Err(RuntimeStateError::new(
            "process current directory requires one bounded trailing-NUL ANSI path",
        ));
    }
    let body = &bytes_with_nul[..bytes_with_nul.len() - 1];
    if body.len() < 3 || !body[0].is_ascii_alphabetic() || body[1] != b':' || !is_separator(body[2])
    {
        return Err(RuntimeStateError::new(
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
                return Err(RuntimeStateError::new(
                    "process current-directory path ends in a Windows-932 lead byte",
                ));
            };
            if !is_windows_932_trail_byte(trail) {
                return Err(RuntimeStateError::new(
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
            return Err(RuntimeStateError::new(
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
        return Err(RuntimeStateError::new(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RuntimeClockState;

    fn current_directory(bytes: &[u8]) -> RuntimeCurrentDirectoryA {
        RuntimeCurrentDirectoryA {
            bytes_with_nul: bytes.to_vec(),
        }
    }

    fn image_path(bytes: &[u8]) -> RuntimeProcessImagePathA {
        RuntimeProcessImagePathA {
            bytes_with_nul: bytes.to_vec(),
        }
    }

    fn process_state() -> RuntimeProcessState {
        let image = image_path(b"C:\\Vine\\fixture.exe\0");
        let directories =
            RuntimeProcessDirectories::new(current_directory(b"C:\\Vine\0"), &image).unwrap();
        RuntimeProcessState::new(
            RuntimeProcessId(0x1000),
            RuntimeClockState {
                wall_clock_file_time: 1,
                tick_count: 2,
                performance_counter: 3,
                performance_frequency: 4,
            },
            RuntimeStartupInfo {
                bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
            },
            image,
            directories,
        )
    }

    #[test]
    fn clock_sampling_observes_time_without_mutation_and_checks_lifecycle() {
        let mut process = process_state();
        assert!(process.sample_tick_count().is_err());
        assert!(process.sample_performance_counter().is_err());
        process.start().unwrap();
        for _ in 0..2 {
            assert_eq!(process.sample_tick_count(), Ok(2));
            assert_eq!(process.sample_performance_counter(), Ok(3));
        }
        assert_eq!(process.lifecycle(), RuntimeProcessLifecycleState::Running);
    }

    #[test]
    fn current_directory_update_normalizes_without_retaining_caller_bytes() {
        let mut directories = RuntimeProcessDirectories::new(
            current_directory(b"D:\\Launcher\0"),
            &image_path(b"C:\\Vine\\fixture.exe\0"),
        )
        .unwrap();
        let mut request = b"c:/VINE/\0".to_vec();
        directories.set_process_image_directory(&request).unwrap();
        assert_eq!(
            directories.current_directory_a(),
            &current_directory(b"C:\\Vine\0")
        );
        request.fill(b'X');
        assert_eq!(
            directories.current_directory_a().bytes_with_nul,
            b"C:\\Vine\0"
        );
    }

    #[test]
    fn drive_root_query_derives_only_from_process_directory_identity() {
        let mut directories = RuntimeProcessDirectories::new(
            current_directory(b"D:\\Launcher\0"),
            &image_path(b"C:\\Vine\\fixture.exe\0"),
        )
        .unwrap();
        assert!(directories.is_process_image_drive_root(Some(b"C:\\\0")));
        assert!(directories.is_process_image_drive_root(Some(b"c:/\0")));
        assert!(!directories.is_process_image_drive_root(Some(b"D:\\\0")));
        assert!(!directories.is_process_image_drive_root(Some(b"C:\\Vine\0")));
        assert!(!directories.is_process_image_drive_root(Some(b".\0")));
        assert!(!directories.is_process_image_drive_root(None));

        directories
            .set_process_image_directory(b"C:\\Vine\0")
            .unwrap();
        assert!(directories.is_process_image_drive_root(None));
    }

    #[test]
    fn rejected_directory_updates_preserve_state() {
        let mut directories = RuntimeProcessDirectories::new(
            current_directory(b"C:\\Vine\0"),
            &image_path(b"C:\\Vine\\fixture.exe\0"),
        )
        .unwrap();
        directories
            .set_process_image_directory(b"C:\\Other\0")
            .unwrap_err();
        directories
            .set_process_image_directory(b"..\0")
            .unwrap_err();
        assert_eq!(
            directories.current_directory_a().bytes_with_nul,
            b"C:\\Vine\0"
        );
        assert!(
            directories
                .is_process_image_directory(b"C:\\Vine\0")
                .unwrap()
        );

        directories
            .set_process_image_directory(b"C:\\Vine\\\0")
            .unwrap();
        directories
            .set_process_image_directory(b"C:\\Vine\0")
            .unwrap();
        assert_eq!(
            directories.current_directory_a().bytes_with_nul,
            b"C:\\Vine\0"
        );
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
    fn startup_rejection_preserves_process_state_and_filter() {
        let mut process = process_state();
        assert_eq!(
            process.lifecycle(),
            RuntimeProcessLifecycleState::Bootstrapping
        );
        process.start().unwrap();
        assert_eq!(process.lifecycle(), RuntimeProcessLifecycleState::Running);
        process
            .set_unhandled_exception_filter(Some(GuestAddress(0x1000_1234)))
            .unwrap();
        assert!(process.start().is_err());
        assert_eq!(process.lifecycle(), RuntimeProcessLifecycleState::Running);
        assert_eq!(
            process.unhandled_exception_filter(),
            Some(GuestAddress(0x1000_1234))
        );
    }

    #[test]
    fn standard_handle_and_environment_copy_collections_are_process_private() {
        let mut process = process_state();
        process.start().unwrap();

        assert_eq!(
            process.standard_file_type(RUNTIME_STANDARD_INPUT_HANDLE),
            None
        );
        let input = process
            .ensure_standard_handle(RUNTIME_STANDARD_INPUT_SELECTOR)
            .unwrap();
        assert_eq!(input, RUNTIME_STANDARD_INPUT_HANDLE);
        assert_eq!(process.standard_file_type(input), Some(2));
        assert_eq!(process.standard_handles().len(), 1);
        assert_eq!(process.ensure_standard_handle(0).unwrap(), u32::MAX);
        assert_eq!(process.standard_handles().len(), 1);
        assert_eq!(
            process
                .ensure_standard_handle(RUNTIME_STANDARD_INPUT_SELECTOR)
                .unwrap(),
            input
        );
        assert_eq!(process.standard_handles().len(), 1);

        process.environment = Some(RuntimeProcessEnvironment {
            initial_teb_address: GuestAddress(0x7ffd_c000),
            peb_address: GuestAddress(0x7ffd_f000),
            command_line: RuntimeCommandLine {
                ansi_buffer_address: GuestAddress(0x7ffd_d200),
                ansi_byte_length_with_nul: 3,
            },
            environment_strings: RuntimeEnvironmentStrings {
                authority_buffer_address: GuestAddress(0x7ffd_d300),
                copy_buffer_address: GuestAddress(0x7ffd_d304),
                byte_length_with_double_nul: 4,
                copy_active: false,
            },
        });
        let copy = process.acquire_environment_strings_w().unwrap();
        process.acquire_environment_strings_w().unwrap_err();
        assert!(process.environment_strings().unwrap().copy_active);
        assert_eq!(
            process.environment_strings().unwrap().copy_buffer_address,
            copy
        );
        process
            .free_environment_strings_w(GuestAddress(copy.0 + 4))
            .unwrap_err();
        assert!(process.environment_strings().unwrap().copy_active);
        assert_eq!(
            process.environment_strings().unwrap().copy_buffer_address,
            copy
        );
        assert_eq!(process.free_environment_strings_w(copy).unwrap(), ());
        assert!(!process.environment_strings().unwrap().copy_active);
    }
}

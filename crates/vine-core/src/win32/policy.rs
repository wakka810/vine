use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::runtime::{
    RUNTIME_GUEST_THREAD_LIMIT, RUNTIME_PRIVATE_HEAP_ARENA_STRIDE,
    RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE, RUNTIME_SOURCE_READ_MAX_BYTES,
    RUNTIME_STANDARD_ERROR_SELECTOR, RUNTIME_STANDARD_INPUT_SELECTOR,
    RUNTIME_STANDARD_OUTPUT_SELECTOR, RuntimeThreadPriority,
};

pub const KERNEL32_COMPATIBILITY_POLICY_SCHEMA: &str = "vine.kernel32-compatibility-policy";
pub const KERNEL32_COMPATIBILITY_POLICY_VERSION: u32 = 2;

const KERNEL32_COMPATIBILITY_POLICY_LEGACY_VERSION: u32 = 1;

const HEAP_ZERO_MEMORY: u32 = 0x0000_0008;
const HEAP_NO_SERIALIZE: u32 = 0x0000_0001;
const CRITICAL_SECTION_DYNAMIC_SPIN_STATE: u32 = 0x0200_0000;
const MAX_POLICY_ID_BYTES: usize = 128;
const MAX_POLICY_ENTRIES: usize = 32;
const MAX_RUNTIME_SYSTEM_EXPORT_NAME_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32CompatibilityPolicy {
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub standard_handle_selectors: Vec<u32>,
    pub standard_handle_file_types: Vec<Kernel32StandardHandleFileType>,
    #[serde(default)]
    pub set_handle_counts: Vec<u32>,
    #[serde(default)]
    pub ansi_command_line: bool,
    #[serde(default)]
    pub wide_environment_strings: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wide_char_to_multi_byte: Option<Kernel32WideCharToMultiBytePolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multi_byte_to_wide_char: Option<Kernel32MultiByteToWideCharPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_current_directory_a: Option<Kernel32SetCurrentDirectoryAPolicy>,
    pub source_files: Option<Kernel32SourceFilePolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_library_a: Option<Kernel32LoadLibraryAPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub get_drive_type_a: Option<Kernel32GetDriveTypeAPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create_mutex_a: Option<Kernel32CreateMutexAPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create_waitable_timer_a: Option<Kernel32CreateWaitableTimerAPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_waitable_timer: Option<Kernel32SetWaitableTimerPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait_for_single_object_ex: Option<Kernel32WaitForSingleObjectExPolicy>,
    #[serde(default)]
    pub sleep_zero: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub get_string_type_w: Option<Kernel32GetStringTypeWPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lc_map_string_w: Option<Kernel32LcMapStringWPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub get_module_file_name_a: Option<Kernel32GetModuleFileNameAPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_code_page_info: Option<Kernel32CodePageInfoPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_default_lang_id: Option<u16>,
    #[serde(default)]
    pub processor_features: Vec<Kernel32ProcessorFeaturePolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_affinity: Option<Kernel32ThreadAffinityPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_priority: Option<Kernel32ThreadPriorityPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create_thread: Option<Kernel32CreateThreadPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_info: Option<Kernel32SystemInfoPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_status: Option<Kernel32MemoryStatusPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub virtual_alloc: Option<Kernel32VirtualAllocPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os_version_info_a: Option<Kernel32OsVersionInfoAPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tick_count: Option<Kernel32TickCountPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performance_counter: Option<Kernel32PerformanceCounterPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heap_allocator: Option<Kernel32HeapAllocatorPolicy>,
    #[serde(default)]
    pub heap_allocations: Vec<Kernel32HeapAllocationPolicy>,
    #[serde(default)]
    pub heap_free_flags: Vec<u32>,
    #[serde(default)]
    pub heap_size_flags: Vec<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initialize_critical_section: Option<Kernel32InitializeCriticalSectionPolicy>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32StandardHandleFileType {
    pub selector: u32,
    pub file_type: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32HeapAllocationPolicy {
    pub flags: u32,
    pub byte_lengths: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32HeapAllocatorPolicy {
    pub allowed_flags: Vec<u32>,
    pub maximum_allocation_bytes: u32,
    #[serde(default)]
    pub large_allocation_bytes: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32InitializeCriticalSectionPolicy {
    pub spin_count_field: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32WideCharToMultiBytePolicy {
    pub active_code_page: u32,
    pub maximum_source_code_units: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32MultiByteToWideCharPolicy {
    #[serde(default)]
    pub allow_active_code_page_selector: bool,
    pub allowed_flags: Vec<u32>,
    pub maximum_source_bytes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32SetCurrentDirectoryAPolicy {
    pub process_image_directory_only: bool,
    pub maximum_path_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32SourceFilePolicy {
    pub allowed_desired_access: Vec<u32>,
    pub allowed_share_modes: Vec<u32>,
    pub allowed_creation_dispositions: Vec<u32>,
    pub allowed_flags_and_attributes: Vec<u32>,
    pub maximum_path_bytes: u32,
    pub maximum_read_bytes: u32,
    #[serde(default)]
    pub maximum_write_bytes: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub get_file_attributes_a: Option<Kernel32GetFileAttributesAPolicy>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32GetFileAttributesAPolicy {
    pub regular_file_attributes: u32,
    pub directory_attributes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32LoadLibraryAPolicy {
    pub maximum_module_name_bytes: u32,
    pub runtime_system_modules: Vec<Kernel32RuntimeSystemModulePolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32RuntimeSystemModulePolicy {
    pub canonical_name: String,
    pub exports: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32GetDriveTypeAPolicy {
    pub process_image_drive_only: bool,
    pub maximum_path_bytes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32CreateMutexAPolicy {
    pub allow_named: bool,
    pub allow_unnamed: bool,
    pub maximum_name_bytes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32CreateWaitableTimerAPolicy {
    pub allow_manual_reset: bool,
    pub allow_auto_reset: bool,
    pub allow_unnamed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32SetWaitableTimerPolicy {
    pub minimum_relative_due_time_100ns: u64,
    pub maximum_relative_due_time_100ns: u64,
    pub allow_one_shot: bool,
    pub allow_completion_routine: bool,
    pub allow_resume: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32WaitForSingleObjectExPolicy {
    pub maximum_timeout_milliseconds: u32,
    pub allow_alertable: bool,
    pub allow_nonalertable: bool,
    pub waitable_timer_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32GetStringTypeWPolicy {
    pub allowed_info_types: Vec<u32>,
    pub maximum_source_code_units: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32LcMapStringWPolicy {
    pub allowed_locales: Vec<u32>,
    pub allowed_flags: Vec<u32>,
    pub maximum_source_code_units: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32GetModuleFileNameAPolicy {
    pub main_module_only: bool,
    pub maximum_capacity_bytes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32CodePageInfoPolicy {
    pub max_char_size: u32,
    pub default_char: [u8; 2],
    pub lead_byte: [u8; 12],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32ProcessorFeaturePolicy {
    pub index: u32,
    pub present: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32ThreadAffinityPolicy {
    pub process_mask: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32ThreadPriorityPolicy {
    pub allowed_priorities: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32CreateThreadPolicy {
    pub default_stack_reserve_bytes: u32,
    pub default_stack_commit_bytes: u32,
    pub allowed_creation_flags: Vec<u32>,
    pub maximum_threads: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32SystemInfoPolicy {
    pub processor_architecture: u16,
    pub page_size: u32,
    pub minimum_application_address: u32,
    pub maximum_application_address: u32,
    pub processor_type: u32,
    pub allocation_granularity: u32,
    pub processor_level: u16,
    pub processor_revision: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32MemoryStatusPolicy {
    pub memory_load_percent: u32,
    pub total_physical_bytes: u32,
    pub available_physical_bytes: u32,
    pub total_page_file_bytes: u32,
    pub available_page_file_bytes: u32,
    pub available_virtual_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32VirtualAllocPolicy {
    pub require_null_preferred_address: bool,
    pub allowed_reserve_byte_lengths: Vec<u32>,
    pub allowed_allocation_types: Vec<u32>,
    pub allowed_protections: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32OsVersionInfoAPolicy {
    pub major_version: u32,
    pub minor_version: u32,
    pub build_number: u32,
    pub platform_id: u32,
    pub service_pack: String,
    pub service_pack_major: u16,
    pub service_pack_minor: u16,
    pub suite_mask: u16,
    pub product_type: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32TickCountPolicy {
    pub increment_per_read: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32PerformanceCounterPolicy {
    pub increment_per_read: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kernel32CompatibilityPolicyError {
    pub code: &'static str,
    pub message: String,
}

impl fmt::Display for Kernel32CompatibilityPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Kernel32CompatibilityPolicyError {}

impl Kernel32CompatibilityPolicy {
    pub fn controlled_v1() -> Self {
        Self {
            schema: KERNEL32_COMPATIBILITY_POLICY_SCHEMA.to_owned(),
            schema_version: KERNEL32_COMPATIBILITY_POLICY_LEGACY_VERSION,
            id: "kernel32-controlled-v1".to_owned(),
            standard_handle_selectors: vec![RUNTIME_STANDARD_INPUT_SELECTOR],
            standard_handle_file_types: vec![Kernel32StandardHandleFileType {
                selector: RUNTIME_STANDARD_INPUT_SELECTOR,
                file_type: 2,
            }],
            set_handle_counts: Vec::new(),
            ansi_command_line: false,
            wide_environment_strings: false,
            wide_char_to_multi_byte: None,
            multi_byte_to_wide_char: None,
            set_current_directory_a: None,
            source_files: None,
            load_library_a: None,
            get_drive_type_a: None,
            create_mutex_a: None,
            create_waitable_timer_a: None,
            set_waitable_timer: None,
            wait_for_single_object_ex: None,
            sleep_zero: false,
            get_string_type_w: None,
            lc_map_string_w: None,
            get_module_file_name_a: None,
            active_code_page_info: None,
            system_default_lang_id: None,
            processor_features: Vec::new(),
            thread_affinity: None,
            thread_priority: None,
            create_thread: None,
            system_info: None,
            memory_status: None,
            virtual_alloc: None,
            os_version_info_a: None,
            tick_count: None,
            performance_counter: None,
            heap_allocator: None,
            heap_allocations: vec![Kernel32HeapAllocationPolicy {
                flags: HEAP_ZERO_MEMORY,
                byte_lengths: vec![0x214],
            }],
            heap_free_flags: Vec::new(),
            heap_size_flags: Vec::new(),
            initialize_critical_section: None,
        }
    }

    pub fn validate(&self) -> Result<(), Kernel32CompatibilityPolicyError> {
        if self.schema != KERNEL32_COMPATIBILITY_POLICY_SCHEMA
            || !matches!(
                self.schema_version,
                KERNEL32_COMPATIBILITY_POLICY_LEGACY_VERSION
                    | KERNEL32_COMPATIBILITY_POLICY_VERSION
            )
        {
            return Err(policy_error(
                "kernel32-policy-schema-unsupported",
                "Kernel32 compatibility policy schema or version is unsupported",
            ));
        }
        if self.id.is_empty()
            || self.id.len() > MAX_POLICY_ID_BYTES
            || !self.id.is_ascii()
            || self.id.as_bytes().contains(&0)
        {
            return Err(policy_error(
                "kernel32-policy-id-invalid",
                "Kernel32 compatibility policy ID must be bounded nonempty ASCII",
            ));
        }
        if self.standard_handle_selectors.is_empty()
            || self.standard_handle_selectors.len() > MAX_POLICY_ENTRIES
            || self.standard_handle_file_types.len() > MAX_POLICY_ENTRIES
            || self.set_handle_counts.len() > MAX_POLICY_ENTRIES
            || self.processor_features.len() > MAX_POLICY_ENTRIES
            || self.heap_allocations.len() > MAX_POLICY_ENTRIES
            || self.heap_free_flags.len() > MAX_POLICY_ENTRIES
            || self.heap_size_flags.len() > MAX_POLICY_ENTRIES
        {
            return Err(policy_error(
                "kernel32-policy-entry-count-invalid",
                "Kernel32 compatibility policy entry counts are outside their bounded limits",
            ));
        }
        if (self.schema_version == KERNEL32_COMPATIBILITY_POLICY_LEGACY_VERSION
            && (self.heap_allocator.is_some() || self.heap_allocations.is_empty()))
            || (self.schema_version == KERNEL32_COMPATIBILITY_POLICY_VERSION
                && (self.heap_allocator.is_none() || !self.heap_allocations.is_empty()))
        {
            return Err(policy_error(
                "kernel32-policy-heap-schema-invalid",
                "version 1 policies require exact legacy heap variants; version 2 policies require one bounded heap allocator",
            ));
        }

        let supported_selectors = BTreeSet::from([
            RUNTIME_STANDARD_INPUT_SELECTOR,
            RUNTIME_STANDARD_OUTPUT_SELECTOR,
            RUNTIME_STANDARD_ERROR_SELECTOR,
        ]);
        let selectors = self
            .standard_handle_selectors
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if selectors.len() != self.standard_handle_selectors.len()
            || !selectors.is_subset(&supported_selectors)
        {
            return Err(policy_error(
                "kernel32-policy-standard-handles-invalid",
                "standard-handle selectors must be unique implemented selector values",
            ));
        }

        let mut file_type_selectors = BTreeSet::new();
        for entry in &self.standard_handle_file_types {
            if !selectors.contains(&entry.selector)
                || !file_type_selectors.insert(entry.selector)
                || !(1..=3).contains(&entry.file_type)
            {
                return Err(policy_error(
                    "kernel32-policy-file-types-invalid",
                    "standard-handle file types must be unique enabled selectors with a supported type",
                ));
            }
        }

        let handle_counts = self
            .set_handle_counts
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if handle_counts.len() != self.set_handle_counts.len() || handle_counts.contains(&0) {
            return Err(policy_error(
                "kernel32-policy-set-handle-counts-invalid",
                "SetHandleCount variants must be unique nonzero counts",
            ));
        }
        if self.wide_environment_strings && !self.ansi_command_line {
            return Err(policy_error(
                "kernel32-policy-environment-layer-invalid",
                "wide environment strings require the preceding ANSI command-line capability",
            ));
        }
        if let Some(conversion) = self.wide_char_to_multi_byte
            && (!self.wide_environment_strings
                || conversion.active_code_page != 932
                || conversion.maximum_source_code_units == 0
                || conversion.maximum_source_code_units > 1_048_576)
        {
            return Err(policy_error(
                "kernel32-policy-wide-char-conversion-invalid",
                "wide-character conversion requires the wide environment layer, Windows-932 ACP, and a bounded nonzero source limit",
            ));
        }
        if let Some(info) = self.active_code_page_info
            && (self.active_code_page().is_none()
                || !(1..=4).contains(&info.max_char_size)
                || !valid_lead_byte_ranges(&info.lead_byte))
        {
            return Err(policy_error(
                "kernel32-policy-code-page-info-invalid",
                "active code-page information requires an enabled ACP, a bounded character size, and ordered trailing-zero lead-byte ranges",
            ));
        }
        if let Some(conversion) = self.multi_byte_to_wide_char.as_ref()
            && !valid_multi_byte_to_wide_char_policy(conversion, self.active_code_page())
        {
            return Err(policy_error(
                "kernel32-policy-multi-byte-conversion-invalid",
                "multibyte conversion requires Windows-932 ACP, bounded unique implemented flags, and a bounded nonzero source limit",
            ));
        }
        if let Some(directory) = self.set_current_directory_a
            && (!self
                .multi_byte_to_wide_char
                .as_ref()
                .is_some_and(|conversion| conversion.allow_active_code_page_selector)
                || !directory.process_image_directory_only
                || !(4..=32_768).contains(&directory.maximum_path_bytes))
        {
            return Err(policy_error(
                "kernel32-policy-set-current-directory-invalid",
                "SetCurrentDirectoryA requires the CP_ACP conversion layer, process-image-directory selection, and a bounded DOS path limit",
            ));
        }
        if let Some(files) = self.source_files.as_ref() {
            let desired_access = files
                .allowed_desired_access
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            let share_modes = files
                .allowed_share_modes
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            let dispositions = files
                .allowed_creation_dispositions
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            let flags = files
                .allowed_flags_and_attributes
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            let writable = desired_access.contains(&0x4000_0000);
            if self.schema_version != KERNEL32_COMPATIBILITY_POLICY_VERSION
                || self.active_code_page() != Some(932)
                || files.allowed_desired_access.is_empty()
                || files.allowed_desired_access.len() > MAX_POLICY_ENTRIES
                || desired_access.len() != files.allowed_desired_access.len()
                || !desired_access.contains(&0x8000_0000)
                || desired_access
                    .iter()
                    .any(|access| !matches!(*access, 0x8000_0000 | 0x4000_0000))
                || files.allowed_share_modes.is_empty()
                || share_modes.len() != files.allowed_share_modes.len()
                || !share_modes.contains(&1)
                || share_modes.iter().any(|share| !matches!(*share, 0 | 1))
                || files.allowed_creation_dispositions.is_empty()
                || dispositions.len() != files.allowed_creation_dispositions.len()
                || !dispositions.contains(&3)
                || dispositions
                    .iter()
                    .any(|disposition| !matches!(*disposition, 2..=4))
                || files.allowed_flags_and_attributes.is_empty()
                || flags.len() != files.allowed_flags_and_attributes.len()
                || flags.iter().any(|value| *value != 0x0800_0020)
                || !(4..=32_768).contains(&files.maximum_path_bytes)
                || !(1..=RUNTIME_SOURCE_READ_MAX_BYTES).contains(&files.maximum_read_bytes)
                || files.get_file_attributes_a.is_some_and(|attributes| {
                    attributes.regular_file_attributes != 0x20
                        || attributes.directory_attributes != 0x30
                })
                || (writable
                    && (!share_modes.contains(&0)
                        || !dispositions.contains(&4)
                        || !(1..=RUNTIME_SOURCE_READ_MAX_BYTES)
                            .contains(&files.maximum_write_bytes)))
                || (!writable && files.maximum_write_bytes != 0)
            {
                return Err(policy_error(
                    "kernel32-policy-source-files-invalid",
                    "file policy requires bounded reached source-read and optional writable-profile variants",
                ));
            }
        }
        if let Some(loader) = self.load_library_a.as_ref()
            && !valid_load_library_a_policy(loader, self.schema_version)
        {
            return Err(policy_error(
                "kernel32-policy-load-library-a-invalid",
                "LoadLibraryA policy requires schema v2 and unique bounded ASCII runtime-system module and export identities",
            ));
        }
        if let Some(drive) = self.get_drive_type_a
            && (!drive.process_image_drive_only
                || !(4..=32_768).contains(&drive.maximum_path_bytes))
        {
            return Err(policy_error(
                "kernel32-policy-get-drive-type-invalid",
                "GetDriveTypeA requires process-image-drive selection and a bounded DOS path limit",
            ));
        }
        if let Some(mutex) = self.create_mutex_a
            && (self.active_code_page() != Some(932)
                || (!mutex.allow_named && !mutex.allow_unnamed)
                || !(2..=32_768).contains(&mutex.maximum_name_bytes))
        {
            return Err(policy_error(
                "kernel32-policy-create-mutex-invalid",
                "CreateMutexA requires Windows-932 ACP, an enabled name mode, and a bounded name limit",
            ));
        }
        if let Some(timer) = self.create_waitable_timer_a
            && (self.schema_version != KERNEL32_COMPATIBILITY_POLICY_VERSION
                || !timer.allow_unnamed
                || (!timer.allow_manual_reset && !timer.allow_auto_reset))
        {
            return Err(policy_error(
                "kernel32-policy-create-waitable-timer-invalid",
                "CreateWaitableTimerA requires schema v2, unnamed-object support, and at least one reset mode",
            ));
        }
        if let Some(timer) = self.set_waitable_timer
            && (self.schema_version != KERNEL32_COMPATIBILITY_POLICY_VERSION
                || self.create_waitable_timer_a.is_none()
                || timer.minimum_relative_due_time_100ns == 0
                || timer.maximum_relative_due_time_100ns < timer.minimum_relative_due_time_100ns
                || timer.maximum_relative_due_time_100ns > 100_000_000
                || !timer.allow_one_shot
                || timer.allow_completion_routine
                || timer.allow_resume)
        {
            return Err(policy_error(
                "kernel32-policy-set-waitable-timer-invalid",
                "SetWaitableTimer requires the creation layer, a bounded relative interval, one-shot mode, and no completion routine or resume request",
            ));
        }
        if let Some(wait) = self.wait_for_single_object_ex
            && (self.set_waitable_timer.is_none()
                || wait.maximum_timeout_milliseconds > 60_000
                || (!wait.allow_alertable && !wait.allow_nonalertable)
                || !wait.waitable_timer_only)
        {
            return Err(policy_error(
                "kernel32-policy-wait-for-single-object-ex-invalid",
                "WaitForSingleObjectEx requires the timer arm layer, a bounded finite timeout, at least one alertable mode, and timer-only object scope",
            ));
        }
        if self.sleep_zero && self.schema_version != KERNEL32_COMPATIBILITY_POLICY_VERSION {
            return Err(policy_error(
                "kernel32-policy-sleep-zero-invalid",
                "Sleep(0) requires the version 2 compatibility-policy boundary",
            ));
        }
        if let Some(classification) = self.get_string_type_w.as_ref()
            && !valid_get_string_type_w_policy(
                classification,
                self.multi_byte_to_wide_char.is_some(),
            )
        {
            return Err(policy_error(
                "kernel32-policy-string-type-invalid",
                "string classification requires the preceding multibyte layer, bounded unique implemented information types, and a bounded nonzero source limit",
            ));
        }
        if let Some(mapping) = self.lc_map_string_w.as_ref()
            && !valid_lc_map_string_w_policy(mapping, self.get_string_type_w.is_some())
        {
            return Err(policy_error(
                "kernel32-policy-lc-map-string-invalid",
                "locale mapping requires the preceding classification layer, bounded unique implemented locales and flags, and a bounded nonzero source limit",
            ));
        }
        if let Some(file_name) = self.get_module_file_name_a
            && (self.lc_map_string_w.is_none()
                || !file_name.main_module_only
                || !(1..=32_768).contains(&file_name.maximum_capacity_bytes))
        {
            return Err(policy_error(
                "kernel32-policy-module-file-name-invalid",
                "module file-name lookup requires the preceding locale layer, main-module-only selection, and a bounded nonzero capacity",
            ));
        }
        if self
            .system_default_lang_id
            .is_some_and(|lang_id| lang_id != 0x0411 || self.lc_map_string_w.is_none())
        {
            return Err(policy_error(
                "kernel32-policy-system-default-lang-id-invalid",
                "system-default LANGID requires the bounded Japanese locale layer and LANGID 0x0411",
            ));
        }

        let processor_feature_indices = self
            .processor_features
            .iter()
            .map(|feature| feature.index)
            .collect::<BTreeSet<_>>();
        if processor_feature_indices.len() != self.processor_features.len() {
            return Err(policy_error(
                "kernel32-policy-processor-features-invalid",
                "processor-feature indices must be unique explicitly configured values",
            ));
        }
        if self
            .thread_affinity
            .is_some_and(|affinity| affinity.process_mask == 0)
        {
            return Err(policy_error(
                "kernel32-policy-thread-affinity-invalid",
                "thread-affinity policy requires a nonzero virtual process mask",
            ));
        }
        if let Some(priority) = self.thread_priority.as_ref() {
            let priorities = priority
                .allowed_priorities
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            if self.schema_version != KERNEL32_COMPATIBILITY_POLICY_VERSION
                || priorities.is_empty()
                || priorities.len() != priority.allowed_priorities.len()
                || priorities
                    .iter()
                    .any(|value| RuntimeThreadPriority::from_win32_value(*value).is_none())
            {
                return Err(policy_error(
                    "kernel32-policy-thread-priority-invalid",
                    "thread-priority policy requires schema v2 and unique implemented Win32 priority values",
                ));
            }
        }
        if let Some(thread) = self.create_thread.as_ref() {
            let flags = thread
                .allowed_creation_flags
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            if self.schema_version != KERNEL32_COMPATIBILITY_POLICY_VERSION
                || thread.default_stack_reserve_bytes == 0
                || thread.default_stack_commit_bytes == 0
                || thread.default_stack_commit_bytes > thread.default_stack_reserve_bytes
                || !thread.default_stack_reserve_bytes.is_multiple_of(4_096)
                || !thread.default_stack_commit_bytes.is_multiple_of(4_096)
                || thread.allowed_creation_flags.is_empty()
                || thread.allowed_creation_flags.len() > 2
                || flags.len() != thread.allowed_creation_flags.len()
                || !flags.is_subset(&BTreeSet::from([0, 0x0000_0004]))
                || !(1..=RUNTIME_GUEST_THREAD_LIMIT as u32).contains(&thread.maximum_threads)
            {
                return Err(policy_error(
                    "kernel32-policy-create-thread-invalid",
                    "CreateThread requires schema v2, bounded page-aligned default stack sizes, reached creation flags, and a bounded thread limit",
                ));
            }
        }
        if let Some(info) = self.system_info
            && (self.schema_version != KERNEL32_COMPATIBILITY_POLICY_VERSION
                || self.thread_affinity.is_none()
                || info.processor_architecture != 0
                || info.page_size != 4_096
                || info.minimum_application_address != 0x0001_0000
                || info.maximum_application_address != 0x7ffe_ffff
                || info.processor_type != 586
                || info.allocation_granularity != 65_536
                || info.processor_level == 0)
        {
            return Err(policy_error(
                "kernel32-policy-system-info-invalid",
                "x86 system information requires schema v2, thread affinity, and the bounded PE32 virtual address profile",
            ));
        }
        if let Some(status) = self.memory_status {
            let Some(info) = self.system_info else {
                return Err(policy_error(
                    "kernel32-policy-memory-status-invalid",
                    "x86 memory status requires the preceding system-information profile",
                ));
            };
            let total_virtual_bytes = info
                .maximum_application_address
                .checked_sub(info.minimum_application_address)
                .and_then(|span| span.checked_add(1));
            if self.schema_version != KERNEL32_COMPATIBILITY_POLICY_VERSION
                || status.memory_load_percent > 100
                || status.total_physical_bytes != 0x7fff_ffff
                || status.available_physical_bytes != status.total_physical_bytes
                || status.total_page_file_bytes != u32::MAX
                || status.available_page_file_bytes != status.total_page_file_bytes
                || status.available_virtual_bytes == 0
                || total_virtual_bytes.is_none_or(|total| status.available_virtual_bytes > total)
            {
                return Err(policy_error(
                    "kernel32-policy-memory-status-invalid",
                    "x86 memory status requires schema v2, the bounded virtual profile, native legacy capacity caps, a percentage load, and a nonzero bounded available-virtual value",
                ));
            }
        }
        if let Some(virtual_alloc) = self.virtual_alloc.as_ref() {
            let reserve_byte_lengths = virtual_alloc
                .allowed_reserve_byte_lengths
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            let allocation_types = virtual_alloc
                .allowed_allocation_types
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            let protections = virtual_alloc
                .allowed_protections
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            if self.schema_version != KERNEL32_COMPATIBILITY_POLICY_VERSION
                || self.system_info.is_none()
                || !virtual_alloc.require_null_preferred_address
                || reserve_byte_lengths.is_empty()
                || reserve_byte_lengths.len() != virtual_alloc.allowed_reserve_byte_lengths.len()
                || reserve_byte_lengths.len() > MAX_POLICY_ENTRIES
                || reserve_byte_lengths
                    .iter()
                    .any(|byte_length| *byte_length != 0x0600_0000)
                || allocation_types != BTreeSet::from([0x0010_2000])
                || allocation_types.len() != virtual_alloc.allowed_allocation_types.len()
                || protections != BTreeSet::from([0x0000_0004])
                || protections.len() != virtual_alloc.allowed_protections.len()
            {
                return Err(policy_error(
                    "kernel32-policy-virtual-alloc-invalid",
                    "VirtualAlloc requires schema v2, the x86 address profile, null top-down reserve-only placement, the reached 96 MiB size, and PAGE_READWRITE",
                ));
            }
        }
        if let Some(version) = self.os_version_info_a.as_ref()
            && (self.schema_version != KERNEL32_COMPATIBILITY_POLICY_VERSION
                || self.system_info.is_none()
                || version.major_version == 0
                || version.platform_id != 2
                || version.service_pack.len() > 127
                || !version.service_pack.is_ascii()
                || version.service_pack.as_bytes().contains(&0)
                || !(1..=3).contains(&version.product_type))
        {
            return Err(policy_error(
                "kernel32-policy-os-version-info-a-invalid",
                "ANSI OS version information requires schema v2, the x86 system-information layer, an NT platform profile, a bounded ASCII service-pack label, and a valid product type",
            ));
        }
        if self
            .tick_count
            .is_some_and(|clock| clock.increment_per_read == 0 || clock.increment_per_read > 1_000)
        {
            return Err(policy_error(
                "kernel32-policy-tick-count-invalid",
                "tick-count policy requires a deterministic increment from 1 through 1000 milliseconds per read",
            ));
        }
        if self.performance_counter.is_some_and(|clock| {
            clock.increment_per_read == 0 || clock.increment_per_read > i64::MAX as u64
        }) {
            return Err(policy_error(
                "kernel32-policy-performance-counter-invalid",
                "performance-counter policy requires a nonzero deterministic increment within the signed LARGE_INTEGER range",
            ));
        }

        if let Some(allocator) = self.heap_allocator.as_ref() {
            let allowed_flags = allocator
                .allowed_flags
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            let large_allocation_bytes = allocator
                .large_allocation_bytes
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            if allocator.allowed_flags.is_empty()
                || allocator.allowed_flags.len() > MAX_POLICY_ENTRIES
                || allowed_flags.len() != allocator.allowed_flags.len()
                || allowed_flags
                    .iter()
                    .any(|flags| !matches!(*flags, 0 | HEAP_ZERO_MEMORY))
                || !(1..=RUNTIME_PRIVATE_HEAP_ARENA_STRIDE)
                    .contains(&allocator.maximum_allocation_bytes)
                || allocator.large_allocation_bytes.len() > MAX_POLICY_ENTRIES
                || large_allocation_bytes.len() != allocator.large_allocation_bytes.len()
                || large_allocation_bytes
                    .iter()
                    .any(|byte_length| *byte_length != RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE)
            {
                return Err(policy_error(
                    "kernel32-policy-heap-allocator-invalid",
                    "heap allocator policy requires unique implemented flags, a bounded ordinary range, and only explicitly implemented large allocation sizes",
                ));
            }
        } else {
            let mut heap_variants = BTreeSet::new();
            for allocation in &self.heap_allocations {
                if !matches!(allocation.flags, 0 | HEAP_ZERO_MEMORY)
                    || allocation.byte_lengths.is_empty()
                    || allocation.byte_lengths.len() > MAX_POLICY_ENTRIES
                {
                    return Err(policy_error(
                        "kernel32-policy-heap-variant-invalid",
                        "heap allocation policy must use implemented flags and bounded sizes",
                    ));
                }
                for &byte_length in &allocation.byte_lengths {
                    if byte_length == 0
                        || byte_length > RUNTIME_PRIVATE_HEAP_ARENA_STRIDE
                        || !heap_variants.insert((allocation.flags, byte_length))
                    {
                        return Err(policy_error(
                            "kernel32-policy-heap-variant-invalid",
                            "heap allocation variants must be unique nonzero in-range flag/size pairs",
                        ));
                    }
                }
            }
        }
        let heap_free_flags = self
            .heap_free_flags
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if heap_free_flags.len() != self.heap_free_flags.len()
            || heap_free_flags.iter().any(|flags| *flags != 0)
        {
            return Err(policy_error(
                "kernel32-policy-heap-free-flags-invalid",
                "HeapFree variants must be unique implemented flag values",
            ));
        }
        let heap_size_flags = self
            .heap_size_flags
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if heap_size_flags.len() != self.heap_size_flags.len()
            || !heap_size_flags.is_subset(&BTreeSet::from([0, HEAP_NO_SERIALIZE]))
        {
            return Err(policy_error(
                "kernel32-policy-heap-size-flags-invalid",
                "HeapSize flags must be unique implemented zero or HEAP_NO_SERIALIZE values",
            ));
        }
        if let Some(initialization) = self.initialize_critical_section {
            let state_flags = initialization.spin_count_field & 0xff00_0000;
            if state_flags != 0 && state_flags != CRITICAL_SECTION_DYNAMIC_SPIN_STATE {
                return Err(policy_error(
                    "kernel32-policy-initialize-critical-section-invalid",
                    "InitializeCriticalSection spin-count field has unsupported state flags",
                ));
            }
        }
        Ok(())
    }

    pub fn allows_standard_handle(&self, selector: u32) -> bool {
        self.standard_handle_selectors.contains(&selector)
    }

    pub fn standard_handle_file_type(&self, selector: u32) -> Option<u32> {
        self.standard_handle_file_types
            .iter()
            .find(|entry| entry.selector == selector)
            .map(|entry| entry.file_type)
    }

    pub fn allows_set_handle_count(&self, count: u32) -> bool {
        self.set_handle_counts.contains(&count)
    }

    pub fn allows_ansi_command_line(&self) -> bool {
        self.ansi_command_line
    }

    pub fn allows_wide_environment_strings(&self) -> bool {
        self.wide_environment_strings
    }

    pub fn wide_char_to_multi_byte(&self) -> Option<&Kernel32WideCharToMultiBytePolicy> {
        self.wide_char_to_multi_byte.as_ref()
    }

    pub fn active_code_page(&self) -> Option<u32> {
        self.wide_char_to_multi_byte
            .map(|conversion| conversion.active_code_page)
    }

    pub fn active_code_page_info(&self) -> Option<(u32, &Kernel32CodePageInfoPolicy)> {
        self.active_code_page()
            .zip(self.active_code_page_info.as_ref())
    }

    pub fn system_default_lang_id(&self) -> Option<u16> {
        self.system_default_lang_id
    }

    pub fn multi_byte_to_wide_char(&self) -> Option<&Kernel32MultiByteToWideCharPolicy> {
        self.multi_byte_to_wide_char.as_ref()
    }

    pub fn set_current_directory_a(&self) -> Option<Kernel32SetCurrentDirectoryAPolicy> {
        self.set_current_directory_a
    }

    pub fn source_files(&self) -> Option<&Kernel32SourceFilePolicy> {
        self.source_files.as_ref()
    }

    pub fn get_file_attributes_a(&self) -> Option<Kernel32GetFileAttributesAPolicy> {
        self.source_files.as_ref()?.get_file_attributes_a
    }

    pub fn load_library_a(&self) -> Option<&Kernel32LoadLibraryAPolicy> {
        self.load_library_a.as_ref()
    }

    pub fn get_drive_type_a(&self) -> Option<Kernel32GetDriveTypeAPolicy> {
        self.get_drive_type_a
    }

    pub fn create_mutex_a(&self) -> Option<Kernel32CreateMutexAPolicy> {
        self.create_mutex_a
    }

    pub fn create_waitable_timer_a(&self) -> Option<Kernel32CreateWaitableTimerAPolicy> {
        self.create_waitable_timer_a
    }

    pub fn set_waitable_timer(&self) -> Option<Kernel32SetWaitableTimerPolicy> {
        self.set_waitable_timer
    }

    pub fn wait_for_single_object_ex(&self) -> Option<Kernel32WaitForSingleObjectExPolicy> {
        self.wait_for_single_object_ex
    }

    pub fn allows_sleep_zero(&self) -> bool {
        self.sleep_zero
    }

    pub fn get_string_type_w(&self) -> Option<&Kernel32GetStringTypeWPolicy> {
        self.get_string_type_w.as_ref()
    }

    pub fn lc_map_string_w(&self) -> Option<&Kernel32LcMapStringWPolicy> {
        self.lc_map_string_w.as_ref()
    }

    pub fn get_module_file_name_a(&self) -> Option<Kernel32GetModuleFileNameAPolicy> {
        self.get_module_file_name_a
    }

    pub fn processor_feature_presence(&self, index: u32) -> Option<bool> {
        self.processor_features
            .iter()
            .find(|feature| feature.index == index)
            .map(|feature| feature.present)
    }

    pub fn thread_affinity_process_mask(&self) -> Option<u32> {
        self.thread_affinity.map(|affinity| affinity.process_mask)
    }

    pub fn allows_thread_priority(&self, priority: i32) -> bool {
        self.thread_priority
            .as_ref()
            .is_some_and(|policy| policy.allowed_priorities.contains(&priority))
    }

    pub fn create_thread(&self) -> Option<&Kernel32CreateThreadPolicy> {
        self.create_thread.as_ref()
    }

    pub fn system_info(&self) -> Option<Kernel32SystemInfoPolicy> {
        self.system_info
    }

    pub fn memory_status(&self) -> Option<Kernel32MemoryStatusPolicy> {
        self.memory_status
    }

    pub fn virtual_alloc(&self) -> Option<&Kernel32VirtualAllocPolicy> {
        self.virtual_alloc.as_ref()
    }

    pub fn os_version_info_a(&self) -> Option<&Kernel32OsVersionInfoAPolicy> {
        self.os_version_info_a.as_ref()
    }

    pub fn tick_count_increment_per_read(&self) -> Option<u32> {
        self.tick_count.map(|clock| clock.increment_per_read)
    }

    pub fn performance_counter_increment_per_read(&self) -> Option<u64> {
        self.performance_counter
            .map(|clock| clock.increment_per_read)
    }

    pub fn allows_heap_allocation(&self, flags: u32, byte_length: u32) -> bool {
        self.heap_allocator.as_ref().map_or_else(
            || {
                self.heap_allocations.iter().any(|allocation| {
                    allocation.flags == flags && allocation.byte_lengths.contains(&byte_length)
                })
            },
            |allocator| {
                byte_length != 0
                    && (byte_length <= allocator.maximum_allocation_bytes
                        || allocator.large_allocation_bytes.contains(&byte_length))
                    && allocator.allowed_flags.contains(&flags)
            },
        )
    }

    pub fn heap_allocator(&self) -> Option<&Kernel32HeapAllocatorPolicy> {
        self.heap_allocator.as_ref()
    }

    pub fn allows_heap_free(&self, flags: u32) -> bool {
        self.heap_free_flags.contains(&flags)
    }

    pub fn allows_heap_size(&self, flags: u32) -> bool {
        self.heap_size_flags.contains(&flags)
    }

    pub fn initialize_critical_section_spin_count_field(&self) -> Option<u32> {
        self.initialize_critical_section
            .map(|initialization| initialization.spin_count_field)
    }
}

fn valid_load_library_a_policy(policy: &Kernel32LoadLibraryAPolicy, schema_version: u32) -> bool {
    if schema_version != KERNEL32_COMPATIBILITY_POLICY_VERSION
        || !(1..=260).contains(&policy.maximum_module_name_bytes)
        || policy.runtime_system_modules.is_empty()
        || policy.runtime_system_modules.len() > MAX_POLICY_ENTRIES
    {
        return false;
    }
    let mut canonical_names = BTreeSet::new();
    for module in &policy.runtime_system_modules {
        let canonical = module.canonical_name.to_ascii_lowercase();
        if module.canonical_name != canonical
            || module.canonical_name.is_empty()
            || module.canonical_name.len() > policy.maximum_module_name_bytes as usize
            || !module.canonical_name.is_ascii()
            || !module.canonical_name.ends_with(".dll")
            || module
                .canonical_name
                .bytes()
                .any(|byte| byte.is_ascii_control() || matches!(byte, b'/' | b'\\' | b':'))
            || !canonical_names.insert(canonical)
            || module.exports.is_empty()
            || module.exports.len() > MAX_POLICY_ENTRIES
        {
            return false;
        }
        let mut exports = BTreeSet::new();
        if module.exports.iter().any(|export| {
            export.is_empty()
                || export.len() > MAX_RUNTIME_SYSTEM_EXPORT_NAME_BYTES
                || !export.is_ascii()
                || export
                    .bytes()
                    .any(|byte| byte.is_ascii_control() || byte == b' ')
                || !exports.insert(export.as_str())
        }) {
            return false;
        }
    }
    true
}

fn valid_lead_byte_ranges(bytes: &[u8; 12]) -> bool {
    let mut terminated = false;
    for pair in bytes.chunks_exact(2) {
        match pair {
            [0, 0] => terminated = true,
            [first, last] if !terminated && *first != 0 && *first <= *last => {}
            _ => return false,
        }
    }
    true
}

fn valid_multi_byte_to_wide_char_policy(
    policy: &Kernel32MultiByteToWideCharPolicy,
    active_code_page: Option<u32>,
) -> bool {
    let flags = policy
        .allowed_flags
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    active_code_page == Some(932)
        && !policy.allowed_flags.is_empty()
        && policy.allowed_flags.len() <= MAX_POLICY_ENTRIES
        && flags.len() == policy.allowed_flags.len()
        && flags.iter().all(|flags| matches!(flags, 0 | 1))
        && (1..=1_048_576).contains(&policy.maximum_source_bytes)
}

fn valid_get_string_type_w_policy(
    policy: &Kernel32GetStringTypeWPolicy,
    multibyte_layer_enabled: bool,
) -> bool {
    let info_types = policy
        .allowed_info_types
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    multibyte_layer_enabled
        && !policy.allowed_info_types.is_empty()
        && policy.allowed_info_types.len() <= MAX_POLICY_ENTRIES
        && info_types.len() == policy.allowed_info_types.len()
        && info_types.iter().all(|info_type| *info_type == 1)
        && (1..=1_048_576).contains(&policy.maximum_source_code_units)
}

fn valid_lc_map_string_w_policy(
    policy: &Kernel32LcMapStringWPolicy,
    classification_layer_enabled: bool,
) -> bool {
    let locales = policy
        .allowed_locales
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let flags = policy
        .allowed_flags
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    classification_layer_enabled
        && !policy.allowed_locales.is_empty()
        && policy.allowed_locales.len() <= MAX_POLICY_ENTRIES
        && locales.len() == policy.allowed_locales.len()
        && locales.iter().all(|locale| *locale == 0x0411)
        && !policy.allowed_flags.is_empty()
        && policy.allowed_flags.len() <= MAX_POLICY_ENTRIES
        && flags.len() == policy.allowed_flags.len()
        && flags.iter().all(|flags| matches!(*flags, 0x0100 | 0x0200))
        && (1..=1_048_576).contains(&policy.maximum_source_code_units)
}

fn policy_error(
    code: &'static str,
    message: impl Into<String>,
) -> Kernel32CompatibilityPolicyError {
    Kernel32CompatibilityPolicyError {
        code,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_bounded_policy_without_target_evidence_identity() {
        let policy = Kernel32CompatibilityPolicy::controlled_v1();
        policy.validate().unwrap();
        assert!(policy.allows_standard_handle(RUNTIME_STANDARD_INPUT_SELECTOR));
        assert_eq!(
            policy.standard_handle_file_type(RUNTIME_STANDARD_INPUT_SELECTOR),
            Some(2)
        );
        assert!(policy.allows_heap_allocation(HEAP_ZERO_MEMORY, 0x214));
        assert!(!policy.allows_heap_free(0));
        assert!(!policy.allows_set_handle_count(32));
        assert!(!policy.allows_ansi_command_line());
        assert!(!policy.allows_wide_environment_strings());
        assert!(policy.wide_char_to_multi_byte().is_none());
        assert!(policy.multi_byte_to_wide_char().is_none());
        assert!(policy.set_current_directory_a().is_none());
        assert!(policy.get_drive_type_a().is_none());
        assert!(policy.get_string_type_w().is_none());
        assert!(policy.lc_map_string_w().is_none());
        assert!(policy.get_module_file_name_a().is_none());
        assert!(policy.active_code_page_info().is_none());
        assert_eq!(policy.system_default_lang_id(), None);
        assert_eq!(policy.processor_feature_presence(10), None);
        assert_eq!(policy.thread_affinity_process_mask(), None);
        assert_eq!(policy.system_info(), None);
        assert!(policy.os_version_info_a().is_none());
        assert_eq!(policy.tick_count_increment_per_read(), None);
        assert_eq!(policy.performance_counter_increment_per_read(), None);

        let mut affinity = policy.clone();
        affinity.thread_affinity = Some(Kernel32ThreadAffinityPolicy { process_mask: 1 });
        affinity.validate().unwrap();
        assert_eq!(affinity.thread_affinity_process_mask(), Some(1));
        affinity.schema_version = KERNEL32_COMPATIBILITY_POLICY_VERSION;
        affinity.heap_allocations.clear();
        affinity.heap_allocator = Some(Kernel32HeapAllocatorPolicy {
            allowed_flags: vec![0, HEAP_ZERO_MEMORY],
            maximum_allocation_bytes: RUNTIME_PRIVATE_HEAP_ARENA_STRIDE,
            large_allocation_bytes: Vec::new(),
        });
        affinity.system_info = Some(Kernel32SystemInfoPolicy {
            processor_architecture: 0,
            page_size: 4_096,
            minimum_application_address: 0x0001_0000,
            maximum_application_address: 0x7ffe_ffff,
            processor_type: 586,
            allocation_granularity: 65_536,
            processor_level: 6,
            processor_revision: 0,
        });
        affinity.validate().unwrap();
        assert_eq!(affinity.system_info().unwrap().processor_level, 6);
        affinity.system_info.as_mut().unwrap().page_size = 8_192;
        assert_eq!(
            affinity.validate().unwrap_err().code,
            "kernel32-policy-system-info-invalid"
        );
        affinity.system_info = None;
        affinity.thread_affinity = Some(Kernel32ThreadAffinityPolicy { process_mask: 0 });
        assert_eq!(
            affinity.validate().unwrap_err().code,
            "kernel32-policy-thread-affinity-invalid"
        );

        let mut tick_count = policy.clone();
        tick_count.tick_count = Some(Kernel32TickCountPolicy {
            increment_per_read: 1,
        });
        tick_count.validate().unwrap();
        assert_eq!(tick_count.tick_count_increment_per_read(), Some(1));
        tick_count.tick_count = Some(Kernel32TickCountPolicy {
            increment_per_read: 0,
        });
        assert_eq!(
            tick_count.validate().unwrap_err().code,
            "kernel32-policy-tick-count-invalid"
        );

        let mut performance_counter = policy.clone();
        performance_counter.performance_counter = Some(Kernel32PerformanceCounterPolicy {
            increment_per_read: 10_000,
        });
        performance_counter.validate().unwrap();
        assert_eq!(
            performance_counter.performance_counter_increment_per_read(),
            Some(10_000)
        );
        performance_counter.performance_counter = Some(Kernel32PerformanceCounterPolicy {
            increment_per_read: 0,
        });
        assert_eq!(
            performance_counter.validate().unwrap_err().code,
            "kernel32-policy-performance-counter-invalid"
        );
        performance_counter.performance_counter = Some(Kernel32PerformanceCounterPolicy {
            increment_per_read: i64::MAX as u64 + 1,
        });
        assert_eq!(
            performance_counter.validate().unwrap_err().code,
            "kernel32-policy-performance-counter-invalid"
        );

        let mut bounded_heap = policy.clone();
        bounded_heap.schema_version = KERNEL32_COMPATIBILITY_POLICY_VERSION;
        bounded_heap.heap_allocations.clear();
        bounded_heap.heap_allocator = Some(Kernel32HeapAllocatorPolicy {
            allowed_flags: vec![0, HEAP_ZERO_MEMORY],
            maximum_allocation_bytes: RUNTIME_PRIVATE_HEAP_ARENA_STRIDE,
            large_allocation_bytes: vec![RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE],
        });
        bounded_heap.validate().unwrap();
        assert!(bounded_heap.allows_heap_allocation(0, 37));
        assert!(
            bounded_heap
                .allows_heap_allocation(HEAP_ZERO_MEMORY, RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE)
        );
        assert!(!bounded_heap.allows_heap_allocation(0, 0));
        assert!(!bounded_heap.allows_heap_allocation(0, 32 * 1024 * 1024));
        assert!(!bounded_heap.allows_heap_allocation(0, RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE + 1));
        assert!(!bounded_heap.allows_heap_allocation(HEAP_NO_SERIALIZE, 37));

        let mut mixed_heap_schema = bounded_heap.clone();
        mixed_heap_schema.heap_allocations = policy.heap_allocations.clone();
        assert_eq!(
            mixed_heap_schema.validate().unwrap_err().code,
            "kernel32-policy-heap-schema-invalid"
        );
        let mut duplicate_heap_flags = bounded_heap.clone();
        duplicate_heap_flags
            .heap_allocator
            .as_mut()
            .unwrap()
            .allowed_flags = vec![0, 0];
        assert_eq!(
            duplicate_heap_flags.validate().unwrap_err().code,
            "kernel32-policy-heap-allocator-invalid"
        );
        let mut oversized_heap_allocation = bounded_heap.clone();
        oversized_heap_allocation
            .heap_allocator
            .as_mut()
            .unwrap()
            .maximum_allocation_bytes = RUNTIME_PRIVATE_HEAP_ARENA_STRIDE + 1;
        assert_eq!(
            oversized_heap_allocation.validate().unwrap_err().code,
            "kernel32-policy-heap-allocator-invalid"
        );
        let mut unsupported_large_allocation = bounded_heap.clone();
        unsupported_large_allocation
            .heap_allocator
            .as_mut()
            .unwrap()
            .large_allocation_bytes = vec![32 * 1024 * 1024];
        assert_eq!(
            unsupported_large_allocation.validate().unwrap_err().code,
            "kernel32-policy-heap-allocator-invalid"
        );

        let mut invalid_environment_layer = policy.clone();
        invalid_environment_layer.wide_environment_strings = true;
        assert_eq!(
            invalid_environment_layer.validate().unwrap_err().code,
            "kernel32-policy-environment-layer-invalid"
        );

        let mut invalid_conversion = policy.clone();
        invalid_conversion.wide_char_to_multi_byte = Some(Kernel32WideCharToMultiBytePolicy {
            active_code_page: 932,
            maximum_source_code_units: 65_536,
        });
        assert_eq!(
            invalid_conversion.validate().unwrap_err().code,
            "kernel32-policy-wide-char-conversion-invalid"
        );

        let mut missing_active_code_page = policy.clone();
        missing_active_code_page.active_code_page_info = Some(Kernel32CodePageInfoPolicy {
            max_char_size: 2,
            default_char: [b'?', 0],
            lead_byte: [0x81, 0x9f, 0xe0, 0xfc, 0, 0, 0, 0, 0, 0, 0, 0],
        });
        assert_eq!(
            missing_active_code_page.validate().unwrap_err().code,
            "kernel32-policy-code-page-info-invalid"
        );

        let mut invalid_lead_bytes = missing_active_code_page;
        invalid_lead_bytes.ansi_command_line = true;
        invalid_lead_bytes.wide_environment_strings = true;
        invalid_lead_bytes.wide_char_to_multi_byte = Some(Kernel32WideCharToMultiBytePolicy {
            active_code_page: 932,
            maximum_source_code_units: 65_536,
        });
        invalid_lead_bytes
            .active_code_page_info
            .as_mut()
            .unwrap()
            .lead_byte = [0x81, 0x9f, 0, 0, 0xe0, 0xfc, 0, 0, 0, 0, 0, 0];
        assert_eq!(
            invalid_lead_bytes.validate().unwrap_err().code,
            "kernel32-policy-code-page-info-invalid"
        );

        let mut invalid_multi_byte = policy.clone();
        invalid_multi_byte.multi_byte_to_wide_char = Some(Kernel32MultiByteToWideCharPolicy {
            allow_active_code_page_selector: false,
            allowed_flags: vec![1],
            maximum_source_bytes: 65_536,
        });
        assert_eq!(
            invalid_multi_byte.validate().unwrap_err().code,
            "kernel32-policy-multi-byte-conversion-invalid"
        );

        let mut invalid_current_directory = policy.clone();
        invalid_current_directory.set_current_directory_a =
            Some(Kernel32SetCurrentDirectoryAPolicy {
                process_image_directory_only: true,
                maximum_path_bytes: 32_768,
            });
        assert_eq!(
            invalid_current_directory.validate().unwrap_err().code,
            "kernel32-policy-set-current-directory-invalid"
        );

        invalid_multi_byte.ansi_command_line = true;
        invalid_multi_byte.wide_environment_strings = true;
        invalid_multi_byte.wide_char_to_multi_byte = Some(Kernel32WideCharToMultiBytePolicy {
            active_code_page: 932,
            maximum_source_code_units: 65_536,
        });
        invalid_multi_byte
            .multi_byte_to_wide_char
            .as_mut()
            .unwrap()
            .allowed_flags = vec![1, 1];
        assert_eq!(
            invalid_multi_byte.validate().unwrap_err().code,
            "kernel32-policy-multi-byte-conversion-invalid"
        );

        let mut invalid_string_type = policy.clone();
        invalid_string_type.get_string_type_w = Some(Kernel32GetStringTypeWPolicy {
            allowed_info_types: vec![1],
            maximum_source_code_units: 65_536,
        });
        assert_eq!(
            invalid_string_type.validate().unwrap_err().code,
            "kernel32-policy-string-type-invalid"
        );

        let mut invalid_lc_map = policy.clone();
        invalid_lc_map.ansi_command_line = true;
        invalid_lc_map.wide_environment_strings = true;
        invalid_lc_map.wide_char_to_multi_byte = Some(Kernel32WideCharToMultiBytePolicy {
            active_code_page: 932,
            maximum_source_code_units: 65_536,
        });
        invalid_lc_map.multi_byte_to_wide_char = Some(Kernel32MultiByteToWideCharPolicy {
            allow_active_code_page_selector: false,
            allowed_flags: vec![1],
            maximum_source_bytes: 65_536,
        });
        invalid_lc_map.get_string_type_w = Some(Kernel32GetStringTypeWPolicy {
            allowed_info_types: vec![1],
            maximum_source_code_units: 65_536,
        });
        invalid_lc_map.lc_map_string_w = Some(Kernel32LcMapStringWPolicy {
            allowed_locales: vec![0x0411],
            allowed_flags: vec![0],
            maximum_source_code_units: 65_536,
        });
        assert_eq!(
            invalid_lc_map.validate().unwrap_err().code,
            "kernel32-policy-lc-map-string-invalid"
        );

        invalid_lc_map
            .lc_map_string_w
            .as_mut()
            .unwrap()
            .allowed_flags = vec![0x0100, 0x0200];
        invalid_lc_map.get_module_file_name_a = Some(Kernel32GetModuleFileNameAPolicy {
            main_module_only: false,
            maximum_capacity_bytes: 32_768,
        });
        assert_eq!(
            invalid_lc_map.validate().unwrap_err().code,
            "kernel32-policy-module-file-name-invalid"
        );

        let mut duplicate_processor_feature = policy.clone();
        duplicate_processor_feature.processor_features = vec![
            Kernel32ProcessorFeaturePolicy {
                index: 10,
                present: true,
            },
            Kernel32ProcessorFeaturePolicy {
                index: 10,
                present: false,
            },
        ];
        assert_eq!(
            duplicate_processor_feature.validate().unwrap_err().code,
            "kernel32-policy-processor-features-invalid"
        );

        let mut duplicate = policy;
        duplicate.heap_allocations[0].byte_lengths.push(0x214);
        assert_eq!(
            duplicate.validate().unwrap_err().code,
            "kernel32-policy-heap-variant-invalid"
        );

        duplicate.heap_allocations[0].byte_lengths.pop();
        duplicate.heap_free_flags = vec![0, 0];
        assert_eq!(
            duplicate.validate().unwrap_err().code,
            "kernel32-policy-heap-free-flags-invalid"
        );
        duplicate.heap_free_flags = vec![1];
        assert_eq!(
            duplicate.validate().unwrap_err().code,
            "kernel32-policy-heap-free-flags-invalid"
        );
        duplicate.heap_free_flags.clear();
        duplicate.heap_size_flags = vec![0, 0];
        assert_eq!(
            duplicate.validate().unwrap_err().code,
            "kernel32-policy-heap-size-flags-invalid"
        );
        duplicate.heap_size_flags = vec![8];
        assert_eq!(
            duplicate.validate().unwrap_err().code,
            "kernel32-policy-heap-size-flags-invalid"
        );
        duplicate.heap_size_flags.clear();
        duplicate.initialize_critical_section = Some(Kernel32InitializeCriticalSectionPolicy {
            spin_count_field: 0x0100_07d0,
        });
        assert_eq!(
            duplicate.validate().unwrap_err().code,
            "kernel32-policy-initialize-critical-section-invalid"
        );
    }
}

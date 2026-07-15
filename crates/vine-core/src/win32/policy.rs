use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::runtime::{
    RUNTIME_PRIVATE_HEAP_ARENA_STRIDE, RUNTIME_STANDARD_ERROR_SELECTOR,
    RUNTIME_STANDARD_INPUT_SELECTOR, RUNTIME_STANDARD_OUTPUT_SELECTOR,
};

pub const KERNEL32_COMPATIBILITY_POLICY_SCHEMA: &str = "vine.kernel32-compatibility-policy";
pub const KERNEL32_COMPATIBILITY_POLICY_VERSION: u32 = 2;

const KERNEL32_COMPATIBILITY_POLICY_LEGACY_VERSION: u32 = 1;

const HEAP_ZERO_MEMORY: u32 = 0x0000_0008;
const HEAP_NO_SERIALIZE: u32 = 0x0000_0001;
const CRITICAL_SECTION_DYNAMIC_SPIN_STATE: u32 = 0x0200_0000;
const MAX_POLICY_ID_BYTES: usize = 128;
const MAX_POLICY_ENTRIES: usize = 32;

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create_mutex_a: Option<Kernel32CreateMutexAPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub get_string_type_w: Option<Kernel32GetStringTypeWPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lc_map_string_w: Option<Kernel32LcMapStringWPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub get_module_file_name_a: Option<Kernel32GetModuleFileNameAPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_code_page_info: Option<Kernel32CodePageInfoPolicy>,
    #[serde(default)]
    pub processor_features: Vec<Kernel32ProcessorFeaturePolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_affinity: Option<Kernel32ThreadAffinityPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tick_count: Option<Kernel32TickCountPolicy>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32CreateMutexAPolicy {
    pub allow_named: bool,
    pub allow_unnamed: bool,
    pub maximum_name_bytes: u32,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32TickCountPolicy {
    pub increment_per_read: u32,
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
            create_mutex_a: None,
            get_string_type_w: None,
            lc_map_string_w: None,
            get_module_file_name_a: None,
            active_code_page_info: None,
            processor_features: Vec::new(),
            thread_affinity: None,
            tick_count: None,
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
        if self
            .tick_count
            .is_some_and(|clock| clock.increment_per_read == 0 || clock.increment_per_read > 1_000)
        {
            return Err(policy_error(
                "kernel32-policy-tick-count-invalid",
                "tick-count policy requires a deterministic increment from 1 through 1000 milliseconds per read",
            ));
        }

        if let Some(allocator) = self.heap_allocator.as_ref() {
            let allowed_flags = allocator
                .allowed_flags
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
            {
                return Err(policy_error(
                    "kernel32-policy-heap-allocator-invalid",
                    "heap allocator policy requires unique implemented flags and a nonzero per-heap-bounded maximum allocation size",
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

    pub fn multi_byte_to_wide_char(&self) -> Option<&Kernel32MultiByteToWideCharPolicy> {
        self.multi_byte_to_wide_char.as_ref()
    }

    pub fn set_current_directory_a(&self) -> Option<Kernel32SetCurrentDirectoryAPolicy> {
        self.set_current_directory_a
    }

    pub fn create_mutex_a(&self) -> Option<Kernel32CreateMutexAPolicy> {
        self.create_mutex_a
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

    pub fn tick_count_increment_per_read(&self) -> Option<u32> {
        self.tick_count.map(|clock| clock.increment_per_read)
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
                    && byte_length <= allocator.maximum_allocation_bytes
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
        assert!(policy.get_string_type_w().is_none());
        assert!(policy.lc_map_string_w().is_none());
        assert!(policy.get_module_file_name_a().is_none());
        assert!(policy.active_code_page_info().is_none());
        assert_eq!(policy.processor_feature_presence(10), None);
        assert_eq!(policy.thread_affinity_process_mask(), None);
        assert_eq!(policy.tick_count_increment_per_read(), None);

        let mut affinity = policy.clone();
        affinity.thread_affinity = Some(Kernel32ThreadAffinityPolicy { process_mask: 1 });
        affinity.validate().unwrap();
        assert_eq!(affinity.thread_affinity_process_mask(), Some(1));
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

        let mut bounded_heap = policy.clone();
        bounded_heap.schema_version = KERNEL32_COMPATIBILITY_POLICY_VERSION;
        bounded_heap.heap_allocations.clear();
        bounded_heap.heap_allocator = Some(Kernel32HeapAllocatorPolicy {
            allowed_flags: vec![0, HEAP_ZERO_MEMORY],
            maximum_allocation_bytes: RUNTIME_PRIVATE_HEAP_ARENA_STRIDE,
        });
        bounded_heap.validate().unwrap();
        assert!(bounded_heap.allows_heap_allocation(0, 37));
        assert!(
            bounded_heap
                .allows_heap_allocation(HEAP_ZERO_MEMORY, RUNTIME_PRIVATE_HEAP_ARENA_STRIDE)
        );
        assert!(!bounded_heap.allows_heap_allocation(0, 0));
        assert!(!bounded_heap.allows_heap_allocation(0, RUNTIME_PRIVATE_HEAP_ARENA_STRIDE + 1));
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
        let mut oversized_heap_allocation = bounded_heap;
        oversized_heap_allocation
            .heap_allocator
            .as_mut()
            .unwrap()
            .maximum_allocation_bytes = RUNTIME_PRIVATE_HEAP_ARENA_STRIDE + 1;
        assert_eq!(
            oversized_heap_allocation.validate().unwrap_err().code,
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

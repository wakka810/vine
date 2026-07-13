use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::runtime::{
    RUNTIME_PRIVATE_HEAP_ARENA_STRIDE, RUNTIME_STANDARD_ERROR_SELECTOR,
    RUNTIME_STANDARD_INPUT_SELECTOR, RUNTIME_STANDARD_OUTPUT_SELECTOR,
};

pub const KERNEL32_COMPATIBILITY_POLICY_SCHEMA: &str = "vine.kernel32-compatibility-policy";
pub const KERNEL32_COMPATIBILITY_POLICY_VERSION: u32 = 1;

const HEAP_ZERO_MEMORY: u32 = 0x0000_0008;
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
    pub heap_allocations: Vec<Kernel32HeapAllocationPolicy>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kernel32WideCharToMultiBytePolicy {
    pub active_code_page: u32,
    pub maximum_source_code_units: u32,
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
            schema_version: KERNEL32_COMPATIBILITY_POLICY_VERSION,
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
            heap_allocations: vec![Kernel32HeapAllocationPolicy {
                flags: HEAP_ZERO_MEMORY,
                byte_lengths: vec![0x214],
            }],
        }
    }

    pub fn validate(&self) -> Result<(), Kernel32CompatibilityPolicyError> {
        if self.schema != KERNEL32_COMPATIBILITY_POLICY_SCHEMA
            || self.schema_version != KERNEL32_COMPATIBILITY_POLICY_VERSION
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
            || self.heap_allocations.is_empty()
            || self.heap_allocations.len() > MAX_POLICY_ENTRIES
        {
            return Err(policy_error(
                "kernel32-policy-entry-count-invalid",
                "Kernel32 compatibility policy entry counts are outside their bounded limits",
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

    pub fn allows_heap_allocation(&self, flags: u32, byte_length: u32) -> bool {
        self.heap_allocations.iter().any(|allocation| {
            allocation.flags == flags && allocation.byte_lengths.contains(&byte_length)
        })
    }
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
        assert!(!policy.allows_set_handle_count(32));
        assert!(!policy.allows_ansi_command_line());
        assert!(!policy.allows_wide_environment_strings());
        assert!(policy.wide_char_to_multi_byte().is_none());

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

        let mut duplicate = policy;
        duplicate.heap_allocations[0].byte_lengths.push(0x214);
        assert_eq!(
            duplicate.validate().unwrap_err().code,
            "kernel32-policy-heap-variant-invalid"
        );
    }
}

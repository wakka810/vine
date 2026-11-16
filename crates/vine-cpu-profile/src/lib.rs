//! Immutable guest CPU information, independent of a particular executor.
//!
//! The CPUID values preserve Vine's product CPU pin
//! `1d19442cea7ba0ca6354f1218f06c468a58c77e6` in its single-processor,
//! ACPI-disabled userspace configuration. They describe guest capabilities,
//! not the subset currently lowered by the independent backend. An executor
//! must report missing implementation explicitly, never remove feature bits
//! to steer guest code around it. No host CPU query or title input is used.
//!
//! Compatibility quirks are deliberate during the CPU migration: unknown
//! leaves return zero even above the maximum; extended maximum is 5 rather
//! than 0x80000005; and the hypervisor vendor leaf reports maximum 0. These are
//! preserved guest-visible pin results, not claims about Intel's normal
//! out-of-range policy (Intel SDM Vol. 2A, CPUID).
//!
//! The existing Windows HAL answers are retained separately below. For
//! example PF_XMMI_INSTRUCTIONS_AVAILABLE remains false although CPUID has
//! SSE, and SYSTEM_INFO revision remains zero rather than signature-derived
//! 0x0703. Consolidating ownership must not silently change either guest path.
//! Microsoft documents that IsProcessorFeaturePresent may also return false
//! when the HAL cannot detect a feature; it is not a CPUID bit projection.
//!
//! Specification references: [Intel SDM](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)
//! (Vol. 2A, CPUID), [Windows feature query](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-isprocessorfeaturepresent),
//! and [SYSTEM_INFO](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/ns-sysinfoapi-system_info).

#![no_std]

/// A selector without a subleaf ignores incoming ECX. Results use architectural
/// CPUID order EAX, EBX, ECX, EDX, not the x86 register encoding order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpuidLeaf {
    pub leaf: u32,
    pub subleaf: Option<u32>,
    pub result: [u32; 4],
}

/// TSC frequency from the product pin. Leaf 0x15 expresses Hz; leaf 0x16
/// expresses MHz. Runtime projects actual host elapsed time at this rate,
/// independently of instruction retirement and logical timer completion.
/// The fixed userspace CPU permits RDTSC (CR4.TSD=0); privileged CR4 writes
/// are outside that execution contract. This rate is not clock resolution.
pub const TSC_FREQUENCY_HZ: u32 = 1_000_000_000;

/// Writable MXCSR bits from product CPU pin
/// `1d19442cea7ba0ca6354f1218f06c468a58c77e6`, cpu.rs::MXCSR_MASK.
/// DAZ (bit 6) is supported; bits 16..31 remain reserved and cause #GP on load.
pub const MXCSR_MASK: u32 = 0xffff;

pub const CPUID_LEAVES: &[CpuidLeaf] = &[
    CpuidLeaf {
        leaf: 0,
        subleaf: None,
        result: [0x16, 0x756e6547, 0x6c65746e, 0x49656e69],
    },
    CpuidLeaf {
        leaf: 1,
        subleaf: None,
        result: [0x673, 0x00010800, 0xc0800001, 0x0780a979],
    },
    CpuidLeaf {
        leaf: 2,
        subleaf: None,
        result: [0x665b5001, 0, 0, 0x007a7000],
    },
    CpuidLeaf {
        leaf: 4,
        subleaf: Some(0),
        result: [0x121, 0x01c0003f, 0x3f, 1],
    },
    CpuidLeaf {
        leaf: 4,
        subleaf: Some(1),
        result: [0x122, 0x01c0003f, 0x3f, 1],
    },
    CpuidLeaf {
        leaf: 4,
        subleaf: Some(2),
        result: [0x143, 0x05c0003f, 0xfff, 1],
    },
    CpuidLeaf {
        leaf: 5,
        subleaf: None,
        result: [0x40, 0x40, 3, 0x00142120],
    },
    CpuidLeaf {
        leaf: 7,
        subleaf: Some(0),
        result: [0, 0x200, 0, 0],
    },
    CpuidLeaf {
        leaf: 0x15,
        subleaf: None,
        result: [1, 1, TSC_FREQUENCY_HZ, 0],
    },
    CpuidLeaf {
        leaf: 0x16,
        subleaf: None,
        result: [1000, 1000, 10, 0],
    },
    CpuidLeaf {
        leaf: 0x40000000,
        subleaf: None,
        result: [0, 0x61774d56, 0x4d566572, 0x65726177],
    },
    CpuidLeaf {
        leaf: 0x80000000,
        subleaf: None,
        result: [5, 0, 0, 0],
    },
];

pub fn cpuid(leaf: u32, subleaf: u32) -> [u32; 4] {
    CPUID_LEAVES
        .iter()
        .find(|row| row.leaf == leaf && row.subleaf.is_none_or(|value| value == subleaf))
        .map_or([0; 4], |row| row.result)
}

/// Existing Windows processor fields, retained without changing the HAL's
/// previously observable answers. Other SYSTEM_INFO memory fields stay owned
/// by the Runtime and do not belong to the CPU capability profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowsProcessorInfo {
    pub architecture: u16,
    pub affinity_mask: u32,
    pub logical_processors: u32,
    pub processor_type: u32,
    pub level: u16,
    pub revision: u16,
}

pub const WINDOWS_PROCESSOR: WindowsProcessorInfo = WindowsProcessorInfo {
    architecture: 0, // PROCESSOR_ARCHITECTURE_INTEL
    affinity_mask: 1,
    logical_processors: 1,
    processor_type: 586, // obsolete PROCESSOR_INTEL_PENTIUM compatibility field
    level: 6,
    revision: 0,
};

pub const fn windows_processor_feature_present(index: u32) -> bool {
    index == 10 // Existing PF_XMMI64_INSTRUCTIONS_AVAILABLE answer.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selectors_ignore_ecx_only_for_nonsubleaf_queries() {
        for (index, row) in CPUID_LEAVES.iter().enumerate() {
            assert_eq!(cpuid(row.leaf, row.subleaf.unwrap_or(u32::MAX)), row.result);
            assert!(
                !CPUID_LEAVES[..index]
                    .iter()
                    .any(|previous| previous.leaf == row.leaf
                        && (previous.subleaf.is_none()
                            || row.subleaf.is_none()
                            || previous.subleaf == row.subleaf))
            );
        }
        for (leaf, subleaf) in [
            (3, 0),
            (4, 3),
            (4, u32::MAX),
            (7, 1),
            (0xb, 0),
            (0x17, 0),
            (0x80000001, 0),
            (0xffffffff, 0xffffffff),
        ] {
            assert_eq!(cpuid(leaf, subleaf), [0; 4]);
        }
    }

    #[test]
    fn identity_features_and_legacy_hal_answers_are_stable() {
        assert_eq!(cpuid(0, 0), [0x16, 0x756e6547, 0x6c65746e, 0x49656e69]);
        assert_eq!(cpuid(1, 0), [0x673, 0x10800, 0xc0800001, 0x0780a979]);
        assert_eq!(cpuid(0x80000000, 0), [5, 0, 0, 0]);
        assert_eq!(
            cpuid(0x40000000, 0),
            [0, 0x61774d56, 0x4d566572, 0x65726177]
        );
        assert_eq!(
            (cpuid(1, 0)[1] >> 16) & 255,
            WINDOWS_PROCESSOR.logical_processors
        );
        assert_eq!(WINDOWS_PROCESSOR.affinity_mask, 1);
        assert_eq!(
            (
                WINDOWS_PROCESSOR.processor_type,
                WINDOWS_PROCESSOR.level,
                WINDOWS_PROCESSOR.revision
            ),
            (586, 6, 0)
        );
        for selector in [0, 3, 6, 7, 8, 13, u32::MAX] {
            assert!(!windows_processor_feature_present(selector));
        }
        assert!(windows_processor_feature_present(10));
    }

    #[test]
    fn displayed_tsc_and_frequency_ratios_agree() {
        let ratio = cpuid(0x15, 0);
        assert_eq!(
            u64::from(ratio[2]) * u64::from(ratio[1]) / u64::from(ratio[0]),
            u64::from(TSC_FREQUENCY_HZ)
        );
        assert_eq!(cpuid(0x16, 0)[0] * 1_000_000, TSC_FREQUENCY_HZ);
    }
}

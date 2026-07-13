use vine_core::{
    Kernel32CompatibilityPolicy, RUNTIME_STANDARD_ERROR_SELECTOR, RUNTIME_STANDARD_INPUT_SELECTOR,
    RUNTIME_STANDARD_OUTPUT_SELECTOR,
};

#[test]
fn versioned_policy_files_validate_without_evidence_manifest() {
    let policies = [
        include_str!("../../../fixtures/runtime-policies/kernel32-controlled-v1.json"),
        include_str!("../../../fixtures/runtime-policies/kernel32-heap-two-v1.json"),
        include_str!("../../../fixtures/runtime-policies/kernel32-standard-output-v1.json"),
        include_str!(
            "../../../fixtures/runtime-policies/kernel32-standard-output-file-type-v1.json"
        ),
        include_str!("../../../fixtures/runtime-policies/kernel32-standard-error-v1.json"),
    ];
    for source in policies {
        let policy: Kernel32CompatibilityPolicy = serde_json::from_str(source).unwrap();
        policy.validate().unwrap();
    }

    let active: Kernel32CompatibilityPolicy = serde_json::from_str(policies[4]).unwrap();
    assert!(active.allows_standard_handle(RUNTIME_STANDARD_INPUT_SELECTOR));
    assert!(active.allows_standard_handle(RUNTIME_STANDARD_OUTPUT_SELECTOR));
    assert!(active.allows_standard_handle(RUNTIME_STANDARD_ERROR_SELECTOR));
    assert_eq!(
        active.standard_handle_file_type(RUNTIME_STANDARD_INPUT_SELECTOR),
        Some(2)
    );
    assert_eq!(
        active.standard_handle_file_type(RUNTIME_STANDARD_OUTPUT_SELECTOR),
        Some(3)
    );
    assert_eq!(
        active.standard_handle_file_type(RUNTIME_STANDARD_ERROR_SELECTOR),
        None
    );
    assert!(active.allows_heap_allocation(0x8, 0x214));
    assert!(active.allows_heap_allocation(0x8, 0x800));
}

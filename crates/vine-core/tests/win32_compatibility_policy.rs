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
        include_str!(
            "../../../fixtures/runtime-policies/kernel32-standard-error-file-type-v1.json"
        ),
        include_str!("../../../fixtures/runtime-policies/kernel32-set-handle-count-v1.json"),
        include_str!("../../../fixtures/runtime-policies/kernel32-command-line-a-v1.json"),
        include_str!("../../../fixtures/runtime-policies/kernel32-environment-strings-w-v1.json"),
        include_str!("../../../fixtures/runtime-policies/kernel32-wide-char-to-multi-byte-v1.json"),
        include_str!("../../../fixtures/runtime-policies/kernel32-private-heap-size-1-v1.json"),
        include_str!("../../../fixtures/runtime-policies/kernel32-private-heap-size-544-v1.json"),
        include_str!("../../../fixtures/runtime-policies/kernel32-get-cp-info-v1.json"),
    ];
    for source in policies {
        let policy: Kernel32CompatibilityPolicy = serde_json::from_str(source).unwrap();
        policy.validate().unwrap();
    }

    let previous: Kernel32CompatibilityPolicy = serde_json::from_str(policies[10]).unwrap();
    assert!(!previous.allows_heap_allocation(0, 0x220));
    let heap_size_544: Kernel32CompatibilityPolicy = serde_json::from_str(policies[11]).unwrap();
    assert!(heap_size_544.allows_heap_allocation(0, 0x220));
    assert!(heap_size_544.active_code_page_info().is_none());
    let active: Kernel32CompatibilityPolicy = serde_json::from_str(policies[12]).unwrap();
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
        Some(3)
    );
    assert!(active.allows_heap_allocation(0x8, 0x214));
    assert!(active.allows_heap_allocation(0x8, 0x800));
    assert!(active.allows_heap_allocation(0, 1));
    assert!(active.allows_heap_allocation(0, 0x220));
    assert!(active.allows_set_handle_count(32));
    assert!(active.allows_ansi_command_line());
    assert!(active.allows_wide_environment_strings());
    assert_eq!(
        active
            .wide_char_to_multi_byte()
            .map(|policy| (policy.active_code_page, policy.maximum_source_code_units,)),
        Some((932, 65_536))
    );
    let (code_page, info) = active
        .active_code_page_info()
        .expect("active code-page information");
    assert_eq!(code_page, 932);
    assert_eq!(info.max_char_size, 2);
    assert_eq!(info.default_char, [b'?', 0]);
    assert_eq!(
        info.lead_byte,
        [0x81, 0x9f, 0xe0, 0xfc, 0, 0, 0, 0, 0, 0, 0, 0]
    );
}

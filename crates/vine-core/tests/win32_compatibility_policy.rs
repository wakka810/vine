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
        include_str!("../../../fixtures/runtime-policies/kernel32-multi-byte-to-wide-char-v1.json"),
        include_str!("../../../fixtures/runtime-policies/kernel32-get-string-type-w-v1.json"),
        include_str!("../../../fixtures/runtime-policies/kernel32-lc-map-string-w-v1.json"),
        include_str!("../../../fixtures/runtime-policies/kernel32-get-module-file-name-a-v1.json"),
        include_str!("../../../fixtures/runtime-policies/kernel32-private-heap-size-16-v1.json"),
        include_str!("../../../fixtures/runtime-policies/kernel32-private-heap-size-4-v1.json"),
        include_str!(
            "../../../fixtures/runtime-policies/kernel32-private-heap-free-size-1-v1.json"
        ),
        include_str!("../../../fixtures/runtime-policies/kernel32-private-heap-size-128-v1.json"),
        include_str!(
            "../../../fixtures/runtime-policies/kernel32-is-processor-feature-present-v1.json"
        ),
        include_str!("../../../fixtures/runtime-policies/kernel32-heap-size-v1.json"),
        include_str!(
            "../../../fixtures/runtime-policies/kernel32-initialize-critical-section-v1.json"
        ),
        include_str!(
            "../../../fixtures/runtime-policies/kernel32-multi-byte-to-wide-char-cp-acp-v1.json"
        ),
        include_str!("../../../fixtures/runtime-policies/kernel32-set-current-directory-a-v1.json"),
        include_str!("../../../fixtures/runtime-policies/kernel32-create-mutex-a-v1.json"),
        include_str!("../../../fixtures/runtime-policies/kernel32-bounded-heap-v2.json"),
        include_str!("../../../fixtures/runtime-policies/kernel32-get-system-info-v1.json"),
        include_str!(
            "../../../fixtures/runtime-policies/kernel32-get-system-default-lang-id-v1.json"
        ),
        include_str!("../../../fixtures/runtime-policies/kernel32-get-drive-type-a-v1.json"),
        include_str!("../../../fixtures/runtime-policies/kernel32-get-version-ex-a-v1.json"),
        include_str!("../../../fixtures/runtime-policies/kernel32-create-waitable-timer-a-v1.json"),
        include_str!("../../../fixtures/runtime-policies/kernel32-large-heap-allocation-v1.json"),
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
    let get_cp_info: Kernel32CompatibilityPolicy = serde_json::from_str(policies[12]).unwrap();
    assert!(get_cp_info.multi_byte_to_wide_char().is_none());
    let multibyte: Kernel32CompatibilityPolicy = serde_json::from_str(policies[13]).unwrap();
    assert!(multibyte.get_string_type_w().is_none());
    let string_type: Kernel32CompatibilityPolicy = serde_json::from_str(policies[14]).unwrap();
    assert!(string_type.lc_map_string_w().is_none());
    let previous: Kernel32CompatibilityPolicy = serde_json::from_str(policies[15]).unwrap();
    assert!(previous.get_module_file_name_a().is_none());
    let active: Kernel32CompatibilityPolicy = serde_json::from_str(policies[16]).unwrap();
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
    let multi_byte = active
        .multi_byte_to_wide_char()
        .expect("multibyte conversion policy");
    assert_eq!(multi_byte.allowed_flags, [1]);
    assert_eq!(multi_byte.maximum_source_bytes, 65_536);
    let string_type = active
        .get_string_type_w()
        .expect("string classification policy");
    assert_eq!(string_type.allowed_info_types, [1]);
    assert_eq!(string_type.maximum_source_code_units, 65_536);
    let mapping = active.lc_map_string_w().expect("locale mapping policy");
    assert_eq!(mapping.allowed_locales, [0x0411]);
    assert_eq!(mapping.allowed_flags, [0x0100, 0x0200]);
    assert_eq!(mapping.maximum_source_code_units, 65_536);
    let file_name = active
        .get_module_file_name_a()
        .expect("module file-name policy");
    assert!(file_name.main_module_only);
    assert_eq!(file_name.maximum_capacity_bytes, 32_768);
    let size_16: Kernel32CompatibilityPolicy = serde_json::from_str(policies[17]).unwrap();
    assert!(size_16.allows_heap_allocation(0, 1));
    assert!(size_16.allows_heap_allocation(0, 16));
    assert!(size_16.allows_heap_allocation(0, 544));
    assert!(!active.allows_heap_allocation(0, 16));
    let size_4: Kernel32CompatibilityPolicy = serde_json::from_str(policies[18]).unwrap();
    assert!(size_4.allows_heap_allocation(0x8, 4));
    assert!(size_4.allows_heap_allocation(0x8, 0x214));
    assert!(size_4.allows_heap_allocation(0x8, 0x800));
    assert!(!size_16.allows_heap_allocation(0x8, 4));
    assert!(!size_4.allows_heap_free(0));
    let heap_free: Kernel32CompatibilityPolicy = serde_json::from_str(policies[19]).unwrap();
    assert!(heap_free.allows_heap_free(0));
    assert!(!heap_free.allows_heap_free(1));
    assert!(heap_free.allows_heap_allocation(0x8, 4));
    assert!(!heap_free.allows_heap_allocation(0x8, 128));
    let heap_size_128: Kernel32CompatibilityPolicy = serde_json::from_str(policies[20]).unwrap();
    assert!(heap_size_128.allows_heap_free(0));
    assert!(heap_size_128.allows_heap_allocation(0x8, 128));
    assert_eq!(heap_size_128.processor_feature_presence(10), None);
    let processor_feature: Kernel32CompatibilityPolicy =
        serde_json::from_str(policies[21]).unwrap();
    assert_eq!(processor_feature.processor_feature_presence(10), Some(true));
    assert_eq!(processor_feature.processor_feature_presence(6), None);
    assert!(!processor_feature.allows_heap_size(0));
    let heap_size: Kernel32CompatibilityPolicy = serde_json::from_str(policies[22]).unwrap();
    assert!(heap_size.allows_heap_size(0));
    assert!(heap_size.allows_heap_size(1));
    assert!(!heap_size.allows_heap_size(8));
    assert_eq!(
        heap_size.initialize_critical_section_spin_count_field(),
        None
    );
    let initialize_critical_section: Kernel32CompatibilityPolicy =
        serde_json::from_str(policies[23]).unwrap();
    assert_eq!(
        initialize_critical_section.initialize_critical_section_spin_count_field(),
        Some(0x0200_07d0)
    );
    assert!(
        !initialize_critical_section
            .multi_byte_to_wide_char()
            .expect("explicit code-page conversion policy")
            .allow_active_code_page_selector
    );
    let active_code_page: Kernel32CompatibilityPolicy = serde_json::from_str(policies[24]).unwrap();
    let active_code_page_conversion = active_code_page
        .multi_byte_to_wide_char()
        .expect("active code-page conversion policy");
    assert!(active_code_page_conversion.allow_active_code_page_selector);
    assert_eq!(active_code_page_conversion.allowed_flags, [0, 1]);
    assert_eq!(active_code_page_conversion.maximum_source_bytes, 65_536);
    assert!(active_code_page.set_current_directory_a().is_none());
    let current_directory: Kernel32CompatibilityPolicy =
        serde_json::from_str(policies[25]).unwrap();
    let current_directory_policy = current_directory
        .set_current_directory_a()
        .expect("process current-directory policy");
    assert!(current_directory_policy.process_image_directory_only);
    assert_eq!(current_directory_policy.maximum_path_bytes, 32_768);
    assert!(current_directory.create_mutex_a().is_none());
    let mutex: Kernel32CompatibilityPolicy = serde_json::from_str(policies[26]).unwrap();
    let mutex_policy = mutex.create_mutex_a().expect("named mutex policy");
    assert!(mutex_policy.allow_named);
    assert!(!mutex_policy.allow_unnamed);
    assert_eq!(mutex_policy.maximum_name_bytes, 32_768);

    let bounded_heap: Kernel32CompatibilityPolicy = serde_json::from_str(policies[27]).unwrap();
    let heap_allocator = bounded_heap
        .heap_allocator()
        .expect("bounded heap allocator policy");
    assert_eq!(heap_allocator.allowed_flags, [0, 8]);
    assert_eq!(heap_allocator.maximum_allocation_bytes, 16 * 1024 * 1024);
    assert!(bounded_heap.allows_heap_allocation(0, 37));
    assert!(bounded_heap.allows_heap_allocation(8, 777));
    assert!(bounded_heap.allows_heap_allocation(8, 16 * 1024 * 1024));
    assert!(!bounded_heap.allows_heap_allocation(0, 0));
    assert!(!bounded_heap.allows_heap_allocation(0, 16 * 1024 * 1024 + 1));
    assert!(!bounded_heap.allows_heap_allocation(1, 37));
    assert!(bounded_heap.system_info().is_none());

    let system_info: Kernel32CompatibilityPolicy = serde_json::from_str(policies[28]).unwrap();
    let profile = system_info
        .system_info()
        .expect("x86 system-information policy");
    assert_eq!(system_info.thread_affinity_process_mask(), Some(1));
    assert_eq!(profile.processor_architecture, 0);
    assert_eq!(profile.page_size, 4_096);
    assert_eq!(profile.minimum_application_address, 0x0001_0000);
    assert_eq!(profile.maximum_application_address, 0x7ffe_ffff);
    assert_eq!(profile.processor_type, 586);
    assert_eq!(profile.allocation_granularity, 65_536);

    let large_heap: Kernel32CompatibilityPolicy = serde_json::from_str(policies[33]).unwrap();
    assert_eq!(
        large_heap
            .heap_allocator()
            .expect("large heap allocator policy")
            .maximum_allocation_bytes,
        16 * 1024 * 1024
    );
    assert_eq!(
        large_heap
            .heap_allocator()
            .expect("large heap allocator policy")
            .large_allocation_bytes,
        [64 * 1024 * 1024]
    );
    assert!(large_heap.allows_heap_allocation(0, 64 * 1024 * 1024));
    assert!(!large_heap.allows_heap_allocation(0, 32 * 1024 * 1024));
    assert!(!large_heap.allows_heap_allocation(0, 64 * 1024 * 1024 + 1));
    assert_eq!(profile.processor_level, 6);
    assert_eq!(profile.processor_revision, 0);
    assert_eq!(system_info.system_default_lang_id(), None);

    let system_default_lang_id: Kernel32CompatibilityPolicy =
        serde_json::from_str(policies[29]).unwrap();
    assert_eq!(
        system_default_lang_id.system_default_lang_id(),
        Some(0x0411)
    );
    assert!(system_default_lang_id.get_drive_type_a().is_none());

    let drive_type: Kernel32CompatibilityPolicy = serde_json::from_str(policies[30]).unwrap();
    let drive_profile = drive_type
        .get_drive_type_a()
        .expect("process-image drive profile");
    assert!(drive_profile.process_image_drive_only);
    assert_eq!(drive_profile.maximum_path_bytes, 32_768);
    assert!(drive_type.os_version_info_a().is_none());

    let os_version: Kernel32CompatibilityPolicy = serde_json::from_str(policies[31]).unwrap();
    let version_profile = os_version
        .os_version_info_a()
        .expect("ANSI OS-version profile");
    assert_eq!(version_profile.major_version, 10);
    assert_eq!(version_profile.minor_version, 0);
    assert_eq!(version_profile.build_number, 19_045);
    assert_eq!(version_profile.platform_id, 2);
    assert_eq!(version_profile.service_pack, "");
    assert_eq!(version_profile.service_pack_major, 0);
    assert_eq!(version_profile.service_pack_minor, 0);
    assert_eq!(version_profile.suite_mask, 0x0100);
    assert_eq!(version_profile.product_type, 1);

    let waitable_timer: Kernel32CompatibilityPolicy = serde_json::from_str(policies[32]).unwrap();
    let timer_profile = waitable_timer
        .create_waitable_timer_a()
        .expect("waitable timer profile");
    assert!(timer_profile.allow_manual_reset);
    assert!(!timer_profile.allow_auto_reset);
    assert!(timer_profile.allow_unnamed);

    let mut unsupported_timer_profile = waitable_timer;
    unsupported_timer_profile
        .create_waitable_timer_a
        .as_mut()
        .unwrap()
        .allow_unnamed = false;
    assert_eq!(
        unsupported_timer_profile.validate().unwrap_err().code,
        "kernel32-policy-create-waitable-timer-invalid"
    );

    let mut unsupported_os_version = os_version.clone();
    unsupported_os_version
        .os_version_info_a
        .as_mut()
        .unwrap()
        .platform_id = 1;
    assert_eq!(
        unsupported_os_version.validate().unwrap_err().code,
        "kernel32-policy-os-version-info-a-invalid"
    );

    let mut oversized_service_pack = os_version;
    oversized_service_pack
        .os_version_info_a
        .as_mut()
        .unwrap()
        .service_pack = "x".repeat(128);
    assert_eq!(
        oversized_service_pack.validate().unwrap_err().code,
        "kernel32-policy-os-version-info-a-invalid"
    );

    let mut unsupported_drive_profile = drive_type.clone();
    unsupported_drive_profile
        .get_drive_type_a
        .as_mut()
        .unwrap()
        .process_image_drive_only = false;
    assert_eq!(
        unsupported_drive_profile.validate().unwrap_err().code,
        "kernel32-policy-get-drive-type-invalid"
    );

    let mut unsupported_lang_id = system_default_lang_id.clone();
    unsupported_lang_id.system_default_lang_id = Some(0x0409);
    assert_eq!(
        unsupported_lang_id.validate().unwrap_err().code,
        "kernel32-policy-system-default-lang-id-invalid"
    );

    let mut missing_locale_layer = Kernel32CompatibilityPolicy::controlled_v1();
    missing_locale_layer.system_default_lang_id = Some(0x0411);
    assert_eq!(
        missing_locale_layer.validate().unwrap_err().code,
        "kernel32-policy-system-default-lang-id-invalid"
    );
}

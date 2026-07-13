use vine_core::{
    GuestMemory, GuestMemoryErrorClass, MemoryLimits,
    install_controlled_process_environment_with_command_line,
    install_controlled_process_environment_with_command_line_and_ansi,
};

#[test]
fn rejects_invalid_command_lines_before_mutation_and_allows_retry() {
    let mut memory = test_memory();
    let before = memory.stats();

    let embedded_nul = install_controlled_process_environment_with_command_line(
        &mut memory,
        "bootstrap\0params.exe",
    )
    .expect_err("embedded NUL");
    assert_eq!(embedded_nul.class, GuestMemoryErrorClass::InvalidRequest);
    assert_eq!(embedded_nul.code, "controlled-command-line-contains-nul");
    assert_eq!(memory.stats(), before);

    let multi_page = "x".repeat(3_000);
    let multi_page_error =
        install_controlled_process_environment_with_command_line(&mut memory, &multi_page)
            .expect_err("one-page command line contract");
    assert_eq!(
        multi_page_error.class,
        GuestMemoryErrorClass::InvalidRequest
    );
    assert_eq!(
        multi_page_error.code,
        "multi-page-command-line-unimplemented"
    );
    assert_eq!(memory.stats(), before);

    let too_long = "x".repeat(32_768);
    let length_error =
        install_controlled_process_environment_with_command_line(&mut memory, &too_long)
            .expect_err("UNICODE_STRING Length limit");
    assert_eq!(length_error.class, GuestMemoryErrorClass::InvalidRequest);
    assert_eq!(
        length_error.code,
        "controlled-command-line-length-not-representable"
    );
    assert_eq!(memory.stats(), before);

    let maximum_too_long = "x".repeat(32_767);
    let maximum_error =
        install_controlled_process_environment_with_command_line(&mut memory, &maximum_too_long)
            .expect_err("UNICODE_STRING MaximumLength limit");
    assert_eq!(maximum_error.class, GuestMemoryErrorClass::InvalidRequest);
    assert_eq!(
        maximum_error.code,
        "controlled-command-line-maximum-length-not-representable"
    );
    assert_eq!(memory.stats(), before);

    let non_ascii = install_controlled_process_environment_with_command_line_and_ansi(
        &mut memory,
        "非ascii.exe",
    )
    .expect_err("non-ASCII ANSI view");
    assert_eq!(non_ascii.class, GuestMemoryErrorClass::InvalidRequest);
    assert_eq!(non_ascii.code, "ansi-command-line-non-ascii-unsupported");
    assert_eq!(memory.stats(), before);

    let combined_view_too_long = "x".repeat(1_360);
    let combined_view_error = install_controlled_process_environment_with_command_line_and_ansi(
        &mut memory,
        &combined_view_too_long,
    )
    .expect_err("combined UTF-16 and ANSI page capacity");
    assert_eq!(
        combined_view_error.class,
        GuestMemoryErrorClass::InvalidRequest
    );
    assert_eq!(
        combined_view_error.code,
        "ansi-command-line-view-does-not-fit"
    );
    assert_eq!(memory.stats(), before);

    let environment =
        install_controlled_process_environment_with_command_line_and_ansi(&mut memory, "retry.exe")
            .expect("retry on the same GuestMemory");
    assert!(environment.owners_sealed);
    let parameters = environment.process_parameters.expect("process parameters");
    assert_eq!(parameters.command_line, "retry.exe");
    assert_eq!(
        parameters
            .ansi
            .expect("ANSI command-line view")
            .byte_length_with_nul,
        10
    );
}

#[test]
fn preflights_process_environment_capacity_without_partial_owners_or_pages() {
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 3,
        max_regions: 4,
        max_owners: 4,
        max_access_bytes: 0x1000,
    })
    .expect("memory");
    let before = memory.stats();
    let error =
        install_controlled_process_environment_with_command_line(&mut memory, "capacity.exe")
            .expect_err("four-page environment exceeds quota");
    assert_eq!(error.class, GuestMemoryErrorClass::ResourceLimit);
    assert_eq!(error.code, "physical-page-quota-exceeded");
    assert_eq!(memory.stats(), before);
}

fn test_memory() -> GuestMemory {
    GuestMemory::new(MemoryLimits {
        max_physical_pages: 4,
        max_regions: 4,
        max_owners: 4,
        max_access_bytes: 0x1000,
    })
    .expect("memory")
}

use crate::environment::{
    CONTROLLED_COMMAND_LINE_BASE, controlled_process_environment_allocation_request,
    install_process_environment,
};
use crate::memory::GuestMemoryErrorClass;
use crate::runtime::ProcessAddressSpaceAllocator;
use crate::{
    GUEST_PAGE_SIZE, GuestMemory, MemoryLimits, MemoryOwnerKind, MemoryRegionKind,
    ProcessAddressAllocationRecord,
};

#[test]
fn rejects_invalid_command_lines_before_mutation_and_allows_retry() {
    let mut memory = test_memory();
    let allocation = environment_allocation();
    let before = memory.stats();

    let embedded_nul =
        install_process_environment(&mut memory, &allocation, "bootstrap\0params.exe")
            .expect_err("embedded NUL");
    assert_eq!(embedded_nul.class, GuestMemoryErrorClass::InvalidRequest);
    assert_eq!(memory.stats(), before);

    let multi_page = "x".repeat(3_000);
    let multi_page_error = install_process_environment(&mut memory, &allocation, &multi_page)
        .expect_err("one-page command line contract");
    assert_eq!(
        multi_page_error.class,
        GuestMemoryErrorClass::InvalidRequest
    );
    assert_eq!(memory.stats(), before);

    let too_long = "x".repeat(32_768);
    let length_error = install_process_environment(&mut memory, &allocation, &too_long)
        .expect_err("UNICODE_STRING Length limit");
    assert_eq!(length_error.class, GuestMemoryErrorClass::InvalidRequest);
    assert_eq!(memory.stats(), before);

    let maximum_too_long = "x".repeat(32_767);
    let maximum_error = install_process_environment(&mut memory, &allocation, &maximum_too_long)
        .expect_err("UNICODE_STRING MaximumLength limit");
    assert_eq!(maximum_error.class, GuestMemoryErrorClass::InvalidRequest);
    assert_eq!(memory.stats(), before);

    let unrepresentable = install_process_environment(&mut memory, &allocation, "🦀.exe")
        .expect_err("Windows-932 cannot represent the command line");
    assert_eq!(unrepresentable.class, GuestMemoryErrorClass::InvalidRequest);
    assert_eq!(memory.stats(), before);

    let combined_view_too_long = "x".repeat(1_360);
    let combined_view_error =
        install_process_environment(&mut memory, &allocation, &combined_view_too_long)
            .expect_err("combined UTF-16 and ANSI page capacity");
    assert_eq!(
        combined_view_error.class,
        GuestMemoryErrorClass::InvalidRequest
    );
    assert_eq!(memory.stats(), before);

    let environment = install_process_environment(&mut memory, &allocation, "retry.exe")
        .expect("retry on the same GuestMemory");
    for region in memory
        .regions()
        .filter(|region| region.address_allocation_id == Some(allocation.id.0))
    {
        assert!(memory.owner_is_sealed(region.owner).unwrap());
    }
    assert_eq!(environment.address_allocation_id, allocation.id);
    assert_eq!(
        memory
            .regions()
            .filter(|region| region.address_allocation_id == Some(allocation.id.0))
            .count(),
        4
    );
    let parameters = environment.process_parameters;
    assert_eq!(parameters.command_line, "retry.exe");
    assert_eq!(parameters.ansi.byte_length_with_nul, 10);
}

#[test]
fn japanese_command_line_has_exact_windows_932_and_utf16_views() {
    let mut memory = test_memory();
    let allocation = environment_allocation();
    let environment = install_process_environment(&mut memory, &allocation, "日本語.exe")
        .expect("Japanese filename");
    let parameters = environment.process_parameters;
    let ansi = parameters.ansi;
    let generation = memory.pending_reflection().unwrap().invalidation.generation;
    memory.acknowledge_delta(generation).unwrap();
    assert_eq!(parameters.command_line, "日本語.exe");
    assert_eq!(
        memory
            .read(ansi.buffer_address, ansi.byte_length_with_nul as usize)
            .unwrap(),
        [
            0x93, 0xfa, 0x96, 0x7b, 0x8c, 0xea, b'.', b'e', b'x', b'e', 0
        ],
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
    let allocation = environment_allocation();
    let before = memory.stats();
    let error = install_process_environment(&mut memory, &allocation, "capacity.exe")
        .expect_err("four-page environment exceeds quota");
    assert_eq!(error.class, GuestMemoryErrorClass::ResourceLimit);
    assert_eq!(memory.stats(), before);
}

#[test]
fn existing_command_line_mapping_is_not_modified_by_failed_bootstrap() {
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 5,
        max_regions: 5,
        max_owners: 5,
        max_access_bytes: 0x1000,
    })
    .unwrap();
    let allocation = environment_allocation();
    let owner = memory.create_owner(MemoryOwnerKind::Anonymous).unwrap();
    memory
        .reserve_fixed(
            owner,
            MemoryRegionKind::Anonymous,
            CONTROLLED_COMMAND_LINE_BASE,
            GUEST_PAGE_SIZE,
        )
        .unwrap();
    let before = memory.stats();
    let regions = memory.regions().collect::<Vec<_>>();
    let error = install_process_environment(&mut memory, &allocation, "game.exe")
        .expect_err("occupied command-line page must be rejected before mutation");
    assert_eq!(error.class, GuestMemoryErrorClass::Conflict);
    assert_eq!(memory.stats(), before);
    assert_eq!(memory.regions().collect::<Vec<_>>(), regions);
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

fn environment_allocation() -> ProcessAddressAllocationRecord {
    let mut allocator = ProcessAddressSpaceAllocator::new().expect("address allocator");
    allocator
        .reserve(controlled_process_environment_allocation_request())
        .expect("environment allocation")
}

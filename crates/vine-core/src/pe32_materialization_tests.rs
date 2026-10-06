use crate as vine_core;
use std::fs;
use std::path::PathBuf;

use vine_core::{
    GuestAddress, GuestMemory, GuestMemoryErrorClass, MemoryLimits, ModuleSnapshot, Pe32Limits,
    build_pe32_load_plan, materialize_pe32_image,
};

const PREFERRED_BASE: u32 = 0x0040_0000;
const LOAD_BASE: u32 = 0x1000_0000;

#[test]
fn materializes_relocated_abi_smoke_through_the_first_unbound_iat() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".work/toolchain-spike/abi-smoke.exe");
    let source = fs::read(&fixture).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}); run `npm run check:toolchain` first",
            fixture.display()
        )
    });
    let source_before = source.clone();
    let pe_limits = Pe32Limits::default();
    let snapshot = ModuleSnapshot::from_bytes(source, &pe_limits).expect("fixture snapshot");
    let loader = build_pe32_load_plan(&snapshot, pe_limits).expect("fixture load plan");

    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 5,
        max_regions: 1,
        max_owners: 1,
        max_access_bytes: 4_096,
    })
    .expect("guest memory");
    let image = materialize_pe32_image(&mut memory, &snapshot, &loader, GuestAddress(LOAD_BASE))
        .expect("relocated fixture materialization");

    let mut fixed_loader = loader.clone();
    fixed_loader.relocations.clear();
    let before_rejection = memory.stats();
    materialize_pe32_image(
        &mut memory,
        &snapshot,
        &fixed_loader,
        GuestAddress(LOAD_BASE + 0x0100_0000),
    )
    .expect_err("an image without relocations cannot load away from its preferred base");
    assert_eq!(memory.stats(), before_rejection);

    assert_eq!(image.source, snapshot.identity());
    assert_eq!(image.layout.image_range.start(), GuestAddress(LOAD_BASE));
    assert_eq!(image.entry_point_address, GuestAddress(0x1000_1000));
    assert_eq!(image.layout.image_range.start(), GuestAddress(LOAD_BASE));
    assert_eq!(image.layout.image_range.byte_length(), 0x5000);
    assert_eq!(memory.stats().physical_pages, 5);
    assert_eq!(memory.stats().sealed_owners, 0);
    let owner = image.layout.owner;
    let header = &loader.header_mapping;
    assert_eq!(
        memory
            .read_initialization(owner, GuestAddress(LOAD_BASE), header.copy_size as usize)
            .expect("materialized headers"),
        snapshot.bytes()[..header.copy_size as usize]
    );
    for section in &loader.sections {
        if section.copy_size != 0 {
            let destination = GuestAddress(LOAD_BASE + section.destination_rva.0);
            let source_start = section.source_offset as usize;
            let source_end = source_start + section.copy_size as usize;
            let mut expected = snapshot.bytes()[source_start..source_end].to_vec();
            for target in &loader.relocations {
                let target = target.0;
                let section_end = section.destination_rva.0 + section.copy_size;
                if target >= section.destination_rva.0 && target + 4 <= section_end {
                    let offset = (target - section.destination_rva.0) as usize;
                    let original = u32::from_le_bytes(
                        expected[offset..offset + 4]
                            .try_into()
                            .expect("four-byte source relocation target"),
                    );
                    expected[offset..offset + 4].copy_from_slice(
                        &original
                            .wrapping_add(LOAD_BASE.wrapping_sub(PREFERRED_BASE))
                            .to_le_bytes(),
                    );
                }
            }
            assert_eq!(
                memory
                    .read_initialization(owner, destination, section.copy_size as usize)
                    .expect("materialized section source"),
                expected,
                "section {} source copy",
                section.name
            );
        }
        let zero_fill_size = section.mapped_size - section.copy_size;
        if zero_fill_size != 0 {
            let destination =
                GuestAddress(LOAD_BASE + section.destination_rva.0 + section.copy_size);
            let zeroes = memory
                .read_initialization(owner, destination, zero_fill_size as usize)
                .expect("materialized section zero-fill");
            assert!(
                zeroes.iter().all(|byte| *byte == 0),
                "section {} zero-fill",
                section.name
            );
        }
    }
    let header_tail = memory
        .read_initialization(
            owner,
            GuestAddress(LOAD_BASE + header.copy_size),
            (header.mapped_size - header.copy_size) as usize,
        )
        .expect("header zero-fill");
    assert!(header_tail.iter().all(|byte| *byte == 0));

    let expected_relocations = [
        (0x1006, 0x1000_2034),
        (0x100f, 0x1000_1030),
        (0x1015, 0x1000_2038),
        (0x101d, 0x1000_3000),
        (0x1023, 0x1000_3000),
    ];
    for (rva, expected) in expected_relocations {
        assert_eq!(
            read_u32(&memory, owner, GuestAddress(LOAD_BASE + rva)),
            expected,
            "HIGHLOW target RVA {rva:#x}"
        );
    }
    assert_eq!(
        read_u32(&memory, owner, GuestAddress(LOAD_BASE + 0x2034)),
        0x2040
    );
    assert_eq!(
        read_u32(&memory, owner, GuestAddress(LOAD_BASE + 0x2038)),
        0x204e
    );
    assert_eq!(
        read_u32(&memory, owner, GuestAddress(LOAD_BASE + 0x3000)),
        0
    );

    let blocked = memory
        .read(GuestAddress(LOAD_BASE), 1)
        .expect_err("mapping invalidation remains pending");
    assert_eq!(
        blocked.class,
        GuestMemoryErrorClass::SynchronizationRequired
    );
    let stats = memory.stats();
    assert_eq!(stats.physical_pages, 5);
    assert_eq!(stats.dirty_pages, 5);
    assert_eq!(stats.generation, 2);
    assert_eq!(stats.pending_invalidation, Some(image.layout.invalidation));
    assert_eq!(snapshot.bytes(), source_before);
}

fn read_u32(memory: &GuestMemory, owner: vine_core::MemoryOwnerId, address: GuestAddress) -> u32 {
    let bytes = memory
        .read_initialization(owner, address, 4)
        .expect("materialized DWORD");
    u32::from_le_bytes(bytes.try_into().expect("four-byte materialized DWORD"))
}

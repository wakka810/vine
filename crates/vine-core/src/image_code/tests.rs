use super::*;
use crate::memory::{GuestRange, PageProtection};
use crate::{
    MemoryLimits, ModuleSnapshot, Pe32Limits, build_pe32_load_plan, materialize_pe32_image,
};

fn image(memory: &mut GuestMemory, file: &str, base: u32) -> (Pe32LoadPlan, ImageMaterialization) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.work/toolchain-spike")
        .join(file);
    let limits = Pe32Limits::default();
    let source = ModuleSnapshot::from_bytes(std::fs::read(path).unwrap(), &limits).unwrap();
    let plan = build_pe32_load_plan(&source, limits).unwrap();
    let image = materialize_pe32_image(memory, &source, &plan, GuestAddress(base)).unwrap();
    (plan, image)
}

#[test]
fn relocated_exports_and_address_taken_code_supply_roots_without_mutation_or_ack() {
    let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
    let (main, exe) = image(&mut memory, "abi-smoke.exe", 0x1200_0000);
    let (dll, library) = image(&mut memory, "vine_fixture.dll", 0x1000_0000);
    assert!(code_preparation_entries(&memory, &[(&main, &exe)], 4096).is_err());
    memory.seal_owner(exe.layout.owner).unwrap();
    memory.seal_owner(library.layout.owner).unwrap();
    let before = memory.stats();
    let reflection = memory.pending_reflection().unwrap();
    let roots =
        code_preparation_entries(&memory, &[(&main, &exe), (&dll, &library)], 4096).unwrap();
    assert!(!roots.limited);
    assert_eq!(roots.entries[0], exe.entry_point_address.0);
    assert!(
        roots.entries.contains(&0x1200_1030),
        "relocated callback operand"
    );
    assert!(
        roots.entries.contains(&0x1000_1000),
        "exported DLL function"
    );
    assert!(
        roots.entries.contains(&0x1000_1010),
        "another export, absent from entry graph"
    );
    assert!(
        !roots.entries.contains(&0x1200_2034),
        "IAT data is not code"
    );
    assert_eq!(
        roots.entries.len(),
        roots.entries.iter().collect::<BTreeSet<_>>().len()
    );
    assert_eq!(memory.stats(), before);
    assert_eq!(reflection, memory.pending_reflection().unwrap());
    assert!(
        memory.read(exe.entry_point_address, 1).is_err(),
        "discovery does not acknowledge CPU reflection"
    );
    let limited = code_preparation_entries(&memory, &[(&main, &exe), (&dll, &library)], 1).unwrap();
    assert_eq!(limited.entries, [exe.entry_point_address.0]);
    assert!(limited.limited);
    assert!(code_preparation_entries(&memory, &[(&main, &exe)], 0).is_err());
    assert!(code_preparation_entries(&memory, &[(&main, &exe)], 4097).is_err());
}

#[test]
fn hints_require_the_live_image_identity_owner_and_executable_mapping() {
    let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
    let (mut plan, image) = image(&mut memory, "abi-smoke.exe", 0x1200_0000);
    memory.seal_owner(image.layout.owner).unwrap();
    plan.image.size_of_image += 4096;
    assert!(code_preparation_entries(&memory, &[(&plan, &image)], 4096).is_err());
    plan.image.size_of_image -= 4096;
    plan.relocations.push(crate::pe32::Rva(0xdead_0000));
    assert!(code_preparation_entries(&memory, &[(&plan, &image)], 4096).is_err());
    plan.relocations.pop();
    memory
        .protect(
            image.layout.owner,
            GuestRange::new(GuestAddress(0x1200_1000), 4096).unwrap(),
            PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .unwrap();
    assert!(
        code_preparation_entries(&memory, &[(&plan, &image)], 4096)
            .unwrap()
            .entries
            .is_empty()
    );
}

#[test]
fn optional_relocation_values_cannot_admit_another_owner_or_non_code_data() {
    let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
    let (plan, exe) = image(&mut memory, "abi-smoke.exe", 0x1200_0000);
    let (_dll, library) = image(&mut memory, "vine_fixture.dll", 0x1000_0000);
    memory
        .write_initialization(
            exe.layout.owner,
            GuestAddress(0x1200_100f),
            &0x1000_1010_u32.to_le_bytes(),
        )
        .unwrap();
    memory.seal_owner(exe.layout.owner).unwrap();
    memory.seal_owner(library.layout.owner).unwrap();
    let roots = code_preparation_entries(&memory, &[(&plan, &exe)], 4096).unwrap();
    assert_eq!(roots.entries, [exe.entry_point_address.0]);
}

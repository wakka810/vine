use super::*;
use crate::memory::{EXECUTE, MirrorMemory, READ, SEALED, WRITE};

fn source(bound: u8) -> MirrorMemory {
    let mut memory = MirrorMemory::new(8).unwrap();
    let mut code = [0xcc; 4096];
    code[..12].copy_from_slice(&[
        0x83, 0xf8, bound, 0x77, 0x2b, 0xff, 0x24, 0x85, 0, 0x20, 0, 0,
    ]);
    code[0x40] = 0xc3;
    code[0x50] = 0xc3;
    let mut data = [0; 4096];
    data[..4].copy_from_slice(&0x1040u32.to_le_bytes());
    data[4..8].copy_from_slice(&0x1050u32.to_le_bytes());
    memory
        .map_page(0x1000, 1, READ | EXECUTE | SEALED, &code)
        .unwrap();
    memory.map_page(0x2000, 2, READ | WRITE, &data).unwrap();
    memory
}

#[test]
fn reads_binary_edges_and_admits_other_roots_without_a_range_assumption() {
    let memory = source(1);
    let graph = discover(&memory, &[0x1000, 0x1005], 64);
    assert_eq!(
        graph.dispatches[&0x1005].targets,
        [Some(0x1040), Some(0x1050)]
    );
    assert_eq!(graph.region.blocks[&0x1005].successors, [0x1040, 0x1050]);
    assert!(graph.region.blocks.contains_key(&0x1040));
    assert!(graph.region.blocks.contains_key(&0x1050));
    assert!(
        !decode::discover(&memory, 0x1000, 64)
            .blocks
            .contains_key(&0x1040)
    );
}

#[test]
fn failed_data_reads_and_excessive_or_wrapping_bounds_are_not_edges() {
    let mut memory = source(1);
    memory.protect_page(0x2000, 2, WRITE).unwrap();
    assert!(discover(&memory, &[0x1000], 64).dispatches.is_empty());
    // Sign-extended imm8 is 0xffffffff, so bound+1 cannot be represented.
    assert!(discover(&source(0xff), &[0x1000], 64).dispatches.is_empty());
    assert!(discover(&source(1), &[0x1000], 3).dispatches.is_empty());
}

#[test]
fn changing_non_executable_table_data_only_changes_future_discovery() {
    let mut memory = source(1);
    let before = discover(&memory, &[0x1000], 64);
    memory.write(0x2000, &0x1050u32.to_le_bytes()).unwrap();
    let after = discover(&memory, &[0x1000], 64);
    assert_eq!(before.dispatches[&0x1005].targets[0], Some(0x1040));
    assert_eq!(after.dispatches[&0x1005].targets[0], Some(0x1050));
    // The earlier IR remains valid because its lowering checks the real target.
    assert!(crate::wasm::compile_control_graph(&before, Default::default(), 0, 64).is_ok());
}

#[test]
fn graph_verification_rejects_an_edge_that_skips_the_original_guest_load() {
    let mut graph = discover(&source(1), &[0x1000], 64);
    assert!(graph.verify().is_ok());
    graph
        .dispatches
        .get_mut(&0x1005)
        .unwrap()
        .operand
        .displacement = 0x3000;
    assert!(crate::wasm::compile_control_graph(&graph, Default::default(), 0, 64).is_err());
}

#[test]
fn ahead_of_time_roots_are_bounded_and_owned_before_cache_publication() {
    use crate::*;
    let owner = backend_new(8);
    let other = backend_new(8);
    let pointer = backend_input_alloc(owner, 8);
    assert_ne!(pointer, 0);
    assert_ne!(backend_compile_entries(other, pointer, 2, 64), 0);
    assert_ne!(backend_compile_entries(owner, pointer, 3, 64), 0);
    assert_ne!(backend_compile_entries(owner, pointer, 0, 64), 0);
    assert_ne!(backend_compile_entries(owner, pointer, 257, 512), 0);
    assert_eq!(backend_cache_selected(owner), 0);
    assert_eq!(backend_cache_selected(other), 0);
    assert_eq!(backend_free(pointer, 8), 0);
    assert_eq!(backend_drop(owner), 0);
    assert_eq!(backend_drop(other), 0);
}

#[test]
fn prepared_target_code_participates_in_mapping_and_smc_dependencies() {
    let mut backend = crate::Backend::new(8).unwrap();
    backend.memory = source(1);
    backend.control_graph = true;
    let mut target = [0xcc; 4096];
    target[0] = 0xc3;
    backend
        .memory
        .map_page(0x4000, 3, READ | EXECUTE | SEALED, &target)
        .unwrap();
    backend
        .memory
        .write(0x2000, &0x4000u32.to_le_bytes())
        .unwrap();
    backend.compile_entries(&[0x1000, 0x1005], 64).unwrap();
    let id = backend.cache.selected_id();
    assert!(backend.cache.validate(id, 0x1000, &backend.memory).is_ok());
    backend.memory.protect_page(0x4000, 3, READ).unwrap();
    assert!(backend.cache.validate(id, 0x1000, &backend.memory).is_err());
    backend
        .memory
        .protect_page(0x4000, 3, READ | EXECUTE | SEALED)
        .unwrap();
    backend.compile(0x1000, 64).unwrap();
    let id = backend.cache.selected_id();
    backend
        .memory
        .map_alias(0x5000, 0x4000, 4, READ | WRITE)
        .unwrap();
    backend.memory.write(0x5000, &[0x90]).unwrap();
    backend.memory.finish_execution().unwrap();
    assert!(backend.cache.validate(id, 0x1000, &backend.memory).is_err());
}

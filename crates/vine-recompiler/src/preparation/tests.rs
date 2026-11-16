use super::*;
use crate::memory::{Access, EXECUTE, PAGE_SIZE, READ, SEALED, WRITE};

fn backend() -> Backend {
    let mut backend = Backend::new(4).unwrap();
    let mut bytes = [0x90; PAGE_SIZE];
    bytes[20..22].copy_from_slice(&[0x0f, 0x0b]);
    backend
        .memory
        .map_page(0x1000, 1, READ | EXECUTE | SEALED, &bytes)
        .unwrap();
    backend
}

fn finish(backend: &mut Backend, limit: usize) -> u32 {
    for _ in 0..100 {
        backend.prepare_next(limit).unwrap();
        let status = backend.preparation.as_ref().unwrap().status;
        if status != READY {
            return status;
        }
    }
    panic!("preparation did not terminate");
}

fn map_jump(backend: &mut Backend, target: u32) {
    let mut bytes = [0xcc; PAGE_SIZE];
    bytes[0] = 0xe9;
    bytes[1..5].copy_from_slice(&target.wrapping_sub(5).to_le_bytes());
    backend
        .memory
        .map_page(0, 2, READ | EXECUTE | SEALED, &bytes)
        .unwrap();
}

#[test]
fn same_conversion_follows_frontiers_without_executing_or_writing() {
    let mut aot = backend();
    let mut jit = backend();
    let cpu = aot.state.clone();
    let epoch = aot.memory.epoch();
    let bytes = aot.memory.read(0x1000, PAGE_SIZE, Access::Read).unwrap();
    aot.begin_preparation(&[0x1000, 0x1000]).unwrap();
    aot.prepare_next(4).unwrap();
    jit.compile(0x1000, 4).unwrap();
    assert_eq!(
        aot.cache.selected().unwrap().code,
        jit.cache.selected().unwrap().code
    );
    assert_eq!(finish(&mut aot, 4), COMPLETE);
    assert!(aot.cache.select(0x1014, &aot.memory).is_some());
    assert_eq!(aot.state, cpu);
    assert_eq!(aot.memory.epoch(), epoch);
    assert_eq!(
        aot.memory.read(0x1000, PAGE_SIZE, Access::Read).unwrap(),
        bytes
    );
    assert!(!aot.memory.has_pending_writes());
    let count = aot.cache.len();
    aot.preparation = None;
    aot.begin_preparation(&[0x1000]).unwrap();
    assert_eq!(finish(&mut aot, 4), COMPLETE);
    assert_eq!(
        aot.cache.len(),
        count,
        "repeated preparation reuses cached frontiers"
    );
}

#[test]
fn demand_reuses_prepared_slots_without_losing_early_code() {
    let mut backend = backend();
    let mut code = [0; PAGE_SIZE];
    for pair in code.chunks_exact_mut(2) {
        pair.copy_from_slice(&[0x0f, 0x0b]);
    }
    backend.memory.unmap_page(0x1000).unwrap();
    backend
        .memory
        .map_page(0x1000, 1, READ | EXECUTE | SEALED, &code)
        .unwrap();
    let roots: Vec<_> = (0..65).map(|i| 0x1100 + i * 2).collect();
    backend.begin_preparation(&roots).unwrap();
    assert_eq!(finish(&mut backend, 1), COMPLETE);
    assert_eq!(backend.cache.len(), 65);
    assert_eq!(backend.cache.pop_retired(), 0);
    let first = backend.cache.select(roots[0], &backend.memory).unwrap();
    let last = backend.cache.select(roots[64], &backend.memory).unwrap();
    for index in 0..80 {
        backend.compile(0x1400 + index * 2, 1).unwrap();
    }
    assert_eq!(backend.cache.pop_retired(), 0);
    for index in 0..80 {
        assert!(
            backend
                .cache
                .select(0x1400 + index * 2, &backend.memory)
                .is_some()
        );
    }
    assert_eq!(backend.cache.select(roots[0], &backend.memory), Some(first));
    assert_eq!(backend.cache.select(roots[64], &backend.memory), Some(last));
}

#[test]
fn known_interior_roots_share_the_first_prepared_body() {
    let mut backend = backend();
    backend
        .begin_preparation(&[0x1000, 0x1005, 0x100a])
        .unwrap();
    backend.prepare_next(64).unwrap();
    let id = backend.cache.selected_id();
    assert_eq!(finish(&mut backend, 64), COMPLETE);
    for pc in [0x1000, 0x1005, 0x100a] {
        assert_eq!(backend.cache.select(pc, &backend.memory), Some(id));
    }
    assert_eq!(backend.cache.len(), 1);
}

#[test]
fn prepared_successors_remain_external_and_keep_normal_invalidation() {
    let mut backend = backend();
    let mut code = [0x90; PAGE_SIZE];
    code[..5].copy_from_slice(&[0xe9, 0xfb, 0x0f, 0, 0]); // JMP 0x1000
    backend
        .memory
        .map_page(0, 2, READ | EXECUTE | SEALED, &code)
        .unwrap();
    backend.begin_preparation(&[0x1000]).unwrap();
    backend.prepare_next(64).unwrap();
    let callee = backend.cache.selected_id();
    assert_eq!(finish(&mut backend, 64), COMPLETE);
    backend.preparation = None;
    backend.begin_preparation(&[0]).unwrap();
    backend.prepare_next(64).unwrap();
    let caller = backend.cache.selected_id();
    assert_ne!(caller, callee);
    let unit = backend.cache.selected().unwrap();
    assert_eq!(
        unit.instruction_count, 1,
        "the prepared successor is not cloned"
    );
    assert_eq!(unit.preparation_frontier, [0x1000]);
    assert_eq!(finish(&mut backend, 64), COMPLETE);
    backend.memory.unmap_page(0x1000).unwrap();
    backend.cache.reconcile(&mut backend.memory);
    assert_eq!(backend.cache.select(0, &backend.memory), Some(caller));
    assert!(backend.cache.select(0x1000, &backend.memory).is_none());
    assert!(!backend.cache.has_prepared_entry(0x1000, &backend.memory));
}

#[test]
fn changed_code_between_preparation_steps_cannot_reuse_stale_identity() {
    let mut backend = backend();
    backend.compile(0x1000, 4).unwrap();
    let id = backend.cache.selected_id();
    backend.begin_preparation(&[0x1000]).unwrap();
    backend
        .memory
        .map_alias(0x3000, 0x1000, 2, READ | WRITE)
        .unwrap();
    backend.memory.write(0x3000, &[0x0f, 0x0b]).unwrap();
    backend.memory.finish_execution().unwrap();
    backend.prepare_next(4).unwrap();
    assert_ne!(backend.cache.selected_id(), id);
    assert!(backend.cache.validate(id, 0x1000, &backend.memory).is_err());
    assert_eq!(finish(&mut backend, 4), COMPLETE);
}

#[test]
fn demand_discovery_keeps_prepared_ownership_after_preparation_ends() {
    for flow in [false, true] {
        let mut backend = backend();
        backend.flow_ir = flow;
        map_jump(&mut backend, 0x1000);
        backend.begin_preparation(&[0x1000]).unwrap();
        assert_eq!(finish(&mut backend, 64), COMPLETE);
        let prepared = backend.cache.selected_id();
        backend.preparation = None;
        let cpu = backend.state.clone();
        let epoch = backend.memory.epoch();
        backend.compile(0, 64).unwrap();
        let unit = backend.cache.selected().unwrap();
        assert_eq!(
            unit.instruction_count, 1,
            "demand code shares the prepared tail"
        );
        assert_eq!(unit.entries, BTreeSet::from([0]));
        assert_eq!(unit.dependencies.keys().copied().collect::<Vec<_>>(), [0]);
        assert_eq!(unit.preparation_frontier, [0x1000]);
        assert_eq!(
            backend.cache.select(0x1000, &backend.memory),
            Some(prepared)
        );
        assert_eq!(backend.state, cpu);
        assert_eq!(backend.memory.epoch(), epoch);

        // Explicit roots still request their own translation, even if another
        // resident unit already owns an executable entry at that address.
        backend.compile_entries(&[0, 0x1000], 64).unwrap();
        let unit = backend.cache.selected().unwrap();
        assert_eq!(unit.entries, BTreeSet::from([0, 0x1000]));
        assert_eq!(unit.instruction_count, 22);
    }
}

#[test]
fn pending_alias_write_revokes_prepared_discovery_boundary() {
    let mut backend = backend();
    map_jump(&mut backend, 0x1000);
    backend.begin_preparation(&[0x1000]).unwrap();
    assert_eq!(finish(&mut backend, 64), COMPLETE);
    backend.preparation = None;
    backend
        .memory
        .map_alias(0x3000, 0x1000, 3, READ | WRITE)
        .unwrap();
    backend.memory.write(0x3000, &[0x40, 0x0f, 0x0b]).unwrap();
    assert!(backend.memory.has_pending_writes());
    assert!(!backend.cache.has_prepared_entry(0x1000, &backend.memory));
    backend.compile(0, 64).unwrap();
    let unit = backend.cache.selected().unwrap();
    assert_eq!(
        unit.instruction_count, 3,
        "stale code cannot truncate discovery"
    );
    assert_eq!(
        unit.dependencies.keys().copied().collect::<Vec<_>>(),
        [0, 0x1000]
    );
}

#[test]
fn prepared_interiors_are_not_implicit_external_entries() {
    let mut backend = backend();
    map_jump(&mut backend, 0x1005);
    backend.begin_preparation(&[0x1000]).unwrap();
    assert_eq!(finish(&mut backend, 64), COMPLETE);
    backend.preparation = None;
    assert!(!backend.cache.has_prepared_entry(0x1005, &backend.memory));
    backend.compile(0, 64).unwrap();
    let unit = backend.cache.selected().unwrap();
    assert_eq!(unit.instruction_count, 17);
    assert_eq!(unit.entries, BTreeSet::from([0, 0x1005]));
    assert_eq!(
        unit.dependencies.keys().copied().collect::<Vec<_>>(),
        [0, 0x1000]
    );
}

#[test]
fn fetch_faults_and_indirect_transfers_remain_execution_work() {
    let mut backend = backend();
    backend.memory.protect_page(0x1000, 1, READ).unwrap();
    backend.begin_preparation(&[0x1000, 0xffff_ffff]).unwrap();
    assert_eq!(finish(&mut backend, 4), COMPLETE);
    assert_eq!(backend.state.exit.kind, 0);
    for entry in [0x1000, 0xffff_ffff] {
        assert!(
            backend.cache.select(entry, &backend.memory).is_some(),
            "every fault entry stays available"
        );
    }
    backend
        .memory
        .protect_page(0x1000, 1, READ | EXECUTE | SEALED)
        .unwrap();
    assert!(backend.cache.select(0x1000, &backend.memory).is_none());
}

#[test]
fn root_buffers_are_owned_bounded_and_validated_before_begin() {
    use crate::{backend_drop, backend_input_alloc, backend_new};
    let owner = backend_new(4);
    let other = backend_new(4);
    let pointer = backend_input_alloc(owner, 4);
    assert_ne!(backend_prepare_begin(other, pointer, 1), 0);
    assert_ne!(backend_prepare_begin(owner, pointer, 0), 0);
    assert_ne!(backend_prepare_begin(owner, pointer, 2), 0);
    assert_ne!(backend_prepare_begin(owner, pointer, 4097), 0);
    assert_eq!(backend_prepare_status(owner), 0);
    assert_eq!(backend_prepare_begin(owner, pointer, 1), 0);
    assert_ne!(backend_prepare_begin(owner, pointer, 1), 0);
    assert_eq!(backend_prepare_end(owner), 0);
    assert_eq!(backend_prepare_status(owner), 0);
    assert_eq!(backend_drop(owner), 0);
    assert_eq!(backend_drop(other), 0);
}

#[test]
fn a_planned_tail_rejects_pending_alias_writes_and_rediscovers_after_completion() {
    let mut backend = backend();
    backend.begin_preparation(&[0x1000]).unwrap();
    backend.prepare_next(4).unwrap();
    let old = backend.cache.selected_id();
    assert!(backend.preparation.as_ref().unwrap().planned.is_some());
    let cpu = backend.state.clone();
    backend
        .memory
        .map_alias(0x3000, 0x1000, 2, READ | WRITE)
        .unwrap();
    assert!(backend.memory.write_small(0x3008, &[0x0f, 0x0b]).unwrap());
    assert!(
        backend
            .prepare_next(4)
            .unwrap_err()
            .contains("pending writes")
    );
    assert_eq!(backend.state, cpu);
    backend.memory.finish_execution().unwrap();
    assert_eq!(finish(&mut backend, 4), COMPLETE);
    let current = backend.cache.select(0x1000, &backend.memory).unwrap();
    assert_ne!(
        current, old,
        "a stale native identity must never be revived"
    );
    backend.cache.select(0x1008, &backend.memory).unwrap();
    assert_eq!(
        backend.cache.selected().unwrap().instruction_count,
        1,
        "the new UD2 ends discovery"
    );
    assert_eq!(backend.state, cpu);
}

#[test]
fn a_planned_missing_page_and_diagnostic_boundary_are_reconsidered_at_handoffs() {
    let mut backend = backend();
    let mut code = [0xcc; PAGE_SIZE];
    code[..2].copy_from_slice(&[0x0f, 0x0b]);
    backend
        .memory
        .map_page(0x2000, 2, READ | EXECUTE | SEALED, &code)
        .unwrap();
    backend.begin_preparation(&[0x1000, 0x2000]).unwrap();
    backend.prepare_next(4).unwrap();
    assert!(backend.preparation.as_ref().unwrap().planned.is_some());
    backend.memory.unmap_page(0x2000).unwrap();
    backend.diagnostic_stop = Some(0x1008);
    assert_eq!(finish(&mut backend, 4), COMPLETE);
    backend.cache.select(0x1008, &backend.memory).unwrap();
    assert!(
        !backend
            .cache
            .selected()
            .unwrap()
            .interior_limits
            .contains_key(&0x1009),
        "no stale sequential suffix may follow the new diagnostic stop"
    );
    backend.cache.select(0x2000, &backend.memory).unwrap();
    assert_eq!(
        backend.cache.selected().unwrap().dependencies.get(&0x2000),
        Some(&None)
    );
    assert_eq!(
        backend.state.exit.kind, 0,
        "preparation never executes the diagnostic or fetch fault"
    );
}

#[test]
fn changing_lowering_at_a_handoff_discards_layout_and_preserves_pending_roots() {
    let mut backend = backend();
    let mut code = [0xcc; PAGE_SIZE];
    code[..2].copy_from_slice(&[0x0f, 0x0b]);
    backend
        .memory
        .map_page(0x2000, 2, READ | EXECUTE | SEALED, &code)
        .unwrap();
    let cpu = backend.state.clone();
    backend.begin_preparation(&[0x1000, 0x2000]).unwrap();
    backend.prepare_next(4).unwrap();
    assert!(backend.preparation.as_ref().unwrap().planned.is_some());
    // This is the same state/cache transition as backend_set_flow_ir.
    backend.flow_ir = true;
    backend.cache.clear();
    backend.prepare_next(4).unwrap();
    assert!(backend.preparation.as_ref().unwrap().planned.is_none());
    assert_eq!(finish(&mut backend, 4), COMPLETE);
    assert!(backend.cache.select(0x1000, &backend.memory).is_some());
    assert!(backend.cache.select(0x2000, &backend.memory).is_some());
    assert_eq!(backend.state, cpu);
}

use super::*;
use crate::memory::{Access, PAGE_SIZE, READ, WRITE};

fn setup() -> (CodeCache, MirrorMemory) {
    let mut memory = MirrorMemory::new(8).unwrap();
    let mut cache = CodeCache::default();
    cache.set_direct_links(true, &mut memory).unwrap();
    for address in [0x1000, 0x2000] {
        memory
            .map_page(address, 1, READ | EXECUTE | SEALED, &[0x90; PAGE_SIZE])
            .unwrap();
    }
    memory.map_alias(0x3000, 0x1000, 2, READ | WRITE).unwrap();
    memory
        .map_page(0x4000, 3, READ | WRITE, &[0; PAGE_SIZE])
        .unwrap();
    cache.reconcile(&mut memory);
    (cache, memory)
}
fn insert(cache: &mut CodeCache, memory: &MirrorMemory, entry: u32, extra: Option<u32>) -> u32 {
    let mut dependencies = BTreeMap::from([(entry & !4095, memory.code_stamp(entry))]);
    if let Some(page) = extra {
        dependencies.insert(page, memory.code_stamp(page));
    }
    let id = cache
        .insert(Unit {
            code: vec![0; 4].into_boxed_slice(),
            entries: BTreeSet::from([entry]),
            dependencies,
            instruction_count: 1,
            interior_limits: BTreeMap::new(),
            preparation_frontier: Vec::new(),
        })
        .unwrap();
    cache.bind(id, cache.link_slot(id), memory).unwrap();
    id
}
fn binding(cache: &CodeCache, entry: u32) -> u64 {
    cache.direct.as_ref().unwrap().binding(entry)
}

#[test]
fn changed_code_dependencies_revoke_only_affected_units_and_retain_completed_effects() {
    for mutation in 0..6 {
        let (mut cache, mut memory) = setup();
        let first = insert(&mut cache, &memory, 0x1000, None);
        let second = insert(&mut cache, &memory, 0x2000, None);
        let both = insert(&mut cache, &memory, 0x2001, Some(0x1000));
        cache.begin_invocation(first, 0x1000, &memory).unwrap();
        cache.end_invocation();
        match mutation {
            0 => memory.write(0x3000, &[0xcc]).unwrap(),
            1 => {
                assert!(memory.write_small(0x3000, &[0xcc]).unwrap());
                memory.finish_execution().unwrap();
            }
            2 => memory
                .reset_page(0x3000, 2, READ | WRITE, &[0xcc; PAGE_SIZE])
                .unwrap(),
            3 => memory.protect_page(0x1000, 1, READ).unwrap(),
            4 => memory.unmap_page(0x1000).unwrap(),
            _ => {
                memory.unmap_page(0x1000).unwrap();
                memory
                    .map_page(0x1000, 7, READ | EXECUTE | SEALED, &[0x90; PAGE_SIZE])
                    .unwrap();
            }
        }
        cache.begin_invocation(second, 0x2000, &memory).unwrap();
        assert!(!cache.direct.as_ref().unwrap().active());
        cache.reconcile(&mut memory);
        assert_eq!(binding(&cache, 0x1000), 0);
        assert_eq!(binding(&cache, 0x2001), 0);
        assert_eq!(binding(&cache, 0x2000) >> 32, u64::from(second));
        assert!(!cache.units.contains_key(&first) && !cache.units.contains_key(&both));
        cache.begin_invocation(second, 0x2000, &memory).unwrap();
        if mutation < 3 {
            assert_eq!(memory.read(0x3000, 1, Access::Read).unwrap(), [0xcc]);
        }
    }
}

#[test]
fn ordinary_dirty_writes_require_finish_but_do_not_revoke_code_or_alias_proof() {
    let (mut cache, mut memory) = setup();
    let id = insert(&mut cache, &memory, 0x1000, None);
    let original = binding(&cache, 0x1000);
    assert!(!memory.write_small(0x4000, &[5]).unwrap());
    cache.begin_invocation(id, 0x1000, &memory).unwrap();
    assert!(!cache.direct.as_ref().unwrap().active());
    memory.finish_execution().unwrap();
    assert!(!memory.code_changes_pending());
    cache.reconcile(&mut memory);
    cache.begin_invocation(id, 0x1000, &memory).unwrap();
    assert_eq!(binding(&cache, 0x1000), original);
    assert_eq!(memory.read(0x4000, 1, Access::Read).unwrap(), [5]);
}

#[test]
fn fetch_fault_dependencies_stay_on_checked_path_and_failed_bind_never_publishes() {
    let (mut cache, memory) = setup();
    for dependency in [0x4000, 0x5000] {
        let id = insert(&mut cache, &memory, 0x1000, Some(dependency));
        assert_eq!(binding(&cache, 0x1000), 0);
        assert_ne!(cache.resolve(0x1000, &memory), 0);
        cache.retire(id);
        assert_eq!(cache.pop_retired(), id);
    }
    let id = insert(&mut cache, &memory, 0x1000, None);
    cache.retire(id);
    assert_eq!(binding(&cache, 0x1000), 0);
    assert!(cache.bind(id, 1, &memory).is_err());
    assert_eq!(binding(&cache, 0x1000), 0);
}

#[test]
fn overlapping_binding_retirement_preserves_new_identity_and_overflow_revokes_survivors() {
    let (mut cache, memory) = setup();
    let old = insert(&mut cache, &memory, 0x1000, None);
    let new = insert(&mut cache, &memory, 0x1000, None);
    cache.retire(old);
    assert_eq!(binding(&cache, 0x1000) >> 32, u64::from(new));
    assert_eq!(cache.pop_retired(), old);
    // Simulate an unpolled consumer while preserving a live bound unit.
    for _ in 0..=cache.maximum_units {
        let id = cache
            .insert(Unit {
                code: vec![0; 4].into_boxed_slice(),
                entries: BTreeSet::from([0x2000]),
                dependencies: BTreeMap::from([(0x2000, memory.code_stamp(0x2000))]),
                instruction_count: 1,
                interior_limits: BTreeMap::new(),
                preparation_frontier: Vec::new(),
            })
            .unwrap();
        cache.retire(id);
    }
    assert_eq!(cache.pop_retired(), RETIRE_ALL);
    assert_eq!(binding(&cache, 0x1000), 0);
    assert!(cache.begin_invocation(new, 0x1000, &memory).is_err());
    cache.bind(new, cache.link_slot(new), &memory).unwrap();
    assert_eq!(binding(&cache, 0x1000) >> 32, u64::from(new));
}

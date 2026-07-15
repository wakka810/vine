use std::collections::{BTreeMap, btree_map::Entry};

use vine_core::{
    DirtyPage, GuestAddress, GuestMemory, GuestRange, MappingOperation, MemoryDelta, MemoryLimits,
    MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind, PageProtection,
};

const BASE: GuestAddress = GuestAddress(0x4000_0000);
const PAGE_SIZE: u32 = 0x1000;
const PAGE_COUNT: u32 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModelPage {
    owner: MemoryOwnerId,
    protection: PageProtection,
    bytes: Vec<u8>,
}

#[test]
fn randomized_guest_memory_operations_match_a_sparse_reference_model() {
    for seed in 1..=32 {
        run_sequence(seed);
    }
}

fn run_sequence(seed: u64) {
    let mut random = Random(seed);
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: PAGE_COUNT as usize,
        max_regions: 4,
        max_owners: 4,
        max_access_bytes: 0x2000,
    })
    .expect("memory");
    let mut owner = memory
        .create_owner(MemoryOwnerKind::Anonymous)
        .expect("owner");
    let whole_range = memory
        .reserve_fixed(
            owner,
            MemoryRegionKind::Anonymous,
            BASE,
            u64::from(PAGE_SIZE * PAGE_COUNT),
        )
        .expect("reserve");
    memory
        .commit(owner, whole_range, read_write())
        .expect("commit initial pages");
    memory.seal_owner(owner).expect("seal owner");

    let reflection = memory.pending_reflection().expect("initial reflection");
    let mut mirror = reflection
        .pages
        .iter()
        .map(|page| {
            (
                page.address.0,
                ModelPage {
                    owner: page.owner,
                    protection: page.protection,
                    bytes: page.bytes.clone(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut model = mirror.clone();
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("initial acknowledgement");

    for _ in 0..128 {
        match random.next_u32() % 9 {
            0 => random_write(&mut random, &mut memory, &mut model, &mut mirror),
            1 => random_protect(&mut random, &mut memory, owner, &mut model, &mut mirror),
            2 => random_decommit(&mut random, &mut memory, owner, &mut model, &mut mirror),
            3 => random_commit(&mut random, &mut memory, owner, &mut model, &mut mirror),
            4 => random_read(&mut random, &memory, &model),
            5 => random_fetch(&mut random, &memory, &model),
            6 => rejected_wrong_owner_operation(&mut memory, whole_range),
            7 => rollback_random_transaction(&mut memory, owner, &model),
            _ => {
                memory
                    .release_region(owner, BASE)
                    .expect("release primary region");
                model.clear();
                synchronize(&mut memory, &mut mirror, &model);
                memory.destroy_owner(owner).expect("destroy released owner");
                owner = memory
                    .create_owner(MemoryOwnerKind::Anonymous)
                    .expect("replacement owner");
                memory
                    .reserve_fixed(
                        owner,
                        MemoryRegionKind::Anonymous,
                        BASE,
                        u64::from(PAGE_SIZE * PAGE_COUNT),
                    )
                    .expect("replacement reserve");
                memory.seal_owner(owner).expect("seal replacement owner");
                synchronize(&mut memory, &mut mirror, &model);
            }
        }
        assert_eq!(mirror, model, "seed {seed}");
        let stats = memory.stats();
        assert_eq!(stats.committed_pages, model.len(), "seed {seed}");
        assert_eq!(stats.physical_pages, model.len(), "seed {seed}");
        assert_eq!(stats.dirty_pages, 0, "seed {seed}");
        assert_eq!(
            stats.generation, stats.acknowledged_generation,
            "seed {seed}"
        );
        assert_eq!(stats.pending_invalidation, None, "seed {seed}");
    }
}

fn random_write(
    random: &mut Random,
    memory: &mut GuestMemory,
    model: &mut BTreeMap<u32, ModelPage>,
    mirror: &mut BTreeMap<u32, ModelPage>,
) {
    let offset = random.next_u32() % (PAGE_SIZE * PAGE_COUNT);
    let length = (random.next_u32() % 16 + 1) as usize;
    let address = GuestAddress(BASE.0 + offset);
    let bytes = (0..length)
        .map(|_| random.next_u32() as u8)
        .collect::<Vec<_>>();
    let expected = model_access(model, address, length, |page| page.protection.write);
    let generation_before = memory.stats().generation;
    let result = memory.write(address, &bytes);
    if expected {
        result.expect("model-approved write");
        write_model(model, address, &bytes);
        synchronize(memory, mirror, model);
    } else {
        result.expect_err("model-rejected write");
        assert_eq!(memory.stats().generation, generation_before);
    }
}

fn random_protect(
    random: &mut Random,
    memory: &mut GuestMemory,
    owner: MemoryOwnerId,
    model: &mut BTreeMap<u32, ModelPage>,
    mirror: &mut BTreeMap<u32, ModelPage>,
) {
    let page_index = random.next_u32() % PAGE_COUNT;
    let address = BASE.0 + page_index * PAGE_SIZE;
    let range = page_range(address);
    let protection = PageProtection {
        read: true,
        write: random.next_u32() & 1 != 0,
        execute: random.next_u32() & 1 != 0,
    };
    let result = memory.protect(owner, range, protection);
    if let Some(page) = model.get_mut(&address) {
        result.expect("model-approved protect");
        page.protection = protection;
        synchronize(memory, mirror, model);
    } else {
        result.expect_err("model-rejected protect");
    }
}

fn random_decommit(
    random: &mut Random,
    memory: &mut GuestMemory,
    owner: MemoryOwnerId,
    model: &mut BTreeMap<u32, ModelPage>,
    mirror: &mut BTreeMap<u32, ModelPage>,
) {
    let address = BASE.0 + (random.next_u32() % PAGE_COUNT) * PAGE_SIZE;
    let result = memory.decommit(owner, page_range(address));
    if model.remove(&address).is_some() {
        result.expect("model-approved decommit");
        synchronize(memory, mirror, model);
    } else {
        result.expect_err("model-rejected decommit");
    }
}

fn random_commit(
    random: &mut Random,
    memory: &mut GuestMemory,
    owner: MemoryOwnerId,
    model: &mut BTreeMap<u32, ModelPage>,
    mirror: &mut BTreeMap<u32, ModelPage>,
) {
    let address = BASE.0 + (random.next_u32() % PAGE_COUNT) * PAGE_SIZE;
    let protection = PageProtection {
        read: true,
        write: random.next_u32() & 1 != 0,
        execute: random.next_u32() & 1 != 0,
    };
    let result = memory.commit(owner, page_range(address), protection);
    match model.entry(address) {
        Entry::Occupied(_) => {
            result.expect_err("model-rejected duplicate commit");
        }
        Entry::Vacant(entry) => {
            result.expect("model-approved commit");
            entry.insert(ModelPage {
                owner,
                protection,
                bytes: vec![0; PAGE_SIZE as usize],
            });
            synchronize(memory, mirror, model);
        }
    }
}

fn random_read(random: &mut Random, memory: &GuestMemory, model: &BTreeMap<u32, ModelPage>) {
    let offset = random.next_u32() % (PAGE_SIZE * PAGE_COUNT);
    let length = (random.next_u32() % 16 + 1) as usize;
    let address = GuestAddress(BASE.0 + offset);
    let result = memory.read(address, length);
    if model_access(model, address, length, |page| page.protection.read) {
        assert_eq!(
            result.expect("model-approved read"),
            read_model(model, address, length)
        );
    } else {
        result.expect_err("model-rejected read");
    }
}

fn random_fetch(random: &mut Random, memory: &GuestMemory, model: &BTreeMap<u32, ModelPage>) {
    let offset = random.next_u32() % (PAGE_SIZE * PAGE_COUNT);
    let length = (random.next_u32() % 8 + 1) as usize;
    let address = GuestAddress(BASE.0 + offset);
    let result = memory.fetch(address, length);
    if model_access(model, address, length, |page| page.protection.execute) {
        assert_eq!(
            result.expect("model-approved fetch"),
            read_model(model, address, length)
        );
    } else {
        result.expect_err("model-rejected fetch");
    }
}

fn rejected_wrong_owner_operation(memory: &mut GuestMemory, range: GuestRange) {
    let wrong_owner = memory
        .create_owner(MemoryOwnerKind::Anonymous)
        .expect("wrong owner");
    let before = memory.stats();
    memory
        .commit(wrong_owner, range, read_write())
        .expect_err("wrong owner commit");
    assert_eq!(memory.stats(), before);
    memory
        .destroy_owner(wrong_owner)
        .expect("destroy wrong owner");
}

fn rollback_random_transaction(
    memory: &mut GuestMemory,
    owner: MemoryOwnerId,
    model: &BTreeMap<u32, ModelPage>,
) {
    let before = memory.stats();
    {
        let mut transaction = memory.begin_transaction();
        if let Some((&address, _)) = model.iter().find(|(_, page)| page.protection.write) {
            transaction
                .write(GuestAddress(address), &[0xfe])
                .expect("transaction write");
        }
        if let Some((&address, _)) = model.iter().next() {
            transaction
                .protect(owner, page_range(address), read_write())
                .expect("transaction protect");
        }
        let temporary_owner = transaction
            .create_owner(MemoryOwnerKind::Runtime)
            .expect("transaction owner");
        transaction
            .reserve_fixed(
                temporary_owner,
                MemoryRegionKind::Runtime,
                GuestAddress(0x5000_0000),
                0x1000,
            )
            .expect("transaction reserve");
    }
    assert_eq!(memory.stats(), before);
}

fn synchronize(
    memory: &mut GuestMemory,
    mirror: &mut BTreeMap<u32, ModelPage>,
    model: &BTreeMap<u32, ModelPage>,
) {
    let delta = memory.pending_delta().expect("pending delta");
    apply_delta(mirror, &delta);
    memory
        .acknowledge_delta(delta.generation)
        .expect("delta acknowledgement");
    assert_eq!(mirror, model);
}

fn apply_delta(mirror: &mut BTreeMap<u32, ModelPage>, delta: &MemoryDelta) {
    for operation in &delta.mapping_operations {
        match operation {
            MappingOperation::Reserve { .. } => {}
            MappingOperation::Commit {
                owner,
                range,
                protection,
                ..
            } => {
                for address in page_addresses(*range) {
                    mirror.insert(
                        address,
                        ModelPage {
                            owner: *owner,
                            protection: *protection,
                            bytes: vec![0; PAGE_SIZE as usize],
                        },
                    );
                }
            }
            MappingOperation::Protect {
                range, protection, ..
            } => {
                for address in page_addresses(*range) {
                    mirror
                        .get_mut(&address)
                        .expect("protected mirror page")
                        .protection = *protection;
                }
            }
            MappingOperation::Decommit { range, .. } | MappingOperation::Release { range, .. } => {
                for address in page_addresses(*range) {
                    mirror.remove(&address);
                }
            }
        }
    }
    for dirty in &delta.dirty_pages {
        apply_dirty_page(mirror, dirty);
    }
}

fn apply_dirty_page(mirror: &mut BTreeMap<u32, ModelPage>, dirty: &DirtyPage) {
    mirror.insert(
        dirty.address.0,
        ModelPage {
            owner: dirty.owner,
            protection: dirty.protection,
            bytes: dirty.bytes.clone(),
        },
    );
}

fn model_access(
    model: &BTreeMap<u32, ModelPage>,
    address: GuestAddress,
    length: usize,
    allowed: impl Fn(&ModelPage) -> bool,
) -> bool {
    (0..length).all(|offset| {
        let current = u64::from(address.0) + offset as u64;
        if current >= u64::from(BASE.0 + PAGE_SIZE * PAGE_COUNT) {
            return false;
        }
        let page = (current as u32) & !(PAGE_SIZE - 1);
        model.get(&page).is_some_and(&allowed)
    })
}

fn write_model(model: &mut BTreeMap<u32, ModelPage>, address: GuestAddress, bytes: &[u8]) {
    for (offset, byte) in bytes.iter().enumerate() {
        let current = address.0 + offset as u32;
        let page = current & !(PAGE_SIZE - 1);
        model.get_mut(&page).expect("write model page").bytes[(current - page) as usize] = *byte;
    }
}

fn read_model(model: &BTreeMap<u32, ModelPage>, address: GuestAddress, length: usize) -> Vec<u8> {
    (0..length)
        .map(|offset| {
            let current = address.0 + offset as u32;
            let page = current & !(PAGE_SIZE - 1);
            model[&page].bytes[(current - page) as usize]
        })
        .collect()
}

fn page_range(address: u32) -> GuestRange {
    GuestRange::new(GuestAddress(address), u64::from(PAGE_SIZE)).expect("page range")
}

fn page_addresses(range: GuestRange) -> impl Iterator<Item = u32> {
    (range.start().0..range.end() as u32).step_by(PAGE_SIZE as usize)
}

fn read_write() -> PageProtection {
    PageProtection {
        read: true,
        write: true,
        execute: false,
    }
}

struct Random(u64);

impl Random {
    fn next_u32(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 32) as u32
    }
}

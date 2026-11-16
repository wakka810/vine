//! Executed as Wasm by test-shared-memory.mjs, with the real shared backing.
use std::cell::RefCell;
use vine_core::{
    GuestAddress, GuestMemory, GuestRange, MemoryLimits, MemoryOwnerId, MemoryOwnerKind,
    MemoryRegionKind, PageProtection,
};

const RW: PageProtection = PageProtection {
    read: true,
    write: true,
    execute: false,
};
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn test_panic(pointer: u32, length: u32);
}
thread_local! { static MEMORY: RefCell<Option<(GuestMemory, MemoryOwnerId)>> = const { RefCell::new(None) }; }
fn ack(memory: &mut GuestMemory) {
    let output = memory.pending_delta_binary().unwrap();
    assert!(output.page_bytes.is_empty());
    memory
        .acknowledge_delta(memory.synchronization_state().generation)
        .unwrap();
}
#[unsafe(no_mangle)]
pub fn setup() -> u32 {
    std::panic::set_hook(Box::new(|error| {
        let message = error.to_string();
        unsafe {
            test_panic(message.as_ptr() as u32, message.len() as u32);
        }
    }));
    let mut memory = GuestMemory::new_shared(MemoryLimits {
        max_physical_pages: 2,
        ..MemoryLimits::default()
    })
    .unwrap();
    assert!(GuestMemory::new_shared(MemoryLimits::default()).is_err());
    let owner = memory.create_owner(MemoryOwnerKind::Anonymous).unwrap();
    let generation = memory.synchronization_state().generation;
    assert!(
        memory
            .reserve_fixed(
                owner,
                MemoryRegionKind::Anonymous,
                GuestAddress(0x7ffff000),
                8192
            )
            .is_err()
    );
    assert_eq!(memory.synchronization_state().generation, generation);
    let range = memory
        .reserve_fixed(
            owner,
            MemoryRegionKind::Anonymous,
            GuestAddress(0x10000),
            12288,
        )
        .unwrap();
    memory
        .commit(owner, GuestRange::new(range.start(), 8192).unwrap(), RW)
        .unwrap();
    let generation = memory.synchronization_state().generation;
    assert!(
        memory
            .commit(
                owner,
                GuestRange::new(GuestAddress(0x12000), 4096).unwrap(),
                RW
            )
            .is_err()
    );
    assert_eq!(memory.synchronization_state().generation, generation);
    memory.seal_owner(owner).unwrap();
    let reflection = memory.pending_reflection_binary().unwrap();
    assert!(reflection.page_bytes.is_empty());
    memory
        .acknowledge_delta(memory.synchronization_state().generation)
        .unwrap();
    let (base, _, offset) = memory.shared_arena_descriptor().unwrap();
    MEMORY.with(|m| *m.borrow_mut() = Some((memory, owner)));
    base + offset + 0x10000
}
#[unsafe(no_mangle)]
pub fn permission_pointer() -> u32 {
    MEMORY.with(|m| {
        m.borrow()
            .as_ref()
            .unwrap()
            .0
            .shared_permissions_pointer()
            .unwrap()
    })
}
#[unsafe(no_mangle)]
pub fn verify() -> u32 {
    MEMORY.with(|m| {
        let mut m = m.borrow_mut();
        let (memory, owner) = m.as_mut().unwrap();
        let owner = *owner;
        let address = GuestAddress(0x10000);
        let range = GuestRange::new(address, 4096).unwrap();
        let before = memory.read(address, 4096).unwrap();
        assert_eq!(&before[..4], &[0x42, 0x19, 0x87, 0x65]);
        let generation = memory.synchronization_state().generation;
        {
            let mut tx = memory.begin_transaction();
            tx.decommit(owner, range).unwrap();
            tx.commit(owner, range, RW).unwrap();
            tx.pending_delta_binary().unwrap();
        }
        assert_eq!(memory.synchronization_state().generation, generation);
        assert_eq!(memory.read(address, 4096).unwrap(), before);
        {
            let mut tx = memory.begin_transaction();
            tx.protect(owner, range, PageProtection { write: false, ..RW })
                .unwrap();
            tx.pending_delta_binary().unwrap();
        }
        memory.validate_shared_execution(generation).unwrap();
        assert_eq!(
            unsafe {
                *((memory.shared_permissions_pointer().unwrap() as usize as *const u8).add(0x10))
            },
            3
        );
        memory
            .protect(owner, range, PageProtection { write: false, ..RW })
            .unwrap();
        ack(memory);
        assert!(memory.write(address, &[0xff]).is_err());
        assert_eq!(memory.read(address, 4096).unwrap(), before);
        memory.protect(owner, range, RW).unwrap();
        ack(memory);
        memory.release_region(owner, address).unwrap();
        ack(memory);
        let range = memory
            .reserve_fixed(owner, MemoryRegionKind::Anonymous, address, 4096)
            .unwrap();
        memory.commit(owner, range, RW).unwrap();
        ack(memory);
        assert!(memory.read(address, 4096).unwrap().iter().all(|b| *b == 0));
        memory.write(address, &[0x7f]).unwrap();
        ack(memory);
    });
    0
}
#[unsafe(no_mangle)]
pub fn discard() {
    MEMORY.with(|m| *m.borrow_mut() = None);
}

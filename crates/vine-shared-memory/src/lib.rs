//! The single-threaded common guest arena. Only this crate constructs Rust
//! references into the reserved Wasm range; vine-core remains unsafe-free.
//! A CPU may access the arena only between Runtime calls, with the Runtime's
//! current page permissions installed and no Rust page references alive.
#![deny(unsafe_op_in_unsafe_fn)]

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::sync::Mutex;

pub const PAGE_BYTES: usize = 4096;
pub const GUEST_BYTES: u32 = 0x8000_0000;
pub const GUEST_OFFSET: u32 = 0x0080_0000;
pub const PHYSICAL_BYTES: u32 = GUEST_OFFSET + GUEST_BYTES;
static RESERVATION: Mutex<(usize, bool)> = Mutex::new((0, false));

#[derive(Debug)]
struct Inner {
    base: usize,
    pages: RefCell<BTreeSet<u32>>,
    permissions: RefCell<Box<[u8]>>,
}
impl Drop for Inner {
    fn drop(&mut self) {
        RESERVATION.lock().expect("arena reservation poisoned").1 = false;
    }
}

#[derive(Debug)]
pub struct Arena(Rc<Inner>);
impl Arena {
    pub fn reserve() -> Result<Self, &'static str> {
        let mut reservation = RESERVATION
            .lock()
            .map_err(|_| "arena reservation poisoned")?;
        if reservation.1 {
            return Err("a guest arena is already live in this Wasm instance");
        }
        if reservation.0 == 0 {
            #[cfg(target_arch = "wasm32")]
            {
                let current = core::arch::wasm32::memory_size::<0>();
                let pages = (PHYSICAL_BYTES as usize) / 65536;
                if current.checked_add(pages).is_none_or(|end| end > 65536) {
                    return Err("guest arena exceeds the Wasm32 memory limit");
                }
                let old = core::arch::wasm32::memory_grow::<0>(pages);
                if old == usize::MAX {
                    return Err("Chrome could not reserve the guest arena");
                }
                reservation.0 = old * 65536;
            }
            #[cfg(not(target_arch = "wasm32"))]
            return Err("the shared guest arena requires Wasm32");
        }
        reservation.1 = true;
        Ok(Self(Rc::new(Inner {
            base: reservation.0,
            pages: RefCell::new(BTreeSet::new()),
            // Cover the entire u32 address space so a denied high guest address
            // never indexes beyond the permission table.
            permissions: RefCell::new(vec![0; 1 << 20].into_boxed_slice()),
        })))
    }
    pub fn physical_base(&self) -> u32 {
        self.0.base as u32
    }
    pub fn guest_base(&self) -> u32 {
        self.physical_base() + GUEST_OFFSET
    }
    pub fn permissions_pointer(&self) -> u32 {
        self.0.permissions.borrow().as_ptr() as usize as u32
    }
    pub fn set_permissions(&mut self, page: u32, flags: u8) {
        assert!(page < GUEST_BYTES / PAGE_BYTES as u32);
        self.0.permissions.borrow_mut()[page as usize] = flags;
    }
    pub fn allocate(&mut self, address: u32) -> Result<Page, &'static str> {
        if address & 4095 != 0 || address >= GUEST_BYTES {
            return Err("guest page is outside the shared user address space");
        }
        if !self.0.pages.borrow_mut().insert(address) {
            return Err("guest page is already borrowed");
        }
        let mut page = Page {
            arena: Rc::clone(&self.0),
            address,
        };
        page.fill(0);
        Ok(page)
    }
}

#[derive(Debug)]
pub struct Page {
    arena: Rc<Inner>,
    address: u32,
}
impl Page {
    pub fn address(&self) -> u32 {
        self.address
    }
    fn pointer(&self) -> *mut [u8; PAGE_BYTES] {
        (self.arena.base + GUEST_OFFSET as usize + self.address as usize) as *mut [u8; PAGE_BYTES]
    }
}
impl Deref for Page {
    type Target = [u8; PAGE_BYTES];
    fn deref(&self) -> &Self::Target {
        // The range was reserved before publication, is never moved/released,
        // and this non-cloneable Page is its unique live Rust owner.
        unsafe { &*self.pointer() }
    }
}
impl DerefMut for Page {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.pointer() }
    }
}
impl Drop for Page {
    fn drop(&mut self) {
        assert!(self.arena.pages.borrow_mut().remove(&self.address));
    }
}

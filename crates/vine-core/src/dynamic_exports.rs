use crate::memory::GuestAddress;

pub const DYNAMIC_EXPORT_ARENA_BASE: GuestAddress = GuestAddress(0x7f02_0000);
pub const DYNAMIC_EXPORT_ARENA_SIZE: u64 = 0x1_0000;
pub const DYNAMIC_EXPORT_SLOT_SIZE: u32 = 0x10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DynamicExportThunkId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicExportThunk {
    pub id: DynamicExportThunkId,
    pub address: GuestAddress,
    pub module: String,
    pub name: String,
}

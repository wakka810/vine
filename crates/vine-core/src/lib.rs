#![forbid(unsafe_code)]

mod bootstrap;
mod callbacks;
mod imports;
mod materialize;
mod memory;
mod pe32;
mod snapshot;

pub use bootstrap::{
    CONTROLLED_INITIAL_STACK_BASE, CONTROLLED_INITIAL_STACK_SIZE, CONTROLLED_INITIAL_STACK_TOP,
    InitialThreadStack, install_controlled_initial_stack,
};
pub use callbacks::{
    CALLBACK_RETURN_ARENA_BASE, CALLBACK_RETURN_ARENA_SIZE, CALLBACK_RETURN_SLOT_SIZE,
    CallbackReturnSentinel, CallbackReturnSentinelId, PROCESS_ENTRY_RETURN_SENTINEL_ADDRESS,
    ProcessEntryReturnSentinel, ProcessEntryReturnSentinelId, RuntimeReturnSentinels,
    install_callback_return_sentinel, install_runtime_return_sentinels,
};
pub use imports::{
    IMPORT_THUNK_ARENA_BASE, IMPORT_THUNK_ARENA_SIZE, IMPORT_THUNK_SLOT_SIZE, ImportBinding,
    ImportThunk, ImportThunkArena, ImportThunkId, RuntimeExport, RuntimeExportCatalog,
    bind_pe32_imports,
};
pub use materialize::{ImageMaterialization, UnboundImport, materialize_pe32_image};
pub use memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestMemoryError, GuestMemoryErrorClass,
    GuestRange, GuestWriteResult, ImageMemoryLayout, MemoryAccess, MemoryFault, MemoryFaultReason,
    MemoryInvalidation, MemoryLimits, MemoryOwnerId, MemoryOwnerKind, MemoryReflection,
    MemoryReflectionPage, MemoryRegionKind, MemoryStats, PageProtection,
};
pub use pe32::{
    DataDirectoryAddressKind, DataDirectoryPlan, FeatureInventory, HeaderMappingPlan, ImageKind,
    ImagePlan, ImportModulePlan, ImportSymbolPlan, ImportTarget, LoaderError, LoaderErrorClass,
    LoaderIngressReport, MappingProtection, Pe32Limits, Pe32LoadPlan, RelocationBlockPlan,
    RelocationOperation, RelocationPlan, Rva, SectionMappingPlan, UnsupportedPoint,
    build_pe32_load_plan,
};
pub use snapshot::{ModuleIdentity, ModuleSnapshot};

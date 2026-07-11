#![forbid(unsafe_code)]

mod bootstrap;
mod callbacks;
mod environment;
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
pub use environment::{
    CONTROLLED_COMMAND_LINE, CONTROLLED_COMMAND_LINE_BASE, CONTROLLED_ENVIRONMENT_PAGE_SIZE,
    CONTROLLED_PEB_BASE, CONTROLLED_PROCESS_PARAMETERS_BASE, CONTROLLED_TEB_BASE,
    InitialProcessEnvironment, InitialProcessParameters, PEB_PROCESS_PARAMETERS_POINTER_OFFSET,
    PROCESS_PARAMETERS_COMMAND_LINE_OFFSET, TEB_PEB_POINTER_OFFSET, TEB_SELF_POINTER_OFFSET,
    UNICODE_STRING_BUFFER_OFFSET, install_controlled_process_environment,
    install_controlled_process_environment_with_command_line,
    install_controlled_process_environment_with_parameters,
};
pub use imports::{
    IMPORT_THUNK_ARENA_BASE, IMPORT_THUNK_ARENA_SIZE, IMPORT_THUNK_SLOT_SIZE, ImportBinding,
    ImportBindingPlan, ImportBindingPolicy, ImportPlanDisposition, ImportThunk, ImportThunkArena,
    ImportThunkDisposition, ImportThunkId, PlannedImport, RuntimeExport, RuntimeExportCatalog,
    bind_pe32_imports, bind_pe32_imports_with_unsupported_traps, commit_pe32_import_binding,
    plan_pe32_imports, plan_pe32_imports_with_unsupported_traps,
};
pub use materialize::{ImageMaterialization, UnboundImport, materialize_pe32_image};
pub use memory::{
    CacheEffects, DirtyPage, GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestMemoryError,
    GuestMemoryErrorClass, GuestMemoryTransaction, GuestPageReconciliation, GuestRange,
    GuestWriteResult, ImageMemoryLayout, MappingOperation, MemoryAccess, MemoryDelta, MemoryFault,
    MemoryFaultReason, MemoryInvalidation, MemoryLimits, MemoryOwnerId, MemoryOwnerKind,
    MemoryReflection, MemoryReflectionPage, MemoryRegionKind, MemoryStats, PageProtection,
};
pub use pe32::{
    DataDirectoryAddressKind, DataDirectoryPlan, FeatureInventory, HeaderMappingPlan, ImageKind,
    ImagePlan, ImportModulePlan, ImportSymbolPlan, ImportTarget, LoaderError, LoaderErrorClass,
    LoaderIngressReport, MappingProtection, Pe32Limits, Pe32LoadPlan, RelocationBlockPlan,
    RelocationOperation, RelocationPlan, Rva, SectionMappingPlan, UnsupportedPoint,
    build_pe32_load_plan,
};
pub use snapshot::{ModuleIdentity, ModuleSnapshot};

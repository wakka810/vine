#![forbid(unsafe_code)]

mod api;
mod bootstrap;
mod callbacks;
mod dll;
mod dynamic_exports;
mod environment;
mod heap;
mod imports;
mod materialize;
mod memory;
mod module_graph;
mod module_images;
mod pe32;
mod runtime;
mod snapshot;
mod win32;

pub use api::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiCallFrame, ApiContract, ApiDeferredCall,
    ApiDirection, ApiDispatcher, ApiEndpointBinding, ApiEndpointIdentity, ApiFault, ApiFaultClass,
    ApiGuestCallbackCall, ApiGuestWrite, ApiGuestWriteReceipt, ApiHandlerResult, ApiImmediate,
    ApiImmediateCompletion, ApiOwnedTrap, ApiPointeeSchema, ApiPointerConstraint, ApiReturnSchema,
    ApiReturnValue, ApiStringEncoding, ApiSymbol, ApiTrap, ApiValueSchema, ApiWriteRequirement,
    CallbackPolicy, CallingConvention, DecodedApiArgument, DispatchOutcome, ExecutionSession,
    GuestCallbackRequest, HostCompletion, HostRequest, LastErrorPolicy, ReentrancyPolicy,
    RegisterUpdate, StackCleanup,
};

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
pub use dynamic_exports::{
    DYNAMIC_EXPORT_ARENA_BASE, DYNAMIC_EXPORT_ARENA_SIZE, DYNAMIC_EXPORT_ENDPOINT_NAMESPACE,
    DYNAMIC_EXPORT_SLOT_SIZE, DynamicExportThunk, DynamicExportThunkId,
    install_dynamic_export_thunk, install_dynamic_export_thunks,
};
pub use environment::{
    CONTROLLED_ANSI_COMMAND_LINE_ENCODING, CONTROLLED_COMMAND_LINE, CONTROLLED_COMMAND_LINE_BASE,
    CONTROLLED_ENVIRONMENT_PAGE_SIZE, CONTROLLED_PEB_BASE, CONTROLLED_PROCESS_PARAMETERS_BASE,
    CONTROLLED_TEB_BASE, CONTROLLED_WIDE_ENVIRONMENT_ENCODING, InitialAnsiCommandLine,
    InitialProcessEnvironment, InitialProcessParameters, InitialWideEnvironmentStrings,
    PEB_PROCESS_PARAMETERS_POINTER_OFFSET, PROCESS_PARAMETERS_COMMAND_LINE_OFFSET,
    PROCESS_PARAMETERS_ENVIRONMENT_POINTER_OFFSET, TEB_PEB_POINTER_OFFSET, TEB_SELF_POINTER_OFFSET,
    UNICODE_STRING_BUFFER_OFFSET, install_controlled_process_environment,
    install_controlled_process_environment_with_command_line,
    install_controlled_process_environment_with_command_line_and_ansi,
    install_controlled_process_environment_with_command_line_ansi_and_environment_strings,
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
    CPU_WRITEBACK_BINARY_VERSION, CacheEffects, CpuWritebackBatch, CpuWritebackBuffers,
    CpuWritebackPage, CpuWritebackReceipt, DirtyPage, GUEST_PAGE_SIZE, GuestAddress, GuestMemory,
    GuestMemoryError, GuestMemoryErrorClass, GuestMemoryTransaction, GuestPageReconciliation,
    GuestRange, GuestWriteResult, ImageMemoryLayout, MEMORY_DELTA_BINARY_VERSION,
    MEMORY_REFLECTION_BINARY_VERSION, MappingOperation, MemoryAccess, MemoryDelta,
    MemoryDeltaBuffers, MemoryFault, MemoryFaultReason, MemoryInvalidation, MemoryLimits,
    MemoryOwnerId, MemoryOwnerKind, MemoryReflection, MemoryReflectionBuffers,
    MemoryReflectionPage, MemoryRegionKind, MemoryRegionSnapshot, MemoryStats,
    MemoryTransactionStats, PageProtection,
};
pub use module_graph::{
    RUNTIME_MODULE_GRAPH_MAX_MODULES, RUNTIME_MODULE_REFERENCE_OWNER_LIMIT, RuntimeModuleClass,
    RuntimeModuleGraph, RuntimeModuleGraphError, RuntimeModuleGraphErrorClass, RuntimeModuleId,
    RuntimeModuleLoadState, RuntimeModuleNode, RuntimeModuleReference, RuntimeModuleReferenceOwner,
    RuntimeModuleRegistration,
};
pub use module_images::{
    RUNTIME_EXPORT_FORWARDER_DEPTH_LIMIT, RUNTIME_EXPORT_LOOKUP_NAME_LIMIT,
    RUNTIME_MODULE_IMAGE_LIMIT, RuntimeExportForwarderHop, RuntimeModuleImage,
    RuntimeModuleImageError, RuntimeModuleImageErrorClass, RuntimeModuleImageRegistry,
    RuntimeResolvedExport,
};
pub use pe32::{
    DataDirectoryAddressKind, DataDirectoryPlan, ExportDirectoryPlan, ExportLookup,
    ExportSymbolPlan, ExportTarget, FeatureInventory, HeaderMappingPlan, ImageKind, ImagePlan,
    ImportModulePlan, ImportSymbolPlan, ImportTarget, LoaderError, LoaderErrorClass,
    LoaderIngressReport, MappingProtection, Pe32Limits, Pe32LoadPlan, RelocationBlockPlan,
    RelocationOperation, RelocationPlan, Rva, SectionMappingPlan, UnsupportedPoint,
    build_pe32_load_plan,
};
pub use runtime::{
    DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, DLL_PROCESS_STATIC_RESERVED, FLS_OUT_OF_INDEXES,
    LOGICAL_ENCODED_POINTER_NAMESPACE, LOGICAL_FLS_INDEX_NAMESPACE,
    LOGICAL_LOADED_MODULE_NAMESPACE, LOGICAL_PRIVATE_HEAP_NAMESPACE,
    LOGICAL_STANDARD_HANDLE_NAMESPACE, LOGICAL_TLS_INDEX_NAMESPACE,
    ProcessAddressAllocationClassification, ProcessAddressAllocationId,
    ProcessAddressAllocationLifetime, ProcessAddressAllocationPurpose,
    ProcessAddressAllocationRecord, ProcessAddressAllocationReleasePolicy,
    ProcessAddressAllocationRequest, ProcessAddressSpaceAllocator, ProcessAddressSpaceLayout,
    RUNTIME_CRITICAL_SECTION_LIMIT, RUNTIME_FLS_INDEX_LIMIT, RUNTIME_PRIVATE_HEAP_ALIGNMENT,
    RUNTIME_PRIVATE_HEAP_ARENA_BASE, RUNTIME_PRIVATE_HEAP_ARENA_STRIDE,
    RUNTIME_PRIVATE_HEAP_BACKING_LIMIT, RUNTIME_PRIVATE_HEAP_FREED_BYTE,
    RUNTIME_PRIVATE_HEAP_MAX_LIVE_ALLOCATIONS, RUNTIME_PRIVATE_HEAP_UNINITIALIZED_BYTE,
    RUNTIME_PROCESS_OBJECT_NAMESPACE, RUNTIME_STANDARD_ERROR_HANDLE,
    RUNTIME_STANDARD_ERROR_SELECTOR, RUNTIME_STANDARD_INPUT_HANDLE,
    RUNTIME_STANDARD_INPUT_SELECTOR, RUNTIME_STANDARD_OUTPUT_HANDLE,
    RUNTIME_STANDARD_OUTPUT_SELECTOR, RUNTIME_THREAD_OBJECT_NAMESPACE, RuntimeAddressRange,
    RuntimeBootstrap, RuntimeCallbackFrame, RuntimeCallbackFrameId, RuntimeCancellation,
    RuntimeClockState, RuntimeCommandLine, RuntimeCriticalSection, RuntimeDllAttachFailure,
    RuntimeDllCallbackReason, RuntimeDllCallbackRequest, RuntimeDllLifecycleAction,
    RuntimeDllLifecycleCompletion, RuntimeDllLifecyclePhase, RuntimeDynamicEndpoint,
    RuntimeEncodedPointer, RuntimeEnvironmentStringCopy, RuntimeEnvironmentStrings, RuntimeFlsSlot,
    RuntimeGuestImport, RuntimeGuestImportBinding, RuntimeGuestImportResolution, RuntimeHandle,
    RuntimeHandleClosePolicy, RuntimeHandleEntry, RuntimeHeap, RuntimeHeapAllocation,
    RuntimeHeapAllocationId, RuntimeHeapBacking, RuntimeHeapInitialization, RuntimeHeapTeardown,
    RuntimeHostCompletion, RuntimeHostRequest, RuntimeMessageId, RuntimeMessageReceiveDisposition,
    RuntimeMessageWake, RuntimeModule, RuntimeModuleDependencyEdge, RuntimeObjectId,
    RuntimeObjectKey, RuntimeObjectKind, RuntimeObjectRecord, RuntimeObjectTable,
    RuntimeProcessCancellation, RuntimeProcessEnvironment, RuntimeProcessId,
    RuntimeProcessLifecycleState, RuntimeProcessState, RuntimeRegistries, RuntimeScheduler,
    RuntimeSchedulerEvent, RuntimeStandardHandle, RuntimeStartupInfo, RuntimeState,
    RuntimeStateError, RuntimeStateErrorClass, RuntimeSystemExportEndpoint,
    RuntimeSystemExportTable, RuntimeThreadId, RuntimeThreadLifecycleState, RuntimeThreadMessage,
    RuntimeThreadState, RuntimeTlsValue, RuntimeWaitCompletion, RuntimeWaitDisposition,
    RuntimeWaitRegistration, RuntimeWaitRegistrationId, RuntimeWaitState, SchedulerThreadState,
    TLS_OUT_OF_INDEXES, X86_CRITICAL_SECTION_BYTE_LENGTH, X86_STARTUP_INFO_W_BYTE_LENGTH,
};
pub use snapshot::{ModuleIdentity, ModuleSnapshot};
pub use win32::{
    DECODE_POINTER_IMPLEMENTATION, ENCODE_POINTER_IMPLEMENTATION,
    ENTER_CRITICAL_SECTION_IMPLEMENTATION, FLS_ALLOC_IMPLEMENTATION, FLS_SET_VALUE_IMPLEMENTATION,
    FREE_ENVIRONMENT_STRINGS_W_IMPLEMENTATION, GET_COMMAND_LINE_A_IMPLEMENTATION,
    GET_CURRENT_PROCESS_ID_IMPLEMENTATION, GET_CURRENT_THREAD_ID_IMPLEMENTATION,
    GET_ENVIRONMENT_STRINGS_W_IMPLEMENTATION, GET_FILE_TYPE_IMPLEMENTATION,
    GET_MODULE_HANDLE_W_IMPLEMENTATION, GET_PROC_ADDRESS_IMPLEMENTATION,
    GET_STARTUP_INFO_W_IMPLEMENTATION, GET_STD_HANDLE_IMPLEMENTATION,
    GET_SYSTEM_TIME_AS_FILE_TIME_IMPLEMENTATION, GET_TICK_COUNT_IMPLEMENTATION,
    HEAP_ALLOC_IMPLEMENTATION, HEAP_CREATE_IMPLEMENTATION, HEAP_SET_INFORMATION_IMPLEMENTATION,
    INITIALIZE_CRITICAL_SECTION_AND_SPIN_COUNT_IMPLEMENTATION,
    KERNEL32_COMPATIBILITY_POLICY_SCHEMA, KERNEL32_COMPATIBILITY_POLICY_VERSION,
    Kernel32CompatibilityPolicy, Kernel32CompatibilityPolicyError, Kernel32HeapAllocationPolicy,
    Kernel32StandardHandleFileType, LEAVE_CRITICAL_SECTION_IMPLEMENTATION,
    QUERY_PERFORMANCE_COUNTER_IMPLEMENTATION, SET_HANDLE_COUNT_IMPLEMENTATION,
    TLS_ALLOC_IMPLEMENTATION, TLS_SET_VALUE_IMPLEMENTATION, decode_pointer_contract,
    dispatch_kernel32_api, dispatch_kernel32_api_with_policy, encode_pointer_contract,
    enter_critical_section_contract, fls_alloc_contract, fls_set_value_contract,
    free_environment_strings_w_contract, get_command_line_a_contract,
    get_current_process_id_contract, get_current_thread_id_contract,
    get_environment_strings_w_contract, get_file_type_contract, get_module_handle_w_contract,
    get_proc_address_contract, get_startup_info_w_contract, get_std_handle_contract,
    get_system_time_as_file_time_contract, get_tick_count_contract, heap_alloc_contract,
    heap_create_contract, heap_set_information_contract,
    initialize_critical_section_and_spin_count_contract, kernel32_contract_for_endpoint,
    leave_critical_section_contract, query_performance_counter_contract, set_handle_count_contract,
    tls_alloc_contract, tls_set_value_contract,
};

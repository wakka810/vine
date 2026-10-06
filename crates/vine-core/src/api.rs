use std::collections::{BTreeMap, BTreeSet};

use crate::callbacks::{
    CALLBACK_SCRATCH_ARENA_BASE, CALLBACK_SCRATCH_ARENA_SIZE, CALLBACK_SCRATCH_SLOT_SIZE,
};
use crate::imports::{ImportThunk, ImportThunkDisposition, ImportThunkId};
use crate::memory::{
    GuestAddress, GuestMemory, GuestMemoryError, GuestMemoryTransaction, GuestRange, MemoryAccess,
    MemoryOwnerKind, MemoryRegionKind, PageProtection,
};
use crate::pe32::ImportTarget;
use crate::runtime::{
    ProcessAddressAllocationClassification, ProcessAddressAllocationPurpose,
    ProcessAddressAllocationRecord, ProcessAddressAllocationReleasePolicy, Runtime,
    RuntimeApiBoundaryDisposition, RuntimeCallbackFrameId, RuntimeCallbackScratch,
    RuntimeDynamicEndpoint, RuntimeGuestImport, RuntimeGuestImportResolution,
    RuntimeSetWindowPosCallbackAction, RuntimeSetWindowPosCallbackMessage,
    RuntimeSetWindowPosDefaultCallbackAction, RuntimeSetWindowPosDefaultCallbackMessage,
    RuntimeSetWindowPosDefaultOperation, RuntimeSetWindowPosOperation, RuntimeStateError,
    RuntimeUpdateWindowOperation, RuntimeWindowCreationCallbackAction,
    RuntimeWindowCreationMessage, RuntimeWindowId,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ApiSymbol {
    Name { name: String },
    Ordinal { ordinal: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ApiEndpointBinding {
    ImportAddressTable,
    DynamicExport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiEndpointIdentity {
    pub id: ImportThunkId,
    pub address: GuestAddress,
    pub module: String,
    pub symbol: ApiSymbol,
    pub iat_address: Option<GuestAddress>,
    pub disposition: Option<ImportThunkDisposition>,
    pub binding: ApiEndpointBinding,
}

impl ApiEndpointIdentity {
    #[cold]
    pub fn diagnostic_symbol(&self) -> String {
        match &self.symbol {
            ApiSymbol::Name { name } => name.clone(),
            ApiSymbol::Ordinal { ordinal } => format!("#{ordinal}"),
        }
    }

    pub fn from_import_thunk(thunk: &ImportThunk) -> Self {
        let symbol = match &thunk.target {
            ImportTarget::Name { name, .. } => ApiSymbol::Name { name: name.clone() },
            ImportTarget::Ordinal { ordinal } => ApiSymbol::Ordinal { ordinal: *ordinal },
        };
        Self {
            id: thunk.id,
            address: thunk.address,
            module: thunk.module.clone(),
            symbol,
            iat_address: Some(thunk.iat_address),
            disposition: Some(thunk.disposition),
            binding: ApiEndpointBinding::ImportAddressTable,
        }
    }

    pub fn from_runtime_guest_import(import: &RuntimeGuestImport) -> Option<Self> {
        let RuntimeGuestImportResolution::RuntimeExport { endpoint } = &import.resolution else {
            return None;
        };
        let symbol = match &import.target {
            ImportTarget::Name { name, .. } => ApiSymbol::Name { name: name.clone() },
            ImportTarget::Ordinal { ordinal } => ApiSymbol::Ordinal { ordinal: *ordinal },
        };
        Some(Self {
            id: endpoint.id,
            address: endpoint.address,
            module: import.requested_module.clone(),
            symbol,
            iat_address: Some(import.iat_address),
            disposition: Some(ImportThunkDisposition::DeclaredRuntimeExport),
            binding: ApiEndpointBinding::ImportAddressTable,
        })
    }

    pub fn from_dynamic_endpoint(endpoint: &RuntimeDynamicEndpoint) -> Self {
        Self {
            id: ImportThunkId(endpoint.id),
            address: endpoint.address,
            module: endpoint.module.clone(),
            symbol: ApiSymbol::Name {
                name: endpoint.name.clone(),
            },
            iat_address: None,
            disposition: None,
            binding: ApiEndpointBinding::DynamicExport,
        }
    }

    fn registry_key(&self) -> (ApiEndpointBinding, ImportThunkId) {
        (self.binding, self.id)
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub struct ApiFault {
    pub message: String,
    pub endpoint: Option<String>,
}

impl ApiFault {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            endpoint: None,
        }
    }

    fn with_endpoint(mut self, endpoint: &ApiEndpointIdentity) -> Self {
        self.endpoint = Some(endpoint_label(endpoint));
        self
    }

    pub(crate) fn from_memory(error: GuestMemoryError, endpoint: &ApiEndpointIdentity) -> Self {
        Self {
            message: error.message,
            endpoint: Some(endpoint_label(endpoint)),
        }
    }

    pub(crate) fn from_runtime(error: RuntimeStateError, endpoint: &ApiEndpointIdentity) -> Self {
        Self {
            message: error.message,
            endpoint: Some(endpoint_label(endpoint)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApiOwnedTrap {
    pub endpoint_id: ImportThunkId,
    pub endpoint_address: GuestAddress,
    pub stack_pointer: GuestAddress,
    pub return_address: GuestAddress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApiCallFrame {
    pub endpoint: ApiEndpointIdentity,
    pub stack_pointer: GuestAddress,
    pub return_address: GuestAddress,
    pub arguments: Vec<u32>,
}

impl ApiCallFrame {
    pub(crate) fn argument_u32(&self, index: usize) -> Option<u32> {
        self.arguments.get(index).copied()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApiGuestWrite {
    pub address: GuestAddress,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub struct ApiImmediate {
    pub(crate) created_thread: Option<crate::runtime::RuntimeThreadId>,
    pub(crate) eax: Option<u32>,
    pub(crate) guest_writes: Vec<ApiGuestWrite>,
    pub(crate) last_error: Option<u32>,
    pub(crate) execution_boundary: RuntimeApiBoundaryDisposition,
}

impl ApiImmediate {
    pub fn new(eax: Option<u32>) -> Self {
        Self {
            eax,
            created_thread: None,
            guest_writes: Vec::new(),

            last_error: None,
            execution_boundary: RuntimeApiBoundaryDisposition::ContinueQuantum,
        }
    }

    pub(crate) fn with_guest_write(mut self, address: GuestAddress, bytes: Vec<u8>) -> Self {
        self.guest_writes.push(ApiGuestWrite { address, bytes });
        self
    }

    pub(crate) fn with_last_error(mut self, last_error: u32) -> Self {
        self.last_error = Some(last_error);
        self
    }

    pub(crate) fn with_execution_boundary(
        mut self,
        disposition: RuntimeApiBoundaryDisposition,
    ) -> Self {
        self.execution_boundary = disposition;
        self
    }
}

#[derive(Debug)]
pub struct HostRequest {
    pub request_id: u64,
    pub service: String,
    pub payload: Vec<u8>,
}

#[derive(Debug)]
pub struct HostCompletion {
    pub request_id: u64,
    pub result: ApiImmediate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GuestCallbackRequest {
    pub target: GuestAddress,
    pub arguments: Vec<GuestCallbackArgument>,
    pub scratch: Vec<u8>,
    pub scratch_pointer_relocations: Vec<GuestCallbackScratchPointerRelocation>,
    pub return_sentinel: GuestAddress,
    pub completion_policy: GuestCallbackCompletionPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GuestCallbackScratchPointerRelocation {
    pub write_offset: u32,
    pub target_offset: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GuestCallbackArgument {
    U32(u32),
    ScratchPointer { offset: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GuestCallbackCompletionPolicy {
    ReturnU32,
    ReturnBool32,
    ReturnBool32WithLastError {
        last_error: u32,
    },
    ReturnConstantU32 {
        value: u32,
    },
    CreateWindowExA {
        window_id: RuntimeWindowId,
        window_handle: u32,
        message: RuntimeWindowCreationMessage,
    },
    SetWindowPos {
        operation: RuntimeSetWindowPosOperation,
        message: RuntimeSetWindowPosCallbackMessage,
    },
    SetWindowPosDefault {
        operation: RuntimeSetWindowPosDefaultOperation,
        message: RuntimeSetWindowPosDefaultCallbackMessage,
    },
    UpdateWindow {
        operation: RuntimeUpdateWindowOperation,
    },
    ExceptionDispatch {
        next_frame: GuestAddress,
        exception_flags: u32,
    },
}

impl GuestCallbackCompletionPolicy {
    fn complete_direct(self, result: u32) -> Option<ApiImmediate> {
        let value = match self {
            Self::ReturnU32 => Some(result),
            Self::ReturnBool32 => Some(u32::from(result != 0)),
            Self::ReturnBool32WithLastError { last_error } => {
                return Some(
                    ApiImmediate::new(Some(u32::from(result != 0))).with_last_error(last_error),
                );
            }
            Self::ReturnConstantU32 { value } => Some(value),
            Self::CreateWindowExA { .. }
            | Self::SetWindowPos { .. }
            | Self::SetWindowPosDefault { .. }
            | Self::UpdateWindow { .. }
            | Self::ExceptionDispatch { .. } => return None,
        };
        Some(ApiImmediate::new(value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RuntimeGuestCallbackSemanticAction {
    WindowCreation(RuntimeWindowCreationCallbackAction),
    SetWindowPos(RuntimeSetWindowPosCallbackAction),
    SetWindowPosDefault(RuntimeSetWindowPosDefaultCallbackAction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedGuestCallback {
    pub frame_id: RuntimeCallbackFrameId,
    pub target: GuestAddress,
    pub return_sentinel: GuestAddress,
    pub allocation_start: GuestAddress,
    pub stack_pointer: GuestAddress,
    pub scratch_address: GuestAddress,
    pub scratch_bytes: u32,
    pub frame_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GuestCallbackFrameImage {
    allocation_start: GuestAddress,
    stack_pointer: GuestAddress,
    scratch_address: GuestAddress,
    scratch_bytes: u32,
    frame_bytes: u32,
    stack: Vec<u8>,
    scratch: Vec<u8>,
}

impl GuestCallbackFrameImage {
    fn prepared(&self, frame: crate::runtime::RuntimeCallbackFrame) -> PreparedGuestCallback {
        PreparedGuestCallback {
            frame_id: frame.id,
            target: frame.target,
            return_sentinel: frame.return_sentinel,
            allocation_start: self.allocation_start,
            stack_pointer: self.stack_pointer,
            scratch_address: self.scratch_address,
            scratch_bytes: self.scratch_bytes,
            frame_bytes: self.frame_bytes,
        }
    }
}

fn callback_scratch_byte_length(
    request: &GuestCallbackRequest,
    endpoint: &ApiEndpointIdentity,
) -> Result<u32, ApiFault> {
    let byte_length = u32::try_from(request.scratch.len()).map_err(|_| {
        ApiFault::new("guest callback scratch does not fit x86 accounting").with_endpoint(endpoint)
    })?;
    if byte_length > CALLBACK_SCRATCH_SLOT_SIZE {
        return Err(ApiFault::new(
            format!(
                "guest callback scratch requires {byte_length} bytes; slot capacity is {CALLBACK_SCRATCH_SLOT_SIZE}"
            ),
        )
        .with_endpoint(endpoint));
    }
    Ok(byte_length)
}

fn validate_callback_scratch_allocation(
    allocation: &ProcessAddressAllocationRecord,
    endpoint: &ApiEndpointIdentity,
) -> Result<(), ApiFault> {
    if allocation.purpose != ProcessAddressAllocationPurpose::CallbackScratch
        || allocation.range.base != CALLBACK_SCRATCH_ARENA_BASE
        || allocation.range.byte_length != CALLBACK_SCRATCH_ARENA_SIZE
        || allocation.usable_byte_length != CALLBACK_SCRATCH_ARENA_SIZE
        || allocation.classification != ProcessAddressAllocationClassification::RuntimeControl
        || allocation.release_policy != ProcessAddressAllocationReleasePolicy::Permanent
        || allocation.executable
    {
        return Err(ApiFault::new(
            "callback scratch address allocation does not match the permanent runtime arena",
        )
        .with_endpoint(endpoint));
    }
    Ok(())
}

fn callback_scratch_slot_range(
    scratch: RuntimeCallbackScratch,
    endpoint: &ApiEndpointIdentity,
) -> Result<GuestRange, ApiFault> {
    if scratch.byte_length == 0 || scratch.byte_length > scratch.capacity {
        return Err(
            ApiFault::new("live callback scratch has an invalid byte length")
                .with_endpoint(endpoint),
        );
    }
    let expected_offset = u32::from(scratch.slot)
        .checked_mul(CALLBACK_SCRATCH_SLOT_SIZE)
        .ok_or_else(|| {
            ApiFault::new("callback scratch slot offset overflowed x86 accounting")
                .with_endpoint(endpoint)
        })?;
    let expected_address = CALLBACK_SCRATCH_ARENA_BASE
        .0
        .checked_add(expected_offset)
        .map(GuestAddress)
        .ok_or_else(|| {
            ApiFault::new("callback scratch slot address overflowed x86 accounting")
                .with_endpoint(endpoint)
        })?;
    if scratch.address != expected_address || scratch.capacity != CALLBACK_SCRATCH_SLOT_SIZE {
        return Err(
            ApiFault::new("callback scratch slot does not match its arena layout")
                .with_endpoint(endpoint),
        );
    }
    let range = GuestRange::new(scratch.address, u64::from(scratch.capacity))
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let arena_end = u64::from(CALLBACK_SCRATCH_ARENA_BASE.0)
        .checked_add(CALLBACK_SCRATCH_ARENA_SIZE)
        .ok_or_else(|| {
            ApiFault::new("callback scratch arena end overflowed x86 accounting")
                .with_endpoint(endpoint)
        })?;
    if range.end() > arena_end {
        return Err(
            ApiFault::new("callback scratch slot extends outside the permanent arena")
                .with_endpoint(endpoint),
        );
    }
    Ok(range)
}

fn callback_scratch_region(
    memory: &GuestMemory,
    allocation: &ProcessAddressAllocationRecord,
    endpoint: &ApiEndpointIdentity,
) -> Result<Option<crate::memory::MemoryRegionInfo>, ApiFault> {
    let mut regions = memory
        .regions()
        .filter(|region| region.address_allocation_id == Some(allocation.id.0));
    let region = regions.next();
    if regions.next().is_some() {
        return Err(ApiFault::new(
            "callback scratch address allocation owns more than one guest-memory region",
        )
        .with_endpoint(endpoint));
    }
    if let Some(region) = region {
        let expected_range =
            GuestRange::new(CALLBACK_SCRATCH_ARENA_BASE, CALLBACK_SCRATCH_ARENA_SIZE)
                .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        if region.kind != MemoryRegionKind::Runtime || region.range != expected_range {
            return Err(ApiFault::new(
                "callback scratch guest-memory region does not match its address allocation",
            )
            .with_endpoint(endpoint));
        }
        Ok(Some(region))
    } else {
        Ok(None)
    }
}

fn preflight_callback_scratch_mapping(
    memory: &GuestMemory,
    allocation: &ProcessAddressAllocationRecord,
    scratch: Option<RuntimeCallbackScratch>,
    endpoint: &ApiEndpointIdentity,
) -> Result<(), ApiFault> {
    let Some(scratch) = scratch else {
        return Ok(());
    };
    validate_callback_scratch_allocation(allocation, endpoint)?;
    let slot_range = callback_scratch_slot_range(scratch, endpoint)?;
    let region = callback_scratch_region(memory, allocation, endpoint)?;
    let additional_owners = usize::from(region.is_none());
    let additional_regions = additional_owners;
    let additional_pages = match memory
        .committed_page(slot_range.start())
        .map_err(|error| ApiFault::from_memory(error, endpoint))?
    {
        None => 1,
        Some((owner, protection)) => {
            let region = region.as_ref().ok_or_else(|| {
                ApiFault::new("callback scratch page is committed without its permanent region")
                    .with_endpoint(endpoint)
            })?;
            if owner != region.owner
                || protection
                    != (PageProtection {
                        read: true,
                        write: true,
                        execute: false,
                    })
            {
                return Err(ApiFault::new(
                    "callback scratch page owner or protection does not match its region",
                )
                .with_endpoint(endpoint));
            }
            0
        }
    };
    memory
        .preflight_growth(additional_owners, additional_regions, additional_pages)
        .map_err(|error| ApiFault::from_memory(error, endpoint))
}

fn materialize_callback_scratch_mapping(
    memory: &mut GuestMemoryTransaction<'_>,
    allocation: &ProcessAddressAllocationRecord,
    scratch: Option<RuntimeCallbackScratch>,
    endpoint: &ApiEndpointIdentity,
) -> Result<(), ApiFault> {
    let Some(scratch) = scratch else {
        return Ok(());
    };
    let slot_range = callback_scratch_slot_range(scratch, endpoint)?;
    let region = match callback_scratch_region(memory, allocation, endpoint)? {
        Some(region) => region,
        None => {
            let owner = memory
                .create_owner(MemoryOwnerKind::Runtime)
                .map_err(|error| ApiFault::from_memory(error, endpoint))?;
            let range = memory
                .reserve_fixed_for_address_allocation(
                    allocation.id.0,
                    owner,
                    MemoryRegionKind::Runtime,
                    allocation.range.base,
                    allocation.range.byte_length,
                )
                .map_err(|error| ApiFault::from_memory(error, endpoint))?;
            crate::memory::MemoryRegionInfo {
                address_allocation_id: Some(allocation.id.0),
                owner,
                kind: MemoryRegionKind::Runtime,
                range,
            }
        }
    };
    match memory
        .committed_page(slot_range.start())
        .map_err(|error| ApiFault::from_memory(error, endpoint))?
    {
        None => {
            memory
                .commit(
                    region.owner,
                    slot_range,
                    PageProtection {
                        read: true,
                        write: true,
                        execute: false,
                    },
                )
                .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        }
        Some((owner, protection))
            if owner == region.owner
                && protection
                    == (PageProtection {
                        read: true,
                        write: true,
                        execute: false,
                    }) => {}
        Some(_) => {
            return Err(
                ApiFault::new("callback scratch page changed after mapping preflight")
                    .with_endpoint(endpoint),
            );
        }
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) enum ApiHandlerResult {
    Immediate(ApiImmediate),
    Deferred(HostRequest),
    GuestCallback(Box<GuestCallbackRequest>),
    Fault(ApiFault),
}

#[derive(Debug)]
pub struct ApiImmediateCompletion {
    pub created_thread: Option<crate::runtime::RuntimeThreadId>,
    pub instruction_pointer: GuestAddress,
    pub stack_pointer: GuestAddress,
    pub eax: Option<u32>,
    pub execution_boundary: RuntimeApiBoundaryDisposition,
}

#[derive(Debug)]
pub struct ApiDeferredCall {
    pub request: HostRequest,
    pub(crate) frame: ApiCallFrame,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiGuestCallbackCall {
    pub(crate) request: Box<GuestCallbackRequest>,
    pub(crate) frame: ApiCallFrame,
}

#[derive(Debug)]
pub enum GuestCallbackReturn {
    Completed(ApiImmediateCompletion),
    Com(crate::com::ComDispatchOutcome),
    Continue(PreparedGuestCallback),
    DllCallback {
        target: GuestAddress,
        stack_pointer: GuestAddress,
    },
    DllAttached,
}

#[derive(Debug)]
pub enum DispatchOutcome {
    Immediate(ApiImmediateCompletion),
    Deferred(ApiDeferredCall),
    GuestCallback(ApiGuestCallbackCall),
    Fault(ApiFault),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86CpuContext {
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
    pub ebx: u32,
    pub esp: u32,
    pub ebp: u32,
    pub esi: u32,
    pub edi: u32,
    pub eip: u32,
    pub eflags: u32,
    pub es: u32,
    pub cs: u32,
    pub ss: u32,
    pub ds: u32,
    pub fs: u32,
    pub gs: u32,
}

pub(crate) fn commit_indirect_guest_writes(
    memory: &mut GuestMemory,
    writes: &[ApiGuestWrite],
) -> Result<(), crate::memory::GuestMemoryError> {
    let mut transaction = memory.begin_transaction();
    for write in writes {
        transaction.write(write.address, &write.bytes)?;
    }
    transaction.commit_transaction();
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub enum ApiSourceLookup {
    File {
        object_id: u32,
        relative_path_a: Vec<u8>,
    },
    Search {
        pattern: Vec<u8>,
        parent: Vec<u8>,
    },
}

impl Runtime {
    pub fn prepare_api_source_lookup(
        &self,
        route: &ApiEndpointIdentity,
        stack: GuestAddress,
    ) -> Result<Option<ApiSourceLookup>, ApiFault> {
        if !route.module.eq_ignore_ascii_case("kernel32.dll") {
            return Ok(None);
        }
        let ApiSymbol::Name { name } = &route.symbol else {
            return Ok(None);
        };
        let source_file = name == "CreateFileA";
        let module_load = name == "LoadLibraryA";
        let file_attributes = name == "GetFileAttributesA";
        let exact_file_search = name == "FindFirstFileA";
        let directory_create = name == "CreateDirectoryA";
        let file_delete = name == "DeleteFileA";
        if !source_file
            && !module_load
            && !file_attributes
            && !exact_file_search
            && !directory_create
            && !file_delete
        {
            return Ok(None);
        }
        let argument = |index: u32| -> Result<u32, ApiFault> {
            let address = stack
                .0
                .checked_add(4 + index * 4)
                .ok_or_else(|| ApiFault::new("source lookup call frame overflowed x86"))?;
            let bytes = self
                .memory
                .read(GuestAddress(address), 4)
                .map_err(|error| ApiFault::from_memory(error, route))?;
            Ok(u32::from_le_bytes(
                bytes.try_into().expect("read one DWORD"),
            ))
        };
        if (source_file
            && (!matches!(argument(1)?, 0x8000_0000 | 0x4000_0000 | 0xc000_0000)
                || !matches!(argument(4)?, 2..=4)))
            || argument(0)? == 0
        {
            return Ok(None);
        }
        let path_start = argument(0)?;
        let mut path = Vec::new();
        for offset in 0..260_u32 {
            let address = path_start
                .checked_add(offset)
                .ok_or_else(|| ApiFault::new("source path overflowed x86"))?;
            let byte = self
                .memory
                .read(GuestAddress(address), 1)
                .map_err(|error| ApiFault::from_memory(error, route))?[0];
            path.push(byte);
            if byte == 0 {
                break;
            }
        }
        if path.last() != Some(&0) {
            return Err(ApiFault::new(
                "source path is not NUL-terminated within the DOS path limit",
            ));
        }
        if exact_file_search
            && path[..path.len() - 1]
                .iter()
                .any(|byte| matches!(byte, b'*' | b'?'))
        {
            // Malformed/out-of-mount patterns belong to the Windows handler.
            if let Ok(Some(directory)) = self.source_search_directory(&path) {
                return Ok(Some(ApiSourceLookup::Search {
                    pattern: path,
                    parent: directory,
                }));
            }
            return Ok(None);
        }
        // Leave unsupported module search/encoding variants to the Windows handler.
        // Only a DLL basename can safely be preflighted against this selected directory.
        if module_load
            && (!path[..path.len() - 1].is_ascii()
                || !path[..path.len() - 1]
                    .iter()
                    .all(|byte| byte.is_ascii_graphic() && !matches!(byte, b'/' | b'\\' | b':'))
                || !path[..path.len() - 1]
                    .to_ascii_lowercase()
                    .ends_with(b".dll"))
        {
            return Ok(None);
        }
        // The browser probes only paths under the selected mount. Invalid or
        // out-of-mount paths must reach Rust's Windows handler for LastError.
        if (file_attributes || exact_file_search || directory_create || file_delete)
            && matches!(
                self.resolve_file_system_path_a(&path),
                Ok(crate::RuntimePathResolution::Directory
                    | crate::RuntimePathResolution::RegularFile)
            )
        {
            return Ok(None);
        }
        let relative = self.source_lookup_relative_path(&path);
        // Out-of-mount paths are answered by the Windows handler, not sent to
        // the selected browser directory as a source lookup.
        let Some(relative) =
            (if file_attributes || exact_file_search || directory_create || file_delete {
                relative.ok().flatten()
            } else {
                relative.map_err(|error| ApiFault::from_runtime(error, route))?
            })
        else {
            return Ok(None);
        };
        let object_id = self
            .next_source_file_object_id()
            .map_err(|error| ApiFault::from_runtime(error, route))?;
        Ok(Some(ApiSourceLookup::File {
            object_id,
            relative_path_a: relative,
        }))
    }
}

impl Runtime {
    pub fn complete_guest_callback(
        &mut self,
        dispatcher: &ApiDispatcher,
        returned_sentinel: GuestAddress,
        result: u32,
    ) -> Result<GuestCallbackReturn, ApiFault> {
        let frame = self
            .current_thread_state()
            .map_err(|error| ApiFault::new(error.message))?
            .callback_frames
            .last()
            .copied()
            .ok_or_else(|| ApiFault::new("callback return has no active Rust frame"))?;
        if self.pending_api_callbacks.contains_key(&frame.id) {
            return self.complete_api_callback(dispatcher, returned_sentinel, result);
        }
        if self.direct_show.callbacks.contains_key(&frame.id) {
            return self
                .complete_directshow_callback(returned_sentinel, result)
                .map(GuestCallbackReturn::Com)
                .map_err(|error| ApiFault::new(error.message));
        }
        if !matches!(
            self.dll_lifecycle_phase(),
            crate::runtime::RuntimeDllLifecyclePhase::ProcessAttaching
                | crate::runtime::RuntimeDllLifecyclePhase::AttachUnwinding
        ) {
            return Err(ApiFault::new(
                "callback return has no pending API call or DLL process attach",
            ));
        }
        match self
            .complete_dll_callback(returned_sentinel, result)
            .map_err(|error| ApiFault::new(error.message))?
        {
            crate::runtime::RuntimeDllLifecycleAction::Callback {
                target,
                stack_pointer,
            } => Ok(GuestCallbackReturn::DllCallback {
                target,
                stack_pointer,
            }),
            crate::runtime::RuntimeDllLifecycleAction::Complete { phase } => {
                debug_assert_eq!(
                    phase,
                    crate::runtime::RuntimeDllLifecyclePhase::ProcessAttached
                );
                Ok(GuestCallbackReturn::DllAttached)
            }
            crate::runtime::RuntimeDllLifecycleAction::AttachFailed { failed_module } => Err(
                ApiFault::new(format!("DLL process attach failed: {}", failed_module)),
            ),
        }
    }

    fn complete_api_callback(
        &mut self,
        dispatcher: &ApiDispatcher,
        returned_sentinel: GuestAddress,
        result: u32,
    ) -> Result<GuestCallbackReturn, ApiFault> {
        let expected = self
            .thread_state(self.current_thread_id())
            .map_err(|error| ApiFault::new(error.message))?
            .callback_frames
            .last()
            .map(|frame| frame.id)
            .ok_or_else(|| ApiFault::new("callback return has no active Rust frame"))?;
        let call = self
            .pending_api_callbacks
            .get(&expected)
            .cloned()
            .ok_or_else(|| ApiFault::new("active callback frame has no pending API call"))?;
        let endpoint = call.frame.endpoint.clone();
        self.validate_callback_frame_return(expected)
            .map_err(|error| ApiFault::from_runtime(error, &endpoint))?;
        let active = self
            .thread_state(self.current_thread_id())
            .map_err(|error| ApiFault::from_runtime(error, &endpoint))?
            .callback_frames
            .last()
            .expect("validated active callback frame");
        if active.target != call.request.target
            || active.return_sentinel != returned_sentinel
            || active.return_sentinel != call.request.return_sentinel
            || active.scratch.map_or(0, |scratch| scratch.byte_length)
                != callback_scratch_byte_length(&call.request, &call.frame.endpoint)?
        {
            return Err(ApiFault::new(
                "callback return does not identify the active target, sentinel, and scratch",
            )
            .with_endpoint(&endpoint));
        }
        let policy = call.request.completion_policy;
        match dispatcher.complete_guest_callback_result(call, result, self) {
            DispatchOutcome::Immediate(completion) => {
                self.pop_callback_frame(expected)
                    .map_err(|error| ApiFault::from_runtime(error, &endpoint))?;
                self.commit_guest_callback_semantics(policy, result, &endpoint)?;
                Ok(GuestCallbackReturn::Completed(completion))
            }
            DispatchOutcome::GuestCallback(next) => {
                let prepared = self.replace_guest_callback(next, expected)?;
                self.commit_guest_callback_semantics(policy, result, &endpoint)?;
                Ok(GuestCallbackReturn::Continue(prepared))
            }
            DispatchOutcome::Fault(fault) => Err(fault),
            DispatchOutcome::Deferred(_) => Err(ApiFault::new(
                "callback completion cannot defer the outer API",
            )
            .with_endpoint(&endpoint)),
        }
    }

    pub fn begin_guest_callback(
        &mut self,
        call: ApiGuestCallbackCall,
    ) -> Result<PreparedGuestCallback, ApiFault> {
        let prepared = self.install_guest_callback_frame(&call, None)?;
        self.pending_api_callbacks.insert(prepared.frame_id, call);
        Ok(prepared)
    }

    pub(crate) fn replace_guest_callback(
        &mut self,
        call: ApiGuestCallbackCall,
        expected: RuntimeCallbackFrameId,
    ) -> Result<PreparedGuestCallback, ApiFault> {
        let prepared = self.install_guest_callback_frame(&call, Some(expected))?;
        self.pending_api_callbacks.insert(prepared.frame_id, call);
        Ok(prepared)
    }

    fn install_guest_callback_frame(
        &mut self,
        call: &ApiGuestCallbackCall,
        expected: Option<RuntimeCallbackFrameId>,
    ) -> Result<PreparedGuestCallback, ApiFault> {
        let endpoint = &call.frame.endpoint;
        let scratch_bytes = callback_scratch_byte_length(&call.request, &call.frame.endpoint)?;
        let runtime_frame = self
            .prepare_callback_frame(
                expected,
                call.request.target,
                call.request.return_sentinel,
                scratch_bytes,
            )
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        if self.pending_api_callbacks.contains_key(&runtime_frame.id) {
            return Err(
                ApiFault::new("new callback frame ID already has a pending API call")
                    .with_endpoint(endpoint),
            );
        }
        let image = prepare_guest_callback_frame(
            &self.memory,
            &call.frame,
            &call.request,
            runtime_frame.scratch,
        )?;
        let scratch_allocation = self
            .address_space_allocator
            .allocation_for_purpose(
                &crate::runtime::ProcessAddressAllocationPurpose::CallbackScratch,
            )
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        preflight_callback_scratch_mapping(
            &self.memory,
            scratch_allocation,
            runtime_frame.scratch,
            endpoint,
        )?;
        let mut transaction = self.memory.begin_transaction();
        materialize_callback_scratch_mapping(
            &mut transaction,
            scratch_allocation,
            runtime_frame.scratch,
            endpoint,
        )?;
        if !image.scratch.is_empty() {
            transaction
                .write(image.scratch_address, &image.scratch)
                .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        }
        transaction
            .write(image.allocation_start, &image.stack)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        transaction.commit_transaction();
        self.publish_callback_frame(runtime_frame, expected.is_some());
        Ok(image.prepared(runtime_frame))
    }

    pub(crate) fn preflight_guest_callback(
        &self,
        frame: &ApiCallFrame,
        request: &GuestCallbackRequest,
    ) -> Result<(), ApiFault> {
        let endpoint = &frame.endpoint;
        let scratch_bytes = callback_scratch_byte_length(request, &frame.endpoint)?;
        let runtime_frame = self
            .prepare_callback_frame(None, request.target, request.return_sentinel, scratch_bytes)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        let (_, scratch_address, _, _) =
            callback_frame_addresses(&self.memory, frame, request, runtime_frame.scratch)?;
        for argument in &request.arguments {
            callback_argument_value(argument, request, scratch_address, endpoint)?;
        }
        for relocation in &request.scratch_pointer_relocations {
            callback_relocation_value(relocation, request, scratch_address, endpoint)?;
        }
        let scratch_allocation = self
            .callback_scratch_allocation()
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        preflight_callback_scratch_mapping(
            &self.memory,
            scratch_allocation,
            runtime_frame.scratch,
            endpoint,
        )
    }

    pub(crate) fn commit_guest_callback_semantics(
        &mut self,
        completion_policy: GuestCallbackCompletionPolicy,
        result: u32,
        endpoint: &ApiEndpointIdentity,
    ) -> Result<Option<RuntimeGuestCallbackSemanticAction>, ApiFault> {
        match completion_policy {
            GuestCallbackCompletionPolicy::ReturnU32
            | GuestCallbackCompletionPolicy::ReturnBool32
            | GuestCallbackCompletionPolicy::ReturnBool32WithLastError { .. }
            | GuestCallbackCompletionPolicy::ReturnConstantU32 { .. }
            | GuestCallbackCompletionPolicy::ExceptionDispatch { .. } => Ok(None),
            GuestCallbackCompletionPolicy::CreateWindowExA {
                window_id,
                window_handle,
                message,
            } => {
                let window = self
                    .window_by_id(window_id)
                    .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
                if window.id != window_id || window.handle != window_handle {
                    return Err(ApiFault::new(
                        "callback semantics do not match the pending logical HWND identity",
                    )
                    .with_endpoint(endpoint));
                }
                let action = self
                    .complete_window_creation_callback(window_id, message, result)
                    .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
                let committed_handle = match &action {
                    RuntimeWindowCreationCallbackAction::Continue { window, .. }
                    | RuntimeWindowCreationCallbackAction::Created { window }
                    | RuntimeWindowCreationCallbackAction::Aborted { window } => window.handle,
                };
                debug_assert_eq!(committed_handle, window_handle);
                Ok(Some(RuntimeGuestCallbackSemanticAction::WindowCreation(
                    action,
                )))
            }
            GuestCallbackCompletionPolicy::SetWindowPos { operation, message } => {
                let action = self
                    .complete_set_window_pos_callback(operation, message, result)
                    .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
                Ok(Some(RuntimeGuestCallbackSemanticAction::SetWindowPos(
                    action,
                )))
            }
            GuestCallbackCompletionPolicy::SetWindowPosDefault { operation, message } => {
                let action = self
                    .complete_set_window_pos_default_callback(operation, message, result)
                    .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
                Ok(Some(
                    RuntimeGuestCallbackSemanticAction::SetWindowPosDefault(action),
                ))
            }
            GuestCallbackCompletionPolicy::UpdateWindow { operation } => {
                self.validate_update_window_callback(operation)
                    .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
                Ok(None)
            }
        }
    }
}

fn callback_frame_addresses(
    memory: &GuestMemory,
    frame: &ApiCallFrame,
    request: &GuestCallbackRequest,
    runtime_scratch: Option<RuntimeCallbackScratch>,
) -> Result<(GuestAddress, GuestAddress, u32, u32), ApiFault> {
    const MAX_CALLBACK_ARGUMENTS: usize = 64;
    const MAX_CALLBACK_SCRATCH_POINTER_RELOCATIONS: usize = 64;

    let endpoint = &frame.endpoint;
    if request.arguments.len() > MAX_CALLBACK_ARGUMENTS
        || request.scratch_pointer_relocations.len() > MAX_CALLBACK_SCRATCH_POINTER_RELOCATIONS
    {
        return Err(ApiFault::new(
                "guest callback arguments, scratch bytes, or relocations exceed the bounded callback frame limit",
            )
            .with_endpoint(endpoint));
    }
    let scratch_bytes_u32 = callback_scratch_byte_length(request, &frame.endpoint)?;
    let scratch_address = match (scratch_bytes_u32, runtime_scratch) {
        (0, None) => GuestAddress(0),
        (0, Some(_)) => {
            return Err(ApiFault::new(
                "callback without scratch bytes received a live runtime scratch slot",
            )
            .with_endpoint(endpoint));
        }
        (byte_length, Some(scratch))
            if scratch.byte_length == byte_length
                && scratch.capacity == CALLBACK_SCRATCH_SLOT_SIZE =>
        {
            scratch.address
        }
        (_, Some(_)) => {
            return Err(ApiFault::new(
                "runtime callback scratch slot does not match the requested byte length",
            )
            .with_endpoint(endpoint));
        }
        (_, None) => {
            return Err(
                ApiFault::new("callback scratch bytes have no runtime-owned scratch slot")
                    .with_endpoint(endpoint),
            );
        }
    };
    let argument_frame_bytes = request
        .arguments
        .len()
        .checked_add(1)
        .and_then(|words| words.checked_mul(4))
        .ok_or_else(|| {
            ApiFault::new("guest callback argument frame overflowed host accounting")
                .with_endpoint(endpoint)
        })?;
    let argument_frame_bytes_u32 = u32::try_from(argument_frame_bytes).map_err(|_| {
        ApiFault::new("guest callback frame does not fit x86 address accounting")
            .with_endpoint(endpoint)
    })?;
    let allocation_start = GuestAddress(
        frame
            .stack_pointer
            .0
            .checked_sub(argument_frame_bytes_u32)
            .ok_or_else(|| {
                ApiFault::new("guest callback frame crosses the start of the x86 address space")
                    .with_endpoint(endpoint)
            })?,
    );

    memory
        .validate_access(request.target, 1, MemoryAccess::Execute)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    memory
        .validate_access(request.return_sentinel, 1, MemoryAccess::Execute)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    memory
        .validate_access(allocation_start, argument_frame_bytes, MemoryAccess::Write)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;

    Ok((
        allocation_start,
        scratch_address,
        argument_frame_bytes_u32,
        scratch_bytes_u32,
    ))
}

fn callback_argument_value(
    argument: &GuestCallbackArgument,
    request: &GuestCallbackRequest,
    scratch_address: GuestAddress,
    endpoint: &ApiEndpointIdentity,
) -> Result<u32, ApiFault> {
    Ok(match *argument {
        GuestCallbackArgument::U32(value) => value,
        GuestCallbackArgument::ScratchPointer { offset } => {
            let offset_usize = usize::try_from(offset).map_err(|_| {
                ApiFault::new("guest callback scratch offset does not fit host accounting")
                    .with_endpoint(endpoint)
            })?;
            if offset % 4 != 0 || offset_usize >= request.scratch.len() {
                return Err(ApiFault::new(
                            "guest callback scratch pointer must be aligned and identify live scratch bytes",
                        )
                        .with_endpoint(endpoint));
            }
            scratch_address.0.checked_add(offset).ok_or_else(|| {
                ApiFault::new("guest callback scratch pointer overflowed x86 accounting")
                    .with_endpoint(endpoint)
            })?
        }
    })
}

fn callback_relocation_value(
    relocation: &GuestCallbackScratchPointerRelocation,
    request: &GuestCallbackRequest,
    scratch_address: GuestAddress,
    endpoint: &ApiEndpointIdentity,
) -> Result<(usize, u32), ApiFault> {
    let write_offset = usize::try_from(relocation.write_offset).map_err(|_| {
        ApiFault::new("guest callback scratch relocation does not fit host accounting")
            .with_endpoint(endpoint)
    })?;
    let target_offset = usize::try_from(relocation.target_offset).map_err(|_| {
        ApiFault::new("guest callback scratch relocation target does not fit host accounting")
            .with_endpoint(endpoint)
    })?;
    if !relocation.write_offset.is_multiple_of(4)
        || write_offset
            .checked_add(4)
            .is_none_or(|end| end > request.scratch.len())
        || target_offset >= request.scratch.len()
    {
        return Err(ApiFault::new(
            "guest callback scratch relocation must write an aligned pointer to live scratch bytes",
        )
        .with_endpoint(endpoint));
    }
    let target = scratch_address
        .0
        .checked_add(relocation.target_offset)
        .ok_or_else(|| {
            ApiFault::new("guest callback scratch relocation overflowed x86 accounting")
                .with_endpoint(endpoint)
        })?;
    Ok((write_offset, target))
}

fn prepare_guest_callback_frame(
    memory: &GuestMemory,
    frame: &ApiCallFrame,
    request: &GuestCallbackRequest,
    runtime_scratch: Option<RuntimeCallbackScratch>,
) -> Result<GuestCallbackFrameImage, ApiFault> {
    let (allocation_start, scratch_address, frame_bytes, scratch_bytes) =
        callback_frame_addresses(memory, frame, request, runtime_scratch)?;
    let mut stack = Vec::with_capacity(frame_bytes as usize);
    let mut scratch = request.scratch.clone();
    stack.extend_from_slice(&request.return_sentinel.0.to_le_bytes());
    for argument in &request.arguments {
        let value = callback_argument_value(argument, request, scratch_address, &frame.endpoint)?;
        stack.extend_from_slice(&value.to_le_bytes());
    }
    for relocation in &request.scratch_pointer_relocations {
        let (offset, target) =
            callback_relocation_value(relocation, request, scratch_address, &frame.endpoint)?;
        scratch[offset..offset + 4].copy_from_slice(&target.to_le_bytes());
    }
    Ok(GuestCallbackFrameImage {
        allocation_start,
        stack_pointer: allocation_start,
        scratch_address,
        scratch_bytes,
        frame_bytes,
        stack,
        scratch,
    })
}

impl Runtime {
    pub fn dispatch_api_call(
        &mut self,
        dispatcher: &ApiDispatcher,
        trap: ApiOwnedTrap,
        callback_sentinel: GuestAddress,
        cpu: Option<X86CpuContext>,
    ) -> DispatchOutcome {
        let exception_unwind_frame = if let Some(endpoint) = dispatcher
            .endpoint(trap.endpoint_address)
            .filter(|endpoint| {
                endpoint.module.eq_ignore_ascii_case("kernel32.dll")
                    && matches!(&endpoint.symbol, ApiSymbol::Name { name } if name == "RtlUnwind")
            }) {
            let thread = match self.thread_state(self.current_thread_id()) {
                Ok(thread) => thread,
                Err(error) => {
                    return DispatchOutcome::Fault(ApiFault::from_runtime(error, endpoint));
                }
            };
            let active = thread
                .callback_frames
                .last()
                .filter(|frame| {
                    self.pending_api_callbacks
                        .get(&frame.id)
                        .is_some_and(|call| {
                            matches!(
                                call.request.completion_policy,
                                GuestCallbackCompletionPolicy::ExceptionDispatch { .. }
                            )
                        })
                })
                .map(|frame| frame.id);
            if let Some(frame_id) = active
                && let Err(error) = self.validate_callback_frame_return(frame_id)
            {
                return DispatchOutcome::Fault(ApiFault::from_runtime(error, endpoint));
            }
            active
        } else {
            None
        };
        let outcome = dispatcher.dispatch_owned(
            trap,
            self,
            cpu.map(|context| (context, callback_sentinel)),
            |frame, runtime, cpu| match frame.endpoint.module.to_ascii_lowercase().as_str() {
                "kernel32.dll" => crate::win32::dispatch_kernel32_api(frame, runtime, cpu),
                "user32.dll" => {
                    crate::user32::dispatch_user32_api(callback_sentinel, frame, runtime)
                }
                "gdi32.dll" => crate::gdi32::dispatch_gdi32_api(callback_sentinel, frame, runtime),
                "gdiplus.dll" => crate::gdiplus::dispatch_gdiplus_api(frame, runtime),
                "imm32.dll" => crate::imm32::dispatch_imm32_api(frame, runtime),
                "ole32.dll" => crate::ole32::dispatch_ole32_api(frame, runtime),
                "shell32.dll" => crate::shell32::dispatch_shell32_api(frame, runtime),
                "winmm.dll" => crate::winmm::dispatch_winmm_api(frame, runtime),
                "d3d9.dll" => crate::d3d9::dispatch_d3d9_api(frame, runtime),
                "dinput8.dll" => crate::dinput8::dispatch_dinput8_api(frame, runtime),
                "dsound.dll" => crate::dsound::dispatch_dsound_api(frame, runtime),
                _ => ApiHandlerResult::Fault(ApiFault::new("Windows module is not implemented")),
            },
        );
        if matches!(outcome, DispatchOutcome::Immediate(_))
            && let Some(frame_id) = exception_unwind_frame
            && let Err(error) = self.pop_callback_frame(frame_id)
        {
            return DispatchOutcome::Fault(ApiFault::new(error.message));
        }
        outcome
    }
}

#[derive(Debug)]
pub struct ApiDispatcher {
    endpoints_by_address: BTreeMap<GuestAddress, (ApiEndpointIdentity, Option<u8>)>,
}

impl ApiDispatcher {
    pub fn from_endpoints(
        endpoints: impl IntoIterator<Item = ApiEndpointIdentity>,
    ) -> Result<Self, ApiFault> {
        let mut endpoints_by_address = BTreeMap::new();
        let mut seen_ids = BTreeSet::new();
        for endpoint in endpoints {
            if endpoints_by_address.contains_key(&endpoint.address)
                || !seen_ids.insert(endpoint.registry_key())
            {
                return Err(ApiFault::new(
                    "API endpoint registry contains a duplicate binding/ID or address",
                ));
            }
            let argument_words = match endpoint.module.to_ascii_lowercase().as_str() {
                "kernel32.dll" => crate::win32::kernel32_argument_words(&endpoint),
                "user32.dll" => crate::user32::user32_argument_words(&endpoint),
                "gdi32.dll" => crate::gdi32::gdi32_argument_words(&endpoint),
                "gdiplus.dll" => crate::gdiplus::gdiplus_argument_words(&endpoint),
                "imm32.dll" => crate::imm32::imm32_argument_words(&endpoint),
                "ole32.dll" => crate::ole32::ole32_argument_words(&endpoint),
                "shell32.dll" => crate::shell32::shell32_argument_words(&endpoint),
                "winmm.dll" => crate::winmm::winmm_argument_words(&endpoint),
                "d3d9.dll" => crate::d3d9::d3d9_argument_words(&endpoint),
                "dinput8.dll" => crate::dinput8::dinput8_argument_words(&endpoint),
                "dsound.dll" => crate::dsound::dsound_argument_words(&endpoint),
                _ => None,
            };
            if argument_words.is_some() {
                validate_implemented_endpoint(&endpoint)?;
            }
            endpoints_by_address.insert(endpoint.address, (endpoint, argument_words));
        }
        Ok(Self {
            endpoints_by_address,
        })
    }

    #[cfg(test)]
    pub(crate) fn with_argument_words(
        entries: impl IntoIterator<Item = (ApiEndpointIdentity, u8)>,
    ) -> Result<Self, ApiFault> {
        let entries = entries.into_iter().collect::<Vec<_>>();
        let mut dispatcher =
            Self::from_endpoints(entries.iter().map(|(endpoint, _)| endpoint.clone()))?;
        for (endpoint, argument_words) in entries {
            validate_implemented_endpoint(&endpoint)?;
            dispatcher
                .endpoints_by_address
                .get_mut(&endpoint.address)
                .expect("registered fixture endpoint")
                .1 = Some(argument_words);
        }
        Ok(dispatcher)
    }

    pub fn endpoint(&self, address: GuestAddress) -> Option<&ApiEndpointIdentity> {
        self.endpoints_by_address
            .get(&address)
            .map(|(endpoint, _)| endpoint)
    }

    pub(crate) fn dispatch_owned<F>(
        &self,
        trap: ApiOwnedTrap,
        runtime: &mut Runtime,
        cpu: Option<(X86CpuContext, GuestAddress)>,
        handler: F,
    ) -> DispatchOutcome
    where
        F: FnOnce(
            &ApiCallFrame,
            &mut Runtime,
            Option<(X86CpuContext, GuestAddress)>,
        ) -> ApiHandlerResult,
    {
        let Some((endpoint, argument_words)) =
            self.endpoints_by_address.get(&trap.endpoint_address)
        else {
            return DispatchOutcome::Fault(ApiFault::new(format!(
                "trap address {:#010x} is not an owned API endpoint",
                trap.endpoint_address.0
            )));
        };
        if endpoint.id != trap.endpoint_id {
            return DispatchOutcome::Fault(
                ApiFault::new(
                    "trap endpoint ID and address do not identify the same owned endpoint",
                )
                .with_endpoint(endpoint),
            );
        }
        let Some(argument_words) = *argument_words else {
            return DispatchOutcome::Fault(unsupported_endpoint(endpoint));
        };
        let frame = match validate_bound_iat(&runtime.memory, endpoint)
            .and_then(|()| decode_call_frame(&runtime.memory, endpoint, argument_words, trap))
        {
            Ok(frame) => frame,
            Err(fault) => return DispatchOutcome::Fault(fault),
        };

        match handler(&frame, runtime, cpu) {
            ApiHandlerResult::Immediate(immediate) => {
                match runtime.complete_api_immediate(argument_words, &frame, immediate) {
                    Ok(completion) => DispatchOutcome::Immediate(completion),
                    Err(fault) => DispatchOutcome::Fault(fault),
                }
            }
            ApiHandlerResult::Deferred(request) => {
                DispatchOutcome::Deferred(ApiDeferredCall { request, frame })
            }
            ApiHandlerResult::GuestCallback(callback) => {
                DispatchOutcome::GuestCallback(ApiGuestCallbackCall {
                    request: callback,
                    frame,
                })
            }
            ApiHandlerResult::Fault(fault) => DispatchOutcome::Fault(fault.with_endpoint(endpoint)),
        }
    }

    pub fn complete_deferred(
        &self,
        call: &ApiDeferredCall,
        completion: HostCompletion,
        runtime: &mut crate::runtime::Runtime,
    ) -> DispatchOutcome {
        let argument_words =
            match self.deferred_argument_words(call, completion.request_id, &runtime.memory) {
                Ok(argument_words) => argument_words,
                Err(fault) => return DispatchOutcome::Fault(fault),
            };
        match runtime.complete_api_immediate(argument_words, &call.frame, completion.result) {
            Ok(completion) => DispatchOutcome::Immediate(completion),
            Err(fault) => DispatchOutcome::Fault(fault),
        }
    }

    pub fn validate_deferred_completion(
        &self,
        call: &ApiDeferredCall,
        request_id: u64,
        memory: &GuestMemory,
    ) -> Result<(), ApiFault> {
        self.deferred_argument_words(call, request_id, memory)
            .map(|_| ())
    }

    pub(crate) fn deferred_argument_words(
        &self,
        call: &ApiDeferredCall,
        request_id: u64,
        memory: &GuestMemory,
    ) -> Result<u8, ApiFault> {
        if call.request.request_id != request_id {
            return Err(ApiFault::new(
                "host completion does not identify the suspended API request",
            )
            .with_endpoint(&call.frame.endpoint));
        }
        self.suspended_argument_words(&call.frame, memory)
    }

    pub(crate) fn complete_guest_callback_result(
        &self,
        call: ApiGuestCallbackCall,
        result: u32,
        runtime: &mut crate::runtime::Runtime,
    ) -> DispatchOutcome {
        let argument_words = match self.suspended_argument_words(&call.frame, &runtime.memory) {
            Ok(argument_words) => argument_words,
            Err(fault) => return DispatchOutcome::Fault(fault),
        };
        let handler_result = if let Some(completion) =
            call.request.completion_policy.complete_direct(result)
        {
            ApiHandlerResult::Immediate(completion)
        } else {
            match call.request.completion_policy {
                GuestCallbackCompletionPolicy::CreateWindowExA { .. } => {
                    crate::user32::resolve_create_window_ex_a_callback(
                        &call.frame.endpoint,
                        &call,
                        result,
                        runtime,
                    )
                }
                GuestCallbackCompletionPolicy::SetWindowPos { .. } => {
                    crate::user32::resolve_set_window_pos_callback(&call, result, runtime)
                }
                GuestCallbackCompletionPolicy::SetWindowPosDefault { .. } => {
                    crate::user32::resolve_set_window_pos_default_callback(&call, result, runtime)
                }
                GuestCallbackCompletionPolicy::UpdateWindow { .. } => {
                    crate::user32::resolve_update_window_callback(&call, runtime)
                }
                GuestCallbackCompletionPolicy::ExceptionDispatch { .. } => {
                    crate::win32::resolve_exception_dispatch_callback(&call, result, runtime)
                }
                GuestCallbackCompletionPolicy::ReturnU32
                | GuestCallbackCompletionPolicy::ReturnBool32
                | GuestCallbackCompletionPolicy::ReturnBool32WithLastError { .. }
                | GuestCallbackCompletionPolicy::ReturnConstantU32 { .. } => {
                    ApiHandlerResult::Fault(ApiFault::new(
                        "direct callback completion policy did not produce an API result",
                    ))
                }
            }
        };
        let endpoint = &call.frame.endpoint;
        match handler_result {
            ApiHandlerResult::Immediate(immediate) => {
                match runtime.complete_api_immediate(argument_words, &call.frame, immediate) {
                    Ok(completion) => DispatchOutcome::Immediate(completion),
                    Err(fault) => DispatchOutcome::Fault(fault),
                }
            }
            ApiHandlerResult::GuestCallback(callback) => {
                DispatchOutcome::GuestCallback(ApiGuestCallbackCall {
                    request: callback,
                    frame: call.frame,
                })
            }
            ApiHandlerResult::Deferred(_) => DispatchOutcome::Fault(
                ApiFault::new(
                    "synchronous guest callback continuation attempted to defer the outer API",
                )
                .with_endpoint(endpoint),
            ),
            ApiHandlerResult::Fault(fault) => DispatchOutcome::Fault(fault.with_endpoint(endpoint)),
        }
    }

    fn suspended_argument_words(
        &self,
        frame: &ApiCallFrame,
        memory: &GuestMemory,
    ) -> Result<u8, ApiFault> {
        let endpoint = &frame.endpoint;
        let argument_words = self
            .endpoints_by_address
            .get(&endpoint.address)
            .filter(|(registered_endpoint, _)| registered_endpoint == endpoint)
            .and_then(|(_, argument_words)| *argument_words)
            .ok_or_else(|| {
                ApiFault::new("suspended API endpoint no longer matches the dispatcher registry")
                    .with_endpoint(endpoint)
            })?;
        validate_bound_iat(memory, endpoint)?;
        Ok(argument_words)
    }
}

fn validate_implemented_endpoint(endpoint: &ApiEndpointIdentity) -> Result<(), ApiFault> {
    match endpoint.binding {
        ApiEndpointBinding::ImportAddressTable
            if endpoint.iat_address.is_none()
                || endpoint.disposition != Some(ImportThunkDisposition::DeclaredRuntimeExport) =>
        {
            Err(ApiFault::new(
                "an unbound or unsupported import endpoint cannot be registered as an implemented API",
            ).with_endpoint(endpoint))
        }
        ApiEndpointBinding::DynamicExport
            if endpoint.iat_address.is_some() || endpoint.disposition.is_some() =>
        {
            Err(ApiFault::new(
                "a dynamic export endpoint cannot carry import binding metadata",
            ).with_endpoint(endpoint))
        }
        _ => Ok(()),
    }
}

fn unsupported_endpoint(endpoint: &ApiEndpointIdentity) -> ApiFault {
    let message = match endpoint.binding {
        ApiEndpointBinding::ImportAddressTable => {
            format!("owned endpoint {} is unsupported", endpoint_label(endpoint))
        }
        ApiEndpointBinding::DynamicExport => format!(
            "process-owned dynamic export endpoint {} is unsupported",
            endpoint_label(endpoint)
        ),
    };
    ApiFault::new(message).with_endpoint(endpoint)
}

fn validate_bound_iat(
    memory: &GuestMemory,
    endpoint: &ApiEndpointIdentity,
) -> Result<(), ApiFault> {
    if endpoint.binding == ApiEndpointBinding::DynamicExport {
        return Ok(());
    }
    let iat_address = endpoint.iat_address.ok_or_else(|| {
        ApiFault::new("an import endpoint has no IAT binding address").with_endpoint(endpoint)
    })?;
    let bytes = memory
        .read(iat_address, 4)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let value = u32::from_le_bytes(bytes.try_into().map_err(|_| {
        ApiFault::new("checked IAT entry did not contain one x86 pointer").with_endpoint(endpoint)
    })?);
    if value != endpoint.address.0 {
        return Err(
            ApiFault::new("IAT entry no longer points at the owned API endpoint")
                .with_endpoint(endpoint),
        );
    }
    Ok(())
}

fn decode_call_frame(
    memory: &GuestMemory,
    endpoint: &ApiEndpointIdentity,
    argument_words: u8,
    trap: ApiOwnedTrap,
) -> Result<ApiCallFrame, ApiFault> {
    let frame_bytes = (u32::from(argument_words) + 1) * 4;
    let bytes = memory
        .read(trap.stack_pointer, frame_bytes as usize)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let mut words = bytes
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().expect("x86 stack word")));
    if words.next() != Some(trap.return_address.0) {
        return Err(ApiFault::new(
            "guest stack return address does not match the trapped CPU state",
        )
        .with_endpoint(endpoint));
    }
    return_stack_pointer(trap.stack_pointer, argument_words, endpoint)?;
    Ok(ApiCallFrame {
        endpoint: endpoint.clone(),
        stack_pointer: trap.stack_pointer,
        return_address: trap.return_address,
        arguments: words.collect(),
    })
}

pub(crate) fn complete_immediate(
    memory: &mut GuestMemory,
    last_error: &mut u32,
    argument_words: u8,
    frame: &ApiCallFrame,
    immediate: ApiImmediate,
) -> Result<ApiImmediateCompletion, ApiFault> {
    let stack_pointer = return_stack_pointer(frame.stack_pointer, argument_words, &frame.endpoint)?;
    if frame.arguments.len() != usize::from(argument_words) {
        return Err(ApiFault::new(
            "API completion frame does not contain the decoded ABI arguments",
        )
        .with_endpoint(&frame.endpoint));
    }

    let mut transaction = memory.begin_transaction();
    for write in immediate.guest_writes {
        transaction
            .write(write.address, &write.bytes)
            .map_err(|error| ApiFault::from_memory(error, &frame.endpoint))?;
    }

    if let Some(value) = immediate.last_error {
        *last_error = value;
    }
    transaction.commit_transaction();
    Ok(ApiImmediateCompletion {
        created_thread: immediate.created_thread,
        instruction_pointer: frame.return_address,
        stack_pointer,
        eax: immediate.eax,
        execution_boundary: immediate.execution_boundary,
    })
}

fn return_stack_pointer(
    stack_pointer: GuestAddress,
    argument_words: u8,
    endpoint: &ApiEndpointIdentity,
) -> Result<GuestAddress, ApiFault> {
    let cleanup_bytes = (u32::from(argument_words) + 1) * 4;
    stack_pointer
        .0
        .checked_add(cleanup_bytes)
        .map(GuestAddress)
        .ok_or_else(|| {
            ApiFault::new("API return stack pointer crosses the x86 address limit")
                .with_endpoint(endpoint)
        })
}

fn endpoint_label(endpoint: &ApiEndpointIdentity) -> String {
    let symbol = match &endpoint.symbol {
        ApiSymbol::Name { name } => name.clone(),
        ApiSymbol::Ordinal { ordinal } => format!("#{ordinal}"),
    };
    format!("{}!{symbol}", endpoint.module)
}

#[cfg(test)]
#[path = "api_dispatcher_tests.rs"]
mod dispatcher_tests;

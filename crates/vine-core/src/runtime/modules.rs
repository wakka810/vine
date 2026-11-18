use std::collections::{BTreeMap, VecDeque};

use super::objects::RuntimeObjectTable;
use super::{
    RuntimeCallbackFrame, RuntimeCallbackFrameId, RuntimeHandle, RuntimeObjectKey,
    RuntimeObjectKind, RuntimeStateError,
};
use crate::callbacks::{CALLBACK_RETURN_ARENA_BASE, CALLBACK_RETURN_ARENA_SIZE};
use crate::imports::{ImportThunkId, RuntimeExport};
use crate::materialize::ImageMaterialization;
use crate::memory::{GuestAddress, GuestMemory, MemoryOwnerId};
use crate::module_graph::{
    RuntimeModuleClass, RuntimeModuleGraph, RuntimeModuleGraphError, RuntimeModuleId,
    RuntimeModuleLoadState, RuntimeModuleRegistration,
};
use crate::module_images::{
    RuntimeModuleImage, RuntimeModuleImageError, RuntimeModuleImageRegistry, RuntimeResolvedExport,
};
use crate::pe32::{ExportLookup, ImportTarget, Pe32LoadPlan};

pub(crate) const DLL_PROCESS_DETACH: u32 = 0;
pub(crate) const DLL_PROCESS_ATTACH: u32 = 1;
pub(crate) const DLL_PROCESS_STATIC_RESERVED: u32 = 1;

const RUNTIME_MODULE_HANDLE_GENERATION: u8 = 1;

// Contracts reached through the Windows loader, resolved to Vine's system host.
// Other API sets remain unresolved; a prefix alone does not imply support.
pub(crate) fn api_set_host(name: &str) -> Option<&'static str> {
    let name = name.to_ascii_lowercase();
    let contract = name.strip_suffix(".dll").unwrap_or(&name);
    match contract {
        "api-ms-win-core-synch-l1-2-0"
        | "api-ms-win-core-fibers-l1-1-1"
        | "api-ms-win-core-localization-l1-2-1" => Some("kernel32.dll"),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeModule {
    pub handle: u32,
    pub requested_name: String,
    pub canonical_name: String,
    pub runtime_system_module: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeGuestImport {
    pub requested_module: String,
    pub target: ImportTarget,
    pub iat_address: GuestAddress,
    pub resolution: RuntimeGuestImportResolution,
}

impl RuntimeGuestImport {
    pub fn resolved_address(&self) -> GuestAddress {
        match &self.resolution {
            RuntimeGuestImportResolution::GuestExport { address } => *address,
            RuntimeGuestImportResolution::RuntimeExport { endpoint } => endpoint.address,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeGuestImportResolution {
    GuestExport {
        address: GuestAddress,
    },
    RuntimeExport {
        endpoint: RuntimeSystemExportEndpoint,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSystemExportEndpoint {
    pub id: ImportThunkId,
    pub address: GuestAddress,
    pub export: RuntimeExport,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeSystemExportTable {
    pub endpoints: Vec<RuntimeSystemExportEndpoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeModuleDependencyEdge {
    pub requester: RuntimeModuleId,
    pub dependency: RuntimeModuleId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeGuestImportBinding {
    pub requester_module_id: RuntimeModuleId,
    pub requester_handle: u32,
    pub image_owner: MemoryOwnerId,
    pub imports: Vec<RuntimeGuestImport>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeDllLifecyclePhase {
    Idle,
    ProcessAttaching,
    AttachUnwinding,
    ProcessAttached,
    ProcessDetaching,
    ProcessDetached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeDllCallbackReason {
    ProcessAttach,
    ProcessDetach,
}

impl RuntimeDllCallbackReason {
    pub const fn value(self) -> u32 {
        match self {
            Self::ProcessAttach => DLL_PROCESS_ATTACH,
            Self::ProcessDetach => DLL_PROCESS_DETACH,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeDllCallbackRequest {
    pub frame: RuntimeCallbackFrame,
    pub(crate) module_id: RuntimeModuleId,
    pub(crate) module_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeDllLifecycleAction {
    Callback {
        target: GuestAddress,
        stack_pointer: GuestAddress,
    },
    Complete {
        phase: RuntimeDllLifecyclePhase,
    },
    AttachFailed {
        failed_module: String,
    },
}

#[derive(Debug, Default, Clone)]
pub(crate) struct RuntimeModules {
    modules: BTreeMap<u32, RuntimeModule>,
    module_graph: RuntimeModuleGraph,
    module_images: RuntimeModuleImageRegistry,
    module_import_bindings: BTreeMap<RuntimeModuleId, RuntimeGuestImportBinding>,
    runtime_system_exports: Option<RuntimeSystemExportTable>,
    dll_lifecycle: RuntimeDllLifecycle,
    pending_dynamic_library: Option<(u32, u32)>,
    pending_library_release: Option<(u32, u32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeDllLifecycle {
    phase: RuntimeDllLifecyclePhase,
    operation: Option<RuntimeDllOperation>,
}

impl Default for RuntimeDllLifecycle {
    fn default() -> Self {
        Self {
            phase: RuntimeDllLifecyclePhase::Idle,
            operation: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeDllOperationKind {
    Attach,
    AttachUnwind,
    #[cfg(test)]
    Detach,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeDllOperation {
    kind: RuntimeDllOperationKind,
    graph_order: Vec<RuntimeModuleId>,
    remaining: VecDeque<RuntimeModuleId>,
    completed: Vec<RuntimeModuleId>,
    pending: Option<RuntimeDllCallbackRequest>,
    attach_failure: Option<String>,
    return_sentinel: GuestAddress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeDllCallbackSpec {
    module_id: RuntimeModuleId,
    module_name: String,
    pub(super) module_base: GuestAddress,
    pub(super) entry_point: GuestAddress,
    pub(super) memory_owner: crate::memory::MemoryOwnerId,
    pub(super) reason: RuntimeDllCallbackReason,
    pub(super) reserved: u32,
    pub(super) return_sentinel: GuestAddress,
}

pub(super) enum RuntimeDllLifecycleRequest {
    Attach(GuestAddress),
    #[cfg(test)]
    Detach(GuestAddress),
    Complete {
        frame_id: RuntimeCallbackFrameId,
        result: u32,
    },
}

impl RuntimeModules {
    pub(crate) fn process_detach_targets(
        &self,
    ) -> Result<Vec<(u32, GuestAddress)>, RuntimeStateError> {
        let order = self
            .module_graph
            .planned_detach_order()
            .map_err(runtime_module_graph_error)?;
        self.dll_callback_order(&order)?
            .into_iter()
            .map(|id| {
                let image = self
                    .module_images
                    .image(id)
                    .map_err(runtime_module_image_error)?;
                Ok((
                    image.image_range.start().0,
                    image.entry_point.expect("DLL callback target"),
                ))
            })
            .collect()
    }
    pub(super) fn stage_dynamic_library(
        &mut self,
        objects: &mut RuntimeObjectTable,
        name: &str,
        catalog: &crate::imports::RuntimeExportCatalog,
        system_modules: &[String],
        thread: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.require_graph_mutation_allowed()?;
        if self.pending_dynamic_library.is_some() {
            return Err(RuntimeStateError::new(
                "nested guest DLL loading is not supported",
            ));
        }
        self.module_graph
            .planned_detach_order()
            .map_err(runtime_module_graph_error)?;
        for name in system_modules {
            if self.module_by_name_handle(name)?.is_none() {
                let handle = objects.next_available_handle(RuntimeObjectKind::Module)?.0;
                self.register(
                    objects,
                    RuntimeModule {
                        handle,
                        requested_name: name.clone(),
                        canonical_name: name.to_ascii_lowercase(),
                        runtime_system_module: true,
                    },
                    RuntimeModuleClass::RuntimeSystemModule,
                )?;
                self.mark_ready(handle)?;
                let id = self.module_id_by_handle(handle)?;
                self.module_graph
                    .attach_ready_dynamic_module(id)
                    .map_err(runtime_module_graph_error)?;
            }
        }
        if let Some(table) = &mut self.runtime_system_exports {
            let endpoints = catalog
                .exports()
                .iter()
                .cloned()
                .enumerate()
                .map(|(index, export)| crate::dll::runtime_export_endpoint(index, export))
                .collect::<Result<Vec<_>, _>>()?;
            if !endpoints.starts_with(&table.endpoints) {
                return Err(RuntimeStateError::new(
                    "dynamic load changed an existing runtime export address",
                ));
            }
            table.endpoints = endpoints;
        }
        let handle = objects.next_available_handle(RuntimeObjectKind::Module)?.0;
        self.register(
            objects,
            RuntimeModule {
                handle,
                requested_name: name.to_owned(),
                canonical_name: name
                    .rsplit(['\\', '/'])
                    .next()
                    .unwrap_or(name)
                    .to_ascii_lowercase(),
                runtime_system_module: false,
            },
            RuntimeModuleClass::GuestDynamicLibrary,
        )?;
        self.add_dynamic_reference(handle, thread)?;
        self.pending_dynamic_library = Some((handle, thread));
        Ok(handle)
    }

    pub(crate) fn has_pending_dynamic_library(&self) -> bool {
        self.pending_dynamic_library.is_some()
    }

    pub(super) fn pending_dynamic_library(&self, handle: u32, thread: u32) -> bool {
        self.pending_dynamic_library == Some((handle, thread))
    }

    pub(super) fn finish_dynamic_library(
        &mut self,
        handle: u32,
        thread: u32,
        success: bool,
    ) -> Result<(), RuntimeStateError> {
        if !self.pending_dynamic_library(handle, thread) {
            return Err(RuntimeStateError::new(
                "DLL completion does not own the pending library",
            ));
        }
        let id = self.module_id_by_handle(handle)?;
        if success {
            self.module_graph
                .attach_ready_dynamic_module(id)
                .map_err(runtime_module_graph_error)?;
        } else {
            self.module_graph
                .remove_dynamic_module(id)
                .map_err(runtime_module_graph_error)?;
            self.module_images.remove(id);
            self.module_import_bindings.remove(&id);
            self.modules.remove(&handle);
        }
        self.pending_dynamic_library = None;
        Ok(())
    }

    pub(super) fn library_requires_detach(&self, handle: u32) -> Result<bool, RuntimeStateError> {
        self.require_graph_mutation_allowed()?;
        if self.pending_dynamic_library.is_some() {
            return Err(RuntimeStateError::new(
                "FreeLibrary during DLL attachment is not implemented",
            ));
        }
        let id = self.module_id_by_handle(handle)?;
        let node = self
            .module_graph
            .module(id)
            .map_err(runtime_module_graph_error)?;
        if !matches!(node.state, RuntimeModuleLoadState::ProcessAttached { .. }) {
            return Err(RuntimeStateError::new(
                "FreeLibrary requires an attached module",
            ));
        }
        let dynamic = node.references.iter().any(|reference| {
            matches!(
                reference.owner,
                crate::module_graph::RuntimeModuleReferenceOwner::DynamicLoad { .. }
            )
        });
        if !dynamic {
            if node.class == RuntimeModuleClass::RuntimeSystemModule {
                return Ok(false); // Runtime service modules remain pinned.
            }
            return Err(RuntimeStateError::new(
                "FreeLibrary without a dynamic DLL reference is not implemented",
            ));
        }
        let detach = node.reference_count() == 1;
        if detach {
            for dependency in &node.dependencies {
                if self
                    .module_graph
                    .module(*dependency)
                    .map_err(runtime_module_graph_error)?
                    .reference_count()
                    == 1
                {
                    return Err(RuntimeStateError::new(
                        "cascading guest DLL detach is not implemented",
                    ));
                }
            }
        }
        Ok(detach)
    }

    pub(super) fn release_library_reference(
        &mut self,
        handle: u32,
    ) -> Result<(), RuntimeStateError> {
        if self.library_requires_detach(handle)? {
            return Err(RuntimeStateError::new(
                "last DLL reference requires process detach",
            ));
        }
        let id = self.module_id_by_handle(handle)?;
        let node = self
            .module_graph
            .module(id)
            .map_err(runtime_module_graph_error)?;
        if node.references.iter().any(|reference| {
            matches!(
                reference.owner,
                crate::module_graph::RuntimeModuleReferenceOwner::DynamicLoad { .. }
            )
        }) {
            self.module_graph
                .release_process_dynamic_reference(id)
                .map_err(runtime_module_graph_error)?;
        }
        Ok(())
    }

    pub(super) fn begin_library_release(
        &mut self,
        handle: u32,
        thread: u32,
    ) -> Result<(), RuntimeStateError> {
        if thread == 0 || !self.library_requires_detach(handle)? {
            return Err(RuntimeStateError::new(
                "DLL release requires its final reference and a live thread",
            ));
        }
        self.pending_library_release = Some((handle, thread));
        Ok(())
    }

    pub(super) fn finish_library_release(
        &mut self,
        handle: u32,
        thread: u32,
    ) -> Result<(), RuntimeStateError> {
        if self.pending_library_release != Some((handle, thread)) {
            return Err(RuntimeStateError::new(
                "DLL detach completion does not own the pending library",
            ));
        }
        let id = self.module_id_by_handle(handle)?;
        self.module_graph
            .release_process_dynamic_reference(id)
            .map_err(runtime_module_graph_error)?;
        self.module_graph
            .remove_dynamic_module(id)
            .map_err(runtime_module_graph_error)?;
        self.module_images.remove(id);
        self.module_import_bindings.remove(&id);
        self.modules.remove(&handle);
        self.pending_library_release = None;
        Ok(())
    }

    pub(super) fn modules(&self) -> &BTreeMap<u32, RuntimeModule> {
        &self.modules
    }

    pub(crate) fn module(&self, handle: u32) -> Result<&RuntimeModule, RuntimeStateError> {
        self.modules.get(&handle).ok_or_else(|| {
            RuntimeStateError::new(format!(
                "logical module handle {handle:#010x} is not loaded"
            ))
        })
    }

    fn validate_registration(&self, module: &RuntimeModule) -> Result<(), RuntimeStateError> {
        if module.handle == 0
            || module.requested_name.is_empty()
            || module.canonical_name.is_empty()
        {
            return Err(RuntimeStateError::new(
                "runtime module requires a nonzero handle and nonempty requested and canonical names",
            ));
        }
        if self.modules.contains_key(&module.handle)
            || self.modules.values().any(|loaded| {
                loaded
                    .canonical_name
                    .eq_ignore_ascii_case(&module.canonical_name)
            })
        {
            return Err(RuntimeStateError::new(
                "runtime module handle or canonical name is already loaded",
            ));
        }
        Ok(())
    }

    fn commit_registration(&mut self, module: RuntimeModule) -> u32 {
        let handle = module.handle;
        let previous = self.modules.insert(handle, module);
        debug_assert!(previous.is_none());
        handle
    }
}

impl RuntimeModules {
    pub(crate) fn graph(&self) -> &RuntimeModuleGraph {
        &self.module_graph
    }

    pub(crate) fn images(&self) -> &RuntimeModuleImageRegistry {
        &self.module_images
    }

    pub(super) fn bindings(&self) -> &BTreeMap<RuntimeModuleId, RuntimeGuestImportBinding> {
        &self.module_import_bindings
    }

    pub(crate) fn runtime_system_exports(&self) -> Option<&RuntimeSystemExportTable> {
        self.runtime_system_exports.as_ref()
    }

    pub(super) fn dll_lifecycle_phase(&self) -> RuntimeDllLifecyclePhase {
        self.dll_lifecycle.phase
    }

    fn require_graph_mutation_allowed(&self) -> Result<(), RuntimeStateError> {
        if self.dll_lifecycle.operation.is_some() || self.pending_library_release.is_some() {
            return Err(RuntimeStateError::new(
                "module graph mutation is not supported while DLL callbacks are in progress",
            ));
        }
        Ok(())
    }

    pub(super) fn require_process_attach_start(&self) -> Result<(), RuntimeStateError> {
        if self.dll_lifecycle.phase != RuntimeDllLifecyclePhase::Idle
            || self.dll_lifecycle.operation.is_some()
        {
            return Err(RuntimeStateError::new(
                "DLL process attach requires an idle lifecycle coordinator",
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn require_process_detach_start(&self) -> Result<(), RuntimeStateError> {
        if self.dll_lifecycle.phase != RuntimeDllLifecyclePhase::ProcessAttached
            || self.dll_lifecycle.operation.is_some()
        {
            return Err(RuntimeStateError::new(
                "DLL process detach requires a completed process attach",
            ));
        }
        Ok(())
    }

    pub(super) fn advance_lifecycle(
        &mut self,
        request: RuntimeDllLifecycleRequest,
        initialize: impl FnOnce(
            Option<&RuntimeDllCallbackSpec>,
        ) -> Result<
            Option<(RuntimeCallbackFrame, GuestAddress)>,
            RuntimeStateError,
        >,
    ) -> Result<RuntimeDllLifecycleAction, RuntimeStateError> {
        match request {
            RuntimeDllLifecycleRequest::Attach(return_sentinel) => {
                self.require_process_attach_start()?;
                self.start_dll_operation(
                    RuntimeDllOperationKind::Attach,
                    return_sentinel,
                    initialize,
                )
            }
            #[cfg(test)]
            RuntimeDllLifecycleRequest::Detach(return_sentinel) => {
                self.require_process_detach_start()?;
                self.start_dll_operation(
                    RuntimeDllOperationKind::Detach,
                    return_sentinel,
                    initialize,
                )
            }
            RuntimeDllLifecycleRequest::Complete { frame_id, result } => {
                self.complete_dll_callback(frame_id, result, initialize)
            }
        }
    }

    fn start_dll_operation(
        &mut self,
        kind: RuntimeDllOperationKind,
        return_sentinel: GuestAddress,
        initialize: impl FnOnce(
            Option<&RuntimeDllCallbackSpec>,
        ) -> Result<
            Option<(RuntimeCallbackFrame, GuestAddress)>,
            RuntimeStateError,
        >,
    ) -> Result<RuntimeDllLifecycleAction, RuntimeStateError> {
        validate_dll_return_sentinel(return_sentinel)?;
        let graph_order = match kind {
            RuntimeDllOperationKind::Attach => self.module_graph.planned_attach_order(),
            #[cfg(test)]
            RuntimeDllOperationKind::Detach => self.module_graph.planned_detach_order(),
            RuntimeDllOperationKind::AttachUnwind => {
                unreachable!("unwind begins at a failed attach callback")
            }
        }
        .map_err(runtime_module_graph_error)?;
        let callback_order = self.dll_callback_order(&graph_order)?;
        let prepared = self.initialize_dll_callback(
            kind,
            callback_order.first().copied(),
            return_sentinel,
            initialize,
        )?;
        if let Some((pending, stack_pointer)) = prepared {
            let target = pending.frame.target;
            let mut remaining: VecDeque<_> = callback_order.into();
            remaining.pop_front();
            self.dll_lifecycle.phase = if kind == RuntimeDllOperationKind::Attach {
                RuntimeDllLifecyclePhase::ProcessAttaching
            } else {
                RuntimeDllLifecyclePhase::ProcessDetaching
            };
            self.dll_lifecycle.operation = Some(RuntimeDllOperation {
                kind,
                graph_order,
                remaining,
                completed: Vec::new(),
                pending: Some(pending),
                attach_failure: None,
                return_sentinel,
            });
            Ok(RuntimeDllLifecycleAction::Callback {
                target,
                stack_pointer,
            })
        } else {
            let phase = self.publish_dll_graph_completion(kind, &graph_order);
            self.dll_lifecycle.phase = phase;
            Ok(RuntimeDllLifecycleAction::Complete { phase })
        }
    }

    fn complete_dll_callback(
        &mut self,
        frame_id: RuntimeCallbackFrameId,
        result: u32,
        initialize: impl FnOnce(
            Option<&RuntimeDllCallbackSpec>,
        ) -> Result<
            Option<(RuntimeCallbackFrame, GuestAddress)>,
            RuntimeStateError,
        >,
    ) -> Result<RuntimeDllLifecycleAction, RuntimeStateError> {
        let operation = self.dll_lifecycle.operation.as_ref().ok_or_else(|| {
            RuntimeStateError::new("DLL lifecycle coordinator has no active operation")
        })?;
        let pending = operation.pending.as_ref().ok_or_else(|| {
            RuntimeStateError::new("DLL lifecycle coordinator has no pending callback")
        })?;
        if pending.frame.id != frame_id {
            return Err(RuntimeStateError::new(
                "DLL callback completion does not identify the pending callback frame",
            ));
        }
        let attach_failed = operation.kind == RuntimeDllOperationKind::Attach && result == 0;
        let (kind, next_module) = if attach_failed {
            (
                RuntimeDllOperationKind::AttachUnwind,
                operation.completed.last().copied(),
            )
        } else {
            (operation.kind, operation.remaining.front().copied())
        };
        if next_module.is_none() {
            if kind != RuntimeDllOperationKind::AttachUnwind {
                let graph_order = if kind == RuntimeDllOperationKind::Attach {
                    self.module_graph.planned_attach_order()
                } else {
                    self.module_graph.planned_detach_order()
                }
                .map_err(runtime_module_graph_error)?;
                if graph_order != operation.graph_order {
                    return Err(RuntimeStateError::new(
                        "module graph order changed while DLL callbacks were in progress",
                    ));
                }
            } else if !attach_failed && operation.attach_failure.is_none() {
                return Err(RuntimeStateError::new(
                    "DLL attach unwind lost the failing module identity",
                ));
            }
        }
        let prepared =
            self.initialize_dll_callback(kind, next_module, operation.return_sentinel, initialize)?;
        // Every fallible identity, graph, stack and memory check has completed.
        // Advance the live operation directly; no lifecycle snapshot crosses this call.
        let mut operation = self
            .dll_lifecycle
            .operation
            .take()
            .expect("validated DLL operation");
        let pending = operation
            .pending
            .take()
            .expect("validated pending DLL callback");
        if attach_failed {
            operation.kind = RuntimeDllOperationKind::AttachUnwind;
            operation.attach_failure = Some(pending.module_name);
            operation.remaining = std::mem::take(&mut operation.completed)
                .into_iter()
                .rev()
                .collect();
            self.dll_lifecycle.phase = RuntimeDllLifecyclePhase::AttachUnwinding;
        } else if operation.kind == RuntimeDllOperationKind::Attach {
            operation.completed.push(pending.module_id);
        }
        if let Some((pending, stack_pointer)) = prepared {
            operation.remaining.pop_front();
            let target = pending.frame.target;
            operation.pending = Some(pending);
            self.dll_lifecycle.operation = Some(operation);
            Ok(RuntimeDllLifecycleAction::Callback {
                target,
                stack_pointer,
            })
        } else if kind == RuntimeDllOperationKind::AttachUnwind {
            self.dll_lifecycle.phase = RuntimeDllLifecyclePhase::Idle;
            Ok(RuntimeDllLifecycleAction::AttachFailed {
                failed_module: operation
                    .attach_failure
                    .expect("validated failing DLL identity"),
            })
        } else {
            let phase = self.publish_dll_graph_completion(kind, &operation.graph_order);
            self.dll_lifecycle.phase = phase;
            Ok(RuntimeDllLifecycleAction::Complete { phase })
        }
    }

    fn initialize_dll_callback(
        &self,
        kind: RuntimeDllOperationKind,
        module_id: Option<RuntimeModuleId>,
        return_sentinel: GuestAddress,
        initialize: impl FnOnce(
            Option<&RuntimeDllCallbackSpec>,
        ) -> Result<
            Option<(RuntimeCallbackFrame, GuestAddress)>,
            RuntimeStateError,
        >,
    ) -> Result<Option<(RuntimeDllCallbackRequest, GuestAddress)>, RuntimeStateError> {
        let reason = if kind == RuntimeDllOperationKind::Attach {
            RuntimeDllCallbackReason::ProcessAttach
        } else {
            RuntimeDllCallbackReason::ProcessDetach
        };
        let spec = module_id
            .map(|id| self.dll_callback_spec(id, reason, return_sentinel))
            .transpose()?;
        let prepared = initialize(spec.as_ref())?;
        Ok(spec.map(|spec| {
            let (frame, stack_pointer) =
                prepared.expect("initialized DLL callback frame and stack");
            (
                RuntimeDllCallbackRequest {
                    frame,
                    module_id: spec.module_id,
                    module_name: spec.module_name,
                },
                stack_pointer,
            )
        }))
    }

    fn publish_dll_graph_completion(
        &mut self,
        kind: RuntimeDllOperationKind,
        graph_order: &[RuntimeModuleId],
    ) -> RuntimeDllLifecyclePhase {
        match kind {
            RuntimeDllOperationKind::Attach => {
                self.module_graph.publish_attach(graph_order);
                RuntimeDllLifecyclePhase::ProcessAttached
            }
            #[cfg(test)]
            RuntimeDllOperationKind::Detach => {
                self.module_graph.publish_detach(graph_order);
                RuntimeDllLifecyclePhase::ProcessDetached
            }
            RuntimeDllOperationKind::AttachUnwind => {
                unreachable!("failed attach does not publish the module graph")
            }
        }
    }

    fn dll_callback_order(
        &self,
        graph_order: &[RuntimeModuleId],
    ) -> Result<Vec<RuntimeModuleId>, RuntimeStateError> {
        let mut callbacks = Vec::new();
        for module_id in graph_order {
            let node = self
                .module_graph
                .module(*module_id)
                .map_err(runtime_module_graph_error)?;
            if matches!(
                node.class,
                RuntimeModuleClass::MainExecutable | RuntimeModuleClass::GuestDynamicLibrary
            ) {
                if !self.module_import_bindings.contains_key(module_id) {
                    return Err(RuntimeStateError::new(format!(
                        "module {} has no committed import binding",
                        node.canonical_name
                    )));
                }
                let image = self
                    .module_images
                    .image(*module_id)
                    .map_err(runtime_module_image_error)?;
                if node.class == RuntimeModuleClass::GuestDynamicLibrary
                    && image.entry_point.is_some()
                {
                    callbacks.push(*module_id);
                }
            }
        }
        Ok(callbacks)
    }

    fn dll_callback_spec(
        &self,
        module_id: RuntimeModuleId,
        reason: RuntimeDllCallbackReason,
        return_sentinel: GuestAddress,
    ) -> Result<RuntimeDllCallbackSpec, RuntimeStateError> {
        let node = self
            .module_graph
            .module(module_id)
            .map_err(runtime_module_graph_error)?;
        let image = self
            .module_images
            .image(module_id)
            .map_err(runtime_module_image_error)?;
        let entry_point = image.entry_point.ok_or_else(|| {
            RuntimeStateError::new("DLL callback order includes a module without an entry point")
        })?;
        Ok(RuntimeDllCallbackSpec {
            module_id,
            module_name: node.canonical_name.clone(),
            module_base: image.image_range.start(),
            entry_point,
            memory_owner: image.memory_owner,
            reason,
            reserved: DLL_PROCESS_STATIC_RESERVED,
            return_sentinel,
        })
    }

    pub(crate) fn resolve(
        &self,
        module: &str,
        symbol: ExportLookup,
    ) -> Result<Option<RuntimeResolvedExport>, RuntimeModuleImageError> {
        self.module_images.resolve(module, symbol)
    }

    pub(super) fn register(
        &mut self,
        objects: &mut RuntimeObjectTable,
        module: RuntimeModule,
        class: RuntimeModuleClass,
    ) -> Result<u32, RuntimeStateError> {
        self.require_graph_mutation_allowed()?;
        self.validate_registration(&module)?;
        let node = self
            .module_graph
            .new_node(RuntimeModuleRegistration {
                handle: module.handle,
                requested_name: module.requested_name.clone(),
                canonical_name: module.canonical_name.clone(),
                class,
            })
            .map_err(runtime_module_graph_error)?;
        objects.register_object_with_handle(
            RuntimeObjectKey::Module {
                handle: module.handle,
            },
            RuntimeHandle(module.handle),
        )?;
        self.module_graph.insert_node(node);
        Ok(self.commit_registration(module))
    }

    pub(super) fn load_runtime_system_module(
        &mut self,
        objects: &mut RuntimeObjectTable,
        requested_name: String,
        canonical_name: String,
        thread_id: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.require_graph_mutation_allowed()?;
        if !matches!(
            self.dll_lifecycle.phase,
            RuntimeDllLifecyclePhase::Idle | RuntimeDllLifecyclePhase::ProcessAttached
        ) {
            return Err(RuntimeStateError::new(
                "dynamic runtime-system module loading requires a process-attached module graph",
            ));
        }
        self.module_graph.planned_detach_order().map_err(|_| RuntimeStateError::new("dynamic runtime-system module loading requires every existing module to be process-attached",
        ))?;
        match self
            .module_graph
            .find_module_by_name(&canonical_name)
            .map_err(runtime_module_graph_error)?
        {
            Some(node) => {
                let node_id = node.id;
                let handle = node.handle;
                let record = self.module(handle)?;
                if node.class != RuntimeModuleClass::RuntimeSystemModule
                    || !record.runtime_system_module
                    || !record
                        .canonical_name
                        .eq_ignore_ascii_case(&node.canonical_name)
                {
                    return Err(RuntimeStateError::new(
                        "the requested module identity is owned by a non-runtime module class",
                    ));
                }
                objects.object_for_handle(RuntimeHandle(handle), RuntimeObjectKind::Module)?;
                self.module_graph
                    .load_existing_runtime_system_module(node_id, thread_id)
                    .map_err(runtime_module_graph_error)?;
                Ok(handle)
            }
            None => {
                let module = RuntimeModule {
                    handle: next_runtime_module_handle(self.modules())?,
                    requested_name,
                    canonical_name,
                    runtime_system_module: true,
                };
                self.validate_registration(&module)?;
                let node = self
                    .module_graph
                    .new_attached_runtime_system_node(
                        RuntimeModuleRegistration {
                            handle: module.handle,
                            requested_name: module.requested_name.clone(),
                            canonical_name: module.canonical_name.clone(),
                            class: RuntimeModuleClass::RuntimeSystemModule,
                        },
                        thread_id,
                    )
                    .map_err(runtime_module_graph_error)?;

                objects.register_object_with_handle(
                    RuntimeObjectKey::Module {
                        handle: module.handle,
                    },
                    RuntimeHandle(module.handle),
                )?;
                self.module_graph.insert_node(node);
                Ok(self.commit_registration(module))
            }
        }
    }

    pub(super) fn module_by_name_handle(
        &self,
        name: &str,
    ) -> Result<Option<u32>, RuntimeStateError> {
        self.module_graph
            .find_module_by_name(api_set_host(name).unwrap_or(name))
            .map(|module| module.map(|module| module.handle))
            .map_err(runtime_module_graph_error)
    }

    pub(super) fn main_module_handle(&self) -> Result<u32, RuntimeStateError> {
        let id = self.module_graph.main_module().ok_or_else(|| {
            RuntimeStateError::new("the process has no registered main executable module")
        })?;
        self.module_graph
            .module(id)
            .map(|module| module.handle)
            .map_err(runtime_module_graph_error)
    }

    pub(super) fn module_id_by_handle(
        &self,
        handle: u32,
    ) -> Result<RuntimeModuleId, RuntimeStateError> {
        self.module_graph
            .module_by_handle(handle)
            .map(|module| module.id)
            .map_err(runtime_module_graph_error)
    }

    #[cfg(test)]
    pub(super) fn add_dependency(
        &mut self,
        requester_handle: u32,
        dependency_handle: u32,
    ) -> Result<(), RuntimeStateError> {
        self.require_graph_mutation_allowed()?;
        let requester = self.module_id_by_handle(requester_handle)?;
        let dependency = self.module_id_by_handle(dependency_handle)?;
        self.module_graph
            .add_dependency(requester, dependency)
            .map_err(runtime_module_graph_error)
    }

    pub(super) fn add_dynamic_reference(
        &mut self,
        handle: u32,
        thread_id: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.require_graph_mutation_allowed()?;
        let module = self.module_id_by_handle(handle)?;
        self.module_graph
            .add_dynamic_reference(module, thread_id)
            .map_err(runtime_module_graph_error)
    }

    pub(super) fn mark_ready(&mut self, handle: u32) -> Result<(), RuntimeStateError> {
        self.require_graph_mutation_allowed()?;
        let module = self.module_id_by_handle(handle)?;
        self.module_graph
            .mark_ready(module)
            .map_err(runtime_module_graph_error)
    }

    #[cfg(test)]
    pub(super) fn attach_all(&mut self) -> Result<Vec<RuntimeModuleId>, RuntimeStateError> {
        if self.dll_lifecycle.phase != RuntimeDllLifecyclePhase::Idle {
            return Err(RuntimeStateError::new(
                "direct module attach cannot bypass the active DLL lifecycle coordinator",
            ));
        }
        self.require_graph_mutation_allowed()?;
        self.module_graph
            .attach_all()
            .map_err(runtime_module_graph_error)
    }

    pub(super) fn register_image(
        &mut self,
        memory: &GuestMemory,
        handle: u32,
        loader: &Pe32LoadPlan,
        materialization: &ImageMaterialization,
    ) -> Result<&RuntimeModuleImage, RuntimeModuleImageError> {
        self.require_graph_mutation_allowed()
            .map_err(runtime_module_image_state_error)?;
        let node = self
            .module_graph
            .module_by_handle(handle)
            .map_err(RuntimeModuleImageError::from_module_graph)?
            .clone();
        self.module_images
            .register(memory, &node, loader, materialization)
    }

    pub(crate) fn register_guest_import_binding(
        &mut self,
        binding: RuntimeGuestImportBinding,
        dependency_edges: &[RuntimeModuleDependencyEdge],
        runtime_system_exports: Option<RuntimeSystemExportTable>,
    ) -> Result<&RuntimeGuestImportBinding, RuntimeStateError> {
        self.require_graph_mutation_allowed()?;
        let requester = self
            .module_graph
            .module_by_handle(binding.requester_handle)
            .map_err(runtime_module_graph_error)?;
        if requester.state != RuntimeModuleLoadState::Registered {
            return Err(RuntimeStateError::new(
                "guest imports can only bind while the requester module is registered",
            ));
        }
        if self.module_import_bindings.contains_key(&requester.id) {
            return Err(RuntimeStateError::new(
                "requester module already has a committed guest import binding",
            ));
        }
        if self.module_images.images().contains_key(&requester.id) {
            return Err(RuntimeStateError::new(
                "requester image must bind imports before mapped image registration",
            ));
        }
        if binding
            .imports
            .iter()
            .any(|import| import.resolved_address().0 == 0)
        {
            return Err(RuntimeStateError::new(
                "resolved guest import address cannot be null",
            ));
        }

        if requester.id != binding.requester_module_id {
            return Err(RuntimeStateError::new(
                "guest import binding requester identity disagrees with its live module",
            ));
        }
        self.module_graph
            .validate_additional_dependencies(
                &dependency_edges
                    .iter()
                    .map(|edge| (edge.requester, edge.dependency))
                    .collect::<Vec<_>>(),
            )
            .map_err(runtime_module_graph_error)?;
        let imports_runtime_exports = binding.imports.iter().any(|import| {
            matches!(
                &import.resolution,
                RuntimeGuestImportResolution::RuntimeExport { .. }
            )
        });
        match (
            self.runtime_system_exports.as_ref(),
            runtime_system_exports.as_ref(),
            imports_runtime_exports,
        ) {
            (None, Some(table), true)
                if binding
                    .imports
                    .iter()
                    .all(|import| match &import.resolution {
                        RuntimeGuestImportResolution::RuntimeExport { endpoint } => {
                            table.endpoints.contains(endpoint)
                        }
                        RuntimeGuestImportResolution::GuestExport { .. } => true,
                    }) => {}
            (Some(_), None, _) | (None, None, false) => {}
            (None, Some(_), false) => {
                return Err(RuntimeStateError::new(
                    "a runtime export table cannot be installed without a runtime-system import",
                ));
            }
            (None, None, true) => {
                return Err(RuntimeStateError::new(
                    "runtime-system imports require an installed runtime export table",
                ));
            }
            (None, Some(_), true) => {
                return Err(RuntimeStateError::new(
                    "new runtime export table does not own every planned runtime endpoint",
                ));
            }
            (Some(_), Some(_), _) => {
                return Err(RuntimeStateError::new(
                    "runtime export table is already installed",
                ));
            }
        }
        self.module_graph.insert_checked_dependencies(
            &dependency_edges
                .iter()
                .map(|edge| (edge.requester, edge.dependency))
                .collect::<Vec<_>>(),
        );
        let requester_module_id = binding.requester_module_id;
        if let Some(runtime_system_exports) = runtime_system_exports {
            self.runtime_system_exports = Some(runtime_system_exports);
        }
        self.module_import_bindings
            .insert(requester_module_id, binding);
        Ok(self
            .module_import_bindings
            .get(&requester_module_id)
            .expect("committed guest import binding disappeared"))
    }
}

fn validate_dll_return_sentinel(return_sentinel: GuestAddress) -> Result<(), RuntimeStateError> {
    let end = u64::from(CALLBACK_RETURN_ARENA_BASE.0) + CALLBACK_RETURN_ARENA_SIZE;
    if return_sentinel.0 < CALLBACK_RETURN_ARENA_BASE.0 || u64::from(return_sentinel.0) >= end {
        return Err(RuntimeStateError::new(
            "DLL callback return sentinel is outside the runtime callback-control arena",
        ));
    }
    Ok(())
}

pub(super) fn runtime_module_graph_error(error: RuntimeModuleGraphError) -> RuntimeStateError {
    RuntimeStateError::new(error.to_string())
}

pub(super) fn runtime_module_image_state_error(
    error: RuntimeStateError,
) -> RuntimeModuleImageError {
    RuntimeModuleImageError::diagnostic(error.to_string())
}

pub(super) fn runtime_module_image_error(error: RuntimeModuleImageError) -> RuntimeStateError {
    RuntimeStateError::new(error.to_string())
}

fn next_runtime_module_handle(
    records: &BTreeMap<u32, RuntimeModule>,
) -> Result<u32, RuntimeStateError> {
    let mut maximum_slot = 0_u16;
    for module in records.values() {
        let handle = RuntimeHandle(module.handle);
        if handle.kind()? != RuntimeObjectKind::Module || handle.slot() == 0 {
            return Err(RuntimeStateError::new(
                "registered module records contain a non-module or zero-slot logical handle",
            ));
        }
        maximum_slot = maximum_slot.max(handle.slot());
    }
    let slot = maximum_slot.checked_add(1).ok_or_else(|| {
        RuntimeStateError::new("logical runtime module handle slots are exhausted")
    })?;
    RuntimeHandle::from_parts(
        RuntimeObjectKind::Module,
        RUNTIME_MODULE_HANDLE_GENERATION,
        slot,
    )
    .map(|handle| handle.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAIN_HANDLE: u32 = 0x4d01_0001;
    const DLL_HANDLE: u32 = 0x4d01_0002;

    fn module(handle: u32, name: &str) -> RuntimeModule {
        RuntimeModule {
            handle,
            requested_name: name.to_owned(),
            canonical_name: name.to_owned(),
            runtime_system_module: false,
        }
    }

    #[test]
    fn registration_publishes_matching_record_graph_and_module_handle() {
        let mut modules = RuntimeModules::default();
        let mut objects = RuntimeObjectTable::default();
        modules
            .register(
                &mut objects,
                module(MAIN_HANDLE, "game.exe"),
                RuntimeModuleClass::MainExecutable,
            )
            .unwrap();
        assert_eq!(
            modules.module(MAIN_HANDLE).unwrap().canonical_name,
            "game.exe"
        );
        assert_eq!(
            modules
                .graph()
                .module_by_handle(MAIN_HANDLE)
                .unwrap()
                .canonical_name,
            "game.exe"
        );
        assert_eq!(
            objects
                .object_for_handle(RuntimeHandle(MAIN_HANDLE), RuntimeObjectKind::Module)
                .unwrap()
                .key,
            RuntimeObjectKey::Module {
                handle: MAIN_HANDLE
            }
        );
    }

    #[test]
    fn rejected_registration_preserves_records_graph_and_objects() {
        let mut modules = RuntimeModules::default();
        let mut objects = RuntimeObjectTable::default();
        modules
            .register(
                &mut objects,
                module(MAIN_HANDLE, "game.exe"),
                RuntimeModuleClass::MainExecutable,
            )
            .unwrap();
        let main_id = modules.graph().module_by_handle(MAIN_HANDLE).unwrap().id;
        let main_object = objects
            .object_for_handle(RuntimeHandle(MAIN_HANDLE), RuntimeObjectKind::Module)
            .unwrap()
            .id;
        assert!(
            modules
                .register(
                    &mut objects,
                    module(DLL_HANDLE, "GAME.EXE"),
                    RuntimeModuleClass::GuestDynamicLibrary
                )
                .is_err()
        );
        assert_eq!(modules.modules().len(), 1);
        assert_eq!(modules.graph().nodes().len(), 1);
        assert_eq!(objects.objects().len(), 1);
        assert_eq!(
            modules.graph().module_by_handle(MAIN_HANDLE).unwrap().id,
            main_id
        );
        assert_eq!(
            objects
                .object_for_handle(RuntimeHandle(MAIN_HANDLE), RuntimeObjectKind::Module)
                .unwrap()
                .id,
            main_object
        );
        assert!(modules.module(DLL_HANDLE).is_err());
        modules
            .register(
                &mut objects,
                module(DLL_HANDLE, "engine.dll"),
                RuntimeModuleClass::GuestDynamicLibrary,
            )
            .unwrap();
        assert_eq!(modules.modules().len(), 2);
        assert_eq!(modules.graph().nodes().len(), 2);
        assert_eq!(objects.objects().len(), 2);
    }

    #[test]
    fn graph_mutations_resolve_only_owned_module_records() {
        let mut modules = RuntimeModules::default();
        let mut objects = RuntimeObjectTable::default();
        for (record, class) in [
            (
                module(MAIN_HANDLE, "game.exe"),
                RuntimeModuleClass::MainExecutable,
            ),
            (
                module(DLL_HANDLE, "engine.dll"),
                RuntimeModuleClass::GuestDynamicLibrary,
            ),
        ] {
            modules.register(&mut objects, record, class).unwrap();
        }

        modules.add_dependency(MAIN_HANDLE, DLL_HANDLE).unwrap();
        let main = modules.graph().module_by_handle(MAIN_HANDLE).unwrap();
        let dependency = modules.graph().module_by_handle(DLL_HANDLE).unwrap();
        assert!(main.dependencies.contains(&dependency.id));
        assert_eq!(modules.modules().len(), modules.graph().nodes().len());
    }

    #[test]
    fn callback_completion_failure_preserves_pending_identity_until_retry() {
        let frame = RuntimeCallbackFrame {
            id: RuntimeCallbackFrameId(1),
            thread_id: super::super::RuntimeThreadId(8192),
            target: GuestAddress(0x3000_1000),
            return_sentinel: CALLBACK_RETURN_ARENA_BASE,
            entry_last_error: 0,
            scratch: None,
        };
        let mut modules = RuntimeModules {
            dll_lifecycle: RuntimeDllLifecycle {
                phase: RuntimeDllLifecyclePhase::AttachUnwinding,
                operation: Some(RuntimeDllOperation {
                    kind: RuntimeDllOperationKind::AttachUnwind,
                    graph_order: Vec::new(),
                    remaining: VecDeque::new(),
                    completed: Vec::new(),
                    pending: Some(RuntimeDllCallbackRequest {
                        frame,
                        module_id: RuntimeModuleId(1),
                        module_name: "codec.dll".to_owned(),
                    }),
                    attach_failure: Some("engine.dll".to_owned()),
                    return_sentinel: CALLBACK_RETURN_ARENA_BASE,
                }),
            },
            ..RuntimeModules::default()
        };
        let before = modules.dll_lifecycle.clone();
        assert!(
            modules
                .advance_lifecycle(
                    RuntimeDllLifecycleRequest::Complete {
                        frame_id: RuntimeCallbackFrameId(99),
                        result: 1,
                    },
                    |_| panic!("stale callback must fail before publication")
                )
                .is_err()
        );
        assert_eq!(modules.dll_lifecycle, before);
        assert!(
            modules
                .advance_lifecycle(
                    RuntimeDllLifecycleRequest::Complete {
                        frame_id: frame.id,
                        result: 1,
                    },
                    |_| Err(RuntimeStateError::new("fixture callback return failed"))
                )
                .is_err()
        );
        assert_eq!(modules.dll_lifecycle, before);
        let action = modules
            .advance_lifecycle(
                RuntimeDllLifecycleRequest::Complete {
                    frame_id: frame.id,
                    result: 1,
                },
                |spec| {
                    assert!(spec.is_none());
                    Ok(None)
                },
            )
            .unwrap();
        assert_eq!(
            action,
            RuntimeDllLifecycleAction::AttachFailed {
                failed_module: "engine.dll".to_owned()
            }
        );
        assert_eq!(
            modules.dll_lifecycle_phase(),
            RuntimeDllLifecyclePhase::Idle
        );
        assert!(modules.dll_lifecycle.operation.is_none());
    }

    #[test]
    fn active_dll_lifecycle_blocks_graph_mutation_without_rollback() {
        let mut objects = RuntimeObjectTable::default();
        let mut modules = RuntimeModules {
            dll_lifecycle: RuntimeDllLifecycle {
                phase: RuntimeDllLifecyclePhase::ProcessAttaching,
                operation: Some(RuntimeDllOperation {
                    kind: RuntimeDllOperationKind::Attach,
                    graph_order: Vec::new(),
                    remaining: VecDeque::new(),
                    completed: Vec::new(),
                    pending: None,
                    attach_failure: None,
                    return_sentinel: CALLBACK_RETURN_ARENA_BASE,
                }),
            },
            ..RuntimeModules::default()
        };
        assert!(
            modules
                .register(
                    &mut objects,
                    module(MAIN_HANDLE, "game.exe"),
                    RuntimeModuleClass::MainExecutable
                )
                .is_err()
        );
        assert_eq!(
            modules.dll_lifecycle_phase(),
            RuntimeDllLifecyclePhase::ProcessAttaching
        );
        assert!(modules.dll_lifecycle.operation.is_some());
        assert!(modules.modules().is_empty());
        assert!(modules.graph().nodes().is_empty());
        assert!(objects.objects().is_empty());
    }

    #[test]
    fn runtime_system_load_reuses_identity_and_separates_thread_references() {
        let (mut modules, mut objects) = attached_module_owner();
        let first = modules
            .load_runtime_system_module(
                &mut objects,
                "D3DX9_43.DLL".to_owned(),
                "d3dx9_43.dll".to_owned(),
                8192,
            )
            .unwrap();
        assert_eq!(first, 0x4d01_0003);
        let second = modules
            .load_runtime_system_module(
                &mut objects,
                "d3dx9_43.dll".to_owned(),
                "d3dx9_43.dll".to_owned(),
                8193,
            )
            .unwrap();
        assert_eq!(second, first);
        let node = modules.module_graph.module_by_handle(first).unwrap();
        for thread_id in [8192, 8193] {
            assert_eq!(
                node.reference_count_for(
                    &crate::module_graph::RuntimeModuleReferenceOwner::DynamicLoad { thread_id }
                ),
                1
            );
        }
    }

    #[test]
    fn runtime_system_load_rejects_invalid_identity_before_any_owner_changes() {
        for (requested, canonical, thread_id) in [
            ("bad.dll", "bad.dll", 0),
            ("bad.dll", "C:\\bad.dll", 8192),
            ("bad\0.dll", "bad.dll", 8192),
            ("fixture.exe", "fixture.exe", 8192),
        ] {
            let (mut modules, mut objects) = attached_module_owner();
            assert!(
                modules
                    .load_runtime_system_module(
                        &mut objects,
                        requested.to_owned(),
                        canonical.to_owned(),
                        thread_id
                    )
                    .is_err()
            );
            assert_eq!(modules.modules().len(), 2);
            assert_eq!(modules.graph().nodes().len(), 2);
            assert_eq!(objects.objects().len(), 2);
            assert_eq!(
                modules.dll_lifecycle_phase(),
                RuntimeDllLifecyclePhase::ProcessAttached
            );
            assert_eq!(
                modules
                    .graph()
                    .module_by_handle(MAIN_HANDLE)
                    .unwrap()
                    .canonical_name,
                "fixture.exe"
            );
            assert_eq!(
                modules
                    .graph()
                    .module_by_handle(DLL_HANDLE)
                    .unwrap()
                    .canonical_name,
                "kernel32.dll"
            );
            let repaired = modules
                .load_runtime_system_module(
                    &mut objects,
                    "D3DX9_43.DLL".to_owned(),
                    "d3dx9_43.dll".to_owned(),
                    8192,
                )
                .unwrap();
            assert_eq!(repaired, 0x4d01_0003);
            assert_eq!(modules.modules().len(), 3);
            assert_eq!(objects.objects().len(), 3);
        }
    }

    fn attached_module_owner() -> (RuntimeModules, RuntimeObjectTable) {
        let mut modules = RuntimeModules::default();
        let mut objects = RuntimeObjectTable::default();
        for (record, class) in [
            (
                module(MAIN_HANDLE, "fixture.exe"),
                RuntimeModuleClass::MainExecutable,
            ),
            (
                RuntimeModule {
                    runtime_system_module: true,
                    ..module(DLL_HANDLE, "kernel32.dll")
                },
                RuntimeModuleClass::RuntimeSystemModule,
            ),
        ] {
            modules.register(&mut objects, record, class).unwrap();
        }
        modules.add_dependency(MAIN_HANDLE, DLL_HANDLE).unwrap();
        modules.mark_ready(MAIN_HANDLE).unwrap();
        modules.mark_ready(DLL_HANDLE).unwrap();
        modules.attach_all().unwrap();
        modules.dll_lifecycle.phase = RuntimeDllLifecyclePhase::ProcessAttached;
        (modules, objects)
    }
}

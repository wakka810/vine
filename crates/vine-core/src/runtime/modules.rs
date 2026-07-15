use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::Serialize;

use super::{
    RuntimeCallbackFrame, RuntimeCallbackFrameId, RuntimeStateError, RuntimeStateErrorClass,
};
use crate::callbacks::{CALLBACK_RETURN_ARENA_BASE, CALLBACK_RETURN_ARENA_SIZE};
use crate::imports::{ImportThunkArena, ImportThunkId, RuntimeExport};
use crate::materialize::ImageMaterialization;
use crate::memory::{GuestAddress, GuestMemory, MemoryInvalidation, MemoryOwnerId};
use crate::module_graph::{
    RuntimeModuleClass, RuntimeModuleGraph, RuntimeModuleGraphError, RuntimeModuleGraphErrorClass,
    RuntimeModuleId, RuntimeModuleLoadState, RuntimeModuleRegistration,
};
use crate::module_images::{
    RuntimeModuleImage, RuntimeModuleImageError, RuntimeModuleImageErrorClass,
    RuntimeModuleImageRegistry, RuntimeResolvedExport,
};
use crate::pe32::{ExportLookup, ImportTarget, LoaderIngressReport};
use crate::snapshot::ModuleIdentity;

pub const LOGICAL_LOADED_MODULE_NAMESPACE: &str = "vine.logical-loaded-module.v1";
pub const DLL_PROCESS_DETACH: u32 = 0;
pub const DLL_PROCESS_ATTACH: u32 = 1;
pub const DLL_PROCESS_STATIC_RESERVED: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeModule {
    pub owner: String,
    pub variant_id: String,
    pub namespace: String,
    pub handle: u32,
    pub requested_name: String,
    pub canonical_name: String,
    pub runtime_system_module: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGuestImport {
    pub requested_module_id: RuntimeModuleId,
    pub requested_module: String,
    #[serde(flatten)]
    pub target: ImportTarget,
    pub iat_address: GuestAddress,
    pub original_iat_value: u32,
    pub resolution: RuntimeGuestImportResolution,
}

impl RuntimeGuestImport {
    pub fn resolved_address(&self) -> GuestAddress {
        match &self.resolution {
            RuntimeGuestImportResolution::GuestExport { export } => export.address,
            RuntimeGuestImportResolution::RuntimeExport { endpoint } => endpoint.address,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RuntimeGuestImportResolution {
    GuestExport {
        export: RuntimeResolvedExport,
    },
    RuntimeExport {
        endpoint: RuntimeSystemExportEndpoint,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSystemExportEndpoint {
    pub id: ImportThunkId,
    pub address: GuestAddress,
    pub export: RuntimeExport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSystemExportTable {
    pub arena: ImportThunkArena,
    pub endpoints: Vec<RuntimeSystemExportEndpoint>,
}

impl RuntimeSystemExportTable {
    pub fn endpoint(
        &self,
        module: &str,
        target: &ImportTarget,
    ) -> Option<&RuntimeSystemExportEndpoint> {
        self.endpoints
            .iter()
            .find(|endpoint| runtime_export_matches_import(&endpoint.export, module, target))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeModuleDependencyEdge {
    pub requester: RuntimeModuleId,
    pub dependency: RuntimeModuleId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGuestImportBinding {
    pub requester_module_id: RuntimeModuleId,
    pub requester_handle: u32,
    pub source: ModuleIdentity,
    pub image_owner: MemoryOwnerId,
    pub image_load_base: GuestAddress,
    pub imports: Vec<RuntimeGuestImport>,
    pub dependency_edges: Vec<RuntimeModuleDependencyEdge>,
    pub image_owner_sealed: bool,
    pub invalidation: MemoryInvalidation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeDllLifecyclePhase {
    Idle,
    ProcessAttaching,
    AttachUnwinding,
    ProcessAttached,
    ProcessDetaching,
    ProcessDetached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeDllCallbackReason {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDllCallbackRequest {
    pub frame: RuntimeCallbackFrame,
    pub module_id: RuntimeModuleId,
    pub module_handle: u32,
    pub module_name: String,
    pub module_base: GuestAddress,
    pub entry_point: GuestAddress,
    pub reason: RuntimeDllCallbackReason,
    pub reserved: u32,
}

impl RuntimeDllCallbackRequest {
    pub fn arguments(&self) -> [u32; 3] {
        [self.module_base.0, self.reason.value(), self.reserved]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDllLifecycleCompletion {
    pub phase: RuntimeDllLifecyclePhase,
    pub graph_order: Vec<RuntimeModuleId>,
    pub callback_order: Vec<RuntimeModuleId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDllAttachFailure {
    pub failed_module_id: RuntimeModuleId,
    pub failed_module: String,
    pub unwound_modules: Vec<RuntimeModuleId>,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum RuntimeDllLifecycleAction {
    Callback {
        request: RuntimeDllCallbackRequest,
    },
    Complete {
        completion: RuntimeDllLifecycleCompletion,
    },
    AttachFailed {
        failure: RuntimeDllAttachFailure,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeModuleRecords {
    modules: BTreeMap<u32, RuntimeModule>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeModules {
    module_graph: RuntimeModuleGraph,
    #[serde(skip)]
    module_images: RuntimeModuleImageRegistry,
    #[serde(skip)]
    module_import_bindings: BTreeMap<RuntimeModuleId, RuntimeGuestImportBinding>,
    #[serde(skip)]
    runtime_system_exports: Option<RuntimeSystemExportTable>,
    #[serde(skip)]
    dll_lifecycle: RuntimeDllLifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeModuleRegistrationPlan {
    module: RuntimeModule,
    previous_graph: RuntimeModuleGraph,
    next_graph: RuntimeModuleGraph,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGuestImportBindingPlan {
    requester_module_id: RuntimeModuleId,
    requester_handle: u32,
    source: ModuleIdentity,
    image_owner: MemoryOwnerId,
    image_load_base: GuestAddress,
    pub(crate) imports: Vec<RuntimeGuestImport>,
    dependency_edges: Vec<RuntimeModuleDependencyEdge>,
    previous_graph: RuntimeModuleGraph,
    next_graph: RuntimeModuleGraph,
    previous_runtime_system_exports: Option<RuntimeSystemExportTable>,
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
    Detach,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeDllOperation {
    kind: RuntimeDllOperationKind,
    graph_order: Vec<RuntimeModuleId>,
    remaining: VecDeque<RuntimeModuleId>,
    completed: Vec<RuntimeModuleId>,
    pending: Option<RuntimeDllCallbackRequest>,
    attach_failure: Option<(RuntimeModuleId, String)>,
    return_sentinel: GuestAddress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeDllCallbackSpec {
    module_id: RuntimeModuleId,
    module_handle: u32,
    module_name: String,
    module_base: GuestAddress,
    entry_point: GuestAddress,
    reason: RuntimeDllCallbackReason,
    reserved: u32,
    return_sentinel: GuestAddress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeDllPlannedAction {
    Callback {
        spec: RuntimeDllCallbackSpec,
    },
    Complete {
        completion: RuntimeDllLifecycleCompletion,
    },
    AttachFailed {
        failure: RuntimeDllAttachFailure,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeDllLifecyclePlan {
    previous: RuntimeDllLifecycle,
    previous_graph: Option<RuntimeModuleGraph>,
    next: RuntimeDllLifecycle,
    next_graph: Option<RuntimeModuleGraph>,
    action: RuntimeDllPlannedAction,
}

impl RuntimeDllLifecyclePlan {
    pub(super) fn callback_entry(&self) -> Option<(GuestAddress, GuestAddress)> {
        match &self.action {
            RuntimeDllPlannedAction::Callback { spec } => {
                Some((spec.entry_point, spec.return_sentinel))
            }
            RuntimeDllPlannedAction::Complete { .. }
            | RuntimeDllPlannedAction::AttachFailed { .. } => None,
        }
    }
}

impl RuntimeModuleRecords {
    pub(super) fn modules(&self) -> &BTreeMap<u32, RuntimeModule> {
        &self.modules
    }

    pub(super) fn module(&self, handle: u32) -> Result<&RuntimeModule, RuntimeStateError> {
        self.modules.get(&handle).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-module",
                "module-handle-unknown",
                format!("logical module handle {handle:#010x} is not loaded"),
            )
        })
    }

    fn validate_registration(&self, module: &RuntimeModule) -> Result<(), RuntimeStateError> {
        if module.owner.is_empty()
            || module.variant_id.is_empty()
            || module.namespace != LOGICAL_LOADED_MODULE_NAMESPACE
            || module.handle == 0
            || module.canonical_name.is_empty()
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-module",
                "module-identity-invalid",
                "runtime module requires controlled owner, variant, namespace, handle, and name",
            ));
        }
        if self.modules.contains_key(&module.handle)
            || self.modules.values().any(|loaded| {
                loaded
                    .canonical_name
                    .eq_ignore_ascii_case(&module.canonical_name)
            })
        {
            return Err(RuntimeStateError::conflict(
                "runtime-module",
                "module-identity-duplicate",
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
    pub(super) fn graph(&self) -> &RuntimeModuleGraph {
        &self.module_graph
    }

    pub(super) fn images(&self) -> &RuntimeModuleImageRegistry {
        &self.module_images
    }

    pub(super) fn bindings(&self) -> &BTreeMap<RuntimeModuleId, RuntimeGuestImportBinding> {
        &self.module_import_bindings
    }

    pub(super) fn runtime_system_exports(&self) -> Option<&RuntimeSystemExportTable> {
        self.runtime_system_exports.as_ref()
    }

    pub(super) fn dll_lifecycle_phase(&self) -> RuntimeDllLifecyclePhase {
        self.dll_lifecycle.phase
    }

    pub(super) fn require_graph_mutation_allowed(&self) -> Result<(), RuntimeStateError> {
        if self.dll_lifecycle.operation.is_some() {
            return Err(RuntimeStateError::conflict(
                "runtime-dll-lifecycle",
                "module-graph-mutation-during-dll-callback-forbidden",
                "module graph mutation is not supported while DLL callbacks are in progress",
            ));
        }
        Ok(())
    }

    pub(super) fn require_process_attach_start(&self) -> Result<(), RuntimeStateError> {
        if self.dll_lifecycle.phase != RuntimeDllLifecyclePhase::Idle
            || self.dll_lifecycle.operation.is_some()
        {
            return Err(RuntimeStateError::conflict(
                "runtime-dll-lifecycle",
                "dll-process-attach-state-invalid",
                "DLL process attach requires an idle lifecycle coordinator",
            ));
        }
        Ok(())
    }

    pub(super) fn require_process_detach_start(&self) -> Result<(), RuntimeStateError> {
        if self.dll_lifecycle.phase != RuntimeDllLifecyclePhase::ProcessAttached
            || self.dll_lifecycle.operation.is_some()
        {
            return Err(RuntimeStateError::conflict(
                "runtime-dll-lifecycle",
                "dll-process-detach-state-invalid",
                "DLL process detach requires a completed process attach",
            ));
        }
        Ok(())
    }

    pub(super) fn plan_process_attach(
        &self,
        return_sentinel: GuestAddress,
    ) -> Result<RuntimeDllLifecyclePlan, RuntimeStateError> {
        self.require_process_attach_start()?;
        validate_dll_return_sentinel(return_sentinel)?;
        let graph_order = self
            .module_graph
            .planned_attach_order()
            .map_err(runtime_module_graph_error)?;
        let callback_order = self.dll_callback_order(&graph_order)?;
        let previous = self.dll_lifecycle.clone();
        let next = RuntimeDllLifecycle {
            phase: RuntimeDllLifecyclePhase::ProcessAttaching,
            operation: Some(RuntimeDllOperation {
                kind: RuntimeDllOperationKind::Attach,
                graph_order,
                remaining: callback_order.into(),
                completed: Vec::new(),
                pending: None,
                attach_failure: None,
                return_sentinel,
            }),
        };
        self.plan_next_dll_action(previous, next)
    }

    pub(super) fn plan_process_detach(
        &self,
        return_sentinel: GuestAddress,
    ) -> Result<RuntimeDllLifecyclePlan, RuntimeStateError> {
        self.require_process_detach_start()?;
        validate_dll_return_sentinel(return_sentinel)?;
        let graph_order = self
            .module_graph
            .planned_detach_order()
            .map_err(runtime_module_graph_error)?;
        let callback_order = self.dll_callback_order(&graph_order)?;
        let previous = self.dll_lifecycle.clone();
        let next = RuntimeDllLifecycle {
            phase: RuntimeDllLifecyclePhase::ProcessDetaching,
            operation: Some(RuntimeDllOperation {
                kind: RuntimeDllOperationKind::Detach,
                graph_order,
                remaining: callback_order.into(),
                completed: Vec::new(),
                pending: None,
                attach_failure: None,
                return_sentinel,
            }),
        };
        self.plan_next_dll_action(previous, next)
    }

    pub(super) fn plan_callback_completion(
        &self,
        frame_id: RuntimeCallbackFrameId,
        result: u32,
    ) -> Result<RuntimeDllLifecyclePlan, RuntimeStateError> {
        let pending = self
            .dll_lifecycle
            .operation
            .as_ref()
            .and_then(|operation| operation.pending.as_ref())
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-dll-lifecycle",
                    "dll-callback-not-pending",
                    "DLL lifecycle coordinator has no pending callback",
                )
            })?;
        if pending.frame.id != frame_id {
            return Err(RuntimeStateError::conflict(
                "runtime-dll-lifecycle",
                "dll-callback-frame-mismatch",
                "DLL callback completion does not identify the pending callback frame",
            ));
        }

        let previous = self.dll_lifecycle.clone();
        let mut next = previous.clone();
        let mut operation = next.operation.take().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-dll-lifecycle",
                "dll-operation-disappeared",
                "validated DLL lifecycle operation disappeared before completion planning",
            )
        })?;
        let pending = operation.pending.take().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-dll-lifecycle",
                "dll-pending-callback-disappeared",
                "validated DLL callback disappeared before completion planning",
            )
        })?;
        match operation.kind {
            RuntimeDllOperationKind::Attach if result == 0 => {
                operation.kind = RuntimeDllOperationKind::AttachUnwind;
                operation.attach_failure = Some((pending.module_id, pending.module_name));
                operation.remaining = operation.completed.iter().rev().copied().collect();
                operation.completed.clear();
                next.phase = RuntimeDllLifecyclePhase::AttachUnwinding;
            }
            RuntimeDllOperationKind::Attach
            | RuntimeDllOperationKind::AttachUnwind
            | RuntimeDllOperationKind::Detach => {
                operation.completed.push(pending.module_id);
            }
        }
        next.operation = Some(operation);
        self.plan_next_dll_action(previous, next)
    }

    pub(super) fn validate_lifecycle_plan(
        &self,
        plan: &RuntimeDllLifecyclePlan,
    ) -> Result<(), RuntimeStateError> {
        if self.dll_lifecycle != plan.previous {
            return Err(RuntimeStateError::conflict(
                "runtime-dll-lifecycle",
                "dll-lifecycle-plan-stale",
                "DLL lifecycle state changed after transition planning",
            ));
        }
        if let Some(previous_graph) = &plan.previous_graph
            && &self.module_graph != previous_graph
        {
            return Err(RuntimeStateError::conflict(
                "runtime-dll-lifecycle",
                "dll-lifecycle-graph-plan-stale",
                "module graph changed after DLL lifecycle transition planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_lifecycle_plan(
        &mut self,
        mut plan: RuntimeDllLifecyclePlan,
        frame: Option<RuntimeCallbackFrame>,
    ) -> RuntimeDllLifecycleAction {
        debug_assert_eq!(self.dll_lifecycle, plan.previous);
        if let Some(previous_graph) = &plan.previous_graph {
            debug_assert_eq!(&self.module_graph, previous_graph);
        }
        let action = match plan.action {
            RuntimeDllPlannedAction::Callback { spec } => {
                let frame = frame.expect("planned DLL callback requires a callback frame");
                debug_assert_eq!(frame.target, spec.entry_point);
                debug_assert_eq!(frame.return_sentinel, spec.return_sentinel);
                let request = RuntimeDllCallbackRequest {
                    frame,
                    module_id: spec.module_id,
                    module_handle: spec.module_handle,
                    module_name: spec.module_name,
                    module_base: spec.module_base,
                    entry_point: spec.entry_point,
                    reason: spec.reason,
                    reserved: spec.reserved,
                };
                plan.next
                    .operation
                    .as_mut()
                    .expect("planned DLL callback lost its lifecycle operation")
                    .pending = Some(request.clone());
                RuntimeDllLifecycleAction::Callback { request }
            }
            RuntimeDllPlannedAction::Complete { completion } => {
                debug_assert!(frame.is_none());
                RuntimeDllLifecycleAction::Complete { completion }
            }
            RuntimeDllPlannedAction::AttachFailed { failure } => {
                debug_assert!(frame.is_none());
                RuntimeDllLifecycleAction::AttachFailed { failure }
            }
        };
        if let Some(next_graph) = plan.next_graph {
            self.module_graph = next_graph;
        }
        self.dll_lifecycle = plan.next;
        action
    }

    fn plan_next_dll_action(
        &self,
        previous: RuntimeDllLifecycle,
        mut next: RuntimeDllLifecycle,
    ) -> Result<RuntimeDllLifecyclePlan, RuntimeStateError> {
        let callback = {
            let operation = next.operation.as_mut().ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-dll-lifecycle",
                    "dll-operation-missing",
                    "DLL lifecycle planning requires an active operation",
                )
            })?;
            operation.remaining.pop_front().map(|module_id| {
                let reason = match operation.kind {
                    RuntimeDllOperationKind::Attach => RuntimeDllCallbackReason::ProcessAttach,
                    RuntimeDllOperationKind::AttachUnwind | RuntimeDllOperationKind::Detach => {
                        RuntimeDllCallbackReason::ProcessDetach
                    }
                };
                (module_id, reason, operation.return_sentinel)
            })
        };
        if let Some((module_id, reason, return_sentinel)) = callback {
            let spec = self.dll_callback_spec(module_id, reason, return_sentinel)?;
            return Ok(RuntimeDllLifecyclePlan {
                previous,
                previous_graph: None,
                next,
                next_graph: None,
                action: RuntimeDllPlannedAction::Callback { spec },
            });
        }

        let operation = next.operation.take().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-dll-lifecycle",
                "dll-operation-missing",
                "DLL lifecycle finalization requires an active operation",
            )
        })?;
        match operation.kind {
            RuntimeDllOperationKind::Attach => {
                let previous_graph = self.module_graph.clone();
                let mut next_graph = previous_graph.clone();
                let graph_order = next_graph
                    .attach_all()
                    .map_err(runtime_module_graph_error)?;
                if graph_order != operation.graph_order {
                    return Err(RuntimeStateError::conflict(
                        "runtime-dll-lifecycle",
                        "dll-lifecycle-graph-order-changed",
                        "module graph order changed while DLL callbacks were in progress",
                    ));
                }
                next.phase = RuntimeDllLifecyclePhase::ProcessAttached;
                Ok(RuntimeDllLifecyclePlan {
                    previous,
                    previous_graph: Some(previous_graph),
                    next,
                    next_graph: Some(next_graph),
                    action: RuntimeDllPlannedAction::Complete {
                        completion: RuntimeDllLifecycleCompletion {
                            phase: RuntimeDllLifecyclePhase::ProcessAttached,
                            graph_order,
                            callback_order: operation.completed,
                        },
                    },
                })
            }
            RuntimeDllOperationKind::AttachUnwind => {
                let (failed_module_id, failed_module) =
                    operation.attach_failure.ok_or_else(|| {
                        RuntimeStateError::runtime_defect(
                            "runtime-dll-lifecycle",
                            "dll-attach-failure-identity-missing",
                            "DLL attach unwind lost the failing module identity",
                        )
                    })?;
                next.phase = RuntimeDllLifecyclePhase::Idle;
                Ok(RuntimeDllLifecyclePlan {
                    previous,
                    previous_graph: None,
                    next,
                    next_graph: None,
                    action: RuntimeDllPlannedAction::AttachFailed {
                        failure: RuntimeDllAttachFailure {
                            failed_module_id,
                            failed_module,
                            unwound_modules: operation.completed,
                            retryable: true,
                        },
                    },
                })
            }
            RuntimeDllOperationKind::Detach => {
                let previous_graph = self.module_graph.clone();
                let mut next_graph = previous_graph.clone();
                let graph_order = next_graph
                    .detach_all()
                    .map_err(runtime_module_graph_error)?;
                if graph_order != operation.graph_order {
                    return Err(RuntimeStateError::conflict(
                        "runtime-dll-lifecycle",
                        "dll-lifecycle-graph-order-changed",
                        "module graph order changed while DLL callbacks were in progress",
                    ));
                }
                next.phase = RuntimeDllLifecyclePhase::ProcessDetached;
                Ok(RuntimeDllLifecyclePlan {
                    previous,
                    previous_graph: Some(previous_graph),
                    next,
                    next_graph: Some(next_graph),
                    action: RuntimeDllPlannedAction::Complete {
                        completion: RuntimeDllLifecycleCompletion {
                            phase: RuntimeDllLifecyclePhase::ProcessDetached,
                            graph_order,
                            callback_order: operation.completed,
                        },
                    },
                })
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
                    return Err(RuntimeStateError::conflict(
                        "runtime-dll-lifecycle",
                        "dll-module-import-binding-missing",
                        format!(
                            "module {} has no committed import binding",
                            node.canonical_name
                        ),
                    ));
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
            RuntimeStateError::runtime_defect(
                "runtime-dll-lifecycle",
                "dll-callback-entry-point-missing",
                "DLL callback order includes a module without an entry point",
            )
        })?;
        Ok(RuntimeDllCallbackSpec {
            module_id,
            module_handle: node.handle,
            module_name: node.canonical_name.clone(),
            module_base: image.load_base,
            entry_point,
            reason,
            reserved: DLL_PROCESS_STATIC_RESERVED,
            return_sentinel,
        })
    }

    pub(super) fn resolve(
        &self,
        module: &str,
        symbol: ExportLookup,
    ) -> Result<RuntimeResolvedExport, RuntimeModuleImageError> {
        self.module_images.resolve(module, symbol)
    }

    pub(super) fn plan_registration(
        &self,
        records: &RuntimeModuleRecords,
        module: RuntimeModule,
        class: RuntimeModuleClass,
    ) -> Result<RuntimeModuleRegistrationPlan, RuntimeStateError> {
        self.require_graph_mutation_allowed()?;
        records.validate_registration(&module)?;
        let previous_graph = self.module_graph.clone();
        let mut next_graph = previous_graph.clone();
        next_graph
            .register_module(RuntimeModuleRegistration {
                handle: module.handle,
                requested_name: module.requested_name.clone(),
                canonical_name: module.canonical_name.clone(),
                class,
            })
            .map_err(runtime_module_graph_error)?;
        Ok(RuntimeModuleRegistrationPlan {
            module,
            previous_graph,
            next_graph,
        })
    }

    pub(super) fn commit_registration(
        &mut self,
        records: &mut RuntimeModuleRecords,
        plan: RuntimeModuleRegistrationPlan,
    ) -> u32 {
        debug_assert_eq!(self.module_graph, plan.previous_graph);
        self.module_graph = plan.next_graph;
        records.commit_registration(plan.module)
    }

    pub(super) fn module_by_name_handle(&self, name: &str) -> Result<u32, RuntimeStateError> {
        self.module_graph
            .module_by_name(name)
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

    pub(super) fn release_dynamic_reference(
        &mut self,
        handle: u32,
        thread_id: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.require_graph_mutation_allowed()?;
        let module = self.module_id_by_handle(handle)?;
        self.module_graph
            .release_dynamic_reference(module, thread_id)
            .map_err(runtime_module_graph_error)
    }

    pub(super) fn mark_ready(&mut self, handle: u32) -> Result<(), RuntimeStateError> {
        self.require_graph_mutation_allowed()?;
        let module = self.module_id_by_handle(handle)?;
        self.module_graph
            .mark_ready(module)
            .map_err(runtime_module_graph_error)
    }

    pub(super) fn attach_all(&mut self) -> Result<Vec<RuntimeModuleId>, RuntimeStateError> {
        if self.dll_lifecycle.phase != RuntimeDllLifecyclePhase::Idle {
            return Err(RuntimeStateError::conflict(
                "runtime-dll-lifecycle",
                "direct-module-attach-after-dll-lifecycle-forbidden",
                "direct module attach cannot bypass the active DLL lifecycle coordinator",
            ));
        }
        self.require_graph_mutation_allowed()?;
        self.module_graph
            .attach_all()
            .map_err(runtime_module_graph_error)
    }

    pub(super) fn detach_all(&mut self) -> Result<Vec<RuntimeModuleId>, RuntimeStateError> {
        if self.dll_lifecycle.phase != RuntimeDllLifecyclePhase::Idle {
            return Err(RuntimeStateError::conflict(
                "runtime-dll-lifecycle",
                "direct-module-detach-after-dll-lifecycle-forbidden",
                "direct module detach cannot bypass the active DLL lifecycle coordinator",
            ));
        }
        self.require_graph_mutation_allowed()?;
        self.module_graph
            .detach_all()
            .map_err(runtime_module_graph_error)
    }

    pub(super) fn register_image(
        &mut self,
        memory: &GuestMemory,
        handle: u32,
        loader: &LoaderIngressReport,
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

    pub(super) fn guest_import_binding(
        &self,
        handle: u32,
    ) -> Result<&RuntimeGuestImportBinding, RuntimeStateError> {
        let module_id = self.module_id_by_handle(handle)?;
        self.module_import_bindings.get(&module_id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-guest-imports",
                "guest-import-binding-missing",
                format!("module handle {handle:#010x} has no committed guest import binding"),
            )
        })
    }

    pub(super) fn plan_guest_import_binding(
        &self,
        requester_handle: u32,
        source: ModuleIdentity,
        image_owner: MemoryOwnerId,
        image_load_base: GuestAddress,
        imports: Vec<RuntimeGuestImport>,
        dependency_edges: Vec<RuntimeModuleDependencyEdge>,
    ) -> Result<RuntimeGuestImportBindingPlan, RuntimeStateError> {
        self.require_graph_mutation_allowed()?;
        let requester = self
            .module_graph
            .module_by_handle(requester_handle)
            .map_err(runtime_module_graph_error)?;
        if requester.state != RuntimeModuleLoadState::Registered {
            return Err(RuntimeStateError::conflict(
                "runtime-guest-imports",
                "guest-import-requester-state-invalid",
                "guest imports can only bind while the requester module is registered",
            ));
        }
        if self.module_import_bindings.contains_key(&requester.id) {
            return Err(RuntimeStateError::conflict(
                "runtime-guest-imports",
                "guest-import-binding-duplicate",
                "requester module already has a committed guest import binding",
            ));
        }
        if self.module_images.images().contains_key(&requester.id) {
            return Err(RuntimeStateError::conflict(
                "runtime-guest-imports",
                "guest-import-requester-image-already-registered",
                "requester image must bind imports before mapped image registration",
            ));
        }
        if imports
            .iter()
            .any(|import| import.resolved_address().0 == 0)
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-guest-imports",
                "guest-import-target-address-null",
                "resolved guest import address cannot be null",
            ));
        }

        let dependency_edges = dependency_edges
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let previous_graph = self.module_graph.clone();
        let mut next_graph = previous_graph.clone();
        for edge in &dependency_edges {
            let requester_node = next_graph
                .module(edge.requester)
                .map_err(runtime_module_graph_error)?;
            next_graph
                .module(edge.dependency)
                .map_err(runtime_module_graph_error)?;
            if !requester_node.dependencies.contains(&edge.dependency) {
                next_graph
                    .add_dependency(edge.requester, edge.dependency)
                    .map_err(runtime_module_graph_error)?;
            }
        }
        Ok(RuntimeGuestImportBindingPlan {
            requester_module_id: requester.id,
            requester_handle,
            source,
            image_owner,
            image_load_base,
            imports,
            dependency_edges,
            previous_graph,
            next_graph,
            previous_runtime_system_exports: self.runtime_system_exports.clone(),
        })
    }

    pub(super) fn commit_guest_import_binding(
        &mut self,
        plan: RuntimeGuestImportBindingPlan,
        invalidation: MemoryInvalidation,
        runtime_system_exports: Option<RuntimeSystemExportTable>,
    ) -> Result<&RuntimeGuestImportBinding, RuntimeStateError> {
        if self.module_graph != plan.previous_graph {
            return Err(RuntimeStateError::conflict(
                "runtime-guest-imports",
                "guest-import-plan-graph-stale",
                "module graph changed after guest import planning",
            ));
        }
        if self
            .module_import_bindings
            .contains_key(&plan.requester_module_id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-guest-imports",
                "guest-import-binding-duplicate",
                "requester module already has a committed guest import binding",
            ));
        }
        if self.runtime_system_exports != plan.previous_runtime_system_exports {
            return Err(RuntimeStateError::conflict(
                "runtime-guest-imports",
                "guest-import-runtime-export-table-stale",
                "runtime system export table changed after guest import planning",
            ));
        }
        let imports_runtime_exports = plan.imports.iter().any(|import| {
            matches!(
                &import.resolution,
                RuntimeGuestImportResolution::RuntimeExport { .. }
            )
        });
        match (
            plan.previous_runtime_system_exports.as_ref(),
            runtime_system_exports.as_ref(),
            imports_runtime_exports,
        ) {
            (None, Some(table), true)
                if plan.imports.iter().all(|import| match &import.resolution {
                    RuntimeGuestImportResolution::RuntimeExport { endpoint } => {
                        table.endpoints.contains(endpoint)
                    }
                    RuntimeGuestImportResolution::GuestExport { .. } => true,
                }) => {}
            (Some(_), None, _) | (None, None, false) => {}
            (None, Some(_), false) => {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-guest-imports",
                    "guest-import-runtime-export-table-unneeded",
                    "a runtime export table cannot be installed without a runtime-system import",
                ));
            }
            (None, None, true) => {
                return Err(RuntimeStateError::conflict(
                    "runtime-guest-imports",
                    "guest-import-runtime-export-table-missing",
                    "runtime-system imports require an installed runtime export table",
                ));
            }
            (None, Some(_), true) => {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-guest-imports",
                    "guest-import-runtime-export-endpoint-missing",
                    "new runtime export table does not own every planned runtime endpoint",
                ));
            }
            (Some(_), Some(_), _) => {
                return Err(RuntimeStateError::conflict(
                    "runtime-guest-imports",
                    "guest-import-runtime-export-table-duplicate",
                    "runtime export table is already installed",
                ));
            }
        }
        let binding = RuntimeGuestImportBinding {
            requester_module_id: plan.requester_module_id,
            requester_handle: plan.requester_handle,
            source: plan.source,
            image_owner: plan.image_owner,
            image_load_base: plan.image_load_base,
            imports: plan.imports,
            dependency_edges: plan.dependency_edges,
            image_owner_sealed: true,
            invalidation,
        };
        self.module_graph = plan.next_graph;
        if let Some(runtime_system_exports) = runtime_system_exports {
            self.runtime_system_exports = Some(runtime_system_exports);
        }
        self.module_import_bindings
            .insert(plan.requester_module_id, binding);
        Ok(self
            .module_import_bindings
            .get(&plan.requester_module_id)
            .expect("committed guest import binding disappeared"))
    }
}

fn validate_dll_return_sentinel(return_sentinel: GuestAddress) -> Result<(), RuntimeStateError> {
    let end = u64::from(CALLBACK_RETURN_ARENA_BASE.0) + CALLBACK_RETURN_ARENA_SIZE;
    if return_sentinel.0 < CALLBACK_RETURN_ARENA_BASE.0 || u64::from(return_sentinel.0) >= end {
        return Err(RuntimeStateError::invalid_input(
            "runtime-dll-lifecycle",
            "dll-return-sentinel-unowned",
            "DLL callback return sentinel is outside the runtime callback-control arena",
        ));
    }
    Ok(())
}

pub(super) fn runtime_module_graph_error(error: RuntimeModuleGraphError) -> RuntimeStateError {
    match error.class {
        RuntimeModuleGraphErrorClass::InvalidInput => {
            RuntimeStateError::invalid_input("runtime-module-graph", error.code, error.message)
        }
        RuntimeModuleGraphErrorClass::NotFound => {
            RuntimeStateError::not_found("runtime-module-graph", error.code, error.message)
        }
        RuntimeModuleGraphErrorClass::Conflict => {
            RuntimeStateError::conflict("runtime-module-graph", error.code, error.message)
        }
        RuntimeModuleGraphErrorClass::ResourceLimit => {
            RuntimeStateError::resource_limit("runtime-module-graph", error.code, error.message)
        }
        RuntimeModuleGraphErrorClass::RuntimeDefect => {
            RuntimeStateError::runtime_defect("runtime-module-graph", error.code, error.message)
        }
    }
}

pub(super) fn runtime_module_image_state_error(
    error: RuntimeStateError,
) -> RuntimeModuleImageError {
    let class = match error.class {
        RuntimeStateErrorClass::InvalidInput => RuntimeModuleImageErrorClass::InvalidInput,
        RuntimeStateErrorClass::NotFound => RuntimeModuleImageErrorClass::NotFound,
        RuntimeStateErrorClass::Conflict => RuntimeModuleImageErrorClass::Conflict,
        RuntimeStateErrorClass::ResourceLimit => RuntimeModuleImageErrorClass::ResourceLimit,
        RuntimeStateErrorClass::RuntimeDefect => RuntimeModuleImageErrorClass::RuntimeDefect,
    };
    RuntimeModuleImageError {
        class,
        code: error.code,
        message: error.message,
    }
}

pub(super) fn runtime_module_image_error(error: RuntimeModuleImageError) -> RuntimeStateError {
    match error.class {
        RuntimeModuleImageErrorClass::InvalidInput => {
            RuntimeStateError::invalid_input("runtime-module-image", error.code, error.message)
        }
        RuntimeModuleImageErrorClass::NotFound => {
            RuntimeStateError::not_found("runtime-module-image", error.code, error.message)
        }
        RuntimeModuleImageErrorClass::Conflict => {
            RuntimeStateError::conflict("runtime-module-image", error.code, error.message)
        }
        RuntimeModuleImageErrorClass::ResourceLimit => {
            RuntimeStateError::resource_limit("runtime-module-image", error.code, error.message)
        }
        RuntimeModuleImageErrorClass::RuntimeDefect => {
            RuntimeStateError::runtime_defect("runtime-module-image", error.code, error.message)
        }
    }
}

fn runtime_export_matches_import(
    export: &RuntimeExport,
    module: &str,
    target: &ImportTarget,
) -> bool {
    match (export, target) {
        (
            RuntimeExport::Name {
                module: export_module,
                name: export_name,
            },
            ImportTarget::Name { name, .. },
        ) => export_module.eq_ignore_ascii_case(module) && export_name == name,
        (
            RuntimeExport::Ordinal {
                module: export_module,
                ordinal: export_ordinal,
            },
            ImportTarget::Ordinal { ordinal },
        ) => export_module.eq_ignore_ascii_case(module) && export_ordinal == ordinal,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAIN_HANDLE: u32 = 0x4d01_0001;
    const DLL_HANDLE: u32 = 0x4d01_0002;

    fn module(handle: u32, name: &str) -> RuntimeModule {
        RuntimeModule {
            owner: "runtime-loaded-modules".to_owned(),
            variant_id: "controlled-module".to_owned(),
            namespace: LOGICAL_LOADED_MODULE_NAMESPACE.to_owned(),
            handle,
            requested_name: name.to_owned(),
            canonical_name: name.to_owned(),
            runtime_system_module: false,
        }
    }

    #[test]
    fn registration_plan_is_mutation_free_and_serialization_shape_is_stable() {
        let mut records = RuntimeModuleRecords::default();
        let mut modules = RuntimeModules::default();
        let plan = modules
            .plan_registration(
                &records,
                module(MAIN_HANDLE, "game.exe"),
                RuntimeModuleClass::MainExecutable,
            )
            .unwrap();
        assert!(records.modules().is_empty());
        assert!(modules.graph().nodes().is_empty());

        modules.commit_registration(&mut records, plan);
        assert_eq!(
            records.module(MAIN_HANDLE).unwrap().canonical_name,
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
        assert!(
            serde_json::to_value(&records)
                .unwrap()
                .get("modules")
                .is_some()
        );
        assert!(
            serde_json::to_value(&modules)
                .unwrap()
                .get("moduleGraph")
                .is_some()
        );
    }

    #[test]
    fn rejected_registration_preserves_records_and_graph() {
        let mut records = RuntimeModuleRecords::default();
        let mut modules = RuntimeModules::default();
        let plan = modules
            .plan_registration(
                &records,
                module(MAIN_HANDLE, "game.exe"),
                RuntimeModuleClass::MainExecutable,
            )
            .unwrap();
        modules.commit_registration(&mut records, plan);
        let records_before = records.clone();
        let modules_before = modules.clone();

        assert_eq!(
            modules
                .plan_registration(
                    &records,
                    module(DLL_HANDLE, "GAME.EXE"),
                    RuntimeModuleClass::GuestDynamicLibrary,
                )
                .unwrap_err()
                .code,
            "module-identity-duplicate"
        );
        assert_eq!(records, records_before);
        assert_eq!(modules, modules_before);
    }

    #[test]
    fn graph_mutations_resolve_only_owned_module_records() {
        let mut records = RuntimeModuleRecords::default();
        let mut modules = RuntimeModules::default();
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
            let plan = modules.plan_registration(&records, record, class).unwrap();
            modules.commit_registration(&mut records, plan);
        }

        modules.add_dependency(MAIN_HANDLE, DLL_HANDLE).unwrap();
        let main = modules.graph().module_by_handle(MAIN_HANDLE).unwrap();
        let dependency = modules.graph().module_by_handle(DLL_HANDLE).unwrap();
        assert!(main.dependencies.contains(&dependency.id));
        assert_eq!(records.modules().len(), modules.graph().nodes().len());
    }

    #[test]
    fn dll_lifecycle_plan_commits_private_pending_callback_state() {
        let mut modules = RuntimeModules::default();
        let previous = modules.dll_lifecycle.clone();
        let next = RuntimeDllLifecycle {
            phase: RuntimeDllLifecyclePhase::ProcessAttaching,
            operation: Some(RuntimeDllOperation {
                kind: RuntimeDllOperationKind::Attach,
                graph_order: vec![RuntimeModuleId(1)],
                remaining: VecDeque::new(),
                completed: Vec::new(),
                pending: None,
                attach_failure: None,
                return_sentinel: CALLBACK_RETURN_ARENA_BASE,
            }),
        };
        let plan = RuntimeDllLifecyclePlan {
            previous,
            previous_graph: None,
            next,
            next_graph: None,
            action: RuntimeDllPlannedAction::Callback {
                spec: RuntimeDllCallbackSpec {
                    module_id: RuntimeModuleId(1),
                    module_handle: DLL_HANDLE,
                    module_name: "engine.dll".to_owned(),
                    module_base: GuestAddress(0x3000_0000),
                    entry_point: GuestAddress(0x3000_1000),
                    reason: RuntimeDllCallbackReason::ProcessAttach,
                    reserved: DLL_PROCESS_STATIC_RESERVED,
                    return_sentinel: CALLBACK_RETURN_ARENA_BASE,
                },
            },
        };
        modules.validate_lifecycle_plan(&plan).unwrap();
        assert_eq!(
            modules.dll_lifecycle_phase(),
            RuntimeDllLifecyclePhase::Idle
        );
        assert!(
            serde_json::to_value(&modules)
                .unwrap()
                .get("dllLifecycle")
                .is_none()
        );

        let frame = RuntimeCallbackFrame {
            id: RuntimeCallbackFrameId(1),
            thread_id: super::super::RuntimeThreadId(8192),
            target: GuestAddress(0x3000_1000),
            return_sentinel: CALLBACK_RETURN_ARENA_BASE,
            entry_last_error: 0,
        };
        let action = modules.commit_lifecycle_plan(plan, Some(frame));
        let RuntimeDllLifecycleAction::Callback { request } = action else {
            panic!("expected planned DLL callback");
        };
        assert_eq!(request.frame, frame);
        assert_eq!(
            modules.dll_lifecycle_phase(),
            RuntimeDllLifecyclePhase::ProcessAttaching
        );
        assert_eq!(
            modules
                .dll_lifecycle
                .operation
                .as_ref()
                .and_then(|operation| operation.pending.as_ref())
                .map(|pending| pending.frame.id),
            Some(RuntimeCallbackFrameId(1))
        );
    }

    #[test]
    fn active_dll_lifecycle_blocks_graph_mutation_without_rollback() {
        let records = RuntimeModuleRecords::default();
        let modules = RuntimeModules {
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
        let before = modules.clone();
        assert_eq!(
            modules
                .plan_registration(
                    &records,
                    module(MAIN_HANDLE, "game.exe"),
                    RuntimeModuleClass::MainExecutable,
                )
                .unwrap_err()
                .code,
            "module-graph-mutation-during-dll-callback-forbidden"
        );
        assert_eq!(modules, before);
        assert!(records.modules().is_empty());
    }

    #[test]
    fn stale_dll_lifecycle_plan_is_rejected_before_commit() {
        let mut modules = RuntimeModules::default();
        let plan = RuntimeDllLifecyclePlan {
            previous: modules.dll_lifecycle.clone(),
            previous_graph: None,
            next: RuntimeDllLifecycle {
                phase: RuntimeDllLifecyclePhase::Idle,
                operation: None,
            },
            next_graph: None,
            action: RuntimeDllPlannedAction::AttachFailed {
                failure: RuntimeDllAttachFailure {
                    failed_module_id: RuntimeModuleId(1),
                    failed_module: "engine.dll".to_owned(),
                    unwound_modules: Vec::new(),
                    retryable: true,
                },
            },
        };
        modules.dll_lifecycle.phase = RuntimeDllLifecyclePhase::ProcessAttached;
        let before = modules.clone();
        assert_eq!(
            modules.validate_lifecycle_plan(&plan).unwrap_err().code,
            "dll-lifecycle-plan-stale"
        );
        assert_eq!(modules, before);
    }
}

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::{
    LOGICAL_LOADED_MODULE_NAMESPACE, RuntimeGuestImport, RuntimeGuestImportBinding,
    RuntimeGuestImportResolution, RuntimeModule, RuntimeModuleDependencyEdge, RuntimeStateError,
    RuntimeStateErrorClass, RuntimeSystemExportTable,
};
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
use crate::pe32::{ExportLookup, LoaderIngressReport};
use crate::snapshot::ModuleIdentity;

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

    pub(super) fn has_binding(&self, module_id: RuntimeModuleId) -> bool {
        self.module_import_bindings.contains_key(&module_id)
    }

    pub(super) fn image(
        &self,
        module_id: RuntimeModuleId,
    ) -> Result<&RuntimeModuleImage, RuntimeModuleImageError> {
        self.module_images.image(module_id)
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
        let module = self.module_id_by_handle(handle)?;
        self.module_graph
            .release_dynamic_reference(module, thread_id)
            .map_err(runtime_module_graph_error)
    }

    pub(super) fn mark_ready(&mut self, handle: u32) -> Result<(), RuntimeStateError> {
        let module = self.module_id_by_handle(handle)?;
        self.module_graph
            .mark_ready(module)
            .map_err(runtime_module_graph_error)
    }

    pub(super) fn attach_all(&mut self) -> Result<Vec<RuntimeModuleId>, RuntimeStateError> {
        self.module_graph
            .attach_all()
            .map_err(runtime_module_graph_error)
    }

    pub(super) fn detach_all(&mut self) -> Result<Vec<RuntimeModuleId>, RuntimeStateError> {
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
}

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Serialize;

const MAX_CANONICAL_MODULE_NAME_BYTES: usize = 260;
const MAX_REQUESTED_MODULE_NAME_BYTES: usize = 32 * 1024;
pub const RUNTIME_MODULE_GRAPH_MAX_MODULES: usize = 4096;
pub const RUNTIME_MODULE_REFERENCE_OWNER_LIMIT: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeModuleId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeModuleClass {
    MainExecutable,
    GuestDynamicLibrary,
    RuntimeSystemModule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum RuntimeModuleLoadState {
    Registered,
    Ready,
    ProcessAttached { sequence: u32 },
    ProcessDetached { sequence: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "owner", rename_all = "kebab-case")]
pub enum RuntimeModuleReferenceOwner {
    ProcessRoot,
    RuntimeService,
    StaticImport { requester: RuntimeModuleId },
    DynamicLoad { thread_id: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeModuleReference {
    pub owner: RuntimeModuleReferenceOwner,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeModuleRegistration {
    pub handle: u32,
    pub requested_name: String,
    pub canonical_name: String,
    pub class: RuntimeModuleClass,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeModuleNode {
    pub id: RuntimeModuleId,
    pub handle: u32,
    pub requested_name: String,
    pub canonical_name: String,
    pub class: RuntimeModuleClass,
    pub state: RuntimeModuleLoadState,
    pub dependencies: BTreeSet<RuntimeModuleId>,
    pub dependents: BTreeSet<RuntimeModuleId>,
    pub references: Vec<RuntimeModuleReference>,
    pub discovery_sequence: u32,
}

impl RuntimeModuleNode {
    pub fn reference_count(&self) -> u32 {
        self.references.iter().fold(0_u32, |total, reference| {
            total.saturating_add(reference.count)
        })
    }

    pub fn reference_count_for(&self, owner: &RuntimeModuleReferenceOwner) -> u32 {
        self.references
            .iter()
            .find(|reference| &reference.owner == owner)
            .map_or(0, |reference| reference.count)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeSystemModuleGraphLoad {
    pub module_id: RuntimeModuleId,
    pub handle: u32,
    pub newly_registered: bool,
    pub thread_reference_count: u32,
    pub attach_sequence: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeSystemModuleGraphLoadPlan {
    expected_mutation_generation: u64,
    next_mutation_generation: u64,
    expected_node_count: usize,
    expected_main_module: Option<RuntimeModuleId>,
    mutation: RuntimeSystemModuleGraphLoadMutation,
    result: RuntimeSystemModuleGraphLoad,
}

impl RuntimeSystemModuleGraphLoadPlan {
    pub(crate) fn result(&self) -> RuntimeSystemModuleGraphLoad {
        self.result
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeSystemModuleGraphLoadMutation {
    Existing {
        expected: RuntimeModuleNode,
        next: RuntimeModuleNode,
    },
    New {
        registration: RuntimeModuleGraphRegistrationPlan,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeModuleGraphRegistrationPlan {
    expected_mutation_generation: u64,
    next_mutation_generation: u64,
    expected_node_count: usize,
    expected_main_module: Option<RuntimeModuleId>,
    expected_next_module_id: u32,
    expected_next_discovery_sequence: u32,
    next_module_id: u32,
    next_discovery_sequence: u32,
    node: RuntimeModuleNode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeModuleGraph {
    nodes: BTreeMap<RuntimeModuleId, RuntimeModuleNode>,
    #[serde(skip)]
    canonical_index: BTreeMap<String, RuntimeModuleId>,
    main_module: Option<RuntimeModuleId>,
    next_module_id: u32,
    next_discovery_sequence: u32,
    #[serde(skip)]
    mutation_generation: u64,
}

impl Default for RuntimeModuleGraph {
    fn default() -> Self {
        Self {
            nodes: BTreeMap::new(),
            canonical_index: BTreeMap::new(),
            main_module: None,
            next_module_id: 1,
            next_discovery_sequence: 1,
            mutation_generation: 0,
        }
    }
}

impl RuntimeModuleGraph {
    pub fn register_module(
        &mut self,
        registration: RuntimeModuleRegistration,
    ) -> Result<RuntimeModuleId, RuntimeModuleGraphError> {
        let plan = self.plan_registration(registration)?;
        let id = plan.node.id;
        self.commit_registration(plan);
        Ok(id)
    }

    fn plan_registration(
        &self,
        registration: RuntimeModuleRegistration,
    ) -> Result<RuntimeModuleGraphRegistrationPlan, RuntimeModuleGraphError> {
        let canonical_name =
            canonical_module_name(&registration.canonical_name, registration.class)?;
        if registration.handle == 0
            || registration.requested_name.is_empty()
            || registration.requested_name.len() > MAX_REQUESTED_MODULE_NAME_BYTES
        {
            return Err(RuntimeModuleGraphError::invalid_input(
                "module-registration-invalid",
                "module registration requires a nonzero handle and bounded requested name",
            ));
        }
        if registration.requested_name.contains('\0') {
            return Err(RuntimeModuleGraphError::invalid_input(
                "module-requested-name-invalid",
                "requested module name contains NUL",
            ));
        }
        if self.nodes.len() >= RUNTIME_MODULE_GRAPH_MAX_MODULES {
            return Err(RuntimeModuleGraphError::resource_limit(
                "module-count-limit-exceeded",
                "runtime module graph reached its version 1 module limit",
            ));
        }
        if self.canonical_index.contains_key(&canonical_name)
            || self
                .nodes
                .values()
                .any(|node| node.handle == registration.handle)
        {
            return Err(RuntimeModuleGraphError::conflict(
                "module-identity-duplicate",
                "canonical module name or logical handle is already registered",
            ));
        }
        if registration.class == RuntimeModuleClass::MainExecutable && self.main_module.is_some() {
            return Err(RuntimeModuleGraphError::conflict(
                "main-module-duplicate",
                "process module graph already has a main executable",
            ));
        }
        let id = RuntimeModuleId(self.next_module_id);
        let next_module_id = self.next_module_id.checked_add(1).ok_or_else(|| {
            RuntimeModuleGraphError::resource_limit(
                "module-id-space-exhausted",
                "runtime module ID space is exhausted",
            )
        })?;
        let discovery_sequence = self.next_discovery_sequence;
        let next_discovery_sequence = discovery_sequence.checked_add(1).ok_or_else(|| {
            RuntimeModuleGraphError::resource_limit(
                "module-discovery-sequence-overflow",
                "runtime module discovery sequence is exhausted",
            )
        })?;
        let mut node = RuntimeModuleNode {
            id,
            handle: registration.handle,
            requested_name: registration.requested_name,
            canonical_name: canonical_name.clone(),
            class: registration.class,
            state: RuntimeModuleLoadState::Registered,
            dependencies: BTreeSet::new(),
            dependents: BTreeSet::new(),
            references: Vec::new(),
            discovery_sequence,
        };
        match registration.class {
            RuntimeModuleClass::MainExecutable => {
                add_reference_to_node(&mut node, RuntimeModuleReferenceOwner::ProcessRoot)?;
            }
            RuntimeModuleClass::RuntimeSystemModule => {
                add_reference_to_node(&mut node, RuntimeModuleReferenceOwner::RuntimeService)?;
            }
            RuntimeModuleClass::GuestDynamicLibrary => {}
        }
        let next_mutation_generation = self.next_mutation_generation()?;
        Ok(RuntimeModuleGraphRegistrationPlan {
            expected_mutation_generation: self.mutation_generation,
            next_mutation_generation,
            expected_node_count: self.nodes.len(),
            expected_main_module: self.main_module,
            expected_next_module_id: self.next_module_id,
            expected_next_discovery_sequence: self.next_discovery_sequence,
            next_module_id,
            next_discovery_sequence,
            node,
        })
    }

    fn registration_plan_is_current(&self, plan: &RuntimeModuleGraphRegistrationPlan) -> bool {
        self.mutation_generation == plan.expected_mutation_generation
            && self.nodes.len() == plan.expected_node_count
            && self.main_module == plan.expected_main_module
            && self.next_module_id == plan.expected_next_module_id
            && self.next_discovery_sequence == plan.expected_next_discovery_sequence
            && !self.canonical_index.contains_key(&plan.node.canonical_name)
            && !self
                .nodes
                .values()
                .any(|node| node.handle == plan.node.handle)
    }

    fn commit_registration(&mut self, plan: RuntimeModuleGraphRegistrationPlan) {
        debug_assert!(self.registration_plan_is_current(&plan));
        if plan.node.class == RuntimeModuleClass::MainExecutable {
            self.main_module = Some(plan.node.id);
        }
        self.next_module_id = plan.next_module_id;
        self.next_discovery_sequence = plan.next_discovery_sequence;
        self.mutation_generation = plan.next_mutation_generation;
        self.canonical_index
            .insert(plan.node.canonical_name.clone(), plan.node.id);
        self.nodes.insert(plan.node.id, plan.node);
    }

    pub fn rollback_registration(
        &mut self,
        id: RuntimeModuleId,
    ) -> Result<RuntimeModuleNode, RuntimeModuleGraphError> {
        let node = self.module(id)?;
        if node.state != RuntimeModuleLoadState::Registered
            || !node.dependencies.is_empty()
            || !node.dependents.is_empty()
            || id.0.checked_add(1) != Some(self.next_module_id)
            || node.discovery_sequence.checked_add(1) != Some(self.next_discovery_sequence)
            || node.references.iter().any(|reference| {
                !matches!(
                    reference.owner,
                    RuntimeModuleReferenceOwner::ProcessRoot
                        | RuntimeModuleReferenceOwner::RuntimeService
                )
            })
        {
            return Err(RuntimeModuleGraphError::conflict(
                "module-registration-rollback-invalid",
                "only the latest unlinked registered module can be rolled back",
            ));
        }
        let next_mutation_generation = self.next_mutation_generation()?;
        let node = self.nodes.remove(&id).ok_or_else(|| {
            RuntimeModuleGraphError::runtime_defect(
                "module-node-missing",
                "validated module disappeared during registration rollback",
            )
        })?;
        self.canonical_index.remove(&node.canonical_name);
        if self.main_module == Some(id) {
            self.main_module = None;
        }
        self.next_module_id = id.0;
        self.next_discovery_sequence = node.discovery_sequence;
        self.mutation_generation = next_mutation_generation;
        Ok(node)
    }

    pub fn add_dependency(
        &mut self,
        requester: RuntimeModuleId,
        dependency: RuntimeModuleId,
    ) -> Result<(), RuntimeModuleGraphError> {
        if requester == dependency {
            return Err(RuntimeModuleGraphError::conflict(
                "module-dependency-self-cycle",
                "module cannot depend on itself",
            ));
        }
        let requester_node = self.module(requester)?;
        let dependency_node = self.module(dependency)?;
        if requester_node.state != RuntimeModuleLoadState::Registered
            || dependency_node.state != RuntimeModuleLoadState::Registered
        {
            return Err(RuntimeModuleGraphError::conflict(
                "module-dependency-state-invalid",
                "dependency edges must be complete before modules become ready",
            ));
        }
        if requester_node.dependencies.contains(&dependency) {
            return Err(RuntimeModuleGraphError::conflict(
                "module-dependency-duplicate",
                "module dependency edge is already registered",
            ));
        }
        if self.reaches(dependency, requester)? {
            return Err(RuntimeModuleGraphError::conflict(
                "module-dependency-cycle",
                "module dependency edge would create a cycle",
            ));
        }
        let owner = RuntimeModuleReferenceOwner::StaticImport { requester };
        preflight_reference_add(dependency_node, &owner)?;

        let mut next_requester = requester_node.clone();
        let mut next_dependency = dependency_node.clone();
        next_requester.dependencies.insert(dependency);
        next_dependency.dependents.insert(requester);
        add_reference_to_node(&mut next_dependency, owner)?;
        let next_mutation_generation = self.next_mutation_generation()?;
        self.nodes.insert(requester, next_requester);
        self.nodes.insert(dependency, next_dependency);
        self.mutation_generation = next_mutation_generation;
        Ok(())
    }

    pub fn add_dynamic_reference(
        &mut self,
        module: RuntimeModuleId,
        thread_id: u32,
    ) -> Result<u32, RuntimeModuleGraphError> {
        if thread_id == 0 {
            return Err(RuntimeModuleGraphError::invalid_input(
                "module-reference-thread-invalid",
                "dynamic module reference requires a nonzero thread ID",
            ));
        }
        let owner = RuntimeModuleReferenceOwner::DynamicLoad { thread_id };
        let next_mutation_generation = self.next_mutation_generation()?;
        let node = self.module_mut(module)?;
        add_reference_to_node(node, owner.clone())?;
        let count = node.reference_count_for(&owner);
        self.mutation_generation = next_mutation_generation;
        Ok(count)
    }

    pub fn release_dynamic_reference(
        &mut self,
        module: RuntimeModuleId,
        thread_id: u32,
    ) -> Result<u32, RuntimeModuleGraphError> {
        if thread_id == 0 {
            return Err(RuntimeModuleGraphError::invalid_input(
                "module-reference-thread-invalid",
                "dynamic module reference requires a nonzero thread ID",
            ));
        }
        let owner = RuntimeModuleReferenceOwner::DynamicLoad { thread_id };
        let next_mutation_generation = self.next_mutation_generation()?;
        let node = self.module_mut(module)?;
        let index = node
            .references
            .iter()
            .position(|reference| reference.owner == owner)
            .ok_or_else(|| {
                RuntimeModuleGraphError::not_found(
                    "module-reference-unknown",
                    "dynamic module reference owner has no live reference",
                )
            })?;
        let remaining = node.references[index].count.checked_sub(1).ok_or_else(|| {
            RuntimeModuleGraphError::runtime_defect(
                "module-reference-count-underflow",
                "runtime module reference count underflowed",
            )
        })?;
        if remaining == 0 {
            node.references.remove(index);
        } else {
            node.references[index].count = remaining;
        }
        self.mutation_generation = next_mutation_generation;
        Ok(remaining)
    }

    pub fn mark_ready(&mut self, module: RuntimeModuleId) -> Result<(), RuntimeModuleGraphError> {
        if self.module(module)?.state != RuntimeModuleLoadState::Registered {
            return Err(RuntimeModuleGraphError::conflict(
                "module-ready-transition-invalid",
                "only a registered module can become ready",
            ));
        }
        let next_mutation_generation = self.next_mutation_generation()?;
        self.module_mut(module)?.state = RuntimeModuleLoadState::Ready;
        self.mutation_generation = next_mutation_generation;
        Ok(())
    }

    pub fn attach_all(&mut self) -> Result<Vec<RuntimeModuleId>, RuntimeModuleGraphError> {
        let order = self.planned_attach_order()?;
        let transitions = order
            .iter()
            .enumerate()
            .map(|(index, id)| {
                let sequence = u32::try_from(index + 1).map_err(|_| {
                    RuntimeModuleGraphError::resource_limit(
                        "module-attach-sequence-overflow",
                        "module attach sequence does not fit 32-bit accounting",
                    )
                })?;
                Ok((*id, RuntimeModuleLoadState::ProcessAttached { sequence }))
            })
            .collect::<Result<Vec<_>, RuntimeModuleGraphError>>()?;
        let mut next_nodes = self.nodes.clone();
        apply_state_transitions(&mut next_nodes, &transitions)?;
        let next_mutation_generation = self.next_mutation_generation()?;
        self.nodes = next_nodes;
        self.mutation_generation = next_mutation_generation;
        Ok(order)
    }

    pub fn attach_ready_runtime_system_module(
        &mut self,
        module: RuntimeModuleId,
    ) -> Result<u32, RuntimeModuleGraphError> {
        let target = self.module(module)?;
        if target.class != RuntimeModuleClass::RuntimeSystemModule
            || target.state != RuntimeModuleLoadState::Ready
            || !target.dependencies.is_empty()
            || !target.dependents.is_empty()
            || target.reference_count() == 0
        {
            return Err(RuntimeModuleGraphError::conflict(
                "runtime-system-module-attach-state-invalid",
                "dynamic runtime-system attachment requires one ready, referenced, callback-free module",
            ));
        }
        if self.main_module.is_none() {
            return Err(RuntimeModuleGraphError::conflict(
                "main-module-missing",
                "dynamic runtime-system attachment requires an attached main executable",
            ));
        }

        let mut sequences = BTreeSet::new();
        let mut maximum_sequence = 0_u32;
        for node in self.nodes.values().filter(|node| node.id != module) {
            let RuntimeModuleLoadState::ProcessAttached { sequence } = node.state else {
                return Err(RuntimeModuleGraphError::conflict(
                    "runtime-system-module-attach-process-state-invalid",
                    "all existing modules must remain process-attached during a dynamic runtime-system load",
                ));
            };
            if sequence == 0 || !sequences.insert(sequence) {
                return Err(RuntimeModuleGraphError::runtime_defect(
                    "module-attach-sequence-invalid",
                    "attached module graph contains a zero or duplicate process-attach sequence",
                ));
            }
            maximum_sequence = maximum_sequence.max(sequence);
        }
        let sequence = maximum_sequence.checked_add(1).ok_or_else(|| {
            RuntimeModuleGraphError::resource_limit(
                "module-attach-sequence-overflow",
                "dynamic runtime-system attach sequence is exhausted",
            )
        })?;
        let next_mutation_generation = self.next_mutation_generation()?;
        self.module_mut(module)?.state = RuntimeModuleLoadState::ProcessAttached { sequence };
        self.mutation_generation = next_mutation_generation;
        Ok(sequence)
    }

    pub(crate) fn plan_existing_runtime_system_module_load(
        &self,
        module: RuntimeModuleId,
        thread_id: u32,
    ) -> Result<RuntimeSystemModuleGraphLoadPlan, RuntimeModuleGraphError> {
        if thread_id == 0 {
            return Err(RuntimeModuleGraphError::invalid_input(
                "module-reference-thread-invalid",
                "dynamic module reference requires a nonzero thread ID",
            ));
        }
        let expected = self.module(module)?.clone();
        let RuntimeModuleLoadState::ProcessAttached { sequence } = expected.state else {
            return Err(RuntimeModuleGraphError::conflict(
                "runtime-system-module-existing-state-invalid",
                "an existing dynamic runtime-system module must remain process-attached",
            ));
        };
        if expected.class != RuntimeModuleClass::RuntimeSystemModule
            || !expected.dependencies.is_empty()
            || !expected.dependents.is_empty()
        {
            return Err(RuntimeModuleGraphError::conflict(
                "runtime-system-module-existing-class-invalid",
                "an existing dynamic runtime-system module must remain callback-free and dependency-free",
            ));
        }
        self.next_runtime_system_module_attach_sequence()?;
        let owner = RuntimeModuleReferenceOwner::DynamicLoad { thread_id };
        let mut next = expected.clone();
        add_reference_to_node(&mut next, owner.clone())?;
        let thread_reference_count = next.reference_count_for(&owner);
        let next_mutation_generation = self.next_mutation_generation()?;
        Ok(RuntimeSystemModuleGraphLoadPlan {
            expected_mutation_generation: self.mutation_generation,
            next_mutation_generation,
            expected_node_count: self.nodes.len(),
            expected_main_module: self.main_module,
            mutation: RuntimeSystemModuleGraphLoadMutation::Existing { expected, next },
            result: RuntimeSystemModuleGraphLoad {
                module_id: module,
                handle: self.module(module)?.handle,
                newly_registered: false,
                thread_reference_count,
                attach_sequence: sequence,
            },
        })
    }

    pub(crate) fn plan_new_runtime_system_module_load(
        &self,
        registration: RuntimeModuleRegistration,
        thread_id: u32,
    ) -> Result<RuntimeSystemModuleGraphLoadPlan, RuntimeModuleGraphError> {
        if thread_id == 0 {
            return Err(RuntimeModuleGraphError::invalid_input(
                "module-reference-thread-invalid",
                "dynamic module reference requires a nonzero thread ID",
            ));
        }
        if registration.class != RuntimeModuleClass::RuntimeSystemModule {
            return Err(RuntimeModuleGraphError::invalid_input(
                "runtime-system-module-registration-class-invalid",
                "dynamic runtime-system load planning requires a runtime-system module registration",
            ));
        }
        let attach_sequence = self.next_runtime_system_module_attach_sequence()?;
        let mut registration = self.plan_registration(registration)?;
        let owner = RuntimeModuleReferenceOwner::DynamicLoad { thread_id };
        add_reference_to_node(&mut registration.node, owner.clone())?;
        registration.node.state = RuntimeModuleLoadState::ProcessAttached {
            sequence: attach_sequence,
        };
        let result = RuntimeSystemModuleGraphLoad {
            module_id: registration.node.id,
            handle: registration.node.handle,
            newly_registered: true,
            thread_reference_count: registration.node.reference_count_for(&owner),
            attach_sequence,
        };
        Ok(RuntimeSystemModuleGraphLoadPlan {
            expected_mutation_generation: registration.expected_mutation_generation,
            next_mutation_generation: registration.next_mutation_generation,
            expected_node_count: self.nodes.len(),
            expected_main_module: self.main_module,
            mutation: RuntimeSystemModuleGraphLoadMutation::New { registration },
            result,
        })
    }

    pub(crate) fn validate_runtime_system_module_load_plan(
        &self,
        plan: &RuntimeSystemModuleGraphLoadPlan,
    ) -> Result<(), RuntimeModuleGraphError> {
        let current = self.mutation_generation == plan.expected_mutation_generation
            && self.nodes.len() == plan.expected_node_count
            && self.main_module == plan.expected_main_module
            && self
                .next_runtime_system_module_attach_sequence()
                .is_ok_and(|sequence| {
                    !plan.result.newly_registered || sequence == plan.result.attach_sequence
                })
            && match &plan.mutation {
                RuntimeSystemModuleGraphLoadMutation::Existing { expected, .. } => {
                    self.nodes.get(&expected.id) == Some(expected)
                }
                RuntimeSystemModuleGraphLoadMutation::New { registration } => {
                    self.registration_plan_is_current(registration)
                }
            };
        if !current {
            return Err(RuntimeModuleGraphError::conflict(
                "runtime-system-module-load-plan-stale",
                "runtime module graph changed after dynamic system-module load planning",
            ));
        }
        Ok(())
    }

    pub(crate) fn commit_runtime_system_module_load(
        &mut self,
        plan: RuntimeSystemModuleGraphLoadPlan,
    ) -> RuntimeSystemModuleGraphLoad {
        debug_assert!(self.validate_runtime_system_module_load_plan(&plan).is_ok());
        let result = plan.result;
        let next_mutation_generation = plan.next_mutation_generation;
        match plan.mutation {
            RuntimeSystemModuleGraphLoadMutation::Existing { next, .. } => {
                self.nodes.insert(next.id, next);
                self.mutation_generation = next_mutation_generation;
            }
            RuntimeSystemModuleGraphLoadMutation::New { registration } => {
                self.commit_registration(registration);
            }
        }
        result
    }

    pub fn planned_attach_order(&self) -> Result<Vec<RuntimeModuleId>, RuntimeModuleGraphError> {
        if self.main_module.is_none() {
            return Err(RuntimeModuleGraphError::conflict(
                "main-module-missing",
                "module graph cannot attach without a main executable",
            ));
        }
        for node in self.nodes.values() {
            if node.state != RuntimeModuleLoadState::Ready {
                return Err(RuntimeModuleGraphError::conflict(
                    "module-attach-state-invalid",
                    "all modules must be ready before process attach ordering",
                ));
            }
            if node.reference_count() == 0 {
                return Err(RuntimeModuleGraphError::conflict(
                    "module-attach-unreferenced",
                    "unreferenced module cannot participate in process attach ordering",
                ));
            }
        }
        self.dependency_first_order()
    }

    pub fn detach_all(&mut self) -> Result<Vec<RuntimeModuleId>, RuntimeModuleGraphError> {
        let order = self.planned_detach_order()?;
        let transitions = order
            .iter()
            .enumerate()
            .map(|(index, id)| {
                let sequence = u32::try_from(index + 1).map_err(|_| {
                    RuntimeModuleGraphError::resource_limit(
                        "module-detach-sequence-overflow",
                        "module detach sequence does not fit 32-bit accounting",
                    )
                })?;
                Ok((*id, RuntimeModuleLoadState::ProcessDetached { sequence }))
            })
            .collect::<Result<Vec<_>, RuntimeModuleGraphError>>()?;
        let mut next_nodes = self.nodes.clone();
        apply_state_transitions(&mut next_nodes, &transitions)?;
        let next_mutation_generation = self.next_mutation_generation()?;
        self.nodes = next_nodes;
        self.mutation_generation = next_mutation_generation;
        Ok(order)
    }

    pub fn planned_detach_order(&self) -> Result<Vec<RuntimeModuleId>, RuntimeModuleGraphError> {
        let mut attached = self
            .nodes
            .values()
            .map(|node| match node.state {
                RuntimeModuleLoadState::ProcessAttached { sequence } => Ok((sequence, node.id)),
                _ => Err(RuntimeModuleGraphError::conflict(
                    "module-detach-state-invalid",
                    "all modules must be attached before process detach ordering",
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        attached.sort_by(|left, right| right.cmp(left));
        Ok(attached.iter().map(|(_, id)| *id).collect())
    }

    pub fn module(
        &self,
        id: RuntimeModuleId,
    ) -> Result<&RuntimeModuleNode, RuntimeModuleGraphError> {
        self.nodes.get(&id).ok_or_else(|| {
            RuntimeModuleGraphError::not_found(
                "module-id-unknown",
                format!("runtime module ID {} is not registered", id.0),
            )
        })
    }

    pub fn module_by_name(
        &self,
        name: &str,
    ) -> Result<&RuntimeModuleNode, RuntimeModuleGraphError> {
        let canonical = normalize_module_lookup(name)?;
        let id = self.canonical_index.get(&canonical).ok_or_else(|| {
            RuntimeModuleGraphError::not_found(
                "module-name-unknown",
                format!("runtime module {name:?} is not registered"),
            )
        })?;
        self.module(*id)
    }

    pub fn module_by_handle(
        &self,
        handle: u32,
    ) -> Result<&RuntimeModuleNode, RuntimeModuleGraphError> {
        self.nodes
            .values()
            .find(|node| node.handle == handle)
            .ok_or_else(|| {
                RuntimeModuleGraphError::not_found(
                    "module-handle-unknown",
                    format!("runtime module handle {handle:#010x} is not registered"),
                )
            })
    }

    pub fn main_module(&self) -> Option<RuntimeModuleId> {
        self.main_module
    }

    pub fn nodes(&self) -> &BTreeMap<RuntimeModuleId, RuntimeModuleNode> {
        &self.nodes
    }

    fn next_mutation_generation(&self) -> Result<u64, RuntimeModuleGraphError> {
        self.mutation_generation.checked_add(1).ok_or_else(|| {
            RuntimeModuleGraphError::resource_limit(
                "module-graph-mutation-generation-overflow",
                "runtime module graph mutation generation is exhausted",
            )
        })
    }

    fn next_runtime_system_module_attach_sequence(&self) -> Result<u32, RuntimeModuleGraphError> {
        if self.main_module.is_none() {
            return Err(RuntimeModuleGraphError::conflict(
                "main-module-missing",
                "dynamic runtime-system attachment requires an attached main executable",
            ));
        }
        let mut sequences = BTreeSet::new();
        let mut maximum_sequence = 0_u32;
        for node in self.nodes.values() {
            let RuntimeModuleLoadState::ProcessAttached { sequence } = node.state else {
                return Err(RuntimeModuleGraphError::conflict(
                    "runtime-system-module-attach-process-state-invalid",
                    "all existing modules must remain process-attached during a dynamic runtime-system load",
                ));
            };
            if sequence == 0 || !sequences.insert(sequence) {
                return Err(RuntimeModuleGraphError::runtime_defect(
                    "module-attach-sequence-invalid",
                    "attached module graph contains a zero or duplicate process-attach sequence",
                ));
            }
            maximum_sequence = maximum_sequence.max(sequence);
        }
        maximum_sequence.checked_add(1).ok_or_else(|| {
            RuntimeModuleGraphError::resource_limit(
                "module-attach-sequence-overflow",
                "dynamic runtime-system attach sequence is exhausted",
            )
        })
    }

    fn module_mut(
        &mut self,
        id: RuntimeModuleId,
    ) -> Result<&mut RuntimeModuleNode, RuntimeModuleGraphError> {
        self.nodes.get_mut(&id).ok_or_else(|| {
            RuntimeModuleGraphError::not_found(
                "module-id-unknown",
                format!("runtime module ID {} is not registered", id.0),
            )
        })
    }

    fn reaches(
        &self,
        start: RuntimeModuleId,
        target: RuntimeModuleId,
    ) -> Result<bool, RuntimeModuleGraphError> {
        let mut pending = vec![start];
        let mut visited = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if id == target {
                return Ok(true);
            }
            if !visited.insert(id) {
                continue;
            }
            pending.extend(self.module(id)?.dependencies.iter().copied());
        }
        Ok(false)
    }

    fn dependency_first_order(&self) -> Result<Vec<RuntimeModuleId>, RuntimeModuleGraphError> {
        let mut pending_dependencies = self
            .nodes
            .iter()
            .map(|(id, node)| (*id, node.dependencies.len()))
            .collect::<BTreeMap<_, _>>();
        let mut ready = self
            .nodes
            .values()
            .filter(|node| node.dependencies.is_empty())
            .map(|node| {
                (
                    node.canonical_name.clone(),
                    node.discovery_sequence,
                    node.id,
                )
            })
            .collect::<BTreeSet<_>>();
        let mut order = Vec::with_capacity(self.nodes.len());
        while let Some((_, _, id)) = ready.pop_first() {
            order.push(id);
            for dependent in &self.module(id)?.dependents {
                let count = pending_dependencies.get_mut(dependent).ok_or_else(|| {
                    RuntimeModuleGraphError::runtime_defect(
                        "module-order-node-missing",
                        "module dependency references a missing ordering node",
                    )
                })?;
                *count = count.checked_sub(1).ok_or_else(|| {
                    RuntimeModuleGraphError::runtime_defect(
                        "module-order-count-underflow",
                        "module dependency ordering count underflowed",
                    )
                })?;
                if *count == 0 {
                    let node = self.module(*dependent)?;
                    ready.insert((
                        node.canonical_name.clone(),
                        node.discovery_sequence,
                        node.id,
                    ));
                }
            }
        }
        if order.len() != self.nodes.len() {
            return Err(RuntimeModuleGraphError::conflict(
                "module-dependency-cycle",
                "module graph contains a dependency cycle",
            ));
        }
        Ok(order)
    }
}

fn apply_state_transitions(
    nodes: &mut BTreeMap<RuntimeModuleId, RuntimeModuleNode>,
    transitions: &[(RuntimeModuleId, RuntimeModuleLoadState)],
) -> Result<(), RuntimeModuleGraphError> {
    for (id, state) in transitions {
        let node = nodes.get_mut(id).ok_or_else(|| {
            RuntimeModuleGraphError::runtime_defect(
                "module-transition-node-missing",
                "validated module disappeared during load-state transition",
            )
        })?;
        node.state = *state;
    }
    Ok(())
}

fn canonical_module_name(
    name: &str,
    class: RuntimeModuleClass,
) -> Result<String, RuntimeModuleGraphError> {
    let canonical = normalize_module_lookup(name)?;
    let expected_extension = if class == RuntimeModuleClass::MainExecutable {
        ".exe"
    } else {
        ".dll"
    };
    if !canonical.ends_with(expected_extension) {
        return Err(RuntimeModuleGraphError::invalid_input(
            "module-canonical-extension-invalid",
            format!("resolved module identity must end in {expected_extension}"),
        ));
    }
    Ok(canonical)
}

fn normalize_module_lookup(name: &str) -> Result<String, RuntimeModuleGraphError> {
    if name.is_empty()
        || name.len() > MAX_CANONICAL_MODULE_NAME_BYTES
        || !name.is_ascii()
        || name
            .bytes()
            .any(|byte| byte.is_ascii_control() || matches!(byte, b'/' | b'\\' | b':'))
    {
        return Err(RuntimeModuleGraphError::invalid_input(
            "module-canonical-name-invalid",
            "canonical module identity must be a bounded ASCII basename without controls or path separators",
        ));
    }
    Ok(name.to_ascii_lowercase())
}

fn preflight_reference_add(
    node: &RuntimeModuleNode,
    owner: &RuntimeModuleReferenceOwner,
) -> Result<(), RuntimeModuleGraphError> {
    if node.reference_count() == u32::MAX
        || node
            .references
            .iter()
            .find(|reference| &reference.owner == owner)
            .is_some_and(|reference| reference.count == u32::MAX)
        || (node.references.len() >= RUNTIME_MODULE_REFERENCE_OWNER_LIMIT
            && !node
                .references
                .iter()
                .any(|reference| &reference.owner == owner))
    {
        return Err(RuntimeModuleGraphError::resource_limit(
            "module-reference-count-overflow",
            "runtime module reference count is exhausted",
        ));
    }
    Ok(())
}

fn add_reference_to_node(
    node: &mut RuntimeModuleNode,
    owner: RuntimeModuleReferenceOwner,
) -> Result<(), RuntimeModuleGraphError> {
    preflight_reference_add(node, &owner)?;
    if let Some(reference) = node
        .references
        .iter_mut()
        .find(|reference| reference.owner == owner)
    {
        reference.count += 1;
    } else {
        node.references
            .push(RuntimeModuleReference { owner, count: 1 });
        node.references
            .sort_by(|left, right| left.owner.cmp(&right.owner));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeModuleGraphErrorClass {
    InvalidInput,
    NotFound,
    Conflict,
    ResourceLimit,
    RuntimeDefect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeModuleGraphError {
    pub class: RuntimeModuleGraphErrorClass,
    pub code: &'static str,
    pub message: String,
}

impl RuntimeModuleGraphError {
    fn invalid_input(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(RuntimeModuleGraphErrorClass::InvalidInput, code, message)
    }

    fn not_found(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(RuntimeModuleGraphErrorClass::NotFound, code, message)
    }

    fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(RuntimeModuleGraphErrorClass::Conflict, code, message)
    }

    fn resource_limit(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(RuntimeModuleGraphErrorClass::ResourceLimit, code, message)
    }

    fn runtime_defect(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(RuntimeModuleGraphErrorClass::RuntimeDefect, code, message)
    }

    fn new(
        class: RuntimeModuleGraphErrorClass,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            class,
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for RuntimeModuleGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "module-graph / {}: {}", self.code, self.message)
    }
}

impl std::error::Error for RuntimeModuleGraphError {}

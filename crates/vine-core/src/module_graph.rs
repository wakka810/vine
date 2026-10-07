use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const MAX_CANONICAL_MODULE_NAME_BYTES: usize = 260;
const MAX_REQUESTED_MODULE_NAME_BYTES: usize = 32 * 1024;
pub const RUNTIME_MODULE_GRAPH_MAX_MODULES: usize = 4096;
pub const RUNTIME_MODULE_REFERENCE_OWNER_LIMIT: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeModuleId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeModuleClass {
    MainExecutable,
    GuestDynamicLibrary,
    RuntimeSystemModule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeModuleLoadState {
    Registered,
    Ready,
    ProcessAttached { sequence: u32 },
    ProcessDetached { sequence: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuntimeModuleReferenceOwner {
    ProcessRoot,
    RuntimeService,
    StaticImport { requester: RuntimeModuleId },
    DynamicLoad { thread_id: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeModuleReference {
    pub owner: RuntimeModuleReferenceOwner,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeModuleRegistration {
    pub handle: u32,
    pub requested_name: String,
    pub canonical_name: String,
    pub class: RuntimeModuleClass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeModuleGraph {
    nodes: BTreeMap<RuntimeModuleId, RuntimeModuleNode>,
    canonical_index: BTreeMap<String, RuntimeModuleId>,
    main_module: Option<RuntimeModuleId>,
    next_module_id: u32,
    next_discovery_sequence: u32,
}

impl Default for RuntimeModuleGraph {
    fn default() -> Self {
        Self {
            nodes: BTreeMap::new(),
            canonical_index: BTreeMap::new(),
            main_module: None,
            next_module_id: 1,
            next_discovery_sequence: 1,
        }
    }
}

impl RuntimeModuleGraph {
    #[cfg(test)]
    pub fn register_module(
        &mut self,
        registration: RuntimeModuleRegistration,
    ) -> Result<RuntimeModuleId, RuntimeModuleGraphError> {
        let node = self.new_node(registration)?;
        let id = node.id;
        self.insert_node(node);
        Ok(id)
    }

    pub(crate) fn new_node(
        &self,
        registration: RuntimeModuleRegistration,
    ) -> Result<RuntimeModuleNode, RuntimeModuleGraphError> {
        let canonical_name =
            canonical_module_name(&registration.canonical_name, registration.class)?;
        if registration.handle == 0
            || registration.requested_name.is_empty()
            || registration.requested_name.len() > MAX_REQUESTED_MODULE_NAME_BYTES
        {
            return Err(RuntimeModuleGraphError::new(
                "module registration requires a nonzero handle and bounded requested name",
            ));
        }
        if registration.requested_name.contains('\0') {
            return Err(RuntimeModuleGraphError::new(
                "requested module name contains NUL",
            ));
        }
        if self.nodes.len() >= RUNTIME_MODULE_GRAPH_MAX_MODULES {
            return Err(RuntimeModuleGraphError::new(
                "runtime module graph reached its module limit",
            ));
        }
        if self.canonical_index.contains_key(&canonical_name)
            || self
                .nodes
                .values()
                .any(|node| node.handle == registration.handle)
        {
            return Err(RuntimeModuleGraphError::new(
                "canonical module name or logical handle is already registered",
            ));
        }
        if registration.class == RuntimeModuleClass::MainExecutable && self.main_module.is_some() {
            return Err(RuntimeModuleGraphError::new(
                "process module graph already has a main executable",
            ));
        }
        let id = RuntimeModuleId(self.next_module_id);
        self.next_module_id
            .checked_add(1)
            .ok_or_else(|| RuntimeModuleGraphError::new("runtime module ID space is exhausted"))?;
        let discovery_sequence = self.next_discovery_sequence;
        discovery_sequence.checked_add(1).ok_or_else(|| {
            RuntimeModuleGraphError::new("runtime module discovery sequence is exhausted")
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
        Ok(node)
    }

    // Only a checked, unpublished node reaches this point. Keeping that node
    // local lets the aggregate publish its handle before infallible insertion.
    pub(crate) fn insert_node(&mut self, node: RuntimeModuleNode) {
        if node.class == RuntimeModuleClass::MainExecutable {
            self.main_module = Some(node.id);
        }
        self.next_module_id = node.id.0 + 1;
        self.next_discovery_sequence = node.discovery_sequence + 1;
        self.canonical_index
            .insert(node.canonical_name.clone(), node.id);
        self.nodes.insert(node.id, node);
    }

    #[cfg(test)]
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
            return Err(RuntimeModuleGraphError::new(
                "only the latest unlinked registered module can be rolled back",
            ));
        }

        let node = self.nodes.remove(&id).ok_or_else(|| {
            RuntimeModuleGraphError::new(
                "validated module disappeared during registration rollback",
            )
        })?;
        self.canonical_index.remove(&node.canonical_name);
        if self.main_module == Some(id) {
            self.main_module = None;
        }
        self.next_module_id = id.0;
        self.next_discovery_sequence = node.discovery_sequence;

        Ok(node)
    }

    pub fn add_dependency(
        &mut self,
        requester: RuntimeModuleId,
        dependency: RuntimeModuleId,
    ) -> Result<(), RuntimeModuleGraphError> {
        if requester == dependency {
            return Err(RuntimeModuleGraphError::new(
                "module cannot depend on itself",
            ));
        }
        let requester_node = self.module(requester)?;
        let dependency_node = self.module(dependency)?;
        if requester_node.state != RuntimeModuleLoadState::Registered
            || dependency_node.state != RuntimeModuleLoadState::Registered
        {
            return Err(RuntimeModuleGraphError::new(
                "dependency edges must be complete before modules become ready",
            ));
        }
        if requester_node.dependencies.contains(&dependency) {
            return Err(RuntimeModuleGraphError::new(
                "module dependency edge is already registered",
            ));
        }
        if self.reaches(dependency, requester)? {
            return Err(RuntimeModuleGraphError::new(
                "module dependency edge would create a cycle",
            ));
        }
        let owner = RuntimeModuleReferenceOwner::StaticImport { requester };

        let dependency_node = self
            .nodes
            .get_mut(&dependency)
            .expect("validated dependency");
        add_reference_to_node(dependency_node, owner)?;
        dependency_node.dependents.insert(requester);
        self.nodes
            .get_mut(&requester)
            .expect("validated requester")
            .dependencies
            .insert(dependency);

        Ok(())
    }

    // Import binding preflights the complete batch before writing the guest IAT.
    // A partial graph update would otherwise outlive a rolled-back memory transaction.
    pub(crate) fn validate_additional_dependencies(
        &self,
        edges: &[(RuntimeModuleId, RuntimeModuleId)],
    ) -> Result<(), RuntimeModuleGraphError> {
        let additional = edges
            .iter()
            .copied()
            .filter(|(requester, dependency)| {
                self.nodes
                    .get(requester)
                    .is_none_or(|node| !node.dependencies.contains(dependency))
            })
            .collect::<BTreeSet<_>>();
        let mut new_references = BTreeMap::<RuntimeModuleId, usize>::new();
        for &(requester, dependency) in &additional {
            if requester == dependency {
                return Err(RuntimeModuleGraphError::new(
                    "module cannot depend on itself",
                ));
            }
            let requester_node = self.module(requester)?;
            let dependency_node = self.module(dependency)?;
            if requester_node.state != RuntimeModuleLoadState::Registered
                || dependency_node.state != RuntimeModuleLoadState::Registered
            {
                return Err(RuntimeModuleGraphError::new(
                    "dependency edges must be complete before modules become ready",
                ));
            }
            *new_references.entry(dependency).or_default() += 1;
            let mut pending = vec![dependency];
            let mut visited = BTreeSet::new();
            while let Some(id) = pending.pop() {
                if id == requester {
                    return Err(RuntimeModuleGraphError::new(
                        "module dependency edges would create a cycle",
                    ));
                }
                if visited.insert(id) {
                    pending.extend(self.module(id)?.dependencies.iter().copied());
                    pending.extend(
                        additional
                            .iter()
                            .filter(|(from, _)| *from == id)
                            .map(|(_, to)| *to),
                    );
                }
            }
        }
        for (dependency, count) in new_references {
            let node = self.module(dependency)?;
            if u32::try_from(count)
                .ok()
                .and_then(|count| node.reference_count().checked_add(count))
                .is_none()
                || node
                    .references
                    .len()
                    .checked_add(count)
                    .is_none_or(|owners| owners > RUNTIME_MODULE_REFERENCE_OWNER_LIMIT)
            {
                return Err(RuntimeModuleGraphError::new(
                    "runtime module reference count is exhausted",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn insert_checked_dependencies(
        &mut self,
        edges: &[(RuntimeModuleId, RuntimeModuleId)],
    ) {
        for &(requester, dependency) in edges {
            if self.nodes[&requester].dependencies.contains(&dependency) {
                continue;
            }
            self.add_dependency(requester, dependency)
                .expect("preflighted import dependencies must remain valid within one Rust call");
        }
    }

    pub fn add_dynamic_reference(
        &mut self,
        module: RuntimeModuleId,
        thread_id: u32,
    ) -> Result<u32, RuntimeModuleGraphError> {
        if thread_id == 0 {
            return Err(RuntimeModuleGraphError::new(
                "dynamic module reference requires a nonzero thread ID",
            ));
        }
        let owner = RuntimeModuleReferenceOwner::DynamicLoad { thread_id };

        let node = self.module_mut(module)?;
        add_reference_to_node(node, owner.clone())?;
        let count = node.reference_count_for(&owner);

        Ok(count)
    }

    #[cfg(test)]
    pub fn release_dynamic_reference(
        &mut self,
        module: RuntimeModuleId,
        thread_id: u32,
    ) -> Result<u32, RuntimeModuleGraphError> {
        if thread_id == 0 {
            return Err(RuntimeModuleGraphError::new(
                "dynamic module reference requires a nonzero thread ID",
            ));
        }
        let owner = RuntimeModuleReferenceOwner::DynamicLoad { thread_id };

        let node = self.module_mut(module)?;
        let index = node
            .references
            .iter()
            .position(|reference| reference.owner == owner)
            .ok_or_else(|| {
                RuntimeModuleGraphError::new("dynamic module reference owner has no live reference")
            })?;
        let remaining = node.references[index].count.checked_sub(1).ok_or_else(|| {
            RuntimeModuleGraphError::new("runtime module reference count underflowed")
        })?;
        if remaining == 0 {
            node.references.remove(index);
        } else {
            node.references[index].count = remaining;
        }

        Ok(remaining)
    }

    pub fn mark_ready(&mut self, module: RuntimeModuleId) -> Result<(), RuntimeModuleGraphError> {
        if self.module(module)?.state != RuntimeModuleLoadState::Registered {
            return Err(RuntimeModuleGraphError::new(
                "only a registered module can become ready",
            ));
        }

        self.module_mut(module)?.state = RuntimeModuleLoadState::Ready;

        Ok(())
    }

    #[cfg(test)]
    pub fn attach_all(&mut self) -> Result<Vec<RuntimeModuleId>, RuntimeModuleGraphError> {
        let order = self.planned_attach_order()?;
        self.publish_attach(&order);
        Ok(order)
    }

    pub(crate) fn publish_attach(&mut self, order: &[RuntimeModuleId]) {
        for (index, id) in order.iter().enumerate() {
            self.nodes
                .get_mut(id)
                .expect("validated attach order")
                .state = RuntimeModuleLoadState::ProcessAttached {
                sequence: (index + 1) as u32,
            };
        }
    }

    #[cfg(test)]
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
            return Err(RuntimeModuleGraphError::new(
                "dynamic runtime-system attachment requires one ready, referenced, callback-free module",
            ));
        }
        if self.main_module.is_none() {
            return Err(RuntimeModuleGraphError::new(
                "dynamic runtime-system attachment requires an attached main executable",
            ));
        }

        let mut sequences = BTreeSet::new();
        let mut maximum_sequence = 0_u32;
        for node in self.nodes.values().filter(|node| node.id != module) {
            let RuntimeModuleLoadState::ProcessAttached { sequence } = node.state else {
                return Err(RuntimeModuleGraphError::new(
                    "all existing modules must remain process-attached during a dynamic runtime-system load",
                ));
            };
            if sequence == 0 || !sequences.insert(sequence) {
                return Err(RuntimeModuleGraphError::new(
                    "attached module graph contains a zero or duplicate process-attach sequence",
                ));
            }
            maximum_sequence = maximum_sequence.max(sequence);
        }
        let sequence = maximum_sequence.checked_add(1).ok_or_else(|| {
            RuntimeModuleGraphError::new("dynamic runtime-system attach sequence is exhausted")
        })?;

        self.module_mut(module)?.state = RuntimeModuleLoadState::ProcessAttached { sequence };

        Ok(sequence)
    }

    pub(crate) fn load_existing_runtime_system_module(
        &mut self,
        module: RuntimeModuleId,
        thread_id: u32,
    ) -> Result<(), RuntimeModuleGraphError> {
        if thread_id == 0 {
            return Err(RuntimeModuleGraphError::new(
                "dynamic module reference requires a nonzero thread ID",
            ));
        }
        let node = self.module(module)?;
        let RuntimeModuleLoadState::ProcessAttached { .. } = node.state else {
            return Err(RuntimeModuleGraphError::new(
                "an existing dynamic runtime-system module must remain process-attached",
            ));
        };
        if node.class != RuntimeModuleClass::RuntimeSystemModule {
            return Err(RuntimeModuleGraphError::new(
                "only a runtime-system module can use the callback-free load path",
            ));
        }
        self.next_runtime_system_module_attach_sequence()?;
        let owner = RuntimeModuleReferenceOwner::DynamicLoad { thread_id };
        let node = self.nodes.get_mut(&module).expect("validated module");
        add_reference_to_node(node, owner)
    }

    pub(crate) fn new_attached_runtime_system_node(
        &self,
        registration: RuntimeModuleRegistration,
        thread_id: u32,
    ) -> Result<RuntimeModuleNode, RuntimeModuleGraphError> {
        if thread_id == 0 {
            return Err(RuntimeModuleGraphError::new(
                "dynamic module reference requires a nonzero thread ID",
            ));
        }
        if registration.class != RuntimeModuleClass::RuntimeSystemModule {
            return Err(RuntimeModuleGraphError::new(
                "dynamic runtime-system loading requires a runtime-system module",
            ));
        }
        let sequence = self.next_runtime_system_module_attach_sequence()?;
        let mut node = self.new_node(registration)?;
        add_reference_to_node(
            &mut node,
            RuntimeModuleReferenceOwner::DynamicLoad { thread_id },
        )?;
        node.state = RuntimeModuleLoadState::ProcessAttached { sequence };
        Ok(node)
    }

    pub fn planned_attach_order(&self) -> Result<Vec<RuntimeModuleId>, RuntimeModuleGraphError> {
        if self.main_module.is_none() {
            return Err(RuntimeModuleGraphError::new(
                "module graph cannot attach without a main executable",
            ));
        }
        for node in self.nodes.values() {
            if node.state != RuntimeModuleLoadState::Ready {
                return Err(RuntimeModuleGraphError::new(
                    "all modules must be ready before process attach ordering",
                ));
            }
            if node.reference_count() == 0 {
                return Err(RuntimeModuleGraphError::new(
                    "unreferenced module cannot participate in process attach ordering",
                ));
            }
        }
        let order = self.dependency_first_order()?;
        u32::try_from(order.len()).map_err(|_| {
            RuntimeModuleGraphError::new("module attach sequence does not fit 32-bit accounting")
        })?;
        Ok(order)
    }

    #[cfg(test)]
    pub fn detach_all(&mut self) -> Result<Vec<RuntimeModuleId>, RuntimeModuleGraphError> {
        let order = self.planned_detach_order()?;
        self.publish_detach(&order);
        Ok(order)
    }

    #[cfg(test)]
    pub(crate) fn publish_detach(&mut self, order: &[RuntimeModuleId]) {
        for (index, id) in order.iter().enumerate() {
            self.nodes
                .get_mut(id)
                .expect("validated detach order")
                .state = RuntimeModuleLoadState::ProcessDetached {
                sequence: (index + 1) as u32,
            };
        }
    }

    pub fn planned_detach_order(&self) -> Result<Vec<RuntimeModuleId>, RuntimeModuleGraphError> {
        let mut attached = self
            .nodes
            .values()
            .map(|node| match node.state {
                RuntimeModuleLoadState::ProcessAttached { sequence } => Ok((sequence, node.id)),
                _ => Err(RuntimeModuleGraphError::new(
                    "all modules must be attached before process detach ordering",
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        u32::try_from(attached.len()).map_err(|_| {
            RuntimeModuleGraphError::new("module detach sequence does not fit 32-bit accounting")
        })?;
        attached.sort_by(|left, right| right.cmp(left));
        Ok(attached.iter().map(|(_, id)| *id).collect())
    }

    pub fn module(
        &self,
        id: RuntimeModuleId,
    ) -> Result<&RuntimeModuleNode, RuntimeModuleGraphError> {
        self.nodes.get(&id).ok_or_else(|| {
            RuntimeModuleGraphError::new(format!("runtime module ID {} is not registered", id.0))
        })
    }

    pub fn module_by_name(
        &self,
        name: &str,
    ) -> Result<&RuntimeModuleNode, RuntimeModuleGraphError> {
        self.find_module_by_name(name)?.ok_or_else(|| {
            RuntimeModuleGraphError::new(format!("runtime module {name:?} is not registered"))
        })
    }

    pub(crate) fn find_module_by_name(
        &self,
        name: &str,
    ) -> Result<Option<&RuntimeModuleNode>, RuntimeModuleGraphError> {
        let Some(canonical) = normalize_module_lookup(name) else {
            return Ok(None);
        };
        let Some(id) = self.canonical_index.get(&canonical) else {
            return Ok(None);
        };
        let node = self.nodes.get(id).ok_or_else(|| {
            RuntimeModuleGraphError::new(
                "registered module name does not resolve to a live module node",
            )
        })?;
        if node.id != *id || node.canonical_name != canonical {
            return Err(RuntimeModuleGraphError::new(
                "registered module name and live node identity disagree",
            ));
        }
        Ok(Some(node))
    }

    pub fn module_by_handle(
        &self,
        handle: u32,
    ) -> Result<&RuntimeModuleNode, RuntimeModuleGraphError> {
        self.nodes
            .values()
            .find(|node| node.handle == handle)
            .ok_or_else(|| {
                RuntimeModuleGraphError::new(format!(
                    "runtime module handle {handle:#010x} is not registered"
                ))
            })
    }

    pub fn main_module(&self) -> Option<RuntimeModuleId> {
        self.main_module
    }

    #[cfg(test)]
    pub fn nodes(&self) -> &BTreeMap<RuntimeModuleId, RuntimeModuleNode> {
        &self.nodes
    }

    fn next_runtime_system_module_attach_sequence(&self) -> Result<u32, RuntimeModuleGraphError> {
        if self.main_module.is_none() {
            return Err(RuntimeModuleGraphError::new(
                "dynamic runtime-system attachment requires an attached main executable",
            ));
        }
        let mut sequences = BTreeSet::new();
        let mut maximum_sequence = 0_u32;
        for node in self.nodes.values() {
            let RuntimeModuleLoadState::ProcessAttached { sequence } = node.state else {
                return Err(RuntimeModuleGraphError::new(
                    "all existing modules must remain process-attached during a dynamic runtime-system load",
                ));
            };
            if sequence == 0 || !sequences.insert(sequence) {
                return Err(RuntimeModuleGraphError::new(
                    "attached module graph contains a zero or duplicate process-attach sequence",
                ));
            }
            maximum_sequence = maximum_sequence.max(sequence);
        }
        maximum_sequence.checked_add(1).ok_or_else(|| {
            RuntimeModuleGraphError::new("dynamic runtime-system attach sequence is exhausted")
        })
    }

    fn module_mut(
        &mut self,
        id: RuntimeModuleId,
    ) -> Result<&mut RuntimeModuleNode, RuntimeModuleGraphError> {
        self.nodes.get_mut(&id).ok_or_else(|| {
            RuntimeModuleGraphError::new(format!("runtime module ID {} is not registered", id.0))
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
                    RuntimeModuleGraphError::new(
                        "module dependency references a missing ordering node",
                    )
                })?;
                *count = count.checked_sub(1).ok_or_else(|| {
                    RuntimeModuleGraphError::new("module dependency ordering count underflowed")
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
            return Err(RuntimeModuleGraphError::new(
                "module graph contains a dependency cycle",
            ));
        }
        Ok(order)
    }
}

fn canonical_module_name(
    name: &str,
    class: RuntimeModuleClass,
) -> Result<String, RuntimeModuleGraphError> {
    let canonical = normalize_module_lookup(name).ok_or_else(|| {
        RuntimeModuleGraphError::new("module registration requires a bounded Windows-932 basename")
    })?;
    let expected_extension = if class == RuntimeModuleClass::MainExecutable {
        ".exe"
    } else {
        ".dll"
    };
    if !canonical.ends_with(expected_extension) {
        return Err(RuntimeModuleGraphError::new(format!(
            "resolved module identity must end in {expected_extension}"
        )));
    }
    Ok(canonical)
}

fn normalize_module_lookup(name: &str) -> Option<String> {
    let (encoded, _, had_errors) = encoding_rs::SHIFT_JIS.encode(name);
    let mut byte_index = 0;
    let mut separator = false;
    while byte_index < encoded.len() {
        let byte = encoded[byte_index];
        if (0x81..=0x9f).contains(&byte) || (0xe0..=0xfc).contains(&byte) {
            byte_index += 2; // A Windows-932 trail byte may itself be 0x5c.
        } else {
            separator |= matches!(byte, b'/' | b'\\' | b':');
            byte_index += 1;
        }
    }
    if name.is_empty()
        || encoded.len() > MAX_CANONICAL_MODULE_NAME_BYTES
        || had_errors
        || separator
        || name.chars().any(char::is_control)
    {
        return None;
    }
    Some(name.to_ascii_lowercase())
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
        return Err(RuntimeModuleGraphError::new(
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeModuleGraphError(String);

impl RuntimeModuleGraphError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for RuntimeModuleGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for RuntimeModuleGraphError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph_with_two_modules() -> (RuntimeModuleGraph, RuntimeModuleId, RuntimeModuleId) {
        let mut graph = RuntimeModuleGraph::default();
        let main = graph
            .register_module(RuntimeModuleRegistration {
                handle: 1,
                requested_name: "game.exe".to_owned(),
                canonical_name: "game.exe".to_owned(),
                class: RuntimeModuleClass::MainExecutable,
            })
            .unwrap();
        let dependency = graph
            .register_module(RuntimeModuleRegistration {
                handle: 2,
                requested_name: "engine.dll".to_owned(),
                canonical_name: "engine.dll".to_owned(),
                class: RuntimeModuleClass::GuestDynamicLibrary,
            })
            .unwrap();
        (graph, main, dependency)
    }

    #[test]
    fn module_name_lookup_checks_sandbox_names_and_ignores_ascii_case() {
        let (graph, main, _) = graph_with_two_modules();
        for name in [
            "missing.dll",
            "",
            "../game.exe",
            "game/engine.dll",
            "C:game.exe",
            "game\0.exe",
            "😀.dll",
        ] {
            assert!(graph.find_module_by_name(name).unwrap().is_none());
        }
        assert_eq!(
            graph.find_module_by_name("GAME.EXE").unwrap().unwrap().id,
            main
        );
    }

    #[test]
    fn import_dependency_batch_checks_cycles_and_capacity_before_any_edge() {
        let (mut graph, main, engine) = graph_with_two_modules();
        let codec = graph
            .register_module(RuntimeModuleRegistration {
                handle: 3,
                requested_name: "codec.dll".to_owned(),
                canonical_name: "codec.dll".to_owned(),
                class: RuntimeModuleClass::GuestDynamicLibrary,
            })
            .unwrap();
        let before = graph.clone();
        assert!(
            graph
                .validate_additional_dependencies(&[(main, engine), (engine, codec), (codec, main)])
                .is_err()
        );
        assert_eq!(graph, before);
        assert!(
            graph
                .validate_additional_dependencies(&[(main, engine), (engine, RuntimeModuleId(99))])
                .is_err()
        );
        assert_eq!(graph, before);

        let edges = [(main, engine), (engine, codec)];
        graph.validate_additional_dependencies(&edges).unwrap();
        graph.insert_checked_dependencies(&edges);
        assert!(graph.module(main).unwrap().dependencies.contains(&engine));
        assert!(graph.module(engine).unwrap().dependencies.contains(&codec));
        let bound = graph.clone();
        graph.validate_additional_dependencies(&edges).unwrap();
        graph.insert_checked_dependencies(&edges);
        assert_eq!(graph, bound, "rebinding an existing edge adds no reference");

        let mut exhausted = before;
        exhausted
            .nodes
            .get_mut(&codec)
            .unwrap()
            .references
            .push(RuntimeModuleReference {
                owner: RuntimeModuleReferenceOwner::DynamicLoad { thread_id: 1 },
                count: u32::MAX,
            });
        let before_rejection = exhausted.clone();
        assert!(exhausted.validate_additional_dependencies(&edges).is_err());
        assert_eq!(exhausted, before_rejection);
    }

    #[test]
    fn exhausted_dependency_reference_does_not_publish_an_edge() {
        let (mut graph, main, dependency) = graph_with_two_modules();
        graph
            .nodes
            .get_mut(&dependency)
            .unwrap()
            .references
            .push(RuntimeModuleReference {
                owner: RuntimeModuleReferenceOwner::DynamicLoad { thread_id: 1 },
                count: u32::MAX,
            });
        let before = graph.clone();
        assert!(graph.add_dependency(main, dependency).is_err());
        assert_eq!(graph, before);
    }

    #[test]
    fn checked_node_exhaustion_and_invalid_registration_preserve_identity_allocators() {
        for exhausted_id in [true, false] {
            let mut graph = RuntimeModuleGraph::default();
            if exhausted_id {
                graph.next_module_id = u32::MAX;
            } else {
                graph.next_discovery_sequence = u32::MAX;
            }
            let before = graph.clone();
            assert!(
                graph
                    .register_module(RuntimeModuleRegistration {
                        handle: 1,
                        requested_name: "game.exe".to_owned(),
                        canonical_name: "game.exe".to_owned(),
                        class: RuntimeModuleClass::MainExecutable,
                    })
                    .is_err()
            );
            assert_eq!(graph, before);
        }
        let mut graph = RuntimeModuleGraph::default();
        let before = graph.clone();
        assert!(
            graph
                .register_module(RuntimeModuleRegistration {
                    handle: 1,
                    requested_name: "bad\0.exe".to_owned(),
                    canonical_name: "game.exe".to_owned(),
                    class: RuntimeModuleClass::MainExecutable,
                })
                .is_err()
        );
        assert_eq!(graph, before);
    }

    #[test]
    fn invalid_dependency_and_premature_detach_preserve_the_graph() {
        let (mut graph, main, dependency) = graph_with_two_modules();
        let before = graph.clone();
        assert!(graph.add_dependency(main, RuntimeModuleId(99)).is_err());
        assert_eq!(graph, before);
        assert!(graph.detach_all().is_err());
        assert_eq!(graph, before);

        graph.add_dependency(main, dependency).unwrap();
        graph.mark_ready(main).unwrap();
        graph.mark_ready(dependency).unwrap();
        let before = graph.clone();
        assert!(graph.detach_all().is_err());
        assert_eq!(graph, before);
        assert_eq!(graph.attach_all().unwrap(), [dependency, main]);
        assert_eq!(graph.detach_all().unwrap(), [main, dependency]);
    }
}

use std::collections::BTreeMap;

use serde::Serialize;

use super::user32::RuntimeImeAppWindowTarget;
use super::{RuntimeStateError, RuntimeThreadId, RuntimeWindowId};

pub const LOGICAL_IME_CONTEXT_HANDLE_NAMESPACE: &str = "vine.logical-ime-context.v1";
pub const RUNTIME_IME_CONTEXT_HANDLE_PREFIX: u32 = 0x494d_0000;
pub const RUNTIME_IME_CONTEXT_LIMIT: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeImeContextId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeImeContext {
    pub id: RuntimeImeContextId,
    pub handle: u32,
    pub namespace: String,
    pub owner_thread_id: RuntimeThreadId,
    pub default_for_thread: bool,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeImeContextReference {
    pub context_id: RuntimeImeContextId,
    pub handle: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeImeContextAssociation {
    pub window_id: RuntimeWindowId,
    pub window_handle: u32,
    pub owner_thread_id: RuntimeThreadId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<RuntimeImeContextReference>,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeImeAssociationResult {
    pub previous_context_handle: u32,
    pub context_handle: u32,
    pub changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeImeEnsureWindowDisposition {
    Existing {
        context: RuntimeImeContext,
        association: RuntimeImeContextAssociation,
    },
    Create {
        context: RuntimeImeContext,
        create_context: bool,
        association: RuntimeImeContextAssociation,
        expected_next_context_id: u16,
        next_context_id: u16,
        next_generation: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeImeEnsureWindowPlan {
    expected_generation: u64,
    target: RuntimeImeAppWindowTarget,
    disposition: RuntimeImeEnsureWindowDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeImeAssociationDisposition {
    Unchanged {
        association: RuntimeImeContextAssociation,
        result: RuntimeImeAssociationResult,
    },
    Update {
        previous: RuntimeImeContextAssociation,
        next: RuntimeImeContextAssociation,
        result: RuntimeImeAssociationResult,
        next_generation: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeImeAssociationPlan {
    expected_generation: u64,
    disposition: RuntimeImeAssociationDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeImm32TeardownPlan {
    expected_generation: u64,
    next_generation: u64,
    contexts: usize,
    default_contexts: usize,
    associations: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeImm32 {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    contexts: BTreeMap<RuntimeImeContextId, RuntimeImeContext>,
    #[serde(skip)]
    context_handle_index: BTreeMap<u32, RuntimeImeContextId>,
    #[serde(skip)]
    default_contexts: BTreeMap<RuntimeThreadId, RuntimeImeContextId>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    associations: BTreeMap<RuntimeWindowId, RuntimeImeContextAssociation>,
    next_context_id: u16,
    generation: u64,
}

impl Default for RuntimeImm32 {
    fn default() -> Self {
        Self {
            contexts: BTreeMap::new(),
            context_handle_index: BTreeMap::new(),
            default_contexts: BTreeMap::new(),
            associations: BTreeMap::new(),
            next_context_id: 1,
            generation: 0,
        }
    }
}

impl RuntimeImm32 {
    pub(super) fn plan_ensure_window(
        &self,
        target: RuntimeImeAppWindowTarget,
    ) -> Result<RuntimeImeEnsureWindowPlan, RuntimeStateError> {
        if let Some(association) = self.associations.get(&target.window_id) {
            self.validate_association_target(association, target)?;
            let context = self.default_context(target.owner_thread_id)?.clone();
            return Ok(RuntimeImeEnsureWindowPlan {
                expected_generation: self.generation,
                target,
                disposition: RuntimeImeEnsureWindowDisposition::Existing {
                    context,
                    association: association.clone(),
                },
            });
        }

        let (context, create_context, expected_next_context_id, next_context_id) = if let Some(id) =
            self.default_contexts.get(&target.owner_thread_id)
        {
            let context = self.contexts.get(id).cloned().ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-imm32",
                    "default-ime-context-index-dangling",
                    "default IME context index does not resolve to a live context",
                )
            })?;
            self.validate_context_identity(&context)?;
            (context, false, self.next_context_id, self.next_context_id)
        } else {
            if self.contexts.len() >= RUNTIME_IME_CONTEXT_LIMIT || self.next_context_id == 0 {
                return Err(RuntimeStateError::resource_limit(
                    "runtime-imm32",
                    "ime-context-capacity-exhausted",
                    "logical IME context capacity is exhausted",
                ));
            }
            let id = RuntimeImeContextId(u64::from(self.next_context_id));
            let handle = RUNTIME_IME_CONTEXT_HANDLE_PREFIX | u32::from(self.next_context_id);
            if self.contexts.contains_key(&id) || self.context_handle_index.contains_key(&handle) {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-imm32",
                    "ime-context-identity-collision",
                    "logical IME context allocator selected a live identity",
                ));
            }
            let next_context_id = self.next_context_id.checked_add(1).unwrap_or(0);
            (
                RuntimeImeContext {
                    id,
                    handle,
                    namespace: LOGICAL_IME_CONTEXT_HANDLE_NAMESPACE.to_owned(),
                    owner_thread_id: target.owner_thread_id,
                    default_for_thread: true,
                    generation: self.next_generation()?,
                },
                true,
                self.next_context_id,
                next_context_id,
            )
        };
        let next_generation = self.next_generation()?;
        let association = RuntimeImeContextAssociation {
            window_id: target.window_id,
            window_handle: target.window_handle,
            owner_thread_id: target.owner_thread_id,
            context: Some(RuntimeImeContextReference {
                context_id: context.id,
                handle: context.handle,
            }),
            generation: next_generation,
        };
        Ok(RuntimeImeEnsureWindowPlan {
            expected_generation: self.generation,
            target,
            disposition: RuntimeImeEnsureWindowDisposition::Create {
                context,
                create_context,
                association,
                expected_next_context_id,
                next_context_id,
                next_generation,
            },
        })
    }

    pub(super) fn preflight_ensure_window(
        &self,
        plan: &RuntimeImeEnsureWindowPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation {
            return Err(stale_plan("ime-window-ensure-plan-stale"));
        }
        match &plan.disposition {
            RuntimeImeEnsureWindowDisposition::Existing {
                context,
                association,
            } => {
                self.validate_context_identity(context)?;
                self.validate_association_target(association, plan.target)?;
                if self.default_contexts.get(&plan.target.owner_thread_id) != Some(&context.id)
                    || self.contexts.get(&context.id) != Some(context)
                    || self.associations.get(&plan.target.window_id) != Some(association)
                {
                    return Err(stale_plan("ime-window-ensure-plan-stale"));
                }
            }
            RuntimeImeEnsureWindowDisposition::Create {
                context,
                create_context,
                association,
                expected_next_context_id,
                next_context_id,
                next_generation,
            } => {
                if self.associations.contains_key(&plan.target.window_id)
                    || association.window_id != plan.target.window_id
                    || association.window_handle != plan.target.window_handle
                    || association.owner_thread_id != plan.target.owner_thread_id
                    || association.generation != *next_generation
                    || association.context
                        != Some(RuntimeImeContextReference {
                            context_id: context.id,
                            handle: context.handle,
                        })
                {
                    return Err(stale_plan("ime-window-ensure-plan-stale"));
                }
                if *create_context {
                    if self.next_context_id != *expected_next_context_id
                        || context.id.0 != u64::from(*expected_next_context_id)
                        || context.handle
                            != RUNTIME_IME_CONTEXT_HANDLE_PREFIX
                                | u32::from(*expected_next_context_id)
                        || *next_context_id != expected_next_context_id.checked_add(1).unwrap_or(0)
                        || self.contexts.len() >= RUNTIME_IME_CONTEXT_LIMIT
                        || self.contexts.contains_key(&context.id)
                        || self.context_handle_index.contains_key(&context.handle)
                        || self
                            .default_contexts
                            .contains_key(&plan.target.owner_thread_id)
                    {
                        return Err(stale_plan("ime-window-ensure-plan-stale"));
                    }
                } else if self.default_contexts.get(&plan.target.owner_thread_id)
                    != Some(&context.id)
                    || self.contexts.get(&context.id) != Some(context)
                {
                    return Err(stale_plan("ime-window-ensure-plan-stale"));
                }
            }
        }
        Ok(())
    }

    pub(super) fn commit_ensure_window_preflighted(
        &mut self,
        plan: RuntimeImeEnsureWindowPlan,
    ) -> RuntimeImeContextAssociation {
        debug_assert!(self.preflight_ensure_window(&plan).is_ok());
        match plan.disposition {
            RuntimeImeEnsureWindowDisposition::Existing { association, .. } => association,
            RuntimeImeEnsureWindowDisposition::Create {
                context,
                create_context,
                association,
                next_context_id,
                next_generation,
                ..
            } => {
                if create_context {
                    self.next_context_id = next_context_id;
                    self.context_handle_index.insert(context.handle, context.id);
                    self.default_contexts
                        .insert(context.owner_thread_id, context.id);
                    self.contexts.insert(context.id, context);
                }
                self.associations
                    .insert(association.window_id, association.clone());
                self.generation = next_generation;
                association
            }
        }
    }

    pub(super) fn plan_association(
        &self,
        target: RuntimeImeAppWindowTarget,
        context_handle: u32,
    ) -> Result<RuntimeImeAssociationPlan, RuntimeStateError> {
        let previous = self
            .associations
            .get(&target.window_id)
            .cloned()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-imm32",
                    "ime-window-association-uninitialized",
                    "logical window has no materialized default IME context association",
                )
            })?;
        self.validate_association_target(&previous, target)?;
        let next_context = if context_handle == 0 {
            None
        } else {
            let context = self.context(context_handle)?;
            if context.owner_thread_id != target.owner_thread_id {
                return Err(RuntimeStateError::conflict(
                    "runtime-imm32",
                    "ime-context-owner-thread-mismatch",
                    "logical IME context belongs to a different owner thread",
                ));
            }
            Some(RuntimeImeContextReference {
                context_id: context.id,
                handle: context.handle,
            })
        };
        let previous_context_handle = previous.context.map_or(0, |context| context.handle);
        let result = RuntimeImeAssociationResult {
            previous_context_handle,
            context_handle,
            changed: previous.context != next_context,
        };
        let disposition = if result.changed {
            let next_generation = self.next_generation()?;
            let mut next = previous.clone();
            next.context = next_context;
            next.generation = next_generation;
            RuntimeImeAssociationDisposition::Update {
                previous,
                next,
                result,
                next_generation,
            }
        } else {
            RuntimeImeAssociationDisposition::Unchanged {
                association: previous,
                result,
            }
        };
        Ok(RuntimeImeAssociationPlan {
            expected_generation: self.generation,
            disposition,
        })
    }

    pub(super) fn preflight_association(
        &self,
        plan: &RuntimeImeAssociationPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation {
            return Err(stale_plan("ime-association-plan-stale"));
        }
        let (previous, next) = match &plan.disposition {
            RuntimeImeAssociationDisposition::Unchanged { association, .. } => {
                (association, association)
            }
            RuntimeImeAssociationDisposition::Update { previous, next, .. } => (previous, next),
        };
        if self.associations.get(&previous.window_id) != Some(previous) {
            return Err(stale_plan("ime-association-plan-stale"));
        }
        self.validate_association(previous)?;
        self.validate_association(next)
    }

    pub(super) fn commit_association(
        &mut self,
        plan: RuntimeImeAssociationPlan,
    ) -> Result<RuntimeImeAssociationResult, RuntimeStateError> {
        self.preflight_association(&plan)?;
        match plan.disposition {
            RuntimeImeAssociationDisposition::Unchanged { result, .. } => Ok(result),
            RuntimeImeAssociationDisposition::Update {
                next,
                result,
                next_generation,
                ..
            } => {
                self.associations.insert(next.window_id, next);
                self.generation = next_generation;
                Ok(result)
            }
        }
    }

    pub fn context(&self, handle: u32) -> Result<&RuntimeImeContext, RuntimeStateError> {
        let id = self.context_handle_index.get(&handle).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-imm32",
                "ime-context-handle-unknown",
                format!("logical HIMC {handle:#010x} is not live"),
            )
        })?;
        let context = self.contexts.get(id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-imm32",
                "ime-context-handle-index-dangling",
                "logical HIMC index does not resolve to a live context",
            )
        })?;
        self.validate_context_identity(context)?;
        Ok(context)
    }

    pub fn contexts(&self) -> &BTreeMap<RuntimeImeContextId, RuntimeImeContext> {
        &self.contexts
    }

    pub fn associations(&self) -> &BTreeMap<RuntimeWindowId, RuntimeImeContextAssociation> {
        &self.associations
    }

    pub(super) fn plan_process_teardown(
        &self,
    ) -> Result<Option<RuntimeImm32TeardownPlan>, RuntimeStateError> {
        if self.is_empty() {
            return Ok(None);
        }
        Ok(Some(RuntimeImm32TeardownPlan {
            expected_generation: self.generation,
            next_generation: self.next_generation()?,
            contexts: self.contexts.len(),
            default_contexts: self.default_contexts.len(),
            associations: self.associations.len(),
        }))
    }

    pub(super) fn preflight_process_teardown(
        &self,
        plan: &RuntimeImm32TeardownPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.contexts.len() != plan.contexts
            || self.context_handle_index.len() != plan.contexts
            || self.default_contexts.len() != plan.default_contexts
            || self.associations.len() != plan.associations
        {
            return Err(stale_plan("imm32-teardown-plan-stale"));
        }
        Ok(())
    }

    pub(super) fn commit_process_teardown(&mut self, plan: RuntimeImm32TeardownPlan) {
        debug_assert!(self.preflight_process_teardown(&plan).is_ok());
        self.contexts.clear();
        self.context_handle_index.clear();
        self.default_contexts.clear();
        self.associations.clear();
        self.generation = plan.next_generation;
    }

    pub(super) fn is_empty(&self) -> bool {
        self.contexts.is_empty()
            && self.context_handle_index.is_empty()
            && self.default_contexts.is_empty()
            && self.associations.is_empty()
    }

    fn default_context(
        &self,
        owner_thread_id: RuntimeThreadId,
    ) -> Result<&RuntimeImeContext, RuntimeStateError> {
        let id = self.default_contexts.get(&owner_thread_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-imm32",
                "default-ime-context-missing",
                "materialized IME window association has no thread default context",
            )
        })?;
        let context = self.contexts.get(id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-imm32",
                "default-ime-context-index-dangling",
                "default IME context index does not resolve to a live context",
            )
        })?;
        self.validate_context_identity(context)?;
        Ok(context)
    }

    fn validate_context_identity(
        &self,
        context: &RuntimeImeContext,
    ) -> Result<(), RuntimeStateError> {
        if context.id.0 == 0
            || context.handle & 0xffff_0000 != RUNTIME_IME_CONTEXT_HANDLE_PREFIX
            || context.handle & 0xffff == 0
            || context.id.0 != u64::from(context.handle & 0xffff)
            || context.namespace != LOGICAL_IME_CONTEXT_HANDLE_NAMESPACE
            || !context.default_for_thread
            || self.context_handle_index.get(&context.handle) != Some(&context.id)
            || self.default_contexts.get(&context.owner_thread_id) != Some(&context.id)
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-imm32",
                "ime-context-record-inconsistent",
                "logical IME context does not match its namespace or indexes",
            ));
        }
        Ok(())
    }

    fn validate_association_target(
        &self,
        association: &RuntimeImeContextAssociation,
        target: RuntimeImeAppWindowTarget,
    ) -> Result<(), RuntimeStateError> {
        if association.window_id != target.window_id
            || association.window_handle != target.window_handle
            || association.owner_thread_id != target.owner_thread_id
        {
            return Err(RuntimeStateError::conflict(
                "runtime-imm32",
                "ime-window-association-target-mismatch",
                "logical IME association does not identify the selected app window",
            ));
        }
        self.validate_association(association)
    }

    fn validate_association(
        &self,
        association: &RuntimeImeContextAssociation,
    ) -> Result<(), RuntimeStateError> {
        if let Some(reference) = association.context {
            let context = self.context(reference.handle)?;
            if context.id != reference.context_id
                || context.owner_thread_id != association.owner_thread_id
            {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-imm32",
                    "ime-association-context-mismatch",
                    "logical IME association context does not match its owner or identity",
                ));
            }
        }
        Ok(())
    }

    fn next_generation(&self) -> Result<u64, RuntimeStateError> {
        self.generation.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-imm32",
                "imm32-generation-exhausted",
                "IMM32 resource generation is exhausted",
            )
        })
    }
}

fn stale_plan(code: &'static str) -> RuntimeStateError {
    RuntimeStateError::conflict(
        "runtime-imm32",
        code,
        "IMM32 state changed after mutation planning",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(window_id: u64, window_handle: u32, thread_id: u32) -> RuntimeImeAppWindowTarget {
        RuntimeImeAppWindowTarget {
            window_id: RuntimeWindowId(window_id),
            window_handle,
            owner_thread_id: RuntimeThreadId(thread_id),
        }
    }

    #[test]
    fn default_context_detach_repeat_query_restore_and_teardown_preserve_authority() {
        let mut imm32 = RuntimeImm32::default();
        let main = target(1, 0x4801_0001, 0x2000);
        let ensure = imm32.plan_ensure_window(main).unwrap();
        imm32.preflight_ensure_window(&ensure).unwrap();
        let association = imm32.commit_ensure_window_preflighted(ensure);
        let context = association.context.unwrap();
        assert_eq!(context.handle, RUNTIME_IME_CONTEXT_HANDLE_PREFIX | 1);
        assert_eq!(imm32.contexts().len(), 1);
        assert_eq!(imm32.associations().len(), 1);

        let detach = imm32.plan_association(main, 0).unwrap();
        let detached = imm32.commit_association(detach).unwrap();
        assert_eq!(detached.previous_context_handle, context.handle);
        assert_eq!(detached.context_handle, 0);
        assert!(detached.changed);
        assert_eq!(imm32.associations()[&main.window_id].context, None);

        let repeat_query = imm32.plan_ensure_window(main).unwrap();
        let repeated = imm32.commit_ensure_window_preflighted(repeat_query);
        assert_eq!(repeated.context, None);
        assert_eq!(imm32.contexts().len(), 1);

        let restore = imm32.plan_association(main, context.handle).unwrap();
        let restored = imm32.commit_association(restore).unwrap();
        assert_eq!(restored.previous_context_handle, 0);
        assert_eq!(restored.context_handle, context.handle);
        assert!(restored.changed);
        assert_eq!(imm32.associations()[&main.window_id].context, Some(context));

        let second = target(2, 0x4801_0003, 0x2000);
        let second_ensure = imm32.plan_ensure_window(second).unwrap();
        let second_association = imm32.commit_ensure_window_preflighted(second_ensure);
        assert_eq!(second_association.context, Some(context));
        assert_eq!(imm32.contexts().len(), 1);

        let teardown = imm32.plan_process_teardown().unwrap().unwrap();
        imm32.preflight_process_teardown(&teardown).unwrap();
        imm32.commit_process_teardown(teardown);
        assert!(imm32.contexts().is_empty());
        assert!(imm32.associations().is_empty());
    }

    #[test]
    fn stale_and_wrong_owner_mutations_are_rejected_without_partial_state() {
        let mut imm32 = RuntimeImm32::default();
        let main = target(1, 0x4801_0001, 0x2000);
        let stale = imm32.plan_ensure_window(main).unwrap();
        let committed = imm32
            .plan_ensure_window(target(2, 0x4801_0002, 0x2000))
            .unwrap();
        imm32.commit_ensure_window_preflighted(committed);
        let before = imm32.clone();
        assert_eq!(
            imm32.preflight_ensure_window(&stale).unwrap_err().code,
            "ime-window-ensure-plan-stale"
        );
        assert_eq!(imm32, before);

        let main_ensure = imm32.plan_ensure_window(main).unwrap();
        let association = imm32.commit_ensure_window_preflighted(main_ensure);
        let context = association.context.unwrap();
        let other_thread = target(3, 0x4801_0003, 0x2001);
        let other_ensure = imm32.plan_ensure_window(other_thread).unwrap();
        imm32.commit_ensure_window_preflighted(other_ensure);
        let before = imm32.clone();
        assert_eq!(
            imm32
                .plan_association(other_thread, context.handle)
                .unwrap_err()
                .code,
            "ime-context-owner-thread-mismatch"
        );
        assert_eq!(imm32, before);
    }
}

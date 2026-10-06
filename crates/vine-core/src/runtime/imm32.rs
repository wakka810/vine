use std::collections::BTreeMap;

use super::user32::RuntimeImeAppWindowTarget;
use super::{RuntimeStateError, RuntimeThreadId, RuntimeWindowId};

pub const RUNTIME_IME_CONTEXT_HANDLE_PREFIX: u32 = 0x494d_0000;
pub const RUNTIME_IME_CONTEXT_LIMIT: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeImeContextId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeImeContext {
    pub id: RuntimeImeContextId,
    pub handle: u32,
    pub owner_thread_id: RuntimeThreadId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeImeContextReference {
    pub context_id: RuntimeImeContextId,
    pub handle: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeImeContextAssociation {
    pub window_id: RuntimeWindowId,
    pub window_handle: u32,
    pub owner_thread_id: RuntimeThreadId,
    pub context: Option<RuntimeImeContextReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeImm32 {
    contexts: BTreeMap<RuntimeImeContextId, RuntimeImeContext>,
    context_handle_index: BTreeMap<u32, RuntimeImeContextId>,
    default_contexts: BTreeMap<RuntimeThreadId, RuntimeImeContextId>,
    associations: BTreeMap<RuntimeWindowId, RuntimeImeContextAssociation>,
    next_context_id: u16,
}

impl Default for RuntimeImm32 {
    fn default() -> Self {
        Self {
            contexts: BTreeMap::new(),
            context_handle_index: BTreeMap::new(),
            default_contexts: BTreeMap::new(),
            associations: BTreeMap::new(),
            next_context_id: 1,
        }
    }
}

impl RuntimeImm32 {
    pub(super) fn ensure_window(
        &mut self,
        target: RuntimeImeAppWindowTarget,
    ) -> Result<(), RuntimeStateError> {
        if let Some(association) = self.associations.get(&target.window_id) {
            self.validate_association_target(association, target)?;
            self.default_context(target.owner_thread_id)?;
            return Ok(());
        }

        let create_context = !self.default_contexts.contains_key(&target.owner_thread_id);
        let (context_id, context_handle) = if create_context {
            if self.contexts.len() >= RUNTIME_IME_CONTEXT_LIMIT || self.next_context_id == 0 {
                return Err(RuntimeStateError::new(
                    "logical IME context capacity is exhausted",
                ));
            }
            let id = RuntimeImeContextId(u64::from(self.next_context_id));
            let handle = RUNTIME_IME_CONTEXT_HANDLE_PREFIX | u32::from(self.next_context_id);
            if self.contexts.contains_key(&id) || self.context_handle_index.contains_key(&handle) {
                return Err(RuntimeStateError::new(
                    "logical IME context allocator selected a live identity",
                ));
            }
            (id, handle)
        } else {
            let context = self.default_context(target.owner_thread_id)?;
            (context.id, context.handle)
        };
        let association = RuntimeImeContextAssociation {
            window_id: target.window_id,
            window_handle: target.window_handle,
            owner_thread_id: target.owner_thread_id,
            context: Some(RuntimeImeContextReference {
                context_id,
                handle: context_handle,
            }),
        };

        // No fallible work remains: publish the context and association together.
        if create_context {
            self.next_context_id = self.next_context_id.checked_add(1).unwrap_or(0);
            self.context_handle_index.insert(context_handle, context_id);
            self.default_contexts
                .insert(target.owner_thread_id, context_id);
            self.contexts.insert(
                context_id,
                RuntimeImeContext {
                    id: context_id,
                    handle: context_handle,
                    owner_thread_id: target.owner_thread_id,
                },
            );
        }
        self.associations.insert(target.window_id, association);
        Ok(())
    }

    pub(super) fn associate_context(
        &mut self,
        target: RuntimeImeAppWindowTarget,
        context_handle: u32,
    ) -> Result<u32, RuntimeStateError> {
        let previous = self.associations.get(&target.window_id).ok_or_else(|| {
            RuntimeStateError::new(
                "logical window has no materialized default IME context association",
            )
        })?;
        self.validate_association_target(previous, target)?;
        let next_context = if context_handle == 0 {
            None
        } else {
            let context = self.context(context_handle)?;
            if context.owner_thread_id != target.owner_thread_id {
                return Err(RuntimeStateError::new(
                    "logical IME context belongs to a different owner thread",
                ));
            }
            Some(RuntimeImeContextReference {
                context_id: context.id,
                handle: context.handle,
            })
        };
        let previous_context_handle = previous.context.map_or(0, |context| context.handle);
        self.associations
            .get_mut(&target.window_id)
            .expect("validated IME window association disappeared")
            .context = next_context;
        Ok(previous_context_handle)
    }

    pub fn context(&self, handle: u32) -> Result<&RuntimeImeContext, RuntimeStateError> {
        let id = self.context_handle_index.get(&handle).ok_or_else(|| {
            RuntimeStateError::new(format!("logical HIMC {handle:#010x} is not live"))
        })?;
        let context = self.contexts.get(id).ok_or_else(|| {
            RuntimeStateError::new("logical HIMC index does not resolve to a live context")
        })?;
        self.validate_context_identity(context)?;
        Ok(context)
    }

    #[cfg(test)]
    pub(crate) fn contexts(&self) -> &BTreeMap<RuntimeImeContextId, RuntimeImeContext> {
        &self.contexts
    }

    #[cfg(test)]
    pub(crate) fn associations(&self) -> &BTreeMap<RuntimeWindowId, RuntimeImeContextAssociation> {
        &self.associations
    }

    fn default_context(
        &self,
        owner_thread_id: RuntimeThreadId,
    ) -> Result<&RuntimeImeContext, RuntimeStateError> {
        let id = self.default_contexts.get(&owner_thread_id).ok_or_else(|| {
            RuntimeStateError::new(
                "materialized IME window association has no thread default context",
            )
        })?;
        let context = self.contexts.get(id).ok_or_else(|| {
            RuntimeStateError::new("default IME context index does not resolve to a live context")
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
            || self.context_handle_index.get(&context.handle) != Some(&context.id)
            || self.default_contexts.get(&context.owner_thread_id) != Some(&context.id)
        {
            return Err(RuntimeStateError::new(
                "logical IME context does not match its handle or indexes",
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
            return Err(RuntimeStateError::new(
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
                return Err(RuntimeStateError::new(
                    "logical IME association context does not match its owner or identity",
                ));
            }
        }
        Ok(())
    }
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
    fn default_context_detach_repeat_query_and_restore_preserve_authority() {
        let mut imm32 = RuntimeImm32::default();
        let main = target(1, 0x4801_0001, 0x2000);
        imm32.ensure_window(main).unwrap();
        let association = imm32.associations[&main.window_id].clone();
        let context = association.context.unwrap();
        assert_eq!(context.handle, RUNTIME_IME_CONTEXT_HANDLE_PREFIX | 1);
        assert_eq!(imm32.contexts().len(), 1);
        assert_eq!(imm32.associations().len(), 1);

        let detached = imm32.associate_context(main, 0).unwrap();
        assert_eq!(detached, context.handle);
        assert_eq!(imm32.associations()[&main.window_id].context, None);

        imm32.ensure_window(main).unwrap();
        let repeated = &imm32.associations[&main.window_id];
        assert_eq!(repeated.context, None);
        assert_eq!(imm32.contexts().len(), 1);

        let restored = imm32.associate_context(main, context.handle).unwrap();
        assert_eq!(restored, 0);
        assert_eq!(imm32.associations()[&main.window_id].context, Some(context));

        let second = target(2, 0x4801_0003, 0x2000);
        imm32.ensure_window(second).unwrap();
        let second_association = &imm32.associations[&second.window_id];
        assert_eq!(second_association.context, Some(context));
        assert_eq!(imm32.contexts().len(), 1);
    }

    #[test]
    fn association_validates_identity_before_mutation() {
        let mut imm32 = RuntimeImm32::default();
        let main = target(1, 0x4801_0001, 0x2000);
        assert!(imm32.associate_context(main, 0).is_err());
        assert!(imm32.contexts.is_empty());
        assert!(imm32.context_handle_index.is_empty());
        assert!(imm32.default_contexts.is_empty());
        assert!(imm32.associations.is_empty());

        imm32.ensure_window(main).unwrap();
        let context = imm32.associations[&main.window_id].context.unwrap();
        assert_eq!(context.handle, RUNTIME_IME_CONTEXT_HANDLE_PREFIX | 1);
        for (invalid_target, handle) in [
            (main, 0x494d_ffff),
            (target(1, 0x4801_0002, 0x2000), 0),
            (target(1, 0x4801_0001, 0x2001), 0),
        ] {
            assert!(imm32.associate_context(invalid_target, handle).is_err());
            let association = &imm32.associations[&main.window_id];
            assert_eq!(association.window_handle, main.window_handle);
            assert_eq!(association.owner_thread_id, main.owner_thread_id);
            assert_eq!(association.context, Some(context));
            assert_eq!(imm32.contexts.len(), 1);
            assert_eq!(imm32.associations.len(), 1);
            assert_eq!(
                imm32.context(context.handle).unwrap().id,
                context.context_id
            );
            assert_eq!(
                imm32.default_context(main.owner_thread_id).unwrap().id,
                context.context_id
            );
        }

        assert_eq!(
            imm32.associate_context(main, context.handle).unwrap(),
            context.handle
        );
        assert_eq!(imm32.associations[&main.window_id].context, Some(context));

        let detached = imm32.associate_context(main, 0).unwrap();
        assert_eq!(detached, context.handle);
        assert_eq!(imm32.associate_context(main, 0).unwrap(), 0);
        assert_eq!(imm32.associations[&main.window_id].context, None);
        assert_eq!(
            imm32.context(context.handle).unwrap().id,
            context.context_id
        );
        assert_eq!(imm32.next_context_id, 2);
    }

    #[test]
    fn ensure_validates_capacity_and_identity_before_publication() {
        let main = target(1, 0x4801_0001, 0x2000);
        let mut imm32 = RuntimeImm32 {
            next_context_id: 0,
            ..RuntimeImm32::default()
        };
        assert!(imm32.ensure_window(main).is_err());
        assert!(imm32.contexts.is_empty());
        assert!(imm32.context_handle_index.is_empty());
        assert!(imm32.default_contexts.is_empty());
        assert!(imm32.associations.is_empty());
        assert_eq!(imm32.next_context_id, 0);

        imm32.next_context_id = 1;
        imm32.ensure_window(main).unwrap();
        let association = imm32.associations[&main.window_id].clone();
        imm32.ensure_window(main).unwrap();
        assert_eq!(imm32.associations[&main.window_id], association);
        assert!(imm32.ensure_window(target(1, 0x4801_0002, 0x2000)).is_err());
        assert_eq!(imm32.associations[&main.window_id], association);
        assert_eq!(imm32.associations.len(), 1);
        assert_eq!(imm32.contexts.len(), 1);
        assert_eq!(imm32.next_context_id, 2);
        imm32.ensure_window(target(2, 0x4801_0002, 0x2000)).unwrap();
        let second = &imm32.associations[&RuntimeWindowId(2)];
        assert_eq!(second.context, association.context);
        assert_eq!(imm32.contexts().len(), 1);
        assert_eq!(imm32.associations().len(), 2);
    }

    #[test]
    fn wrong_owner_association_is_rejected_without_partial_state() {
        let mut imm32 = RuntimeImm32::default();
        let main = target(1, 0x4801_0001, 0x2000);
        imm32.ensure_window(main).unwrap();
        let association = imm32.associations[&main.window_id].clone();
        let context = association.context.unwrap();
        let other_thread = target(3, 0x4801_0003, 0x2001);
        imm32.ensure_window(other_thread).unwrap();
        let other_context = imm32.associations[&other_thread.window_id].context.unwrap();
        imm32
            .associate_context(other_thread, context.handle)
            .expect_err("HIMC belongs to another thread");
        assert_eq!(imm32.associations[&main.window_id].context, Some(context));
        assert_eq!(
            imm32.associations[&other_thread.window_id].context,
            Some(other_context)
        );
        assert_eq!(imm32.contexts.len(), 2);
        assert_eq!(imm32.associations.len(), 2);
        assert_eq!(
            imm32.context(context.handle).unwrap().owner_thread_id,
            main.owner_thread_id
        );
        assert_eq!(
            imm32.context(other_context.handle).unwrap().owner_thread_id,
            other_thread.owner_thread_id
        );
        assert_eq!(
            imm32.associate_context(other_thread, 0).unwrap(),
            other_context.handle
        );
        assert_eq!(
            imm32
                .associate_context(other_thread, other_context.handle)
                .unwrap(),
            0
        );
    }
}

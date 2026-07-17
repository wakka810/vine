use std::collections::BTreeMap;

use serde::Serialize;

use crate::memory::GuestAddress;
use crate::module_graph::RuntimeModuleId;
use crate::pe32::Rva;

use super::RuntimeStateError;

pub const LOGICAL_ICON_HANDLE_NAMESPACE: &str = "vine.logical-icon.v1";
pub const RUNTIME_SHARED_ICON_LIMIT: usize = 4_096;
pub const RUNTIME_ICON_HANDLE_BASE: u32 = 0x4901_0001;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeIconId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeIconResource {
    pub module_id: RuntimeModuleId,
    pub module_load_base: GuestAddress,
    pub module_source_sha256: String,
    pub group_resource_id: u32,
    pub language_id: u32,
    pub icon_resource_id: u32,
    pub group_data_rva: Rva,
    pub group_data_size: u32,
    pub icon_data_rva: Rva,
    pub icon_data_size: u32,
    pub width: u16,
    pub height: u16,
    pub planes: u16,
    pub bits_per_pixel: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeIcon {
    pub id: RuntimeIconId,
    pub handle: u32,
    pub namespace: String,
    pub shared: bool,
    pub resource: RuntimeIconResource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLoadIconAResult {
    pub icon_id: RuntimeIconId,
    pub handle: u32,
    pub already_loaded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeIconLoadDisposition {
    Existing(RuntimeIcon),
    Create {
        icon: RuntimeIcon,
        next_icon_id: u64,
        next_icon_handle: u32,
        next_generation: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeIconLoadPlan {
    expected_generation: u64,
    disposition: RuntimeIconLoadDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeUser32TeardownPlan {
    expected_generation: u64,
    next_generation: u64,
    retired_icons: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeUser32 {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    icons: BTreeMap<RuntimeIconId, RuntimeIcon>,
    #[serde(skip)]
    resource_index: BTreeMap<RuntimeIconResource, RuntimeIconId>,
    #[serde(skip)]
    handle_index: BTreeMap<u32, RuntimeIconId>,
    next_icon_id: u64,
    next_icon_handle: u32,
    generation: u64,
}

impl Default for RuntimeUser32 {
    fn default() -> Self {
        Self {
            icons: BTreeMap::new(),
            resource_index: BTreeMap::new(),
            handle_index: BTreeMap::new(),
            next_icon_id: 1,
            next_icon_handle: RUNTIME_ICON_HANDLE_BASE,
            generation: 0,
        }
    }
}

impl RuntimeUser32 {
    pub(super) fn plan_shared_icon_load(
        &self,
        resource: RuntimeIconResource,
    ) -> Result<RuntimeIconLoadPlan, RuntimeStateError> {
        validate_icon_resource(&resource)?;
        if let Some(id) = self.resource_index.get(&resource) {
            let icon = self.icons.get(id).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-user32",
                    "icon-resource-index-dangling",
                    "shared icon resource index does not resolve to a live icon",
                )
            })?;
            if self.handle_index.get(&icon.handle) != Some(id) {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-user32",
                    "icon-handle-index-mismatch",
                    "shared icon handle index does not match its resource identity",
                ));
            }
            return Ok(RuntimeIconLoadPlan {
                expected_generation: self.generation,
                disposition: RuntimeIconLoadDisposition::Existing(icon.clone()),
            });
        }
        if self.icons.len() >= RUNTIME_SHARED_ICON_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-user32",
                "shared-icon-limit-exceeded",
                "process shared icon count exceeds the runtime bound",
            ));
        }
        let id = RuntimeIconId(self.next_icon_id);
        if id.0 == 0 || self.icons.contains_key(&id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-user32",
                "icon-id-collision",
                "shared icon allocator selected an invalid or live identity",
            ));
        }
        if self.next_icon_handle < RUNTIME_ICON_HANDLE_BASE
            || self.handle_index.contains_key(&self.next_icon_handle)
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-user32",
                "icon-handle-collision",
                "shared icon allocator selected an invalid or live handle",
            ));
        }
        let next_icon_id = self.next_icon_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-user32",
                "icon-id-space-exhausted",
                "shared icon ID space is exhausted",
            )
        })?;
        let next_icon_handle = self.next_icon_handle.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-user32",
                "icon-handle-space-exhausted",
                "shared icon handle space is exhausted",
            )
        })?;
        Ok(RuntimeIconLoadPlan {
            expected_generation: self.generation,
            disposition: RuntimeIconLoadDisposition::Create {
                icon: RuntimeIcon {
                    id,
                    handle: self.next_icon_handle,
                    namespace: LOGICAL_ICON_HANDLE_NAMESPACE.to_owned(),
                    shared: true,
                    resource,
                },
                next_icon_id,
                next_icon_handle,
                next_generation: self.next_generation()?,
            },
        })
    }

    pub(super) fn preflight_shared_icon_load(
        &self,
        plan: &RuntimeIconLoadPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "icon-load-plan-stale",
                "shared icon state changed after load planning",
            ));
        }
        match &plan.disposition {
            RuntimeIconLoadDisposition::Existing(expected) => {
                if self.icons.get(&expected.id) != Some(expected)
                    || self.resource_index.get(&expected.resource) != Some(&expected.id)
                    || self.handle_index.get(&expected.handle) != Some(&expected.id)
                {
                    return Err(RuntimeStateError::conflict(
                        "runtime-user32",
                        "icon-load-plan-stale",
                        "shared icon identity changed after load planning",
                    ));
                }
            }
            RuntimeIconLoadDisposition::Create { icon, .. } => {
                if self.next_icon_id != icon.id.0
                    || self.next_icon_handle != icon.handle
                    || self.icons.contains_key(&icon.id)
                    || self.resource_index.contains_key(&icon.resource)
                    || self.handle_index.contains_key(&icon.handle)
                {
                    return Err(RuntimeStateError::conflict(
                        "runtime-user32",
                        "icon-load-plan-stale",
                        "shared icon allocator changed after load planning",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn commit_shared_icon_load(
        &mut self,
        plan: RuntimeIconLoadPlan,
    ) -> Result<RuntimeLoadIconAResult, RuntimeStateError> {
        self.preflight_shared_icon_load(&plan)?;
        match plan.disposition {
            RuntimeIconLoadDisposition::Existing(icon) => Ok(RuntimeLoadIconAResult {
                icon_id: icon.id,
                handle: icon.handle,
                already_loaded: true,
            }),
            RuntimeIconLoadDisposition::Create {
                icon,
                next_icon_id,
                next_icon_handle,
                next_generation,
            } => {
                let result = RuntimeLoadIconAResult {
                    icon_id: icon.id,
                    handle: icon.handle,
                    already_loaded: false,
                };
                self.next_icon_id = next_icon_id;
                self.next_icon_handle = next_icon_handle;
                self.generation = next_generation;
                let previous_resource = self.resource_index.insert(icon.resource.clone(), icon.id);
                let previous_handle = self.handle_index.insert(icon.handle, icon.id);
                let previous_icon = self.icons.insert(icon.id, icon);
                debug_assert!(previous_resource.is_none());
                debug_assert!(previous_handle.is_none());
                debug_assert!(previous_icon.is_none());
                Ok(result)
            }
        }
    }

    pub(super) fn plan_process_teardown(
        &self,
    ) -> Result<Option<RuntimeUser32TeardownPlan>, RuntimeStateError> {
        if self.icons.is_empty() {
            return Ok(None);
        }
        Ok(Some(RuntimeUser32TeardownPlan {
            expected_generation: self.generation,
            next_generation: self.next_generation()?,
            retired_icons: self.icons.len(),
        }))
    }

    pub(super) fn preflight_process_teardown(
        &self,
        plan: &RuntimeUser32TeardownPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.icons.len() != plan.retired_icons
            || self.resource_index.len() != plan.retired_icons
            || self.handle_index.len() != plan.retired_icons
        {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "user32-teardown-plan-stale",
                "shared icon state changed after process teardown planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_process_teardown(&mut self, plan: RuntimeUser32TeardownPlan) {
        debug_assert!(self.preflight_process_teardown(&plan).is_ok());
        self.icons.clear();
        self.resource_index.clear();
        self.handle_index.clear();
        self.generation = plan.next_generation;
    }

    pub fn icon(&self, handle: u32) -> Result<&RuntimeIcon, RuntimeStateError> {
        let id = self.handle_index.get(&handle).copied().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-user32",
                "icon-handle-unknown",
                format!("logical HICON {handle:#010x} is not live"),
            )
        })?;
        self.icons.get(&id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-user32",
                "icon-handle-index-dangling",
                "logical HICON index does not resolve to a live icon",
            )
        })
    }

    pub fn icons(&self) -> &BTreeMap<RuntimeIconId, RuntimeIcon> {
        &self.icons
    }

    pub(super) fn is_empty(&self) -> bool {
        self.icons.is_empty()
    }

    fn next_generation(&self) -> Result<u64, RuntimeStateError> {
        self.generation.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-user32",
                "user32-generation-exhausted",
                "User32 resource generation is exhausted",
            )
        })
    }
}

fn validate_icon_resource(resource: &RuntimeIconResource) -> Result<(), RuntimeStateError> {
    if resource.module_id.0 == 0
        || resource.module_load_base.0 == 0
        || resource.module_source_sha256.len() != 64
        || !resource
            .module_source_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || resource.group_resource_id > u32::from(u16::MAX)
        || resource.language_id > u32::from(u16::MAX)
        || resource.icon_resource_id > u32::from(u16::MAX)
        || resource.group_data_rva.0 == 0
        || resource.group_data_size == 0
        || resource.icon_data_rva.0 == 0
        || resource.icon_data_size == 0
        || resource.width == 0
        || resource.width > 256
        || resource.height == 0
        || resource.height > 256
        || resource.planes == 0
        || resource.bits_per_pixel == 0
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-user32",
            "icon-resource-invalid",
            "shared icon resource identity and dimensions must be bounded PE32 metadata",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_icon_plans_reuse_identity_and_teardown_atomically() {
        let mut user32 = RuntimeUser32::default();
        let first_resource = resource(101, 3, 0x3000);
        let first_plan = user32
            .plan_shared_icon_load(first_resource.clone())
            .unwrap();
        assert!(user32.icons().is_empty());
        let first = user32.commit_shared_icon_load(first_plan).unwrap();
        assert_eq!(first.icon_id, RuntimeIconId(1));
        assert_eq!(first.handle, RUNTIME_ICON_HANDLE_BASE);
        assert!(!first.already_loaded);

        let repeated_plan = user32
            .plan_shared_icon_load(first_resource)
            .expect("repeated icon plan");
        let repeated = user32.commit_shared_icon_load(repeated_plan).unwrap();
        assert_eq!(repeated.icon_id, first.icon_id);
        assert_eq!(repeated.handle, first.handle);
        assert!(repeated.already_loaded);

        let stale = user32
            .plan_shared_icon_load(resource(102, 4, 0x4000))
            .unwrap();
        let intervening_plan = user32
            .plan_shared_icon_load(resource(103, 5, 0x5000))
            .unwrap();
        user32.commit_shared_icon_load(intervening_plan).unwrap();
        let before_stale = user32.clone();
        assert_eq!(
            user32.commit_shared_icon_load(stale).unwrap_err().code,
            "icon-load-plan-stale"
        );
        assert_eq!(user32, before_stale);

        let stale_teardown = user32.plan_process_teardown().unwrap().unwrap();
        let later_plan = user32
            .plan_shared_icon_load(resource(104, 6, 0x6000))
            .unwrap();
        user32.commit_shared_icon_load(later_plan).unwrap();
        assert_eq!(
            user32
                .preflight_process_teardown(&stale_teardown)
                .unwrap_err()
                .code,
            "user32-teardown-plan-stale"
        );
        let teardown = user32.plan_process_teardown().unwrap().unwrap();
        user32.preflight_process_teardown(&teardown).unwrap();
        user32.commit_process_teardown(teardown);
        assert!(user32.icons().is_empty());
        assert!(user32.resource_index.is_empty());
        assert!(user32.handle_index.is_empty());
    }

    #[test]
    fn shared_icon_capacity_failure_preserves_identity_and_allocators() {
        let mut user32 = RuntimeUser32::default();
        for ordinal in 1..=RUNTIME_SHARED_ICON_LIMIT {
            let ordinal = u32::try_from(ordinal).unwrap();
            let plan = user32
                .plan_shared_icon_load(resource(ordinal, ordinal, 0x3000 + ordinal))
                .unwrap();
            let result = user32.commit_shared_icon_load(plan).unwrap();
            assert_eq!(result.icon_id.0, u64::from(ordinal));
        }
        let before = user32.clone();
        let error = user32
            .plan_shared_icon_load(resource(4_097, 4_097, 0x5000))
            .expect_err("shared icon capacity must be bounded");
        assert_eq!(
            error.class,
            super::super::RuntimeStateErrorClass::ResourceLimit
        );
        assert_eq!(error.code, "shared-icon-limit-exceeded");
        assert_eq!(user32, before);
    }

    fn resource(
        group_resource_id: u32,
        icon_resource_id: u32,
        icon_data_rva: u32,
    ) -> RuntimeIconResource {
        RuntimeIconResource {
            module_id: RuntimeModuleId(1),
            module_load_base: GuestAddress(0x1000_0000),
            module_source_sha256: "0".repeat(64),
            group_resource_id,
            language_id: 1_041,
            icon_resource_id,
            group_data_rva: Rva(0x2000),
            group_data_size: 48,
            icon_data_rva: Rva(icon_data_rva),
            icon_data_size: 4_264,
            width: 32,
            height: 32,
            planes: 1,
            bits_per_pixel: 32,
        }
    }
}

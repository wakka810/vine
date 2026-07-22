use serde::Serialize;

use super::RuntimeStateError;

pub const LOGICAL_MONITOR_HANDLE_NAMESPACE: &str = "vine.logical-monitor.v1";
pub const RUNTIME_PRIMARY_MONITOR_HANDLE: u32 = 0x4f01_0001;

const MAX_DISPLAY_PROFILE_ID_BYTES: usize = 128;
const MAX_LOGICAL_MONITOR_DIMENSION: i32 = 32_768;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl RuntimeRect {
    pub fn to_x86_bytes(self) -> Vec<u8> {
        [self.left, self.top, self.right, self.bottom]
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDisplayProfile {
    pub id: String,
    pub primary_monitor_rect: RuntimeRect,
    pub primary_work_rect: RuntimeRect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLogicalMonitor {
    pub namespace: String,
    pub handle: u32,
    pub primary: bool,
    pub rect: RuntimeRect,
    pub work_rect: RuntimeRect,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeDisplays {
    #[serde(skip_serializing_if = "Option::is_none")]
    profile_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    primary_monitor: Option<RuntimeLogicalMonitor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeDisplayConfigurationPlan {
    profile_id: String,
    primary_monitor: RuntimeLogicalMonitor,
}

impl RuntimeDisplays {
    pub(super) fn plan_configuration(
        &self,
        profile: RuntimeDisplayProfile,
    ) -> Result<RuntimeDisplayConfigurationPlan, RuntimeStateError> {
        if self.profile_id.is_some() || self.primary_monitor.is_some() {
            return Err(RuntimeStateError::conflict(
                "runtime-display",
                "display-profile-already-configured",
                "logical display profile may be configured only once",
            ));
        }
        if profile.id.is_empty()
            || profile.id.len() > MAX_DISPLAY_PROFILE_ID_BYTES
            || !profile.id.is_ascii()
            || profile.id.as_bytes().contains(&0)
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-display",
                "display-profile-id-invalid",
                "logical display profile ID must be bounded nonempty ASCII",
            ));
        }
        let rect = profile.primary_monitor_rect;
        let work_rect = profile.primary_work_rect;
        if rect.left != 0
            || rect.top != 0
            || rect.right <= rect.left
            || rect.bottom <= rect.top
            || rect.right > MAX_LOGICAL_MONITOR_DIMENSION
            || rect.bottom > MAX_LOGICAL_MONITOR_DIMENSION
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-display",
                "primary-monitor-rect-invalid",
                "single logical primary monitor requires a bounded positive rectangle at the virtual origin",
            ));
        }
        if work_rect.left < rect.left
            || work_rect.top < rect.top
            || work_rect.right <= work_rect.left
            || work_rect.bottom <= work_rect.top
            || work_rect.right > rect.right
            || work_rect.bottom > rect.bottom
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-display",
                "primary-work-rect-invalid",
                "logical primary work rectangle must be positive and contained by its monitor rectangle",
            ));
        }
        Ok(RuntimeDisplayConfigurationPlan {
            profile_id: profile.id,
            primary_monitor: RuntimeLogicalMonitor {
                namespace: LOGICAL_MONITOR_HANDLE_NAMESPACE.to_owned(),
                handle: RUNTIME_PRIMARY_MONITOR_HANDLE,
                primary: true,
                rect,
                work_rect,
            },
        })
    }

    pub(super) fn commit_configuration(
        &mut self,
        plan: RuntimeDisplayConfigurationPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.profile_id.is_some() || self.primary_monitor.is_some() {
            return Err(RuntimeStateError::conflict(
                "runtime-display",
                "display-configuration-plan-stale",
                "logical display configuration plan no longer targets an empty owner",
            ));
        }
        self.profile_id = Some(plan.profile_id);
        self.primary_monitor = Some(plan.primary_monitor);
        Ok(())
    }

    pub(super) fn profile_id(&self) -> Option<&str> {
        self.profile_id.as_deref()
    }

    pub(super) fn primary_monitor(&self) -> Option<&RuntimeLogicalMonitor> {
        self.primary_monitor.as_ref()
    }

    pub(super) fn monitor(&self, handle: u32) -> Result<&RuntimeLogicalMonitor, RuntimeStateError> {
        self.primary_monitor
            .as_ref()
            .filter(|monitor| monitor.handle == handle)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-display",
                    "logical-monitor-handle-unknown",
                    format!("logical HMONITOR {handle:#010x} is not live"),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> RuntimeDisplayProfile {
        RuntimeDisplayProfile {
            id: "single-monitor-1920x1080-v1".to_owned(),
            primary_monitor_rect: RuntimeRect {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            },
            primary_work_rect: RuntimeRect {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1032,
            },
        }
    }

    #[test]
    fn configuration_is_single_owner_and_rejects_invalid_geometry_without_mutation() {
        let mut displays = RuntimeDisplays::default();
        let mut invalid = profile();
        invalid.primary_monitor_rect.right = 0;
        assert_eq!(
            displays.plan_configuration(invalid).unwrap_err().code,
            "primary-monitor-rect-invalid"
        );
        assert!(displays.profile_id().is_none());
        assert!(displays.primary_monitor().is_none());

        let plan = displays.plan_configuration(profile()).unwrap();
        let stale = plan.clone();
        displays.commit_configuration(plan).unwrap();
        assert_eq!(displays.profile_id(), Some("single-monitor-1920x1080-v1"));
        let monitor = displays.primary_monitor().unwrap();
        assert_eq!(monitor.handle, RUNTIME_PRIMARY_MONITOR_HANDLE);
        assert_eq!(monitor.rect.to_x86_bytes().len(), 16);
        assert_eq!(monitor.work_rect.bottom, 1032);
        assert_eq!(displays.monitor(monitor.handle).unwrap(), monitor);
        assert_eq!(
            displays.monitor(0).unwrap_err().code,
            "logical-monitor-handle-unknown"
        );
        assert_eq!(
            displays.commit_configuration(stale).unwrap_err().code,
            "display-configuration-plan-stale"
        );
        assert_eq!(displays.profile_id(), Some("single-monitor-1920x1080-v1"));
        assert_eq!(
            displays.primary_monitor().unwrap().handle,
            RUNTIME_PRIMARY_MONITOR_HANDLE
        );
        assert_eq!(
            displays.plan_configuration(profile()).unwrap_err().code,
            "display-profile-already-configured"
        );
    }

    #[test]
    fn configuration_rejects_work_area_outside_monitor_without_mutation() {
        let displays = RuntimeDisplays::default();
        let mut invalid = profile();
        invalid.primary_work_rect.bottom = 1081;
        assert_eq!(
            displays.plan_configuration(invalid).unwrap_err().code,
            "primary-work-rect-invalid"
        );
        assert!(displays.profile_id().is_none());
        assert!(displays.primary_monitor().is_none());
    }
}

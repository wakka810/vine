use super::RuntimeStateError;

pub const RUNTIME_PRIMARY_MONITOR_HANDLE: u32 = 0x4f01_0001;

const MAX_LOGICAL_MONITOR_DIMENSION: i32 = 32_768;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDisplayProfile {
    pub primary_monitor_rect: RuntimeRect,
    pub primary_work_rect: RuntimeRect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLogicalMonitor {
    pub handle: u32,
    pub primary: bool,
    pub rect: RuntimeRect,
    pub work_rect: RuntimeRect,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RuntimeDisplays {
    primary_monitor: Option<RuntimeLogicalMonitor>,
}

impl RuntimeDisplays {
    pub(super) fn configure(
        &mut self,
        profile: RuntimeDisplayProfile,
    ) -> Result<(), RuntimeStateError> {
        if self.primary_monitor.is_some() {
            return Err(RuntimeStateError::new(
                "logical display profile may be configured only once",
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
            return Err(RuntimeStateError::new(
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
            return Err(RuntimeStateError::new(
                "logical primary work rectangle must be positive and contained by its monitor rectangle",
            ));
        }
        self.primary_monitor = Some(RuntimeLogicalMonitor {
            handle: RUNTIME_PRIMARY_MONITOR_HANDLE,
            primary: true,
            rect,
            work_rect,
        });
        Ok(())
    }

    pub(super) fn primary_monitor(&self) -> Option<&RuntimeLogicalMonitor> {
        self.primary_monitor.as_ref()
    }

    pub(super) fn monitor(&self, handle: u32) -> Option<&RuntimeLogicalMonitor> {
        self.primary_monitor
            .as_ref()
            .filter(|monitor| monitor.handle == handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> RuntimeDisplayProfile {
        RuntimeDisplayProfile {
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
        displays.configure(invalid).unwrap_err();
        assert!(displays.primary_monitor().is_none());

        displays.configure(profile()).unwrap();
        let monitor = displays.primary_monitor().unwrap();
        assert_eq!(monitor.handle, RUNTIME_PRIMARY_MONITOR_HANDLE);
        assert_eq!(monitor.rect.to_x86_bytes().len(), 16);
        assert_eq!(monitor.work_rect.bottom, 1032);
        assert_eq!(displays.monitor(monitor.handle).unwrap(), monitor);
        assert!(displays.monitor(0).is_none());
        assert_eq!(
            displays.primary_monitor().unwrap().handle,
            RUNTIME_PRIMARY_MONITOR_HANDLE
        );
        displays.configure(profile()).unwrap_err();
    }

    #[test]
    fn configuration_rejects_work_area_outside_monitor_without_mutation() {
        let mut displays = RuntimeDisplays::default();
        let mut invalid = profile();
        invalid.primary_work_rect.bottom = 1081;
        displays.configure(invalid).unwrap_err();
        assert!(displays.primary_monitor().is_none());
    }
}

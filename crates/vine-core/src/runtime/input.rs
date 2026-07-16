use serde::Serialize;

use super::RuntimeStateError;

const MAX_INPUT_PROFILE_ID_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInputProfile {
    pub id: String,
    pub initial_mouse_buttons_swapped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeMouseButtonMapping {
    pub profile_id: String,
    pub initial_swapped: bool,
    pub swapped: bool,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSwapMouseButtonResult {
    pub previous_swapped: bool,
    pub current_swapped: bool,
    pub generation: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    mouse_button_mapping: Option<RuntimeMouseButtonMapping>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeInputConfigurationPlan {
    mapping: RuntimeMouseButtonMapping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeMouseButtonSwapPlan {
    expected_generation: u64,
    expected_swapped: bool,
    next_swapped: bool,
    next_generation: u64,
}

impl RuntimeInput {
    pub(super) fn plan_configuration(
        &self,
        profile: RuntimeInputProfile,
    ) -> Result<RuntimeInputConfigurationPlan, RuntimeStateError> {
        if self.mouse_button_mapping.is_some() {
            return Err(RuntimeStateError::conflict(
                "runtime-input",
                "input-profile-already-configured",
                "logical input profile may be configured only once",
            ));
        }
        if profile.id.is_empty()
            || profile.id.len() > MAX_INPUT_PROFILE_ID_BYTES
            || !profile.id.is_ascii()
            || profile.id.as_bytes().contains(&0)
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-input",
                "input-profile-id-invalid",
                "logical input profile ID must be bounded nonempty ASCII",
            ));
        }
        Ok(RuntimeInputConfigurationPlan {
            mapping: RuntimeMouseButtonMapping {
                profile_id: profile.id,
                initial_swapped: profile.initial_mouse_buttons_swapped,
                swapped: profile.initial_mouse_buttons_swapped,
                generation: 0,
            },
        })
    }

    pub(super) fn commit_configuration(
        &mut self,
        plan: RuntimeInputConfigurationPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.mouse_button_mapping.is_some() {
            return Err(RuntimeStateError::conflict(
                "runtime-input",
                "input-configuration-plan-stale",
                "logical input configuration plan no longer targets an empty owner",
            ));
        }
        self.mouse_button_mapping = Some(plan.mapping);
        Ok(())
    }

    pub(super) fn plan_mouse_button_swap(
        &self,
        swapped: bool,
    ) -> Result<RuntimeMouseButtonSwapPlan, RuntimeStateError> {
        let mapping = self.mouse_button_mapping.as_ref().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-input",
                "mouse-button-mapping-unconfigured",
                "logical mouse button mapping must be configured before it is changed",
            )
        })?;
        let next_generation = if mapping.swapped == swapped {
            mapping.generation
        } else {
            mapping.generation.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-input",
                    "mouse-button-mapping-generation-overflow",
                    "logical mouse button mapping generation is exhausted",
                )
            })?
        };
        Ok(RuntimeMouseButtonSwapPlan {
            expected_generation: mapping.generation,
            expected_swapped: mapping.swapped,
            next_swapped: swapped,
            next_generation,
        })
    }

    pub(super) fn commit_mouse_button_swap(
        &mut self,
        plan: RuntimeMouseButtonSwapPlan,
    ) -> Result<RuntimeSwapMouseButtonResult, RuntimeStateError> {
        let mapping = self.mouse_button_mapping.as_mut().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-input",
                "mouse-button-mapping-unconfigured",
                "logical mouse button mapping disappeared before commit",
            )
        })?;
        if mapping.generation != plan.expected_generation
            || mapping.swapped != plan.expected_swapped
        {
            return Err(RuntimeStateError::conflict(
                "runtime-input",
                "mouse-button-swap-plan-stale",
                "logical mouse button mapping changed after the swap was planned",
            ));
        }
        mapping.swapped = plan.next_swapped;
        mapping.generation = plan.next_generation;
        Ok(RuntimeSwapMouseButtonResult {
            previous_swapped: plan.expected_swapped,
            current_swapped: mapping.swapped,
            generation: mapping.generation,
        })
    }

    pub(super) fn mouse_button_mapping(&self) -> Option<&RuntimeMouseButtonMapping> {
        self.mouse_button_mapping.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(initial_mouse_buttons_swapped: bool) -> RuntimeInputProfile {
        RuntimeInputProfile {
            id: "normal-mouse-buttons-v1".to_owned(),
            initial_mouse_buttons_swapped,
        }
    }

    #[test]
    fn configuration_and_swaps_are_planned_and_generation_tracks_only_changes() {
        let mut input = RuntimeInput::default();
        let configuration = input.plan_configuration(profile(false)).unwrap();
        let stale_configuration = configuration.clone();
        input.commit_configuration(configuration).unwrap();
        assert_eq!(
            input
                .commit_configuration(stale_configuration)
                .unwrap_err()
                .code,
            "input-configuration-plan-stale"
        );

        let no_change = input.plan_mouse_button_swap(false).unwrap();
        let result = input.commit_mouse_button_swap(no_change).unwrap();
        assert!(!result.previous_swapped);
        assert!(!result.current_swapped);
        assert_eq!(result.generation, 0);

        let swap = input.plan_mouse_button_swap(true).unwrap();
        let stale = swap;
        let result = input.commit_mouse_button_swap(swap).unwrap();
        assert!(!result.previous_swapped);
        assert!(result.current_swapped);
        assert_eq!(result.generation, 1);
        assert_eq!(
            input.commit_mouse_button_swap(stale).unwrap_err().code,
            "mouse-button-swap-plan-stale"
        );
        assert_eq!(input.mouse_button_mapping().unwrap().generation, 1);
    }

    #[test]
    fn invalid_or_missing_profiles_fail_without_mutation() {
        let input = RuntimeInput::default();
        let before = input.clone();
        let mut invalid = profile(false);
        invalid.id.clear();
        assert_eq!(
            input.plan_configuration(invalid).unwrap_err().code,
            "input-profile-id-invalid"
        );
        assert_eq!(input, before);
        assert_eq!(
            input.plan_mouse_button_swap(true).unwrap_err().code,
            "mouse-button-mapping-unconfigured"
        );
        assert_eq!(input, before);
    }
}

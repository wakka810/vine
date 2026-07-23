use std::collections::BTreeMap;

use serde::Serialize;

use super::RuntimeStateError;

pub const RUNTIME_GDIPLUS_SESSION_TOKEN_NAMESPACE: &str = "vine.logical-gdiplus-session-token.v1";
pub const RUNTIME_GDIPLUS_SESSION_TOKEN_PREFIX: u32 = 0x4750_0000;
pub const RUNTIME_GDIPLUS_SESSION_LIMIT: usize = 256;

const MAX_STARTUP_PROFILE_ID_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeGdiplusSessionId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGdiplusStartupRequest {
    pub profile_id: String,
    pub version: u32,
    pub debug_event_callback: u32,
    pub suppress_background_thread: bool,
    pub suppress_external_codecs: bool,
    pub notification_output_requested: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGdiplusSession {
    pub id: RuntimeGdiplusSessionId,
    pub token_namespace: String,
    pub token_id: u16,
    pub token: u32,
    pub startup: RuntimeGdiplusStartupRequest,
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeGdiplus {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    sessions: BTreeMap<RuntimeGdiplusSessionId, RuntimeGdiplusSession>,
    #[serde(skip)]
    token_index: BTreeMap<u32, RuntimeGdiplusSessionId>,
    next_session_id: u64,
    next_token_id: u16,
    generation: u64,
}

impl Default for RuntimeGdiplus {
    fn default() -> Self {
        Self {
            sessions: BTreeMap::new(),
            token_index: BTreeMap::new(),
            next_session_id: 1,
            next_token_id: 1,
            generation: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeGdiplusStartupPlan {
    expected_generation: u64,
    next_generation: u64,
    expected_next_session_id: u64,
    next_session_id: u64,
    expected_next_token_id: u16,
    next_token_id: u16,
    session: RuntimeGdiplusSession,
}

impl RuntimeGdiplusStartupPlan {
    pub(super) fn session(&self) -> &RuntimeGdiplusSession {
        &self.session
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeGdiplusShutdownPlan {
    expected_generation: u64,
    next_generation: u64,
    session: RuntimeGdiplusSession,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeGdiplusTeardownPlan {
    expected_generation: u64,
    next_generation: u64,
    sessions: usize,
}

impl RuntimeGdiplus {
    pub(super) fn sessions(&self) -> &BTreeMap<RuntimeGdiplusSessionId, RuntimeGdiplusSession> {
        &self.sessions
    }

    pub(super) fn plan_startup(
        &self,
        request: RuntimeGdiplusStartupRequest,
    ) -> Result<RuntimeGdiplusStartupPlan, RuntimeStateError> {
        validate_startup_request(&request)?;
        if self.sessions.len() >= RUNTIME_GDIPLUS_SESSION_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-gdiplus",
                "gdiplus-session-capacity-exhausted",
                "logical GDI+ session capacity is exhausted",
            ));
        }
        if self.next_session_id == 0 {
            return Err(RuntimeStateError::resource_limit(
                "runtime-gdiplus",
                "gdiplus-session-id-space-exhausted",
                "logical GDI+ session identity space is exhausted",
            ));
        }
        if self.next_token_id == 0 {
            return Err(RuntimeStateError::resource_limit(
                "runtime-gdiplus",
                "gdiplus-session-token-space-exhausted",
                "logical GDI+ session token space is exhausted",
            ));
        }
        let id = RuntimeGdiplusSessionId(self.next_session_id);
        let token_id = self.next_token_id;
        let token = RUNTIME_GDIPLUS_SESSION_TOKEN_PREFIX | u32::from(token_id);
        if self.sessions.contains_key(&id) || self.token_index.contains_key(&token) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-gdiplus",
                "gdiplus-session-identity-collision",
                "logical GDI+ session allocator selected a live identity",
            ));
        }
        let next_session_id = self.next_session_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-gdiplus",
                "gdiplus-session-id-space-exhausted",
                "logical GDI+ session identity space is exhausted",
            )
        })?;
        let next_generation = checked_next_generation(self.generation)?;
        Ok(RuntimeGdiplusStartupPlan {
            expected_generation: self.generation,
            next_generation,
            expected_next_session_id: self.next_session_id,
            next_session_id,
            expected_next_token_id: self.next_token_id,
            next_token_id: self.next_token_id.checked_add(1).unwrap_or(0),
            session: RuntimeGdiplusSession {
                id,
                token_namespace: RUNTIME_GDIPLUS_SESSION_TOKEN_NAMESPACE.to_owned(),
                token_id,
                token,
                startup: request,
                generation: next_generation,
            },
        })
    }

    pub(super) fn preflight_startup(
        &self,
        plan: &RuntimeGdiplusStartupPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.next_session_id != plan.expected_next_session_id
            || self.next_token_id != plan.expected_next_token_id
            || self.sessions.len() >= RUNTIME_GDIPLUS_SESSION_LIMIT
            || self.sessions.contains_key(&plan.session.id)
            || self.token_index.contains_key(&plan.session.token)
            || plan.session.id.0 != plan.expected_next_session_id
            || plan.session.token_id != plan.expected_next_token_id
            || plan.session.token
                != RUNTIME_GDIPLUS_SESSION_TOKEN_PREFIX | u32::from(plan.session.token_id)
            || plan.session.generation != plan.next_generation
        {
            return Err(stale_plan("gdiplus-startup-plan-stale"));
        }
        validate_startup_request(&plan.session.startup)
    }

    pub(super) fn commit_startup(
        &mut self,
        plan: RuntimeGdiplusStartupPlan,
    ) -> Result<RuntimeGdiplusSession, RuntimeStateError> {
        self.preflight_startup(&plan)?;
        let session = plan.session;
        self.sessions.insert(session.id, session.clone());
        self.token_index.insert(session.token, session.id);
        self.next_session_id = plan.next_session_id;
        self.next_token_id = plan.next_token_id;
        self.generation = plan.next_generation;
        Ok(session)
    }

    pub(super) fn plan_shutdown(
        &self,
        token: u32,
    ) -> Result<RuntimeGdiplusShutdownPlan, RuntimeStateError> {
        let session = self.session_by_token(token)?.clone();
        Ok(RuntimeGdiplusShutdownPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            session,
        })
    }

    pub(super) fn preflight_shutdown(
        &self,
        plan: &RuntimeGdiplusShutdownPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.sessions.get(&plan.session.id) != Some(&plan.session)
            || self.token_index.get(&plan.session.token) != Some(&plan.session.id)
        {
            return Err(stale_plan("gdiplus-shutdown-plan-stale"));
        }
        Ok(())
    }

    pub(super) fn commit_shutdown(
        &mut self,
        plan: RuntimeGdiplusShutdownPlan,
    ) -> Result<RuntimeGdiplusSession, RuntimeStateError> {
        self.preflight_shutdown(&plan)?;
        self.token_index.remove(&plan.session.token);
        let session = self
            .sessions
            .remove(&plan.session.id)
            .expect("preflighted GDI+ session disappeared");
        self.generation = plan.next_generation;
        Ok(session)
    }

    pub(super) fn plan_process_teardown(
        &self,
    ) -> Result<Option<RuntimeGdiplusTeardownPlan>, RuntimeStateError> {
        if self.is_empty() {
            return Ok(None);
        }
        Ok(Some(RuntimeGdiplusTeardownPlan {
            expected_generation: self.generation,
            next_generation: checked_next_generation(self.generation)?,
            sessions: self.sessions.len(),
        }))
    }

    pub(super) fn preflight_process_teardown(
        &self,
        plan: &RuntimeGdiplusTeardownPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.sessions.len() != plan.sessions
            || self.token_index.len() != plan.sessions
        {
            return Err(stale_plan("gdiplus-teardown-plan-stale"));
        }
        Ok(())
    }

    pub(super) fn commit_process_teardown(&mut self, plan: RuntimeGdiplusTeardownPlan) {
        self.sessions.clear();
        self.token_index.clear();
        self.generation = plan.next_generation;
    }

    pub(super) fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    fn session_by_token(&self, token: u32) -> Result<&RuntimeGdiplusSession, RuntimeStateError> {
        let token_id = token & 0xffff;
        if token_id == 0 || token & 0xffff_0000 != RUNTIME_GDIPLUS_SESSION_TOKEN_PREFIX {
            return Err(RuntimeStateError::invalid_input(
                "runtime-gdiplus",
                "gdiplus-session-token-invalid",
                format!("GDI+ token {token:#010x} is outside the runtime-owned namespace"),
            ));
        }
        let id = self.token_index.get(&token).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-gdiplus",
                "gdiplus-session-token-unknown",
                format!("GDI+ token {token:#010x} is not live"),
            )
        })?;
        let session = self.sessions.get(id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-gdiplus",
                "gdiplus-session-token-index-dangling",
                "logical GDI+ token index references a missing session",
            )
        })?;
        if session.token != token
            || u32::from(session.token_id) != token_id
            || session.token_namespace != RUNTIME_GDIPLUS_SESSION_TOKEN_NAMESPACE
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-gdiplus",
                "gdiplus-session-record-inconsistent",
                "logical GDI+ session does not match its token index",
            ));
        }
        Ok(session)
    }
}

fn validate_startup_request(
    request: &RuntimeGdiplusStartupRequest,
) -> Result<(), RuntimeStateError> {
    if request.profile_id.is_empty()
        || request.profile_id.len() > MAX_STARTUP_PROFILE_ID_BYTES
        || !request.profile_id.is_ascii()
        || request.profile_id.as_bytes().contains(&0)
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-gdiplus",
            "gdiplus-startup-profile-id-invalid",
            "logical GDI+ startup profile requires bounded nonempty ASCII identity",
        ));
    }
    Ok(())
}

fn checked_next_generation(generation: u64) -> Result<u64, RuntimeStateError> {
    generation.checked_add(1).ok_or_else(|| {
        RuntimeStateError::resource_limit(
            "runtime-gdiplus",
            "gdiplus-generation-exhausted",
            "logical GDI+ generation is exhausted",
        )
    })
}

fn stale_plan(code: &'static str) -> RuntimeStateError {
    RuntimeStateError::conflict(
        "runtime-gdiplus",
        code,
        "logical GDI+ state changed after the mutation was planned",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(profile_id: &str) -> RuntimeGdiplusStartupRequest {
        RuntimeGdiplusStartupRequest {
            profile_id: profile_id.to_owned(),
            version: 1,
            debug_event_callback: 0,
            suppress_background_thread: false,
            suppress_external_codecs: false,
            notification_output_requested: false,
        }
    }

    #[test]
    fn startup_and_shutdown_plans_are_mutation_free_and_relation_checked() {
        let mut gdiplus = RuntimeGdiplus::default();
        let before = gdiplus.clone();
        let first_plan = gdiplus.plan_startup(request("primary-v1")).unwrap();
        let stale_startup = first_plan.clone();
        assert_eq!(gdiplus, before);
        assert_eq!(
            first_plan.session().token,
            RUNTIME_GDIPLUS_SESSION_TOKEN_PREFIX | 1
        );
        let first = gdiplus.commit_startup(first_plan).unwrap();
        assert_eq!(first.id, RuntimeGdiplusSessionId(1));
        assert_eq!(gdiplus.sessions().len(), 1);
        assert_eq!(
            gdiplus.commit_startup(stale_startup).unwrap_err().code,
            "gdiplus-startup-plan-stale"
        );

        let shutdown = gdiplus.plan_shutdown(first.token).unwrap();
        let stale_shutdown = shutdown.clone();
        let removed = gdiplus.commit_shutdown(shutdown).unwrap();
        assert_eq!(removed, first);
        assert!(gdiplus.is_empty());
        assert_eq!(
            gdiplus.commit_shutdown(stale_shutdown).unwrap_err().code,
            "gdiplus-shutdown-plan-stale"
        );
        assert_eq!(
            gdiplus.plan_shutdown(first.token).unwrap_err().code,
            "gdiplus-session-token-unknown"
        );
        assert_eq!(
            gdiplus.plan_shutdown(1).unwrap_err().code,
            "gdiplus-session-token-invalid"
        );
    }

    #[test]
    fn distinct_sessions_have_non_native_monotonic_tokens_and_bounded_capacity() {
        let mut gdiplus = RuntimeGdiplus::default();
        for index in 0..RUNTIME_GDIPLUS_SESSION_LIMIT {
            let plan = gdiplus
                .plan_startup(request(&format!("session-{index}")))
                .unwrap();
            let session = gdiplus.commit_startup(plan).unwrap();
            assert_eq!(session.id.0, index as u64 + 1);
            assert_eq!(u32::from(session.token_id), index as u32 + 1);
            assert_eq!(
                session.token,
                RUNTIME_GDIPLUS_SESSION_TOKEN_PREFIX | (index as u32 + 1)
            );
        }
        let before = gdiplus.clone();
        assert_eq!(
            gdiplus
                .plan_startup(request("over-capacity"))
                .unwrap_err()
                .code,
            "gdiplus-session-capacity-exhausted"
        );
        assert_eq!(gdiplus, before);
    }

    #[test]
    fn process_teardown_rejects_stale_plan_and_clears_both_indexes() {
        let mut gdiplus = RuntimeGdiplus::default();
        let first = gdiplus
            .commit_startup(gdiplus.plan_startup(request("first")).unwrap())
            .unwrap();
        let teardown = gdiplus.plan_process_teardown().unwrap().unwrap();
        let stale = teardown;
        let second = gdiplus
            .commit_startup(gdiplus.plan_startup(request("second")).unwrap())
            .unwrap();
        assert_eq!(
            gdiplus.preflight_process_teardown(&stale).unwrap_err().code,
            "gdiplus-teardown-plan-stale"
        );
        assert!(gdiplus.plan_shutdown(first.token).is_ok());
        assert!(gdiplus.plan_shutdown(second.token).is_ok());

        let teardown = gdiplus.plan_process_teardown().unwrap().unwrap();
        gdiplus.preflight_process_teardown(&teardown).unwrap();
        gdiplus.commit_process_teardown(teardown);
        assert!(gdiplus.is_empty());
        assert!(gdiplus.plan_process_teardown().unwrap().is_none());
        assert_eq!(
            gdiplus.plan_shutdown(first.token).unwrap_err().code,
            "gdiplus-session-token-unknown"
        );
    }

    #[test]
    fn invalid_profile_fails_before_allocator_mutation() {
        let gdiplus = RuntimeGdiplus::default();
        let before = gdiplus.clone();
        assert_eq!(
            gdiplus.plan_startup(request("")).unwrap_err().code,
            "gdiplus-startup-profile-id-invalid"
        );
        assert_eq!(gdiplus, before);
    }
}

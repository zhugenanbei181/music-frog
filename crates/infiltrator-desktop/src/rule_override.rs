//! DUAL-12-08: desktop host adapter for the tracer's one-click reverse-apply.
//!
//! Loads the current profile's rule list and commits a rewritten list through
//! the same CORE-004 apply transaction the editor save path uses (atomic write,
//! reload-or-restart, readiness, rollback). It shares the runtime's apply
//! guard so a tracer override cannot interleave with another config apply.

use async_trait::async_trait;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::rule_source_identity::rule_workspace;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::rule_location::RuleLocation;
use infiltrator_contract::rule_source::RuleSourceIdentity;
use infiltrator_core::apply::{
    ApplyError, ApplyParams, EndpointConfigReloader, apply_confirmed_profile,
};
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::rules::source_identity::identify_rules_document;
use infiltrator_domain::rules::{RuleEntry, apply_rules_to_yaml, load_rules_from_yaml};
use infiltrator_domain::yaml_edit::rule_location::replace_rule_at_location;
use infiltrator_ports::endpoint::EndpointSource;
use infiltrator_ports::error::PortError;
use infiltrator_ports::rule_tracer::{RuleOverridePort, RuleWorkspace};
use mihomo_api::error::MihomoError;
use mihomo_config::manager::ConfigManager;
use mihomo_platform::defaults::DefaultCredentialStore;
use std::io::ErrorKind;
use std::sync::{Arc, Weak};
use tokio::sync::Mutex;

/// Host persistence capability behind the shared rule override command.
pub struct DesktopRuleOverridePort {
    config: Arc<ConfigManager<DefaultCredentialStore>>,
    session: Weak<CoreApplication>,
    endpoints: Arc<dyn EndpointSource>,
    apply_guard: Arc<Mutex<()>>,
}

impl DesktopRuleOverridePort {
    pub fn new(
        config: Arc<ConfigManager<DefaultCredentialStore>>,
        session: Arc<CoreApplication>,
        endpoints: Arc<dyn EndpointSource>,
        apply_guard: Arc<Mutex<()>>,
    ) -> Self {
        Self {
            config,
            session: Arc::downgrade(&session),
            endpoints,
            apply_guard,
        }
    }
    async fn commit_updated(
        &self,
        source: &RuleSourceIdentity,
        profile: &str,
        updated: &str,
    ) -> Result<(), PortError> {
        let reloader = EndpointConfigReloader::new(self.endpoints.clone());
        let params = ApplyParams {
            strategy: ApplyStrategy::PreferReload,
            ..ApplyParams::default()
        };
        let session = self.session.upgrade().ok_or_else(|| {
            PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "The product session has closed",
                false,
            ))
        })?;
        apply_confirmed_profile(
            session.as_ref(),
            &self.config,
            &reloader,
            updated,
            params,
            source,
        )
        .await
        .map(|_| ())
        .map_err(|error| {
            let code = match error {
                ApplyError::SourceChanged | ApplyError::Busy { .. } => ErrorCode::NotReady,
                ApplyError::Validation(_) => ErrorCode::Configuration,
                ApplyError::Write(_) => ErrorCode::Storage,
                ApplyError::Permission(_) => ErrorCode::Permission,
                ApplyError::RolledBack { .. }
                | ApplyError::RollbackFailed { .. }
                | ApplyError::Lifecycle(_) => ErrorCode::InvalidState,
            };
            PortError::Rejected(Failure::new(
                code,
                format!("profile {profile}: {error}"),
                true,
            ))
        })
    }
}

#[async_trait]
impl RuleOverridePort for DesktopRuleOverridePort {
    async fn load_rule_workspace(&self) -> Result<RuleWorkspace, PortError> {
        let profile = self.config.get_current().await.map_err(config_error)?;
        let content = self.config.load(&profile).await.map_err(config_error)?;
        rule_workspace(profile, &content).map_err(PortError::Rejected)
    }

    async fn compare_and_apply_rules(
        &self,
        expected: &RuleWorkspace,
        entries: &[RuleEntry],
    ) -> Result<(), PortError> {
        let _guard = self.apply_guard.lock().await;
        let profile = self.config.get_current().await.map_err(config_error)?;
        let content = self.config.load(&profile).await.map_err(config_error)?;
        if identify_rules_document(profile.clone(), &content) != expected.source
            || load_rules_from_yaml(&content).map_err(|error| {
                PortError::Rejected(Failure::new(
                    ErrorCode::Configuration,
                    error.to_string(),
                    false,
                ))
            })? != expected.rules
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "The profile or rules changed; run the simulation again before applying",
                true,
            )));
        }
        let updated = apply_rules_to_yaml(&content, entries).map_err(|error| {
            PortError::Rejected(Failure::new(
                ErrorCode::Configuration,
                error.to_string(),
                false,
            ))
        })?;

        self.commit_updated(&expected.source, &profile, &updated)
            .await
    }
    async fn compare_and_apply_named_rule(
        &self,
        expected: &RuleWorkspace,
        location: &RuleLocation,
        expected_rule: &str,
        replacement: &str,
    ) -> Result<(), PortError> {
        let _guard = self.apply_guard.lock().await;
        let profile = self.config.get_current().await.map_err(config_error)?;
        let content = self.config.load(&profile).await.map_err(config_error)?;
        if identify_rules_document(profile.clone(), &content) != expected.source {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "The source document changed before applying",
                true,
            )));
        }
        let updated = replace_rule_at_location(&content, location, expected_rule, replacement)
            .map_err(|error| PortError::Rejected(Failure::unsupported(error.to_string())))?;
        self.commit_updated(&expected.source, &profile, &updated)
            .await
    }
}

fn config_error(error: MihomoError) -> PortError {
    match error {
        MihomoError::Io(error) if error.kind() == ErrorKind::PermissionDenied => {
            PortError::PermissionDenied(error.to_string())
        }
        MihomoError::Io(error) => PortError::Io(error.to_string()),
        MihomoError::NotFound(reason) => PortError::NotFound(reason),
        other => PortError::Rejected(Failure::new(
            ErrorCode::Configuration,
            other.to_string(),
            false,
        )),
    }
}

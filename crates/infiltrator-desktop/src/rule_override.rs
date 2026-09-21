//! DUAL-12-08: desktop host adapter for the tracer's one-click reverse-apply.
//!
//! Loads the current profile's rule list and commits a rewritten list through
//! the same CORE-004 apply transaction the editor save path uses (atomic write,
//! reload-or-restart, readiness, rollback). It shares the runtime's apply
//! guard so a tracer override cannot interleave with another config apply.

use std::sync::Arc;

use async_trait::async_trait;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_core::apply::{ApplyParams, EndpointConfigReloader, apply_current_profile};
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::rules::RuleEntry;
use infiltrator_ports::endpoint::EndpointSource;
use infiltrator_ports::error::PortError;
use infiltrator_ports::rule_tracer::RuleOverridePort;
use mihomo_config::manager::ConfigManager;
use mihomo_platform::defaults::DefaultCredentialStore;

/// Host persistence capability behind `RuleTracerPort::apply_override`.
pub struct DesktopRuleOverridePort {
    config: Arc<ConfigManager<DefaultCredentialStore>>,
    session: Arc<CoreApplication>,
    endpoints: Arc<dyn EndpointSource>,
    apply_guard: Arc<tokio::sync::Mutex<()>>,
}

impl DesktopRuleOverridePort {
    pub fn new(
        config: Arc<ConfigManager<DefaultCredentialStore>>,
        session: Arc<CoreApplication>,
        endpoints: Arc<dyn EndpointSource>,
        apply_guard: Arc<tokio::sync::Mutex<()>>,
    ) -> Self {
        Self {
            config,
            session,
            endpoints,
            apply_guard,
        }
    }
}

#[async_trait]
impl RuleOverridePort for DesktopRuleOverridePort {
    async fn load_rule_entries(&self) -> Result<Vec<RuleEntry>, PortError> {
        let profile = self
            .config
            .get_current()
            .await
            .map_err(|error| PortError::Failed(error.to_string()))?;
        let content = self
            .config
            .load(&profile)
            .await
            .map_err(|error| PortError::Failed(error.to_string()))?;
        infiltrator_domain::rules::load_rules_from_yaml(&content)
            .map_err(|error| PortError::Failed(error.to_string()))
    }

    async fn apply_rule_entries(&self, entries: &[RuleEntry]) -> Result<(), PortError> {
        let _guard = self.apply_guard.lock().await;
        let profile = self
            .config
            .get_current()
            .await
            .map_err(|error| PortError::Failed(error.to_string()))?;
        let content = self
            .config
            .load(&profile)
            .await
            .map_err(|error| PortError::Failed(error.to_string()))?;
        let updated = infiltrator_domain::rules::apply_rules_to_yaml(&content, entries)
            .map_err(|error| PortError::Failed(error.to_string()))?;

        let reloader = EndpointConfigReloader::new(self.endpoints.clone());
        let params = ApplyParams {
            strategy: ApplyStrategy::PreferReload,
            ..ApplyParams::default()
        };
        apply_current_profile(
            self.session.as_ref(),
            &self.config,
            &reloader,
            &updated,
            params,
        )
        .await
        .map(|_| ())
        .map_err(|error| PortError::Failed(format!("profile {profile}: {error}")))
    }
}

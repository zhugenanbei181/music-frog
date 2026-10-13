//! Iced-side access to the application configuration facade.

use crate::configs_dir::config_manager;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::profile_application::ProfileApplication;
#[cfg(any(feature = "capture-tools", test))]
use infiltrator_application::rule_trace_fixtures::RuleTraceStore;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_ports::profile_store::ProfileStore;
use mihomo_config::profile_document_codec::MihomoProfileDocumentCodec;
use std::sync::Arc;

pub async fn application() -> Result<ConfigurationApplication, InfiltratorError> {
    let store = config_manager().await?;
    Ok(for_mihomo_store(store))
}

/// Compose the Mihomo-owned profile codec at the Iced host boundary.
pub fn for_mihomo_store(store: Arc<dyn ProfileStore>) -> ConfigurationApplication {
    for_mihomo_profiles(profiles_for_mihomo(store))
}

/// Compose every Iced-owned profile use case with the selected Mihomo codec.
///
/// The profile application stays engine-neutral; the desktop host owns this
/// engine-specific adapter choice at its composition boundary.
pub fn profiles_for_mihomo(store: Arc<dyn ProfileStore>) -> ProfileApplication {
    ProfileApplication::new(store).with_document_codec(Arc::new(MihomoProfileDocumentCodec))
}

/// Use one Iced host-selected Mihomo codec for configuration operations.
pub fn for_mihomo_profiles(profiles: ProfileApplication) -> ConfigurationApplication {
    let profiles = profiles.with_document_codec(Arc::new(MihomoProfileDocumentCodec));
    ConfigurationApplication::from_profile_application(profiles)
}

/// Explicit Mihomo adapter selection for isolated rule trace fixtures.
#[cfg(any(feature = "capture-tools", test))]
pub fn mihomo_rule_trace_fixture() -> Arc<RuleTraceStore> {
    Arc::new(RuleTraceStore::with_document_codec(Arc::new(
        MihomoProfileDocumentCodec,
    )))
}

#[cfg(test)]
#[path = "../tests/gui/configuration_tests.rs"]
mod tests;

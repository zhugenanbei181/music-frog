//! Iced-side access to the application configuration facade.

use crate::configs_dir::config_manager;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::profile_application::ProfileApplication;
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
    for_mihomo_profiles(ProfileApplication::new(store))
}

/// Use one Iced host-selected Mihomo codec for configuration operations.
pub fn for_mihomo_profiles(profiles: ProfileApplication) -> ConfigurationApplication {
    let profiles = profiles.with_document_codec(Arc::new(MihomoProfileDocumentCodec));
    ConfigurationApplication::from_profile_application(profiles)
}

/// Explicit Mihomo adapter selection for isolated rule trace fixtures.
pub fn mihomo_rule_trace_fixture() -> Arc<RuleTraceStore> {
    Arc::new(RuleTraceStore::with_document_codec(Arc::new(
        MihomoProfileDocumentCodec,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_application::dns_hosts_fixtures::{HOSTS_PROFILE, HostsCaptureStore};
    use infiltrator_contract::dns_hosts::DnsHostEntry;

    #[tokio::test]
    async fn host_configuration_factory_binds_the_mihomo_codec() {
        let store = Arc::new(HostsCaptureStore::default());
        let application = for_mihomo_store(store);

        let observed = application.load_hosts_profile().await.unwrap();
        assert_eq!(observed.profile, HOSTS_PROFILE);
        assert_eq!(
            observed.entries,
            vec![
                DnsHostEntry {
                    domain: "live.test".into(),
                    address: "9.9.9.9".into(),
                },
                DnsHostEntry {
                    domain: "second.test".into(),
                    address: "8.8.8.8".into(),
                },
            ]
        );
    }
}

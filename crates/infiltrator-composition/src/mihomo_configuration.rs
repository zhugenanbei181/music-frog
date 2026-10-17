//! Select Mihomo's native profile codec at the product composition boundary.

use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_ports::profile_store::ProfileStore;
use mihomo_config::profile_document_codec::MihomoProfileDocumentCodec;
use std::sync::Arc;

use infiltrator_contract::profile_document_identity::{
    ProfileDocumentIdentity, ProfileDocumentRevision, ProfileDocumentSourceIdentity,
    ProfileDocumentSourceKind, ProfileId, ProfileSidecarRevision, ProfileSourceId,
    ProfileWorkspaceIdentity,
};
#[cfg(feature = "capture-fixtures")]
use infiltrator_domain::profile_options::ProfileOptions;
#[cfg(feature = "capture-fixtures")]
use infiltrator_domain::profile_source::hash_document_bytes;
#[cfg(feature = "capture-fixtures")]
use infiltrator_ports::profile_document_codec::ProfileConfigurationCodecPort;
#[cfg(feature = "capture-fixtures")]
use infiltrator_ports::rule_tracer::RuleWorkspace;
#[cfg(feature = "capture-fixtures")]
use infiltrator_test_support::rule_trace_fixtures::RuleTraceStore;
#[cfg(feature = "capture-fixtures")]
use lurker_core_contract::identity::EngineId;
#[cfg(feature = "capture-fixtures")]
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
#[cfg(feature = "capture-fixtures")]
use lurker_core_ports::error::PortError;
#[cfg(feature = "capture-fixtures")]
use mihomo_config::profile_option_store::{delete_options, save_options};
#[cfg(feature = "capture-fixtures")]
use std::path::Path;

/// Construct profile use cases with the Mihomo YAML document codec.
pub fn profiles_for_mihomo(store: Arc<dyn ProfileStore>) -> ProfileApplication {
    ProfileApplication::new(store).with_document_codec(Arc::new(MihomoProfileDocumentCodec))
}

/// Construct configuration use cases with the Mihomo YAML document codec.
pub fn for_mihomo_store(store: Arc<dyn ProfileStore>) -> ConfigurationApplication {
    for_mihomo_profiles(profiles_for_mihomo(store))
}

/// Bind an existing profile use case to the Mihomo YAML document codec.
pub fn for_mihomo_profiles(profiles: ProfileApplication) -> ConfigurationApplication {
    let profiles = profiles.with_document_codec(Arc::new(MihomoProfileDocumentCodec));
    ConfigurationApplication::from_profile_application(profiles)
}

/// Provide the Mihomo codec through its neutral port for composition-level
/// adapter conformance and isolated capture fixtures.
#[cfg(feature = "capture-fixtures")]
pub fn profile_document_codec_fixture() -> Arc<dyn ProfileConfigurationCodecPort> {
    Arc::new(MihomoProfileDocumentCodec)
}

/// Parse rule workspace fixture data using the selected Mihomo codec.
#[cfg(feature = "capture-fixtures")]
pub fn mihomo_rule_workspace_fixture(
    profile: &str,
    content: &str,
) -> Result<RuleWorkspace, PortError> {
    let profile_id = ProfileId::new(profile).map_err(|error| PortError::Io(error.to_string()))?;
    let source_id =
        ProfileSourceId::new(profile).map_err(|error| PortError::Io(error.to_string()))?;
    let revision = ProfileDocumentRevision::new(hash_document_bytes(content))
        .map_err(|error| PortError::Io(error.to_string()))?;
    let identity = ProfileWorkspaceIdentity::new(
        ProfileDocumentIdentity::new(
            profile_id,
            EngineId::new("mihomo").map_err(|error| PortError::Io(error.to_string()))?,
            ProfileDocumentSourceIdentity::new(ProfileDocumentSourceKind::Local, source_id),
            revision,
            ProfileFormatId::new("mihomo-yaml")
                .map_err(|error| PortError::Io(error.to_string()))?,
            ProfileSchemaVersion::new(1).map_err(|error| PortError::Io(error.to_string()))?,
        ),
        ProfileSidecarRevision::Absent,
    );
    let projection =
        profile_document_codec_fixture().read_rule_workspace(&identity, content.as_bytes())?;
    Ok(RuleWorkspace {
        source: projection.source,
        profile_identity: identity,
        rules: projection.rules,
        targets: projection.targets,
        sub_rules: projection.sub_rules,
    })
}

/// Write Mihomo profile options for isolated integration fixtures.
#[cfg(feature = "capture-fixtures")]
pub async fn save_profile_options_fixture(
    config_dir: &Path,
    profile: &str,
    options: &ProfileOptions,
) -> Result<(), PortError> {
    save_options(config_dir, profile, options).await
}

/// Remove Mihomo profile options for isolated integration fixtures.
#[cfg(feature = "capture-fixtures")]
pub async fn delete_profile_options_fixture(
    config_dir: &Path,
    profile: &str,
) -> Result<(), PortError> {
    delete_options(config_dir, profile).await
}

/// Build the Mihomo codec for isolated rule-trace capture data.
#[cfg(feature = "capture-fixtures")]
pub fn mihomo_rule_trace_fixture() -> Arc<RuleTraceStore> {
    Arc::new(RuleTraceStore::with_document_codec(Arc::new(
        MihomoProfileDocumentCodec,
    )))
}

#[cfg(all(test, feature = "capture-fixtures"))]
mod tests {
    use super::for_mihomo_store;
    use infiltrator_contract::dns_hosts::DnsHostEntry;
    use infiltrator_test_support::dns_hosts_fixtures::{HOSTS_PROFILE, HostsCaptureStore};
    use std::sync::Arc;

    #[tokio::test]
    async fn profile_composition_binds_the_mihomo_codec() {
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

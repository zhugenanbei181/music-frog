use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::rule_trace_fixtures::RuleTraceStore;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::profile_document_identity::{
    ProfileDocumentIdentity, ProfileDocumentRevision, ProfileDocumentSourceIdentity,
    ProfileDocumentSourceKind, ProfileFormatId, ProfileId, ProfileSchemaVersion,
    ProfileSidecarRevision, ProfileSourceId, ProfileWorkspaceIdentity,
};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::rule_tracer::RuleWorkspace;
use std::sync::Arc;

pub(crate) fn mihomo_configuration<S: ProfileStore + 'static>(
    store: Arc<S>,
) -> ConfigurationApplication {
    let store: Arc<dyn ProfileStore> = store;
    let profiles = ProfileApplication::new(store).with_document_codec(Arc::new(
        mihomo_config::profile_document_codec::MihomoProfileDocumentCodec,
    ));
    ConfigurationApplication::from_profile_application(profiles)
}

pub(crate) fn mihomo_rule_trace_fixture() -> Arc<RuleTraceStore> {
    Arc::new(RuleTraceStore::with_document_codec(Arc::new(
        mihomo_config::profile_document_codec::MihomoProfileDocumentCodec,
    )))
}

pub(crate) fn mihomo_rule_workspace(profile: &str, content: &str) -> RuleWorkspace {
    let identity = ProfileWorkspaceIdentity::new(
        ProfileDocumentIdentity::new(
            ProfileId::new(profile).expect("fixture profile ID"),
            EngineId::new("mihomo").expect("fixture engine ID"),
            ProfileDocumentSourceIdentity::new(
                ProfileDocumentSourceKind::Local,
                ProfileSourceId::new(profile).expect("fixture source ID"),
            ),
            ProfileDocumentRevision::new(infiltrator_domain::profile_source::hash_document_bytes(
                content,
            ))
            .expect("fixture document revision"),
            ProfileFormatId::new("mihomo-yaml").expect("fixture format ID"),
            ProfileSchemaVersion::new(1).expect("fixture schema version"),
        ),
        ProfileSidecarRevision::Absent,
    );
    let projection = mihomo_config::profile_document_codec::MihomoProfileDocumentCodec
        .read_rule_workspace(&identity, content.as_bytes())
        .expect("Mihomo codec reads fixture rules");
    RuleWorkspace {
        source: projection.source,
        profile_identity: identity,
        rules: projection.rules,
        targets: projection.targets,
        sub_rules: projection.sub_rules,
    }
}

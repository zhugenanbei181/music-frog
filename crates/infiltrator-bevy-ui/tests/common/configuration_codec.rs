use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::rule_tracer::RuleWorkspace;
use infiltrator_test_support::rule_trace_fixtures::RuleTraceStore;
use std::sync::Arc;

pub(crate) fn mihomo_configuration<S: ProfileStore + 'static>(
    store: Arc<S>,
) -> ConfigurationApplication {
    let store: Arc<dyn ProfileStore> = store;
    infiltrator_composition::mihomo_configuration::for_mihomo_store(store)
}

pub(crate) fn mihomo_rule_trace_fixture() -> Arc<RuleTraceStore> {
    infiltrator_composition::mihomo_configuration::mihomo_rule_trace_fixture()
}

pub(crate) fn mihomo_rule_workspace(profile: &str, content: &str) -> RuleWorkspace {
    infiltrator_composition::mihomo_configuration::mihomo_rule_workspace_fixture(profile, content)
        .expect("Mihomo codec reads fixture rules")
}

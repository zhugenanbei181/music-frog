use infiltrator_composition::mihomo_configuration::{
    self, for_mihomo_store, mihomo_rule_workspace_fixture,
};
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::rule_tracer::RuleWorkspace;
use infiltrator_test_support::rule_trace_fixtures::RuleTraceStore;
use std::sync::Arc;

pub(crate) fn mihomo_configuration<S: ProfileStore + 'static>(
    store: Arc<S>,
) -> ConfigurationApplication {
    let store: Arc<dyn ProfileStore> = store;
    for_mihomo_store(store)
}

pub(crate) fn mihomo_rule_trace_fixture() -> Arc<RuleTraceStore> {
    mihomo_configuration::mihomo_rule_trace_fixture()
}

pub(crate) fn mihomo_rule_workspace(profile: &str, content: &str) -> RuleWorkspace {
    mihomo_rule_workspace_fixture(profile, content)
        .expect("Mihomo codec reads fixture rules")
}

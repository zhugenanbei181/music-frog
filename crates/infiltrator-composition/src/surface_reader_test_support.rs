use infiltrator_contract::core_identity::CoreVersion;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::core_capability_application::CoreCapabilityApplication;
use infiltrator_contract::command::{CommandIntent, CommandResult};
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, AdapterRevision, CoreCapabilityEntry, CoreCapabilityMatrix,
    CoreCapabilityScope, CoreLifecycleOperation, CoreOperation, EngineCapabilityDecision,
    HostCapabilityDecision,
};
use infiltrator_contract::core_identity::{CoreCommandRequest, EngineId};
use infiltrator_contract::surface::HostKind;
use infiltrator_ports::core_capability_provider::{
    CoreCapabilityProvider, CoreCapabilityProviderError,
};
use std::sync::Arc;

struct TestLifecycleCapabilityProvider;

impl CoreCapabilityProvider for TestLifecycleCapabilityProvider {
    fn matrix_for(
        &self,
        engine_id: &EngineId,
        core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        let scope = CoreCapabilityScope::new(
            engine_id.clone(),
            core_version.cloned(),
            HostKind::Desktop,
            AdapterRevision::new(1),
        );
        let entries = [
            CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
            CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
            CoreOperation::Lifecycle(CoreLifecycleOperation::Restart),
        ]
        .into_iter()
        .map(|operation| {
            CoreCapabilityEntry::new(
                operation,
                EngineCapabilityDecision::Supported,
                HostCapabilityDecision::Supported,
                AdapterCapabilityDecision::Supported,
            )
        })
        .collect();
        CoreCapabilityMatrix::new(scope, entries).map_err(|error| {
            CoreCapabilityProviderError::MatrixUnavailable {
                reason: error.to_string(),
            }
        })
    }
}

pub(super) fn install_lifecycle_capabilities(application: &CoreApplication) {
    application
        .install_core_capability_application(CoreCapabilityApplication::new(Arc::new(
            TestLifecycleCapabilityProvider,
        )))
        .expect("test lifecycle capabilities install before use");
}

pub(super) async fn test_execute_result_at_current_scope(
    application: &CoreApplication,
    intent: CommandIntent,
) -> CommandResult {
    let scope = application
        .snapshot()
        .scope()
        .expect("Core application test fixture must expose its owner identity");
    application
        .execute_scoped(CoreCommandRequest { scope, intent })
        .await
        .result
}

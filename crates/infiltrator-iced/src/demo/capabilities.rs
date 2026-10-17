//! Typed host capability facts for isolated demo actions.

use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::core_capability_application::CoreCapabilityApplication;
use infiltrator_contract::capability::{
    Availability, Capability, CapabilitySnapshot, CapabilityStatus,
};
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, AdapterRevision, CoreCapabilityEntry, CoreCapabilityMatrix,
    CoreCapabilityScope, CoreLifecycleOperation, CoreOperation, EngineCapabilityDecision,
    HostCapabilityDecision,
};
use infiltrator_contract::surface::HostKind;
use infiltrator_ports::capability_provider::CapabilityProvider;
use infiltrator_ports::core_capability_provider::{
    CoreCapabilityProvider, CoreCapabilityProviderError,
};
use lurker_core_contract::identity::{CoreVersion, EngineId};
use std::sync::Arc;

pub(super) struct DemoHostCapabilityProvider {
    capability: Capability,
}

impl DemoHostCapabilityProvider {
    pub(super) const fn new(capability: Capability) -> Self {
        Self { capability }
    }
}

impl CapabilityProvider for DemoHostCapabilityProvider {
    fn host_kind(&self) -> HostKind {
        HostKind::Desktop
    }

    fn capabilities(&self) -> CapabilitySnapshot {
        CapabilitySnapshot::new(
            HostKind::Desktop,
            1,
            vec![CapabilityStatus {
                capability: self.capability,
                availability: Availability::Supported,
            }],
        )
    }
}

pub(super) fn install_demo_log_lifecycle_capability(core: &CoreApplication) {
    core.install_core_capability_application(CoreCapabilityApplication::new(Arc::new(
        DemoLogCapabilityProvider,
    )))
    .expect("demo log lifecycle capability installs before its fixture starts");
}

struct DemoLogCapabilityProvider;

impl CoreCapabilityProvider for DemoLogCapabilityProvider {
    fn matrix_for(
        &self,
        engine_id: &EngineId,
        core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        let expected = EngineId::new("log-capture").expect("valid demo fixture engine");
        if engine_id != &expected {
            return Err(CoreCapabilityProviderError::EngineUnavailable {
                engine_id: engine_id.clone(),
                reason: "demo log capture only supports its isolated fixture engine".to_owned(),
            });
        }
        let scope = CoreCapabilityScope::new(
            expected,
            core_version.cloned(),
            HostKind::Desktop,
            AdapterRevision::new(1),
        );
        CoreCapabilityMatrix::new(
            scope,
            vec![CoreCapabilityEntry::new(
                CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
                EngineCapabilityDecision::Supported,
                HostCapabilityDecision::Supported,
                AdapterCapabilityDecision::Supported,
            )],
        )
        .map_err(|error| CoreCapabilityProviderError::MatrixUnavailable {
            reason: error.to_string(),
        })
    }
}

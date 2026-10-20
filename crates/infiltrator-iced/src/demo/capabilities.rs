//! Typed host and core capability facts for isolated demo actions.
//!
//! A capture owner runs *real* commands against its own fixture engine, so it
//! has to publish the very capability matrix that admits those commands. A
//! demo that omits the matrix is refused by the shared capability gate before
//! the command ever reaches the owner: the surface would then show a locally
//! invented failure instead of the owner's real answer.

use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::core_capability_application::CoreCapabilityApplication;
use infiltrator_contract::capability::{Availability, CapabilitySnapshot, CapabilityStatus};
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
use lurker_core_contract::capability::Capability;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::identity::{CoreIdentity, CoreInstanceId, CoreVersion, EngineId};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use lurker_core_ports::error::PortError;
use std::sync::Arc;

/// The engine of the isolated log/telemetry capture owner.
pub(super) const LOG_CAPTURE_ENGINE: &str = "log-capture";
/// The engine of the isolated capture owner that refuses lifecycle effects.
pub(super) const CAPTURE_ENGINE: &str = "capture";
/// The engine of the isolated routing-intent capture owner.
pub(super) const ROUTING_INTENT_ENGINE: &str = "mihomo";
/// The engine of an isolated capture that reads a Mihomo profile document.
///
/// The shared owners refuse a write whose owner belongs to another engine: a
/// Mihomo profile store is a Mihomo owner, so the capture's core has to be a
/// Mihomo core too.
pub(super) const MIHOMO_CAPTURE_ENGINE: &str = "mihomo";

/// The isolated capture core of a Mihomo capture.
///
/// The capture owns an isolated Core instance that never starts a real
/// engine: it publishes the Mihomo identity the profile fixtures use and it
/// refuses every lifecycle effect.
pub(super) struct DemoMihomoCaptureCore;

#[async_trait::async_trait]
impl CoreProcess for DemoMihomoCaptureCore {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            EngineId::new(MIHOMO_CAPTURE_ENGINE).expect("valid capture engine"),
            CoreInstanceId::new("mihomo-capture").expect("valid capture instance"),
            Some(CoreVersion::new("1.19.12").expect("valid capture version")),
        ))
    }

    async fn start(&self, _context: CoreProcessStartContext) -> Result<(), PortError> {
        Err(PortError::Failed(
            "the capture core refuses lifecycle effects".into(),
        ))
    }

    async fn stop(&self) -> Result<(), PortError> {
        Err(PortError::Failed(
            "the capture core refuses lifecycle effects".into(),
        ))
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(CoreLifecycle::Running)
    }

    fn controller_endpoint(&self) -> Option<String> {
        None
    }
}

#[async_trait::async_trait]
impl CoreReadiness for DemoMihomoCaptureCore {
    async fn probe(&self) -> Result<String, PortError> {
        Ok("mihomo-capture".into())
    }

    async fn active_core_version(&self) -> Result<Option<CoreVersion>, PortError> {
        Ok(Some(
            CoreVersion::new("1.19.12").expect("valid capture version"),
        ))
    }
}

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

/// The core capability matrix one isolated capture owner publishes.
///
/// The matrix is engine-scoped: a command that names another engine is never
/// admitted, so a capture can only run the operations it declared for its own
/// fixture owner.
pub(super) struct DemoCoreCapabilityProvider {
    engine: EngineId,
    operations: Vec<CoreOperation>,
}

impl CoreCapabilityProvider for DemoCoreCapabilityProvider {
    fn matrix_for(
        &self,
        engine_id: &EngineId,
        core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        if engine_id != &self.engine {
            return Err(CoreCapabilityProviderError::EngineUnavailable {
                engine_id: engine_id.clone(),
                reason: "this demo capture only supports its own fixture engine".to_owned(),
            });
        }
        let scope = CoreCapabilityScope::new(
            self.engine.clone(),
            core_version.cloned(),
            HostKind::Desktop,
            AdapterRevision::new(1),
        );
        let entries = self
            .operations
            .iter()
            .copied()
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

/// Publish the exact core capability matrix a capture owner needs.
///
/// Without this matrix the shared capability gate refuses every core-backed
/// command, and the surface would publish a locally invented refusal instead
/// of the owner's real terminal answer. The installed matrix is returned so
/// the command facade can admit the very same operations.
pub(super) fn install_demo_core_capabilities(
    core: &CoreApplication,
    engine: &str,
    operations: impl IntoIterator<Item = CoreOperation>,
) -> CoreCapabilityApplication {
    let application = CoreCapabilityApplication::new(Arc::new(DemoCoreCapabilityProvider {
        engine: EngineId::new(engine).expect("valid demo fixture engine"),
        operations: operations.into_iter().collect(),
    }));
    core.install_core_capability_application(application.clone())
        .expect("demo core capabilities install before their fixture starts");
    application
}

/// The log capture owner runs the shared log lifecycle.
pub(super) fn install_demo_log_lifecycle_capability(core: &CoreApplication) {
    install_demo_core_capabilities(
        core,
        LOG_CAPTURE_ENGINE,
        [CoreOperation::Lifecycle(CoreLifecycleOperation::Start)],
    );
}

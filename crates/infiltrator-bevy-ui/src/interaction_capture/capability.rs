//! The capture's own capability evidence for the shared command gates.
//!
//! The product command bus admits every command through two typed gates: the
//! engine-scoped capability matrix (a Core command) and the host capability
//! provider (a host-only command). An isolated capture runs the very same
//! gates, so the capture declares the capabilities its own fixture really
//! provides instead of bypassing them. A capture that skipped the gate would
//! prove nothing about the product path.
use crate::command_execution::ApplicationCommandSink;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::core_capability_application::CoreCapabilityApplication;
use infiltrator_contract::capability::{Availability, CapabilitySnapshot, CapabilityStatus};
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, AdapterRevision, CoreCapabilityEntry, CoreCapabilityMatrix,
    CoreCapabilityScope, CoreOperation, EngineCapabilityDecision, HostCapabilityDecision,
};
use infiltrator_contract::surface::HostKind;
use infiltrator_ports::capability_provider::CapabilityProvider;
use infiltrator_ports::core_capability_provider::{
    CoreCapabilityProvider, CoreCapabilityProviderError,
};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::identity::{CoreVersion, EngineId};
use lurker_core_ports::core_scope::CoreScopeObservation;
use std::sync::{Arc, RwLock};

/// The engine-scoped capability matrix an isolated capture core publishes.
struct CaptureCoreCapabilityProvider {
    operations: Vec<CoreOperation>,
}

impl CoreCapabilityProvider for CaptureCoreCapabilityProvider {
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

/// The host capability provider an isolated capture host publishes.
struct CaptureHostCapabilityProvider {
    capabilities: Vec<Capability>,
}

impl CapabilityProvider for CaptureHostCapabilityProvider {
    fn host_kind(&self) -> HostKind {
        HostKind::Desktop
    }
    fn capabilities(&self) -> CapabilitySnapshot {
        let entries = self
            .capabilities
            .iter()
            .copied()
            .map(|capability| CapabilityStatus {
                capability,
                availability: Availability::Supported,
            })
            .collect();
        CapabilitySnapshot::new(HostKind::Desktop, 1, entries)
    }
}

/// The engine-scoped operations an isolated capture core really implements.
pub fn capture_core_operations() -> Vec<CoreOperation> {
    use infiltrator_contract::core_capabilities::{
        CoreLifecycleOperation, DnsOperation, FakeIpCacheOperation, RulesOperation,
    };
    vec![
        CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
        CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
        CoreOperation::Lifecycle(CoreLifecycleOperation::Restart),
        CoreOperation::Dns(DnsOperation::Query),
        CoreOperation::Dns(DnsOperation::ApplyConfiguration),
        CoreOperation::FakeIpCache(FakeIpCacheOperation::Flush),
        CoreOperation::Rules(RulesOperation::Trace),
        CoreOperation::Rules(RulesOperation::Apply),
    ]
}

/// The capture's engine-scoped capability matrix as the shared command
/// application's own gate.
pub fn capture_core_capability_application() -> CoreCapabilityApplication {
    CoreCapabilityApplication::new(Arc::new(CaptureCoreCapabilityProvider {
        operations: capture_core_operations(),
    }))
}

/// Install the capture's engine-scoped capability matrix on an isolated core.
pub fn install_core_capabilities(core: &CoreApplication) {
    core.install_core_capability_application(capture_core_capability_application())
        .expect("capture core capabilities install before use");
}

/// The host capabilities an isolated capture host really provides.
pub fn capture_host_capabilities(
    capabilities: impl IntoIterator<Item = Capability>,
) -> Arc<dyn CapabilityProvider> {
    Arc::new(CaptureHostCapabilityProvider {
        capabilities: capabilities.into_iter().collect(),
    })
}

/// The host capabilities every isolated capture host really provides: the
/// shared command bus needs the DNS, Core lifecycle, log and profile hosts the
/// capture fixtures really compose.
pub fn default_capture_host_capabilities() -> Arc<dyn CapabilityProvider> {
    capture_host_capabilities([
        Capability::CoreLifecycle,
        Capability::Dns,
        Capability::Logs,
        Capability::Profiles,
    ])
}

/// The shared command application of an isolated capture: it publishes the
/// capture's own engine-scoped matrix and the capture host's capabilities, so
/// every command travels the product's real admission path.
pub fn capture_command_application() -> CommandApplication {
    CommandApplication::new()
        .with_core_capabilities(capture_core_capability_application())
        .with_host_capabilities(default_capture_host_capabilities())
}

/// The scope an isolated capture owner publishes to the surface.
pub fn capture_scope_observation(core: &CoreApplication) -> Arc<RwLock<CoreScopeObservation>> {
    let observed = match core.snapshot().scope() {
        Some(scope) => CoreScopeObservation::Observed(scope),
        None => CoreScopeObservation::Unavailable,
    };
    Arc::new(RwLock::new(observed))
}

/// The product command sink of an isolated capture: the surface's shared
/// typed commands travel the Core owner's scoped route, exactly like the
/// product host's commands do.
pub fn capture_scoped_sink(
    core: Arc<CoreApplication>,
    handler: CommandApplication,
) -> ApplicationCommandSink {
    install_core_capabilities(&core);
    core.install_command_handler(Arc::new(handler));
    let observed = capture_scope_observation(&core);
    ApplicationCommandSink::new_for_surface(core, observed)
}

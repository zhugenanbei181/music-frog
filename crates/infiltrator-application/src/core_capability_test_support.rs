//! Explicit capability fixtures for application command behavior tests.

use crate::command_application::CommandApplication;
use crate::command_application::handler::{CommandHandler, ValidatedCoreCommandScope};
use crate::core_capability_application::CoreCapabilityApplication;
use infiltrator_contract::capability::{Availability, CapabilitySnapshot, CapabilityStatus};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
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
use lurker_core_contract::error::Failure;
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use std::sync::Arc;

struct TestHostCapabilities;

impl CapabilityProvider for TestHostCapabilities {
    fn host_kind(&self) -> HostKind {
        HostKind::Desktop
    }

    fn capabilities(&self) -> CapabilitySnapshot {
        let entries = [
            Capability::CoreLifecycle,
            Capability::Profiles,
            Capability::RoutingIntent,
            Capability::Connections,
            Capability::Logs,
            Capability::Dns,
            Capability::Tun,
            Capability::SystemProxy,
            Capability::LanAccessControl,
            Capability::Ipv6Routing,
            Capability::UwpLoopback,
            Capability::PacService,
            Capability::NetworkRoaming,
            Capability::VpnService,
            Capability::Autostart,
            Capability::CoreVersionInstall,
            Capability::WebDavSync,
            Capability::AppRouting,
            Capability::Speedtest,
        ]
        .into_iter()
        .map(|capability| CapabilityStatus {
            capability,
            availability: Availability::Supported,
        })
        .collect();
        CapabilitySnapshot::new(HostKind::Desktop, 1, entries)
    }
}

struct TestCapabilityProvider {
    matrix: CoreCapabilityMatrix,
}

impl CoreCapabilityProvider for TestCapabilityProvider {
    fn matrix_for(
        &self,
        _engine_id: &EngineId,
        _core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        Ok(self.matrix.clone())
    }
}

pub fn core_scope() -> CoreScope {
    let engine = EngineId::new("mihomo").expect("valid test engine");
    let instance = CoreInstanceId::new("test-core").expect("valid test instance");
    let version = CoreVersion::new("1.19.12").expect("valid test version");
    CoreScope {
        identity: CoreIdentity::new(engine, instance, Some(version)),
        generation: CoreGeneration::new(1),
        session_token: None,
    }
}

pub fn capability_application() -> CoreCapabilityApplication {
    let scope = CoreCapabilityScope::new(
        EngineId::new("mihomo").expect("valid test engine"),
        Some(CoreVersion::new("1.19.12").expect("valid test version")),
        HostKind::Desktop,
        AdapterRevision::new(1),
    );
    let entries = CoreOperation::all()
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
    let matrix = CoreCapabilityMatrix::new(scope, entries).expect("valid test matrix");
    CoreCapabilityApplication::new(Arc::new(TestCapabilityProvider { matrix }))
}

pub async fn execute_core_command(
    application: &CommandApplication,
    intent: CommandIntent,
) -> Result<(), Failure> {
    application
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            intent,
        )
        .await
}

pub async fn execute_core_output_command(
    application: &CommandApplication,
    intent: CommandIntent,
) -> Result<CommandOutput, Failure> {
    application
        .handle_output_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            intent,
        )
        .await
}

pub fn with_test_capabilities(application: CommandApplication) -> CommandApplication {
    application
        .with_core_capabilities(capability_application())
        .with_host_capabilities(Arc::new(TestHostCapabilities))
}

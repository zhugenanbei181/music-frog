//! Engine-neutral Desktop Core owner runtime used by the sing-box startup path and engine-switch handoff.

use crate::surface::core_surface_pump;
use async_trait::async_trait;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::core_capability_application::CoreCapabilityApplication;
use infiltrator_application::surface_application::SurfacePump;
use infiltrator_composition::spawn_core_watchdog;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceWriteAuthorization;
use infiltrator_contract::adapter_operation_declaration::AdapterOperationOwner;
use infiltrator_contract::capability::{
    Availability, Capability, CapabilitySnapshot, CapabilityStatus,
};
use infiltrator_contract::core_capabilities::{
    CapabilityAccess, CoreCapabilityKey, CoreCapabilityStatus, CoreLifecycleOperation,
    CoreOperation, EngineCapabilityDecision,
};
use infiltrator_contract::core_identity::{CoreScope, EngineId};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_domain::runtime::{
    ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider, TrafficData,
};
use infiltrator_ports::core_binary_selection::CoreBinarySelection;
use infiltrator_ports::core_capability_provider::CoreCapabilityProvider;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::error::PortError;
use infiltrator_ports::host_runtime::{HostRuntime, TunServiceStatus};
use infiltrator_ports::profile_configuration_transaction::ProfileConfigurationTransactionPort;
use infiltrator_ports::runtime_configuration::RuntimeConfigurationPorts;
use infiltrator_ports::runtime_gateway::{ManagedRuntime, RuntimeGateway, RuntimeStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub(crate) struct DesktopCoreOwnerRuntime {
    application: Arc<CoreApplication>,
    binary_path: PathBuf,
    controller_url: String,
    _configuration_transaction: Arc<dyn ProfileConfigurationTransactionPort>,
    _watchdog: infiltrator_composition::CoreWatchdogHandle,
}

impl DesktopCoreOwnerRuntime {
    pub(crate) fn validate_engine_start_capability(
        provider: &Arc<dyn CoreCapabilityProvider>,
        selection: &CoreBinarySelection,
        engine_id: &EngineId,
    ) -> Result<(), PortError> {
        let matrix = CoreCapabilityApplication::new(Arc::clone(provider))
            .matrix_for(engine_id, Some(&selection.version))
            .map_err(PortError::Rejected)?;
        let resolution = matrix.resolve(CoreCapabilityKey::new(
            matrix.scope.clone(),
            CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
        ));
        let lifecycle_owner_is_application = matches!(
            &resolution.engine,
            Some(EngineCapabilityDecision::OwnerManaged {
                owner: AdapterOperationOwner::CoreLifecycle,
            })
        );
        match (resolution.status, lifecycle_owner_is_application) {
            (
                CoreCapabilityStatus::Available {
                    access: CapabilityAccess::Supported,
                    ..
                },
                true,
            ) => Ok(()),
            (status, _) => Err(PortError::unsupported(
                Capability::CoreLifecycle,
                format!(
                    "Core start is not evidenced for selected version {} on this target: {status:?}",
                    selection.version.raw()
                ),
            )),
        }
    }

    pub(crate) fn from_bound_application(
        application: Arc<CoreApplication>,
        binary_path: PathBuf,
        controller_url: String,
        configuration_transaction: Arc<dyn ProfileConfigurationTransactionPort>,
    ) -> Self {
        let watchdog = spawn_core_watchdog(Arc::clone(&application));
        Self {
            application,
            binary_path,
            controller_url,
            _configuration_transaction: configuration_transaction,
            _watchdog: watchdog,
        }
    }

    pub(crate) fn application(&self) -> Arc<CoreApplication> {
        Arc::clone(&self.application)
    }

    pub(crate) async fn surface_pump(
        &self,
        surface: SurfaceKind,
        sample_interval: Duration,
    ) -> anyhow::Result<SurfacePump> {
        core_surface_pump(
            Arc::clone(&self.application),
            surface,
            desktop_core_lifecycle_capabilities(),
            sample_interval,
        )
        .await
    }
}

pub(crate) fn desktop_core_lifecycle_capabilities() -> CapabilitySnapshot {
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
        availability: match capability {
            Capability::CoreLifecycle => Availability::Supported,
            _ => Availability::Unsupported {
                reason: "desktop Core-only handoff has not connected this operation".to_owned(),
            },
        },
    })
    .collect();
    CapabilitySnapshot::new(HostKind::Desktop, 0, entries)
}

fn unsupported<T>(capability: Capability, operation: &'static str) -> Result<T, PortError> {
    Err(PortError::unsupported(capability, operation))
}

#[async_trait]
impl RuntimeGateway for DesktopCoreOwnerRuntime {
    fn current_core_scope(&self) -> Result<CoreScope, PortError> {
        unsupported(
            Capability::CoreLifecycle,
            "desktop Core-only handoff has not connected runtime gateway operations",
        )
    }

    fn configuration_ports(&self) -> RuntimeConfigurationPorts {
        RuntimeConfigurationPorts::unsupported()
    }

    async fn switch_proxy(&self, _group: &str, _proxy: &str) -> Result<(), PortError> {
        unsupported(
            Capability::RoutingIntent,
            "desktop Core-only handoff does not expose proxy selection",
        )
    }

    async fn test_delay(
        &self,
        _proxy: &str,
        _url: &str,
        _timeout_ms: u32,
    ) -> Result<u32, PortError> {
        unsupported(
            Capability::RoutingIntent,
            "desktop Core-only handoff does not expose delay probing",
        )
    }

    async fn get_proxy_providers(&self) -> Result<Vec<ProxyProvider>, PortError> {
        unsupported(
            Capability::RoutingIntent,
            "desktop Core-only handoff does not expose proxy-provider listing",
        )
    }

    async fn get_rule_providers(&self) -> Result<Vec<RuleProvider>, PortError> {
        unsupported(
            Capability::RoutingIntent,
            "desktop Core-only handoff does not expose rule-provider listing",
        )
    }

    async fn update_proxy_provider(&self, _name: &str) -> Result<(), PortError> {
        unsupported(
            Capability::RoutingIntent,
            "desktop Core-only handoff does not expose proxy-provider updates",
        )
    }

    async fn update_rule_provider(&self, _name: &str) -> Result<(), PortError> {
        unsupported(
            Capability::RoutingIntent,
            "desktop Core-only handoff does not expose rule-provider updates",
        )
    }

    async fn get_connections(&self) -> Result<ConnectionSnapshot, PortError> {
        unsupported(
            Capability::Connections,
            "desktop Core-only handoff does not expose connection listing",
        )
    }

    async fn get_memory(&self) -> Result<MemoryData, PortError> {
        unsupported(
            Capability::Connections,
            "desktop Core-only handoff does not expose memory observation",
        )
    }

    async fn close_connection(&self, _id: &str) -> Result<(), PortError> {
        unsupported(
            Capability::Connections,
            "desktop Core-only handoff does not expose connection close",
        )
    }

    async fn close_all_connections(&self) -> Result<(), PortError> {
        unsupported(
            Capability::Connections,
            "desktop Core-only handoff does not expose connection close",
        )
    }

    async fn stream_logs(
        &self,
        _level: Option<String>,
    ) -> Result<RuntimeStream<String>, PortError> {
        unsupported(
            Capability::Logs,
            "desktop Core-only handoff does not expose log streaming",
        )
    }

    async fn stream_traffic(&self) -> Result<RuntimeStream<TrafficData>, PortError> {
        unsupported(
            Capability::Connections,
            "desktop Core-only handoff does not expose traffic streaming",
        )
    }

    async fn stream_connections(&self) -> Result<RuntimeStream<ConnectionSnapshot>, PortError> {
        unsupported(
            Capability::Connections,
            "desktop Core-only handoff does not expose connection streaming",
        )
    }
}

#[async_trait]
impl ManagedRuntime for DesktopCoreOwnerRuntime {
    fn engine_id(&self) -> Option<EngineId> {
        CoreLifecyclePort::core_identity(self.application.as_ref()).map(|identity| identity.engine)
    }

    fn generation(&self) -> u64 {
        self.application.generation()
    }

    fn core_lifecycle(&self) -> CoreLifecycle {
        CoreLifecyclePort::lifecycle(self.application.as_ref())
    }

    fn active_profile_reference_write_authorization(
        &self,
    ) -> Result<ActiveProfileReferenceWriteAuthorization, PortError> {
        self.application
            .active_profile_reference_write_authorization()
            .map_err(PortError::Rejected)
    }

    async fn is_running(&self) -> bool {
        matches!(
            self.core_lifecycle(),
            CoreLifecycle::Running | CoreLifecycle::Ready
        )
    }

    async fn restart(&self) -> Result<u64, PortError> {
        unsupported(
            Capability::CoreLifecycle,
            "desktop Core-only handoff does not expose restart",
        )
    }

    async fn http_proxy_endpoint(&self) -> Result<Option<String>, PortError> {
        unsupported(
            Capability::CoreLifecycle,
            "desktop Core-only handoff does not expose proxy endpoints",
        )
    }

    async fn shutdown(&self) -> Result<(), PortError> {
        CoreLifecyclePort::stop(self.application.as_ref())
            .await
            .map(|_| ())
            .map_err(|error| PortError::Failed(error.to_string()))
    }
}

impl HostRuntime for DesktopCoreOwnerRuntime {
    fn controller_url(&self) -> String {
        self.controller_url.clone()
    }

    fn core_binary_path(&self) -> PathBuf {
        self.binary_path.clone()
    }

    fn tun_service_status(&self) -> TunServiceStatus {
        TunServiceStatus::Unsupported
    }

    fn lifecycle_port(&self) -> Arc<dyn CoreLifecyclePort> {
        Arc::new(self.application.as_ref().clone())
    }
}

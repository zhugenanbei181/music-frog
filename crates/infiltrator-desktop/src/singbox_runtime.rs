//! Desktop-owned sing-box lifecycle composition.

use crate::surface::core_surface_pump;
use async_trait::async_trait;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::core_capability_application::CoreCapabilityApplication;
use infiltrator_application::surface_application::SurfacePump;
use infiltrator_composition::active_profile_backend::{
    ActiveProfileBackendSelector, ActiveProfileSelectionPreflight, PreviousCoreOwner,
};
use infiltrator_composition::engine_backend_factory::{
    EngineBackendFactoryRegistry, SingBoxEngineBackendProvider,
};
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
use infiltrator_contract::core_identity::{CoreInstanceId, CoreScope, EngineId};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::version::CoreArtifactVerification;
use infiltrator_domain::runtime::{
    ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider, TrafficData,
};
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::core_binary_selection::{CoreBinarySelection, CoreBinarySelectionSource};
use infiltrator_ports::core_capability_provider::CoreCapabilityProvider;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::engine_backend_factory::EngineBackendPorts;
use infiltrator_ports::engine_backend_factory::{EngineBackendFactory, EngineBackendProvider};
use infiltrator_ports::error::PortError;
use infiltrator_ports::host_runtime::{HostRuntime, TunServiceStatus};
use infiltrator_ports::outbound::OutboundScopeSource;
use infiltrator_ports::profile_configuration_transaction::ProfileConfigurationTransactionPort;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::runtime_configuration::RuntimeConfigurationPorts;
use infiltrator_ports::runtime_gateway::{ManagedRuntime, RuntimeGateway, RuntimeStream};
use singbox_adapter::command::{SingBoxCommandPort, TokioSingBoxCommandPort};
use singbox_adapter::controller_configuration::SingBoxControllerConfiguration;
use singbox_adapter::engine_id;
use singbox_adapter::http::{ReqwestSingBoxClashHttpPort, SingBoxClashHttpPort};
use std::path::PathBuf;
use std::process;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) struct SingBoxRuntime {
    application: Arc<CoreApplication>,
    binary_path: PathBuf,
    controller_url: String,
    _configuration_transaction: Arc<dyn ProfileConfigurationTransactionPort>,
    _watchdog: infiltrator_composition::CoreWatchdogHandle,
}

impl SingBoxRuntime {
    pub(crate) async fn prepare(
        selection: CoreBinarySelection,
        reference_store: Arc<dyn ActiveProfileReferenceStore>,
        profile_store: Arc<dyn ProfileStore>,
        config_path: PathBuf,
        preflight: ActiveProfileSelectionPreflight,
    ) -> anyhow::Result<Self> {
        let command: Arc<dyn SingBoxCommandPort> = Arc::new(TokioSingBoxCommandPort);
        let http: Arc<dyn SingBoxClashHttpPort> = Arc::new(ReqwestSingBoxClashHttpPort);
        Self::prepare_with_ports(
            selection,
            reference_store,
            profile_store,
            config_path,
            preflight,
            command,
            http,
        )
        .await
    }

    async fn prepare_with_ports(
        selection: CoreBinarySelection,
        reference_store: Arc<dyn ActiveProfileReferenceStore>,
        profile_store: Arc<dyn ProfileStore>,
        config_path: PathBuf,
        preflight: ActiveProfileSelectionPreflight,
        command: Arc<dyn SingBoxCommandPort>,
        http: Arc<dyn SingBoxClashHttpPort>,
    ) -> anyhow::Result<Self> {
        let engine_id = engine_id();
        validate_selection(&selection, &engine_id)?;
        if preflight.reference.engine_id != engine_id || profile_store.engine_id() != engine_id {
            return Err(anyhow::Error::new(PortError::unsupported(
                Capability::CoreLifecycle,
                "sing-box runtime inputs do not share the selected engine identity",
            )));
        }

        let workspace = profile_store
            .load_workspace(&preflight.reference.profile_id)
            .await?;
        if workspace.document != preflight.document {
            return Err(anyhow::Error::new(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "sing-box profile changed after product preflight",
                true,
            ))));
        }
        let controller =
            SingBoxControllerConfiguration::from_profile_json(&workspace.document.raw_bytes)?;
        let capability_provider = preflight.capability_provider();
        validate_version_capability(&capability_provider, &selection, &engine_id)?;
        let provider = Arc::new(SingBoxEngineBackendProvider::new(
            selection.binary_path.clone(),
            config_path,
            desktop_singbox_instance_id()?,
            controller.clone(),
            command,
            http,
        )?);
        let factory_provider: Arc<dyn EngineBackendProvider> = provider.clone();
        let factory: Arc<dyn EngineBackendFactory> =
            Arc::new(EngineBackendFactoryRegistry::new([factory_provider])?);
        let selected = ActiveProfileBackendSelector::new(
            reference_store,
            [Arc::clone(&profile_store)],
            factory,
            capability_provider,
        )
        .select_backend_from_preflight(preflight, &PreviousCoreOwner::Unowned)
        .await?;
        let backend = selected.backend.clone();
        let application_runtime =
            infiltrator_composition::tokio_application_runtime().map_err(anyhow::Error::msg)?;
        let application = Arc::new(CoreApplication::new(
            backend.process(),
            backend.readiness(),
            application_runtime,
        ));

        if let Err(failure) = selected.install_core_capabilities(application.as_ref()) {
            let error = PortError::Rejected(failure);
            let close_error = application.close().await.err();
            return Err(close_candidate_error(error, close_error));
        }
        let binding = bind_core(application.as_ref(), &provider, &backend, profile_store);
        if let Err(error) = binding {
            let close_error = application.close().await.err();
            return Err(close_candidate_error(error, close_error));
        }
        if let Err(error) = selected
            .install_owner_binding_for_application(application.as_ref())
            .await
        {
            let _ = application.close().await;
            return Err(anyhow::Error::new(error));
        }

        let configuration_transaction = backend.configuration_transaction();
        let watchdog = spawn_core_watchdog(Arc::clone(&application));
        Ok(Self {
            application,
            binary_path: selection.binary_path,
            controller_url: controller.endpoint(),
            _configuration_transaction: configuration_transaction,
            _watchdog: watchdog,
        })
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
            singbox_surface_capabilities(),
            sample_interval,
        )
        .await
    }
}

fn bind_core(
    application: &CoreApplication,
    provider: &SingBoxEngineBackendProvider,
    backend: &EngineBackendPorts,
    profile_store: Arc<dyn ProfileStore>,
) -> Result<(), PortError> {
    let owner_scope: Arc<dyn OutboundScopeSource> = Arc::new(application.clone());
    provider.bind_core_owner_scope(backend, owner_scope)?;
    let lifecycle: Arc<dyn CoreLifecyclePort> = Arc::new(application.clone());
    provider.bind_profile_configuration_transaction(backend, profile_store, lifecycle)
}

fn close_candidate_error(error: PortError, close_error: Option<Failure>) -> anyhow::Error {
    match close_error {
        None => anyhow::Error::new(error),
        Some(close_error) => anyhow::Error::new(PortError::Rejected(Failure::new(
            ErrorCode::InvalidState,
            format!(
                "{error}; closing the unstarted sing-box candidate failed: {}",
                close_error.message
            ),
            true,
        ))),
    }
}

fn validate_selection(
    selection: &CoreBinarySelection,
    engine_id: &EngineId,
) -> Result<(), PortError> {
    if selection.engine_id != *engine_id {
        return Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "sing-box desktop runtime received a selection for another engine",
        ));
    }
    if !matches!(selection.source, CoreBinarySelectionSource::ManagedVersion) {
        return Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "sing-box desktop runtime requires an explicitly managed binary",
        ));
    }
    match &selection.verification {
        CoreArtifactVerification::Verified {
            engine_id: verified_engine,
            version,
        } if verified_engine == engine_id && version == selection.version.raw() => Ok(()),
        _ => Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "sing-box binary selection lacks matching managed artifact verification",
        )),
    }
}

fn validate_version_capability(
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
    let owner_is_application = matches!(
        &resolution.engine,
        Some(EngineCapabilityDecision::OwnerManaged {
            owner: AdapterOperationOwner::CoreLifecycle,
        })
    );
    match (resolution.status, owner_is_application) {
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
                "sing-box Core start is not evidenced for selected version {} on this target: {status:?}",
                selection.version.raw()
            ),
        )),
    }
}

fn desktop_singbox_instance_id() -> Result<CoreInstanceId, PortError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| PortError::Failed("system clock predates the Unix epoch".to_owned()))?
        .as_nanos();
    CoreInstanceId::new(format!("desktop-sing-box-{}-{timestamp}", process::id()))
        .map_err(|error| PortError::Failed(error.to_string()))
}

fn singbox_surface_capabilities() -> CapabilitySnapshot {
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
                reason: "sing-box desktop product composition has not connected this operation"
                    .to_owned(),
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
impl RuntimeGateway for SingBoxRuntime {
    fn current_core_scope(&self) -> Result<CoreScope, PortError> {
        unsupported(
            Capability::CoreLifecycle,
            "sing-box desktop composition has not connected runtime gateway operations",
        )
    }

    fn configuration_ports(&self) -> RuntimeConfigurationPorts {
        RuntimeConfigurationPorts::unsupported()
    }

    async fn switch_proxy(&self, _group: &str, _proxy: &str) -> Result<(), PortError> {
        unsupported(
            Capability::RoutingIntent,
            "sing-box proxy selection is not product-connected",
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
            "sing-box delay probing is not product-connected",
        )
    }

    async fn get_proxy_providers(&self) -> Result<Vec<ProxyProvider>, PortError> {
        unsupported(
            Capability::RoutingIntent,
            "sing-box proxy-provider listing is not product-connected",
        )
    }

    async fn get_rule_providers(&self) -> Result<Vec<RuleProvider>, PortError> {
        unsupported(
            Capability::RoutingIntent,
            "sing-box rule-provider listing is not product-connected",
        )
    }

    async fn update_proxy_provider(&self, _name: &str) -> Result<(), PortError> {
        unsupported(
            Capability::RoutingIntent,
            "sing-box proxy-provider update is not product-connected",
        )
    }

    async fn update_rule_provider(&self, _name: &str) -> Result<(), PortError> {
        unsupported(
            Capability::RoutingIntent,
            "sing-box rule-provider update is not product-connected",
        )
    }

    async fn get_connections(&self) -> Result<ConnectionSnapshot, PortError> {
        unsupported(
            Capability::Connections,
            "sing-box connection listing is not product-connected",
        )
    }

    async fn get_memory(&self) -> Result<MemoryData, PortError> {
        unsupported(
            Capability::Connections,
            "sing-box memory observation is not product-connected",
        )
    }

    async fn close_connection(&self, _id: &str) -> Result<(), PortError> {
        unsupported(
            Capability::Connections,
            "sing-box connection close is not product-connected",
        )
    }

    async fn close_all_connections(&self) -> Result<(), PortError> {
        unsupported(
            Capability::Connections,
            "sing-box connection close is not product-connected",
        )
    }

    async fn stream_logs(
        &self,
        _level: Option<String>,
    ) -> Result<RuntimeStream<String>, PortError> {
        unsupported(
            Capability::Logs,
            "sing-box log streaming is not product-connected",
        )
    }

    async fn stream_traffic(&self) -> Result<RuntimeStream<TrafficData>, PortError> {
        unsupported(
            Capability::Connections,
            "sing-box traffic streaming is not product-connected",
        )
    }

    async fn stream_connections(&self) -> Result<RuntimeStream<ConnectionSnapshot>, PortError> {
        unsupported(
            Capability::Connections,
            "sing-box connection streaming is not product-connected",
        )
    }
}

#[async_trait]
impl ManagedRuntime for SingBoxRuntime {
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
            "sing-box restart is not evidence-backed",
        )
    }

    async fn http_proxy_endpoint(&self) -> Result<Option<String>, PortError> {
        unsupported(
            Capability::CoreLifecycle,
            "sing-box proxy endpoint projection is not product-connected",
        )
    }

    async fn shutdown(&self) -> Result<(), PortError> {
        CoreLifecyclePort::stop(self.application.as_ref())
            .await
            .map(|_| ())
            .map_err(|error| PortError::Failed(error.to_string()))
    }
}

impl HostRuntime for SingBoxRuntime {
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

#[cfg(test)]
#[path = "singbox_runtime_tests.rs"]
mod tests;

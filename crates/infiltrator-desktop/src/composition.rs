//! Desktop composition root helpers.
//!
//! This module is the only place in the desktop host that assembles a
//! Tokio-backed application service with a concrete process and controller
//! adapter. UI crates receive the resulting `CoreApplication` handle.

use crate::certificate_authority::DesktopCertificateAuthority;
use crate::host::DesktopHostCapabilities;
use crate::log_export::DesktopLogExportPort;
use crate::mtu::DesktopMtuProbe;
use crate::network_roaming::DesktopNetworkRoamingPort;
use crate::pac_service::DesktopPacServicePort;
use crate::rule_provider_cache::DesktopRuleProviderCache;
use crate::runtime::MihomoRuntime;
use crate::service::ServiceManager;
use crate::service_mode::DesktopServiceMode;
use crate::storage::{home_dir, port_conflict, version_factory};
use crate::subscription_import_port::DesktopSubscriptionImportPort;
use crate::subscription_notification_port::DesktopSubscriptionNotificationPort;
use crate::surface::SurfaceEngines;
use crate::system_proxy::DesktopSystemProxy;
use crate::uwp_loopback_port::DesktopUwpLoopbackPort;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::core_capability_application::CoreCapabilityApplication;
use infiltrator_application::log_export_application::LogExportApplication;
use infiltrator_application::mtu_application::MtuApplication;
use infiltrator_application::network_roaming_application::NetworkRoamingApplication;
use infiltrator_application::outbound_application::OutboundApplication;
use infiltrator_application::pac_application::PacApplication;
use infiltrator_application::port_conflict_application::PortConflictApplication;
use infiltrator_application::service_mode_application::ServiceModeApplication;
use infiltrator_application::surface_application::SurfacePump;
use infiltrator_application::system_proxy_application::SystemProxyApplication;
use infiltrator_application::uwp_loopback_application::UwpLoopbackApplication;
use infiltrator_application::version_application::VersionApplication;
use infiltrator_composition::engine_backend_factory::EngineBackendFactoryRegistry;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_capabilities::{
    ConnectionOperation, CoreGarbageCollectionOperation, CoreLifecycleOperation, CoreOperation,
    DnsOperation, FakeIpCacheOperation, GeoOperation, LogsOperation, OutboundOperation,
    ProfileOperation, ProviderOperation, ProxyControlOperation, RulesOperation,
    SubscriptionOperation, TunOperation, VersionOperation,
};
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_core::apply::EndpointConfigReloader;
use infiltrator_core::profile_configuration_transaction::{
    MihomoBinaryConfigurationCandidateValidator, MihomoProfileConfigurationTransactionAdapter,
};
use infiltrator_core::version_port::MihomoVersionPort;
use infiltrator_ports::core_binary_selection::{
    BundledBinarySource, CoreBinarySelection, CoreBinarySelectionPolicy, CoreBinarySelectionRequest,
};
use infiltrator_ports::core_log_level::CoreLogLevelPort;
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::endpoint::EndpointSource;
use infiltrator_ports::engine_backend_factory::{
    EngineBackendFactory, EngineBackendOwnerScopeBinding, EngineBackendOwnerScopeBindingResult,
    EngineBackendPorts, EngineBackendProvider, EngineOperationPorts,
};
use infiltrator_ports::engine_version_backend_factory::EngineVersionBackendFactory;
use infiltrator_ports::error::PortError;
use infiltrator_ports::host_runtime::HostRuntime;
use infiltrator_ports::outbound::OutboundPort;
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationTransactionBinding, ProfileConfigurationTransactionScope,
};
use infiltrator_ports::profile_document_codec::ProfileDocumentCodecPort;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::subscription_source::SubscriptionSource;
use mihomo_api::client::MihomoClient;
use mihomo_api::core_capability_provider::MihomoCoreCapabilityProvider;
use mihomo_api::outbound::MihomoOutboundAdapter;
use mihomo_api::overview::ControllerOverviewReader;
use mihomo_api::readiness::ControllerReadiness;
use mihomo_api::routing_intent::MihomoRoutingIntentAdapter;
use mihomo_config::manager::ConfigManager;
use mihomo_config::profile_document_codec::MihomoProfileDocumentCodec;
use mihomo_platform::defaults::DefaultCredentialStore;
use mihomo_version::bundled_kernel::packaged_candidates;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// Engine-neutral handles returned to the owning desktop session. The
/// registered Mihomo provider remains the only concrete runtime owner here.
pub(crate) struct DesktopProductBackend {
    pub application: Arc<CoreApplication>,
    pub host_runtime: Arc<dyn HostRuntime>,
    pub surface_pump: SurfacePump,
}

pub(crate) async fn open_default_product_backend(
    surface: SurfaceKind,
) -> anyhow::Result<DesktopProductBackend> {
    open_product_backend(MihomoVersionPort::engine_identifier(), surface).await
}

/// Open the registered desktop Core provider from a typed engine identity.
/// Unknown engines fail in the registered runtime provider lookup before
/// version selection, configuration, process, or controller setup begins.
pub(crate) async fn open_product_backend(
    engine_id: EngineId,
    surface: SurfaceKind,
) -> anyhow::Result<DesktopProductBackend> {
    desktop_runtime_factory()?.provider(&engine_id)?;
    let version_factory = version_factory()?;
    let home = home_dir()?;
    open_product_backend_with_source(
        engine_id,
        surface,
        &version_factory,
        BundledBinarySource {
            candidates: packaged_candidates()?,
            data_directory: home,
        },
    )
    .await
}

async fn open_product_backend_with_source(
    engine_id: EngineId,
    surface: SurfaceKind,
    version_factory: &dyn EngineVersionBackendFactory,
    bundled_source: BundledBinarySource,
) -> anyhow::Result<DesktopProductBackend> {
    let provider = desktop_runtime_factory()?.provider(&engine_id)?;
    let selection = select_product_binary(
        version_factory,
        &engine_id,
        CoreBinarySelectionPolicy::PreferBundled(bundled_source),
    )
    .await?;
    match provider
        .create(selection, DesktopRuntimeRequest::OpenProduct { surface })
        .await?
    {
        DesktopRuntimeOutcome::Product(backend) => Ok(backend),
        DesktopRuntimeOutcome::BootHost(_) => Err(anyhow::anyhow!(
            "desktop Mihomo provider returned a boot host for a product request"
        )),
    }
}

pub(crate) async fn materialize_boot_host(
    selection: CoreBinarySelection,
) -> anyhow::Result<Arc<dyn HostRuntime>> {
    let provider = desktop_runtime_factory()?.provider(&selection.engine_id)?;
    match provider
        .create(selection, DesktopRuntimeRequest::MaterializeBootHost)
        .await?
    {
        DesktopRuntimeOutcome::BootHost(host_runtime) => Ok(host_runtime),
        DesktopRuntimeOutcome::Product(_) => Err(anyhow::anyhow!(
            "desktop Mihomo provider returned a product session for a boot-host request"
        )),
    }
}

/// Product-level runtime construction has a wider host type than the common
/// process/config/runtime bundle. Keep that host-specific boundary here while
/// resolving it with the same engine identity and provider semantics.
#[async_trait::async_trait]
trait DesktopEngineRuntimeProvider: Send + Sync {
    fn engine_id(&self) -> EngineId;

    async fn create(
        &self,
        selection: CoreBinarySelection,
        request: DesktopRuntimeRequest,
    ) -> anyhow::Result<DesktopRuntimeOutcome>;
}

#[derive(Clone, Copy)]
enum DesktopRuntimeRequest {
    OpenProduct { surface: SurfaceKind },
    MaterializeBootHost,
}

enum DesktopRuntimeOutcome {
    Product(DesktopProductBackend),
    BootHost(Arc<dyn HostRuntime>),
}

struct DesktopEngineRuntimeFactory {
    providers: HashMap<EngineId, Arc<dyn DesktopEngineRuntimeProvider>>,
}

impl DesktopEngineRuntimeFactory {
    fn new(
        providers: impl IntoIterator<Item = Arc<dyn DesktopEngineRuntimeProvider>>,
    ) -> Result<Self, PortError> {
        let mut by_engine = HashMap::new();
        for provider in providers {
            let engine_id = provider.engine_id();
            if by_engine.insert(engine_id.clone(), provider).is_some() {
                return Err(PortError::Failed(format!(
                    "duplicate desktop runtime provider for engine {}",
                    engine_id.as_str()
                )));
            }
        }
        Ok(Self {
            providers: by_engine,
        })
    }

    fn provider(
        &self,
        engine_id: &EngineId,
    ) -> Result<Arc<dyn DesktopEngineRuntimeProvider>, PortError> {
        self.providers.get(engine_id).cloned().ok_or_else(|| {
            PortError::unsupported(
                Capability::CoreLifecycle,
                format!(
                    "no desktop Core runtime provider is registered for {}",
                    engine_id.as_str()
                ),
            )
        })
    }
}

fn desktop_runtime_factory() -> Result<DesktopEngineRuntimeFactory, PortError> {
    let provider: Arc<dyn DesktopEngineRuntimeProvider> = Arc::new(DesktopMihomoRuntimeProvider);
    DesktopEngineRuntimeFactory::new([provider])
}

struct DesktopMihomoRuntimeProvider;

#[async_trait::async_trait]
impl DesktopEngineRuntimeProvider for DesktopMihomoRuntimeProvider {
    fn engine_id(&self) -> EngineId {
        MihomoVersionPort::engine_identifier()
    }

    async fn create(
        &self,
        selection: CoreBinarySelection,
        request: DesktopRuntimeRequest,
    ) -> anyhow::Result<DesktopRuntimeOutcome> {
        if selection.engine_id != self.engine_id() {
            return Err(anyhow::Error::new(PortError::unsupported(
                Capability::CoreLifecycle,
                format!(
                    "Mihomo desktop provider cannot own selected engine {}",
                    selection.engine_id.as_str()
                ),
            )));
        }

        match request {
            DesktopRuntimeRequest::OpenProduct { surface } => {
                let runtime = Arc::new(MihomoRuntime::prepare_offline(selection).await?);
                let product_setup = async {
                    runtime.install_product_commands().await?;
                    runtime
                        .surface_pump(surface, Duration::from_millis(700))
                        .await
                }
                .await;
                let surface_pump = match product_setup {
                    Ok(pump) => pump,
                    Err(error) => {
                        let _ = runtime.application().close().await;
                        return Err(error);
                    }
                };
                let application = runtime.application();
                let host_runtime: Arc<dyn HostRuntime> = runtime;
                Ok(DesktopRuntimeOutcome::Product(DesktopProductBackend {
                    application,
                    host_runtime,
                    surface_pump,
                }))
            }
            DesktopRuntimeRequest::MaterializeBootHost => {
                let runtime = Arc::new(MihomoRuntime::bootstrap_offline(selection).await?);
                let host_runtime: Arc<dyn HostRuntime> = runtime;
                Ok(DesktopRuntimeOutcome::BootHost(host_runtime))
            }
        }
    }
}

async fn select_product_binary(
    factory: &dyn EngineVersionBackendFactory,
    engine_id: &EngineId,
    policy: CoreBinarySelectionPolicy,
) -> Result<CoreBinarySelection, PortError> {
    let selector = factory.binary_selection_port(engine_id)?;
    if selector.engine_id() != *engine_id {
        return Err(PortError::Failed(format!(
            "binary selector registered for {} returned adapter for {}",
            engine_id.as_str(),
            selector.engine_id().as_str()
        )));
    }
    let selection = selector
        .select(CoreBinarySelectionRequest {
            engine_id: engine_id.clone(),
            policy,
        })
        .await?;
    if selection.engine_id != *engine_id {
        return Err(PortError::Failed(format!(
            "binary selector for {} returned a selection for {}",
            engine_id.as_str(),
            selection.engine_id.as_str()
        )));
    }
    Ok(selection)
}

/// Build the 0.30 lifecycle application over the desktop process host and
/// Mihomo controller readiness adapter.
///
/// The speedtest owner and surfaces use the same engine-scoped outbound port
/// and accepted inventory revision as selection and one-shot probes.
pub(crate) fn core_application(
    service: &ServiceManager,
    backend: DesktopMihomoBackend,
    secret: Option<String>,
    engines: SurfaceEngines,
    speedtest: infiltrator_application::speedtest_application::SpeedtestApplication,
    profile_store: Arc<dyn ProfileStore>,
    subscription_source: Arc<dyn SubscriptionSource>,
    config_manager: Arc<ConfigManager<DefaultCredentialStore>>,
    endpoints: Arc<dyn EndpointSource>,
    apply_guard: Arc<Mutex<()>>,
) -> anyhow::Result<CoreApplication> {
    let exports = Arc::new(DesktopLogExportPort::new(profile_store.config_dir()));
    let export_secrets = secret.iter().cloned().collect();
    let client = backend.client.clone();
    let runtime = infiltrator_composition::tokio_application_runtime()
        .map_err(|error| anyhow::anyhow!(error))?;
    let application = CoreApplication::new_with_overview(
        backend.ports.process(),
        backend.ports.readiness(),
        Arc::new(ControllerOverviewReader::new(client.clone())),
        runtime.clone(),
    );
    match backend
        .ports
        .bind_owner_scope_source(Arc::new(application.clone()))?
    {
        EngineBackendOwnerScopeBindingResult::NotRequired => {}
        EngineBackendOwnerScopeBindingResult::Bound => {}
    }
    let transaction_binding = backend.ports.configuration_transaction_binding();
    let document_codec = backend.ports.configuration();
    let transaction_scope = ProfileConfigurationTransactionScope::new(document_codec.scope());
    let stopped_validator = Arc::new(MihomoBinaryConfigurationCandidateValidator::new(
        transaction_scope,
        service.binary_path().to_path_buf(),
        config_manager.config_dir().to_path_buf(),
    )?);
    let transaction = Arc::new(MihomoProfileConfigurationTransactionAdapter::new(
        document_codec,
        stopped_validator,
        Arc::new(application.clone()),
        config_manager,
        Arc::new(EndpointConfigReloader::new(endpoints)),
        apply_guard,
    )?);
    transaction_binding.bind(transaction)?;
    application
        .install_log_gateway(Arc::new(backend.client.clone()))
        .map_err(|failure| anyhow::anyhow!(failure.message))?;
    let outbound_port = backend.ports.operations().outbound()?;
    application
        .install_outbound_port(outbound_port.clone())
        .map_err(|failure| anyhow::anyhow!(failure.message))?;
    let outbound_application = OutboundApplication::new(outbound_port);
    speedtest
        .install_outbound_application(outbound_application.clone())
        .map_err(|failure| anyhow::anyhow!(failure.message))?;
    application
        .install_speedtest_application(speedtest)
        .map_err(|failure| anyhow::anyhow!(failure.message))?;
    let commands = desktop_command_application(DesktopCommandServices {
        binary_path: service.binary_path().to_path_buf(),
        client: client.clone(),
        backend_ports: backend.ports,
        log_stream: DesktopLogStreamAdapter::Unavailable,
        engines,
        profile_store,
        subscription_source,
    })?;
    application
        .install_core_capability_application(commands.core_capability_application())
        .map_err(|failure| anyhow::anyhow!(failure.message))?;
    application.install_command_handler(Arc::new(
        commands
            .with_logs(application.log_application())
            .with_outbound(outbound_application)
            .with_outbound_ui_preferences(application.outbound_ui_preferences_application())
            .with_log_export(LogExportApplication::new(
                application.log_application(),
                Some(exports),
                export_secrets,
            )),
    ));
    Ok(application)
}

/// Ports and engines used by either desktop peer's command service.
pub struct DesktopCommandServices {
    pub binary_path: PathBuf,
    pub client: MihomoClient,
    pub backend_ports: EngineBackendPorts,
    /// Whether this product composition has installed a host adapter that
    /// proves the active Core scope before opening a log stream.
    pub log_stream: DesktopLogStreamAdapter,
    pub engines: SurfaceEngines,
    pub profile_store: Arc<dyn ProfileStore>,
    pub subscription_source: Arc<dyn SubscriptionSource>,
}

/// Typed declaration of log-stream ownership at the command composition
/// boundary. A controller client alone cannot provide the required scope
/// proof, so only the managed runtime composition may enable this operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopLogStreamAdapter {
    Unavailable,
    ManagedRuntimeScopeBound,
}

pub fn desktop_command_application(
    services: DesktopCommandServices,
) -> anyhow::Result<CommandApplication> {
    let DesktopCommandServices {
        binary_path,
        client,
        backend_ports,
        log_stream,
        engines,
        profile_store,
        subscription_source,
    } = services;
    let SurfaceEngines {
        profiles,
        configuration: _,
        snapshots,
        scripts,
        script_exports,
        doctor,
        rule_tracer,
        rule_list,
        dns_cache,
        dns_query,
        dns_latency,
        dns_leak,
        stun_probe,
    } = engines;
    let profiles = profiles.with_document_codec(backend_ports.configuration());
    let configuration = ConfigurationApplication::from_profile_application(profiles.clone());
    let runtime = infiltrator_composition::tokio_application_runtime()
        .map_err(|error| anyhow::anyhow!(error))?;
    // Keep the Bevy command seam live in the desktop composition: version
    // rollback is an application use-case, not a UI-local file operation.
    let version_factory = version_factory()?;
    let versions =
        VersionApplication::from_factory(MihomoVersionPort::engine_identifier(), &version_factory)?;
    let service_mode =
        ServiceModeApplication::new(Arc::new(DesktopServiceMode::new(binary_path.clone())));
    let port_conflicts = PortConflictApplication::new(Arc::new(port_conflict()?));
    let pac = PacApplication::new(
        Arc::new(client.clone()),
        Arc::new(DesktopPacServicePort::shared()),
    );
    let network_roaming = NetworkRoamingApplication::new(
        Arc::new(DesktopNetworkRoamingPort::shared()),
        Some(Arc::new(client.clone())),
    );
    // DUAL-11-06/07: the kernel's provider cache lives next to the profiles
    // (`-d <config dir>`), so the command handler purges exactly that folder.
    let rule_provider_cache = Arc::new(DesktopRuleProviderCache::new(profile_store.config_dir()));
    let mut adapter_operations = vec![
        CoreOperation::Profile(ProfileOperation::ApplyFeature),
        CoreOperation::Dns(DnsOperation::ApplyConfiguration),
        CoreOperation::Dns(DnsOperation::Query),
        CoreOperation::Rules(RulesOperation::Trace),
        CoreOperation::Rules(RulesOperation::Apply),
        CoreOperation::Subscription(SubscriptionOperation::Refresh),
        CoreOperation::Subscription(SubscriptionOperation::Import),
        CoreOperation::Subscription(SubscriptionOperation::Update),
        CoreOperation::Provider(ProviderOperation::ListRule),
        CoreOperation::Provider(ProviderOperation::UpdateRule),
        CoreOperation::Outbound(OutboundOperation::Select),
        CoreOperation::Outbound(OutboundOperation::Probe),
        CoreOperation::Outbound(OutboundOperation::List),
        CoreOperation::Connection(ConnectionOperation::Close),
        CoreOperation::Logs(LogsOperation::SetLevel),
        CoreOperation::Geo(GeoOperation::UpdateDatabases),
        CoreOperation::FakeIpCache(FakeIpCacheOperation::Flush),
        CoreOperation::Tun(TunOperation::Configure),
        CoreOperation::CoreGarbageCollection(CoreGarbageCollectionOperation::Trigger),
        CoreOperation::Version(VersionOperation::Inspect),
        CoreOperation::Version(VersionOperation::Rollback),
        CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
        CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
        CoreOperation::Lifecycle(CoreLifecycleOperation::Restart),
        CoreOperation::ProxyControl(ProxyControlOperation::SetRoutingIntent),
        CoreOperation::ProxyControl(ProxyControlOperation::GetRoutingIntent),
    ];
    match log_stream {
        DesktopLogStreamAdapter::Unavailable => {}
        DesktopLogStreamAdapter::ManagedRuntimeScopeBound => {
            adapter_operations.push(CoreOperation::Logs(LogsOperation::Stream));
        }
    }
    let core_capabilities = CoreCapabilityApplication::new(Arc::new(
        MihomoCoreCapabilityProvider::new(HostKind::Desktop, adapter_operations),
    ));
    let core_log_level_port: Arc<dyn CoreLogLevelPort> = Arc::new(client.clone());
    Ok(CommandApplication::new()
        .with_core_capabilities(core_capabilities)
        .with_routing_intent_port(backend_ports.operations().routing_intent()?)
        .with_core_log_level_port(core_log_level_port)
        .with_host_capabilities(Arc::new(DesktopHostCapabilities))
        .with_scripts(scripts, script_exports)
        .with_doctor(doctor)
        .with_runtime(Arc::new(client.clone()))
        .with_application_runtime(runtime)
        .with_profile(profiles)
        .with_configuration(configuration)
        .with_subscription_source(subscription_source)
        .with_import_source(Arc::new(DesktopSubscriptionImportPort))
        .with_subscription_notifier(Arc::new(DesktopSubscriptionNotificationPort))
        .with_certificate_authority(Arc::new(DesktopCertificateAuthority::new()))
        .with_mtu(MtuApplication::new(Arc::new(DesktopMtuProbe::new())))
        .with_system_proxy(SystemProxyApplication::new(Arc::new(
            DesktopSystemProxy::new(),
        )))
        .with_uwp_loopback(UwpLoopbackApplication::new(Arc::new(
            DesktopUwpLoopbackPort,
        )))
        .with_pac(pac)
        .with_network_roaming(network_roaming)
        .with_versions(versions)
        .with_service_mode(service_mode)
        .with_port_conflicts(port_conflicts)
        .with_snapshots(snapshots)
        .with_rule_tracer(rule_tracer)
        .with_rule_list(rule_list)
        .with_dns_query(dns_query)
        .with_dns_cache(dns_cache)
        .with_dns_latency(dns_latency)
        .with_dns_leak(dns_leak)
        .with_stun_probe(stun_probe)
        .with_rule_provider_cache(rule_provider_cache))
}

struct DesktopMihomoBackendProvider {
    process: Arc<dyn CoreProcess>,
    controller_url: String,
    secret: Option<String>,
    client: MihomoClient,
}

impl EngineBackendProvider for DesktopMihomoBackendProvider {
    fn engine_id(&self) -> EngineId {
        MihomoVersionPort::engine_identifier()
    }

    fn create_backend(&self) -> Result<EngineBackendPorts, PortError> {
        let engine_id = self.engine_id();
        let readiness: Arc<dyn CoreReadiness> = Arc::new(ControllerReadiness::new(
            self.controller_url.clone(),
            self.secret.clone(),
        ));
        let configuration = Arc::new(MihomoProfileDocumentCodec);
        let transaction = Arc::new(ProfileConfigurationTransactionBinding::new(
            ProfileConfigurationTransactionScope::new(configuration.scope()),
        ));
        let owner_scope = Arc::new(EngineBackendOwnerScopeBinding::new(engine_id.clone()));
        let outbound: Arc<dyn OutboundPort> = Arc::new(MihomoOutboundAdapter::new(
            Arc::new(self.client.clone()),
            owner_scope.clone(),
        ));
        let operations = EngineOperationPorts::available(
            outbound,
            Arc::new(MihomoRoutingIntentAdapter::new(self.client.clone())),
            owner_scope,
        );
        let ports = EngineBackendPorts::new(
            engine_id,
            Arc::clone(&self.process),
            readiness,
            configuration,
            transaction,
            operations,
        )?;
        Ok(ports)
    }
}

pub(crate) struct DesktopMihomoBackend {
    pub ports: EngineBackendPorts,
    pub client: MihomoClient,
}

pub(crate) fn desktop_backend(
    service: &ServiceManager,
    controller_url: &str,
    secret: Option<String>,
    engine_id: &EngineId,
) -> Result<DesktopMihomoBackend, PortError> {
    desktop_runtime_factory()?.provider(engine_id)?;
    let client = MihomoClient::new(controller_url, secret.clone())
        .map_err(|error| PortError::Network(error.to_string()))?;
    let provider = Arc::new(DesktopMihomoBackendProvider {
        process: service.core_process(),
        controller_url: controller_url.to_owned(),
        secret,
        client,
    });
    desktop_backend_from_provider(provider, engine_id)
}

fn desktop_backend_from_provider(
    provider: Arc<DesktopMihomoBackendProvider>,
    engine_id: &EngineId,
) -> Result<DesktopMihomoBackend, PortError> {
    let factory_provider: Arc<dyn EngineBackendProvider> = provider.clone();
    let registry = EngineBackendFactoryRegistry::new([factory_provider])?;
    let ports = registry.backend_ports(engine_id)?;
    Ok(DesktopMihomoBackend {
        ports,
        client: provider.client.clone(),
    })
}

#[cfg(test)]
#[path = "composition_tests.rs"]
mod tests;

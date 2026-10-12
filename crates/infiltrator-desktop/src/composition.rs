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
use infiltrator_composition::active_profile_backend::{
    ActiveProfileSelectionPreflight, PreviousCoreOwner, SelectedActiveProfileBackend,
    authorize_lifecycle_start, preflight_active_profile_reference,
};
use infiltrator_composition::engine_backend_factory::EngineBackendFactoryRegistry;
use infiltrator_composition::mihomo_engine_backend_provider::MihomoEngineBackendProvider;
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
use infiltrator_core::settings_io::app_config_manager;
use infiltrator_core::version_port::MihomoVersionPort;
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::core_binary_selection::{
    BundledBinarySource, CoreBinarySelection, CoreBinarySelectionPolicy, CoreBinarySelectionRequest,
};
use infiltrator_ports::core_capability_provider::CoreCapabilityProvider;
use infiltrator_ports::core_log_level::CoreLogLevelPort;
use infiltrator_ports::endpoint::EndpointSource;
use infiltrator_ports::engine_backend_factory::{
    EngineBackendOwnerScopeBindingResult, EngineBackendPorts, EngineBackendProvider,
};
use infiltrator_ports::engine_version_backend_factory::EngineVersionBackendFactory;
use infiltrator_ports::error::PortError;
use infiltrator_ports::host_runtime::HostRuntime;
use infiltrator_ports::profile_configuration_transaction::ProfileConfigurationTransactionScope;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::subscription_source::SubscriptionSource;
use mihomo_api::client::MihomoClient;
use mihomo_api::core_capability_provider::MihomoCoreCapabilityProvider;
use mihomo_api::overview::ControllerOverviewReader;
use mihomo_config::manager::ConfigManager;
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
    let config_manager = Arc::new(app_config_manager().await?);
    let capability_provider =
        desktop_core_capability_provider(DesktopLogStreamAdapter::ManagedRuntimeScopeBound);
    let mihomo_engine_id = MihomoVersionPort::engine_identifier();
    desktop_runtime_factory()?.provider(&mihomo_engine_id)?;
    authorize_lifecycle_start(&capability_provider, &mihomo_engine_id)?;
    // Do not create a new-install profile or publish a migrated reference
    // until this host/target is evidenced to own Core lifecycle.
    config_manager.migrate_legacy_mihomo_selection().await?;
    let references: Arc<dyn ActiveProfileReferenceStore> = config_manager.clone();
    let profiles: Arc<dyn ProfileStore> = config_manager.clone();
    let preflight = preflight_active_profile_reference(
        references,
        [profiles],
        Arc::clone(&capability_provider),
    )
    .await?;
    let engine_id = preflight.reference.engine_id.clone();
    // Provider existence is checked before binary selection can copy or stage
    // any candidate. Desktop SingBox remains Unsupported until a host provider
    // and target-specific Start capability exist.
    desktop_runtime_factory()?.provider(&engine_id)?;
    let version_factory = version_factory()?;
    let home = home_dir()?;
    open_product_backend_with_source(
        preflight,
        config_manager,
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
    preflight: ActiveProfileSelectionPreflight,
    config_manager: Arc<ConfigManager<DefaultCredentialStore>>,
    surface: SurfaceKind,
    version_factory: &dyn EngineVersionBackendFactory,
    bundled_source: BundledBinarySource,
) -> anyhow::Result<DesktopProductBackend> {
    let engine_id = preflight.reference.engine_id.clone();
    let provider = desktop_runtime_factory()?.provider(&engine_id)?;
    let selection = select_product_binary(
        version_factory,
        &engine_id,
        CoreBinarySelectionPolicy::PreferBundled(bundled_source),
    )
    .await?;
    match provider
        .create(
            selection,
            DesktopRuntimeRequest::OpenProduct {
                surface,
                config_manager,
                preflight,
            },
        )
        .await?
    {
        DesktopRuntimeOutcome::Product(backend) => Ok(backend),
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

enum DesktopRuntimeRequest {
    OpenProduct {
        surface: SurfaceKind,
        config_manager: Arc<ConfigManager<DefaultCredentialStore>>,
        preflight: ActiveProfileSelectionPreflight,
    },
}

enum DesktopRuntimeOutcome {
    Product(DesktopProductBackend),
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
            DesktopRuntimeRequest::OpenProduct {
                surface,
                config_manager,
                preflight,
            } => {
                let runtime = Arc::new(
                    MihomoRuntime::prepare_offline(selection, config_manager, preflight).await?,
                );
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
pub(crate) async fn core_application(
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
    let DesktopMihomoBackend {
        ports,
        client,
        capability_provider,
        selection,
    } = backend;
    let exports = Arc::new(DesktopLogExportPort::new(profile_store.config_dir()));
    let export_secrets = secret.iter().cloned().collect();
    let client = client.clone();
    let runtime = infiltrator_composition::tokio_application_runtime()
        .map_err(|error| anyhow::anyhow!(error))?;
    let application = CoreApplication::new_with_overview(
        ports.process(),
        ports.readiness(),
        Arc::new(ControllerOverviewReader::new(client.clone())),
        runtime.clone(),
    );
    match ports.bind_owner_scope_source(Arc::new(application.clone()))? {
        EngineBackendOwnerScopeBindingResult::NotRequired => {}
        EngineBackendOwnerScopeBindingResult::Bound => {}
    }
    let transaction_binding = ports.configuration_transaction_binding();
    let document_codec = ports.configuration();
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
        .install_log_gateway(Arc::new(client.clone()))
        .map_err(|failure| anyhow::anyhow!(failure.message))?;
    let outbound_port = ports.operations().outbound()?;
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
        backend_ports: ports,
        core_capability_provider: Arc::clone(&capability_provider),
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
    let DesktopBackendSelection::ActiveProfile(selection) = selection;
    selection
        .install_owner_binding_for_application(&application)
        .await?;
    Ok(application)
}

/// Ports and engines used by either desktop peer's command service.
pub struct DesktopCommandServices {
    pub binary_path: PathBuf,
    pub client: MihomoClient,
    pub backend_ports: EngineBackendPorts,
    pub core_capability_provider: Arc<dyn CoreCapabilityProvider>,
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
        core_capability_provider,
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
    let core_capabilities = CoreCapabilityApplication::new(core_capability_provider);
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

pub(crate) fn desktop_core_capability_provider(
    log_stream: DesktopLogStreamAdapter,
) -> Arc<dyn CoreCapabilityProvider> {
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
    Arc::new(MihomoCoreCapabilityProvider::new(
        HostKind::Desktop,
        adapter_operations,
    ))
}

pub(crate) struct DesktopMihomoBackend {
    pub ports: EngineBackendPorts,
    pub client: MihomoClient,
    pub capability_provider: Arc<dyn CoreCapabilityProvider>,
    pub selection: DesktopBackendSelection,
}

pub(crate) enum DesktopBackendSelection {
    ActiveProfile(SelectedActiveProfileBackend),
}

pub(crate) async fn desktop_product_backend(
    service: &ServiceManager,
    controller_url: &str,
    secret: Option<String>,
    config_manager: Arc<ConfigManager<DefaultCredentialStore>>,
    preflight: ActiveProfileSelectionPreflight,
) -> Result<DesktopMihomoBackend, PortError> {
    let capability_provider = preflight.capability_provider();
    let client = MihomoClient::new(controller_url, secret.clone())
        .map_err(|error| PortError::Network(error.to_string()))?;
    let readiness: Arc<dyn infiltrator_ports::core_process::CoreReadiness> = Arc::new(
        mihomo_api::readiness::ControllerReadiness::new(controller_url.to_owned(), secret),
    );
    let provider = Arc::new(MihomoEngineBackendProvider::new(
        service.core_process(),
        readiness,
        client.clone(),
    )?);
    let factory_provider: Arc<dyn EngineBackendProvider> = provider;
    let factory = Arc::new(EngineBackendFactoryRegistry::new([factory_provider])?);
    let references: Arc<dyn ActiveProfileReferenceStore> = config_manager.clone();
    let profiles: Arc<dyn ProfileStore> = config_manager;
    let selector =
        infiltrator_composition::active_profile_backend::ActiveProfileBackendSelector::new(
            references,
            [profiles],
            factory,
            Arc::clone(&capability_provider),
        );
    let selected = selector
        .select_backend_from_preflight(preflight, &PreviousCoreOwner::Unowned)
        .await?;
    if selected.reference.engine_id != MihomoVersionPort::engine_identifier() {
        return Err(PortError::unsupported(
            Capability::CoreLifecycle,
            format!(
                "desktop Mihomo host cannot bind selected engine {}",
                selected.reference.engine_id.as_str()
            ),
        ));
    }
    let ports = selected.backend.clone();
    let capability_provider = selected.capability_provider();
    Ok(DesktopMihomoBackend {
        ports,
        client,
        capability_provider,
        selection: DesktopBackendSelection::ActiveProfile(selected),
    })
}

#[cfg(test)]
#[path = "composition_tests.rs"]
mod tests;

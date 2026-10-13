//! Android host/application composition for the 0.30 seam.

use crate::configuration_candidate_validator::{
    AndroidBridgeConfigurationValidationClient, AndroidCoreConfigurationValidationClient,
    AndroidProfileConfigurationCandidateValidator,
};
use crate::core_launch_spec::AndroidMihomoProfileLaunchSpecProvider;
use crate::native_host::clipboard::AndroidClipboardPort;
use crate::runtime::{AndroidBridgeAdapter, AndroidCoreProcessAdapter};
use crate::vpn_service::AndroidVpnServicePort;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::core_capability_application::CoreCapabilityApplication;
use infiltrator_application::mtu_application::MtuApplication;
use infiltrator_application::offline_startup_application::OfflineStartupApplication;
use infiltrator_application::outbound_application::OutboundApplication;
use infiltrator_application::overview::UnavailableOverviewReader;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::vpn_application::VpnServiceApplication;
use infiltrator_composition::engine_backend_factory::EngineBackendFactoryRegistry;
use infiltrator_composition::mihomo_engine_backend_provider::MihomoEngineBackendProvider;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_capabilities::{
    ConnectionOperation, CoreLifecycleOperation, CoreOperation, GeoOperation, LogsOperation,
    OutboundOperation, ProfileOperation, ProviderOperation, ProxyControlOperation,
};
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface::HostKind;
use infiltrator_core::apply::EndpointConfigReloader;
use infiltrator_core::profile_configuration_transaction::MihomoProfileConfigurationTransactionAdapter;
use infiltrator_core::version_port::MihomoVersionPort;
use infiltrator_domain::runtime::{ProxyProvider, RuleProvider};
use infiltrator_platform::android_bridge::AndroidBridge;
use infiltrator_ports::core_capability_provider::CoreCapabilityProvider;
use infiltrator_ports::core_log_level::CoreLogLevelPort;
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::engine_backend_factory::{
    EngineBackendFactory, EngineBackendOwnerScopeBindingResult, EngineBackendPorts,
    EngineBackendProvider,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::overview::OverviewReader;
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationCandidateValidatorPort, ProfileConfigurationTransactionPort,
    ProfileConfigurationTransactionScope, UnavailableProfileConfigurationTransaction,
};
use infiltrator_ports::profile_document_codec::ProfileDocumentCodecPort;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::client::MihomoClient;
use mihomo_api::core_capability_provider::MihomoCoreCapabilityProvider;
use mihomo_api::overview::ControllerOverviewReader;
use mihomo_api::readiness::ControllerReadiness;
use mihomo_api::runtime_configuration::RuntimeScopeResolver;
use mihomo_config::endpoint::ProfileEndpointSource;
use mihomo_config::manager::ConfigManager;
use std::sync::Arc;
use tokio::sync::Mutex;

#[path = "composition/singbox.rs"]
pub(crate) mod singbox;

/// Explicit dependencies owned by the Android `:vpn` composition.
///
/// The profile store is supplied by `SharedCore`; this type deliberately does
/// not construct or clone a second `ConfigManager`.
#[derive(Clone)]
pub struct AndroidCoreComposition<S: SecureStore + 'static> {
    profile_store: Arc<ConfigManager<S>>,
}

#[derive(Clone)]
enum AndroidControllerAdapters {
    Available { client: MihomoClient },
    Unavailable { failure: PortError },
}

impl AndroidControllerAdapters {
    fn new(controller_url: &str, secret: Option<String>) -> Self {
        match MihomoClient::new(controller_url, secret) {
            Ok(client) => Self::Available { client },
            Err(error) => Self::Unavailable {
                failure: PortError::Network(error.to_string()),
            },
        }
    }

    fn overview_reader(&self) -> Arc<dyn OverviewReader> {
        match self {
            Self::Available { client } => Arc::new(ControllerOverviewReader::new(client.clone())),
            Self::Unavailable { failure } => {
                Arc::new(UnavailableOverviewReader::new(failure.clone()))
            }
        }
    }

    fn runtime_gateway(&self) -> Arc<dyn RuntimeGateway> {
        match self {
            Self::Available { client } => Arc::new(client.clone()),
            Self::Unavailable { failure } => Arc::new(UnavailableAndroidRuntimeGateway {
                failure: failure.clone(),
            }),
        }
    }

    fn capability_provider(&self) -> Arc<dyn CoreCapabilityProvider> {
        let mut adapter_operations = vec![
            CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
            CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
            CoreOperation::Lifecycle(CoreLifecycleOperation::Restart),
        ];
        match self {
            Self::Available { .. } => adapter_operations.extend([
                CoreOperation::Outbound(OutboundOperation::Select),
                CoreOperation::Outbound(OutboundOperation::Probe),
                CoreOperation::Outbound(OutboundOperation::List),
                CoreOperation::Profile(ProfileOperation::Validate),
                CoreOperation::Profile(ProfileOperation::Apply),
                CoreOperation::Connection(ConnectionOperation::Close),
                CoreOperation::Logs(LogsOperation::SetLevel),
                CoreOperation::Logs(LogsOperation::Stream),
                CoreOperation::Geo(GeoOperation::UpdateDatabases),
                CoreOperation::Provider(ProviderOperation::ListRule),
                CoreOperation::Provider(ProviderOperation::UpdateRule),
                CoreOperation::ProxyControl(ProxyControlOperation::SetRoutingIntent),
                CoreOperation::ProxyControl(ProxyControlOperation::GetRoutingIntent),
            ]),
            Self::Unavailable { .. } => {}
        }
        Arc::new(MihomoCoreCapabilityProvider::new(
            HostKind::Android,
            adapter_operations,
        ))
    }

    fn bind_configuration_scope(&self, application: &CoreApplication) {
        if let Self::Available { client } = self {
            let scope_application = application.clone();
            let scope_resolver: RuntimeScopeResolver = Arc::new(move || {
                let lifecycle = scope_application.snapshot().lifecycle_snapshot();
                match &lifecycle.lifecycle {
                    CoreLifecycle::Running | CoreLifecycle::Ready => {
                        lifecycle.scope().ok_or_else(|| {
                            PortError::Failed(
                                "active Core identity is unavailable for configuration access"
                                    .to_owned(),
                            )
                        })
                    }
                    _ => Err(PortError::Failed(
                        "configuration access requires a running Core".to_owned(),
                    )),
                }
            });
            client.set_configuration_scope_resolver(scope_resolver);
        }
    }
}

/// Build the same controller-derived capability source used by the owner
/// composition so launch preflight can reject an unsupported target before
/// the backend factory is dispatched.
pub(crate) fn android_core_capability_provider(
    controller_url: &str,
    secret: Option<String>,
) -> Arc<dyn CoreCapabilityProvider> {
    AndroidControllerAdapters::new(controller_url, secret).capability_provider()
}

/// Target-scoped lifecycle evidence can be checked before the one-time profile
/// migration creates a new-install document or publishes a selection.
pub(crate) fn android_core_lifecycle_capability_provider() -> Arc<dyn CoreCapabilityProvider> {
    Arc::new(MihomoCoreCapabilityProvider::new(
        HostKind::Android,
        [
            CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
            CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
            CoreOperation::Lifecycle(CoreLifecycleOperation::Restart),
        ],
    ))
}

struct AndroidMihomoBackendProvider<S: SecureStore + 'static> {
    bridge: Arc<dyn AndroidBridge>,
    profile_store: Arc<ConfigManager<S>>,
    controller_adapters: AndroidControllerAdapters,
    readiness: Arc<dyn CoreReadiness>,
}

impl<S: SecureStore + 'static> AndroidMihomoBackendProvider<S> {
    fn new<B: AndroidBridge + 'static>(
        engine_id: EngineId,
        bridge: B,
        profile_store: Arc<ConfigManager<S>>,
        controller_url: &str,
        secret: Option<String>,
        readiness: Arc<dyn CoreReadiness>,
    ) -> Result<Self, PortError> {
        Self::admit_engine(&engine_id)?;
        let controller_adapters = AndroidControllerAdapters::new(controller_url, secret);
        let bridge: Arc<dyn AndroidBridge> = Arc::new(bridge);
        Ok(Self {
            bridge,
            profile_store,
            controller_adapters,
            readiness,
        })
    }

    fn admit_engine(engine_id: &EngineId) -> Result<(), PortError> {
        let registered_engine = MihomoVersionPort::engine_identifier();
        if engine_id == &registered_engine {
            Ok(())
        } else {
            Err(PortError::unsupported(
                Capability::CoreLifecycle,
                format!(
                    "Android Mihomo backend provider cannot serve engine {}",
                    engine_id.as_str()
                ),
            ))
        }
    }
}

impl<S: SecureStore + 'static> EngineBackendProvider for AndroidMihomoBackendProvider<S> {
    fn engine_id(&self) -> EngineId {
        MihomoVersionPort::engine_identifier()
    }

    fn create_backend(&self) -> Result<EngineBackendPorts, PortError> {
        let working_directory = self.bridge.data_dir().ok_or_else(|| {
            PortError::Failed("Android Core launch working directory is unavailable".to_owned())
        })?;
        let launch_spec_source = Arc::new(AndroidMihomoProfileLaunchSpecProvider::new(
            Arc::clone(&self.profile_store),
            working_directory,
        ));
        let process: Arc<dyn CoreProcess> = Arc::new(AndroidCoreProcessAdapter::new(
            self.engine_id(),
            self.bridge.clone(),
            launch_spec_source,
        ));
        let provider = match &self.controller_adapters {
            AndroidControllerAdapters::Available { client } => MihomoEngineBackendProvider::new(
                process,
                Arc::clone(&self.readiness),
                client.clone(),
            )?,
            AndroidControllerAdapters::Unavailable { failure } => {
                MihomoEngineBackendProvider::without_controller(
                    process,
                    Arc::clone(&self.readiness),
                    failure.to_string(),
                )?
            }
        };
        provider.create_backend()
    }
}

pub(crate) fn android_mihomo_backend_provider<S: SecureStore + 'static>(
    engine_id: EngineId,
    bridge: Arc<dyn AndroidBridge>,
    profile_store: Arc<ConfigManager<S>>,
    controller_url: &str,
    secret: Option<String>,
    readiness: Arc<dyn CoreReadiness>,
) -> Result<Arc<dyn EngineBackendProvider>, PortError> {
    Ok(Arc::new(AndroidMihomoBackendProvider::new(
        engine_id,
        bridge,
        profile_store,
        controller_url,
        secret,
        readiness,
    )?))
}

struct UnavailableAndroidRuntimeGateway {
    failure: PortError,
}

impl UnavailableAndroidRuntimeGateway {
    fn failure(&self) -> PortError {
        self.failure.clone()
    }
}

#[async_trait::async_trait]
impl RuntimeGateway for UnavailableAndroidRuntimeGateway {
    async fn switch_proxy(&self, _group: &str, _proxy: &str) -> Result<(), PortError> {
        Err(self.failure())
    }

    async fn test_delay(
        &self,
        _proxy: &str,
        _url: &str,
        _timeout_ms: u32,
    ) -> Result<u32, PortError> {
        Err(self.failure())
    }

    async fn get_proxy_providers(&self) -> Result<Vec<ProxyProvider>, PortError> {
        Err(self.failure())
    }

    async fn get_rule_providers(&self) -> Result<Vec<RuleProvider>, PortError> {
        Err(self.failure())
    }

    async fn update_proxy_provider(&self, _name: &str) -> Result<(), PortError> {
        Err(self.failure())
    }

    async fn update_rule_provider(&self, _name: &str) -> Result<(), PortError> {
        Err(self.failure())
    }
}

impl<S: SecureStore + 'static> AndroidCoreComposition<S> {
    pub fn new(profile_store: Arc<ConfigManager<S>>) -> Self {
        Self { profile_store }
    }

    fn profile_application(
        &self,
        document_codec: Arc<dyn ProfileDocumentCodecPort>,
    ) -> ProfileApplication {
        let store: Arc<dyn ProfileStore> = self.profile_store.shared_engine_profile_store();
        ProfileApplication::new(store).with_document_codec(document_codec)
    }
}

/// Compose the Android host's local-only startup proof for either UI surface.
pub fn offline_startup_application<B>(bridge: B) -> OfflineStartupApplication
where
    B: AndroidBridge + 'static,
{
    OfflineStartupApplication::new(Arc::new(AndroidBridgeAdapter::new(bridge)))
}

/// Compose the Android native physical-link MTU observer. A bridge without
/// link metrics yields a typed unsupported result rather than a fake value.
pub fn mtu_application<B>(bridge: B) -> MtuApplication
where
    B: AndroidBridge + 'static,
{
    MtuApplication::new(Arc::new(AndroidBridgeAdapter::new(bridge)))
}

/// Assemble the shared application service with Android's bridge-backed Core
/// process port and the Mihomo controller readiness adapter.
pub fn core_application<B>(
    bridge: B,
    controller_url: impl Into<String>,
    secret: Option<String>,
    composition: AndroidCoreComposition<impl SecureStore + 'static>,
) -> Result<CoreApplication, String>
where
    B: AndroidBridge + 'static,
{
    let controller_url = controller_url.into();
    let readiness: Arc<dyn CoreReadiness> = Arc::new(ControllerReadiness::new(
        controller_url.clone(),
        secret.clone(),
    ));
    core_application_with_readiness(bridge, controller_url, secret, composition, readiness)
}

pub(crate) fn core_application_with_readiness<B, S>(
    bridge: B,
    controller_url: String,
    secret: Option<String>,
    composition: AndroidCoreComposition<S>,
    readiness: Arc<dyn CoreReadiness>,
) -> Result<CoreApplication, String>
where
    B: AndroidBridge + 'static,
    S: SecureStore + 'static,
{
    core_application_for_engine(
        MihomoVersionPort::engine_identifier(),
        bridge,
        controller_url,
        secret,
        composition,
        readiness,
        AndroidCapabilitySource::FromControllerAdapters,
    )
    .map_err(|error| error.to_string())
}

pub(crate) enum AndroidCapabilitySource {
    FromControllerAdapters,
    Provided(Arc<dyn CoreCapabilityProvider>),
}

pub(crate) fn core_application_with_readiness_and_provider<B, S>(
    bridge: B,
    controller_url: String,
    secret: Option<String>,
    composition: AndroidCoreComposition<S>,
    readiness: Arc<dyn CoreReadiness>,
    capability_provider: Arc<dyn CoreCapabilityProvider>,
) -> Result<CoreApplication, String>
where
    B: AndroidBridge + 'static,
    S: SecureStore + 'static,
{
    core_application_for_engine(
        MihomoVersionPort::engine_identifier(),
        bridge,
        controller_url,
        secret,
        composition,
        readiness,
        AndroidCapabilitySource::Provided(capability_provider),
    )
    .map_err(|error| error.to_string())
}

pub(crate) fn core_application_for_engine<B, S>(
    engine_id: EngineId,
    bridge: B,
    controller_url: String,
    secret: Option<String>,
    composition: AndroidCoreComposition<S>,
    readiness: Arc<dyn CoreReadiness>,
    capability_source: AndroidCapabilitySource,
) -> Result<CoreApplication, PortError>
where
    B: AndroidBridge + 'static,
    S: SecureStore + 'static,
{
    let provider = Arc::new(AndroidMihomoBackendProvider::new(
        engine_id.clone(),
        bridge,
        Arc::clone(&composition.profile_store),
        &controller_url,
        secret,
        readiness,
    )?);
    let bridge = Arc::clone(&provider.bridge);
    let provider_for_factory: Arc<dyn EngineBackendProvider> = provider.clone();
    let factory = EngineBackendFactoryRegistry::new([provider_for_factory])?;
    // The registry rejects a missing engine before it invokes the provider.
    // That keeps bridge directory lookup and controller client construction
    // behind the successful engine selection.
    let backend = factory.backend_ports(&engine_id)?;
    let controller_adapters = &provider.controller_adapters;
    core_application_from_backend(
        backend,
        bridge,
        composition,
        controller_adapters,
        capability_source,
    )
}

pub(crate) fn core_application_from_selected_mihomo_backend<S>(
    backend: EngineBackendPorts,
    bridge: Arc<dyn AndroidBridge>,
    profile_store: Arc<ConfigManager<S>>,
    controller_url: &str,
    secret: Option<String>,
    capability_provider: Arc<dyn CoreCapabilityProvider>,
) -> Result<CoreApplication, PortError>
where
    S: SecureStore + 'static,
{
    let controller_adapters = AndroidControllerAdapters::new(controller_url, secret);
    core_application_from_backend(
        backend,
        bridge,
        AndroidCoreComposition::new(profile_store),
        &controller_adapters,
        AndroidCapabilitySource::Provided(capability_provider),
    )
}

fn core_application_from_backend<S: SecureStore + 'static>(
    backend: EngineBackendPorts,
    bridge: Arc<dyn AndroidBridge>,
    composition: AndroidCoreComposition<S>,
    controller_adapters: &AndroidControllerAdapters,
    capability_source: AndroidCapabilitySource,
) -> Result<CoreApplication, PortError> {
    let overview_reader = controller_adapters.overview_reader();
    let capability_provider: Arc<dyn CoreCapabilityProvider> = match capability_source {
        AndroidCapabilitySource::Provided(provider) => provider,
        AndroidCapabilitySource::FromControllerAdapters => {
            controller_adapters.capability_provider()
        }
    };
    let runtime = infiltrator_composition::tokio_application_runtime()
        .map_err(|error| PortError::Failed(error.to_string()))?;
    let application = CoreApplication::new_with_overview(
        backend.process(),
        backend.readiness(),
        overview_reader,
        runtime.clone(),
    );
    match backend.bind_owner_scope_source(Arc::new(application.clone()))? {
        EngineBackendOwnerScopeBindingResult::NotRequired => {}
        EngineBackendOwnerScopeBindingResult::Bound => {}
    }
    let transaction_binding = backend.configuration_transaction_binding();
    let document_codec = backend.configuration();
    let transaction_scope = ProfileConfigurationTransactionScope::new(document_codec.scope());
    let transaction: Arc<dyn ProfileConfigurationTransactionPort> = match controller_adapters {
        AndroidControllerAdapters::Available { .. } => {
            let validation_client: Arc<dyn AndroidCoreConfigurationValidationClient> = Arc::new(
                AndroidBridgeConfigurationValidationClient::new(Arc::clone(&bridge)),
            );
            let working_directory = bridge.data_dir().ok_or_else(|| {
                PortError::unsupported(
                    Capability::Profiles,
                    "Android host did not provide an app-private Core working directory",
                )
            })?;
            let stopped_validator: Arc<dyn ProfileConfigurationCandidateValidatorPort> =
                Arc::new(AndroidProfileConfigurationCandidateValidator::new(
                    transaction_scope.clone(),
                    composition.profile_store.config_dir().to_path_buf(),
                    working_directory,
                    validation_client,
                ));
            let endpoints = Arc::new(ProfileEndpointSource::new(Arc::clone(
                &composition.profile_store,
            )));
            Arc::new(MihomoProfileConfigurationTransactionAdapter::new(
                document_codec,
                stopped_validator,
                Arc::new(application.clone()),
                Arc::clone(&composition.profile_store),
                Arc::new(EndpointConfigReloader::new(endpoints)),
                Arc::new(Mutex::new(())),
            )?)
        }
        AndroidControllerAdapters::Unavailable { failure } => {
            Arc::new(UnavailableProfileConfigurationTransaction::new(
                transaction_scope,
                format!("Mihomo controller configuration transaction is unavailable: {failure}"),
            ))
        }
    };
    transaction_binding.bind(transaction)?;
    controller_adapters.bind_configuration_scope(&application);
    let mtu = MtuApplication::new(Arc::new(AndroidBridgeAdapter::new(bridge.clone())));
    let core_capabilities = CoreCapabilityApplication::new(capability_provider);
    application
        .install_core_capability_application(core_capabilities.clone())
        .map_err(|failure| PortError::Failed(failure.message))?;
    let mut handler = CommandApplication::new()
        .with_core_capabilities(core_capabilities)
        .with_host_capabilities(Arc::new(AndroidBridgeAdapter::new(bridge.clone())))
        .with_application_runtime(runtime)
        .with_profile(composition.profile_application(backend.configuration()))
        .with_outbound_ui_preferences(application.outbound_ui_preferences_application())
        .with_mtu(mtu)
        .with_vpn(VpnServiceApplication::new(Arc::new(
            AndroidVpnServicePort::shared(),
        )));
    match controller_adapters {
        AndroidControllerAdapters::Available { client } => {
            let core_log_level_port: Arc<dyn CoreLogLevelPort> = Arc::new(client.clone());
            let operations = backend.operations();
            handler = handler
                .with_core_log_level_port(core_log_level_port)
                .with_routing_intent_port(operations.routing_intent()?);
            let outbound_port = operations.outbound()?;
            application
                .install_outbound_port(outbound_port.clone())
                .map_err(|failure| PortError::Failed(failure.message))?;
            handler = handler.with_outbound(OutboundApplication::new(outbound_port));
            application
                .install_log_gateway(controller_adapters.runtime_gateway())
                .map_err(|failure| PortError::Failed(failure.message))?;
            handler = handler
                .with_runtime(controller_adapters.runtime_gateway())
                .with_logs(application.log_application());
        }
        AndroidControllerAdapters::Unavailable { .. } => {}
    }
    // BANDROID-008: the shared import command reads the real system clipboard
    // through the registered Activity channel. The port resolves lazily, so a
    // composition built before the Activity registers still reaches it; until
    // then the command keeps its typed unsupported result.
    handler = handler.with_import_source(Arc::new(AndroidClipboardPort::from_registry()));
    application.install_command_handler(Arc::new(handler));
    Ok(application)
}

#[cfg(test)]
#[path = "composition_tests.rs"]
mod tests;

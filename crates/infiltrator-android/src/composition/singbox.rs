use super::*;
use crate::core_launch_spec::AndroidSingBoxProfileLaunchSpecProvider;
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_application::version_application::VersionApplication;
use infiltrator_contract::core_capabilities::EngineBinaryTarget;
use infiltrator_contract::profile_document_identity::ProfileDocument;
use infiltrator_ports::core_binary_selection::CoreBinarySelectionPort;
use infiltrator_ports::profile_configuration_transaction::ProfileConfigurationTransactionBinding;
use singbox_api::controller_configuration::SingBoxControllerConfiguration;
use singbox_config::profile_document_codec::SingBoxProfileDocumentCodec;
use singbox_process::command::TokioSingBoxCommandPort;
use std::path::PathBuf;

/// Assemble the Android :vpn lifecycle owner for one already selected,
/// revision-bound sing-box document and managed binary selector.
pub(crate) fn core_application_for_singbox<B, S>(
    bridge: B,
    profile_store: Arc<ConfigManager<S>>,
    profile_document: ProfileDocument,
    binary_selection: Arc<dyn CoreBinarySelectionPort>,
    working_directory: PathBuf,
    expected_target: EngineBinaryTarget,
    readiness: Arc<dyn CoreReadiness>,
    capability_provider: Arc<dyn CoreCapabilityProvider>,
    versions: VersionApplication,
    settings: SettingsApplication,
) -> Result<CoreApplication, PortError>
where
    B: AndroidBridge + 'static,
    S: SecureStore + 'static,
{
    let engine_id = singbox_api::engine_id();
    if profile_store.engine_id() != engine_id
        || profile_document.identity.engine_id != engine_id
        || binary_selection.engine_id() != engine_id
        || versions.engine_id() != &engine_id
    {
        return Err(PortError::unsupported(
            Capability::CoreVersionInstall,
            "Android sing-box composition received a foreign engine binding",
        ));
    }
    SingBoxControllerConfiguration::from_profile_json(&profile_document.raw_bytes).map_err(
        |error| {
            PortError::Rejected(infiltrator_contract::error::Failure::new(
                infiltrator_contract::error::ErrorCode::Configuration,
                format!("Android sing-box controller configuration is unavailable: {error}"),
                false,
            ))
        },
    )?;
    let bridge: Arc<dyn AndroidBridge> = Arc::new(bridge);
    let launch_spec_source = Arc::new(AndroidSingBoxProfileLaunchSpecProvider::new(
        Arc::clone(&profile_store),
        profile_document,
        binary_selection,
        working_directory,
        expected_target,
        Arc::new(TokioSingBoxCommandPort),
    ));
    let process: Arc<dyn CoreProcess> = Arc::new(AndroidCoreProcessAdapter::new(
        engine_id.clone(),
        Arc::clone(&bridge),
        launch_spec_source,
    ));
    let codec: Arc<dyn ProfileDocumentCodecPort> = Arc::new(SingBoxProfileDocumentCodec);
    let transaction_binding = Arc::new(ProfileConfigurationTransactionBinding::new(
        ProfileConfigurationTransactionScope::new(codec.scope()),
    ));
    let provider: Arc<dyn EngineBackendProvider> = Arc::new(AndroidSingBoxBackendProvider {
        engine_id: engine_id.clone(),
        process,
        readiness,
        codec: Arc::clone(&codec),
        transaction_binding: Arc::clone(&transaction_binding),
    });
    let factory = EngineBackendFactoryRegistry::new([provider])?;
    let backend = factory.backend_ports(&engine_id)?;
    let runtime = infiltrator_composition::tokio_application_runtime()
        .map_err(|error| PortError::Failed(error.to_string()))?;
    let overview = Arc::new(UnavailableOverviewReader::new(PortError::Failed(
        "Android sing-box Overview adapter is not connected".to_owned(),
    )));
    let application = CoreApplication::new_with_overview(
        backend.process(),
        backend.readiness(),
        overview,
        runtime.clone(),
    );
    match backend.bind_owner_scope_source(Arc::new(application.clone()))? {
        EngineBackendOwnerScopeBindingResult::NotRequired
        | EngineBackendOwnerScopeBindingResult::Bound => {}
    }
    backend.configuration_transaction_binding().bind(Arc::new(
        UnavailableProfileConfigurationTransaction::new(
            ProfileConfigurationTransactionScope::new(codec.scope()),
            "Android sing-box native profile transactions are not connected",
        ),
    ))?;
    let mtu = MtuApplication::new(Arc::new(AndroidBridgeAdapter::new(Arc::clone(&bridge))));
    let core_capabilities = CoreCapabilityApplication::new(capability_provider);
    application
        .install_core_capability_application(core_capabilities.clone())
        .map_err(|failure| PortError::Failed(failure.message))?;
    let store: Arc<dyn ProfileStore> = profile_store;
    let handler = CommandApplication::new()
        .with_core_capabilities(core_capabilities)
        .with_host_capabilities(Arc::new(AndroidBridgeAdapter::new(Arc::clone(&bridge))))
        .with_application_runtime(runtime)
        .with_profile(ProfileApplication::new(store).with_document_codec(codec))
        .with_versions(versions)
        .with_settings(settings)
        .with_mtu(mtu)
        .with_vpn(VpnServiceApplication::new(Arc::new(
            AndroidVpnServicePort::shared(),
        )))
        .with_import_source(Arc::new(AndroidClipboardPort::from_registry()));
    application.install_command_handler(Arc::new(handler));
    Ok(application)
}

struct AndroidSingBoxBackendProvider {
    engine_id: EngineId,
    process: Arc<dyn CoreProcess>,
    readiness: Arc<dyn CoreReadiness>,
    codec: Arc<dyn ProfileDocumentCodecPort>,
    transaction_binding: Arc<ProfileConfigurationTransactionBinding>,
}

impl EngineBackendProvider for AndroidSingBoxBackendProvider {
    fn engine_id(&self) -> EngineId {
        self.engine_id.clone()
    }

    fn create_backend(&self) -> Result<EngineBackendPorts, PortError> {
        EngineBackendPorts::new(
            self.engine_id.clone(),
            Arc::clone(&self.process),
            Arc::clone(&self.readiness),
            Arc::clone(&self.codec),
            Arc::clone(&self.transaction_binding),
            infiltrator_ports::engine_backend_factory::EngineOperationPorts::unsupported(
                "Android sing-box runtime operations are not connected",
            ),
        )
    }
}

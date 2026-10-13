use super::*;
use crate::core_launch_spec::AndroidSingBoxProfileLaunchSpecProvider;
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_application::version_application::VersionApplication;
use infiltrator_contract::android_control::AndroidCoreBinaryTarget;
use infiltrator_contract::core_capabilities::EngineBinaryTarget;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document_identity::ProfileDocument;
use infiltrator_contract::version::CoreArtifactVerification;
use infiltrator_ports::core_binary_selection::{
    CoreBinarySelection, CoreBinarySelectionPort, CoreBinarySelectionSource,
};
use infiltrator_ports::engine_backend_factory::EngineOperationPorts;
use infiltrator_ports::profile_configuration_transaction::ProfileConfigurationTransactionBinding;
use singbox_api::controller_configuration::SingBoxControllerConfiguration;
use singbox_config::config::SingBoxProfileConfigurationAdapter;
use singbox_config::profile_configuration_transaction::SingBoxProfileConfigurationTransactionAdapter;
use singbox_config::profile_document_codec::SingBoxProfileDocumentCodec;
use singbox_process::command::SingBoxCommandPort;
use singbox_process::process::SingBoxCoreOperationGate;
use std::path::PathBuf;

/// Assemble the Android :vpn lifecycle owner for one already selected,
/// revision-bound sing-box document and managed binary selector.
pub(crate) async fn core_application_for_singbox<B, S>(
    bridge: B,
    profile_store: Arc<ConfigManager<S>>,
    profile_document: ProfileDocument,
    binary_selection: Arc<dyn CoreBinarySelectionPort>,
    selected_binary: CoreBinarySelection,
    working_directory: PathBuf,
    expected_target: EngineBinaryTarget,
    command: Arc<dyn SingBoxCommandPort>,
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
    if versions.engine_id() != &engine_id {
        return Err(PortError::unsupported(
            Capability::CoreVersionInstall,
            "Android sing-box composition received a foreign engine binding",
        ));
    }
    let bridge: Arc<dyn AndroidBridge> = Arc::new(bridge);
    let provider = android_singbox_backend_provider(
        Arc::clone(&bridge),
        Arc::clone(&profile_store),
        profile_document,
        binary_selection,
        selected_binary,
        working_directory,
        expected_target,
        command,
        readiness,
    )
    .await?;
    let factory = EngineBackendFactoryRegistry::new([Arc::clone(&provider.provider)])?;
    let backend = factory.backend_ports(&engine_id)?;
    core_application_from_selected_singbox_backend(
        backend,
        bridge,
        profile_store,
        provider.configuration,
        provider.operation_gate,
        capability_provider,
        versions,
        settings,
    )
}

pub(crate) struct AndroidSingBoxBackendProviderParts {
    pub(crate) provider: Arc<dyn EngineBackendProvider>,
    pub(crate) configuration: Arc<SingBoxProfileConfigurationAdapter>,
    pub(crate) operation_gate: Arc<SingBoxCoreOperationGate>,
}

pub(crate) async fn android_singbox_backend_provider<S>(
    bridge: Arc<dyn AndroidBridge>,
    profile_store: Arc<ConfigManager<S>>,
    profile_document: ProfileDocument,
    binary_selection: Arc<dyn CoreBinarySelectionPort>,
    selected_binary: CoreBinarySelection,
    working_directory: PathBuf,
    expected_target: EngineBinaryTarget,
    command: Arc<dyn SingBoxCommandPort>,
    readiness: Arc<dyn CoreReadiness>,
) -> Result<AndroidSingBoxBackendProviderParts, PortError>
where
    S: SecureStore + 'static,
{
    let engine_id = singbox_api::engine_id();
    if profile_store.engine_id() != engine_id
        || profile_document.identity.engine_id != engine_id
        || binary_selection.engine_id() != engine_id
        || selected_binary.engine_id != engine_id
    {
        return Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "Android sing-box backend provider received a foreign engine binding",
        ));
    }
    SingBoxControllerConfiguration::from_profile_json(&profile_document.raw_bytes).map_err(
        |error| {
            PortError::Rejected(Failure::new(
                ErrorCode::Configuration,
                format!("Android sing-box controller configuration is unavailable: {error}"),
                false,
            ))
        },
    )?;
    let expected_target = expected_target;
    selected_binary
        .validate_for(&engine_id, expected_target)
        .map_err(|error| {
            PortError::unsupported(
                Capability::CoreVersionInstall,
                format!("Android sing-box selected binary is invalid: {error}"),
            )
        })?;
    let verified = matches!(
        &selected_binary.verification,
        CoreArtifactVerification::Verified { engine_id: verified_engine, version }
            if verified_engine == &engine_id && version == selected_binary.version.raw()
    );
    let android_target = expected_target
        .target_triple()
        .and_then(AndroidCoreBinaryTarget::from_target_triple)
        .is_some();
    let expected_binary_path = expected_target.target_triple().map(|target| {
        working_directory
            .join("engines/sing-box/targets")
            .join(target)
            .join("versions")
            .join(selected_binary.version.raw())
            .join("sing-box")
    });
    if !verified
        || !android_target
        || selected_binary.source != CoreBinarySelectionSource::ManagedVersion
        || expected_binary_path.as_ref() != Some(&selected_binary.binary_path)
    {
        return Err(PortError::unsupported(
            Capability::CoreVersionInstall,
            "Android sing-box requires verified managed binary evidence",
        ));
    }
    let configuration_path = profile_store
        .existing_profile_document_path(profile_document.identity.profile_id.as_str())
        .await
        .map_err(|error| PortError::Io(error.to_string()))?;
    let configuration = Arc::new(SingBoxProfileConfigurationAdapter::new(
        selected_binary.binary_path,
        configuration_path,
        Arc::clone(&command),
    ));
    let operation_gate = Arc::new(SingBoxCoreOperationGate::new());
    let launch_spec_source = Arc::new(AndroidSingBoxProfileLaunchSpecProvider::new(
        Arc::clone(&profile_store),
        profile_document,
        binary_selection,
        working_directory,
        expected_target,
        command,
    ));
    let process: Arc<dyn CoreProcess> = Arc::new(AndroidCoreProcessAdapter::new_singbox(
        engine_id.clone(),
        Arc::clone(&bridge),
        launch_spec_source,
        Arc::clone(&operation_gate),
    ));
    let codec: Arc<dyn ProfileDocumentCodecPort> = Arc::new(SingBoxProfileDocumentCodec);
    let transaction_binding = Arc::new(ProfileConfigurationTransactionBinding::new(
        ProfileConfigurationTransactionScope::new(codec.scope()),
    ));
    Ok(AndroidSingBoxBackendProviderParts {
        provider: Arc::new(AndroidSingBoxBackendProvider {
            engine_id: engine_id.clone(),
            process,
            readiness,
            codec: Arc::clone(&codec),
            transaction_binding: Arc::clone(&transaction_binding),
        }),
        configuration,
        operation_gate,
    })
}

pub(crate) fn core_application_from_selected_singbox_backend<S>(
    backend: EngineBackendPorts,
    bridge: Arc<dyn AndroidBridge>,
    profile_store: Arc<ConfigManager<S>>,
    configuration: Arc<SingBoxProfileConfigurationAdapter>,
    operation_gate: Arc<SingBoxCoreOperationGate>,
    capability_provider: Arc<dyn CoreCapabilityProvider>,
    versions: VersionApplication,
    settings: SettingsApplication,
) -> Result<CoreApplication, PortError>
where
    S: SecureStore + 'static,
{
    let engine_id = singbox_api::engine_id();
    if backend.engine_id() != &engine_id
        || profile_store.engine_id() != engine_id
        || versions.engine_id() != &engine_id
    {
        return Err(PortError::unsupported(
            Capability::CoreVersionInstall,
            "Android sing-box CoreApplication received a foreign engine binding",
        ));
    }
    let codec = backend.configuration();
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
    match backend.bind_owner_core_scope(Arc::new(application.clone()))? {
        EngineBackendOwnerScopeBindingResult::NotRequired
        | EngineBackendOwnerScopeBindingResult::Bound => {}
    }
    let process_identity = backend.process().core_identity().ok_or_else(|| {
        PortError::Failed("Android sing-box process identity is unavailable".to_owned())
    })?;
    let profile_store_port: Arc<dyn ProfileStore> = profile_store.shared_engine_profile_store();
    let transaction = Arc::new(SingBoxProfileConfigurationTransactionAdapter::new(
        profile_store_port,
        Arc::new(application.clone()),
        configuration,
        operation_gate,
        process_identity,
    )?);
    backend
        .configuration_transaction_binding()
        .bind(transaction)?;
    let mtu = MtuApplication::new(Arc::new(AndroidBridgeAdapter::new(Arc::clone(&bridge))));
    let core_capabilities = CoreCapabilityApplication::new(capability_provider);
    application
        .install_core_capability_application(core_capabilities.clone())
        .map_err(|failure| PortError::Failed(failure.message))?;
    let store: Arc<dyn ProfileStore> = profile_store.shared_engine_profile_store();
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
            EngineOperationPorts::unsupported(
                "Android sing-box runtime operations are not connected",
            ),
        )
    }
}

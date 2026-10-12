use super::{SharedCore, SharedCoreEngine, authorize_android_core_start};
use crate::composition::singbox::core_application_for_singbox;
use crate::core_version_owner::{core_binary_selection_port, core_version_application};
use crate::ffi::{FfiErrorCode, FfiStatus};
use crate::host_support::{build_settings_application, map_mihomo_error};
use infiltrator_application::android_control_application::AndroidControlApplication;
use infiltrator_contract::active_profile_reference::ActiveProfileReference;
use infiltrator_contract::core_capabilities::{
    CoreLifecycleOperation, CoreOperation, EngineBinaryTarget, VersionOperation,
};
use infiltrator_contract::core_identity::CoreGeneration;
use infiltrator_contract::surface::HostKind;
use infiltrator_contract::version::CoreArtifactVerification;
use infiltrator_platform::defaults::DefaultCredentialStore;
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceLease;
use infiltrator_ports::core_binary_selection::{
    CoreBinarySelection, CoreBinarySelectionPolicy, CoreBinarySelectionRequest,
    CoreBinarySelectionSource,
};
use infiltrator_ports::core_capability_provider::CoreCapabilityProvider;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_profile_store::store::ProfileDocumentExtension;
use mihomo_config::manager::ConfigManager;
use singbox_api::capability_provider::SingBoxCoreCapabilityProvider;
use singbox_api::controller_configuration::SingBoxControllerConfiguration;
use singbox_api::readiness::SingBoxControllerReadiness;
use singbox_config::profile_document_codec::SingBoxProfileDocumentCodec;
use singbox_process::command::TokioSingBoxCommandPort;
use std::path::PathBuf;
use std::sync::Arc;

pub(super) async fn build_shared_core(
    bridge: Arc<dyn infiltrator_platform::android_bridge::AndroidBridge>,
    reference_store: Arc<ConfigManager<DefaultCredentialStore>>,
    reference: ActiveProfileReference,
    selection_lease: Box<dyn ActiveProfileReferenceLease>,
) -> Result<Arc<SharedCore>, FfiStatus> {
    let engine_id = singbox_api::engine_id();
    let config = profile_store_at_root(reference_store.config_root().to_path_buf()).await?;
    if reference.engine_id != engine_id || config.engine_id() != engine_id {
        return Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android sing-box owner received a foreign active profile reference",
        ));
    }
    let workspace =
        ProfileStore::load_workspace(config.engine_profile_store(), &reference.profile_id)
            .await
            .map_err(|error| {
                FfiStatus::err(
                    FfiErrorCode::Config,
                    format!("Android sing-box profile document is unavailable: {error}"),
                )
            })?;
    if workspace.document.identity.engine_id != reference.engine_id
        || workspace.document.identity.profile_id != reference.profile_id
    {
        return Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android sing-box active profile reference does not match its persisted document",
        ));
    }
    let working_directory = bridge.data_dir().ok_or_else(|| {
        FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android host did not provide an app-private Core working directory",
        )
    })?;
    let versions = core_version_application(engine_id.clone())?;
    let settings = build_settings_application().await?;
    let binary_selection = core_binary_selection_port(&engine_id)?;
    let selected_binary = binary_selection
        .select(CoreBinarySelectionRequest {
            engine_id: engine_id.clone(),
            policy: CoreBinarySelectionPolicy::ManagedOnly,
        })
        .await
        .map_err(|error| {
            FfiStatus::err(
                FfiErrorCode::NotReady,
                format!("Android sing-box managed binary is unavailable: {error}"),
            )
        })?;
    validate_binary_selection(
        &selected_binary,
        &engine_id,
        EngineBinaryTarget::current(),
        &working_directory,
    )?;

    let capability_provider: Arc<dyn CoreCapabilityProvider> =
        Arc::new(SingBoxCoreCapabilityProvider::new(
            HostKind::Android,
            [
                CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
                CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
                CoreOperation::Lifecycle(CoreLifecycleOperation::Restart),
                CoreOperation::Version(VersionOperation::Inspect),
                CoreOperation::Version(VersionOperation::Rollback),
            ],
        ));
    authorize_android_core_start(Arc::clone(&capability_provider), &engine_id)?;
    let controller = SingBoxControllerConfiguration::from_profile_json(
        &workspace.document.raw_bytes,
    )
    .map_err(|error| {
        FfiStatus::err(
            FfiErrorCode::Config,
            format!("Android sing-box controller endpoint is invalid: {error}"),
        )
    })?;
    let readiness: Arc<dyn infiltrator_ports::core_process::CoreReadiness> = Arc::new(
        SingBoxControllerReadiness::with_reqwest(controller).map_err(|error| {
            FfiStatus::err(
                FfiErrorCode::Config,
                format!("Android sing-box readiness adapter is unavailable: {error}"),
            )
        })?,
    );
    let application = core_application_for_singbox(
        Arc::clone(&bridge),
        Arc::clone(&config),
        workspace.document,
        binary_selection,
        selected_binary,
        working_directory,
        EngineBinaryTarget::current(),
        Arc::new(TokioSingBoxCommandPort),
        readiness,
        capability_provider,
        versions,
        settings,
    )
    .await
    .map_err(|error| {
        FfiStatus::err(
            FfiErrorCode::NotReady,
            format!("Android sing-box Core composition failed: {error}"),
        )
    })?;
    let owner_identity = application.snapshot().identity.ok_or_else(|| {
        FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android sing-box Core backend has no typed owner identity",
        )
    })?;
    if owner_identity.engine != reference.engine_id {
        return Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android sing-box Core backend identity does not match the active profile engine",
        ));
    }
    let owner_binding = selection_lease
        .into_owner_binding(
            owner_identity,
            CoreGeneration::new(application.snapshot().generation),
        )
        .await
        .map_err(|error| {
            FfiStatus::err(
                FfiErrorCode::NotReady,
                format!("Android sing-box Core owner could not bind the active profile: {error}"),
            )
        })?;
    application
        .install_active_profile_owner_binding(owner_binding)
        .map_err(|failure| {
            FfiStatus::from(infiltrator_contract::error::InfiltratorError::Backend(
                failure.message,
            ))
        })?;
    let control = AndroidControlApplication::new(application.clone());
    let watchdog = infiltrator_composition::spawn_core_watchdog(Arc::new(application.clone()));
    Ok(Arc::new(SharedCore {
        application,
        control,
        engine: SharedCoreEngine::SingBox,
        config,
        _watchdog: watchdog,
    }))
}

pub(super) async fn profile_store_at_root(
    config_root: PathBuf,
) -> Result<Arc<ConfigManager<DefaultCredentialStore>>, FfiStatus> {
    let config = Arc::new(
        ConfigManager::with_profile_codec_and_store_at_root(
            config_root,
            DefaultCredentialStore::default(),
            Arc::new(SingBoxProfileDocumentCodec),
            ProfileDocumentExtension::Json,
        )
        .map_err(map_mihomo_error)?,
    );
    config
        .initialize_profile_namespace()
        .await
        .map_err(map_mihomo_error)?;
    Ok(config)
}

pub(super) fn validate_binary_selection(
    selection: &CoreBinarySelection,
    engine_id: &infiltrator_contract::core_identity::EngineId,
    expected_target: EngineBinaryTarget,
    working_directory: &std::path::Path,
) -> Result<(), FfiStatus> {
    selection
        .validate_for(engine_id, expected_target)
        .map_err(|error| {
            FfiStatus::err(
                FfiErrorCode::NotReady,
                format!("Android sing-box binary target is unavailable: {error}"),
            )
        })?;
    let verified = matches!(
        &selection.verification,
        CoreArtifactVerification::Verified { engine_id: verified_engine, version }
            if verified_engine == engine_id && version == selection.version.raw()
    );
    let android_target = expected_target.target_triple().is_some_and(|target| {
        infiltrator_contract::android_control::AndroidCoreBinaryTarget::from_target_triple(target)
            .is_some()
    });
    let expected_path = expected_target.target_triple().map(|target| {
        working_directory
            .join("engines/sing-box/targets")
            .join(target)
            .join("versions")
            .join(selection.version.raw())
            .join("sing-box")
    });
    if !android_target
        || selection.source != CoreBinarySelectionSource::ManagedVersion
        || !verified
        || expected_path.as_ref() != Some(&selection.binary_path)
    {
        return Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android sing-box requires an exactly verified managed binary for this ABI",
        ));
    }
    Ok(())
}

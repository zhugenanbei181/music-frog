use super::owner_state::{
    SharedCoreOwnerAdmission, SharedCoreOwnerSwitchCompletion, SharedCoreOwnerSwitchFinish,
    begin_shared_core_switch, finish_shared_core_switch,
};
use super::singbox::{profile_store_at_root, validate_binary_selection};
use super::{
    SharedCore, SharedCoreEngine, authorize_android_core_start, shared_core, shared_core_owner_slot,
};
use crate::composition::singbox::{
    AndroidSingBoxBackendProviderParts, android_singbox_backend_provider,
    core_application_from_selected_singbox_backend,
};
use crate::composition::{
    android_core_capability_provider, android_mihomo_backend_provider,
    core_application_from_selected_mihomo_backend,
};
use crate::core_launch_spec::AndroidProfileReadiness;
use crate::core_launch_spec::singbox::AndroidSingBoxProfileLaunchSpecProvider;
use crate::core_version_owner::{core_binary_selection_port, core_version_application};
use crate::ffi::{FfiErrorCode, FfiStatus};
use crate::host_support::{build_settings_application, map_mihomo_error, map_port_error};
use crate::runtime::CoreLaunchSpecSource;
use infiltrator_application::android_control_application::AndroidControlApplication;
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_application::version_application::VersionApplication;
use infiltrator_composition::active_profile_backend::{
    ActiveProfileBackendSelector, ActiveProfileBackendSwitchFailurePhase, PreviousCoreOwner,
};
use infiltrator_composition::engine_backend_factory::EngineBackendFactoryRegistry;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceState, ActiveProfileReferenceTarget,
};
use infiltrator_contract::core_capabilities::{
    CoreLifecycleOperation, CoreOperation, EngineBinaryTarget, VersionOperation,
};
use infiltrator_contract::profile_document_identity::{ProfileDocument, ProfileId};
use infiltrator_contract::surface::HostKind;
use infiltrator_core::version_port::MihomoVersionPort;
use infiltrator_platform::android_bridge::{AndroidBridge, get_android_bridge};
use infiltrator_platform::defaults::DefaultCredentialStore;
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::core_binary_selection::{
    CoreBinarySelectionPolicy, CoreBinarySelectionRequest,
};
use infiltrator_ports::core_capability_provider::CoreCapabilityProvider;
use infiltrator_ports::core_lifecycle::CoreOwnerTerminationPort;
use infiltrator_ports::core_process::CoreReadiness;
use infiltrator_ports::endpoint::EndpointSource;
use infiltrator_ports::engine_backend_factory::{EngineBackendFactory, EngineBackendProvider};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_profile_store::store::ProfileDocumentExtension;
use mihomo_config::endpoint::ProfileEndpointSource;
use mihomo_config::manager::ConfigManager;
use mihomo_config::profile_document_codec::MihomoProfileDocumentCodec;
use singbox_api::capability_provider::SingBoxCoreCapabilityProvider;
use singbox_api::controller_configuration::SingBoxControllerConfiguration;
use singbox_api::readiness::SingBoxControllerReadiness;
use singbox_config::config::SingBoxProfileConfigurationAdapter;
use singbox_process::command::TokioSingBoxCommandPort;
use singbox_process::process::SingBoxCoreOperationGate;
use std::path::PathBuf;
use std::sync::Arc;

enum PreparedTargetRuntime {
    Mihomo {
        endpoints: Arc<ProfileEndpointSource<DefaultCredentialStore>>,
        controller_url: String,
        controller_secret: Option<String>,
    },
    SingBox {
        configuration: Arc<SingBoxProfileConfigurationAdapter>,
        operation_gate: Arc<SingBoxCoreOperationGate>,
        versions: VersionApplication,
        settings: SettingsApplication,
    },
}

struct PreparedTarget {
    config: Arc<ConfigManager<DefaultCredentialStore>>,
    document: ProfileDocument,
    provider: Arc<dyn EngineBackendProvider>,
    capabilities: Arc<dyn CoreCapabilityProvider>,
    runtime: PreparedTargetRuntime,
}

enum AndroidProfileSelectionAction {
    ActivateMihomo(ProfileId),
    ReplaceOwner(ActiveProfileReferenceTarget),
    AlreadySelected,
}

pub(crate) async fn select_profile_target_status(
    target: ActiveProfileReferenceTarget,
) -> FfiStatus {
    let core = match shared_core().await {
        Ok(core) => core,
        Err(status) => return status,
    };
    let owner_engine = match &core.engine {
        SharedCoreEngine::Mihomo { .. } => MihomoVersionPort::engine_identifier(),
        SharedCoreEngine::SingBox => singbox_api::engine_id(),
    };
    let current_reference =
        match ProfileStore::read_active_profile_reference(core.config.engine_profile_store()).await
        {
            Ok(ActiveProfileReferenceState::Selected(reference)) => reference,
            Ok(ActiveProfileReferenceState::Unselected { .. }) => {
                return FfiStatus::err(
                    FfiErrorCode::NotReady,
                    "Android Core owner has no selected profile reference",
                );
            }
            Err(error) => return map_port_error(error),
        };
    if current_reference.engine_id != owner_engine {
        return FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android Core owner engine does not match its active profile reference",
        );
    }
    match profile_selection_action(current_reference, target) {
        Ok(AndroidProfileSelectionAction::ActivateMihomo(profile_id)) => {
            super::activate_profile_status(profile_id.as_str()).await
        }
        Ok(AndroidProfileSelectionAction::ReplaceOwner(target)) => {
            replace_shared_core_owner_for_profile(target).await
        }
        Ok(AndroidProfileSelectionAction::AlreadySelected) => FfiStatus::ok(),
        Err(status) => status,
    }
}

fn profile_selection_action(
    current_reference: ActiveProfileReference,
    target: ActiveProfileReferenceTarget,
) -> Result<AndroidProfileSelectionAction, FfiStatus> {
    match target {
        ActiveProfileReferenceTarget::Unselected => Err(FfiStatus::err(
            FfiErrorCode::Config,
            "Android profile selection requires an engine-scoped target",
        )),
        ActiveProfileReferenceTarget::Selected {
            engine_id,
            profile_id,
        } if engine_id == current_reference.engine_id
            && profile_id == current_reference.profile_id =>
        {
            Ok(AndroidProfileSelectionAction::AlreadySelected)
        }
        ActiveProfileReferenceTarget::Selected {
            engine_id,
            profile_id,
        } if engine_id == current_reference.engine_id
            && engine_id == MihomoVersionPort::engine_identifier() =>
        {
            Ok(AndroidProfileSelectionAction::ActivateMihomo(profile_id))
        }
        target @ ActiveProfileReferenceTarget::Selected { .. } => {
            Ok(AndroidProfileSelectionAction::ReplaceOwner(target))
        }
    }
}

async fn replace_shared_core_owner_for_profile(target: ActiveProfileReferenceTarget) -> FfiStatus {
    let previous = {
        let mut state = shared_core_owner_slot().lock().await;
        match begin_shared_core_switch(&mut state) {
            Ok(previous) => previous,
            Err(admission) => return switch_admission_failure(admission),
        }
    };
    let bridge = match get_android_bridge() {
        Some(bridge) => bridge,
        None => {
            return restore_previous(
                &previous,
                FfiStatus::err(FfiErrorCode::NotReady, "Android host bridge is unavailable"),
            )
            .await;
        }
    };
    let prepared = match prepare_target(&previous, Arc::clone(&bridge), &target).await {
        Ok(prepared) => prepared,
        Err(status) => return restore_previous(&previous, status).await,
    };
    let target_profile_store: Arc<dyn ProfileStore> = prepared.config.shared_engine_profile_store();
    let reference_store: Arc<dyn ActiveProfileReferenceStore> =
        previous.config.shared_engine_profile_store();
    let factory = match EngineBackendFactoryRegistry::new([Arc::clone(&prepared.provider)]) {
        Ok(factory) => Arc::new(factory) as Arc<dyn EngineBackendFactory>,
        Err(error) => {
            return restore_previous(&previous, map_port_error(error)).await;
        }
    };
    let selector = ActiveProfileBackendSelector::new(
        reference_store,
        [target_profile_store],
        factory,
        Arc::clone(&prepared.capabilities),
    );
    let previous_owner: Arc<dyn CoreOwnerTerminationPort> = Arc::new(previous.application.clone());
    let selected = match selector
        .replace_owner_for_profile(
            target.clone(),
            &prepared.document,
            &PreviousCoreOwner::Owned(previous_owner),
        )
        .await
    {
        Ok(selected) => selected,
        Err(error) => {
            let phase = error.phase();
            let status = map_port_error(error.into_failure());
            return match phase {
                ActiveProfileBackendSwitchFailurePhase::BeforeReservation => {
                    restore_previous(&previous, status).await
                }
                ActiveProfileBackendSwitchFailurePhase::ReservationPending
                | ActiveProfileBackendSwitchFailurePhase::OwnerTerminationUnconfirmed
                | ActiveProfileBackendSwitchFailurePhase::OldOwnerClosed => {
                    require_owner_recovery(&previous, status).await
                }
            };
        }
    };
    let backend = selected.backend.clone();
    let capabilities = selected.capability_provider();
    let application_result = match &prepared.runtime {
        PreparedTargetRuntime::Mihomo {
            controller_url,
            controller_secret,
            ..
        } => core_application_from_selected_mihomo_backend(
            backend,
            Arc::clone(&bridge),
            Arc::clone(&prepared.config),
            controller_url,
            controller_secret.clone(),
            capabilities,
        ),
        PreparedTargetRuntime::SingBox {
            configuration,
            operation_gate,
            versions,
            settings,
        } => core_application_from_selected_singbox_backend(
            backend,
            Arc::clone(&bridge),
            Arc::clone(&prepared.config),
            Arc::clone(configuration),
            Arc::clone(operation_gate),
            capabilities,
            versions.clone(),
            settings.clone(),
        ),
    };
    let application = match application_result {
        Ok(application) => application,
        Err(error) => {
            let abort = selected.abort_before_core_application().await;
            let status = match abort {
                Ok(_) => map_port_error(error),
                Err(abort_error) => map_port_error(abort_error),
            };
            return require_owner_recovery(&previous, status).await;
        }
    };
    if let Err(error) = selected
        .install_owner_binding_for_application(&application)
        .await
    {
        return require_owner_recovery(&previous, map_port_error(error)).await;
    }
    let candidate = match &prepared.runtime {
        PreparedTargetRuntime::Mihomo { endpoints, .. } => Arc::new(SharedCore {
            control: AndroidControlApplication::new(application.clone()),
            application: application.clone(),
            engine: SharedCoreEngine::Mihomo {
                endpoints: Arc::clone(endpoints) as Arc<dyn EndpointSource>,
            },
            config: Arc::clone(&prepared.config),
            _watchdog: infiltrator_composition::spawn_core_watchdog(Arc::new(application.clone())),
        }),
        PreparedTargetRuntime::SingBox { .. } => Arc::new(SharedCore {
            control: AndroidControlApplication::new(application.clone()),
            application: application.clone(),
            engine: SharedCoreEngine::SingBox,
            config: Arc::clone(&prepared.config),
            _watchdog: infiltrator_composition::spawn_core_watchdog(Arc::new(application.clone())),
        }),
    };
    let completion = {
        let mut state = shared_core_owner_slot().lock().await;
        finish_shared_core_switch(
            &mut state,
            &previous,
            SharedCoreOwnerSwitchCompletion::Installed(Arc::clone(&candidate)),
        )
    };
    match completion {
        SharedCoreOwnerSwitchFinish::Installed => FfiStatus::ok(),
        SharedCoreOwnerSwitchFinish::OwnerStateChanged => {
            let _ = application.close().await;
            FfiStatus::err(
                FfiErrorCode::NotReady,
                "Android Core owner changed during profile replacement; rebind is required",
            )
        }
        SharedCoreOwnerSwitchFinish::Restored | SharedCoreOwnerSwitchFinish::RecoveryRequired => {
            FfiStatus::err(
                FfiErrorCode::NotReady,
                "Android profile owner replacement did not install the selected candidate",
            )
        }
    }
}

async fn prepare_target(
    previous: &SharedCore,
    bridge: Arc<dyn AndroidBridge>,
    target: &ActiveProfileReferenceTarget,
) -> Result<PreparedTarget, FfiStatus> {
    let (engine_id, profile_id) = match target {
        ActiveProfileReferenceTarget::Selected {
            engine_id,
            profile_id,
        } => (engine_id.clone(), profile_id.clone()),
        ActiveProfileReferenceTarget::Unselected => {
            return Err(FfiStatus::err(
                FfiErrorCode::Config,
                "Android profile owner replacement requires a selected target",
            ));
        }
    };
    let config_root = previous.config.config_root().to_path_buf();
    if engine_id == MihomoVersionPort::engine_identifier() {
        return prepare_mihomo_target(config_root, bridge, profile_id).await;
    }
    if engine_id == singbox_api::engine_id() {
        return prepare_singbox_target(config_root, bridge, profile_id).await;
    }
    Err(FfiStatus::err(
        FfiErrorCode::NotSupported,
        format!("Android has no Core backend for `{}`", engine_id.as_str()),
    ))
}

async fn prepare_mihomo_target(
    config_root: PathBuf,
    bridge: Arc<dyn AndroidBridge>,
    profile_id: ProfileId,
) -> Result<PreparedTarget, FfiStatus> {
    if bridge.data_dir().is_none() {
        return Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android Core launch working directory is unavailable",
        ));
    }
    let config = Arc::new(
        ConfigManager::with_profile_codec_and_store_at_root(
            config_root,
            DefaultCredentialStore::default(),
            Arc::new(MihomoProfileDocumentCodec),
            ProfileDocumentExtension::Yaml,
        )
        .map_err(map_mihomo_error)?,
    );
    config
        .initialize_profile_namespace()
        .await
        .map_err(map_mihomo_error)?;
    let workspace = ProfileStore::load_workspace(config.engine_profile_store(), &profile_id)
        .await
        .map_err(map_port_error)?;
    if workspace.document.identity.engine_id != MihomoVersionPort::engine_identifier()
        || workspace.document.identity.profile_id != profile_id
    {
        return Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android Mihomo target does not match its persisted profile document",
        ));
    }
    let endpoints = Arc::new(ProfileEndpointSource::new(Arc::clone(&config)));
    let endpoint = endpoints
        .resolve_profile(&profile_id)
        .await
        .map_err(map_port_error)?;
    let capabilities = android_core_capability_provider(&endpoint.url, endpoint.secret.clone());
    authorize_android_core_start(
        Arc::clone(&capabilities),
        &MihomoVersionPort::engine_identifier(),
    )?;
    let readiness: Arc<dyn CoreReadiness> =
        Arc::new(AndroidProfileReadiness::new(Arc::clone(&endpoints)));
    let provider = android_mihomo_backend_provider(
        MihomoVersionPort::engine_identifier(),
        bridge,
        Arc::clone(&config),
        &endpoint.url,
        endpoint.secret.clone(),
        readiness,
    )
    .map_err(map_port_error)?;
    Ok(PreparedTarget {
        config,
        document: workspace.document,
        provider,
        capabilities,
        runtime: PreparedTargetRuntime::Mihomo {
            endpoints,
            controller_url: endpoint.url,
            controller_secret: endpoint.secret,
        },
    })
}

async fn prepare_singbox_target(
    config_root: PathBuf,
    bridge: Arc<dyn AndroidBridge>,
    profile_id: ProfileId,
) -> Result<PreparedTarget, FfiStatus> {
    let config = profile_store_at_root(config_root).await?;
    let workspace = ProfileStore::load_workspace(config.engine_profile_store(), &profile_id)
        .await
        .map_err(map_port_error)?;
    if workspace.document.identity.engine_id != singbox_api::engine_id()
        || workspace.document.identity.profile_id != profile_id
    {
        return Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android sing-box target does not match its persisted profile document",
        ));
    }
    let working_directory = bridge.data_dir().ok_or_else(|| {
        FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android host did not provide an app-private Core working directory",
        )
    })?;
    let engine_id = singbox_api::engine_id();
    let binary_selection = core_binary_selection_port(&engine_id)?;
    let expected_target = EngineBinaryTarget::current();
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
    let readiness: Arc<dyn CoreReadiness> = Arc::new(
        SingBoxControllerReadiness::with_reqwest(controller).map_err(|error| {
            FfiStatus::err(
                FfiErrorCode::Config,
                format!("Android sing-box readiness adapter is unavailable: {error}"),
            )
        })?,
    );
    let launch_preflight = AndroidSingBoxProfileLaunchSpecProvider::new(
        Arc::clone(&config),
        workspace.document.clone(),
        Arc::clone(&binary_selection),
        working_directory.clone(),
        expected_target,
        Arc::new(TokioSingBoxCommandPort),
    );
    CoreLaunchSpecSource::resolve(&launch_preflight)
        .await
        .map_err(map_port_error)?;
    let selected_binary = binary_selection
        .select(CoreBinarySelectionRequest {
            engine_id: engine_id.clone(),
            policy: CoreBinarySelectionPolicy::ManagedOnly,
        })
        .await
        .map_err(map_port_error)?;
    validate_binary_selection(
        &selected_binary,
        &engine_id,
        expected_target,
        &working_directory,
    )?;
    let provider_parts = android_singbox_backend_provider(
        bridge,
        Arc::clone(&config),
        workspace.document.clone(),
        binary_selection,
        selected_binary,
        working_directory,
        expected_target,
        Arc::new(TokioSingBoxCommandPort),
        readiness,
    )
    .await
    .map_err(map_port_error)?;
    let versions = core_version_application(engine_id)?;
    let settings = build_settings_application().await?;
    let AndroidSingBoxBackendProviderParts {
        provider,
        configuration,
        operation_gate,
    } = provider_parts;
    Ok(PreparedTarget {
        config,
        document: workspace.document,
        provider,
        capabilities: capability_provider,
        runtime: PreparedTargetRuntime::SingBox {
            configuration,
            operation_gate,
            versions,
            settings,
        },
    })
}

async fn restore_previous(previous: &Arc<SharedCore>, failure: FfiStatus) -> FfiStatus {
    let completion = {
        let mut state = shared_core_owner_slot().lock().await;
        finish_shared_core_switch(
            &mut state,
            previous,
            SharedCoreOwnerSwitchCompletion::RestorePrevious,
        )
    };
    match completion {
        SharedCoreOwnerSwitchFinish::Restored => failure,
        SharedCoreOwnerSwitchFinish::OwnerStateChanged => FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android Core owner changed during switch preflight; rebind is required",
        ),
        SharedCoreOwnerSwitchFinish::Installed | SharedCoreOwnerSwitchFinish::RecoveryRequired => {
            FfiStatus::err(
                FfiErrorCode::NotReady,
                "Android Core owner could not restore the previous backend",
            )
        }
    }
}

async fn require_owner_recovery(previous: &Arc<SharedCore>, failure: FfiStatus) -> FfiStatus {
    let completion = {
        let mut state = shared_core_owner_slot().lock().await;
        finish_shared_core_switch(
            &mut state,
            previous,
            SharedCoreOwnerSwitchCompletion::RecoveryRequired,
        )
    };
    match completion {
        SharedCoreOwnerSwitchFinish::RecoveryRequired => failure,
        SharedCoreOwnerSwitchFinish::OwnerStateChanged => FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android Core owner changed during switch; rebind is required",
        ),
        SharedCoreOwnerSwitchFinish::Installed | SharedCoreOwnerSwitchFinish::Restored => {
            FfiStatus::err(
                FfiErrorCode::NotReady,
                "Android Core owner could not enter recovery-required state",
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::active_profile_reference::ActiveProfileReferenceRevision;
    use lurker_core_contract::identity::EngineId;

    #[test]
    fn different_mihomo_profile_uses_profile_activation() {
        assert!(matches!(
            profile_selection_action(
                reference(EngineId::mihomo(), "active"),
                selected(EngineId::mihomo(), "main")
            ),
            Ok(AndroidProfileSelectionAction::ActivateMihomo(_))
        ));
    }

    #[test]
    fn different_singbox_profile_replaces_the_live_owner() {
        assert!(matches!(
            profile_selection_action(
                reference(EngineId::sing_box(), "active"),
                selected(EngineId::sing_box(), "main")
            ),
            Ok(AndroidProfileSelectionAction::ReplaceOwner(_))
        ));
    }

    #[test]
    fn selecting_the_current_profile_is_idempotent() {
        assert!(matches!(
            profile_selection_action(
                reference(EngineId::sing_box(), "main"),
                selected(EngineId::sing_box(), "main")
            ),
            Ok(AndroidProfileSelectionAction::AlreadySelected)
        ));
    }

    #[test]
    fn cross_engine_selection_enters_the_owner_replacement_transaction() {
        assert!(matches!(
            profile_selection_action(
                reference(EngineId::mihomo(), "active"),
                selected(EngineId::sing_box(), "main")
            ),
            Ok(AndroidProfileSelectionAction::ReplaceOwner(_))
        ));
        assert!(matches!(
            profile_selection_action(
                reference(EngineId::sing_box(), "active"),
                selected(EngineId::mihomo(), "main")
            ),
            Ok(AndroidProfileSelectionAction::ReplaceOwner(_))
        ));
    }

    #[test]
    fn unselected_profile_target_is_rejected_before_owner_replacement() {
        assert!(matches!(
            profile_selection_action(
                reference(EngineId::mihomo(), "main"),
                ActiveProfileReferenceTarget::Unselected,
            ),
            Err(status) if status.code == FfiErrorCode::Config
        ));
    }

    fn reference(engine_id: EngineId, profile_id: &str) -> ActiveProfileReference {
        ActiveProfileReference::new(
            engine_id,
            ProfileId::new(profile_id).expect("valid profile ID"),
            ActiveProfileReferenceRevision::new(1),
        )
    }

    fn selected(engine_id: EngineId, profile_id: &str) -> ActiveProfileReferenceTarget {
        ActiveProfileReferenceTarget::selected(
            engine_id,
            ProfileId::new(profile_id).expect("valid profile ID"),
        )
    }
}

fn switch_admission_failure(admission: SharedCoreOwnerAdmission<'static>) -> FfiStatus {
    match admission {
        SharedCoreOwnerAdmission::Build => FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android Core owner is not active for profile replacement",
        ),
        SharedCoreOwnerAdmission::RecoveryRequired => FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android Core owner recovery is required before profile replacement",
        ),
        SharedCoreOwnerAdmission::Switching => FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android Core owner already has a profile replacement in progress",
        ),
        SharedCoreOwnerAdmission::Closing => FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android Core owner is closing and cannot replace its profile",
        ),
        SharedCoreOwnerAdmission::Existing(_) => FfiStatus::err(
            FfiErrorCode::Unknown,
            "Android Core owner replacement admission returned an invalid active state",
        ),
    }
}

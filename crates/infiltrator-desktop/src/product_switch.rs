//! Desktop Core owner switch preflight and engine-specific candidate binding.

use crate::composition::mihomo_backend::{
    desktop_mihomo_backend_from_selected, desktop_mihomo_engine_provider,
};
use crate::composition::{
    DesktopLogStreamAdapter, DesktopProductBackend, desktop_core_capability_provider,
    desktop_runtime_factory, select_product_binary,
};
use crate::core_owner_runtime::DesktopCoreOwnerRuntime;
use crate::product::DesktopProductSwitchFailure;
use crate::runtime::{
    MihomoRuntime, MihomoRuntimeAssemblyInputs, collect_geoip_candidates,
    ensure_geoip_database_offline,
};
use crate::service::ServiceManager;
use crate::service_mode::DesktopServiceMode;
use crate::singbox_runtime::{SingBoxRuntime, desktop_singbox_instance_id, validate_selection};
use crate::storage::{
    DesktopEngineProfileStores, engine_profile_stores, home_dir, version_factory,
};
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_composition::active_profile_backend::{
    ActiveProfileBackendSelector, ActiveProfileBackendSwitchError,
    ActiveProfileBackendSwitchFailurePhase, PreviousCoreOwner, SelectedActiveProfileBackend,
};
use infiltrator_composition::engine_backend_factory::{
    EngineBackendFactoryRegistry, SingBoxEngineBackendProvider,
};
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceTarget;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::profile_document_identity::{ProfileDocument, ProfileId};
use infiltrator_contract::snapshot::CoreLifecycleSnapshot;
use infiltrator_contract::surface::SurfaceKind;
use infiltrator_core::settings_io::app_config_manager;
use infiltrator_core::version_port::MihomoVersionPort;
use infiltrator_platform::defaults::DefaultCredentialStore;
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::core_binary_selection::{BundledBinarySource, CoreBinarySelection};
use infiltrator_ports::engine_backend_factory::{
    EngineBackendFactory, EngineBackendProvider, EngineBackendProviderAvailability,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::host_runtime::HostRuntime;
use infiltrator_ports::profile_store::ProfileStore;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::EngineId;
use mihomo_api::client::MihomoClient;
use mihomo_config::endpoint::ProfileEndpointSource;
use mihomo_config::manager::ConfigManager;
use mihomo_config::yaml;
use mihomo_version::bundled_kernel::packaged_candidates;
use singbox_api::controller_configuration::SingBoxControllerConfiguration;
use singbox_api::http::ReqwestSingBoxClashHttpPort;
use singbox_process::command::TokioSingBoxCommandPort;
use std::fmt::Display;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

enum PreparedDesktopEngine {
    Mihomo {
        selection: CoreBinarySelection,
        config_manager: Arc<ConfigManager<DefaultCredentialStore>>,
        service_manager: ServiceManager,
        config_path: PathBuf,
        endpoints: Arc<ProfileEndpointSource<DefaultCredentialStore>>,
        controller_url: String,
        endpoint_secret: Option<String>,
        client: MihomoClient,
        provider: Arc<dyn EngineBackendProvider>,
        profile_document: ProfileDocument,
    },
    SingBox {
        selection: CoreBinarySelection,
        profile_store: Arc<dyn ProfileStore>,
        controller: SingBoxControllerConfiguration,
        provider: Arc<SingBoxEngineBackendProvider>,
        profile_document: ProfileDocument,
    },
}

struct PreparedDesktopSwitch {
    target: ActiveProfileReferenceTarget,
    prepared: PreparedDesktopEngine,
    prepared_document: ProfileDocument,
    selector: ActiveProfileBackendSelector,
}

pub(crate) async fn switch_default_product_engine(
    previous_application: Arc<CoreApplication>,
    target: ActiveProfileReferenceTarget,
    surface: SurfaceKind,
) -> Result<DesktopProductBackend, DesktopProductSwitchFailure> {
    let prepared = prepare_desktop_switch(target).await?;
    let selected = prepared
        .selector
        .replace_owner_for_profile(
            prepared.target.clone(),
            &prepared.prepared_document,
            &PreviousCoreOwner::Owned(previous_application),
        )
        .await
        .map_err(map_switch_error)?;
    compose_desktop_switch(prepared.prepared, selected, surface).await
}

pub(crate) async fn retry_default_product_engine(
    target: ActiveProfileReferenceTarget,
    old_owner_terminal: &CoreLifecycleSnapshot,
    surface: SurfaceKind,
) -> Result<DesktopProductBackend, DesktopProductSwitchFailure> {
    let prepared = prepare_desktop_switch(target).await?;
    let selected = prepared
        .selector
        .resume_owner_switch_for_profile(
            prepared.target.clone(),
            &prepared.prepared_document,
            old_owner_terminal,
        )
        .await
        .map_err(old_owner_closed)?;
    compose_desktop_switch(prepared.prepared, selected, surface).await
}

async fn prepare_desktop_switch(
    target: ActiveProfileReferenceTarget,
) -> Result<PreparedDesktopSwitch, DesktopProductSwitchFailure> {
    let (engine_id, profile_id) = selected_target(target.clone())?;
    let config_manager = Arc::new(app_config_manager().await.map_err(before_reservation)?);
    let home = home_dir().map_err(before_reservation)?;
    let profile_stores =
        Arc::new(engine_profile_stores(Arc::clone(&config_manager)).map_err(before_reservation)?);
    let profile_store = profile_stores
        .for_engine(&engine_id)
        .map_err(before_reservation)?;
    let target_workspace = profile_store
        .load_workspace(&profile_id)
        .await
        .map_err(before_reservation)?;
    validate_target_document(&engine_id, &profile_id, &target_workspace.document)
        .map_err(before_reservation)?;

    let runtime_provider = desktop_runtime_factory()
        .and_then(|factory| factory.provider(&engine_id))
        .map_err(before_reservation)?;
    let policy = runtime_provider.binary_selection_policy(BundledBinarySource {
        candidates: packaged_candidates().map_err(before_reservation)?,
        data_directory: home,
    });
    let version_factory = version_factory().map_err(before_reservation)?;
    let selection = select_product_binary(&version_factory, &engine_id, policy)
        .await
        .map_err(before_reservation)?;
    let capabilities =
        desktop_core_capability_provider(DesktopLogStreamAdapter::CoreOwnerScopeBound)
            .map_err(before_reservation)?;
    DesktopCoreOwnerRuntime::validate_engine_start_capability(
        &capabilities,
        &selection,
        &engine_id,
    )
    .map_err(before_reservation)?;

    let prepared = match engine_id.as_str() {
        "mihomo" => {
            prepare_mihomo(
                selection,
                profile_id,
                Arc::clone(&config_manager),
                Arc::clone(&profile_stores),
                target_workspace.document,
            )
            .await?
        }
        "sing-box" => {
            prepare_sing_box(
                selection,
                Arc::clone(&profile_store),
                Arc::clone(&profile_stores),
                profile_id,
                target_workspace.document,
            )
            .await?
        }
        _ => {
            return Err(before_reservation(PortError::unsupported(
                Capability::CoreLifecycle,
                format!(
                    "no Desktop Core runtime is registered for {}",
                    engine_id.as_str()
                ),
            )));
        }
    };

    let (engine_factory, prepared_document, capability_provider) = match &prepared {
        PreparedDesktopEngine::Mihomo { provider, .. } => {
            let provider: Arc<dyn EngineBackendProvider> = Arc::clone(provider);
            let factory: Arc<dyn EngineBackendFactory> = Arc::new(
                EngineBackendFactoryRegistry::new([Arc::clone(&provider)])
                    .map_err(before_reservation)?,
            );
            (factory, prepared_document_for(&prepared), capabilities)
        }
        PreparedDesktopEngine::SingBox {
            provider,
            selection,
            ..
        } => {
            validate_selection(selection, &engine_id).map_err(before_reservation)?;
            let factory_provider: Arc<dyn EngineBackendProvider> = provider.clone();
            let factory: Arc<dyn EngineBackendFactory> = Arc::new(
                EngineBackendFactoryRegistry::new([factory_provider])
                    .map_err(before_reservation)?,
            );
            (factory, prepared_document_for(&prepared), capabilities)
        }
    };
    if let EngineBackendProviderAvailability::Unsupported { reason } =
        engine_factory.provider_availability(&engine_id)
    {
        return Err(before_reservation(PortError::unsupported(
            Capability::CoreLifecycle,
            reason,
        )));
    }

    let reference_store: Arc<dyn ActiveProfileReferenceStore> =
        config_manager.shared_engine_profile_store();
    let selector = ActiveProfileBackendSelector::new(
        reference_store,
        profile_stores.all(),
        engine_factory,
        capability_provider,
    );
    Ok(PreparedDesktopSwitch {
        target,
        prepared,
        prepared_document,
        selector,
    })
}

async fn compose_desktop_switch(
    prepared: PreparedDesktopEngine,
    selected: SelectedActiveProfileBackend,
    surface: SurfaceKind,
) -> Result<DesktopProductBackend, DesktopProductSwitchFailure> {
    match prepared {
        PreparedDesktopEngine::Mihomo {
            selection,
            config_manager,
            service_manager,
            config_path,
            endpoints,
            controller_url,
            endpoint_secret,
            client,
            provider: _,
            profile_document: _,
        } => {
            compose_mihomo_switch(MihomoSwitchCompositionInputs {
                selection,
                config_manager,
                service_manager,
                config_path,
                endpoints,
                controller_url,
                endpoint_secret,
                client,
                selected,
                surface,
            })
            .await
        }
        PreparedDesktopEngine::SingBox {
            selection,
            profile_store,
            controller,
            provider,
            profile_document: _,
        } => {
            let profiles = ProfileApplication::new(Arc::clone(&profile_store));
            let runtime = SingBoxRuntime::compose_selected(
                selection,
                profile_store,
                controller,
                provider,
                selected,
            )
            .await
            .map_err(old_owner_closed)?;
            let application = runtime.application();
            let surface_pump = match runtime
                .surface_pump(surface, Duration::from_millis(700))
                .await
            {
                Ok(pump) => pump,
                Err(error) => {
                    let failure = match application.close().await {
                        Ok(_) => error.to_string(),
                        Err(close_error) => format!(
                            "{error}; closing the unstarted sing-box candidate also failed: {}",
                            close_error.message
                        ),
                    };
                    return Err(old_owner_closed(failure));
                }
            };
            let host_runtime: Arc<dyn HostRuntime> = Arc::new(runtime);
            Ok(DesktopProductBackend {
                application,
                profiles,
                host_runtime,
                surface_pump,
            })
        }
    }
}

async fn prepare_mihomo(
    selection: CoreBinarySelection,
    profile_id: ProfileId,
    config_manager: Arc<ConfigManager<DefaultCredentialStore>>,
    profile_stores: Arc<DesktopEngineProfileStores>,
    initial_document: ProfileDocument,
) -> Result<PreparedDesktopEngine, DesktopProductSwitchFailure> {
    let profile_store = profile_stores
        .for_engine(&MihomoVersionPort::engine_identifier())
        .map_err(before_reservation)?;
    let initial_workspace = profile_store
        .load_workspace(&profile_id)
        .await
        .map_err(before_reservation)?;
    if initial_workspace.document != initial_document {
        return Err(before_reservation(Failure::new(
            ErrorCode::NotReady,
            "Mihomo profile changed before Desktop switch preparation",
            true,
        )));
    }
    let content = config_manager
        .load(profile_id.as_str())
        .await
        .map_err(before_reservation)?;
    yaml::validate(&content).map_err(before_reservation)?;
    config_manager
        .ensure_proxy_ports_for_profile(&profile_id)
        .await
        .map_err(before_reservation)?;
    let ensured_controller_url = config_manager
        .ensure_external_controller_for_profile(&profile_id)
        .await
        .map_err(before_reservation)?;
    let config_path = config_manager
        .existing_profile_document_path(profile_id.as_str())
        .await
        .map_err(before_reservation)?;
    let service_manager = ServiceManager::new(selection.binary_path.clone(), config_path.clone());
    let endpoints = Arc::new(ProfileEndpointSource::new(Arc::clone(&config_manager)));
    let endpoint = endpoints
        .resolve_profile(&profile_id)
        .await
        .map_err(before_reservation)?;
    if endpoint.url != ensured_controller_url {
        return Err(before_reservation(Failure::new(
            ErrorCode::NotReady,
            "Mihomo endpoint changed during Desktop switch preflight",
            true,
        )));
    }
    let workspace = profile_store
        .load_workspace(&profile_id)
        .await
        .map_err(before_reservation)?;
    validate_target_document(
        &MihomoVersionPort::engine_identifier(),
        &profile_id,
        &workspace.document,
    )
    .map_err(before_reservation)?;
    let geoip_candidates =
        collect_geoip_candidates(&selection.binary_path, &selection.related_asset_candidates);
    ensure_geoip_database_offline(&config_path, &geoip_candidates)
        .await
        .map_err(before_reservation)?;
    let (provider, client) = desktop_mihomo_engine_provider(
        &service_manager,
        &ensured_controller_url,
        endpoint.secret.clone(),
    )
    .map_err(before_reservation)?;
    let provider: Arc<dyn EngineBackendProvider> = provider;
    Ok(PreparedDesktopEngine::Mihomo {
        selection,
        config_manager,
        service_manager,
        config_path,
        endpoints,
        controller_url: ensured_controller_url,
        endpoint_secret: endpoint.secret,
        client,
        provider,
        profile_document: workspace.document,
    })
}

async fn prepare_sing_box(
    selection: CoreBinarySelection,
    profile_store: Arc<dyn ProfileStore>,
    profile_stores: Arc<DesktopEngineProfileStores>,
    profile_id: ProfileId,
    document: ProfileDocument,
) -> Result<PreparedDesktopEngine, DesktopProductSwitchFailure> {
    let sing_box_manager = Arc::clone(&profile_stores.sing_box_manager);
    let config_path = sing_box_manager
        .existing_profile_document_path(profile_id.as_str())
        .await
        .map_err(before_reservation)?;
    let workspace = profile_store
        .load_workspace(&profile_id)
        .await
        .map_err(before_reservation)?;
    if workspace.document != document {
        return Err(before_reservation(PortError::Rejected(Failure::new(
            ErrorCode::NotReady,
            "sing-box profile changed during Desktop switch preflight",
            true,
        ))));
    }
    let controller = SingBoxControllerConfiguration::from_profile_json(&document.raw_bytes)
        .map_err(before_reservation)?;
    let provider = Arc::new(
        SingBoxEngineBackendProvider::new(
            selection.binary_path.clone(),
            config_path.clone(),
            desktop_singbox_instance_id().map_err(before_reservation)?,
            controller.clone(),
            Arc::new(TokioSingBoxCommandPort),
            Arc::new(ReqwestSingBoxClashHttpPort),
        )
        .map_err(before_reservation)?,
    );
    Ok(PreparedDesktopEngine::SingBox {
        selection,
        profile_store,
        controller,
        provider,
        profile_document: document,
    })
}

struct MihomoSwitchCompositionInputs {
    selection: CoreBinarySelection,
    config_manager: Arc<ConfigManager<DefaultCredentialStore>>,
    service_manager: ServiceManager,
    config_path: PathBuf,
    endpoints: Arc<ProfileEndpointSource<DefaultCredentialStore>>,
    controller_url: String,
    endpoint_secret: Option<String>,
    client: MihomoClient,
    selected: SelectedActiveProfileBackend,
    surface: SurfaceKind,
}

async fn compose_mihomo_switch(
    inputs: MihomoSwitchCompositionInputs,
) -> Result<DesktopProductBackend, DesktopProductSwitchFailure> {
    let MihomoSwitchCompositionInputs {
        selection,
        config_manager,
        service_manager,
        config_path,
        endpoints,
        controller_url,
        endpoint_secret,
        client,
        selected,
        surface,
    } = inputs;
    let backend = desktop_mihomo_backend_from_selected(client, selected)
        .await
        .map_err(old_owner_closed)?;
    let binary_path = selection.binary_path.clone();
    let runtime = Arc::new(
        MihomoRuntime::assemble_product(MihomoRuntimeAssemblyInputs {
            selection,
            config_manager: config_manager.clone(),
            config_path,
            controller_url,
            service_manager,
            service_mode: Arc::new(DesktopServiceMode::new(binary_path)),
            endpoints,
            endpoint_secret,
            backend,
        })
        .await
        .map_err(old_owner_closed)?,
    );
    let application = runtime.application();
    let profiles = runtime.profiles();
    let surface_pump = match runtime
        .surface_pump(surface, Duration::from_millis(700))
        .await
    {
        Ok(pump) => pump,
        Err(error) => {
            let failure = match application.close().await {
                Ok(_) => error.to_string(),
                Err(close_error) => format!(
                    "{error}; closing the unstarted Mihomo candidate also failed: {}",
                    close_error.message
                ),
            };
            return Err(old_owner_closed(failure));
        }
    };
    let host_runtime: Arc<dyn HostRuntime> = runtime;
    Ok(DesktopProductBackend {
        application,
        profiles,
        host_runtime,
        surface_pump,
    })
}

fn selected_target(
    target: ActiveProfileReferenceTarget,
) -> Result<(EngineId, ProfileId), DesktopProductSwitchFailure> {
    match target {
        ActiveProfileReferenceTarget::Selected {
            engine_id,
            profile_id,
        } => Ok((engine_id, profile_id)),
        ActiveProfileReferenceTarget::Unselected => Err(before_failure(Failure::unsupported(
            "Desktop Core switch requires a selected profile",
        ))),
    }
}

fn validate_target_document(
    engine_id: &EngineId,
    profile_id: &ProfileId,
    document: &ProfileDocument,
) -> Result<(), DesktopProductSwitchFailure> {
    if &document.identity.engine_id != engine_id || &document.identity.profile_id != profile_id {
        return Err(before_reservation(Failure::new(
            ErrorCode::InvalidInput,
            "Desktop switch target document identity does not match the selected reference",
            false,
        )));
    }
    Ok(())
}

fn prepared_document_for(prepared: &PreparedDesktopEngine) -> ProfileDocument {
    match prepared {
        PreparedDesktopEngine::Mihomo {
            profile_document, ..
        }
        | PreparedDesktopEngine::SingBox {
            profile_document, ..
        } => profile_document.clone(),
    }
}

fn before_reservation(error: impl Display) -> DesktopProductSwitchFailure {
    before_failure(Failure::new(ErrorCode::NotReady, error.to_string(), true))
}

fn before_failure(failure: Failure) -> DesktopProductSwitchFailure {
    DesktopProductSwitchFailure::BeforeReservation(failure)
}

fn old_owner_closed(error: impl Display) -> DesktopProductSwitchFailure {
    DesktopProductSwitchFailure::OldOwnerClosed(Failure::new(
        ErrorCode::NotReady,
        error.to_string(),
        true,
    ))
}

fn map_switch_error(error: ActiveProfileBackendSwitchError) -> DesktopProductSwitchFailure {
    let phase = error.phase();
    let failure = Failure::from(error.into_failure());
    match phase {
        ActiveProfileBackendSwitchFailurePhase::BeforeReservation => {
            DesktopProductSwitchFailure::BeforeReservation(failure)
        }
        ActiveProfileBackendSwitchFailurePhase::ReservationPending => {
            DesktopProductSwitchFailure::ReservationPending(failure)
        }
        ActiveProfileBackendSwitchFailurePhase::OwnerTerminationUnconfirmed => {
            DesktopProductSwitchFailure::OwnerTerminationUnconfirmed(failure)
        }
        ActiveProfileBackendSwitchFailurePhase::OldOwnerClosed => {
            DesktopProductSwitchFailure::OldOwnerClosed(failure)
        }
    }
}

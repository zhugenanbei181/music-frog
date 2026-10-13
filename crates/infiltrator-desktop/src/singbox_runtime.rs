//! Desktop-owned sing-box lifecycle composition.

use crate::core_owner_runtime::DesktopCoreOwnerRuntime;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_composition::active_profile_backend::{
    ActiveProfileBackendSelector, ActiveProfileSelectionPreflight, PreviousCoreOwner,
    SelectedActiveProfileBackend,
};
use infiltrator_composition::engine_backend_factory::{
    EngineBackendFactoryRegistry, SingBoxEngineBackendProvider,
};
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_capabilities::EngineBinaryTarget;
use infiltrator_contract::core_identity::{CoreInstanceId, EngineId};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::version::CoreArtifactVerification;
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::core_binary_selection::{CoreBinarySelection, CoreBinarySelectionSource};
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::core_scope::CoreScopeSource;
use infiltrator_ports::engine_backend_factory::{
    EngineBackendFactory, EngineBackendPorts, EngineBackendProvider,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use singbox_api::controller_configuration::SingBoxControllerConfiguration;
use singbox_api::engine_id;
use singbox_api::http::{ReqwestSingBoxClashHttpPort, SingBoxClashHttpPort};
use singbox_process::command::{SingBoxCommandPort, TokioSingBoxCommandPort};
use std::path::PathBuf;
use std::process;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct SingBoxRuntime;

impl SingBoxRuntime {
    pub(crate) async fn prepare(
        selection: CoreBinarySelection,
        reference_store: Arc<dyn ActiveProfileReferenceStore>,
        profile_store: Arc<dyn ProfileStore>,
        config_path: PathBuf,
        preflight: ActiveProfileSelectionPreflight,
    ) -> anyhow::Result<DesktopCoreOwnerRuntime> {
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
    ) -> anyhow::Result<DesktopCoreOwnerRuntime> {
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
        DesktopCoreOwnerRuntime::validate_engine_start_capability(
            &capability_provider,
            &selection,
            &engine_id,
        )?;
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
        Self::compose_selected(selection, profile_store, controller, provider, selected).await
    }

    pub(crate) async fn compose_selected(
        selection: CoreBinarySelection,
        profile_store: Arc<dyn ProfileStore>,
        controller: SingBoxControllerConfiguration,
        provider: Arc<SingBoxEngineBackendProvider>,
        selected: SelectedActiveProfileBackend,
    ) -> anyhow::Result<DesktopCoreOwnerRuntime> {
        if let Err(error) = validate_selection(&selection, &engine_id()) {
            let abort_error = selected.abort_before_core_application().await.err();
            return Err(close_candidate_error(error, abort_error));
        }
        if profile_store.engine_id() != engine_id()
            || selected.reference.engine_id != engine_id()
            || selected.document.identity.engine_id != engine_id()
        {
            let error = PortError::unsupported(
                Capability::CoreLifecycle,
                "selected sing-box runtime inputs do not share one engine identity",
            );
            let abort_error = selected.abort_before_core_application().await.err();
            return Err(close_candidate_error(error, abort_error));
        }
        let capability_provider = selected.capability_provider();
        if let Err(error) = DesktopCoreOwnerRuntime::validate_engine_start_capability(
            &capability_provider,
            &selection,
            &engine_id(),
        ) {
            let abort_error = selected.abort_before_core_application().await.err();
            return Err(close_candidate_error(error, abort_error));
        }
        let backend = selected.backend.clone();
        let application_runtime = match infiltrator_composition::tokio_application_runtime() {
            Ok(runtime) => runtime,
            Err(error) => {
                selected
                    .abort_before_core_application()
                    .await
                    .map_err(anyhow::Error::new)?;
                return Err(anyhow::Error::msg(error));
            }
        };
        let application = Arc::new(CoreApplication::new(
            backend.process(),
            backend.readiness(),
            application_runtime,
        ));

        if let Err(failure) = selected.install_core_capabilities(application.as_ref()) {
            let error = PortError::Rejected(failure);
            let discard_error = selected
                .discard_application_candidate(application.as_ref())
                .await
                .err();
            return Err(close_candidate_error(error, discard_error));
        }
        let binding = bind_core(application.as_ref(), &provider, &backend, profile_store);
        if let Err(error) = binding {
            let discard_error = selected
                .discard_application_candidate(application.as_ref())
                .await
                .err();
            return Err(close_candidate_error(error, discard_error));
        }
        if let Err(error) = selected
            .install_owner_binding_for_application(application.as_ref())
            .await
        {
            return Err(anyhow::Error::new(error));
        }

        let configuration_transaction = backend.configuration_transaction();
        Ok(DesktopCoreOwnerRuntime::from_bound_application(
            application,
            selection.binary_path,
            controller.endpoint(),
            configuration_transaction,
        ))
    }
}

fn bind_core(
    application: &CoreApplication,
    provider: &SingBoxEngineBackendProvider,
    backend: &EngineBackendPorts,
    profile_store: Arc<dyn ProfileStore>,
) -> Result<(), PortError> {
    let owner_scope: Arc<dyn CoreScopeSource> = Arc::new(application.clone());
    provider.bind_core_owner_scope(backend, owner_scope)?;
    let lifecycle: Arc<dyn CoreLifecyclePort> = Arc::new(application.clone());
    provider.bind_profile_configuration_transaction(backend, profile_store, lifecycle)
}

fn close_candidate_error(error: PortError, discard_error: Option<PortError>) -> anyhow::Error {
    match discard_error {
        None => anyhow::Error::new(error),
        Some(discard_error) => anyhow::Error::new(PortError::Rejected(Failure::new(
            ErrorCode::InvalidState,
            format!("{error}; discarding the unstarted sing-box candidate failed: {discard_error}"),
            true,
        ))),
    }
}

pub(crate) fn validate_selection(
    selection: &CoreBinarySelection,
    engine_id: &EngineId,
) -> Result<(), PortError> {
    selection.validate_for(engine_id, EngineBinaryTarget::current())?;
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

pub(crate) fn desktop_singbox_instance_id() -> Result<CoreInstanceId, PortError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| PortError::Failed("system clock predates the Unix epoch".to_owned()))?
        .as_nanos();
    CoreInstanceId::new(format!("desktop-sing-box-{}-{timestamp}", process::id()))
        .map_err(|error| PortError::Failed(error.to_string()))
}

#[cfg(test)]
#[path = "singbox_runtime_tests.rs"]
mod tests;

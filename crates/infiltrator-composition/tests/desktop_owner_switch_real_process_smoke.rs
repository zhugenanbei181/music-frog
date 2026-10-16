//! test-intent: real_process

use infiltrator_composition::active_profile_backend::SelectedActiveProfileBackend;
use std::mem;
use async_trait::async_trait;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_composition::active_profile_backend::{
    ActiveProfileBackendSelector, ActiveProfileBackendSwitchFailurePhase, PreviousCoreOwner,
};
use infiltrator_composition::engine_backend_factory::{
    EngineBackendFactoryRegistry, SingBoxEngineBackendProvider,
};
use infiltrator_composition::engine_capability_provider_registry::EngineCoreCapabilityProviderRegistry;
use infiltrator_composition::mihomo_engine_backend_provider::MihomoEngineBackendProvider;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceRevision, ActiveProfileReferenceState, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::core_capabilities::{CoreLifecycleOperation, CoreOperation};
use infiltrator_contract::core_identity::{CoreInstanceId, EngineId};
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface::HostKind;
use infiltrator_ports::active_profile_reference_store::{
    ActiveProfileOwnerSwitchRecovery, ActiveProfileReferenceStore,
};
use infiltrator_ports::core_capability_provider::CoreCapabilityProvider;
use infiltrator_ports::core_lifecycle::{CoreLifecyclePort, CoreOwnerTerminationPort};
use infiltrator_ports::engine_backend_factory::{EngineBackendPorts, EngineBackendProvider};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::secure_store::SecureStore;
use infiltrator_profile_store::store::ProfileDocumentExtension;
use mihomo_api::client::MihomoClient;
use mihomo_api::core_capability_provider::MihomoCoreCapabilityProvider;
use mihomo_api::readiness::ControllerReadiness;
use mihomo_config::manager::ConfigManager;
use mihomo_config::profile_document_codec::MihomoProfileDocumentCodec;
use mihomo_process::desktop::ProcessCoreController;
use singbox_api::capability_provider::SingBoxCoreCapabilityProvider;
use singbox_api::controller_configuration::SingBoxControllerConfiguration;
use singbox_config::profile_document_codec::SingBoxProfileDocumentCodec;
use std::env;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

enum CandidateDispatchState {
    RejectFirstCreation,
    Available,
}

struct RecoveringSingBoxProvider {
    inner: Arc<SingBoxEngineBackendProvider>,
    state: Mutex<CandidateDispatchState>,
}

impl RecoveringSingBoxProvider {
    fn new(inner: Arc<SingBoxEngineBackendProvider>) -> Self {
        Self {
            inner,
            state: Mutex::new(CandidateDispatchState::RejectFirstCreation),
        }
    }
}

impl EngineBackendProvider for RecoveringSingBoxProvider {
    fn engine_id(&self) -> EngineId {
        EngineBackendProvider::engine_id(self.inner.as_ref())
    }

    fn create_backend(&self) -> Result<EngineBackendPorts, PortError> {
        let creation = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| PortError::Failed("smoke provider state was poisoned".into()))?;
            match mem::replace(&mut *state, CandidateDispatchState::Available) {
                CandidateDispatchState::RejectFirstCreation => Err(PortError::Failed(
                    "injected one-time sing-box candidate dispatch failure".into(),
                )),
                CandidateDispatchState::Available => Ok(()),
            }
        };
        creation?;
        EngineBackendProvider::create_backend(self.inner.as_ref())
    }
}

struct SmokeSecureStore;

#[async_trait]
impl SecureStore for SmokeSecureStore {
    async fn get(&self, _namespace: &str, _key: &str) -> Result<Option<String>, PortError> {
        Ok(None)
    }

    async fn set(&self, _namespace: &str, _key: &str, _value: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn delete(&self, _namespace: &str, _key: &str) -> Result<(), PortError> {
        Ok(())
    }
}

fn required_environment(name: &str) -> Result<String, String> {
    env::var(name).map_err(|_| format!("{name} must be set by the SHA-locked owner-switch smoke"))
}

fn loopback_endpoint() -> Result<String, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|error| format!("reserve a loopback controller port: {error}"))?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("read loopback controller port: {error}"))?;
    drop(listener);
    Ok(format!("http://{address}"))
}

fn selected_profile(
    state: ActiveProfileReferenceState,
    engine_id: EngineId,
    profile_id: ProfileId,
) -> Result<ActiveProfileReferenceState, String> {
    match state {
        ActiveProfileReferenceState::Selected(reference)
            if reference.engine_id == engine_id && reference.profile_id == profile_id =>
        {
            Ok(ActiveProfileReferenceState::Selected(reference))
        }
        other => Err(format!(
            "active reference did not read back {}/{}: {other:?}",
            engine_id.as_str(),
            profile_id.as_str()
        )),
    }
}

async fn require_active_reference(
    references: &dyn ActiveProfileReferenceStore,
    engine_id: EngineId,
    profile_id: ProfileId,
    revision: u64,
) -> Result<(), String> {
    let state = references
        .read()
        .await
        .map_err(|error| format!("read durable active reference: {error}"))?;
    let ActiveProfileReferenceState::Selected(reference) =
        selected_profile(state, engine_id.clone(), profile_id.clone())?
    else {
        unreachable!("selected_profile returns a selected reference")
    };
    if reference.revision != ActiveProfileReferenceRevision::new(revision) {
        return Err(format!(
            "active reference revision was {}, expected {revision}",
            reference.revision.value()
        ));
    }
    Ok(())
}

async fn compose_owner(
    selected: SelectedActiveProfileBackend,
    applications: &mut Vec<Arc<CoreApplication>>,
    sing_box_provider: &SingBoxEngineBackendProvider,
    sing_box_store: Arc<dyn ProfileStore>,
) -> Result<Arc<CoreApplication>, String> {
    let backend = selected.backend.clone();
    let application = Arc::new(CoreApplication::new(
        backend.process(),
        backend.readiness(),
        tokio_application_runtime()
            .map_err(|error| format!("construct application runtime: {error}"))?,
    ));

    let bind_candidate = async {
        selected
            .install_core_capabilities(&application)
            .map_err(|error| error.to_string())?;
        backend
            .bind_owner_core_scope(Arc::new(application.as_ref().clone()))
            .map_err(|error| error.to_string())?;
        if backend.engine_id() == &EngineId::sing_box() {
            sing_box_provider
                .bind_profile_configuration_transaction(
                    &backend,
                    sing_box_store,
                    Arc::new(application.as_ref().clone()),
                )
                .map_err(|error| error.to_string())?;
        }
        Ok::<(), String>(())
    }
    .await;
    if let Err(error) = bind_candidate {
        let cleanup = selected
            .discard_application_candidate(&application)
            .await
            .map_err(|cleanup| format!("discard unbound candidate: {cleanup}"));
        return match cleanup {
            Ok(()) => Err(error),
            Err(cleanup) => Err(format!("{error}; {cleanup}")),
        };
    }
    selected
        .install_owner_binding_for_application(&application)
        .await
        .map_err(|error| format!("install selected Core owner binding: {error}"))?;
    applications.push(Arc::clone(&application));
    Ok(application)
}

async fn start_owner(application: &CoreApplication) -> Result<(), String> {
    CoreLifecyclePort::start(application)
        .await
        .map_err(|error| format!("start selected Core owner: {error}"))?;
    match application.snapshot().lifecycle {
        CoreLifecycle::Ready | CoreLifecycle::Running => Ok(()),
        lifecycle => Err(format!(
            "Core start returned without a ready lifecycle snapshot: {lifecycle:?}"
        )),
    }
}

async fn close_applications(applications: &[Arc<CoreApplication>]) -> Result<(), String> {
    let mut failures = Vec::new();
    for application in applications.iter().rev() {
        let snapshot = application.snapshot().lifecycle_snapshot();
        if snapshot.lifecycle == CoreLifecycle::Stopped && snapshot.session_token.is_none() {
            continue;
        }
        if let Err(error) = application.close().await {
            failures.push(error.to_string());
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Core owner cleanup failed: {}",
            failures.join("; ")
        ))
    }
}

async fn run_owner_switch_smoke(
    home: &TempDir,
    applications: &mut Vec<Arc<CoreApplication>>,
) -> Result<(), String> {
    let mihomo_binary = PathBuf::from(required_environment("MIHOMO_REAL_BINARY")?);
    let sing_box_binary = PathBuf::from(required_environment("SINGBOX_REAL_BINARY")?);
    let expected_mihomo_version = required_environment("MIHOMO_REAL_VERSION")?;
    let expected_sing_box_version = required_environment("SINGBOX_REAL_VERSION")?;
    if expected_mihomo_version != "v1.19.32" || expected_sing_box_version != "1.14.3" {
        return Err(format!(
            "owner-switch smoke requires locked Mihomo v1.19.32 and sing-box 1.14.3, got {expected_mihomo_version} and {expected_sing_box_version}"
        ));
    }

    let configs_root = home.path().join("configs");
    let mihomo_manager = ConfigManager::with_profile_codec_and_store_at_root(
        configs_root.clone(),
        SmokeSecureStore,
        Arc::new(MihomoProfileDocumentCodec),
        ProfileDocumentExtension::Yaml,
    )
    .map_err(|error| format!("construct Mihomo profile store: {error}"))?;
    let sing_box_manager = ConfigManager::with_profile_codec_and_store_at_root(
        configs_root,
        SmokeSecureStore,
        Arc::new(SingBoxProfileDocumentCodec),
        ProfileDocumentExtension::Json,
    )
    .map_err(|error| format!("construct sing-box profile store: {error}"))?;

    mihomo_manager
        .initialize_profile_namespace()
        .await
        .map_err(|error| format!("initialize Mihomo profile namespace: {error}"))?;
    sing_box_manager
        .initialize_profile_namespace()
        .await
        .map_err(|error| format!("initialize sing-box profile namespace: {error}"))?;

    let profile_id = ProfileId::new("primary").map_err(|error| error.to_string())?;
    let mihomo_endpoint = loopback_endpoint()?;
    let sing_box_endpoint = loopback_endpoint()?;
    let mihomo_document = format!(
        "port: 0\nsocks-port: 0\nredir-port: 0\ntproxy-port: 0\nmixed-port: 0\nallow-lan: false\nmode: rule\nlog-level: error\nipv6: false\nexternal-controller: {}\ntun:\n  enable: false\nproxies: []\nproxy-groups: []\nrules:\n  - MATCH,DIRECT\n",
        mihomo_endpoint.trim_start_matches("http://")
    );
    let sing_box_document = format!(
        r#"{{"log":{{"level":"error"}},"inbounds":[],"outbounds":[{{"type":"direct","tag":"direct-a"}},{{"type":"direct","tag":"direct-b"}},{{"type":"selector","tag":"selector","outbounds":["direct-a","direct-b"],"default":"direct-a"}}],"experimental":{{"clash_api":{{"external_controller":"{}"}}}}}}"#,
        sing_box_endpoint.trim_start_matches("http://")
    );
    ProfileStore::save_inactive_profile(
        mihomo_manager.engine_profile_store(),
        profile_id.as_str(),
        &mihomo_document,
    )
    .await
    .map_err(|error| format!("save Mihomo smoke profile: {error}"))?;
    ProfileStore::save_inactive_profile(
        sing_box_manager.engine_profile_store(),
        profile_id.as_str(),
        &sing_box_document,
    )
    .await
    .map_err(|error| format!("save sing-box smoke profile: {error}"))?;

    let mihomo_store = mihomo_manager.shared_engine_profile_store();
    let sing_box_store = sing_box_manager.shared_engine_profile_store();
    let mihomo_profile_path = mihomo_manager
        .existing_profile_document_path(profile_id.as_str())
        .await
        .map_err(|error| format!("resolve Mihomo profile path: {error}"))?;
    let sing_box_profile_path = sing_box_manager
        .existing_profile_document_path(profile_id.as_str())
        .await
        .map_err(|error| format!("resolve sing-box profile path: {error}"))?;

    let initial_reference = ActiveProfileReferenceStore::read(mihomo_store.as_ref())
        .await
        .map_err(|error| format!("read initial active profile reference: {error}"))?;
    let selected_reference = ActiveProfileReferenceStore::compare_and_set(
        mihomo_store.as_ref(),
        initial_reference.revision(),
        ActiveProfileReferenceTarget::selected(mihomo_manager.engine_id(), profile_id.clone()),
        ActiveProfileReferenceWriteAuthorization::Unowned,
    )
    .await
    .map_err(|error| format!("select initial Mihomo profile: {error}"))?;
    selected_profile(
        selected_reference,
        mihomo_manager.engine_id(),
        profile_id.clone(),
    )?;

    let mihomo_controller = MihomoClient::new(&mihomo_endpoint, None)
        .map_err(|error| format!("construct Mihomo controller: {error}"))?;
    let mihomo_process = ProcessCoreController::with_pid_file(
        mihomo_binary,
        mihomo_profile_path,
        home.path().join("mihomo.pid"),
    );
    let mihomo_provider = Arc::new(
        MihomoEngineBackendProvider::new(
            Arc::new(mihomo_process),
            Arc::new(ControllerReadiness::new(mihomo_endpoint, None)),
            mihomo_controller,
        )
        .map_err(|error| format!("construct production Mihomo provider: {error}"))?,
    );
    let sing_box_controller =
        SingBoxControllerConfiguration::from_profile_json(sing_box_document.as_bytes())
            .map_err(|error| format!("inspect sing-box controller configuration: {error}"))?;
    let sing_box_provider = Arc::new(
        SingBoxEngineBackendProvider::with_tokio(
            sing_box_binary,
            sing_box_profile_path,
            CoreInstanceId::new("owner-switch-real-sing-box").map_err(|error| error.to_string())?,
            sing_box_controller,
        )
        .map_err(|error| format!("construct production sing-box provider: {error}"))?,
    );

    let start = CoreOperation::Lifecycle(CoreLifecycleOperation::Start);
    let mihomo_capabilities = Arc::new(MihomoCoreCapabilityProvider::new(
        HostKind::Desktop,
        [start],
    ));
    let sing_box_capabilities = Arc::new(SingBoxCoreCapabilityProvider::new(
        HostKind::Desktop,
        [start],
    ));
    let capability_providers: Vec<(EngineId, Arc<dyn CoreCapabilityProvider>)> = vec![
        (
            EngineId::new("mihomo").map_err(|error| error.to_string())?,
            mihomo_capabilities,
        ),
        (EngineId::sing_box(), sing_box_capabilities),
    ];
    let capabilities = Arc::new(
        EngineCoreCapabilityProviderRegistry::new(capability_providers)
            .map_err(|error| format!("construct dual-engine capability registry: {error}"))?,
    );
    let sing_box_dispatch_provider: Arc<dyn EngineBackendProvider> = Arc::new(
        RecoveringSingBoxProvider::new(Arc::clone(&sing_box_provider)),
    );
    let providers: Vec<Arc<dyn EngineBackendProvider>> =
        vec![mihomo_provider, sing_box_dispatch_provider];
    let factory = Arc::new(
        EngineBackendFactoryRegistry::new(providers)
            .map_err(|error| format!("construct dual-engine backend registry: {error}"))?,
    );
    let references: Arc<dyn ActiveProfileReferenceStore> = mihomo_store.clone();
    let mihomo_store_port: Arc<dyn ProfileStore> = mihomo_store;
    let sing_box_store_port: Arc<dyn ProfileStore> = sing_box_store.clone();
    let profile_stores: Vec<Arc<dyn ProfileStore>> = vec![
        Arc::clone(&mihomo_store_port),
        Arc::clone(&sing_box_store_port),
    ];
    let selector = ActiveProfileBackendSelector::new(
        Arc::clone(&references),
        profile_stores,
        factory,
        capabilities,
    );

    let selected = selector
        .select_backend(&PreviousCoreOwner::Unowned)
        .await
        .map_err(|error| format!("select initial Mihomo owner: {error}"))?;
    let mihomo_application = compose_owner(
        selected,
        applications,
        sing_box_provider.as_ref(),
        Arc::clone(&sing_box_store_port),
    )
    .await?;
    start_owner(&mihomo_application).await?;
    require_active_reference(
        references.as_ref(),
        EngineId::new("mihomo").map_err(|error| error.to_string())?,
        profile_id.clone(),
        1,
    )
    .await?;

    let sing_box_document = ProfileStore::load_workspace(sing_box_store_port.as_ref(), &profile_id)
        .await
        .map_err(|error| format!("load exact sing-box target document: {error}"))?
        .document;
    let target_sing_box =
        ActiveProfileReferenceTarget::selected(EngineId::sing_box(), profile_id.clone());
    let mihomo_owner: Arc<dyn CoreOwnerTerminationPort> = mihomo_application.clone();
    let first_switch = selector
        .replace_owner_for_profile(
            target_sing_box.clone(),
            &sing_box_document,
            &PreviousCoreOwner::Owned(mihomo_owner),
        )
        .await;
    let mihomo_terminal = mihomo_application.snapshot().lifecycle_snapshot();
    if mihomo_terminal.lifecycle != CoreLifecycle::Stopped
        || mihomo_terminal.session_token.is_some()
    {
        return Err(format!(
            "Mihomo owner did not produce a terminal readback: {mihomo_terminal:?}"
        ));
    }
    match first_switch {
        Err(error) if error.phase() == ActiveProfileBackendSwitchFailurePhase::OldOwnerClosed => {
            if !error
                .to_string()
                .contains("injected one-time sing-box candidate dispatch failure")
            {
                return Err(format!("unexpected recovered switch failure: {error}"));
            }
        }
        Err(error) => {
            return Err(format!(
                "injected sing-box dispatch failure reached the wrong typed phase: {error:?}"
            ));
        }
        Ok(candidate) => {
            candidate
                .abort_before_core_application()
                .await
                .map_err(|error| format!("abort unexpected sing-box candidate: {error}"))?;
            return Err("the first sing-box candidate dispatch did not fail as injected".into());
        }
    }
    require_active_reference(
        references.as_ref(),
        EngineId::sing_box(),
        profile_id.clone(),
        2,
    )
    .await?;
    match references
        .recover_owner_switch()
        .await
        .map_err(|error| format!("read durable owner-switch recovery state: {error}"))?
    {
        ActiveProfileOwnerSwitchRecovery::NoPendingSwitch => {}
        _ => return Err("failed candidate dispatch left an owner-switch journal pending".into()),
    }
    let sing_box_selected = selector
        .resume_owner_switch_for_profile(target_sing_box, &sing_box_document, &mihomo_terminal)
        .await
        .map_err(|error| {
            format!("recover sing-box selection from old terminal readback: {error}")
        })?;
    let sing_box_application = compose_owner(
        sing_box_selected,
        applications,
        sing_box_provider.as_ref(),
        Arc::clone(&sing_box_store_port),
    )
    .await?;
    start_owner(&sing_box_application).await?;

    let mihomo_document =
        ProfileStore::load_workspace(mihomo_store_port.as_ref(), &profile_id).await;
    let mihomo_document = mihomo_document
        .map_err(|error| format!("load exact Mihomo target document: {error}"))?
        .document;
    let sing_box_owner: Arc<dyn CoreOwnerTerminationPort> = sing_box_application.clone();
    let mihomo_selected = selector
        .replace_owner_for_profile(
            ActiveProfileReferenceTarget::selected(
                EngineId::new("mihomo").map_err(|error| error.to_string())?,
                profile_id.clone(),
            ),
            &mihomo_document,
            &PreviousCoreOwner::Owned(sing_box_owner),
        )
        .await
        .map_err(|error| format!("switch live sing-box owner back to Mihomo: {error}"))?;
    let sing_box_terminal = sing_box_application.snapshot().lifecycle_snapshot();
    if sing_box_terminal.lifecycle != CoreLifecycle::Stopped
        || sing_box_terminal.session_token.is_some()
    {
        return Err(format!(
            "sing-box owner did not produce a terminal readback: {sing_box_terminal:?}"
        ));
    }
    require_active_reference(
        references.as_ref(),
        EngineId::new("mihomo").map_err(|error| error.to_string())?,
        profile_id.clone(),
        3,
    )
    .await?;
    let final_application = compose_owner(
        mihomo_selected,
        applications,
        sing_box_provider.as_ref(),
        Arc::clone(&sing_box_store_port),
    )
    .await?;
    start_owner(&final_application).await?;

    Ok(())
}

#[tokio::test]
#[ignore = "requires SHA-locked Mihomo v1.19.32 and sing-box v1.14.3 real binaries"]
async fn locked_real_binaries_switch_live_owner_mihomo_sing_box_mihomo() {
    let mut applications = Vec::new();
    let home = TempDir::new().expect("create smoke home");
    let result = run_owner_switch_smoke(&home, &mut applications).await;
    let cleanup = close_applications(&applications).await;
    match (result, cleanup) {
        (Ok(()), Ok(())) => {}
        (Err(error), Ok(())) => panic!("live owner-switch smoke failed: {error}"),
        (Ok(()), Err(error)) => panic!("live owner-switch cleanup failed: {error}"),
        (Err(error), Err(cleanup)) => panic!("{error}; {cleanup}"),
    }
}

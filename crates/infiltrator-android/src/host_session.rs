//! Core application lifecycle: shared [`CoreApplication`] wiring, adoption of an
//! already-running core, the restart-with-readiness path, and the
//! apply-current-profile transaction with rollback.

use crate::runtime::AndroidBridgeAdapter;
use infiltrator_application::android_control_application::AndroidControlApplication;
use infiltrator_application::android_core_owner_recovery_application::AndroidCoreOwnerRecoveryApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::core_capability_application::CoreCapabilityApplication;
use infiltrator_application::core_owner_shutdown_application::CoreOwnerShutdownApplication;
use infiltrator_application::vpn_application::VpnServiceApplication;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceWriteAuthorization;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceState,
};
use infiltrator_contract::adapter_operation_declaration::AdapterOperationOwner;
use infiltrator_contract::android_control::{
    AndroidCoreProcessObservation, AndroidHostProcessRole,
};
use infiltrator_contract::core_capabilities::{
    CapabilityAccess, CoreCapabilityKey, CoreCapabilityStatus, CoreLifecycleOperation,
    CoreOperation, EngineCapabilityDecision,
};
use infiltrator_contract::core_identity::{CoreGeneration, CoreIdentity, CoreInstanceId};
use infiltrator_contract::core_process::CoreProcessStartContext;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity;
use infiltrator_core::apply::{
    ApplyError, ApplyParams, EndpointConfigReloader, apply_active_profile_reference,
};
use infiltrator_core::apply_workspace::apply_active_profile_workspace;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_platform::android_bridge::{AndroidBridge, get_android_bridge};
use infiltrator_platform::defaults::DefaultCredentialStore;
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::core_capability_provider::CoreCapabilityProvider;
use infiltrator_ports::endpoint::EndpointSource;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use infiltrator_ports::runtime_gateway::ActiveProfileReferenceApplier;
use mihomo_config::endpoint::ProfileEndpointSource;
use mihomo_config::manager::ConfigManager;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

use crate::composition::{
    AndroidCoreComposition, android_core_capability_provider,
    android_core_lifecycle_capability_provider, core_application_with_readiness_and_provider,
};
use crate::core_launch_spec::AndroidProfileReadiness;
use crate::ffi::{FfiErrorCode, FfiStatus};
use crate::host_support::{build_config_manager, map_mihomo_error, map_port_error};

#[path = "host_session/singbox.rs"]
mod singbox;
#[path = "host_session/switch.rs"]
pub(crate) mod switch;

/// Shared frontend wiring for the mihomo core: the application service plus
/// the [`ConfigManager`] endpoint source it resolves against. Lazily
/// constructed on first use; the controller resolves the Android bridge
/// dynamically on every call, so a bridge re-registration does not strand
/// this slot.
pub(crate) struct SharedCore {
    pub(crate) application: CoreApplication,
    pub(crate) control: AndroidControlApplication,
    pub(crate) engine: SharedCoreEngine,
    config: Arc<ConfigManager<DefaultCredentialStore>>,
    pub(crate) _watchdog: infiltrator_composition::CoreWatchdogHandle,
}

pub(crate) enum SharedCoreEngine {
    Mihomo { endpoints: Arc<dyn EndpointSource> },
    SingBox,
}

enum SharedCoreOwnerState {
    Uninitialized,
    Recovering,
    Ready,
    Active(Arc<SharedCore>),
    Switching(Arc<SharedCore>),
    RecoveryRequired,
    Closing(Arc<SharedCore>),
    ClosingWithoutCore,
    Closed,
}

enum SharedCoreOwnerAdmission<'a> {
    Existing(&'a Arc<SharedCore>),
    Build,
    RecoveryRequired,
    Switching,
    Closing,
}

enum SharedCoreOwnerSwitchCompletion {
    Installed(Arc<SharedCore>),
    RestorePrevious,
    RecoveryRequired,
}

enum SharedCoreOwnerSwitchFinish {
    Installed,
    Restored,
    RecoveryRequired,
    OwnerStateChanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SharedCoreOwnerSessionBegin {
    NewSession,
    AlreadyReady,
    RecoveryInProgress,
    RetryClose,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SharedCoreOwnerSessionCompletion {
    Ready,
    AlreadyReady,
    RecoveryNotStarted,
    RecoveryRequired,
    Closing,
}

fn begin_owner_session_transition(state: &mut SharedCoreOwnerState) -> SharedCoreOwnerSessionBegin {
    match state {
        SharedCoreOwnerState::Uninitialized
        | SharedCoreOwnerState::RecoveryRequired
        | SharedCoreOwnerState::Closed => {
            *state = SharedCoreOwnerState::Recovering;
            SharedCoreOwnerSessionBegin::NewSession
        }
        SharedCoreOwnerState::Recovering => SharedCoreOwnerSessionBegin::RecoveryInProgress,
        SharedCoreOwnerState::Ready | SharedCoreOwnerState::Active(_) => {
            SharedCoreOwnerSessionBegin::AlreadyReady
        }
        SharedCoreOwnerState::Switching(_) => SharedCoreOwnerSessionBegin::RecoveryInProgress,
        SharedCoreOwnerState::Closing(_) | SharedCoreOwnerState::ClosingWithoutCore => {
            SharedCoreOwnerSessionBegin::RetryClose
        }
    }
}

fn complete_owner_session_transition(
    state: &mut SharedCoreOwnerState,
) -> SharedCoreOwnerSessionCompletion {
    match state {
        SharedCoreOwnerState::Recovering => {
            *state = SharedCoreOwnerState::Ready;
            SharedCoreOwnerSessionCompletion::Ready
        }
        SharedCoreOwnerState::Ready | SharedCoreOwnerState::Active(_) => {
            SharedCoreOwnerSessionCompletion::AlreadyReady
        }
        SharedCoreOwnerState::Uninitialized => SharedCoreOwnerSessionCompletion::RecoveryNotStarted,
        SharedCoreOwnerState::Switching(_) => SharedCoreOwnerSessionCompletion::Closing,
        SharedCoreOwnerState::RecoveryRequired => {
            SharedCoreOwnerSessionCompletion::RecoveryRequired
        }
        SharedCoreOwnerState::Closing(_)
        | SharedCoreOwnerState::ClosingWithoutCore
        | SharedCoreOwnerState::Closed => SharedCoreOwnerSessionCompletion::Closing,
    }
}

fn shared_core_owner_admission(state: &SharedCoreOwnerState) -> SharedCoreOwnerAdmission<'_> {
    match state {
        SharedCoreOwnerState::Active(core) => SharedCoreOwnerAdmission::Existing(core),
        SharedCoreOwnerState::Ready => SharedCoreOwnerAdmission::Build,
        SharedCoreOwnerState::Uninitialized
        | SharedCoreOwnerState::Recovering
        | SharedCoreOwnerState::RecoveryRequired => SharedCoreOwnerAdmission::RecoveryRequired,
        SharedCoreOwnerState::Switching(_) => SharedCoreOwnerAdmission::Switching,
        SharedCoreOwnerState::Closing(_)
        | SharedCoreOwnerState::ClosingWithoutCore
        | SharedCoreOwnerState::Closed => SharedCoreOwnerAdmission::Closing,
    }
}

fn begin_shared_core_switch(
    state: &mut SharedCoreOwnerState,
) -> Result<Arc<SharedCore>, SharedCoreOwnerAdmission<'static>> {
    match state {
        SharedCoreOwnerState::Active(core) => {
            let previous = Arc::clone(core);
            *state = SharedCoreOwnerState::Switching(Arc::clone(&previous));
            Ok(previous)
        }
        SharedCoreOwnerState::Switching(_) => Err(SharedCoreOwnerAdmission::Switching),
        SharedCoreOwnerState::Uninitialized
        | SharedCoreOwnerState::Recovering
        | SharedCoreOwnerState::RecoveryRequired => Err(SharedCoreOwnerAdmission::RecoveryRequired),
        SharedCoreOwnerState::Ready => Err(SharedCoreOwnerAdmission::Build),
        SharedCoreOwnerState::Closing(_)
        | SharedCoreOwnerState::ClosingWithoutCore
        | SharedCoreOwnerState::Closed => Err(SharedCoreOwnerAdmission::Closing),
    }
}

fn finish_shared_core_switch(
    state: &mut SharedCoreOwnerState,
    previous: &Arc<SharedCore>,
    completion: SharedCoreOwnerSwitchCompletion,
) -> SharedCoreOwnerSwitchFinish {
    match state {
        SharedCoreOwnerState::Switching(current) if Arc::ptr_eq(current, previous) => {
            match completion {
                SharedCoreOwnerSwitchCompletion::Installed(candidate) => {
                    *state = SharedCoreOwnerState::Active(candidate);
                    SharedCoreOwnerSwitchFinish::Installed
                }
                SharedCoreOwnerSwitchCompletion::RestorePrevious => {
                    *state = SharedCoreOwnerState::Active(Arc::clone(previous));
                    SharedCoreOwnerSwitchFinish::Restored
                }
                SharedCoreOwnerSwitchCompletion::RecoveryRequired => {
                    *state = SharedCoreOwnerState::RecoveryRequired;
                    SharedCoreOwnerSwitchFinish::RecoveryRequired
                }
            }
        }
        SharedCoreOwnerState::Switching(_) => SharedCoreOwnerSwitchFinish::OwnerStateChanged,
        _ => SharedCoreOwnerSwitchFinish::OwnerStateChanged,
    }
}

fn shared_core_owner_slot() -> &'static Mutex<SharedCoreOwnerState> {
    static SHARED_CORE: OnceLock<Mutex<SharedCoreOwnerState>> = OnceLock::new();
    SHARED_CORE.get_or_init(|| Mutex::new(SharedCoreOwnerState::Uninitialized))
}

pub(crate) async fn shared_core() -> Result<Arc<SharedCore>, FfiStatus> {
    let bridge = get_android_bridge().ok_or_else(|| {
        FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android host bridge is not registered",
        )
    })?;
    if bridge.process_role() != AndroidHostProcessRole::CoreOwner {
        return Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "CoreApplication is owned by the Android :vpn process",
        ));
    }

    let mut state = shared_core_owner_slot().lock().await;
    match shared_core_owner_admission(&state) {
        SharedCoreOwnerAdmission::Existing(core) => return Ok(Arc::clone(core)),
        SharedCoreOwnerAdmission::RecoveryRequired => {
            return Err(FfiStatus::err(
                FfiErrorCode::NotReady,
                "Android Core owner startup recovery has not completed",
            ));
        }
        SharedCoreOwnerAdmission::Build => {}
        SharedCoreOwnerAdmission::Closing => {
            return Err(FfiStatus::err(
                FfiErrorCode::NotReady,
                "Android Core owner session is closing or closed",
            ));
        }
        SharedCoreOwnerAdmission::Switching => {
            return Err(FfiStatus::err(
                FfiErrorCode::NotReady,
                "Android Core owner is switching engine; retry after owner readback",
            ));
        }
    }
    let core = build_shared_core(bridge).await?;
    *state = SharedCoreOwnerState::Active(Arc::clone(&core));
    Ok(core)
}

pub(crate) async fn begin_shared_core_owner_session()
-> Result<SharedCoreOwnerSessionBegin, FfiStatus> {
    let mut state = shared_core_owner_slot().lock().await;
    Ok(begin_owner_session_transition(&mut state))
}

pub(crate) async fn complete_shared_core_owner_session() -> Result<(), FfiStatus> {
    let mut state = shared_core_owner_slot().lock().await;
    match complete_owner_session_transition(&mut state) {
        SharedCoreOwnerSessionCompletion::Ready
        | SharedCoreOwnerSessionCompletion::AlreadyReady => Ok(()),
        SharedCoreOwnerSessionCompletion::RecoveryNotStarted => Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android Core owner session cannot become ready before startup recovery",
        )),
        SharedCoreOwnerSessionCompletion::RecoveryRequired => Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android Core owner recovery is required before the session can become ready",
        )),
        SharedCoreOwnerSessionCompletion::Closing => Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android Core owner session is closing",
        )),
    }
}

pub(crate) async fn recover_android_owner_before_binding(
    bridge: Arc<dyn AndroidBridge>,
    vpn: VpnServiceApplication,
) -> Result<(), FfiStatus> {
    let config = Arc::new(build_config_manager().await?);
    let observation = bridge
        .core_process_observation()
        .await
        .map_err(map_port_error)?;
    let identity = match &observation {
        AndroidCoreProcessObservation::Running { owned }
        | AndroidCoreProcessObservation::Exited { owned, .. } => match &owned.context {
            CoreProcessStartContext::Managed { identity, .. } => identity.clone(),
            CoreProcessStartContext::Standalone { .. } => {
                return Err(FfiStatus::err(
                    FfiErrorCode::NotReady,
                    "Android Core recovery cannot bind a standalone child",
                ));
            }
        },
        AndroidCoreProcessObservation::NeverStarted
        | AndroidCoreProcessObservation::Unavailable { .. } => {
            let state = ProfileStore::read_active_profile_reference(config.engine_profile_store())
                .await
                .map_err(|error| {
                    FfiStatus::err(
                        FfiErrorCode::NotReady,
                        format!("Android active profile reference is unavailable: {error}"),
                    )
                })?;
            let engine_id = match state {
                ActiveProfileReferenceState::Selected(reference) => reference.engine_id,
                ActiveProfileReferenceState::Unselected { .. } => config.engine_id(),
            };
            CoreIdentity::new(
                engine_id,
                CoreInstanceId::new("android-active")
                    .map_err(|error| FfiStatus::err(FfiErrorCode::Unknown, error.to_string()))?,
                None,
            )
        }
    };
    let process = Arc::new(AndroidBridgeAdapter::new(Arc::clone(&bridge)));
    let owner_store: Arc<dyn ActiveProfileReferenceStore> = config.shared_engine_profile_store();
    AndroidCoreOwnerRecoveryApplication::new(process, owner_store, vpn, identity)
        .recover()
        .await
        .map_err(|failure| {
            FfiStatus::err(
                FfiErrorCode::NotReady,
                format!("Android Core owner recovery failed: {}", failure.message),
            )
        })?;
    config
        .migrate_legacy_mihomo_selection()
        .await
        .map_err(|error| {
            FfiStatus::err(
                FfiErrorCode::NotReady,
                format!("Android active profile reference is unavailable after recovery: {error}"),
            )
        })?;
    Ok(())
}

pub(crate) async fn close_shared_core_owner(vpn: VpnServiceApplication) -> Result<(), FfiStatus> {
    let mut state = shared_core_owner_slot().lock().await;
    let core = match &*state {
        SharedCoreOwnerState::Uninitialized
        | SharedCoreOwnerState::Recovering
        | SharedCoreOwnerState::Ready
        | SharedCoreOwnerState::RecoveryRequired
        | SharedCoreOwnerState::ClosingWithoutCore => None,
        SharedCoreOwnerState::Switching(_) => {
            return Err(FfiStatus::err(
                FfiErrorCode::NotReady,
                "Android Core owner cannot close during profile owner replacement",
            ));
        }
        SharedCoreOwnerState::Active(core) | SharedCoreOwnerState::Closing(core) => {
            Some(Arc::clone(core))
        }
        SharedCoreOwnerState::Closed => return Ok(()),
    };
    match core {
        None => {
            *state = SharedCoreOwnerState::ClosingWithoutCore;
            vpn.stop()
                .await
                .map_err(|failure| FfiStatus::err(FfiErrorCode::NotReady, failure.message))?;
            *state = SharedCoreOwnerState::Closed;
            Ok(())
        }
        Some(core) => {
            *state = SharedCoreOwnerState::Closing(Arc::clone(&core));
            CoreOwnerShutdownApplication::new(core.application.clone(), vpn)
                .close()
                .await
                .map_err(|failure| FfiStatus::err(FfiErrorCode::NotReady, failure.message))?;
            *state = SharedCoreOwnerState::Closed;
            Ok(())
        }
    }
}

async fn build_shared_core(bridge: Arc<dyn AndroidBridge>) -> Result<Arc<SharedCore>, FfiStatus> {
    let config = Arc::new(build_config_manager().await?);
    config
        .migrate_legacy_mihomo_selection()
        .await
        .map_err(|error| {
            FfiStatus::err(
                FfiErrorCode::NotReady,
                format!("Android legacy Mihomo selection migration failed: {error}"),
            )
        })?;
    let profile_store = config.engine_profile_store();
    let selection_lease =
        ActiveProfileReferenceStore::acquire_provider_selection_lease(profile_store)
            .await
            .map_err(|error| {
                FfiStatus::err(
                    FfiErrorCode::NotReady,
                    format!("Android active Core selection is unavailable: {error}"),
                )
            })?;
    let reference = match selection_lease.state() {
        ActiveProfileReferenceState::Unselected { .. } => {
            return Err(FfiStatus::err(
                FfiErrorCode::NotReady,
                "Android has no product-wide active profile reference",
            ));
        }
        ActiveProfileReferenceState::Selected(reference) => reference.clone(),
    };
    match reference.engine_id.as_str() {
        "mihomo" => build_mihomo_shared_core(bridge, config, reference, selection_lease).await,
        "sing-box" => singbox::build_shared_core(bridge, config, reference, selection_lease).await,
        engine => Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            format!("Android has no registered Core backend for engine `{engine}`"),
        )),
    }
}

async fn build_mihomo_shared_core(
    bridge: Arc<dyn AndroidBridge>,
    config: Arc<ConfigManager<DefaultCredentialStore>>,
    reference: ActiveProfileReference,
    selection_lease: Box<
        dyn infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceLease,
    >,
) -> Result<Arc<SharedCore>, FfiStatus> {
    if reference.engine_id != config.engine_id() {
        return Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android Mihomo owner received a foreign active profile reference",
        ));
    }
    authorize_android_core_start(
        android_core_lifecycle_capability_provider(),
        &reference.engine_id,
    )?;
    let endpoints = Arc::new(ProfileEndpointSource::new(Arc::clone(&config)));
    let workspace =
        ProfileStore::load_workspace(config.engine_profile_store(), &reference.profile_id)
            .await
            .map_err(|error| {
                FfiStatus::err(
                    FfiErrorCode::Config,
                    format!("Android active profile document is unavailable: {error}"),
                )
            })?;
    if workspace.document.identity.engine_id != reference.engine_id
        || workspace.document.identity.profile_id != reference.profile_id
    {
        return Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android active profile reference does not match its persisted document",
        ));
    }
    let initial_endpoint = endpoints
        .resolve_profile(&reference.profile_id)
        .await
        .map_err(|error| {
            FfiStatus::err(
                FfiErrorCode::Config,
                format!("Android active Core endpoint is unavailable: {error}"),
            )
        })?;
    let initial_capability_provider =
        android_core_capability_provider(&initial_endpoint.url, initial_endpoint.secret.clone());
    authorize_android_core_start(
        Arc::clone(&initial_capability_provider),
        &reference.engine_id,
    )?;

    // All provider, profile, and target capability checks pass before this
    // profile-specific configuration write.
    config
        .ensure_external_controller_for_profile(&reference.profile_id)
        .await
        .map_err(map_mihomo_error)?;
    let endpoint = endpoints
        .resolve_profile(&reference.profile_id)
        .await
        .map_err(|error| {
            FfiStatus::err(
                FfiErrorCode::Config,
                format!("Android active Core endpoint is unavailable: {error}"),
            )
        })?;
    let capability_provider =
        android_core_capability_provider(&endpoint.url, endpoint.secret.clone());
    authorize_android_core_start(Arc::clone(&capability_provider), &reference.engine_id)?;
    let controller_url = endpoint.url;
    let controller_secret = endpoint.secret;
    let application = core_application_with_readiness_and_provider(
        bridge,
        controller_url,
        controller_secret,
        AndroidCoreComposition::new(Arc::clone(&config)),
        Arc::new(AndroidProfileReadiness::new(Arc::clone(&endpoints))),
        capability_provider,
    )
    .map_err(|error| FfiStatus::err(FfiErrorCode::Unknown, error))?;
    let owner_identity = application.snapshot().identity.ok_or_else(|| {
        FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android Core backend has no typed owner identity",
        )
    })?;
    if owner_identity.engine != reference.engine_id {
        return Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android Core backend identity does not match the active profile engine",
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
                format!("Android Core owner could not bind the active profile: {error}"),
            )
        })?;
    application
        .install_active_profile_owner_binding(owner_binding)
        .map_err(|failure| FfiStatus::from(InfiltratorError::Backend(failure.message)))?;
    let control = AndroidControlApplication::new(application.clone());
    let watchdog = infiltrator_composition::spawn_core_watchdog(Arc::new(application.clone()));
    Ok(Arc::new(SharedCore {
        application,
        control,
        engine: SharedCoreEngine::Mihomo {
            endpoints: endpoints as Arc<dyn EndpointSource>,
        },
        config,
        _watchdog: watchdog,
    }))
}

fn authorize_android_core_start(
    provider: Arc<dyn CoreCapabilityProvider>,
    engine_id: &infiltrator_contract::core_identity::EngineId,
) -> Result<(), FfiStatus> {
    let matrix = CoreCapabilityApplication::new(provider)
        .matrix_for(engine_id, None)
        .map_err(|failure| {
            FfiStatus::err(
                FfiErrorCode::NotReady,
                format!(
                    "Android Core capability matrix is unavailable: {}",
                    failure.message
                ),
            )
        })?;
    let resolution = matrix.resolve(CoreCapabilityKey::new(
        matrix.scope.clone(),
        CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
    ));
    match (resolution.status, resolution.engine) {
        (
            CoreCapabilityStatus::Available {
                access: CapabilityAccess::Supported,
                ..
            },
            Some(EngineCapabilityDecision::OwnerManaged {
                owner: AdapterOperationOwner::CoreLifecycle,
            }),
        ) => Ok(()),
        (status, _) => Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            format!("Android Core start is not evidenced on this target: {status:?}"),
        )),
    }
}

struct AndroidCoreProfileApplier {
    core: Arc<SharedCore>,
}

#[async_trait::async_trait]
impl ActiveProfileReferenceApplier for AndroidCoreProfileApplier {
    fn engine_id(&self) -> Option<infiltrator_contract::core_identity::EngineId> {
        infiltrator_ports::core_lifecycle::CoreLifecyclePort::core_identity(&self.core.application)
            .map(|identity| identity.engine)
    }

    fn core_lifecycle(&self) -> infiltrator_contract::snapshot::CoreLifecycle {
        infiltrator_ports::core_lifecycle::CoreLifecyclePort::lifecycle(&self.core.application)
    }

    fn write_authorization(
        &self,
    ) -> Result<
        infiltrator_contract::active_profile_reference::ActiveProfileReferenceWriteAuthorization,
        PortError,
    > {
        self.core
            .application
            .active_profile_reference_write_authorization()
            .map_err(PortError::Rejected)
    }

    async fn apply_active_profile_reference(
        &self,
        expected: &ActiveProfileReference,
        strategy: ApplyStrategy,
    ) -> Result<u64, PortError> {
        let endpoints = match &self.core.engine {
            SharedCoreEngine::Mihomo { endpoints } => Arc::clone(endpoints),
            SharedCoreEngine::SingBox => {
                return Err(PortError::unsupported(
                    infiltrator_contract::capability::Capability::Profiles,
                    "Android sing-box active-profile runtime apply is not connected",
                ));
            }
        };
        let core = Arc::clone(&self.core);
        let expected = expected.clone();
        tokio::spawn(async move {
            let reloader = EndpointConfigReloader::new(endpoints);
            let outcome = apply_active_profile_reference(
                &core.application,
                &core.config,
                &reloader,
                &expected,
                ApplyParams {
                    strategy,
                    ..ApplyParams::default()
                },
            )
            .await
            .map_err(android_apply_error)?;
            Ok(outcome.generation)
        })
        .await
        .map_err(|error| {
            PortError::Failed(format!(
                "Android selected-profile apply continuation failed: {error}"
            ))
        })?
    }

    async fn apply_active_profile_workspace(
        &self,
        expected_reference: &ActiveProfileReference,
        expected_workspace: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
        authorization: ActiveProfileReferenceWriteAuthorization,
        strategy: ApplyStrategy,
    ) -> Result<ProfileWorkspace, PortError> {
        let endpoints = match &self.core.engine {
            SharedCoreEngine::Mihomo { endpoints } => Arc::clone(endpoints),
            SharedCoreEngine::SingBox => {
                return Err(PortError::unsupported(
                    infiltrator_contract::capability::Capability::Profiles,
                    "Android sing-box active workspace apply is not connected",
                ));
            }
        };
        let core = Arc::clone(&self.core);
        let expected_reference = expected_reference.clone();
        let expected_workspace = expected_workspace.clone();
        let update = update.clone();
        tokio::spawn(async move {
            let reloader = EndpointConfigReloader::new(endpoints);
            apply_active_profile_workspace(
                &core.application,
                &core.config,
                &reloader,
                &expected_reference,
                &expected_workspace,
                &update,
                authorization,
                ApplyParams {
                    strategy,
                    ..ApplyParams::default()
                },
            )
            .await
            .map_err(android_apply_error)
        })
        .await
        .map_err(|error| {
            PortError::Failed(format!(
                "Android workspace apply continuation failed: {error}"
            ))
        })?
    }
}

fn android_apply_error(error: ApplyError) -> PortError {
    PortError::Rejected(infiltrator_core::apply::failure_from_apply_error(error))
}

/// Apply the observed selected profile through the shared application use-case.
/// It validates that the full reference state remains current, then the Android
/// adapter holds the exact selection lease through native apply and rollback.
pub(crate) async fn apply_selected_profile_reference_status(
    expected: infiltrator_contract::active_profile_reference::ActiveProfileReferenceState,
) -> FfiStatus {
    let core = match shared_core().await {
        Ok(core) => core,
        Err(status) => return status,
    };
    if matches!(&core.engine, SharedCoreEngine::SingBox) {
        return FfiStatus::err(
            FfiErrorCode::NotSupported,
            "Android sing-box active-profile runtime apply is not connected",
        );
    }
    if let Err(failure) = core.application.adopt_if_running().await {
        return FfiStatus::from(InfiltratorError::Backend(failure.message));
    }
    let application = infiltrator_application::profile_application::ProfileApplication::new(
        core.config.shared_engine_profile_store(),
    );
    let applier: Arc<dyn ActiveProfileReferenceApplier> = Arc::new(AndroidCoreProfileApplier {
        core: Arc::clone(&core),
    });
    application
        .apply_selected_profile_reference(applier, &expected, ApplyStrategy::AlwaysRestart)
        .await
        .map(|outcome| {
            match outcome {
            infiltrator_application::profile_application::SelectedProfileApplyOutcome::Applied {
                ..
            } => FfiStatus::ok(),
            infiltrator_application::profile_application::SelectedProfileApplyOutcome::NotActive(
                infiltrator_contract::snapshot::CoreInactiveLifecycle::Stopped,
            ) => FfiStatus::err(
                FfiErrorCode::NotReady,
                "the selected profile was retained while Core remains stopped",
            ),
            infiltrator_application::profile_application::SelectedProfileApplyOutcome::NotActive(
                infiltrator_contract::snapshot::CoreInactiveLifecycle::Failed,
            ) => FfiStatus::err(
                FfiErrorCode::NotReady,
                "the selected profile was retained while Core is failed",
            ),
        }
        })
        .unwrap_or_else(crate::host_support::map_application_failure)
}

/// Activate an Android profile through the same application-level CAS and
/// conditional recovery path used by other managed runtimes.
pub(crate) async fn activate_profile_status(profile: &str) -> FfiStatus {
    let core = match shared_core().await {
        Ok(core) => core,
        Err(status) => return status,
    };
    if matches!(&core.engine, SharedCoreEngine::SingBox) {
        return FfiStatus::err(
            FfiErrorCode::NotSupported,
            "Android sing-box profile activation requires rebuilding its selected-document Core owner",
        );
    }
    if let Err(failure) = core.application.adopt_if_running().await {
        return FfiStatus::from(InfiltratorError::Backend(failure.message));
    }
    let application = infiltrator_application::profile_application::ProfileApplication::new(
        core.config.shared_engine_profile_store(),
    );
    let applier: Arc<dyn ActiveProfileReferenceApplier> = Arc::new(AndroidCoreProfileApplier {
        core: Arc::clone(&core),
    });
    let outcome = match application
        .activate_profile_with_applier(Arc::clone(&applier), profile)
        .await
    {
        Ok(outcome) => outcome,
        Err(failure) => return crate::host_support::map_application_failure(failure),
    };
    match outcome {
        infiltrator_application::profile_application::ProfileActivationOutcome::AlreadySelectedWithRuntime => {
            let expected = match ProfileStore::read_active_profile_reference(core.config.engine_profile_store()).await {
                Ok(expected) => expected,
                Err(error) => {
                    return FfiStatus::err(
                        FfiErrorCode::NotReady,
                        format!("active profile reference could not be read after selection: {error}"),
                    );
                }
            };
            if !matches!(
                &expected,
                infiltrator_contract::active_profile_reference::ActiveProfileReferenceState::Selected(reference)
                    if reference.engine_id == core.config.engine_id()
                        && reference.profile_id.as_str() == profile.trim()
            ) {
                return FfiStatus::err(
                    FfiErrorCode::NotReady,
                    "active profile changed after the requested selection was observed",
                );
            }
            application
                .apply_selected_profile_reference(
                    Arc::clone(&applier),
                    &expected,
                    ApplyStrategy::AlwaysRestart,
                )
                .await
                .map(|_| FfiStatus::ok())
                .unwrap_or_else(crate::host_support::map_application_failure)
        }
        infiltrator_application::profile_application::ProfileActivationOutcome::SelectedAndApplied => {
            FfiStatus::ok()
        }
        infiltrator_application::profile_application::ProfileActivationOutcome::AlreadySelectedWhileInactive(
            infiltrator_contract::snapshot::CoreInactiveLifecycle::Stopped,
        )
        | infiltrator_application::profile_application::ProfileActivationOutcome::SelectedWhileInactive(
            infiltrator_contract::snapshot::CoreInactiveLifecycle::Stopped,
        )
        | infiltrator_application::profile_application::ProfileActivationOutcome::CurrentProfileUpdatedWhileInactive(
            infiltrator_contract::snapshot::CoreInactiveLifecycle::Stopped,
        ) => FfiStatus {
            code: FfiErrorCode::Ok,
            message: Some("profile selected; Core remains stopped".to_owned()),
            localization: None,
        },
        infiltrator_application::profile_application::ProfileActivationOutcome::AlreadySelectedWhileInactive(
            infiltrator_contract::snapshot::CoreInactiveLifecycle::Failed,
        )
        | infiltrator_application::profile_application::ProfileActivationOutcome::SelectedWhileInactive(
            infiltrator_contract::snapshot::CoreInactiveLifecycle::Failed,
        )
        | infiltrator_application::profile_application::ProfileActivationOutcome::CurrentProfileUpdatedWhileInactive(
            infiltrator_contract::snapshot::CoreInactiveLifecycle::Failed,
        ) => FfiStatus {
            code: FfiErrorCode::Ok,
            message: Some("profile selected; Core is failed".to_owned()),
            localization: None,
        },
        infiltrator_application::profile_application::ProfileActivationOutcome::AlreadySelectedWithoutRuntime
        | infiltrator_application::profile_application::ProfileActivationOutcome::SelectedWithoutRuntime
        | infiltrator_application::profile_application::ProfileActivationOutcome::CurrentProfileUpdatedAndApplied => {
            FfiStatus::err(
                FfiErrorCode::InvalidState,
                "Android profile activation did not produce a runtime apply outcome",
            )
        }
    }
}

pub(crate) async fn active_profile_reference_state()
-> Result<infiltrator_contract::active_profile_reference::ActiveProfileReferenceState, FfiStatus> {
    let core = shared_core().await?;
    ProfileStore::read_active_profile_reference(core.config.engine_profile_store())
        .await
        .map_err(|error| {
            FfiStatus::err(
                FfiErrorCode::NotReady,
                format!("active profile reference is unavailable: {error}"),
            )
        })
}

#[cfg(test)]
mod shared_core_owner_session_state_tests {
    use super::{
        SharedCoreOwnerAdmission, SharedCoreOwnerSessionBegin, SharedCoreOwnerSessionCompletion,
        SharedCoreOwnerState, begin_owner_session_transition, complete_owner_session_transition,
        shared_core_owner_admission,
    };

    #[test]
    fn shared_core_is_unavailable_until_startup_recovery_completes() {
        let mut state = SharedCoreOwnerState::Uninitialized;
        assert!(matches!(
            shared_core_owner_admission(&state),
            SharedCoreOwnerAdmission::RecoveryRequired
        ));
        assert_eq!(
            begin_owner_session_transition(&mut state),
            SharedCoreOwnerSessionBegin::NewSession
        );
        assert!(matches!(
            shared_core_owner_admission(&state),
            SharedCoreOwnerAdmission::RecoveryRequired
        ));
        assert_eq!(
            begin_owner_session_transition(&mut state),
            SharedCoreOwnerSessionBegin::RecoveryInProgress
        );
        assert_eq!(
            complete_owner_session_transition(&mut state),
            SharedCoreOwnerSessionCompletion::Ready
        );
        assert!(matches!(
            shared_core_owner_admission(&state),
            SharedCoreOwnerAdmission::Build
        ));
        assert_eq!(
            begin_owner_session_transition(&mut state),
            SharedCoreOwnerSessionBegin::AlreadyReady
        );
    }

    #[test]
    fn closing_or_unstarted_owner_cannot_be_marked_ready() {
        let mut uninitialized = SharedCoreOwnerState::Uninitialized;
        assert_eq!(
            complete_owner_session_transition(&mut uninitialized),
            SharedCoreOwnerSessionCompletion::RecoveryNotStarted
        );

        let mut closing = SharedCoreOwnerState::ClosingWithoutCore;
        assert_eq!(
            begin_owner_session_transition(&mut closing),
            SharedCoreOwnerSessionBegin::RetryClose
        );
        assert_eq!(
            complete_owner_session_transition(&mut closing),
            SharedCoreOwnerSessionCompletion::Closing
        );
        assert!(matches!(
            shared_core_owner_admission(&closing),
            SharedCoreOwnerAdmission::Closing
        ));
    }
}

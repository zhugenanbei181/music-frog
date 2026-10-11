//! Core application lifecycle: shared [`CoreApplication`] wiring, adoption of an
//! already-running core, the restart-with-readiness path, and the
//! apply-current-profile transaction with rollback.

use infiltrator_application::android_control_application::AndroidControlApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_contract::android_control::AndroidHostProcessRole;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_core::apply::{
    ApplyError, ApplyParams, EndpointConfigReloader, apply_current_profile,
};
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_ports::endpoint::EndpointSource;
use infiltrator_ports::secure_store::SecureStore;
use mihomo_config::endpoint::ProfileEndpointSource;
use mihomo_config::manager::ConfigManager;
use mihomo_platform::android_bridge::get_android_bridge;
use mihomo_platform::defaults::DefaultCredentialStore;
use std::sync::{Arc, OnceLock};
use tokio::sync::OnceCell;

use crate::composition::{AndroidCoreComposition, core_application_with_readiness};
use crate::core_launch_spec::AndroidProfileReadiness;
use crate::ffi::{FfiErrorCode, FfiStatus};
use crate::host_support::{build_config_manager, map_mihomo_error};

/// Shared frontend wiring for the mihomo core: the application service plus
/// the [`ConfigManager`] endpoint source it resolves against. Lazily
/// constructed on first use; the controller resolves the Android bridge
/// dynamically on every call, so a bridge re-registration does not strand
/// this slot.
pub(crate) struct SharedCore {
    pub(crate) application: CoreApplication,
    pub(crate) control: AndroidControlApplication,
    pub(crate) endpoints: Arc<dyn EndpointSource>,
    config: Arc<ConfigManager<DefaultCredentialStore>>,
    pub(crate) _watchdog: infiltrator_composition::CoreWatchdogHandle,
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

    static SHARED_CORE: OnceLock<OnceCell<Arc<SharedCore>>> = OnceLock::new();
    let slot = SHARED_CORE.get_or_init(OnceCell::new);
    let core = slot
        .get_or_try_init(|| {
            let bridge = Arc::clone(&bridge);
            async move {
                let config = Arc::new(build_config_manager().await?);
                config
                    .ensure_default_config()
                    .await
                    .map_err(map_mihomo_error)?;
                config
                    .ensure_external_controller()
                    .await
                    .map_err(map_mihomo_error)?;
                let endpoints = Arc::new(ProfileEndpointSource::new(Arc::clone(&config)));
                let endpoint_source: Arc<dyn EndpointSource> = endpoints.clone();
                let endpoint = endpoint_source.resolve().await.map_err(|error| {
                    FfiStatus::err(
                        FfiErrorCode::Config,
                        format!("Android active Core endpoint is unavailable: {error}"),
                    )
                })?;
                let controller_url = endpoint.url;
                let controller_secret = endpoint.secret;
                // Compose/FFI and Bevy both reach this product composition. It
                // installs the Rust command handler, including VPN lifecycle.
                let application = core_application_with_readiness(
                    bridge,
                    controller_url,
                    controller_secret,
                    AndroidCoreComposition::new(Arc::clone(&config)),
                    Arc::new(AndroidProfileReadiness::new(Arc::clone(&endpoints))),
                )
                .map_err(|error| FfiStatus::err(FfiErrorCode::Unknown, error))?;
                let control = AndroidControlApplication::new(application.clone());
                let watchdog =
                    infiltrator_composition::spawn_core_watchdog(Arc::new(application.clone()));
                Ok::<Arc<SharedCore>, FfiStatus>(Arc::new(SharedCore {
                    application,
                    control,
                    endpoints: endpoints as Arc<dyn EndpointSource>,
                    config,
                    _watchdog: watchdog,
                }))
            }
        })
        .await?;
    Ok(Arc::clone(core))
}

/// Undo a `set_current` after a failed apply so the active-profile metadata
/// matches the config the core is running again.
async fn restore_current_profile<S: SecureStore>(
    config: &ConfigManager<S>,
    previous: Option<String>,
) -> Result<(), String> {
    let Some(previous) = previous else {
        return Ok(());
    };
    let current = config.get_current().await.map_err(|err| err.to_string())?;
    if current == previous {
        return Ok(());
    }
    config
        .set_current(&previous)
        .await
        .map_err(|err| err.to_string())
}

/// Apply the *current* profile content through the application transaction
/// (validate → atomic write → restart → readiness → rollback on failure).
/// `previous` is the active profile name before the caller switched to the
/// profile being applied; it is restored when the transaction rolls back.
/// This helper is owner-local; UI requests enter through the typed profile
/// operation, which runs this transaction inside the owner process. Construction
/// or readiness failures are returned as typed FFI failures; there is no restart
/// without lifecycle/readback proof.
pub(crate) async fn apply_current_profile_status(previous: Option<String>) -> FfiStatus {
    let core = match shared_core().await {
        Ok(core) => core,
        Err(status) => return status,
    };
    if let Err(failure) = core.application.adopt_if_running().await {
        return FfiStatus::from(InfiltratorError::Backend(failure.message));
    }
    let current = match core.config.get_current().await {
        Ok(current) => current,
        Err(err) => return map_mihomo_error(err),
    };
    let content = match core.config.load(&current).await {
        Ok(content) => content,
        Err(err) => return map_mihomo_error(err),
    };
    let reloader = EndpointConfigReloader::new(core.endpoints.clone() as Arc<dyn EndpointSource>);
    let params = ApplyParams {
        strategy: ApplyStrategy::AlwaysRestart,
        ..ApplyParams::default()
    };
    match apply_current_profile(&core.application, &core.config, &reloader, &content, params).await
    {
        Ok(_) => FfiStatus::ok(),
        Err(ApplyError::RolledBack { cause }) => {
            if let Err(restore_err) = restore_current_profile(&core.config, previous).await {
                return FfiStatus::err(
                    FfiErrorCode::InvalidState,
                    format!(
                        "apply failed ({cause}) and active profile restore failed: {restore_err}"
                    ),
                );
            }
            FfiStatus::err(
                FfiErrorCode::InvalidState,
                format!("apply failed, previous profile restored and core recovered: {cause}"),
            )
        }
        Err(ApplyError::RollbackFailed { cause, rollback }) => {
            // Best-effort metadata restore; both failures stay visible.
            if let Err(restore_err) = restore_current_profile(&core.config, previous).await {
                log::error!("active profile restore failed after rollback failure: {restore_err}");
            }
            FfiStatus::err(
                FfiErrorCode::InvalidState,
                format!(
                    "apply failed ({cause}) and rollback failed as well; core is down: {rollback}"
                ),
            )
        }
        Err(err) => map_apply_error(err),
    }
}

fn map_apply_error(err: ApplyError) -> FfiStatus {
    match err {
        ApplyError::Validation(message) | ApplyError::Write(message) => {
            FfiStatus::err(FfiErrorCode::Config, message)
        }
        ApplyError::Lifecycle(message) => FfiStatus::err(FfiErrorCode::InvalidState, message),
        // RolledBack/RollbackFailed are handled by the apply caller with
        // set_current restoration; anything reaching here still carries its
        // readable message.
        other => FfiStatus::err(FfiErrorCode::InvalidState, other.to_string()),
    }
}

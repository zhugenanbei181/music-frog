//! Core session lifecycle: shared [`CoreSession`] wiring, adoption of an
//! already-running core, the restart-with-readiness path, and the
//! apply-current-profile transaction with rollback.

use std::sync::{Arc, Mutex, OnceLock};

use infiltrator_core::apply::{
    ApplyError, ApplyParams, ApplyStrategy, SessionConfigReloader, apply_current_profile,
};
use infiltrator_core::error::InfiltratorError;
use infiltrator_core::session::{
    CoreSession, CoreStatus, MihomoVersionProbe, ProfileEndpointSource, READINESS_TIMEOUT,
    SessionError,
};
use mihomo_api::error::MihomoError;
use mihomo_config::manager::ConfigManager;
use mihomo_platform::android_bridge::get_android_bridge;
use mihomo_platform::traits::{CoreController, CredentialStore, DefaultCredentialStore};

#[cfg(target_os = "android")]
use mihomo_platform::android::AndroidCoreController;

use super::support::map_mihomo_error;
use crate::ffi::{FfiErrorCode, FfiStatus};

/// Shared frontend wiring for the mihomo core: the unified [`CoreSession`]
/// (status machine, generation protocol, readiness probing) plus the
/// [`ConfigManager`] its endpoint source resolves against. Lazily constructed
/// on first use; the controller resolves the Android bridge dynamically on
/// every call, so a bridge re-registration does not strand this slot.
pub(super) struct SharedCore {
    // Accessed from support.rs's controller client fallback path.
    pub(super) session: Arc<CoreSession>,
    config: Arc<ConfigManager<DefaultCredentialStore>>,
}

/// Core lifecycle controller for the shared session. On Android this is
/// mihomo_platform's [`AndroidCoreController`], which delegates to the
/// globally registered JNI bridge (survives bridge re-registration).
#[cfg(target_os = "android")]
fn platform_core_controller() -> Arc<dyn CoreController> {
    Arc::new(AndroidCoreController)
}

/// Bridge-backed controller for non-Android builds. The bridge registry is
/// simply never populated there, so the session stays `Stopped`; this only
/// keeps the shared-session wiring compilable for host tests.
#[cfg(not(target_os = "android"))]
struct BridgeCoreController;

#[cfg(not(target_os = "android"))]
#[async_trait::async_trait]
impl CoreController for BridgeCoreController {
    async fn start(&self) -> mihomo_api::error::Result<()> {
        let bridge = get_android_bridge().ok_or_else(|| {
            MihomoError::Service("Android bridge is not configured (core start)".into())
        })?;
        bridge.core_start().await
    }

    async fn stop(&self) -> mihomo_api::error::Result<()> {
        let bridge = get_android_bridge().ok_or_else(|| {
            MihomoError::Service("Android bridge is not configured (core stop)".into())
        })?;
        bridge.core_stop().await
    }

    async fn is_running(&self) -> bool {
        platform_core_is_running().await
    }

    fn controller_url(&self) -> Option<String> {
        get_android_bridge().and_then(|bridge| bridge.core_controller_url())
    }
}

#[cfg(not(target_os = "android"))]
fn platform_core_controller() -> Arc<dyn CoreController> {
    Arc::new(BridgeCoreController)
}

#[cfg(target_os = "android")]
async fn platform_core_is_running() -> bool {
    AndroidCoreController.is_running().await
}

#[cfg(not(target_os = "android"))]
async fn platform_core_is_running() -> bool {
    match get_android_bridge() {
        Some(bridge) => bridge.core_is_running().await.unwrap_or_else(|err| {
            log::warn!("android core is_running failed: {err}");
            false
        }),
        None => false,
    }
}

pub(super) fn shared_core() -> Result<Arc<SharedCore>, FfiStatus> {
    static SHARED_CORE: OnceLock<Mutex<Option<Arc<SharedCore>>>> = OnceLock::new();
    let slot = SHARED_CORE.get_or_init(|| Mutex::new(None));
    let mut guard = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(core) = guard.as_ref() {
        return Ok(Arc::clone(core));
    }
    let config = Arc::new(ConfigManager::new().map_err(map_mihomo_error)?);
    let endpoints = Arc::new(ProfileEndpointSource::new(Arc::clone(&config)));
    let probe = Arc::new(MihomoVersionProbe::new(Arc::clone(&endpoints)));
    let session = Arc::new(CoreSession::new(
        platform_core_controller(),
        endpoints,
        probe,
    ));
    let core = Arc::new(SharedCore { session, config });
    *guard = Some(Arc::clone(&core));
    Ok(core)
}

async fn restart_runtime_if_running() {
    if let Err(status) = restart_with_readiness().await {
        log::error!("core restart failed: {status:?}");
    }
}

/// A freshly constructed session reports [`CoreStatus::Stopped`] even when
/// the Kotlin side already started the core behind the session's back (the
/// VPN flow calls the bridge directly, and the platform `start` is
/// idempotent). Prove the live controller and adopt the process into the
/// session so later transactions treat it as running instead of skipping
/// the restart.
async fn adopt_running_core(session: &CoreSession) -> Result<(), FfiStatus> {
    if session.status() != CoreStatus::Stopped || !platform_core_is_running().await {
        return Ok(());
    }
    let generation = session.start().await.map_err(map_session_error)?;
    session
        .wait_for_ready(generation, READINESS_TIMEOUT)
        .await
        .map_err(map_session_error)
}

/// Core restart through the shared session: stop + start under one
/// generation, then block on controller readiness. Falls back to the legacy
/// bridge restart when the session cannot be constructed.
async fn restart_with_readiness() -> Result<(), FfiStatus> {
    let core = match shared_core() {
        Ok(core) => core,
        Err(_) => {
            legacy_bridge_restart().await;
            return Ok(());
        }
    };
    adopt_running_core(&core.session).await?;
    let generation = core.session.restart().await.map_err(map_session_error)?;
    core.session
        .wait_for_ready(generation, READINESS_TIMEOUT)
        .await
        .map_err(map_session_error)
}

/// Pre-session behavior kept as the fallback path: raw bridge stop+start
/// without readiness proof.
async fn legacy_bridge_restart() {
    if let Some(bridge) = get_android_bridge()
        && let Ok(true) = bridge.core_is_running().await
    {
        let _ = bridge.core_stop().await;
        let _ = bridge.core_start().await;
    }
}

/// Undo a `set_current` after a failed apply so the active-profile metadata
/// matches the config the core is running again.
async fn restore_current_profile<S: CredentialStore>(
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

/// Apply the *current* profile content through the session transaction
/// (validate → atomic write → restart → readiness → rollback on failure).
/// `previous` is the active profile name before the caller switched to the
/// profile being applied; it is restored when the transaction rolls back.
/// When the shared session cannot be constructed, falls back to the legacy
/// bridge restart so the pre-session behavior is kept.
pub(super) async fn apply_current_profile_status(previous: Option<String>) -> FfiStatus {
    let core = match shared_core() {
        Ok(core) => core,
        Err(_) => {
            restart_runtime_if_running().await;
            return FfiStatus::ok();
        }
    };
    if let Err(status) = adopt_running_core(&core.session).await {
        return status;
    }
    let current = match core.config.get_current().await {
        Ok(current) => current,
        Err(err) => return map_mihomo_error(err),
    };
    let content = match core.config.load(&current).await {
        Ok(content) => content,
        Err(err) => return map_mihomo_error(err),
    };
    let reloader = SessionConfigReloader::new(Arc::clone(&core.session));
    let params = ApplyParams {
        strategy: ApplyStrategy::AlwaysRestart,
        ..ApplyParams::default()
    };
    match apply_current_profile(&core.session, &core.config, &reloader, &content, params).await {
        Ok(_) => FfiStatus::ok(),
        Err(ApplyError::RolledBack { cause }) => {
            if let Err(restore_err) = restore_current_profile(&core.config, previous).await {
                return FfiStatus::err(
                    FfiErrorCode::InvalidState,
                    format!("apply failed ({cause}) and active profile restore failed: {restore_err}"),
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
                format!("apply failed ({cause}) and rollback failed as well; core is down: {rollback}"),
            )
        }
        Err(err) => map_apply_error(err),
    }
}

fn map_session_error(err: SessionError) -> FfiStatus {
    // Route session failures through the existing
    // InfiltratorError -> FfiStatus channel so they stay readable at the
    // FFI boundary instead of being collapsed into a generic string.
    FfiStatus::from(InfiltratorError::from(err))
}

fn map_apply_error(err: ApplyError) -> FfiStatus {
    match err {
        ApplyError::Validation(message) | ApplyError::Write(message) => {
            FfiStatus::err(FfiErrorCode::Config, message)
        }
        ApplyError::Session(session_error) => map_session_error(session_error),
        // RolledBack/RollbackFailed are handled by the apply caller with
        // set_current restoration; anything reaching here still carries its
        // readable message.
        other => FfiStatus::err(FfiErrorCode::InvalidState, other.to_string()),
    }
}

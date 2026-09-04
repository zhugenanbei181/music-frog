//! Core application lifecycle: shared [`CoreApplication`] wiring, adoption of an
//! already-running core, the restart-with-readiness path, and the
//! apply-current-profile transaction with rollback.

use std::sync::{Arc, Mutex, OnceLock};

use infiltrator_application::core_application::CoreApplication;
use infiltrator_contract::command::{CommandIntent, CommandResult};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_core::apply::{
    ApplyError, ApplyParams, ApplyStrategy, EndpointConfigReloader, apply_current_profile,
};
use infiltrator_core::error::InfiltratorError;
use infiltrator_core::session::{MihomoVersionProbe, ProfileEndpointSource, ReadinessProbe};
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::endpoint::EndpointSource;
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use mihomo_config::manager::ConfigManager;
use mihomo_platform::android_bridge::get_android_bridge;
use mihomo_platform::defaults::DefaultCredentialStore;

#[cfg(target_os = "android")]
use mihomo_platform::android::AndroidCoreController;

use super::support::{build_config_manager, map_mihomo_error};
use crate::ffi::{FfiErrorCode, FfiStatus};

/// Shared frontend wiring for the mihomo core: the application service plus
/// the [`ConfigManager`] endpoint source it resolves against. Lazily
/// constructed on first use; the controller resolves the Android bridge
/// dynamically on every call, so a bridge re-registration does not strand
/// this slot.
pub(super) struct SharedCore {
    pub(super) application: CoreApplication,
    pub(super) endpoints: Arc<dyn EndpointSource>,
    config: Arc<ConfigManager<DefaultCredentialStore>>,
}

/// Adapts the endpoint-aware readiness probe to the 0.30 application port.
struct ApplicationReadiness {
    endpoints: Arc<ProfileEndpointSource<DefaultCredentialStore>>,
    probe: Arc<MihomoVersionProbe<DefaultCredentialStore>>,
}

#[async_trait::async_trait]
impl CoreReadiness for ApplicationReadiness {
    async fn probe(&self) -> Result<String, PortError> {
        self.probe
            .probe()
            .await
            .map_err(|error| PortError::Network(error.to_string()))?;
        self.endpoints
            .resolve()
            .await
            .map(|endpoint| endpoint.url)
            .map_err(|error| PortError::Network(error.to_string()))
    }
}

/// Core lifecycle controller for the shared session. On Android this is
/// mihomo_platform's [`AndroidCoreController`], which delegates to the
/// globally registered JNI bridge (survives bridge re-registration).
#[cfg(target_os = "android")]
fn platform_core_controller() -> Arc<dyn CoreProcess> {
    Arc::new(AndroidCoreController)
}

/// Bridge-backed controller for non-Android builds. The bridge registry is
/// simply never populated there, so the session stays `Stopped`; this only
/// keeps the shared-session wiring compilable for host tests.
#[cfg(not(target_os = "android"))]
struct BridgeCoreController;

#[cfg(not(target_os = "android"))]
#[async_trait::async_trait]
impl CoreProcess for BridgeCoreController {
    async fn start(&self) -> std::result::Result<(), PortError> {
        let bridge = get_android_bridge().ok_or_else(|| {
            PortError::Failed("Android bridge is not configured (core start)".into())
        })?;
        bridge
            .core_start()
            .await
            .map_err(|error| PortError::Failed(error.to_string()))
    }

    async fn stop(&self) -> std::result::Result<(), PortError> {
        let bridge = get_android_bridge().ok_or_else(|| {
            PortError::Failed("Android bridge is not configured (core stop)".into())
        })?;
        bridge
            .core_stop()
            .await
            .map_err(|error| PortError::Failed(error.to_string()))
    }

    async fn status(&self) -> std::result::Result<CoreLifecycle, PortError> {
        let running = get_android_bridge()
            .ok_or_else(|| PortError::Failed("Android bridge is not configured".into()))?
            .core_is_running()
            .await
            .map_err(|error| PortError::Failed(error.to_string()))?;
        Ok(if running {
            CoreLifecycle::Running
        } else {
            CoreLifecycle::Stopped
        })
    }

    fn controller_endpoint(&self) -> Option<String> {
        get_android_bridge().and_then(|bridge| bridge.core_controller_url())
    }
}

#[cfg(not(target_os = "android"))]
fn platform_core_controller() -> Arc<dyn CoreProcess> {
    Arc::new(BridgeCoreController)
}

pub(super) async fn shared_core() -> Result<Arc<SharedCore>, FfiStatus> {
    static SHARED_CORE: OnceLock<Mutex<Option<Arc<SharedCore>>>> = OnceLock::new();
    let slot = SHARED_CORE.get_or_init(|| Mutex::new(None));
    let cached = {
        let guard = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.as_ref().cloned()
    };
    if let Some(core) = cached {
        return Ok(core);
    }
    // Init path only: the configs-dir redirect resolution (settings load)
    // stays off the hot path; a concurrent winner's manager is reused below.
    let config = Arc::new(build_config_manager().await?);
    let mut guard = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(core) = guard.as_ref() {
        return Ok(Arc::clone(core));
    }
    let endpoints = Arc::new(ProfileEndpointSource::new(Arc::clone(&config)));
    let probe = Arc::new(MihomoVersionProbe::new(Arc::clone(&endpoints)));
    let process = platform_core_controller();
    let application = CoreApplication::new(
        process,
        Arc::new(ApplicationReadiness {
            endpoints: Arc::clone(&endpoints),
            probe,
        }),
    );
    let core = Arc::new(SharedCore {
        application,
        endpoints: endpoints as Arc<dyn EndpointSource>,
        config,
    });
    *guard = Some(Arc::clone(&core));
    Ok(core)
}

async fn restart_runtime_if_running() {
    if let Err(status) = restart_with_readiness().await {
        log::error!("core restart failed: {status:?}");
    }
}

/// Core restart through the shared application: stop + start under one
/// generation, then block on controller readiness. Falls back to the legacy
/// bridge restart when the session cannot be constructed.
async fn restart_with_readiness() -> Result<(), FfiStatus> {
    let core = match shared_core().await {
        Ok(core) => core,
        Err(_) => {
            legacy_bridge_restart().await;
            return Ok(());
        }
    };
    if let Err(failure) = core.application.adopt_if_running().await {
        return Err(FfiStatus::from(InfiltratorError::Mihomo(failure.message)));
    }
    match core.application.execute(CommandIntent::RestartCore).await {
        CommandResult::Completed { .. } => Ok(()),
        CommandResult::Rejected { failure, .. } => {
            Err(FfiStatus::from(InfiltratorError::Mihomo(failure.message)))
        }
        CommandResult::Accepted { .. } => Err(FfiStatus::from(InfiltratorError::Internal(
            "application restart unexpectedly returned Accepted".to_string(),
        ))),
    }
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
/// When the shared application cannot be constructed, falls back to the legacy
/// bridge restart so the pre-session behavior is kept.
pub(super) async fn apply_current_profile_status(previous: Option<String>) -> FfiStatus {
    let core = match shared_core().await {
        Ok(core) => core,
        Err(_) => {
            restart_runtime_if_running().await;
            return FfiStatus::ok();
        }
    };
    if let Err(failure) = core.application.adopt_if_running().await {
        return FfiStatus::from(InfiltratorError::Mihomo(failure.message));
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

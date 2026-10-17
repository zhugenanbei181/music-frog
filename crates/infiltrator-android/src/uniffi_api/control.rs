//! Typed Android product control boundary shared by Compose and Bevy hosts.

use super::product::android_execute_product_command_json;
use crate::ffi::{FfiErrorCode, FfiStatus};
use crate::host_session::shared_core;
use crate::host_support::{get_runtime, map_application_failure};
use infiltrator_contract::android_control;
use infiltrator_contract::android_control::{
    AndroidCoreLifecycleSnapshot, AndroidHostProcessRole, AndroidProductCommand,
    AndroidProductCommandOutcome, AndroidProductCommandRequest, AndroidProductControlIntent,
    VpnOwnerCoreReadback, VpnOwnerSnapshot, validate_android_control_owner_observation,
    validate_android_control_terminal_readback, validate_android_vpn_stop_terminal_readback,
};
use infiltrator_contract::command::CommandResult;
use infiltrator_contract::vpn::VpnStopTarget;
use infiltrator_platform::android_bridge::{AndroidBridge, get_android_bridge};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::lifecycle::CoreLifecycle;

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AndroidControlIntent {
    StartProxy,
    StopProxy,
    ResumeKernel,
    PauseKernel,
    RestartKernel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AndroidCoreLifecycle {
    Stopped,
    Starting,
    Ready,
    Running,
    Stopping,
    Failed,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct AndroidCoreLifecycleResult {
    pub status: FfiStatus,
    pub lifecycle: Option<AndroidCoreLifecycle>,
    pub engine_id: Option<String>,
    pub core_instance_id: Option<String>,
    pub core_version: Option<String>,
    pub generation: u64,
    pub session_token: Option<String>,
    pub revision: u64,
    /// Full serialized contract Failure. A successful Failed lifecycle keeps
    /// its typed error facts here instead of turning the read itself into an error.
    pub failure_json: Option<String>,
}

impl From<CoreLifecycle> for AndroidCoreLifecycle {
    fn from(lifecycle: CoreLifecycle) -> Self {
        match lifecycle {
            CoreLifecycle::Stopped => Self::Stopped,
            CoreLifecycle::Starting => Self::Starting,
            CoreLifecycle::Ready => Self::Ready,
            CoreLifecycle::Running => Self::Running,
            CoreLifecycle::Stopping => Self::Stopping,
            CoreLifecycle::Failed => Self::Failed,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum VpnAuthorizationResult {
    Granted,
    Denied,
    Revoked,
}

/// Android Binder/Messenger protocol version. Compose, Bevy and `:vpn` must
/// use the same version; incompatible request/reply changes require a bump.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AndroidHostProtocolVersion {
    V10,
}

/// Host IPC operations performed only by the `:vpn` process owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AndroidHostRequest {
    ReadCoreLifecycle,
    ExecuteProductCommand,
    ReadSurfaceSnapshot,
    ExecuteOwnerOperation,
    ExecuteProfileAgeCredentialOwnerOperation,
    ReadOwnerDataRevision,
    StartVpnService,
    StopVpnService,
    ReadVpnAuthorization,
    ReadVpnOwnerSnapshot,
    ApplyVpnConfiguration,
}

/// Typed readback for an Android host IPC request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AndroidHostReply {
    CoreLifecycleAvailable,
    ProductCommandCompleted,
    SurfaceSnapshotAvailable,
    OwnerOperationCompleted,
    VpnConfigurationBusy,
    OwnerDataRevisionAvailable,
    StaleOwnerData,
    OwnerSessionChanged,
    StaleSnapshot,
    VpnOwnerSnapshotAvailable,
    VpnConfigurationAccepted,
    VpnAuthorized,
    VpnRunning,
    VpnStopped,
    VpnPermissionRequired,
    VpnStartTimedOut,
    VpnStopTimedOut,
    VpnStopSuperseded,
    VpnStopCanceled,
    Failed,
    ServiceUnavailable,
}

impl From<VpnAuthorizationResult> for android_control::VpnAuthorizationResult {
    fn from(result: VpnAuthorizationResult) -> Self {
        match result {
            VpnAuthorizationResult::Granted => Self::Granted,
            VpnAuthorizationResult::Denied => Self::Denied,
            VpnAuthorizationResult::Revoked => Self::Revoked,
        }
    }
}

#[uniffi::export]
pub async fn android_control(intent: AndroidControlIntent) -> FfiStatus {
    get_runtime()
        .spawn(async move {
            let Some(bridge) = get_android_bridge() else {
                return FfiStatus::err(
                    FfiErrorCode::NotReady,
                    "Android host bridge is unavailable",
                );
            };
            execute_control_intent_with_bridge(
                bridge.as_ref(),
                intent,
                ControlCommandDispatch::RegisteredBridge,
            )
            .await
        })
        .await
        .unwrap_or_else(|error| {
            FfiStatus::err(
                FfiErrorCode::Unknown,
                format!("Android control task failed: {error}"),
            )
        })
}

#[derive(Clone, Copy)]
enum ControlCommandDispatch {
    RegisteredBridge,
    #[cfg_attr(not(test), allow(dead_code))]
    ProvidedBridge,
}

async fn execute_control_intent_with_bridge(
    bridge: &dyn AndroidBridge,
    intent: AndroidControlIntent,
    dispatch: ControlCommandDispatch,
) -> FfiStatus {
    let role = bridge.process_role();
    if intent == AndroidControlIntent::StopProxy {
        return match role {
            AndroidHostProcessRole::UiClient | AndroidHostProcessRole::CoreOwner => {
                match bridge.vpn_stop(VpnStopTarget::Current).await {
                    Ok(receipt) => match validate_android_vpn_stop_terminal_readback(&receipt) {
                        Ok(()) => FfiStatus::ok(),
                        Err(failure) => map_application_failure(failure),
                    },
                    Err(error) => FfiStatus::err(FfiErrorCode::NotReady, error.to_string()),
                }
            }
            AndroidHostProcessRole::Unsupported => FfiStatus::err(
                FfiErrorCode::NotReady,
                "Android VPN stop is unavailable in this process",
            ),
        };
    }

    let initial_snapshot = match control_owner_snapshot(bridge, role).await {
        Ok(snapshot) => snapshot,
        Err(status) => return status,
    };
    if let Err(failure) = validate_android_control_owner_observation(&initial_snapshot) {
        return map_application_failure(failure);
    }

    let contract_intent = match intent {
        AndroidControlIntent::StartProxy => AndroidProductControlIntent::StartProxy,
        AndroidControlIntent::ResumeKernel => AndroidProductControlIntent::ResumeKernel,
        AndroidControlIntent::PauseKernel => AndroidProductControlIntent::PauseKernel,
        AndroidControlIntent::RestartKernel => AndroidProductControlIntent::RestartKernel,
        AndroidControlIntent::StopProxy => unreachable!("StopProxy returned through VPN IPC"),
    };
    let scope = match (&initial_snapshot.core, role) {
        (VpnOwnerCoreReadback::Available(core), AndroidHostProcessRole::UiClient)
        | (VpnOwnerCoreReadback::Available(core), AndroidHostProcessRole::CoreOwner) => {
            core.scope()
        }
        (VpnOwnerCoreReadback::Unavailable { failure_message }, _) => {
            return FfiStatus::err(FfiErrorCode::NotReady, failure_message);
        }
        (_, AndroidHostProcessRole::Unsupported) => {
            return FfiStatus::err(
                FfiErrorCode::NotReady,
                "Android product controls are unavailable in this process",
            );
        }
    };
    let Some(scope) = scope else {
        return FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android core identity is unavailable; control request was not sent",
        );
    };
    let request = AndroidProductCommandRequest {
        scope,
        command: AndroidProductCommand::Control(contract_intent),
    };
    let request_json = match serde_json::to_string(&request) {
        Ok(json) => json,
        Err(error) => {
            return FfiStatus::err(
                FfiErrorCode::InvalidInput,
                format!("Android control request serialization failed: {error}"),
            );
        }
    };
    let outcome_json = match role {
        AndroidHostProcessRole::CoreOwner => {
            android_execute_product_command_json(request_json).await
        }
        AndroidHostProcessRole::UiClient => {
            let result = match dispatch {
                ControlCommandDispatch::RegisteredBridge => match get_android_bridge() {
                    Some(current) => current
                        .execute_product_command_json(&request_json)
                        .await
                        .map_err(|error| error.to_string()),
                    None => Err("Android host bridge is unavailable".to_owned()),
                },
                ControlCommandDispatch::ProvidedBridge => bridge
                    .execute_product_command_json(&request_json)
                    .await
                    .map_err(|error| error.to_string()),
            };
            match result {
                Ok(outcome) => outcome,
                Err(error) => return FfiStatus::err(FfiErrorCode::NotReady, error),
            }
        }
        AndroidHostProcessRole::Unsupported => unreachable!("unsupported role returned earlier"),
    };
    match serde_json::from_str::<AndroidProductCommandOutcome>(&outcome_json) {
        Ok(AndroidProductCommandOutcome::Terminal(outcome)) => {
            let outcome = *outcome;
            match outcome.result {
                CommandResult::Completed { .. } => {
                    let terminal_snapshot = match control_owner_snapshot(bridge, role).await {
                        Ok(snapshot) => snapshot,
                        Err(status) => return status,
                    };
                    match validate_android_control_terminal_readback(
                        contract_intent,
                        &initial_snapshot,
                        &terminal_snapshot,
                        outcome.scope.as_ref(),
                    ) {
                        Ok(()) => FfiStatus::ok(),
                        Err(failure) => map_application_failure(failure),
                    }
                }
                CommandResult::Produced { .. } => FfiStatus::err(
                    FfiErrorCode::InvalidState,
                    "Android lifecycle control returned an unexpected produced result",
                ),
                CommandResult::Rejected { failure, .. } => map_application_failure(failure),
                CommandResult::Accepted { .. } => FfiStatus::err(
                    FfiErrorCode::InvalidState,
                    "Android core control returned a non-terminal result",
                ),
            }
        }
        Ok(AndroidProductCommandOutcome::Unavailable { failure, .. }) => {
            map_application_failure(failure)
        }
        Err(error) => FfiStatus::err(
            FfiErrorCode::NotReady,
            format!("Android core control returned an invalid result: {error}"),
        ),
    }
}

async fn control_owner_snapshot(
    bridge: &dyn AndroidBridge,
    role: AndroidHostProcessRole,
) -> Result<VpnOwnerSnapshot, FfiStatus> {
    let mut snapshot = bridge
        .vpn_owner_snapshot()
        .await
        .map_err(|error| FfiStatus::err(FfiErrorCode::NotReady, error.to_string()))?;
    match role {
        AndroidHostProcessRole::UiClient => {}
        AndroidHostProcessRole::CoreOwner => {
            let core = shared_core().await?;
            let current = core.application.snapshot();
            snapshot.core = VpnOwnerCoreReadback::Available(current.lifecycle_snapshot());
        }
        AndroidHostProcessRole::Unsupported => {
            return Err(FfiStatus::err(
                FfiErrorCode::NotReady,
                "Android product controls are unavailable in this process",
            ));
        }
    }
    Ok(snapshot)
}

/// Read the shared Rust lifecycle after adopting an already-running Android
/// owner process when this UI process has just been recreated.
#[uniffi::export]
pub async fn android_core_lifecycle_status() -> AndroidCoreLifecycleResult {
    get_runtime()
        .spawn(async move {
            if let Some(bridge) = get_android_bridge()
                && bridge.process_role() == AndroidHostProcessRole::UiClient
            {
                return match bridge.core_lifecycle_snapshot().await {
                    Ok(snapshot) => core_lifecycle_result(snapshot),
                    Err(error) => AndroidCoreLifecycleResult {
                        status: FfiStatus::err(FfiErrorCode::NotReady, error.to_string()),
                        lifecycle: None,
                        engine_id: None,
                        core_instance_id: None,
                        core_version: None,
                        generation: 0,
                        session_token: None,
                        revision: 0,
                        failure_json: None,
                    },
                };
            }
            let core = match shared_core().await {
                Ok(core) => core,
                Err(status) => {
                    return AndroidCoreLifecycleResult {
                        status,
                        lifecycle: None,
                        engine_id: None,
                        core_instance_id: None,
                        core_version: None,
                        generation: 0,
                        session_token: None,
                        revision: 0,
                        failure_json: None,
                    };
                }
            };
            if let Err(failure) = core.application.adopt_if_running().await {
                return AndroidCoreLifecycleResult {
                    status: map_application_failure(failure),
                    lifecycle: None,
                    engine_id: None,
                    core_instance_id: None,
                    core_version: None,
                    generation: 0,
                    session_token: None,
                    revision: 0,
                    failure_json: None,
                };
            }
            let snapshot = core.application.snapshot();
            core_lifecycle_result(snapshot.lifecycle_snapshot().into())
        })
        .await
        .unwrap_or_else(|error| AndroidCoreLifecycleResult {
            status: FfiStatus::err(
                FfiErrorCode::Unknown,
                format!("Android core lifecycle query failed: {error}"),
            ),
            lifecycle: None,
            engine_id: None,
            core_instance_id: None,
            core_version: None,
            generation: 0,
            session_token: None,
            revision: 0,
            failure_json: None,
        })
}

fn core_lifecycle_result(snapshot: AndroidCoreLifecycleSnapshot) -> AndroidCoreLifecycleResult {
    AndroidCoreLifecycleResult {
        status: FfiStatus::ok(),
        lifecycle: Some(snapshot.lifecycle.into()),
        engine_id: snapshot
            .identity
            .as_ref()
            .map(|identity| identity.engine.as_str().to_owned()),
        core_instance_id: snapshot
            .identity
            .as_ref()
            .map(|identity| identity.instance.as_str().to_owned()),
        core_version: snapshot
            .identity
            .and_then(|identity| identity.version.map(|version| version.raw().to_owned())),
        generation: snapshot.generation,
        session_token: snapshot
            .session_token
            .map(|token| token.value().to_string()),
        revision: snapshot.revision,
        failure_json: snapshot
            .failure
            .as_ref()
            .map(|failure| serde_json::to_string(failure).expect("Failure serializes")),
    }
}

#[cfg(test)]
#[path = "control_tests.rs"]
mod tests;

#[uniffi::export]
pub fn vpn_authorization_result(result: VpnAuthorizationResult) -> FfiStatus {
    let result: android_control::VpnAuthorizationResult = result.into();
    match result {
        android_control::VpnAuthorizationResult::Granted => FfiStatus::ok(),
        android_control::VpnAuthorizationResult::Denied => map_application_failure(Failure::new(
            ErrorCode::Permission,
            "Android VPN authorization was denied",
            false,
        )),
        android_control::VpnAuthorizationResult::Revoked => map_application_failure(Failure::new(
            ErrorCode::Permission,
            "Android VPN authorization was revoked",
            false,
        )),
    }
}

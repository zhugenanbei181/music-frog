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
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::vpn::VpnStopTarget;
use mihomo_platform::android_bridge::{AndroidBridge, get_android_bridge};

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
    V9,
}

/// Host IPC operations performed only by the `:vpn` process owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AndroidHostRequest {
    ReadCoreLifecycle,
    ExecuteProductCommand,
    ReadSurfaceSnapshot,
    ExecuteOwnerOperation,
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
mod tests {
    use super::{
        AndroidControlIntent, AndroidCoreLifecycle, ControlCommandDispatch, core_lifecycle_result,
        execute_control_intent_with_bridge,
    };
    use crate::ffi::FfiErrorCode;
    use async_trait::async_trait;
    use infiltrator_contract::android_control::{
        AndroidCoreLifecycleSnapshot, AndroidCoreProcessState, AndroidHostProcessRole,
        AndroidProductCommandOutcome, AndroidProductCommandRequest, AndroidProductControlIntent,
        CoreLaunchSpec, VpnOwnerCoreReadback, VpnOwnerLifecycle, VpnOwnerReadiness,
        VpnOwnerResourceFact, VpnOwnerResourceKind, VpnOwnerResourceState, VpnOwnerSnapshot,
        validate_android_control_terminal_readback, validate_android_vpn_stop_terminal_readback,
    };
    use infiltrator_contract::command::{CommandResult, RequestId};
    use infiltrator_contract::core_identity::{
        CoreCommandOutcome, CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
    };
    use infiltrator_contract::error::{ErrorCode, Failure};
    use infiltrator_contract::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};
    use infiltrator_contract::vpn::{
        PreparedVpnConfiguration, VpnConfigurationApplyOutcome, VpnOwnerStopReceipt,
        VpnStartOutcome, VpnStopTarget,
    };
    use mihomo_api::error::Result;
    use mihomo_platform::android_bridge::{
        AndroidBridge, CoreProcessStartReceipt, CoreProcessStopReceipt,
    };
    use std::path::PathBuf;
    use std::sync::Mutex;

    struct FakeOwnerTransport {
        snapshot: AndroidCoreLifecycleSnapshot,
        current_scope: CoreScope,
        vpn_snapshot: VpnOwnerSnapshot,
        terminal_vpn_snapshot: Mutex<VpnOwnerSnapshot>,
        vpn_snapshot_reads: std::sync::atomic::AtomicUsize,
        events: Mutex<Vec<&'static str>>,
        applied_scopes: Mutex<Vec<CoreScope>>,
    }

    impl FakeOwnerTransport {
        fn new(snapshot: AndroidCoreLifecycleSnapshot, current_scope: CoreScope) -> Self {
            let vpn_snapshot = control_owner_snapshot(
                snapshot.clone(),
                VpnOwnerLifecycle::Running,
                VpnOwnerReadiness::Ready,
                4,
            );
            Self {
                terminal_vpn_snapshot: Mutex::new(vpn_snapshot.clone()),
                vpn_snapshot_reads: std::sync::atomic::AtomicUsize::new(0),
                vpn_snapshot,
                snapshot,
                current_scope,
                events: Mutex::new(Vec::new()),
                applied_scopes: Mutex::new(Vec::new()),
            }
        }

        fn events(&self) -> Vec<&'static str> {
            self.events.lock().expect("event lock").clone()
        }

        fn applied_scopes(&self) -> Vec<CoreScope> {
            self.applied_scopes.lock().expect("scope lock").clone()
        }
    }

    #[async_trait]
    impl AndroidBridge for FakeOwnerTransport {
        fn process_role(&self) -> AndroidHostProcessRole {
            AndroidHostProcessRole::UiClient
        }

        async fn core_start(
            &self,
            _launch_spec: &CoreLaunchSpec,
        ) -> Result<CoreProcessStartReceipt> {
            Ok(CoreProcessStartReceipt::NotVpnOwner)
        }

        async fn core_stop(&self) -> Result<CoreProcessStopReceipt> {
            Ok(CoreProcessStopReceipt::NotVpnOwner)
        }

        async fn core_process_state(&self) -> Result<AndroidCoreProcessState> {
            Ok(AndroidCoreProcessState::Unavailable)
        }

        async fn core_lifecycle_snapshot(&self) -> Result<AndroidCoreLifecycleSnapshot> {
            self.events
                .lock()
                .expect("event lock")
                .push("core-readback");
            Ok(self.snapshot.clone())
        }

        async fn execute_product_command_json(&self, command_json: &str) -> Result<String> {
            self.events.lock().expect("event lock").push("command");
            let request = serde_json::from_str::<AndroidProductCommandRequest>(command_json)
                .expect("typed control request decodes");
            if request.scope != self.current_scope {
                return Ok(
                    serde_json::to_string(&AndroidProductCommandOutcome::Unavailable {
                        scope: Some(request.scope),
                        failure: Failure::new(
                            ErrorCode::NotReady,
                            "owner rejected a stale Core scope before command execution",
                            true,
                        ),
                    })
                    .expect("stale acknowledgment serializes"),
                );
            }
            self.applied_scopes
                .lock()
                .expect("scope lock")
                .push(request.scope.clone());
            Ok(
                serde_json::to_string(&AndroidProductCommandOutcome::Terminal(Box::new(
                    CoreCommandOutcome {
                        scope: Some(self.current_scope.clone()),
                        result: CommandResult::Completed {
                            request_id: RequestId(71),
                        },
                    },
                )))
                .expect("terminal acknowledgment serializes"),
            )
        }

        async fn credential_get(&self, _service: &str, _key: &str) -> Result<Option<String>> {
            Ok(None)
        }

        async fn credential_set(&self, _service: &str, _key: &str, _value: &str) -> Result<()> {
            Ok(())
        }

        async fn credential_delete(&self, _service: &str, _key: &str) -> Result<()> {
            Ok(())
        }

        fn data_dir(&self) -> Option<PathBuf> {
            None
        }

        fn cache_dir(&self) -> Option<PathBuf> {
            None
        }

        async fn vpn_start(&self) -> Result<VpnStartOutcome> {
            Ok(VpnStartOutcome::OwnerUnavailable)
        }

        async fn vpn_apply_configuration(
            &self,
            _prepared: &PreparedVpnConfiguration,
        ) -> Result<VpnConfigurationApplyOutcome> {
            Ok(VpnConfigurationApplyOutcome::OwnerUnavailable)
        }

        async fn vpn_stop(&self, _target: VpnStopTarget) -> Result<VpnOwnerStopReceipt> {
            self.events.lock().expect("event lock").push("vpn-stop");
            Ok(VpnOwnerStopReceipt::Stopped {
                snapshot: self
                    .terminal_vpn_snapshot
                    .lock()
                    .expect("terminal snapshot lock")
                    .clone(),
            })
        }

        async fn vpn_owner_snapshot(&self) -> Result<VpnOwnerSnapshot> {
            let read_index = self
                .vpn_snapshot_reads
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.events
                .lock()
                .expect("event lock")
                .push(if read_index == 0 {
                    "vpn-initial-readback"
                } else {
                    "vpn-terminal-readback"
                });
            Ok(if read_index == 0 {
                self.vpn_snapshot.clone()
            } else {
                self.terminal_vpn_snapshot
                    .lock()
                    .expect("terminal snapshot lock")
                    .clone()
            })
        }
    }

    fn scope(generation: u64) -> CoreScope {
        CoreScope {
            identity: CoreIdentity::new(
                EngineId::new("mihomo").expect("engine ID"),
                CoreInstanceId::new("android-active").expect("instance ID"),
                None,
            ),
            generation: CoreGeneration::new(generation),
            session_token: Some(infiltrator_contract::session::SessionToken::new(
                generation as u128,
            )),
        }
    }

    fn snapshot(scope: &CoreScope) -> AndroidCoreLifecycleSnapshot {
        AndroidCoreLifecycleSnapshot {
            identity: Some(scope.identity.clone()),
            lifecycle: CoreLifecycle::Running,
            generation: scope.generation.value(),
            session_token: scope.session_token,
            revision: scope.generation.value(),
            failure: None,
        }
    }

    fn core_snapshot_for_owner(snapshot: &AndroidCoreLifecycleSnapshot) -> CoreLifecycleSnapshot {
        CoreLifecycleSnapshot {
            identity: snapshot.identity.clone(),
            lifecycle: snapshot.lifecycle.clone(),
            generation: snapshot.generation,
            session_token: snapshot.session_token,
            revision: snapshot.revision,
            failure: snapshot.failure.clone(),
        }
    }

    fn stopped_vpn_snapshot() -> VpnOwnerSnapshot {
        VpnOwnerSnapshot {
            owner_session_id: "owner-a".to_owned(),
            revision: 7,
            vpn_generation: infiltrator_contract::vpn::VpnGeneration::new(5),
            vpn_state: VpnOwnerLifecycle::Stopped,
            readiness: VpnOwnerReadiness::Stopped,
            resources: [
                VpnOwnerResourceKind::PreparedConfiguration,
                VpnOwnerResourceKind::Worker,
                VpnOwnerResourceKind::TunnelDescriptor,
                VpnOwnerResourceKind::RoutingLease,
            ]
            .into_iter()
            .map(|kind| VpnOwnerResourceFact {
                kind,
                state: VpnOwnerResourceState::Absent,
            })
            .collect(),
            foreground: false,
            mtu: None,
            route_count: 0,
            dns_servers: Vec::new(),
            ipv6: false,
            core: VpnOwnerCoreReadback::Unavailable {
                failure_message: "Core readback not needed for VPN stop".to_owned(),
            },
        }
    }

    fn control_owner_snapshot(
        core: AndroidCoreLifecycleSnapshot,
        vpn_state: VpnOwnerLifecycle,
        readiness: VpnOwnerReadiness,
        revision: u64,
    ) -> VpnOwnerSnapshot {
        let vpn_generation = infiltrator_contract::vpn::VpnGeneration::new(5);
        let resources = [
            VpnOwnerResourceKind::PreparedConfiguration,
            VpnOwnerResourceKind::Worker,
            VpnOwnerResourceKind::TunnelDescriptor,
            VpnOwnerResourceKind::RoutingLease,
        ]
        .into_iter()
        .map(|kind| VpnOwnerResourceFact {
            kind,
            state: match (&vpn_state, readiness, kind) {
                (
                    VpnOwnerLifecycle::Running,
                    VpnOwnerReadiness::Ready,
                    VpnOwnerResourceKind::Worker | VpnOwnerResourceKind::TunnelDescriptor,
                ) => VpnOwnerResourceState::Held {
                    generation: vpn_generation.value(),
                },
                _ => VpnOwnerResourceState::Absent,
            },
        })
        .collect();
        let (foreground, mtu, route_count, dns_servers) = match (&vpn_state, readiness) {
            (VpnOwnerLifecycle::Running, VpnOwnerReadiness::Ready) => {
                (true, Some(1500), 1, vec!["1.1.1.1".to_owned()])
            }
            _ => (false, None, 0, Vec::new()),
        };
        VpnOwnerSnapshot {
            owner_session_id: "owner-session".to_owned(),
            revision,
            vpn_generation,
            vpn_state,
            readiness,
            resources,
            foreground,
            mtu,
            route_count,
            dns_servers,
            ipv6: false,
            core: VpnOwnerCoreReadback::Available(core_snapshot_for_owner(&core)),
        }
    }

    fn lifecycle_snapshot(
        scope: &CoreScope,
        lifecycle: CoreLifecycle,
    ) -> AndroidCoreLifecycleSnapshot {
        let mut snapshot = snapshot(scope);
        snapshot.lifecycle = lifecycle.clone();
        if lifecycle == CoreLifecycle::Stopped {
            snapshot.session_token = None;
        }
        snapshot
    }

    #[test]
    fn core_lifecycle_projection_preserves_every_contract_state() {
        let states = [
            (CoreLifecycle::Stopped, AndroidCoreLifecycle::Stopped),
            (CoreLifecycle::Starting, AndroidCoreLifecycle::Starting),
            (CoreLifecycle::Ready, AndroidCoreLifecycle::Ready),
            (CoreLifecycle::Running, AndroidCoreLifecycle::Running),
            (CoreLifecycle::Stopping, AndroidCoreLifecycle::Stopping),
            (CoreLifecycle::Failed, AndroidCoreLifecycle::Failed),
        ];

        for (contract, ffi) in states {
            assert_eq!(AndroidCoreLifecycle::from(contract), ffi);
        }
    }

    #[test]
    fn failed_core_snapshot_is_a_successful_typed_readback() {
        let scope = scope(17);
        let failure = Failure::new(
            ErrorCode::Configuration,
            "profile YAML could not be parsed",
            true,
        )
        .with_reason(infiltrator_contract::error::FailureReason::YamlSyntax {
            line: 23,
            column: 6,
        });
        let mut snapshot = snapshot(&scope);
        snapshot.lifecycle = CoreLifecycle::Failed;
        snapshot.revision = 29;
        snapshot.failure = Some(failure.clone());

        let result = core_lifecycle_result(snapshot);

        assert_eq!(result.status.code, FfiErrorCode::Ok);
        assert_eq!(result.lifecycle, Some(AndroidCoreLifecycle::Failed));
        assert_eq!(result.engine_id.as_deref(), Some("mihomo"));
        assert_eq!(result.core_instance_id.as_deref(), Some("android-active"));
        assert_eq!(result.generation, 17);
        assert_eq!(result.session_token.as_deref(), Some("17"));
        assert_eq!(result.revision, 29);
        assert_eq!(
            serde_json::from_str::<Failure>(
                result
                    .failure_json
                    .as_deref()
                    .expect("typed failure is carried"),
            )
            .expect("deserialize failure"),
            failure,
        );
    }

    #[test]
    fn stop_success_requires_a_terminal_owner_snapshot_with_released_resources() {
        let mut stopped = stopped_vpn_snapshot();
        assert_eq!(
            validate_android_vpn_stop_terminal_readback(&VpnOwnerStopReceipt::Stopped {
                snapshot: stopped.clone(),
            }),
            Ok(()),
        );

        stopped.resources[0].state = VpnOwnerResourceState::Held { generation: 5 };
        assert_eq!(
            validate_android_vpn_stop_terminal_readback(&VpnOwnerStopReceipt::Stopped {
                snapshot: stopped,
            })
            .expect_err("a terminal state with a held resource is not stopped")
            .code,
            ErrorCode::InvalidState,
        );
    }

    #[test]
    fn stop_failure_preserves_its_typed_failure_without_inventing_a_snapshot() {
        let failure = Failure::new(ErrorCode::NotReady, "owner stop timed out", true);
        assert_eq!(
            validate_android_vpn_stop_terminal_readback(&VpnOwnerStopReceipt::TimedOut {
                failure: failure.clone(),
            }),
            Err(failure),
        );
    }

    #[test]
    fn start_proxy_requires_core_readiness_and_running_data_plane() {
        let core_scope = scope(7);
        let core = lifecycle_snapshot(&core_scope, CoreLifecycle::Ready);
        let before = control_owner_snapshot(
            core.clone(),
            VpnOwnerLifecycle::Stopped,
            VpnOwnerReadiness::Stopped,
            4,
        );
        let after = control_owner_snapshot(
            core.clone(),
            VpnOwnerLifecycle::Running,
            VpnOwnerReadiness::Ready,
            5,
        );

        assert_eq!(
            validate_android_control_terminal_readback(
                AndroidProductControlIntent::StartProxy,
                &before,
                &after,
                core.scope().as_ref(),
            ),
            Ok(()),
        );

        let failed_core = lifecycle_snapshot(&core_scope, CoreLifecycle::Failed);
        let failed_after = control_owner_snapshot(
            failed_core.clone(),
            VpnOwnerLifecycle::Running,
            VpnOwnerReadiness::Ready,
            5,
        );
        assert_eq!(
            validate_android_control_terminal_readback(
                AndroidProductControlIntent::StartProxy,
                &before,
                &failed_after,
                failed_core.scope().as_ref(),
            )
            .expect_err("VPN Running cannot replace failed Core readback")
            .code,
            ErrorCode::InvalidState,
        );

        let mut worker_missing = control_owner_snapshot(
            core.clone(),
            VpnOwnerLifecycle::Running,
            VpnOwnerReadiness::Ready,
            5,
        );
        worker_missing
            .resources
            .retain(|resource| resource.kind != VpnOwnerResourceKind::Worker);
        assert_eq!(
            validate_android_control_terminal_readback(
                AndroidProductControlIntent::StartProxy,
                &before,
                &worker_missing,
                core.scope().as_ref(),
            )
            .expect_err("Running without an owner-held packet worker is not data-plane ready")
            .code,
            ErrorCode::InvalidState,
        );
    }

    #[test]
    fn resume_kernel_requires_ready_core_and_preserves_vpn_activation() {
        let stopped_scope = scope(7);
        let stopped_core = lifecycle_snapshot(&stopped_scope, CoreLifecycle::Stopped);
        let before = control_owner_snapshot(
            stopped_core,
            VpnOwnerLifecycle::Stopped,
            VpnOwnerReadiness::Stopped,
            4,
        );
        let resumed_scope = scope(8);
        let resumed_core = lifecycle_snapshot(&resumed_scope, CoreLifecycle::Ready);
        let after = control_owner_snapshot(
            resumed_core.clone(),
            VpnOwnerLifecycle::Stopped,
            VpnOwnerReadiness::Stopped,
            5,
        );

        assert_eq!(
            validate_android_control_terminal_readback(
                AndroidProductControlIntent::ResumeKernel,
                &before,
                &after,
                resumed_core.scope().as_ref(),
            ),
            Ok(()),
        );

        let vpn_started = control_owner_snapshot(
            resumed_core.clone(),
            VpnOwnerLifecycle::Running,
            VpnOwnerReadiness::Ready,
            5,
        );
        assert_eq!(
            validate_android_control_terminal_readback(
                AndroidProductControlIntent::ResumeKernel,
                &before,
                &vpn_started,
                resumed_core.scope().as_ref(),
            )
            .expect_err("ResumeKernel must not start the VPN data plane")
            .code,
            ErrorCode::InvalidState,
        );
    }

    #[test]
    fn pause_kernel_requires_stopped_core_and_fully_released_vpn_resources() {
        let running_scope = scope(8);
        let running_core = lifecycle_snapshot(&running_scope, CoreLifecycle::Running);
        let before = control_owner_snapshot(
            running_core.clone(),
            VpnOwnerLifecycle::Running,
            VpnOwnerReadiness::Ready,
            4,
        );
        let mut stopped_core = running_core.clone();
        stopped_core.lifecycle = CoreLifecycle::Stopped;
        stopped_core.session_token = None;
        stopped_core.revision += 1;
        let after = control_owner_snapshot(
            stopped_core.clone(),
            VpnOwnerLifecycle::Stopped,
            VpnOwnerReadiness::Stopped,
            5,
        );

        assert_eq!(
            validate_android_control_terminal_readback(
                AndroidProductControlIntent::PauseKernel,
                &before,
                &after,
                stopped_core.scope().as_ref(),
            ),
            Ok(()),
        );

        let core_still_running = control_owner_snapshot(
            running_core.clone(),
            VpnOwnerLifecycle::Stopped,
            VpnOwnerReadiness::Stopped,
            5,
        );
        assert_eq!(
            validate_android_control_terminal_readback(
                AndroidProductControlIntent::PauseKernel,
                &before,
                &core_still_running,
                running_core.scope().as_ref(),
            )
            .expect_err("VPN stop alone does not complete PauseKernel")
            .code,
            ErrorCode::InvalidState,
        );
    }

    #[test]
    fn restart_kernel_requires_a_new_core_session_and_stopped_vpn() {
        let prior_scope = scope(8);
        let prior_core = lifecycle_snapshot(&prior_scope, CoreLifecycle::Running);
        let before = control_owner_snapshot(
            prior_core,
            VpnOwnerLifecycle::Running,
            VpnOwnerReadiness::Ready,
            4,
        );
        let restarted_scope = scope(9);
        let restarted_core = lifecycle_snapshot(&restarted_scope, CoreLifecycle::Ready);
        let after = control_owner_snapshot(
            restarted_core.clone(),
            VpnOwnerLifecycle::Stopped,
            VpnOwnerReadiness::Stopped,
            5,
        );

        assert_eq!(
            validate_android_control_terminal_readback(
                AndroidProductControlIntent::RestartKernel,
                &before,
                &after,
                restarted_core.scope().as_ref(),
            ),
            Ok(()),
        );

        let vpn_still_running = control_owner_snapshot(
            restarted_core.clone(),
            VpnOwnerLifecycle::Running,
            VpnOwnerReadiness::Ready,
            5,
        );
        assert_eq!(
            validate_android_control_terminal_readback(
                AndroidProductControlIntent::RestartKernel,
                &before,
                &vpn_still_running,
                restarted_core.scope().as_ref(),
            )
            .expect_err("Core Running alone must not claim the proxy is running")
            .code,
            ErrorCode::InvalidState,
        );
    }

    #[tokio::test]
    async fn ui_control_reads_owner_terminal_facts_after_the_command() {
        let current = scope(7);
        let bridge = FakeOwnerTransport::new(snapshot(&current), current.clone());

        let status = execute_control_intent_with_bridge(
            &bridge,
            AndroidControlIntent::StartProxy,
            ControlCommandDispatch::ProvidedBridge,
        )
        .await;

        assert_eq!(status.code, FfiErrorCode::Ok);
        assert_eq!(
            bridge.events(),
            vec!["vpn-initial-readback", "command", "vpn-terminal-readback"]
        );
        assert_eq!(bridge.applied_scopes(), vec![current]);
    }

    #[tokio::test]
    async fn stop_proxy_uses_dedicated_stopped_receipt_without_core_command() {
        let current = scope(7);
        let bridge = FakeOwnerTransport::new(snapshot(&current), current);
        {
            let mut terminal = bridge
                .terminal_vpn_snapshot
                .lock()
                .expect("terminal snapshot lock");
            *terminal = control_owner_snapshot(
                snapshot(&scope(7)),
                VpnOwnerLifecycle::Stopped,
                VpnOwnerReadiness::Stopped,
                5,
            );
        }

        let status = execute_control_intent_with_bridge(
            &bridge,
            AndroidControlIntent::StopProxy,
            ControlCommandDispatch::ProvidedBridge,
        )
        .await;

        assert_eq!(status.code, FfiErrorCode::Ok);
        assert_eq!(bridge.events(), vec!["vpn-stop"]);
        assert!(bridge.applied_scopes().is_empty());
    }

    #[tokio::test]
    async fn completed_start_without_ready_owner_data_plane_is_not_success() {
        let current = scope(7);
        let bridge = FakeOwnerTransport::new(snapshot(&current), current);
        {
            let mut terminal = bridge
                .terminal_vpn_snapshot
                .lock()
                .expect("terminal snapshot lock");
            terminal.vpn_state = VpnOwnerLifecycle::TunnelEstablished;
            terminal.readiness = VpnOwnerReadiness::AwaitingDataPlane;
            terminal.revision += 1;
        }

        let status = execute_control_intent_with_bridge(
            &bridge,
            AndroidControlIntent::StartProxy,
            ControlCommandDispatch::ProvidedBridge,
        )
        .await;

        assert_eq!(status.code, FfiErrorCode::InvalidState);
        assert_eq!(
            status.message.as_deref(),
            Some("StartProxy completed without owner-confirmed VPN data-plane readiness"),
        );
        assert_eq!(
            bridge.events(),
            vec!["vpn-initial-readback", "command", "vpn-terminal-readback"]
        );
        assert_eq!(bridge.applied_scopes().len(), 1);
    }

    #[tokio::test]
    async fn unavailable_vpn_owner_readback_prevents_a_new_command() {
        let current = scope(8);
        let mut bridge = FakeOwnerTransport::new(snapshot(&current), current);
        bridge.vpn_snapshot.vpn_state = VpnOwnerLifecycle::Unavailable {
            failure_message: "owner unavailable".to_owned(),
        };
        bridge.vpn_snapshot.readiness = VpnOwnerReadiness::Unavailable;

        let status = execute_control_intent_with_bridge(
            &bridge,
            AndroidControlIntent::RestartKernel,
            ControlCommandDispatch::ProvidedBridge,
        )
        .await;

        assert_eq!(status.code, FfiErrorCode::NotReady);
        assert_eq!(bridge.events(), vec!["vpn-initial-readback"]);
        assert!(bridge.applied_scopes().is_empty());
        assert_eq!(status.message.as_deref(), Some("owner unavailable"));
    }

    #[tokio::test]
    async fn stale_core_scope_reply_is_typed_and_has_no_owner_side_effect() {
        let current = scope(9);
        let stale = scope(8);
        let bridge = FakeOwnerTransport::new(snapshot(&stale), current);

        let status = execute_control_intent_with_bridge(
            &bridge,
            AndroidControlIntent::PauseKernel,
            ControlCommandDispatch::ProvidedBridge,
        )
        .await;

        assert_eq!(status.code, FfiErrorCode::NotReady);
        assert_eq!(bridge.events(), vec!["vpn-initial-readback", "command"]);
        assert!(bridge.applied_scopes().is_empty());
        assert_eq!(
            status.message.as_deref(),
            Some("owner rejected a stale Core scope before command execution")
        );
    }

    #[test]
    fn owner_session_change_invalidates_a_control_terminal_readback() {
        let core_scope = scope(8);
        let core = lifecycle_snapshot(&core_scope, CoreLifecycle::Ready);
        let before = control_owner_snapshot(
            core.clone(),
            VpnOwnerLifecycle::Stopped,
            VpnOwnerReadiness::Stopped,
            4,
        );
        let mut after = control_owner_snapshot(
            core.clone(),
            VpnOwnerLifecycle::Running,
            VpnOwnerReadiness::Ready,
            5,
        );
        after.owner_session_id = "restarted-owner-session".to_owned();

        let failure = validate_android_control_terminal_readback(
            AndroidProductControlIntent::StartProxy,
            &before,
            &after,
            core.scope().as_ref(),
        )
        .expect_err("a replaced owner cannot confirm the previous command");

        assert_eq!(failure.code, ErrorCode::InvalidState);
        assert!(failure.message.contains("owner session changed"));
    }

    #[test]
    fn changed_owner_facts_without_revision_advance_are_rejected() {
        let core_scope = scope(8);
        let core = lifecycle_snapshot(&core_scope, CoreLifecycle::Ready);
        let before = control_owner_snapshot(
            core.clone(),
            VpnOwnerLifecycle::Stopped,
            VpnOwnerReadiness::Stopped,
            4,
        );
        let after = control_owner_snapshot(
            core.clone(),
            VpnOwnerLifecycle::Running,
            VpnOwnerReadiness::Ready,
            4,
        );

        let failure = validate_android_control_terminal_readback(
            AndroidProductControlIntent::StartProxy,
            &before,
            &after,
            core.scope().as_ref(),
        )
        .expect_err("changed terminal facts must advance the owner revision");

        assert_eq!(failure.code, ErrorCode::InvalidState);
        assert!(failure.message.contains("without advancing its revision"));
    }

    #[test]
    fn terminal_command_scope_must_match_the_owner_core_generation() {
        let core_scope = scope(9);
        let core = lifecycle_snapshot(&core_scope, CoreLifecycle::Ready);
        let before = control_owner_snapshot(
            core.clone(),
            VpnOwnerLifecycle::Stopped,
            VpnOwnerReadiness::Stopped,
            4,
        );
        let after = control_owner_snapshot(
            core.clone(),
            VpnOwnerLifecycle::Running,
            VpnOwnerReadiness::Ready,
            5,
        );
        let stale_scope = scope(8);

        let failure = validate_android_control_terminal_readback(
            AndroidProductControlIntent::StartProxy,
            &before,
            &after,
            Some(&stale_scope),
        )
        .expect_err("a command result from an older Core generation is stale");

        assert_eq!(failure.code, ErrorCode::InvalidState);
        assert!(failure.message.contains("different Core generations"));
    }
}

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

use super::{
    AndroidControlIntent, AndroidCoreLifecycle, ControlCommandDispatch, core_lifecycle_result,
    execute_control_intent_with_bridge,
};
use crate::ffi::FfiErrorCode;
use async_trait::async_trait;
use infiltrator_contract::android_control::core_launch::CoreLaunchSpec;
use infiltrator_contract::android_control::{
    AndroidCoreLifecycleSnapshot, AndroidCoreProcessObservation,
    AndroidCoreProcessUnavailableReason, AndroidHostProcessRole, AndroidProductCommandOutcome,
    AndroidProductCommandRequest, AndroidProductControlIntent, VpnOwnerCoreReadback,
    VpnOwnerLifecycle, VpnOwnerReadiness, VpnOwnerResourceFact, VpnOwnerResourceKind,
    VpnOwnerResourceState, VpnOwnerSnapshot, validate_android_control_terminal_readback,
    validate_android_vpn_stop_terminal_readback,
};
use infiltrator_contract::command::{CommandResult, RequestId};
use infiltrator_contract::core_identity::CoreCommandOutcome;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};
use infiltrator_contract::vpn::{
    PreparedVpnConfiguration, VpnConfigurationApplyOutcome, VpnGeneration, VpnOwnerStopReceipt,
    VpnStartOutcome, VpnStopTarget,
};
use infiltrator_platform::android_bridge::{
    AndroidBridge, AndroidBridgeResult, CoreProcessStartReceipt, CoreProcessStopReceipt,
};
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::error::{ErrorCode, Failure, FailureReason};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
};
use lurker_core_contract::session::SessionToken;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

struct FakeOwnerTransport {
    snapshot: AndroidCoreLifecycleSnapshot,
    current_scope: CoreScope,
    vpn_snapshot: VpnOwnerSnapshot,
    terminal_vpn_snapshot: Mutex<VpnOwnerSnapshot>,
    vpn_snapshot_reads: AtomicUsize,
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
            vpn_snapshot_reads: AtomicUsize::new(0),
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
        _context: &CoreProcessStartContext,
    ) -> AndroidBridgeResult<CoreProcessStartReceipt> {
        Ok(CoreProcessStartReceipt::NotVpnOwner)
    }

    async fn core_stop(&self) -> AndroidBridgeResult<CoreProcessStopReceipt> {
        Ok(CoreProcessStopReceipt::NotVpnOwner)
    }

    async fn core_process_observation(&self) -> AndroidBridgeResult<AndroidCoreProcessObservation> {
        Ok(AndroidCoreProcessObservation::Unavailable {
            reason: AndroidCoreProcessUnavailableReason::ProcessInspectionUnavailable,
        })
    }

    async fn core_lifecycle_snapshot(&self) -> AndroidBridgeResult<AndroidCoreLifecycleSnapshot> {
        self.events
            .lock()
            .expect("event lock")
            .push("core-readback");
        Ok(self.snapshot.clone())
    }

    async fn execute_product_command_json(
        &self,
        command_json: &str,
    ) -> AndroidBridgeResult<String> {
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

    async fn credential_get(
        &self,
        _service: &str,
        _key: &str,
    ) -> AndroidBridgeResult<Option<String>> {
        Ok(None)
    }

    async fn credential_set(
        &self,
        _service: &str,
        _key: &str,
        _value: &str,
    ) -> AndroidBridgeResult<()> {
        Ok(())
    }

    async fn credential_delete(&self, _service: &str, _key: &str) -> AndroidBridgeResult<()> {
        Ok(())
    }

    fn data_dir(&self) -> Option<PathBuf> {
        None
    }

    fn cache_dir(&self) -> Option<PathBuf> {
        None
    }

    async fn vpn_start(&self) -> AndroidBridgeResult<VpnStartOutcome> {
        Ok(VpnStartOutcome::OwnerUnavailable)
    }

    async fn vpn_apply_configuration(
        &self,
        _prepared: &PreparedVpnConfiguration,
    ) -> AndroidBridgeResult<VpnConfigurationApplyOutcome> {
        Ok(VpnConfigurationApplyOutcome::OwnerUnavailable)
    }

    async fn vpn_stop(&self, _target: VpnStopTarget) -> AndroidBridgeResult<VpnOwnerStopReceipt> {
        self.events.lock().expect("event lock").push("vpn-stop");
        Ok(VpnOwnerStopReceipt::Stopped {
            snapshot: self
                .terminal_vpn_snapshot
                .lock()
                .expect("terminal snapshot lock")
                .clone(),
        })
    }

    async fn vpn_owner_snapshot(&self) -> AndroidBridgeResult<VpnOwnerSnapshot> {
        let read_index = self.vpn_snapshot_reads.fetch_add(1, Ordering::SeqCst);
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
        session_token: Some(SessionToken::new(generation as u128)),
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
        vpn_generation: VpnGeneration::new(5),
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
    let vpn_generation = VpnGeneration::new(5);
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

fn lifecycle_snapshot(scope: &CoreScope, lifecycle: CoreLifecycle) -> AndroidCoreLifecycleSnapshot {
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
    .with_reason(FailureReason::YamlSyntax {
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

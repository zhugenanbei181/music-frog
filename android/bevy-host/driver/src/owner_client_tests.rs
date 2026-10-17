use super::*;
use infiltrator_contract::android_control::{
    AndroidProductControlIntent, VpnOwnerCoreReadback, VpnOwnerLifecycle, VpnOwnerReadiness,
    VpnOwnerResourceFact, VpnOwnerResourceKind, VpnOwnerResourceState, VpnOwnerSnapshot,
};
use infiltrator_contract::snapshot::CoreSnapshot;
use infiltrator_contract::vpn::{VpnGeneration, VpnOwnerStopReceipt};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
};
use lurker_core_contract::lifecycle::{CoreLifecycle, CoreLifecycleSnapshot};
use lurker_core_contract::routing_intent::RoutingIntentSnapshot;
use std::sync::atomic::AtomicBool;

struct FakeOwnerTransport {
    snapshot: Mutex<Option<String>>,
    outcome: Mutex<String>,
    owner_snapshots: Mutex<VecDeque<VpnOwnerSnapshot>>,
    stop_receipt: Mutex<VpnOwnerStopReceipt>,
    commands_received: Mutex<Vec<AndroidProductCommand>>,
    commands: AtomicU64,
    block_commands: AtomicBool,
    stop_requests: AtomicU64,
    events: Mutex<Vec<&'static str>>,
}

impl OwnerTransport for FakeOwnerTransport {
    fn read_surface_snapshot_json(&self) -> Result<Option<String>, Failure> {
        Ok(self.snapshot.lock().expect("snapshot lock").clone())
    }

    fn read_vpn_owner_snapshot(&self) -> Result<VpnOwnerSnapshot, Failure> {
        self.events.lock().expect("event lock").push("vpn-readback");
        self.owner_snapshots
            .lock()
            .expect("VPN snapshot queue lock")
            .pop_front()
            .ok_or_else(|| Failure::new(ErrorCode::NotReady, "fake VPN snapshot queue empty", true))
    }

    fn execute_product_command_json(&self, request_json: &str) -> Result<String, Failure> {
        self.events.lock().expect("event lock").push("command");
        self.commands.fetch_add(1, Ordering::SeqCst);
        let request = serde_json::from_str::<AndroidProductCommandRequest>(request_json)
            .expect("typed command request");
        assert!(
            request
                .scope
                .identity
                .instance
                .as_str()
                .starts_with("owner-")
        );
        self.commands_received
            .lock()
            .expect("received commands lock")
            .push(request.command);
        while self.block_commands.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(1));
        }
        Ok(self.outcome.lock().expect("outcome lock").clone())
    }

    fn stop_vpn_current(&self) -> Result<VpnOwnerStopReceipt, Failure> {
        self.events.lock().expect("event lock").push("vpn-stop");
        self.stop_requests.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .stop_receipt
            .lock()
            .expect("VPN stop receipt lock")
            .clone())
    }
}

fn core_scope(instance: &str, generation: u64, token: Option<u128>) -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::new("mihomo").expect("engine ID"),
            CoreInstanceId::new(instance).expect("instance ID"),
            None,
        ),
        generation: CoreGeneration::new(generation),
        session_token: token.map(lurker_core_contract::session::SessionToken::new),
    }
}

fn owner_snapshot(scope: &CoreScope, revision: u64) -> SurfaceSnapshot {
    SurfaceSnapshot {
        surface: SurfaceKind::BevyAndroid,
        origin: SurfaceOrigin::Live,
        generation: scope.generation.value(),
        revision,
        core: CoreSnapshot {
            identity: Some(scope.identity.clone()),
            lifecycle: CoreLifecycle::Running,
            generation: scope.generation.value(),
            session_token: scope.session_token,
            revision,
            routing_intent: RoutingIntentSnapshot::demo_fixture(),
            core_version: Some("1.19.12".to_owned()),
            sampled_at_epoch_ms: Some(1),
            failure: None,
            upload_bps: 1.0,
            download_bps: 2.0,
            active_connections: 0,
            watchdog: Default::default(),
        },
        ..SurfaceSnapshot::unavailable(
            SurfaceKind::BevyAndroid,
            HostKind::Android,
            Failure::new(ErrorCode::NotReady, "owner starting", true),
        )
    }
}

fn vpn_owner_snapshot(
    core: VpnOwnerCoreReadback,
    revision: u64,
    vpn_state: VpnOwnerLifecycle,
    readiness: VpnOwnerReadiness,
) -> VpnOwnerSnapshot {
    vpn_owner_snapshot_at_generation(core, revision, vpn_state, readiness, 5)
}

fn vpn_owner_snapshot_at_generation(
    core: VpnOwnerCoreReadback,
    revision: u64,
    vpn_state: VpnOwnerLifecycle,
    readiness: VpnOwnerReadiness,
    vpn_generation: u64,
) -> VpnOwnerSnapshot {
    let vpn_generation = VpnGeneration::new(vpn_generation);
    let running = matches!(
        (&vpn_state, readiness),
        (VpnOwnerLifecycle::Running, VpnOwnerReadiness::Ready)
    );
    VpnOwnerSnapshot {
        owner_session_id: "bevy-owner-session".to_owned(),
        revision,
        vpn_generation,
        vpn_state: vpn_state.clone(),
        readiness,
        resources: [
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
        .collect(),
        foreground: running,
        mtu: running.then_some(1500),
        route_count: if running { 1 } else { 0 },
        dns_servers: if running {
            vec!["1.1.1.1".to_owned()]
        } else {
            Vec::new()
        },
        ipv6: false,
        core,
    }
}

#[derive(Clone, Copy)]
enum ControlScenarioKind {
    StartProxy,
    ResumeKernel,
    PauseKernel,
    RestartKernel,
}

struct ControlScenario {
    ui_command: UiCommand,
    expected_intent: AndroidProductControlIntent,
    request_scope: CoreScope,
    terminal_scope: CoreScope,
    before: VpnOwnerSnapshot,
    after: VpnOwnerSnapshot,
}

fn control_scenario(kind: ControlScenarioKind) -> ControlScenario {
    let (
        ui_command,
        expected_intent,
        before_scope,
        before_lifecycle,
        before_vpn,
        before_readiness,
        before_vpn_generation,
        after_scope,
        after_lifecycle,
        after_vpn,
        after_readiness,
        after_vpn_generation,
    ) = match kind {
        ControlScenarioKind::StartProxy => (
            UiCommand::StartVpn,
            AndroidProductControlIntent::StartProxy,
            core_scope("owner-bevy", 7, None),
            CoreLifecycle::Stopped,
            VpnOwnerLifecycle::Stopped,
            VpnOwnerReadiness::Stopped,
            5,
            core_scope("owner-bevy", 8, Some(80)),
            CoreLifecycle::Ready,
            VpnOwnerLifecycle::Running,
            VpnOwnerReadiness::Ready,
            6,
        ),
        ControlScenarioKind::ResumeKernel => (
            UiCommand::StartCore,
            AndroidProductControlIntent::ResumeKernel,
            core_scope("owner-bevy", 7, None),
            CoreLifecycle::Stopped,
            VpnOwnerLifecycle::Stopped,
            VpnOwnerReadiness::Stopped,
            5,
            core_scope("owner-bevy", 8, Some(80)),
            CoreLifecycle::Ready,
            VpnOwnerLifecycle::Stopped,
            VpnOwnerReadiness::Stopped,
            5,
        ),
        ControlScenarioKind::PauseKernel => (
            UiCommand::StopCore,
            AndroidProductControlIntent::PauseKernel,
            core_scope("owner-bevy", 8, Some(80)),
            CoreLifecycle::Running,
            VpnOwnerLifecycle::Running,
            VpnOwnerReadiness::Ready,
            5,
            core_scope("owner-bevy", 8, None),
            CoreLifecycle::Stopped,
            VpnOwnerLifecycle::Stopped,
            VpnOwnerReadiness::Stopped,
            5,
        ),
        ControlScenarioKind::RestartKernel => (
            UiCommand::RestartCore,
            AndroidProductControlIntent::RestartKernel,
            core_scope("owner-bevy", 8, Some(80)),
            CoreLifecycle::Running,
            VpnOwnerLifecycle::Running,
            VpnOwnerReadiness::Ready,
            5,
            core_scope("owner-bevy", 9, Some(90)),
            CoreLifecycle::Ready,
            VpnOwnerLifecycle::Stopped,
            VpnOwnerReadiness::Stopped,
            5,
        ),
    };
    let request_scope = before_scope.clone();
    let before_core = core_lifecycle_readback(&before_scope, before_lifecycle, 10);
    let terminal_scope = after_scope.clone();
    let after_core = core_lifecycle_readback(&after_scope, after_lifecycle, 11);
    ControlScenario {
        ui_command,
        expected_intent,
        request_scope,
        terminal_scope,
        before: vpn_owner_snapshot_at_generation(
            before_core,
            10,
            before_vpn,
            before_readiness,
            before_vpn_generation,
        ),
        after: vpn_owner_snapshot_at_generation(
            after_core,
            11,
            after_vpn,
            after_readiness,
            after_vpn_generation,
        ),
    }
}

fn core_lifecycle_readback(
    scope: &CoreScope,
    lifecycle: CoreLifecycle,
    revision: u64,
) -> VpnOwnerCoreReadback {
    VpnOwnerCoreReadback::Available(CoreLifecycleSnapshot {
        identity: Some(scope.identity.clone()),
        lifecycle,
        generation: scope.generation.value(),
        session_token: scope.session_token,
        revision,
        failure: None,
    })
}

fn core_owner_readback(snapshot: &SurfaceSnapshot) -> VpnOwnerCoreReadback {
    VpnOwnerCoreReadback::Available(CoreLifecycleSnapshot {
        identity: snapshot.core.identity.clone(),
        lifecycle: snapshot.core.lifecycle.clone(),
        generation: snapshot.core.generation,
        session_token: snapshot.core.session_token,
        revision: snapshot.core.revision,
        failure: snapshot.core.failure.clone(),
    })
}

fn command_outcome(scope: Option<CoreScope>, result: CommandResult) -> String {
    serde_json::to_string(&AndroidProductCommandOutcome::Terminal(Box::new(
        infiltrator_contract::core_identity::CoreCommandOutcome { scope, result },
    )))
    .expect("serialize command outcome")
}

fn fake_transport(snapshot: Option<SurfaceSnapshot>, outcome: String) -> Arc<FakeOwnerTransport> {
    let core = snapshot
        .as_ref()
        .map(core_owner_readback)
        .unwrap_or_else(|| VpnOwnerCoreReadback::Unavailable {
            failure_message: "Core readback unavailable".to_owned(),
        });
    let owner_snapshot = vpn_owner_snapshot(
        core,
        1,
        VpnOwnerLifecycle::Stopped,
        VpnOwnerReadiness::Stopped,
    );
    Arc::new(FakeOwnerTransport {
        snapshot: Mutex::new(
            snapshot.map(|snapshot| serde_json::to_string(&snapshot).expect("snapshot JSON")),
        ),
        outcome: Mutex::new(outcome),
        owner_snapshots: Mutex::new(VecDeque::from([
            owner_snapshot.clone(),
            owner_snapshot.clone(),
        ])),
        stop_receipt: Mutex::new(VpnOwnerStopReceipt::Stopped {
            snapshot: owner_snapshot,
        }),
        commands_received: Mutex::new(Vec::new()),
        commands: AtomicU64::new(0),
        block_commands: AtomicBool::new(false),
        stop_requests: AtomicU64::new(0),
        events: Mutex::new(Vec::new()),
    })
}

fn test_policy() -> OwnerClientPolicy {
    OwnerClientPolicy {
        surface_interval: Duration::from_millis(5),
        command_timeout: Duration::from_millis(100),
    }
}

#[test]
fn owner_snapshot_decoder_requires_a_scoped_live_bevy_android_surface() {
    let unscoped = SurfaceSnapshot::unavailable(
        SurfaceKind::BevyAndroid,
        HostKind::Android,
        Failure::new(ErrorCode::NotReady, "owner is starting", true),
    );
    let unscoped_json = serde_json::to_string(&unscoped).expect("snapshot JSON");
    assert_eq!(
        parse_owner_snapshot(&unscoped_json)
            .expect_err("unscoped snapshot must be rejected")
            .code,
        ErrorCode::NotReady
    );

    let scope = core_scope("owner-bevy", 7, Some(70));
    let valid = owner_snapshot(&scope, 1);
    let valid_json = serde_json::to_string(&valid).expect("snapshot JSON");
    assert_eq!(parse_owner_snapshot(&valid_json), Ok(valid.clone()));

    let mut demo_snapshot = valid.clone();
    demo_snapshot.origin = SurfaceOrigin::Demo;
    let demo_json = serde_json::to_string(&demo_snapshot).expect("snapshot JSON");
    assert_eq!(
        parse_owner_snapshot(&demo_json)
            .expect_err("demo data must not enter the Android owner surface")
            .code,
        ErrorCode::InvalidInput
    );

    let mut wrong_surface = valid;
    wrong_surface.surface = SurfaceKind::AndroidCompose;
    let wrong_surface_json = serde_json::to_string(&wrong_surface).expect("snapshot JSON");
    assert_eq!(
        parse_owner_snapshot(&wrong_surface_json)
            .expect_err("Compose owner data must not enter the Bevy surface")
            .code,
        ErrorCode::InvalidInput
    );
}

#[test]
fn client_rejects_commands_until_owner_publishes_a_scoped_snapshot() {
    let outcome = command_outcome(
        None,
        CommandResult::Completed {
            request_id: RequestId::new(1),
        },
    );
    let transport = fake_transport(None, outcome);
    let client = AndroidOwnerClient::start_with_policy(transport.clone(), test_policy())
        .expect("start owner worker");

    assert!(
        client
            .submit_tracked(UiCommand::CloseAllConnections)
            .is_none()
    );
    thread::sleep(Duration::from_millis(10));
    assert_eq!(transport.commands.load(Ordering::SeqCst), 0);
}

#[test]
fn client_sends_kernel_resume_as_a_typed_owner_control_command() {
    let scope = core_scope("owner-bevy", 7, Some(70));
    let transport = fake_transport(
        Some(owner_snapshot(&scope, 1)),
        command_outcome(
            Some(scope.clone()),
            CommandResult::Completed {
                request_id: RequestId::new(1),
            },
        ),
    );
    let client = AndroidOwnerClient::start_with_policy(transport.clone(), test_policy())
        .expect("start owner worker");
    wait_for_scope(&client, &scope);

    let request_id = client
        .submit_tracked(UiCommand::StartCore)
        .expect("observed owner scope admits kernel resume");
    let result = wait_for_result(&client);

    assert_eq!(result.request_id, request_id);
    assert!(result.result.is_ok());
    let commands = transport
        .commands_received
        .lock()
        .expect("received commands lock");
    assert_eq!(
        commands.as_slice(),
        &[AndroidProductCommand::Control(
            AndroidProductControlIntent::ResumeKernel
        )]
    );
    assert_eq!(
        transport.events.lock().expect("event lock").as_slice(),
        &["vpn-readback", "command", "vpn-readback"],
    );
}

#[test]
fn bevy_control_intents_share_the_owner_terminal_gate_for_each_lifecycle_semantic() {
    for kind in [
        ControlScenarioKind::StartProxy,
        ControlScenarioKind::ResumeKernel,
        ControlScenarioKind::PauseKernel,
        ControlScenarioKind::RestartKernel,
    ] {
        let scenario = control_scenario(kind);
        let command = product_command(&scenario.ui_command).expect("typed lifecycle command");
        assert_eq!(
            command,
            AndroidProductCommand::Control(scenario.expected_intent),
        );
        let transport = fake_transport(
            Some(owner_snapshot(&scenario.request_scope, 10)),
            command_outcome(
                Some(scenario.terminal_scope.clone()),
                CommandResult::Completed {
                    request_id: RequestId::new(1),
                },
            ),
        );
        *transport
            .owner_snapshots
            .lock()
            .expect("VPN snapshot queue lock") = VecDeque::from([scenario.before, scenario.after]);

        let result = execute_product_command(&*transport, &scenario.request_scope, &command);

        assert_eq!(result, Ok(CommandOutput::Unit));
        assert_eq!(
            transport.events.lock().expect("event lock").as_slice(),
            &["vpn-readback", "command", "vpn-readback"],
        );
    }
    assert_eq!(
        product_command(&UiCommand::StopCore).and_then(|command| match command {
            AndroidProductCommand::Control(intent) => Some(intent),
            AndroidProductCommand::Application(_) => None,
        }),
        Some(AndroidProductControlIntent::PauseKernel),
    );
}

#[test]
fn client_does_not_treat_a_restarted_core_as_a_running_proxy() {
    let scenario = control_scenario(ControlScenarioKind::RestartKernel);
    let command = product_command(&scenario.ui_command).expect("typed lifecycle command");
    let mut terminal_core = scenario.after.core.clone();
    if let VpnOwnerCoreReadback::Available(core) = &mut terminal_core {
        core.lifecycle = CoreLifecycle::Running;
    }
    let transport = fake_transport(
        Some(owner_snapshot(&scenario.request_scope, 10)),
        command_outcome(
            Some(scenario.terminal_scope.clone()),
            CommandResult::Completed {
                request_id: RequestId::new(1),
            },
        ),
    );
    *transport
        .owner_snapshots
        .lock()
        .expect("VPN snapshot queue lock") = VecDeque::from([
        scenario.before,
        vpn_owner_snapshot_at_generation(
            terminal_core,
            12,
            VpnOwnerLifecycle::Running,
            VpnOwnerReadiness::Ready,
            5,
        ),
    ]);

    let result = execute_product_command(&*transport, &scenario.request_scope, &command);

    assert_eq!(
        result.as_ref().err().map(|failure| failure.code.clone()),
        Some(ErrorCode::InvalidState),
    );
}

#[test]
fn client_stops_vpn_through_current_owner_without_a_core_scope_or_product_command() {
    let transport = fake_transport(None, String::new());
    let client = AndroidOwnerClient::start_with_policy(transport.clone(), test_policy())
        .expect("start owner worker");
    assert_eq!(product_command(&UiCommand::StopVpn), None);

    let request_id = client
        .submit_tracked(UiCommand::StopVpn)
        .expect("VPN stop does not require a CoreScope snapshot");
    let event = wait_for_result(&client);

    assert_eq!(event.request_id, request_id);
    assert_eq!(event.result, Ok(CommandOutput::Unit));
    assert_eq!(transport.stop_requests.load(Ordering::SeqCst), 1);
    assert_eq!(transport.commands.load(Ordering::SeqCst), 0);
    assert_eq!(
        transport.events.lock().expect("event lock").as_slice(),
        &["vpn-stop"],
    );
}

#[test]
fn client_keeps_superseded_cancelled_and_failed_vpn_stops_terminal() {
    let snapshot = vpn_owner_snapshot(
        VpnOwnerCoreReadback::Unavailable {
            failure_message: "Core readback unavailable".to_owned(),
        },
        1,
        VpnOwnerLifecycle::Stopped,
        VpnOwnerReadiness::Stopped,
    );
    let failure = Failure::new(ErrorCode::NotReady, "stop did not complete", true);
    let receipts = [
        VpnOwnerStopReceipt::OwnerUnavailable {
            failure: failure.clone(),
        },
        VpnOwnerStopReceipt::OwnerUnavailableWithReadback {
            failure: failure.clone(),
            snapshot: snapshot.clone(),
        },
        VpnOwnerStopReceipt::Failed {
            failure: failure.clone(),
        },
        VpnOwnerStopReceipt::FailedWithReadback {
            failure: failure.clone(),
            snapshot: snapshot.clone(),
        },
        VpnOwnerStopReceipt::TimedOut {
            failure: failure.clone(),
        },
        VpnOwnerStopReceipt::TimedOutWithReadback {
            failure: failure.clone(),
            snapshot: snapshot.clone(),
        },
        VpnOwnerStopReceipt::Superseded {
            failure: failure.clone(),
        },
        VpnOwnerStopReceipt::SupersededWithReadback {
            failure: failure.clone(),
            snapshot: snapshot.clone(),
        },
        VpnOwnerStopReceipt::Canceled {
            failure: failure.clone(),
        },
        VpnOwnerStopReceipt::CanceledWithReadback { failure, snapshot },
    ];
    for receipt in receipts {
        let transport = fake_transport(None, String::new());
        *transport
            .stop_receipt
            .lock()
            .expect("VPN stop receipt lock") = receipt.clone();
        let client = AndroidOwnerClient::start_with_policy(transport.clone(), test_policy())
            .expect("start owner worker");

        assert!(client.submit_tracked(UiCommand::StopVpn).is_some());
        let event = wait_for_result(&client);

        assert!(event.result.is_err(), "{receipt:?} must not report success");
        assert_eq!(transport.stop_requests.load(Ordering::SeqCst), 1);
        assert_eq!(transport.commands.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn client_rejects_product_control_success_without_ready_packet_worker_readback() {
    let scope = core_scope("owner-bevy", 7, Some(70));
    let surface = owner_snapshot(&scope, 1);
    let core = core_owner_readback(&surface);
    let transport = fake_transport(
        Some(surface),
        command_outcome(
            Some(scope.clone()),
            CommandResult::Completed {
                request_id: RequestId::new(1),
            },
        ),
    );
    *transport
        .owner_snapshots
        .lock()
        .expect("VPN snapshot queue lock") = VecDeque::from([
        vpn_owner_snapshot(
            core.clone(),
            1,
            VpnOwnerLifecycle::Stopped,
            VpnOwnerReadiness::Stopped,
        ),
        vpn_owner_snapshot(
            core,
            2,
            VpnOwnerLifecycle::TunnelEstablished,
            VpnOwnerReadiness::AwaitingDataPlane,
        ),
    ]);
    let client = AndroidOwnerClient::start_with_policy(transport.clone(), test_policy())
        .expect("start owner worker");
    wait_for_scope(&client, &scope);

    assert!(client.submit_tracked(UiCommand::StartVpn).is_some());
    let event = wait_for_result(&client);

    assert_eq!(
        event
            .result
            .as_ref()
            .err()
            .map(|failure| failure.code.clone()),
        Some(ErrorCode::InvalidState),
    );
    assert_eq!(
        transport.events.lock().expect("event lock").as_slice(),
        &["vpn-readback", "command", "vpn-readback"],
    );
}

#[test]
fn client_rejects_control_receipt_from_a_rebound_vpn_owner_session() {
    let scope = core_scope("owner-bevy", 7, Some(70));
    let surface = owner_snapshot(&scope, 1);
    let core = core_owner_readback(&surface);
    let transport = fake_transport(
        Some(surface),
        command_outcome(
            Some(scope.clone()),
            CommandResult::Completed {
                request_id: RequestId::new(1),
            },
        ),
    );
    let mut terminal = vpn_owner_snapshot(
        core.clone(),
        2,
        VpnOwnerLifecycle::Stopped,
        VpnOwnerReadiness::Stopped,
    );
    terminal.owner_session_id = "rebound-owner-session".to_owned();
    *transport
        .owner_snapshots
        .lock()
        .expect("VPN snapshot queue lock") = VecDeque::from([
        vpn_owner_snapshot(
            core,
            1,
            VpnOwnerLifecycle::Stopped,
            VpnOwnerReadiness::Stopped,
        ),
        terminal,
    ]);
    let client = AndroidOwnerClient::start_with_policy(transport.clone(), test_policy())
        .expect("start owner worker");
    wait_for_scope(&client, &scope);

    assert!(client.submit_tracked(UiCommand::StartCore).is_some());
    let event = wait_for_result(&client);

    assert_eq!(
        event
            .result
            .as_ref()
            .err()
            .map(|failure| failure.code.clone()),
        Some(ErrorCode::InvalidState),
    );
}

#[test]
fn client_does_not_send_control_when_initial_vpn_readback_has_a_stale_core_scope() {
    let request_scope = core_scope("owner-bevy", 7, Some(70));
    let foreign_scope = core_scope("owner-old", 7, Some(70));
    let transport = fake_transport(
        Some(owner_snapshot(&request_scope, 1)),
        command_outcome(
            Some(request_scope.clone()),
            CommandResult::Completed {
                request_id: RequestId::new(1),
            },
        ),
    );
    *transport
        .owner_snapshots
        .lock()
        .expect("VPN snapshot queue lock") = VecDeque::from([vpn_owner_snapshot(
        VpnOwnerCoreReadback::Available(CoreLifecycleSnapshot {
            identity: Some(foreign_scope.identity.clone()),
            lifecycle: CoreLifecycle::Running,
            generation: foreign_scope.generation.value(),
            session_token: foreign_scope.session_token,
            revision: 1,
            failure: None,
        }),
        1,
        VpnOwnerLifecycle::Stopped,
        VpnOwnerReadiness::Stopped,
    )]);
    let client = AndroidOwnerClient::start_with_policy(transport.clone(), test_policy())
        .expect("start owner worker");
    wait_for_scope(&client, &request_scope);

    assert!(client.submit_tracked(UiCommand::StartCore).is_some());
    let event = wait_for_result(&client);

    assert!(event.result.is_err());
    assert_eq!(transport.commands.load(Ordering::SeqCst), 0);
    assert_eq!(
        transport.events.lock().expect("event lock").as_slice(),
        &["vpn-readback"],
    );
}

#[test]
fn client_sends_typed_scope_and_rejects_a_terminal_from_another_owner_scope() {
    let request_scope = core_scope("owner-bevy", 7, Some(70));
    let foreign_scope = core_scope("owner-old", 7, Some(70));
    let transport = fake_transport(
        Some(owner_snapshot(&request_scope, 1)),
        command_outcome(
            Some(foreign_scope),
            CommandResult::Completed {
                request_id: RequestId::new(1),
            },
        ),
    );
    let client = AndroidOwnerClient::start_with_policy(transport.clone(), test_policy())
        .expect("start owner worker");
    wait_for_scope(&client, &request_scope);

    assert!(
        client
            .submit_tracked(UiCommand::CloseAllConnections)
            .is_some()
    );
    let event = wait_for_result(&client);

    assert!(event.result.is_err());
    assert_eq!(
        event
            .result
            .as_ref()
            .err()
            .map(|failure| failure.code.clone()),
        Some(ErrorCode::NotReady)
    );
    assert_eq!(transport.commands.load(Ordering::SeqCst), 1);
}

#[test]
fn client_rejects_terminals_from_an_expired_generation_or_session() {
    let request_scope = core_scope("owner-bevy", 7, Some(70));
    let stale_scopes = [
        core_scope("owner-bevy", 6, Some(70)),
        core_scope("owner-bevy", 7, Some(71)),
    ];

    for stale_scope in stale_scopes {
        let transport = fake_transport(
            Some(owner_snapshot(&request_scope, 1)),
            command_outcome(
                Some(stale_scope),
                CommandResult::Completed {
                    request_id: RequestId::new(1),
                },
            ),
        );
        let client = AndroidOwnerClient::start_with_policy(transport, test_policy())
            .expect("start owner worker");
        wait_for_scope(&client, &request_scope);

        assert!(
            client
                .submit_tracked(UiCommand::CloseAllConnections)
                .is_some()
        );
        let event = wait_for_result(&client);

        assert!(event.result.is_err());
        assert_eq!(
            event
                .result
                .as_ref()
                .err()
                .map(|failure| failure.code.clone()),
            Some(ErrorCode::NotReady)
        );
    }
}

#[test]
fn client_returns_a_typed_timeout_and_ignores_the_late_terminal() {
    let scope = core_scope("owner-bevy", 7, Some(70));
    let transport = fake_transport(
        Some(owner_snapshot(&scope, 1)),
        command_outcome(
            Some(scope.clone()),
            CommandResult::Completed {
                request_id: RequestId::new(1),
            },
        ),
    );
    transport.block_commands.store(true, Ordering::SeqCst);
    let client = AndroidOwnerClient::start_with_policy(transport.clone(), test_policy())
        .expect("start owner worker");
    wait_for_scope(&client, &scope);
    assert!(
        client
            .submit_tracked(UiCommand::CloseAllConnections)
            .is_some()
    );

    thread::sleep(Duration::from_millis(110));
    let event = wait_for_result(&client);
    assert!(event.result.is_err());
    assert_eq!(
        event
            .result
            .as_ref()
            .err()
            .map(|failure| failure.code.clone()),
        Some(ErrorCode::NotReady)
    );
    transport.block_commands.store(false, Ordering::SeqCst);
    thread::sleep(Duration::from_millis(10));
    assert!(client.drain_results().is_empty());
}

fn wait_for_scope(client: &AndroidOwnerClient, scope: &CoreScope) {
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline {
        if client.latest_snapshot().core.scope().as_ref() == Some(scope) {
            return;
        }
        thread::sleep(Duration::from_millis(2));
    }
    panic!("owner scope was not read");
}

fn wait_for_result(client: &AndroidOwnerClient) -> CommandExecutedEvent {
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline {
        if let Some(result) = client.drain_results().into_iter().next() {
            return result;
        }
        thread::sleep(Duration::from_millis(2));
    }
    panic!("owner command did not reach a terminal result");
}

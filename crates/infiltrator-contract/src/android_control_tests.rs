use super::{
    AndroidChildProcessIdentity, AndroidControlIntent, AndroidCoreLifecycleSnapshot,
    AndroidCoreProcessExitEvidence, AndroidCoreProcessObservation,
    AndroidCoreProcessRecoveryReceipt, AndroidCoreProcessUnavailableReason, AndroidHostProcessRole,
    AndroidOwnedCoreProcess, AndroidProductCommand, AndroidProductCommandOutcome,
    AndroidProductCommandRequest, AndroidProductControlIntent, VpnOwnerCoreReadback,
    VpnOwnerLifecycle, VpnOwnerReadiness, VpnOwnerResourceFact, VpnOwnerResourceKind,
    VpnOwnerResourceState, VpnOwnerSnapshot, validate_android_control_owner_observation,
    validate_android_control_terminal_readback,
};
use crate::command::{CommandIntent, CommandResult, RequestId};
use crate::core_identity::CoreCommandOutcome;
use crate::vpn::VpnGeneration;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::error::{ErrorCode, Failure, FailureReason};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use lurker_core_contract::lifecycle::{CoreLifecycle, CoreLifecycleSnapshot};
use lurker_core_contract::session::SessionToken;

#[test]
fn android_lifecycle_snapshot_uses_decimal_text_for_full_width_session_tokens() {
    let snapshot = AndroidCoreLifecycleSnapshot {
        identity: Some(CoreIdentity::new(
            EngineId::new("mihomo").expect("engine"),
            CoreInstanceId::new("android-active").expect("instance"),
            None,
        )),
        lifecycle: CoreLifecycle::Running,
        generation: 4,
        session_token: Some(SessionToken::new(u128::MAX)),
        revision: 9,
        failure: None,
    };

    let encoded = serde_json::to_string(&snapshot).expect("serialize lifecycle snapshot");
    assert!(encoded.contains("\"session_token\":\"340282366920938463463374607431768211455\""));
    assert_eq!(
        serde_json::from_str::<AndroidCoreLifecycleSnapshot>(&encoded)
            .expect("deserialize decimal session token"),
        snapshot
    );
}

#[test]
fn control_intent_wire_codes_round_trip_and_reject_unknown_values() {
    let intents = [
        AndroidControlIntent::StartProxy,
        AndroidControlIntent::StopProxy,
        AndroidControlIntent::ResumeKernel,
        AndroidControlIntent::PauseKernel,
        AndroidControlIntent::RestartKernel,
    ];

    for intent in intents {
        assert_eq!(
            AndroidControlIntent::from_wire_code(intent.wire_code()),
            Some(intent)
        );
    }
    assert_eq!(AndroidControlIntent::from_wire_code(-1), None);
    for old_code in 5..=11 {
        assert_eq!(AndroidControlIntent::from_wire_code(old_code), None);
    }
}

#[test]
fn android_product_commands_and_results_round_trip_as_typed_json() {
    let scope = CoreScope {
        identity: CoreIdentity::new(
            EngineId::new("mihomo").expect("engine"),
            CoreInstanceId::new("android-active").expect("instance"),
            None,
        ),
        generation: CoreGeneration::new(4),
        session_token: None,
    };
    let command = AndroidProductCommandRequest {
        scope: scope.clone(),
        command: AndroidProductCommand::Application(Box::new(CommandIntent::StartProxy)),
    };
    let encoded = serde_json::to_string(&command).expect("serialize command");
    assert_eq!(
        serde_json::from_str::<AndroidProductCommandRequest>(&encoded)
            .expect("deserialize command"),
        command
    );

    let control_command = AndroidProductCommandRequest {
        scope: scope.clone(),
        command: AndroidProductCommand::Control(AndroidProductControlIntent::ResumeKernel),
    };
    let encoded = serde_json::to_string(&control_command).expect("serialize control command");
    assert_eq!(
        serde_json::from_str::<AndroidProductCommandRequest>(&encoded)
            .expect("deserialize control command"),
        control_command
    );

    let legacy_stop = format!(
        "{{\"scope\":{},\"command\":{{\"Control\":\"StopProxy\"}}}}",
        serde_json::to_string(&scope).expect("serialize scope")
    );
    assert!(serde_json::from_str::<AndroidProductCommandRequest>(&legacy_stop).is_err());

    let outcome = AndroidProductCommandOutcome::Terminal(Box::new(CoreCommandOutcome {
        scope: Some(scope),
        result: CommandResult::Completed {
            request_id: RequestId(17),
        },
    }));
    let encoded = serde_json::to_string(&outcome).expect("serialize result");
    assert_eq!(
        serde_json::from_str::<AndroidProductCommandOutcome>(&encoded).expect("deserialize result"),
        outcome
    );

    let unavailable = AndroidProductCommandOutcome::Unavailable {
        scope: None,
        failure: Failure::new(ErrorCode::NotReady, "owner unavailable", true),
    };
    let encoded = serde_json::to_string(&unavailable).expect("serialize failure");
    assert_eq!(
        serde_json::from_str::<AndroidProductCommandOutcome>(&encoded)
            .expect("deserialize failure"),
        unavailable
    );
}

#[test]
fn application_command_wire_preserves_unit_and_record_intent_variants() {
    let scope = CoreScope {
        identity: CoreIdentity::new(
            EngineId::new("mihomo").expect("engine"),
            CoreInstanceId::new("android-active").expect("instance"),
            None,
        ),
        generation: CoreGeneration::new(4),
        session_token: None,
    };
    let cases = [
        (CommandIntent::ClearLogs, serde_json::json!("ClearLogs")),
        (
            CommandIntent::CloseAllConnections,
            serde_json::json!("CloseAllConnections"),
        ),
        (
            CommandIntent::CloseConnection {
                id: "connection-1".to_owned(),
            },
            serde_json::json!({"CloseConnection": {"id": "connection-1"}}),
        ),
    ];

    for (intent, expected_intent) in cases {
        let request = AndroidProductCommandRequest {
            scope: scope.clone(),
            command: AndroidProductCommand::Application(Box::new(intent)),
        };
        let encoded = serde_json::to_value(request).expect("serialize application command");
        assert_eq!(encoded["command"]["Application"], expected_intent);
        assert_eq!(
            serde_json::from_value::<AndroidProductCommandRequest>(encoded)
                .expect("deserialize application command")
                .scope,
            scope,
        );
    }
}

#[test]
fn typed_core_process_observations_round_trip_exact_child_facts() {
    let owned = AndroidOwnedCoreProcess {
        context: CoreProcessStartContext::managed(
            CoreIdentity::new(
                EngineId::new("mihomo").expect("engine ID"),
                CoreInstanceId::new("android-active").expect("instance ID"),
                None,
            ),
            CoreGeneration::new(7),
            SessionToken::new(42),
        ),
        child: AndroidChildProcessIdentity {
            pid: 321,
            start_time_ticks: 987_654,
        },
    };
    let observations = [
        AndroidCoreProcessObservation::NeverStarted,
        AndroidCoreProcessObservation::Running {
            owned: owned.clone(),
        },
        AndroidCoreProcessObservation::Exited {
            owned,
            evidence: AndroidCoreProcessExitEvidence::ProcessStartIdentityChanged,
        },
        AndroidCoreProcessObservation::Unavailable {
            reason: AndroidCoreProcessUnavailableReason::OwnershipRecordCorrupt,
        },
    ];

    for observation in observations {
        let encoded = serde_json::to_string(&observation).expect("serialize observation");
        assert_eq!(
            serde_json::from_str::<AndroidCoreProcessObservation>(&encoded)
                .expect("deserialize observation"),
            observation,
        );
    }
}

#[test]
fn android_core_process_recovery_receipts_have_exhaustive_wire_codes() {
    let receipts = [
        AndroidCoreProcessRecoveryReceipt::Consumed,
        AndroidCoreProcessRecoveryReceipt::AlreadyAbsent,
        AndroidCoreProcessRecoveryReceipt::ContextMismatch,
        AndroidCoreProcessRecoveryReceipt::ChildStillRunning,
        AndroidCoreProcessRecoveryReceipt::ObservationUnavailable,
        AndroidCoreProcessRecoveryReceipt::NotVpnOwner,
    ];
    for receipt in receipts {
        assert_eq!(
            AndroidCoreProcessRecoveryReceipt::from_wire_code(receipt.wire_code()),
            Some(receipt),
        );
    }
    assert_eq!(AndroidCoreProcessRecoveryReceipt::from_wire_code(-1), None);
    assert_eq!(AndroidCoreProcessRecoveryReceipt::from_wire_code(6), None);
}

#[test]
fn android_host_process_role_wire_codes_are_typed_and_fail_closed() {
    let roles = [
        AndroidHostProcessRole::UiClient,
        AndroidHostProcessRole::CoreOwner,
        AndroidHostProcessRole::Unsupported,
    ];

    for role in roles {
        assert_eq!(
            AndroidHostProcessRole::from_wire_code(role.wire_code()),
            Some(role)
        );
    }
    assert_eq!(AndroidHostProcessRole::from_wire_code(-1), None);
    assert_eq!(AndroidHostProcessRole::from_wire_code(3), None);
}

#[test]
fn vpn_owner_snapshot_round_trips_core_identity_resources_and_decimal_session_token() {
    let snapshot = VpnOwnerSnapshot {
        owner_session_id: "owner-session".to_owned(),
        revision: 14,
        vpn_generation: VpnGeneration::new(9),
        vpn_state: VpnOwnerLifecycle::Running,
        readiness: VpnOwnerReadiness::Ready,
        resources: [
            VpnOwnerResourceKind::Worker,
            VpnOwnerResourceKind::TunnelDescriptor,
        ]
        .into_iter()
        .map(|kind| VpnOwnerResourceFact {
            kind,
            state: VpnOwnerResourceState::Held { generation: 9 },
        })
        .collect(),
        foreground: true,
        mtu: Some(1500),
        route_count: 3,
        dns_servers: vec!["1.1.1.1".to_owned()],
        ipv6: true,
        core: VpnOwnerCoreReadback::Available(CoreLifecycleSnapshot {
            identity: Some(CoreIdentity::new(
                EngineId::new("mihomo").expect("engine"),
                CoreInstanceId::new("android-active").expect("instance"),
                None,
            )),
            lifecycle: CoreLifecycle::Running,
            generation: 21,
            session_token: Some(SessionToken::new(u128::MAX)),
            revision: 32,
            failure: None,
        }),
    };

    let encoded = serde_json::to_string(&snapshot).expect("serialize owner snapshot");
    assert!(encoded.contains("\"session_token\":\"340282366920938463463374607431768211455\""));
    assert_eq!(
        serde_json::from_str::<VpnOwnerSnapshot>(&encoded).expect("deserialize owner snapshot"),
        snapshot
    );
    assert!(snapshot.is_running());
}

#[test]
fn vpn_owner_snapshot_preserves_failed_core_failure_facts() {
    let failure = Failure::new(
        ErrorCode::Configuration,
        "profile YAML could not be parsed",
        true,
    )
    .with_reason(FailureReason::YamlSyntax {
        line: 23,
        column: 6,
    });
    let core = CoreLifecycleSnapshot {
        identity: Some(CoreIdentity::new(
            EngineId::new("mihomo").expect("engine"),
            CoreInstanceId::new("android-active").expect("instance"),
            Some(CoreVersion::new("1.20.0").expect("version")),
        )),
        lifecycle: CoreLifecycle::Failed,
        generation: 21,
        session_token: Some(SessionToken::new(u128::MAX)),
        revision: 33,
        failure: Some(failure.clone()),
    };
    let snapshot = VpnOwnerSnapshot {
        owner_session_id: "owner-session".to_owned(),
        revision: 14,
        vpn_generation: VpnGeneration::new(9),
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
        core: VpnOwnerCoreReadback::Available(core.clone()),
    };

    let encoded = serde_json::to_string(&snapshot).expect("serialize owner snapshot");
    let value: serde_json::Value = serde_json::from_str(&encoded).expect("owner JSON parses");
    assert_eq!(
        value["core"]["Available"]["failure"],
        serde_json::to_value(&failure).expect("typed failure serializes"),
    );
    let decoded =
        serde_json::from_str::<VpnOwnerSnapshot>(&encoded).expect("deserialize owner snapshot");

    assert_eq!(decoded.core, VpnOwnerCoreReadback::Available(core));
    let VpnOwnerCoreReadback::Available(decoded_core) = decoded.core else {
        panic!("failed Core snapshot facts must remain Available")
    };
    assert_eq!(decoded_core.lifecycle, CoreLifecycle::Failed);
    assert_eq!(decoded_core.failure, Some(failure));
    assert_eq!(
        decoded_core.session_token,
        Some(SessionToken::new(u128::MAX))
    );
}

#[test]
fn stop_terminal_requires_typed_terminal_readiness_and_every_resource_absent() {
    let mut snapshot = VpnOwnerSnapshot {
        owner_session_id: "owner-session".to_owned(),
        revision: 4,
        vpn_generation: VpnGeneration::new(3),
        vpn_state: VpnOwnerLifecycle::Stopped,
        readiness: VpnOwnerReadiness::Stopped,
        resources: Vec::new(),
        foreground: false,
        mtu: None,
        route_count: 0,
        dns_servers: Vec::new(),
        ipv6: false,
        core: VpnOwnerCoreReadback::Unavailable {
            failure_message: "Core readback unavailable".to_owned(),
        },
    };
    assert!(!snapshot.is_stopped());

    snapshot.resources = [
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
    .collect();
    assert!(snapshot.is_stopped());

    snapshot.resources[1].state = VpnOwnerResourceState::CleanupPending {
        generation: 3,
        failure_message: "worker stop is pending".to_owned(),
    };
    snapshot.readiness = VpnOwnerReadiness::CleanupPending;
    assert!(!snapshot.is_stopped());
}

#[test]
fn control_owner_observation_requires_session_revision_and_complete_resources() {
    let mut snapshot = VpnOwnerSnapshot {
        owner_session_id: "owner-session".to_owned(),
        revision: 1,
        vpn_generation: VpnGeneration::new(3),
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
        core: VpnOwnerCoreReadback::Available(CoreLifecycleSnapshot {
            identity: Some(CoreIdentity::new(
                EngineId::new("mihomo").expect("engine"),
                CoreInstanceId::new("android-active").expect("instance"),
                None,
            )),
            lifecycle: CoreLifecycle::Stopped,
            generation: 3,
            session_token: None,
            revision: 9,
            failure: None,
        }),
    };
    assert_eq!(
        validate_android_control_owner_observation(&snapshot),
        Ok(())
    );

    let before = snapshot.clone();
    let mut after = before.clone();
    if let VpnOwnerCoreReadback::Available(core) = &mut after.core {
        core.revision += 1;
    }
    let terminal_scope = match &after.core {
        VpnOwnerCoreReadback::Available(core) => core.scope(),
        VpnOwnerCoreReadback::Unavailable { .. } => None,
    };
    let failure = validate_android_control_terminal_readback(
        AndroidProductControlIntent::ResumeKernel,
        &before,
        &after,
        terminal_scope.as_ref(),
    )
    .expect_err("Core facts in an owner snapshot must advance the owner revision");
    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert!(failure.message.contains("without advancing its revision"));

    let mut before = snapshot.clone();
    before.revision = 4;

    let mut stale_owner_revision = before.clone();
    stale_owner_revision.revision = 3;
    assert!(
        validate_android_control_terminal_readback(
            AndroidProductControlIntent::ResumeKernel,
            &before,
            &stale_owner_revision,
            terminal_scope.as_ref(),
        )
        .expect_err("owner revision cannot regress")
        .message
        .contains("stale lifecycle revision")
    );

    let mut stale_vpn_generation = before.clone();
    stale_vpn_generation.revision = 5;
    stale_vpn_generation.vpn_generation = VpnGeneration::new(2);
    assert!(
        validate_android_control_terminal_readback(
            AndroidProductControlIntent::ResumeKernel,
            &before,
            &stale_vpn_generation,
            terminal_scope.as_ref(),
        )
        .expect_err("VPN generation cannot regress")
        .message
        .contains("regressed Core or VPN generation/revision facts")
    );

    let mut stale_core_generation = before.clone();
    stale_core_generation.revision = 5;
    if let VpnOwnerCoreReadback::Available(core) = &mut stale_core_generation.core {
        core.generation = 2;
    }
    let stale_core_scope = match &stale_core_generation.core {
        VpnOwnerCoreReadback::Available(core) => core.scope(),
        VpnOwnerCoreReadback::Unavailable { .. } => None,
    };
    assert!(
        validate_android_control_terminal_readback(
            AndroidProductControlIntent::ResumeKernel,
            &before,
            &stale_core_generation,
            stale_core_scope.as_ref(),
        )
        .expect_err("Core generation cannot regress")
        .message
        .contains("regressed Core or VPN generation/revision facts")
    );

    let mut stale_core_revision = before.clone();
    stale_core_revision.revision = 5;
    if let VpnOwnerCoreReadback::Available(core) = &mut stale_core_revision.core {
        core.revision = 8;
    }
    assert!(
        validate_android_control_terminal_readback(
            AndroidProductControlIntent::ResumeKernel,
            &before,
            &stale_core_revision,
            terminal_scope.as_ref(),
        )
        .expect_err("Core revision cannot regress")
        .message
        .contains("regressed Core or VPN generation/revision facts")
    );

    snapshot.owner_session_id.clear();
    assert_eq!(
        validate_android_control_owner_observation(&snapshot)
            .expect_err("readback without an owner session is stale")
            .code,
        ErrorCode::InvalidState,
    );
    snapshot.owner_session_id = "owner-session".to_owned();

    snapshot.revision = 0;
    assert_eq!(
        validate_android_control_owner_observation(&snapshot)
            .expect_err("readback without a revision is stale")
            .code,
        ErrorCode::InvalidState,
    );
    snapshot.revision = 1;

    snapshot.resources.pop();
    assert_eq!(
        validate_android_control_owner_observation(&snapshot)
            .expect_err("readback without every resource fact is incomplete")
            .code,
        ErrorCode::InvalidState,
    );
}

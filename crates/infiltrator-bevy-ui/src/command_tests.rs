use super::*;
use infiltrator_contract::android_control::AndroidControlIntent;
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::outbound::identity::SnapshotRevision;
use lurker_core_contract::outbound::preferences::OutboundSortOrder;
use lurker_core_contract::outbound::preferences::OutboundUiPreferenceIntent;
use lurker_core_contract::outbound::preferences::OutboundUiPreferenceRequest;
use lurker_core_contract::outbound::probe::OutboundProbeBatchScope;
use lurker_core_contract::outbound::probe::OutboundProbeConcurrency;
use lurker_core_contract::outbound::probe::ProbeOutboundBatchRequest;
use lurker_core_contract::outbound::probe::ProbeOutboundRequest;

#[test]
fn demo_command_sink_records_and_clears() {
    let sink = DemoCommandSink::accepting();
    assert!(sink.submitted().is_empty());

    sink.submit(UiCommand::ClearLogs);
    sink.submit(UiCommand::CloseAllConnections);

    let items = sink.submitted();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0], UiCommand::ClearLogs);
    assert_eq!(items[1], UiCommand::CloseAllConnections);

    sink.clear();
    assert!(sink.submitted().is_empty());
}

#[test]
fn business_commands_map_to_the_shared_contract() {
    let outbound_request = OutboundSelectionRequest::new(
        OutboundId::new(EngineId::new("mihomo").unwrap(), "GLOBAL").unwrap(),
        OutboundId::new(EngineId::new("mihomo").unwrap(), "HK-01").unwrap(),
        SnapshotRevision::new(9),
    )
    .unwrap();
    assert_eq!(
        UiCommand::SelectOutbound {
            request: outbound_request.clone(),
        }
        .to_intent(),
        Some(CommandIntent::SelectOutbound {
            request: outbound_request,
        })
    );
    assert_eq!(
        UiCommand::SetRoutingIntent(RoutingIntent::RouteAllThroughProxy).to_intent(),
        Some(CommandIntent::SetRoutingIntent {
            intent: RoutingIntent::RouteAllThroughProxy,
        })
    );
    assert!(
        UiCommand::ApplyOutboundUiPreference {
            request: OutboundUiPreferenceRequest::new(
                SnapshotRevision::new(9),
                OutboundUiPreferenceIntent::SetSortOrder(OutboundSortOrder::LatencyDescending,),
            )
            .unwrap(),
        }
        .to_intent()
        .is_some()
    );
    let probe = ProbeOutboundRequest::new(
        OutboundId::new(EngineId::new("mihomo").unwrap(), "HK-01").unwrap(),
        SnapshotRevision::new(9),
        "https://example.test/204",
        5000,
    )
    .unwrap();
    assert_eq!(
        UiCommand::ProbeOutbound {
            request: probe.clone(),
        }
        .to_intent(),
        Some(CommandIntent::ProbeOutbound { request: probe })
    );
    assert_eq!(
        UiCommand::RollbackCore.to_intent(),
        Some(CommandIntent::RollbackCore)
    );
    assert_eq!(
        UiCommand::SetCoreLogLevel(CoreLogLevel::Debug).to_intent(),
        Some(CommandIntent::SetCoreLogLevel {
            level: CoreLogLevel::Debug,
        })
    );
    assert_eq!(
        UiCommand::SetTunStack(TunStack::Mixed).to_intent(),
        Some(CommandIntent::SetTunStack {
            stack: TunStack::Mixed,
        })
    );
    assert_eq!(
        UiCommand::ProbeTunMtu.to_intent(),
        Some(CommandIntent::ProbeTunMtu)
    );
    assert_eq!(
        UiCommand::RefreshPublicIpProbe.to_intent(),
        Some(CommandIntent::RefreshPublicIpProbe)
    );
    assert_eq!(
        UiCommand::ResetOverviewCardOrder.to_intent(),
        Some(CommandIntent::ResetOverviewCardOrder)
    );
    assert_eq!(
        UiCommand::SetTunAutoRoute(false).to_intent(),
        Some(CommandIntent::SetTunAutoRoute { enabled: false })
    );
    assert_eq!(
        UiCommand::SetTunStrictRoute(true).to_intent(),
        Some(CommandIntent::SetTunStrictRoute { enabled: true })
    );
    assert_eq!(
        UiCommand::SetSystemProxy { enabled: true }.to_intent(),
        Some(CommandIntent::SetSystemProxy { enabled: true })
    );
    assert_eq!(
        UiCommand::SetLanSharing {
            enabled: true,
            mixed_port: 8080,
            bind_address: "192.168.1.10".to_owned(),
        }
        .to_intent(),
        Some(CommandIntent::SetLanSharing {
            enabled: true,
            mixed_port: 8080,
            bind_address: "192.168.1.10".to_owned(),
        })
    );
    assert_eq!(
        UiCommand::SetLanSecurity {
            allowed_ips: vec!["192.168.1.0/24".to_owned()],
            disallowed_ips: vec!["192.168.1.10/32".to_owned()],
            skip_auth_prefixes: vec!["127.0.0.0/8".to_owned()],
            authentication_enabled: true,
            credentials: Some(LanCredentials {
                username: "lan-user".to_owned(),
                password: "secret-value".to_owned(),
            }),
        }
        .to_intent(),
        Some(CommandIntent::SetLanSecurity {
            allowed_ips: vec!["192.168.1.0/24".to_owned()],
            disallowed_ips: vec!["192.168.1.10/32".to_owned()],
            skip_auth_prefixes: vec!["127.0.0.0/8".to_owned()],
            authentication_enabled: true,
            credentials: Some(LanCredentials {
                username: "lan-user".to_owned(),
                password: "secret-value".to_owned(),
            }),
        })
    );
    assert_eq!(
        UiCommand::SetIpv6Routing { enabled: false }.to_intent(),
        Some(CommandIntent::SetIpv6Routing { enabled: false })
    );
    assert_eq!(
        UiCommand::ScanUwpApps.to_intent(),
        Some(CommandIntent::ScanUwpApps)
    );
    assert_eq!(
        UiCommand::SetAllUwpExemptions { exempt: true }.to_intent(),
        Some(CommandIntent::SetAllUwpExemptions { exempt: true })
    );
    assert_eq!(
        UiCommand::ApplyPac {
            enabled: true,
            bypass_domains: vec!["example.com".to_owned()],
            bypass_lan: true,
            minify: false,
        }
        .to_intent(),
        Some(CommandIntent::ApplyPac {
            enabled: true,
            bypass_domains: vec!["example.com".to_owned()],
            bypass_lan: true,
            minify: false,
        })
    );
}

#[test]
fn speedtest_commands_carry_validated_concurrency_and_typed_batch_request() {
    let concurrency = SpeedtestConcurrency::new(12).expect("bounded concurrency");
    assert_eq!(
        UiCommand::SetSpeedtestConcurrency { concurrency }.to_intent(),
        Some(CommandIntent::SetSpeedtestConcurrency { concurrency })
    );
    let run_id = SpeedtestRunId::new(7).expect("valid speedtest run ID");
    assert_eq!(
        UiCommand::CancelSpeedtestRun { run_id }.to_intent(),
        Some(CommandIntent::CancelSpeedtestRun { run_id })
    );
    // The request is built from accepted probe settings, neutral scope,
    // and the observed inventory revision.
    let batch_request = ProbeOutboundBatchRequest::new(
        OutboundProbeBatchScope::All,
        SnapshotRevision::new(9),
        "https://cp.cloudflare.com/generate_204",
        5000,
        OutboundProbeConcurrency::new(4).unwrap(),
    )
    .unwrap();
    assert_eq!(
        UiCommand::ProbeOutboundBatch {
            request: batch_request.clone(),
        }
        .to_intent(),
        Some(CommandIntent::ProbeOutboundBatch {
            request: batch_request,
        })
    );
}

#[test]
fn lifecycle_commands_share_the_core_application_intents() {
    assert_eq!(
        UiCommand::StartCore.to_intent(),
        Some(CommandIntent::StartCore)
    );
    assert_eq!(
        UiCommand::StopCore.to_intent(),
        Some(CommandIntent::StopCore)
    );
    assert_eq!(
        UiCommand::RestartCore.to_intent(),
        Some(CommandIntent::RestartCore)
    );
    assert_eq!(
        UiCommand::StartCore.android_control_intent(),
        Some(AndroidControlIntent::ResumeKernel)
    );
    assert_eq!(
        UiCommand::StopCore.android_control_intent(),
        Some(AndroidControlIntent::PauseKernel)
    );
    assert_eq!(
        UiCommand::StartVpn.android_control_intent(),
        Some(AndroidControlIntent::StartProxy)
    );
    assert_eq!(UiCommand::StopVpn.android_control_intent(), None);
    assert_eq!(UiCommand::StopVpn.to_intent(), Some(CommandIntent::StopVpn));
}

use super::*;
use infiltrator_presentation::dns_page_model::DnsPageTab;

#[test]
fn test_tray_and_exit() {
    let (mut state, _) = AppState::new();

    // Tray events shouldn't crash and must map onto the same actions as the
    // old muda menu ids: icon click shows the window, quit requests exit.
    let _ = state.update(Message::TrayEvent(TrayEvent::IconActivated));
    assert_eq!(
        resolve_tray_event(
            &TrayEvent::MenuActivated {
                id: TRAY_ACTION_QUIT,
                payload: None,
            },
            false,
            false,
        ),
        Some(TrayIntent::Exit)
    );
    let _ = state.update(Message::TrayEvent(TrayEvent::MenuActivated {
        id: TRAY_ACTION_QUIT,
        payload: None,
    }));

    let _ = state.update(Message::Exit);
}

#[test]
fn test_tab_state_switches() {
    let (mut state, _) = AppState::new();
    state.editor.rules_page = 3;
    let _ = state.update(Message::SetRulesTab(RulesTab::JsonEditors));
    assert_eq!(state.editor.rules_model.tab(), RulesTab::JsonEditors);
    assert_eq!(state.editor.rules_page, 0);

    let _ = state.update(Message::SetRulesJsonTab(RulesJsonSection::Sniffer));
    assert_eq!(
        state.editor.rules_model.json_section(),
        RulesJsonSection::Sniffer
    );

    let _ = state.update(Message::SetDnsTab(DnsPageTab::Tun));
    assert_eq!(state.editor.dns_model.tab(), DnsPageTab::Tun);
}

#[test]
fn test_i18n_fallback() {
    let lang = Lang("fr-FR"); // Unsupported
    assert_eq!(
        lang.tr("nav_overview"),
        "核心概览",
        "Should fallback to ZH for unsupported locales"
    );
}

#[test]
fn test_error_and_toast_redaction() {
    let (mut state, _) = AppState::new();

    // set_error is the only writer of error_msg and must redact secrets.
    state.set_error("update failed: https://sub.example.com/d?token=tok1234");
    let error = state.shell.error_msg.clone().expect("error stored");
    assert!(error.contains("token=***"), "redacted error: {error}");
    assert!(!error.contains("tok1234"), "raw token leaked: {error}");

    // Every toast funnels through Message::ShowToast and is redacted there.
    let _ = state.update(Message::ShowToast(
        "secret: supersecret42".into(),
        ToastStatus::Error,
    ));
    let (content, _) = state.shell.toasts[0].clone();
    assert_eq!(content, "secret: ***");
}

#[test]
fn test_p0_confirmation_is_staged_and_cancellable() {
    let (mut state, _) = AppState::new();

    let _ = state.update(Message::RequestConfirmation(ConfirmAction::DeleteProfile(
        "unused".to_string(),
    )));
    assert_eq!(
        state.shell.confirmation,
        Some(ConfirmAction::DeleteProfile("unused".to_string()))
    );
    let _ = state.update(Message::CancelConfirmation);
    assert!(state.shell.confirmation.is_none());

    let _ = state.update(Message::RequestConfirmation(
        ConfirmAction::CloseAllConnections,
    ));
    let _ = state.update(Message::ConfirmAction);
    assert!(state.shell.confirmation.is_none());
    assert!(state.shell.error_msg.is_none());
}

#[test]
fn test_p0_runtime_stream_scope_and_failure_projection() {
    let (mut state, _) = AppState::new();
    state.runtime.runtime_generation = 7;
    let shared_session = SessionToken::new(77);
    let previous_scope = runtime_stream_scope("mihomo", "owner-a", shared_session);
    let current_scope = runtime_stream_scope("sing-box", "owner-b", shared_session);
    state.runtime.core_lifecycle.identity = Some(current_scope.identity.clone());
    state.runtime.core_lifecycle.lifecycle = CoreLifecycle::Running;
    state.runtime.core_lifecycle.generation = current_scope.generation.value();
    state.runtime.core_lifecycle.session_token = current_scope.session_token;

    let _ = state.update(Message::RuntimeStreamLogReceived(
        current_scope.clone(),
        "current".to_string(),
    ));
    assert_eq!(state.diag.logs.back().map(String::as_str), Some("current"));

    let _ = state.update(Message::RuntimeStreamLogReceived(
        previous_scope.clone(),
        "stale".to_string(),
    ));
    assert_eq!(state.diag.logs.len(), 1);
    let _ = state.update(Message::RuntimeStreamStateChanged {
        kind: RuntimeStreamKind::Traffic,
        scope: current_scope.clone(),
        state: RuntimeStreamState::Connected,
    });
    let _ = state.update(Message::RuntimeStreamStateChanged {
        kind: RuntimeStreamKind::Traffic,
        scope: previous_scope,
        state: RuntimeStreamState::Failed("old engine stream failed".to_string()),
    });
    assert_eq!(
        state.diag.traffic_stream_state,
        RuntimeStreamState::Connected
    );
    assert!(state.shell.error_msg.is_none());

    let _ = state.update(Message::RuntimeStreamStateChanged {
        kind: RuntimeStreamKind::Traffic,
        scope: current_scope,
        state: RuntimeStreamState::Failed("socket closed".to_string()),
    });
    assert!(matches!(
        state.diag.traffic_stream_state,
        RuntimeStreamState::Failed(_)
    ));
    assert!(state.shell.error_msg.is_some());
}

#[test]
fn test_p0_ip_probe_metadata_and_explicit_language() {
    let (mut state, _) = AppState::new();
    let task_id = state.shell.last_task_id;
    let _ = state.update(Message::IpInfoReceived(
        Ok(IpProbeResult {
            ip: "203.0.113.10".to_string(),
            provider: "test-provider".to_string(),
            checked_at: "2026-08-30 12:00:00".to_string(),
        }),
        task_id,
    ));
    assert_eq!(state.diag.public_ip.as_deref(), Some("203.0.113.10"));
    assert_eq!(
        state.diag.public_ip_provider.as_deref(),
        Some("test-provider")
    );
    assert_eq!(
        state.diag.public_ip_checked_at.as_deref(),
        Some("2026-08-30 12:00:00")
    );

    let original_language = state.shell.lang.clone();
    assert_eq!(
        state
            .update(Message::SetLanguage("en-US".to_string()))
            .units(),
        0
    );
    assert_eq!(
        state.shell.lang, original_language,
        "an uncomposed host cannot claim a persisted language change"
    );
    assert_eq!(
        match &state.shell.language_choice.phase {
            LanguageChoicePhase::Rejected(failure) => {
                failure.code.clone()
            }
            LanguageChoicePhase::Ready
            | LanguageChoicePhase::Pending(_)
            | LanguageChoicePhase::Failed { .. }
            | LanguageChoicePhase::AwaitingObservation { .. } => {
                panic!("uncomposed language change must be rejected")
            }
        },
        ErrorCode::NotReady
    );
}

#[test]
fn test_p0_download_progress_token_and_sync_progress() {
    let (mut state, _) = AppState::new();
    let stale_run = match state.runtime.core_download.start() {
        CoreDownloadStart::Started { run_id, .. } => run_id,
        CoreDownloadStart::AlreadyDownloading => panic!("first download must start"),
    };
    assert_eq!(
        state.runtime.core_download.complete(stale_run),
        CoreDownloadCompletionAdmission::Applied
    );
    let current_run = match state.runtime.core_download.start() {
        CoreDownloadStart::Started { run_id, .. } => run_id,
        CoreDownloadStart::AlreadyDownloading => {
            panic!("second download must start after completion")
        }
    };
    let stale = CoreDownloadProgress {
        downloaded: 100,
        total: Some(200),
        speed_bytes: 10,
    };
    let _ = state.update(Message::CoreDownloadProgress {
        progress: stale,
        run_id: stale_run,
    });
    assert_eq!(
        state.runtime.core_download.progress().unwrap().downloaded,
        0
    );

    let current = CoreDownloadProgress {
        downloaded: 100,
        total: Some(200),
        speed_bytes: 10,
    };
    let _ = state.update(Message::CoreDownloadProgress {
        progress: current,
        run_id: current_run,
    });
    assert_eq!(state.runtime.core_download.progress_fraction(), 0.5);
    assert_eq!(
        state.runtime.core_download.progress().unwrap().downloaded,
        100
    );

    let _ = state.update(Message::SyncProgress(SyncProgress {
        phase: "上传配置".to_string(),
        current: 2,
        total: 4,
    }));
    assert_eq!(
        state.profile.sync_progress.as_ref().map(|p| p.current),
        Some(2)
    );
}

#[test]
fn test_p0_runtime_patch_failure_restores_the_previous_snapshot() {
    let (mut state, _) = AppState::new();
    state.runtime.routing_intent = Some(RoutingIntent::FollowRules);
    state.runtime.runtime_patch_stage = RuntimePatchStage::Applying {
        snapshot: RuntimePatchSnapshot {
            ipv6_enabled: true,
            tun_enabled: Some(false),
            tun_stack: "gvisor".to_string(),
            tun_stack_selector: "gvisor".to_string(),
            tun_auto_route: true,
            tun_strict_route: false,
            sniffer_enabled: true,
        },
    };
    state.runtime.runtime_patch_token = 11;
    state.runtime.routing_intent = Some(RoutingIntent::RouteAllThroughProxy);
    let generation = state.runtime.runtime_generation;

    let _ = state.update(Message::RuntimePatchResult(
        Err(InfiltratorError::Backend("controller rejected".to_string())),
        11,
        generation,
    ));
    assert_eq!(
        state.runtime.routing_intent.as_ref(),
        Some(&RoutingIntent::RouteAllThroughProxy),
        "a TUN/config patch does not own the routing intent"
    );
    assert!(!state.runtime.runtime_patch_stage.is_applying());
    assert!(state.shell.error_msg.is_some());

    state.runtime.routing_intent = Some(RoutingIntent::BypassProxy);
    let generation = state.runtime.runtime_generation;
    let _ = state.update(Message::RuntimePatchResult(Ok(()), 10, generation));
    assert_eq!(
        state.runtime.routing_intent.as_ref(),
        Some(&RoutingIntent::BypassProxy)
    );
}

#[test]
fn the_runtime_patch_stage_refuses_a_second_patch_while_one_is_in_flight() {
    let (mut state, _) = AppState::new();
    assert_eq!(state.runtime.runtime_patch_stage, RuntimePatchStage::Idle);

    let snapshot = RuntimePatchSnapshot {
        ipv6_enabled: true,
        tun_enabled: Some(false),
        tun_stack: "gvisor".to_string(),
        tun_stack_selector: "gvisor".to_string(),
        tun_auto_route: true,
        tun_strict_route: false,
        sniffer_enabled: true,
    };
    assert_eq!(state.runtime.begin_runtime_patch(snapshot.clone()), Ok(()));
    assert_eq!(
        state.runtime.runtime_patch_stage,
        RuntimePatchStage::Applying {
            snapshot: snapshot.clone()
        }
    );

    // A second patch is refused: the running stage keeps its own snapshot.
    assert_eq!(
        state.runtime.begin_runtime_patch(RuntimePatchSnapshot {
            ipv6_enabled: false,
            tun_enabled: Some(true),
            tun_stack: "system".to_string(),
            tun_stack_selector: "system".to_string(),
            tun_auto_route: false,
            tun_strict_route: false,
            sniffer_enabled: false,
        }),
        Err(RuntimePatchRefusal::AlreadyApplying)
    );
    assert_eq!(
        state.runtime.runtime_patch_stage,
        RuntimePatchStage::Applying {
            snapshot: snapshot.clone()
        }
    );

    // A rejected patch hands its restore snapshot back and returns to idle.
    assert_eq!(state.runtime.take_runtime_patch_snapshot(), Some(snapshot));
    assert_eq!(state.runtime.runtime_patch_stage, RuntimePatchStage::Idle);
    assert_eq!(state.runtime.take_runtime_patch_snapshot(), None);
}

#[test]
fn connections_pagination_windows_and_clamps() {
    use infiltrator_domain::connection_view::ConnectionSortKey;
    use infiltrator_domain::runtime::{Connection, ConnectionMetadata, ConnectionSnapshot};

    let snapshot_with = |count: usize| ConnectionSnapshot {
        download_total: 0,
        upload_total: 0,
        connections: (0..count)
            .map(|i| Connection {
                id: i.to_string(),
                metadata: ConnectionMetadata::default(),
                upload: 0,
                download: 0,
                start: String::new(),
                rule: String::new(),
                rule_payload: String::new(),
                chains: Vec::new(),
            })
            .collect(),
    };

    let (mut state, _) = AppState::new();
    state.diag.connections_page_size = 100;

    // 250 connections → 3 pages; next from page 0 → 1 → 2, then clamps at 2.
    let _ = state.update(Message::ConnectionsReceived(snapshot_with(250)));
    for expected in [1usize, 2, 2] {
        let _ = state.update(Message::ConnectionsNextPage);
        assert_eq!(state.diag.connections_page, expected);
    }
    let (page, start, end) = state.connections_window(250);
    assert_eq!((page, start, end), (2, 200, 250));

    // Snapshot shrinks below the current page → clamped back into range.
    let _ = state.update(Message::ConnectionsReceived(snapshot_with(80)));
    let (page, start, end) = state.connections_window(80);
    assert_eq!((page, start, end), (0, 0, 80));
    assert_eq!(state.diag.connections_page, 0);

    // Prev from page 0 saturates at 0.
    let _ = state.update(Message::ConnectionsPrevPage);
    assert_eq!(state.diag.connections_page, 0);

    // Filter/sort changes reset to the first page.
    let _ = state.update(Message::ConnectionsReceived(snapshot_with(250)));
    let _ = state.update(Message::ConnectionsNextPage);
    let _ = state.update(Message::UpdateRuntimeConnectionFilter("x".into()));
    assert_eq!(state.diag.connections_page, 0);
    let _ = state.update(Message::ConnectionsNextPage);
    let _ = state.update(Message::UpdateRuntimeConnectionSort(
        ConnectionSortKey::HostAsc,
    ));
    assert_eq!(state.diag.connections_page, 0);
}

#[test]
fn connection_idle_timeout_and_activity_tracking() {
    use infiltrator_domain::connection_activity::DEFAULT_IDLE_TIMEOUT_SECS;
    use infiltrator_domain::runtime::{Connection, ConnectionMetadata, ConnectionSnapshot};

    let (mut state, _) = AppState::new();
    assert_eq!(
        state.diag.connection_idle_timeout_secs,
        DEFAULT_IDLE_TIMEOUT_SECS
    );

    // The timeout selector writes one of the shared choices.
    let _ = state.update(Message::SetConnectionIdleTimeout(1800));
    assert_eq!(state.diag.connection_idle_timeout_secs, 1800);

    // Feeding a connection snapshot records its byte totals for idle detection.
    let snapshot = ConnectionSnapshot {
        download_total: 0,
        upload_total: 0,
        connections: vec![Connection {
            id: "c-idle".to_string(),
            metadata: ConnectionMetadata::default(),
            upload: 10,
            download: 20,
            start: String::new(),
            rule: String::new(),
            rule_payload: String::new(),
            chains: Vec::new(),
        }],
    };
    let _ = state.update(Message::ConnectionsReceived(snapshot));
    assert_eq!(state.diag.connection_activity.tracked(), 1);
    assert!(
        state
            .diag
            .connection_activity
            .last_active_secs("c-idle")
            .is_some()
    );
}

#[test]
fn connection_instantaneous_rates_derive_from_successive_snapshots() {
    use infiltrator_domain::connection_rate::HIGH_THROUGHPUT_THRESHOLD_BPS;
    use infiltrator_domain::runtime::{Connection, ConnectionMetadata, ConnectionSnapshot};
    use std::time::{Duration, Instant};

    let (mut state, _) = AppState::new();
    assert!(state.diag.connection_rate_book.is_empty());
    assert!(!state.diag.connection_pulse_active());

    let snapshot_with = |up: u64, down: u64| ConnectionSnapshot {
        download_total: down,
        upload_total: up,
        connections: vec![Connection {
            id: "c-rate".to_string(),
            metadata: ConnectionMetadata::default(),
            upload: up,
            download: down,
            ..Connection::default()
        }],
    };

    // Deterministic timestamps: the derivation divides by the elapsed time we
    // hand it, exactly like the live stream would.
    let base = Instant::now();
    let _ = state.apply_connections_snapshot(snapshot_with(1_000, 2_000), base);
    assert_eq!(
        state.diag.connection_rate_book.get("c-rate"),
        Default::default(),
        "a first observation has no honest rate"
    );

    let _ = state
        .apply_connections_snapshot(snapshot_with(3_000, 10_000), base + Duration::from_secs(2));
    let rate = state.diag.connection_rate_book.get("c-rate");
    assert_eq!(rate.upload_bps, 1_000.0);
    assert_eq!(rate.download_bps, 4_000.0);
    assert!(!state.diag.connection_pulse_active());

    // Crossing the shared 5 MB/s threshold arms the pulse for the next frame.
    let _ = state.apply_connections_snapshot(
        snapshot_with(3_000, 10_000 + (HIGH_THROUGHPUT_THRESHOLD_BPS * 2.0) as u64),
        base + Duration::from_secs(4),
    );
    let rate = state.diag.connection_rate_book.get("c-rate");
    assert!(rate.is_high_throughput());
    assert!(state.diag.connection_pulse_active());

    // A later frame advances the breathing phase while a high-throughput
    // connection stays present, and the shared intensity follows it.
    let _ = state.update(Message::TickFrame(base + Duration::from_millis(1_500)));
    assert!(state.diag.connection_pulse_phase > 0.0);
    assert!(
        pulse_intensity(
            rate.upload_bps,
            rate.download_bps,
            state.diag.connection_pulse_phase
        ) > 0.0
    );

    // Once the connection disappears the book and the pulse go quiet.
    let _ = state.apply_connections_snapshot(
        ConnectionSnapshot {
            download_total: 0,
            upload_total: 0,
            connections: Vec::new(),
        },
        base + Duration::from_secs(6),
    );
    assert!(state.diag.connection_rate_book.is_empty());
    assert!(!state.diag.connection_pulse_active());
    let _ = state.update(Message::TickFrame(base + Duration::from_secs(7)));
    assert_eq!(state.diag.connection_pulse_phase, 0.0);
}

#[test]
fn process_exit_uses_the_host_cleanup_callback_before_shutdown_task() {
    let (mut state, _) = AppState::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let callback_calls = Arc::clone(&calls);
    state.attach_exit_cleanup(Arc::new(move || {
        callback_calls.fetch_add(1, Ordering::SeqCst);
    }));

    let _ = state.update(Message::Exit);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

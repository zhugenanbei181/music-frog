//! Core AppState pipeline tests: navigation, runtime/config/status flows,
//! DNS & profile loading, editor, tray glue, tabs and i18n fallback.
//! Mounted via `src/test_mounts.rs` (crate root).
//! test-intent: behavior

use crate::state::AppState;
use crate::state::core_operations::{CoreDownloadCompletionAdmission, CoreDownloadStart};
use crate::test_mounts::profile_edit_fixture;
use crate::tray::spec::{TRAY_ACTION_QUIT, TrayEvent, TrayIntent, resolve_tray_event};
use crate::types::app::{ConfirmAction, CoreDownloadProgress, Route, SyncProgress, ToastStatus};
use crate::types::dns::DnsTab;
use crate::types::message::Message;
use crate::types::profile_edit::ProfileEditReply;
use crate::types::runtime::{
    IpProbeResult, RebuildFlowState, RuntimePatchSnapshot, RuntimeStreamKind, RuntimeStreamState,
};
use iced::widget::text_editor;
use infiltrator_application::rule_list_fixtures::list_document;
use infiltrator_application::runtime_configuration_application::RuntimeConfigurationReadout;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_control::CoreControlLabel;
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
};
use infiltrator_contract::error::{ErrorCode, Failure, InfiltratorError};
use infiltrator_contract::profile_protection::ProfileWriteAuthorization;
use infiltrator_contract::routing_intent::RoutingIntent;
use infiltrator_contract::routing_intent::RoutingIntentSnapshot;
use infiltrator_contract::routing_intent::RoutingIntentStatus;
use infiltrator_contract::rules_workspace::{RulesJsonSection, RulesTab};
use infiltrator_contract::runtime_control::RuntimeControlStatus;
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::system_toggle::SystemToggleState;
use infiltrator_domain::connection_rate::pulse_intensity;
use infiltrator_domain::profiles::ProfileInfo;
use infiltrator_domain::rules::RuleEntry;
use infiltrator_domain::runtime::TrafficData;
use infiltrator_ports::runtime_configuration::{
    ConfigurationObservation, ConfigurationRevision, DnsRuntimeConfiguration, Ipv6Configuration,
    LanAuthenticationState, LanSecurityConfiguration, LanSharingConfiguration, ObservedValue,
    ProxyPortsConfiguration, SnifferConfiguration, TunConfiguration,
};
use infiltrator_shared::locales::{Lang, Localizer};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn runtime_config_readout(
    tun: TunConfiguration,
    ipv6_enabled: ObservedValue<bool>,
    nameservers: ObservedValue<Vec<String>>,
) -> RuntimeConfigurationReadout {
    let scope = runtime_stream_scope("mihomo", "runtime-config-test", SessionToken::new(91));
    let revision = ConfigurationRevision::new(1);
    RuntimeConfigurationReadout {
        tun: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: tun.clone(),
        },
        lan_sharing: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: LanSharingConfiguration {
                enabled: ObservedValue::Unreported,
                mixed_port: ObservedValue::Unreported,
                bind_address: ObservedValue::Unreported,
            },
        },
        lan_security: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: LanSecurityConfiguration {
                allowed_ips: ObservedValue::Unreported,
                disallowed_ips: ObservedValue::Unreported,
                skip_auth_prefixes: ObservedValue::Unreported,
                authentication: LanAuthenticationState::Unreported,
            },
        },
        ipv6: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: Ipv6Configuration {
                enabled: ipv6_enabled,
                tun_enabled: tun.enabled,
            },
        },
        sniffer: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: SnifferConfiguration {
                enabled: ObservedValue::Unreported,
            },
        },
        dns: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: DnsRuntimeConfiguration {
                nameserver: nameservers,
                ..Default::default()
            },
        },
        core_log_level: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: ObservedValue::Unreported,
        },
        proxy_ports: ConfigurationObservation {
            scope,
            revision,
            value: ProxyPortsConfiguration::default(),
        },
    }
}

#[test]
fn test_route_navigation() {
    let (mut state, _) = AppState::new();
    assert_eq!(state.shell.current_route, Route::Overview);
    assert!(!state.shell.history.can_go_back());
    assert!(!state.shell.history.can_go_forward());

    let _ = state.update(Message::Navigate(Route::Runtime));
    assert_eq!(state.shell.current_route, Route::Runtime);
    assert!(state.shell.history.can_go_back());
    assert!(!state.shell.history.can_go_forward());

    let _ = state.update(Message::Navigate(Route::Settings));
    assert_eq!(state.shell.current_route, Route::Settings);

    let _ = state.update(Message::Navigate(Route::Doctor));
    assert_eq!(state.shell.current_route, Route::Doctor);

    let _ = state.update(Message::Navigate(Route::AppRouting));
    assert_eq!(state.shell.current_route, Route::AppRouting);

    // Back to Doctor
    let _ = state.update(Message::NavigateBack);
    assert_eq!(state.shell.current_route, Route::Doctor);
    assert!(state.shell.history.can_go_forward());

    // Back to Settings
    let _ = state.update(Message::NavigateBack);
    assert_eq!(state.shell.current_route, Route::Settings);

    // Forward to Doctor
    let _ = state.update(Message::NavigateForward);
    assert_eq!(state.shell.current_route, Route::Doctor);

    // Navigate to Proxies branches and clears forward stack
    let _ = state.update(Message::Navigate(Route::Proxies));
    assert_eq!(state.shell.current_route, Route::Proxies);
    assert!(!state.shell.history.can_go_forward());

    // Same route navigation is idempotent
    let _ = state.update(Message::Navigate(Route::Proxies));
    assert_eq!(state.shell.current_route, Route::Proxies);
}

#[test]
fn test_runtime_config_sync() {
    let (mut state, _) = AppState::new();
    let generation = state.runtime.runtime_generation;

    // Simulate config fetch
    let _ = state.update(Message::RuntimeConfigFetched(
        Ok(runtime_config_readout(
            TunConfiguration {
                enabled: ObservedValue::Reported(true),
                stack: ObservedValue::Reported("gvisor".into()),
                auto_route: ObservedValue::Reported(true),
                strict_route: ObservedValue::Reported(false),
                ..Default::default()
            },
            ObservedValue::Reported(false),
            ObservedValue::Reported(vec!["1.1.1.1".into()]),
        )),
        generation,
    ));

    assert_eq!(
        state.runtime.routing_intent, None,
        "runtime config does not contain routing-intent readback"
    );
    assert!(!state.runtime.ipv6_routing.enabled);
    assert!(state.runtime.tun_enabled.unwrap());
    assert_eq!(state.editor.dns_nameservers[0], "1.1.1.1");
}

#[test]
fn runtime_config_replay_preserves_missing_fields_and_typed_failure_blocks_last_observed_tun() {
    let (mut state, _) = AppState::new();
    let generation = state.runtime.runtime_generation;
    state.editor.tun_stack = "draft-stack".into();
    let _ = state.update(Message::RuntimeConfigFetched(
        Ok(runtime_config_readout(
            TunConfiguration::default(),
            ObservedValue::Unreported,
            ObservedValue::Unreported,
        )),
        generation,
    ));
    assert_eq!(state.runtime.tun_enabled, None);
    assert_eq!(state.runtime.system_toggles.tun, SystemToggleState::Unknown);
    assert_eq!(state.runtime.runtime_control.tun_stack, None);
    assert_eq!(state.runtime.runtime_control.ipv6_routing, None);
    assert_eq!(state.editor.tun_stack, "draft-stack");
    let _ = state.update(Message::RuntimeConfigFetched(
        Ok(runtime_config_readout(
            TunConfiguration {
                enabled: ObservedValue::Reported(false),
                stack: ObservedValue::Reported("system".into()),
                ..Default::default()
            },
            ObservedValue::Reported(false),
            ObservedValue::Unreported,
        )),
        generation,
    ));
    assert_eq!(state.runtime.tun_enabled, Some(false));
    assert_eq!(state.editor.tun_stack, "system");
    let failure = Failure::new(ErrorCode::Authentication, "read denied", false);
    let _ = state.update(Message::RuntimeConfigFetched(
        Err(failure.clone()),
        generation,
    ));
    assert_eq!(
        state.runtime.runtime_control.status,
        RuntimeControlStatus::Failed {
            failure: failure.clone()
        }
    );
    assert_eq!(state.runtime.tun_enabled, Some(false));
    assert_eq!(
        state.runtime.runtime_control.tun_stack.as_deref(),
        Some("system")
    );
    assert_eq!(
        state.runtime.system_toggles.tun,
        SystemToggleState::Failed {
            failure: failure.clone()
        }
    );
    assert!(!state.runtime.system_toggles.tun.can_toggle());
    assert_eq!(
        state.runtime.routing_intent_state.status,
        RoutingIntentStatus::Unobserved
    );
    assert_eq!(state.runtime.routing_intent_state.failure, None);
    let before = state.runtime.runtime_control.clone();
    let _ = state.update(Message::RuntimeConfigFetched(
        Ok(runtime_config_readout(
            TunConfiguration::default(),
            ObservedValue::Unreported,
            ObservedValue::Unreported,
        )),
        generation.wrapping_add(1),
    ));
    assert_eq!(state.runtime.runtime_control, before);
}

#[test]
fn routing_intent_actions_keep_typed_readback_and_failures() {
    let (mut state, _) = AppState::new();

    let generation = state.runtime.runtime_generation;
    state.runtime.routing_intent_actions.observe(
        generation,
        1,
        RoutingIntentSnapshot::demo_fixture(),
    );
    let request = state
        .runtime
        .routing_intent_actions
        .begin(RoutingIntent::RouteAllThroughProxy)
        .unwrap();
    let _ = state.update(Message::RoutingIntentFinished {
        request,
        result: Ok(RoutingIntent::RouteAllThroughProxy),
    });
    assert_eq!(
        state.runtime.routing_intent.as_ref(),
        Some(&RoutingIntent::RouteAllThroughProxy)
    );
    assert!(state.shell.error_msg.is_none());
    let request = state
        .runtime
        .routing_intent_actions
        .begin(RoutingIntent::BypassProxy)
        .unwrap();
    let failure = Failure::new(ErrorCode::Network, "API Error", true);
    let _ = state.update(Message::RoutingIntentFinished {
        request,
        result: Err(failure.clone()),
    });
    assert_eq!(state.runtime.routing_intent_actions.failure, Some(failure));
    assert_eq!(
        state.runtime.routing_intent_actions.retry_target(),
        Some(RoutingIntent::BypassProxy)
    );
    assert_eq!(
        state.runtime.routing_intent.as_ref(),
        Some(&RoutingIntent::RouteAllThroughProxy)
    );
    assert!(state.shell.error_msg.is_none());
}

#[test]
fn test_traffic_throttling_logic() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::TrafficReceived(TrafficData {
        up: 1000,
        down: 1000,
    }));
    assert_eq!(state.diag.traffic.as_ref().unwrap().up, 1000);

    // No throttling currently implemented
    let _ = state.update(Message::TrafficReceived(TrafficData {
        up: 1500,
        down: 1500,
    }));
    assert_eq!(state.diag.traffic.as_ref().unwrap().up, 1500);

    // Updated
    let _ = state.update(Message::TrafficReceived(TrafficData {
        up: 3000,
        down: 3000,
    }));
    assert_eq!(state.diag.traffic.as_ref().unwrap().up, 3000);
}

#[test]
fn test_dns_server_list_manipulation() {
    let (mut state, _) = AppState::new();
    state.editor.dns_nameservers = vec!["old".into()];

    let _ = state.update(Message::UpdateDnsServer(0, "new".into()));
    assert_eq!(state.editor.dns_nameservers[0], "new");

    let _ = state.update(Message::AddDnsServer);
    assert_eq!(state.editor.dns_nameservers.len(), 2);

    let _ = state.update(Message::AddDnsServerTemplate(
        "https://1.1.1.1/dns-query".into(),
    ));
    assert_eq!(state.editor.dns_nameservers.len(), 3);
    assert_eq!(state.editor.dns_nameservers[2], "https://1.1.1.1/dns-query");

    let _ = state.update(Message::RemoveDnsServer(0));
    assert_eq!(state.editor.dns_nameservers.len(), 2);
    assert_eq!(state.editor.dns_nameservers[0], "");

    // Fallbacks
    let _ = state.update(Message::AddFallbackDnsServer);
    assert_eq!(state.editor.dns_fallback_servers.len(), 1);

    let _ = state.update(Message::UpdateFallbackDnsServer(0, "8.8.8.8".into()));
    assert_eq!(state.editor.dns_fallback_servers[0], "8.8.8.8");

    let _ = state.update(Message::RemoveFallbackDnsServer(0));
    assert_eq!(state.editor.dns_fallback_servers.len(), 0);
}

#[test]
fn test_system_integration_states() {
    let (mut state, _) = AppState::new();

    // System Proxy
    state.runtime.system_proxy_enabled = false;
    let _ = state.update(Message::SetSystemProxy(true));
    assert!(state.runtime.system_proxy_enabled);

    // Rollback on error
    let _ = state.update(Message::SystemProxySet(Err(InfiltratorError::Privilege(
        "Access denied".into(),
    ))));
    assert!(
        !state.runtime.system_proxy_enabled,
        "Should rollback on failure"
    );
    assert_eq!(
        state.shell.error_msg.as_ref().unwrap(),
        "Privilege error: Access denied"
    );

    // Autostart
    state.runtime.autostart_enabled = false;
    let _ = state.update(Message::SetAutostart(true));
    assert!(state.runtime.autostart_enabled);

    let _ = state.update(Message::AutostartSet(Err(InfiltratorError::Internal(
        "Registry lock".into(),
    ))));
    assert!(
        !state.runtime.autostart_enabled,
        "Should rollback autostart on failure"
    );
}

#[test]
fn test_profiles_and_rules_loading() {
    let (mut state, _) = AppState::new();

    // Profiles loaded
    let _ = state.update(Message::ProfilesLoaded(Ok(vec![ProfileInfo {
        name: "test".into(),
        path: "test.yaml".into(),
        active: true,
        ..Default::default()
    }])));
    assert_eq!(state.profile.profiles.len(), 1);
    assert!(!state.profile.is_loading_profiles);

    // Rules loaded
    let _ = state.update(Message::RulesLoaded(Ok(list_document(vec![RuleEntry {
        rule: "DOMAIN,example.com,DIRECT".into(),
        enabled: true,
    }]))));
    assert_eq!(state.editor.rule_list.draft.len(), 1);
}

#[test]
fn core_lifecycle_controls_require_an_attached_owner() {
    let (mut state, _) = AppState::new();

    let _ = state.update(Message::StartCore);
    assert_eq!(state.runtime.lifecycle_pending, None);
    assert_eq!(
        state.core_control_projection().label,
        CoreControlLabel::Unavailable
    );

    let _ = state.update(Message::CoreStopped);
    assert!(state.diag.traffic.is_none());
}

#[test]
fn test_rebuild_flow_state_transitions() {
    let (mut state, _) = AppState::new();
    state.editor.rule_list.draft = vec![RuleEntry {
        rule: "MATCH,DIRECT".into(),
        enabled: true,
    }];

    let _ = state.update(Message::SaveDns);
    assert!(matches!(
        state.runtime.rebuild_flow,
        RebuildFlowState::Saving { .. }
    ));

    let _ = state.update(Message::RuntimeRebuildFinished(Err(
        InfiltratorError::Backend("boom".into()),
    )));
    assert!(matches!(
        state.runtime.rebuild_flow,
        RebuildFlowState::Failed { .. }
    ));

    let _ = state.update(Message::ClearRebuildFlow);
    assert!(matches!(state.runtime.rebuild_flow, RebuildFlowState::Idle));
}

#[test]
fn test_log_buffer_limit_and_queue() {
    let (mut state, _) = AppState::new();

    for i in 0..650 {
        let _ = state.update(Message::LogReceived(format!("log {}", i)));
    }

    assert_eq!(state.diag.logs.len(), 500);
    assert_eq!(state.diag.logs.front().unwrap(), "log 150");
    assert_eq!(state.diag.logs.back().unwrap(), "log 649");
}

#[test]
fn test_editor_actions() {
    let (mut state, _) = AppState::new();

    // Simulate successful load
    let reply = profile_edit_fixture::document(
        &mut state,
        PathBuf::from("config.yaml"),
        "proxies: []".into(),
    );
    let _ = state.update(reply);
    assert_eq!(
        state.editor.editor_path.as_ref().unwrap().to_str().unwrap(),
        "config.yaml"
    );
    assert_eq!(state.editor.editor_content.text(), "proxies: []");

    // Editor action (simulating typing)
    let _ = state.update(Message::EditorAction(text_editor::Action::Edit(
        text_editor::Edit::Insert('a'),
    )));
    assert_ne!(state.editor.editor_content.text(), "proxies: []");

    // An unrelated old result cannot claim this draft saved.
    state.shell.current_route = Route::Editor;
    let pending = state
        .editor
        .document_session
        .begin_document(
            "proxies: []".into(),
            ProfileWriteAuthorization::RequireUnlock,
        )
        .unwrap();
    let _ = state.update(Message::ProfileSaved(ProfileEditReply {
        pending,
        result: Ok(CommandOutput::Unit),
    }));
    assert_eq!(
        match &state.editor.document_session.operation {
            infiltrator_application::profile_edit_session::ProfileEditOperationPhase::Failed(
                failure,
            ) => failure.code.clone(),
            infiltrator_application::profile_edit_session::ProfileEditOperationPhase::Ready
            | infiltrator_application::profile_edit_session::ProfileEditOperationPhase::Pending(
                _,
            )
            | infiltrator_application::profile_edit_session::ProfileEditOperationPhase::Saved => {
                panic!("mismatched terminal output must be a typed failure")
            }
        },
        ErrorCode::InvalidState
    );
    assert!(!matches!(
        &state.editor.document_session.operation,
        infiltrator_application::profile_edit_session::ProfileEditOperationPhase::Saved
    ));
    assert_eq!(state.shell.current_route, Route::Editor);
}

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
    assert_eq!(state.editor.rules_tab, RulesTab::JsonEditors);
    assert_eq!(state.editor.rules_page, 0);

    let _ = state.update(Message::SetRulesJsonTab(RulesJsonSection::Sniffer));
    assert_eq!(state.editor.rules_json_tab, RulesJsonSection::Sniffer);

    let _ = state.update(Message::SetDnsTab(DnsTab::Tun));
    assert_eq!(state.editor.dns_tab, DnsTab::Tun);
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

fn runtime_stream_scope(engine: &str, instance: &str, session_token: SessionToken) -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::new(engine).expect("engine ID"),
            CoreInstanceId::new(instance).expect("instance ID"),
            None,
        ),
        generation: CoreGeneration::new(7),
        session_token: Some(session_token),
    }
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
            infiltrator_application::language_choice::LanguageChoicePhase::Rejected(failure) => {
                failure.code.clone()
            }
            infiltrator_application::language_choice::LanguageChoicePhase::Ready
            | infiltrator_application::language_choice::LanguageChoicePhase::Pending(_)
            | infiltrator_application::language_choice::LanguageChoicePhase::Failed { .. }
            | infiltrator_application::language_choice::LanguageChoicePhase::AwaitingObservation {
                ..
            } => panic!("uncomposed language change must be rejected"),
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
    state.runtime.pending_runtime_patch = Some(RuntimePatchSnapshot {
        ipv6_enabled: true,
        tun_enabled: Some(false),
        tun_stack: "gvisor".to_string(),
        tun_stack_selector: "gvisor".to_string(),
        tun_auto_route: true,
        tun_strict_route: false,
        sniffer_enabled: true,
    });
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
    assert!(state.runtime.pending_runtime_patch.is_none());
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
fn connections_pagination_windows_and_clamps() {
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
    let _ = state.update(Message::UpdateRuntimeConnectionSort("host_asc".into()));
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

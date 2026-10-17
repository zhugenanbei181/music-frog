//! Core AppState pipeline tests: navigation, runtime/config/status flows,
//! DNS & profile loading, editor, tray glue, tabs and i18n fallback.
//! Mounted via `src/test_mounts.rs` (crate root).
//! test-intent: behavior

use crate::state::AppState;
use crate::state::core_operations::{CoreDownloadCompletionAdmission, CoreDownloadStart};
use crate::state::profiles::ProfileListLoadPhase;
use crate::test_mounts::profile_edit_fixture;
use crate::tray::spec::{TRAY_ACTION_QUIT, TrayEvent, TrayIntent, resolve_tray_event};
use crate::types::app::{ConfirmAction, CoreDownloadProgress, Route, SyncProgress, ToastStatus};
use crate::types::dns::DnsTab;
use crate::types::message::Message;
use crate::types::profile_edit::ProfileEditReply;
use crate::types::runtime::{
    ConfigurationSaveState, IpProbeResult, RuntimePatchSnapshot, RuntimeStreamKind,
    RuntimeStreamState,
};
use iced::widget::text_editor;
use infiltrator_application::runtime_configuration_application::RuntimeConfigurationReadout;
use infiltrator_composition::capture_fixtures::list_document;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_control::CoreControlLabel;
use infiltrator_contract::profile_protection::ProfileWriteAuthorization;
use infiltrator_contract::rules_workspace::{RulesJsonSection, RulesTab};
use infiltrator_contract::runtime_control::RuntimeControlStatus;
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
use infiltrator_presentation::language_choice::LanguageChoicePhase;
use infiltrator_presentation::profile_edit_session::ProfileEditOperationPhase;
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_core_contract::error::{ErrorCode, Failure, InfiltratorError};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::routing_intent::RoutingIntent;
use lurker_core_contract::routing_intent::RoutingIntentSnapshot;
use lurker_core_contract::routing_intent::RoutingIntentStatus;
use lurker_core_contract::session::SessionToken;
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
    assert_eq!(
        state.runtime.routing_intent_actions.failure().cloned(),
        Some(failure)
    );
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
    assert_eq!(state.profile.list_load_phase, ProfileListLoadPhase::Idle);

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
fn test_configuration_save_state_transitions() {
    let (mut state, _) = AppState::new();
    state.editor.rule_list.draft = vec![RuleEntry {
        rule: "MATCH,DIRECT".into(),
        enabled: true,
    }];

    let _ = state.update(Message::SaveDns);
    assert!(matches!(
        state.runtime.configuration_save_state,
        ConfigurationSaveState::Saving { .. }
    ));

    let _ = state.update(Message::DnsSaved(Err(InfiltratorError::Backend(
        "boom".into(),
    ))));
    assert!(matches!(
        state.runtime.configuration_save_state,
        ConfigurationSaveState::Failed { .. }
    ));

    let _ = state.update(Message::ClearConfigurationSaveState);
    assert!(matches!(
        state.runtime.configuration_save_state,
        ConfigurationSaveState::Idle
    ));
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
            ProfileEditOperationPhase::Failed(failure) => failure.code.clone(),
            ProfileEditOperationPhase::Ready
            | ProfileEditOperationPhase::Pending(_)
            | ProfileEditOperationPhase::Saved => {
                panic!("mismatched terminal output must be a typed failure")
            }
        },
        ErrorCode::InvalidState
    );
    assert!(!matches!(
        &state.editor.document_session.operation,
        ProfileEditOperationPhase::Saved
    ));
    assert_eq!(state.shell.current_route, Route::Editor);
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

#[path = "app_state/routing_tests.rs"]
mod routing_tests;

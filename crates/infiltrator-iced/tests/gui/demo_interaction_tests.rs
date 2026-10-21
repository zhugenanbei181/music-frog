//! test-intent: behavior
//! The demo performs the business's own gesture and the surface state proves it.
//!
//! Every test here drives the real demo activation (the same dispatch the
//! capture pipeline uses) and then reads the real surface state. A demo that
//! only repainted its page could not satisfy any of these assertions.
use super::*;
use crate::demo::sync::demo_conflict_key;
use crate::state::AppState;
use crate::test_mounts::surface_task_harness::drain_surface_task;
use crate::types::app::ConfirmAction;
use infiltrator_contract::overview_layout::OverviewCardKind;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::privileged_network::PrivilegedNetworkState;
use infiltrator_contract::reconnect_mask::ReconnectMaskStatus;
use infiltrator_contract::rules_workspace::RulesTab;
use infiltrator_contract::shell_readout::ShellObservation;
use infiltrator_contract::snapshot::CoreWatchdogState;
use infiltrator_contract::speedtest::SpeedtestRunState;
use infiltrator_domain::app_routing::AppRoutingRule;
use infiltrator_ports::host_runtime::TunServiceStatus;
use infiltrator_presentation::doctor_actions::DoctorActionPhase;
use infiltrator_presentation::traffic_readout::TrafficFreshness;
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::routing_intent::RoutingIntent;

fn demo_env(page: Route, scenario: FeatureId) -> DemoEnv {
    DemoEnv {
        enabled: true,
        page,
        pane: EditorPane::Profile,
        providers_tab: false,
        lang: "zh-CN".to_string(),
        skin: iced::Theme::Dark,
        window_size: DEFAULT_WINDOW,
        capture_marker: None,
        scenario: Some(scenario),
    }
}

/// Drive the demo activation's task to the end, feeding every message the
/// surface produces back into the real update pipeline.
///
/// The demo pipeline is the surface pipeline plus the demo's own scenario
/// routing, so the drained messages travel through the same update path the
/// demo runtime uses.
fn drain(state: &mut AppState, task: Task<Message>) {
    drain_surface_task(state, task, update);
}

#[test]
fn the_proxy_search_narrows_the_shared_list_and_highlights_the_match() {
    let env = demo_env(Route::Proxies, FeatureId::ProxiesSearchHighlight);
    let (state, _task) = AppState::demo(&env);
    assert_eq!(state.runtime.proxy_filter, "HK");
    let (group, members) = state
        .runtime
        .filtered_groups
        .first()
        .expect("the search keeps the matching group");
    assert_eq!(group, "PROXIES");
    assert_eq!(members.len(), 2);
    assert!(
        state
            .runtime
            .proxy_name_runs
            .get("HK-01")
            .is_some_and(|runs| runs.iter().any(|run| run.highlighted && run.text == "HK"))
    );
}

#[test]
fn the_proxy_group_starts_folded_and_the_demo_opens_it() {
    // The fixture folds the first group, so the fold business has something
    // to open. The fold is the shared proxies model's fact, so the fixture is
    // read before the capture opens it.
    let mut fixture_env = demo_env(Route::Proxies, FeatureId::ProxiesSearchHighlight);
    fixture_env.scenario = None;
    let (fixture, _task) = AppState::demo(&fixture_env);
    let group_id = fixture
        .runtime
        .proxy_groups
        .iter()
        .find(|group| group.name == "PROXIES")
        .and_then(|group| group.id.clone())
        .expect("typed demo collection");
    assert!(
        !fixture.runtime.proxies_model.group_expanded(&group_id),
        "the demo fixture folds the first group so the fold has something to open"
    );

    let (opened, _task) =
        AppState::demo(&demo_env(Route::Proxies, FeatureId::ProxiesGroupExpanded));
    assert!(
        opened.runtime.proxies_model.group_expanded(&group_id),
        "the demo opened the group the fold business is about"
    );
}

#[test]
fn the_overview_card_order_moves_one_real_card() {
    let env = demo_env(Route::Overview, FeatureId::OverviewCardReorder);
    let (mut state, task) = AppState::demo(&env);
    // The move is the owner's business: the surface dispatches the shared
    // intent and the owner's published order is what the page renders.
    drain(&mut state, task);
    let order = &state.diag.overview_card_order;
    assert_eq!(order.first(), Some(&OverviewCardKind::Traffic));
    assert_ne!(order, &OverviewCardKind::DEFAULT_ORDER.to_vec());
}

#[test]
fn the_rule_search_narrows_the_shared_rule_list() {
    let env = demo_env(Route::Rules, FeatureId::RulesSearchHighlight);
    let (state, _task) = AppState::demo(&env);
    assert_eq!(state.editor.rules_model.filter_text(), "google.com");
    assert_eq!(state.editor.rules_filtered_count(), 1);
    assert!(state.editor.rule_list.draft.len() > 1);
}

#[test]
fn the_aggregation_wizard_opens_on_the_profiles_page() {
    let env = demo_env(Route::Profiles, FeatureId::ProfilesAggregatorWizard);
    let (state, _task) = AppState::demo(&env);
    assert!(state.profile.aggregator_modal_open);
}

#[test]
fn the_profile_delete_asks_for_the_confirmation_first() {
    let env = demo_env(Route::Profiles, FeatureId::ProfilesDeleteConfirm);
    let (state, _task) = AppState::demo(&env);
    assert!(matches!(
        state.shell.confirmation,
        Some(ConfirmAction::DeleteProfile(_))
    ));
}

#[test]
fn the_routing_intent_capture_lands_the_owner_network_refusal() {
    let env = demo_env(Route::Overview, FeatureId::ShellRoutingIntentControl);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    let actions = &state.runtime.routing_intent_actions;
    assert!(
        actions
            .failure()
            .is_some_and(|failure| failure.code == ErrorCode::Network)
    );
    assert_eq!(
        actions.retry_target(),
        Some(RoutingIntent::RouteAllThroughProxy)
    );
    assert!(actions.pending_change().is_none());
    assert_eq!(
        actions.render_snapshot().current,
        Some(RoutingIntent::FollowRules)
    );
}

#[test]
fn the_routing_intent_capture_keeps_the_owner_authentication_refusal() {
    let env = demo_env(Route::Overview, FeatureId::ShellRoutingIntentAuthentication);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    let actions = &state.runtime.routing_intent_actions;
    assert!(
        actions
            .failure()
            .is_some_and(|failure| failure.code == ErrorCode::Authentication)
    );
    assert!(actions.needs_controller_settings());
    assert!(actions.retry_target().is_none());
}

#[test]
fn the_telemetry_capture_observes_the_stale_zero_readback() {
    let env = demo_env(Route::Runtime, FeatureId::RuntimeTelemetryObservation);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    // The stale zero readback is a typed fact; only its copy is localised, so
    // the assertion never compares a copy.
    let readout = state.traffic_readout();
    assert_eq!(readout.freshness, TrafficFreshness::Failed);
    assert!(matches!(
        state.shell.readout.upload_bps,
        ShellObservation::Stale(rate) | ShellObservation::Current(rate) if rate == 0.0
    ));
    assert!(
        state
            .shell
            .readout
            .rate_failure
            .as_ref()
            .is_some_and(|failure| failure.message.contains("isolated telemetry read denied"))
    );
}

#[test]
fn the_hosts_capture_lands_the_owner_permission_refusal() {
    let env = demo_env(Route::Dns, FeatureId::DnsHostsEditor);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    let editor = &state.editor.dns_hosts_editor;
    assert!(editor.is_open());
    assert!(!editor.is_pending());
    assert!(
        editor
            .failure()
            .is_some_and(|failure| failure.code == ErrorCode::Permission)
    );
    assert!(editor.is_importing_legacy());
    assert_eq!(editor.rows.len(), 3);
}

#[test]
fn the_query_capture_lands_the_owner_answer() {
    let env = demo_env(Route::Dns, FeatureId::DnsQueryDetails);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    let model = &state.diag.dns_query;
    assert!(model.is_open());
    assert!(!model.is_pending());
    assert!(model.current_failure().is_none());
    assert_eq!(model.requested_operation(), model.snapshot.report_id);
    assert!(
        model
            .snapshot
            .report
            .as_ref()
            .is_some_and(|report| report.response.answers.len() == 2)
    );
}

#[test]
fn the_tracer_capture_lands_the_owner_decision_chain() {
    let env = demo_env(Route::Rules, FeatureId::RulesTracerDrawer);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    let model = &state.editor.rule_trace;
    assert_eq!(state.editor.rules_model.tab(), RulesTab::Tracer);
    assert!(!model.busy());
    assert!(model.current_failure().is_none());
    assert!(model.snapshot.report.as_ref().is_some_and(|report| {
        report
            .decision_chain
            .as_ref()
            .is_some_and(|chain| chain.matched_rule_raw == "DOMAIN-SUFFIX,google.com,PROXY")
    }));
}

#[test]
fn the_rule_list_capture_lands_the_owner_commit_refusal() {
    let env = demo_env(Route::Rules, FeatureId::RulesListEditor);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    let model = &state.editor.rule_list;
    assert_eq!(state.editor.rules_model.tab(), RulesTab::List);
    assert!(model.dirty());
    assert!(model.can_save());
    assert!(!model.is_pending());
    assert!(
        model
            .failure()
            .is_some_and(|failure| failure.code == ErrorCode::Permission)
    );
}

#[test]
fn the_filter_capture_lands_the_owner_permission_refusal() {
    let env = demo_env(Route::Editor, FeatureId::ProfilesFilterEditor);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    let editor = &state.editor.filter_editor;
    assert!(editor.pending().is_none());
    assert!(
        editor
            .failure()
            .is_some_and(|failure| failure.code == ErrorCode::Permission)
    );
}

#[test]
fn the_tun_permission_capture_reports_the_missing_privilege_and_the_host_answer() {
    let env = demo_env(Route::Settings, FeatureId::SettingsTunPermission);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    assert_eq!(
        state.runtime.tun_service_status,
        Some(TunServiceStatus::MissingPrivilege),
        "the host reported that the TUN service lacks the elevation privilege"
    );
    // The host's refusal is the shell's error banner: the toast that
    // announces it is auto-dismissed, the banner is the durable fact.
    let refusal = state
        .shell
        .error_msg
        .as_deref()
        .expect("the TUN permission preparation was answered by the host");
    assert!(
        refusal.contains("TUN"),
        "the refusal names the TUN service the host could not prepare: {refusal}"
    );
}

#[test]
fn the_lan_security_capture_types_the_shared_allow_list_draft() {
    let env = demo_env(Route::Settings, FeatureId::SettingsLanSecurity);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    assert_eq!(
        state.runtime.settings_text.lan_allowed_ips, "10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16",
        "the surface carries the allow list the user typed"
    );
    assert_eq!(
        state
            .runtime
            .settings_model
            .lan_security()
            .allowed_ips
            .len(),
        3,
        "the shared settings model parsed the typed allow list"
    );
}

#[test]
fn the_port_conflict_capture_lands_the_owner_conflict_report() {
    let env = demo_env(Route::Settings, FeatureId::SettingsPortConflictRepair);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    assert!(
        state.runtime.port_conflicts.has_conflicts(),
        "the shared port-conflict owner reported the taken proxy port"
    );
    assert_eq!(state.runtime.port_conflicts.conflicts.len(), 1);
}

#[test]
fn the_permission_panel_capture_lands_the_typed_unsupported_status() {
    let env = demo_env(Route::Settings, FeatureId::SettingsPermissionPanel);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    assert!(
        matches!(
            state.runtime.privileged_network.state,
            PrivilegedNetworkState::Unsupported { .. }
        ),
        "the host answered the privileged regression with its typed unsupported status"
    );
}

#[test]
fn the_core_rollback_capture_arms_the_confirmation_instead_of_rolling_back() {
    let env = demo_env(Route::Settings, FeatureId::CoreVersionRollbackConfirm);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    assert_eq!(
        state.runtime.core_versions.rollback.target.as_deref(),
        Some("v1.18.8"),
        "the owner published the previously runnable core the rollback control offers"
    );
    assert!(
        matches!(
            state.shell.confirmation,
            Some(ConfirmAction::RollbackCore(_))
        ),
        "the destructive rollback is armed behind the shared confirmation, never dispatched"
    );
}

#[test]
fn the_topology_drilldown_capture_lands_on_the_shared_navigation_target() {
    let env = demo_env(Route::Overview, FeatureId::OverviewTopologyDrilldown);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    assert_eq!(
        state.shell.current_route,
        Route::Rules,
        "the topology stage drilled down to its shared navigation target"
    );
    assert_eq!(
        state.editor.rules_model.filter_text(),
        "mrs / geoip",
        "the drill-down carried the stage detail into the target page"
    );
}

#[test]
fn the_speedtest_cancel_capture_lands_the_owner_cancellation() {
    let env = demo_env(Route::Overview, FeatureId::SpeedtestProgressCancel);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    assert!(
        matches!(
            state.diag.speedtest.state,
            SpeedtestRunState::CancelRequested { .. }
        ),
        "the owner accepted the cancellation of the running probe"
    );
}

#[test]
fn the_speedtest_history_capture_compares_two_recorded_runs() {
    let env = demo_env(Route::Runtime, FeatureId::SpeedtestHistoryComparison);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    assert!(state.diag.speedtest_detail_open);
    assert_eq!(
        state.diag.speedtest.recent_history.len(),
        2,
        "the comparison view carries two recorded runs"
    );
}

#[test]
fn the_keyboard_focus_capture_moves_the_focus_ring_with_the_arrow_key() {
    let env = demo_env(Route::Overview, FeatureId::ShellKeyboardFocus);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    assert!(state.shell.command_palette_open);
    assert_eq!(
        state.shell.command_selected_index, 1,
        "the arrow key moved the focus ring to the next catalogue row"
    );
}

#[test]
fn the_reconnect_mask_capture_lands_the_watchdog_reconnection() {
    let env = demo_env(Route::Overview, FeatureId::OverviewReconnectMask);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    assert_eq!(
        state.runtime.reconnect_mask.status,
        ReconnectMaskStatus::Reconnecting,
        "the mask is the projection of the watchdog's waiting state"
    );
    assert_eq!(state.runtime.reconnect_mask.attempt, Some(1));
}

#[test]
fn the_lifecycle_recovery_capture_lands_the_watchdog_recovery_state() {
    let env = demo_env(Route::Doctor, FeatureId::LifecycleRecoveryPanel);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    assert!(
        matches!(
            state.diag.crash_watchdog.shared.state,
            CoreWatchdogState::Waiting { .. }
        ),
        "the lifecycle recovery panel shows the watchdog's own recovery state"
    );
    assert!(
        matches!(state.diag.doctor.action.phase, DoctorActionPhase::Idle),
        "the diagnostic command phase stays untouched, so the two doctor states cannot fight"
    );
}

#[test]
fn the_profile_import_capture_types_the_subscription_url() {
    let env = demo_env(Route::Profiles, FeatureId::ProfilesImportModal);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    assert_eq!(
        state.profile.import_url, "https://sub.example.com/api/v1/client/subscribe?token=demo",
        "the import form carries the subscription URL the user typed"
    );
    assert!(!state.profile.import_name.is_empty());
}

#[test]
fn the_app_routing_capture_edits_the_per_application_rule() {
    let env = demo_env(Route::AppRouting, FeatureId::AppRoutingPolicyEditor);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    assert!(
        !state.app_routing.processes.is_empty(),
        "the policy grid renders the demo's processes"
    );
    assert_eq!(
        state.app_routing.custom_rules.get("chrome").copied(),
        Some(AppRoutingRule::Direct),
        "the page carries the rule the user picked"
    );
}

#[test]
fn the_subscription_capture_lands_the_owner_network_refusal() {
    let env = demo_env(Route::Profiles, FeatureId::ProfilesSubscriptionError);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    let refusal = state
        .shell
        .error_msg
        .as_deref()
        .expect("the owner refused the subscription refresh");
    assert!(
        refusal.contains("capture subscription source is offline"),
        "the banner carries the owner's own refusal: {refusal}"
    );
}

#[test]
fn the_aggregation_preview_capture_renders_the_shared_report() {
    let env = demo_env(Route::Profiles, FeatureId::ProfilesAggregatorPreview);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    let report = state.profile.aggregator_report.as_ref().unwrap_or_else(|| {
        panic!(
            "the shared aggregation application published its report: error={:?} selected={:?}",
            state.shell.error_msg, state.profile.aggregator_selected_profiles
        )
    });
    assert!(
        report.source_count > 0,
        "the report names the sources the shared application aggregated"
    );
    assert!(
        !report.yaml.is_empty(),
        "the report carries the generated document"
    );
}

#[test]
fn the_script_sandbox_capture_runs_the_shared_engine() {
    let env = demo_env(Route::Editor, FeatureId::ProfilesScriptSandbox);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    let snapshot = state
        .editor
        .script_sandbox
        .snapshot()
        .expect("the shared script engine published its report");
    assert!(
        !snapshot.matched_directives.is_empty(),
        "the shared engine really evaluated the preset directives"
    );
}

#[test]
fn the_script_export_capture_opens_the_owner_review() {
    let env = demo_env(Route::Editor, FeatureId::ProfilesScriptExportReview);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    assert!(
        state.editor.script_sandbox.is_export_visible(),
        "the export review is open over the owner's frozen bytes"
    );
    let export = state
        .editor
        .script_sandbox
        .export()
        .expect("the review carries the owner's export snapshot");
    assert!(
        !export.file_name.is_empty(),
        "the review names the artifact the owner composed"
    );
}

#[test]
fn the_diff_viewer_capture_renders_the_shared_diff() {
    let env = demo_env(Route::Editor, FeatureId::ProfilesDiffViewer);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    let diff = state
        .editor
        .snapshot_diff_modal
        .diff()
        .expect("the shared snapshot application published its diff");
    assert!(
        !diff.unified_lines.is_empty(),
        "the modal renders the shared application's real diff"
    );
}

#[test]
fn the_yaml_error_capture_navigates_to_the_codec_diagnostic() {
    let env = demo_env(Route::Editor, FeatureId::EditorYamlErrorNavigation);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    let line = state.editor.syntax_error_line.unwrap_or_else(|| {
        panic!(
            "the shared codec reported the offending line: syntax_error={:?} lines={} can_edit={}",
            state.editor.syntax_error,
            state.editor.editor_content.lines().count(),
            state.editor.document_session.can_edit()
        )
    });
    assert!(
        state.editor.syntax_error.is_some(),
        "the editor carries the codec's own diagnostic"
    );
    assert!(
        line > 1,
        "the diagnostic names the line the editor navigated to: {line}"
    );
}

#[test]
fn the_sync_conflict_capture_records_the_per_key_decision() {
    let env = demo_env(Route::Sync, FeatureId::SyncConflictResolver);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    let diff = state
        .profile
        .sync_diff
        .as_ref()
        .expect("the merge session is open");
    let key = demo_conflict_key(&state).expect("the merge session has a key");
    assert_eq!(
        diff.picks.get(&key).copied(),
        Some(true),
        "the session carries the user's per-key decision"
    );
}

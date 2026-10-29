//! The capture is published only when the surface state proves the business.
//!
//! Every predicate reads the real surface state (or the dispatched command)
//! the business owns. A demo that only repaints its page can never satisfy
//! one of them, so the pixel judge never sees a route-only render published
//! as an activated business.
use crate::demo::app_routing::DEMO_PROCESS;
use crate::demo::profiles::IMPORT_URL;
use crate::demo::rule_provider::CAPTURE_REFRESH_FAILURE;
use crate::demo::rules::RULE_SEARCH_QUERY;
use crate::demo::settings::LAN_ALLOWED_IPS;
use crate::demo::sync::demo_conflict_key;
use crate::state::AppState;
use crate::state::editor::SnapshotDiffModalState;
use crate::state::runtime::{ProbeSettingsDialogState, ProxyGroupOrderDialogState};
use crate::types::app::{ConfirmAction, Route};
use infiltrator_contract::doctor::DoctorAction;
use infiltrator_contract::fixture_identities::HK_PRIMARY;
use infiltrator_contract::overview_layout::OverviewCardKind;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::privileged_network::PrivilegedNetworkState;
use infiltrator_contract::reconnect_mask::ReconnectMaskStatus;
use infiltrator_contract::rules_workspace::RulesTab;
use infiltrator_contract::shell_readout::ShellObservation;
use infiltrator_contract::snapshot::CoreWatchdogState;
use infiltrator_contract::speedtest::SpeedtestRunState;
use infiltrator_contract::traffic_topology::TrafficTopologyStage;
use infiltrator_domain::app_routing::AppRoutingRule;
use infiltrator_domain::connection_view::ConnectionGroupingMode;
use infiltrator_ports::host_runtime::TunServiceStatus;
use infiltrator_presentation::doctor_actions::DoctorActionPhase;
use infiltrator_presentation::language_choice::LanguageChoicePhase;
use infiltrator_presentation::traffic_readout::TrafficFreshness;
use infiltrator_test_support::doctor_capture_fixtures::CHECK_ID;
use infiltrator_test_support::filter_capture_store::{FILTER_POLICY, FILTER_PROFILE};
use infiltrator_test_support::profile_store_fixtures::LARGE_PROFILE_RULE_COUNT;
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::outbound::details::OutboundObservation;
use lurker_core_contract::routing_intent::RoutingIntent;
use lurker_ui_presentation::dns_leak_actions::DnsLeakActionPhase;

pub(crate) fn ready(state: &AppState, feature: FeatureId) -> bool {
    if state.shell.capture_region_bounds.is_none() {
        return false;
    }
    match feature {
        FeatureId::RuntimeTelemetryObservation => {
            // The stale zero readback is a typed fact; only its copy is
            // localised, so the readiness check never compares a copy.
            let readout = state.traffic_readout();
            matches!(readout.freshness, TrafficFreshness::Failed)
                && matches!(
                    state.shell.readout.upload_bps,
                    ShellObservation::Stale(rate) | ShellObservation::Current(rate) if rate == 0.0
                )
                && state
                    .shell
                    .readout
                    .rate_failure
                    .as_ref()
                    .is_some_and(|failure| {
                        failure.message.contains("isolated telemetry read denied")
                    })
        }
        FeatureId::LogsRedactedExport => {
            state.diag.log_export.is_open()
                && !state.diag.log_export.is_pending()
                && state
                    .diag
                    .log_export
                    .summary()
                    .is_some_and(|summary| summary.records == 3)
                && state.diag.log_export.receipt().is_none()
                && state
                    .diag
                    .log_export
                    .failure()
                    .is_some_and(|failure| failure.code == ErrorCode::Permission)
        }
        FeatureId::LogsScrollLock => {
            !state.diag.logs_page.follow().should_follow()
                && state.diag.logs_page.source_current()
                && state.diag.logs_page.source_count() == 35
        }
        FeatureId::LogsSearchHighlight => {
            state.diag.logs_page.query() == "api\\.example|timeout"
                && state.diag.logs_page.source_current()
                && state.diag.logs_page.source_count() == 3
                && state.diag.logs_page.matched_count() == 2
        }
        FeatureId::ShellRoutingIntentControl | FeatureId::ShellRoutingIntentAuthentication => {
            let actions = &state.runtime.routing_intent_actions;
            let failed = if feature == FeatureId::ShellRoutingIntentAuthentication {
                actions.needs_controller_settings()
                    && actions.retry_target().is_none()
                    && actions
                        .failure()
                        .is_some_and(|failure| failure.code == ErrorCode::Authentication)
            } else {
                actions.retry_target() == Some(RoutingIntent::RouteAllThroughProxy)
                    && actions
                        .failure()
                        .is_some_and(|failure| failure.code == ErrorCode::Network)
            };
            failed
                && actions.pending_change().is_none()
                && actions.render_snapshot().current == Some(RoutingIntent::FollowRules)
        }
        FeatureId::ProfilesSnapshotRestoreConfirm | FeatureId::SyncRestoreConfirm => {
            let model = &state.editor.snapshot_restore;
            model.is_visible()
                && !model.is_busy()
                && model.failure().is_none()
                && model.review().is_some()
        }
        FeatureId::ProfilesFilterEditor => {
            let editor = &state.editor.filter_editor;
            editor.source_profile() == Some(FILTER_PROFILE)
                && editor.draft.include == "HK"
                && editor.draft.advanced_policy.as_deref() == Some(FILTER_POLICY)
                && editor.pending().is_none()
                && editor
                    .failure()
                    .is_some_and(|failure| failure.code == ErrorCode::Permission)
        }
        FeatureId::RulesStatisticsInspector => {
            let model = &state.editor.rule_hit_audit;
            model.current()
                && model.confirmation().is_some()
                && model.can_confirm_cleanup(&state.editor.rule_list)
                && !state.editor.rule_list.dirty()
        }
        FeatureId::RulesListEditor => {
            let model = &state.editor.rule_list;
            state.editor.rules_model.tab() == RulesTab::List
                && !model.draft.is_empty()
                && model.dirty()
                && model.can_save()
                && !model.is_pending()
                && model
                    .failure()
                    .is_some_and(|failure| failure.code == ErrorCode::Permission)
                && model.draft.first().is_some_and(|rule| {
                    !rule.enabled && rule.rule == "SRC-IP-CIDR,10.0.0.0/8,DIRECT"
                })
        }
        FeatureId::RulesOverrideEditor => {
            state
                .editor
                .rule_trace
                .override_pending_operation()
                .is_none()
                && matches!(&state.shell.confirmation, Some(ConfirmAction::TracerOverride(request))
                if request.expected_rule == "DOMAIN-SUFFIX,google.com,PROXY" && request.new_target == "DIRECT")
                && state.editor.rule_trace.confirmation().is_some()
        }
        FeatureId::RulesTracerDrawer => {
            let model = &state.editor.rule_trace;
            state.editor.rules_model.tab() == RulesTab::Tracer
                && !model.busy()
                && model.current_failure().is_none()
                && model.snapshot.report.as_ref().is_some_and(|report| {
                    model.draft_matches(report)
                        && report.decision_chain.as_ref().is_some_and(|chain| {
                            chain.matched_rule_raw == "DOMAIN-SUFFIX,google.com,PROXY"
                        })
                })
        }
        FeatureId::DnsQueryDetails => {
            let model = &state.diag.dns_query;
            model.is_open()
                && !model.is_pending()
                && model.current_failure().is_none()
                && model.requested_operation().is_some()
                && model.requested_operation() == model.snapshot.report_id
                && model
                    .snapshot
                    .report
                    .as_ref()
                    .is_some_and(|report| report.response.answers.len() == 2)
        }
        FeatureId::DnsFakeIpFlushConfirm => {
            let model = &state.diag.dns_cache_actions;
            model.is_open() && !model.is_confirmed() && !model.is_pending()
        }
        FeatureId::DnsHostsEditor => {
            let editor = &state.editor.dns_hosts_editor;
            editor.is_open()
                && !editor.is_pending()
                && editor.can_apply()
                && editor
                    .failure()
                    .is_some_and(|failure| failure.code == ErrorCode::Permission)
                && editor.is_importing_legacy()
                && editor.rows.len() == 3
                && editor.applied.as_ref().is_some_and(|profile| {
                    profile.entries.len() == 2 && profile.legacy_entries.len() == 1
                })
        }
        FeatureId::DnsLeakAlert => {
            matches!(state.diag.dns_leak_action.phase, DnsLeakActionPhase::Idle)
                && state.editor.dns_leak.conclusion().is_divergent()
                && state.editor.dns_leak.observations.len() == 2
        }
        FeatureId::DoctorFailureRecovery => {
            matches!(
                &state.diag.doctor.action.phase,
                DoctorActionPhase::Failed {
                    failure,
                    ..
                } if failure.code == ErrorCode::Permission
            ) && state.surface.latest().is_some_and(|snapshot| {
                snapshot
                    .pages
                    .doctor
                    .data
                    .as_ref()
                    .is_some_and(|data| data.checks.iter().any(|check| check.id == CHECK_ID))
            })
        }
        FeatureId::DoctorRepairProgress => {
            // The owner published its pending phase: the repair command runs
            // and the surface renders the phase the owner really holds.
            state.diag.doctor.is_busy()
                && matches!(
                    state.diag.doctor.current_action(),
                    Some(DoctorAction::RepairAll | DoctorAction::RepairOne(_))
                )
        }
        FeatureId::SettingsLanguageChoice => {
            state.shell.lang == "en-US"
                && matches!(
                    &state.shell.language_choice.phase,
                    LanguageChoicePhase::Ready
                )
                && state.shell.language_user_selected
        }
        FeatureId::ConnectionsCloseAllConfirm => {
            state.shell.confirmation == Some(ConfirmAction::CloseAllConnections)
        }
        FeatureId::ConnectionsSearchHighlight => {
            state.diag.connections_model.mode().is_flat()
                && state.runtime.runtime_connection_filter == "API-0"
                && state.diag.connections_model.matched_count() == 2
                && state.diag.connections_model.source_count() == 9
                && state
                    .diag
                    .connections_model
                    .search_row("group-0")
                    .is_some_and(|row| {
                        row.endpoint
                            .iter()
                            .any(|run| run.highlighted && run.text == "api-0")
                    })
        }
        FeatureId::ConnectionsGrouping => {
            let groups = &state.diag.connections_model;
            groups.mode() == ConnectionGroupingMode::ByProcess
                && state.runtime.runtime_connection_filter == "CLIENT-0"
                && groups.rows().len() == 1
                && groups.rows()[0].key == "client-0"
                && groups.rows()[0].count == 2
                && groups.rows()[0].traffic == "↑ 2.00 KB / ↓ 4.00 KB"
        }
        FeatureId::ConnectionsDetailsDrawer => state
            .diag
            .inspecting_connection_id
            .as_ref()
            .is_some_and(|id| {
                state
                    .diag
                    .connections
                    .as_ref()
                    .is_some_and(|s| s.connections.iter().any(|c| &c.id == id))
            }),
        FeatureId::SpeedtestDetailsModal => {
            state.diag.speedtest_detail_open && !state.diag.speedtest.results.is_empty()
        }
        FeatureId::ShellCommandPalette => {
            state.shell.command_palette_open
                && state.shell.command_query == "dns"
                && !state.filtered_command_indices().is_empty()
        }
        FeatureId::ProxiesCustomNodeModal => {
            state.runtime.custom_node_modal_open && state.runtime.custom_node_studio.draft.is_some()
        }
        FeatureId::ProxiesUriImportPreview => {
            state.runtime.custom_node_modal_open
                && state
                    .runtime
                    .custom_node_studio
                    .draft
                    .as_ref()
                    .is_some_and(|draft| draft.name == "Imported-Node")
                && state.runtime.custom_node_studio.uri_preview.is_some()
        }
        FeatureId::ProxiesGroupExpanded => {
            state
                .runtime
                .filtered_groups
                .first()
                .is_some_and(|(name, members)| {
                    let expanded = state
                        .runtime
                        .proxy_groups
                        .iter()
                        .find(|group| &group.name == name)
                        .is_some_and(|group| group.expanded);
                    !members.is_empty() && expanded
                })
        }
        FeatureId::ProxiesGroupReorder => {
            state.runtime.group_order_dialog == ProxyGroupOrderDialogState::Editing
                && state.runtime.group_order_editor.can_apply()
        }
        FeatureId::ProxiesProbeSettings => {
            matches!(
                state.runtime.probe_settings_dialog,
                ProbeSettingsDialogState::Editing
            ) && state.runtime.runtime_probe_settings_editor.can_apply()
                && state
                    .runtime
                    .runtime_probe_settings_editor
                    .draft()
                    .is_some_and(|draft| draft.timeout_ms == "60000")
        }
        FeatureId::OverviewCardReorder => {
            // The shared layout order really moved: the page assembles its
            // cards from that order.
            let order = &state.diag.overview_card_order;
            order.first() == Some(&OverviewCardKind::Traffic)
                && order != &OverviewCardKind::DEFAULT_ORDER.to_vec()
        }
        FeatureId::RulesSearchHighlight => {
            // The rule list really narrowed to the searched rule.
            state.editor.rules_model.filter_text() == RULE_SEARCH_QUERY
                && state.editor.rules_filtered_count() == 1
                && state.editor.rule_list.draft.len() > 1
        }
        FeatureId::ProfilesAggregatorWizard => state.profile.aggregator_modal_open,
        FeatureId::ProfilesDeleteConfirm => state
            .shell
            .confirmation
            .as_ref()
            .is_some_and(|action| matches!(action, ConfirmAction::DeleteProfile(_))),
        FeatureId::ProxiesSearchHighlight => {
            state.runtime.proxy_filter == "HK"
                && state
                    .runtime
                    .filtered_groups
                    .first()
                    .is_some_and(|(name, members)| {
                        name == "PROXIES"
                            && members.len() == 2
                            && members.iter().any(|member| member.as_str() == HK_PRIMARY)
                    })
                && state
                    .runtime
                    .proxy_name_runs
                    .get(HK_PRIMARY)
                    .is_some_and(|runs| runs.iter().any(|run| run.highlighted && run.text == "HK"))
        }
        FeatureId::ProxiesNodeDetailDrawer => state
            .runtime
            .outbound_inspecting
            .as_ref()
            .is_some_and(|id| {
                state.outbound_detail(id).is_some_and(|detail| {
                    matches!(detail.current_latency_ms, OutboundObservation::Observed(_))
                })
            }),
        FeatureId::SettingsTunPermission => {
            // The host reported the TUN service without the privilege the
            // preparation flow asks for, and the preparation request itself
            // was answered by the host.
            matches!(
                state.runtime.tun_service_status,
                Some(TunServiceStatus::MissingPrivilege)
            ) && state.shell.error_msg.is_some()
        }
        FeatureId::SettingsLanSecurity => {
            // The shared LAN draft carries the allow list the user typed.
            state.runtime.settings_text.lan_allowed_ips == LAN_ALLOWED_IPS
                && state
                    .runtime
                    .settings_model
                    .lan_security()
                    .allowed_ips
                    .len()
                    == 3
        }
        FeatureId::SettingsPortConflictRepair => state.runtime.port_conflicts.has_conflicts(),
        FeatureId::SettingsPermissionPanel => matches!(
            state.runtime.privileged_network.state,
            PrivilegedNetworkState::Unsupported { .. }
        ),
        FeatureId::CoreVersionRollbackConfirm => {
            // The owner published a rollback target and the destructive
            // rollback is armed behind the shared confirmation surface: the
            // command itself is never dispatched by the capture.
            state
                .runtime
                .core_versions
                .rollback
                .target
                .clone()
                .is_some_and(|target| !target.is_empty())
                && matches!(
                    state.shell.confirmation,
                    Some(ConfirmAction::RollbackCore(_))
                )
        }
        FeatureId::OverviewTopologyDrilldown => {
            // The drill-down landed on the shared navigation target and
            // carried the stage's own detail into the target page.
            let stage_detail = state
                .runtime
                .traffic_topology
                .node(TrafficTopologyStage::RuleSet)
                .map(|node| node.detail.clone())
                .unwrap_or_default();
            state.shell.current_route == Route::Rules
                && !stage_detail.is_empty()
                && state.editor.rules_model.filter_text() == stage_detail.to_ascii_lowercase()
        }
        FeatureId::SpeedtestProgressCancel => matches!(
            state.diag.speedtest.state,
            SpeedtestRunState::CancelRequested { .. }
        ),
        FeatureId::SpeedtestHistoryComparison => {
            state.diag.speedtest_detail_open && state.diag.speedtest.recent_history.len() == 2
        }
        FeatureId::ShellKeyboardFocus => {
            // The palette is open and the keyboard moved the focus ring to the
            // next catalogue row.
            state.shell.command_palette_open && state.shell.command_selected_index == 1
        }
        FeatureId::OverviewReconnectMask => {
            state.runtime.reconnect_mask.status == ReconnectMaskStatus::Reconnecting
                && state.runtime.reconnect_mask.attempt.is_some()
        }
        FeatureId::LifecycleRecoveryPanel => {
            // The lifecycle recovery panel shows the watchdog's own recovery
            // state; the diagnostic command phase stays untouched so the two
            // certified doctor states cannot fight over the same phase.
            matches!(
                state.diag.crash_watchdog.shared.state,
                CoreWatchdogState::Waiting { .. }
                    | CoreWatchdogState::Restarting { .. }
                    | CoreWatchdogState::Tripped { .. }
            ) && matches!(state.diag.doctor.action.phase, DoctorActionPhase::Idle)
        }
        FeatureId::ProfilesImportModal => {
            state.profile.import_url == IMPORT_URL && !state.profile.import_name.is_empty()
        }
        FeatureId::AppRoutingPolicyEditor => {
            // The per-application routing policy the page renders really
            // carries the rule the user picked.
            state.app_routing.custom_rules.get(DEMO_PROCESS) == Some(&AppRoutingRule::Direct)
                && !state.app_routing.processes.is_empty()
        }
        FeatureId::ProfilesSubscriptionError => {
            // The owner's typed refusal is on screen: the shell's durable
            // error banner carries the shared failure's message.
            state
                .shell
                .error_msg
                .as_deref()
                .is_some_and(|message| message.contains("capture subscription source is offline"))
        }
        FeatureId::ProfilesAggregatorPreview => {
            // The modal renders the shared aggregation report, not a local
            // summary: the report carries the sources it aggregated.
            state
                .profile
                .aggregator_report
                .as_ref()
                .is_some_and(|report| report.source_count > 0 && !report.yaml.is_empty())
        }
        FeatureId::ProfilesScriptSandbox => {
            // The console renders the shared engine's report.
            state
                .editor
                .script_sandbox
                .snapshot()
                .is_some_and(|snapshot| snapshot.execution_time_ms > 0 || snapshot.has_error())
        }
        FeatureId::ProfilesScriptExportReview => {
            // The review modal is open over the owner's frozen export bytes.
            state.editor.script_sandbox.is_export_visible()
                && state
                    .editor
                    .script_sandbox
                    .export()
                    .is_some_and(|export| !export.file_name.is_empty())
        }
        FeatureId::ProfilesDiffViewer => {
            // The modal renders the shared application's real diff.
            matches!(
                state.editor.snapshot_diff_modal,
                SnapshotDiffModalState::Loaded { .. }
            )
        }
        FeatureId::EditorYamlErrorNavigation => {
            // The shared codec's own diagnostic names the line the editor
            // navigated to.
            state.editor.syntax_error.is_some() && state.editor.syntax_error_line.is_some()
        }
        FeatureId::EditorLargeDocumentWindow => {
            // The editor really holds the capture store's large document and
            // renders it through the shared bounded window: the window does
            // not cover the document.
            let viewport = state.editor.profile_viewport;
            viewport.total_lines() > LARGE_PROFILE_RULE_COUNT
                && !viewport.covers_document()
                && state.editor.document_session.can_edit()
        }
        FeatureId::EditorSnippetInsertion => {
            // The shared codec spliced the catalogue body at the caret and the
            // document is still valid, so the insertion really happened.
            state
                .editor
                .editor_content
                .text()
                .contains("DOMAIN-SUFFIX,google.com,PROXIES")
                && state.editor.syntax_error.is_none()
                && state.editor.document_session.can_edit()
        }
        FeatureId::SyncConflictResolver => {
            // The merge session really carries the user's per-key decision.
            let Some(key) = demo_conflict_key(state) else {
                return false;
            };
            state
                .profile
                .sync_diff
                .as_ref()
                .is_some_and(|diff| diff.picks.get(&key) == Some(&true))
        }
        FeatureId::RulesProviderRefreshError => {
            // The owner's typed refusal reached the durable error banner: the
            // surface shows the provider source's own reason, not a local one.
            state
                .shell
                .error_msg
                .as_deref()
                .is_some_and(|message| message.contains(CAPTURE_REFRESH_FAILURE))
        }
    }
}

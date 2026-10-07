//! Deterministic interaction fixtures use the production update pipeline.
use crate::state::AppState;
use crate::types::app::ConfirmAction;
use crate::types::message::Message;
use infiltrator_application::connection_grouping_fixtures::{grouping_snapshot, grouping_surface};
use infiltrator_application::doctor_capture_fixtures::CHECK_ID;
use infiltrator_application::filter_capture_store::{FILTER_POLICY, FILTER_PROFILE};
use infiltrator_application::proxy_group_order_fixtures::{
    FIRST_GROUP, LAST_GROUP, MOVED_GROUP, observed_groups,
};
use infiltrator_application::proxy_inspection_fixtures::{
    INSPECTION_GROUP, INSPECTION_NODE, observed_proxy,
};
use infiltrator_application::proxy_search_fixtures::{
    SEARCH_GROUP, SEARCH_QUERY, observed_proxies,
};
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::rules_workspace::RulesTab;
use infiltrator_contract::speedtest::SpeedtestSnapshot;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_domain::connection_view::ConnectionGroupingMode;
use infiltrator_domain::proxy::{Proxy, ProxyGroup};

pub(super) fn parse(raw: &str) -> FeatureId {
    FeatureId::ALL
        .iter()
        .copied()
        .find(|feature| feature.spec().id == raw)
        .unwrap_or_else(|| panic!("unknown or unimplemented interaction capture: {raw}"))
}

pub(super) fn activate(state: &mut AppState, feature: FeatureId) {
    if matches!(
        feature,
        FeatureId::ConnectionsGrouping | FeatureId::ConnectionsSearchHighlight
    ) {
        let snapshot = grouping_surface(state.surface.latest().cloned().unwrap_or_else(|| {
            SurfaceSnapshot::unavailable(
                SurfaceKind::IcedDesktop,
                HostKind::Desktop,
                Failure::unsupported("unrelated capture services"),
            )
        }));
        let _ = state.update(Message::SurfaceSnapshotUpdated(Box::new(snapshot)));
        let _ = state.update(Message::ConnectionsReceived(grouping_snapshot()));
        let search = feature == FeatureId::ConnectionsSearchHighlight;
        let _ = state.update(Message::SetConnectionGroupingMode(if search {
            ConnectionGroupingMode::Flat
        } else {
            ConnectionGroupingMode::ByProcess
        }));
        let _ = state.update(Message::UpdateRuntimeConnectionFilter(
            if search { "API-0" } else { "CLIENT-0" }.into(),
        ));
        return;
    }
    if feature == FeatureId::ProxiesGroupReorder {
        state.runtime.proxies = observed_groups();
        state.recompute_filtered_groups();
        assert_eq!(
            state
                .update(Message::MoveProxyGroupUp(MOVED_GROUP.into()))
                .units(),
            0
        );
        return;
    }
    if feature == FeatureId::ProxiesProbeSettings {
        state.runtime.probe_options_editor.can_persist = true;
        assert_eq!(state.update(Message::OpenProxyProbeOptions).units(), 0);
        assert_eq!(
            state
                .update(Message::UpdateDelayTestUrl(
                    "https://probe.example.test/204".into()
                ))
                .units(),
            0
        );
        assert_eq!(
            state
                .update(Message::UpdateDelayTimeoutMs("32767".into()))
                .units(),
            0
        );
        return;
    }
    if feature == FeatureId::ProxiesSearchHighlight {
        state.runtime.proxies = observed_proxies();
        state.recompute_filtered_groups();
        assert_eq!(
            state
                .update(Message::FilterProxies(SEARCH_QUERY.into()))
                .units(),
            0
        );
        return;
    }
    if feature == FeatureId::ProxiesNodeDetailDrawer {
        state
            .runtime
            .proxies
            .insert(INSPECTION_NODE.into(), observed_proxy());
        state.runtime.proxies.insert(
            INSPECTION_GROUP.into(),
            Proxy::Selector(ProxyGroup {
                name: INSPECTION_GROUP.into(),
                all: vec![INSPECTION_NODE.into()],
                now: INSPECTION_NODE.into(),
                ..Default::default()
            }),
        );
        state.recompute_filtered_groups();
        assert_eq!(
            state
                .update(Message::InspectProxy(Some(INSPECTION_NODE.into())))
                .units(),
            0
        );
        return;
    }
    if feature == FeatureId::ProxiesGroupExpanded {
        let group = state
            .runtime
            .filtered_groups
            .first()
            .expect("proxy fixture")
            .0
            .clone();
        for _ in 0..2 {
            assert_eq!(
                state
                    .update(Message::ToggleProxyGroupExpanded(group.clone()))
                    .units(),
                0
            );
        }
        return;
    }
    let message = match feature {
        FeatureId::ConnectionsCloseAllConfirm => {
            Message::RequestConfirmation(ConfirmAction::CloseAllConnections)
        }
        FeatureId::ConnectionsDetailsDrawer => Message::InspectConnection(Some(
            state
                .diag
                .connections
                .as_ref()
                .and_then(|s| s.connections.first())
                .expect("connection inspection fixture")
                .id
                .clone(),
        )),
        FeatureId::SpeedtestDetailsModal => {
            state.diag.speedtest = SpeedtestSnapshot::demo_fixture();
            Message::OpenSpeedtestDetail
        }
        FeatureId::ShellCommandPalette => Message::OpenCommandPalette,
        FeatureId::DnsFakeIpFlushConfirm => Message::FlushFakeIpCache,
        FeatureId::ProxiesCustomNodeModal | FeatureId::ProxiesUriImportPreview => {
            Message::OpenCustomNodeModal
        }
        _ => return,
    };
    assert_eq!(
        state.update(message).units(),
        0,
        "opening an inspection cannot schedule effects"
    );
    if feature == FeatureId::ProxiesUriImportPreview {
        assert_eq!(state.update(Message::UpdateCustomNodeUriInput("vless://b831381d-6324-4d53-ad4f-8cda48b30811@node.example.com:443?security=tls#Imported-Node".into())).units(), 0);
        assert_eq!(state.update(Message::ParseAndImportCustomUri).units(), 0);
    }
    if feature == FeatureId::ShellCommandPalette {
        assert_eq!(
            state.update(Message::SetCommandQuery("dns".into())).units(),
            0
        );
    }
}

pub(super) fn ready(state: &AppState, feature: FeatureId) -> bool {
    if state.shell.capture_region_bounds.is_none() {
        return false;
    }
    match feature {
        FeatureId::RuntimeTelemetryObservation => {
            state.traffic_readout().upload == "0 B/s (stale)"
                && state
                    .traffic_readout()
                    .failure
                    .contains("isolated telemetry read denied")
                && !state.traffic_readout().current
        }
        FeatureId::LogsRedactedExport => {
            state.diag.log_export.open
                && state.diag.log_export.pending.is_none()
                && state
                    .diag
                    .log_export
                    .summary
                    .as_ref()
                    .is_some_and(|summary| summary.records == 3)
                && state.diag.log_export.receipt.is_none()
                && state
                    .diag
                    .log_export
                    .failure
                    .as_ref()
                    .is_some_and(|failure| failure.code == ErrorCode::Permission)
        }
        FeatureId::LogsScrollLock => {
            !state.diag.log_search.follow.should_follow()
                && state.diag.log_search.source_current()
                && state.diag.log_search.source_count() == 35
        }
        FeatureId::LogsSearchHighlight => {
            state.diag.log_search.query() == "api\\.example|timeout"
                && state.diag.log_search.source_current()
                && state.diag.log_search.source_count() == 3
                && state.diag.log_search.matched_count() == 2
        }
        FeatureId::ShellProxyModeControl | FeatureId::ShellProxyModeAuthentication => {
            let actions = &state.runtime.mode_actions;
            let failed = if feature == FeatureId::ShellProxyModeAuthentication {
                actions.needs_controller_settings()
                    && actions.retry_target().is_none()
                    && actions
                        .failure
                        .as_ref()
                        .is_some_and(|failure| failure.code == ErrorCode::Authentication)
            } else {
                actions.retry_target() == Some(ProxyMode::Global)
                    && actions
                        .failure
                        .as_ref()
                        .is_some_and(|failure| failure.code == ErrorCode::Network)
            };
            failed && actions.pending.is_none() && actions.observed.current == Some(ProxyMode::Rule)
        }
        FeatureId::ProfilesSnapshotRestoreConfirm => {
            let model = &state.editor.snapshot_restore;
            model.visible && !model.busy() && model.failure.is_none() && model.review.is_some()
        }
        FeatureId::ProfilesFilterEditor => {
            let editor = &state.editor.filter_editor;
            editor.source_profile() == Some(FILTER_PROFILE)
                && editor.draft.include == "HK"
                && editor.draft.advanced_policy.as_deref() == Some(FILTER_POLICY)
                && editor.pending.is_none()
                && editor
                    .failure
                    .as_ref()
                    .is_some_and(|failure| failure.code == ErrorCode::Permission)
        }
        FeatureId::RulesStatisticsInspector => {
            let model = &state.editor.rule_hit_audit;
            model.current()
                && model.confirmation.is_some()
                && model.can_confirm_cleanup(&state.editor.rule_list)
                && !state.editor.rule_list.dirty()
        }
        FeatureId::RulesListEditor => {
            let model = &state.editor.rule_list;
            state.editor.rules_tab == RulesTab::List
                && !model.draft.is_empty()
                && model.dirty()
                && model.can_save()
                && model.pending.is_none()
                && model
                    .failure
                    .as_ref()
                    .is_some_and(|failure| failure.code == ErrorCode::Permission)
                && model.draft.first().is_some_and(|rule| {
                    !rule.enabled && rule.rule == "SRC-IP-CIDR,10.0.0.0/8,DIRECT"
                })
        }
        FeatureId::RulesOverrideEditor => {
            state.editor.rule_trace.override_pending.is_none()
                && matches!(&state.shell.confirmation, Some(ConfirmAction::TracerOverride(request))
                if request.expected_rule == "DOMAIN-SUFFIX,google.com,PROXY" && request.new_target == "DIRECT")
                && state.editor.rule_trace.confirmation.is_some()
        }
        FeatureId::RulesTracerDrawer => {
            let model = &state.editor.rule_trace;
            state.editor.rules_tab == RulesTab::Tracer
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
            model.open
                && model.pending.is_none()
                && model.failure.is_none()
                && model.requested.is_some()
                && model.requested == model.snapshot.report_id
                && model
                    .snapshot
                    .report
                    .as_ref()
                    .is_some_and(|report| report.response.answers.len() == 2)
        }
        FeatureId::DnsFakeIpFlushConfirm => {
            let model = &state.diag.dns_cache_actions;
            model.open && !model.confirmed && model.pending.is_none()
        }
        FeatureId::DnsHostsEditor => {
            let editor = &state.editor.dns_hosts_editor;
            editor.open
                && editor.pending.is_none()
                && editor.can_apply()
                && editor
                    .failure
                    .as_ref()
                    .is_some_and(|failure| failure.code == ErrorCode::Permission)
                && editor.importing_legacy
                && editor.rows.len() == 3
                && editor.applied.as_ref().is_some_and(|profile| {
                    profile.entries.len() == 2 && profile.legacy_entries.len() == 1
                })
        }
        FeatureId::DnsLeakAlert => {
            state.diag.dns_leak_action.pending.is_none()
                && state.diag.dns_leak_action.failure.is_none()
                && state.editor.dns_leak.conclusion().is_divergent()
                && state.editor.dns_leak.observations.len() == 2
        }
        FeatureId::DoctorFailureRecovery => {
            state.diag.doctor.action.can_retry()
                && state
                    .diag
                    .doctor
                    .action
                    .failure
                    .as_ref()
                    .is_some_and(|failure| failure.code == ErrorCode::Permission)
                && state.surface.latest().is_some_and(|snapshot| {
                    snapshot
                        .pages
                        .doctor
                        .data
                        .as_ref()
                        .is_some_and(|data| data.checks.iter().any(|check| check.id == CHECK_ID))
                })
        }
        FeatureId::SettingsLanguageChoice => {
            state.shell.lang == "en-US"
                && state.shell.language_choice.pending.is_none()
                && state.shell.language_choice.failure.is_none()
                && state.shell.language_user_selected
        }
        FeatureId::ConnectionsCloseAllConfirm => {
            state.shell.confirmation == Some(ConfirmAction::CloseAllConnections)
        }
        FeatureId::ConnectionsSearchHighlight => {
            state.diag.connection_groups.mode().is_flat()
                && state.runtime.runtime_connection_filter == "API-0"
                && state.diag.connection_groups.matched_count() == 2
                && state.diag.connection_groups.source_count() == 9
                && state
                    .diag
                    .connection_groups
                    .search_row("group-0")
                    .is_some_and(|row| {
                        row.endpoint
                            .iter()
                            .any(|run| run.highlighted && run.text == "api-0")
                    })
        }
        FeatureId::ConnectionsGrouping => {
            let groups = &state.diag.connection_groups;
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
            state.diag.speedtest_detail_open && !state.diag.speedtest.node_results.is_empty()
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
                .is_some_and(|(name, group)| {
                    !group.is_empty() && state.runtime.proxy_ui_preferences.is_group_expanded(name)
                })
        }
        FeatureId::ProxiesGroupReorder => {
            state.runtime.group_order_open
                && state.runtime.group_order_editor.can_apply()
                && state.runtime.group_order_editor.draft == [MOVED_GROUP, FIRST_GROUP, LAST_GROUP]
        }
        FeatureId::ProxiesProbeSettings => {
            state.runtime.probe_options_open
                && state.runtime.probe_options_editor.can_apply()
                && state.runtime.probe_options_editor.draft.timeout_ms == "32767"
        }
        FeatureId::ProxiesSearchHighlight => {
            state.runtime.proxy_filter == SEARCH_QUERY
                && state
                    .runtime
                    .proxy_groups
                    .iter()
                    .any(|group| group.name == SEARCH_GROUP && group.proxies.len() == 1)
                && state
                    .runtime
                    .proxy_name_runs
                    .get(INSPECTION_NODE)
                    .is_some_and(|runs| {
                        runs.iter()
                            .any(|run| run.highlighted && run.text == SEARCH_QUERY)
                    })
        }
        FeatureId::ProxiesNodeDetailDrawer => {
            state.runtime.inspecting_proxy.as_deref() == Some(INSPECTION_NODE)
                && state
                    .proxy_inspection(INSPECTION_NODE)
                    .is_some_and(|detail| detail.history.len() == 3 && detail.delay_ms == Some(42))
        }
        _ => true,
    }
}

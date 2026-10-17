//! Deterministic interaction fixtures use the production update pipeline.
use crate::state::AppState;
use crate::state::runtime::{ProbeSettingsDialogState, ProxyGroupOrderDialogState};
use crate::types::app::ConfirmAction;
use crate::types::message::Message;
use infiltrator_composition::capture_fixtures::grouping_surface;
use infiltrator_contract::fixture_identities::HK_PRIMARY;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::rules_workspace::RulesTab;
use infiltrator_contract::runtime_settings::{RuntimeProbeSettings, RuntimeProbeSettingsState};
use infiltrator_contract::speedtest::SpeedtestSnapshot;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_domain::connection_view::ConnectionGroupingMode;
use infiltrator_presentation::doctor_actions::DoctorActionPhase;
use infiltrator_presentation::language_choice::LanguageChoicePhase;
use infiltrator_presentation::traffic_readout::TrafficFreshness;
use infiltrator_test_support::connection_grouping_fixtures::grouping_snapshot;
use infiltrator_test_support::doctor_capture_fixtures::CHECK_ID;
use infiltrator_test_support::filter_capture_store::{FILTER_POLICY, FILTER_PROFILE};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::outbound::details::OutboundObservation;
use lurker_core_contract::routing_intent::RoutingIntent;
use lurker_ui_presentation::dns_leak_actions::DnsLeakActionPhase;

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
        let group = state
            .runtime
            .proxy_groups
            .get(1)
            .expect("typed outbound fixture has reorderable groups")
            .id
            .clone();
        assert_eq!(
            state
                .update(Message::MoveProxyGroupUp(
                    group.expect("typed group identity")
                ))
                .units(),
            0
        );
        return;
    }
    if feature == FeatureId::ProxiesProbeSettings {
        state.runtime.runtime_probe_settings_editor.observe(
            &RuntimeProbeSettingsState::Available {
                settings: RuntimeProbeSettings::default(),
            },
        );
        assert_eq!(state.update(Message::OpenRuntimeProbeSettings).units(), 0);
        assert_eq!(
            state
                .update(Message::UpdateRuntimeProbeUrlDraft(
                    "https://probe.example.test/204".into()
                ))
                .units(),
            0
        );
        assert_eq!(
            state
                .update(Message::UpdateRuntimeProbeTimeoutDraft("60000".into()))
                .units(),
            0
        );
        return;
    }
    if feature == FeatureId::ProxiesSearchHighlight {
        assert_eq!(state.update(Message::FilterProxies("HK".into())).units(), 0);
        return;
    }
    if feature == FeatureId::ProxiesNodeDetailDrawer {
        let outbound_id = state
            .runtime
            .proxy_groups
            .iter()
            .flat_map(|group| &group.proxies)
            .find(|member| member.id.is_some())
            .and_then(|member| member.id.clone())
            .expect("typed outbound fixture has detail rows");
        assert_eq!(
            state
                .update(Message::InspectOutbound(Some(outbound_id)))
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
        let group_id = state
            .runtime
            .proxy_groups
            .iter()
            .find(|candidate| candidate.name == group)
            .and_then(|candidate| candidate.id.clone())
            .expect("typed demo collection ID");
        for _ in 0..2 {
            assert_eq!(
                state
                    .update(Message::ToggleOutboundGroupExpanded(group_id.clone()))
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

pub(crate) fn ready(state: &AppState, feature: FeatureId) -> bool {
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
                && matches!(state.traffic_readout().freshness, TrafficFreshness::Failed)
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
        FeatureId::ProfilesSnapshotRestoreConfirm => {
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
            state.editor.rules_tab == RulesTab::List
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
        FeatureId::ProxiesSearchHighlight => {
            state.runtime.proxy_filter == "HK"
                && state
                    .runtime
                    .proxy_groups
                    .iter()
                    .any(|group| group.name == "PROXIES" && group.proxies.len() == 2)
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
        _ => true,
    }
}

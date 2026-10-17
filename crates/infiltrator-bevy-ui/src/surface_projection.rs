use super::surface_projection_defaults::{
    empty_app_routing, empty_connections, empty_dns, empty_doctor, empty_logs, empty_profiles,
    empty_rules, empty_sync,
};
use super::surface_settings::empty_settings;
use super::*;
use crate::pages::app_routing::AppItem;
use lurker_core_contract::outbound::details::OutboundDetail;
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::outbound::preferences::OutboundLayout;
use std::cmp::Ordering;
use std::collections::HashMap;

use crate::pages::doctor::{DoctorCheckItem, DoctorProjection};
use crate::pages::logs::LogEntry;
use crate::pages::logs_ring::bounded_tail;
use crate::pages::profiles::ProfileItem;
use crate::pages::proxies::{ProxyGroup, ProxyNode};
use crate::pages::rules::{RuleItem, RuleProviderItem};
use crate::pages::sync::{ConflictingKey, SnapshotItem, SyncConflictInfo};
use crate::projection::{OverviewOrigin, OverviewProjection, OverviewState};
use infiltrator_contract::connection::ConnectionStreamPhase;
use infiltrator_contract::logs::LogLevel;
use infiltrator_contract::proxies::ProxyGroupClassification;
use infiltrator_contract::resources::{CoreResourceSnapshot, ResourceAvailability};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_presentation::routing_intent_projection;
use infiltrator_presentation::system_toggle_projection;
use lurker_core_contract::outbound::{
    details::{
        OutboundCollectionKind, OutboundDetailsState, OutboundFeature, OutboundHealthObservation,
        OutboundObservation, OutboundProtocolObservation, OutboundSnapshot,
    },
    inventory::{OutboundInventoryState, OutboundSelectionState, SelectionAvailability},
    preferences::{OutboundHealthFilter, OutboundSortOrder},
};
use std::time;

pub(super) fn overview_projection(
    snapshot: &surface_snapshot::SurfaceSnapshot,
) -> OverviewProjection {
    let page = snapshot.pages.overview.data.as_ref();
    let core = &snapshot.core;
    let state = match core.lifecycle {
        CoreLifecycle::Running | CoreLifecycle::Ready => OverviewState::Running,
        CoreLifecycle::Stopped => OverviewState::Stopped,
        CoreLifecycle::Starting | CoreLifecycle::Stopping | CoreLifecycle::Failed => {
            OverviewState::Unavailable
        }
    };
    OverviewProjection {
        lifecycle: core.lifecycle.clone(),
        readout: snapshot.shell_readout.clone(),
        state,
        upload_bps: page
            .map(|value| value.upload_bps)
            .unwrap_or(core.upload_bps),
        download_bps: page
            .map(|value| value.download_bps)
            .unwrap_or(core.download_bps),
        active_connections: page
            .map(|value| value.active_connections)
            .unwrap_or(core.active_connections),
        memory_bytes: match &snapshot.resources {
            CoreResourceSnapshot::NotObserved => ResourceAvailability::NotObserved,
            CoreResourceSnapshot::Observed { memory, .. } => memory.clone(),
        },
        sampled_at: core
            .sampled_at_epoch_ms
            .and_then(|value| u64::try_from(value).ok())
            .map(time::Duration::from_millis)
            .unwrap_or_default(),
        failure: snapshot
            .failure
            .as_ref()
            .or(core.failure.as_ref())
            .map(|failure| failure.message.clone()),
        origin: {
            #[cfg(any(feature = "capture-tools", test))]
            {
                match snapshot.origin {
                    surface_snapshot::SurfaceOrigin::Demo
                        if snapshot.surface == SurfaceKind::BevyDesktop =>
                    {
                        OverviewOrigin::Demo
                    }
                    surface_snapshot::SurfaceOrigin::Demo
                    | surface_snapshot::SurfaceOrigin::Live => OverviewOrigin::LiveCore,
                }
            }
            #[cfg(not(any(feature = "capture-tools", test)))]
            {
                OverviewOrigin::LiveCore
            }
        },
        core_version: page
            .and_then(|value| value.core_version.clone())
            .or_else(|| core.core_version.clone()),
        traffic_waveform: snapshot.traffic_waveform.clone(),
        traffic_scale: snapshot.traffic_scale.clone(),
        traffic_topology: snapshot.traffic_topology.clone(),
        active_exit: snapshot.active_exit.clone(),
        public_ip: snapshot.public_ip.clone(),
        layout: snapshot.overview_layout.clone(),
        reconnect_mask: snapshot.reconnect_mask.clone(),
        viewport: snapshot.viewport.clone(),
        subscription_quota: snapshot.subscription_quota.clone(),
        system_toggles: system_toggle_projection::from_surface(snapshot),
        routing_intent: routing_intent_projection::from_surface(snapshot),
        speedtest: snapshot.speedtest.clone(),
        cpu_percent: match &snapshot.resources {
            CoreResourceSnapshot::NotObserved => ResourceAvailability::NotObserved,
            CoreResourceSnapshot::Observed { cpu_percent, .. } => cpu_percent.clone(),
        },
        total_traffic_bytes: snapshot
            .pages
            .connections
            .data
            .as_ref()
            .map(|c| c.total_upload_bytes + c.total_download_bytes),
    }
}

pub(super) fn proxies_projection(
    snapshot: &surface_snapshot::SurfaceSnapshot,
) -> ProxiesProjection {
    use crate::pages::proxies_filter::{matches_proxy_filter, name_runs};
    use infiltrator_contract::surface_snapshot::PageStatus;

    let scope = snapshot.core.scope();
    let inventory = matches!(
        snapshot.outbound_inventory.status,
        PageStatus::Ready | PageStatus::Empty
    )
    .then_some(snapshot.outbound_inventory.data.as_ref())
    .flatten()
    .filter(|inventory| scope.as_ref() == Some(inventory.scope()));
    let details = matches!(
        snapshot.outbound_details.status,
        PageStatus::Ready | PageStatus::Empty
    )
    .then_some(snapshot.outbound_details.data.as_ref())
    .flatten()
    .filter(|details| scope.as_ref() == Some(details.scope()));
    let preferences = matches!(
        snapshot.outbound_ui_preferences.status,
        PageStatus::Ready | PageStatus::Empty
    )
    .then_some(snapshot.outbound_ui_preferences.data.as_ref())
    .flatten()
    .filter(|preferences| scope.as_ref() == Some(preferences.scope()));

    let mut groups = Vec::new();
    if let Some(inventory) = inventory
        && let OutboundInventoryState::Ready {
            entries,
            collections,
        } = inventory.inventory()
    {
        let detail_snapshot = details.filter(|details| details.scope() == inventory.scope());
        let detail_rows = detail_snapshot.and_then(|details| match details.state() {
            OutboundDetailsState::Ready { details } => Some(details.as_slice()),
            OutboundDetailsState::Unknown
            | OutboundDetailsState::Empty
            | OutboundDetailsState::Unsupported
            | OutboundDetailsState::Failed { .. } => None,
        });
        for collection in collections {
            let selected = match &collection.selection {
                OutboundSelectionState::Ready { selected } => Some(selected),
                OutboundSelectionState::Unknown
                | OutboundSelectionState::Empty
                | OutboundSelectionState::Unsupported
                | OutboundSelectionState::Failed { .. } => None,
            };
            let collection_detail =
                detail_rows.and_then(|rows| rows.iter().find(|detail| detail.id == collection.id));
            let (group_type, classification) =
                collection_kind(collection_detail.map(|detail| &detail.collection_kind));
            let mut proxies: Vec<_> = collection
                .member_ids
                .iter()
                .filter_map(|id| {
                    let entry = entries.iter().find(|entry| entry.id == *id)?;
                    let detail =
                        detail_rows.and_then(|rows| rows.iter().find(|detail| detail.id == *id));
                    let node_type = detail.map_or_else(String::new, protocol_name);
                    let delay_ms = detail.and_then(latency_ms);
                    let health =
                        detail.map_or(OutboundHealthObservation::Unknown, |detail| detail.health);
                    let favorite = preferences
                        .is_some_and(|preferences| preferences.favorite_outbound_ids.contains(id));
                    let features = detail.map_or_else(Vec::new, feature_names);
                    let node = ProxyNode {
                        outbound_id: Some(id.clone()),
                        name: entry
                            .display_name
                            .clone()
                            .unwrap_or_else(|| id.value().to_owned()),
                        node_type,
                        delay_ms,
                        health,
                        selected: selected.is_some_and(|selected| selected == id),
                        favorite,
                        features,
                    };
                    if preferences.is_some_and(|preferences| {
                        preferences.health_filter == OutboundHealthFilter::HealthyOnly
                            && node.health != OutboundHealthObservation::Healthy
                    }) {
                        return None;
                    }
                    Some(node)
                })
                .collect();

            if let Some(preferences) = preferences {
                let group_matches = collection
                    .id
                    .value()
                    .to_ascii_lowercase()
                    .contains(&preferences.search_query.trim().to_ascii_lowercase());
                if !preferences.search_query.trim().is_empty() && !group_matches {
                    proxies.retain(|node| matches_proxy_filter(node, &preferences.search_query));
                }
                sort_outbound_nodes(&mut proxies, preferences.sort_order);
            }
            let expanded = preferences.is_none_or(|preferences| {
                !preferences.collapsed_group_ids.contains(&collection.id)
            });
            groups.push(ProxyGroup {
                collection_id: Some(collection.id.clone()),
                name: collection.id.value().to_owned(),
                group_type,
                classification,
                current: selected
                    .map(|id| outbound_display_name(id, entries))
                    .unwrap_or_default(),
                outbound_selection_available: collection.availability
                    == SelectionAvailability::Selectable,
                expanded,
                proxies,
            });
        }
    }

    if let Some(preferences) = preferences {
        let positions: HashMap<_, _> = preferences
            .group_order
            .iter()
            .enumerate()
            .map(|(index, id)| (id, index))
            .collect();
        groups.sort_by_key(|group| {
            group
                .collection_id
                .as_ref()
                .and_then(|id| positions.get(id).copied())
                .unwrap_or(usize::MAX)
        });
    }

    let search_query =
        preferences.map_or_else(String::new, |preferences| preferences.search_query.clone());
    let name_runs = groups
        .iter()
        .flat_map(|group| group.proxies.iter())
        .map(|node| (node.name.clone(), name_runs(node, &search_query)))
        .collect();
    ProxiesProjection {
        name_runs,
        search_query,
        groups,
        testing: false,
        filter_alive: preferences.is_some_and(|preferences| {
            preferences.health_filter == OutboundHealthFilter::HealthyOnly
        }),
        compact_view: preferences
            .is_some_and(|preferences| preferences.layout == OutboundLayout::CompactList),
        active_exit: snapshot.active_exit.name.clone().unwrap_or_default(),
        custom_node: matches!(
            snapshot.protocol_studio.status,
            PageStatus::Ready | PageStatus::Empty
        )
        .then_some(snapshot.protocol_studio.data.as_ref())
        .flatten()
        .cloned()
        .unwrap_or_default(),
    }
}

fn collection_kind(
    observation: Option<&OutboundObservation<OutboundCollectionKind>>,
) -> (String, ProxyGroupClassification) {
    match observation {
        Some(OutboundObservation::Observed(OutboundCollectionKind::ManualSelection)) => {
            ("Selector".to_owned(), ProxyGroupClassification::Selector)
        }
        Some(OutboundObservation::Observed(OutboundCollectionKind::AutomaticSelection)) => {
            ("Automatic".to_owned(), ProxyGroupClassification::UrlTest)
        }
        Some(OutboundObservation::Observed(OutboundCollectionKind::Failover)) => {
            ("Failover".to_owned(), ProxyGroupClassification::Fallback)
        }
        Some(OutboundObservation::Observed(OutboundCollectionKind::LoadBalancing)) => (
            "LoadBalance".to_owned(),
            ProxyGroupClassification::LoadBalance,
        ),
        Some(OutboundObservation::Observed(OutboundCollectionKind::Relay)) => {
            ("Relay".to_owned(), ProxyGroupClassification::Relay)
        }
        Some(OutboundObservation::Unknown | OutboundObservation::Unsupported) | None => {
            ("Collection".to_owned(), ProxyGroupClassification::Selector)
        }
    }
}

fn protocol_name(detail: &OutboundDetail) -> String {
    match &detail.protocol {
        OutboundProtocolObservation::Reported(protocol) => detail
            .protocol_display_name
            .clone()
            .unwrap_or_else(|| protocol.value().to_owned()),
        OutboundProtocolObservation::Unknown | OutboundProtocolObservation::Unsupported => {
            String::new()
        }
    }
}

fn latency_ms(detail: &OutboundDetail) -> Option<u32> {
    match detail.current_latency_ms {
        OutboundObservation::Observed(delay) => Some(delay),
        OutboundObservation::Unknown | OutboundObservation::Unsupported => None,
    }
}

fn feature_names(detail: &OutboundDetail) -> Vec<String> {
    match &detail.features {
        OutboundObservation::Observed(features) => features
            .iter()
            .map(|feature| match feature {
                OutboundFeature::Udp => "UDP".to_owned(),
            })
            .collect(),
        OutboundObservation::Unknown | OutboundObservation::Unsupported => Vec::new(),
    }
}

fn sort_outbound_nodes(nodes: &mut [ProxyNode], order: OutboundSortOrder) {
    nodes.sort_by(|left, right| {
        let favorite_order = || right.favorite.cmp(&left.favorite);
        match order {
            OutboundSortOrder::LatencyAscending => favorite_order()
                .then_with(|| delay_ascending(left.delay_ms, right.delay_ms))
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase())),
            OutboundSortOrder::LatencyDescending => favorite_order()
                .then_with(|| delay_descending(left.delay_ms, right.delay_ms))
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase())),
            OutboundSortOrder::NameAscending => {
                left.name.to_lowercase().cmp(&right.name.to_lowercase())
            }
            OutboundSortOrder::NameDescending => {
                right.name.to_lowercase().cmp(&left.name.to_lowercase())
            }
        }
    });
}

fn delay_ascending(left: Option<u32>, right: Option<u32>) -> Ordering {
    match (
        left.filter(|delay| *delay > 0),
        right.filter(|delay| *delay > 0),
    ) {
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn delay_descending(left: Option<u32>, right: Option<u32>) -> Ordering {
    delay_ascending(right, left)
}

fn outbound_display_name(id: &OutboundId, entries: &[OutboundSnapshot]) -> String {
    entries
        .iter()
        .find(|entry| entry.id == *id)
        .and_then(|entry| entry.display_name.as_deref())
        .unwrap_or_else(|| id.value())
        .to_owned()
}

pub(super) fn profiles_projection(
    snapshot: &surface_snapshot::SurfaceSnapshot,
) -> ProfilesProjection {
    let mut projection = snapshot
        .pages
        .profiles
        .data
        .as_ref()
        .map(|value| ProfilesProjection {
            profiles: value
                .profiles
                .iter()
                .map(|profile| ProfileItem {
                    id: profile.id.clone(),
                    name: profile.name.clone(),
                    url: profile.url.clone(),
                    updated_at: profile.updated_at.clone(),
                    upload_bytes: profile.upload_bytes,
                    download_bytes: profile.download_bytes,
                    total_bytes: profile.total_bytes,
                    is_active: profile.is_active,
                    user_agent: profile.user_agent.clone(),
                    insecure_skip_verify: profile.insecure_skip_verify,
                    etag: profile.etag.clone(),
                    last_modified: profile.last_modified.clone(),
                    has_backup: profile.has_backup,
                    cron_expression: profile.cron_expression.clone(),
                    auto_update_enabled: profile.auto_update_enabled,
                    update_interval_hours: profile.update_interval_hours,
                    next_update: profile.next_update.clone(),
                    auto_reload_core: profile.auto_reload_core,
                    filter: profile.filter.clone(),
                    filter_source: profile.filter_source.clone(),
                    write_protection: profile.write_protection,
                })
                .collect(),
            auto_update_interval_hours: value.auto_update_interval_hours,
            updating: value.updating,
            aggregation: value.aggregation.clone(),
            aggregation_templates: value.aggregation_templates.clone(),
            aggregation_templates_available: value.aggregation_templates_available,
            yaml_ast_diff: snapshot.yaml_ast_diff.clone(),
            // DUAL-09-06/07/11/14: the shared history, apply transaction and
            // loaded editor document travel with the page read model.
            snapshot_history: value.snapshot_history.clone(),
            apply_transaction: value.apply_transaction.clone(),
            profile_document: snapshot.profile_editor.document.clone(),
            profile_options: snapshot.profile_editor.options.clone(),
            editor_read: snapshot.profile_editor.read.clone(),
            script_sandbox: snapshot.script_sandbox.clone(),
            script_export: snapshot.script_export.clone(),
        })
        .unwrap_or_else(empty_profiles);
    projection.profile_document = snapshot.profile_editor.document.clone();
    projection.profile_options = snapshot.profile_editor.options.clone();
    projection.editor_read = snapshot.profile_editor.read.clone();
    projection
}

pub(super) fn rules_projection(snapshot: &surface_snapshot::SurfaceSnapshot) -> RulesProjection {
    snapshot
        .pages
        .rules
        .data
        .as_ref()
        .map(|value| RulesProjection {
            total_rules: value.total_rules,
            default_action: value.default_action.clone(),
            tracer: value.tracer.clone(),
            hit_audit: value.hit_audit.clone(),
            mrs_acceleration: value.mrs_acceleration.clone(),
            // DUAL-11-08: the publish cap and the omitted count are shared
            // facts, so the page can render the truncation honestly.
            truncated_rule_count: value.is_truncated().then(|| value.omitted_rule_count()),
            rule_publish_limit: value.rule_publish_limit,
            provider_cache: value.provider_cache.clone(),
            etag_support: value.etag_support,
            json_documents: value.json_documents.clone(),
            providers: value
                .providers
                .iter()
                .map(|provider| RuleProviderItem {
                    name: provider.name.clone(),
                    provider_type: provider.provider_type.clone(),
                    rule_count: provider.rule_count,
                    behavior: provider.behavior.clone(),
                    vehicle_type: provider.vehicle_type.clone(),
                    updated_at: provider.updated_at.clone(),
                    source_url: provider.source_url.clone(),
                    refresh_interval_secs: provider.refresh_interval_secs,
                    cache_fingerprint: provider.cache_fingerprint.clone(),
                })
                .collect(),
            rules: value
                .rules
                .iter()
                .map(|rule| RuleItem {
                    edit_id: rule.edit_id,
                    raw: rule.raw.clone(),
                    source_ip: rule.source_ip,
                    no_resolve: rule.no_resolve,
                    failure: rule.failure.clone(),
                    id: rule.id,
                    rule_type: rule.rule_type.clone(),
                    payload: rule.payload.clone(),
                    proxy: rule.proxy.clone(),
                    hit_count: rule.hit_count,
                    is_enabled: rule.is_enabled,
                    last_hit_secs: rule.last_hit_secs,
                    is_shadowed: rule.is_shadowed,
                    shadow_reason: rule.shadow_reason.clone(),
                })
                .collect(),
        })
        .unwrap_or_else(empty_rules)
}

pub(super) fn connections_projection(
    snapshot: &surface_snapshot::SurfaceSnapshot,
) -> ConnectionsProjection {
    let stream_phase = ConnectionStreamPhase::from_page_status(&snapshot.pages.connections.status);
    snapshot
        .pages
        .connections
        .data
        .as_ref()
        .map(|value| ConnectionsProjection {
            total_connections: value.total_connections,
            total_upload_bytes: value.total_upload_bytes,
            total_download_bytes: value.total_download_bytes,
            stream_phase,
            connections: value.connections.clone(),
        })
        .unwrap_or_else(|| empty_connections_with_phase(stream_phase))
}

fn empty_connections_with_phase(stream_phase: ConnectionStreamPhase) -> ConnectionsProjection {
    let mut projection = empty_connections();
    projection.stream_phase = stream_phase;
    projection
}

pub(super) fn logs_projection(snapshot: &surface_snapshot::SurfaceSnapshot) -> LogsProjection {
    snapshot
        .pages
        .logs
        .data
        .as_ref()
        .map(|value| LogsProjection {
            status: snapshot.pages.logs.status.clone(),
            generation: snapshot.core.generation,
            session_token: snapshot.core.session_token,
            total_entries: value.total_entries,
            active_level: value.active_level.as_deref().map(LogLevel::from_identifier),
            entries: bounded_tail(
                value
                    .entries
                    .iter()
                    .map(|entry| LogEntry {
                        id: entry.id,
                        timestamp: entry.timestamp.clone(),
                        level: LogLevel::from_identifier(&entry.level),
                        tag: entry.tag.clone(),
                        message: entry.message.clone(),
                    })
                    .collect(),
            ),
        })
        .unwrap_or_else(|| {
            let mut projection = empty_logs();
            projection.status = snapshot.pages.logs.status.clone();
            projection.generation = snapshot.core.generation;
            projection.session_token = snapshot.core.session_token;
            projection
        })
}

pub(super) fn dns_projection(snapshot: &surface_snapshot::SurfaceSnapshot) -> DnsProjection {
    snapshot
        .pages
        .dns
        .data
        .as_ref()
        .map(|dns| {
            DnsProjection::from_snapshot(
                dns,
                &snapshot.dns_leak,
                snapshot.dns_hosts.data.as_ref(),
                &snapshot.dns_cache.report,
            )
        })
        .unwrap_or_else(|| {
            let mut projection = empty_dns();
            projection.leak = snapshot.dns_leak.clone();
            projection.cache_flush = snapshot.dns_cache.report.clone();
            projection
        })
}

pub(super) fn doctor_projection(snapshot: &surface_snapshot::SurfaceSnapshot) -> DoctorProjection {
    let watchdog = snapshot.core.watchdog.clone();
    snapshot
        .pages
        .doctor
        .data
        .as_ref()
        .map(|value| DoctorProjection {
            report_finished_at: value.report_finished_at,
            overall_healthy: value.overall_healthy,
            last_run: value.last_run.clone(),
            watchdog: watchdog.clone(),
            checks: value
                .checks
                .iter()
                .map(|check| DoctorCheckItem {
                    kind: check.kind,
                    detail_copy_key: check.detail_copy_key.clone(),
                    id: check.id.clone(),
                    name: check.name.clone(),
                    category: check.category.clone(),
                    state: check.state,
                    detail: check.detail.clone(),
                    hint: check.hint.clone(),
                    fix_available: check.fix_available,
                })
                .collect(),
        })
        .unwrap_or_else(|| {
            let mut projection = empty_doctor();
            projection.watchdog = watchdog;
            projection
        })
}

pub(super) fn app_routing_projection(
    snapshot: &surface_snapshot::SurfaceSnapshot,
) -> AppRoutingProjection {
    snapshot
        .pages
        .app_routing
        .data
        .as_ref()
        .map(|value| AppRoutingProjection {
            mode: value.mode.into(),
            include_system: value.include_system,
            uwp_loopback: value.uwp_loopback.clone(),
            apps: value
                .apps
                .iter()
                .map(|app| AppItem {
                    id: app.id.clone(),
                    name: app.name.clone(),
                    process_name: app.process_name.clone(),
                    rule: app.rule.into(),
                    is_system: app.is_system,
                })
                .collect(),
        })
        .unwrap_or_else(empty_app_routing)
}

pub(super) fn sync_projection(snapshot: &surface_snapshot::SurfaceSnapshot) -> SyncProjection {
    snapshot
        .pages
        .sync
        .data
        .as_ref()
        .map(|value| SyncProjection {
            status: value.status,
            history_status: value.history_status.clone(),
            server_url: value.server_url.clone(),
            username: value.username.clone(),
            last_sync: value.last_sync.clone(),
            auto_sync: value.auto_sync,
            conflict: value.conflict.as_ref().map(|conflict| SyncConflictInfo {
                remote_device: conflict.remote_device.clone(),
                conflict_time: conflict.conflict_time.clone(),
                conflicting_keys: conflict
                    .conflicting_keys
                    .iter()
                    .map(|(key, local, remote)| ConflictingKey {
                        key: key.clone(),
                        local_value: local.clone(),
                        remote_value: remote.clone(),
                    })
                    .collect(),
            }),
            snapshots: value
                .snapshots
                .iter()
                .map(|item| SnapshotItem {
                    profile: item.profile.clone(),
                    id: item.id.clone(),
                    timestamp: item.timestamp.clone(),
                    device: item.device.clone(),
                    size_bytes: item.size_bytes,
                })
                .collect(),
        })
        .unwrap_or_else(|| {
            let mut projection = empty_sync();
            projection.history_status = snapshot.pages.sync.status.clone();
            projection
        })
}

pub(super) fn settings_projection(
    snapshot: &surface_snapshot::SurfaceSnapshot,
) -> SettingsProjection {
    let mut projection = snapshot
        .pages
        .settings
        .data
        .clone()
        .map(|value| SettingsProjection {
            close_to_tray: value.close_to_tray,
            notifications_enabled: value.notifications_enabled,
            preference_status: snapshot.pages.settings.status.clone(),
            runtime_status: snapshot.runtime_control.status.clone(),
            theme: value.theme.clone(),
            autostart: value.autostart,
            system_proxy: value.system_proxy,
            system_proxy_snapshot: snapshot.system_proxy.clone(),
            system_proxy_recovery: snapshot.system_proxy_recovery.clone(),
            mixed_port: value.mixed_port,
            allow_lan: value.allow_lan,
            lan_bind_address: value.lan_bind_address.clone(),
            lan_security: value.lan_security.clone(),
            ipv6_routing: value.ipv6_routing,
            pac: value.pac.clone(),
            network_roaming: snapshot.network_roaming.clone(),
            vpn: snapshot.vpn.clone(),
            privileged_network: snapshot.privileged_network.clone(),
            tun_enabled: value.tun_enabled,
            tun_stack: value.tun_stack,
            tun_auto_route: value.tun_auto_route,
            tun_strict_route: value.tun_strict_route,
            controller_port: value.controller_port,
            log_level: value.log_level,
            core_channel: value.core_channel,
            core_versions: snapshot.versions.clone(),
            core_integrity: snapshot.versions.verification.clone(),
            controller_auth: snapshot.controller_auth,
            service_mode: snapshot.service_mode,
            port_conflicts: snapshot.port_conflicts.clone(),
            core_resources: snapshot.resources.clone(),
            offline_startup: snapshot.offline_startup.clone(),
            mtu: snapshot.mtu.clone(),
            mini_hud: value.mini_hud,
        })
        .unwrap_or_else(|| {
            let mut projection = empty_settings();
            projection.preference_status = snapshot.pages.settings.status.clone();
            projection.runtime_status = snapshot.runtime_control.status.clone();
            projection.core_versions = snapshot.versions.clone();
            projection.core_integrity = snapshot.versions.verification.clone();
            projection.controller_auth = snapshot.controller_auth;
            projection.service_mode = snapshot.service_mode;
            projection.port_conflicts = snapshot.port_conflicts.clone();
            projection.core_resources = snapshot.resources.clone();
            projection.offline_startup = snapshot.offline_startup.clone();
            projection.mtu = snapshot.mtu.clone();
            projection.network_roaming = snapshot.network_roaming.clone();
            projection.vpn = snapshot.vpn.clone();
            projection.privileged_network = snapshot.privileged_network.clone();
            projection
        });
    let observed = &snapshot.runtime_control;
    projection.runtime_status = observed.status.clone();
    projection.mixed_port = observed.mixed_port;
    projection.allow_lan = observed.allow_lan;
    projection.lan_bind_address = observed.lan_bind_address.clone();
    projection.lan_security = observed.lan_security.clone();
    projection.ipv6_routing = observed.ipv6_routing;
    projection.tun_enabled = observed.tun_enabled;
    projection.tun_stack = observed.tun_stack.clone();
    projection.tun_auto_route = observed.tun_auto_route;
    projection.tun_strict_route = observed.tun_strict_route;
    projection.log_level = observed.log_level.clone();
    projection
}

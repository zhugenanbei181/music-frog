use super::surface_demo::{
    empty_app_routing, empty_connections, empty_dns, empty_doctor, empty_logs, empty_profiles,
    empty_proxies, empty_rules, empty_sync,
};
use super::surface_settings::empty_settings;
use super::*;
use crate::pages::app_routing::AppItem;
use infiltrator_domain::app_routing::{AppRoutingMode, AppRoutingRule};

use crate::pages::doctor::{DoctorCheckItem, DoctorProjection};
use crate::pages::logs::LogEntry;
use crate::pages::profiles::ProfileItem;
use crate::pages::proxies::{ProxyGroup, ProxyNode};
use crate::pages::rules::{RuleItem, RuleProviderItem};
use crate::pages::sync::{ConflictingKey, SnapshotItem, SyncConflictInfo};
use crate::projection::OverviewState;
use infiltrator_application::proxy_mode_application::ProxyModeApplication;
use infiltrator_application::system_toggle_application::SystemToggleApplication;
use infiltrator_contract::connection::ConnectionStreamPhase;
use infiltrator_contract::logs::LogLevel;
use infiltrator_contract::snapshot::CoreLifecycle;
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
        memory_bytes: page
            .and_then(|value| value.memory_bytes)
            .or(core.memory_bytes),
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
        origin: if snapshot.surface == SurfaceKind::BevyDesktop
            && snapshot.origin == surface_snapshot::SurfaceOrigin::Demo
        {
            OverviewOrigin::Demo
        } else {
            OverviewOrigin::LiveCore
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
        system_toggles: SystemToggleApplication::from_surface(snapshot),
        proxy_mode: ProxyModeApplication::from_surface(snapshot),
        speedtest: snapshot.speedtest.clone(),
        cpu_percent: snapshot.resources.cpu_percent,
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
    snapshot
        .pages
        .proxies
        .data
        .as_ref()
        .map(|value| ProxiesProjection {
            name_runs: value.name_runs.clone(),
            search_query: value.search_query.clone(),
            groups: value
                .groups
                .iter()
                .map(|group| ProxyGroup {
                    name: group.name.clone(),
                    group_type: group.group_type.clone(),
                    classification: group.resolved_classification(),
                    current: group.current.clone(),
                    expanded: group.expanded,
                    proxies: group
                        .proxies
                        .iter()
                        .map(|proxy| ProxyNode {
                            name: proxy.name.clone(),
                            node_type: proxy.node_type.clone(),
                            delay_ms: proxy.delay_ms,
                            selected: proxy.selected,
                            favorite: proxy.favorite,
                            features: proxy.features.clone(),
                        })
                        .collect(),
                })
                .collect(),
            testing: value.testing,
            filter_alive: value.filter_alive.enabled,
            compact_view: value.compact_view,
            active_exit: value.active_exit.clone(),
            custom_node: value.custom_node.clone(),
        })
        .unwrap_or_else(empty_proxies)
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
                    rule_count: provider.rule_count,
                    behavior: provider.behavior.clone(),
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
            entries: value
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
            mode: match value.mode.to_ascii_lowercase().as_str() {
                "proxy_selected" | "proxy_list" | "whitelist" => AppRoutingMode::ProxySelected,
                "bypass_selected" | "bypass_list" | "blacklist" => AppRoutingMode::BypassSelected,
                _ => AppRoutingMode::ProxyAll,
            },
            include_system: value.include_system,
            uwp_loopback: value.uwp_loopback.clone(),
            apps: value
                .apps
                .iter()
                .map(|app| AppItem {
                    id: app.id.clone(),
                    name: app.name.clone(),
                    process_name: app.process_name.clone(),
                    rule: match app.rule.to_ascii_lowercase().as_str() {
                        "direct" => AppRoutingRule::Direct,
                        "block" => AppRoutingRule::Block,
                        _ => AppRoutingRule::Proxy,
                    },
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

use super::*;

use super::surface_demo::{
    empty_app_routing, empty_connections, empty_dns, empty_doctor, empty_logs, empty_profiles,
    empty_proxies, empty_rules, empty_settings, empty_sync,
};
use crate::pages::app_routing::{AppItem, AppRouteRule, AppRoutingMode};
use crate::pages::connections::ConnectionItem;
use crate::pages::doctor::{DoctorCheckItem, DoctorCheckState, DoctorProjection};
use crate::pages::logs::{LogEntry, LogLevel};
use crate::pages::profiles::ProfileItem;
use crate::pages::proxies::{ProxyGroup, ProxyNode};
use crate::pages::rules::{RuleItem, RuleProviderItem};
use crate::pages::sync::{ConflictingKey, SnapshotItem, SyncConflictInfo, SyncStatus};
use crate::projection::OverviewState;
use infiltrator_contract::snapshot::CoreLifecycle;

fn log_level_from_str(value: &str) -> Option<LogLevel> {
    match value.trim().to_ascii_lowercase().as_str() {
        "debug" => Some(LogLevel::Debug),
        "info" => Some(LogLevel::Info),
        "warn" | "warning" => Some(LogLevel::Warn),
        "error" | "err" => Some(LogLevel::Error),
        _ => None,
    }
}

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
        state,
        mode: page
            .and_then(|value| value.proxy_mode)
            .or(core.proxy_mode)
            .unwrap_or_default(),
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
            .map(std::time::Duration::from_millis)
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
        system_toggles: infiltrator_application::system_toggle_application::SystemToggleApplication::from_surface(snapshot),
        proxy_mode: infiltrator_application::proxy_mode_application::ProxyModeApplication::from_surface(snapshot),
        speedtest: snapshot.speedtest.clone(),
        cpu_percent: snapshot.resources.cpu_percent,
        total_traffic_bytes: snapshot.pages.connections.data.as_ref().map(|c| c.total_upload_bytes + c.total_download_bytes),
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
            active_exit: value.active_exit.clone(),
        })
        .unwrap_or_else(empty_proxies)
}

pub(super) fn profiles_projection(
    snapshot: &surface_snapshot::SurfaceSnapshot,
) -> ProfilesProjection {
    snapshot
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
                })
                .collect(),
            auto_update_interval_hours: value.auto_update_interval_hours,
            updating: value.updating,
        })
        .unwrap_or_else(empty_profiles)
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
            hit_audit: value.tracer.hit_audit.clone(),
            mrs_acceleration: value.mrs_acceleration.clone(),
            // DUAL-11-08: the publish cap and the omitted count are shared
            // facts, so the page can render the truncation honestly.
            truncated_rule_count: value.is_truncated().then(|| value.omitted_rule_count()),
            rule_publish_limit: value.rule_publish_limit,
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
                })
                .collect(),
            rules: value
                .rules
                .iter()
                .map(|rule| RuleItem {
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
    let stream_phase = infiltrator_contract::connection::ConnectionStreamPhase::from_page_status(
        &snapshot.pages.connections.status,
    );
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
            connections: value
                .connections
                .iter()
                .map(|connection| ConnectionItem {
                    id: connection.id.clone(),
                    host: connection.host.clone(),
                    process: connection.process.clone(),
                    rule: connection.rule.clone(),
                    chain: connection.chain.clone(),
                    chains: connection.chains.clone(),
                    upload_bps: connection.upload_bps,
                    download_bps: connection.download_bps,
                    upload_total: connection.upload_total,
                    download_total: connection.download_total,
                })
                .collect(),
        })
        .unwrap_or_else(|| empty_connections_with_phase(stream_phase))
}

fn empty_connections_with_phase(
    stream_phase: infiltrator_contract::connection::ConnectionStreamPhase,
) -> ConnectionsProjection {
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
            total_entries: value.total_entries,
            active_level: value.active_level.as_deref().and_then(log_level_from_str),
            entries: value
                .entries
                .iter()
                .map(|entry| LogEntry {
                    timestamp: entry.timestamp.clone(),
                    level: log_level_from_str(&entry.level).unwrap_or(LogLevel::Info),
                    tag: entry.tag.clone(),
                    message: entry.message.clone(),
                })
                .collect(),
        })
        .unwrap_or_else(empty_logs)
}

pub(super) fn dns_projection(snapshot: &surface_snapshot::SurfaceSnapshot) -> DnsProjection {
    snapshot
        .pages
        .dns
        .data
        .as_ref()
        .map(DnsProjection::from_snapshot)
        .unwrap_or_else(empty_dns)
}

pub(super) fn doctor_projection(snapshot: &surface_snapshot::SurfaceSnapshot) -> DoctorProjection {
    let watchdog = snapshot.core.watchdog.clone();
    snapshot
        .pages
        .doctor
        .data
        .as_ref()
        .map(|value| DoctorProjection {
            overall_healthy: value.overall_healthy,
            last_run: value.last_run.clone(),
            watchdog: watchdog.clone(),
            checks: value
                .checks
                .iter()
                .map(|check| DoctorCheckItem {
                    id: check.id.clone(),
                    name: check.name.clone(),
                    category: check.category.clone(),
                    state: match check.state.to_ascii_lowercase().as_str() {
                        "fail" | "failed" => DoctorCheckState::Fail,
                        "warn" | "warning" => DoctorCheckState::Warning,
                        _ => DoctorCheckState::Pass,
                    },
                    detail: check.detail.clone(),
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
                "proxy_selected" | "proxy_list" | "whitelist" => AppRoutingMode::BypassList,
                "bypass_selected" | "bypass_list" | "blacklist" => AppRoutingMode::ProxyList,
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
                        "direct" => AppRouteRule::Direct,
                        "block" => AppRouteRule::Block,
                        _ => AppRouteRule::Proxy,
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
            status: match value.status.to_ascii_lowercase().as_str() {
                "syncing" => SyncStatus::Syncing,
                "conflict" => SyncStatus::Conflict,
                "error" => SyncStatus::Error,
                "disconnected" => SyncStatus::Disconnected,
                _ => SyncStatus::Connected,
            },
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
                    id: item.id.clone(),
                    timestamp: item.timestamp.clone(),
                    device: item.device.clone(),
                    size_bytes: item.size_bytes,
                })
                .collect(),
        })
        .unwrap_or_else(empty_sync)
}

pub(super) fn settings_projection(
    snapshot: &surface_snapshot::SurfaceSnapshot,
) -> SettingsProjection {
    snapshot
        .pages
        .settings
        .data
        .clone()
        .map(|value| SettingsProjection {
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
        })
}

//! Slint demo and capture fixture host.
//!
//! Activated via `--demo` or `INFILTRATOR_DEMO=1`.
//! Provides realistic snapshots without connecting to real cores or external network,
//! and writes the `CAPTURE_READY` marker for background virtual compositor screenshots.

mod fixtures;
mod outbound;
mod owner;

use super::*;
use crate::capture_scenario::{self, CaptureBounds};
use infiltrator_contract::active_exit::ActiveExitSnapshot;
use infiltrator_contract::connection::ConnectionRateObservation;
use infiltrator_contract::lan::LanSharingSnapshot;
use infiltrator_contract::logs::LogStreamState;
use infiltrator_contract::public_ip::PublicIpProbeSnapshot;
use infiltrator_contract::resources::{CoreGcStatus, CoreResourceSnapshot, ResourceAvailability};
use infiltrator_contract::rule_snapshot::RuleSnapshot;
use infiltrator_contract::shell_readout::{ShellObservation, ShellProfile};
use infiltrator_contract::speedtest::{
    SpeedtestConcurrency, SpeedtestOutboundResult, SpeedtestResultState, SpeedtestRunState,
    SpeedtestSampleOutcome, SpeedtestSnapshot,
};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{
    ConnectionSnapshot, ConnectionsPageSnapshot, LogSnapshot, LogsPageSnapshot,
    OverviewPageSnapshot, ProfileSnapshot, ProfilesPageSnapshot, RulesPageSnapshot,
    SettingsPageSnapshot, SurfaceSnapshot,
};
use infiltrator_contract::system_proxy::{SystemProxyObservation, SystemProxySnapshot};
use infiltrator_contract::theme::{SystemAppearance, ThemePreference};
use infiltrator_contract::theme_palette::{ResolvedThemePalette, SystemDynamicColor};
use infiltrator_contract::traffic_waveform::{TrafficSample, TrafficWaveformSnapshot};
use infiltrator_contract::tray_status::TrayIconColorMode;
use lurker_core_contract::error::Failure;
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::page::{PageData, SurfaceOrigin};
use lurker_core_contract::profile_protection::ProfileWriteProtection;
use lurker_core_contract::routing_intent::RoutingIntentSnapshot;
use std::cell::Cell;
use std::env;
use std::f64::consts::TAU;
use std::fs::{OpenOptions, create_dir_all};
use std::io::Write as _;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct DemoSettings {
    pub enabled: bool,
    pub page: String,
    pub skin: String,
    pub window_size: (f32, f32),
    pub marker: Option<PathBuf>,
    pub scenario: Option<String>,
}

impl DemoSettings {
    pub fn from_env() -> Self {
        let enabled = env::args().any(|arg| arg == "--demo")
            || env::var("INFILTRATOR_DEMO").is_ok_and(|v| v.trim() == "1");

        let raw_page = env::var("INFILTRATOR_PAGE").unwrap_or_else(|_| "overview".into());
        let page = match raw_page.trim().to_ascii_lowercase().as_str() {
            "overview" | "home" => "Home",
            "proxies" | "proxy" => "Proxy",
            "profiles" | "config" => "Config",
            "settings" => "Settings",
            "runtime" | "network" => "Network",
            "connections" => "Connections",
            "logs" => "Logs",
            "rules" | "rules-providers" => "Rules",
            "traffic" => "Traffic",
            "app_settings" | "app-settings" => "AppSettings",
            "about" => "About",
            "dns" => "Dns",
            "editor" => "Editor",
            "filters" | "filter" => "Filters",
            "doctor" => "Doctor",
            "sync" => "Sync",
            "app_routing" | "app-routing" => "AppRouting",
            "connections_workbench" | "connections-workbench" => "ConnectionsWorkbench",
            "logs_workbench" | "logs-workbench" => "LogsWorkbench",
            "runtime_workbench" | "runtime-workbench" => "RuntimeWorkbench",
            "proxies_workbench" | "proxies-workbench" => "ProxiesWorkbench",
            "rules_workbench" | "rules-workbench" => "RulesWorkbench",
            "settings_workbench" | "settings-workbench" => "SettingsWorkbench",
            "shell_workbench" | "shell-workbench" => "ShellWorkbench",
            "overview_workbench" | "overview-workbench" => "OverviewWorkbench",
            _ => "Home",
        }
        .to_string();

        let raw_skin = env::var("INFILTRATOR_SKIN").unwrap_or_else(|_| "dark".into());
        let skin = match raw_skin.trim().to_ascii_lowercase().as_str() {
            "light" => "light",
            "forest" => "forest",
            "amoled" => "amoled",
            _ => "dark",
        }
        .to_string();

        let window_size = env::var("INFILTRATOR_WINDOW_SIZE")
            .ok()
            .and_then(|raw| {
                let parts: Vec<&str> = raw.split('x').collect();
                if parts.len() == 2 {
                    if let (Ok(w), Ok(h)) = (parts[0].parse::<f32>(), parts[1].parse::<f32>()) {
                        return Some((w, h));
                    }
                }
                None
            })
            .unwrap_or((1180.0, 780.0));

        let marker = env::var("INFILTRATOR_CAPTURE_MARKER")
            .ok()
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty());

        let scenario = env::var("INFILTRATOR_SCENARIO")
            .ok()
            .filter(|v| !v.trim().is_empty());

        Self {
            enabled,
            page,
            skin,
            window_size,
            marker,
            scenario,
        }
    }
}

pub struct DemoProductHost {
    snapshot: Mutex<SurfaceSnapshot>,
}

impl DemoProductHost {
    pub fn new(settings: &DemoSettings) -> Self {
        let mut snapshot = build_demo_snapshot(settings);
        snapshot.origin = SurfaceOrigin::Demo;
        Self {
            snapshot: Mutex::new(snapshot),
        }
    }
}

impl SlintProductHost for DemoProductHost {
    fn read_snapshot(&self) -> Result<SurfaceSnapshot, Failure> {
        Ok(self
            .snapshot
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_else(|_| {
                SurfaceSnapshot::unavailable(
                    SurfaceKind::SlintDesktop,
                    HostKind::Desktop,
                    Failure::unsupported("the demo snapshot is poisoned"),
                )
            }))
    }

    fn execute(
        &self,
        _snapshot: Option<&SurfaceSnapshot>,
        action: SlintAction,
    ) -> Result<(), Failure> {
        self.execute_typed(_snapshot, action).map(|_| ())
    }

    fn execute_typed(
        &self,
        _snapshot: Option<&SurfaceSnapshot>,
        action: SlintAction,
    ) -> Result<CommandOutput, Failure> {
        let mut guard = self
            .snapshot
            .lock()
            .map_err(|_| Failure::unsupported("the demo snapshot is poisoned"))?;
        owner::respond(&mut guard, action)
    }

    fn autostart_enabled(&self) -> Option<bool> {
        Some(true)
    }
}

fn build_demo_snapshot(settings: &DemoSettings) -> SurfaceSnapshot {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::unsupported("demo snapshot provides explicit fixtures"),
    );
    snapshot.origin = SurfaceOrigin::Demo;
    snapshot.failure = None;
    snapshot.core.failure = None;
    snapshot.core.lifecycle = CoreLifecycle::Running;
    snapshot.core.identity = Some(outbound::demo_core_scope().identity);
    snapshot.core.core_version = Some("v1.19.18".into());
    snapshot.core.active_connections = 8;
    snapshot.core.routing_intent = RoutingIntentSnapshot::demo_fixture();

    const MB: f64 = 1024.0 * 1024.0;
    snapshot.shell_readout.upload_bps = ShellObservation::Current(2.5 * MB);
    snapshot.shell_readout.download_bps = ShellObservation::Current(12.8 * MB);
    snapshot.shell_readout.profile = ShellObservation::Current(ShellProfile {
        id: "profile-1".into(),
        name: "Alpha Fast (HK/JP/US)".into(),
        subscription: true,
        used_bytes: Some(8 * 1024 * 1024 * 1024),
        total_bytes: Some(100 * 1024 * 1024 * 1024),
        usage_fraction: Some(0.08),
    });
    // The profile card counts the engine's proxies; the fixture publishes the
    // same readback the live composition would report.
    snapshot.shell_readout.proxies = ShellObservation::Current(4);
    snapshot.system_proxy = SystemProxySnapshot::from_observation(
        1,
        SystemProxyObservation {
            enabled: true,
            endpoint: Some("127.0.0.1:7890".to_owned()),
            bypass: None,
        },
    );

    // One readable slow sweep per direction, like the reference dashboard:
    // a real waveform sample is a non-negative rate, so the old fixture's
    // signed sine produced clipped plateaus instead of a continuous curve.
    let samples: Vec<TrafficSample> = (0..60u32)
        .map(|i| {
            let phase = f64::from(i) / 59.0;
            let up = 0.45 + 0.42 * (phase * TAU - 0.4).sin() + 0.14 * (phase * TAU * 2.7).sin();
            let down =
                0.52 + 0.39 * (phase * TAU - 1.1).sin() + 0.10 * (phase * TAU * 1.8 + 0.6).sin();
            TrafficSample {
                sampled_at_epoch_ms: None,
                upload_bps: up.max(0.0) * 2.3 * MB,
                download_bps: down.max(0.0) * 6.8 * MB,
            }
        })
        .collect();
    snapshot.traffic_waveform = TrafficWaveformSnapshot {
        generation: 1,
        revision: samples.len() as u64,
        samples,
    };
    snapshot.traffic_history.today_upload_bytes = (1.6 * 1024.0 * 1024.0 * 1024.0) as u64;
    snapshot.traffic_history.today_download_bytes = (504.8 * 1024.0 * 1024.0) as u64;
    snapshot.traffic_history.today_has_data = true;

    // Four common-service probes, in the reference dashboard's order. The
    // fixture publishes real typed observations so the health, memory and CPU
    // cards are exercised by the capture instead of rendering dashes.
    snapshot.speedtest = SpeedtestSnapshot {
        revision: 1,
        state: SpeedtestRunState::Idle,
        concurrency: SpeedtestConcurrency::default(),
        results: ["Google", "GitHub", "YouTube", "ChatGPT"]
            .iter()
            .zip([951_u32, 1679, 4277, 1242])
            .map(|(name, delay)| SpeedtestOutboundResult {
                outbound_id: OutboundId::new(
                    EngineId::new("mihomo").expect("valid engine id"),
                    name.to_string(),
                )
                .expect("valid demo outbound id"),
                display_name: Some((*name).to_string()),
                protocol_display_name: None,
                samples: vec![SpeedtestSampleOutcome::Measured { delay_ms: delay }],
                state: SpeedtestResultState::Completed,
                tested_at_epoch_ms: 0,
            })
            .collect(),
        recent_history: Vec::new(),
    };
    snapshot.resources = CoreResourceSnapshot::Observed {
        scope: Box::new(outbound::demo_core_scope()),
        memory: ResourceAvailability::Available(208 * 1024 * 1024),
        cpu_percent: ResourceAvailability::Available(24.6),
        memory_soft_limit_bytes: 512 * 1024 * 1024,
        gc: Box::new(CoreGcStatus::NotAttempted),
    };

    snapshot.active_exit = ActiveExitSnapshot::demo_fixture();

    let (inventory, details, preferences) = outbound::demo_outbound_inventory();
    snapshot.outbound_inventory = PageData::ready(inventory);
    snapshot.outbound_details = PageData::ready(details);
    snapshot.outbound_ui_preferences = PageData::ready(preferences);

    let theme_str = match settings.skin.as_str() {
        "light" => "light",
        "forest" => "forest",
        "amoled" => "amoled",
        _ => "dark",
    };
    // One theme fact, one source: the fixture publishes the readback the
    // real composition derives from this hydrated setting, so every
    // projection paints the fixture skin from the authoritative readback.
    snapshot.theme_readback = ResolvedThemePalette::resolve(
        ThemePreference::from_setting(theme_str),
        SystemAppearance::Unobserved,
        SystemDynamicColor::Unobserved,
    );

    snapshot.pages.settings = PageData::ready(SettingsPageSnapshot {
        close_to_tray: Some(false),
        notifications_enabled: Some(true),
        language: "zh-CN".into(),
        theme: theme_str.into(),
        autostart: true,
        system_proxy: true,
        // The LAN sharing configuration is one owner readback: the fixture
        // publishes the whole confirmed configuration or nothing at all.
        lan_sharing: Some(LanSharingSnapshot::new(1, false, 7890, "*")),
        lan_security: None,
        ipv6_routing: None,
        pac: Default::default(),
        tun_enabled: Some(false),
        tun_stack: Some("gvisor".into()),
        tun_auto_route: Some(true),
        tun_strict_route: Some(false),
        controller_port: Some(9090),
        log_level: Some("info".into()),
        core_channel: "alpha".into(),
        mini_hud: Default::default(),
        refresh_current_profile_on_start: false,
        custom_accent_color: None,
        tray_icon_color_mode: TrayIconColorMode::default(),
    });

    snapshot.pages.profiles = PageData::ready(ProfilesPageSnapshot {
        profiles: vec![
            ProfileSnapshot {
                id: "profile-1".into(),
                name: "Alpha Fast (HK/JP/US)".into(),
                url: "https://example.com/sub/alpha".into(),
                updated_at: "2026-10-16 12:00:00".into(),
                upload_bytes: Some(1024 * 1024 * 512),
                download_bytes: Some(1024 * 1024 * 8192),
                total_bytes: Some(1024 * 1024 * 1024 * 128),
                is_active: true,
                user_agent: "".into(),
                insecure_skip_verify: false,
                etag: None,
                last_modified: None,
                has_backup: false,
                cron_expression: None,
                auto_update_enabled: true,
                update_interval_hours: Some(24),
                next_update: None,
                auto_reload_core: true,
                filter: fixtures::subscription_filter(),
                filter_source: Ok(fixtures::profile_workspace_identity()),
                write_protection: ProfileWriteProtection::RemoteSubscription,
            },
            ProfileSnapshot {
                id: "profile-2".into(),
                name: "Beta Local Direct".into(),
                url: "".into(),
                updated_at: "2026-10-15 18:30:00".into(),
                upload_bytes: Some(1024 * 1024 * 120),
                download_bytes: Some(1024 * 1024 * 1250),
                total_bytes: Some(1024 * 1024 * 1024 * 64),
                is_active: false,
                user_agent: "".into(),
                insecure_skip_verify: false,
                etag: None,
                last_modified: None,
                has_backup: false,
                cron_expression: None,
                auto_update_enabled: false,
                update_interval_hours: None,
                next_update: None,
                auto_reload_core: false,
                filter: Default::default(),
                filter_source: Ok(fixtures::profile_workspace_identity()),
                write_protection: ProfileWriteProtection::Editable,
            },
        ],
        auto_update_interval_hours: 24,
        updating: false,
        aggregation: Some(fixtures::aggregation_report()),
        aggregation_templates: Vec::new(),
        aggregation_templates_available: false,
        snapshot_history: Some(fixtures::snapshot_history()),
        apply_transaction: None,
        named_overrides: Default::default(),
    });

    // The workbench panels of the capture read the shared read models of the
    // businesses they carry, so a receipt shows the interaction and never an
    // empty panel.
    snapshot.dns_leak = fixtures::dns_leak_report();
    snapshot.dns_cache = fixtures::dns_cache_snapshot();
    snapshot.dns_query = fixtures::dns_query_snapshot();
    snapshot.dns_hosts = PageData::ready(fixtures::dns_hosts_profile());
    snapshot.public_ip = PublicIpProbeSnapshot::demo_fixture();
    snapshot.pages.doctor = PageData::ready(fixtures::doctor_page());
    snapshot.pages.sync = PageData::ready(fixtures::sync_page());
    snapshot.pages.app_routing = PageData::ready(fixtures::app_routing_page());
    snapshot.script_sandbox = Some(fixtures::script_sandbox());
    snapshot.script_export = Some(fixtures::script_export());
    snapshot.yaml_ast_diff = Some(fixtures::yaml_ast_diff());

    snapshot.pages.connections = PageData::ready(ConnectionsPageSnapshot {
        total_connections: 6,
        total_upload_bytes: 48 * 1024 * 1024,
        total_download_bytes: 380 * 1024 * 1024,
        connections: vec![
            ConnectionSnapshot {
                id: "conn-1".into(),
                start: "2026-10-16T12:10:05Z".into(),
                destination_host: "api.github.com:443".into(),
                host: "api.github.com".into(),
                process: "git".into(),
                rule: "DomainSuffix".into(),
                rule_payload: "github.com".into(),
                chain: "PROXIES -> HK-01".into(),
                chains: vec!["PROXIES".into(), "HK-01".into()],
                network: "tcp".into(),
                source_ip: "127.0.0.1".into(),
                source_port: "54320".into(),
                destination_ip: "20.205.243.166".into(),
                destination_port: "443".into(),
                destination_geo_ip: Some(vec!["US".into()]),
                destination_ip_asn: "15169 Google LLC".into(),
                rate_observation: ConnectionRateObservation::Unobserved,
                upload_bps: 2.0 * MB,
                download_bps: 8.5 * MB,
                upload_total: 2 * 1024 * 1024,
                download_total: 48 * 1024 * 1024,
            },
            ConnectionSnapshot {
                id: "conn-2".into(),
                start: "2026-10-16T12:11:10Z".into(),
                destination_host: "slint.dev:443".into(),
                host: "slint.dev".into(),
                process: "firefox".into(),
                rule: "DomainSuffix".into(),
                rule_payload: "slint.dev".into(),
                chain: "PROXIES -> JP-01".into(),
                chains: vec!["PROXIES".into(), "JP-01".into()],
                network: "tcp".into(),
                source_ip: "127.0.0.1".into(),
                source_port: "54322".into(),
                destination_ip: "104.21.5.88".into(),
                destination_port: "443".into(),
                destination_geo_ip: Some(vec!["US".into()]),
                destination_ip_asn: "13335 Cloudflare".into(),
                rate_observation: ConnectionRateObservation::Unobserved,
                upload_bps: 0.5 * MB,
                download_bps: 4.3 * MB,
                upload_total: 512 * 1024,
                download_total: 14 * 1024 * 1024,
            },
            ConnectionSnapshot {
                id: "conn-3".into(),
                start: "2026-10-16T12:11:38Z".into(),
                destination_host: "openapi.qoder.com.cn:443".into(),
                host: "openapi.qoder.com.cn".into(),
                process: "codex".into(),
                rule: "DomainSuffix".into(),
                rule_payload: "qoder.com.cn".into(),
                chain: "AI -> SG-01".into(),
                chains: vec!["AI".into(), "SG-01".into()],
                network: "tcp".into(),
                source_ip: "127.0.0.1".into(),
                source_port: "54328".into(),
                destination_ip: "104.18.12.7".into(),
                destination_port: "443".into(),
                destination_geo_ip: Some(vec!["SG".into()]),
                destination_ip_asn: "13335 Cloudflare".into(),
                rate_observation: ConnectionRateObservation::Unobserved,
                upload_bps: 0.9 * MB,
                download_bps: 3.1 * MB,
                upload_total: 96 * 1024,
                download_total: 320 * 1024,
            },
            ConnectionSnapshot {
                id: "conn-4".into(),
                start: "2026-10-16T12:11:42Z".into(),
                destination_host: "snap-access.cn-north-4.myhuaweicloud.com:443".into(),
                host: "snap-access.cn-north-4.myhuaweicloud.com".into(),
                process: "snap".into(),
                rule: "DomainSuffix".into(),
                rule_payload: "myhuaweicloud.com".into(),
                chain: "DIRECT".into(),
                chains: vec!["DIRECT".into()],
                network: "tcp".into(),
                source_ip: "127.0.0.1".into(),
                source_port: "54331".into(),
                destination_ip: "121.36.10.22".into(),
                destination_port: "443".into(),
                destination_geo_ip: Some(vec!["CN".into()]),
                destination_ip_asn: "55990 Huawei Cloud".into(),
                rate_observation: ConnectionRateObservation::Unobserved,
                upload_bps: 0.1 * MB,
                download_bps: 1.8 * MB,
                upload_total: 3 * 1024,
                download_total: 6300 * 1024,
            },
            ConnectionSnapshot {
                id: "conn-5".into(),
                start: "2026-10-16T12:11:47Z".into(),
                destination_host: "ab.chatgpt.com:443".into(),
                host: "ab.chatgpt.com".into(),
                process: "curl".into(),
                rule: "GeoIP".into(),
                rule_payload: "US".into(),
                chain: "GLOBAL -> US-01".into(),
                chains: vec!["GLOBAL".into(), "US-01".into()],
                network: "tcp".into(),
                source_ip: "127.0.0.1".into(),
                source_port: "54336".into(),
                destination_ip: "104.18.42.231".into(),
                destination_port: "443".into(),
                destination_geo_ip: Some(vec!["US".into()]),
                destination_ip_asn: "13335 Cloudflare".into(),
                rate_observation: ConnectionRateObservation::Unobserved,
                upload_bps: 7.9 * MB,
                download_bps: 3.0 * 1024.0,
                upload_total: 7900 * 1024,
                download_total: 3 * 1024,
            },
            ConnectionSnapshot {
                id: "conn-6".into(),
                start: "2026-10-16T12:11:51Z".into(),
                destination_host: "opengw.developer.huaweicloud.com:443".into(),
                host: "opengw.developer.huaweicloud.com".into(),
                process: "node".into(),
                rule: "DomainSuffix".into(),
                rule_payload: "huaweicloud.com".into(),
                chain: "DIRECT".into(),
                chains: vec!["DIRECT".into()],
                network: "tcp".into(),
                source_ip: "127.0.0.1".into(),
                source_port: "54340".into(),
                destination_ip: "119.8.35.18".into(),
                destination_port: "443".into(),
                destination_geo_ip: Some(vec!["CN".into()]),
                destination_ip_asn: "55990 Huawei Cloud".into(),
                rate_observation: ConnectionRateObservation::Unobserved,
                upload_bps: 3.0 * 1024.0,
                download_bps: 974.0 * 1024.0,
                upload_total: 3 * 1024,
                download_total: 974 * 1024,
            },
        ],
    });

    snapshot.pages.rules = PageData::ready(RulesPageSnapshot {
        document: None,
        total_rules: 3,
        default_action: "DIRECT".into(),
        providers: Vec::new(),
        rules: vec![
            RuleSnapshot {
                id: 1,
                edit_id: None,
                rule_type: "DomainSuffix".into(),
                payload: "github.com".into(),
                proxy: "PROXIES".into(),
                hit_count: Some(152),
                raw: "DOMAIN-SUFFIX,github.com,PROXIES".into(),
                source_ip: false,
                failure: None,
                is_enabled: true,
                no_resolve: false,
                last_hit_secs: None,
                is_shadowed: false,
                shadow_reason: None,
            },
            RuleSnapshot {
                id: 2,
                edit_id: None,
                rule_type: "GeoIP".into(),
                payload: "CN".into(),
                proxy: "DIRECT".into(),
                hit_count: Some(1420),
                raw: "GEOIP,CN,DIRECT".into(),
                source_ip: false,
                failure: None,
                is_enabled: true,
                no_resolve: false,
                last_hit_secs: None,
                is_shadowed: false,
                shadow_reason: None,
            },
            RuleSnapshot {
                id: 3,
                edit_id: None,
                rule_type: "Match".into(),
                payload: "".into(),
                proxy: "PROXIES".into(),
                hit_count: Some(89),
                raw: "MATCH,PROXIES".into(),
                source_ip: false,
                failure: None,
                is_enabled: true,
                no_resolve: false,
                last_hit_secs: None,
                is_shadowed: false,
                shadow_reason: None,
            },
        ],
        tracer: Default::default(),
        mrs_acceleration: Default::default(),
        hit_audit: None,
        rule_publish_limit: 500,
        provider_cache: Default::default(),
        etag_support: Default::default(),
        json_documents: Vec::new(),
    });

    snapshot.pages.logs = PageData::ready(LogsPageSnapshot {
        stream: LogStreamState::Live,
        total_entries: 3,
        active_level: Some("info".into()),
        entries: vec![
            LogSnapshot {
                id: 1,
                raw: None,
                timestamp: "12:00:01".into(),
                level: "info".into(),
                tag: "CORE".into(),
                message: "Mihomo proxy core initialized (v1.19.18)".into(),
            },
            LogSnapshot {
                id: 2,
                raw: None,
                timestamp: "12:00:02".into(),
                level: "info".into(),
                tag: "DNS".into(),
                message: "Fake-IP mapping pool active [198.18.0.1/16]".into(),
            },
            LogSnapshot {
                id: 3,
                raw: None,
                timestamp: "12:00:03".into(),
                level: "info".into(),
                tag: "ROUTING".into(),
                message: "Policy group route match: HK IPLC 01 (38ms)".into(),
            },
        ],
    });

    snapshot.pages.overview = PageData::ready(OverviewPageSnapshot {
        routing_intent: RoutingIntentSnapshot::demo_fixture(),
        upload_bps: 2.5 * MB,
        download_bps: 12.8 * MB,
        active_connections: 8,
        core_version: Some("v1.19.18".into()),
    });

    snapshot
}

static CAPTURE_MARKER_WRITTEN: AtomicBool = AtomicBool::new(false);

pub fn write_capture_marker(
    settings: &DemoSettings,
    _window: &AppWindow,
    activated: bool,
    bounds: Option<CaptureBounds>,
) {
    if !settings.enabled || CAPTURE_MARKER_WRITTEN.load(Ordering::SeqCst) {
        return;
    }
    let Some(path) = settings.marker.as_ref() else {
        return;
    };

    let marker_page = match settings.page.as_str() {
        "Home" => "overview",
        "Proxy" => "proxies",
        "Config" => "profiles",
        "Network" => "runtime",
        "Rules" => "rules",
        "Connections" => "connections",
        "Logs" => "logs",
        "Settings" => "settings",
        "AppSettings" => "app_settings",
        "Traffic" => "traffic",
        "About" => "about",
        // The workbench pages carry the parity businesses of the DNS, editor,
        // filter, doctor, sync and routing panels.
        "Dns" => "dns",
        "Editor" => "editor",
        "Filters" => "filters",
        "Doctor" => "doctor",
        "Sync" => "sync",
        "AppRouting" => "app_routing",
        "ConnectionsWorkbench" => "connections_workbench",
        "LogsWorkbench" => "logs_workbench",
        "RuntimeWorkbench" => "runtime_workbench",
        "ProxiesWorkbench" => "proxies_workbench",
        "RulesWorkbench" => "rules_workbench",
        "SettingsWorkbench" => "settings_workbench",
        "ShellWorkbench" => "shell_workbench",
        "OverviewWorkbench" => "overview_workbench",
        _ => "overview",
    };

    let mut line = format!(
        "CAPTURE_READY page={} skin={}\n",
        marker_page, settings.skin
    );

    if let Some(scenario) = &settings.scenario {
        // A scenario is only published as activated when the surface really
        // opened it and its real geometry is known.
        match (activated, bounds) {
            (true, Some(bounds)) => {
                line = format!(
                    "{} scenario={} activated=true bounds={},{},{},{}\n",
                    line.trim_end(),
                    scenario,
                    bounds.x,
                    bounds.y,
                    bounds.width,
                    bounds.height
                );
            }
            _ => {
                line = format!(
                    "{} scenario={} activated=false\n",
                    line.trim_end(),
                    scenario
                );
            }
        }
    }

    if let Some(parent) = path.parent() {
        let _ = create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        if file.write_all(line.as_bytes()).is_ok() {
            CAPTURE_MARKER_WRITTEN.store(true, Ordering::SeqCst);
        }
    }
}

/// Runs the demo fixture host with the real product wiring.
///
/// The demo registers the product callbacks and answers the commands the
/// capture's gestures send, so the capture drives the same code the running
/// product drives.
pub fn run_demo(settings: DemoSettings) -> Result<(), slint::PlatformError> {
    let host = Arc::new(DemoProductHost::new(&settings));
    let snapshot = host
        .read_snapshot()
        .map_err(|failure| slint::PlatformError::OtherError(Box::new(failure)))?;
    crate::run_with_host_prepared(host, move |window, workbenches| {
        prepare_demo_window(window, workbenches, &settings, &snapshot);
    })
}

/// Prepares the demo window before the owner worker starts.
fn prepare_demo_window(
    window: &AppWindow,
    workbenches: &Arc<Workbenches>,
    settings: &DemoSettings,
    snapshot: &SurfaceSnapshot,
) {
    window.set_frameless(true);
    window.window().set_size(slint::LogicalSize::new(
        settings.window_size.0,
        settings.window_size.1,
    ));
    lumen_projection::project_snapshot(window, snapshot, Some(true));
    lumen_projection::project_workbench(window, snapshot, workbenches);

    // Explicitly enforce the page after projection; the theme rides the
    // snapshot's published readback, so there is no second theme writer.
    window.set_page(settings.page.clone().into());
    lumen_projection::project_workbench(window, snapshot, workbenches);
    open_proxies_group(window, snapshot);
    window.set_notice("".into());

    install_capture_timer(window, Arc::clone(workbenches), settings, snapshot.clone());
}

/// Opens the proxies page on the group the owner marked as active.
///
/// The proxies page reaches its node list through a group, and the outbound
/// search is only reachable on that list. The demo therefore opens the page
/// the way a user reaches the search, so the route-only render and the
/// activated render differ by the search and by nothing else.
fn open_proxies_group(window: &AppWindow, snapshot: &SurfaceSnapshot) {
    if window.get_page().as_str() != "Proxy" {
        return;
    }
    let Some(group) = snapshot.active_exit.group.as_deref() else {
        return;
    };
    window.set_selected_group(group.into());
}

/// Publishes the capture marker once the business really shows its outcome.
///
/// The marker names the business the capture activated, so a scenario whose
/// business never opened is published as not activated.
fn install_capture_timer(
    window: &AppWindow,
    workbenches: Arc<Workbenches>,
    settings: &DemoSettings,
    snapshot: SurfaceSnapshot,
) {
    let marker_settings = settings.clone();
    let scenario = settings.scenario.clone();
    let weak_window = window.as_weak();
    let activated = Cell::new(false);
    let activation = Rc::new(capture_scenario::Activation::default());
    let previous_bounds = Cell::new(None::<CaptureBounds>);
    let ticks = Cell::new(0u32);
    let compact = settings.window_size.0 < 900.0;
    let timer = Rc::new(slint::Timer::default());
    let timer_for_callback = Rc::clone(&timer);
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(100),
        move || {
            let Some(window) = weak_window.upgrade() else {
                timer_for_callback.stop();
                return;
            };
            ticks.set(ticks.get() + 1);
            if let Some(scenario) = &scenario
                && !activated.get()
            {
                // The activation drives the business's own gesture and waits
                // until the business shows its own outcome.
                activated.set(capture_scenario::activate(
                    &window,
                    scenario,
                    &snapshot,
                    &workbenches,
                    &activation,
                    compact,
                ));
            }
            let bounds = capture_scenario::bounds(&window);
            let stable = bounds.is_some() && previous_bounds.get() == bounds;
            previous_bounds.set(bounds);
            // The window needs a few frames before its first paint, and a
            // scenario needs a settled sheet before its bounds are real.
            if ticks.get() < 5 || (scenario.is_some() && (!stable || !activated.get())) {
                return;
            }
            write_capture_marker(&marker_settings, &window, activated.get(), bounds);
            timer_for_callback.stop();
        },
    );
}

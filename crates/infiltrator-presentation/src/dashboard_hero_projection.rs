use infiltrator_contract::lan_share::LanShareWizardSnapshot;
use infiltrator_contract::runtime_status::CoreProcessObservation;
use infiltrator_contract::shell_readout::ShellObservation;
/// Multi-module dashboard projections matching the FlClash high-density dashboard reference (#021~#028).
use infiltrator_contract::surface_snapshot::ConnectionSnapshot;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::system_proxy::SystemProxyStatus;
use infiltrator_contract::traffic_waveform::TrafficWaveformSnapshot;
use infiltrator_shared::byte_units::format_bytes;
use infiltrator_shared::country_flags::node_flag_emoji;
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::outbound::inventory::{
    OutboundInventorySnapshot, OutboundInventoryState, OutboundSelectionState,
    SelectionAvailability,
};
use lurker_core_contract::routing_intent::{RoutingIntent, RoutingIntentSnapshot};

/// Dashboard Hero card presentation model (TODO-021).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DashboardHeroProjection {
    pub status_label: String,
    pub status_role: &'static str,
    pub is_running: bool,
    pub exit_ip: String,
    pub exit_country_code: String,
    pub organization: String,
    pub quality_tag: String,
    pub uptime_formatted: String,
}

impl DashboardHeroProjection {
    pub fn from_snapshot(snapshot: &SurfaceSnapshot) -> Self {
        let (status_label, status_role, is_running) = match &snapshot.runtime_status.core_process {
            CoreProcessObservation::Running | CoreProcessObservation::Ready => {
                ("运行中".to_string(), "running", true)
            }
            CoreProcessObservation::Starting => ("启动中…".to_string(), "paused", false),
            CoreProcessObservation::Stopping => ("停止中…".to_string(), "paused", false),
            CoreProcessObservation::Failed { .. } => ("故障".to_string(), "error", false),
            CoreProcessObservation::Stopped
            | CoreProcessObservation::Unselected
            | CoreProcessObservation::Unknown => ("已停止".to_string(), "offline", false),
        };

        let exit_ip = snapshot
            .public_ip
            .ip
            .clone()
            .unwrap_or_else(|| "—".to_string());

        let exit_country_code = snapshot.public_ip.country_code.clone().unwrap_or_default();

        let organization = snapshot
            .public_ip
            .isp
            .clone()
            .unwrap_or_else(|| "未知网络组织".to_string());

        let quality_tag = if let Some(quality) = &snapshot.ip_quality.record {
            quality.usage_type.label_zh().to_string()
        } else {
            "普通线路".to_string()
        };

        // The owner does not publish a core session start yet, so the card
        // reports an honest unknown instead of a fabricated duration.
        let uptime_formatted = "—".to_string();

        Self {
            status_label,
            status_role,
            is_running,
            exit_ip,
            exit_country_code,
            organization,
            quality_tag,
            uptime_formatted,
        }
    }
}

/// Traffic mini-waveform projection with 60-sample normalized buffer (TODO-022).
#[derive(Clone, Debug, PartialEq)]
pub struct TrafficWaveformProjection {
    pub live_down_formatted: String,
    pub live_up_formatted: String,
    pub peak_down_formatted: String,
    pub peak_up_formatted: String,
    pub total_down_formatted: String,
    pub total_up_formatted: String,
    pub normalized_down_points: Vec<f32>,
    pub normalized_up_points: Vec<f32>,
}

impl TrafficWaveformProjection {
    pub fn from_snapshot(snapshot: &SurfaceSnapshot) -> Self {
        Self::from_waveform(
            &snapshot.traffic_waveform,
            snapshot.traffic_history.today_download_bytes,
            snapshot.traffic_history.today_upload_bytes,
        )
    }

    pub fn from_waveform(
        waveform: &TrafficWaveformSnapshot,
        total_down_bytes: u64,
        total_up_bytes: u64,
    ) -> Self {
        let last_sample = waveform.samples.last();
        let live_down = last_sample.map(|s| s.download_bps).unwrap_or(0.0);
        let live_up = last_sample.map(|s| s.upload_bps).unwrap_or(0.0);

        let peak_down = waveform
            .samples
            .iter()
            .map(|s| s.download_bps)
            .fold(0.0_f64, f64::max);
        let peak_up = waveform
            .samples
            .iter()
            .map(|s| s.upload_bps)
            .fold(0.0_f64, f64::max);

        let normalized_down_points = if peak_down > 0.0 {
            waveform
                .samples
                .iter()
                .map(|s| (s.download_bps / peak_down) as f32)
                .collect()
        } else {
            vec![0.0; waveform.samples.len()]
        };

        let normalized_up_points = if peak_up > 0.0 {
            waveform
                .samples
                .iter()
                .map(|s| (s.upload_bps / peak_up) as f32)
                .collect()
        } else {
            vec![0.0; waveform.samples.len()]
        };

        Self {
            live_down_formatted: format_rate(live_down),
            live_up_formatted: format_rate(live_up),
            peak_down_formatted: format!("峰值 {}", format_rate(peak_down)),
            peak_up_formatted: format!("峰值 {}", format_rate(peak_up)),
            total_down_formatted: format!("累计 {}", format_bytes(total_down_bytes)),
            total_up_formatted: format!("累计 {}", format_bytes(total_up_bytes)),
            normalized_down_points,
            normalized_up_points,
        }
    }
}

/// Outbound mode segmented pill selector choice (TODO-023).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutboundModeChoice {
    Rule,
    Global,
    Direct,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutboundModeSelectorProjection {
    pub selected: OutboundModeChoice,
    pub is_interactive: bool,
}

impl OutboundModeSelectorProjection {
    pub fn from_intent(intent: &RoutingIntentSnapshot, is_running: bool) -> Self {
        let selected = match intent.current.as_ref() {
            Some(RoutingIntent::FollowRules) => OutboundModeChoice::Rule,
            Some(RoutingIntent::RouteAllThroughProxy) => OutboundModeChoice::Global,
            Some(RoutingIntent::BypassProxy) => OutboundModeChoice::Direct,
            Some(RoutingIntent::EngineDefined(_)) | None => OutboundModeChoice::Rule,
        };
        Self {
            selected,
            is_interactive: is_running,
        }
    }

    pub fn from_snapshot(snapshot: &SurfaceSnapshot) -> Self {
        let is_running = matches!(
            snapshot.runtime_status.core_process,
            CoreProcessObservation::Running | CoreProcessObservation::Ready
        );
        Self::from_intent(&snapshot.core.routing_intent, is_running)
    }
}

/// Quick switches projection for system proxy, TUN, and LAN sharing (TODO-024, TODO-025).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuickSwitchesProjection {
    pub system_proxy_enabled: bool,
    pub system_proxy_available: bool,
    pub system_proxy_state_text: &'static str,

    pub tun_enabled: bool,
    pub tun_available: bool,
    pub tun_state_text: &'static str,

    pub lan_share_enabled: bool,
    pub lan_share_available: bool,
    pub lan_ip_text: String,
    pub lan_port_text: String,
}

impl QuickSwitchesProjection {
    pub fn from_states(
        system_proxy_status: &SystemProxyStatus,
        tun_enabled: bool,
        tun_available: bool,
        lan_share: Option<&LanShareWizardSnapshot>,
    ) -> Self {
        let (system_proxy_enabled, system_proxy_available) = match system_proxy_status {
            SystemProxyStatus::Enabled => (true, true),
            SystemProxyStatus::Disabled => (false, true),
            SystemProxyStatus::Failed { .. }
            | SystemProxyStatus::Unsupported { .. }
            | SystemProxyStatus::Unknown => (false, false),
        };
        let system_proxy_state_text = if system_proxy_enabled {
            "已开启"
        } else {
            "已关闭"
        };

        let tun_state_text = if tun_enabled {
            "已开启"
        } else {
            "已关闭"
        };

        let (lan_share_enabled, lan_share_available, lan_ip_text, lan_port_text) =
            if let Some(lan) = lan_share {
                let ip = lan
                    .primary_endpoint
                    .as_ref()
                    .map(|e| e.ip.clone())
                    .unwrap_or_else(|| "127.0.0.1".to_string());
                (lan.is_lan_enabled, true, ip, lan.mixed_port.to_string())
            } else {
                (false, false, "127.0.0.1".to_string(), "7890".to_string())
            };

        Self {
            system_proxy_enabled,
            system_proxy_available,
            system_proxy_state_text,
            tun_enabled,
            tun_available,
            tun_state_text,
            lan_share_enabled,
            lan_share_available,
            lan_ip_text,
            lan_port_text,
        }
    }

    pub fn from_snapshot(snapshot: &SurfaceSnapshot) -> Self {
        let tun_enabled = snapshot
            .pages
            .settings
            .data
            .as_ref()
            .and_then(|d| d.tun_enabled)
            .unwrap_or(false);
        let tun_available = snapshot
            .pages
            .settings
            .data
            .as_ref()
            .and_then(|d| d.tun_enabled)
            .is_some();
        Self::from_states(
            &snapshot.system_proxy.status,
            tun_enabled,
            tun_available,
            None,
        )
    }
}

/// Four-tile service health matrix projection (TODO-027).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceHealthItem {
    pub name: &'static str,
    pub icon_id: &'static str,
    pub latency_ms: Option<u32>,
    pub status_role: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceHealthMatrixProjection {
    pub services: Vec<ServiceHealthItem>,
}

impl ServiceHealthMatrixProjection {
    pub fn from_latencies(
        google: Option<u32>,
        github: Option<u32>,
        youtube: Option<u32>,
        openai: Option<u32>,
    ) -> Self {
        let list = [
            ("Google", "google", google),
            ("GitHub", "github", github),
            ("YouTube", "youtube", youtube),
            ("ChatGPT", "chatgpt", openai),
        ];

        let mut services = Vec::with_capacity(4);
        for (name, icon_id, latency) in list {
            let status_role = match latency {
                Some(ms) if ms <= 160 => "good",
                Some(ms) if ms <= 300 => "warning",
                Some(_) => "poor",
                None => "dim",
            };
            services.push(ServiceHealthItem {
                name,
                icon_id,
                latency_ms: latency,
                status_role,
            });
        }

        Self { services }
    }
}

/// Dual-color memory consumption projection (TODO-028).
#[derive(Clone, Debug, PartialEq)]
pub struct MemoryInfoCardProjection {
    pub total_formatted: String,
    pub app_formatted: String,
    pub core_formatted: String,
    pub app_fraction: f32,
    pub core_fraction: f32,
}

/// Resource gauge presentation model (TODO-108): one memory bar and one CPU
/// bar, with unavailable facts rendered as explicit empty gauges rather than
/// fabricated zeros.
#[derive(Clone, Debug, PartialEq)]
pub struct ResourceGaugeProjection {
    pub memory_text: String,
    pub cpu_text: String,
    pub memory_fraction: f32,
    pub cpu_fraction: f32,
    pub observed: bool,
}

impl ResourceGaugeProjection {
    pub fn new(
        memory_bytes: Option<u64>,
        memory_limit_bytes: Option<u64>,
        cpu_percent: Option<f32>,
    ) -> Self {
        let memory_text = memory_bytes.map_or_else(|| "—".to_string(), format_bytes);
        let cpu_text = cpu_percent.map_or_else(
            || "—".to_string(),
            |percent| format!("{:.1}%", percent.clamp(0.0, 100.0)),
        );
        let memory_fraction = memory_bytes
            .zip(memory_limit_bytes)
            .filter(|(_, limit)| *limit > 0)
            .map_or(0.0, |(bytes, limit)| {
                (bytes as f32 / limit as f32).clamp(0.0, 1.0)
            });
        let cpu_fraction = cpu_percent.map_or(0.0, |percent| percent.clamp(0.0, 100.0) / 100.0);
        let observed = memory_bytes.is_some() || cpu_percent.is_some();

        Self {
            memory_text,
            cpu_text,
            memory_fraction,
            cpu_fraction,
            observed,
        }
    }
}

impl MemoryInfoCardProjection {
    pub fn from_bytes(app_bytes: u64, core_bytes: u64) -> Self {
        let total = app_bytes + core_bytes;
        let total_formatted = format_bytes(total);
        let app_formatted = format!("应用 {}", format_bytes(app_bytes));
        let core_formatted = format!("内核 {}", format_bytes(core_bytes));

        let (app_fraction, core_fraction) = if total > 0 {
            (
                (app_bytes as f32) / (total as f32),
                (core_bytes as f32) / (total as f32),
            )
        } else {
            (0.5, 0.5)
        };

        Self {
            total_formatted,
            app_formatted,
            core_formatted,
            app_fraction,
            core_fraction,
        }
    }
}

/// Profile summary card projection (TODO-026).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileQuickCardProjection {
    pub profile_name: String,
    pub updated_relative: String,
    pub is_local: bool,
    pub policy_groups_count: usize,
    pub proxies_count: usize,
    pub rules_count: usize,
}

impl ProfileQuickCardProjection {
    pub fn from_snapshot(snapshot: &SurfaceSnapshot) -> Self {
        let active_profile = snapshot
            .pages
            .profiles
            .data
            .as_ref()
            .and_then(|p| p.profiles.iter().find(|prof| prof.is_active));

        let profile_name = active_profile
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "默认配置.yaml".to_string());

        let updated_relative = active_profile
            .map(|p| {
                if p.updated_at.is_empty() {
                    "本地配置".to_string()
                } else {
                    format!("更新于 {}", p.updated_at)
                }
            })
            .unwrap_or_else(|| "本地配置".to_string());

        let rules_count = snapshot
            .pages
            .rules
            .data
            .as_ref()
            .map(|r| r.rules.len())
            .unwrap_or(0);

        let proxies_count = match &snapshot.shell_readout.proxies {
            ShellObservation::Current(count) | ShellObservation::Stale(count) => *count,
            _ => 0,
        };
        // The same authoritative inventory the policy grid reads: a card that
        // claimed "0 策略组" beside a grid of three groups contradicted the
        // owner's own readback.
        let policy_groups_count = match snapshot.outbound_inventory.data.as_ref() {
            Some(inventory) => match inventory.inventory() {
                OutboundInventoryState::Ready { collections, .. } => collections.len(),
                _ => 0,
            },
            None => 0,
        };

        Self {
            profile_name,
            updated_relative,
            is_local: active_profile.map(|p| p.url.is_empty()).unwrap_or(true),
            policy_groups_count,
            proxies_count,
            rules_count,
        }
    }
}

/// Quick policy group item presentation model (TODO-029).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyGroupMemberProjection {
    pub outbound_id: OutboundId,
    pub display_name: String,
    pub flag_emoji: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyGroupGridItem {
    pub collection_id: OutboundId,
    pub name: String,
    pub avatar_char: String,
    pub selected_outbound_id: Option<OutboundId>,
    pub selected_node: Option<String>,
    pub selected_flag: Option<String>,
    pub availability: SelectionAvailability,
    pub members: Vec<PolicyGroupMemberProjection>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PolicyGroupGridProjection {
    pub observed: bool,
    pub total_count: usize,
    pub groups: Vec<PolicyGroupGridItem>,
}

impl PolicyGroupGridProjection {
    pub fn unavailable() -> Self {
        Self {
            observed: false,
            total_count: 0,
            groups: Vec::new(),
        }
    }

    pub fn from_snapshot(snapshot: &SurfaceSnapshot) -> Self {
        snapshot
            .outbound_inventory
            .data
            .as_ref()
            .map(Self::from_inventory)
            .unwrap_or_else(Self::unavailable)
    }

    pub fn from_inventory(inventory: &OutboundInventorySnapshot) -> Self {
        let OutboundInventoryState::Ready {
            entries,
            collections,
        } = inventory.inventory()
        else {
            return Self {
                observed: matches!(
                    inventory.inventory(),
                    OutboundInventoryState::Empty | OutboundInventoryState::Unsupported
                ),
                total_count: 0,
                groups: Vec::new(),
            };
        };
        let display_name = |id: &OutboundId| {
            entries
                .iter()
                .find(|entry| &entry.id == id)
                .and_then(|entry| entry.display_name.clone())
                .unwrap_or_else(|| id.value().to_owned())
        };
        let groups: Vec<PolicyGroupGridItem> = collections
            .iter()
            .map(|collection| {
                let selected_outbound_id = match &collection.selection {
                    OutboundSelectionState::Ready { selected } => Some(selected.clone()),
                    _ => None,
                };
                let selected_node = selected_outbound_id.as_ref().map(display_name);
                let selected_flag = selected_node
                    .as_ref()
                    .map(|name| node_flag_emoji(name).into());
                let name = display_name(&collection.id);
                Self::group_item(
                    collection.id.clone(),
                    name,
                    selected_outbound_id,
                    selected_node,
                    selected_flag,
                    collection.availability,
                    collection
                        .member_ids
                        .iter()
                        .map(|member_id| {
                            let display_name = display_name(member_id);
                            PolicyGroupMemberProjection {
                                outbound_id: member_id.clone(),
                                flag_emoji: node_flag_emoji(&display_name).into(),
                                display_name,
                            }
                        })
                        .collect(),
                )
            })
            .collect();
        Self {
            observed: true,
            total_count: groups.len(),
            groups,
        }
    }

    fn group_item(
        collection_id: OutboundId,
        name: String,
        selected_outbound_id: Option<OutboundId>,
        selected_node: Option<String>,
        selected_flag: Option<String>,
        availability: SelectionAvailability,
        members: Vec<PolicyGroupMemberProjection>,
    ) -> PolicyGroupGridItem {
        let avatar_char = name
            .chars()
            .find(|c| !c.is_whitespace())
            .unwrap_or('#')
            .to_ascii_uppercase()
            .to_string();
        PolicyGroupGridItem {
            collection_id,
            name,
            avatar_char,
            selected_outbound_id,
            selected_node,
            selected_flag,
            availability,
            members,
        }
    }
}

/// Live connection mini table row and summary projection (TODO-030).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveConnectionMiniTableRow {
    pub id: String,
    pub destination: String,
    pub network: String,
    pub outbound: String,
    pub download_formatted: String,
    pub upload_formatted: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveConnectionMiniTableProjection {
    pub tcp_count: usize,
    pub udp_count: usize,
    pub total_active: usize,
    pub rows: Vec<LiveConnectionMiniTableRow>,
}

impl LiveConnectionMiniTableProjection {
    pub fn from_connections(conns: &[ConnectionSnapshot]) -> Self {
        let mut tcp_count = 0;
        let mut udp_count = 0;
        let mut rows = Vec::with_capacity(conns.len());
        for c in conns {
            if c.network.eq_ignore_ascii_case("tcp") {
                tcp_count += 1;
            } else if c.network.eq_ignore_ascii_case("udp") {
                udp_count += 1;
            }
            rows.push(LiveConnectionMiniTableRow {
                id: c.id.clone(),
                destination: if c.destination_host.is_empty() {
                    c.host.clone()
                } else {
                    c.destination_host.clone()
                },
                network: c.network.to_ascii_uppercase(),
                outbound: c.chain.clone(),
                download_formatted: format_bytes(c.download_total),
                upload_formatted: format_bytes(c.upload_total),
            });
        }
        Self {
            tcp_count,
            udp_count,
            total_active: conns.len(),
            rows,
        }
    }
}

fn format_rate(bps: f64) -> String {
    if bps >= 1_000_000_000.0 {
        format!("{:.1} GB/s", bps / 1_000_000_000.0)
    } else if bps >= 1_000_000.0 {
        format!("{:.1} MB/s", bps / 1_000_000.0)
    } else if bps >= 1_000.0 {
        format!("{:.1} KB/s", bps / 1_000.0)
    } else {
        format!("{:.0} B/s", bps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::traffic_waveform::TrafficSample;

    #[test]
    fn traffic_waveform_projection_peak_and_normalization() {
        let waveform = TrafficWaveformSnapshot {
            generation: 1,
            revision: 1,
            samples: vec![
                TrafficSample {
                    sampled_at_epoch_ms: Some(1000),
                    upload_bps: 1000.0,
                    download_bps: 2000.0,
                },
                TrafficSample {
                    sampled_at_epoch_ms: Some(2000),
                    upload_bps: 5000.0,
                    download_bps: 10000.0,
                },
            ],
        };

        let proj = TrafficWaveformProjection::from_waveform(&waveform, 1048576, 524288);
        assert_eq!(proj.live_down_formatted, "10.0 KB/s");
        assert_eq!(proj.live_up_formatted, "5.0 KB/s");
        assert_eq!(proj.peak_down_formatted, "峰值 10.0 KB/s");
        assert_eq!(proj.peak_up_formatted, "峰值 5.0 KB/s");
        assert_eq!(proj.normalized_down_points, vec![0.2, 1.0]);
        assert_eq!(proj.normalized_up_points, vec![0.2, 1.0]);
    }

    #[test]
    fn memory_info_card_projection_proportions() {
        let proj = MemoryInfoCardProjection::from_bytes(100, 300);
        assert_eq!(proj.app_fraction, 0.25);
        assert_eq!(proj.core_fraction, 0.75);
    }

    #[test]
    fn service_health_matrix_grading() {
        let proj =
            ServiceHealthMatrixProjection::from_latencies(Some(80), Some(200), Some(500), None);
        assert_eq!(proj.services[0].status_role, "good");
        assert_eq!(proj.services[1].status_role, "warning");
        assert_eq!(proj.services[2].status_role, "poor");
        assert_eq!(proj.services[3].status_role, "dim");
    }

    #[test]
    fn quick_switches_projection_states() {
        let switches =
            QuickSwitchesProjection::from_states(&SystemProxyStatus::Enabled, true, true, None);
        assert!(switches.system_proxy_enabled);
        assert_eq!(switches.system_proxy_state_text, "已开启");
        assert!(switches.tun_enabled);
        assert_eq!(switches.tun_state_text, "已开启");
        assert!(!switches.lan_share_enabled);
    }

    #[test]
    fn hero_and_mode_selector_projection_snapshots() {
        use infiltrator_contract::surface::{HostKind, SurfaceKind};
        use lurker_core_contract::error::Failure;
        let mut snap = SurfaceSnapshot::unavailable(
            SurfaceKind::IcedDesktop,
            HostKind::Desktop,
            Failure::unsupported("test"),
        );
        snap.runtime_status.core_process = CoreProcessObservation::Running;
        snap.public_ip.ip = Some("189.24.110.20".to_string());
        snap.public_ip.country_code = Some("DE".to_string());
        snap.public_ip.isp = Some("NetLab Global".to_string());

        let hero = DashboardHeroProjection::from_snapshot(&snap);
        assert!(hero.is_running);
        assert_eq!(hero.status_label, "运行中");
        assert_eq!(hero.status_role, "running");
        assert_eq!(hero.exit_ip, "189.24.110.20");
        assert_eq!(hero.exit_country_code, "DE");
        assert_eq!(hero.organization, "NetLab Global");

        let mode = OutboundModeSelectorProjection::from_snapshot(&snap);
        assert_eq!(mode.selected, OutboundModeChoice::Rule);
        assert!(mode.is_interactive);
    }

    #[test]
    fn profile_quick_card_and_connection_table_projections() {
        use ConnectionSnapshot;
        use infiltrator_contract::surface::{HostKind, SurfaceKind};
        use lurker_core_contract::error::Failure;

        let snap = SurfaceSnapshot::unavailable(
            SurfaceKind::IcedDesktop,
            HostKind::Desktop,
            Failure::unsupported("test"),
        );

        let card = ProfileQuickCardProjection::from_snapshot(&snap);
        assert_eq!(card.profile_name, "默认配置.yaml");
        assert!(card.is_local);

        let conns = vec![ConnectionSnapshot {
            id: "1".into(),
            start: "0".into(),
            destination_host: "google.com".into(),
            host: "google.com".into(),
            process: "".into(),
            rule: "MATCH".into(),
            rule_payload: "".into(),
            chain: "Proxy".into(),
            chains: vec![],
            network: "tcp".into(),
            source_ip: "127.0.0.1".into(),
            source_port: "1234".into(),
            destination_ip: "8.8.8.8".into(),
            destination_port: "443".into(),
            destination_geo_ip: None,
            destination_ip_asn: "".into(),
            rate_observation: Default::default(),
            upload_bps: 0.0,
            download_bps: 0.0,
            upload_total: 1024,
            download_total: 2048,
        }];

        let table = LiveConnectionMiniTableProjection::from_connections(&conns);
        assert_eq!(table.total_active, 1);
        assert_eq!(table.tcp_count, 1);
        assert_eq!(table.udp_count, 0);
        assert_eq!(table.rows[0].destination, "google.com");
        assert_eq!(table.rows[0].network, "TCP");
    }

    #[test]
    fn policy_group_grid_projection_preserves_ids_and_flags() {
        use lurker_core_contract::identity::{
            CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
        };
        use lurker_core_contract::outbound::details::OutboundSnapshot;
        use lurker_core_contract::outbound::identity::SnapshotRevision;
        use lurker_core_contract::outbound::inventory::{
            OutboundInventorySnapshot, OutboundInventoryState, OutboundSelectionSet,
        };

        let engine = EngineId::new("mihomo").expect("test engine ID is valid");
        let scope = CoreScope {
            identity: CoreIdentity::new(
                engine.clone(),
                CoreInstanceId::new("test").expect("test instance ID is valid"),
                None,
            ),
            generation: CoreGeneration::new(1),
            session_token: None,
        };
        let id = |value: &str| OutboundId::new(engine.clone(), value).expect("test outbound ID");
        let entries = [
            ("PROXY", Some("节点选择")),
            ("HK-01", Some("🇭 香港 01")),
            ("JP-02", Some("日本东京 02")),
        ]
        .into_iter()
        .map(|(value, display_name)| OutboundSnapshot {
            id: id(value),
            display_name: display_name.map(str::to_owned),
        })
        .collect();
        let inventory = OutboundInventorySnapshot::new(
            scope,
            SnapshotRevision::new(7),
            OutboundInventoryState::Ready {
                entries,
                collections: vec![OutboundSelectionSet {
                    id: id("PROXY"),
                    member_ids: vec![id("HK-01"), id("JP-02")],
                    availability: SelectionAvailability::Selectable,
                    selection: OutboundSelectionState::Ready {
                        selected: id("HK-01"),
                    },
                }],
            },
        )
        .expect("test inventory is valid");

        let projection = PolicyGroupGridProjection::from_inventory(&inventory);
        assert!(projection.observed);
        assert_eq!(projection.total_count, 1);
        let group = &projection.groups[0];
        assert_eq!(group.name, "节点选择");
        assert_eq!(group.avatar_char, "节");
        assert_eq!(group.selected_flag.as_deref(), Some("🇭🇰"));
        assert_eq!(group.members.len(), 2);
        assert_eq!(group.members[1].display_name, "日本东京 02");
    }
}

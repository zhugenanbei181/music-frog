//! The page-level read model shared by every UI surface.
//!
//! This module deliberately contains product data, not widget data. Iced,
//! Bevy, Compose, Admin and CLI adapters may project these values differently,
//! but they must not invent a second business-facing page state model.

use crate::active_exit::ActiveExitSnapshot;
use crate::aggregator::{AggregationReport, AggregationTemplate};
use crate::android_control::VpnOwnerCoreReadback;
use crate::app_routing::{AppRoutingMode, AppRoutingRule};
use crate::apply_transaction::ApplyTransactionSnapshot;
use crate::capability::CapabilitySnapshot;
use crate::connection::ConnectionRateObservation;
use crate::controller::ControllerAuthSnapshot;
use crate::core_capabilities::CoreCapabilitySnapshot;
use crate::dns::{
    DnsCoreSwitches, DnsEnhancedMode, DnsFakeIpFilterMode, DnsFallbackPolicy, DnsServerTag,
    FakeIpMappingPool,
};
use crate::dns_cache::DnsCacheSnapshot;
use crate::dns_hosts::DnsHostsProfile;
use crate::dns_latency::DnsLatencyReport;
use crate::dns_leak::DnsLeakReport;
use crate::dns_query::DnsQuerySnapshot;
use crate::dns_query_history::DnsQueryHistorySnapshot;
use crate::dns_self_heal::DnsSelfHealSnapshot;
use crate::doctor::{DoctorCheckKind, DoctorStatus};
use crate::ip_quality::IpQualitySnapshot;
use crate::ipv6::Ipv6RoutingSnapshot;
use crate::lan::{LanSecuritySnapshot, LanSharingSnapshot};
use crate::language::LanguageSettingsSnapshot;
use crate::logs::LogStreamState;
use crate::mini_hud::MiniHudPlacement;
use crate::mrs_acceleration::MrsAccelerationSnapshot;
use crate::mtu::MtuNegotiationSnapshot;
use crate::named_override::NamedOverrideCatalog;
use crate::network_roaming::NetworkRoamingSnapshot;
use crate::offline_startup::OfflineStartupSnapshot;
use crate::overview_layout::OverviewLayoutSnapshot;
use crate::pac::PacSnapshot;
use crate::port_conflict::PortConflictSnapshot;
use crate::privileged_network::PrivilegedNetworkSnapshot;
use crate::profile_editor_read::ProfileEditorSnapshot;
use crate::protocol_fidelity::ProtocolStudioSnapshot;
use crate::provider_cache::{KernelEtagSupportSnapshot, RuleProviderCacheSnapshot};
use crate::public_ip::PublicIpProbeSnapshot;
use crate::reconnect_mask::ReconnectMaskSnapshot;
use crate::resources::CoreResourceSnapshot;
use crate::responsive_viewport::ResponsiveViewportSnapshot;
use crate::rule_document::RuleDocumentSnapshot;
use crate::rule_hit_audit::RuleHitAuditSnapshot;
use crate::rule_provider_snapshot::RuleProviderSnapshot;
use crate::rule_snapshot::RuleSnapshot;
use crate::rule_trace_run::RuleTraceExecution;
use crate::rule_tracer::RuleTracerSnapshot;
use crate::rules_workspace::RulesJsonDocumentSnapshot;
use crate::runtime_control::RuntimeControlSnapshot;
use crate::runtime_settings::RuntimeProbeSettingsState;
use crate::runtime_status::RuntimeStatusSnapshot;
use crate::script_export::ScriptExportSnapshot;
use crate::script_sandbox::ScriptSandboxSnapshot;
use crate::service_mode::ServiceModeSnapshot;
use crate::shell_readout::ShellReadoutSnapshot;
use crate::snapshot::CoreSnapshot;
use crate::snapshot_history::SnapshotHistorySnapshot;
use crate::speedtest::SpeedtestSnapshot;
use crate::stun_probe::StunProbeReport;
use crate::subscription_import::SubscriptionFilterDraft;
use crate::subscription_quota::SubscriptionQuotaSnapshot;
use crate::surface::{HostKind, SurfaceKind};
use crate::sync_snapshot::SyncPageSnapshot;
use crate::system_proxy::{SystemProxyRecoverySnapshot, SystemProxySnapshot};
use crate::traffic_history::{ClosedConnectionsSnapshot, TrafficHistorySnapshot};
use crate::traffic_scale::TrafficScaleSnapshot;
use crate::traffic_topology::TrafficTopologySnapshot;
use crate::traffic_waveform::TrafficWaveformSnapshot;
use crate::uwp::UwpLoopbackSnapshot;
use crate::version::CoreVersionSnapshot;
use crate::vpn::VpnSurfaceObservation;
use crate::yaml_ast_diff::YamlAstDiffSnapshot;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::outbound::details::OutboundDetailsSnapshot;
use lurker_core_contract::outbound::inventory::OutboundInventorySnapshot;
use lurker_core_contract::outbound::preferences::OutboundUiPreferencesSnapshot;
use lurker_core_contract::page::{PageData, PageStatus, SurfaceOrigin, outbound_page_status};
use lurker_core_contract::profile_document_identity::ProfileWorkspaceIdentity;
use lurker_core_contract::profile_protection::ProfileWriteProtection;
use lurker_core_contract::routing_intent::RoutingIntentSnapshot;
use serde::{Deserialize, Serialize};

/// Canonical page vocabulary shared by the two primary UI surfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageId {
    Overview,
    Proxies,
    Profiles,
    Rules,
    Connections,
    Logs,
    Dns,
    Doctor,
    AppRouting,
    Sync,
    Settings,
}

impl PageId {
    pub const ALL: [Self; 11] = [
        Self::Overview,
        Self::Proxies,
        Self::Profiles,
        Self::Rules,
        Self::Connections,
        Self::Logs,
        Self::Dns,
        Self::Doctor,
        Self::AppRouting,
        Self::Sync,
        Self::Settings,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Proxies => "proxies",
            Self::Profiles => "profiles",
            Self::Rules => "rules",
            Self::Connections => "connections",
            Self::Logs => "logs",
            Self::Dns => "dns",
            Self::Doctor => "doctor",
            Self::AppRouting => "app_routing",
            Self::Sync => "sync",
            Self::Settings => "settings",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OverviewPageSnapshot {
    pub routing_intent: RoutingIntentSnapshot,
    pub upload_bps: f64,
    pub download_bps: f64,
    pub active_connections: u32,
    pub core_version: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfileSnapshot {
    pub id: String,
    pub name: String,
    pub url: String,
    pub updated_at: String,
    pub upload_bytes: Option<u64>,
    pub download_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub is_active: bool,
    /// Per-profile conditional-request User-Agent (empty = provider default).
    #[serde(default)]
    pub user_agent: String,
    /// Per-profile TLS certificate-skip preference.
    #[serde(default)]
    pub insecure_skip_verify: bool,
    /// Cached `ETag` validator from the last successful download.
    #[serde(default)]
    pub etag: Option<String>,
    /// Cached `Last-Modified` validator from the last successful download.
    #[serde(default)]
    pub last_modified: Option<String>,
    /// DUAL-07-13: a transient pre-save `.bak` copy exists and can be restored.
    #[serde(default)]
    pub has_backup: bool,
    /// DUAL-07-03: the profile's cron schedule (empty = interval/manual).
    #[serde(default)]
    pub cron_expression: Option<String>,
    /// DUAL-07-14: whether the profile participates in scheduled auto-updates.
    #[serde(default)]
    pub auto_update_enabled: bool,
    /// DUAL-07-14: the fixed update interval in hours (Cron-only profiles
    /// honestly report `None`).
    #[serde(default)]
    pub update_interval_hours: Option<u32>,
    /// DUAL-07-14: the next scheduled update instant, RFC3339, when known.
    #[serde(default)]
    pub next_update: Option<String>,
    /// DUAL-07-09: whether a successful update of this profile is applied to
    /// the running core. The profile metadata default is `true`.
    #[serde(default = "default_auto_reload_core")]
    pub auto_reload_core: bool,
    /// DUAL-07-08: the profile's stored node-keyword filter draft.
    #[serde(default)]
    pub filter: SubscriptionFilterDraft,
    #[serde(default = "unobserved_filter_source")]
    pub filter_source: Result<ProfileWorkspaceIdentity, Failure>,
    /// DUAL-09-12: direct-edit protection derived from the subscription
    /// source. Both surfaces render the same classification.
    #[serde(default)]
    pub write_protection: ProfileWriteProtection,
}

/// DUAL-07-09: the profile metadata default for the auto-reload preference.
fn default_auto_reload_core() -> bool {
    true
}
fn unobserved_filter_source() -> Result<ProfileWorkspaceIdentity, Failure> {
    Err(Failure::new(
        ErrorCode::NotReady,
        "Profile filter source has not been observed",
        true,
    ))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfilesPageSnapshot {
    pub profiles: Vec<ProfileSnapshot>,
    pub auto_update_interval_hours: u32,
    pub updating: bool,
    /// DUAL-08: the last shared aggregation preview, or `None` when no draft
    /// has been previewed in this process yet.
    #[serde(default)]
    pub aggregation: Option<AggregationReport>,
    /// DUAL-08-13: persisted aggregation template library.
    #[serde(default)]
    pub aggregation_templates: Vec<AggregationTemplate>,
    /// Whether the profile store exposed the template sidecar. `false` means
    /// the host keeps no template library (typed unsupported) or the read
    /// failed — surfaces must not render that as "no templates".
    #[serde(default)]
    pub aggregation_templates_available: bool,
    /// DUAL-09-06/07: the active profile's snapshot history plus the shared
    /// prune view. `None` means no history has been loaded in this process yet.
    #[serde(default)]
    pub snapshot_history: Option<SnapshotHistorySnapshot>,
    /// DUAL-09-11: the last apply transaction of the host core. `None` means no
    /// transaction has run in this process yet — surfaces must not claim the
    /// config is verified.
    #[serde(default)]
    pub apply_transaction: Option<ApplyTransactionSnapshot>,
    /// SLINT-004: Shared named YAML overrides catalog.
    #[serde(default)]
    pub named_overrides: NamedOverrideCatalog,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RulesPageSnapshot {
    /// Complete editable source; the rendered `rules` window is never a save payload.
    #[serde(default)]
    pub document: Option<RuleDocumentSnapshot>,
    /// DUAL-11-08: total rules present in the active profile, *before* the
    /// honest publish cap. `rules.len()` is the published window and may be
    /// smaller; both facts are published so a surface never reports a truncated
    /// list as if it were complete.
    pub total_rules: usize,
    pub default_action: String,
    pub providers: Vec<RuleProviderSnapshot>,
    pub rules: Vec<RuleSnapshot>,
    #[serde(default)]
    pub tracer: RuleTracerSnapshot,
    #[serde(default)]
    pub mrs_acceleration: MrsAccelerationSnapshot,
    #[serde(default)]
    pub hit_audit: Option<RuleHitAuditSnapshot>,
    /// DUAL-11-08: cap the publisher applied to `rules` (0 = uncapped).
    #[serde(default)]
    pub rule_publish_limit: usize,
    /// DUAL-11-07: the observed kernel rule-provider cache location.
    #[serde(default)]
    pub provider_cache: RuleProviderCacheSnapshot,
    /// DUAL-11-05: the kernel's real `etag-support` capability declared by the
    /// active profile (a top-level key; mihomo defaults it to `true`). The
    /// client publishes the declaration, never the per-request `304` outcome,
    /// which stays inside the kernel.
    #[serde(default)]
    pub etag_support: KernelEtagSupportSnapshot,
    /// DUAL-11-14: the rules-workspace JSON documents, serialised from the same
    /// active profile the Iced JSON editors load through their ports. Empty on
    /// hosts without a configuration application.
    #[serde(default)]
    pub json_documents: Vec<RulesJsonDocumentSnapshot>,
}

impl RulesPageSnapshot {
    /// DUAL-11-08: rules the publish cap dropped from the rendered list.
    pub fn omitted_rule_count(&self) -> usize {
        self.total_rules.saturating_sub(self.rules.len())
    }

    /// DUAL-11-08: whether the published rule list is a truncated view of the
    /// profile's rule list.
    pub fn is_truncated(&self) -> bool {
        self.omitted_rule_count() > 0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnectionSnapshot {
    pub id: String,
    /// Controller start timestamp; empty means it was not observed.
    #[serde(default)]
    pub start: String,
    #[serde(default)]
    pub destination_host: String,
    pub host: String,
    pub process: String,
    pub rule: String,
    /// DUAL-13-14: the matched rule payload the detail drawer renders.
    #[serde(default)]
    pub rule_payload: String,
    /// Joined route chain retained for compact fallback rendering.
    pub chain: String,
    /// DUAL-13-06: the parsed route chain, one hop per entry, from the matched
    /// inbound rule through each policy group to the outbound. Empty when the
    /// core did not report a chain.
    #[serde(default)]
    pub chains: Vec<String>,
    /// DUAL-13-14: transport label (`tcp`/`udp`) as the core reported it.
    #[serde(default)]
    pub network: String,
    /// DUAL-13-14: local endpoint of the connection.
    #[serde(default)]
    pub source_ip: String,
    #[serde(default)]
    pub source_port: String,
    /// DUAL-13-14: remote endpoint of the connection.
    #[serde(default)]
    pub destination_ip: String,
    #[serde(default)]
    pub destination_port: String,
    /// DUAL-13-05: the kernel's GEOIP rule-evaluation result for the
    /// destination IP (`/connections` `metadata.destinationGeoIP`). `None` =
    /// the kernel never queried; `Some(vec![])` = queried with no record;
    /// `Some(codes)` = real kernel-resolved codes. The client reads no MMDB
    /// itself and never substitutes a location.
    #[serde(default)]
    pub destination_geo_ip: Option<Vec<String>>,
    /// DUAL-13-05: the kernel's raw `destinationIPASN` value (e.g.
    /// `15169 Google LLC`). `""` = no IP-ASN rule ran; whitespace-only =
    /// evaluated with no record; the surfaces classify it through the shared
    /// `destination_asn_fact` reduction and never invent an ASN.
    #[serde(default)]
    pub destination_ip_asn: String,
    #[serde(default)]
    pub rate_observation: ConnectionRateObservation,
    pub upload_bps: f64,
    pub download_bps: f64,
    pub upload_total: u64,
    pub download_total: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnectionsPageSnapshot {
    pub total_connections: usize,
    pub total_upload_bytes: u64,
    pub total_download_bytes: u64,
    pub connections: Vec<ConnectionSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogSnapshot {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub raw: Option<String>,
    pub timestamp: String,
    pub level: String,
    pub tag: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogsPageSnapshot {
    #[serde(default)]
    pub stream: LogStreamState,
    pub total_entries: usize,
    pub active_level: Option<String>,
    pub entries: Vec<LogSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsServerSnapshot {
    pub address: String,
    pub protocol: String,
    pub latency_ms: Option<u32>,
    pub is_fallback: bool,
    #[serde(default)]
    pub tags: Vec<DnsServerTag>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsPageSnapshot {
    /// Domain mapping mode (`dns.enhanced-mode`).
    #[serde(default)]
    pub enhanced_mode: DnsEnhancedMode,
    pub cache_entries: usize,
    pub fake_ip_range: String,
    pub servers: Vec<DnsServerSnapshot>,
    /// The six system-level switches of the DNS workbench form.
    #[serde(default)]
    pub switches: DnsCoreSwitches,
    /// Fake-IP filter mode (`dns.fake-ip-filter-mode`).
    #[serde(default)]
    pub filter_mode: DnsFakeIpFilterMode,
    /// Tier-1 bootstrap resolvers (`dns.default-nameserver`).
    #[serde(default)]
    pub default_nameserver: Vec<String>,
    /// Fallback resolver policy (`dns.fallback-filter`).
    #[serde(default)]
    pub fallback_policy: DnsFallbackPolicy,
    /// `dns.fake-ip-filter` patterns or rules.
    #[serde(default)]
    pub fake_ip_filter: Vec<String>,
    /// `dns.proxy-server-nameserver` resolvers.
    #[serde(default)]
    pub proxy_server_nameserver: Vec<String>,
    /// `dns.direct-nameserver` resolvers.
    #[serde(default)]
    pub direct_nameserver: Vec<String>,
    /// DUAL-14-06: the observed Fake-IP bindings published to both surfaces.
    #[serde(default)]
    pub fake_ip_pool: FakeIpMappingPool,
    /// DUAL-14-10: the last real per-nameserver latency probe of this host.
    #[serde(default)]
    pub latency: DnsLatencyReport,
    /// DUAL-14-09 (re-scoped): the last real STUN UDP-egress probe of this
    /// host/process, compared against the expected proxied egress. This is the
    /// host's own UDP mapping, not a browser WebRTC result.
    #[serde(default)]
    pub stun: StunProbeReport,
    /// DUAL-14-13: the shared DNS self-heal observation.
    #[serde(default)]
    pub self_heal: DnsSelfHealSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorCheckSnapshot {
    #[serde(default)]
    pub kind: Option<DoctorCheckKind>,
    #[serde(default)]
    pub detail_copy_key: Option<String>,
    pub id: String,
    pub name: String,
    pub category: String,
    pub state: DoctorStatus,
    pub detail: String,
    pub fix_available: bool,
    #[serde(default)]
    pub hint: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorPageSnapshot {
    pub overall_healthy: bool,
    pub last_run: String,
    pub checks: Vec<DoctorCheckSnapshot>,
    #[serde(default)]
    pub report_started_at: Option<u64>,
    #[serde(default)]
    pub report_finished_at: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSnapshot {
    pub id: String,
    pub name: String,
    pub process_name: String,
    pub rule: AppRoutingRule,
    pub is_system: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppRoutingPageSnapshot {
    pub mode: AppRoutingMode,
    pub include_system: bool,
    pub apps: Vec<AppSnapshot>,
    #[serde(default)]
    pub uwp_loopback: UwpLoopbackSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsPageSnapshot {
    #[serde(default)]
    pub close_to_tray: Option<bool>,
    #[serde(default)]
    pub notifications_enabled: Option<bool>,
    #[serde(default = "default_language_code")]
    pub language: String,
    #[serde(default = "default_theme_preference")]
    pub theme: String,
    pub autostart: bool,
    pub system_proxy: bool,
    /// The confirmed LAN sharing configuration. `None` means the owner never
    /// observed it — not that the sharing is off.
    #[serde(default)]
    pub lan_sharing: Option<LanSharingSnapshot>,
    #[serde(default)]
    pub lan_security: Option<LanSecuritySnapshot>,
    #[serde(default)]
    pub ipv6_routing: Option<Ipv6RoutingSnapshot>,
    #[serde(default)]
    pub pac: PacSnapshot,
    pub tun_enabled: Option<bool>,
    pub tun_stack: Option<String>,
    #[serde(default)]
    pub tun_auto_route: Option<bool>,
    #[serde(default)]
    pub tun_strict_route: Option<bool>,
    pub controller_port: Option<u16>,
    pub log_level: Option<String>,
    #[serde(default)]
    pub core_channel: String,
    /// Persisted Mini HUD placement (shared geometry, DUAL-15-04). Carried in
    /// the settings page snapshot so both surfaces read one placement.
    #[serde(default)]
    pub mini_hud: MiniHudPlacement,
    /// Refresh the active profile on application startup.
    #[serde(default)]
    pub refresh_current_profile_on_start: bool,
    /// Custom accent color hex code (e.g. "#016888").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_accent_color: Option<String>,
}

fn default_language_code() -> String {
    "zh-CN".into()
}

fn default_theme_preference() -> String {
    "system".into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurfacePages {
    pub overview: PageData<OverviewPageSnapshot>,
    pub profiles: PageData<ProfilesPageSnapshot>,
    pub rules: PageData<RulesPageSnapshot>,
    pub connections: PageData<ConnectionsPageSnapshot>,
    pub logs: PageData<LogsPageSnapshot>,
    pub dns: PageData<DnsPageSnapshot>,
    pub doctor: PageData<DoctorPageSnapshot>,
    pub app_routing: PageData<AppRoutingPageSnapshot>,
    pub sync: PageData<SyncPageSnapshot>,
    pub settings: PageData<SettingsPageSnapshot>,
}

impl SurfacePages {
    pub fn unavailable(failure: Failure) -> Self {
        Self {
            overview: PageData::unavailable(failure.clone()),
            profiles: PageData::unavailable(failure.clone()),
            rules: PageData::unavailable(failure.clone()),
            connections: PageData::unavailable(failure.clone()),
            logs: PageData::unavailable(failure.clone()),
            dns: PageData::unavailable(failure.clone()),
            doctor: PageData::unavailable(failure.clone()),
            app_routing: PageData::unavailable(failure.clone()),
            sync: PageData::unavailable(failure.clone()),
            settings: PageData::unavailable(failure),
        }
    }

    fn status(&self, page: PageId) -> Option<&PageStatus> {
        match page {
            PageId::Overview => Some(&self.overview.status),
            PageId::Proxies => None,
            PageId::Profiles => Some(&self.profiles.status),
            PageId::Rules => Some(&self.rules.status),
            PageId::Connections => Some(&self.connections.status),
            PageId::Logs => Some(&self.logs.status),
            PageId::Dns => Some(&self.dns.status),
            PageId::Doctor => Some(&self.doctor.status),
            PageId::AppRouting => Some(&self.app_routing.status),
            PageId::Sync => Some(&self.sync.status),
            PageId::Settings => Some(&self.settings.status),
        }
    }
}

pub fn unobserved_hosts() -> PageData<DnsHostsProfile> {
    PageData::unavailable(Failure::unsupported(
        "no profile-backed Hosts reader is configured",
    ))
}

/// One canonical read model consumed by every inbound surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurfaceSnapshot {
    #[serde(default)]
    pub runtime_control: RuntimeControlSnapshot,
    /// Derived cross-surface status axes. Consumers render this projection
    /// instead of inferring product health from one Core/VPN boolean.
    #[serde(default)]
    pub runtime_status: RuntimeStatusSnapshot,
    #[serde(default)]
    pub shell_readout: ShellReadoutSnapshot,
    #[serde(default)]
    pub probe_settings: RuntimeProbeSettingsState,
    #[serde(default)]
    pub language_settings: LanguageSettingsSnapshot,
    /// Independent echo observation stays available when DNS configuration reads fail.
    #[serde(default)]
    pub dns_leak: DnsLeakReport,
    /// Independent flush outcomes survive unavailable DNS configuration.
    #[serde(default = "DnsCacheSnapshot::unavailable")]
    pub dns_cache: DnsCacheSnapshot,
    #[serde(default = "DnsQuerySnapshot::unavailable")]
    pub dns_query: DnsQuerySnapshot,
    #[serde(default)]
    pub dns_query_history: DnsQueryHistorySnapshot,
    /// Simulation results survive unrelated page/provider read failures.
    #[serde(default)]
    pub rule_trace: RuleTraceExecution,
    /// Profile-backed root Hosts have independent provenance from runtime DNS fallback.
    #[serde(default = "unobserved_hosts")]
    pub dns_hosts: PageData<DnsHostsProfile>,
    pub surface: SurfaceKind,
    pub origin: SurfaceOrigin,
    pub generation: u64,
    pub revision: u64,
    pub core: CoreSnapshot,
    pub capabilities: CapabilitySnapshot,
    /// Engine/version/host/adapter capability facts authored by the Core owner.
    pub core_capability_snapshot: PageData<CoreCapabilitySnapshot>,
    pub failure: Option<Failure>,
    pub pages: SurfacePages,
    /// Shared custom-node protocol studio with an explicit observation state.
    pub protocol_studio: PageData<ProtocolStudioSnapshot>,
    /// Independent online core-channel probe results shared by both UIs.
    pub versions: CoreVersionSnapshot,
    /// Controller authentication status without exposing the secret.
    #[serde(default)]
    pub controller_auth: ControllerAuthSnapshot,
    /// Dynamic host privilege/service status for TUN ownership.
    #[serde(default)]
    pub service_mode: ServiceModeSnapshot,
    /// Current binding observations for the mixed proxy and controller ports.
    #[serde(default)]
    pub port_conflicts: PortConflictSnapshot,
    /// Current core memory/CPU observation and soft-quota GC state.
    #[serde(default)]
    pub resources: CoreResourceSnapshot,
    /// Local-only startup proof; remote enhancement remains optional.
    #[serde(default)]
    pub offline_startup: OfflineStartupSnapshot,
    /// Physical-to-TUN MTU negotiation result.
    #[serde(default)]
    pub mtu: MtuNegotiationSnapshot,
    /// Host system HTTP/SOCKS proxy state.
    #[serde(default)]
    pub system_proxy: SystemProxySnapshot,
    /// Startup recovery result for an orphaned system-proxy journal.
    #[serde(default)]
    pub system_proxy_recovery: SystemProxyRecoverySnapshot,
    /// Physical-link/default-gateway observation and TUN route recovery.
    #[serde(default)]
    pub network_roaming: NetworkRoamingSnapshot,
    /// Exact `:vpn` owner readback for Android; other hosts report a typed
    /// unsupported/not-applicable/unavailable disposition.
    #[serde(default)]
    pub vpn: VpnSurfaceObservation,
    /// Bounded live upload/download samples shared by both primary surfaces.
    #[serde(default)]
    pub traffic_waveform: TrafficWaveformSnapshot,
    /// Rolling 7-day traffic usage history and charts (SLINT-007).
    #[serde(default)]
    pub traffic_history: TrafficHistorySnapshot,
    /// Closed connection history and telemetry records (SLINT-007).
    #[serde(default)]
    pub closed_connections: ClosedConnectionsSnapshot,
    /// Dynamic shared max/unit/tick scale for the traffic waveform.
    #[serde(default)]
    pub traffic_scale: TrafficScaleSnapshot,
    /// Live routing chain derived from controller connections/configuration.
    #[serde(default)]
    pub traffic_topology: TrafficTopologySnapshot,
    /// Current selected proxy-group outbound node and its available facts.
    #[serde(default)]
    pub active_exit: ActiveExitSnapshot,
    /// Engine-neutral outbound inventory observation, fenced to its core scope.
    pub outbound_inventory: PageData<OutboundInventorySnapshot>,
    /// Neutral protocol and health facts aligned with the observed inventory revision.
    pub outbound_details: PageData<OutboundDetailsSnapshot>,
    /// Typed engine-scoped outbound presentation preferences, owned and
    /// revisioned by the active Core application.
    pub outbound_ui_preferences: PageData<OutboundUiPreferencesSnapshot>,
    #[serde(default)]
    pub public_ip: PublicIpProbeSnapshot,
    #[serde(default)]
    pub ip_quality: IpQualitySnapshot,
    #[serde(default)]
    pub overview_layout: OverviewLayoutSnapshot,
    #[serde(default)]
    pub reconnect_mask: ReconnectMaskSnapshot,
    #[serde(default)]
    pub viewport: ResponsiveViewportSnapshot,
    /// Current active subscription usage and expiry facts.
    #[serde(default)]
    pub subscription_quota: SubscriptionQuotaSnapshot,
    #[serde(default)]
    pub profile_editor: ProfileEditorSnapshot,
    /// Host-injected privileged-network regression readback.
    #[serde(default)]
    pub privileged_network: PrivilegedNetworkSnapshot,
    /// AST configuration diff and snapshot comparison readback.
    #[serde(default)]
    pub yaml_ast_diff: Option<YamlAstDiffSnapshot>,
    /// DUAL-10-05/14: the shared directive-DSL sandbox console read model published
    /// by `ScriptApplication` (no JavaScript engine is bundled).
    #[serde(default)]
    pub script_sandbox: Option<ScriptSandboxSnapshot>,
    /// DUAL-10-12: the shared export read model published by
    /// `ScriptExportApplication` (real file name/bytes/checksum + host outcome).
    #[serde(default)]
    pub script_export: Option<ScriptExportSnapshot>,
    /// Shared concurrent speedtest, jitter, and packet loss telemetry.
    #[serde(default)]
    pub speedtest: SpeedtestSnapshot,
}

/// Surface-level event vocabulary. Toolkit adapters may translate this into
/// an Iced message, Bevy trigger, Compose state update, or REST stream item.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SurfaceEvent {
    SnapshotUpdated(SurfaceSnapshot),
}

#[path = "surface_snapshot/validation.rs"]
mod validation;

impl SurfaceSnapshot {
    pub fn unavailable(surface: SurfaceKind, host: HostKind, failure: Failure) -> Self {
        Self {
            shell_readout: Default::default(),
            runtime_control: Default::default(),
            runtime_status: RuntimeStatusSnapshot::default(),
            probe_settings: Default::default(),
            language_settings: Default::default(),
            dns_leak: Default::default(),
            dns_cache: DnsCacheSnapshot::unavailable(),
            dns_query: DnsQuerySnapshot::unavailable(),
            dns_query_history: DnsQueryHistorySnapshot::default(),
            rule_trace: RuleTraceExecution::default(),
            dns_hosts: unobserved_hosts(),
            surface,
            origin: SurfaceOrigin::Live,
            generation: 0,
            revision: 0,
            core: CoreSnapshot {
                identity: None,
                lifecycle: CoreLifecycle::Starting,
                generation: 0,
                session_token: None,
                revision: 0,
                routing_intent: RoutingIntentSnapshot::default(),
                core_version: None,
                sampled_at_epoch_ms: None,
                failure: Some(failure.clone()),
                upload_bps: 0.0,
                download_bps: 0.0,
                active_connections: 0,
                watchdog: Default::default(),
            },
            capabilities: CapabilitySnapshot::new(host, 0, Vec::new()),
            core_capability_snapshot: PageData::unavailable(Failure::new(
                ErrorCode::NotReady,
                "core capability matrix has not been observed",
                true,
            )),
            failure: Some(failure.clone()),
            pages: SurfacePages::unavailable(failure.clone()),
            protocol_studio: PageData::unavailable(Failure::new(
                ErrorCode::NotReady,
                "protocol studio has not been observed",
                true,
            )),
            versions: CoreVersionSnapshot::unavailable(),
            controller_auth: ControllerAuthSnapshot::default(),
            service_mode: ServiceModeSnapshot::default(),
            port_conflicts: PortConflictSnapshot::default(),
            resources: CoreResourceSnapshot::default(),
            offline_startup: OfflineStartupSnapshot::default(),
            mtu: MtuNegotiationSnapshot::default(),
            system_proxy: SystemProxySnapshot::default(),
            system_proxy_recovery: SystemProxyRecoverySnapshot::default(),
            network_roaming: NetworkRoamingSnapshot::default(),
            vpn: VpnSurfaceObservation::NotApplicable,
            privileged_network: PrivilegedNetworkSnapshot::default(),
            traffic_waveform: TrafficWaveformSnapshot::default(),
            traffic_history: Default::default(),
            closed_connections: Default::default(),
            traffic_scale: TrafficScaleSnapshot::default(),
            traffic_topology: TrafficTopologySnapshot::default(),
            active_exit: ActiveExitSnapshot::default(),
            outbound_inventory: PageData::unavailable(failure.clone()),
            outbound_details: PageData::unavailable(failure.clone()),
            outbound_ui_preferences: PageData::unavailable(Failure::unsupported(
                "engine-scoped outbound UI preferences are not composed",
            )),
            public_ip: PublicIpProbeSnapshot::default(),
            ip_quality: IpQualitySnapshot::default(),
            overview_layout: OverviewLayoutSnapshot::default(),
            reconnect_mask: ReconnectMaskSnapshot::default(),
            viewport: ResponsiveViewportSnapshot::default(),
            subscription_quota: SubscriptionQuotaSnapshot::default(),
            profile_editor: ProfileEditorSnapshot::default(),
            yaml_ast_diff: None,
            script_sandbox: None,
            script_export: None,
            speedtest: SpeedtestSnapshot::default(),
        }
    }

    pub fn page_status(&self, page: PageId) -> PageStatus {
        if page == PageId::Proxies {
            return outbound_page_status(&self.outbound_inventory);
        }
        self.pages
            .status(page)
            .expect("non-proxy page has an owning page status")
            .clone()
    }

    /// Whether this snapshot belongs to a newer, still-valid core session.
    /// Generation orders restarts; the token fences delayed events from an
    /// older session even when their transport revision happens to be larger.
    pub fn is_newer_than(&self, current: &Self) -> bool {
        let next_scope = self.core.scope();
        let current_scope = current.core.scope();
        match (&next_scope, &current_scope) {
            (Some(next), Some(previous))
                if next.identity.engine != previous.identity.engine
                    || next.identity.instance != previous.identity.instance =>
            {
                return false;
            }
            (Some(next), Some(previous))
                if next.identity.version != previous.identity.version
                    && next.generation <= previous.generation =>
            {
                return false;
            }
            // A surface may start with an unavailable snapshot before the
            // composition has published its core identity. Once an identity
            // is known, an unscoped snapshot cannot replace it.
            (None, Some(_)) => return false,
            _ => {}
        }
        if let (Some(next), Some(previous)) = (&next_scope, &current_scope) {
            if next.generation != previous.generation {
                return next.generation > previous.generation;
            }
        } else if self.core.generation != current.core.generation {
            return self.core.generation > current.core.generation;
        }
        match (self.core.session_token, current.core.session_token) {
            (Some(_), None) => return true,
            // A stopped snapshot has no active token but is still the valid
            // terminal state of the current generation.
            (None, Some(_)) => {
                return self.revision > current.revision || self.generation > current.generation;
            }
            (Some(next), Some(previous)) if next != previous => return false,
            _ => {}
        }
        if self.generation != current.generation {
            return self.generation > current.generation;
        }
        self.revision > current.revision
    }
}

#[cfg(test)]
#[path = "surface_snapshot_tests.rs"]
mod tests;

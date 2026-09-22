//! The page-level read model shared by every UI surface.
//!
//! This module deliberately contains product data, not widget data. Iced,
//! Bevy, Compose, Admin and CLI adapters may project these values differently,
//! but they must not invent a second business-facing page state model.

use crate::capability::CapabilitySnapshot;
use crate::command::ProxyMode;
use crate::controller::ControllerAuthSnapshot;
use crate::error::Failure;
use crate::lan::LanSecuritySnapshot;
use crate::mtu::MtuNegotiationSnapshot;
use crate::offline_startup::OfflineStartupSnapshot;
use crate::port_conflict::PortConflictSnapshot;
use crate::resources::CoreResourceSnapshot;
use crate::service_mode::ServiceModeSnapshot;
use crate::snapshot::{CoreLifecycle, CoreSnapshot};
use crate::surface::{HostKind, SurfaceKind};
use crate::system_proxy::SystemProxyRecoverySnapshot;
use crate::system_proxy::SystemProxySnapshot;
use crate::version::CoreVersionSnapshot;
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

/// State of one page read model. `Unavailable` is different from an empty
/// result: a surface must tell the user why a host capability is absent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageStatus {
    Loading,
    Ready,
    Empty,
    Unavailable { failure: Failure },
    Failed { failure: Failure },
}

/// Whether a surface snapshot is an explicit fixture or a host/application
/// observation. This is data, not something a UI may infer from empty fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceOrigin {
    Demo,
    Live,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageData<T> {
    pub status: PageStatus,
    pub data: Option<T>,
}

impl<T> PageData<T> {
    pub fn loading() -> Self {
        Self {
            status: PageStatus::Loading,
            data: None,
        }
    }

    pub fn ready(data: T) -> Self {
        Self {
            status: PageStatus::Ready,
            data: Some(data),
        }
    }

    pub fn empty(data: T) -> Self {
        Self {
            status: PageStatus::Empty,
            data: Some(data),
        }
    }

    pub fn unavailable(failure: Failure) -> Self {
        Self {
            status: PageStatus::Unavailable { failure },
            data: None,
        }
    }

    pub fn failed(failure: Failure) -> Self {
        Self {
            status: PageStatus::Failed { failure },
            data: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OverviewPageSnapshot {
    pub proxy_mode: Option<ProxyMode>,
    pub upload_bps: f64,
    pub download_bps: f64,
    pub active_connections: u32,
    pub memory_bytes: Option<u64>,
    pub core_version: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProxyNodeSnapshot {
    pub name: String,
    pub node_type: String,
    pub delay_ms: Option<u32>,
    pub selected: bool,
    pub favorite: bool,
    pub features: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProxyGroupSnapshot {
    pub name: String,
    pub group_type: String,
    #[serde(default)]
    pub classification: Option<crate::proxies::ProxyGroupClassification>,
    pub current: String,
    pub expanded: bool,
    pub proxies: Vec<ProxyNodeSnapshot>,
}

impl ProxyGroupSnapshot {
    pub fn resolved_classification(&self) -> crate::proxies::ProxyGroupClassification {
        self.classification
            .or_else(|| crate::proxies::ProxyGroupClassification::from_str_loose(&self.group_type))
            .unwrap_or(crate::proxies::ProxyGroupClassification::Selector)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProxiesPageSnapshot {
    pub groups: Vec<ProxyGroupSnapshot>,
    pub testing: bool,
    pub active_exit: String,
    #[serde(default)]
    pub filter_alive: crate::proxies::ProxyFilterAliveSnapshot,
    #[serde(default)]
    pub sort_order: crate::proxies::ProxySortOrder,
    #[serde(default)]
    pub compact_view: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfileSnapshot {
    pub id: String,
    pub name: String,
    pub url: String,
    pub updated_at: String,
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub total_bytes: u64,
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
    /// DUAL-07-08: the profile's stored node-keyword filter draft.
    #[serde(default)]
    pub filter: crate::subscription_import::SubscriptionFilterDraft,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfilesPageSnapshot {
    pub profiles: Vec<ProfileSnapshot>,
    pub auto_update_interval_hours: u32,
    pub updating: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RuleSnapshot {
    pub id: usize,
    pub rule_type: String,
    pub payload: String,
    pub proxy: String,
    pub hit_count: u64,
    #[serde(default)]
    pub is_enabled: bool,
    #[serde(default)]
    pub no_resolve: bool,
    #[serde(default)]
    pub last_hit_secs: Option<u64>,
    #[serde(default)]
    pub is_shadowed: bool,
    #[serde(default)]
    pub shadow_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuleProviderSnapshot {
    pub name: String,
    pub rule_count: usize,
    pub behavior: String,
    pub updated_at: String,
    /// DUAL-11-04: the `rule-providers` source URL declared in the active
    /// profile, when the provider is config-backed (runtime-only providers
    /// honestly report `None` instead of a fabricated address).
    #[serde(default)]
    pub source_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RulesPageSnapshot {
    pub total_rules: usize,
    pub default_action: String,
    pub providers: Vec<RuleProviderSnapshot>,
    pub rules: Vec<RuleSnapshot>,
    #[serde(default)]
    pub tracer: crate::rule_tracer::RuleTracerSnapshot,
    #[serde(default)]
    pub mrs_acceleration: crate::mrs_acceleration::MrsAccelerationSnapshot,
    #[serde(default)]
    pub total_hits: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnectionSnapshot {
    pub id: String,
    pub host: String,
    pub process: String,
    pub rule: String,
    /// Joined route chain retained for compact fallback rendering.
    pub chain: String,
    /// DUAL-13-06: the parsed route chain, one hop per entry, from the matched
    /// inbound rule through each policy group to the outbound. Empty when the
    /// core did not report a chain.
    #[serde(default)]
    pub chains: Vec<String>,
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
    pub timestamp: String,
    pub level: String,
    pub tag: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogsPageSnapshot {
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
    pub tags: Vec<crate::dns::DnsServerTag>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsPageSnapshot {
    /// Domain mapping mode (`dns.enhanced-mode`).
    #[serde(default)]
    pub enhanced_mode: crate::dns::DnsEnhancedMode,
    pub cache_entries: usize,
    pub fake_ip_range: String,
    pub servers: Vec<DnsServerSnapshot>,
    /// The six system-level switches of the DNS workbench form.
    #[serde(default)]
    pub switches: crate::dns::DnsCoreSwitches,
    /// Fake-IP filter mode (`dns.fake-ip-filter-mode`).
    #[serde(default)]
    pub filter_mode: crate::dns::DnsFakeIpFilterMode,
    /// Tier-1 bootstrap resolvers (`dns.default-nameserver`).
    #[serde(default)]
    pub default_nameserver: Vec<String>,
    /// Fallback resolver policy (`dns.fallback-filter`).
    #[serde(default)]
    pub fallback_policy: crate::dns::DnsFallbackPolicy,
    /// `dns.fake-ip-filter` patterns or rules.
    #[serde(default)]
    pub fake_ip_filter: Vec<String>,
    /// `dns.proxy-server-nameserver` resolvers.
    #[serde(default)]
    pub proxy_server_nameserver: Vec<String>,
    /// `dns.direct-nameserver` resolvers.
    #[serde(default)]
    pub direct_nameserver: Vec<String>,
    /// Honest per-target report of the last DNS cache flush.
    #[serde(default)]
    pub cache_flush: crate::dns::DnsCacheFlushReport,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorCheckSnapshot {
    pub id: String,
    pub name: String,
    pub category: String,
    pub state: String,
    pub detail: String,
    pub fix_available: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorPageSnapshot {
    pub overall_healthy: bool,
    pub last_run: String,
    pub checks: Vec<DoctorCheckSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSnapshot {
    pub id: String,
    pub name: String,
    pub process_name: String,
    pub rule: String,
    pub is_system: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppRoutingPageSnapshot {
    pub mode: String,
    pub include_system: bool,
    pub apps: Vec<AppSnapshot>,
    #[serde(default)]
    pub uwp_loopback: crate::uwp::UwpLoopbackSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncConflictSnapshot {
    pub remote_device: String,
    pub conflict_time: String,
    pub conflicting_keys: Vec<(String, String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotItemSnapshot {
    pub id: String,
    pub timestamp: String,
    pub device: String,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncPageSnapshot {
    pub status: String,
    pub server_url: String,
    pub username: String,
    pub last_sync: Option<String>,
    pub auto_sync: bool,
    pub conflict: Option<SyncConflictSnapshot>,
    pub snapshots: Vec<SnapshotItemSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsPageSnapshot {
    pub autostart: bool,
    pub system_proxy: bool,
    pub mixed_port: u16,
    pub allow_lan: bool,
    #[serde(default = "default_lan_bind_address")]
    pub lan_bind_address: String,
    #[serde(default)]
    pub lan_security: LanSecuritySnapshot,
    #[serde(default)]
    pub ipv6_routing: crate::ipv6::Ipv6RoutingSnapshot,
    #[serde(default)]
    pub pac: crate::pac::PacSnapshot,
    pub tun_enabled: bool,
    pub tun_stack: String,
    #[serde(default)]
    pub tun_auto_route: bool,
    #[serde(default)]
    pub tun_strict_route: bool,
    pub controller_port: u16,
    pub log_level: String,
    #[serde(default)]
    pub core_channel: String,
}

fn default_lan_bind_address() -> String {
    crate::lan::DEFAULT_BIND_ADDRESS.to_owned()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurfacePages {
    pub overview: PageData<OverviewPageSnapshot>,
    pub proxies: PageData<ProxiesPageSnapshot>,
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
            proxies: PageData::unavailable(failure.clone()),
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

    pub fn status(&self, page: PageId) -> &PageStatus {
        match page {
            PageId::Overview => &self.overview.status,
            PageId::Proxies => &self.proxies.status,
            PageId::Profiles => &self.profiles.status,
            PageId::Rules => &self.rules.status,
            PageId::Connections => &self.connections.status,
            PageId::Logs => &self.logs.status,
            PageId::Dns => &self.dns.status,
            PageId::Doctor => &self.doctor.status,
            PageId::AppRouting => &self.app_routing.status,
            PageId::Sync => &self.sync.status,
            PageId::Settings => &self.settings.status,
        }
    }
}

/// One canonical read model consumed by every inbound surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurfaceSnapshot {
    pub surface: SurfaceKind,
    pub origin: SurfaceOrigin,
    pub generation: u64,
    pub revision: u64,
    pub core: CoreSnapshot,
    pub capabilities: CapabilitySnapshot,
    pub failure: Option<Failure>,
    pub pages: SurfacePages,
    /// Independent online core-channel probe results shared by both UIs.
    #[serde(default)]
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
    pub network_roaming: crate::network_roaming::NetworkRoamingSnapshot,
    /// Android VpnService permission/foreground/tun2proxy session state.
    #[serde(default)]
    pub vpn: crate::vpn::VpnSessionSnapshot,
    /// Bounded live upload/download samples shared by both primary surfaces.
    #[serde(default)]
    pub traffic_waveform: crate::traffic_waveform::TrafficWaveformSnapshot,
    /// Dynamic shared max/unit/tick scale for the traffic waveform.
    #[serde(default)]
    pub traffic_scale: crate::traffic_scale::TrafficScaleSnapshot,
    /// Live routing chain derived from controller connections/configuration.
    #[serde(default)]
    pub traffic_topology: crate::traffic_topology::TrafficTopologySnapshot,
    /// Current selected proxy-group outbound node and its available facts.
    #[serde(default)]
    pub active_exit: crate::active_exit::ActiveExitSnapshot,
    #[serde(default)]
    pub public_ip: crate::public_ip::PublicIpProbeSnapshot,
    #[serde(default)]
    pub overview_layout: crate::overview_layout::OverviewLayoutSnapshot,
    #[serde(default)]
    pub reconnect_mask: crate::reconnect_mask::ReconnectMaskSnapshot,
    #[serde(default)]
    pub viewport: crate::responsive_viewport::ResponsiveViewportSnapshot,
    /// Current active subscription usage and expiry facts.
    #[serde(default)]
    pub subscription_quota: crate::subscription_quota::SubscriptionQuotaSnapshot,
    /// Host-injected privileged-network regression readback.
    #[serde(default)]
    pub privileged_network: crate::privileged_network::PrivilegedNetworkSnapshot,
    /// AST configuration diff and snapshot comparison readback.
    #[serde(default)]
    pub yaml_ast_diff: Option<crate::yaml_ast_diff::YamlAstDiffSnapshot>,
    /// Active QuickJS sandbox console and execution telemetry.
    #[serde(default)]
    pub script_sandbox: Option<crate::script_sandbox::ScriptSandboxSnapshot>,
    /// Shared concurrent speedtest, jitter, and packet loss telemetry.
    #[serde(default)]
    pub speedtest: crate::speedtest::SpeedtestSnapshot,
}

/// Surface-level event vocabulary. Toolkit adapters may translate this into
/// an Iced message, Bevy trigger, Compose state update, or REST stream item.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SurfaceEvent {
    SnapshotUpdated(SurfaceSnapshot),
}

impl SurfaceSnapshot {
    pub fn unavailable(surface: SurfaceKind, host: HostKind, failure: Failure) -> Self {
        Self {
            surface,
            origin: SurfaceOrigin::Live,
            generation: 0,
            revision: 0,
            core: CoreSnapshot {
                lifecycle: CoreLifecycle::Starting,
                generation: 0,
                session_token: None,
                revision: 0,
                proxy_mode: Some(ProxyMode::Rule),
                core_version: None,
                sampled_at_epoch_ms: None,
                failure: Some(failure.clone()),
                upload_bps: 0.0,
                download_bps: 0.0,
                active_connections: 0,
                memory_bytes: None,
                watchdog: Default::default(),
            },
            capabilities: CapabilitySnapshot::new(host, 0, Vec::new()),
            failure: Some(failure.clone()),
            pages: SurfacePages::unavailable(failure),
            versions: CoreVersionSnapshot::default(),
            controller_auth: ControllerAuthSnapshot::default(),
            service_mode: ServiceModeSnapshot::default(),
            port_conflicts: PortConflictSnapshot::default(),
            resources: CoreResourceSnapshot::default(),
            offline_startup: OfflineStartupSnapshot::default(),
            mtu: MtuNegotiationSnapshot::default(),
            system_proxy: SystemProxySnapshot::default(),
            system_proxy_recovery: SystemProxyRecoverySnapshot::default(),
            network_roaming: crate::network_roaming::NetworkRoamingSnapshot::default(),
            vpn: crate::vpn::VpnSessionSnapshot::default(),
            privileged_network: crate::privileged_network::PrivilegedNetworkSnapshot::default(),
            traffic_waveform: crate::traffic_waveform::TrafficWaveformSnapshot::default(),
            traffic_scale: crate::traffic_scale::TrafficScaleSnapshot::default(),
            traffic_topology: crate::traffic_topology::TrafficTopologySnapshot::default(),
            active_exit: crate::active_exit::ActiveExitSnapshot::default(),
            public_ip: crate::public_ip::PublicIpProbeSnapshot::default(),
            overview_layout: crate::overview_layout::OverviewLayoutSnapshot::default(),
            reconnect_mask: crate::reconnect_mask::ReconnectMaskSnapshot::default(),
            viewport: crate::responsive_viewport::ResponsiveViewportSnapshot::default(),
            subscription_quota: crate::subscription_quota::SubscriptionQuotaSnapshot::default(),
            yaml_ast_diff: None,
            script_sandbox: None,
            speedtest: crate::speedtest::SpeedtestSnapshot::default(),
        }
    }

    pub fn page_status(&self, page: PageId) -> &PageStatus {
        self.pages.status(page)
    }

    /// Whether this snapshot belongs to a newer, still-valid core session.
    /// Generation orders restarts; the token fences delayed events from an
    /// older session even when their transport revision happens to be larger.
    pub fn is_newer_than(&self, current: &Self) -> bool {
        if self.generation != current.generation {
            return self.generation > current.generation;
        }
        match (self.core.session_token, current.core.session_token) {
            (Some(_), None) => true,
            // A stopped snapshot has no active token but is still the valid
            // terminal state of the current generation.
            (None, Some(_)) => self.revision > current.revision,
            (Some(next), Some(previous)) if next != previous => false,
            _ => self.revision > current.revision,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_vocabulary_has_exactly_eleven_entries() {
        assert_eq!(PageId::ALL.len(), 11);
        assert_eq!(PageId::ALL[0].as_str(), "overview");
        assert_eq!(PageId::ALL[10].as_str(), "settings");
    }

    #[test]
    fn unavailable_is_not_an_empty_page() {
        let failure = Failure::unsupported("host did not provide this capability");
        let page = PageData::<String>::unavailable(failure.clone());
        assert!(page.data.is_none());
        assert_eq!(page.status, PageStatus::Unavailable { failure });
    }

    #[test]
    fn session_token_fences_old_snapshots_even_with_a_larger_revision() {
        let mut current = SurfaceSnapshot::unavailable(
            SurfaceKind::BevyDesktop,
            HostKind::Desktop,
            Failure::unsupported("not ready"),
        );
        current.generation = 4;
        current.core.generation = 4;
        current.core.session_token = Some(crate::session::SessionToken::new(40));
        current.revision = 10;
        current.core.revision = 10;

        let mut stale = current.clone();
        stale.core.session_token = Some(crate::session::SessionToken::new(39));
        stale.revision = 11;
        stale.core.revision = 11;
        assert!(!stale.is_newer_than(&current));

        let mut next = current.clone();
        next.generation = 5;
        next.core.generation = 5;
        next.core.session_token = Some(crate::session::SessionToken::new(50));
        next.revision = 1;
        next.core.revision = 1;
        assert!(next.is_newer_than(&current));

        let mut stopped = current.clone();
        stopped.core.session_token = None;
        stopped.revision = 11;
        stopped.core.revision = 11;
        assert!(stopped.is_newer_than(&current));
    }

    #[test]
    fn unavailable_surface_defaults_to_offline_first_without_claiming_ready() {
        let snapshot = SurfaceSnapshot::unavailable(
            SurfaceKind::IcedDesktop,
            HostKind::Desktop,
            Failure::unsupported("not composed"),
        );
        assert_eq!(
            snapshot.offline_startup.policy,
            crate::offline_startup::StartupNetworkPolicy::OfflineFirst
        );
        assert!(!snapshot.offline_startup.is_offline_startable());
    }
}

//! Runtime-domain types: core lifecycle status, the live runtime config
//! snapshot and the profile-rebuild flow state.

use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeStreamKind {
    Logs,
    Traffic,
    Connections,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RuntimeStreamState {
    #[default]
    Idle,
    Connecting,
    Connected,
    Reconnecting,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum RuntimeStatus {
    #[default]
    Stopped,
    Starting,
    Running,
    Error(InfiltratorError),
}

impl RuntimeStatus {
    /// Toolkit adapter from the shared lifecycle snapshot. The Iced runtime
    /// status remains a local rendering type; lifecycle truth stays in the
    /// application-owned CoreSnapshot.
    pub fn from_core_snapshot(snapshot: &CoreSnapshot) -> Self {
        match snapshot.lifecycle {
            CoreLifecycle::Stopped => Self::Stopped,
            CoreLifecycle::Starting | CoreLifecycle::Stopping => Self::Starting,
            CoreLifecycle::Ready | CoreLifecycle::Running => Self::Running,
            CoreLifecycle::Failed => Self::Error(InfiltratorError::Mihomo(
                snapshot
                    .failure
                    .as_ref()
                    .map(|failure| failure.message.clone())
                    .unwrap_or_else(|| "core lifecycle failed".to_owned()),
            )),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeConfig {
    pub mode: String,
    pub ipv6_enabled: bool,
    pub allow_lan: bool,
    pub mixed_port: u16,
    pub bind_address: String,
    pub lan_allowed_ips: Vec<String>,
    pub lan_disallowed_ips: Vec<String>,
    pub skip_auth_prefixes: Vec<String>,
    pub authentication_enabled: bool,
    pub authentication_user_count: usize,
    pub authentication_username: Option<String>,
    /// The core only reports `script` when the loaded profile carries a
    /// top-level `script:` block; without it `mode: script` is invalid.
    pub script_block_present: bool,
    pub tun_enabled: bool,
    pub dns_nameservers: Vec<String>,
    pub dns_fallback: Vec<String>,
    pub dns_enhanced_mode: String,
    pub tun_stack: String,
    pub tun_auto_route: bool,
    pub tun_strict_route: bool,
    pub sniffer_enabled: bool,
}

/// Result of the user-requested public-egress probe. The provider and local
/// completion time are kept beside the value so the UI never presents an
/// opaque external request as if it were a core/controller metric.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpProbeResult {
    pub ip: String,
    pub provider: String,
    pub checked_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimePatchSnapshot {
    pub proxy_mode: Option<String>,
    pub ipv6_enabled: bool,
    pub tun_enabled: Option<bool>,
    pub tun_stack: String,
    pub tun_stack_selector: String,
    pub tun_auto_route: bool,
    pub tun_strict_route: bool,
    pub sniffer_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum RebuildFlowState {
    #[default]
    Idle,
    Saving {
        label: String,
    },
    Rebuilding {
        label: String,
    },
    Done {
        label: String,
    },
    Failed {
        label: String,
        error: String,
    },
}

/// Grouping dimension for the live connections audit view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectionGroupingMode {
    #[default]
    Flat,
    ByProcess,
    ByHost,
}

/// State for the PCAP network packet capture and Sniffer auditor.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PcapCaptureState {
    pub is_capturing: bool,
    pub packet_count: usize,
    pub total_bytes: usize,
    pub exported_path: Option<String>,
}

/// Iced's local field is only an adapter alias for the shared contract; it is
/// not a second network-roaming business model.
pub type NetworkRoamingState = infiltrator_contract::network_roaming::NetworkRoamingSnapshot;

/// State for structured regex log filtering and sanitized credential redaction.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LogFilterState {
    pub regex_query: String,
    pub level_filter: String,
    pub exported_redacted_path: Option<String>,
}

/// State for the multi-point latency time-series and stability radar.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LatencyRadarState {
    pub selected_node: String,
    pub samples: Vec<u64>,
    pub avg_ms: f64,
    pub min_ms: u64,
    pub max_ms: u64,
    pub jitter_ms: f64,
    pub stability_score: u8,
}

/// Stage in the atomic configuration apply transaction.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ApplyTransactionStage {
    #[default]
    Idle,
    Preflight,
    Reloading,
    Probing,
    Committed,
    RolledBack(String),
}

/// State for the multi-stage config apply transaction and safe-cut rollback guard.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ApplyTransactionGuardState {
    pub stage: ApplyTransactionStage,
    pub staging_config_saved: bool,
    pub health_probe_passed: bool,
}

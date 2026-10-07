//! Runtime-domain types: core lifecycle status, the live runtime config
//! snapshot and the profile-rebuild flow state.

use infiltrator_contract::connection::ConnectionStreamPhase;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::proxy_mode::ProxyModeSnapshot;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeStreamKind {
    Logs,
    Traffic,
    Connections,
}

/// Wall-clock seconds used to timestamp connection activity observations.
pub fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
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

impl RuntimeStreamState {
    /// Map the Iced-local stream state onto the cross-surface phase enum
    /// (DUAL-13-01) so both surfaces' stream badges share one vocabulary.
    pub fn shared_phase(&self) -> ConnectionStreamPhase {
        use infiltrator_contract::connection::ConnectionStreamPhase;
        match self {
            Self::Idle => ConnectionStreamPhase::Idle,
            Self::Connecting => ConnectionStreamPhase::Connecting,
            Self::Connected => ConnectionStreamPhase::Live,
            Self::Reconnecting => ConnectionStreamPhase::Reconnecting,
            Self::Failed(_) => ConnectionStreamPhase::Unavailable,
        }
    }
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
    pub proxy_mode_state: ProxyModeSnapshot,
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

/// State for the PCAP network packet capture and Sniffer auditor.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PcapCaptureState {
    pub is_capturing: bool,
    pub packet_count: usize,
    pub total_bytes: usize,
    pub exported_path: Option<String>,
}

/// State for structured regex log filtering and sanitized credential redaction.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LogFilterState {
    pub regex_query: String,
    pub level_filter: String,
}

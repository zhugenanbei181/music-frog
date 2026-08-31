//! Runtime-domain types: core lifecycle status, the live runtime config
//! snapshot and the profile-rebuild flow state.

use infiltrator_core::error::InfiltratorError;

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

#[derive(Debug, Clone, Default)]
pub struct RuntimeConfig {
    pub mode: String,
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
    pub tun_enabled: Option<bool>,
    pub tun_stack: String,
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

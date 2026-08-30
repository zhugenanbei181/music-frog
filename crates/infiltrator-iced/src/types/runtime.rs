//! Runtime-domain types: core lifecycle status, the live runtime config
//! snapshot and the profile-rebuild flow state.

use super::InfiltratorError;

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
    pub tun_enabled: bool,
    pub dns_nameservers: Vec<String>,
    pub dns_fallback: Vec<String>,
    pub dns_enhanced_mode: String,
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

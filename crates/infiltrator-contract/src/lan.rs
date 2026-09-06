//! Shared live contract for Mihomo LAN proxy sharing.

use serde::{Deserialize, Serialize};

/// Mihomo's wildcard bind address for all local interfaces.
pub const DEFAULT_BIND_ADDRESS: &str = "*";

/// A confirmed live Allow-LAN configuration. Failures stay in the enclosing
/// application result instead of being represented as guessed UI state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanSharingSnapshot {
    pub enabled: bool,
    pub mixed_port: u16,
    pub bind_address: String,
    pub revision: u64,
}

impl LanSharingSnapshot {
    pub fn new(
        revision: u64,
        enabled: bool,
        mixed_port: u16,
        bind_address: impl Into<String>,
    ) -> Self {
        Self {
            enabled,
            mixed_port,
            bind_address: bind_address.into(),
            revision,
        }
    }
}

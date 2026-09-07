use crate::command::ProxyMode;
use serde::{Deserialize, Serialize};

/// High-level read status for proxy mode configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProxyModeStatus {
    Ready,
    Pending,
    Unsupported,
    Failed,
}

/// Shared proxy mode snapshot describing current mode, available choices, and gating facts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyModeSnapshot {
    pub current: ProxyMode,
    pub script_available: bool,
    pub status: ProxyModeStatus,
    pub failure: Option<String>,
}

impl Default for ProxyModeSnapshot {
    fn default() -> Self {
        Self {
            current: ProxyMode::Rule,
            script_available: false,
            status: ProxyModeStatus::Ready,
            failure: None,
        }
    }
}

impl ProxyModeSnapshot {
    pub fn demo_fixture() -> Self {
        Self {
            current: ProxyMode::Rule,
            script_available: true,
            status: ProxyModeStatus::Ready,
            failure: None,
        }
    }

    pub fn is_drawable(&self) -> bool {
        matches!(self.status, ProxyModeStatus::Ready | ProxyModeStatus::Pending)
    }

    pub fn is_mode_selectable(&self, mode: ProxyMode) -> bool {
        if !self.is_drawable() {
            return false;
        }
        if mode == ProxyMode::Script {
            self.script_available
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_mode_selectable_policy() {
        let mut snapshot = ProxyModeSnapshot::default();
        assert!(snapshot.is_drawable());
        assert!(snapshot.is_mode_selectable(ProxyMode::Rule));
        assert!(snapshot.is_mode_selectable(ProxyMode::Global));
        assert!(snapshot.is_mode_selectable(ProxyMode::Direct));
        assert!(!snapshot.is_mode_selectable(ProxyMode::Script));

        snapshot.script_available = true;
        assert!(snapshot.is_mode_selectable(ProxyMode::Script));

        snapshot.status = ProxyModeStatus::Unsupported;
        assert!(!snapshot.is_drawable());
        assert!(!snapshot.is_mode_selectable(ProxyMode::Rule));
        assert!(!snapshot.is_mode_selectable(ProxyMode::Script));
    }
}

use crate::command::ProxyMode;
use crate::error::Failure;
use serde::{Deserialize, Serialize};

/// High-level read status for proxy mode configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProxyModeStatus {
    Unobserved,
    Ready,
    Pending,
    Unsupported,
    Failed,
}

/// Shared proxy mode snapshot describing current mode, available choices, and gating facts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyModeSnapshot {
    pub current: Option<ProxyMode>,
    pub script_available: Option<bool>,
    pub status: ProxyModeStatus,
    pub failure: Option<Failure>,
}

impl Default for ProxyModeSnapshot {
    fn default() -> Self {
        Self {
            current: None,
            script_available: None,
            status: ProxyModeStatus::Unobserved,
            failure: None,
        }
    }
}

impl ProxyModeSnapshot {
    pub fn demo_fixture() -> Self {
        Self {
            current: Some(ProxyMode::Rule),
            script_available: Some(true),
            status: ProxyModeStatus::Ready,
            failure: None,
        }
    }

    pub fn is_drawable(&self) -> bool {
        matches!(
            self.status,
            ProxyModeStatus::Ready | ProxyModeStatus::Pending
        )
    }

    pub fn is_mode_selectable(&self, mode: ProxyMode) -> bool {
        if self.status != ProxyModeStatus::Ready {
            return false;
        }
        if mode == ProxyMode::Script {
            self.script_available == Some(true)
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
        let unobserved = ProxyModeSnapshot::default();
        assert_eq!(unobserved.current, None);
        assert!(!unobserved.is_drawable());
        for mode in ProxyMode::ALL {
            assert!(!unobserved.is_mode_selectable(mode));
        }
        let mut snapshot = ProxyModeSnapshot::demo_fixture();
        snapshot.script_available = Some(false);
        assert!(snapshot.is_drawable());
        assert!(snapshot.is_mode_selectable(ProxyMode::Rule));
        assert!(snapshot.is_mode_selectable(ProxyMode::Global));
        assert!(snapshot.is_mode_selectable(ProxyMode::Direct));
        assert!(!snapshot.is_mode_selectable(ProxyMode::Script));

        snapshot.script_available = Some(true);
        assert!(snapshot.is_mode_selectable(ProxyMode::Script));

        snapshot.status = ProxyModeStatus::Unsupported;
        assert!(!snapshot.is_drawable());
        assert!(!snapshot.is_mode_selectable(ProxyMode::Rule));
        assert!(!snapshot.is_mode_selectable(ProxyMode::Script));

        snapshot.status = ProxyModeStatus::Pending;
        assert!(
            snapshot.is_drawable(),
            "pending retains the observed selection"
        );
        for mode in ProxyMode::ALL {
            assert!(
                !snapshot.is_mode_selectable(mode),
                "pending cannot accept a second command"
            );
        }
    }
}

//! Shared state and policy vocabulary for the system-level quick toggles.

use crate::error::Failure;
use serde::{Deserialize, Serialize};

/// The system-owned controls exposed in both primary UI sidebars.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemToggle {
    SystemProxy,
    Tun,
}

/// A toggle's authoritative state, including states in which a switch must
/// not accept another user action.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemToggleState {
    #[default]
    Unknown,
    Disabled,
    Enabled,
    Pending { desired: bool },
    Unsupported { failure: Failure },
    Failed { failure: Failure },
}

impl SystemToggleState {
    pub fn from_enabled(enabled: bool) -> Self {
        if enabled {
            Self::Enabled
        } else {
            Self::Disabled
        }
    }

    pub fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled | Self::Pending { desired: true })
    }

    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending { .. })
    }

    pub fn can_toggle(&self) -> bool {
        matches!(self, Self::Enabled | Self::Disabled)
    }

    /// Short, toolkit-neutral status copy used by compact switch controls.
    pub fn compact_label(&self) -> &'static str {
        match self {
            Self::Enabled => "开",
            Self::Disabled => "关",
            Self::Pending { .. } => "…",
            Self::Unknown | Self::Unsupported { .. } | Self::Failed { .. } => "—",
        }
    }
}

/// One shared projection for the two system-level sidebar switches.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemToggleSnapshot {
    pub system_proxy: SystemToggleState,
    pub tun: SystemToggleState,
    pub revision: u64,
}

impl SystemToggleSnapshot {
    pub fn from_legacy(system_proxy: bool, tun: Option<bool>, revision: u64) -> Self {
        Self {
            system_proxy: SystemToggleState::from_enabled(system_proxy),
            tun: tun.map_or(SystemToggleState::Unknown, SystemToggleState::from_enabled),
            revision,
        }
    }

    pub fn state(&self, toggle: SystemToggle) -> &SystemToggleState {
        match toggle {
            SystemToggle::SystemProxy => &self.system_proxy,
            SystemToggle::Tun => &self.tun,
        }
    }

    pub fn with_pending(mut self, toggle: SystemToggle, desired: bool) -> Self {
        let state = SystemToggleState::Pending { desired };
        match toggle {
            SystemToggle::SystemProxy => self.system_proxy = state,
            SystemToggle::Tun => self.tun = state,
        }
        self
    }

    pub fn with_legacy_tun(mut self, tun: Option<bool>) -> Self {
        if !self.tun.is_pending() {
            self.tun = tun.map_or(SystemToggleState::Unknown, SystemToggleState::from_enabled);
        }
        self
    }

    pub fn with_tun_readback(mut self, tun: Option<bool>) -> Self {
        self.tun = tun.map_or(SystemToggleState::Unknown, SystemToggleState::from_enabled);
        self
    }

    pub fn with_legacy_system_proxy(mut self, enabled: bool) -> Self {
        if !self.system_proxy.is_pending() {
            self.system_proxy = SystemToggleState::from_enabled(enabled);
        }
        self
    }

    pub fn with_system_proxy_readback(mut self, enabled: bool) -> Self {
        self.system_proxy = SystemToggleState::from_enabled(enabled);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_state_keeps_the_requested_visual_value_but_blocks_reentry() {
        let snapshot = SystemToggleSnapshot::from_legacy(false, Some(true), 4)
            .with_pending(SystemToggle::SystemProxy, true);
        let state = snapshot.state(SystemToggle::SystemProxy);
        assert!(state.is_enabled());
        assert!(state.is_pending());
        assert!(!state.can_toggle());
        assert_eq!(state.compact_label(), "…");
    }

    #[test]
    fn unknown_and_failure_states_are_not_presented_as_disabled() {
        let failure = Failure::unsupported("host has no toggle");
        let snapshot = SystemToggleSnapshot {
            system_proxy: SystemToggleState::Unsupported { failure },
            tun: SystemToggleState::Unknown,
            revision: 1,
        };
        assert!(!snapshot.system_proxy.is_enabled());
        assert!(!snapshot.system_proxy.can_toggle());
        assert_eq!(snapshot.tun.compact_label(), "—");
    }
}

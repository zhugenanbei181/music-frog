//! Shared read model for the currently selected Mihomo outbound node.

use crate::outbound::identity::OutboundId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveExitStatus {
    #[default]
    Unknown,
    Ready {
        group_id: OutboundId,
        outbound_id: OutboundId,
    },
    Empty,
    Unsupported,
    Failed,
}

/// The selected proxy node and the controller facts available for it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ActiveExitSnapshot {
    pub generation: u64,
    pub revision: u64,
    pub status: ActiveExitStatus,
    pub failure: Option<String>,
    pub group: Option<String>,
    pub name: Option<String>,
    /// Country/region code parsed from the node label, never a guessed IP
    /// geolocation. `None` is an honest unknown flag state.
    pub country_code: Option<String>,
    pub protocol: Option<String>,
    pub delay_ms: Option<u32>,
    pub alive: Option<bool>,
}

impl ActiveExitSnapshot {
    /// Explicit fixture for demo/screenshot hosts only.
    pub fn demo_fixture() -> Self {
        let engine =
            crate::core_identity::EngineId::new("mihomo").expect("demo engine identity is valid");
        Self {
            generation: 1,
            revision: 1,
            status: ActiveExitStatus::Ready {
                group_id: OutboundId::new(engine.clone(), "GLOBAL")
                    .expect("demo group identity is valid"),
                outbound_id: OutboundId::new(engine, "香港 IPLC 01")
                    .expect("demo outbound identity is valid"),
            },
            failure: None,
            group: Some("GLOBAL".to_owned()),
            name: Some("香港 IPLC 01".to_owned()),
            country_code: Some("HK".to_owned()),
            protocol: Some("VLESS · Reality".to_owned()),
            delay_ms: Some(38),
            alive: Some(true),
        }
    }

    pub fn unsupported(generation: u64, revision: u64, reason: impl Into<String>) -> Self {
        Self {
            generation,
            revision,
            status: ActiveExitStatus::Unsupported,
            failure: Some(reason.into()),
            ..Self::default()
        }
    }

    pub fn failed(generation: u64, revision: u64, reason: impl Into<String>) -> Self {
        Self {
            generation,
            revision,
            status: ActiveExitStatus::Failed,
            failure: Some(reason.into()),
            ..Self::default()
        }
    }

    pub fn unavailable(generation: u64, revision: u64, reason: impl Into<String>) -> Self {
        Self {
            generation,
            revision,
            status: ActiveExitStatus::Unknown,
            failure: Some(reason.into()),
            ..Self::default()
        }
    }

    pub fn is_drawable(&self) -> bool {
        matches!(&self.status, ActiveExitStatus::Ready { .. })
            && self.name.as_ref().is_some_and(|name| !name.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_exposes_high_fidelity_exit_facts() {
        let snapshot = ActiveExitSnapshot::demo_fixture();
        assert!(snapshot.is_drawable());
        assert_eq!(snapshot.country_code.as_deref(), Some("HK"));
        assert_eq!(snapshot.protocol.as_deref(), Some("VLESS · Reality"));
        assert_eq!(snapshot.delay_ms, Some(38));
    }

    #[test]
    fn unsupported_exit_is_not_drawable() {
        let snapshot = ActiveExitSnapshot::unsupported(2, 3, "proxy gateway missing");
        assert!(!snapshot.is_drawable());
        assert_eq!(snapshot.failure.as_deref(), Some("proxy gateway missing"));
    }
}

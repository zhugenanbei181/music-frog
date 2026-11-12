//! Shared read model for the public IP and privacy probe.

use crate::public_ip_probe::PublicIpProbeState;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicIpProbeStatus {
    #[default]
    Unknown,
    Ready,
    Probing,
    Empty,
    Unsupported,
    Failed,
}

/// Shared snapshot reflecting the observed public outbound IP and GeoIP facts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PublicIpProbeSnapshot {
    pub generation: u64,
    pub revision: u64,
    pub status: PublicIpProbeStatus,
    pub failure: Option<String>,
    pub ip: Option<String>,
    pub country_code: Option<String>,
    pub city: Option<String>,
    pub isp: Option<String>,
    pub asn: Option<u32>,
    pub provider: Option<String>,
    pub checked_at_epoch_ms: Option<u64>,
}

impl PublicIpProbeSnapshot {
    /// Project the one owner-authored state into the stable surface readback.
    pub fn from_state(state: &PublicIpProbeState) -> Self {
        match state {
            PublicIpProbeState::Idle => Self::default(),
            PublicIpProbeState::Probing { stamp } => Self {
                generation: stamp.core_generation,
                revision: stamp.revision,
                status: PublicIpProbeStatus::Probing,
                ..Self::default()
            },
            PublicIpProbeState::Succeeded { stamp, observation } => Self {
                generation: stamp.core_generation,
                revision: stamp.revision,
                status: PublicIpProbeStatus::Ready,
                ip: Some(observation.ip.clone()),
                country_code: observation.country.clone(),
                city: observation.city.clone(),
                provider: Some(observation.provider.clone()),
                checked_at_epoch_ms: Some(observation.checked_at_epoch_ms),
                ..Self::default()
            },
            PublicIpProbeState::Failed { stamp, reason } => {
                Self::failed(stamp.core_generation, stamp.revision, reason.clone())
            }
            PublicIpProbeState::Stale { .. } => Self::default(),
        }
    }

    /// Explicit fixture for demo/screenshot hosts.
    pub fn demo_fixture() -> Self {
        Self {
            generation: 1,
            revision: 1,
            status: PublicIpProbeStatus::Ready,
            failure: None,
            ip: Some("203.0.113.7".to_owned()),
            country_code: Some("HK".to_owned()),
            city: Some("Hong Kong".to_owned()),
            isp: Some("Cloudflare / APNIC".to_owned()),
            asn: Some(13335),
            provider: Some("ipapi.is".to_owned()),
            checked_at_epoch_ms: Some(1741300000000),
        }
    }

    pub fn unsupported(generation: u64, revision: u64, reason: impl Into<String>) -> Self {
        Self {
            generation,
            revision,
            status: PublicIpProbeStatus::Unsupported,
            failure: Some(reason.into()),
            ..Self::default()
        }
    }

    pub fn failed(generation: u64, revision: u64, reason: impl Into<String>) -> Self {
        Self {
            generation,
            revision,
            status: PublicIpProbeStatus::Failed,
            failure: Some(reason.into()),
            ..Self::default()
        }
    }

    pub fn unavailable(generation: u64, revision: u64, reason: impl Into<String>) -> Self {
        Self {
            generation,
            revision,
            status: PublicIpProbeStatus::Unknown,
            failure: Some(reason.into()),
            ..Self::default()
        }
    }

    pub fn is_drawable(&self) -> bool {
        self.status == PublicIpProbeStatus::Ready
            && self.ip.as_ref().is_some_and(|ip| !ip.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_exposes_drawable_facts() {
        let snapshot = PublicIpProbeSnapshot::demo_fixture();
        assert!(snapshot.is_drawable());
        assert_eq!(snapshot.country_code.as_deref(), Some("HK"));
        assert_eq!(snapshot.city.as_deref(), Some("Hong Kong"));
        assert_eq!(snapshot.isp.as_deref(), Some("Cloudflare / APNIC"));
        assert_eq!(snapshot.ip.as_deref(), Some("203.0.113.7"));
    }

    #[test]
    fn failed_probe_is_not_drawable() {
        let snapshot = PublicIpProbeSnapshot::failed(1, 2, "network unreachable");
        assert!(!snapshot.is_drawable());
        assert_eq!(snapshot.status, PublicIpProbeStatus::Failed);
    }
}

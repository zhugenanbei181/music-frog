use crate::surface::HostKind;
use lurker_core_contract::capability::Capability;
use serde::{Deserialize, Serialize};

/// Stable reason identities for capability facts.
///
/// A snapshot is a domain fact, not a translated view: it publishes a stable
/// key, and the presentation layer resolves that key through the product
/// catalog. User-facing prose never lives in the snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityReason {
    NotRegistered,
}

impl CapabilityReason {
    pub const ALL: [Self; 1] = [Self::NotRegistered];

    pub const fn key(self) -> &'static str {
        match self {
            Self::NotRegistered => "capability_not_registered",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Availability {
    Supported,
    Experimental,
    Unsupported { reason: String },
    Unavailable { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityStatus {
    pub capability: Capability,
    pub availability: Availability,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitySnapshot {
    pub host: HostKind,
    pub revision: u64,
    pub entries: Vec<CapabilityStatus>,
}

impl CapabilitySnapshot {
    pub fn new(host: HostKind, revision: u64, entries: Vec<CapabilityStatus>) -> Self {
        Self {
            host,
            revision,
            entries,
        }
    }

    pub fn availability(&self, capability: Capability) -> Availability {
        self.entries
            .iter()
            .find(|entry| entry.capability == capability)
            .map(|entry| entry.availability.clone())
            .unwrap_or_else(|| Availability::Unsupported {
                reason: CapabilityReason::NotRegistered.key().to_string(),
            })
    }

    pub fn supports(&self, capability: Capability) -> bool {
        matches!(
            self.availability(capability),
            Availability::Supported | Availability::Experimental
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Availability, CapabilityReason, CapabilitySnapshot};
    use crate::surface::HostKind;
    use lurker_core_contract::capability::Capability;

    #[test]
    fn a_missing_capability_publishes_a_stable_reason_key() {
        let snapshot = CapabilitySnapshot::new(HostKind::Desktop, 1, Vec::new());
        assert_eq!(
            snapshot.availability(Capability::Tun),
            Availability::Unsupported {
                reason: CapabilityReason::NotRegistered.key().to_owned(),
            }
        );
    }

    #[test]
    fn every_reason_has_one_stable_key() {
        assert_eq!(
            CapabilityReason::ALL.map(CapabilityReason::key),
            ["capability_not_registered"]
        );
    }
}

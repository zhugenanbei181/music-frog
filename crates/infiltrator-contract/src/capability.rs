use crate::surface::HostKind;
use serde::{Deserialize, Serialize};

/// A capability describes what a host can actually perform.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    CoreLifecycle,
    Profiles,
    ProxyMode,
    Connections,
    Logs,
    Dns,
    Tun,
    SystemProxy,
    Autostart,
    CoreVersionInstall,
    WebDavSync,
    AppRouting,
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
                reason: "capability not registered".to_string(),
            })
    }

    pub fn supports(&self, capability: Capability) -> bool {
        matches!(
            self.availability(capability),
            Availability::Supported | Availability::Experimental
        )
    }
}

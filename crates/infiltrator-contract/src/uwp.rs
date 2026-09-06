//! Shared contract for Windows AppContainer loopback exemptions.

use serde::{Deserialize, Serialize};

/// Runtime-neutral description of one installed AppContainer package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UwpPackageSnapshot {
    pub sid: String,
    pub display_name: String,
    pub package_family_name: String,
    pub loopback_exempt: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UwpLoopbackAvailability {
    Supported,
    Unsupported { reason: String },
    Unavailable { reason: String },
}

impl Default for UwpLoopbackAvailability {
    fn default() -> Self {
        Self::Unsupported {
            reason: "UWP loopback is not composed for this host".to_owned(),
        }
    }
}

/// Confirmed AppContainer loopback state. A missing host adapter is explicit
/// and never represented as an empty successful package list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UwpLoopbackSnapshot {
    pub availability: UwpLoopbackAvailability,
    pub packages: Vec<UwpPackageSnapshot>,
    pub revision: u64,
}

impl Default for UwpLoopbackSnapshot {
    fn default() -> Self {
        Self::unsupported(0, "UWP loopback is not composed for this host")
    }
}

impl UwpLoopbackSnapshot {
    pub fn supported(revision: u64, packages: Vec<UwpPackageSnapshot>) -> Self {
        Self {
            availability: UwpLoopbackAvailability::Supported,
            packages,
            revision,
        }
    }

    pub fn unsupported(revision: u64, reason: impl Into<String>) -> Self {
        Self {
            availability: UwpLoopbackAvailability::Unsupported {
                reason: reason.into(),
            },
            packages: Vec::new(),
            revision,
        }
    }

    pub fn unavailable(revision: u64, reason: impl Into<String>) -> Self {
        Self {
            availability: UwpLoopbackAvailability::Unavailable {
                reason: reason.into(),
            },
            packages: Vec::new(),
            revision,
        }
    }

    pub const fn is_supported(&self) -> bool {
        matches!(self.availability, UwpLoopbackAvailability::Supported)
    }
}

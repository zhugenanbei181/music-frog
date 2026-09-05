//! Shared TUN protocol-stack vocabulary.
//!
//! Mihomo's current top-level TUN configuration accepts `system`, `gvisor`
//! and `mixed`.  `lwip` is retained as a reference catalog entry because it
//! appears in upstream performance material, but it is deliberately marked
//! non-live so a UI can never send an unsupported value to the core.

use serde::{Deserialize, Serialize};

/// TUN stack names used by the shared command and settings projections.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TunStack {
    #[default]
    Gvisor,
    System,
    Mixed,
    Lwip,
}

impl TunStack {
    pub const ALL: [Self; 4] = [Self::Gvisor, Self::System, Self::Mixed, Self::Lwip];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gvisor => "gvisor",
            Self::System => "system",
            Self::Mixed => "mixed",
            Self::Lwip => "lwip",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Gvisor => "gVisor",
            Self::System => "System",
            Self::Mixed => "Mixed",
            Self::Lwip => "LWIP",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "gvisor" | "gvisor-stack" => Some(Self::Gvisor),
            "system" | "system-stack" => Some(Self::System),
            "mixed" | "mixed-stack" => Some(Self::Mixed),
            "lwip" | "lwip-stack" => Some(Self::Lwip),
            _ => None,
        }
    }

    /// Whether Mihomo's current top-level TUN config can receive this value.
    pub const fn is_live_supported(self) -> bool {
        !matches!(self, Self::Lwip)
    }

    pub fn availability(self) -> TunStackAvailability {
        if self.is_live_supported() {
            TunStackAvailability::Supported
        } else {
            TunStackAvailability::ReferenceOnly {
                reason: "upstream documents LWIP for comparison only; live top-level TUN values are system/gvisor/mixed".to_owned(),
            }
        }
    }
}

/// Why an entry is or is not selectable by a live command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TunStackAvailability {
    Supported,
    ReferenceOnly { reason: String },
}

/// UI-neutral option record so both toolkits can render the same four-entry
/// catalog and the same disabled LWIP explanation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunStackOption {
    pub stack: TunStack,
    pub availability: TunStackAvailability,
}

impl TunStack {
    pub fn options() -> [TunStackOption; 4] {
        Self::ALL.map(|stack| TunStackOption {
            stack,
            availability: stack.availability(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_contains_four_stable_wire_names() {
        assert_eq!(
            TunStack::ALL.map(TunStack::as_str),
            ["gvisor", "system", "mixed", "lwip"]
        );
        assert_eq!(TunStack::parse(" MIXED "), Some(TunStack::Mixed));
        assert_eq!(TunStack::parse("unknown"), None);
    }

    #[test]
    fn only_upstream_live_values_are_supported_for_apply() {
        assert!(TunStack::Gvisor.is_live_supported());
        assert!(TunStack::System.is_live_supported());
        assert!(TunStack::Mixed.is_live_supported());
        assert!(!TunStack::Lwip.is_live_supported());
        assert!(matches!(
            TunStack::Lwip.availability(),
            TunStackAvailability::ReferenceOnly { .. }
        ));
    }
}

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

/// Explicit update intent for an optional persisted TUN stack.
///
/// `Keep` leaves the existing stack untouched, `Clear` removes the YAML
/// field, and `Set` stores one typed stack. This avoids using `Option<TunStack>`
/// to hide two different operations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TunStackUpdate {
    #[default]
    Keep,
    Clear,
    Set(TunStack),
}

/// Mihomo's top-level TUN system-DNS policy introduced in v1.19.33.
///
/// An absent field is the upstream automatic policy; the three concrete values
/// are the only non-empty values accepted by Mihomo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TunDnsMode {
    Disabled,
    Native,
    Hijack,
}

/// Explicit update intent for the persisted optional `tun.dns-mode` field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TunDnsModeUpdate {
    #[default]
    Keep,
    Clear,
    Set(TunDnsMode),
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

impl TunDnsMode {
    pub const ALL: [Self; 3] = [Self::Disabled, Self::Native, Self::Hijack];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Native => "native",
            Self::Hijack => "hijack",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "disabled" => Some(Self::Disabled),
            "native" => Some(Self::Native),
            "hijack" => Some(Self::Hijack),
            _ => None,
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

/// One entry of the TUN stack selector a surface renders.
///
/// The contract owns the catalog; this type adds the two facts a control
/// needs and cannot invent: the wire name it sends back, and whether the
/// entry may be applied at all. A surface therefore cannot offer a value the
/// core would refuse, cannot spell the same stack two ways, and cannot
/// rebuild the vocabulary in its own file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunStackChoice {
    pub stack: TunStack,
    /// The value a surface sends back when the user picks this entry.
    pub wire_name: &'static str,
    /// Whether picking this entry may reach the core.
    pub selectable: bool,
    /// Why the entry cannot be applied, when it cannot.
    pub unavailable_reason: Option<&'static str>,
}

impl TunStack {
    /// The entries a TUN stack selector offers, in the contract's own order.
    ///
    /// Every entry is present, including the reference-only one: hiding it
    /// would leave a user who already has such a value with a selector that
    /// silently shows nothing selected. An entry that cannot be applied
    /// carries the reason, so the surface can explain it instead of dropping
    /// it.
    pub const fn choices() -> [TunStackChoice; 4] {
        [
            TunStackChoice {
                stack: TunStack::Gvisor,
                wire_name: "gvisor",
                selectable: true,
                unavailable_reason: None,
            },
            TunStackChoice {
                stack: TunStack::System,
                wire_name: "system",
                selectable: true,
                unavailable_reason: None,
            },
            TunStackChoice {
                stack: TunStack::Mixed,
                wire_name: "mixed",
                selectable: true,
                unavailable_reason: None,
            },
            TunStackChoice {
                stack: TunStack::Lwip,
                wire_name: "lwip",
                selectable: false,
                unavailable_reason: Some(
                    "upstream documents LWIP for comparison only; live top-level TUN values are system/gvisor/mixed",
                ),
            },
        ]
    }

    /// The choice a selector shows as selected for a wire name.
    ///
    /// `None` means the value is not in the catalog at all - a corrupt
    /// configuration, not a missing selection.
    pub fn choice_for_wire(wire_name: &str) -> Option<TunStackChoice> {
        Self::parse(wire_name).and_then(|stack| {
            Self::choices()
                .into_iter()
                .find(|choice| choice.stack == stack)
        })
    }

    /// The index of a wire name in [`TunStack::choices`], or `None`.
    pub fn choice_index_for_wire(wire_name: &str) -> Option<usize> {
        let stack = Self::parse(wire_name)?;
        Self::choices()
            .iter()
            .position(|choice| choice.stack == stack)
    }

    /// The wire name of the entry at a catalog index.
    pub fn wire_name_at(index: usize) -> &'static str {
        let choices = Self::choices();
        choices[index.min(choices.len() - 1)].wire_name
    }

    /// The wire name of the entry a selector may offer, at that selector's
    /// index.
    pub fn selectable_wire_name_at(index: usize) -> &'static str {
        Self::SELECTABLE_WIRE_NAMES[index.min(Self::SELECTABLE_WIRE_NAMES.len() - 1)]
    }

    /// The wire names of the entries a selector may offer.
    ///
    /// A fixed array so a control can borrow it for its whole lifetime: the
    /// values a surface offers are the ones the core accepts, and a surface
    /// never assembles the list itself.
    pub const SELECTABLE_WIRE_NAMES: [&'static str; 3] = ["gvisor", "system", "mixed"];

    /// The entries a selector may offer.
    pub const fn selectable_choices() -> [TunStackChoice; 3] {
        [
            TunStackChoice {
                stack: TunStack::Gvisor,
                wire_name: "gvisor",
                selectable: true,
                unavailable_reason: None,
            },
            TunStackChoice {
                stack: TunStack::System,
                wire_name: "system",
                selectable: true,
                unavailable_reason: None,
            },
            TunStackChoice {
                stack: TunStack::Mixed,
                wire_name: "mixed",
                selectable: true,
                unavailable_reason: None,
            },
        ]
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

    #[test]
    fn the_selector_catalog_is_the_contract_catalog() {
        assert_eq!(
            TunStack::choices().map(|choice| choice.stack),
            TunStack::ALL
        );
        assert_eq!(
            TunStack::choices().map(|choice| choice.wire_name),
            ["gvisor", "system", "mixed", "lwip"]
        );
    }

    #[test]
    fn only_live_values_are_selectable_and_the_rest_explain_why() {
        for choice in TunStack::choices() {
            assert_eq!(
                choice.selectable,
                choice.stack.is_live_supported(),
                "{choice:?}"
            );
            assert_eq!(
                choice.unavailable_reason.is_some(),
                !choice.selectable,
                "{choice:?}"
            );
        }
    }

    #[test]
    fn a_catalog_index_and_a_wire_name_name_the_same_entry() {
        for (index, choice) in TunStack::choices().iter().enumerate() {
            assert_eq!(TunStack::wire_name_at(index), choice.wire_name);
            assert_eq!(
                TunStack::choice_index_for_wire(choice.wire_name),
                Some(index)
            );
            assert_eq!(
                TunStack::choice_for_wire(choice.wire_name).map(|found| found.stack),
                Some(choice.stack)
            );
        }
        assert_eq!(TunStack::wire_name_at(99), "lwip");
        assert_eq!(TunStack::choice_for_wire("unknown"), None);
        assert_eq!(TunStack::choice_index_for_wire(" Gvisor "), Some(0));
    }

    #[test]
    fn the_offered_index_names_the_offered_entry() {
        for (index, name) in TunStack::SELECTABLE_WIRE_NAMES.iter().enumerate() {
            assert_eq!(TunStack::selectable_wire_name_at(index), *name);
        }
        assert_eq!(TunStack::selectable_wire_name_at(99), "mixed");
    }

    #[test]
    fn the_offered_entries_are_exactly_the_live_ones() {
        assert_eq!(
            TunStack::selectable_choices()
                .map(|choice| choice.wire_name),
            TunStack::SELECTABLE_WIRE_NAMES
        );
        for choice in TunStack::selectable_choices() {
            assert!(choice.selectable, "{choice:?}");
            assert!(choice.stack.is_live_supported(), "{choice:?}");
        }
    }

    #[test]
    fn stack_update_is_a_typed_three_way_intent() {
        assert_eq!(TunStackUpdate::default(), TunStackUpdate::Keep);
        assert_eq!(
            serde_json::from_str::<TunStackUpdate>(r#"{"Set":"system"}"#).unwrap(),
            TunStackUpdate::Set(TunStack::System),
        );
        assert!(serde_json::from_str::<TunStackUpdate>(r#"{"Set":"unknown"}"#).is_err());
    }

    #[test]
    fn dns_mode_is_a_closed_typed_catalog() {
        assert_eq!(
            TunDnsMode::ALL.map(TunDnsMode::as_str),
            ["disabled", "native", "hijack"]
        );
        assert_eq!(TunDnsMode::parse(" HIJACK "), Some(TunDnsMode::Hijack));
        assert_eq!(TunDnsMode::parse("automatic"), None);
        assert_eq!(
            serde_json::from_str::<TunDnsMode>(r#""native""#).unwrap(),
            TunDnsMode::Native
        );
        assert!(serde_json::from_str::<TunDnsMode>(r#""unknown""#).is_err());
    }

    #[test]
    fn dns_mode_update_is_a_typed_three_way_intent() {
        assert_eq!(TunDnsModeUpdate::default(), TunDnsModeUpdate::Keep);
        assert_eq!(
            serde_json::from_str::<TunDnsModeUpdate>(r#"{"Set":"hijack"}"#).unwrap(),
            TunDnsModeUpdate::Set(TunDnsMode::Hijack)
        );
        assert!(serde_json::from_str::<TunDnsModeUpdate>(r#"{"Set":"unknown"}"#).is_err());
    }
}

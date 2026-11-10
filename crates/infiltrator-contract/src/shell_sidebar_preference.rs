//! Typed desktop sidebar-expansion facts, reconciled against FlClash v0.9.3.
//!
//! The reference stores `sidebarExpanded` in app settings, reads the legacy
//! `showLabel` key through one migration, and gates the action with
//! `canExpandSidebar(viewMode) == desktop`. A surface therefore needs two
//! distinct typed facts: what the user chose and whether the current viewport
//! can honour it. This module owns those facts and the one resolver that folds
//! them into a presentation; it never reads a window or holds mutable UI state.

use crate::responsive_viewport::{SidebarForm, ViewportTier};
use serde::{Deserialize, Serialize};

/// Canonical persisted key for the user-owned sidebar-expansion preference.
pub const SIDEBAR_EXPANDED_SETTING_KEY: &str = "sidebar_expanded";

/// Legacy persisted key retained only for an explicit version migration.
pub const SIDEBAR_EXPANDED_LEGACY_KEY: &str = "show_label";

/// The user-owned choice: keep destination labels visible when the viewport
/// permits, or collapse them to an icon rail.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidebarExpansionPreference {
    #[default]
    Expanded,
    Collapsed,
}

impl SidebarExpansionPreference {
    pub const ALL: [Self; 2] = [Self::Expanded, Self::Collapsed];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Expanded => "expanded",
            Self::Collapsed => "collapsed",
        }
    }

    /// Parse the canonical or legacy wire value. Unknown values stay absent so
    /// a caller can keep the owner default instead of guessing a second state.
    pub fn from_wire_value(raw: &str) -> Option<Self> {
        match raw.trim() {
            "true" | "expanded" => Some(Self::Expanded),
            "false" | "collapsed" => Some(Self::Collapsed),
            _ => None,
        }
    }

    pub const fn as_wire_value(self) -> &'static str {
        match self {
            Self::Expanded => "true",
            Self::Collapsed => "false",
        }
    }

    /// The one typed transition of the preference: a toggle never becomes a
    /// viewport-derived boolean or an optional fallback.
    pub const fn toggled(self) -> Self {
        match self {
            Self::Expanded => Self::Collapsed,
            Self::Collapsed => Self::Expanded,
        }
    }
}

/// Whether the viewport permits an expanded labelled column.
///
/// FlClash allows expansion only above its laptop boundary; below that the
/// labelled column would make content narrower than the next lower form. The
/// `Fixed` variants make an already-resolved shell form usable in tests and
/// adapters without re-deriving the tier from a width.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidebarExpansionCapability {
    Tier(ViewportTier),
    Fixed(SidebarForm),
}

impl SidebarExpansionCapability {
    pub const fn from_tier(tier: ViewportTier) -> Self {
        Self::Tier(tier)
    }

    pub const fn from_form(form: SidebarForm) -> Self {
        Self::Fixed(form)
    }

    pub const fn form(self) -> SidebarForm {
        match self {
            Self::Tier(tier) => tier.sidebar_form(),
            Self::Fixed(form) => form,
        }
    }

    pub const fn can_expand(self) -> bool {
        self.form().shows_labels()
    }
}

/// What the sidebar should paint after preference and viewport are resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidebarExpansionPresentation {
    /// The resolved form already owns no vertical column.
    NoColumn { form: SidebarForm },
    /// Rail, bottom bar, or a viewport that cannot honour expansion.
    Collapsed { form: SidebarForm },
    /// The user preference is honoured in a labelled desktop column.
    Expanded { form: SidebarForm },
}

impl SidebarExpansionPresentation {
    pub const fn form(self) -> SidebarForm {
        match self {
            Self::NoColumn { form } | Self::Collapsed { form } | Self::Expanded { form } => form,
        }
    }

    pub const fn shows_labels(self) -> bool {
        matches!(self, Self::Expanded { .. })
    }
}

/// One typed command from a surface to the preference owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidebarExpansionIntent {
    Set(SidebarExpansionPreference),
    Toggle,
}

/// Resolve the preference and viewport capability into one presentation fact.
pub const fn resolve_sidebar_expansion(
    preference: SidebarExpansionPreference,
    capability: SidebarExpansionCapability,
) -> SidebarExpansionPresentation {
    let form = capability.form();
    if !form.has_column() {
        return SidebarExpansionPresentation::NoColumn { form };
    }
    if !capability.can_expand() || matches!(preference, SidebarExpansionPreference::Collapsed) {
        return SidebarExpansionPresentation::Collapsed {
            form: SidebarForm::Rail,
        };
    }
    SidebarExpansionPresentation::Expanded { form }
}

/// Apply one typed intent to the current preference; the result is the exact
/// value the owner should persist and read back.
pub const fn apply_sidebar_expansion_intent(
    current: SidebarExpansionPreference,
    intent: SidebarExpansionIntent,
) -> SidebarExpansionPreference {
    match intent {
        SidebarExpansionIntent::Set(preference) => preference,
        SidebarExpansionIntent::Toggle => current.toggled(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::responsive_viewport::TIERS;

    #[test]
    fn the_wire_identity_is_canonical_and_toggle_is_exhaustive() {
        for preference in SidebarExpansionPreference::ALL {
            assert_eq!(
                SidebarExpansionPreference::from_wire_value(preference.as_wire_value()),
                Some(preference)
            );
            assert_ne!(preference.toggled(), preference);
            assert_eq!(preference.toggled().toggled(), preference);
        }
        assert_eq!(SidebarExpansionPreference::from_wire_value("unknown"), None);
    }

    #[test]
    fn only_labelled_desktop_tiers_can_expand() {
        assert!(!SidebarExpansionCapability::from_tier(ViewportTier::Compact).can_expand());
        assert!(!SidebarExpansionCapability::from_tier(ViewportTier::Medium).can_expand());
        assert!(SidebarExpansionCapability::from_tier(ViewportTier::Expanded).can_expand());
        assert!(SidebarExpansionCapability::from_tier(ViewportTier::Ultra).can_expand());
        for tier in TIERS {
            let capability = SidebarExpansionCapability::from_tier(tier);
            assert_eq!(capability.form(), tier.sidebar_form());
        }
    }

    #[test]
    fn a_collapsed_preference_becomes_the_existing_rail_form() {
        let capability = SidebarExpansionCapability::from_tier(ViewportTier::Expanded);
        assert_eq!(
            resolve_sidebar_expansion(SidebarExpansionPreference::Expanded, capability),
            SidebarExpansionPresentation::Expanded {
                form: SidebarForm::Standard
            }
        );
        assert_eq!(
            resolve_sidebar_expansion(SidebarExpansionPreference::Collapsed, capability),
            SidebarExpansionPresentation::Collapsed {
                form: SidebarForm::Rail
            }
        );
        assert!(
            !resolve_sidebar_expansion(SidebarExpansionPreference::Collapsed, capability)
                .shows_labels()
        );
    }

    #[test]
    fn a_narrow_or_bottom_form_is_never_reinterpreted_as_expanded() {
        let medium = SidebarExpansionCapability::from_tier(ViewportTier::Medium);
        let bottom = SidebarExpansionCapability::from_form(SidebarForm::BottomNav);
        assert_eq!(
            resolve_sidebar_expansion(SidebarExpansionPreference::Expanded, medium),
            SidebarExpansionPresentation::Collapsed {
                form: SidebarForm::Rail
            }
        );
        assert_eq!(
            resolve_sidebar_expansion(SidebarExpansionPreference::Expanded, bottom),
            SidebarExpansionPresentation::NoColumn {
                form: SidebarForm::BottomNav
            }
        );
    }

    #[test]
    fn a_typed_intent_produces_one_persisted_preference() {
        assert_eq!(
            apply_sidebar_expansion_intent(
                SidebarExpansionPreference::Expanded,
                SidebarExpansionIntent::Toggle
            ),
            SidebarExpansionPreference::Collapsed
        );
        assert_eq!(
            apply_sidebar_expansion_intent(
                SidebarExpansionPreference::Collapsed,
                SidebarExpansionIntent::Set(SidebarExpansionPreference::Expanded)
            ),
            SidebarExpansionPreference::Expanded
        );
    }
}

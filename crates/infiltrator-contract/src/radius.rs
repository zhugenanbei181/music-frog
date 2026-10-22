//! The corner-radius ladder and the role each rung owns (EFFECTS-002 #7).
//!
//! Every rung owns exactly one role and the roles are mutually exclusive: a
//! centered dialog is the modal rung, a panel-style dialog is the card rung
//! and a bottom sheet is the bottom-sheet rung. There is no fourth "dialog"
//! rung, so the ladder cannot be read two ways.
//!
//! The ladder is published twice, and the two publications cannot drift:
//! [`RadiusRole`] names the role a corner plays, and [`Radius`] is the corner
//! itself — it can only be built from a role, so a surface cannot paint a
//! radius it invented.

/// Micro tag, compact tooltip bubble (4px).
pub const XS: f32 = 4.0;
/// Secondary button, input inner chip, segmented control pill (8px).
pub const SM: f32 = 8.0;
/// Standard control, text input, primary button (10px).
pub const CONTROL: f32 = 10.0;
/// Standard card container, panel-style dialog card (16px).
pub const CARD: f32 = 16.0;
/// Centered modal dialog, centered command palette (24px).
pub const MODAL: f32 = 24.0;
/// Mobile bottom sheet top edge radius (28px).
pub const BOTTOM_SHEET: f32 = 28.0;
/// Fully-rounded pill capsule, status chip (999px).
pub const PILL: f32 = 999.0;

/// The role a rounded corner plays.
///
/// The enum is the vocabulary a surface programs against: it asks for the
/// role and receives the rung the ladder resolved, so two surfaces cannot
/// round the same widget differently.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RadiusRole {
    /// A micro tag or a compact tooltip bubble.
    MicroTag,
    /// A secondary button, an input inner chip, a segmented control pill.
    SecondaryControl,
    /// A standard control, a text input, a primary button.
    Control,
    /// A standard card container or a panel-style dialog card.
    Card,
    /// A centered modal dialog or a centered command palette.
    CenteredDialog,
    /// The top edge of a mobile bottom sheet.
    BottomSheet,
    /// A fully rounded pill capsule or a status chip.
    Pill,
}

impl RadiusRole {
    /// Every role of the ladder.
    pub const ALL: [Self; 7] = [
        Self::MicroTag,
        Self::SecondaryControl,
        Self::Control,
        Self::Card,
        Self::CenteredDialog,
        Self::BottomSheet,
        Self::Pill,
    ];

    /// The rung this role owns, in logical pixels.
    pub const fn rung(self) -> f32 {
        match self {
            Self::MicroTag => XS,
            Self::SecondaryControl => SM,
            Self::Control => CONTROL,
            Self::Card => CARD,
            Self::CenteredDialog => MODAL,
            Self::BottomSheet => BOTTOM_SHEET,
            Self::Pill => PILL,
        }
    }

    /// The stable name of this role.
    pub const fn label(self) -> &'static str {
        match self {
            Self::MicroTag => "micro-tag",
            Self::SecondaryControl => "secondary-control",
            Self::Control => "control",
            Self::Card => "card",
            Self::CenteredDialog => "centered-dialog",
            Self::BottomSheet => "bottom-sheet",
            Self::Pill => "pill",
        }
    }

    /// The role that owns exactly this radius.
    ///
    /// The ladder is a closed set: a radius that is not one of its rungs has
    /// no role, because a corner that belongs to no role is a corner nobody
    /// decided on.
    pub fn from_px(px: f32) -> Option<Self> {
        Self::ALL.into_iter().find(|role| role.rung() == px)
    }
}

/// A corner radius the contract resolved (EFFECTS-002 #7).
///
/// The radius is carried by its role, so the only way to obtain one is to
/// name the role it plays: a surface cannot paint a corner it invented, and
/// two surfaces cannot round the same widget differently.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Radius {
    role: RadiusRole,
}

impl Radius {
    /// The radius that plays this role.
    pub const fn new(role: RadiusRole) -> Self {
        Self { role }
    }

    /// The role this radius plays.
    pub const fn role(self) -> RadiusRole {
        self.role
    }

    /// The length of this radius in logical pixels.
    pub const fn px(self) -> f32 {
        self.role.rung()
    }
}

/// The rung that owns a role.
pub const fn radius_for(role: RadiusRole) -> f32 {
    role.rung()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_role_owns_exactly_one_rung_and_no_rung_is_shared() {
        for role in RadiusRole::ALL {
            assert!(role.rung() > 0.0);
            assert_eq!(radius_for(role), role.rung());
            assert!(!role.label().is_empty());
        }
        let mut rungs = RadiusRole::ALL.map(|role| role.rung());
        rungs.sort_by(|left, right| left.total_cmp(right));
        for pair in rungs.windows(2) {
            assert_ne!(pair[0], pair[1], "two radius roles must not share one rung");
        }
    }

    #[test]
    fn the_dialog_roles_stay_distinct() {
        // A centered dialog is rounder than a panel-style one, and the bottom
        // sheet is rounder than the centered dialog.
        assert_eq!(RadiusRole::CenteredDialog.rung(), MODAL);
        assert_eq!(RadiusRole::Card.rung(), CARD);
        assert_eq!(RadiusRole::BottomSheet.rung(), BOTTOM_SHEET);
        assert!(RadiusRole::CenteredDialog.rung() > RadiusRole::Card.rung());
        assert!(RadiusRole::BottomSheet.rung() > RadiusRole::CenteredDialog.rung());
        assert_eq!(RadiusRole::Pill.rung(), PILL);
        assert!(RadiusRole::Pill.rung() > RadiusRole::BottomSheet.rung());
    }

    #[test]
    fn a_role_is_found_only_by_its_own_rung() {
        assert_eq!(RadiusRole::from_px(16.0), Some(RadiusRole::Card));
        assert_eq!(RadiusRole::from_px(24.0), Some(RadiusRole::CenteredDialog));
        assert_eq!(RadiusRole::from_px(999.0), Some(RadiusRole::Pill));
        // The ladder never rounds a radius it does not publish.
        assert_eq!(RadiusRole::from_px(18.0), None);
        assert_eq!(RadiusRole::from_px(0.0), None);
        for role in RadiusRole::ALL {
            assert_eq!(RadiusRole::from_px(role.rung()), Some(role));
        }
    }

    #[test]
    fn a_radius_carries_the_role_it_plays() {
        let radius = Radius::new(RadiusRole::Card);
        assert_eq!(radius.role(), RadiusRole::Card);
        assert_eq!(radius.px(), CARD);
        assert_eq!(radius.px(), radius_for(RadiusRole::Card));
        for role in RadiusRole::ALL {
            let radius = Radius::new(role);
            assert_eq!(radius.role(), role);
            assert_eq!(radius.px(), role.rung());
        }
    }
}

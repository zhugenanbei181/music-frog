//! Shared mobile dock-band contract.
//!
//! The bottom dock is one geometry owned by one typed state. A surface must
//! not combine a `present` flag with a `visible` flag: that quietly creates a
//! third, illegal state where content is still clipped for a band that no
//! longer draws its dock. Both the reserved region and the floating pill read
//! the same variant.

use serde::{Deserialize, Serialize};

/// Whether the mobile dock band owns a region at the bottom of the window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MobileDockBandState {
    /// No dock band exists; page ground reaches the window's bottom inset.
    #[default]
    Closed,
    /// The band is reserved and its floating dock is mounted. Content is
    /// clipped above the band; the band itself shows only page ground.
    Reserved,
}

/// The only input that can change dock-band ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MobileDockBandIntent {
    /// A compact, primary page needs its dock.
    Reserve,
    /// A page, viewport or lifecycle change retires the dock.
    Close,
}

/// The surface material of a floating dock.
///
/// The dock is a glass veil: it paints alpha, so the page ground reads through
/// it and the capsule reads as a control that floats above the page instead of
/// a plate that covers it. An opaque plate is the failure this type exists to
/// prevent, so the alpha is owned here and every surface renders the same
/// decision. A skin may only choose the veil's tint; the translucency itself
/// is one number with one owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MobileDockMaterial;

impl MobileDockMaterial {
    /// The alpha every floating dock surface paints. 60% keeps the capsule
    /// unmistakably a veil (the page behind it stays visible) while still
    /// lifting the plate clear of the page ground.
    pub const VEIL_ALPHA: u8 = 0x99;

    /// Whether a rendered surface is a veil rather than an opaque plate.
    pub const fn is_translucent() -> bool {
        Self::VEIL_ALPHA < 0xFF
    }

    /// The alpha byte a surface's colour literal must carry.
    pub const fn veil_alpha_byte() -> u8 {
        Self::VEIL_ALPHA
    }
}

impl MobileDockBandState {
    /// Apply one typed intent. Every legal transition is explicit here.
    pub const fn transition(self, intent: MobileDockBandIntent) -> Self {
        match (self, intent) {
            (Self::Closed, MobileDockBandIntent::Reserve) => Self::Reserved,
            (Self::Reserved, MobileDockBandIntent::Close) => Self::Closed,
            (Self::Closed, MobileDockBandIntent::Close)
            | (Self::Reserved, MobileDockBandIntent::Reserve) => self,
        }
    }

    /// Whether the dock pill is mounted.
    pub const fn draws_dock(self) -> bool {
        matches!(self, Self::Reserved)
    }

    /// Whether page content must stop above the dock band.
    pub const fn reserves_content_band(self) -> bool {
        matches!(self, Self::Reserved)
    }
}

#[cfg(test)]
mod tests {
    use super::{MobileDockBandIntent, MobileDockBandState, MobileDockMaterial};

    #[test]
    fn reservation_and_drawing_are_one_exclusive_state() {
        for state in [MobileDockBandState::Closed, MobileDockBandState::Reserved] {
            assert_eq!(state.draws_dock(), state.reserves_content_band());
        }
    }

    #[test]
    fn only_typed_intents_change_the_state() {
        let reserved = MobileDockBandState::default().transition(MobileDockBandIntent::Reserve);
        assert_eq!(reserved, MobileDockBandState::Reserved);
        assert_eq!(
            reserved.transition(MobileDockBandIntent::Reserve),
            MobileDockBandState::Reserved
        );
        assert_eq!(
            reserved.transition(MobileDockBandIntent::Close),
            MobileDockBandState::Closed
        );
    }

    #[test]
    fn the_dock_veil_is_never_an_opaque_plate() {
        assert!(MobileDockMaterial::is_translucent());
        assert!(MobileDockMaterial::veil_alpha_byte() < 0xFF);
        assert!(MobileDockMaterial::veil_alpha_byte() > 0x00);
    }

    #[test]
    fn the_veil_alpha_is_one_number_for_every_surface() {
        assert_eq!(
            MobileDockMaterial::veil_alpha_byte(),
            MobileDockMaterial::VEIL_ALPHA
        );
    }
}

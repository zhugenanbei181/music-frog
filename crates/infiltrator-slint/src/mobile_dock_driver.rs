//! Host projection of the shared mobile dock-band state into Slint.
//!
//! The window's viewport tier and page identity are observed facts. This
//! module folds them through the shared presentation projector and publishes
//! one typed Slint enum; the `.slint` tree never writes a dock flag.

use crate::{AppWindow, MobileDockBandState};
use infiltrator_contract::mobile_chrome;
use infiltrator_presentation::mobile_chrome_projection::{
    MobileDockBandFacts, initial_mobile_dock_band, project_mobile_dock_band,
};

/// Convert the contract state into the generated Slint enum. This is the only
/// adapter between the two closed vocabularies.
pub(crate) fn to_ui_state(state: mobile_chrome::MobileDockBandState) -> MobileDockBandState {
    match state {
        mobile_chrome::MobileDockBandState::Closed => MobileDockBandState::Closed,
        mobile_chrome::MobileDockBandState::Reserved => MobileDockBandState::Reserved,
    }
}

fn to_contract_state(state: MobileDockBandState) -> mobile_chrome::MobileDockBandState {
    match state {
        MobileDockBandState::Closed => mobile_chrome::MobileDockBandState::Closed,
        MobileDockBandState::Reserved => mobile_chrome::MobileDockBandState::Reserved,
    }
}

fn facts(window: &AppWindow) -> MobileDockBandFacts {
    MobileDockBandFacts {
        compact_layout: window.get_compact_layout(),
        primary_page: window.get_primary_page(),
    }
}

/// Publish the initial state before the first owner snapshot is observed.
pub(crate) fn install(window: &AppWindow) {
    let state = initial_mobile_dock_band(facts(window));
    window.set_mobile_dock_band_state(to_ui_state(state));
}

/// Re-project after any viewport or page change. The previous typed state and
/// the observed facts meet only in the shared pure transition.
pub(crate) fn project(window: &AppWindow) {
    let previous = to_contract_state(window.get_mobile_dock_band_state());
    let state = project_mobile_dock_band(previous, facts(window));
    window.set_mobile_dock_band_state(to_ui_state(state));
}

#[cfg(test)]
mod tests {
    use super::{initial_mobile_dock_band, project_mobile_dock_band, to_ui_state};
    use crate::MobileDockBandState;
    use infiltrator_contract::mobile_chrome;
    use infiltrator_presentation::mobile_chrome_projection::MobileDockBandFacts;

    #[test]
    fn every_contract_variant_has_one_ui_variant() {
        assert_eq!(
            to_ui_state(mobile_chrome::MobileDockBandState::Closed),
            MobileDockBandState::Closed
        );
        assert_eq!(
            to_ui_state(mobile_chrome::MobileDockBandState::Reserved),
            MobileDockBandState::Reserved
        );
    }

    #[test]
    fn the_projection_publishes_one_state_for_observed_facts() {
        let reserved = initial_mobile_dock_band(MobileDockBandFacts {
            compact_layout: true,
            primary_page: true,
        });
        assert_eq!(to_ui_state(reserved), MobileDockBandState::Reserved);
    }

    #[test]
    fn every_host_reprojection_uses_the_typed_transition() {
        let previous = mobile_chrome::MobileDockBandState::Reserved;
        let closed = project_mobile_dock_band(
            previous,
            MobileDockBandFacts {
                compact_layout: true,
                primary_page: false,
            },
        );
        assert_eq!(to_ui_state(closed), MobileDockBandState::Closed);
    }
}

//! Host projection of the shared mobile dock-band state into Slint.
//!
//! The window's viewport tier and page identity are observed facts. This
//! module folds them through the shared presentation projector and publishes
//! one typed Slint enum; the `.slint` tree never writes a dock flag.

use crate::{AppWindow, MobileDockBandState, NavigationForm};
use infiltrator_contract::mobile_chrome;
use infiltrator_contract::responsive_viewport::SidebarForm;
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

/// The one adapter between the shell's published form and the contract's.
///
/// The direction is one-way on purpose: the exported ladder owns the decision
/// (`ViewportLadder.shell_navigation` binds the property in `.slint`), so Rust
/// only reads the vocabulary back. A new form in either language is a compile
/// error here, which is the point of naming the form instead of a boolean.
/// The capture marker reads the same form back through this one adapter.
pub(crate) fn to_contract_form(form: NavigationForm) -> SidebarForm {
    match form {
        NavigationForm::BottomNav => SidebarForm::BottomNav,
        NavigationForm::Rail => SidebarForm::Rail,
        NavigationForm::Standard => SidebarForm::Standard,
        NavigationForm::Wide => SidebarForm::Wide,
    }
}

fn facts(window: &AppWindow) -> MobileDockBandFacts {
    MobileDockBandFacts {
        navigation: to_contract_form(window.get_navigation_form()),
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
    use super::{
        initial_mobile_dock_band, project_mobile_dock_band, to_contract_form, to_ui_state,
    };
    use crate::MobileDockBandState;
    use crate::NavigationForm;
    use infiltrator_contract::mobile_chrome;
    use infiltrator_contract::responsive_viewport::{SidebarForm, TIERS, ViewportTier};
    use infiltrator_presentation::mobile_chrome_projection::MobileDockBandFacts;
    use infiltrator_presentation::mobile_chrome_projection::intent_for_mobile_dock_band;

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
        // The navigation vocabulary crosses the boundary with nothing dropped
        // and nothing invented on either side.
        for (ui, contract) in [
            (NavigationForm::BottomNav, SidebarForm::BottomNav),
            (NavigationForm::Rail, SidebarForm::Rail),
            (NavigationForm::Standard, SidebarForm::Standard),
            (NavigationForm::Wide, SidebarForm::Wide),
        ] {
            assert_eq!(to_contract_form(ui), contract);
        }
    }

    #[test]
    fn the_ladder_is_the_only_author_of_the_desktop_form() {
        // The surface publishes `ViewportLadder.shell_navigation`, so the host
        // projection must agree with the contract for every tier, on both
        // surface kinds. A desktop medium window gets a rail and therefore no
        // dock band; a phone gets the bar at every width.
        for tier in TIERS {
            assert_eq!(tier.shell_navigation(false), tier.sidebar_form());
            assert_eq!(tier.shell_navigation(true), SidebarForm::BottomNav);
            let intent = intent_for_mobile_dock_band(MobileDockBandFacts {
                navigation: tier.sidebar_form(),
                primary_page: true,
            });
            assert_eq!(
                intent == mobile_chrome::MobileDockBandIntent::Reserve,
                tier.sidebar_form() == SidebarForm::BottomNav,
                "{} reserves a band exactly when it mounts a bar",
                tier.id()
            );
        }
        assert_eq!(ViewportTier::Medium.sidebar_form(), SidebarForm::Rail);
    }

    #[test]
    fn the_projection_publishes_one_state_for_observed_facts() {
        let reserved = initial_mobile_dock_band(MobileDockBandFacts {
            navigation: SidebarForm::BottomNav,
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
                navigation: SidebarForm::BottomNav,
                primary_page: false,
            },
        );
        assert_eq!(to_ui_state(closed), MobileDockBandState::Closed);
    }

    #[test]
    fn a_rail_window_reprojects_out_of_the_reserved_band() {
        // The window was a phone-shaped frame and grew into a medium desktop
        // window: the typed transition must retire the band, not keep it.
        let reserved = mobile_chrome::MobileDockBandState::Reserved;
        let closed = project_mobile_dock_band(
            reserved,
            MobileDockBandFacts {
                navigation: SidebarForm::Rail,
                primary_page: true,
            },
        );
        assert_eq!(to_ui_state(closed), MobileDockBandState::Closed);
    }
}

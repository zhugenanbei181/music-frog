//! Pure mobile dock-band projection shared by UI surfaces.

use infiltrator_contract::mobile_chrome::{MobileDockBandIntent, MobileDockBandState};
use infiltrator_contract::responsive_viewport::SidebarForm;

/// The two binary presentation facts that decide dock-band ownership.
///
/// These are independent observed facts, not stages: the typed enum returned
/// by [`project_mobile_dock_band`] is the only stored state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MobileDockBandFacts {
    /// The navigation form the shell actually mounted.
    ///
    /// This used to be a `compact_layout` boolean, which conflated page
    /// density with navigation: a 720px desktop window was "compact" because
    /// its cards stack, so the dock reserved a phone's bottom band for a
    /// window that navigates with an icon rail. The band belongs to the bar,
    /// so the bar's own typed form is what decides.
    pub navigation: SidebarForm,
    pub primary_page: bool,
}

/// Fold the observed viewport/page facts into one typed intent.
pub fn intent_for_mobile_dock_band(facts: MobileDockBandFacts) -> MobileDockBandIntent {
    if facts.navigation == SidebarForm::BottomNav && facts.primary_page {
        MobileDockBandIntent::Reserve
    } else {
        MobileDockBandIntent::Close
    }
}

/// Project the owner's previous state and the observed facts into one state.
pub fn project_mobile_dock_band(
    previous: MobileDockBandState,
    facts: MobileDockBandFacts,
) -> MobileDockBandState {
    previous.transition(intent_for_mobile_dock_band(facts))
}

/// Project the dock band from its initial state without a retained UI flag.
pub fn initial_mobile_dock_band(facts: MobileDockBandFacts) -> MobileDockBandState {
    project_mobile_dock_band(MobileDockBandState::default(), facts)
}

#[cfg(test)]
mod tests {
    use super::{MobileDockBandFacts, initial_mobile_dock_band, project_mobile_dock_band};
    use infiltrator_contract::mobile_chrome::{MobileDockBandIntent, MobileDockBandState};
    use infiltrator_contract::responsive_viewport::SidebarForm;

    const FACTS: MobileDockBandFacts = MobileDockBandFacts {
        navigation: SidebarForm::BottomNav,
        primary_page: true,
    };

    #[test]
    fn compact_primary_pages_reserve_one_band() {
        let reserved = initial_mobile_dock_band(FACTS);
        assert_eq!(reserved, MobileDockBandState::Reserved);
        assert!(reserved.draws_dock());
        assert!(reserved.reserves_content_band());
    }

    #[test]
    fn closed_facts_retire_both_the_pill_and_the_reservation() {
        let reserved = initial_mobile_dock_band(FACTS);
        let closed = project_mobile_dock_band(
            reserved,
            MobileDockBandFacts {
                primary_page: false,
                ..FACTS
            },
        );
        assert_eq!(closed, MobileDockBandState::Closed);
        assert!(!closed.draws_dock());
        assert!(!closed.reserves_content_band());
    }

    #[test]
    fn the_fold_publishes_a_typed_intent_not_a_flag() {
        assert_eq!(
            super::intent_for_mobile_dock_band(FACTS),
            MobileDockBandIntent::Reserve
        );
        assert_eq!(
            super::intent_for_mobile_dock_band(MobileDockBandFacts {
                navigation: SidebarForm::Standard,
                ..FACTS
            }),
            MobileDockBandIntent::Close
        );
    }

    #[test]
    fn an_icon_rail_never_buys_a_bottom_band() {
        // The rail is a medium desktop window's navigation: it owns a column,
        // so the page keeps the band the bar would have needed. Reserving one
        // anyway is the 72u of dead air SPACEUTIL-006 measured at 720x480.
        for navigation in [SidebarForm::Rail, SidebarForm::Standard, SidebarForm::Wide] {
            assert_eq!(
                super::intent_for_mobile_dock_band(MobileDockBandFacts {
                    navigation,
                    primary_page: true,
                }),
                MobileDockBandIntent::Close,
                "{navigation:?} mounts no bar, so it reserves no band"
            );
        }
    }
}

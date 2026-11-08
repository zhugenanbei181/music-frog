//! Pure mobile dock-band projection shared by UI surfaces.

use infiltrator_contract::mobile_chrome::{MobileDockBandIntent, MobileDockBandState};

/// The two binary presentation facts that decide dock-band ownership.
///
/// These are independent observed facts, not stages: the typed enum returned
/// by [`project_mobile_dock_band`] is the only stored state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MobileDockBandFacts {
    pub compact_layout: bool,
    pub primary_page: bool,
}

/// Fold the observed viewport/page facts into one typed intent.
pub fn intent_for_mobile_dock_band(facts: MobileDockBandFacts) -> MobileDockBandIntent {
    if facts.compact_layout && facts.primary_page {
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

    const FACTS: MobileDockBandFacts = MobileDockBandFacts {
        compact_layout: true,
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
                compact_layout: false,
                ..FACTS
            }),
            MobileDockBandIntent::Close
        );
    }
}

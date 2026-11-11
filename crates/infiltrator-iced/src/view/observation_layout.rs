//! Typed layout ownership shared by standalone observation pages.

use crate::state::AppState;
use iced::Length;

/// Which frame owns an observation list's remaining page-body height.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ObservationListOwner {
    /// The runtime page scrolls a stack of cards; each list keeps its own
    /// bounded viewport so the following cards stay reachable.
    RuntimeCard,
    /// A standalone page's card is the page body; its list fills the leftover
    /// height instead of leaving a second ground below the scroll box.
    PageBody,
}

impl ObservationListOwner {
    pub fn list_height(self) -> Length {
        match self {
            Self::RuntimeCard => Length::Fixed(260.0),
            Self::PageBody => Length::Fill,
        }
    }
}

/// The destination label is a primary navigation fact, not optional detail.
/// A narrow card puts it on its own line so fixed protocol/action controls
/// cannot squeeze it into a column of broken glyphs.
pub fn connection_rows_use_stacked_headline(state: &AppState) -> bool {
    state.shell.viewport.tier.is_narrow()
}

/// A standalone search-highlight row keeps its endpoint on one line. Native
/// capture geometry measures that endpoint inside the row card; word wrapping
/// can place it below the card's visible head, which is a real clipped receipt.
pub fn connection_rows_use_single_line_text(state: &AppState) -> bool {
    connection_rows_use_stacked_headline(state)
}

#[cfg(test)]
#[path = "../../tests/gui/observation_layout_tests.rs"]
mod tests;

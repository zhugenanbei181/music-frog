//! The parity capture scenario of the Iced surface.
//!
//! A capture scenario names one business of the shared parity catalogue. The
//! activation opens the same surface state a user gesture opens, and the
//! bounds are the real on-screen geometry of the surface that was opened. A
//! receipt therefore proves an interaction and never a route change.
//!
//! The set of businesses the surface carries is closed and typed: a business
//! the surface does not carry has no scenario, so the capture path can never
//! publish it as activated.

use crate::demo::interaction::activate;
use crate::demo::interaction::ready;
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::parity::FeatureId;

/// Whether the Iced surface carries a business of the shared catalogue.
///
/// The catalogue names the page a business lives on; an empty page means the
/// surface does not implement the business at all.
pub(crate) fn carried(feature: FeatureId) -> bool {
    !feature.spec().iced_page.is_empty()
}

/// Opens the business a capture scenario names.
///
/// A business the surface does not carry is never opened, so the capture can
/// not publish it as activated.
pub(crate) fn open_business(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    if !carried(feature) {
        return Task::none();
    }
    activate::activate(state, feature)
}

/// Whether the surface really opened the business.
///
/// The answer is read from the real surface state, so a demo that only
/// repainted its page can never claim the business.
pub(crate) fn business_opened(state: &AppState, feature: FeatureId) -> bool {
    carried(feature) && ready::ready(state, feature)
}

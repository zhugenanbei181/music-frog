//! The rules page performs its own search gesture.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::parity::FeatureId;

/// The rule the demo searches for: exactly one row of the demo rule list.
pub(super) const RULE_SEARCH_QUERY: &str = "google.com";

pub(super) fn activate(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    match feature {
        FeatureId::RulesSearchHighlight => {
            // The user types a rule filter: the shared rules page narrows to
            // the matching rows and highlights the matched text.
            let _ = state.update(Message::FilterRules(RULE_SEARCH_QUERY.into()));
            Task::none()
        }
        _ => Task::none(),
    }
}

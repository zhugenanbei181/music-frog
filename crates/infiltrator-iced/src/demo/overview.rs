//! The Overview page performs its own card-reorder gesture.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::overview_layout::OverviewCardKind;
use infiltrator_contract::parity::FeatureId;

pub(super) fn activate(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    match feature {
        FeatureId::OverviewCardReorder => {
            // The user moves one Overview card one slot up: the shared layout
            // order changes and the page assembles its cards from that order.
            assert_eq!(
                state
                    .update(Message::MoveOverviewCardUp(OverviewCardKind::Traffic))
                    .units(),
                0
            );
            Task::none()
        }
        _ => Task::none(),
    }
}

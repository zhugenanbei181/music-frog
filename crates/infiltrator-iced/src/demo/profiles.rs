//! The profiles page performs its own wizard and confirmation gestures.
use crate::state::AppState;
use crate::types::app::ConfirmAction;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::parity::FeatureId;

pub(super) fn activate(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    match feature {
        FeatureId::ProfilesAggregatorWizard => {
            // The user opens the multi-profile aggregation wizard.
            state.update(Message::OpenAggregatorModal);
            Task::none()
        }
        FeatureId::ProfilesDeleteConfirm => {
            // The user asks to delete a profile: the surface asks for the
            // confirmation before it touches the profile store.
            let profile = state
                .profile
                .profiles
                .iter()
                .find(|profile| !profile.active)
                .or_else(|| state.profile.profiles.first())
                .map(|profile| profile.name.clone())
                .expect("the demo carries profiles to delete");
            assert_eq!(
                state
                    .update(Message::RequestConfirmation(ConfirmAction::DeleteProfile(
                        profile
                    )))
                    .units(),
                0
            );
            Task::none()
        }
        _ => Task::none(),
    }
}

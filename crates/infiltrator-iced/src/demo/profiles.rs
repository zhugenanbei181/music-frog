//! The profiles page performs its own wizard, confirmation and import gestures.
use crate::state::AppState;
use crate::types::app::ConfirmAction;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::parity::FeatureId;

/// The subscription URL the capture types into the shared import form.
pub(super) const IMPORT_URL: &str = "https://sub.example.com/api/v1/client/subscribe?token=demo";

pub(super) fn activate(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    match feature {
        FeatureId::ProfilesAggregatorWizard => {
            // The user opens the multi-profile aggregation wizard. The
            // template catalogue the wizard offers is read from the user's
            // own store, so the demo keeps that task out of the session.
            let _ = state.update(Message::OpenAggregatorModal);
            Task::none()
        }
        FeatureId::ProfilesImportModal => {
            // The user types the subscription URL into the import form: the
            // shared import draft carries it until the import command runs.
            assert_eq!(
                state
                    .update(Message::UpdateImportUrl(IMPORT_URL.to_string()))
                    .units(),
                0
            );
            assert_eq!(
                state
                    .update(Message::UpdateImportName("机场订阅".to_string()))
                    .units(),
                0
            );
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

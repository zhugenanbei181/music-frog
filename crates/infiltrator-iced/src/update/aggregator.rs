//! DUAL-08 aggregator modal handlers.
//!
//! The modal owns no aggregation logic: it edits the shared
//! [`AggregationDraft`] fields, asks the shared
//! `ProfileAggregationApplication` for the preview, and stores the returned
//! `AggregationReport` verbatim. The "save as new profile" action routes
//! through the same shared application so the source profiles are never
//! touched.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::profile_aggregation_application::ProfileAggregationApplication;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_contract::aggregator::AggregationDraft;
use infiltrator_contract::error::InfiltratorError;

impl AppState {
    /// The draft currently edited in the modal.
    pub(crate) fn aggregator_draft(&self) -> AggregationDraft {
        AggregationDraft {
            source_profiles: self.profile.aggregator_selected_profiles.clone(),
            target_name: self.profile.aggregator_name_input.clone(),
            deduplicate: self.profile.aggregator_deduplicate,
            deduplicate_names: true,
            geo_cluster: self.profile.aggregator_geo_cluster,
            generate_groups: self.profile.aggregator_generate_groups,
            remove_emojis: self.profile.aggregator_remove_emojis,
        }
    }

    pub(super) fn update_aggregator(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenAggregatorModal => {
                self.profile.aggregator_modal_open = true;
                self.profile.aggregator_selected_profiles = self
                    .profile
                    .profiles
                    .iter()
                    .map(|profile| profile.name.clone())
                    .collect();
                self.profile.aggregator_report = None;
                Task::none()
            }
            Message::CloseAggregatorModal => {
                self.profile.aggregator_modal_open = false;
                Task::none()
            }
            Message::ToggleAggregatorProfileSelection(name) => {
                let selected = &mut self.profile.aggregator_selected_profiles;
                match selected.iter().position(|entry| entry == &name) {
                    Some(position) => {
                        selected.remove(position);
                    }
                    None => selected.push(name),
                }
                // A changed source selection invalidates the previous preview.
                self.profile.aggregator_report = None;
                Task::none()
            }
            Message::UpdateAggregatorName(name) => {
                self.profile.aggregator_name_input = name;
                Task::none()
            }
            Message::ToggleAggregatorDeduplicate => {
                self.profile.aggregator_deduplicate = !self.profile.aggregator_deduplicate;
                self.profile.aggregator_report = None;
                Task::none()
            }
            Message::ToggleAggregatorGeoCluster => {
                self.profile.aggregator_geo_cluster = !self.profile.aggregator_geo_cluster;
                self.profile.aggregator_report = None;
                Task::none()
            }
            Message::ToggleAggregatorGenerateGroups => {
                self.profile.aggregator_generate_groups = !self.profile.aggregator_generate_groups;
                self.profile.aggregator_report = None;
                Task::none()
            }
            Message::ToggleAggregatorRemoveEmojis => {
                self.profile.aggregator_remove_emojis = !self.profile.aggregator_remove_emojis;
                self.profile.aggregator_report = None;
                Task::none()
            }
            Message::PreviewProfileAggregation => {
                if self.profile.aggregator_selected_profiles.is_empty() {
                    return Task::done(Message::ShowToast(
                        "Select at least one source profile".to_string(),
                        ToastStatus::Error,
                    ));
                }
                self.profile.is_aggregating = true;
                let draft = self.aggregator_draft();
                Task::perform(
                    async move {
                        let store = crate::configs_dir::config_manager().await?;
                        let application =
                            ProfileAggregationApplication::new(ProfileApplication::new(store));
                        application
                            .preview(&draft)
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))
                    },
                    Message::AggregationPreviewFinished,
                )
            }
            Message::AggregationPreviewFinished(result) => {
                self.profile.is_aggregating = false;
                match result {
                    Ok(report) => {
                        self.profile.aggregator_report = Some(report);
                        Task::none()
                    }
                    Err(error) => {
                        self.set_error(&error);
                        Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::CreateAggregatedProfile => {
                let draft = self.aggregator_draft();
                if draft.target_name.trim().is_empty() {
                    return Task::done(Message::ShowToast(
                        "Aggregated profile name is required".to_string(),
                        ToastStatus::Error,
                    ));
                }
                self.profile.is_aggregating = true;
                Task::perform(
                    async move {
                        let store = crate::configs_dir::config_manager().await?;
                        let application =
                            ProfileAggregationApplication::new(ProfileApplication::new(store));
                        application
                            .create_profile(&draft)
                            .await
                            .map(|_| draft.target_name.clone())
                            .map_err(|failure| InfiltratorError::Config(failure.message))
                    },
                    Message::AggregatedProfileCreated,
                )
            }
            Message::AggregatedProfileCreated(result) => {
                self.profile.is_aggregating = false;
                match result {
                    Ok(name) => Task::batch(vec![
                        Task::done(Message::LoadProfiles),
                        Task::done(Message::ShowToast(
                            format!("Aggregated profile '{name}' created"),
                            ToastStatus::Success,
                        )),
                    ]),
                    Err(error) => {
                        self.set_error(&error);
                        Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                    }
                }
            }
            _ => Task::none(),
        }
    }
}

//! DUAL-08 aggregator modal handlers.
//!
//! The modal owns no aggregation logic: it edits the shared
//! [`AggregationDraft`] fields, asks the shared
//! `ProfileAggregationApplication` for the preview, and stores the returned
//! `AggregationReport` verbatim. Save/activate/re-aggregate and the template
//! library all route through the same shared application, so the source
//! profiles are never touched.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::profile_aggregation_application::ProfileAggregationApplication;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_contract::aggregator::{AggregationDraft, AggregationRenameRule};
use infiltrator_contract::error::InfiltratorError;
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};

type AggregationOutcome = infiltrator_contract::aggregator::AggregatedProfileOutcome;

impl AppState {
    /// The draft currently edited in the modal. Err carries the offending
    /// rename line so the surface can point at it instead of silently
    /// dropping a malformed rule.
    pub(crate) fn aggregator_draft(&self) -> Result<AggregationDraft, String> {
        Ok(AggregationDraft {
            source_profiles: self.profile.aggregator_selected_profiles.clone(),
            target_name: self.profile.aggregator_name_input.clone(),
            deduplicate: self.profile.aggregator_deduplicate,
            deduplicate_names: true,
            geo_cluster: self.profile.aggregator_geo_cluster,
            generate_groups: self.profile.aggregator_generate_groups,
            remove_emojis: self.profile.aggregator_remove_emojis,
            rename_rules: AggregationRenameRule::parse_list(&self.profile.aggregator_renames)?,
            custom_groups: self.profile.aggregator_custom_groups.clone(),
            availability_precheck: self.profile.aggregator_availability_precheck,
            activate_after_create: self.profile.aggregator_activate_after_create,
        })
    }

    /// Build the draft for a submit action, rendering a localized toast when
    /// the rename text is malformed.
    fn aggregator_submit_draft(&self) -> Result<AggregationDraft, Task<Message>> {
        self.aggregator_draft().map_err(|line| {
            let lang = Lang(&self.shell.lang);
            let text = interpolate(
                &lang.tr("aggregator_rename_invalid"),
                &[("line", line.as_str())],
            );
            Task::done(Message::ShowToast(text, ToastStatus::Error))
        })
    }

    fn aggregator_application(
        store: std::sync::Arc<dyn infiltrator_ports::profile_store::ProfileStore>,
    ) -> ProfileAggregationApplication {
        ProfileAggregationApplication::new(ProfileApplication::new(store))
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
                Task::done(Message::LoadAggregatorTemplates)
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
            Message::ToggleAggregatorAvailabilityPrecheck => {
                self.profile.aggregator_availability_precheck =
                    !self.profile.aggregator_availability_precheck;
                self.profile.aggregator_report = None;
                Task::none()
            }
            Message::ToggleAggregatorActivateAfterCreate => {
                self.profile.aggregator_activate_after_create =
                    !self.profile.aggregator_activate_after_create;
                Task::none()
            }
            Message::UpdateAggregatorRenames(text) => {
                self.profile.aggregator_renames = text;
                self.profile.aggregator_report = None;
                Task::none()
            }
            Message::UpdateAggregatorTemplateName(name) => {
                self.profile.aggregator_template_name = name;
                Task::none()
            }
            Message::UpdateAggregatorCustomGroupName(name) => {
                self.profile.aggregator_custom_name = name;
                Task::none()
            }
            Message::UpdateAggregatorCustomGroupKeywords(keywords) => {
                self.profile.aggregator_custom_keywords = keywords;
                Task::none()
            }
            Message::AddAggregatorCustomGroup => {
                let name = self.profile.aggregator_custom_name.trim().to_string();
                if name.is_empty() {
                    let lang = Lang(&self.shell.lang);
                    return Task::done(Message::ShowToast(
                        lang.tr("aggregator_custom_name_required").to_string(),
                        ToastStatus::Error,
                    ));
                }
                let keywords = self
                    .profile
                    .aggregator_custom_keywords
                    .split([',', '，', ';'])
                    .map(str::trim)
                    .filter(|keyword| !keyword.is_empty())
                    .map(str::to_string)
                    .collect();
                self.profile.aggregator_custom_groups.push(
                    infiltrator_contract::aggregator::AggregationCustomGroup {
                        name,
                        group_type: "select".to_string(),
                        member_keywords: keywords,
                    },
                );
                self.profile.aggregator_custom_name.clear();
                self.profile.aggregator_custom_keywords.clear();
                self.profile.aggregator_report = None;
                Task::none()
            }
            Message::RemoveAggregatorCustomGroup(index) => {
                if index < self.profile.aggregator_custom_groups.len() {
                    self.profile.aggregator_custom_groups.remove(index);
                    self.profile.aggregator_report = None;
                }
                Task::none()
            }
            Message::LoadAggregatorTemplates => Task::perform(
                async {
                    let store = crate::configs_dir::config_manager().await?;
                    Self::aggregator_application(store)
                        .list_templates()
                        .await
                        .map_err(|failure| InfiltratorError::Config(failure.message))
                },
                Message::AggregatorTemplatesLoaded,
            ),
            Message::AggregatorTemplatesLoaded(result) => match result {
                Ok(templates) => {
                    self.profile.aggregator_templates = templates;
                    Task::none()
                }
                Err(error) => {
                    self.set_error(&error);
                    Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                }
            },
            Message::ApplyAggregatorTemplate(name) => {
                let Some(template) = self
                    .profile
                    .aggregator_templates
                    .iter()
                    .find(|template| template.name == name)
                    .cloned()
                else {
                    let lang = Lang(&self.shell.lang);
                    return Task::done(Message::ShowToast(
                        interpolate(
                            &lang.tr("aggregator_template_missing"),
                            &[("name", name.as_str())],
                        ),
                        ToastStatus::Error,
                    ));
                };
                self.apply_aggregator_template(&template.draft);
                self.profile.aggregator_report = None;
                self.profile.aggregator_template_name = template.name;
                Task::none()
            }
            Message::SaveAggregatorTemplate => {
                let draft = match self.aggregator_submit_draft() {
                    Ok(draft) => draft,
                    Err(task) => return task,
                };
                let name = self.profile.aggregator_template_name.clone();
                Task::perform(
                    async move {
                        let store = crate::configs_dir::config_manager().await?;
                        Self::aggregator_application(store)
                            .save_template(&name, &draft)
                            .await
                            .map(|template| template.name)
                            .map_err(|failure| InfiltratorError::Config(failure.message))
                    },
                    Message::AggregatorTemplateSaved,
                )
            }
            Message::AggregatorTemplateSaved(result) => match result {
                Ok(name) => Task::batch(vec![
                    Task::done(Message::LoadAggregatorTemplates),
                    Task::done(Message::ShowToast(
                        interpolate(
                            &Lang(&self.shell.lang).tr("aggregator_template_saved"),
                            &[("name", name.as_str())],
                        ),
                        ToastStatus::Success,
                    )),
                ]),
                Err(error) => {
                    self.set_error(&error);
                    Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                }
            },
            Message::DeleteAggregatorTemplate(name) => Task::perform(
                async move {
                    let store = crate::configs_dir::config_manager().await?;
                    Self::aggregator_application(store)
                        .delete_template(&name)
                        .await
                        .map(|removed| (name, removed))
                        .map_err(|failure| InfiltratorError::Config(failure.message))
                },
                Message::AggregatorTemplateDeleted,
            ),
            Message::AggregatorTemplateDeleted(result) => match result {
                Ok((_name, true)) => Task::done(Message::LoadAggregatorTemplates),
                Ok((name, false)) => {
                    let lang = Lang(&self.shell.lang);
                    Task::batch(vec![
                        Task::done(Message::ShowToast(
                            interpolate(
                                &lang.tr("aggregator_template_missing"),
                                &[("name", name.as_str())],
                            ),
                            ToastStatus::Warning,
                        )),
                        Task::done(Message::LoadAggregatorTemplates),
                    ])
                }
                Err(error) => {
                    self.set_error(&error);
                    Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                }
            },
            Message::PreviewProfileAggregation => {
                if self.profile.aggregator_selected_profiles.is_empty() {
                    let lang = Lang(&self.shell.lang);
                    return Task::done(Message::ShowToast(
                        lang.tr("aggregator_select_source_required").to_string(),
                        ToastStatus::Error,
                    ));
                }
                let draft = match self.aggregator_submit_draft() {
                    Ok(draft) => draft,
                    Err(task) => return task,
                };
                self.profile.is_aggregating = true;
                Task::perform(
                    async move {
                        let store = crate::configs_dir::config_manager().await?;
                        Self::aggregator_application(store)
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
                if self.profile.aggregator_name_input.trim().is_empty() {
                    let lang = Lang(&self.shell.lang);
                    return Task::done(Message::ShowToast(
                        lang.tr("aggregator_name_required").to_string(),
                        ToastStatus::Error,
                    ));
                }
                let draft = match self.aggregator_submit_draft() {
                    Ok(draft) => draft,
                    Err(task) => return task,
                };
                self.profile.is_aggregating = true;
                // DUAL-08-12: the host runtime is the shared activation seam;
                // without one the shared path still switches the active profile.
                let runtime = self.runtime.runtime.clone();
                Task::perform(
                    async move {
                        let store = crate::configs_dir::config_manager().await?;
                        Self::aggregator_application(store)
                            .create_profile_with_runtime(runtime, &draft)
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))
                    },
                    Message::AggregatedProfileCreated,
                )
            }
            Message::AggregatedProfileCreated(result) => {
                self.profile.is_aggregating = false;
                self.aggregation_outcome_tasks(result, "aggregator_created")
            }
            Message::ReAggregateProfile(template_name) => {
                let runtime = self.runtime.runtime.clone();
                Task::perform(
                    async move {
                        let store = crate::configs_dir::config_manager().await?;
                        Self::aggregator_application(store)
                            .reaggregate(runtime, &template_name)
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))
                    },
                    Message::AggregationReaggregated,
                )
            }
            Message::AggregationReaggregated(result) => {
                self.aggregation_outcome_tasks(result, "aggregator_refreshed")
            }
            _ => Task::none(),
        }
    }

    /// Copy a saved template into the wizard's own fields (the wizard remains
    /// the only editor of the draft).
    fn apply_aggregator_template(&mut self, draft: &AggregationDraft) {
        self.profile.aggregator_selected_profiles = draft.source_profiles.clone();
        self.profile.aggregator_name_input = draft.target_name.clone();
        self.profile.aggregator_deduplicate = draft.deduplicate;
        self.profile.aggregator_geo_cluster = draft.geo_cluster;
        self.profile.aggregator_generate_groups = draft.generate_groups;
        self.profile.aggregator_remove_emojis = draft.remove_emojis;
        self.profile.aggregator_availability_precheck = draft.availability_precheck;
        self.profile.aggregator_activate_after_create = draft.activate_after_create;
        self.profile.aggregator_renames = AggregationRenameRule::to_text(&draft.rename_rules);
        self.profile.aggregator_custom_groups = draft.custom_groups.clone();
    }

    /// Shared tail of the create/re-aggregate flows: reload the profile list,
    /// refresh templates, and toast the activation truth the outcome carries.
    fn aggregation_outcome_tasks(
        &mut self,
        result: Result<AggregationOutcome, InfiltratorError>,
        key: &str,
    ) -> Task<Message> {
        match result {
            Ok(outcome) => {
                let lang = Lang(&self.shell.lang);
                let state_key = if outcome.core_reloaded {
                    "aggregator_state_reloaded"
                } else if outcome.activated {
                    "aggregator_state_switched"
                } else {
                    "aggregator_state_saved"
                };
                let text = interpolate(
                    &lang.tr(key),
                    &[
                        ("name", outcome.profile_name.as_str()),
                        ("state", lang.tr(state_key).as_ref()),
                    ],
                );
                Task::batch(vec![
                    Task::done(Message::LoadProfiles),
                    Task::done(Message::LoadAggregatorTemplates),
                    Task::done(Message::ShowToast(text, ToastStatus::Success)),
                ])
            }
            Err(error) => {
                self.set_error(&error);
                Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
            }
        }
    }
}

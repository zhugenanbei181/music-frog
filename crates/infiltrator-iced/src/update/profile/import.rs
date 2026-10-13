//! Profile import handlers: subscription-URL import and local YAML file
//! import (file picker, metadata fields, activation).

use crate::configs_dir::config_manager;
use crate::configuration::profiles_for_mihomo;
use crate::host::storage::{subscription_import_port, subscription_source};
use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use crate::update::core::profile_apply::activate_profile;
use iced::Task;
use infiltrator_application::profile_application::{
    ProfileActivationOutcome, ProfileRuntimeAccess,
};
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceState;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::config::validate_yaml;
use infiltrator_domain::profile_converter::ProfileConverter;
use infiltrator_domain::profiles::sanitize_profile_name;
use infiltrator_ports::runtime_gateway::ActiveProfileReferenceApplier;
use infiltrator_shared::locales::{Lang, Localizer};
use std::path;
use tokio::task::spawn_blocking;

enum ProfileImportTarget {
    SelectedActive,
    OtherSelection,
}

impl AppState {
    pub(super) fn update_import(&mut self, message: Message) -> Task<Message> {
        let copy_locale = self.shell.lang.clone();
        match message {
            Message::UpdateImportUrl(url) => {
                self.profile.import_url = url;
                Task::none()
            }
            Message::UpdateImportName(name) => {
                self.profile.import_name = name;
                Task::none()
            }
            Message::UpdateImportActivate(enabled) => {
                self.profile.import_activate = enabled;
                Task::none()
            }
            Message::ImportProfile => {
                let url = self.profile.import_url.trim().to_string();
                let name = self.profile.import_name.trim().to_string();
                let activate = self.profile.import_activate;
                let runtime = self.runtime.runtime.clone();
                let runtime = crate::update::core::profile_apply::runtime_access(runtime);
                if name.is_empty() || url.is_empty() {
                    return Task::done(Message::ShowToast(
                        "Name and subscription URL are required".to_string(),
                        ToastStatus::Error,
                    ));
                }

                self.profile.is_importing = true;
                Task::perform(
                    async move {
                        let profile_name = sanitize_profile_name(&name)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let cm = config_manager().await?;
                        let profiles = profiles_for_mihomo(cm.clone());
                        let active_reference = profiles
                            .active_profile_reference()
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))?;
                        let target = match &active_reference {
                            ActiveProfileReferenceState::Selected(reference)
                                if reference.engine_id == profiles.engine_id()
                                    && reference.profile_id.as_str() == profile_name =>
                            {
                                ProfileImportTarget::SelectedActive
                            }
                            ActiveProfileReferenceState::Selected(_)
                            | ActiveProfileReferenceState::Unselected { .. } => {
                                ProfileImportTarget::OtherSelection
                            }
                        };
                        match (&runtime, target) {
                            (
                                ProfileRuntimeAccess::CoreOwner(_),
                                ProfileImportTarget::SelectedActive,
                            ) => {
                                return Err(InfiltratorError::Config(
                                    Lang(&copy_locale)
                                        .tr("profile_import_requires_stopped_core")
                                        .into_owned(),
                                ));
                            }
                            (
                                ProfileRuntimeAccess::Offline,
                                ProfileImportTarget::SelectedActive,
                            ) => {
                                return Err(InfiltratorError::Config(
                                    "selected profile import must use the revision-bound content editor"
                                        .to_owned(),
                                ));
                            }
                            (
                                ProfileRuntimeAccess::Offline | ProfileRuntimeAccess::CoreOwner(_),
                                ProfileImportTarget::OtherSelection,
                            ) => {}
                        }
                        let source = subscription_source();
                        profiles
                            .import_subscription(&source, &profile_name, &url)
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))?;

                        let activation = match activate {
                            true => Some(activate_profile(runtime, &profile_name).await?),
                            false => None,
                        };
                        Ok(activation)
                    },
                    Message::ProfileImported,
                )
            }
            Message::ProfileImported(result) => {
                self.profile.is_importing = false;
                match result {
                    Ok(activation) => {
                        self.invalidate_rules_dns_views();
                        self.profile.import_url.clear();
                        self.profile.import_name.clear();
                        self.profile.import_activate = false;
                        let mut tasks = vec![
                            Task::done(Message::LoadProfiles),
                            Task::done(Message::ShowToast(
                                "Profile imported successfully".to_string(),
                                ToastStatus::Success,
                            )),
                        ];
                        match activation {
                            None => {}
                            Some(
                                ProfileActivationOutcome::AlreadySelectedWithoutRuntime
                                | ProfileActivationOutcome::SelectedWithoutRuntime,
                            ) => {}
                            Some(
                                ProfileActivationOutcome::AlreadySelectedWithRuntime
                                | ProfileActivationOutcome::SelectedAndApplied
                                | ProfileActivationOutcome::CurrentProfileUpdatedAndApplied,
                            ) => {
                                if let Some(runtime) = self.runtime.runtime.clone() {
                                    self.sync_runtime_slot(Some(runtime));
                                }
                            }
                            Some(
                                ProfileActivationOutcome::AlreadySelectedWhileInactive(
                                    infiltrator_contract::snapshot::CoreInactiveLifecycle::Stopped,
                                )
                                | ProfileActivationOutcome::SelectedWhileInactive(
                                    infiltrator_contract::snapshot::CoreInactiveLifecycle::Stopped,
                                )
                                | ProfileActivationOutcome::CurrentProfileUpdatedWhileInactive(
                                    infiltrator_contract::snapshot::CoreInactiveLifecycle::Stopped,
                                ),
                            ) => tasks.push(Task::done(Message::ShowToast(
                                Lang(&self.shell.lang)
                                    .tr("profile_selected_core_stopped")
                                    .into_owned(),
                                ToastStatus::Info,
                            ))),
                            Some(
                                ProfileActivationOutcome::AlreadySelectedWhileInactive(
                                    infiltrator_contract::snapshot::CoreInactiveLifecycle::Failed,
                                )
                                | ProfileActivationOutcome::SelectedWhileInactive(
                                    infiltrator_contract::snapshot::CoreInactiveLifecycle::Failed,
                                )
                                | ProfileActivationOutcome::CurrentProfileUpdatedWhileInactive(
                                    infiltrator_contract::snapshot::CoreInactiveLifecycle::Failed,
                                ),
                            ) => tasks.push(Task::done(Message::ShowToast(
                                Lang(&self.shell.lang)
                                    .tr("profile_selected_core_failed")
                                    .into_owned(),
                                ToastStatus::Warning,
                            ))),
                        }
                        Task::batch(tasks)
                    }
                    Err(e) => {
                        self.set_error(&e);
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::BrowseLocalImportFile => Task::perform(
                async {
                    spawn_blocking(|| {
                        rfd::FileDialog::new()
                            .add_filter("Configs & Subscriptions", &["yaml", "yml", "json", "txt"])
                            .pick_file()
                    })
                    .await
                    .ok()
                    .flatten()
                },
                Message::LocalImportFilePicked,
            ),
            Message::LocalImportFilePicked(path) => {
                if let Some(path) = path {
                    self.profile.local_import_path = path.to_string_lossy().to_string();
                    if self.profile.local_import_name.trim().is_empty()
                        && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                    {
                        self.profile.local_import_name = stem.to_string();
                    }
                }
                Task::none()
            }
            Message::UpdateLocalImportPath(path) => {
                self.profile.local_import_path = path;
                if self.profile.local_import_name.trim().is_empty()
                    && let Some(stem) = path::Path::new(&self.profile.local_import_path)
                        .file_stem()
                        .and_then(|s| s.to_str())
                {
                    self.profile.local_import_name = stem.to_string();
                }
                Task::none()
            }
            Message::UpdateLocalImportName(name) => {
                self.profile.local_import_name = name;
                Task::none()
            }
            Message::UpdateLocalImportActivate(enabled) => {
                self.profile.local_import_activate = enabled;
                Task::none()
            }
            Message::ImportLocalProfile => {
                let path = self.profile.local_import_path.trim().to_string();
                let name = self.profile.local_import_name.trim().to_string();
                let activate = self.profile.local_import_activate;
                let runtime = self.runtime.runtime.clone();
                if path.is_empty() || name.is_empty() {
                    return Task::done(Message::ShowToast(
                        "Local path and profile name are required".to_string(),
                        ToastStatus::Error,
                    ));
                }

                self.profile.is_importing_local = true;
                Task::perform(
                    async move {
                        let profile_name = sanitize_profile_name(&name)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        // DUAL-07-01: the local-file channel goes through the
                        // host import port so Iced no longer owns a second
                        // filesystem read path.
                        let content = subscription_import_port()
                            .read_local_file(&path)
                            .await
                            .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                        let content =
                            ProfileConverter::detect_and_convert(&content).unwrap_or(content);
                        validate_yaml(&content)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;

                        let cm = config_manager().await?;
                        let profiles = profiles_for_mihomo(cm.clone());
                        let expected = profiles
                            .active_profile_reference()
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))?;
                        let target = match &expected {
                            ActiveProfileReferenceState::Selected(reference)
                                if reference.engine_id == profiles.engine_id()
                                    && reference.profile_id.as_str() == profile_name =>
                            {
                                ProfileImportTarget::SelectedActive
                            }
                            ActiveProfileReferenceState::Selected(_)
                            | ActiveProfileReferenceState::Unselected { .. } => {
                                ProfileImportTarget::OtherSelection
                            }
                        };
                        let runtime = crate::update::core::profile_apply::runtime_access(runtime);
                        let activation = match target {
                            ProfileImportTarget::SelectedActive => {
                                let saved = profiles
                                    .save_profile_content_at_reference(
                                        runtime,
                                        profile_name,
                                        content,
                                        ApplyStrategy::AlwaysRestart,
                                        expected,
                                    )
                                    .await
                                    .map_err(|failure| InfiltratorError::Config(failure.message))?;
                                Some(match saved {
                                    infiltrator_application::profile_application::ProfileContentSaveOutcome::Applied => {
                                        ProfileActivationOutcome::CurrentProfileUpdatedAndApplied
                                    }
                                    infiltrator_application::profile_application::ProfileContentSaveOutcome::StoredWhileInactive(
                                        infiltrator_contract::snapshot::CoreInactiveLifecycle::Stopped,
                                    ) => {
                                        ProfileActivationOutcome::CurrentProfileUpdatedWhileInactive(
                                            infiltrator_contract::snapshot::CoreInactiveLifecycle::Stopped,
                                        )
                                    }
                                    infiltrator_application::profile_application::ProfileContentSaveOutcome::StoredWhileInactive(
                                        infiltrator_contract::snapshot::CoreInactiveLifecycle::Failed,
                                    ) => {
                                        ProfileActivationOutcome::CurrentProfileUpdatedWhileInactive(
                                            infiltrator_contract::snapshot::CoreInactiveLifecycle::Failed,
                                        )
                                    }
                                    infiltrator_application::profile_application::ProfileContentSaveOutcome::StoredOffline => {
                                        ProfileActivationOutcome::AlreadySelectedWithoutRuntime
                                    }
                                    infiltrator_application::profile_application::ProfileContentSaveOutcome::StoredInactive => {
                                        return Err(InfiltratorError::Config(
                                            "selected profile import was stored as inactive".to_owned(),
                                        ));
                                    }
                                })
                            }
                            ProfileImportTarget::OtherSelection => {
                                profiles
                                    .save_profile_content_at_reference(
                                        ProfileRuntimeAccess::<dyn ActiveProfileReferenceApplier>::Offline,
                                        profile_name.clone(),
                                        content,
                                        ApplyStrategy::AlwaysRestart,
                                        expected,
                                    )
                                    .await
                                    .map_err(|failure| {
                                        InfiltratorError::Config(failure.message)
                                    })?;
                                match activate {
                                    true => Some(activate_profile(runtime, &profile_name).await?),
                                    false => None,
                                }
                            }
                        };
                        Ok(activation)
                    },
                    Message::LocalProfileImported,
                )
            }
            Message::LocalProfileImported(result) => {
                self.profile.is_importing_local = false;
                match result {
                    Ok(activation) => {
                        self.invalidate_rules_dns_views();
                        self.profile.local_import_path.clear();
                        self.profile.local_import_name.clear();
                        self.profile.local_import_activate = false;
                        let mut tasks = vec![
                            Task::done(Message::LoadProfiles),
                            Task::done(Message::ShowToast(
                                "Local profile imported".to_string(),
                                ToastStatus::Success,
                            )),
                        ];
                        match activation {
                            None => {}
                            Some(
                                ProfileActivationOutcome::AlreadySelectedWithoutRuntime
                                | ProfileActivationOutcome::SelectedWithoutRuntime,
                            ) => {}
                            Some(
                                ProfileActivationOutcome::AlreadySelectedWithRuntime
                                | ProfileActivationOutcome::SelectedAndApplied
                                | ProfileActivationOutcome::CurrentProfileUpdatedAndApplied,
                            ) => {
                                if let Some(runtime) = self.runtime.runtime.clone() {
                                    self.sync_runtime_slot(Some(runtime));
                                }
                            }
                            Some(
                                ProfileActivationOutcome::AlreadySelectedWhileInactive(
                                    infiltrator_contract::snapshot::CoreInactiveLifecycle::Stopped,
                                )
                                | ProfileActivationOutcome::SelectedWhileInactive(
                                    infiltrator_contract::snapshot::CoreInactiveLifecycle::Stopped,
                                )
                                | ProfileActivationOutcome::CurrentProfileUpdatedWhileInactive(
                                    infiltrator_contract::snapshot::CoreInactiveLifecycle::Stopped,
                                ),
                            ) => tasks.push(Task::done(Message::ShowToast(
                                Lang(&self.shell.lang)
                                    .tr("profile_selected_core_stopped")
                                    .into_owned(),
                                ToastStatus::Info,
                            ))),
                            Some(
                                ProfileActivationOutcome::AlreadySelectedWhileInactive(
                                    infiltrator_contract::snapshot::CoreInactiveLifecycle::Failed,
                                )
                                | ProfileActivationOutcome::SelectedWhileInactive(
                                    infiltrator_contract::snapshot::CoreInactiveLifecycle::Failed,
                                )
                                | ProfileActivationOutcome::CurrentProfileUpdatedWhileInactive(
                                    infiltrator_contract::snapshot::CoreInactiveLifecycle::Failed,
                                ),
                            ) => tasks.push(Task::done(Message::ShowToast(
                                Lang(&self.shell.lang)
                                    .tr("profile_selected_core_failed")
                                    .into_owned(),
                                ToastStatus::Warning,
                            ))),
                        }
                        Task::batch(tasks)
                    }
                    Err(e) => {
                        self.set_error(&e);
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            _ => Task::none(),
        }
    }
}

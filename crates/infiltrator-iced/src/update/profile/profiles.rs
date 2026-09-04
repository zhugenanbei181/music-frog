//! Profile catalog handlers: listing, filtering, reset-to-default,
//! activation and deletion.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_ports::runtime_gateway::ManagedRuntime;

impl AppState {
    pub(super) fn update_profiles(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::LoadProfiles => {
                self.profile.is_loading_profiles = true;
                Task::perform(
                    async {
                        let store = crate::configs_dir::config_manager().await?;
                        ProfileApplication::new(store)
                            .list_profiles()
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))
                    },
                    Message::ProfilesLoaded,
                )
            }
            Message::ProfilesLoaded(result) => {
                self.profile.is_loading_profiles = false;
                self.refresh_tray();
                match result {
                    Ok(profiles) => {
                        self.profile.profiles = profiles;
                        self.sync_subscription_editor();
                        Task::none()
                    }
                    Err(e) => {
                        self.set_error(&e);
                        Task::none()
                    }
                }
            }
            Message::UpdateProfilesFilter(filter) => {
                self.profile.profiles_filter = filter;
                Task::none()
            }
            Message::ClearProfiles => {
                self.shell.error_msg = None;
                let runtime = self.take_app_runtime();
                self.profile.restart_after_profile_reset = runtime.is_some();
                self.runtime.status = crate::types::runtime::RuntimeStatus::Stopped;
                self.profile.is_loading_profiles = true;
                Task::perform(
                    async move {
                        if let Some(runtime) = runtime {
                            ManagedRuntime::shutdown(runtime.as_ref())
                                .await
                                .map_err(|error| InfiltratorError::Mihomo(error.to_string()))?;
                        }
                        infiltrator_desktop::storage::reset_profiles_to_default()
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        Ok(())
                    },
                    Message::ProfilesCleared,
                )
            }
            Message::ProfilesCleared(result) => {
                self.profile.is_loading_profiles = false;
                match result {
                    Ok(_) => {
                        let restart = self.profile.restart_after_profile_reset;
                        self.profile.restart_after_profile_reset = false;
                        self.invalidate_rules_dns_views();
                        self.profile.profiles_filter.clear();
                        let mut tasks = vec![
                            Task::done(Message::LoadProfiles),
                            Task::done(Message::ShowToast(
                                "Profiles reset to default".to_string(),
                                ToastStatus::Success,
                            )),
                        ];
                        if restart {
                            tasks.push(Task::done(Message::StartProxy));
                        }
                        Task::batch(tasks)
                    }
                    Err(e) => {
                        self.profile.restart_after_profile_reset = false;
                        self.set_error(&e);
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::SetActiveProfile(name) => {
                self.shell.error_msg = None;
                self.invalidate_rules_dns_views();
                let runtime = self.runtime.runtime.clone();
                Task::perform(
                    async move {
                        crate::update::core::profile_apply::activate_profile(runtime, &name).await
                    },
                    |result: Result<bool, InfiltratorError>| match result {
                        Ok(was_running) => Message::ProfileActivationFinished(Ok(was_running)),
                        Err(e) => Message::ProfileActivationFinished(Err(e)),
                    },
                )
            }
            Message::ProfileActivationFinished(result) => match result {
                Ok(true) => {
                    if let Some(runtime) = self.runtime.runtime.clone() {
                        self.sync_runtime_slot(Some(runtime));
                    }
                    self.refresh_tray();
                    Task::batch(vec![
                        Task::done(Message::LoadProfiles),
                        Task::done(Message::ShowToast(
                            "Profile activated".to_string(),
                            ToastStatus::Success,
                        )),
                    ])
                }
                Ok(false) => Task::batch(vec![
                    Task::done(Message::LoadProfiles),
                    Task::done(Message::StartProxy),
                ]),
                Err(error) => {
                    self.set_error(&error);
                    Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                }
            },
            Message::DeleteProfile(name) => Task::perform(
                async move {
                    let store = crate::configs_dir::config_manager().await?;
                    ProfileApplication::new(store)
                        .delete_profile(&name)
                        .await
                        .map_err(|failure| InfiltratorError::Config(failure.message))
                },
                Message::ProfileDeleted,
            ),
            Message::ProfileDeleted(result) => match result {
                Ok(_) => {
                    self.invalidate_rules_dns_views();
                    Task::batch(vec![
                        Task::done(Message::LoadProfiles),
                        Task::done(Message::ShowToast(
                            "Profile deleted".to_string(),
                            ToastStatus::Success,
                        )),
                    ])
                }
                Err(e) => {
                    self.set_error(&e);
                    Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                }
            },
            _ => Task::none(),
        }
    }
}

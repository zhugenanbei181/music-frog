//! Profile catalog handlers: listing, filtering, reset-to-default,
//! activation and deletion.

use crate::state::AppState;
use crate::types::{InfiltratorError, Message, ToastStatus};
use iced::Task;
use mihomo_config::manager::ConfigManager;

impl AppState {
    pub(super) fn update_profiles(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::LoadProfiles => {
                self.is_loading_profiles = true;
                Task::perform(
                    async {
                        let cm = ConfigManager::new()
                            .map_err(|e: mihomo_api::error::MihomoError| InfiltratorError::from(e))?;
                        cm.list_profiles()
                            .await
                            .map_err(|e: mihomo_api::error::MihomoError| InfiltratorError::from(e))
                    },
                    Message::ProfilesLoaded,
                )
            }
            Message::ProfilesLoaded(result) => {
                self.is_loading_profiles = false;
                match result {
                    Ok(profiles) => {
                        self.profiles = profiles;
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
                self.profiles_filter = filter;
                Task::none()
            }
            Message::ClearProfiles => {
                self.error_msg = None;
                self.is_loading_profiles = true;
                Task::perform(
                    async {
                        infiltrator_core::profiles::reset_profiles_to_default()
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        Ok(())
                    },
                    Message::ProfilesCleared,
                )
            }
            Message::ProfilesCleared(result) => {
                self.is_loading_profiles = false;
                match result {
                    Ok(_) => {
                        self.invalidate_rules_dns_views();
                        self.profiles_filter.clear();
                        Task::batch(vec![
                            Task::done(Message::LoadProfiles),
                            Task::done(Message::StartProxy),
                            Task::done(Message::ShowToast(
                                "Profiles reset to default".to_string(),
                                ToastStatus::Success,
                            )),
                        ])
                    }
                    Err(e) => {
                        self.set_error(&e);
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::SetActiveProfile(name) => {
                self.error_msg = None;
                self.invalidate_rules_dns_views();
                let runtime = self.runtime.clone();
                Task::perform(
                    async move {
                        let cm = ConfigManager::new()
                            .map_err(|e: mihomo_api::error::MihomoError| InfiltratorError::from(e))?;
                        let previous = cm
                            .get_current()
                            .await
                            .map_err(|e: mihomo_api::error::MihomoError| InfiltratorError::from(e))?;
                        cm.set_current(&name)
                            .await
                            .map_err(|e: mihomo_api::error::MihomoError| InfiltratorError::from(e))?;
                        if let Some(rt) = runtime {
                            // Core is live: run the CORE-004 transaction so
                            // the switch actually reaches mihomo, with a
                            // readiness check and rollback on failure.
                            if let Err(e) = rt
                                .apply_current_config(infiltrator_core::apply::ApplyStrategy::AlwaysRestart)
                                .await
                            {
                                let _ = cm.set_current(&previous).await;
                                return Err(InfiltratorError::Mihomo(e.to_string()));
                            }
                        }
                        // No live core: leaving current set is enough — the
                        // follow-up StartProxy boots with the new profile.
                        Ok(())
                    },
                    |result: Result<(), InfiltratorError>| match result {
                        Ok(()) => Message::StartProxy,
                        Err(e) => Message::ShowToast(e.to_string(), ToastStatus::Error),
                    },
                )
            }
            Message::DeleteProfile(name) => Task::perform(
                async move {
                    let cm = ConfigManager::new().map_err(InfiltratorError::from)?;
                    cm.delete_profile(&name)
                        .await
                        .map_err(InfiltratorError::from)?;
                    Ok(())
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

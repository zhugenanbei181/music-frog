//! Subscription handlers: per-profile subscription settings, manual and
//! scheduled (tick) updates, plus the subscription-editor sync helper.

use crate::state::AppState;
use crate::types::{InfiltratorError, Message, ToastStatus};
use chrono::Utc;
use iced::Task;
use mihomo_config::manager::ConfigManager;

impl AppState {
    /// Keep the subscription editor fields in sync with the selected profile
    /// (falls back to the active or first profile when the selection is
    /// empty or stale).
    pub(super) fn sync_subscription_editor(&mut self) {
        if self.profiles.is_empty() {
            self.subscription_profile_name.clear();
            self.subscription_url.clear();
            self.subscription_auto_update_enabled = false;
            self.subscription_update_interval_hours.clear();
            return;
        }

        let selected_name = if !self.subscription_profile_name.is_empty()
            && self
                .profiles
                .iter()
                .any(|p| p.name == self.subscription_profile_name)
        {
            self.subscription_profile_name.clone()
        } else if let Some(active) = self.profiles.iter().find(|p| p.active) {
            active.name.clone()
        } else {
            self.profiles[0].name.clone()
        };

        self.subscription_profile_name = selected_name.clone();
        if let Some(profile) = self.profiles.iter().find(|p| p.name == selected_name) {
            self.subscription_url = profile.subscription_url.clone().unwrap_or_default();
            self.subscription_auto_update_enabled = profile.auto_update_enabled;
            self.subscription_update_interval_hours = profile
                .update_interval_hours
                .map(|hours| hours.to_string())
                .unwrap_or_else(|| "24".to_string());
        }
    }

    pub(super) fn update_subscription(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectSubscriptionProfile(name) => {
                self.subscription_profile_name = name;
                self.sync_subscription_editor();
                Task::none()
            }
            Message::UpdateSubscriptionUrl(url) => {
                self.subscription_url = url;
                Task::none()
            }
            Message::UpdateSubscriptionAutoUpdate(enabled) => {
                self.subscription_auto_update_enabled = enabled;
                Task::none()
            }
            Message::UpdateSubscriptionInterval(interval) => {
                self.subscription_update_interval_hours = interval;
                Task::none()
            }
            Message::SaveSubscriptionSettings => {
                let profile_name = self.subscription_profile_name.clone();
                let url = self.subscription_url.trim().to_string();
                let auto_update = self.subscription_auto_update_enabled;
                let interval_raw = self.subscription_update_interval_hours.trim().to_string();

                if profile_name.is_empty() {
                    return Task::done(Message::ShowToast(
                        "Please select a profile".to_string(),
                        ToastStatus::Error,
                    ));
                }
                if auto_update && url.is_empty() {
                    return Task::done(Message::ShowToast(
                        "Subscription URL is required when auto update is enabled".to_string(),
                        ToastStatus::Error,
                    ));
                }
                let interval_hours = if auto_update {
                    let normalized = if interval_raw.is_empty() {
                        "24".to_string()
                    } else {
                        interval_raw
                    };
                    match normalized.parse::<u32>() {
                        Ok(v) if v > 0 => Some(v),
                        _ => {
                            return Task::done(Message::ShowToast(
                                "Update interval must be a positive number".to_string(),
                                ToastStatus::Error,
                            ));
                        }
                    }
                } else {
                    None
                };

                self.is_saving_subscription = true;
                Task::perform(
                    async move {
                        let cm = ConfigManager::new().map_err(InfiltratorError::from)?;
                        let mut metadata = cm
                            .get_profile_metadata(&profile_name)
                            .await
                            .map_err(InfiltratorError::from)?;

                        if url.is_empty() {
                            metadata.subscription_url = None;
                            metadata.auto_update_enabled = false;
                            metadata.update_interval_hours = None;
                            metadata.last_updated = None;
                            metadata.next_update = None;
                        } else {
                            metadata.subscription_url = Some(url);
                            metadata.auto_update_enabled = auto_update;
                            metadata.update_interval_hours = interval_hours;
                            metadata.next_update = None;
                        }

                        cm.update_profile_metadata(&profile_name, &metadata)
                            .await
                            .map_err(InfiltratorError::from)?;
                        Ok(())
                    },
                    Message::SubscriptionSettingsSaved,
                )
            }
            Message::SubscriptionSettingsSaved(result) => {
                self.is_saving_subscription = false;
                match result {
                    Ok(_) => Task::batch(vec![
                        Task::done(Message::LoadProfiles),
                        Task::done(Message::ShowToast(
                            "Subscription settings saved".to_string(),
                            ToastStatus::Success,
                        )),
                    ]),
                    Err(e) => {
                        self.set_error(&e);
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::UpdateSubscriptionNow => {
                let profile_name = self.subscription_profile_name.clone();
                if profile_name.is_empty() {
                    return Task::done(Message::ShowToast(
                        "Please select a profile".to_string(),
                        ToastStatus::Error,
                    ));
                }
                self.is_updating_subscription_now = true;
                Task::perform(
                    async move {
                        infiltrator_core::profiles::update_profile(&profile_name)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        Ok(())
                    },
                    Message::SubscriptionUpdatedNow,
                )
            }
            Message::SubscriptionUpdatedNow(result) => {
                self.is_updating_subscription_now = false;
                match result {
                    Ok(_) => Task::batch(vec![
                        Task::done(Message::LoadProfiles),
                        Task::done(Message::ShowToast(
                            "Subscription updated".to_string(),
                            ToastStatus::Success,
                        )),
                    ]),
                    Err(e) => {
                        self.set_error(&e);
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::TickSubUpdate => Task::perform(
                async move {
                    let manager = ConfigManager::new().map_err(InfiltratorError::from)?;
                    let profiles = manager
                        .list_profiles()
                        .await
                        .map_err(InfiltratorError::from)?;
                    let now = Utc::now();
                    let mut updated_names = Vec::new();
                    let mut active_updated = false;

                    for profile in profiles {
                        if !profile.auto_update_enabled {
                            continue;
                        }
                        let Some(url) = profile.subscription_url.as_deref() else {
                            continue;
                        };
                        if url.trim().is_empty() {
                            continue;
                        }

                        let due = profile.next_update.is_none_or(|next| next <= now);
                        if !due {
                            continue;
                        }

                        infiltrator_core::profiles::update_profile(&profile.name)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        if profile.active {
                            active_updated = true;
                        }
                        updated_names.push(profile.name);
                    }

                    Ok((updated_names, active_updated))
                },
                Message::SubscriptionAutoUpdated,
            ),
            Message::SubscriptionAutoUpdated(result) => match result {
                Ok((updated_profiles, active_updated)) => {
                    if updated_profiles.is_empty() {
                        return Task::none();
                    }
                    let mut tasks = vec![
                        Task::done(Message::LoadProfiles),
                        Task::done(Message::ShowToast(
                            format!("Auto-updated: {}", updated_profiles.join(", ")),
                            ToastStatus::Success,
                        )),
                    ];
                    if active_updated {
                        tasks.push(Task::done(Message::StartProxy));
                    }
                    Task::batch(tasks)
                }
                Err(e) => {
                    self.set_error(&e);
                    Task::done(Message::ShowToast(
                        format!("Auto update failed: {}", e),
                        ToastStatus::Warning,
                    ))
                }
            },
            _ => Task::none(),
        }
    }
}

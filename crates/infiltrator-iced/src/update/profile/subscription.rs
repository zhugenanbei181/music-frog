//! Subscription handlers: per-profile subscription settings, manual and
//! scheduled (tick) updates, plus the subscription-editor sync helper.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use chrono::Utc;
use iced::Task;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_ports::runtime_gateway::ManagedRuntime;
use infiltrator_shared::locales::Localizer;

impl AppState {
    /// Keep the subscription editor fields in sync with the selected profile
    /// (falls back to the active or first profile when the selection is
    /// empty or stale).
    pub(super) fn sync_subscription_editor(&mut self) {
        if self.profile.profiles.is_empty() {
            self.profile.subscription_profile_name.clear();
            self.profile.subscription_url.clear();
            self.profile.subscription_auto_update_enabled = false;
            self.profile.subscription_update_interval_hours.clear();
            return;
        }

        let selected_name = if !self.profile.subscription_profile_name.is_empty()
            && self
                .profile
                .profiles
                .iter()
                .any(|p| p.name == self.profile.subscription_profile_name)
        {
            self.profile.subscription_profile_name.clone()
        } else if let Some(active) = self.profile.profiles.iter().find(|p| p.active) {
            active.name.clone()
        } else {
            self.profile.profiles[0].name.clone()
        };

        self.profile.subscription_profile_name = selected_name.clone();
        if let Some(profile) = self
            .profile
            .profiles
            .iter()
            .find(|p| p.name == selected_name)
        {
            self.profile.subscription_url = profile.subscription_url.clone().unwrap_or_default();
            self.profile.subscription_auto_update_enabled = profile.auto_update_enabled;
            self.profile.subscription_update_interval_hours = profile
                .update_interval_hours
                .map(|hours| hours.to_string())
                .unwrap_or_else(|| "24".to_string());
        }
    }

    pub(super) fn update_subscription(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectSubscriptionProfile(name) => {
                self.profile.subscription_profile_name = name;
                self.sync_subscription_editor();
                // Prefill the per-profile filter editor for the new selection.
                Task::done(Message::LoadProfileFilter)
            }
            Message::UpdateSubscriptionUrl(url) => {
                self.profile.subscription_url = url;
                Task::none()
            }
            Message::UpdateSubscriptionAutoUpdate(enabled) => {
                self.profile.subscription_auto_update_enabled = enabled;
                Task::none()
            }
            Message::UpdateSubscriptionInterval(interval) => {
                self.profile.subscription_update_interval_hours = interval;
                Task::none()
            }
            Message::SaveSubscriptionSettings => {
                let profile_name = self.profile.subscription_profile_name.clone();
                let url = self.profile.subscription_url.trim().to_string();
                let auto_update = self.profile.subscription_auto_update_enabled;
                let interval_raw = self
                    .profile
                    .subscription_update_interval_hours
                    .trim()
                    .to_string();

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

                self.profile.is_saving_subscription = true;
                Task::perform(
                    async move {
                        let cm = crate::configs_dir::config_manager().await?;
                        let application = ProfileApplication::new(cm);
                        let mut metadata = application
                            .load_metadata(&profile_name)
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))?;

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

                        application
                            .update_metadata(&profile_name, &metadata)
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))?;
                        Ok(())
                    },
                    Message::SubscriptionSettingsSaved,
                )
            }
            Message::SubscriptionSettingsSaved(result) => {
                self.profile.is_saving_subscription = false;
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
                let profile_name = self.profile.subscription_profile_name.clone();
                if profile_name.is_empty() {
                    return Task::done(Message::ShowToast(
                        "Please select a profile".to_string(),
                        ToastStatus::Error,
                    ));
                }
                self.profile.is_updating_subscription_now = true;
                let runtime = self.runtime.runtime.clone();
                Task::perform(
                    async move {
                        let cm = crate::configs_dir::config_manager().await?;
                        let application = ProfileApplication::new(cm);
                        let source = infiltrator_desktop::storage::subscription_source();
                        application
                            .update_subscription(&source, &profile_name)
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))?;
                        let current = application
                            .current_profile()
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))?;
                        if let Some(runtime) = runtime
                            && current == profile_name
                        {
                            ManagedRuntime::apply_current_config(
                                runtime.as_ref(),
                                infiltrator_domain::apply::ApplyStrategy::AlwaysRestart,
                            )
                            .await
                            .map_err(|error| InfiltratorError::Mihomo(error.to_string()))?;
                            Ok(true)
                        } else {
                            application
                                .clear_backup(&profile_name)
                                .await
                                .map_err(|failure| InfiltratorError::Config(failure.message))?;
                            Ok(false)
                        }
                    },
                    Message::SubscriptionUpdatedNow,
                )
            }
            Message::SubscriptionUpdatedNow(result) => {
                self.profile.is_updating_subscription_now = false;
                self.refresh_tray();
                match result {
                    Ok(reloaded) => {
                        if reloaded && let Some(runtime) = self.runtime.runtime.clone() {
                            self.sync_runtime_slot(Some(runtime));
                        }
                        Task::batch(vec![
                            Task::done(Message::LoadProfiles),
                            Task::done(Message::ShowToast(
                                "Subscription updated".to_string(),
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
            Message::TickSubUpdate => {
                let runtime = self.runtime.runtime.clone();
                Task::perform(
                    async move {
                        let manager = crate::configs_dir::config_manager().await?;
                        let application = ProfileApplication::new(manager);
                        let profiles = application
                            .list_profiles()
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))?;
                        let source = infiltrator_desktop::storage::subscription_source();
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

                            application
                                .update_subscription(&source, &profile.name)
                                .await
                                .map_err(|failure| InfiltratorError::Config(failure.message))?;
                            if profile.active {
                                active_updated = true;
                                if let Some(runtime) = runtime.as_ref() {
                                    ManagedRuntime::apply_current_config(
                                        runtime.as_ref(),
                                        infiltrator_domain::apply::ApplyStrategy::AlwaysRestart,
                                    )
                                    .await
                                    .map_err(|error| InfiltratorError::Mihomo(error.to_string()))?;
                                } else {
                                    application.clear_backup(&profile.name).await.map_err(
                                        |failure| InfiltratorError::Config(failure.message),
                                    )?;
                                }
                            } else {
                                application
                                    .clear_backup(&profile.name)
                                    .await
                                    .map_err(|failure| InfiltratorError::Config(failure.message))?;
                            }
                            updated_names.push(profile.name);
                        }

                        Ok((updated_names, active_updated))
                    },
                    Message::SubscriptionAutoUpdated,
                )
            }
            Message::SubscriptionAutoUpdated(result) => match result {
                Ok((updated_profiles, active_updated)) => {
                    if updated_profiles.is_empty() {
                        return Task::none();
                    }
                    self.refresh_tray();
                    // 0.20: 自动更新可能发生在窗口不可见时，同时发系统通知
                    // （正文=更新的 profile 名列表，urgency Low）。
                    let tasks = vec![
                        Task::done(Message::LoadProfiles),
                        Task::done(Message::ShowToast(
                            format!("Auto-updated: {}", updated_profiles.join(", ")),
                            ToastStatus::Success,
                        )),
                        self.system_notify(
                            "notify_sub_auto_updated",
                            &updated_profiles.join(", "),
                            crate::notify::NotifyUrgency::Low,
                        ),
                    ];
                    if active_updated && let Some(runtime) = self.runtime.runtime.clone() {
                        self.sync_runtime_slot(Some(runtime));
                    }
                    Task::batch(tasks)
                }
                Err(e) => {
                    self.set_error(&e);
                    Task::batch(vec![
                        Task::done(Message::ShowToast(
                            format!("Auto update failed: {}", e),
                            ToastStatus::Warning,
                        )),
                        // 0.20: 失败走 Critical 系统通知（正文=错误串，先脱敏）。
                        self.system_notify(
                            "notify_sub_update_failed",
                            &e.to_string(),
                            crate::notify::NotifyUrgency::Critical,
                        ),
                    ])
                }
            },
            Message::UpdateAllSubscriptionsNow => {
                // Tray "update all" entry: refresh every profile carrying a
                // subscription URL right away, regardless of its schedule.
                if self.profile.is_updating_subscription_now {
                    return Task::none();
                }
                self.profile.is_updating_subscription_now = true;
                let runtime = self.runtime.runtime.clone();
                Task::perform(
                    async move {
                        let manager = crate::configs_dir::config_manager().await?;
                        let application = ProfileApplication::new(manager);
                        let profiles = application
                            .list_profiles()
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))?;
                        let source = infiltrator_desktop::storage::subscription_source();
                        let mut outcomes = Vec::new();

                        for profile in profiles {
                            let Some(url) = profile.subscription_url.as_deref() else {
                                continue;
                            };
                            if url.trim().is_empty() {
                                continue;
                            }
                            let outcome = async {
                                application
                                    .update_subscription(&source, &profile.name)
                                    .await
                                    .map_err(|failure| InfiltratorError::Config(failure.message))?;
                                if profile.active {
                                    if let Some(runtime) = runtime.as_ref() {
                                        ManagedRuntime::apply_current_config(
                                            runtime.as_ref(),
                                            infiltrator_domain::apply::ApplyStrategy::AlwaysRestart,
                                        )
                                        .await
                                        .map_err(
                                            |error| InfiltratorError::Mihomo(error.to_string()),
                                        )?;
                                    } else {
                                        application.clear_backup(&profile.name).await.map_err(
                                            |failure| InfiltratorError::Config(failure.message),
                                        )?;
                                    }
                                } else {
                                    application.clear_backup(&profile.name).await.map_err(
                                        |failure| InfiltratorError::Config(failure.message),
                                    )?;
                                }
                                Ok(())
                            }
                            .await;
                            outcomes.push((
                                profile.name,
                                outcome.map_err(|e: InfiltratorError| e.to_string()),
                            ));
                        }

                        Ok(outcomes)
                    },
                    Message::AllSubscriptionsUpdated,
                )
            }
            Message::AllSubscriptionsUpdated(result) => {
                self.profile.is_updating_subscription_now = false;
                let toast_none = infiltrator_shared::locales::Lang(&self.shell.lang)
                    .tr("update_all_none")
                    .into_owned();
                let toast_done = infiltrator_shared::locales::Lang(&self.shell.lang)
                    .tr("sub_update_done")
                    .into_owned();
                match result {
                    Ok(outcomes) => {
                        if outcomes.iter().any(|(name, r)| {
                            r.is_ok()
                                && self
                                    .profile
                                    .profiles
                                    .iter()
                                    .any(|p| p.active && p.name == *name)
                        }) && let Some(runtime) = self.runtime.runtime.clone()
                        {
                            self.sync_runtime_slot(Some(runtime));
                        }
                        self.refresh_tray();
                        let failed = outcomes.iter().filter(|(_, r)| r.is_err()).count();
                        let (toast, status) = if outcomes.is_empty() {
                            (toast_none, ToastStatus::Info)
                        } else if failed == 0 {
                            (toast_done, ToastStatus::Success)
                        } else {
                            (
                                format!(
                                    "{} (✓{} ✗{})",
                                    toast_done,
                                    outcomes.len() - failed,
                                    failed
                                ),
                                ToastStatus::Warning,
                            )
                        };
                        Task::batch(vec![
                            Task::done(Message::LoadProfiles),
                            Task::done(Message::ShowToast(toast, status)),
                        ])
                    }
                    Err(e) => {
                        self.set_error(&e);
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::SetProfileAutoUpdate { name, enabled } => {
                // Tray checkmark entry: flip one profile's auto-update flag
                // directly, without routing through the editor form state.
                if enabled {
                    let has_url = self
                        .profile
                        .profiles
                        .iter()
                        .find(|p| p.name == name)
                        .and_then(|p| p.subscription_url.as_deref())
                        .is_some_and(|url| !url.trim().is_empty());
                    if !has_url {
                        let lang = infiltrator_shared::locales::Lang(&self.shell.lang);
                        return Task::done(Message::ShowToast(
                            lang.tr("tray_auto_update_no_url").into_owned(),
                            ToastStatus::Error,
                        ));
                    }
                }
                Task::perform(
                    async move {
                        let cm = crate::configs_dir::config_manager().await?;
                        let application = ProfileApplication::new(cm);
                        let mut metadata = application
                            .load_metadata(&name)
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))?;
                        metadata.auto_update_enabled = enabled;
                        application
                            .update_metadata(&name, &metadata)
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))?;
                        Ok(name)
                    },
                    Message::ProfileAutoUpdateSet,
                )
            }
            Message::ProfileAutoUpdateSet(result) => {
                self.refresh_tray();
                match result {
                    Ok(_) => {
                        let lang = infiltrator_shared::locales::Lang(&self.shell.lang);
                        Task::batch(vec![
                            Task::done(Message::LoadProfiles),
                            Task::done(Message::ShowToast(
                                lang.tr("tray_auto_update_toggled").into_owned(),
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
            _ => Task::none(),
        }
    }
}

//! Subscription handlers: per-profile subscription settings, manual and
//! scheduled (tick) updates, plus the subscription-editor sync helper.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use chrono::Utc;
use iced::Task;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::subscription_refresh_application::SubscriptionRefreshApplication;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::subscription_import::{
    SubscriptionBatchReport, SubscriptionUpdateOutcome, SubscriptionUpdateReport,
};
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
            self.profile.subscription_user_agent.clear();
            self.profile.subscription_insecure_skip_verify = false;
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
            self.profile.subscription_user_agent = profile.user_agent.clone().unwrap_or_default();
            self.profile.subscription_insecure_skip_verify = profile.insecure_skip_verify;
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
            Message::UpdateSubscriptionUserAgent(user_agent) => {
                self.profile.subscription_user_agent = user_agent;
                Task::none()
            }
            Message::UpdateSubscriptionInsecureSkipVerify(enabled) => {
                self.profile.subscription_insecure_skip_verify = enabled;
                Task::none()
            }
            Message::SaveSubscriptionSettings => {
                let profile_name = self.profile.subscription_profile_name.clone();
                let url = self.profile.subscription_url.trim().to_string();
                let auto_update = self.profile.subscription_auto_update_enabled;
                let user_agent = self.profile.subscription_user_agent.trim().to_string();
                let insecure_skip_verify = self.profile.subscription_insecure_skip_verify;
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
                        metadata.user_agent = if user_agent.is_empty() {
                            None
                        } else {
                            Some(user_agent)
                        };
                        metadata.insecure_skip_verify = insecure_skip_verify;

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
                        let source = crate::host::storage::subscription_source();
                        // DUAL-07-05/06: retry with exponential backoff through
                        // the injected runtime, guarded by the shared
                        // single-flight slot so a scheduled tick cannot race
                        // this manual refresh.
                        let refresh = SubscriptionRefreshApplication::with_default_policy(
                            application.clone(),
                            crate::host::runtime::application_runtime(),
                        );
                        let mut report = refresh
                            .refresh_profile(&source, &profile_name)
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))?;
                        let current = application
                            .current_profile()
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))?;
                        let reloaded = if let Some(runtime) = runtime
                            && current == profile_name
                        {
                            ManagedRuntime::apply_current_config(
                                runtime.as_ref(),
                                infiltrator_domain::apply::ApplyStrategy::AlwaysRestart,
                            )
                            .await
                            .map_err(|error| InfiltratorError::Mihomo(error.to_string()))?;
                            true
                        } else {
                            application
                                .clear_backup(&profile_name)
                                .await
                                .map_err(|failure| InfiltratorError::Config(failure.message))?;
                            false
                        };
                        report.reloaded_core = reloaded;
                        Ok(report)
                    },
                    Message::SubscriptionUpdatedNow,
                )
            }
            Message::SubscriptionUpdatedNow(result) => {
                self.profile.is_updating_subscription_now = false;
                self.refresh_tray();
                match result {
                    Ok(report) => {
                        if report.reloaded_core
                            && let Some(runtime) = self.runtime.runtime.clone()
                        {
                            self.sync_runtime_slot(Some(runtime));
                        }
                        let lang = infiltrator_shared::locales::Lang(&self.shell.lang);
                        let (text, status) = subscription_update_toast(&lang, &report);
                        Task::batch(vec![
                            Task::done(Message::LoadProfiles),
                            Task::done(Message::ShowToast(text, status)),
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
                        let source = crate::host::storage::subscription_source();
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
                // Tray + toolbar "update all" entry: refresh every profile
                // carrying a subscription URL right away, regardless of its
                // schedule, through the shared bounded-concurrency batch path.
                if self.profile.is_updating_subscription_now {
                    return Task::none();
                }
                self.profile.is_updating_subscription_now = true;
                let runtime = self.runtime.runtime.clone();
                Task::perform(
                    async move {
                        let manager = crate::configs_dir::config_manager().await?;
                        let application = ProfileApplication::new(manager);
                        let source = crate::host::storage::subscription_source();
                        // DUAL-07-05/06: the batch shares the retry/backoff and
                        // single-flight orchestration with the single-profile
                        // refresh instead of re-implementing it.
                        let refresh = SubscriptionRefreshApplication::with_default_policy(
                            application.clone(),
                            crate::host::runtime::application_runtime(),
                        );
                        let report = refresh
                            .refresh_all(&source, BATCH_UPDATE_CONCURRENCY)
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))?;

                        let current = application
                            .current_profile()
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))?;
                        let active_updated = report.outcomes.iter().any(|outcome| {
                            outcome.profile_name == current
                                && matches!(
                                    outcome.outcome,
                                    SubscriptionUpdateOutcome::Updated { .. }
                                )
                        });
                        if active_updated && let Some(runtime) = runtime.as_ref() {
                            ManagedRuntime::apply_current_config(
                                runtime.as_ref(),
                                infiltrator_domain::apply::ApplyStrategy::AlwaysRestart,
                            )
                            .await
                            .map_err(|error| InfiltratorError::Mihomo(error.to_string()))?;
                        }
                        Ok(report)
                    },
                    Message::AllSubscriptionsUpdated,
                )
            }
            Message::AllSubscriptionsUpdated(result) => {
                self.profile.is_updating_subscription_now = false;
                match result {
                    Ok(report) => {
                        let active_updated = self
                            .profile
                            .profiles
                            .iter()
                            .find(|p| p.active)
                            .is_some_and(|active| {
                                report.outcomes.iter().any(|outcome| {
                                    outcome.profile_name == active.name
                                        && matches!(
                                            outcome.outcome,
                                            SubscriptionUpdateOutcome::Updated { .. }
                                        )
                                })
                            });
                        if active_updated && let Some(runtime) = self.runtime.runtime.clone() {
                            self.sync_runtime_slot(Some(runtime));
                        }
                        self.refresh_tray();
                        let lang = infiltrator_shared::locales::Lang(&self.shell.lang);
                        let (toast, status) = batch_update_toast(&lang, &report);
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
            Message::RestoreSubscriptionBackup => {
                // DUAL-07-13: restore the selected profile's transient
                // pre-save `.bak` copy through the shared application.
                let profile_name = self.profile.subscription_profile_name.clone();
                if profile_name.is_empty() {
                    return Task::done(Message::ShowToast(
                        "Please select a profile".to_string(),
                        ToastStatus::Error,
                    ));
                }
                Task::perform(
                    async move {
                        let cm = crate::configs_dir::config_manager().await?;
                        let application = ProfileApplication::new(cm);
                        application
                            .restore_backup(&profile_name)
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))
                    },
                    Message::SubscriptionBackupRestored,
                )
            }
            Message::SubscriptionBackupRestored(result) => {
                let lang = infiltrator_shared::locales::Lang(&self.shell.lang);
                match result {
                    Ok(restored) => {
                        let (key, status) = if restored {
                            ("profiles_backup_restored", ToastStatus::Success)
                        } else {
                            ("profiles_backup_missing", ToastStatus::Info)
                        };
                        Task::batch(vec![
                            Task::done(Message::LoadProfiles),
                            Task::done(Message::ShowToast(lang.tr(key).into_owned(), status)),
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

/// Bounded concurrency for the shared "update all subscriptions" batch.
const BATCH_UPDATE_CONCURRENCY: usize = 5;

/// Summarize a shared batch report into user-facing toast text. An empty or
/// all-skipped batch is informational; any failure downgrades to warning, and
/// a batch where nothing changed is reported as not-modified rather than a
/// fabricated success.
pub(crate) fn batch_update_toast(
    lang: &infiltrator_shared::locales::Lang<'_>,
    report: &SubscriptionBatchReport,
) -> (String, ToastStatus) {
    use infiltrator_shared::locales::Localizer;
    let attempted = report.updated + report.not_modified + report.failed;
    if attempted == 0 {
        return (lang.tr("update_all_none").into_owned(), ToastStatus::Info);
    }
    if report.failed > 0 {
        return (
            format!(
                "{} (✓{} ⟳{} ✗{})",
                lang.tr("sub_update_done"),
                report.updated,
                report.not_modified,
                report.failed
            ),
            ToastStatus::Warning,
        );
    }
    if report.updated == 0 {
        return (
            lang.tr("sub_update_not_modified").into_owned(),
            ToastStatus::Info,
        );
    }
    (
        lang.tr("sub_update_done").into_owned(),
        ToastStatus::Success,
    )
}

/// Map a shared subscription update report to the user-facing toast text and
/// status. A `304 Not Modified` is reported as informational, never as a
/// fabricated success; quota/expiry warnings downgrade a success to warning.
pub(crate) fn subscription_update_toast(
    lang: &infiltrator_shared::locales::Lang<'_>,
    report: &SubscriptionUpdateReport,
) -> (String, ToastStatus) {
    use infiltrator_shared::locales::Localizer;
    match &report.outcome {
        SubscriptionUpdateOutcome::NotModified { .. } => (
            lang.tr("sub_update_not_modified").into_owned(),
            ToastStatus::Info,
        ),
        _ if report.usage_warning || report.expiry_warning => (
            format!(
                "{} · {}",
                lang.tr("sub_update_done"),
                lang.tr("sub_update_quota_warning")
            ),
            ToastStatus::Warning,
        ),
        _ => (
            lang.tr("sub_update_done").into_owned(),
            ToastStatus::Success,
        ),
    }
}

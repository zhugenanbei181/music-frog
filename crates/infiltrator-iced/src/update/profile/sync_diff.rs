//! Sync-conflict key-level diff merge. The diff itself comes from
//! `sync_engine::conflict_resolution::diff_yaml_configs`; the user then picks
//! local/remote per top-level key and the merged document commits through the
//! shared apply transaction.
//!
//! Note the merge recomputes nothing behind the user's back: values are read
//! again at apply time and the validated apply transaction (with rollback)
//! protects the live core, so a file changed after the diff was computed can
//! only fail loudly, never silently corrupt.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use crate::types::options::{SyncDiffBundle, SyncDiffState};
use iced::Task;
use infiltrator_core::apply::ApplyStrategy;
use infiltrator_core::error::InfiltratorError;
use infiltrator_shared::locales::{Lang, Localizer};
use std::collections::HashSet;

impl AppState {
    pub(super) fn update_sync_diff(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::LoadSyncDiff(profile) => {
                if self.profile.is_loading_sync_diff {
                    return Task::none();
                }
                let Some(conflict) = self
                    .profile
                    .sync_conflicts
                    .iter()
                    .find(|conflict| conflict.profile == profile)
                    .cloned()
                else {
                    return Task::none();
                };
                self.profile.is_loading_sync_diff = true;
                Task::perform(
                    async move {
                        let manager = crate::configs_dir::config_manager().await?;
                        let local = manager
                            .load(&conflict.profile)
                            .await
                            .map_err(InfiltratorError::from)?;
                        let remote = tokio::fs::read_to_string(&conflict.remote_path)
                            .await
                            .map_err(|error| InfiltratorError::Io(error.to_string()))?;
                        let summary =
                            sync_engine::conflict_resolution::diff_yaml_configs(&local, &remote)
                                .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                        Ok(SyncDiffBundle {
                            profile: conflict.profile,
                            remote_path: conflict.remote_path,
                            added: summary.added_keys,
                            removed: summary.removed_keys,
                            modified: summary.modified_keys,
                        })
                    },
                    Message::SyncDiffLoaded,
                )
            }
            Message::SyncDiffLoaded(result) => {
                self.profile.is_loading_sync_diff = false;
                match result {
                    Ok(bundle) => {
                        self.profile.sync_diff = Some(SyncDiffState::new(bundle));
                    }
                    Err(error) => {
                        self.set_error(&error);
                        return Task::done(Message::ShowToast(
                            error.to_string(),
                            ToastStatus::Error,
                        ));
                    }
                }
                Task::none()
            }
            Message::PickSyncDiffKey(key, take_remote) => {
                if let Some(diff) = &mut self.profile.sync_diff {
                    diff.picks.insert(key, take_remote);
                }
                Task::none()
            }
            Message::SetSyncDiffPicks(take_remote) => {
                if let Some(diff) = &mut self.profile.sync_diff {
                    let keys = diff.bundle.all_keys();
                    for key in keys {
                        diff.picks.insert(key, take_remote);
                    }
                }
                Task::none()
            }
            Message::CloseSyncDiff => {
                self.profile.sync_diff = None;
                Task::none()
            }
            Message::ApplySyncDiffMerge => {
                if self.profile.is_applying_sync_diff {
                    return Task::none();
                }
                let Some(diff) = self.profile.sync_diff.clone() else {
                    return Task::none();
                };
                let Some(conflict) = self
                    .profile
                    .sync_conflicts
                    .iter()
                    .find(|conflict| conflict.profile == diff.bundle.profile)
                    .cloned()
                else {
                    self.profile.sync_diff = None;
                    return Task::none();
                };
                self.profile.is_applying_sync_diff = true;
                let runtime = self.runtime.runtime.clone();
                Task::perform(
                    async move {
                        let removed_keys: HashSet<String> =
                            diff.bundle.removed.iter().cloned().collect();
                        let mut take_remote = Vec::new();
                        let mut accept_removals = Vec::new();
                        for (key, pick) in &diff.picks {
                            if !pick {
                                continue;
                            }
                            if removed_keys.contains(key) {
                                accept_removals.push(key.clone());
                            } else {
                                take_remote.push(key.clone());
                            }
                        }
                        let manager = crate::configs_dir::config_manager().await?;
                        let local = manager
                            .load(&conflict.profile)
                            .await
                            .map_err(InfiltratorError::from)?;
                        let remote = tokio::fs::read_to_string(&conflict.remote_path)
                            .await
                            .map_err(|error| InfiltratorError::Io(error.to_string()))?;
                        let merged = infiltrator_domain::mixin::merge_yaml_key_picks(
                            &local,
                            &remote,
                            &take_remote,
                            &accept_removals,
                        )
                        .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                        infiltrator_core::config::validate_yaml(&merged)
                            .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                        crate::update::core::profile_apply::save_profile_content(
                            runtime,
                            conflict.profile.clone(),
                            merged,
                            ApplyStrategy::PreferReload,
                        )
                        .await?;
                        tokio::fs::remove_file(&conflict.remote_path)
                            .await
                            .map_err(|error| InfiltratorError::Io(error.to_string()))?;
                        Ok(conflict.profile)
                    },
                    Message::SyncDiffMerged,
                )
            }
            Message::SyncDiffMerged(result) => {
                self.profile.is_applying_sync_diff = false;
                match result {
                    Ok(profile) => {
                        self.profile.sync_diff = None;
                        self.profile
                            .sync_conflicts
                            .retain(|conflict| conflict.profile != profile);
                        if let Some(runtime) = self.runtime.runtime.clone() {
                            self.sync_runtime_slot(Some(runtime));
                        }
                        self.invalidate_rules_dns_views();
                        let lang = Lang(&self.shell.lang);
                        Task::batch(vec![
                            Task::done(Message::LoadProfiles),
                            Task::done(Message::ShowToast(
                                format!("{}：{profile}", lang.tr("toast_diff_merged")),
                                ToastStatus::Success,
                            )),
                        ])
                    }
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

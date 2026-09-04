//! WebDAV sync handlers: upload/download of profiles to the user's DAV
//! account, completion toasts and the periodic sync tick.

use crate::state::AppState;
use crate::types::app::{SyncConflict, SyncProgress, SyncSummary, ToastStatus};
use crate::types::message::Message;
use dav_client::{DavClient, client::WebDavClient};
use iced::futures::SinkExt;
use iced::{Task, stream};
use infiltrator_contract::error::InfiltratorError;
use infiltrator_shared::locales::{Lang, Localizer};
use mihomo_platform::sandbox_validator::{PathValidationResult, SandboxValidator};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::AsyncWriteExt;

impl AppState {
    pub(super) fn update_sync(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SyncUpload => {
                if self.profile.is_syncing {
                    // A manual sync is already in flight; the tick-spawned
                    // chain is dropped here, so its notify flag must go too
                    // (otherwise the manual completion would notify).
                    self.profile.sync_from_tick = false;
                    return Task::none();
                }
                let url = self.profile.webdav_url.clone();
                let user = self.profile.webdav_user.clone();
                let pass = self.profile.webdav_pass.clone();
                // 客户端在 spawn 前构造（测试注入点：指向本地桩服务器）。
                // 构造失败不 spawn worker，直接走 SyncFinished 错误臂 ——
                // toast/清理/通知语义与原先 worker 内构造失败完全等价。
                let client = match WebDavClient::new(&url, &user, &pass) {
                    Ok(client) => client,
                    Err(e) => {
                        return Task::done(Message::SyncFinished(Err(InfiltratorError::Sync(
                            e.to_string(),
                        ))));
                    }
                };
                let cancel = Arc::new(AtomicBool::new(false));
                self.profile.sync_cancel = Some(cancel.clone());
                self.profile.is_syncing = true;
                self.refresh_tray();
                let operation = stream::channel(
                    100,
                    move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                        let result = async {
                            let cm = crate::configs_dir::config_manager().await?;
                            let profiles =
                                cm.list_profiles().await.map_err(infiltrator_contract::error::from_mihomo)?;
                            let total = profiles.len();
                            let _ = output.try_send(Message::SyncProgress(SyncProgress {
                                phase: "上传配置".to_string(),
                                current: 0,
                                total,
                            }));

                            let mut uploaded = 0usize;
                            for profile in profiles {
                                check_cancelled(&cancel)?;
                                let content = tokio::fs::read_to_string(&profile.path)
                                    .await
                                    .map_err(|e| InfiltratorError::Io(e.to_string()))?;
                                check_cancelled(&cancel)?;
                                client
                                    .put(
                                        &format!("{}.yaml", profile.name),
                                        content.as_bytes(),
                                        None,
                                    )
                                    .await
                                    .map_err(|e| InfiltratorError::Sync(e.to_string()))?;
                                uploaded += 1;
                                let _ = output.try_send(Message::SyncProgress(SyncProgress {
                                    phase: "上传配置".to_string(),
                                    current: uploaded,
                                    total,
                                }));
                            }
                            Ok(SyncSummary {
                                uploaded,
                                ..SyncSummary::default()
                            })
                        }
                        .await;
                        let _ = output.send(Message::SyncFinished(result)).await;
                    },
                );
                Task::run(operation, |message| message)
            }
            Message::SyncDownload => {
                if self.profile.is_syncing {
                    return Task::none();
                }
                let url = self.profile.webdav_url.clone();
                let user = self.profile.webdav_user.clone();
                let pass = self.profile.webdav_pass.clone();
                // 同 SyncUpload：客户端在 spawn 前构造（测试注入点），
                // 构造失败提前定论并走同一 SyncFinished 错误臂。
                let client = match WebDavClient::new(&url, &user, &pass) {
                    Ok(client) => client,
                    Err(e) => {
                        return Task::done(Message::SyncFinished(Err(InfiltratorError::Sync(
                            e.to_string(),
                        ))));
                    }
                };
                let cancel = Arc::new(AtomicBool::new(false));
                self.profile.sync_cancel = Some(cancel.clone());
                self.profile.is_syncing = true;
                self.refresh_tray();
                let runtime_present = self.runtime.runtime.is_some();
                let operation = stream::channel(
                    100,
                    move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                        let result = async {
                            let files = client
                                .list("")
                                .await
                                .map_err(|e| InfiltratorError::Sync(e.to_string()))?;
                            let mut remote_profiles = Vec::new();
                            let mut remote_names = HashSet::new();
                            for file in files {
                                if let Some(profile_name) = safe_remote_profile_name(&file.path)? {
                                    if !remote_names.insert(profile_name.clone()) {
                                        return Err(InfiltratorError::Sync(format!(
                                            "远端配置路径映射冲突: {}",
                                            profile_name
                                        )));
                                    }
                                    remote_profiles.push((file.path, profile_name));
                                }
                            }
                            // 沙箱根必须与 manager 的 configs 目录解析一致
                            //（env > settings `configs_dir`），否则重定向后
                            // 下载写入会被判定越界。
                            let config_root = crate::configs_dir::configs_dir().await?;
                            let sandbox = SandboxValidator::new(config_root.clone());
                            let manager = crate::configs_dir::config_manager().await?;
                            let active_profile = manager
                                .get_current()
                                .await
                                .map_err(infiltrator_contract::error::from_mihomo)?;
                            let total = remote_profiles.len();
                            let _ = output.try_send(Message::SyncProgress(SyncProgress {
                                phase: "下载配置".to_string(),
                                current: 0,
                                total,
                            }));

                            let mut downloaded = 0usize;
                            let mut conflicts = 0usize;
                            let mut conflict_files = Vec::new();
                            let mut active_profile_changed = false;
                            let mut processed = 0usize;
                            for (remote_path, profile_name) in remote_profiles {
                                check_cancelled(&cancel)?;
                                let content = client
                                    .get(&remote_path)
                                    .await
                                    .map_err(|e| InfiltratorError::Sync(e.to_string()))?;
                                let content = String::from_utf8(content).map_err(|error| {
                                    InfiltratorError::Config(format!(
                                        "远端配置 {} 不是 UTF-8 YAML: {error}",
                                        remote_path
                                    ))
                                })?;
                                infiltrator_core::config::validate_yaml(&content)
                                    .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                                let path = config_root.join(format!("{profile_name}.yaml"));
                                if sandbox.validate_path(&path) != PathValidationResult::Allowed {
                                    return Err(InfiltratorError::Sync(format!(
                                        "远端配置目标超出本地配置目录: {}",
                                        path.display()
                                    )));
                                }
                                match tokio::fs::read_to_string(&path).await {
                                    Ok(local) if local == content => {
                                        processed += 1;
                                        let _ =
                                            output.try_send(Message::SyncProgress(SyncProgress {
                                                phase: "下载配置".to_string(),
                                                current: processed,
                                                total,
                                            }));
                                        continue;
                                    }
                                    Ok(_) => {
                                        let conflict_path = conflict_backup_path(&path);
                                        atomic_write_file(&conflict_path, content.as_bytes())
                                            .await?;
                                        conflicts += 1;
                                        conflict_files.push(SyncConflict {
                                            profile: profile_name.clone(),
                                            remote_path: conflict_path,
                                        });
                                        processed += 1;
                                        let _ =
                                            output.try_send(Message::SyncProgress(SyncProgress {
                                                phase: "下载配置".to_string(),
                                                current: processed,
                                                total,
                                            }));
                                        continue;
                                    }
                                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                                    Err(error) => {
                                        return Err(InfiltratorError::Io(error.to_string()));
                                    }
                                }
                                check_cancelled(&cancel)?;
                                manager
                                    .save(&profile_name, &content)
                                    .await
                                    .map_err(infiltrator_contract::error::from_mihomo)?;
                                let is_active = active_profile == profile_name;
                                if is_active {
                                    active_profile_changed = true;
                                }
                                if !is_active || !runtime_present {
                                    manager
                                        .clear_backup(&profile_name)
                                        .await
                                        .map_err(infiltrator_contract::error::from_mihomo)?;
                                }
                                downloaded += 1;
                                processed += 1;
                                let _ = output.try_send(Message::SyncProgress(SyncProgress {
                                    phase: "下载配置".to_string(),
                                    current: processed,
                                    total,
                                }));
                            }
                            Ok(SyncSummary {
                                downloaded,
                                conflicts,
                                active_profile_changed,
                                conflict_files,
                                ..SyncSummary::default()
                            })
                        }
                        .await;
                        let _ = output.send(Message::SyncFinished(result)).await;
                    },
                );
                Task::run(operation, |message| message)
            }
            Message::SyncFinished(result) => {
                // 0.20: only the scheduler-driven chain (TickWebDavSync →
                // SyncUpload) notifies; manual upload/download stay silent.
                // The flag is consumed exactly once, on either outcome.
                let notify_this_sync = std::mem::take(&mut self.profile.sync_from_tick);
                self.profile.is_syncing = false;
                self.profile.sync_cancel = None;
                self.profile.sync_progress = None;
                self.refresh_tray();
                match result {
                    Ok(summary) => {
                        self.profile.sync_conflicts = summary.conflict_files.clone();
                        let mut tasks = vec![Task::done(Message::LoadProfiles)];
                        if summary.active_profile_changed && self.runtime.runtime.is_some() {
                            tasks.push(self.trigger_runtime_rebuild());
                        } else if summary.active_profile_changed {
                            // The worker deliberately kept the backup for
                            // an active profile in case a live core needed a
                            // rebuild. If the core stopped meanwhile there
                            // is no rebuild consumer, so clear that transient
                            // file after the durable download succeeds.
                            tasks.push(Task::perform(
                                async {
                                    if let Ok(manager) = crate::configs_dir::config_manager().await
                                        && let Ok(profile) = manager.get_current().await
                                    {
                                        let _ = manager.clear_backup(&profile).await;
                                    }
                                },
                                |_| Message::Noop,
                            ));
                        }
                        let lang = Lang(&self.shell.lang);
                        tasks.push(Task::done(Message::ShowToast(
                            format!(
                                "{}：{} {}，{} {}，{} {}",
                                lang.tr("toast_sync_finished"),
                                lang.tr("toast_sync_uploaded"),
                                summary.uploaded,
                                lang.tr("toast_sync_downloaded"),
                                summary.downloaded,
                                lang.tr("toast_sync_conflicts"),
                                summary.conflicts
                            ),
                            if summary.conflicts > 0 {
                                ToastStatus::Warning
                            } else {
                                ToastStatus::Success
                            },
                        )));
                        if notify_this_sync {
                            // 正文带三向计数，有冲突时抬到 Normal，否则 Low。
                            tasks.push(self.system_notify(
                                "notify_sync_finished",
                                &format!(
                                    "{} {}, {} {}, {} {}",
                                    lang.tr("toast_sync_uploaded"),
                                    summary.uploaded,
                                    lang.tr("toast_sync_downloaded"),
                                    summary.downloaded,
                                    lang.tr("toast_sync_conflicts"),
                                    summary.conflicts
                                ),
                                if summary.conflicts > 0 {
                                    crate::notify::NotifyUrgency::Normal
                                } else {
                                    crate::notify::NotifyUrgency::Low
                                },
                            ));
                        }
                        Task::batch(tasks)
                    }
                    Err(e) => {
                        self.set_error(&e);
                        let cancelled = e.to_string().contains("同步已取消");
                        let toast = Task::done(Message::ShowToast(
                            e.to_string(),
                            if cancelled {
                                ToastStatus::Warning
                            } else {
                                ToastStatus::Error
                            },
                        ));
                        // 「同步已取消」（用户主动操作）不发系统通知。
                        if notify_this_sync && !cancelled {
                            Task::batch(vec![
                                toast,
                                self.system_notify(
                                    "notify_sync_failed",
                                    &e.to_string(),
                                    crate::notify::NotifyUrgency::Critical,
                                ),
                            ])
                        } else {
                            toast
                        }
                    }
                }
            }
            Message::SyncProgress(progress) => {
                self.profile.sync_progress = Some(progress);
                self.refresh_tray_throttled();
                Task::none()
            }
            Message::ResolveSyncConflict(profile) => {
                let Some(conflict) = self
                    .profile
                    .sync_conflicts
                    .iter()
                    .find(|conflict| conflict.profile == profile)
                    .cloned()
                else {
                    return Task::none();
                };
                let runtime = self.runtime.runtime.clone();
                Task::perform(
                    async move {
                        let content = tokio::fs::read_to_string(&conflict.remote_path)
                            .await
                            .map_err(|error| InfiltratorError::Io(error.to_string()))?;
                        infiltrator_core::config::validate_yaml(&content)
                            .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                        crate::update::core::profile_apply::save_profile_content(
                            runtime,
                            conflict.profile.clone(),
                            content,
                            infiltrator_core::apply::ApplyStrategy::PreferReload,
                        )
                        .await?;
                        tokio::fs::remove_file(&conflict.remote_path)
                            .await
                            .map_err(|error| InfiltratorError::Io(error.to_string()))?;
                        Ok(conflict.profile)
                    },
                    Message::SyncConflictResolved,
                )
            }
            Message::SyncConflictResolved(result) => match result {
                Ok(profile) => {
                    self.profile
                        .sync_conflicts
                        .retain(|conflict| conflict.profile != profile);
                    if let Some(runtime) = self.runtime.runtime.clone() {
                        self.sync_runtime_slot(Some(runtime));
                    }
                    Task::batch(vec![
                        Task::done(Message::LoadProfiles),
                        Task::done(Message::ShowToast(
                            format!(
                                "{}：{profile}",
                                Lang(&self.shell.lang).tr("toast_sync_conflict_resolved")
                            ),
                            ToastStatus::Success,
                        )),
                    ])
                }
                Err(error) => {
                    self.set_error(&error);
                    Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                }
            },
            Message::DismissSyncConflict(profile) => {
                let Some(conflict) = self
                    .profile
                    .sync_conflicts
                    .iter()
                    .find(|conflict| conflict.profile == profile)
                    .cloned()
                else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        tokio::fs::remove_file(&conflict.remote_path)
                            .await
                            .map_err(|error| InfiltratorError::Io(error.to_string()))?;
                        Ok(conflict.profile)
                    },
                    Message::SyncConflictDismissed,
                )
            }
            Message::SyncConflictDismissed(result) => match result {
                Ok(profile) => {
                    self.profile
                        .sync_conflicts
                        .retain(|conflict| conflict.profile != profile);
                    Task::done(Message::ShowToast(
                        format!(
                            "{}：{profile}",
                            Lang(&self.shell.lang).tr("toast_sync_conflict_dismissed")
                        ),
                        ToastStatus::Info,
                    ))
                }
                Err(error) => {
                    self.set_error(&error);
                    Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                }
            },
            Message::CancelWebDavSync => {
                if let Some(cancel) = &self.profile.sync_cancel {
                    cancel.store(true, Ordering::Release);
                }
                Task::none()
            }
            Message::TestWebDavConnection => {
                let url = self.profile.webdav_url.trim().to_string();
                let user = self.profile.webdav_user.trim().to_string();
                let pass = self.profile.webdav_pass.clone();
                if url.is_empty() || user.is_empty() {
                    return Task::done(Message::ShowToast(
                        "WebDAV 地址和用户名不能为空".to_string(),
                        ToastStatus::Error,
                    ));
                }
                self.profile.is_testing_webdav = true;
                Task::perform(
                    async move {
                        let client = WebDavClient::new(&url, &user, &pass)
                            .map_err(|e| InfiltratorError::Sync(e.to_string()))?;
                        client
                            .list("")
                            .await
                            .map_err(|e| InfiltratorError::Sync(e.to_string()))?;
                        Ok(())
                    },
                    Message::WebDavConnectionTested,
                )
            }
            Message::WebDavConnectionTested(result) => {
                self.profile.is_testing_webdav = false;
                match result {
                    Ok(()) => Task::done(Message::ShowToast(
                        "WebDAV 连接成功".to_string(),
                        ToastStatus::Success,
                    )),
                    Err(error) => {
                        self.set_error(&error);
                        Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::TickWebDavSync => {
                if self.profile.webdav_enabled
                    && !self.profile.webdav_url.is_empty()
                    && !self.profile.webdav_user.is_empty()
                {
                    // 0.20: 标记这条链为调度器驱动，SyncFinished 才会发系统
                    // 通知；手动上传/下载不置位、保持静默。
                    self.profile.sync_from_tick = true;
                    return Task::done(Message::SyncUpload);
                }
                Task::none()
            }
            other => self.update_sync_diff(other),
        }
    }
}

fn check_cancelled(cancel: &AtomicBool) -> Result<(), InfiltratorError> {
    if cancel.load(Ordering::Acquire) {
        Err(InfiltratorError::Sync("同步已取消".to_string()))
    } else {
        Ok(())
    }
}

fn safe_remote_profile_name(remote_path: &str) -> Result<Option<String>, InfiltratorError> {
    let trimmed = remote_path.trim_matches('/');
    if remote_path.contains('\\')
        || remote_path.contains("://")
        || trimmed.split('/').any(|part| part == "..")
    {
        return Err(InfiltratorError::Sync(format!(
            "拒绝不安全的远端配置路径: {remote_path}"
        )));
    }
    let Some(file_name) = trimmed.rsplit('/').next() else {
        return Ok(None);
    };
    if !file_name.ends_with(".yaml") && !file_name.ends_with(".yml") {
        return Ok(None);
    }
    if file_name == ".yaml" || file_name == ".yml" || file_name.contains("..") {
        return Err(InfiltratorError::Sync(format!(
            "拒绝不安全的远端配置路径: {remote_path}"
        )));
    }
    let profile_name = file_name
        .rsplit_once('.')
        .map(|(name, _)| name)
        .unwrap_or_default();
    infiltrator_domain::profiles::sanitize_profile_name(profile_name)
        .map(Some)
        .map_err(|error| InfiltratorError::Config(error.to_string()))
}

fn conflict_backup_path(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("profile");
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    path.with_file_name(format!("{stem}.remote-conflict-{stamp}.yaml"))
}

async fn atomic_write_file(path: &Path, content: &[u8]) -> Result<(), InfiltratorError> {
    let parent = path
        .parent()
        .ok_or_else(|| InfiltratorError::Io(format!("路径没有父目录: {}", path.display())))?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(infiltrator_contract::error::from_mihomo)?;
    let temp = path.with_file_name(format!(
        ".{}.sync-tmp",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("profile")
    ));
    let result = async {
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .await?;
        file.write_all(content).await?;
        file.sync_all().await?;
        drop(file);
        #[cfg(windows)]
        if tokio::fs::try_exists(path).await? {
            tokio::fs::remove_file(path).await?;
        }
        tokio::fs::rename(&temp, path).await?;
        Ok::<(), std::io::Error>(())
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&temp).await;
    }
    result.map_err(|error| InfiltratorError::Io(error.to_string()))
}

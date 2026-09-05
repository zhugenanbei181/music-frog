//! WebDAV sync handlers: upload/download of profiles to the user's DAV
//! account, completion toasts and the periodic sync tick.

use crate::state::AppState;
use crate::types::app::{SyncConflict, SyncProgress, SyncSummary, ToastStatus};
use crate::types::message::Message;
use iced::futures::SinkExt;
use iced::{Task, stream};
use infiltrator_application::sync_application::SyncApplication;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::sync::{
    SyncConflict as ContractSyncConflict, SyncProgress as ContractSyncProgress,
    SyncTransferReport,
};
use infiltrator_domain::settings::WebDavConfig;
use infiltrator_ports::sync::SyncProgressSink;
use infiltrator_shared::locales::{Lang, Localizer};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

struct IcedSyncProgressSink {
    output: std::sync::Mutex<iced::futures::channel::mpsc::Sender<Message>>,
    cancel: Arc<AtomicBool>,
}

impl IcedSyncProgressSink {
    fn new(
        output: iced::futures::channel::mpsc::Sender<Message>,
        cancel: Arc<AtomicBool>,
    ) -> Self {
        Self {
            output: std::sync::Mutex::new(output),
            cancel,
        }
    }
}

impl SyncProgressSink for IcedSyncProgressSink {
    fn progress(&self, progress: ContractSyncProgress) {
        if let Ok(mut output) = self.output.lock() {
            let _ = output.try_send(Message::SyncProgress(SyncProgress {
                phase: progress.phase,
                current: progress.current as usize,
                total: progress.total as usize,
            }));
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Acquire)
    }
}

pub(super) fn sync_application() -> Result<SyncApplication, InfiltratorError> {
    let port = infiltrator_desktop::storage::sync()
        .map_err(|error| InfiltratorError::Sync(error.to_string()))?;
    Ok(SyncApplication::new(Arc::new(port)))
}

fn webdav_config(
    enabled: bool,
    url: String,
    username: String,
    password: String,
) -> WebDavConfig {
    WebDavConfig {
        enabled,
        url,
        username,
        password,
        ..WebDavConfig::default()
    }
}

fn transfer_to_summary(report: SyncTransferReport) -> SyncSummary {
    SyncSummary {
        uploaded: report.uploaded as usize,
        downloaded: report.downloaded as usize,
        conflicts: report.conflicts as usize,
        active_profile_changed: report.active_profile_changed,
        conflict_files: report
            .conflict_files
            .into_iter()
            .map(contract_conflict_to_ui)
            .collect(),
    }
}

fn contract_conflict_to_ui(conflict: ContractSyncConflict) -> SyncConflict {
    SyncConflict {
        profile: conflict.profile,
        remote_path: conflict.remote_path.into(),
    }
}

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
                let application = match sync_application() {
                    Ok(application) => application,
                    Err(error) => {
                        return Task::done(Message::SyncFinished(Err(error)));
                    }
                };
                let config = webdav_config(
                    self.profile.webdav_enabled,
                    self.profile.webdav_url.clone(),
                    self.profile.webdav_user.clone(),
                    self.profile.webdav_pass.clone(),
                );
                let cancel = Arc::new(AtomicBool::new(false));
                self.profile.sync_cancel = Some(cancel.clone());
                self.profile.is_syncing = true;
                self.refresh_tray();
                let operation = stream::channel(
                    100,
                    move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                        let observer = Arc::new(IcedSyncProgressSink::new(
                            output.clone(),
                            cancel.clone(),
                        ));
                        let result = async {
                            let configs_dir = crate::configs_dir::configs_dir()
                                .await?
                                .to_string_lossy()
                                .into_owned();
                            application
                                .upload(config, Some(configs_dir), observer)
                                .await
                                .map(transfer_to_summary)
                                .map_err(|failure| InfiltratorError::Sync(failure.message))
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
                let application = match sync_application() {
                    Ok(application) => application,
                    Err(error) => {
                        return Task::done(Message::SyncFinished(Err(error)));
                    }
                };
                let config = webdav_config(
                    self.profile.webdav_enabled,
                    self.profile.webdav_url.clone(),
                    self.profile.webdav_user.clone(),
                    self.profile.webdav_pass.clone(),
                );
                let cancel = Arc::new(AtomicBool::new(false));
                self.profile.sync_cancel = Some(cancel.clone());
                self.profile.is_syncing = true;
                self.refresh_tray();
                let runtime_present = self.runtime.runtime.is_some();
                let operation = stream::channel(
                    100,
                    move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                        let observer = Arc::new(IcedSyncProgressSink::new(
                            output.clone(),
                            cancel.clone(),
                        ));
                        let result = async {
                            let configs_dir = crate::configs_dir::configs_dir()
                                .await?
                                .to_string_lossy()
                                .into_owned();
                            application
                                .download(
                                    config,
                                    Some(configs_dir),
                                    runtime_present,
                                    observer,
                                )
                                .await
                                .map(transfer_to_summary)
                                .map_err(|failure| InfiltratorError::Sync(failure.message))
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
                        let content = read_conflict_file(&conflict).await?;
                        infiltrator_domain::config::validate_yaml(&content)
                            .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                        crate::update::core::profile_apply::save_profile_content(
                            runtime,
                            conflict.profile.clone(),
                            content,
                            infiltrator_domain::apply::ApplyStrategy::PreferReload,
                        )
                        .await?;
                        delete_conflict_file(&conflict).await?;
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
                        delete_conflict_file(&conflict).await?;
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
                let application = match sync_application() {
                    Ok(application) => application,
                    Err(error) => {
                        return Task::done(Message::WebDavConnectionTested(Err(error)));
                    }
                };
                let config = webdav_config(true, url, user, pass);
                Task::perform(
                    async move { application.test(config).await.map(|_| ()).map_err(|failure| InfiltratorError::Sync(failure.message)) },
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

async fn read_conflict_file(conflict: &SyncConflict) -> Result<String, InfiltratorError> {
    let configs_dir = crate::configs_dir::configs_dir().await?;
    sync_application()?
        .read_conflict(
            configs_dir.to_string_lossy().into_owned(),
            conflict.remote_path.to_string_lossy().into_owned(),
        )
        .await
        .map_err(|failure| InfiltratorError::Io(failure.message))
}

async fn delete_conflict_file(conflict: &SyncConflict) -> Result<(), InfiltratorError> {
    let configs_dir = crate::configs_dir::configs_dir().await?;
    sync_application()?
        .delete_conflict(
            configs_dir.to_string_lossy().into_owned(),
            conflict.remote_path.to_string_lossy().into_owned(),
        )
        .await
        .map_err(|failure| InfiltratorError::Io(failure.message))
}

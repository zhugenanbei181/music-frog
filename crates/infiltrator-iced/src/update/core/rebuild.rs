//! Runtime rebuild flow: the shared "saved, now rebuild mihomo" state
//! machine used by every config save path (rules, providers, sniffer,
//! DNS, Fake-IP, TUN).

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use crate::types::runtime::{RebuildFlowState, RuntimeStatus};
use iced::Task;
use infiltrator_core::error::InfiltratorError;
use mihomo_version::manager::VersionManager;
use std::sync::Arc;

impl AppState {
    fn active_rebuild_label(&self) -> String {
        match &self.runtime.rebuild_flow {
            RebuildFlowState::Saving { label }
            | RebuildFlowState::Rebuilding { label }
            | RebuildFlowState::Done { label }
            | RebuildFlowState::Failed { label, .. } => label.clone(),
            RebuildFlowState::Idle => "Configuration".to_string(),
        }
    }

    pub(super) fn begin_save_phase(&mut self, label: &str) {
        self.runtime.rebuild_flow = RebuildFlowState::Saving {
            label: label.to_string(),
        };
    }

    pub(super) fn finish_without_rebuild(&mut self, label: String) -> Task<Message> {
        if let Some(runtime) = self.runtime.runtime.clone() {
            // A successful AlwaysRestart apply keeps the same Arc but moves
            // the CoreSession to a new generation. Refresh the app/admin
            // snapshot before the stream subscription accepts new events.
            self.sync_runtime_slot(Some(runtime));
        }
        self.runtime.rebuild_flow = RebuildFlowState::Done {
            label: label.clone(),
        };
        let clear_backup = Task::perform(
            async {
                if let Ok(manager) = crate::configs_dir::config_manager().await
                    && let Ok(profile) = manager.get_current().await
                {
                    let _ = manager.clear_backup(&profile).await;
                }
            },
            |_| Message::Noop,
        );
        Task::batch(vec![
            clear_backup,
            Task::done(Message::ShowToast(
                format!("{label} saved"),
                ToastStatus::Success,
            )),
            Task::perform(
                async {
                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                },
                |_| Message::ClearRebuildFlow,
            ),
        ])
    }

    pub(crate) fn trigger_runtime_rebuild(&mut self) -> Task<Message> {
        let label = self.active_rebuild_label();
        let Some(runtime) = self.take_app_runtime() else {
            return self.finish_without_rebuild(label);
        };

        self.runtime.rebuild_flow = RebuildFlowState::Rebuilding {
            label: label.clone(),
        };
        self.runtime.status = RuntimeStatus::Starting;

        Task::perform(
            async move {
                let _ = runtime.shutdown().await;
                let manager = crate::configs_dir::config_manager().await?;
                let profile = manager
                    .get_current()
                    .await
                    .map_err(InfiltratorError::from)?;
                let vm = VersionManager::new().map_err(InfiltratorError::from)?;
                let data_dir =
                    mihomo_platform::paths::get_home_dir().map_err(InfiltratorError::from)?;
                let candidates = vec![];
                match infiltrator_desktop::boot::bootstrap_with_retry(
                    &vm,
                    true,
                    &candidates,
                    &data_dir,
                )
                .await
                {
                    Ok(outcome) => {
                        manager
                            .clear_backup(&profile)
                            .await
                            .map_err(InfiltratorError::from)?;
                        Ok(Arc::new(outcome.runtime))
                    }
                    Err(cause) => {
                        let restored = manager
                            .restore_backup(&profile)
                            .await
                            .map_err(InfiltratorError::from)?;
                        let _ = manager.clear_backup(&profile).await;
                        if restored {
                            Err(InfiltratorError::Mihomo(format!(
                                "重建失败，已恢复上一份配置: {cause}"
                            )))
                        } else {
                            Err(InfiltratorError::Mihomo(cause.to_string()))
                        }
                    }
                }
            },
            Message::RuntimeRebuildFinished,
        )
    }

    /// Rebuild flow completion. Unmatched messages fall through to the next
    /// domain in the `update_core` chain.
    pub(super) fn update_core_rebuild(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::RuntimeRebuildFinished(result) => {
                let label = self.active_rebuild_label();
                match result {
                    Ok(runtime) => {
                        self.sync_runtime_slot(Some(runtime));
                        self.runtime.status = RuntimeStatus::Running;
                        self.runtime.runtime_poll_tick = 0;
                        self.runtime.runtime_prev_upload_total = None;
                        self.runtime.runtime_prev_download_total = None;
                        self.runtime.runtime_prev_snapshot_at = None;
                        self.runtime.rebuild_flow = RebuildFlowState::Done {
                            label: label.clone(),
                        };
                        Task::batch(vec![
                            Task::done(Message::FetchRuntimeConfig),
                            Task::done(Message::LoadProxies),
                            Task::done(Message::RefreshRuntimeNow),
                            Task::done(Message::ShowToast(
                                format!("{label} saved and rebuilt"),
                                ToastStatus::Success,
                            )),
                            Task::perform(
                                async {
                                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                                },
                                |_| Message::ClearRebuildFlow,
                            ),
                        ])
                    }
                    Err(e) => {
                        self.runtime.status = RuntimeStatus::Error(e.clone());
                        self.set_error(&e);
                        self.runtime.rebuild_flow = RebuildFlowState::Failed {
                            label,
                            error: e.to_string(),
                        };
                        Task::batch(vec![
                            Task::done(Message::ShowToast(
                                format!("Rebuild failed: {}", e),
                                ToastStatus::Error,
                            )),
                            // 0.20: 重建失败可能发生在窗口不可见时（WebDAV
                            // 同步拉活 / WebUI 触发），补一条 Critical 系统通知。
                            self.system_notify(
                                "notify_rebuild_failed",
                                &e.to_string(),
                                crate::notify::NotifyUrgency::Critical,
                            ),
                            Task::perform(
                                async {
                                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                                },
                                |_| Message::ClearRebuildFlow,
                            ),
                        ])
                    }
                }
            }
            Message::ClearRebuildFlow => {
                self.runtime.rebuild_flow = RebuildFlowState::Idle;
                Task::none()
            }
            other => self.update_core_kernels(other),
        }
    }
}

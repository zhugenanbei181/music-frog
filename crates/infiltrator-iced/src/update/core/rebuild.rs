//! Runtime rebuild flow: the shared "saved, now rebuild mihomo" state
//! machine used by every config save path (rules, providers, sniffer,
//! DNS, Fake-IP, TUN).

use crate::configs_dir::config_manager;
use crate::notify::NotifyUrgency;
use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use crate::types::runtime::{RebuildFlowState, RuntimeStatus};
use iced::Task;
use infiltrator_desktop::product::DesktopProductSessionAction;
use tokio::time;
use tokio::time::sleep;

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
            // the CoreApplication to a new generation. Refresh the app/admin
            // snapshot before the stream subscription accepts new events.
            self.sync_runtime_slot(Some(runtime));
        }
        self.runtime.rebuild_flow = RebuildFlowState::Done {
            label: label.clone(),
        };
        let clear_backup = Task::perform(
            async {
                if let Ok(manager) = config_manager().await
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
                    sleep(time::Duration::from_secs(4)).await;
                },
                |_| Message::ClearRebuildFlow,
            ),
        ])
    }

    pub(crate) fn trigger_runtime_rebuild(&mut self) -> Task<Message> {
        let label = self.active_rebuild_label();
        let Some(session) = self.product_session.clone() else {
            if self.runtime.runtime.is_some() {
                let error = InfiltratorError::Config(
                    "runtime rebuild requires the bound DesktopProductSession".to_owned(),
                );
                self.runtime.status = RuntimeStatus::Error(error.clone());
                self.runtime.rebuild_flow = RebuildFlowState::Failed {
                    label,
                    error: error.to_string(),
                };
                return Task::done(Message::RuntimeRebuildFinished(Err(error)));
            }
            return self.finish_without_rebuild(label);
        };

        self.runtime.rebuild_flow = RebuildFlowState::Rebuilding {
            label: label.clone(),
        };
        self.runtime.status = RuntimeStatus::Starting;

        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    session.apply(DesktopProductSessionAction::RecomposeSelectedProfile)
                })
                .await
                .map_err(|error| {
                    InfiltratorError::Internal(format!(
                        "desktop Core owner action worker failed: {error}"
                    ))
                })?
                .map(|_| ())
                .map_err(|error| InfiltratorError::Config(error.to_string()))
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
                    Ok(()) => {
                        if let Err(failure) = self.refresh_desktop_product_session() {
                            let error = InfiltratorError::Config(failure.to_string());
                            self.runtime.status = RuntimeStatus::Error(error.clone());
                            self.set_error(&error);
                            self.runtime.rebuild_flow = RebuildFlowState::Failed {
                                label,
                                error: error.to_string(),
                            };
                            return Task::done(Message::ShowToast(
                                format!("Rebuild failed: {error}"),
                                ToastStatus::Error,
                            ));
                        }
                        self.runtime.runtime_poll_tick = 0;
                        self.runtime.runtime_prev_upload_total = None;
                        self.runtime.runtime_prev_download_total = None;
                        self.runtime.runtime_prev_snapshot_at = None;
                        self.runtime.rebuild_flow = RebuildFlowState::Done {
                            label: label.clone(),
                        };
                        Task::batch(vec![
                            Task::done(Message::FetchRuntimeConfig),
                            Task::done(Message::RefreshOutboundInventory),
                            Task::done(Message::RefreshRuntimeNow),
                            Task::done(Message::ShowToast(
                                format!("{label} saved and rebuilt"),
                                ToastStatus::Success,
                            )),
                            Task::perform(
                                async {
                                    sleep(time::Duration::from_secs(4)).await;
                                },
                                |_| Message::ClearRebuildFlow,
                            ),
                        ])
                    }
                    Err(e) => {
                        let _ = self.refresh_desktop_product_session();
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
                                NotifyUrgency::Critical,
                            ),
                            Task::perform(
                                async {
                                    sleep(time::Duration::from_secs(4)).await;
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

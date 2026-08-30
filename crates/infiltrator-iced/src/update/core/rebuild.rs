//! Runtime rebuild flow: the shared "saved, now rebuild mihomo" state
//! machine used by every config save path (rules, providers, sniffer,
//! DNS, Fake-IP, TUN).

use crate::state::AppState;
use crate::types::{InfiltratorError, Message, RebuildFlowState, RuntimeStatus, ToastStatus};
use infiltrator_desktop::runtime::MihomoRuntime;
use mihomo_version::manager::VersionManager;
use std::sync::Arc;
use iced::Task;

impl AppState {
    fn active_rebuild_label(&self) -> String {
        match &self.rebuild_flow {
            RebuildFlowState::Saving { label }
            | RebuildFlowState::Rebuilding { label }
            | RebuildFlowState::Done { label }
            | RebuildFlowState::Failed { label, .. } => label.clone(),
            RebuildFlowState::Idle => "Configuration".to_string(),
        }
    }

    pub(super) fn begin_save_phase(&mut self, label: &str) {
        self.rebuild_flow = RebuildFlowState::Saving {
            label: label.to_string(),
        };
    }

    fn finish_without_rebuild(&mut self, label: String) -> Task<Message> {
        self.rebuild_flow = RebuildFlowState::Done {
            label: label.clone(),
        };
        Task::batch(vec![
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

    pub(super) fn trigger_runtime_rebuild(&mut self) -> Task<Message> {
        let label = self.active_rebuild_label();
        let Some(runtime) = self.take_app_runtime() else {
            return self.finish_without_rebuild(label);
        };

        self.rebuild_flow = RebuildFlowState::Rebuilding {
            label: label.clone(),
        };
        self.status = RuntimeStatus::Starting;

        Task::perform(
            async move {
                let _ = runtime.shutdown().await;
                let vm = VersionManager::new().map_err(InfiltratorError::from)?;
                let data_dir = mihomo_platform::paths::get_home_dir().map_err(InfiltratorError::from)?;
                let candidates = vec![];
                let rebuilt = MihomoRuntime::bootstrap(&vm, true, &candidates, &data_dir)
                    .await
                    .map_err(|e: anyhow::Error| InfiltratorError::Mihomo(e.to_string()))?;
                Ok(Arc::new(rebuilt))
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
                        self.status = RuntimeStatus::Running;
                        self.runtime_poll_tick = 0;
                        self.runtime_prev_upload_total = None;
                        self.runtime_prev_download_total = None;
                        self.rebuild_flow = RebuildFlowState::Done {
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
                        self.status = RuntimeStatus::Error(e.clone());
                        self.set_error(&e);
                        self.rebuild_flow = RebuildFlowState::Failed {
                            label,
                            error: e.to_string(),
                        };
                        Task::batch(vec![
                            Task::done(Message::ShowToast(
                                format!("Rebuild failed: {}", e),
                                ToastStatus::Error,
                            )),
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
                self.rebuild_flow = RebuildFlowState::Idle;
                Task::none()
            }
            other => self.update_core_kernels(other),
        }
    }
}

//! Proxy runtime lifecycle: booting and shutting down the mihomo runtime
//! plus app-level autostart wiring.

use crate::autostart;
use crate::state::AppState;
use crate::types::{InfiltratorError, Message, RuntimeStatus};
use iced::Task;
use infiltrator_desktop::runtime::MihomoRuntime;
use mihomo_version::manager::VersionManager;
use std::sync::Arc;

impl AppState {
    pub fn cancel_all_tasks(&mut self) {
        self.shell.last_task_id += 1;
    }

    /// Runtime start/stop plus autostart toggles. Unmatched messages fall
    /// through to the next domain in the [`update_core`] chain.
    pub(super) fn update_core_lifecycle(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::StartProxy => {
                self.cancel_all_tasks();
                self.runtime.status = RuntimeStatus::Starting;
                self.shell.error_msg = None;
                self.runtime.runtime_poll_tick = 0;
                self.runtime.runtime_prev_upload_total = None;
                self.runtime.runtime_prev_download_total = None;
                Task::perform(
                    async {
                        let vm = VersionManager::new().map_err(
                            |e: mihomo_api::error::MihomoError| {
                                InfiltratorError::Mihomo(e.to_string())
                            },
                        )?;
                        let data_dir = mihomo_platform::paths::get_home_dir().map_err(
                            |e: mihomo_api::error::MihomoError| {
                                InfiltratorError::Mihomo(e.to_string())
                            },
                        )?;
                        let candidates = vec![];
                        let r = MihomoRuntime::bootstrap(&vm, true, &candidates, &data_dir)
                            .await
                            .map_err(|e: anyhow::Error| InfiltratorError::Mihomo(e.to_string()))?;
                        Ok(Arc::new(r))
                    },
                    Message::ProxyStarted,
                )
            }
            Message::StopProxy => {
                self.cancel_all_tasks();
                let rt = self.take_app_runtime();
                self.runtime.status = RuntimeStatus::Stopped;
                Task::perform(
                    async move {
                        if let Some(r) = rt {
                            let _ = r.shutdown().await;
                        }
                    },
                    |_| Message::ProxyStopped,
                )
            }
            Message::ProxyStarted(result) => match result {
                Ok(runtime) => {
                    self.runtime.status = RuntimeStatus::Running;
                    self.sync_runtime_slot(Some(runtime));
                    Task::batch(vec![
                        Task::done(Message::FetchRuntimeConfig),
                        Task::done(Message::LoadProxies),
                        Task::done(Message::FetchIpInfo),
                        Task::done(Message::RefreshRuntimeNow),
                    ])
                }
                Err(e) => {
                    self.runtime.status = RuntimeStatus::Error(e.clone());
                    self.set_error(&e);
                    Task::none()
                }
            },
            Message::ProxyStopped => {
                self.diag.traffic = None;
                self.diag.traffic_history.clear();
                self.diag.connections = None;
                self.diag.memory = None;
                self.diag.public_ip = None;
                self.runtime.runtime_selected_group.clear();
                self.runtime.runtime_selected_proxy.clear();
                self.runtime.runtime_prev_upload_total = None;
                self.runtime.runtime_prev_download_total = None;
                self.runtime.runtime_poll_tick = 0;
                self.diag.logs.clear();
                self.runtime.proxy_mode = None;
                self.runtime.tun_enabled = None;
                self.runtime.status = RuntimeStatus::Stopped;
                Task::none()
            }
            Message::SetAutostart(enabled) => {
                self.runtime.autostart_enabled = enabled;
                Task::perform(
                    async move {
                        autostart::set_autostart_enabled(crate::AUTOSTART_REG_NAME, enabled)
                            .map_err(|e: anyhow::Error| InfiltratorError::Internal(e.to_string()))
                    },
                    Message::AutostartSet,
                )
            }
            Message::AutostartSet(result) => {
                if let Err(e) = result {
                    self.runtime.autostart_enabled = !self.runtime.autostart_enabled;
                    self.set_error(&e);
                }
                Task::none()
            }
            other => self.update_core_settings(other),
        }
    }
}

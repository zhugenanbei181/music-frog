//! Proxy runtime lifecycle: booting and shutting down the mihomo runtime
//! plus app-level autostart wiring.

use crate::autostart;
use crate::state::AppState;
use crate::types::{InfiltratorError, Message, RuntimeStatus};
use infiltrator_desktop::runtime::MihomoRuntime;
use mihomo_version::manager::VersionManager;
use std::sync::Arc;
use iced::Task;

impl AppState {
    pub fn cancel_all_tasks(&mut self) {
        self.last_task_id += 1;
    }

    /// Runtime start/stop plus autostart toggles. Unmatched messages fall
    /// through to the next domain in the [`update_core`] chain.
    pub(super) fn update_core_lifecycle(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::StartProxy => {
                self.cancel_all_tasks();
                self.status = RuntimeStatus::Starting;
                self.error_msg = None;
                self.runtime_poll_tick = 0;
                self.runtime_prev_upload_total = None;
                self.runtime_prev_download_total = None;
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
                            .map_err(|e: anyhow::Error| {
                                InfiltratorError::Mihomo(e.to_string())
                            })?;
                        Ok(Arc::new(r))
                    },
                    Message::ProxyStarted,
                )
            }
            Message::StopProxy => {
                self.cancel_all_tasks();
                let rt = self.take_app_runtime();
                self.status = RuntimeStatus::Stopped;
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
                    self.status = RuntimeStatus::Running;
                    self.sync_runtime_slot(Some(runtime));
                    Task::batch(vec![
                        Task::done(Message::FetchRuntimeConfig),
                        Task::done(Message::LoadProxies),
                        Task::done(Message::FetchIpInfo),
                        Task::done(Message::RefreshRuntimeNow),
                    ])
                }
                Err(e) => {
                    self.status = RuntimeStatus::Error(e.clone());
                    self.set_error(&e);
                    Task::none()
                }
            },
            Message::ProxyStopped => {
                self.traffic = None;
                self.traffic_history.clear();
                self.connections = None;
                self.memory = None;
                self.public_ip = None;
                self.runtime_selected_group.clear();
                self.runtime_selected_proxy.clear();
                self.runtime_prev_upload_total = None;
                self.runtime_prev_download_total = None;
                self.runtime_poll_tick = 0;
                self.logs.clear();
                self.proxy_mode = None;
                self.tun_enabled = None;
                self.status = RuntimeStatus::Stopped;
                Task::none()
            }
            Message::SetAutostart(enabled) => {
                self.autostart_enabled = enabled;
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
                    self.autostart_enabled = !self.autostart_enabled;
                    self.set_error(&e);
                }
                Task::none()
            }
            other => self.update_core_settings(other),
        }
    }
}

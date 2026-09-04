//! Proxy runtime lifecycle: booting and shutting down the mihomo runtime
//! plus app-level autostart wiring.

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::runtime::RuntimeStatus;
use iced::Task;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_shared::autostart;
use infiltrator_shared::locales::Localizer;
use mihomo_version::manager::VersionManager;
use std::sync::Arc;

impl AppState {
    pub fn cancel_all_tasks(&mut self) {
        self.shell.last_task_id += 1;
        self.runtime.lifecycle_token = self.runtime.lifecycle_token.wrapping_add(1);
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
                self.runtime.runtime_prev_snapshot_at = None;
                let lifecycle_token = self.runtime.lifecycle_token;
                Task::perform(
                    async {
                        let vm = VersionManager::new()
                            .map_err(|e| InfiltratorError::Mihomo(e.to_string()))?;
                        let data_dir = mihomo_platform::paths::get_home_dir()
                            .map_err(|e| InfiltratorError::Mihomo(e.to_string()))?;
                        let candidates = vec![];
                        // Boot retry loop: up to 3 attempts with controller
                        // port rotation between attempts (ledger §1.2).
                        let outcome = infiltrator_desktop::boot::bootstrap_with_retry(
                            &vm,
                            true,
                            &candidates,
                            &data_dir,
                        )
                        .await
                        .map_err(|e: anyhow::Error| {
                            if let Some(boot_error) =
                                e.downcast_ref::<infiltrator_desktop::boot::BootError>()
                            {
                                InfiltratorError::Mihomo(format!(
                                    "启动失败（已尝试控制端口 {:?}）: {}",
                                    boot_error.tried, boot_error.source
                                ))
                            } else {
                                InfiltratorError::Mihomo(e.to_string())
                            }
                        })?;
                        Ok((Arc::new(outcome.runtime), outcome.rotated))
                    },
                    move |result| Message::ProxyStarted(result, lifecycle_token),
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
            Message::ProxyStarted(result, lifecycle_token) => {
                if lifecycle_token != self.runtime.lifecycle_token {
                    if let Ok((runtime, _)) = result {
                        return Task::perform(
                            async move {
                                let _ = runtime.shutdown().await;
                            },
                            |_| Message::Noop,
                        );
                    }
                    return Task::none();
                }
                match result {
                    Ok((runtime, rotated)) => {
                        self.runtime.status = RuntimeStatus::Running;
                        self.sync_runtime_slot(Some(runtime.clone()));
                        let mut tasks = vec![
                            Task::done(Message::FetchRuntimeConfig),
                            Task::done(Message::LoadProxies),
                            Task::done(Message::RefreshRuntimeNow),
                        ];
                        if rotated {
                            let lang = infiltrator_shared::locales::Lang(&self.shell.lang);
                            tasks.push(Task::done(Message::ShowToast(
                                format!(
                                    "{} {}",
                                    lang.tr("toast_port_rotated"),
                                    runtime.controller_url
                                ),
                                crate::types::app::ToastStatus::Warning,
                            )));
                        }
                        self.refresh_tray();
                        Task::batch(tasks)
                    }
                    Err(e) => {
                        self.runtime.status = RuntimeStatus::Error(e.clone());
                        self.set_error(&e);
                        // 0.20: 启动失败往往发生在托盘/后台拉起时（窗口不可见），
                        // 除了错误横幅再发一条 Critical 系统通知。
                        self.system_notify(
                            "notify_kernel_error",
                            &e.to_string(),
                            crate::notify::NotifyUrgency::Critical,
                        )
                    }
                }
            }
            Message::ProxyStopped => {
                self.diag.traffic = None;
                self.diag.traffic_history.clear();
                self.diag.connections = None;
                self.diag.memory = None;
                self.diag.public_ip = None;
                self.diag.public_ip_provider = None;
                self.diag.public_ip_checked_at = None;
                self.diag.public_ip_error = None;
                self.runtime.runtime_selected_group.clear();
                self.runtime.runtime_selected_proxy.clear();
                self.runtime.runtime_prev_upload_total = None;
                self.runtime.runtime_prev_download_total = None;
                self.runtime.runtime_prev_snapshot_at = None;
                self.runtime.runtime_poll_tick = 0;
                self.diag.logs.clear();
                self.diag.logs_stream_state = crate::types::runtime::RuntimeStreamState::Idle;
                self.diag.traffic_stream_state = crate::types::runtime::RuntimeStreamState::Idle;
                self.diag.connections_stream_state =
                    crate::types::runtime::RuntimeStreamState::Idle;
                self.runtime.proxy_mode = None;
                self.runtime.script_block_present = false;
                self.runtime.tun_enabled = None;
                self.runtime.status = RuntimeStatus::Stopped;
                self.refresh_tray();
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
                self.refresh_tray();
                Task::none()
            }
            other => self.update_core_settings(other),
        }
    }
}

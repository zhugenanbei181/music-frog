//! Proxy runtime lifecycle: booting and shutting down the mihomo runtime
//! plus app-level autostart wiring.

use crate::host::boot::bootstrap_host_runtime_from_current_home;
use crate::notify::NotifyUrgency;
use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use crate::types::runtime::{RuntimeStatus, RuntimeStreamState};
use iced::Task;
use infiltrator_contract::core_control::CoreControlAction;
use infiltrator_contract::error::{Failure, InfiltratorError};
use infiltrator_desktop::boot::BootError;
use infiltrator_ports::runtime_gateway::ManagedRuntime;
use infiltrator_shared::autostart;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

impl AppState {
    pub fn cancel_all_tasks(&mut self) {
        self.shell.last_task_id += 1;
        self.runtime.lifecycle_token = self.runtime.lifecycle_token.wrapping_add(1);
        self.runtime.lifecycle_pending = None;
    }

    /// Runtime start/stop plus autostart toggles. Unmatched messages fall
    /// through to the next domain in the [`update_core`] chain.
    pub(super) fn update_core_lifecycle(&mut self, message: Message) -> Task<Message> {
        let copy_locale = self.shell.lang.clone();
        match message {
            Message::StartProxy => {
                if self.commands.is_some() {
                    return self.submit_shared_core_control(CoreControlAction::Start);
                }
                if self.runtime.host_composition_failure.is_some() {
                    return Task::none();
                }
                if self.runtime.lifecycle_pending.is_some()
                    || matches!(
                        self.runtime.status,
                        RuntimeStatus::Running | RuntimeStatus::Starting
                    )
                {
                    return Task::none();
                }
                self.cancel_all_tasks();
                self.runtime.lifecycle_pending = Some(CoreControlAction::Start);
                self.runtime.lifecycle_failure = None;
                self.runtime.status = RuntimeStatus::Starting;
                self.shell.error_msg = None;
                self.runtime.runtime_poll_tick = 0;
                self.runtime.runtime_prev_upload_total = None;
                self.runtime.runtime_prev_download_total = None;
                self.runtime.runtime_prev_snapshot_at = None;
                let lifecycle_token = self.runtime.lifecycle_token;
                Task::perform(
                    async move {
                        let candidates = vec![];
                        // Boot retry loop: up to 3 attempts with controller
                        // port rotation between attempts (ledger §1.2).
                        let outcome = bootstrap_host_runtime_from_current_home(true, &candidates)
                            .await
                            .map_err(|e: anyhow::Error| {
                                if let Some(boot_error) = e.downcast_ref::<BootError>() {
                                    InfiltratorError::Mihomo(localize(
                                        &copy_locale,
                                        "core_boot_ports_failed",
                                        &[
                                            ("ports", format!("{:?}", boot_error.tried)),
                                            ("reason", boot_error.source.to_string()),
                                        ],
                                    ))
                                } else {
                                    InfiltratorError::Mihomo(e.to_string())
                                }
                            })?;
                        Ok(outcome)
                    },
                    move |result| Message::ProxyStarted(result, lifecycle_token),
                )
            }
            Message::StopProxy => {
                if self.commands.is_some() {
                    return self.submit_shared_core_control(CoreControlAction::Stop);
                }
                if self.runtime.host_composition_failure.is_some() {
                    return Task::none();
                }
                if self.runtime.lifecycle_pending.is_some() {
                    return Task::none();
                }
                self.cancel_all_tasks();
                self.runtime.lifecycle_pending = Some(CoreControlAction::Stop);
                self.runtime.lifecycle_failure = None;
                let rt = self.runtime.runtime.clone();
                let token = self.runtime.lifecycle_token;
                Task::perform(
                    async move {
                        match rt {
                            Some(runtime) => ManagedRuntime::shutdown(runtime.as_ref())
                                .await
                                .map_err(Failure::from),
                            None => Ok(()),
                        }
                    },
                    move |result| Message::ProxyStopFinished(result, token),
                )
            }
            Message::ProxyStopFinished(result, token) => {
                if token != self.runtime.lifecycle_token
                    || self.runtime.lifecycle_pending != Some(CoreControlAction::Stop)
                {
                    return Task::none();
                }
                self.runtime.lifecycle_pending = None;
                match result {
                    Ok(()) => {
                        self.sync_runtime_slot(None);
                        self.update_core_lifecycle(Message::ProxyStopped)
                    }
                    Err(failure) => {
                        self.runtime.lifecycle_failure = Some(failure.clone());
                        self.set_error(InfiltratorError::Mihomo(failure.message));
                        Task::none()
                    }
                }
            }
            Message::CoreControlFinished {
                action,
                token,
                result,
                snapshot,
            } => self.finish_shared_core_control(action, token, result, *snapshot),
            Message::ProxyStarted(result, lifecycle_token) => {
                if lifecycle_token != self.runtime.lifecycle_token {
                    if let Ok((runtime, _)) = result {
                        return Task::perform(
                            async move {
                                let _ = ManagedRuntime::shutdown(runtime.as_ref()).await;
                            },
                            |_| Message::Noop,
                        );
                    }
                    return Task::none();
                }
                self.runtime.lifecycle_pending = None;
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
                            let lang = Lang(&self.shell.lang);
                            tasks.push(Task::done(Message::ShowToast(
                                format!(
                                    "{} {}",
                                    lang.tr("toast_port_rotated"),
                                    runtime.controller_url()
                                ),
                                ToastStatus::Warning,
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
                            NotifyUrgency::Critical,
                        )
                    }
                }
            }
            Message::ProxyStopped => {
                self.runtime.lifecycle_pending = None;
                self.runtime.lifecycle_failure = None;
                self.diag.traffic = None;
                self.diag.traffic_history.clear();
                self.diag.connections = None;
                self.diag.connection_groups.clear_observation();
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
                self.diag.logs_stream_state = RuntimeStreamState::Idle;
                self.diag.traffic_stream_state = RuntimeStreamState::Idle;
                self.diag.connections_stream_state = RuntimeStreamState::Idle;
                self.runtime.proxy_mode = None;
                self.runtime.proxy_mode_state = Default::default();
                self.runtime.mode_actions.invalidate();
                self.runtime.runtime_control = Default::default();
                self.runtime.script_block_present = false;
                self.runtime.tun_enabled = None;
                self.runtime.system_toggles =
                    self.runtime.system_toggles.clone().with_tun_readback(None);
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

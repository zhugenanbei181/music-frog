//! Kernel management: checking for core updates, downloading mihomo
//! versions with progress streaming and managing installed kernels
//! (list, default selection, delete).

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::{Task, stream};
use infiltrator_contract::error::InfiltratorError;
use infiltrator_shared::locales::Localizer;
use mihomo_version::channel::{Channel, fetch_latest};
use mihomo_version::manager::VersionManager;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

impl AppState {
    /// Kernel download/management. This is the last domain in the
    /// `update_core` chain, so its fallback arm returns `Task::none()` for
    /// every message no core domain owns.
    pub(super) fn update_core_kernels(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::CheckCoreUpdate => {
                self.runtime.is_checking_update = true;
                let channel =
                    Channel::from_str(&self.runtime.core_channel).unwrap_or(Channel::Stable);
                Task::perform(
                    async move {
                        fetch_latest(channel)
                            .await
                            .map_err(infiltrator_contract::error::from_mihomo)
                            .map(|info| info.version)
                    },
                    Message::CoreUpdateInfo,
                )
            }
            Message::CoreUpdateInfo(result) => {
                self.runtime.is_checking_update = false;
                self.refresh_tray();
                match result {
                    Ok(version) => {
                        self.runtime.latest_core_version = Some(version);
                        Task::none()
                    }
                    Err(e) => {
                        self.set_error(&e);
                        Task::none()
                    }
                }
            }
            Message::SetCoreChannel(channel) => {
                if Channel::from_str(&channel).is_ok() {
                    self.runtime.core_channel = channel.to_ascii_lowercase();
                }
                Task::none()
            }
            Message::DownloadCore(version) => {
                if self.runtime.is_downloading_core {
                    return Task::none();
                }
                self.runtime.core_download_token = self.runtime.core_download_token.wrapping_add(1);
                let token = self.runtime.core_download_token;
                let cancel = Arc::new(AtomicBool::new(false));
                self.runtime.core_download_cancel = Some(cancel.clone());
                self.runtime.is_downloading_core = true;
                let stream = stream::channel(
                    100,
                    move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                        let vm = match VersionManager::new() {
                            Ok(v) => v,
                            Err(e) => {
                                let _ = output.try_send(Message::CoreDownloadFinished(
                                    Err(infiltrator_contract::error::from_mihomo(e)),
                                    token,
                                ));
                                return;
                            }
                        };

                        let mut progress_output = output.clone();
                        let mut previous_at = std::time::Instant::now();
                        let mut previous_bytes = 0u64;
                        let cancel_for_install = cancel.clone();
                        match vm
                            .install_with_progress_and_cancel(
                                &version,
                                move |progress| {
                                    let now = std::time::Instant::now();
                                    let elapsed =
                                        now.saturating_duration_since(previous_at).as_secs_f64();
                                    let speed_bytes = if elapsed > 0.0 {
                                        progress.downloaded.saturating_sub(previous_bytes) as f64
                                            / elapsed
                                    } else {
                                        0.0
                                    } as u64;
                                    previous_at = now;
                                    previous_bytes = progress.downloaded;
                                    let _ =
                                        progress_output.try_send(Message::CoreDownloadProgress(
                                            crate::types::app::CoreDownloadProgress {
                                                downloaded: progress.downloaded,
                                                total: progress.total,
                                                speed_bytes,
                                            },
                                            token,
                                        ));
                                },
                                move || cancel_for_install.load(Ordering::Acquire),
                            )
                            .await
                        {
                            Ok(_) => {
                                let _ = output
                                    .try_send(Message::CoreDownloadFinished(Ok(version), token));
                            }
                            Err(e) => {
                                let _ = output.try_send(Message::CoreDownloadFinished(
                                    Err(infiltrator_contract::error::from_mihomo(e)),
                                    token,
                                ));
                            }
                        }
                    },
                );
                Task::run(stream, |m| m)
            }
            Message::CoreDownloadProgress(progress, token) => {
                if token != self.runtime.core_download_token {
                    return Task::none();
                }
                self.runtime.download_progress = progress.total.map_or(0.0, |total| {
                    if total == 0 {
                        0.0
                    } else {
                        (progress.downloaded as f32 / total as f32).clamp(0.0, 1.0)
                    }
                });
                self.runtime.download_stats = Some(progress);
                self.refresh_tray_throttled();
                Task::none()
            }
            Message::CancelCoreDownload => {
                if let Some(cancel) = &self.runtime.core_download_cancel {
                    cancel.store(true, Ordering::Release);
                }
                Task::none()
            }
            Message::CoreDownloadFinished(result, token) => {
                if token != self.runtime.core_download_token {
                    return Task::none();
                }
                self.runtime.download_progress = 0.0;
                self.runtime.download_stats = None;
                self.runtime.core_download_cancel = None;
                self.runtime.is_downloading_core = false;
                self.refresh_tray();
                match result {
                    Ok(_) => Task::done(Message::LoadKernels),
                    Err(e) => {
                        self.set_error(&e);
                        let cancelled = e.to_string().contains("下载已取消");
                        Task::done(Message::ShowToast(
                            e.to_string(),
                            if cancelled {
                                ToastStatus::Warning
                            } else {
                                ToastStatus::Error
                            },
                        ))
                    }
                }
            }
            Message::LoadKernels => Task::perform(
                async {
                    let vm = VersionManager::new().map_err(infiltrator_contract::error::from_mihomo)?;
                    vm.list_installed().await.map_err(infiltrator_contract::error::from_mihomo)
                },
                Message::KernelsLoaded,
            ),
            Message::KernelsLoaded(result) => {
                match result {
                    Ok(versions) => self.runtime.installed_kernels = versions,
                    Err(e) => self.set_error(&e),
                }
                self.refresh_tray();
                Task::none()
            }
            Message::SetDefaultKernel(version) => Task::perform(
                async move {
                    let vm = VersionManager::new().map_err(infiltrator_contract::error::from_mihomo)?;
                    vm.set_default(&version)
                        .await
                        .map_err(infiltrator_contract::error::from_mihomo)
                },
                Message::KernelOperationFinished,
            ),
            Message::DeleteKernel(version) => Task::perform(
                async move {
                    let vm = VersionManager::new().map_err(infiltrator_contract::error::from_mihomo)?;
                    vm.uninstall(&version).await.map_err(infiltrator_contract::error::from_mihomo)
                },
                Message::KernelOperationFinished,
            ),
            Message::KernelOperationFinished(result) => match result {
                Ok(()) => Task::done(Message::LoadKernels),
                Err(error) => {
                    self.set_error(&error);
                    self.refresh_tray();
                    Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                }
            },
            Message::FactoryReset => {
                self.cancel_all_tasks();
                self.runtime.core_download_token = self.runtime.core_download_token.wrapping_add(1);
                if let Some(cancel) = &self.runtime.core_download_cancel {
                    cancel.store(true, Ordering::Release);
                }
                self.runtime.core_download_cancel = None;
                self.runtime.is_downloading_core = false;
                if let Some(cancel) = &self.profile.sync_cancel {
                    cancel.store(true, Ordering::Release);
                }
                self.profile.sync_cancel = None;
                self.profile.is_syncing = false;
                self.profile.sync_progress = None;
                self.runtime.system_proxy_enabled = false;
                self.shell.error_msg = None;
                self.shell.confirmation = None;
                self.shell.is_factory_resetting = true;
                self.shell.admin_server.shutdown();
                let runtime = self.take_app_runtime();
                self.runtime.status = crate::types::runtime::RuntimeStatus::Stopped;
                Task::perform(
                    async move {
                        if let Some(runtime) = runtime {
                            tokio::time::timeout(Duration::from_secs(5), runtime.shutdown())
                                .await
                                .map_err(|_| {
                                    InfiltratorError::Internal(
                                        "停止内核超时，未执行恢复出厂".to_string(),
                                    )
                                })?
                                .map_err(|error| InfiltratorError::Mihomo(error.to_string()))?;
                        }

                        infiltrator_desktop::proxy::apply_system_proxy(None)
                            .map_err(|error| InfiltratorError::Privilege(error.to_string()))?;
                        infiltrator_shared::autostart::set_autostart_enabled(
                            crate::AUTOSTART_REG_NAME,
                            false,
                        )
                        .map_err(|error| InfiltratorError::Internal(error.to_string()))?;

                        let home = mihomo_platform::paths::get_home_dir()
                            .map_err(infiltrator_contract::error::from_mihomo)?;

                        // 必须趁 settings.toml 还在时解析 configs 目录
                        // （settings 的 configs_dir 可指向云同步目录）并枚举
                        // profile 名清 keyring；settings 一旦先删，云目录里的
                        // cache.db / geoip / options / snapshots 就会整体漏删。
                        let manager = infiltrator_core::settings_io::app_config_manager().await.ok();
                        let configs_dir = manager.as_ref().map(|m| m.config_dir().to_path_buf());
                        if let Some(manager) = &manager {
                            match manager.list_profiles().await {
                                Ok(profiles) => {
                                    for profile in profiles {
                                        // 无凭证的 profile 删除会报 NotFound，
                                        // 只有确实存过订阅 URL 的才值得告警。
                                        let stored = profile.subscription_url.is_some();
                                        if let Err(error) = manager
                                            .delete_subscription_credential(&profile.name)
                                            .await
                                            && stored
                                        {
                                            log::warn!(
                                                "factory reset: subscription credential for {} \
                                                 purge failed: {error}",
                                                profile.name
                                            );
                                        }
                                    }
                                }
                                Err(error) => log::warn!(
                                    "factory reset: list profiles for credential purge: {error}"
                                ),
                            }
                        } else {
                            log::warn!(
                                "factory reset: configs dir unresolved, cloud-synced files may \
                                 remain"
                            );
                        }

                        // 纯文件系统清理：settings/config.toml 删除失败整体
                        // 报错；目录/日志失败只记 warning（契约见模块文档）。
                        let report = tokio::task::spawn_blocking(move || {
                            infiltrator_core::factory_reset::execute(&home, configs_dir.as_deref())
                        })
                        .await
                        .map_err(|error| InfiltratorError::Internal(error.to_string()))?
                        .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                        for warning in &report.warnings {
                            log::warn!("factory reset: {warning}");
                        }

                        // settings 已删，configs 回落 `<home>/configs`：重建
                        // default 配置与当前指针，落出厂态。
                        infiltrator_core::profiles::reset_profiles_to_default()
                            .await
                            .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                        Ok(())
                    },
                    Message::FactoryResetFinished,
                )
            }
            Message::FactoryResetFinished(result) => {
                self.shell.is_factory_resetting = false;
                match result {
                    Ok(()) => {
                        let demo = self.shell.demo;
                        let capture_marker = self.shell.capture_marker.clone();
                        // 在状态整体重置前取旧语言：fresh 状态尚未载入偏好。
                        let toast_done = infiltrator_shared::locales::Lang(&self.shell.lang)
                            .tr("toast_factory_reset_done")
                            .into_owned();
                        let tray_controller = self.shell.tray_controller.take();
                        let tray_events = self.shell.tray_events.take();
                        let mut fresh = Self::empty();
                        fresh.shell.tray_controller = tray_controller;
                        fresh.shell.tray_events = tray_events;
                        *self = fresh;
                        self.shell.demo = demo;
                        self.shell.capture_marker = capture_marker;
                        self.refresh_tray();
                        Task::batch(vec![
                            Task::done(Message::LoadProfiles),
                            Task::done(Message::LoadKernels),
                            Task::perform(
                                async {
                                    let home = mihomo_platform::paths::get_home_dir()
                                        .map_err(infiltrator_contract::error::from_mihomo)?;
                                    let path = infiltrator_core::settings_io::settings_path(&home)
                                        .map_err(|error| {
                                            InfiltratorError::Config(error.to_string())
                                        })?;
                                    infiltrator_core::settings_io::load_settings(&path)
                                        .await
                                        .map_err(|error| {
                                            InfiltratorError::Config(error.to_string())
                                        })
                                },
                                Message::SettingsLoaded,
                            ),
                            Task::done(Message::ShowToast(toast_done, ToastStatus::Success)),
                        ])
                    }
                    Err(error) => {
                        self.set_error(&error);
                        let toast = format!(
                            "{}: {error}",
                            infiltrator_shared::locales::Lang(&self.shell.lang)
                                .tr("factory_reset_failed")
                        );
                        Task::done(Message::ShowToast(toast, ToastStatus::Error))
                    }
                }
            }
            _ => Task::none(),
        }
    }
}

//! Kernel management: checking for core updates, downloading mihomo
//! versions with progress streaming and managing installed kernels
//! (list, default selection, delete).

use crate::host::desktop::system_proxy_port;
use crate::host::storage::{factory_reset, home_dir, profile_store, reset_profiles_to_default};
use crate::settings_store::load;
use crate::state::AppState;
use crate::state::runtime::SystemProxyStage;
use crate::state::core_operations::{
    CoreDownloadCancelAdmission, CoreDownloadCompletionAdmission, CoreDownloadInvalidation,
    CoreDownloadProgressAdmission, CoreDownloadRunId, CoreDownloadStart,
    CoreUpdateCheckCompletionAdmission, CoreUpdateCheckInvalidation, CoreUpdateCheckStart,
};
use crate::state::profiles::WebDavSyncPhase;
use crate::types::app::{CoreDownloadProgress, ToastStatus};
use crate::types::message::Message;
use crate::types::runtime::RuntimeStatus;
use crate::version_application::{application, release_channel};
use iced::futures::channel::mpsc;
use iced::{Task, stream};
use infiltrator_application::system_proxy_application::SystemProxyApplication;
use infiltrator_application::version_application::VersionApplication;
use infiltrator_contract::version::{CoreArtifactVerification, VersionDownloadProgress};
use infiltrator_ports::version::VersionProgressSink;
use infiltrator_shared::autostart::set_autostart_enabled;
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_core_contract::error::{ErrorCode, Failure, InfiltratorError, from_backend};
use lurker_ui_presentation::failure_projection::failure_message;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::task::spawn_blocking;
use tokio::time::timeout;

struct IcedVersionProgressSink {
    output: Mutex<mpsc::Sender<Message>>,
    cancel: Arc<AtomicBool>,
    run_id: CoreDownloadRunId,
    previous: Mutex<(Instant, u64)>,
}

impl IcedVersionProgressSink {
    fn new(
        output: mpsc::Sender<Message>,
        cancel: Arc<AtomicBool>,
        run_id: CoreDownloadRunId,
    ) -> Self {
        Self {
            output: Mutex::new(output),
            cancel,
            run_id,
            previous: Mutex::new((Instant::now(), 0)),
        }
    }
}

impl VersionProgressSink for IcedVersionProgressSink {
    fn progress(&self, progress: VersionDownloadProgress) {
        let mut previous = self.previous.lock().expect("version progress lock");
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(previous.0).as_secs_f64();
        let speed_bytes = if elapsed > 0.0 {
            progress.downloaded.saturating_sub(previous.1) as f64 / elapsed
        } else {
            0.0
        } as u64;
        *previous = (now, progress.downloaded);
        if let Ok(mut output) = self.output.lock() {
            let _ = output.try_send(Message::CoreDownloadProgress {
                run_id: self.run_id,
                progress: CoreDownloadProgress {
                    downloaded: progress.downloaded,
                    total: progress.total,
                    speed_bytes,
                },
            });
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Acquire)
    }
}

fn version_application() -> Result<VersionApplication, InfiltratorError> {
    application()
}

impl AppState {
    /// Kernel download/management. This is the last domain in the
    /// `update_core` chain, so its fallback arm returns `Task::none()` for
    /// every message no core domain owns.
    pub(super) fn update_core_kernels(&mut self, message: Message) -> Task<Message> {
        let copy_locale = self.shell.lang.clone();
        match message {
            Message::CheckCoreUpdate => {
                let check_id = match self.runtime.core_update_check.start() {
                    CoreUpdateCheckStart::Started(check_id) => check_id,
                    CoreUpdateCheckStart::AlreadyChecking => return Task::none(),
                };
                let raw_channel = self.runtime.core_channel.clone();
                Task::perform(
                    async move {
                        let application = version_application()?;
                        let channel =
                            release_channel(application.engine_id().clone(), &raw_channel);
                        application
                            .latest(channel)
                            .await
                            .map_err(|failure| InfiltratorError::Download(failure.message))
                            .map(|info| info.version)
                    },
                    move |result| Message::CoreUpdateFinished { check_id, result },
                )
            }
            Message::CoreUpdateFinished { check_id, result } => {
                if self
                    .runtime
                    .core_update_check
                    .complete(check_id, result.clone())
                    == CoreUpdateCheckCompletionAdmission::StaleCheck
                {
                    return Task::none();
                }
                self.refresh_tray();
                match result {
                    Ok(_) => Task::none(),
                    Err(e) => {
                        self.set_error(&e);
                        Task::none()
                    }
                }
            }
            Message::SetCoreChannel(channel) => {
                if matches!(
                    channel.trim().to_ascii_lowercase().as_str(),
                    "stable"
                        | "alpha"
                        | "pre-release"
                        | "prerelease"
                        | "meta"
                        | "meta-core"
                        | "metacore"
                ) {
                    self.runtime.core_channel = channel.to_ascii_lowercase();
                    match self.runtime.core_update_check.invalidate() {
                        CoreUpdateCheckInvalidation::Unchecked
                        | CoreUpdateCheckInvalidation::Invalidated(_) => {}
                    }
                }
                Task::none()
            }
            Message::DownloadCore(version) => {
                let (run_id, cancel) = match self.runtime.core_download.start() {
                    CoreDownloadStart::Started {
                        run_id,
                        cancellation,
                    } => (run_id, cancellation),
                    CoreDownloadStart::AlreadyDownloading => return Task::none(),
                };
                let application = match version_application() {
                    Ok(application) => application,
                    Err(error) => {
                        return Task::done(Message::CoreDownloadFinished {
                            result: Err(Failure::new(
                                ErrorCode::NotReady,
                                error.to_string(),
                                false,
                            )),
                            engine_id: None,
                            run_id,
                        });
                    }
                };
                let engine_id = application.engine_id().clone();
                let stream =
                    stream::channel(100, move |mut output: mpsc::Sender<Message>| async move {
                        let progress =
                            Arc::new(IcedVersionProgressSink::new(output.clone(), cancel, run_id));
                        match application.install(version.clone(), progress).await {
                            Ok(_) => {
                                let _ = output.try_send(Message::CoreDownloadFinished {
                                    result: Ok(version),
                                    engine_id: Some(engine_id),
                                    run_id,
                                });
                            }
                            Err(failure) => {
                                let _ = output.try_send(Message::CoreDownloadFinished {
                                    result: Err(failure),
                                    engine_id: Some(engine_id),
                                    run_id,
                                });
                            }
                        }
                    });
                Task::run(stream, |m| m)
            }
            Message::CoreDownloadProgress { run_id, progress } => {
                if self.runtime.core_download.admit_progress(run_id, progress)
                    == CoreDownloadProgressAdmission::StaleRun
                {
                    return Task::none();
                }
                self.refresh_tray_throttled();
                Task::none()
            }
            Message::CancelCoreDownload => {
                match self.runtime.core_download.request_cancel() {
                    CoreDownloadCancelAdmission::Requested(_)
                    | CoreDownloadCancelAdmission::AlreadyRequested(_)
                    | CoreDownloadCancelAdmission::NotDownloading => {}
                }
                Task::none()
            }
            Message::CoreDownloadFinished {
                result,
                engine_id,
                run_id,
            } => {
                if self.runtime.core_download.complete(run_id)
                    == CoreDownloadCompletionAdmission::StaleRun
                {
                    return Task::none();
                }
                self.refresh_tray();
                match result {
                    Ok(version) => {
                        self.runtime.core_integrity = engine_id
                            .map_or(CoreArtifactVerification::Unknown, |engine_id| {
                                CoreArtifactVerification::Verified { engine_id, version }
                            });
                        Task::done(Message::LoadKernels)
                    }
                    Err(e) => {
                        self.runtime.core_integrity =
                            engine_id.map_or(CoreArtifactVerification::Unknown, |engine_id| {
                                CoreArtifactVerification::Rejected {
                                    engine_id,
                                    version: "unknown".to_owned(),
                                    failure: e.clone(),
                                }
                            });
                        let cancelled = e.code == ErrorCode::Canceled;
                        let message = failure_message(&e, &Lang(&self.shell.lang));
                        if !cancelled {
                            self.set_error(&message);
                        }
                        Task::done(Message::ShowToast(
                            message,
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
                    version_application()?
                        .list_installed()
                        .await
                        .map_err(|failure| InfiltratorError::Download(failure.message))
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
                    version_application()?
                        .activate(&version)
                        .await
                        .map_err(|failure| InfiltratorError::Download(failure.message))
                },
                Message::KernelOperationFinished,
            ),
            Message::RollbackCore => Task::perform(
                async {
                    version_application()?
                        .rollback()
                        .await
                        .map(|_| ())
                        .map_err(|failure| InfiltratorError::Download(failure.message))
                },
                Message::KernelOperationFinished,
            ),
            Message::DeleteKernel(version) => Task::perform(
                async move {
                    version_application()?
                        .uninstall(&version)
                        .await
                        .map_err(|failure| InfiltratorError::Download(failure.message))
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
                match self.runtime.core_download.invalidate() {
                    CoreDownloadInvalidation::Idle | CoreDownloadInvalidation::Cancelled(_) => {}
                }
                match self.runtime.core_update_check.invalidate() {
                    CoreUpdateCheckInvalidation::Unchecked
                    | CoreUpdateCheckInvalidation::Invalidated(_) => {}
                }
                if let Some(cancel) = &self.profile.sync_cancel {
                    cancel.store(true, Ordering::Release);
                }
                self.profile.sync_cancel = None;
                self.profile.webdav_sync_phase = WebDavSyncPhase::Idle;
                self.profile.sync_progress = None;
                self.runtime.system_proxy_stage = SystemProxyStage::Disabled;
                self.runtime.system_toggles = self
                    .runtime
                    .system_toggles
                    .clone()
                    .with_system_proxy_readback(false);
                self.shell.error_msg = None;
                self.shell.confirmation = None;
                self.shell.is_factory_resetting = true;
                self.shell.admin_server.shutdown();
                let runtime = self.take_app_runtime();
                let system_proxy_application = self
                    .runtime
                    .system_proxy_application
                    .clone()
                    .unwrap_or_else(|| SystemProxyApplication::new(system_proxy_port()));
                self.runtime.status = RuntimeStatus::Stopped;
                Task::perform(
                    async move {
                        if let Some(runtime) = runtime {
                            timeout(Duration::from_secs(5), runtime.lifecycle_port().stop())
                                .await
                                .map_err(|_| {
                                    InfiltratorError::Internal(
                                        Lang(&copy_locale)
                                            .tr("core_reset_stop_timeout")
                                            .into_owned(),
                                    )
                                })?
                                .map_err(|error| InfiltratorError::Backend(error.to_string()))?;
                        }

                        system_proxy_application
                            .set_enabled(false, None, None)
                            .await
                            .map_err(|failure| InfiltratorError::Privilege(failure.message))?;
                        set_autostart_enabled(crate::AUTOSTART_REG_NAME, false)
                            .map_err(|error| InfiltratorError::Internal(error.to_string()))?;

                        let home = home_dir().map_err(from_backend)?;

                        // 必须趁 settings.toml 还在时解析 configs 目录
                        // （settings 的 configs_dir 可指向云同步目录）并枚举
                        // profile 名清 keyring；settings 一旦先删，云目录里的
                        // cache.db / geoip / options / snapshots 就会整体漏删。
                        let manager = profile_store().await.ok();
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
                        let warnings =
                            spawn_blocking(move || factory_reset(&home, configs_dir.as_deref()))
                                .await
                                .map_err(|error| InfiltratorError::Internal(error.to_string()))?
                                .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                        for warning in &warnings {
                            log::warn!("factory reset: {warning}");
                        }

                        // settings 已删，configs 回落 `<home>/configs`：重建
                        // default 配置与当前指针，落出厂态。
                        reset_profiles_to_default()
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
                        let toast_done = Lang(&self.shell.lang)
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
                            Task::perform(load(), Message::SettingsLoaded),
                            Task::done(Message::ShowToast(toast_done, ToastStatus::Success)),
                        ])
                    }
                    Err(error) => {
                        self.set_error(&error);
                        let toast = format!(
                            "{}: {error}",
                            Lang(&self.shell.lang).tr("factory_reset_failed")
                        );
                        Task::done(Message::ShowToast(toast, ToastStatus::Error))
                    }
                }
            }
            _ => Task::none(),
        }
    }
}

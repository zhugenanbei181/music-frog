//! Kernel management: checking for core updates, downloading mihomo
//! versions with progress streaming and managing installed kernels
//! (list, default selection, delete).

use crate::state::AppState;
use crate::types::{InfiltratorError, Message, ToastStatus};
use iced::{Task, stream};
use mihomo_version::channel::Channel;
use mihomo_version::manager::VersionManager;

impl AppState {
    /// Kernel download/management. This is the last domain in the
    /// `update_core` chain, so its fallback arm returns `Task::none()` for
    /// every message no core domain owns.
    pub(super) fn update_core_kernels(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::CheckCoreUpdate => {
                self.runtime.is_checking_update = true;
                Task::perform(
                    async {
                        let vm = VersionManager::new().map_err(InfiltratorError::from)?;
                        vm.install_channel(Channel::Stable)
                            .await
                            .map_err(InfiltratorError::from)
                    },
                    Message::CoreUpdateInfo,
                )
            }
            Message::CoreUpdateInfo(result) => {
                self.runtime.is_checking_update = false;
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
            Message::DownloadCore(version) => {
                let stream = stream::channel(
                    100,
                    move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                        let vm = match VersionManager::new() {
                            Ok(v) => v,
                            Err(e) => {
                                let _ = output.try_send(Message::CoreDownloadFinished(Err(
                                    InfiltratorError::from(e),
                                )));
                                return;
                            }
                        };

                        match vm.install(&version).await {
                            Ok(_) => {
                                let _ = output.try_send(Message::CoreDownloadFinished(Ok(version)));
                            }
                            Err(e) => {
                                let _ = output.try_send(Message::CoreDownloadFinished(Err(
                                    InfiltratorError::from(e),
                                )));
                            }
                        }
                    },
                );
                Task::run(stream, |m| m)
            }
            Message::CoreDownloadProgress(progress) => {
                self.runtime.download_progress = progress;
                Task::none()
            }
            Message::CoreDownloadFinished(result) => {
                self.runtime.download_progress = 0.0;
                match result {
                    Ok(_) => Task::done(Message::LoadKernels),
                    Err(e) => {
                        self.set_error(&e);
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::LoadKernels => Task::perform(
                async {
                    let vm = VersionManager::new().map_err(InfiltratorError::from)?;
                    vm.list_installed().await.map_err(InfiltratorError::from)
                },
                Message::KernelsLoaded,
            ),
            Message::KernelsLoaded(result) => {
                match result {
                    Ok(versions) => self.runtime.installed_kernels = versions,
                    Err(e) => self.set_error(&e),
                }
                Task::none()
            }
            Message::SetDefaultKernel(version) => Task::perform(
                async move {
                    let vm = VersionManager::new().map_err(InfiltratorError::from)?;
                    vm.set_default(&version)
                        .await
                        .map_err(InfiltratorError::from)
                },
                |_| Message::LoadKernels,
            ),
            Message::DeleteKernel(version) => Task::perform(
                async move {
                    let vm = VersionManager::new().map_err(InfiltratorError::from)?;
                    vm.uninstall(&version).await.map_err(InfiltratorError::from)
                },
                |_| Message::LoadKernels,
            ),
            Message::FactoryReset => Task::none(),
            _ => Task::none(),
        }
    }
}

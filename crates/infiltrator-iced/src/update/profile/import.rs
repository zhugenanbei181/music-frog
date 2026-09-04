//! Profile import handlers: subscription-URL import and local YAML file
//! import (file picker, metadata fields, activation).

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_ports::runtime_gateway::ManagedRuntime;

impl AppState {
    pub(super) fn update_import(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::UpdateImportUrl(url) => {
                self.profile.import_url = url;
                Task::none()
            }
            Message::UpdateImportName(name) => {
                self.profile.import_name = name;
                Task::none()
            }
            Message::UpdateImportActivate(enabled) => {
                self.profile.import_activate = enabled;
                Task::none()
            }
            Message::ImportProfile => {
                let url = self.profile.import_url.trim().to_string();
                let name = self.profile.import_name.trim().to_string();
                let activate = self.profile.import_activate;
                let runtime = self.runtime.runtime.clone();
                if name.is_empty() || url.is_empty() {
                    return Task::done(Message::ShowToast(
                        "Name and subscription URL are required".to_string(),
                        ToastStatus::Error,
                    ));
                }

                self.profile.is_importing = true;
                Task::perform(
                    async move {
                        let profile_name = infiltrator_domain::profiles::sanitize_profile_name(&name)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let cm = crate::configs_dir::config_manager().await?;
                        let current = cm.get_current().await.map_err(infiltrator_contract::error::from_mihomo)?;
                        if runtime.is_some() && current == profile_name {
                            return Err(InfiltratorError::Config(
                                "内核运行时不能直接覆盖当前配置，请先停止内核后再导入".to_string(),
                            ));
                        }
                        infiltrator_core::profiles::create_profile_from_url(&profile_name, &url)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;

                        let reloaded = if activate {
                            crate::update::core::profile_apply::activate_profile(
                                runtime,
                                &profile_name,
                            )
                            .await?
                        } else {
                            false
                        };
                        Ok(reloaded)
                    },
                    Message::ProfileImported,
                )
            }
            Message::ProfileImported(result) => {
                self.profile.is_importing = false;
                match result {
                    Ok(reloaded) => {
                        self.invalidate_rules_dns_views();
                        let activate = self.profile.import_activate;
                        self.profile.import_url.clear();
                        self.profile.import_name.clear();
                        self.profile.import_activate = false;
                        let mut tasks = vec![
                            Task::done(Message::LoadProfiles),
                            Task::done(Message::ShowToast(
                                "Profile imported successfully".to_string(),
                                ToastStatus::Success,
                            )),
                        ];
                        if activate && !reloaded {
                            tasks.push(Task::done(Message::StartProxy));
                        } else if reloaded && let Some(runtime) = self.runtime.runtime.clone() {
                            self.sync_runtime_slot(Some(runtime));
                        }
                        Task::batch(tasks)
                    }
                    Err(e) => {
                        self.set_error(&e);
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::BrowseLocalImportFile => Task::perform(
                async {
                    tokio::task::spawn_blocking(|| {
                        rfd::FileDialog::new()
                            .add_filter("Configs & Subscriptions", &["yaml", "yml", "json", "txt"])
                            .pick_file()
                    })
                    .await
                    .ok()
                    .flatten()
                },
                Message::LocalImportFilePicked,
            ),
            Message::LocalImportFilePicked(path) => {
                if let Some(path) = path {
                    self.profile.local_import_path = path.to_string_lossy().to_string();
                    if self.profile.local_import_name.trim().is_empty()
                        && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                    {
                        self.profile.local_import_name = stem.to_string();
                    }
                }
                Task::none()
            }
            Message::UpdateLocalImportPath(path) => {
                self.profile.local_import_path = path;
                if self.profile.local_import_name.trim().is_empty()
                    && let Some(stem) = std::path::Path::new(&self.profile.local_import_path)
                        .file_stem()
                        .and_then(|s| s.to_str())
                {
                    self.profile.local_import_name = stem.to_string();
                }
                Task::none()
            }
            Message::UpdateLocalImportName(name) => {
                self.profile.local_import_name = name;
                Task::none()
            }
            Message::UpdateLocalImportActivate(enabled) => {
                self.profile.local_import_activate = enabled;
                Task::none()
            }
            Message::ImportLocalProfile => {
                let path = self.profile.local_import_path.trim().to_string();
                let name = self.profile.local_import_name.trim().to_string();
                let activate = self.profile.local_import_activate;
                let runtime = self.runtime.runtime.clone();
                if path.is_empty() || name.is_empty() {
                    return Task::done(Message::ShowToast(
                        "Local path and profile name are required".to_string(),
                        ToastStatus::Error,
                    ));
                }

                self.profile.is_importing_local = true;
                Task::perform(
                    async move {
                        let profile_name = infiltrator_domain::profiles::sanitize_profile_name(&name)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let content = tokio::fs::read_to_string(&path)
                            .await
                            .map_err(|e| InfiltratorError::Io(e.to_string()))?;
                        let content = infiltrator_domain::profile_converter::ProfileConverter::detect_and_convert(&content)
                            .unwrap_or(content);
                        infiltrator_domain::config::validate_yaml(&content)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;

                        let cm = crate::configs_dir::config_manager().await?;
                        let current = cm.get_current().await.map_err(infiltrator_contract::error::from_mihomo)?;
                        let reloaded = match (runtime, current == profile_name) {
                            (Some(runtime), true) => {
                                ManagedRuntime::apply_profile_content(
                                    runtime.as_ref(),
                                    &content,
                                    ApplyStrategy::AlwaysRestart,
                                )
                                    .await
                                    .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                                true
                            }
                            (runtime, false) => {
                                cm.save(&profile_name, &content)
                                    .await
                                    .map_err(infiltrator_contract::error::from_mihomo)?;
                                if activate {
                                    crate::update::core::profile_apply::activate_profile(
                                        runtime,
                                        &profile_name,
                                    )
                                    .await?
                                } else {
                                    false
                                }
                            }
                            (None, true) => {
                                cm.save(&profile_name, &content)
                                    .await
                                    .map_err(infiltrator_contract::error::from_mihomo)?;
                                false
                            }
                        };
                        Ok(reloaded)
                    },
                    Message::LocalProfileImported,
                )
            }
            Message::LocalProfileImported(result) => {
                self.profile.is_importing_local = false;
                match result {
                    Ok(reloaded) => {
                        self.invalidate_rules_dns_views();
                        let activate = self.profile.local_import_activate;
                        self.profile.local_import_path.clear();
                        self.profile.local_import_name.clear();
                        self.profile.local_import_activate = false;
                        let mut tasks = vec![
                            Task::done(Message::LoadProfiles),
                            Task::done(Message::ShowToast(
                                "Local profile imported".to_string(),
                                ToastStatus::Success,
                            )),
                        ];
                        if activate && !reloaded {
                            tasks.push(Task::done(Message::StartProxy));
                        } else if reloaded && let Some(runtime) = self.runtime.runtime.clone() {
                            self.sync_runtime_slot(Some(runtime));
                        }
                        Task::batch(tasks)
                    }
                    Err(e) => {
                        self.set_error(&e);
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            _ => Task::none(),
        }
    }
}

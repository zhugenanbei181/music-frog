//! App-settings handlers: WebDAV account fields, editor path preference and
//! the load-modify-save of the settings file.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;
use infiltrator_core::error::InfiltratorError;
use infiltrator_core::settings::{AppSettings, WebDavConfig};
use std::str::FromStr;

impl AppState {
    pub(super) fn update_settings(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::UpdateWebDavUrl(url) => {
                self.profile.webdav_url = url;
                Task::none()
            }
            Message::UpdateWebDavUser(user) => {
                self.profile.webdav_user = user;
                Task::none()
            }
            Message::UpdateWebDavPass(pass) => {
                self.profile.webdav_pass = pass;
                Task::none()
            }
            Message::UpdateWebDavEnabled(enabled) => {
                self.profile.webdav_enabled = enabled;
                Task::none()
            }
            Message::UpdateWebDavSyncInterval(v) => {
                self.profile.webdav_sync_interval_mins = v;
                Task::none()
            }
            Message::UpdateWebDavSyncOnStartup(enabled) => {
                self.profile.webdav_sync_on_startup = enabled;
                Task::none()
            }
            Message::UpdateEditorPathSetting(path) => {
                self.editor.editor_path_setting = path;
                Task::none()
            }
            Message::UpdateNotificationsEnabled(enabled) => {
                self.shell.notifications_enabled = enabled;
                Task::none()
            }
            Message::SetLanguage(language) => {
                self.shell.lang = match language.as_str() {
                    "en-US" | "en" => "en-US".to_string(),
                    _ => "zh-CN".to_string(),
                };
                Task::none()
            }
            Message::SaveAppSettings => {
                let interval = if !self.profile.webdav_enabled
                    && self.profile.webdav_sync_interval_mins.trim().is_empty()
                {
                    60
                } else {
                    match self.profile.webdav_sync_interval_mins.trim().parse::<u32>() {
                        Ok(v) if v > 0 => v,
                        _ => {
                            return Task::done(Message::ShowToast(
                                "WebDAV sync interval must be a positive integer".to_string(),
                                ToastStatus::Error,
                            ));
                        }
                    }
                };

                self.profile.is_saving_app_settings = true;
                let language = self.shell.lang.clone();
                let core_channel = self.profile_core_channel();
                let theme = crate::view::theme::theme_to_name(&self.shell.theme).to_string();
                let editor_path = if self.editor.editor_path_setting.trim().is_empty() {
                    None
                } else {
                    Some(self.editor.editor_path_setting.trim().to_string())
                };
                let webdav_password = self.profile.webdav_pass.clone();
                let webdav = WebDavConfig {
                    enabled: self.profile.webdav_enabled,
                    url: self.profile.webdav_url.clone(),
                    username: self.profile.webdav_user.clone(),
                    // 密码不落盘：真实值只进 OS keyring（见下方任务体），
                    // settings.toml 的序列化永远跳过该字段。
                    password: String::new(),
                    sync_interval_mins: interval,
                    sync_on_startup: self.profile.webdav_sync_on_startup,
                };
                let notifications_enabled = self.shell.notifications_enabled;
                let close_to_tray = self.shell.close_to_tray;
                let system_proxy_bypass = if self.shell.system_proxy_bypass.trim().is_empty() {
                    None
                } else {
                    Some(self.shell.system_proxy_bypass.trim().to_string())
                };

                Task::perform(
                    async move {
                        // 密码只进 OS keyring：空串=清除条目，非空=写入；
                        // 失败则整体不落盘，保持「settings 文件 + keyring」
                        // 状态一致（避免其他字段更新而凭据悄悄丢失）。
                        let store = mihomo_platform::defaults::DefaultCredentialStore::default();
                        if webdav_password.is_empty() {
                            infiltrator_core::settings::clear_webdav_password(&store).await;
                        } else {
                            infiltrator_core::settings::save_webdav_password(
                                &store,
                                &webdav_password,
                            )
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        }
                        let base_dir = mihomo_platform::paths::get_home_dir()
                            .map_err(InfiltratorError::from)?;
                        let settings_path = infiltrator_core::settings::settings_path(&base_dir)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let mut settings =
                            infiltrator_core::settings::load_settings(&settings_path)
                                .await
                                .unwrap_or_else(|_| AppSettings::default());
                        settings.language = language;
                        settings.core_channel = core_channel;
                        settings.theme = theme;
                        settings.editor_path = editor_path;
                        settings.webdav = webdav;
                        settings.notifications_enabled = notifications_enabled;
                        settings.close_to_tray = close_to_tray;
                        settings.system_proxy_bypass = system_proxy_bypass;
                        infiltrator_core::settings::save_settings(&settings_path, &settings)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        Ok(())
                    },
                    Message::AppSettingsSaved,
                )
            }
            Message::AppSettingsSaved(result) => {
                self.profile.is_saving_app_settings = false;
                match result {
                    Ok(_) => {
                        self.editor.editor_path =
                            if self.editor.editor_path_setting.trim().is_empty() {
                                None
                            } else {
                                Some(std::path::PathBuf::from(
                                    self.editor.editor_path_setting.trim(),
                                ))
                            };
                        Task::done(Message::ShowToast(
                            "App settings saved".to_string(),
                            ToastStatus::Success,
                        ))
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

    fn profile_core_channel(&self) -> String {
        mihomo_version::channel::Channel::from_str(&self.runtime.core_channel)
            .map(|channel| channel.as_str().to_string())
            .unwrap_or_else(|_| "stable".to_string())
    }
}

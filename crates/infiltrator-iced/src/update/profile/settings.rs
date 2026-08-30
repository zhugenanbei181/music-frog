//! App-settings handlers: WebDAV account fields, editor path preference and
//! the load-modify-save of the settings file.

use crate::state::AppState;
use crate::types::{InfiltratorError, Message, ToastStatus};
use iced::Task;
use infiltrator_core::settings::{AppSettings, WebDavConfig};

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
                let theme = if self.shell.theme == iced::Theme::Light {
                    "light".to_string()
                } else {
                    "dark".to_string()
                };
                let editor_path = if self.editor.editor_path_setting.trim().is_empty() {
                    None
                } else {
                    Some(self.editor.editor_path_setting.trim().to_string())
                };
                let webdav = WebDavConfig {
                    enabled: self.profile.webdav_enabled,
                    url: self.profile.webdav_url.clone(),
                    username: self.profile.webdav_user.clone(),
                    password: self.profile.webdav_pass.clone(),
                    sync_interval_mins: interval,
                    sync_on_startup: self.profile.webdav_sync_on_startup,
                };

                Task::perform(
                    async move {
                        let base_dir = mihomo_platform::paths::get_home_dir()
                            .map_err(InfiltratorError::from)?;
                        let settings_path = infiltrator_core::settings::settings_path(&base_dir)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let mut settings =
                            infiltrator_core::settings::load_settings(&settings_path)
                                .await
                                .unwrap_or_else(|_| AppSettings::default());
                        settings.language = language;
                        settings.theme = theme;
                        settings.editor_path = editor_path;
                        settings.webdav = webdav;
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
}

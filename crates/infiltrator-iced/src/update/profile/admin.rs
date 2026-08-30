//! Web-admin handlers: admin toggle/port input, persistence of the admin
//! settings slice, server lifecycle results and opening the web admin.

use crate::state::AppState;
use crate::types::{InfiltratorError, Message, ToastStatus};
use iced::Task;
use infiltrator_core::settings::AppSettings;
use infiltrator_shared::locales::Localizer;

impl AppState {
    /// Parse the admin port input; `None` when it is not a usable TCP port.
    pub fn parse_admin_port(input: &str) -> Option<u16> {
        input.trim().parse::<u16>().ok().filter(|port| *port > 0)
    }

    /// Load-modify-save only the admin slice of the settings file, mirroring
    /// how the runtime-panel settings are persisted from this frontend.
    fn persist_admin_settings_task(&self) -> Task<Message> {
        let admin = infiltrator_core::settings::AdminServerConfig {
            enabled: self.admin_enabled,
            port: self.admin_port,
        };
        Task::perform(
            async move {
                let base_dir =
                    mihomo_platform::paths::get_home_dir().map_err(InfiltratorError::from)?;
                let settings_path = infiltrator_core::settings::settings_path(&base_dir)
                    .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                let mut settings =
                    infiltrator_core::settings::load_settings(&settings_path)
                        .await
                        .unwrap_or_else(|_| AppSettings::default());
                settings.admin = admin;
                infiltrator_core::settings::save_settings(&settings_path, &settings)
                    .await
                    .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                Ok(())
            },
            Message::AdminSettingsSaved,
        )
    }

    pub(super) fn update_admin(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SetAdminEnabled(enabled) => {
                self.admin_enabled = enabled;
                self.refresh_tray();
                Task::batch(vec![
                    self.persist_admin_settings_task(),
                    self.apply_admin_server_lifecycle(),
                ])
            }
            Message::UpdateAdminPort(input) => {
                self.admin_port_input = input;
                Task::none()
            }
            Message::ApplyAdminSettings => {
                match Self::parse_admin_port(&self.admin_port_input) {
                    Some(port) => {
                        self.admin_port = port;
                        self.admin_port_input = port.to_string();
                        self.refresh_tray();
                        Task::batch(vec![
                            self.persist_admin_settings_task(),
                            self.apply_admin_server_lifecycle(),
                        ])
                    }
                    None => {
                        let lang = crate::locales::Lang(&self.lang);
                        Task::done(Message::ShowToast(
                            lang.tr("settings_admin_invalid_port").into_owned(),
                            ToastStatus::Error,
                        ))
                    }
                }
            }
            Message::AdminSettingsSaved(result) => match result {
                Ok(_) => Task::none(),
                Err(e) => {
                    self.set_error(&e);
                    Task::done(Message::ShowToast(
                        format!("Web 管理端设置保存失败: {e}"),
                        ToastStatus::Error,
                    ))
                }
            },
            Message::AdminServerStarted(result) => {
                let lang = crate::locales::Lang(&self.lang);
                self.refresh_tray();
                match result {
                    Ok(url) => Task::done(Message::ShowToast(
                        format!("{}: {url}", lang.tr("settings_admin_started")),
                        ToastStatus::Success,
                    )),
                    Err(e) => Task::done(Message::ShowToast(
                        format!("{}: {e}", lang.tr("settings_admin_start_failed")),
                        ToastStatus::Error,
                    )),
                }
            }
            Message::OpenWebAdmin => self.open_web_admin(),
            _ => Task::none(),
        }
    }
}

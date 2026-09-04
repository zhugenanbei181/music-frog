//! Web-admin handlers: admin toggle/port input, persistence of the admin
//! settings slice, server lifecycle results and opening the web admin.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;
use infiltrator_domain::settings::AdminServerConfig;
use infiltrator_shared::locales::Localizer;

impl AppState {
    /// Parse the admin port input; `None` when it is not a usable TCP port.
    pub fn parse_admin_port(input: &str) -> Option<u16> {
        input.trim().parse::<u16>().ok().filter(|port| *port > 0)
    }

    /// Load-modify-save only the admin slice of the settings file, mirroring
    /// how the runtime-panel settings are persisted from this frontend.
    fn persist_admin_settings_task(&self) -> Task<Message> {
        let admin = AdminServerConfig {
            enabled: self.shell.admin_enabled,
            port: self.shell.admin_port,
        };
        Task::perform(
            async move {
                crate::settings_store::update(|settings| settings.admin = admin).await
            },
            Message::AdminSettingsSaved,
        )
    }

    pub(super) fn update_admin(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SetAdminEnabled(enabled) => {
                self.shell.admin_enabled = enabled;
                self.refresh_tray();
                Task::batch(vec![
                    self.persist_admin_settings_task(),
                    self.apply_admin_server_lifecycle(),
                ])
            }
            Message::UpdateAdminPort(input) => {
                self.shell.admin_port_input = input;
                Task::none()
            }
            Message::ApplyAdminSettings => {
                match Self::parse_admin_port(&self.shell.admin_port_input) {
                    Some(port) => {
                        self.shell.admin_port = port;
                        self.shell.admin_port_input = port.to_string();
                        self.refresh_tray();
                        Task::batch(vec![
                            self.persist_admin_settings_task(),
                            self.apply_admin_server_lifecycle(),
                        ])
                    }
                    None => {
                        let lang = infiltrator_shared::locales::Lang(&self.shell.lang);
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
                let lang = infiltrator_shared::locales::Lang(&self.shell.lang);
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
            _ => Task::none(),
        }
    }
}

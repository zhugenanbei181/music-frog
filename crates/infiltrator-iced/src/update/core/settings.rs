//! Application settings: mirroring persisted [`AppSettings`] snapshots onto
//! the UI state and routing admin WebUI host commands.

use super::proxies::{
    DEFAULT_RUNTIME_DELAY_TEST_URL, MAX_RUNTIME_DELAY_TIMEOUT_MS, MIN_RUNTIME_DELAY_TIMEOUT_MS,
};
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_core::settings::AppSettings;
use std::str::FromStr;

impl AppState {
    /// Mirror a loaded [`AppSettings`] snapshot onto the UI state fields.
    /// Returns whether the WebDAV sync-on-startup flow should trigger; the
    /// startup path (`SettingsLoaded`) honors it, the WebUI save path
    /// (`ExternalSettingsLoaded`) deliberately does not.
    fn apply_loaded_settings(&mut self, settings: AppSettings) -> bool {
        if !settings.language.trim().is_empty() {
            self.shell.lang = settings.language;
        }
        if !settings.theme.trim().is_empty() {
            self.shell.theme = crate::view::theme::theme_from_name(&settings.theme);
        }
        self.editor.editor_path = settings.editor_path.clone().map(std::path::PathBuf::from);
        self.editor.editor_path_setting = settings.editor_path.unwrap_or_default();
        self.profile.webdav_enabled = settings.webdav.enabled;
        self.profile.webdav_url = settings.webdav.url;
        self.profile.webdav_user = settings.webdav.username;
        self.profile.webdav_pass = settings.webdav.password;
        self.profile.webdav_sync_interval_mins = settings.webdav.sync_interval_mins.to_string();
        self.profile.webdav_sync_on_startup = settings.webdav.sync_on_startup;
        self.runtime.runtime_auto_refresh = settings.runtime_panel.auto_refresh;
        self.runtime.core_channel =
            mihomo_version::channel::Channel::from_str(&settings.core_channel)
                .map(|channel| channel.as_str().to_string())
                .unwrap_or_else(|_| "stable".to_string());
        self.runtime.proxy_delay_sort =
            Self::normalize_delay_sort_key(&settings.runtime_panel.delay_sort).to_string();
        self.runtime.proxy_sort_by_delay = self.runtime.proxy_delay_sort.starts_with("delay_");
        self.runtime.runtime_delay_test_url =
            if settings.runtime_panel.delay_test_url.trim().is_empty() {
                DEFAULT_RUNTIME_DELAY_TEST_URL.to_string()
            } else {
                settings.runtime_panel.delay_test_url
            };
        let timeout = settings
            .runtime_panel
            .delay_timeout_ms
            .clamp(MIN_RUNTIME_DELAY_TIMEOUT_MS, MAX_RUNTIME_DELAY_TIMEOUT_MS);
        self.runtime.runtime_delay_timeout_ms = timeout.to_string();
        self.runtime.runtime_connection_filter = settings.runtime_panel.connection_filter;
        self.runtime.runtime_connection_sort =
            Self::normalize_connection_sort_key(&settings.runtime_panel.connection_sort)
                .to_string();
        self.shell.admin_enabled = settings.admin.enabled;
        self.shell.admin_port = settings.admin.port;
        self.shell.admin_port_input = settings.admin.port.to_string();
        // 0.20 OS 系统通知开关镜像（notify.rs 的 system_notify 读它短路）。
        self.shell.notifications_enabled = settings.notifications_enabled;
        self.shell.close_to_tray = settings.close_to_tray;
        self.shell.system_proxy_bypass = settings.system_proxy_bypass.unwrap_or_default();
        self.profile.webdav_enabled
            && self.profile.webdav_sync_on_startup
            && !self.profile.webdav_url.trim().is_empty()
            && !self.profile.webdav_user.trim().is_empty()
    }

    /// Settings application and admin host command routing. Unmatched
    /// messages fall through to the next domain in the `update_core` chain.
    pub(super) fn update_core_settings(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SettingsLoaded(result) => match result {
                Ok(settings) => {
                    let sync_on_startup = self.apply_loaded_settings(settings);
                    let mut tasks = vec![self.apply_admin_server_lifecycle()];
                    if sync_on_startup {
                        tasks.push(Task::done(Message::SyncDownload));
                    }
                    self.refresh_tray();
                    Task::batch(tasks)
                }
                Err(e) => {
                    self.set_error(&e);
                    Task::none()
                }
            },
            Message::ExternalSettingsLoaded(result) => {
                // The admin WebUI saved settings; re-apply without the
                // WebDAV sync-on-startup side effect.
                match result {
                    Ok(settings) => {
                        self.apply_loaded_settings(settings);
                        self.apply_admin_server_lifecycle()
                    }
                    Err(e) => {
                        self.set_error(&e);
                        Task::none()
                    }
                }
            }
            Message::AdminHostCommand(command) => self.handle_admin_host_command(command),
            other => self.update_core_monitoring(other),
        }
    }
}

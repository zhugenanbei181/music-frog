//! Application settings: mirroring persisted [`AppSettings`] snapshots onto
//! the UI state and routing admin WebUI host commands.

use super::proxies::{
    DEFAULT_RUNTIME_DELAY_TEST_URL, MAX_RUNTIME_DELAY_TIMEOUT_MS, MIN_RUNTIME_DELAY_TIMEOUT_MS,
};
use crate::state::AppState;
use crate::types::Message;
use infiltrator_core::settings::AppSettings;
use iced::Task;

impl AppState {
    /// Mirror a loaded [`AppSettings`] snapshot onto the UI state fields.
    /// Returns whether the WebDAV sync-on-startup flow should trigger; the
    /// startup path (`SettingsLoaded`) honors it, the WebUI save path
    /// (`ExternalSettingsLoaded`) deliberately does not.
    fn apply_loaded_settings(&mut self, settings: AppSettings) -> bool {
        if !settings.language.trim().is_empty() {
            self.lang = settings.language;
        }
        let theme = settings.theme.trim().to_ascii_lowercase();
        if theme == "light" {
            self.theme = iced::Theme::Light;
        } else if theme == "dark" {
            self.theme = iced::Theme::Dark;
        }
        self.editor_path = settings.editor_path.clone().map(std::path::PathBuf::from);
        self.editor_path_setting = settings.editor_path.unwrap_or_default();
        self.webdav_enabled = settings.webdav.enabled;
        self.webdav_url = settings.webdav.url;
        self.webdav_user = settings.webdav.username;
        self.webdav_pass = settings.webdav.password;
        self.webdav_sync_interval_mins = settings.webdav.sync_interval_mins.to_string();
        self.webdav_sync_on_startup = settings.webdav.sync_on_startup;
        self.runtime_auto_refresh = settings.runtime_panel.auto_refresh;
        self.proxy_delay_sort =
            Self::normalize_delay_sort_key(&settings.runtime_panel.delay_sort).to_string();
        self.proxy_sort_by_delay = self.proxy_delay_sort.starts_with("delay_");
        self.runtime_delay_test_url = if settings.runtime_panel.delay_test_url.trim().is_empty() {
            DEFAULT_RUNTIME_DELAY_TEST_URL.to_string()
        } else {
            settings.runtime_panel.delay_test_url
        };
        let timeout = settings
            .runtime_panel
            .delay_timeout_ms
            .max(MIN_RUNTIME_DELAY_TIMEOUT_MS)
            .min(MAX_RUNTIME_DELAY_TIMEOUT_MS);
        self.runtime_delay_timeout_ms = timeout.to_string();
        self.runtime_connection_filter = settings.runtime_panel.connection_filter;
        self.runtime_connection_sort =
            Self::normalize_connection_sort_key(&settings.runtime_panel.connection_sort).to_string();
        self.admin_enabled = settings.admin.enabled;
        self.admin_port = settings.admin.port;
        self.admin_port_input = settings.admin.port.to_string();
        self.webdav_enabled
            && self.webdav_sync_on_startup
            && !self.webdav_url.trim().is_empty()
            && !self.webdav_user.trim().is_empty()
    }

    /// Settings application and admin host command routing. Unmatched
    /// messages fall through to the next domain in the `update_core` chain.
    pub(super) fn update_core_settings(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SettingsLoaded(result) => {
                match result {
                    Ok(settings) => {
                        let sync_on_startup = self.apply_loaded_settings(settings);
                        let mut tasks = vec![self.apply_admin_server_lifecycle()];
                        if sync_on_startup {
                            tasks.push(Task::done(Message::SyncDownload));
                        }
                        Task::batch(tasks)
                    }
                    Err(e) => {
                        self.set_error(&e);
                        Task::none()
                    }
                }
            }
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

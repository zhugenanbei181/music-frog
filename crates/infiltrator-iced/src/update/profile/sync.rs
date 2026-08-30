//! WebDAV sync handlers: upload/download of profiles to the user's DAV
//! account, completion toasts and the periodic sync tick.

use crate::state::AppState;
use crate::types::{InfiltratorError, Message, ToastStatus};
use dav_client::{DavClient, client::WebDavClient};
use iced::Task;
use mihomo_config::manager::ConfigManager;

impl AppState {
    pub(super) fn update_sync(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SyncUpload => {
                let url = self.webdav_url.clone();
                let user = self.webdav_user.clone();
                let pass = self.webdav_pass.clone();
                self.is_syncing = true;
                Task::perform(
                    async move {
                        let client = WebDavClient::new(&url, &user, &pass)
                            .map_err(|e| InfiltratorError::Sync(e.to_string()))?;
                        let cm = ConfigManager::new()
                            .map_err(|e: mihomo_api::error::MihomoError| InfiltratorError::from(e))?;
                        let profiles = cm
                            .list_profiles()
                            .await
                            .map_err(|e: mihomo_api::error::MihomoError| InfiltratorError::from(e))?;

                        for profile in profiles {
                            let content = tokio::fs::read_to_string(&profile.path)
                                .await
                                .map_err(|e| InfiltratorError::Io(e.to_string()))?;
                            client
                                .put(&format!("{}.yaml", profile.name), content.as_bytes(), None)
                                .await
                                .map_err(|e| InfiltratorError::Sync(e.to_string()))?;
                        }
                        Ok(())
                    },
                    Message::SyncFinished,
                )
            }
            Message::SyncDownload => {
                let url = self.webdav_url.clone();
                let user = self.webdav_user.clone();
                let pass = self.webdav_pass.clone();
                self.is_syncing = true;
                Task::perform(
                    async move {
                        let client = WebDavClient::new(&url, &user, &pass)
                            .map_err(|e| InfiltratorError::Sync(e.to_string()))?;
                        let files = client
                            .list("")
                            .await
                            .map_err(|e| InfiltratorError::Sync(e.to_string()))?;

                        for file in files {
                            if file.path.ends_with(".yaml") {
                                let content = client
                                    .get(&file.path)
                                    .await
                                    .map_err(|e| InfiltratorError::Sync(e.to_string()))?;
                                let data_dir = mihomo_platform::paths::get_home_dir().map_err(
                                    |e: mihomo_api::error::MihomoError| InfiltratorError::from(e),
                                )?;
                                let path = data_dir.join("configs").join(file.path);
                                tokio::fs::write(path, content)
                                    .await
                                    .map_err(|e| InfiltratorError::Io(e.to_string()))?;
                            }
                        }
                        Ok(())
                    },
                    Message::SyncFinished,
                )
            }
            Message::SyncFinished(result) => {
                self.is_syncing = false;
                match result {
                    Ok(_) => Task::done(Message::ShowToast(
                        "Sync completed".to_string(),
                        ToastStatus::Success,
                    )),
                    Err(e) => {
                        self.set_error(&e);
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::TickWebDavSync => {
                if self.webdav_enabled
                    && !self.webdav_url.is_empty()
                    && !self.webdav_user.is_empty()
                {
                    return Task::done(Message::SyncUpload);
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }
}

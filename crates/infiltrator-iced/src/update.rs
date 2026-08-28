pub mod core;
pub mod profile;
pub mod ui;

use crate::state::AppState;
use crate::types::Message;
use iced::Task;

impl AppState {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // UI & Navigation
            Message::Navigate(_)
            | Message::ToggleTheme
            | Message::TickFrame(_)
            | Message::WindowClosed(_)
            | Message::HideWindow
            | Message::ShowWindow
            | Message::Exit
            | Message::ShowToast(_, _)
            | Message::RemoveToast(_)
            | Message::SetSystemProxy(_)
            | Message::SystemProxySet(_)
            | Message::TogglePerfPanel => self.update_ui(message),

            // Profiles & Sync
            Message::LoadProfiles
            | Message::ProfilesLoaded(_)
            | Message::SetActiveProfile(_)
            | Message::UpdateImportUrl(_)
            | Message::UpdateImportName(_)
            | Message::UpdateImportActivate(_)
            | Message::ImportProfile
            | Message::ProfileImported(_)
            | Message::DeleteProfile(_)
            | Message::ProfileDeleted(_)
            | Message::UpdateLocalImportPath(_)
            | Message::BrowseLocalImportFile
            | Message::LocalImportFilePicked(_)
            | Message::UpdateLocalImportName(_)
            | Message::UpdateLocalImportActivate(_)
            | Message::ImportLocalProfile
            | Message::LocalProfileImported(_)
            | Message::SelectSubscriptionProfile(_)
            | Message::UpdateSubscriptionUrl(_)
            | Message::UpdateSubscriptionAutoUpdate(_)
            | Message::UpdateSubscriptionInterval(_)
            | Message::SaveSubscriptionSettings
            | Message::SubscriptionSettingsSaved(_)
            | Message::UpdateSubscriptionNow
            | Message::SubscriptionUpdatedNow(_)
            | Message::SubscriptionAutoUpdated(_)
            | Message::UpdateProfilesFilter(_)
            | Message::ClearProfiles
            | Message::ProfilesCleared(_)
            | Message::EditProfile(_)
            | Message::ProfileContentLoaded(_)
            | Message::EditorAction(_)
            | Message::SaveProfile
            | Message::ProfileSaved(_)
            | Message::UpdateWebDavUrl(_)
            | Message::UpdateWebDavUser(_)
            | Message::UpdateWebDavPass(_)
            | Message::UpdateWebDavEnabled(_)
            | Message::UpdateWebDavSyncInterval(_)
            | Message::UpdateWebDavSyncOnStartup(_)
            | Message::UpdateEditorPathSetting(_)
            | Message::SaveAppSettings
            | Message::AppSettingsSaved(_)
            | Message::SyncUpload
            | Message::SyncDownload
            | Message::SyncFinished(_)
            | Message::TickSubUpdate
            | Message::TickWebDavSync => self.update_profile(message),

            // Core & Network
            _ => self.update_core(message),
        }
    }
}

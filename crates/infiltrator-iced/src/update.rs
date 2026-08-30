pub mod core;
pub mod profile;
pub mod ui;

use crate::state::AppState;
use crate::types::Message;
use iced::Task;

impl AppState {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        // demo-mode: every message that would reach the network, spawn a
        // process/server, write a settings/profile/rules file, touch the
        // system proxy / autostart / registry or open an external app is a
        // no-op. Read-only local UI state changes keep flowing so all pages
        // stay interactive (most runtime actions already no-op naturally
        // because the demo keeps `AppState::runtime` unset).
        if self.shell.demo {
            match message {
                Message::StartProxy
                | Message::StopProxy
                | Message::FetchIpInfo
                | Message::SetSystemProxy(_)
                | Message::SetAutostart(_)
                | Message::SetAdminEnabled(_)
                | Message::ApplyAdminSettings
                | Message::TickSubUpdate
                | Message::TickWebDavSync
                | Message::SaveAppSettings
                | Message::SaveSubscriptionSettings
                | Message::UpdateSubscriptionNow
                | Message::ImportProfile
                | Message::ImportLocalProfile
                | Message::BrowseLocalImportFile
                | Message::DeleteProfile(_)
                | Message::SetActiveProfile(_)
                | Message::ClearProfiles
                | Message::SaveProfile
                | Message::SaveRules
                | Message::AddCustomRule
                | Message::SaveDns
                | Message::SaveFakeIpConfig
                | Message::SaveTunConfig
                | Message::SaveRuleProvidersJson
                | Message::SaveProxyProvidersJson
                | Message::SaveSnifferJson
                | Message::SyncUpload
                | Message::SyncDownload
                | Message::LoadKernels
                | Message::CheckCoreUpdate
                | Message::DownloadCore(_)
                | Message::DeleteKernel(_)
                | Message::SetDefaultKernel(_)
                | Message::FactoryReset
                | Message::OpenConfigDir
                | Message::OpenWebAdmin
                | Message::RequestAdminPrivilege
                | Message::FlushFakeIpCache => return Task::none(),
                _ => {}
            }
        }

        match message {
            // UI & Navigation
            Message::Navigate(_)
            | Message::ToggleTheme
            | Message::TickFrame(_)
            | Message::WindowClosed(_)
            | Message::HideWindow
            | Message::ShowWindow
            | Message::Exit
            | Message::TrayEvent(_)
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
            | Message::SetAdminEnabled(_)
            | Message::UpdateAdminPort(_)
            | Message::ApplyAdminSettings
            | Message::AdminSettingsSaved(_)
            | Message::AdminServerStarted(_)
            | Message::OpenWebAdmin
            | Message::SyncUpload
            | Message::SyncDownload
            | Message::SyncFinished(_)
            | Message::TickSubUpdate
            | Message::TickWebDavSync => self.update_profile(message),

            // Core & Network
            Message::ToggleProxyGroupExpanded(group) => {
                // ui-wave2-p：None 表示初始状态（默认展开第一组）；首次交互时以当前
                // 过滤结果的第一组为基线，之后完全由用户点击决定展开集合。
                let mut ids = self
                    .runtime
                    .proxy_groups_expanded
                    .take()
                    .unwrap_or_else(|| {
                        self.runtime
                            .filtered_groups
                            .first()
                            .map(|(name, _)| vec![name.clone()])
                            .unwrap_or_default()
                    });
                match ids.iter().position(|g| g == &group) {
                    Some(index) => {
                        ids.remove(index);
                    }
                    None => ids.push(group),
                }
                self.runtime.proxy_groups_expanded = Some(ids);
                Task::none()
            }
            _ => self.update_core(message),
        }
    }
}

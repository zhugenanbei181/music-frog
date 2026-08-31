//! Profile-domain message handlers (the `update_profile` dispatch).
//!
//! Handlers are grouped by business topic in the sibling submodules; the
//! root keeps only the dispatcher and the shared cache invalidation helper.

mod admin;
mod editor;
mod import;
mod options;
mod profiles;
mod settings;
mod subscription;
mod sync;
mod sync_diff;

use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;

impl AppState {
    /// Invalidate every rules / advanced-config cache so the next visit to
    /// those pages re-reads from disk (profiles, rules and editor writes all
    /// funnel through here).
    fn invalidate_rules_dns_views(&mut self) {
        self.editor.rules_loaded_once = false;
        self.editor.advanced_configs_loaded_once = false;
        self.editor.rules_render_cache.clear();
        self.editor.rules_filtered_indices.clear();
        self.editor.rules_page = 0;
        self.editor.rules_heavy_ready = false;
        self.editor.dns_heavy_ready = false;
        self.editor.rule_providers_editor_state = crate::types::editor::EditorLazyState::Unloaded;
        self.editor.proxy_providers_editor_state = crate::types::editor::EditorLazyState::Unloaded;
        self.editor.sniffer_editor_state = crate::types::editor::EditorLazyState::Unloaded;
        self.editor.dns_editor_state = crate::types::editor::EditorLazyState::Unloaded;
        self.editor.fake_ip_editor_state = crate::types::editor::EditorLazyState::Unloaded;
        self.editor.tun_editor_state = crate::types::editor::EditorLazyState::Unloaded;
    }

    pub fn update_profile(&mut self, message: Message) -> Task<Message> {
        match message {
            // Profile catalog: load / filter / reset / activate / delete.
            Message::LoadProfiles
            | Message::ProfilesLoaded(_)
            | Message::UpdateProfilesFilter(_)
            | Message::ClearProfiles
            | Message::ProfilesCleared(_)
            | Message::SetActiveProfile(_)
            | Message::ProfileActivationFinished(_)
            | Message::DeleteProfile(_)
            | Message::ProfileDeleted(_) => self.update_profiles(message),

            // Profile import: subscription URL + local YAML file.
            Message::UpdateImportUrl(_)
            | Message::UpdateImportName(_)
            | Message::UpdateImportActivate(_)
            | Message::ImportProfile
            | Message::ProfileImported(_)
            | Message::BrowseLocalImportFile
            | Message::LocalImportFilePicked(_)
            | Message::UpdateLocalImportPath(_)
            | Message::UpdateLocalImportName(_)
            | Message::UpdateLocalImportActivate(_)
            | Message::ImportLocalProfile
            | Message::LocalProfileImported(_) => self.update_import(message),

            // Subscription settings and (auto-)updates.
            Message::SelectSubscriptionProfile(_)
            | Message::UpdateSubscriptionUrl(_)
            | Message::UpdateSubscriptionAutoUpdate(_)
            | Message::UpdateSubscriptionInterval(_)
            | Message::SaveSubscriptionSettings
            | Message::SubscriptionSettingsSaved(_)
            | Message::UpdateSubscriptionNow
            | Message::SubscriptionUpdatedNow(_)
            | Message::SubscriptionAutoUpdated(_)
            // Tray bulk entries (update-all / per-profile auto-update).
            | Message::UpdateAllSubscriptionsNow
            | Message::AllSubscriptionsUpdated(_)
            | Message::SetProfileAutoUpdate { .. }
            | Message::ProfileAutoUpdateSet(_)
            | Message::TickSubUpdate => self.update_subscription(message),

            // Profile YAML editor.
            Message::EditProfile(_)
            | Message::EditProfileAs(_, _)
            | Message::ProfileContentLoaded(_)
            | Message::LoadProfileSnapshots
            | Message::ProfileSnapshotsLoaded(_)
            | Message::RestoreProfileSnapshot(_)
            | Message::ProfileSnapshotRestored(_)
            | Message::EditorAction(_)
            | Message::SaveProfile
            | Message::ProfileSaved(_) => self.update_editor(message),

            // Profile options: mixin overlay editor + subscription filter.
            Message::SetEditorPane(_)
            | Message::MixinEditorAction(_)
            | Message::MixinLoaded(_)
            | Message::SaveMixin
            | Message::MixinSaved(_)
            | Message::LoadProfileFilter
            | Message::ProfileFilterLoaded(_)
            | Message::UpdateFilterInclude(_)
            | Message::UpdateFilterExclude(_)
            | Message::UpdateFilterExcludeTypes(_)
            | Message::UpdateFilterRenames(_)
            | Message::UpdateFilterDedup(_)
            | Message::SaveProfileFilter
            | Message::ProfileFilterSaved(_) => self.update_options(message),

            // App settings: WebDAV account, editor path, language/theme.
            Message::UpdateWebDavUrl(_)
            | Message::UpdateWebDavUser(_)
            | Message::UpdateWebDavPass(_)
            | Message::UpdateWebDavEnabled(_)
            | Message::UpdateWebDavSyncInterval(_)
            | Message::UpdateWebDavSyncOnStartup(_)
            | Message::UpdateEditorPathSetting(_)
            | Message::UpdateNotificationsEnabled(_)
            | Message::SetLanguage(_)
            | Message::SaveAppSettings
            | Message::AppSettingsSaved(_) => self.update_settings(message),

            // Web admin server settings.
            Message::SetAdminEnabled(_)
            | Message::UpdateAdminPort(_)
            | Message::ApplyAdminSettings
            | Message::AdminSettingsSaved(_)
            | Message::AdminServerStarted(_)
            => self.update_admin(message),

            // WebDAV sync.
            Message::SyncUpload
            | Message::SyncDownload
            | Message::SyncFinished(_)
            | Message::SyncProgress(_)
            | Message::ResolveSyncConflict(_)
            | Message::DismissSyncConflict(_)
            | Message::SyncConflictResolved(_)
            | Message::SyncConflictDismissed(_)
            | Message::LoadSyncDiff(_)
            | Message::SyncDiffLoaded(_)
            | Message::PickSyncDiffKey(_, _)
            | Message::SetSyncDiffPicks(_)
            | Message::ApplySyncDiffMerge
            | Message::SyncDiffMerged(_)
            | Message::CloseSyncDiff
            | Message::CancelWebDavSync
            | Message::TestWebDavConnection
            | Message::WebDavConnectionTested(_)
            | Message::TickWebDavSync => self.update_sync(message),

            _ => Task::none(),
        }
    }
}

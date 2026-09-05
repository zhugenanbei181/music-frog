mod ui_wave5;
mod ui_wave4;
mod ui_wave3;
pub mod core;
pub mod profile;
pub mod ui;

use crate::state::AppState;
use crate::types::message::Message;
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
                Message::SetAppRoutingMode(mode) => {
                    self.app_routing.mode = mode;
                    return Task::none();
                }
                Message::SetAppRouteRule { process, rule } => {
                    self.app_routing.custom_rules.insert(process, rule);
                    return Task::none();
                }
                Message::RefreshAppRoutingProcesses
                | Message::AppRoutingConfigLoaded(_)
                | Message::AppRoutingPersisted(_) => return Task::none(),
                Message::StartProxy
                | Message::StopProxy
                | Message::FetchIpInfo
                | Message::SetSystemProxy(_)
                | Message::SetProxyMode(_)
                | Message::SetTunEnabled(_)
                | Message::SetTunStack(_)
                | Message::SetTunAutoRoute(_)
                | Message::SetTunStrictRoute(_)
                | Message::SetSnifferEnabled(_)
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
                | Message::RestoreProfileSnapshot(_)
                | Message::SaveMixin
                | Message::LoadSyncDiff(_)
                | Message::ApplySyncDiffMerge
                | Message::SaveProfileFilter
                | Message::LoadProfileFilter
                | Message::ScanMrsProviders
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
                | Message::TestWebDavConnection
                | Message::LoadKernels
                | Message::CheckCoreUpdate
                | Message::DownloadCore(_)
                | Message::CancelCoreDownload
                | Message::DeleteKernel(_)
                | Message::SetDefaultKernel(_)
                | Message::FactoryReset
                | Message::OpenConfigDir
                | Message::RequestAdminPrivilege
                | Message::InstallTunService
                | Message::RefreshTunServiceStatus
                | Message::FlushFakeIpCache
                // Doctor 面板走 loopback HTTP；demo 会话没有内嵌 admin server。
                | Message::RunDoctor
                | Message::RunDoctorFix
                | Message::RunBootstrap => return Task::none(),
                _ => {}
            }
        }

        match message {
            // UI & Navigation
            Message::ToggleCommandPalette
            | Message::OpenCommandPalette
            | Message::CloseCommandPalette
            | Message::SetCommandQuery(_)
            | Message::SelectNextCommand
            | Message::SelectPrevCommand
            | Message::ExecuteCommand(_)
            | Message::InspectConnection(_)
            | Message::CloseSingleConnection(_)
            | Message::InsertYamlSnippet(_)
            | Message::FormatYamlEditor
            | Message::RefreshAppRoutingProcesses
            | Message::AppRoutingProcessesLoaded(_)
            | Message::AppRoutingConfigLoaded(_)
            | Message::AppRoutingPersisted(_)
            | Message::SetAppRoutingFilter(_)
            | Message::SetAppRoutingMode(_)
            | Message::SetAppRouteRule { .. }
            | Message::SetAppRoutingCategory(_)
            | Message::MoveProxyGroupUp(_)
            | Message::MoveProxyGroupDown(_)
            | Message::ResetProxyGroupOrder
            | Message::ToggleMiniHudMode
            | Message::SetAlwaysOnTop(_)
            | Message::RunScriptSandboxTest
            | Message::SelectScriptPreset(_)
            | Message::UpdateScriptSandboxCode(_)
            | Message::UpdateScriptSandboxInputYaml(_)
            | Message::ClearScriptSandbox
            | Message::RunDnsLeakProbe
            | Message::DnsLeakProbeFinished(_)
            | Message::OpenCustomNodeModal
            | Message::CloseCustomNodeModal
            | Message::UpdateCustomNodeUriInput(_)
            | Message::ParseAndImportCustomUri
            | Message::ExportNodeAsUri(_)
            | Message::SaveCustomNodeForm
            | Message::OpenAggregatorModal
            | Message::CloseAggregatorModal
            | Message::ToggleAggregatorProfileSelection(_)
            | Message::UpdateAggregatorName(_)
            | Message::ExecuteProfileAggregation
            | Message::SetConnectionGroupingMode(_)
            | Message::AddQuickRuleFromConnection { .. }
            | Message::OpenSnapshotDiff(_)
            | Message::CloseSnapshotDiff
            | Message::RollbackToSnapshot(_)
            | Message::UpdateHotkeyCombo { .. }
            | Message::ToggleHotkeyEnabled(_)
            | Message::TogglePcapCapture
            | Message::ExportPcapBuffer
            | Message::UpdateSubRuleOperator(_)
            | Message::AddSubRuleCondition(_)
            | Message::RemoveSubRuleCondition(_)
            | Message::UpdateSubRuleTarget(_)
            | Message::InsertSubRuleIntoRules
            | Message::RunNodeSpeedtest(_)
            | Message::NodeSpeedtestFinished(_)
            | Message::CheckGeoDataUpdates
            | Message::TriggerGeoDataUpdate
            | Message::GeoDataUpdateFinished(_)
            | Message::ScanUwpApps
            | Message::UwpAppsLoaded(_)
            | Message::ExemptAllUwpApps
            | Message::ClearAllUwpExemptions
            | Message::ToggleUwpAppExemption(_)
            | Message::UpdateEncryptedBackupPassphrase(_)
            | Message::ExportEncryptedPackage
            | Message::ImportEncryptedPackage
            | Message::PollNetworkInterfaces
            | Message::NetworkInterfacesPolled(_)
            | Message::ForceGatewayReconnect
            | Message::CheckCrashWatchdog
            | Message::RecoverOrphanedState
            | Message::ExportCrashDiagnostics
            | Message::LaunchWebDashboard(_)
            | Message::UpdateLogRegexFilter(_)
            | Message::SetLogLevelFilter(_)
            | Message::ExportRedactedLogs
            | Message::EvaluateSubscriptionQuota
            | Message::UpdateCronScheduleHours(_)
            | Message::UpdatePacBypassSubnets(_)
            | Message::CompileAndValidatePac
            | Message::TogglePacMode(_)
            | Message::AuditStaleRules
            | Message::DisableZeroHitRules
            | Message::SelectRadarNode(_)
            | Message::RecordRadarLatencySample { .. }
            | Message::SelectTunStack(_)
            | Message::ProbeOptimalMtu
            | Message::MtuProbed(_)
            | Message::UnpackRuleProviderToCustom(_)
            | Message::PurgeRuleProviderCache
            | Message::TriggerAtomicConfigApply
            | Message::ApplyTransactionStageChanged(_)
            | Message::ToggleLanSharing(_)
            | Message::UpdateLanSharingPort(_)
            | Message::UpdateLanAclWhitelist(_)
            | Message::Navigate(_)
            | Message::NavigateBack
            | Message::NavigateForward
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
            | Message::TogglePerfPanel
            | Message::RequestConfirmation(_)
            | Message::ConfirmAction
            | Message::CancelConfirmation
            | Message::ClearError
            | Message::OpenConfigDir
            | Message::OpenConfigDirFinished(_) => self.update_ui(message),

            // Profiles & Sync
            Message::LoadProfiles
            | Message::ProfilesLoaded(_)
            | Message::SetActiveProfile(_)
            | Message::ProfileActivationFinished(_)
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
            | Message::UpdateAllSubscriptionsNow
            | Message::AllSubscriptionsUpdated(_)
            | Message::SetProfileAutoUpdate { .. }
            | Message::ProfileAutoUpdateSet(_)
            | Message::UpdateProfilesFilter(_)
            | Message::ClearProfiles
            | Message::ProfilesCleared(_)
            | Message::EditProfile(_)
            | Message::EditProfileAs(_, _)
            | Message::ProfileContentLoaded(_)
            | Message::LoadProfileSnapshots
            | Message::ProfileSnapshotsLoaded(_)
            | Message::RestoreProfileSnapshot(_)
            | Message::ProfileSnapshotRestored(_)
            | Message::EditorAction(_)
            | Message::SaveProfile
            | Message::ProfileSaved(_)
            | Message::SetEditorPane(_)
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
            | Message::ProfileFilterSaved(_)
            | Message::UpdateWebDavUrl(_)
            | Message::UpdateWebDavUser(_)
            | Message::UpdateWebDavPass(_)
            | Message::UpdateWebDavEnabled(_)
            | Message::UpdateWebDavSyncInterval(_)
            | Message::UpdateWebDavSyncOnStartup(_)
            | Message::UpdateEditorPathSetting(_)
            | Message::SetLanguage(_)
            | Message::SaveAppSettings
            | Message::AppSettingsSaved(_)
            | Message::SetAdminEnabled(_)
            | Message::UpdateAdminPort(_)
            | Message::ApplyAdminSettings
            | Message::AdminSettingsSaved(_)
            | Message::AdminServerStarted(_)
            | Message::SyncUpload
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

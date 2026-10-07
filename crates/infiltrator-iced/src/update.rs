use crate::types::script::ScriptAction;
use crate::types::snapshot_restore::RestoreAction;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::overview_layout::OverviewCardKind;
use infiltrator_contract::responsive_viewport::ResponsiveViewportSnapshot;
pub mod aggregator;
mod chrome;
pub mod core;
mod log_export;
mod mini_hud;
pub mod profile;
pub mod protocol_codec;
mod rule_statistics;
mod script_workbench;
pub mod shell;
mod snapshot_diff;
mod snapshot_restore;
mod system_proxy;
pub mod ui;
mod ui_wave3;
mod ui_wave4;
mod ui_wave5;

use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;

impl AppState {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        if let Message::SnapshotRestore(action) = message {
            return self.update_snapshot_restore(action);
        }
        if self.editor.snapshot_restore.visible {
            if matches!(
                message,
                Message::EditorAction(_)
                    | Message::MixinEditorAction(_)
                    | Message::InsertYamlSnippet(_)
                    | Message::FormatYamlEditor
                    | Message::SaveProfile
                    | Message::DiscardProfileDraft
            ) {
                return Task::none();
            }
            if let Message::KeyboardChord { key, .. } = &message {
                return if key.eq_ignore_ascii_case("escape") {
                    self.update_snapshot_restore(RestoreAction::Cancel)
                } else {
                    Task::none()
                };
            }
            if matches!(
                message,
                Message::Navigate(_) | Message::NavigateBack | Message::NavigateForward
            ) {
                return self.update_snapshot_restore(RestoreAction::Cancel);
            }
        }
        if self.editor.script_sandbox.export_visible {
            if let Message::KeyboardChord { key, .. } = &message {
                return if key.eq_ignore_ascii_case("escape") {
                    self.update_script_workbench(Message::Script(ScriptAction::CancelExport))
                } else {
                    Task::none()
                };
            }
            if matches!(
                message,
                Message::Navigate(_) | Message::NavigateBack | Message::NavigateForward
            ) {
                return Task::none();
            }
        }
        if self.diag.log_export.open {
            if let Message::KeyboardChord { key, .. } = &message {
                return if key.eq_ignore_ascii_case("escape") {
                    self.update_log_export(Message::CancelLogExport)
                } else {
                    Task::none()
                };
            }
            if matches!(
                message,
                Message::Navigate(_) | Message::NavigateBack | Message::NavigateForward
            ) {
                return Task::none();
            }
        }
        // demo-mode: every message that would reach the network, spawn a
        // process/server, write a settings/profile/rules file, touch the
        // system proxy / autostart / registry or open an external app is a
        // no-op. Read-only local UI state changes keep flowing so all pages
        // stay interactive (most runtime actions already no-op naturally
        // because the demo keeps `AppState::runtime` unset).
        if self.shell.demo {
            match message {
                Message::SetProxyMode(_) if self.commands.is_none() => return Task::none(),
                Message::SaveProfileFilter | Message::LoadProfileFilter
                    if self.commands.is_none() =>
                {
                    return Task::none();
                }
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
                | Message::SetCoreLogLevel(_)
                | Message::CoreLogLevelFinished(_, _)
                | Message::ServiceModePrepared(_)
                | Message::RepairPortConflicts
                | Message::PortConflictsRepaired(_)
                | Message::SetIpv6Routing(_)
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
                | Message::PreviewProfileAggregation
                | Message::CreateAggregatedProfile
                | Message::BrowseLocalImportFile
                | Message::DeleteProfile(_)
                | Message::SetActiveProfile(_)
                | Message::ClearProfiles
                | Message::SaveProfile
                | Message::DiscardProfileDraft
                | Message::RestoreProfileSnapshot(_)
                | Message::SaveMixin
                | Message::DiscardMixinDraft
                | Message::LoadSyncDiff(_)
                | Message::ApplySyncDiffMerge
                | Message::ScanMrsProviders
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
                | Message::RollbackCore
                | Message::FactoryReset
                | Message::OpenConfigDir
                | Message::RequestAdminPrivilege
                | Message::InstallTunService
                | Message::RefreshTunServiceStatus
                | Message::RunDnsLatencyProbe
                | Message::DnsLeakProbed(_)
                | Message::RunStunProbe
                | Message::StunProbed(_)
                | Message::RunPrivilegedNetworkRegression
                | Message::PrivilegedNetworkRegressionUpdated(_) => return Task::none(),
                Message::SaveRules
                | Message::RunDnsLeakProbe
                | Message::RetryDnsLeakProbe
                | Message::RunDoctor
                | Message::RunDoctorFix
                | Message::RunBootstrap
                | Message::RepairDoctorIssue(_)
                | Message::RetryDoctorCommand
                    if self.commands.is_none() =>
                {
                    return Task::none();
                }
                _ => {}
            }
        }

        match message {
            Message::ExportRedactedLogs
            | Message::ConfirmLogExport
            | Message::CancelLogExport
            | Message::RetryLogExport
            | Message::LogExportFinished { .. } => self.update_log_export(message),
            Message::LanguageChoiceApplied { token, result } => {
                self.finish_language_choice(token, result);
                Task::none()
            }
            Message::RetryLanguageChoice => self
                .shell
                .language_choice
                .requested
                .map(|preference| self.set_language_choice(preference.as_setting().into()))
                .unwrap_or_else(Task::none),
            Message::OpenProxyGroupOrder => self.open_group_order(None),
            Message::ApplyProxyGroupOrder => self.apply_group_order(),
            Message::CancelProxyGroupOrder => {
                if self.runtime.group_order_editor.pending.is_none() {
                    self.runtime.group_order_editor.cancel();
                    self.runtime.group_order_open = false;
                }
                Task::none()
            }
            Message::ProxyGroupOrderApplied { token, result } => {
                let success = result.is_ok();
                if self.runtime.group_order_editor.finish(token, result) && success {
                    self.runtime.group_order_open = false;
                }
                Task::none()
            }
            Message::OpenProxyProbeOptions => {
                self.runtime.probe_options_open = true;
                Task::none()
            }
            Message::ApplyProxyProbeOptions => self.apply_probe_options(),
            Message::CancelProxyProbeOptions => {
                if self.runtime.probe_options_editor.pending.is_some() {
                    return Task::none();
                }
                self.runtime.probe_options_open = false;
                self.runtime.probe_options_editor.cancel();
                self.sync_probe_draft_fields();
                Task::none()
            }
            Message::ProxyProbeOptionsApplied { token, result } => {
                let succeeded = result.is_ok();
                if self.runtime.probe_options_editor.finish(token, result) && succeeded {
                    self.runtime.probe_options_open = false;
                }
                self.sync_probe_draft_fields();
                Task::none()
            }
            Message::ProxyPreferenceFinished { token, result } => {
                self.finish_proxy_preference(token, result)
            }
            Message::ProxySearchFinished {
                token,
                query,
                result,
            } => self.finish_proxy_search(token, query, result),
            Message::SurfaceSnapshotUpdated(snapshot) => {
                self.apply_shared_surface_snapshot(*snapshot);
                // DUAL-15-02: a live traffic sample may have arrived; push the
                // shared rate badge (throttled + deduped inside).
                self.refresh_tray_rates();
                self.sync_logs_viewport_task()
            }
            // Window geometry is local shell state, but the derived layout tier
            // is the shared contract so both surfaces classify widths identically.
            Message::WindowResized(width, height) => {
                self.shell.viewport = ResponsiveViewportSnapshot::from_dimensions(width, height);
                // Paginated lists derive their page budget from the tier so a
                // short/narrow window builds fewer heavy rows. Keeping it on the
                // stored field means view and paging logic share one source.
                let tier = self.shell.viewport.tier;
                self.editor.rules_page_size = tier.list_page_rows(200);
                self.diag.connections_page_size = tier.list_page_rows(100);
                Task::none()
            }
            // DUAL-15-08: focus is the only window power fact Iced exposes, so
            // the shared cadence resolves to Active (60 FPS) or Background
            // (2 FPS) for the animation frame tick.
            Message::WindowFocusChanged(focused) => {
                self.shell.window_focused = focused;
                Task::none()
            }
            // DUAL-15-11: one shared composition state fed by the real toolkit
            // IME events (the text widget still owns the field text). Chord
            // dispatch consults this state so composing keys stay with the
            // input method instead of triggering shell shortcuts.
            Message::ImeComposition(event) => {
                self.shell.ime.apply(event);
                Task::none()
            }
            // Overview card order is the shared `OverviewLayoutSnapshot`; the
            // view assembles its cards from this local projection of it.
            Message::MoveOverviewCardUp(kind) => {
                self.move_overview_card(kind, true);
                Task::none()
            }
            Message::MoveOverviewCardDown(kind) => {
                self.move_overview_card(kind, false);
                Task::none()
            }
            Message::ResetOverviewCardOrder => {
                self.diag.overview_card_order = OverviewCardKind::DEFAULT_ORDER.to_vec();
                Task::none()
            }
            // DUAL-08 multi-subscription aggregator: the modal state, the real
            // shared preview and the "save as new profile" action.
            Message::OpenAggregatorModal
            | Message::CloseAggregatorModal
            | Message::ToggleAggregatorProfileSelection(_)
            | Message::UpdateAggregatorName(_)
            | Message::ToggleAggregatorDeduplicate
            | Message::ToggleAggregatorGeoCluster
            | Message::ToggleAggregatorGenerateGroups
            | Message::ToggleAggregatorRemoveEmojis
            | Message::ToggleAggregatorAvailabilityPrecheck
            | Message::ToggleAggregatorActivateAfterCreate
            | Message::UpdateAggregatorRenames(_)
            | Message::UpdateAggregatorCustomGroupName(_)
            | Message::UpdateAggregatorCustomGroupKeywords(_)
            | Message::AddAggregatorCustomGroup
            | Message::RemoveAggregatorCustomGroup(_)
            | Message::LoadAggregatorTemplates
            | Message::AggregatorTemplatesLoaded(_)
            | Message::ApplyAggregatorTemplate(_)
            | Message::SaveAggregatorTemplate
            | Message::UpdateAggregatorTemplateName(_)
            | Message::AggregatorTemplateSaved(_)
            | Message::DeleteAggregatorTemplate(_)
            | Message::AggregatorTemplateDeleted(_)
            | Message::ReAggregateProfile(_)
            | Message::AggregationReaggregated(_)
            | Message::PreviewProfileAggregation
            | Message::AggregationPreviewFinished(_)
            | Message::CreateAggregatedProfile
            | Message::AggregatedProfileCreated(_) => self.update_aggregator(message),
            // UI & Navigation
            Message::ToggleCommandPalette
            | Message::OpenCommandPalette
            | Message::CloseCommandPalette
            | Message::SetCommandQuery(_)
            | Message::SelectNextCommand
            | Message::SelectPrevCommand
            | Message::ExecuteCommand(_)
            | Message::ExecuteSelectedCommand
            | Message::CopyConnectionHost
            | Message::InspectConnection(_)
            | Message::CloseSingleConnection(_)
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
            | Message::MiniHudMoved { .. }
            | Message::MiniHudDragReleased
            | Message::MiniHudPlacementUpdated(_)
            | Message::MiniHudDisplayKnown(_)
            | Message::WindowIdResolved(_)
            | Message::WindowChromeDragRequested
            | Message::WindowChromeToggleMaximize
            | Message::WindowChromeMinimize
            | Message::WindowChromeClose
            | Message::Script(_)
            | Message::SnapshotRestore(_)
            | Message::OpenCustomNodeModal
            | Message::CloseCustomNodeModal
            | Message::UpdateCustomNodeUriInput(_)
            | Message::ParseAndImportCustomUri
            | Message::UpdateCustomNodeDraft(_)
            | Message::UpdateCustomNodeField(_, _)
            | Message::ExportCustomNodeUri
            | Message::SaveCustomNodeForm
            | Message::CustomNodeSaved(_)
            | Message::ScanCustomNodeDialer
            | Message::CustomNodeDialerScanned(_)
            | Message::VerifyCustomNodeCertificateAuthority
            | Message::SetConnectionGroupingMode(_)
            | Message::AddQuickRuleFromConnection { .. }
            | Message::OpenSnapshotDiff(_)
            | Message::CloseSnapshotDiff
            | Message::SnapshotDiffLoaded(_)
            | Message::SetSnapshotDiffMode(_)
            | Message::RefreshSnapshotDiff
            | Message::RollbackToSnapshot(_)
            | Message::SetProfileProtectionOverride(_)
            | Message::ToggleHotkeyEnabled(_)
            | Message::TogglePcapCapture
            | Message::ExportPcapBuffer
            | Message::UpdateSubRuleOperator(_)
            | Message::AddSubRuleCondition(_)
            | Message::RemoveSubRuleCondition(_)
            | Message::UpdateSubRuleTarget(_)
            | Message::InsertSubRuleIntoRules
            | Message::RunNodeSpeedtest(_)
            | Message::SpeedtestSnapshotUpdated(_)
            | Message::OpenSpeedtestDetail
            | Message::CloseSpeedtestDetail
            | Message::CheckGeoDataUpdates
            | Message::TriggerGeoDataUpdate
            | Message::GeoDataUpdateResult(_)
            | Message::ScanUwpApps
            | Message::UwpAppsLoaded(_)
            | Message::UwpSnapshotLoaded(_)
            | Message::ExemptAllUwpApps
            | Message::ClearAllUwpExemptions
            | Message::ToggleUwpAppExemption(_)
            | Message::UwpExemptionsChanged(_)
            | Message::UpdateEncryptedBackupPassphrase(_)
            | Message::ExportEncryptedPackage
            | Message::ImportEncryptedPackage
            | Message::PollNetworkInterfaces
            | Message::NetworkInterfacesPolled(_)
            | Message::ForceGatewayReconnect
            | Message::NetworkRoamingRepaired(_)
            | Message::StartVpn
            | Message::StopVpn
            | Message::VpnSessionUpdated(_)
            | Message::RunPrivilegedNetworkRegression
            | Message::PrivilegedNetworkRegressionUpdated(_)
            | Message::CheckCrashWatchdog
            | Message::RecoverOrphanedState
            | Message::ExportCrashDiagnostics
            | Message::LaunchWebDashboard(_)
            | Message::UpdateLogRegexFilter(_)
            | Message::SetLogLevelFilter(_)
            | Message::UpdatePacBypassSubnets(_)
            | Message::CompileAndValidatePac
            | Message::PacApplied(_)
            | Message::TogglePacMode(_)
            | Message::RuleStatistics(_)
            | Message::SelectTunStack(_)
            | Message::ProbeOptimalMtu
            | Message::MtuProbed(_)
            | Message::MtuProbeFinished(_)
            | Message::UnpackRuleProviderToCustom(_)
            | Message::PurgeRuleProviderCache
            | Message::ToggleLanSharing(_)
            | Message::UpdateLanSharingPort(_)
            | Message::UpdateLanBindAddress(_)
            | Message::UpdateLanAclWhitelist(_)
            | Message::ApplyLanSharing
            | Message::LanSharingSet(_, _)
            | Message::UpdateLanAllowedIps(_)
            | Message::UpdateLanDisallowedIps(_)
            | Message::UpdateLanSkipAuthPrefixes(_)
            | Message::ToggleLanAuthentication(_)
            | Message::UpdateLanAuthUsername(_)
            | Message::UpdateLanAuthPassword(_)
            | Message::ApplyLanSecurity
            | Message::LanSecuritySet(_, _)
            | Message::Navigate(_)
            | Message::NavigateBack
            | Message::NavigateForward
            | Message::ToggleTheme
            | Message::SetTheme(_)
            | Message::SystemThemeChanged(_)
            | Message::CycleThemePreference
            | Message::BeginHotkeyCapture(_)
            | Message::CancelHotkeyCapture
            | Message::KeyboardChord { .. }
            | Message::ResetHotkey(_)
            | Message::ShortcutsUpdated(_)
            | Message::TickFrame(_)
            | Message::CaptureRegionMeasured(_)
            | Message::CaptureFrameRendered { .. }
            | Message::WindowClosed(_)
            | Message::HideWindow
            | Message::ShowWindow
            | Message::Exit
            | Message::TrayEvent(_)
            | Message::ShowToast(_, _)
            | Message::RemoveToast(_)
            | Message::SetSystemProxy(_)
            | Message::SystemProxySet(_)
            | Message::SystemProxyReconciled(_)
            | Message::SystemProxyRecoveryFinished(_)
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
            | Message::UpdateSubscriptionCron(_)
            | Message::UpdateSubscriptionUserAgent(_)
            | Message::UpdateSubscriptionInsecureSkipVerify(_)
            | Message::UpdateSubscriptionAutoReload(_)
            | Message::SaveSubscriptionSettings
            | Message::SubscriptionSettingsSaved(_)
            | Message::UpdateSubscriptionNow
            | Message::SubscriptionUpdatedNow(_)
            | Message::SubscriptionAutoUpdated(_)
            | Message::RestoreSubscriptionBackup
            | Message::SubscriptionBackupRestored(_)
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
            | Message::BackupProfileSnapshot
            | Message::ProfileSnapshotBackedUp(_)
            | Message::SetSnapshotPruneKeep(_)
            | Message::PruneProfileSnapshots
            | Message::ProfileSnapshotsPruned(_)
            | Message::ArmRestoreProfileSnapshot(_)
            | Message::CancelRestoreProfileSnapshot
            | Message::RestoreProfileSnapshot(_)
            | Message::EditorAction(_)
            | Message::InsertYamlSnippet(_)
            | Message::SaveProfile
            | Message::DiscardProfileDraft
            | Message::ProfileSaved(_)
            | Message::SetEditorPane(_)
            | Message::MixinEditorAction(_)
            | Message::MixinLoaded(_)
            | Message::SaveMixin
            | Message::DiscardMixinDraft
            | Message::MixinSaved(_)
            | Message::ToggleMixinPreset(_, _)
            | Message::LoadProfileFilter
            | Message::ProfileFilterLoaded { .. }
            | Message::UpdateFilterInclude(_)
            | Message::UpdateFilterExclude(_)
            | Message::UpdateFilterExcludeTypes(_)
            | Message::UpdateFilterRenames(_)
            | Message::UpdateFilterAdvancedPolicy(_)
            | Message::UpdateFilterDedup(_)
            | Message::SaveProfileFilter
            | Message::ProfileFilterSaved { .. }
            | Message::DiscardProfileFilter
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
                if self.commands.is_some() {
                    if !self
                        .runtime
                        .proxy_groups
                        .iter()
                        .any(|entry| entry.name == group)
                    {
                        return Task::none();
                    }
                    return self
                        .submit_proxy_preference(CommandIntent::ToggleProxyGroupExpand { group });
                }
                if !self
                    .runtime
                    .proxies
                    .get(&group)
                    .is_some_and(|proxy| proxy.is_group())
                {
                    return Task::none();
                }
                self.runtime
                    .proxy_ui_preferences
                    .toggle_group_expand(&group);
                self.recompute_filtered_groups();
                Task::none()
            }
            _ => self.update_core(message),
        }
    }
}

//! Debug rendering for rules, DNS, settings, sync and kernel messages.
//!
//! Split out of the single `Debug` impl along the message-family seam; the
//! parent `debug` module dispatches to these sections in turn.

use crate::types::message::Message;
use std::fmt;

pub(super) fn fmt(m: &Message, f: &mut fmt::Formatter<'_>) -> Option<fmt::Result> {
    Some(match m {
        Message::LoadRules => write!(f, "LoadRules"),
        Message::RulesBundleLoaded(Ok(bundle)) => write!(
            f,
            "RulesBundleLoaded(Ok({} rules, rp:{} chars, pp:{} chars, sn:{} chars))",
            bundle.rules.len(),
            bundle.rule_providers_json.len(),
            bundle.proxy_providers_json.len(),
            bundle.sniffer_json.len()
        ),
        Message::RulesBundleLoaded(Err(e)) => write!(f, "RulesBundleLoaded(Err({:?}))", e),
        Message::RulesLoaded(Ok(r)) => write!(f, "RulesLoaded(Ok({} rules))", r.len()),
        Message::RulesLoaded(Err(e)) => write!(f, "RulesLoaded(Err({:?}))", e),
        Message::SetRulesTab(tab) => write!(f, "SetRulesTab({:?})", tab),
        Message::SetRulesJsonTab(tab) => write!(f, "SetRulesJsonTab({:?})", tab),
        Message::ToggleRulesProvidersExpanded => write!(f, "ToggleRulesProvidersExpanded"),
        Message::RulesPrevPage => write!(f, "RulesPrevPage"),
        Message::RulesNextPage => write!(f, "RulesNextPage"),
        Message::RulesSetPage(page) => write!(f, "RulesSetPage({})", page),
        Message::RulesListScrolled {
            offset_px,
            viewport_px,
        } => write!(
            f,
            "RulesListScrolled {{ offset_px: {offset_px}, viewport_px: {viewport_px} }}"
        ),
        Message::EnsureRuleProvidersEditorLoaded => {
            write!(f, "EnsureRuleProvidersEditorLoaded")
        }
        Message::EnsureProxyProvidersEditorLoaded => {
            write!(f, "EnsureProxyProvidersEditorLoaded")
        }
        Message::EnsureSnifferEditorLoaded => write!(f, "EnsureSnifferEditorLoaded"),
        Message::ActivateRulesHeavyView => write!(f, "ActivateRulesHeavyView"),
        Message::RuleProvidersJsonLoaded(Ok(json)) => {
            write!(f, "RuleProvidersJsonLoaded(Ok({} chars))", json.len())
        }
        Message::RuleProvidersJsonLoaded(Err(e)) => {
            write!(f, "RuleProvidersJsonLoaded(Err({:?}))", e)
        }
        Message::ProxyProvidersJsonLoaded(Ok(json)) => {
            write!(f, "ProxyProvidersJsonLoaded(Ok({} chars))", json.len())
        }
        Message::ProxyProvidersJsonLoaded(Err(e)) => {
            write!(f, "ProxyProvidersJsonLoaded(Err({:?}))", e)
        }
        Message::SnifferJsonLoaded(Ok(json)) => {
            write!(f, "SnifferJsonLoaded(Ok({} chars))", json.len())
        }
        Message::SnifferJsonLoaded(Err(e)) => write!(f, "SnifferJsonLoaded(Err({:?}))", e),
        Message::LoadProviders => write!(f, "LoadProviders"),
        Message::ProvidersLoaded(Ok((p, r))) => write!(
            f,
            "ProvidersLoaded(Ok({} proxies, {} rules))",
            p.len(),
            r.len()
        ),
        Message::ProvidersLoaded(Err(e)) => write!(f, "ProvidersLoaded(Err({:?}))", e),
        Message::UpdateProxyProvider(p) => write!(f, "UpdateProxyProvider({})", p),
        Message::UpdateRuleProvider(p) => write!(f, "UpdateRuleProvider({})", p),
        Message::FilterRules(s) => write!(f, "FilterRules({})", s),
        Message::UpdateRulesTracerInput(s) => write!(f, "UpdateRulesTracerInput({s})"),
        Message::UpdateTracerSourceIp(s) => write!(f, "UpdateTracerSourceIp({s})"),
        Message::RunRulesTracer => write!(f, "RunRulesTracer"),
        Message::UpdateTracerOverrideTarget(s) => {
            write!(f, "UpdateTracerOverrideTarget({s})")
        }
        Message::ApplyTracerRuleOverride { rule_index } => {
            write!(f, "ApplyTracerRuleOverride({rule_index})")
        }
        Message::TracerRuleOverrideApplied(result) => {
            write!(f, "TracerRuleOverrideApplied({result:?})")
        }
        Message::UpdateFilteredGroups => write!(f, "UpdateFilteredGroups"),
        Message::UpdateNewRuleType(s) => write!(f, "UpdateNewRuleType({})", s),
        Message::UpdateNewRulePayload(s) => write!(f, "UpdateNewRulePayload({})", s),
        Message::UpdateNewRuleTarget(s) => write!(f, "UpdateNewRuleTarget({})", s),
        Message::AddCustomRule => write!(f, "AddCustomRule"),
        Message::RuleAdded(Ok(_)) => write!(f, "RuleAdded(Ok)"),
        Message::RuleAdded(Err(e)) => write!(f, "RuleAdded(Err({:?}))", e),
        Message::ToggleRuleEnabled(index) => write!(f, "ToggleRuleEnabled({})", index),
        Message::MoveRuleUp(index) => write!(f, "MoveRuleUp({})", index),
        Message::MoveRuleDown(index) => write!(f, "MoveRuleDown({})", index),
        Message::SaveRules => write!(f, "SaveRules"),
        Message::ApplyGameRoutingPresets => write!(f, "ApplyGameRoutingPresets"),
        Message::UpdateGeoDatabases => write!(f, "UpdateGeoDatabases"),
        Message::GeoDatabasesUpdated(Ok(_)) => write!(f, "GeoDatabasesUpdated(Ok)"),
        Message::GeoDatabasesUpdated(Err(e)) => write!(f, "GeoDatabasesUpdated(Err({:?}))", e),
        Message::RulesSaved(Ok(_)) => write!(f, "RulesSaved(Ok)"),
        Message::RulesSaved(Err(e)) => write!(f, "RulesSaved(Err({:?}))", e),
        Message::InspectRuleProviderDiff(opt) => write!(f, "InspectRuleProviderDiff({opt:?})"),
        Message::UnpackRuleProvider(name) => write!(f, "UnpackRuleProvider({name})"),
        Message::RuleProviderDiffLoaded(Ok(diff)) => {
            write!(f, "RuleProviderDiffLoaded(Ok({}))", diff.provider_name)
        }
        Message::RuleProviderDiffLoaded(Err(e)) => {
            write!(f, "RuleProviderDiffLoaded(Err({e:?}))")
        }
        Message::RuleProvidersEditorAction(_) => write!(f, "RuleProvidersEditorAction"),
        Message::SaveRuleProvidersJson => write!(f, "SaveRuleProvidersJson"),
        Message::RuleProvidersJsonSaved(Ok(_)) => write!(f, "RuleProvidersJsonSaved(Ok)"),
        Message::RuleProvidersJsonSaved(Err(e)) => {
            write!(f, "RuleProvidersJsonSaved(Err({:?}))", e)
        }
        Message::ProxyProvidersEditorAction(_) => write!(f, "ProxyProvidersEditorAction"),
        Message::SaveProxyProvidersJson => write!(f, "SaveProxyProvidersJson"),
        Message::ProxyProvidersJsonSaved(Ok(_)) => write!(f, "ProxyProvidersJsonSaved(Ok)"),
        Message::ProxyProvidersJsonSaved(Err(e)) => {
            write!(f, "ProxyProvidersJsonSaved(Err({:?}))", e)
        }
        Message::SnifferEditorAction(_) => write!(f, "SnifferEditorAction"),
        Message::SaveSnifferJson => write!(f, "SaveSnifferJson"),
        Message::SnifferJsonSaved(Ok(_)) => write!(f, "SnifferJsonSaved(Ok)"),
        Message::SnifferJsonSaved(Err(e)) => write!(f, "SnifferJsonSaved(Err({:?}))", e),
        Message::LoadAdvancedConfigs => write!(f, "LoadAdvancedConfigs"),
        Message::AdvancedConfigsBundleLoaded(Ok(bundle)) => write!(
            f,
            "AdvancedConfigsBundleLoaded(Ok(dns:{} chars, fake:{} chars, tun:{} chars))",
            bundle.dns_json.len(),
            bundle.fake_ip_json.len(),
            bundle.tun_json.len()
        ),
        Message::AdvancedConfigsBundleLoaded(Err(e)) => {
            write!(f, "AdvancedConfigsBundleLoaded(Err({:?}))", e)
        }
        Message::SetDnsTab(tab) => write!(f, "SetDnsTab({:?})", tab),
        Message::SetAdvancedMode(tab, mode) => {
            write!(f, "SetAdvancedMode({:?}, {:?})", tab, mode)
        }
        Message::RefreshDnsOnly => write!(f, "RefreshDnsOnly"),
        Message::RefreshFakeIpOnly => write!(f, "RefreshFakeIpOnly"),
        Message::RefreshTunOnly => write!(f, "RefreshTunOnly"),
        Message::EnsureDnsEditorLoaded => write!(f, "EnsureDnsEditorLoaded"),
        Message::EnsureFakeIpEditorLoaded => write!(f, "EnsureFakeIpEditorLoaded"),
        Message::EnsureTunEditorLoaded => write!(f, "EnsureTunEditorLoaded"),
        Message::ActivateDnsHeavyView => write!(f, "ActivateDnsHeavyView"),
        Message::DnsConfigJsonLoaded(Ok(json)) => {
            write!(f, "DnsConfigJsonLoaded(Ok({} chars))", json.len())
        }
        Message::DnsConfigJsonLoaded(Err(e)) => write!(f, "DnsConfigJsonLoaded(Err({:?}))", e),
        Message::FakeIpConfigJsonLoaded(Ok(json)) => {
            write!(f, "FakeIpConfigJsonLoaded(Ok({} chars))", json.len())
        }
        Message::FakeIpConfigJsonLoaded(Err(e)) => {
            write!(f, "FakeIpConfigJsonLoaded(Err({:?}))", e)
        }
        Message::TunConfigJsonLoaded(Ok(json)) => {
            write!(f, "TunConfigJsonLoaded(Ok({} chars))", json.len())
        }
        Message::TunConfigJsonLoaded(Err(e)) => write!(f, "TunConfigJsonLoaded(Err({:?}))", e),
        Message::UpdateDnsFormEnable(v) => write!(f, "UpdateDnsFormEnable({})", v),
        Message::UpdateDnsFormBootstrapNameserver(v) => {
            write!(f, "UpdateDnsFormBootstrapNameserver({})", v)
        }
        Message::UpdateDnsFormNameserver(v) => write!(f, "UpdateDnsFormNameserver({})", v),
        Message::UpdateDnsFormFallback(v) => write!(f, "UpdateDnsFormFallback({})", v),
        Message::UpdateDnsFormFallbackGeoip(v) => {
            write!(f, "UpdateDnsFormFallbackGeoip({})", v)
        }
        Message::UpdateDnsFormFallbackGeoipCode(v) => {
            write!(f, "UpdateDnsFormFallbackGeoipCode({})", v)
        }
        Message::UpdateDnsFormFallbackTrigger(v) => {
            write!(f, "UpdateDnsFormFallbackTrigger({})", v)
        }
        Message::UpdateDnsFormEnhancedMode(v) => {
            write!(f, "UpdateDnsFormEnhancedMode({:?})", v)
        }
        Message::UpdateDnsFormFilterMode(v) => write!(f, "UpdateDnsFormFilterMode({:?})", v),
        Message::UpdateDnsFormFakeIpRange(v) => write!(f, "UpdateDnsFormFakeIpRange({})", v),
        Message::UpdateDnsFormFakeIpFilter(v) => write!(f, "UpdateDnsFormFakeIpFilter({})", v),
        Message::UpdateDnsFormIpv6(v) => write!(f, "UpdateDnsFormIpv6({})", v),
        Message::UpdateDnsFormCache(v) => write!(f, "UpdateDnsFormCache({})", v),
        Message::UpdateDnsFormUseHosts(v) => write!(f, "UpdateDnsFormUseHosts({})", v),
        Message::UpdateDnsFormUseSystemHosts(v) => {
            write!(f, "UpdateDnsFormUseSystemHosts({})", v)
        }
        Message::UpdateDnsFormRespectRules(v) => write!(f, "UpdateDnsFormRespectRules({})", v),
        Message::UpdateDnsFormProxyServerNameserver(v) => {
            write!(f, "UpdateDnsFormProxyServerNameserver({})", v)
        }
        Message::UpdateDnsFormDirectNameserver(v) => {
            write!(f, "UpdateDnsFormDirectNameserver({})", v)
        }
        Message::UpdateFakeIpFormRange(v) => write!(f, "UpdateFakeIpFormRange({})", v),
        Message::UpdateFakeIpFormFilter(v) => write!(f, "UpdateFakeIpFormFilter({})", v),
        Message::UpdateFakeIpFormStore(v) => write!(f, "UpdateFakeIpFormStore({})", v),
        Message::UpdateDnsFakeIpQuery(v) => write!(f, "UpdateDnsFakeIpQuery({})", v),
        Message::UpdateDnsHostsAddress(v) => write!(f, "UpdateDnsHostsAddress({})", v),
        Message::UpdateDnsHostsDomain(v) => write!(f, "UpdateDnsHostsDomain({})", v),
        Message::AddDnsHostRow => write!(f, "AddDnsHostRow"),
        Message::RemoveDnsHostRow(i) => write!(f, "RemoveDnsHostRow({})", i),
        Message::SaveDnsHosts => write!(f, "SaveDnsHosts"),
        Message::DnsHostsSaved(Ok(_)) => write!(f, "DnsHostsSaved(Ok)"),
        Message::DnsHostsSaved(Err(error)) => write!(f, "DnsHostsSaved(Err({:?}))", error),
        Message::UpdateTunFormEnable(v) => write!(f, "UpdateTunFormEnable({})", v),
        Message::UpdateTunFormStack(v) => write!(f, "UpdateTunFormStack({})", v),
        Message::UpdateTunFormMtu(v) => write!(f, "UpdateTunFormMtu({})", v),
        Message::UpdateTunFormDnsHijack(v) => write!(f, "UpdateTunFormDnsHijack({})", v),
        Message::UpdateTunFormAutoRoute(v) => write!(f, "UpdateTunFormAutoRoute({})", v),
        Message::UpdateTunFormAutoDetectInterface(v) => {
            write!(f, "UpdateTunFormAutoDetectInterface({})", v)
        }
        Message::UpdateTunFormStrictRoute(v) => write!(f, "UpdateTunFormStrictRoute({})", v),
        Message::DnsConfigEditorAction(_) => write!(f, "DnsConfigEditorAction"),
        Message::FakeIpConfigEditorAction(_) => write!(f, "FakeIpConfigEditorAction"),
        Message::TunConfigEditorAction(_) => write!(f, "TunConfigEditorAction"),
        Message::TickSubUpdate => write!(f, "TickSubUpdate"),
        Message::TickWebDavSync => write!(f, "TickWebDavSync"),
        Message::TickRuntimeRefresh => write!(f, "TickRuntimeRefresh"),
        Message::TickFrame(now) => write!(f, "TickFrame({:?})", now),
        Message::TrayEvent(e) => write!(f, "TrayEvent({:?})", e),
        Message::Exit => write!(f, "Exit"),
        Message::UpdateDnsServer(i, s) => write!(f, "UpdateDnsServer({}, {})", i, s),
        Message::UpdateDnsEnhancedMode(m) => write!(f, "UpdateDnsEnhancedMode({})", m),
        Message::AddDnsServer => write!(f, "AddDnsServer"),
        Message::AddDnsServerTemplate(s) => write!(f, "AddDnsServerTemplate({})", s),
        Message::RemoveDnsServer(i) => write!(f, "RemoveDnsServer({})", i),
        Message::UpdateFallbackDnsServer(i, s) => {
            write!(f, "UpdateFallbackDnsServer({}, {})", i, s)
        }
        Message::AddFallbackDnsServer => write!(f, "AddFallbackDnsServer"),
        Message::RemoveFallbackDnsServer(i) => write!(f, "RemoveFallbackDnsServer({})", i),
        Message::SaveDns => write!(f, "SaveDns"),
        Message::DnsSaved(Ok(_)) => write!(f, "DnsSaved(Ok)"),
        Message::DnsSaved(Err(e)) => write!(f, "DnsSaved(Err({:?}))", e),
        Message::SaveFakeIpConfig => write!(f, "SaveFakeIpConfig"),
        Message::FakeIpConfigSaved(Ok(_)) => write!(f, "FakeIpConfigSaved(Ok)"),
        Message::FakeIpConfigSaved(Err(e)) => write!(f, "FakeIpConfigSaved(Err({:?}))", e),
        Message::SaveTunConfig => write!(f, "SaveTunConfig"),
        Message::TunConfigSaved(Ok(_)) => write!(f, "TunConfigSaved(Ok)"),
        Message::TunConfigSaved(Err(e)) => write!(f, "TunConfigSaved(Err({:?}))", e),
        Message::SetAutostart(b) => write!(f, "SetAutostart({})", b),
        Message::AutostartSet(Ok(_)) => write!(f, "AutostartSet(Ok)"),
        Message::AutostartSet(Err(e)) => write!(f, "AutostartSet(Err({:?}))", e),
        Message::UpdateNotificationsEnabled(b) => {
            write!(f, "UpdateNotificationsEnabled({})", b)
        }
        Message::UpdateCloseToTray(b) => write!(f, "UpdateCloseToTray({b})"),
        Message::UpdateWebDavEnabled(b) => write!(f, "UpdateWebDavEnabled({})", b),
        Message::UpdateWebDavUrl(s) => write!(f, "UpdateWebDavUrl({})", s),
        Message::UpdateWebDavUser(s) => write!(f, "UpdateWebDavUser({})", s),
        Message::UpdateWebDavPass(_) => write!(f, "UpdateWebDavPass(***)"),
        Message::UpdateWebDavSyncInterval(s) => write!(f, "UpdateWebDavSyncInterval({})", s),
        Message::UpdateWebDavSyncOnStartup(b) => write!(f, "UpdateWebDavSyncOnStartup({})", b),
        Message::SaveAppSettings => write!(f, "SaveAppSettings"),
        Message::AppSettingsSaved(Ok(_)) => write!(f, "AppSettingsSaved(Ok)"),
        Message::AppSettingsSaved(Err(e)) => write!(f, "AppSettingsSaved(Err({:?}))", e),
        Message::UpdateEditorPathSetting(s) => write!(f, "UpdateEditorPathSetting({})", s),
        Message::SetLanguage(language) => write!(f, "SetLanguage({})", language),
        Message::SetAdminEnabled(b) => write!(f, "SetAdminEnabled({})", b),
        Message::UpdateAdminPort(s) => write!(f, "UpdateAdminPort({})", s),
        Message::ApplyAdminSettings => write!(f, "ApplyAdminSettings"),
        Message::AdminSettingsSaved(Ok(_)) => write!(f, "AdminSettingsSaved(Ok)"),
        Message::AdminSettingsSaved(Err(e)) => write!(f, "AdminSettingsSaved(Err({:?}))", e),
        Message::AdminServerStarted(Ok(url)) => write!(f, "AdminServerStarted(Ok({}))", url),
        Message::AdminServerStarted(Err(e)) => write!(f, "AdminServerStarted(Err({:?}))", e),
        Message::AdminHostCommand(command) => write!(f, "AdminHostCommand({:?})", command),
        Message::ExternalSettingsLoaded(Ok(_)) => write!(f, "ExternalSettingsLoaded(Ok)"),
        Message::ExternalSettingsLoaded(Err(e)) => {
            write!(f, "ExternalSettingsLoaded(Err({:?}))", e)
        }
        Message::SyncUpload => write!(f, "SyncUpload"),
        Message::SyncDownload => write!(f, "SyncDownload"),
        Message::SyncFinished(Ok(summary)) => write!(
            f,
            "SyncFinished(Ok(uploaded={}, downloaded={}, conflicts={}, active_changed={}))",
            summary.uploaded, summary.downloaded, summary.conflicts, summary.active_profile_changed
        ),
        Message::SyncFinished(Err(e)) => write!(f, "SyncFinished(Err({:?}))", e),
        Message::SyncProgress(progress) => write!(
            f,
            "SyncProgress({}, {}/{})",
            progress.phase, progress.current, progress.total
        ),
        Message::ResolveSyncConflict(profile) => write!(f, "ResolveSyncConflict({})", profile),
        Message::DismissSyncConflict(profile) => write!(f, "DismissSyncConflict({})", profile),
        Message::SyncConflictResolved(Ok(profile)) => {
            write!(f, "SyncConflictResolved(Ok({}))", profile)
        }
        Message::SyncConflictResolved(Err(error)) => {
            write!(f, "SyncConflictResolved(Err({:?}))", error)
        }
        Message::SyncConflictDismissed(Ok(profile)) => {
            write!(f, "SyncConflictDismissed(Ok({}))", profile)
        }
        Message::SyncConflictDismissed(Err(error)) => {
            write!(f, "SyncConflictDismissed(Err({:?}))", error)
        }
        Message::CancelWebDavSync => write!(f, "CancelWebDavSync"),
        Message::TestWebDavConnection => write!(f, "TestWebDavConnection"),
        Message::WebDavConnectionTested(Ok(_)) => write!(f, "WebDavConnectionTested(Ok)"),
        Message::WebDavConnectionTested(Err(error)) => {
            write!(f, "WebDavConnectionTested(Err({:?}))", error)
        }
        Message::SetSystemProxy(b) => write!(f, "SetSystemProxy({})", b),
        Message::UpdateSystemProxyBypass(s) => write!(f, "UpdateSystemProxyBypass({s})"),
        Message::SystemProxySet(Ok(snapshot)) => {
            write!(f, "SystemProxySet(Ok(revision={}))", snapshot.revision)
        }
        Message::SystemProxySet(Err(e)) => write!(f, "SystemProxySet(Err({:?}))", e),
        Message::SystemProxyReconciled(snapshot) => write!(
            f,
            "SystemProxyReconciled(revision={}, repairs={})",
            snapshot.revision, snapshot.repair_count
        ),
        Message::SystemProxyRecoveryFinished(snapshot) => write!(
            f,
            "SystemProxyRecoveryFinished(revision={})",
            snapshot.revision
        ),
        Message::RequestAdminPrivilege => write!(f, "RequestAdminPrivilege"),
        Message::RequestConfirmation(action) => write!(f, "RequestConfirmation({action:?})"),
        Message::ConfirmAction => write!(f, "ConfirmAction"),
        Message::CancelConfirmation => write!(f, "CancelConfirmation"),
        Message::ClearError => write!(f, "ClearError"),
        Message::EditProfile(p) => write!(f, "EditProfile({:?})", p),
        Message::ProfileContentLoaded(Ok((p, _))) => {
            write!(f, "ProfileContentLoaded(Ok({:?}))", p)
        }
        Message::ProfileContentLoaded(Err(e)) => {
            write!(f, "ProfileContentLoaded(Err({:?}))", e)
        }
        Message::LoadProfileSnapshots => write!(f, "LoadProfileSnapshots"),
        Message::ProfileSnapshotsLoaded(Ok(history)) => {
            write!(
                f,
                "ProfileSnapshotsLoaded(Ok({} snapshots))",
                history.entries.len()
            )
        }
        Message::ProfileSnapshotsLoaded(Err(error)) => {
            write!(f, "ProfileSnapshotsLoaded(Err({:?}))", error)
        }
        Message::RestoreProfileSnapshot(path) => {
            write!(f, "RestoreProfileSnapshot({:?})", path)
        }
        Message::ProfileSnapshotRestored(Ok(_)) => write!(f, "ProfileSnapshotRestored(Ok)"),
        Message::ProfileSnapshotRestored(Err(error)) => {
            write!(f, "ProfileSnapshotRestored(Err({:?}))", error)
        }
        Message::EditorAction(_) => write!(f, "EditorAction"),
        Message::SaveProfile => write!(f, "SaveProfile"),
        Message::ProfileSaved(Ok(_)) => write!(f, "ProfileSaved(Ok)"),
        Message::ProfileSaved(Err(e)) => write!(f, "ProfileSaved(Err({:?}))", e),
        Message::OpenConfigDirFinished(Ok(_)) => write!(f, "OpenConfigDirFinished(Ok)"),
        Message::OpenConfigDirFinished(Err(error)) => {
            write!(f, "OpenConfigDirFinished(Err({:?}))", error)
        }
        Message::LoadKernels => write!(f, "LoadKernels"),
        Message::KernelsLoaded(Ok(k)) => write!(f, "KernelsLoaded(Ok({} kernels))", k.len()),
        Message::KernelsLoaded(Err(e)) => write!(f, "KernelsLoaded(Err({:?}))", e),
        Message::CheckCoreUpdate => write!(f, "CheckCoreUpdate"),
        Message::CoreUpdateInfo(Ok(v)) => write!(f, "CoreUpdateInfo(Ok({}))", v),
        Message::CoreUpdateInfo(Err(e)) => write!(f, "CoreUpdateInfo(Err({:?}))", e),
        Message::SetCoreChannel(channel) => write!(f, "SetCoreChannel({})", channel),
        Message::DownloadCore(v) => write!(f, "DownloadCore({})", v),
        Message::CoreDownloadProgress(progress, token) => {
            write!(
                f,
                "CoreDownloadProgress({}/{:?}, {} B/s, token={})",
                progress.downloaded, progress.total, progress.speed_bytes, token
            )
        }
        Message::CoreDownloadFinished(Ok(v), token) => {
            write!(f, "CoreDownloadFinished(Ok({}), token={})", v, token)
        }
        Message::CoreDownloadFinished(Err(e), token) => {
            write!(f, "CoreDownloadFinished(Err({:?}), token={})", e, token)
        }
        Message::CancelCoreDownload => write!(f, "CancelCoreDownload"),
        Message::DeleteKernel(v) => write!(f, "DeleteKernel({})", v),
        Message::SetDefaultKernel(v) => write!(f, "SetDefaultKernel({})", v),
        Message::RollbackCore => write!(f, "RollbackCore"),
        Message::KernelOperationFinished(Ok(_)) => write!(f, "KernelOperationFinished(Ok)"),
        Message::KernelOperationFinished(Err(error)) => {
            write!(f, "KernelOperationFinished(Err({:?}))", error)
        }
        Message::FactoryReset => write!(f, "FactoryReset"),
        Message::FactoryResetFinished(Ok(_)) => write!(f, "FactoryResetFinished(Ok)"),
        Message::FactoryResetFinished(Err(error)) => {
            write!(f, "FactoryResetFinished(Err({:?}))", error)
        }
        Message::OpenConfigDir => write!(f, "OpenConfigDir"),
        Message::FlushFakeIpCache => write!(f, "FlushFakeIpCache"),
        Message::DnsCacheFlushed(Ok(report)) => write!(f, "DnsCacheFlushed(Ok({report:?}))"),
        Message::DnsCacheFlushed(Err(error)) => write!(f, "DnsCacheFlushed(Err({error}))"),
        Message::TestProxyDelay(p) => write!(f, "TestProxyDelay({})", p),
        Message::TestGroupDelay(g) => write!(f, "TestGroupDelay({})", g),
        Message::ProxyTested(p, Ok(d)) => write!(f, "ProxyTested({}, Ok({}ms))", p, d),
        Message::ProxyTested(p, Err(e)) => write!(f, "ProxyTested({}, Err({:?}))", p, e),
        Message::WindowClosed(id) => write!(f, "WindowClosed({:?})", id),
        Message::HideWindow => write!(f, "HideWindow"),
        Message::ShowWindow => write!(f, "ShowWindow"),
        Message::UpdateRuntimeAutoRefresh(v) => write!(f, "UpdateRuntimeAutoRefresh({})", v),
        Message::RuntimePanelSettingsSaved(Ok(_)) => write!(f, "RuntimePanelSettingsSaved(Ok)"),
        Message::RuntimePanelSettingsSaved(Err(e)) => {
            write!(f, "RuntimePanelSettingsSaved(Err({:?}))", e)
        }
        Message::RuntimeRebuildFinished(Ok(_)) => write!(f, "RuntimeRebuildFinished(Ok)"),
        Message::RuntimeRebuildFinished(Err(e)) => {
            write!(f, "RuntimeRebuildFinished(Err({:?}))", e)
        }
        Message::ClearRebuildFlow => write!(f, "ClearRebuildFlow"),
        Message::TogglePerfPanel => write!(f, "TogglePerfPanel"),
        Message::ToggleTheme => write!(f, "ToggleTheme"),
        Message::SetTheme(t) => write!(f, "SetTheme({t})"),
        Message::SystemThemeChanged(prefers_dark) => {
            write!(f, "SystemThemeChanged(prefers_dark={prefers_dark})")
        }
        Message::CycleThemePreference => write!(f, "CycleThemePreference"),
        Message::ShowToast(s, st) => write!(f, "ShowToast({}, {:?})", s, st),
        Message::RemoveToast(id) => write!(f, "RemoveToast({id})"),
        Message::TestAllProxyDelays => write!(f, "TestAllProxyDelays"),
        Message::MoveOverviewCardUp(kind) => write!(f, "MoveOverviewCardUp({kind:?})"),
        Message::MoveOverviewCardDown(kind) => write!(f, "MoveOverviewCardDown({kind:?})"),
        Message::ResetOverviewCardOrder => write!(f, "ResetOverviewCardOrder"),
        Message::SpeedtestScopeUpdated(Ok(snapshot)) => write!(
            f,
            "SpeedtestScopeUpdated(Ok(alive={}, total={}))",
            snapshot.alive_nodes_count(),
            snapshot.node_count()
        ),
        Message::SpeedtestScopeUpdated(Err(e)) => {
            write!(f, "SpeedtestScopeUpdated(Err({:?}))", e)
        }
        Message::CancelSpeedtest => write!(f, "CancelSpeedtest"),
        Message::UpdateSpeedtestTestUrl(url) => {
            write!(f, "UpdateSpeedtestTestUrl({url})")
        }
        Message::AdjustSpeedtestConcurrency(delta) => {
            write!(f, "AdjustSpeedtestConcurrency({delta})")
        }
        Message::OpenSpeedtestDetail => write!(f, "OpenSpeedtestDetail"),
        Message::CloseSpeedtestDetail => write!(f, "CloseSpeedtestDetail"),
        _ => return None,
    })
}

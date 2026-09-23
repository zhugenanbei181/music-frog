//! Debug rendering for wave-3/4/5 panels: PCAP, sub-rules, speedtest, UWP, roaming, PAC, LAN and more.
//!
//! Split out of the single `Debug` impl along the message-family seam; the
//! parent `debug` module dispatches to these sections in turn.

use crate::types::message::Message;
use std::fmt;

pub(super) fn fmt(m: &Message, f: &mut fmt::Formatter<'_>) -> Option<fmt::Result> {
    Some(match m {
        // Wave 2: DNS Leak, Custom Node, Aggregator, Grouping, Snapshot Diff, Hotkeys
        Message::RunDnsLeakProbe => write!(f, "RunDnsLeakProbe"),
        Message::RunDnsLatencyProbe => write!(f, "RunDnsLatencyProbe"),
        Message::DnsLatencyProbed(_) => write!(f, "DnsLatencyProbed"),
        Message::DnsLeakProbed(report) => {
            write!(
                f,
                "DnsLeakProbed({:?})",
                report.as_ref().map(|r| r.conclusion())
            )
        }
        Message::RunStunProbe => write!(f, "RunStunProbe"),
        Message::StunProbed(report) => {
            write!(
                f,
                "StunProbed({:?})",
                report.as_ref().map(|r| r.status.clone())
            )
        }
        Message::OpenCustomNodeModal => write!(f, "OpenCustomNodeModal"),
        Message::CloseCustomNodeModal => write!(f, "CloseCustomNodeModal"),
        Message::UpdateCustomNodeUriInput(u) => {
            write!(f, "UpdateCustomNodeUriInput({} chars)", u.len())
        }
        Message::ParseAndImportCustomUri => write!(f, "ParseAndImportCustomUri"),
        Message::UpdateCustomNodeDraft(draft) => {
            write!(f, "UpdateCustomNodeDraft({})", draft.node_type)
        }
        Message::ExportCustomNodeUri => write!(f, "ExportCustomNodeUri"),
        Message::CustomNodeSaved(Ok(_)) => write!(f, "CustomNodeSaved(Ok)"),
        Message::CustomNodeSaved(Err(e)) => write!(f, "CustomNodeSaved(Err({e:?}))"),
        Message::SaveCustomNodeForm => write!(f, "SaveCustomNodeForm"),
        Message::ScanCustomNodeDialer => write!(f, "ScanCustomNodeDialer"),
        Message::CustomNodeDialerScanned(Ok(report)) => write!(
            f,
            "CustomNodeDialerScanned(Ok(chains={}, loops={}))",
            report.chains.len(),
            report.loops.len()
        ),
        Message::CustomNodeDialerScanned(Err(e)) => {
            write!(f, "CustomNodeDialerScanned(Err({e:?}))")
        }
        Message::VerifyCustomNodeCertificateAuthority => {
            write!(f, "VerifyCustomNodeCertificateAuthority")
        }
        Message::OpenAggregatorModal => write!(f, "OpenAggregatorModal"),
        Message::CloseAggregatorModal => write!(f, "CloseAggregatorModal"),
        Message::ToggleAggregatorProfileSelection(p) => {
            write!(f, "ToggleAggregatorProfileSelection({p})")
        }
        Message::UpdateAggregatorName(n) => write!(f, "UpdateAggregatorName({n})"),
        Message::ToggleAggregatorDeduplicate => write!(f, "ToggleAggregatorDeduplicate"),
        Message::ToggleAggregatorGeoCluster => write!(f, "ToggleAggregatorGeoCluster"),
        Message::ToggleAggregatorGenerateGroups => write!(f, "ToggleAggregatorGenerateGroups"),
        Message::ToggleAggregatorRemoveEmojis => write!(f, "ToggleAggregatorRemoveEmojis"),
        Message::PreviewProfileAggregation => write!(f, "PreviewProfileAggregation"),
        Message::AggregationPreviewFinished(result) => {
            write!(f, "AggregationPreviewFinished({})", result.is_ok())
        }
        Message::CreateAggregatedProfile => write!(f, "CreateAggregatedProfile"),
        Message::AggregatedProfileCreated(result) => {
            write!(f, "AggregatedProfileCreated({})", result.is_ok())
        }
        Message::ToggleAggregatorAvailabilityPrecheck => {
            write!(f, "ToggleAggregatorAvailabilityPrecheck")
        }
        Message::ToggleAggregatorActivateAfterCreate => {
            write!(f, "ToggleAggregatorActivateAfterCreate")
        }
        Message::UpdateAggregatorRenames(text) => {
            write!(f, "UpdateAggregatorRenames({text})")
        }
        Message::UpdateAggregatorCustomGroupName(name) => {
            write!(f, "UpdateAggregatorCustomGroupName({name})")
        }
        Message::UpdateAggregatorCustomGroupKeywords(keywords) => {
            write!(f, "UpdateAggregatorCustomGroupKeywords({keywords})")
        }
        Message::AddAggregatorCustomGroup => write!(f, "AddAggregatorCustomGroup"),
        Message::RemoveAggregatorCustomGroup(index) => {
            write!(f, "RemoveAggregatorCustomGroup({index})")
        }
        Message::LoadAggregatorTemplates => write!(f, "LoadAggregatorTemplates"),
        Message::AggregatorTemplatesLoaded(result) => match result {
            Ok(templates) => write!(f, "AggregatorTemplatesLoaded(Ok({}))", templates.len()),
            Err(error) => write!(f, "AggregatorTemplatesLoaded(Err({:?}))", error),
        },
        Message::ApplyAggregatorTemplate(name) => write!(f, "ApplyAggregatorTemplate({name})"),
        Message::SaveAggregatorTemplate => write!(f, "SaveAggregatorTemplate"),
        Message::UpdateAggregatorTemplateName(name) => {
            write!(f, "UpdateAggregatorTemplateName({name})")
        }
        Message::AggregatorTemplateSaved(result) => match result {
            Ok(name) => write!(f, "AggregatorTemplateSaved(Ok({name}))"),
            Err(error) => write!(f, "AggregatorTemplateSaved(Err({:?}))", error),
        },
        Message::DeleteAggregatorTemplate(name) => {
            write!(f, "DeleteAggregatorTemplate({name})")
        }
        Message::AggregatorTemplateDeleted(result) => match result {
            Ok((name, removed)) => {
                write!(
                    f,
                    "AggregatorTemplateDeleted(Ok({name}, removed={removed}))"
                )
            }
            Err(error) => write!(f, "AggregatorTemplateDeleted(Err({:?}))", error),
        },
        Message::ReAggregateProfile(name) => write!(f, "ReAggregateProfile({name})"),
        Message::AggregationReaggregated(result) => {
            write!(f, "AggregationReaggregated({})", result.is_ok())
        }
        Message::SetConnectionGroupingMode(m) => write!(f, "SetConnectionGroupingMode({m:?})"),
        Message::AddQuickRuleFromConnection { pattern, target } => {
            write!(f, "AddQuickRuleFromConnection({pattern} -> {target})")
        }
        Message::OpenSnapshotDiff(id) => write!(f, "OpenSnapshotDiff({id})"),
        Message::CloseSnapshotDiff => write!(f, "CloseSnapshotDiff"),
        Message::SnapshotDiffLoaded(Ok(diff)) => write!(
            f,
            "SnapshotDiffLoaded(Ok(+{} -{} ~{}))",
            diff.stats.additions, diff.stats.deletions, diff.stats.modifications
        ),
        Message::SnapshotDiffLoaded(Err(error)) => {
            write!(f, "SnapshotDiffLoaded(Err({:?}))", error)
        }
        Message::ArmRestoreProfileSnapshot(path) => {
            write!(f, "ArmRestoreProfileSnapshot({})", path.display())
        }
        Message::CancelRestoreProfileSnapshot => write!(f, "CancelRestoreProfileSnapshot"),
        Message::SetSnapshotDiffMode(mode) => write!(f, "SetSnapshotDiffMode({mode:?})"),
        Message::RefreshSnapshotDiff => write!(f, "RefreshSnapshotDiff"),
        Message::ArmSnapshotRollback => write!(f, "ArmSnapshotRollback"),
        Message::CancelSnapshotRollback => write!(f, "CancelSnapshotRollback"),
        Message::RollbackToSnapshot(id) => write!(f, "RollbackToSnapshot({id})"),
        Message::SetProfileProtectionOverride(allow) => {
            write!(f, "SetProfileProtectionOverride({allow})")
        }
        Message::BeginHotkeyCapture(action) => {
            write!(f, "BeginHotkeyCapture({})", action.id())
        }
        Message::CancelHotkeyCapture => write!(f, "CancelHotkeyCapture"),
        Message::KeyboardChord { key, modifiers } => write!(
            f,
            "KeyboardChord(key={key}, ctrl={}, alt={}, shift={}, meta={})",
            modifiers.ctrl, modifiers.alt, modifiers.shift, modifiers.meta
        ),
        Message::ToggleHotkeyEnabled(action) => {
            write!(f, "ToggleHotkeyEnabled({})", action.id())
        }
        Message::ResetHotkey(action) => write!(f, "ResetHotkey({})", action.id()),
        Message::ShortcutsUpdated(Ok(registry)) => {
            write!(
                f,
                "ShortcutsUpdated(Ok(bindings={}))",
                registry.bindings().len()
            )
        }
        Message::ShortcutsUpdated(Err(error)) => {
            write!(f, "ShortcutsUpdated(Err({error}))")
        }

        // Wave 3: PCAP Exporter, Sub-Rules, Speedtest, GeoData, UWP, Encrypted Backup
        Message::TogglePcapCapture => write!(f, "TogglePcapCapture"),
        Message::ExportPcapBuffer => write!(f, "ExportPcapBuffer"),
        Message::UpdateSubRuleOperator(op) => write!(f, "UpdateSubRuleOperator({op})"),
        Message::AddSubRuleCondition(c) => write!(f, "AddSubRuleCondition({c})"),
        Message::RemoveSubRuleCondition(idx) => write!(f, "RemoveSubRuleCondition({idx})"),
        Message::UpdateSubRuleTarget(t) => write!(f, "UpdateSubRuleTarget({t})"),
        Message::InsertSubRuleIntoRules => write!(f, "InsertSubRuleIntoRules"),
        Message::RunNodeSpeedtest(node) => write!(f, "RunNodeSpeedtest({node})"),
        Message::SpeedtestSnapshotUpdated(Ok(snapshot)) => write!(
            f,
            "SpeedtestSnapshotUpdated(Ok(nodes={}))",
            snapshot.node_results.len()
        ),
        Message::SpeedtestSnapshotUpdated(Err(error)) => {
            write!(f, "SpeedtestSnapshotUpdated(Err({error:?}))")
        }
        Message::CheckGeoDataUpdates => write!(f, "CheckGeoDataUpdates"),
        Message::TriggerGeoDataUpdate => write!(f, "TriggerGeoDataUpdate"),
        Message::GeoDataUpdateResult(result) => {
            write!(f, "GeoDataUpdateResult({result:?})")
        }
        Message::ScanUwpApps => write!(f, "ScanUwpApps"),
        Message::UwpAppsLoaded(apps) => write!(f, "UwpAppsLoaded({} apps)", apps.len()),
        Message::UwpSnapshotLoaded(snapshot) => write!(
            f,
            "UwpSnapshotLoaded({} packages, revision={})",
            snapshot.packages.len(),
            snapshot.revision
        ),
        Message::ExemptAllUwpApps => write!(f, "ExemptAllUwpApps"),
        Message::ClearAllUwpExemptions => write!(f, "ClearAllUwpExemptions"),
        Message::ToggleUwpAppExemption(sid) => write!(f, "ToggleUwpAppExemption({sid})"),
        Message::UwpExemptionsChanged(Ok(snapshot)) => write!(
            f,
            "UwpExemptionsChanged(Ok({} packages, revision={}))",
            snapshot.packages.len(),
            snapshot.revision
        ),
        Message::UwpExemptionsChanged(Err(error)) => {
            write!(f, "UwpExemptionsChanged(Err({:?}))", error)
        }
        Message::UpdateEncryptedBackupPassphrase(_) => {
            write!(f, "UpdateEncryptedBackupPassphrase")
        }
        Message::ExportEncryptedPackage => write!(f, "ExportEncryptedPackage"),
        Message::ImportEncryptedPackage => write!(f, "ImportEncryptedPackage"),

        // Wave 4: Network Roaming, Crash Watchdog, Web Dashboard, Log Regex, Quota, PAC
        Message::PollNetworkInterfaces => write!(f, "PollNetworkInterfaces"),
        Message::NetworkInterfacesPolled(snapshot) => write!(
            f,
            "NetworkInterfacesPolled({} ifaces, revision={})",
            snapshot.interfaces.len(),
            snapshot.revision
        ),
        Message::ForceGatewayReconnect => write!(f, "ForceGatewayReconnect"),
        Message::NetworkRoamingRepaired(Ok(snapshot)) => write!(
            f,
            "NetworkRoamingRepaired(Ok(repairs={}, revision={}))",
            snapshot.route_repair_count, snapshot.revision
        ),
        Message::NetworkRoamingRepaired(Err(error)) => {
            write!(f, "NetworkRoamingRepaired(Err({error:?}))")
        }
        Message::StartVpn => write!(f, "StartVpn"),
        Message::StopVpn => write!(f, "StopVpn"),
        Message::VpnSessionUpdated(Ok(snapshot)) => write!(
            f,
            "VpnSessionUpdated(Ok(state={:?}, revision={}))",
            snapshot.state, snapshot.revision
        ),
        Message::VpnSessionUpdated(Err(error)) => {
            write!(f, "VpnSessionUpdated(Err({error:?}))")
        }
        Message::RunPrivilegedNetworkRegression => {
            write!(f, "RunPrivilegedNetworkRegression")
        }
        Message::PrivilegedNetworkRegressionUpdated(Ok(snapshot)) => write!(
            f,
            "PrivilegedNetworkRegressionUpdated(Ok(state={:?}, revision={}))",
            snapshot.state, snapshot.revision
        ),
        Message::PrivilegedNetworkRegressionUpdated(Err(error)) => {
            write!(f, "PrivilegedNetworkRegressionUpdated(Err({error:?}))")
        }
        Message::CheckCrashWatchdog => write!(f, "CheckCrashWatchdog"),
        Message::RecoverOrphanedState => write!(f, "RecoverOrphanedState"),
        Message::ExportCrashDiagnostics => write!(f, "ExportCrashDiagnostics"),
        Message::LaunchWebDashboard(dash) => write!(f, "LaunchWebDashboard({dash})"),
        Message::UpdateLogRegexFilter(q) => write!(f, "UpdateLogRegexFilter({q})"),
        Message::SetLogLevelFilter(lvl) => write!(f, "SetLogLevelFilter({lvl})"),
        Message::ExportRedactedLogs => write!(f, "ExportRedactedLogs"),
        Message::EvaluateSubscriptionQuota => write!(f, "EvaluateSubscriptionQuota"),
        Message::UpdateCronScheduleHours(h) => write!(f, "UpdateCronScheduleHours({h})"),
        Message::UpdatePacBypassSubnets(s) => write!(f, "UpdatePacBypassSubnets({s})"),
        Message::CompileAndValidatePac => write!(f, "CompileAndValidatePac"),
        Message::PacApplied(Ok(snapshot)) => write!(
            f,
            "PacApplied(Ok(script_bytes={}, revision={}))",
            snapshot.script_bytes, snapshot.revision
        ),
        Message::PacApplied(Err(error)) => write!(f, "PacApplied(Err({error:?}))"),
        Message::TogglePacMode(on) => write!(f, "TogglePacMode({on})"),
        Message::AuditStaleRules => write!(f, "AuditStaleRules"),
        Message::DisableZeroHitRules => write!(f, "DisableZeroHitRules"),
        Message::ClearRuleHitCounters => write!(f, "ClearRuleHitCounters"),
        Message::SelectRadarNode(n) => write!(f, "SelectRadarNode({n})"),
        Message::RecordRadarLatencySample { node, latency_ms } => {
            write!(f, "RecordRadarLatencySample({node}: {latency_ms}ms)")
        }
        Message::SelectTunStack(s) => write!(f, "SelectTunStack({s})"),
        Message::ProbeOptimalMtu => write!(f, "ProbeOptimalMtu"),
        Message::MtuProbed(mtu) => write!(f, "MtuProbed({mtu})"),
        Message::MtuProbeFinished(completion) => {
            write!(
                f,
                "MtuProbeFinished(revision={})",
                completion.snapshot.revision
            )
        }
        Message::UnpackRuleProviderToCustom(p) => write!(f, "UnpackRuleProviderToCustom({p})"),
        Message::PurgeRuleProviderCache => write!(f, "PurgeRuleProviderCache"),
        Message::RuleProviderUnpacked(Ok(plan)) => write!(
            f,
            "RuleProviderUnpacked({} imported {} rules from {})",
            plan.provider_name,
            plan.imported(),
            plan.origin.as_str()
        ),
        Message::RuleProviderUnpacked(Err(error)) => {
            write!(f, "RuleProviderUnpacked(Err({error}))")
        }
        Message::RuleProviderCachePurged(Ok(purge)) => write!(
            f,
            "RuleProviderCachePurged({} files, {} bytes)",
            purge.files_removed, purge.bytes_freed
        ),
        Message::RuleProviderCachePurged(Err(error)) => {
            write!(f, "RuleProviderCachePurged(Err({error}))")
        }
        Message::TriggerAtomicConfigApply => write!(f, "TriggerAtomicConfigApply"),
        Message::ApplyTransactionStageChanged(st) => {
            write!(f, "ApplyTransactionStageChanged({st:?})")
        }
        Message::ToggleLanSharing(on) => write!(f, "ToggleLanSharing({on})"),
        Message::UpdateLanSharingPort(p) => write!(f, "UpdateLanSharingPort({p})"),
        Message::UpdateLanBindAddress(address) => {
            write!(f, "UpdateLanBindAddress({address})")
        }
        Message::UpdateLanAclWhitelist(w) => write!(f, "UpdateLanAclWhitelist({w})"),
        Message::ApplyLanSharing => write!(f, "ApplyLanSharing"),
        Message::LanSharingSet(Ok(snapshot), generation) => write!(
            f,
            "LanSharingSet(Ok(revision={}, generation={generation}))",
            snapshot.revision
        ),
        Message::LanSharingSet(Err(error), generation) => {
            write!(f, "LanSharingSet(Err({error:?}), generation={generation})")
        }
        Message::UpdateLanAllowedIps(value) => write!(f, "UpdateLanAllowedIps({value})"),
        Message::UpdateLanDisallowedIps(value) => {
            write!(f, "UpdateLanDisallowedIps({value})")
        }
        Message::UpdateLanSkipAuthPrefixes(value) => {
            write!(f, "UpdateLanSkipAuthPrefixes({value})")
        }
        Message::ToggleLanAuthentication(enabled) => {
            write!(f, "ToggleLanAuthentication({enabled})")
        }
        Message::UpdateLanAuthUsername(username) => {
            write!(f, "UpdateLanAuthUsername({username})")
        }
        Message::UpdateLanAuthPassword(_) => write!(f, "UpdateLanAuthPassword(<redacted>)"),
        Message::ApplyLanSecurity => write!(f, "ApplyLanSecurity"),
        Message::LanSecuritySet(Ok(snapshot), generation) => write!(
            f,
            "LanSecuritySet(Ok(revision={}, auth_users={}, generation={generation}))",
            snapshot.revision, snapshot.authentication_user_count
        ),
        Message::LanSecuritySet(Err(error), generation) => {
            write!(f, "LanSecuritySet(Err({error:?}), generation={generation})")
        }
        _ => return None,
    })
}

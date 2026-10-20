//! Debug rendering for profile, subscription, proxy and runtime messages.
//!
//! Split out of the single `Debug` impl along the message-family seam; the
//! parent `debug` module dispatches to these sections in turn.

use crate::types::message::Message;
use infiltrator_ports::runtime_configuration::ObservedValue;
use std::fmt;

pub(super) fn fmt(m: &Message, f: &mut fmt::Formatter<'_>) -> Option<fmt::Result> {
    Some(match m {
        Message::Noop => write!(f, "Noop"),
        Message::SurfaceSnapshotUpdated(snapshot) => write!(
            f,
            "SurfaceSnapshotUpdated(revision={}, generation={})",
            snapshot.revision, snapshot.generation
        ),
        Message::Navigate(route) => write!(f, "Navigate({:?})", route),
        Message::WindowResized(w, h) => write!(f, "WindowResized({w}x{h})"),
        Message::WindowFocusChanged(focused) => {
            write!(f, "WindowFocusChanged({focused})")
        }
        Message::ImeComposition(event) => {
            write!(f, "ImeComposition({event:?})")
        }
        Message::NavigateBack => write!(f, "NavigateBack"),
        Message::NavigateForward => write!(f, "NavigateForward"),
        Message::StartCore => write!(f, "StartCore"),
        Message::StopCore => write!(f, "StopCore"),
        Message::ProbeInspectedOutbound => write!(f, "ProbeInspectedOutbound"),
        Message::StartOutboundProbe { scope, request } => {
            write!(
                f,
                "StartOutboundProbe(scope={scope:?}, request={request:?})"
            )
        }
        Message::StartOutboundProbeBatch {
            scope,
            target_scope,
            expected_inventory_revision,
        } => write!(
            f,
            "StartOutboundProbeBatch(scope={scope:?}, target={target_scope:?}, revision={expected_inventory_revision:?})"
        ),
        Message::OutboundProbeFinished {
            token,
            scope,
            request,
            terminal_scope,
            result,
        } => write!(
            f,
            "OutboundProbeFinished(token={token}, scope={scope:?}, request={request:?}, terminal={terminal_scope:?}, {result:?})"
        ),
        Message::OutboundProbeBatchFinished {
            token,
            scope,
            request,
            terminal_scope,
            result,
        } => write!(
            f,
            "OutboundProbeBatchFinished(token={token}, scope={scope:?}, request={request:?}, terminal={terminal_scope:?}, {result:?})"
        ),
        Message::DismissOutboundProbeBatch => write!(f, "DismissOutboundProbeBatch"),
        Message::CoreStopped => write!(f, "CoreStopped"),
        Message::CoreControlFinished {
            action,
            token,
            result,
            ..
        } => {
            write!(
                f,
                "CoreControlFinished({action:?}, {result:?}, token={token})"
            )
        }
        Message::SettingsLoaded(Ok(_)) => write!(f, "SettingsLoaded(Ok)"),
        Message::SettingsLoaded(Err(e)) => write!(f, "SettingsLoaded(Err({:?}))", e),
        Message::LoadProfiles => write!(f, "LoadProfiles"),
        Message::ProfilesLoaded(Ok(p)) => write!(f, "ProfilesLoaded(Ok({} profiles))", p.len()),
        Message::ProfilesLoaded(Err(e)) => write!(f, "ProfilesLoaded(Err({:?}))", e),
        Message::SetActiveProfile(name) => write!(f, "SetActiveProfile({})", name),
        Message::ProfileActivationFinished(Ok(outcome)) => {
            write!(f, "ProfileActivationFinished(Ok({outcome:?}))")
        }
        Message::ProfileActivationFinished(Err(error)) => {
            write!(f, "ProfileActivationFinished(Err({:?}))", error)
        }
        Message::UpdateImportUrl(url) => write!(f, "UpdateImportUrl({})", url),
        Message::UpdateImportName(name) => write!(f, "UpdateImportName({})", name),
        Message::UpdateImportActivate(enabled) => {
            write!(f, "UpdateImportActivate({})", enabled)
        }
        Message::ImportProfile => write!(f, "ImportProfile"),
        Message::ProfileImported(Ok(Some(outcome))) => {
            write!(f, "ProfileImported(Ok({outcome:?}))")
        }
        Message::ProfileImported(Ok(None)) => write!(f, "ProfileImported(Ok(no activation))"),
        Message::ProfileImported(Err(e)) => write!(f, "ProfileImported(Err({:?}))", e),
        Message::DeleteProfile(name) => write!(f, "DeleteProfile({})", name),
        Message::ProfileDeleted(Ok(_)) => write!(f, "ProfileDeleted(Ok)"),
        Message::ProfileDeleted(Err(e)) => write!(f, "ProfileDeleted(Err({:?}))", e),
        Message::UpdateLocalImportPath(path) => write!(f, "UpdateLocalImportPath({})", path),
        Message::BrowseLocalImportFile => write!(f, "BrowseLocalImportFile"),
        Message::LocalImportFilePicked(Some(path)) => {
            write!(f, "LocalImportFilePicked(Some({:?}))", path)
        }
        Message::LocalImportFilePicked(None) => write!(f, "LocalImportFilePicked(None)"),
        Message::UpdateLocalImportName(name) => write!(f, "UpdateLocalImportName({})", name),
        Message::UpdateLocalImportActivate(enabled) => {
            write!(f, "UpdateLocalImportActivate({})", enabled)
        }
        Message::ImportLocalProfile => write!(f, "ImportLocalProfile"),
        Message::LocalProfileImported(Ok(Some(outcome))) => {
            write!(f, "LocalProfileImported(Ok({outcome:?}))")
        }
        Message::LocalProfileImported(Ok(None)) => {
            write!(f, "LocalProfileImported(Ok(no activation))")
        }
        Message::LocalProfileImported(Err(e)) => {
            write!(f, "LocalProfileImported(Err({:?}))", e)
        }
        Message::SelectSubscriptionProfile(name) => {
            write!(f, "SelectSubscriptionProfile({})", name)
        }
        Message::UpdateSubscriptionUrl(url) => write!(f, "UpdateSubscriptionUrl({})", url),
        Message::UpdateSubscriptionAutoUpdate(enabled) => {
            write!(f, "UpdateSubscriptionAutoUpdate({})", enabled)
        }
        Message::UpdateSubscriptionInterval(v) => {
            write!(f, "UpdateSubscriptionInterval({})", v)
        }
        Message::UpdateSubscriptionCron(v) => {
            write!(f, "UpdateSubscriptionCron({})", v)
        }
        Message::UpdateSubscriptionUserAgent(v) => {
            write!(f, "UpdateSubscriptionUserAgent({})", v)
        }
        Message::UpdateSubscriptionAutoReload(v) => {
            write!(f, "UpdateSubscriptionAutoReload({})", v)
        }
        Message::UpdateSubscriptionInsecureSkipVerify(v) => {
            write!(f, "UpdateSubscriptionInsecureSkipVerify({})", v)
        }
        Message::SaveSubscriptionSettings => write!(f, "SaveSubscriptionSettings"),
        Message::SubscriptionSettingsSaved(Ok(_)) => write!(f, "SubscriptionSettingsSaved(Ok)"),
        Message::SubscriptionSettingsSaved(Err(e)) => {
            write!(f, "SubscriptionSettingsSaved(Err({:?}))", e)
        }
        Message::UpdateSubscriptionNow => write!(f, "UpdateSubscriptionNow"),
        Message::SubscriptionUpdatedNow(Ok(report)) => {
            write!(
                f,
                "SubscriptionUpdatedNow(Ok(profile={}))",
                report.profile_name
            )
        }
        Message::SubscriptionUpdatedNow(Err(e)) => {
            write!(f, "SubscriptionUpdatedNow(Err({:?}))", e)
        }
        Message::SubscriptionAutoUpdated(Ok((names, active_updated))) => write!(
            f,
            "SubscriptionAutoUpdated(Ok({} profiles, active_updated={}))",
            names.len(),
            active_updated
        ),
        Message::SubscriptionAutoUpdated(Err(e)) => {
            write!(f, "SubscriptionAutoUpdated(Err({:?}))", e)
        }
        Message::UpdateAllSubscriptionsNow => write!(f, "UpdateAllSubscriptionsNow"),
        Message::AllSubscriptionsUpdated(Ok(report)) => write!(
            f,
            "AllSubscriptionsUpdated(Ok({} profiles, {} updated, {} failed, {} skipped))",
            report.total, report.updated, report.failed, report.skipped
        ),
        Message::AllSubscriptionsUpdated(Err(e)) => {
            write!(f, "AllSubscriptionsUpdated(Err({:?}))", e)
        }
        Message::RestoreSubscriptionBackup => write!(f, "RestoreSubscriptionBackup"),
        Message::SubscriptionBackupRestored(Ok(restored)) => {
            write!(f, "SubscriptionBackupRestored(Ok({restored}))")
        }
        Message::SubscriptionBackupRestored(Err(e)) => {
            write!(f, "SubscriptionBackupRestored(Err({:?}))", e)
        }
        Message::SetProfileAutoUpdate { name, enabled } => {
            write!(f, "SetProfileAutoUpdate({name}, {enabled})")
        }
        Message::ProfileAutoUpdateSet(Ok(name)) => {
            write!(f, "ProfileAutoUpdateSet(Ok({}))", name)
        }
        Message::ProfileAutoUpdateSet(Err(e)) => {
            write!(f, "ProfileAutoUpdateSet(Err({:?}))", e)
        }
        Message::UpdateProfilesFilter(s) => write!(f, "UpdateProfilesFilter({})", s),
        Message::ClearProfiles => write!(f, "ClearProfiles"),
        Message::ProfilesCleared(Ok(_)) => write!(f, "ProfilesCleared(Ok)"),
        Message::ProfilesCleared(Err(e)) => write!(f, "ProfilesCleared(Err({:?}))", e),
        Message::RefreshOutboundInventory => write!(f, "RefreshOutboundInventory"),
        Message::SelectOutbound { scope, request } => {
            write!(f, "SelectOutbound(scope={scope:?}, request={request:?})")
        }
        Message::PrepareRuntimeOutboundSelection { scope, request } => write!(
            f,
            "PrepareRuntimeOutboundSelection(scope={scope:?}, request={request:?})"
        ),
        Message::OutboundUiPreferenceFinished {
            token,
            scope,
            request,
            terminal_scope,
            result,
        } => write!(
            f,
            "OutboundUiPreferenceFinished(token={token}, scope={scope:?}, request={request:?}, terminal={terminal_scope:?}, {result:?})"
        ),
        Message::FilterProxies(s) => write!(f, "FilterProxies({})", s),
        Message::ToggleFilterAlive(v) => write!(f, "ToggleFilterAlive({v})"),
        Message::ToggleProxyCompactView => write!(f, "ToggleProxyCompactView"),
        Message::InspectOutbound(id) => write!(f, "InspectOutbound({id:?})"),
        Message::OpenAddCustomNodeModal(b) => write!(f, "OpenAddCustomNodeModal({b})"),
        Message::UpdateNewNodeType(s) => write!(f, "UpdateNewNodeType({s})"),
        Message::UpdateNewNodeName(s) => write!(f, "UpdateNewNodeName({s})"),
        Message::UpdateNewNodeServer(s) => write!(f, "UpdateNewNodeServer({s})"),
        Message::UpdateNewNodePort(s) => write!(f, "UpdateNewNodePort({s})"),
        Message::UpdateNewNodeCredential(s) => write!(f, "UpdateNewNodeCredential({s})"),
        Message::UpdateNewNodeCipher(s) => write!(f, "UpdateNewNodeCipher({s})"),
        Message::UpdateNewNodeTls(b) => write!(f, "UpdateNewNodeTls({b})"),
        Message::SubmitAddCustomNode => write!(f, "SubmitAddCustomNode"),
        Message::CustomNodeAdded(Ok(_)) => write!(f, "CustomNodeAdded(Ok)"),
        Message::CustomNodeAdded(Err(e)) => write!(f, "CustomNodeAdded(Err({:?}))", e),
        Message::SetOutboundSortOrder(order) => write!(f, "SetOutboundSortOrder({order:?})"),
        Message::SetRuntimePanelDelaySort(order) => {
            write!(f, "SetRuntimePanelDelaySort({order:?})")
        }
        Message::UpdateRuntimeProbeUrlDraft(s) => write!(f, "UpdateRuntimeProbeUrlDraft({})", s),
        Message::UpdateRuntimeProbeTimeoutDraft(s) => {
            write!(f, "UpdateRuntimeProbeTimeoutDraft({})", s)
        }
        Message::UpdateRuntimeSelectedGroup(s) => {
            write!(f, "UpdateRuntimeSelectedGroup({})", s)
        }
        Message::ApplyRuntimeSelectedProxy => write!(f, "ApplyRuntimeSelectedProxy"),
        Message::UpdateRuntimeConnectionFilter(s) => {
            write!(f, "UpdateRuntimeConnectionFilter({})", s)
        }
        Message::UpdateRuntimeConnectionSort(s) => {
            write!(f, "UpdateRuntimeConnectionSort({:?})", s)
        }
        Message::RefreshRuntimeNow => write!(f, "RefreshRuntimeNow"),
        Message::TrafficReceived(t) => {
            write!(f, "TrafficReceived(up: {}, down: {})", t.up, t.down)
        }
        Message::IpInfoReceived(Ok(result), id) => {
            write!(
                f,
                "IpInfoReceived(Ok(ip={}, provider={}), taskId: {})",
                result.ip, result.provider, id
            )
        }
        Message::IpInfoReceived(Err(e), id) => {
            write!(f, "IpInfoReceived(Err({:?}), taskId: {})", e, id)
        }
        Message::ConnectionsReceived(c) => write!(
            f,
            "ConnectionsReceived({} connections)",
            c.connections.len()
        ),
        Message::LogReceived(l) => write!(f, "LogReceived({})", l),
        Message::RuntimeStreamLogReceived(scope, _) => {
            write!(f, "RuntimeStreamLogReceived(scope={scope:?})")
        }
        Message::RuntimeStreamTrafficReceived(scope, data) => write!(
            f,
            "RuntimeStreamTrafficReceived(scope={scope:?}, up={}, down={})",
            data.up, data.down
        ),
        Message::RuntimeStreamConnectionsReceived(scope, snapshot) => write!(
            f,
            "RuntimeStreamConnectionsReceived(scope={scope:?}, connections={})",
            snapshot.connections.len()
        ),
        Message::RuntimeStreamStateChanged { kind, scope, state } => write!(
            f,
            "RuntimeStreamStateChanged({kind:?}, scope={scope:?}, state={state:?})"
        ),
        Message::LogsCommandFinished { generation, result } => write!(
            f,
            "LogsCommandFinished(generation={generation}, result={result:?})"
        ),
        Message::ToggleLogFollow => write!(f, "ToggleLogFollow"),
        Message::LogsScrolled {
            offset,
            content,
            viewport,
        } => write!(
            f,
            "LogsScrolled(offset={offset}, content={content}, viewport={viewport})"
        ),
        Message::ClearRuntimeLogs => write!(f, "ClearRuntimeLogs"),
        Message::SetCoreLogLevel(l) => write!(f, "SetCoreLogLevel({})", l),
        Message::CoreLogLevelFinished(Ok(_), previous) => {
            write!(f, "CoreLogLevelFinished(Ok, previous={})", previous)
        }
        Message::CoreLogLevelFinished(Err(e), previous) => write!(
            f,
            "CoreLogLevelFinished(Err({:?}), previous={})",
            e, previous
        ),
        Message::CloseConnection(id) => write!(f, "CloseConnection({})", id),
        Message::CloseAllConnections => write!(f, "CloseAllConnections"),
        Message::CloseFilteredConnections => write!(f, "CloseFilteredConnections"),
        Message::SetConnectionIdleTimeout(secs) => {
            write!(f, "SetConnectionIdleTimeout({secs}s)")
        }
        Message::SweepIdleConnections => write!(f, "SweepIdleConnections"),
        Message::ConnectionsPrevPage => write!(f, "ConnectionsPrevPage"),
        Message::ConnectionsNextPage => write!(f, "ConnectionsNextPage"),
        Message::FetchRuntimeConfig => write!(f, "FetchRuntimeConfig"),
        Message::FetchIpInfo => write!(f, "FetchIpInfo"),
        Message::RuntimeConfigFetched(Ok(config), generation) => {
            let tun = match &config.tun.value.enabled {
                ObservedValue::Unreported => "unknown",
                ObservedValue::Reported(true) => "enabled",
                ObservedValue::Reported(false) => "disabled",
            };
            let dns_observed = matches!(&config.dns.value.nameserver, ObservedValue::Reported(_));
            write!(
                f,
                "RuntimeConfigFetched(gen={generation}, tun={tun}, DNS observed={dns_observed})"
            )
        }
        Message::RuntimeConfigFetched(Err(e), generation) => {
            write!(
                f,
                "RuntimeConfigFetched(Err({:?}), generation={})",
                e, generation
            )
        }
        Message::SetRoutingIntent(intent) => {
            write!(f, "SetRoutingIntent({intent:?})")
        }
        Message::SetIpv6Routing(enabled) => write!(f, "SetIpv6Routing({enabled})"),
        Message::SetTunEnabled(t) => write!(f, "SetTunEnabled({})", t),
        Message::InstallTunService => write!(f, "InstallTunService"),
        Message::RefreshTunServiceStatus => write!(f, "RefreshTunServiceStatus"),
        Message::TunServiceStatusLoaded(Ok(status)) => {
            write!(f, "TunServiceStatusLoaded(Ok({status:?}))")
        }
        Message::TunServiceStatusLoaded(Err(error)) => {
            write!(f, "TunServiceStatusLoaded(Err({:?}))", error)
        }
        Message::TunServiceInstalled(Ok(_)) => write!(f, "TunServiceInstalled(Ok)"),
        Message::TunServiceInstalled(Err(error)) => {
            write!(f, "TunServiceInstalled(Err({:?}))", error)
        }
        Message::ServiceModePrepared(Ok(snapshot)) => {
            write!(f, "ServiceModePrepared(Ok({:?}))", snapshot)
        }
        Message::ServiceModePrepared(Err(error)) => {
            write!(f, "ServiceModePrepared(Err({:?}))", error)
        }
        Message::RepairPortConflicts => write!(f, "RepairPortConflicts"),
        Message::PortConflictsRepaired(Ok(snapshot)) => {
            write!(f, "PortConflictsRepaired(Ok({:?}))", snapshot)
        }
        Message::PortConflictsRepaired(Err(error)) => {
            write!(f, "PortConflictsRepaired(Err({:?}))", error)
        }
        Message::SetTunStack(s) => write!(f, "SetTunStack({})", s),
        Message::SetTunAutoRoute(a) => write!(f, "SetTunAutoRoute({})", a),
        Message::SetTunStrictRoute(s) => write!(f, "SetTunStrictRoute({})", s),
        Message::SetSnifferEnabled(s) => write!(f, "SetSnifferEnabled({})", s),
        Message::RoutingIntentFinished { request, result } => write!(
            f,
            "RoutingIntentFinished({request:?}, success={})",
            result.is_ok()
        ),
        Message::RetryRoutingIntent => write!(f, "RetryRoutingIntent"),
        Message::DismissRoutingIntentFailure => write!(f, "DismissRoutingIntentFailure"),
        Message::RuntimePatchResult(Ok(_), token, generation) => write!(
            f,
            "RuntimePatchResult(Ok, token={token}, generation={generation})"
        ),
        Message::RuntimePatchResult(Err(error), token, generation) => write!(
            f,
            "RuntimePatchResult(Err({:?}), token={token}, generation={generation})",
            error
        ),
        Message::OperationResult(Ok(_)) => write!(f, "OperationResult(Ok)"),
        Message::OperationResult(Err(e)) => write!(f, "OperationResult(Err({:?}))", e),
        _ => return None,
    })
}

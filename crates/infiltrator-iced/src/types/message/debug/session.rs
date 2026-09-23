//! Debug rendering for profile, subscription, proxy and runtime messages.
//!
//! Split out of the single `Debug` impl along the message-family seam; the
//! parent `debug` module dispatches to these sections in turn.

use crate::types::message::Message;
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
        Message::StartProxy => write!(f, "StartProxy"),
        Message::StopProxy => write!(f, "StopProxy"),
        Message::ProxyStarted(Ok(_), token) => write!(f, "ProxyStarted(Ok, token={token})"),
        Message::ProxyStarted(Err(e), token) => {
            write!(f, "ProxyStarted(Err({:?}), token={token})", e)
        }
        Message::ProxyStopped => write!(f, "ProxyStopped"),
        Message::SettingsLoaded(Ok(_)) => write!(f, "SettingsLoaded(Ok)"),
        Message::SettingsLoaded(Err(e)) => write!(f, "SettingsLoaded(Err({:?}))", e),
        Message::LoadProfiles => write!(f, "LoadProfiles"),
        Message::ProfilesLoaded(Ok(p)) => write!(f, "ProfilesLoaded(Ok({} profiles))", p.len()),
        Message::ProfilesLoaded(Err(e)) => write!(f, "ProfilesLoaded(Err({:?}))", e),
        Message::SetActiveProfile(name) => write!(f, "SetActiveProfile({})", name),
        Message::ProfileActivationFinished(Ok(reloaded)) => {
            write!(f, "ProfileActivationFinished(Ok(reloaded={}))", reloaded)
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
        Message::ProfileImported(Ok(reloaded)) => {
            write!(f, "ProfileImported(Ok(reloaded={}))", reloaded)
        }
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
        Message::LocalProfileImported(Ok(reloaded)) => {
            write!(f, "LocalProfileImported(Ok(reloaded={}))", reloaded)
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
        Message::LoadProxies => write!(f, "LoadProxies"),
        Message::ProxiesLoaded(Ok(p)) => write!(f, "ProxiesLoaded(Ok({} proxies))", p.len()),
        Message::ProxiesLoaded(Err(e)) => write!(f, "ProxiesLoaded(Err({:?}))", e),
        Message::SelectProxy(g, n) => write!(f, "SelectProxy({}, {})", g, n),
        Message::FilterProxies(s) => write!(f, "FilterProxies({})", s),
        Message::ToggleFilterAlive(v) => write!(f, "ToggleFilterAlive({v})"),
        Message::ToggleFavoriteProxy(p) => write!(f, "ToggleFavoriteProxy({p})"),
        Message::ToggleProxyCompactView => write!(f, "ToggleProxyCompactView"),
        Message::InspectProxy(p) => write!(f, "InspectProxy({p:?})"),
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
        Message::ToggleProxySort => write!(f, "ToggleProxySort"),
        Message::UpdateProxyDelaySort(s) => write!(f, "UpdateProxyDelaySort({})", s),
        Message::UpdateDelayTestUrl(s) => write!(f, "UpdateDelayTestUrl({})", s),
        Message::UpdateDelayTimeoutMs(s) => write!(f, "UpdateDelayTimeoutMs({})", s),
        Message::UpdateRuntimeSelectedGroup(s) => {
            write!(f, "UpdateRuntimeSelectedGroup({})", s)
        }
        Message::UpdateRuntimeSelectedProxy(s) => {
            write!(f, "UpdateRuntimeSelectedProxy({})", s)
        }
        Message::ApplyRuntimeSelectedProxy => write!(f, "ApplyRuntimeSelectedProxy"),
        Message::UpdateRuntimeConnectionFilter(s) => {
            write!(f, "UpdateRuntimeConnectionFilter({})", s)
        }
        Message::UpdateRuntimeConnectionSort(s) => {
            write!(f, "UpdateRuntimeConnectionSort({})", s)
        }
        Message::RefreshRuntimeNow => write!(f, "RefreshRuntimeNow"),
        Message::TrafficReceived(t) => {
            write!(f, "TrafficReceived(up: {}, down: {})", t.up, t.down)
        }
        Message::MemoryReceived(m) => write!(
            f,
            "MemoryReceived(in_use: {}, os_limit: {})",
            m.in_use, m.os_limit
        ),
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
        Message::RuntimeStreamLogReceived(generation, _) => {
            write!(f, "RuntimeStreamLogReceived(generation={generation})")
        }
        Message::RuntimeStreamTrafficReceived(generation, data) => write!(
            f,
            "RuntimeStreamTrafficReceived(generation={}, up={}, down={})",
            generation, data.up, data.down
        ),
        Message::RuntimeStreamConnectionsReceived(generation, snapshot) => write!(
            f,
            "RuntimeStreamConnectionsReceived(generation={}, connections={})",
            generation,
            snapshot.connections.len()
        ),
        Message::RuntimeStreamStateChanged {
            kind,
            generation,
            state,
        } => write!(
            f,
            "RuntimeStreamStateChanged({kind:?}, generation={generation}, state={state:?})"
        ),
        Message::RuntimePollFailed(error) => write!(f, "RuntimePollFailed({error})"),
        Message::ClearRuntimeLogs => write!(f, "ClearRuntimeLogs"),
        Message::SetLogLevel(l) => write!(f, "SetLogLevel({})", l),
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
            write!(
                f,
                "RuntimeConfigFetched(gen={}, {}, {}, {} DNS, {} FB, {}, {}, {}, {}, {})",
                generation,
                config.mode,
                config.tun_enabled,
                config.dns_nameservers.len(),
                config.dns_fallback.len(),
                config.dns_enhanced_mode,
                config.tun_stack,
                config.tun_auto_route,
                config.tun_strict_route,
                config.sniffer_enabled
            )
        }
        Message::RuntimeConfigFetched(Err(e), generation) => {
            write!(
                f,
                "RuntimeConfigFetched(Err({:?}), generation={})",
                e, generation
            )
        }
        Message::SetProxyMode(m) => write!(f, "SetProxyMode({})", m),
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
        Message::ModeSetResult(Ok(_)) => write!(f, "ModeSetResult(Ok)"),
        Message::ModeSetResult(Err(e)) => write!(f, "ModeSetResult(Err({:?}))", e),
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

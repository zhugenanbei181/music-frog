//! Exhaustive classification of product commands before command dispatch.

use crate::core_capability_application::{CoreCapabilityRequirement, CoreCommandEffect};
use infiltrator_contract::capability::Capability;
use infiltrator_contract::command::{CommandIntent, CommandResult};
use infiltrator_contract::core_capabilities::{
    ConnectionOperation, CoreGarbageCollectionOperation, CoreLifecycleOperation, CoreOperation,
    DnsOperation, FakeIpCacheOperation, GeoOperation, LogsOperation, OutboundOperation,
    ProfileOperation, ProviderOperation, ProxyControlOperation, RulesOperation,
    SubscriptionOperation, TunOperation, VersionOperation,
};
use infiltrator_contract::core_identity::{CoreCommandOutcome, CoreScope};
use infiltrator_contract::error::{ErrorCode, Failure};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandCapabilityRoute {
    CoreBackend(CoreCapabilityRequirement),
    LifecycleOwned(CoreCapabilityRequirement),
    HostOnly(Capability),
    LocalOnly,
    LifecycleChild,
}

/// How much the terminal core scope may move after a command has been
/// accepted. Configuration operations can restart or reload the same core
/// instance; live controller operations must leave its observed scope stable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreTerminalScopePolicy {
    Stable,
    SameInstanceMayAdvance,
    SameInstanceMayStop,
}

/// Return the allowed terminal movement for an intent.
pub fn terminal_scope_policy(intent: &CommandIntent) -> CoreTerminalScopePolicy {
    use CoreTerminalScopePolicy::{SameInstanceMayAdvance, SameInstanceMayStop, Stable};

    match intent {
        CommandIntent::StartCore
        | CommandIntent::RestartCore
        | CommandIntent::ResumeKernel
        | CommandIntent::RestartKernel
        | CommandIntent::StartProxy
        | CommandIntent::SwitchProfile { .. }
        | CommandIntent::UpdateProfile { .. }
        | CommandIntent::UpdateAllSubscriptions
        | CommandIntent::SaveSubscriptionFilter { .. }
        | CommandIntent::CreateAggregatedProfile { .. }
        | CommandIntent::ReAggregateProfile { .. }
        | CommandIntent::SaveCustomNodeDraft { .. }
        | CommandIntent::ApplyDnsSettings { .. }
        | CommandIntent::SaveProfileDocument { .. }
        | CommandIntent::SaveMixinOverlay { .. }
        | CommandIntent::RollbackCore => SameInstanceMayAdvance,
        CommandIntent::StopCore | CommandIntent::PauseKernel => SameInstanceMayStop,
        CommandIntent::RunScriptSandbox { .. }
        | CommandIntent::ClearScriptSandbox { .. }
        | CommandIntent::PrepareScriptExport { .. }
        | CommandIntent::SaveScriptExport { .. }
        | CommandIntent::CancelScriptExport { .. }
        | CommandIntent::StopProxy
        | CommandIntent::SetRoutingIntent { .. }
        | CommandIntent::GetRoutingIntent
        | CommandIntent::PrepareServiceMode
        | CommandIntent::RepairPortConflicts
        | CommandIntent::SetCoreLogLevel { .. }
        | CommandIntent::SetTunStack { .. }
        | CommandIntent::ReadOutboundInventory
        | CommandIntent::SelectOutbound { .. }
        | CommandIntent::ProbeOutbound { .. }
        | CommandIntent::ProbeOutboundBatch { .. }
        | CommandIntent::ApplyOutboundUiPreference { .. }
        | CommandIntent::StartSpeedtest { .. }
        | CommandIntent::SetSpeedtestConcurrency { .. }
        | CommandIntent::CancelSpeedtestRun { .. }
        | CommandIntent::SetRuntimeProbeSettings { .. }
        | CommandIntent::ReorderOverviewCards { .. }
        | CommandIntent::ResetOverviewCardOrder
        | CommandIntent::RestoreSubscriptionBackup { .. }
        | CommandIntent::UpdateSubscriptionFetchSettings { .. }
        | CommandIntent::SetSubscriptionAutoReload { .. }
        | CommandIntent::UpdateSubscriptionSchedule { .. }
        | CommandIntent::ImportSubscription { .. }
        | CommandIntent::DeleteProfile { .. }
        | CommandIntent::PreviewProfileAggregation { .. }
        | CommandIntent::SaveAggregationTemplate { .. }
        | CommandIntent::DeleteAggregationTemplate { .. }
        | CommandIntent::RefreshRuleProviders
        | CommandIntent::RefreshRuleProvider { .. }
        | CommandIntent::SimulateRuleTrace { .. }
        | CommandIntent::ApplyTracerRuleOverride { .. }
        | CommandIntent::ResetRuleHitCounters { .. }
        | CommandIntent::UnpackRuleProvider { .. }
        | CommandIntent::PurgeRuleProviderCache
        | CommandIntent::ToggleRuleEnabled { .. }
        | CommandIntent::CommitRuleList { .. }
        | CommandIntent::MoveRule { .. }
        | CommandIntent::AddCustomRule { .. }
        | CommandIntent::ApplyGameRoutingPresets { .. }
        | CommandIntent::UpgradeGeoDatabases
        | CommandIntent::ApplyProfileConfiguration { .. }
        | CommandIntent::CloseConnection { .. }
        | CommandIntent::CloseAllConnections
        | CommandIntent::ClearLogs
        | CommandIntent::PrepareLogExport
        | CommandIntent::SaveLogExport { .. }
        | CommandIntent::CancelLogExport { .. }
        | CommandIntent::SetLogLevelFilter { .. }
        | CommandIntent::ClearDnsCache { .. }
        | CommandIntent::QueryDns { .. }
        | CommandIntent::TestDnsLatency
        | CommandIntent::TestDnsLeak
        | CommandIntent::RunStunProbe
        | CommandIntent::RunDoctorDiagnostics
        | CommandIntent::BootstrapDoctor
        | CommandIntent::RepairDoctorIssue { .. }
        | CommandIntent::RepairAllDoctorIssues
        | CommandIntent::ToggleTun { .. }
        | CommandIntent::SetTunAutoRoute { .. }
        | CommandIntent::SetTunStrictRoute { .. }
        | CommandIntent::ProbeTunMtu
        | CommandIntent::RefreshPublicIpProbe
        | CommandIntent::SetSystemProxy { .. }
        | CommandIntent::SetLanSharing { .. }
        | CommandIntent::SetLanSecurity { .. }
        | CommandIntent::SetIpv6Routing { .. }
        | CommandIntent::ScanUwpApps
        | CommandIntent::SetUwpAppExemption { .. }
        | CommandIntent::SetAllUwpExemptions { .. }
        | CommandIntent::ApplyPac { .. }
        | CommandIntent::RefreshNetworkRoaming
        | CommandIntent::RepairNetworkRoutes
        | CommandIntent::CheckVpnAuthorization
        | CommandIntent::StartVpn
        | CommandIntent::StopVpn
        | CommandIntent::RunPrivilegedNetworkRegression
        | CommandIntent::SetAppRoutingPackageSelection { .. }
        | CommandIntent::SetAppRoutingMode { .. }
        | CommandIntent::SetAppRule { .. }
        | CommandIntent::SyncNow
        | CommandIntent::CreateBackupSnapshot { .. }
        | CommandIntent::ResolveConflictKeepLocal
        | CommandIntent::ResolveConflictTakeRemote
        | CommandIntent::PrepareSnapshotRestore { .. }
        | CommandIntent::ConfirmSnapshotRestore { .. }
        | CommandIntent::CancelSnapshotRestore { .. }
        | CommandIntent::LoadSnapshotDiff { .. }
        | CommandIntent::LoadSnapshotHistory { .. }
        | CommandIntent::PruneSnapshots { .. }
        | CommandIntent::LoadProfileDocument { .. }
        | CommandIntent::LoadProfileOptions { .. }
        | CommandIntent::PrepareCustomNodeDraft { .. }
        | CommandIntent::ImportCustomNodeUri { .. }
        | CommandIntent::ScanDialerChains
        | CommandIntent::UpdateCustomNodeDraftField { .. }
        | CommandIntent::ResolveCertificateAuthority { .. }
        | CommandIntent::SetLanguage { .. }
        | CommandIntent::UpdateSetting { .. }
        | CommandIntent::CheckUpdates => Stable,
    }
}

/// Check the terminal fence carried by a scoped command acknowledgment.
pub fn validate_terminal_scope(
    request_scope: &CoreScope,
    intent: &CommandIntent,
    outcome: CoreCommandOutcome,
) -> CoreCommandOutcome {
    let terminal_policy = terminal_scope_policy(intent);
    let terminal_matches = outcome
        .scope
        .as_ref()
        .is_some_and(|terminal| match terminal_policy {
            CoreTerminalScopePolicy::Stable => terminal == request_scope,
            CoreTerminalScopePolicy::SameInstanceMayAdvance => {
                terminal == request_scope
                    || (terminal.identity.engine == request_scope.identity.engine
                        && terminal.identity.instance == request_scope.identity.instance
                        && terminal.generation > request_scope.generation)
            }
            CoreTerminalScopePolicy::SameInstanceMayStop => {
                terminal == request_scope
                    || (terminal.identity == request_scope.identity
                        && terminal.generation == request_scope.generation
                        && request_scope.session_token.is_some()
                        && terminal.session_token.is_none())
            }
        });
    let valid = match &outcome.result {
        CommandResult::Rejected { failure, .. } => {
            terminal_matches || failure.code == ErrorCode::InvalidState
        }
        CommandResult::Accepted { .. }
        | CommandResult::Completed { .. }
        | CommandResult::Produced { .. } => terminal_matches,
    };
    if valid {
        return outcome;
    }
    let request_id = match &outcome.result {
        CommandResult::Accepted { request_id }
        | CommandResult::Completed { request_id }
        | CommandResult::Produced { request_id, .. }
        | CommandResult::Rejected { request_id, .. } => *request_id,
    };
    CoreCommandOutcome {
        scope: outcome.scope,
        result: CommandResult::Rejected {
            request_id,
            failure: Failure::new(
                ErrorCode::InvalidState,
                "terminal core scope diverged from the scoped command request",
                false,
            ),
        },
    }
}

use CommandCapabilityRoute::{CoreBackend, HostOnly, LifecycleChild, LifecycleOwned, LocalOnly};
use CoreCommandEffect::{Read, Write};
use CoreOperation::{
    Connection, CoreGarbageCollection, Dns, FakeIpCache, Geo, Lifecycle, Logs, Outbound, Profile,
    Provider, ProxyControl, Rules, Subscription, Tun, Version,
};

pub fn classify_command(intent: &CommandIntent) -> CommandCapabilityRoute {
    match intent {
        CommandIntent::StartCore | CommandIntent::ResumeKernel => LifecycleOwned(
            CoreCapabilityRequirement::new(Lifecycle(CoreLifecycleOperation::Start), Write),
        ),
        CommandIntent::StopCore | CommandIntent::PauseKernel => LifecycleOwned(
            CoreCapabilityRequirement::new(Lifecycle(CoreLifecycleOperation::Stop), Write),
        ),
        CommandIntent::RestartCore | CommandIntent::RestartKernel => LifecycleOwned(
            CoreCapabilityRequirement::new(Lifecycle(CoreLifecycleOperation::Restart), Write),
        ),
        CommandIntent::StartProxy => LifecycleOwned(CoreCapabilityRequirement::new(
            Lifecycle(CoreLifecycleOperation::Start),
            Write,
        )),
        CommandIntent::StopProxy => LifecycleChild,
        CommandIntent::SetRoutingIntent { .. } => CoreBackend(CoreCapabilityRequirement::new(
            ProxyControl(ProxyControlOperation::SetRoutingIntent),
            Write,
        )),
        CommandIntent::GetRoutingIntent => CoreBackend(CoreCapabilityRequirement::new(
            ProxyControl(ProxyControlOperation::GetRoutingIntent),
            Read,
        )),
        CommandIntent::CheckVpnAuthorization | CommandIntent::StartVpn | CommandIntent::StopVpn => {
            LifecycleChild
        }
        CommandIntent::SetCoreLogLevel { .. } => CoreBackend(CoreCapabilityRequirement::new(
            Logs(LogsOperation::SetLevel),
            Write,
        )),
        CommandIntent::SetTunStack { .. }
        | CommandIntent::ToggleTun { .. }
        | CommandIntent::SetTunAutoRoute { .. }
        | CommandIntent::SetTunStrictRoute { .. } => CoreBackend(CoreCapabilityRequirement::new(
            Tun(TunOperation::Configure),
            Write,
        )),
        CommandIntent::SwitchProfile { .. } => CoreBackend(CoreCapabilityRequirement::new(
            Profile(ProfileOperation::Apply),
            Write,
        )),
        CommandIntent::ReadOutboundInventory => CoreBackend(CoreCapabilityRequirement::new(
            Outbound(OutboundOperation::List),
            Read,
        )),
        CommandIntent::SelectOutbound { .. } => CoreBackend(CoreCapabilityRequirement::new(
            Outbound(OutboundOperation::Select),
            Write,
        )),
        CommandIntent::ProbeOutbound { .. } | CommandIntent::ProbeOutboundBatch { .. } => {
            CoreBackend(CoreCapabilityRequirement::new(
                Outbound(OutboundOperation::Probe),
                Write,
            ))
        }
        CommandIntent::ApplyOutboundUiPreference { .. } => CoreBackend(
            CoreCapabilityRequirement::new(Outbound(OutboundOperation::List), Read),
        ),
        CommandIntent::StartSpeedtest { .. } => CoreBackend(CoreCapabilityRequirement::new(
            Outbound(OutboundOperation::Probe),
            Write,
        )),
        CommandIntent::UpdateProfile { .. } => CoreBackend(CoreCapabilityRequirement::new(
            Subscription(SubscriptionOperation::Update),
            Write,
        )),
        CommandIntent::UpdateAllSubscriptions => CoreBackend(CoreCapabilityRequirement::new(
            Subscription(SubscriptionOperation::Refresh),
            Write,
        )),
        CommandIntent::ImportSubscription { .. } => CoreBackend(CoreCapabilityRequirement::new(
            Subscription(SubscriptionOperation::Import),
            Write,
        )),
        CommandIntent::SaveSubscriptionFilter { .. } => CoreBackend(
            CoreCapabilityRequirement::new(Profile(ProfileOperation::ApplyFeature), Write),
        ),
        CommandIntent::RefreshRuleProviders | CommandIntent::RefreshRuleProvider { .. } => {
            CoreBackend(CoreCapabilityRequirement::new(
                Provider(ProviderOperation::UpdateRule),
                Write,
            ))
        }
        CommandIntent::SimulateRuleTrace { .. } => CoreBackend(CoreCapabilityRequirement::new(
            Rules(RulesOperation::Trace),
            Read,
        )),
        CommandIntent::ApplyTracerRuleOverride { .. }
        | CommandIntent::ToggleRuleEnabled { .. }
        | CommandIntent::CommitRuleList { .. }
        | CommandIntent::MoveRule { .. }
        | CommandIntent::AddCustomRule { .. }
        | CommandIntent::ApplyGameRoutingPresets { .. }
        | CommandIntent::ApplyProfileConfiguration { .. } => CoreBackend(
            CoreCapabilityRequirement::new(Rules(RulesOperation::Apply), Write),
        ),
        CommandIntent::UnpackRuleProvider { .. } => CoreBackend(CoreCapabilityRequirement::new(
            Provider(ProviderOperation::ListRule),
            Read,
        )),
        CommandIntent::PurgeRuleProviderCache => CoreBackend(CoreCapabilityRequirement::new(
            CoreGarbageCollection(CoreGarbageCollectionOperation::Trigger),
            Write,
        )),
        CommandIntent::UpgradeGeoDatabases => CoreBackend(CoreCapabilityRequirement::new(
            Geo(GeoOperation::UpdateDatabases),
            Write,
        )),
        CommandIntent::CloseConnection { .. } | CommandIntent::CloseAllConnections => CoreBackend(
            CoreCapabilityRequirement::new(Connection(ConnectionOperation::Close), Write),
        ),
        CommandIntent::ClearDnsCache { .. } => CoreBackend(CoreCapabilityRequirement::new(
            FakeIpCache(FakeIpCacheOperation::Flush),
            Write,
        )),
        CommandIntent::QueryDns { .. } => CoreBackend(CoreCapabilityRequirement::new(
            Dns(DnsOperation::Query),
            Read,
        )),
        CommandIntent::ApplyDnsSettings { .. } => CoreBackend(CoreCapabilityRequirement::new(
            Dns(DnsOperation::ApplyConfiguration),
            Write,
        )),
        CommandIntent::SetLanSharing { .. }
        | CommandIntent::SetLanSecurity { .. }
        | CommandIntent::SetIpv6Routing { .. } => CoreBackend(CoreCapabilityRequirement::new(
            Profile(ProfileOperation::ApplyFeature),
            Write,
        )),
        CommandIntent::LoadProfileDocument { .. } => LocalOnly,
        CommandIntent::LoadProfileOptions { .. } => LocalOnly,
        CommandIntent::SaveProfileDocument { .. } | CommandIntent::SaveCustomNodeDraft { .. } => {
            CoreBackend(CoreCapabilityRequirement::new(
                Profile(ProfileOperation::Apply),
                Write,
            ))
        }
        CommandIntent::SaveMixinOverlay { .. } => CoreBackend(CoreCapabilityRequirement::new(
            Profile(ProfileOperation::ApplyFeature),
            Write,
        )),
        CommandIntent::DeleteProfile { .. }
        | CommandIntent::RestoreSubscriptionBackup { .. }
        | CommandIntent::UpdateSubscriptionFetchSettings { .. }
        | CommandIntent::SetSubscriptionAutoReload { .. }
        | CommandIntent::UpdateSubscriptionSchedule { .. }
        | CommandIntent::SaveAggregationTemplate { .. }
        | CommandIntent::DeleteAggregationTemplate { .. } => CoreBackend(
            CoreCapabilityRequirement::new(Profile(ProfileOperation::ApplyFeature), Write),
        ),
        CommandIntent::PreviewProfileAggregation { .. } => CoreBackend(
            CoreCapabilityRequirement::new(Profile(ProfileOperation::Validate), Read),
        ),
        CommandIntent::CreateAggregatedProfile { .. }
        | CommandIntent::ReAggregateProfile { .. } => CoreBackend(CoreCapabilityRequirement::new(
            Profile(ProfileOperation::Apply),
            Write,
        )),
        CommandIntent::ClearLogs => HostOnly(Capability::Logs),
        CommandIntent::ScanDialerChains => CoreBackend(CoreCapabilityRequirement::new(
            Profile(ProfileOperation::Validate),
            Read,
        )),
        CommandIntent::RollbackCore => CoreBackend(CoreCapabilityRequirement::new(
            Version(VersionOperation::Rollback),
            Write,
        )),
        CommandIntent::CheckUpdates => CoreBackend(CoreCapabilityRequirement::new(
            Version(VersionOperation::Inspect),
            Read,
        )),
        CommandIntent::PrepareServiceMode | CommandIntent::RepairPortConflicts => {
            HostOnly(Capability::CoreLifecycle)
        }
        CommandIntent::TestDnsLatency
        | CommandIntent::TestDnsLeak
        | CommandIntent::RunStunProbe => HostOnly(Capability::Dns),
        CommandIntent::RunDoctorDiagnostics
        | CommandIntent::BootstrapDoctor
        | CommandIntent::RepairDoctorIssue { .. }
        | CommandIntent::RepairAllDoctorIssues
        | CommandIntent::RunPrivilegedNetworkRegression => HostOnly(Capability::CoreLifecycle),
        CommandIntent::ProbeTunMtu => HostOnly(Capability::Tun),
        CommandIntent::RefreshPublicIpProbe => HostOnly(Capability::Speedtest),
        CommandIntent::SetSystemProxy { .. } => HostOnly(Capability::SystemProxy),
        CommandIntent::ScanUwpApps
        | CommandIntent::SetUwpAppExemption { .. }
        | CommandIntent::SetAllUwpExemptions { .. } => HostOnly(Capability::UwpLoopback),
        CommandIntent::ApplyPac { .. } => HostOnly(Capability::PacService),
        CommandIntent::RefreshNetworkRoaming | CommandIntent::RepairNetworkRoutes => {
            HostOnly(Capability::NetworkRoaming)
        }
        CommandIntent::SetAppRoutingPackageSelection { .. }
        | CommandIntent::SetAppRoutingMode { .. }
        | CommandIntent::SetAppRule { .. } => HostOnly(Capability::AppRouting),
        CommandIntent::ResolveCertificateAuthority { .. } => HostOnly(Capability::Profiles),
        CommandIntent::RunScriptSandbox { .. }
        | CommandIntent::ClearScriptSandbox { .. }
        | CommandIntent::PrepareScriptExport { .. }
        | CommandIntent::SaveScriptExport { .. }
        | CommandIntent::CancelScriptExport { .. }
        | CommandIntent::SetSpeedtestConcurrency { .. }
        | CommandIntent::CancelSpeedtestRun { .. }
        | CommandIntent::SetRuntimeProbeSettings { .. }
        | CommandIntent::ReorderOverviewCards { .. }
        | CommandIntent::ResetOverviewCardOrder
        | CommandIntent::ResetRuleHitCounters { .. }
        | CommandIntent::PrepareLogExport
        | CommandIntent::SaveLogExport { .. }
        | CommandIntent::CancelLogExport { .. }
        | CommandIntent::SetLogLevelFilter { .. }
        | CommandIntent::SetLanguage { .. }
        | CommandIntent::UpdateSetting { .. }
        | CommandIntent::SyncNow
        | CommandIntent::CreateBackupSnapshot { .. }
        | CommandIntent::ResolveConflictKeepLocal
        | CommandIntent::ResolveConflictTakeRemote
        | CommandIntent::PrepareSnapshotRestore { .. }
        | CommandIntent::ConfirmSnapshotRestore { .. }
        | CommandIntent::CancelSnapshotRestore { .. }
        | CommandIntent::LoadSnapshotDiff { .. }
        | CommandIntent::LoadSnapshotHistory { .. }
        | CommandIntent::PruneSnapshots { .. }
        | CommandIntent::PrepareCustomNodeDraft { .. }
        | CommandIntent::ImportCustomNodeUri { .. }
        | CommandIntent::UpdateCustomNodeDraftField { .. } => LocalOnly,
    }
}

#[cfg(test)]
#[path = "command_capability_tests.rs"]
mod tests;

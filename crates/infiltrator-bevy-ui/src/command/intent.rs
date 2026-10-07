//! The exhaustive mapping from native UI actions to shared product intents.
use super::UiCommand;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::rule_edit::RuleMoveDirection;
use infiltrator_contract::snapshot_history::SNAPSHOT_DEFAULT_KEEP;
impl UiCommand {
    /// Convert a business command to the shared application contract. Local
    /// presentation actions intentionally return `None`.
    pub fn to_intent(&self) -> Option<CommandIntent> {
        match self {
            Self::RunScriptSandbox { request } => Some(CommandIntent::RunScriptSandbox {
                request: request.clone(),
            }),
            Self::ClearScriptSandbox { operation } => Some(CommandIntent::ClearScriptSandbox {
                operation: *operation,
            }),
            Self::PrepareScriptExport { draft, .. } => Some(CommandIntent::PrepareScriptExport {
                draft: draft.clone(),
            }),
            Self::SaveScriptExport { identity, .. } => Some(CommandIntent::SaveScriptExport {
                identity: identity.clone(),
            }),
            Self::CancelScriptExport { identity, .. } => Some(CommandIntent::CancelScriptExport {
                identity: identity.clone(),
            }),
            Self::StartCore => Some(CommandIntent::StartCore),
            Self::StopCore => Some(CommandIntent::StopCore),
            Self::RestartCore => Some(CommandIntent::RestartCore),
            Self::PrepareServiceMode => Some(CommandIntent::PrepareServiceMode),
            Self::RepairPortConflicts => Some(CommandIntent::RepairPortConflicts),
            Self::SetCoreLogLevel(level) => Some(CommandIntent::SetCoreLogLevel { level: *level }),
            Self::SetTunStack(stack) => Some(CommandIntent::SetTunStack { stack: *stack }),
            Self::SetTunAutoRoute(enabled) => {
                Some(CommandIntent::SetTunAutoRoute { enabled: *enabled })
            }
            Self::SetTunStrictRoute(enabled) => {
                Some(CommandIntent::SetTunStrictRoute { enabled: *enabled })
            }
            Self::ProbeTunMtu => Some(CommandIntent::ProbeTunMtu),
            Self::RefreshPublicIpProbe => Some(CommandIntent::RefreshPublicIpProbe),
            Self::ReorderOverviewCards { order } => Some(CommandIntent::ReorderOverviewCards {
                order: order.clone(),
            }),
            Self::ResetOverviewCardOrder => Some(CommandIntent::ResetOverviewCardOrder),
            Self::MoveOverviewCardUp(_) | Self::MoveOverviewCardDown(_) => None,
            Self::SetProxyMode(mode) => Some(CommandIntent::SetProxyMode { mode: *mode }),
            Self::SelectProxyNode { group, node } => Some(CommandIntent::SelectProxyNode {
                group: group.clone(),
                node: node.clone(),
            }),
            Self::CancelSpeedtest => Some(CommandIntent::CancelSpeedtest),
            Self::SetSpeedtestConcurrency { limit } => {
                Some(CommandIntent::SetSpeedtestConcurrency { limit: *limit })
            }
            Self::TestAllProxyGroups => Some(CommandIntent::TestDelay {
                group: None,
                url: None,
                timeout_ms: None,
            }),
            Self::TestAllProxyGroupsWithUrl { url } => Some(CommandIntent::TestDelay {
                group: None,
                url: Some(url.clone()),
                timeout_ms: None,
            }),
            Self::TestProxyGroup { group } => Some(CommandIntent::TestDelay {
                group: Some(group.clone()),
                url: None,
                timeout_ms: None,
            }),
            Self::TestProxyNode { node } => Some(CommandIntent::TestNodeDelay {
                node: node.clone(),
                url: None,
                timeout_ms: None,
            }),
            Self::ToggleProxyGroupExpand { group } => Some(CommandIntent::ToggleProxyGroupExpand {
                group: group.clone(),
            }),
            Self::SetProxyProbeOptions { options } => Some(CommandIntent::SetProxyProbeOptions {
                options: options.clone(),
            }),
            Self::SetProxySearchQuery { query } => Some(CommandIntent::SetProxySearchQuery {
                query: query.clone(),
            }),
            Self::SetProxySortOrder(order) => {
                Some(CommandIntent::SetProxySortOrder { order: *order })
            }
            Self::ToggleFilterAlive(enabled) => {
                Some(CommandIntent::ToggleFilterAlive { enabled: *enabled })
            }
            Self::ToggleFavoriteProxy(proxy) => Some(CommandIntent::ToggleFavoriteProxy {
                proxy: proxy.clone(),
            }),
            Self::SetProxyCompactView(compact) => {
                Some(CommandIntent::SetProxyCompactView { compact: *compact })
            }
            Self::ReorderProxyGroups { group_names } => Some(CommandIntent::ReorderProxyGroups {
                group_names: group_names.clone(),
            }),
            Self::ResetProxyGroupOrder => Some(CommandIntent::ResetProxyGroupOrder),
            Self::ActivateProfile { id } => Some(CommandIntent::SwitchProfile {
                profile_id: id.clone(),
            }),
            Self::UpdateProfile { id } => Some(CommandIntent::UpdateProfile {
                profile_id: id.clone(),
            }),
            Self::UpdateAllSubscriptions => Some(CommandIntent::UpdateAllSubscriptions),
            Self::RestoreSubscriptionBackup { id } => {
                Some(CommandIntent::RestoreSubscriptionBackup {
                    profile_id: id.clone(),
                })
            }
            Self::SaveSubscriptionFetchSettings {
                profile_id,
                user_agent,
                insecure_skip_verify,
            } => Some(CommandIntent::UpdateSubscriptionFetchSettings {
                profile_id: profile_id.clone(),
                user_agent: user_agent.clone(),
                insecure_skip_verify: *insecure_skip_verify,
            }),
            Self::SaveSubscriptionFilter { source, filter } => {
                Some(CommandIntent::SaveSubscriptionFilter {
                    source: source.clone(),
                    filter: filter.clone(),
                })
            }
            Self::ImportSubscription {
                profile_id,
                channel,
                source,
            } => Some(CommandIntent::ImportSubscription {
                profile_id: profile_id.clone(),
                channel: *channel,
                source: source.clone(),
            }),
            Self::DeleteProfile { id } => Some(CommandIntent::DeleteProfile {
                profile_id: id.clone(),
            }),
            Self::PreviewProfileAggregation { draft } => {
                Some(CommandIntent::PreviewProfileAggregation {
                    draft: draft.clone(),
                })
            }
            Self::CreateAggregatedProfile { draft } => {
                Some(CommandIntent::CreateAggregatedProfile {
                    draft: draft.clone(),
                })
            }
            Self::SaveAggregationTemplate { name, draft } => {
                Some(CommandIntent::SaveAggregationTemplate {
                    name: name.clone(),
                    draft: draft.clone(),
                })
            }
            Self::DeleteAggregationTemplate { name } => {
                Some(CommandIntent::DeleteAggregationTemplate { name: name.clone() })
            }
            Self::PrepareCustomNodeDraft { draft } => Some(CommandIntent::PrepareCustomNodeDraft {
                draft: draft.clone(),
            }),
            Self::ImportCustomNodeUri { uri } => {
                Some(CommandIntent::ImportCustomNodeUri { uri: uri.clone() })
            }
            Self::SaveCustomNodeDraft { draft } => Some(CommandIntent::SaveCustomNodeDraft {
                draft: draft.clone(),
            }),
            Self::UpdateCustomNodeDraftField { field, value } => {
                Some(CommandIntent::UpdateCustomNodeDraftField {
                    field: field.clone(),
                    value: value.clone(),
                })
            }
            Self::ScanCustomNodeDialer => Some(CommandIntent::ScanDialerChains),
            Self::VerifyCustomNodeCa { trust } => {
                Some(CommandIntent::ResolveCertificateAuthority {
                    trust: trust.clone(),
                })
            }
            Self::ReAggregateProfile { template_name } => Some(CommandIntent::ReAggregateProfile {
                template_name: template_name.clone(),
            }),
            Self::SetSubscriptionAutoReload {
                profile_id,
                enabled,
            } => Some(CommandIntent::SetSubscriptionAutoReload {
                profile_id: profile_id.clone(),
                enabled: *enabled,
            }),
            Self::UpdateSubscriptionSchedule { profile_id, draft } => {
                Some(CommandIntent::UpdateSubscriptionSchedule {
                    profile_id: profile_id.clone(),
                    draft: draft.clone(),
                })
            }
            Self::RefreshRuleProviders => Some(CommandIntent::RefreshRuleProviders),
            Self::ClearRuleHitCounters { expected_source } => {
                Some(CommandIntent::ResetRuleHitCounters {
                    expected_source: expected_source.clone(),
                })
            }
            Self::UnpackRuleProvider(provider) => Some(CommandIntent::UnpackRuleProvider {
                provider_name: provider.clone(),
            }),
            Self::PurgeRuleProviderCache => Some(CommandIntent::PurgeRuleProviderCache),
            Self::CommitRuleList { request } => Some(CommandIntent::CommitRuleList {
                request: request.clone(),
            }),
            Self::ToggleRuleEnabled(index) => {
                Some(CommandIntent::ToggleRuleEnabled { index: *index })
            }
            Self::MoveRuleUp(index) => Some(CommandIntent::MoveRule {
                index: *index,
                direction: RuleMoveDirection::Up,
            }),
            Self::MoveRuleDown(index) => Some(CommandIntent::MoveRule {
                index: *index,
                direction: RuleMoveDirection::Down,
            }),
            Self::AddCustomRule {
                rule_type,
                payload,
                target,
            } => Some(CommandIntent::AddCustomRule {
                rule_type: rule_type.clone(),
                payload: payload.clone(),
                target: target.clone(),
            }),
            Self::ApplyGameRoutingPresets { target } => {
                Some(CommandIntent::ApplyGameRoutingPresets {
                    target: target.clone(),
                })
            }
            Self::UpgradeGeoDatabases => Some(CommandIntent::UpgradeGeoDatabases),
            Self::ApplyRulesJsonDocument { section, json } => {
                Some(CommandIntent::ApplyRulesJsonDocument {
                    section: *section,
                    json: json.clone(),
                })
            }
            Self::SimulateRuleTrace { operation, request } => {
                Some(CommandIntent::SimulateRuleTrace {
                    operation: *operation,
                    request: request.clone(),
                })
            }
            Self::ApplyTracerRuleOverride { request } => {
                Some(CommandIntent::ApplyTracerRuleOverride {
                    request: request.clone(),
                })
            }
            Self::CloseConnection { id } => Some(CommandIntent::CloseConnection { id: id.clone() }),
            Self::CloseAllConnections => Some(CommandIntent::CloseAllConnections),
            Self::ClearLogs => Some(CommandIntent::ClearLogs),
            Self::PrepareLogExport { .. } => Some(CommandIntent::PrepareLogExport),
            Self::SaveLogExport { identity, .. } => Some(CommandIntent::SaveLogExport {
                identity: identity.clone(),
            }),
            Self::CancelLogExport { identity, .. } => Some(CommandIntent::CancelLogExport {
                identity: identity.clone(),
            }),
            Self::SetLogLevelFilter { level } => Some(CommandIntent::SetLogLevelFilter {
                level: level.clone(),
            }),
            Self::ClearDnsCache { operation } => Some(CommandIntent::ClearDnsCache {
                operation: *operation,
            }),
            Self::QueryDns { operation, request } => Some(CommandIntent::QueryDns {
                operation: *operation,
                request: request.clone(),
            }),
            Self::TestDnsLatency => Some(CommandIntent::TestDnsLatency),
            Self::TestDnsLeak => Some(CommandIntent::TestDnsLeak),
            Self::RunStunProbe => Some(CommandIntent::RunStunProbe),
            Self::ApplyDnsSettings { patch } => Some(CommandIntent::ApplyDnsSettings {
                patch: patch.clone(),
            }),
            Self::ToggleTun { enabled } => Some(CommandIntent::ToggleTun { enabled: *enabled }),
            Self::StartVpn => Some(CommandIntent::StartVpn),
            Self::StopVpn => Some(CommandIntent::StopVpn),
            Self::RunPrivilegedNetworkRegression => {
                Some(CommandIntent::RunPrivilegedNetworkRegression)
            }
            Self::SetSystemProxy { enabled } => {
                Some(CommandIntent::SetSystemProxy { enabled: *enabled })
            }
            Self::SetLanSharing {
                enabled,
                mixed_port,
                bind_address,
            } => Some(CommandIntent::SetLanSharing {
                enabled: *enabled,
                mixed_port: *mixed_port,
                bind_address: bind_address.clone(),
            }),
            Self::SetLanSecurity {
                allowed_ips,
                disallowed_ips,
                skip_auth_prefixes,
                authentication_enabled,
                credentials,
            } => Some(CommandIntent::SetLanSecurity {
                allowed_ips: allowed_ips.clone(),
                disallowed_ips: disallowed_ips.clone(),
                skip_auth_prefixes: skip_auth_prefixes.clone(),
                authentication_enabled: *authentication_enabled,
                credentials: credentials.clone(),
            }),
            Self::SetIpv6Routing { enabled } => {
                Some(CommandIntent::SetIpv6Routing { enabled: *enabled })
            }
            Self::ScanUwpApps => Some(CommandIntent::ScanUwpApps),
            Self::SetUwpAppExemption { sid, exempt } => Some(CommandIntent::SetUwpAppExemption {
                sid: sid.clone(),
                exempt: *exempt,
            }),
            Self::SetAllUwpExemptions { exempt } => {
                Some(CommandIntent::SetAllUwpExemptions { exempt: *exempt })
            }
            Self::ApplyPac {
                enabled,
                bypass_domains,
                bypass_lan,
                minify,
            } => Some(CommandIntent::ApplyPac {
                enabled: *enabled,
                bypass_domains: bypass_domains.clone(),
                bypass_lan: *bypass_lan,
                minify: *minify,
            }),
            Self::RefreshNetworkRoaming => Some(CommandIntent::RefreshNetworkRoaming),
            Self::RepairNetworkRoutes => Some(CommandIntent::RepairNetworkRoutes),
            Self::RunDoctorDiagnostics => Some(CommandIntent::RunDoctorDiagnostics),
            Self::BootstrapDoctor => Some(CommandIntent::BootstrapDoctor),
            Self::RepairDoctorIssue { check_id } => Some(CommandIntent::RepairDoctorIssue {
                check_id: check_id.clone(),
            }),
            Self::RepairAllDoctorIssues => Some(CommandIntent::RepairAllDoctorIssues),
            Self::ToggleAppRouting { app_id, enabled } => Some(CommandIntent::ToggleAppRouting {
                app_id: app_id.clone(),
                enabled: *enabled,
            }),
            Self::SetAppRoutingMode { mode } => {
                Some(CommandIntent::SetAppRoutingMode { mode: mode.clone() })
            }
            Self::ToggleIncludeSystemApps { include } => {
                Some(CommandIntent::ToggleIncludeSystemApps { include: *include })
            }
            Self::SetAppRule { app_id, rule } => Some(CommandIntent::SetAppRule {
                app_id: app_id.clone(),
                rule: rule.clone(),
            }),
            Self::SyncNow => Some(CommandIntent::SyncNow),
            Self::CreateBackupSnapshot => {
                Some(CommandIntent::CreateBackupSnapshot { profile: None })
            }
            Self::ResolveConflictKeepLocal => Some(CommandIntent::ResolveConflictKeepLocal),
            Self::ResolveConflictTakeRemote => Some(CommandIntent::ResolveConflictTakeRemote),
            Self::SnapshotRestore { intent } => matches!(
                intent,
                CommandIntent::PrepareSnapshotRestore { .. }
                    | CommandIntent::ConfirmSnapshotRestore { .. }
                    | CommandIntent::CancelSnapshotRestore { .. }
            )
            .then(|| intent.clone()),
            Self::LoadSnapshotDiff { snapshot_id } => Some(CommandIntent::LoadSnapshotDiff {
                profile: None,
                snapshot_id: snapshot_id.clone(),
            }),
            Self::LoadSnapshotHistory => Some(CommandIntent::LoadSnapshotHistory {
                profile: None,
                keep: SNAPSHOT_DEFAULT_KEEP,
            }),
            Self::PruneSnapshots { keep } => Some(CommandIntent::PruneSnapshots {
                profile: None,
                keep: *keep,
            }),
            Self::LoadProfileDocument { profile } => Some(CommandIntent::LoadProfileDocument {
                profile: profile.clone(),
            }),
            Self::SaveProfileDocument {
                source,
                content,
                allow_protected,
            } => Some(CommandIntent::SaveProfileDocument {
                source: source.clone(),
                content: content.clone(),
                allow_protected: *allow_protected,
            }),
            Self::LoadProfileOptions { profile } => Some(CommandIntent::LoadProfileOptions {
                profile: profile.clone(),
            }),
            Self::SaveMixinOverlay { source, mixin_yaml } => {
                Some(CommandIntent::SaveMixinOverlay {
                    source: source.clone(),
                    mixin_yaml: mixin_yaml.clone(),
                })
            }
            Self::RollbackCore => Some(CommandIntent::RollbackCore),
            Self::SetLanguage { preference } => Some(CommandIntent::SetLanguage {
                preference: *preference,
            }),
            Self::UpdateSetting { key, value } => Some(CommandIntent::UpdateSetting {
                key: key.clone(),
                value: value.clone(),
            }),
            Self::CheckUpdates => Some(CommandIntent::CheckUpdates),
        }
    }
}

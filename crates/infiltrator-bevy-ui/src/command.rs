//! Strongly typed UI Command Bus and EntityObserver/Trigger Infrastructure.
//!
//! Charter law (docs/BEVY_UI_FRONTEND.md):
//! All UI user interactions (button clicks, switches, mode selections,
//! reconnects, clears) dispatch through typed commands into a centralized
//! command sink handle. No direct blocking calls in UI systems.

use bevy::app::{App, Plugin};
use bevy::ecs::event::Event;
use bevy::ecs::resource::Resource;
use infiltrator_application::core_application::CoreApplication;
use std::sync::{Arc, Mutex};

use infiltrator_contract::command::{CommandIntent, CoreLogLevel, ProxyMode};
use infiltrator_contract::lan::LanCredentials;
use infiltrator_contract::tun::TunStack;

/// All user action commands emitted from Bevy UI pages and controls.
#[derive(Clone, Debug, PartialEq, Eq)]
#[rustfmt::skip]
pub enum UiCommand {
    /// Start the shared core lifecycle.
    StartCore,
    /// Stop the shared core lifecycle.
    StopCore,
    /// Restart the shared core lifecycle.
    RestartCore,
    /// Prepare the host-owned privileged service mode for TUN routing.
    PrepareServiceMode,
    /// Repair the host-owned mixed/controller port bindings safely.
    RepairPortConflicts,
    /// Change Mihomo's live core log verbosity.
    SetCoreLogLevel(CoreLogLevel),
    /// Change Mihomo's live TUN protocol stack.
    SetTunStack(TunStack),
    /// Toggle Mihomo automatic route installation.
    SetTunAutoRoute(bool),
    /// Toggle Mihomo strict route enforcement.
    SetTunStrictRoute(bool),
    /// Probe the physical link and negotiate the virtual TUN MTU.
    ProbeTunMtu,
    RefreshPublicIpProbe,
    ReorderOverviewCards { order: Vec<infiltrator_contract::overview_layout::OverviewCardKind> },
    MoveOverviewCardUp(infiltrator_contract::overview_layout::OverviewCardKind),
    MoveOverviewCardDown(infiltrator_contract::overview_layout::OverviewCardKind),
    ResetOverviewCardOrder,
    /// Switch core proxy mode (Rule / Global / Direct).
    SetProxyMode(ProxyMode),
    /// Select a specific proxy node in a policy group.
    SelectProxyNode { group: String, node: String },
    /// Run latency benchmark across all proxy groups.
    TestAllProxyGroups,
    /// Cancel the active shared speedtest batch.
    CancelSpeedtest,
    /// Run latency benchmark for a specific proxy group.
    TestProxyGroup { group: String },
    /// Toggle expand/fold of a proxy group card.
    ToggleProxyGroupExpand { group: String },
    /// Change the proxy node sorting order.
    SetProxySortOrder(infiltrator_contract::proxies::ProxySortOrder),
    /// Toggle Filter Alive (只看可用) mode.
    ToggleFilterAlive(bool),
    /// Toggle favorite status of a proxy node.
    ToggleFavoriteProxy(String),
    /// Toggle compact list vs grid view for proxy nodes.
    SetProxyCompactView(bool),
    /// Reorder proxy groups by custom order.
    ReorderProxyGroups { group_names: Vec<String> },
    /// Reset proxy group custom ordering to default.
    ResetProxyGroupOrder,
    /// Activate a subscription configuration profile.
    ActivateProfile { id: String },
    /// Trigger an immediate remote update for a profile.
    UpdateProfile { id: String },
    /// Persist a profile's subscription fetch options.
    SaveSubscriptionFetchSettings {
        profile_id: String,
        user_agent: Option<String>,
        insecure_skip_verify: bool,
    },
    /// Delete a subscription profile.
    DeleteProfile { id: String },
    /// Trigger a remote update for all rule providers.
    RefreshRuleProviders,
    /// Reset every accumulated rule hit counter.
    ClearRuleHitCounters,
    /// DUAL-12-10: set the simulated sandbox source IP the tracer replays.
    SetRuleTracerContext { src_ip: Option<String> },
    /// Re-run the shared rule tracer for a target query.
    SimulateRuleTrace { query: String },
    /// Terminate a single active connection by ID.
    CloseConnection { id: String },
    /// Terminate all active connections.
    CloseAllConnections,
    /// Clear the in-memory log buffer.
    ClearLogs,
    /// Filter logs by severity level string.
    SetLogLevelFilter { level: Option<String> },
    /// Flush the DNS cache and Fake-IP table.
    ClearDnsCache,
    /// Test DNS server latency.
    TestDnsLatency,
    /// Toggle the host-owned TUN/VPN capability.
    ToggleTun { enabled: bool },
    /// Ask the Android host to obtain VpnService consent and start foreground mode.
    StartVpn,
    /// Stop the Android VpnService and tun2proxy worker.
    StopVpn,
    /// Run the host-injected privileged network transaction and rollback test.
    RunPrivilegedNetworkRegression,
    /// Toggle the host-owned system proxy capability.
    SetSystemProxy { enabled: bool },
    /// Apply the live Mihomo LAN listener settings.
    SetLanSharing {
        enabled: bool,
        mixed_port: u16,
        bind_address: String,
    },
    /// Apply LAN CIDR ACLs and the in-memory Basic Authentication input.
    SetLanSecurity {
        allowed_ips: Vec<String>,
        disallowed_ips: Vec<String>,
        skip_auth_prefixes: Vec<String>,
        authentication_enabled: bool,
        credentials: Option<LanCredentials>,
    },
    /// Toggle Mihomo's top-level IPv6 routing policy.
    SetIpv6Routing { enabled: bool },
    /// Refresh Windows AppContainer loopback state.
    ScanUwpApps,
    /// Toggle one Windows AppContainer loopback exemption.
    SetUwpAppExemption { sid: String, exempt: bool },
    /// Apply or clear all Windows AppContainer loopback exemptions.
    SetAllUwpExemptions { exempt: bool },
    /// Generate and apply the shared PAC script/service request.
    ApplyPac {
        enabled: bool,
        bypass_domains: Vec<String>,
        bypass_lan: bool,
        minify: bool,
    },
    /// Refresh physical-link/default-gateway facts through the shared app.
    RefreshNetworkRoaming,
    /// Force a safe TUN route-anchor repair through the shared app.
    RepairNetworkRoutes,
    /// Run full system doctor diagnostics.
    RunDoctorDiagnostics,
    /// Repair a specific doctor issue by check ID.
    RepairDoctorIssue { check_id: String },
    /// Repair all detected doctor issues.
    RepairAllDoctorIssues,
    /// Toggle split tunneling rule for an application.
    ToggleAppRouting { app_id: String, enabled: bool },
    /// Set app routing split tunneling mode.
    SetAppRoutingMode { mode: String },
    /// Toggle include system apps in split tunneling.
    ToggleIncludeSystemApps { include: bool },
    /// Set specific app routing action rule.
    SetAppRule { app_id: String, rule: String },
    /// Immediate WebDAV sync action.
    SyncNow,
    /// Create backup snapshot.
    CreateBackupSnapshot,
    /// Resolve conflict by keeping local.
    ResolveConflictKeepLocal,
    /// Resolve conflict by taking remote.
    ResolveConflictTakeRemote,
    /// Restore a specific snapshot.
    RestoreSnapshot { id: String },
    /// Select the last installed, locally recorded core version.
    RollbackCore,
    /// Update a core or UI setting.
    UpdateSetting { key: String, value: String },
    /// Check for a new core release.
    CheckUpdates,
}

impl UiCommand {
    /// Convert a business command to the shared application contract. Local
    /// presentation actions intentionally return `None`.
    pub fn to_intent(&self) -> Option<CommandIntent> {
        match self {
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
            Self::TestAllProxyGroups => Some(CommandIntent::TestDelay {
                group: None,
                url: None,
                timeout_ms: None,
            }),
            Self::TestProxyGroup { group } => Some(CommandIntent::TestDelay {
                group: Some(group.clone()),
                url: None,
                timeout_ms: None,
            }),
            Self::ToggleProxyGroupExpand { group } => Some(CommandIntent::ToggleProxyGroupExpand {
                group: group.clone(),
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
            Self::SaveSubscriptionFetchSettings {
                profile_id,
                user_agent,
                insecure_skip_verify,
            } => Some(CommandIntent::UpdateSubscriptionFetchSettings {
                profile_id: profile_id.clone(),
                user_agent: user_agent.clone(),
                insecure_skip_verify: *insecure_skip_verify,
            }),
            Self::DeleteProfile { id } => Some(CommandIntent::DeleteProfile {
                profile_id: id.clone(),
            }),
            Self::RefreshRuleProviders => Some(CommandIntent::RefreshRuleProviders),
            Self::ClearRuleHitCounters => Some(CommandIntent::ResetRuleHitCounters),
            Self::SetRuleTracerContext { src_ip } => Some(CommandIntent::SetRuleTracerContext {
                src_ip: src_ip.clone(),
            }),
            Self::SimulateRuleTrace { query } => Some(CommandIntent::SimulateRuleTrace {
                query: query.clone(),
            }),
            Self::CloseConnection { id } => Some(CommandIntent::CloseConnection { id: id.clone() }),
            Self::CloseAllConnections => Some(CommandIntent::CloseAllConnections),
            Self::ClearLogs => Some(CommandIntent::ClearLogs),
            Self::SetLogLevelFilter { level } => Some(CommandIntent::SetLogLevelFilter {
                level: level.clone(),
            }),
            Self::ClearDnsCache => Some(CommandIntent::ClearDnsCache),
            Self::TestDnsLatency => Some(CommandIntent::TestDnsLatency),
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
            Self::CreateBackupSnapshot => Some(CommandIntent::CreateBackupSnapshot),
            Self::ResolveConflictKeepLocal => Some(CommandIntent::ResolveConflictKeepLocal),
            Self::ResolveConflictTakeRemote => Some(CommandIntent::ResolveConflictTakeRemote),
            Self::RestoreSnapshot { id } => Some(CommandIntent::RestoreSnapshot { id: id.clone() }),
            Self::RollbackCore => Some(CommandIntent::RollbackCore),
            Self::UpdateSetting { key, value } => Some(CommandIntent::UpdateSetting {
                key: key.clone(),
                value: value.clone(),
            }),
            Self::CheckUpdates => Some(CommandIntent::CheckUpdates),
        }
    }
}

/// Abstract sink consuming typed UI commands.
pub trait UiCommandSink: Send + Sync {
    /// Submit a command for background processing.
    fn submit(&self, command: UiCommand);
}

/// ECS resource handle wrapping a thread-safe UI command sink.
#[derive(Resource, Clone)]
pub struct CommandSinkHandle(pub Arc<dyn UiCommandSink>);

impl CommandSinkHandle {
    /// Submit a command through the sink.
    pub fn submit(&self, command: UiCommand) {
        self.0.submit(command);
    }
}

/// Demo/in-memory command sink recording submitted commands for testing and mock runs.
#[derive(Clone, Debug, Default)]
pub struct DemoCommandSink {
    history: Arc<Mutex<Vec<UiCommand>>>,
}

impl DemoCommandSink {
    /// Create an active demo command sink.
    pub fn accepting() -> Self {
        Self {
            history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Read all commands submitted so far.
    pub fn submitted(&self) -> Vec<UiCommand> {
        self.history.lock().expect("sink poisoned").clone()
    }

    /// Clear recorded command history.
    pub fn clear(&self) {
        self.history.lock().expect("sink poisoned").clear();
    }
}

impl UiCommandSink for DemoCommandSink {
    fn submit(&self, command: UiCommand) {
        self.history.lock().expect("sink poisoned").push(command);
    }
}

/// Production sink that hands business commands to the shared application
/// service. Presentation-only commands are intentionally ignored here; their
/// state belongs to the Bevy scene rather than the core.
#[derive(Clone)]
pub struct ApplicationCommandSink {
    application: Arc<CoreApplication>,
}

impl ApplicationCommandSink {
    pub fn new(application: Arc<CoreApplication>) -> Self {
        Self { application }
    }

    pub fn application(&self) -> &Arc<CoreApplication> {
        &self.application
    }
}

impl UiCommandSink for ApplicationCommandSink {
    fn submit(&self, command: UiCommand) {
        if let Some(intent) = command.to_intent() {
            self.application.dispatch(intent);
        }
    }
}

/// Plugin installing the command sink handle into the Bevy App.
pub struct CommandPumpPlugin {
    sink: Arc<dyn UiCommandSink>,
}

impl CommandPumpPlugin {
    /// Create plugin with the given command sink implementation.
    pub fn new(sink: Arc<dyn UiCommandSink>) -> Self {
        Self { sink }
    }

    /// Create a production command pump backed by the shared application
    /// service. The application owns execution and runtime details; Bevy
    /// only emits the UI-local command vocabulary.
    pub fn for_application(application: Arc<CoreApplication>) -> Self {
        Self::new(Arc::new(ApplicationCommandSink::new(application)))
    }
}

impl Default for CommandPumpPlugin {
    fn default() -> Self {
        Self {
            sink: Arc::new(DemoCommandSink::accepting()),
        }
    }
}

impl Plugin for CommandPumpPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CommandSinkHandle(Arc::clone(&self.sink)));
    }
}

/// Notification severity level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// User notification event dispatched onto the event bus.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct UiNotificationEvent {
    pub level: NotificationLevel,
    pub title: String,
    pub message: String,
}

/// Event dispatched when a command completes or fails.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct CommandExecutedEvent {
    pub command: UiCommand,
    pub success: bool,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_command_sink_records_and_clears() {
        let sink = DemoCommandSink::accepting();
        assert!(sink.submitted().is_empty());

        sink.submit(UiCommand::ClearLogs);
        sink.submit(UiCommand::CloseAllConnections);

        let items = sink.submitted();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], UiCommand::ClearLogs);
        assert_eq!(items[1], UiCommand::CloseAllConnections);

        sink.clear();
        assert!(sink.submitted().is_empty());
    }

    #[test]
    fn business_commands_map_to_the_shared_contract() {
        assert_eq!(
            UiCommand::SetProxyMode(ProxyMode::Global).to_intent(),
            Some(CommandIntent::SetProxyMode {
                mode: ProxyMode::Global,
            })
        );
        assert_eq!(
            UiCommand::ToggleProxyGroupExpand {
                group: "auto".to_string(),
            }
            .to_intent(),
            Some(CommandIntent::ToggleProxyGroupExpand {
                group: "auto".to_string(),
            })
        );
        assert_eq!(
            UiCommand::SetProxySortOrder(
                infiltrator_contract::proxies::ProxySortOrder::LatencyDesc
            )
            .to_intent(),
            Some(CommandIntent::SetProxySortOrder {
                order: infiltrator_contract::proxies::ProxySortOrder::LatencyDesc,
            })
        );
        assert_eq!(
            UiCommand::ToggleFilterAlive(true).to_intent(),
            Some(CommandIntent::ToggleFilterAlive { enabled: true })
        );
        assert_eq!(
            UiCommand::ToggleFavoriteProxy("HK-01".into()).to_intent(),
            Some(CommandIntent::ToggleFavoriteProxy {
                proxy: "HK-01".into()
            })
        );
        assert_eq!(
            UiCommand::RollbackCore.to_intent(),
            Some(CommandIntent::RollbackCore)
        );
        assert_eq!(
            UiCommand::SetCoreLogLevel(CoreLogLevel::Debug).to_intent(),
            Some(CommandIntent::SetCoreLogLevel {
                level: CoreLogLevel::Debug,
            })
        );
        assert_eq!(
            UiCommand::SetTunStack(TunStack::Mixed).to_intent(),
            Some(CommandIntent::SetTunStack {
                stack: TunStack::Mixed,
            })
        );
        assert_eq!(
            UiCommand::ProbeTunMtu.to_intent(),
            Some(CommandIntent::ProbeTunMtu)
        );
        assert_eq!(
            UiCommand::RefreshPublicIpProbe.to_intent(),
            Some(CommandIntent::RefreshPublicIpProbe)
        );
        assert_eq!(
            UiCommand::ResetOverviewCardOrder.to_intent(),
            Some(CommandIntent::ResetOverviewCardOrder)
        );
        assert_eq!(
            UiCommand::SetTunAutoRoute(false).to_intent(),
            Some(CommandIntent::SetTunAutoRoute { enabled: false })
        );
        assert_eq!(
            UiCommand::SetTunStrictRoute(true).to_intent(),
            Some(CommandIntent::SetTunStrictRoute { enabled: true })
        );
        assert_eq!(
            UiCommand::SetSystemProxy { enabled: true }.to_intent(),
            Some(CommandIntent::SetSystemProxy { enabled: true })
        );
        assert_eq!(
            UiCommand::SetLanSharing {
                enabled: true,
                mixed_port: 8080,
                bind_address: "192.168.1.10".to_owned(),
            }
            .to_intent(),
            Some(CommandIntent::SetLanSharing {
                enabled: true,
                mixed_port: 8080,
                bind_address: "192.168.1.10".to_owned(),
            })
        );
        assert_eq!(
            UiCommand::SetLanSecurity {
                allowed_ips: vec!["192.168.1.0/24".to_owned()],
                disallowed_ips: vec!["192.168.1.10/32".to_owned()],
                skip_auth_prefixes: vec!["127.0.0.0/8".to_owned()],
                authentication_enabled: true,
                credentials: Some(LanCredentials {
                    username: "lan-user".to_owned(),
                    password: "secret-value".to_owned(),
                }),
            }
            .to_intent(),
            Some(CommandIntent::SetLanSecurity {
                allowed_ips: vec!["192.168.1.0/24".to_owned()],
                disallowed_ips: vec!["192.168.1.10/32".to_owned()],
                skip_auth_prefixes: vec!["127.0.0.0/8".to_owned()],
                authentication_enabled: true,
                credentials: Some(LanCredentials {
                    username: "lan-user".to_owned(),
                    password: "secret-value".to_owned(),
                }),
            })
        );
        assert_eq!(
            UiCommand::SetIpv6Routing { enabled: false }.to_intent(),
            Some(CommandIntent::SetIpv6Routing { enabled: false })
        );
        assert_eq!(
            UiCommand::ScanUwpApps.to_intent(),
            Some(CommandIntent::ScanUwpApps)
        );
        assert_eq!(
            UiCommand::SetAllUwpExemptions { exempt: true }.to_intent(),
            Some(CommandIntent::SetAllUwpExemptions { exempt: true })
        );
        assert_eq!(
            UiCommand::ApplyPac {
                enabled: true,
                bypass_domains: vec!["example.com".to_owned()],
                bypass_lan: true,
                minify: false,
            }
            .to_intent(),
            Some(CommandIntent::ApplyPac {
                enabled: true,
                bypass_domains: vec!["example.com".to_owned()],
                bypass_lan: true,
                minify: false,
            })
        );
    }

    #[test]
    fn lifecycle_commands_share_the_core_application_intents() {
        assert_eq!(
            UiCommand::StartCore.to_intent(),
            Some(CommandIntent::StartCore)
        );
        assert_eq!(
            UiCommand::StopCore.to_intent(),
            Some(CommandIntent::StopCore)
        );
        assert_eq!(
            UiCommand::RestartCore.to_intent(),
            Some(CommandIntent::RestartCore)
        );
    }
}

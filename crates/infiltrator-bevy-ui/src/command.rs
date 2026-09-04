//! Strongly typed UI Command Bus and EntityObserver/Trigger Infrastructure.
//!
//! Charter law (docs/BEVY_UI_FRONTEND.md):
//! All UI user interactions (button clicks, switches, mode selections,
//! reconnects, clears) dispatch through typed commands into a centralized
//! command sink handle. No direct blocking calls in UI systems.

use bevy::app::{App, Plugin};
use bevy::ecs::event::Event;
use bevy::ecs::resource::Resource;
use std::sync::{Arc, Mutex};

use crate::projection::ProxyMode;

/// All user action commands emitted from Bevy UI pages and controls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiCommand {
    /// Switch core proxy mode (Rule / Global / Direct).
    SetProxyMode(ProxyMode),
    /// Select a specific proxy node in a policy group.
    SelectProxyNode { group: String, node: String },
    /// Run latency benchmark across all proxy groups.
    TestAllProxyGroups,
    /// Run latency benchmark for a specific proxy group.
    TestProxyGroup { group: String },
    /// Toggle expand/fold of a proxy group card.
    ToggleProxyGroupExpand { group: String },
    /// Activate a subscription configuration profile.
    ActivateProfile { id: String },
    /// Trigger an immediate remote update for a profile.
    UpdateProfile { id: String },
    /// Delete a subscription profile.
    DeleteProfile { id: String },
    /// Trigger a remote update for all rule providers.
    RefreshRuleProviders,
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
    /// Update a core or UI setting.
    UpdateSetting { key: String, value: String },
    /// Request core restart.
    RestartCore,
    /// Request core stop.
    StopCore,
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

/// Plugin installing the command sink handle into the Bevy App.
pub struct CommandPumpPlugin {
    sink: Arc<dyn UiCommandSink>,
}

impl CommandPumpPlugin {
    /// Create plugin with the given command sink implementation.
    pub fn new(sink: Arc<dyn UiCommandSink>) -> Self {
        Self { sink }
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
}

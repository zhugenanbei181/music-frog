//! The business panels the Slint workbench pages carry.
//!
//! Every panel is one business of the shared parity catalogue. A panel owns
//! only the presentation phase of its own gesture: the facts it shows come
//! from the shared owner snapshot and the shared presentation projections, and
//! the command it sends is the shared `CommandIntent` vocabulary. The panel
//! never derives a business fact of its own.
//!
//! The phase is one typed value with typed transitions, so a panel can never
//! hide a stage table behind a combination of flags.

use infiltrator_contract::command_output::CommandOutput;
use lurker_core_contract::error::Failure;
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;

/// The business panels the Slint workbench pages carry.
///
/// Every variant is a business the surface really opens, so the capture path
/// can activate it and publish its real geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BusinessId {
    /// The confirmation that breaks every open connection.
    ConnectionsCloseAllConfirm,
    /// The drawer that shows the facts of one connection.
    ConnectionsDetailsDrawer,
    /// The grouping of the connection list.
    ConnectionsGrouping,
    /// The reviewed export of the redacted log.
    LogsRedactedExport,
    /// The follow/lock control of the log stream.
    LogsScrollLock,
    /// The telemetry readout of the running core.
    RuntimeTelemetryObservation,
    /// The speed test and the cancellation of its run.
    SpeedtestProgressCancel,
    /// The sheet with the details of one speed test.
    SpeedtestDetailsModal,
    /// The comparison of the recorded speed test runs.
    SpeedtestHistoryComparison,
    /// The sheet that adds a custom outbound.
    ProxiesCustomNodeModal,
    /// The expansion of one outbound group.
    ProxiesGroupExpanded,
    /// The reorder of the outbound groups.
    ProxiesGroupReorder,
    /// The drawer with the details of one outbound.
    ProxiesNodeDetailDrawer,
    /// The editor of the probe settings.
    ProxiesProbeSettings,
    /// The preview of an imported outbound URI.
    ProxiesUriImportPreview,
    /// The editor of the rule list.
    RulesListEditor,
    /// The editor of one named rule override.
    RulesOverrideEditor,
    /// The failure of one rule provider refresh.
    RulesProviderRefreshError,
    /// The inspector of the rule hit statistics.
    RulesStatisticsInspector,
    /// The drawer with the trace of one rule.
    RulesTracerDrawer,
    /// The confirmation of a core version rollback.
    CoreVersionRollbackConfirm,
    /// The LAN sharing and its security policy.
    SettingsLanSecurity,
    /// The choice of the interface language.
    SettingsLanguageChoice,
    /// The panel of the privileged network operations.
    SettingsPermissionPanel,
    /// The repair of a port conflict.
    SettingsPortConflictRepair,
    /// The permission the TUN mode needs.
    SettingsTunPermission,
    /// The command palette of the shell.
    ShellCommandPalette,
    /// The control of the routing intent.
    ShellRoutingIntentControl,
    /// The authentication guide of the routing intent.
    ShellRoutingIntentAuthentication,
    /// The bounded window of a large document.
    EditorLargeDocumentWindow,
    /// The insertion of a YAML snippet.
    EditorSnippetInsertion,
    /// The navigation to one YAML error.
    EditorYamlErrorNavigation,
    /// The failure of one subscription import.
    ProfilesSubscriptionError,
    /// The mask that covers a reconnecting core.
    OverviewReconnectMask,
    /// The drill-down into the traffic topology.
    OverviewTopologyDrilldown,
}

impl BusinessId {
    /// Every business panel the Slint surface carries.
    pub const ALL: &'static [Self] = &[
        Self::ConnectionsCloseAllConfirm,
        Self::ConnectionsDetailsDrawer,
        Self::ConnectionsGrouping,
        Self::LogsRedactedExport,
        Self::LogsScrollLock,
        Self::RuntimeTelemetryObservation,
        Self::SpeedtestProgressCancel,
        Self::SpeedtestDetailsModal,
        Self::SpeedtestHistoryComparison,
        Self::ProxiesCustomNodeModal,
        Self::ProxiesGroupExpanded,
        Self::ProxiesGroupReorder,
        Self::ProxiesNodeDetailDrawer,
        Self::ProxiesProbeSettings,
        Self::ProxiesUriImportPreview,
        Self::RulesListEditor,
        Self::RulesOverrideEditor,
        Self::RulesProviderRefreshError,
        Self::RulesStatisticsInspector,
        Self::RulesTracerDrawer,
        Self::CoreVersionRollbackConfirm,
        Self::SettingsLanSecurity,
        Self::SettingsLanguageChoice,
        Self::SettingsPermissionPanel,
        Self::SettingsPortConflictRepair,
        Self::SettingsTunPermission,
        Self::ShellCommandPalette,
        Self::ShellRoutingIntentControl,
        Self::ShellRoutingIntentAuthentication,
        Self::EditorLargeDocumentWindow,
        Self::EditorSnippetInsertion,
        Self::EditorYamlErrorNavigation,
        Self::ProfilesSubscriptionError,
        Self::OverviewReconnectMask,
        Self::OverviewTopologyDrilldown,
    ];

    /// The parity catalogue name of the business.
    pub fn name(self) -> &'static str {
        match self {
            Self::ConnectionsCloseAllConfirm => "connections-close-all-confirm",
            Self::ConnectionsDetailsDrawer => "connections-details-drawer",
            Self::ConnectionsGrouping => "connections-grouping",
            Self::LogsRedactedExport => "logs-redacted-export",
            Self::LogsScrollLock => "logs-scroll-lock",
            Self::RuntimeTelemetryObservation => "runtime-telemetry-observation",
            Self::SpeedtestProgressCancel => "speedtest-progress-cancel",
            Self::SpeedtestDetailsModal => "speedtest-details-modal",
            Self::SpeedtestHistoryComparison => "speedtest-history-comparison",
            Self::ProxiesCustomNodeModal => "proxies-custom-node-modal",
            Self::ProxiesGroupExpanded => "proxies-group-expanded",
            Self::ProxiesGroupReorder => "proxies-group-reorder",
            Self::ProxiesNodeDetailDrawer => "proxies-node-detail-drawer",
            Self::ProxiesProbeSettings => "proxies-probe-settings",
            Self::ProxiesUriImportPreview => "proxies-uri-import-preview",
            Self::RulesListEditor => "rules-list-editor",
            Self::RulesOverrideEditor => "rules-override-editor",
            Self::RulesProviderRefreshError => "rules-provider-refresh-error",
            Self::RulesStatisticsInspector => "rules-statistics-inspector",
            Self::RulesTracerDrawer => "rules-tracer-drawer",
            Self::CoreVersionRollbackConfirm => "core-version-rollback-confirm",
            Self::SettingsLanSecurity => "settings-lan-security",
            Self::SettingsLanguageChoice => "settings-language-choice",
            Self::SettingsPermissionPanel => "settings-permission-panel",
            Self::SettingsPortConflictRepair => "settings-port-conflict-repair",
            Self::SettingsTunPermission => "settings-tun-permission",
            Self::ShellCommandPalette => "shell-command-palette",
            Self::ShellRoutingIntentControl => "shell-routing-intent-control",
            Self::ShellRoutingIntentAuthentication => "shell-routing-intent-authentication",
            Self::EditorLargeDocumentWindow => "editor-large-document-window",
            Self::EditorSnippetInsertion => "editor-snippet-insertion",
            Self::EditorYamlErrorNavigation => "editor-yaml-error-navigation",
            Self::ProfilesSubscriptionError => "profiles-subscription-error",
            Self::OverviewReconnectMask => "overview-reconnect-mask",
            Self::OverviewTopologyDrilldown => "overview-topology-drilldown",
        }
    }

    /// The Slint route that carries the business.
    ///
    /// The route is the page the capture opens and the page the product
    /// navigates to, so the carrier of a business is one page for both.
    pub fn route(self) -> &'static str {
        match self {
            Self::ConnectionsCloseAllConfirm
            | Self::ConnectionsDetailsDrawer
            | Self::ConnectionsGrouping => "ConnectionsWorkbench",
            Self::LogsRedactedExport | Self::LogsScrollLock => "LogsWorkbench",
            Self::RuntimeTelemetryObservation
            | Self::SpeedtestProgressCancel
            | Self::SpeedtestDetailsModal
            | Self::SpeedtestHistoryComparison => "RuntimeWorkbench",
            Self::ProxiesCustomNodeModal
            | Self::ProxiesGroupExpanded
            | Self::ProxiesGroupReorder
            | Self::ProxiesNodeDetailDrawer
            | Self::ProxiesProbeSettings
            | Self::ProxiesUriImportPreview => "ProxiesWorkbench",
            Self::RulesListEditor
            | Self::RulesOverrideEditor
            | Self::RulesProviderRefreshError
            | Self::RulesStatisticsInspector
            | Self::RulesTracerDrawer => "RulesWorkbench",
            Self::CoreVersionRollbackConfirm
            | Self::SettingsLanSecurity
            | Self::SettingsLanguageChoice
            | Self::SettingsPermissionPanel
            | Self::SettingsPortConflictRepair
            | Self::SettingsTunPermission => "SettingsWorkbench",
            Self::ShellCommandPalette
            | Self::ShellRoutingIntentControl
            | Self::ShellRoutingIntentAuthentication => "ShellWorkbench",
            Self::EditorLargeDocumentWindow
            | Self::EditorSnippetInsertion
            | Self::EditorYamlErrorNavigation => "Editor",
            Self::ProfilesSubscriptionError => "Filters",
            Self::OverviewReconnectMask | Self::OverviewTopologyDrilldown => "OverviewWorkbench",
        }
    }

    /// The business panel the surface carries for the parity business.
    pub fn from_name(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|business| business.name() == value)
    }
}

/// The phase of one business panel.
///
/// The phase is the only stage the surface keeps: whether the panel waits for
/// the user, shows its own review, waits for the owner, or shows the owner's
/// answer. Every transition is typed, so no combination of flags can hide a
/// stage of the business.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BusinessPhase {
    /// The panel shows the business and waits for the user's gesture.
    Idle,
    /// The panel's own review is open; the user has not confirmed it yet.
    Reviewing,
    /// The panel asked the owner and the owner's answer is still owed.
    Pending,
    /// The owner refused the panel's command.
    Refused { message: String },
    /// The owner accepted the panel's command.
    Accepted,
    /// The business's own local transition was applied.
    ///
    /// A business that is a view over the facts the owner already published
    /// asks the owner for nothing, so the surface applies the transition the
    /// user confirmed.
    Applied,
}

impl BusinessPhase {
    /// Whether the panel is still waiting for the owner.
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }
}

/// The presentation phase of every business panel the surface carries.
///
/// The phase is the surface's own presentation state: the owner owns the
/// business facts and the command answers, the surface owns whether the user
/// still looks at the panel and whether the answer is still owed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BusinessPanels {
    /// The phase of every panel the surface has opened.
    phases: BTreeMap<BusinessId, BusinessPhase>,
}

impl BusinessPanels {
    /// The phase of one panel.
    pub fn phase(&self, business: BusinessId) -> BusinessPhase {
        self.phases
            .get(&business)
            .cloned()
            .unwrap_or(BusinessPhase::Idle)
    }

    /// Opens the panel's own review.
    pub fn open(&mut self, business: BusinessId) {
        self.phases.insert(business, BusinessPhase::Reviewing);
    }

    /// Closes the panel's own review without asking the owner.
    pub fn dismiss(&mut self, business: BusinessId) {
        self.phases.insert(business, BusinessPhase::Idle);
    }

    /// Records that the panel asked the owner and still waits for its answer.
    pub fn pending(&mut self, business: BusinessId) {
        self.phases.insert(business, BusinessPhase::Pending);
    }

    /// Records that the business's own local transition was applied.
    pub fn apply(&mut self, business: BusinessId) {
        self.phases.insert(business, BusinessPhase::Applied);
    }

    /// Records the owner's answer to the panel's command.
    pub fn settle(&mut self, business: BusinessId, result: &Result<CommandOutput, Failure>) {
        let phase = match result {
            Ok(_) => BusinessPhase::Accepted,
            Err(failure) => BusinessPhase::Refused {
                message: failure.message.clone(),
            },
        };
        self.phases.insert(business, phase);
    }
}

/// The business panels the surface owns.
pub type SharedBusinessPanels = Mutex<BusinessPanels>;

/// Locks the business panels of the surface for a read or a write.
pub fn guard(panels: &SharedBusinessPanels) -> MutexGuard<'_, BusinessPanels> {
    panels.lock().unwrap_or_else(PoisonError::into_inner)
}

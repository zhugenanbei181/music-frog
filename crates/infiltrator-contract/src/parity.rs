//! Peer-product interaction identities and required state-machine paths.
//! UI toolkits and platform types never enter this registry.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct FeatureSpec {
    pub id: &'static str,
    pub iced_page: &'static str,
    pub bevy_page: &'static str,
    /// Page that carries the business on the Slint surface. An empty value means
    /// the third UI does not implement this business yet, so no Slint scenario
    /// anchor may claim it.
    pub slint_page: &'static str,
    pub required_paths: &'static [&'static str],
}

macro_rules! features {
    ($( $variant:ident => ($id:literal, $iced:literal, $bevy:literal, $slint:literal, [$($path:literal),+]) ),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum FeatureId { $( $variant ),+ }
        impl FeatureId {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub const fn spec(self) -> FeatureSpec {
                match self { $( Self::$variant => FeatureSpec {
                    id: $id, iced_page: $iced, bevy_page: $bevy, slint_page: $slint,
                    required_paths: &[$($path),+],
                } ),+ }
            }
        }
    };
}

features! {
    LifecycleRecoveryPanel => ("lifecycle-recovery-panel", "doctor", "doctor", "doctor", ["success", "error", "retry"]),
    CoreVersionRollbackConfirm => ("core-version-rollback-confirm", "settings", "settings", "", ["success", "cancel", "error"]),
    SettingsPermissionPanel => ("settings-permission-panel", "settings", "settings", "", ["success", "error", "unsupported"]),
    SettingsPortConflictRepair => ("settings-port-conflict-repair", "settings", "settings", "", ["success", "error"]),
    SettingsTunPermission => ("settings-tun-permission", "settings", "settings", "", ["success", "cancel", "error", "unsupported"]),
    SettingsLanSecurity => ("settings-lan-security", "settings", "settings", "", ["success", "error"]),
    SettingsLanguageChoice => ("settings-language-choice", "settings", "settings", "", ["success", "error", "retry", "refresh", "focus"]),
    AppRoutingPolicyEditor => ("app-routing-policy-editor", "app_routing", "app_routing", "app_routing", ["success", "cancel", "error", "refresh", "unsupported"]),
    OverviewTopologyDrilldown => ("overview-topology-drilldown", "overview", "overview", "", ["success", "close"]),
    OverviewReconnectMask => ("overview-reconnect-mask", "overview", "overview", "", ["success", "error", "retry"]),
    OverviewCardReorder => ("overview-card-reorder", "overview", "overview", "", ["success", "cancel"]),
    ProxiesProbeSettings => ("proxies-probe-settings", "proxies", "proxies", "", ["success", "cancel", "error", "retry", "refresh"]),
    ProxiesSearchHighlight => ("proxies-search-highlight", "proxies", "proxies", "", ["success", "empty", "clear", "error", "retry", "refresh"]),
    ProxiesGroupExpanded => ("proxies-group-expanded", "proxies", "proxies", "", ["success", "close", "reorder", "added", "removed"]),
    ProxiesNodeDetailDrawer => ("proxies-node-detail-drawer", "proxies", "proxies", "", ["success", "close", "unsupported", "reorder", "removed", "error", "retry"]),
    ProxiesCustomNodeModal => ("proxies-custom-node-modal", "proxies", "proxies", "", ["success", "cancel", "error"]),
    ProxiesUriImportPreview => ("proxies-uri-import-preview", "proxies", "proxies", "", ["success", "cancel", "error"]),
    ProxiesGroupReorder => ("proxies-group-reorder", "proxies", "proxies", "", ["success", "cancel", "error", "retry", "refresh"]),
    SpeedtestProgressCancel => ("speedtest-progress-cancel", "overview", "overview", "", ["success", "cancel", "error"]),
    SpeedtestDetailsModal => ("speedtest-details-modal", "runtime", "overview", "", ["success", "close", "unsupported"]),
    SpeedtestHistoryComparison => ("speedtest-history-comparison", "runtime", "overview", "", ["success", "empty"]),
    ProfilesImportModal => ("profiles-import-modal", "profiles", "profiles", "profiles", ["success", "cancel", "error"]),
    ProfilesDeleteConfirm => ("profiles-delete-confirm", "profiles", "profiles", "profiles", ["success", "cancel", "error"]),
    ProfilesSubscriptionError => ("profiles-subscription-error", "profiles", "profiles", "", ["error", "retry"]),
    ProfilesFilterEditor => ("profiles-filter-editor", "filter", "profiles", "filters", ["success", "cancel", "error"]),
    ProfilesAggregatorWizard => ("profiles-aggregator-wizard", "profiles", "profiles", "filters", ["success", "cancel", "error"]),
    ProfilesAggregatorPreview => ("profiles-aggregator-preview", "profiles", "profiles", "filters", ["success", "cancel", "error"]),
    ProfilesScriptSandbox => ("profiles-script-sandbox", "editor", "profiles", "editor", ["unknown", "success", "error", "retry", "pending", "clear", "presets", "locale"]),
    ProfilesScriptExportReview => ("profiles-script-export-review", "editor", "profiles", "editor", ["success", "cancel", "error", "retry", "pending", "unsupported", "locale"]),
    ProfilesDiffViewer => ("profiles-diff-viewer", "editor", "profiles", "editor", ["success", "close"]),
    ProfilesSnapshotRestoreConfirm => ("profiles-snapshot-restore-confirm", "editor", "profiles", "editor", ["success", "cancel", "error"]),
    EditorYamlErrorNavigation => ("editor-yaml-error-navigation", "editor", "profiles", "", ["success", "error"]),
    EditorSnippetInsertion => ("editor-snippet-insertion", "editor", "profiles", "", ["success", "error"]),
    EditorLargeDocumentWindow => ("editor-large-document-window", "editor", "profiles", "", ["success"]),
    RulesSearchHighlight => ("rules-search-highlight", "rules", "rules", "", ["success", "empty"]),
    RulesListEditor => ("rules-list-editor", "rules", "rules", "", ["success", "cancel", "error", "retry", "refresh", "stale", "focus"]),
    RulesStatisticsInspector => ("rules-statistics-inspector", "rules", "rules", "", ["unknown", "success", "cancel", "error", "retry", "refresh", "stale", "pending", "locale", "cleanup"]),
    RulesTracerDrawer => ("rules-tracer-drawer", "rules", "rules", "", ["success", "close", "error"]),
    RulesOverrideEditor => ("rules-override-editor", "rules", "rules", "", ["success", "cancel", "error"]),
    RulesProviderRefreshError => ("rules-provider-refresh-error", "rules-providers", "rules", "", ["success", "error", "retry"]),
    DnsLeakAlert => ("dns-leak-alert", "dns", "dns", "dns", ["success", "error", "unsupported"]),
    DnsQueryDetails => ("dns-query-details", "dns", "dns", "dns", ["success", "cancel", "error", "retry", "unsupported", "stale", "empty"]),
    DnsFakeIpFlushConfirm => ("dns-fake-ip-flush-confirm", "dns", "dns", "dns", ["success", "cancel", "error", "retry", "unsupported", "stale"]),
    DnsHostsEditor => ("dns-hosts-editor", "dns", "dns", "dns", ["success", "cancel", "error", "retry", "migration", "stale", "empty", "unsupported"]),
    ConnectionsDetailsDrawer => ("connections-details-drawer", "runtime", "connections", "", ["success", "close", "unsupported", "reorder", "removed"]),
    ConnectionsCloseAllConfirm => ("connections-close-all-confirm", "runtime", "connections", "", ["success", "cancel"]),
    ConnectionsSearchHighlight => ("connections-search-highlight", "runtime", "connections", "", ["success", "empty"]),
    ConnectionsGrouping => ("connections-grouping", "runtime", "connections", "", ["success"]),
    LogsScrollLock => ("logs-scroll-lock", "runtime", "logs", "", ["success", "lock", "resume", "history", "growth"]),
    RuntimeTelemetryObservation => ("runtime-telemetry-observation", "runtime", "overview", "", ["unknown", "zero", "error", "stale", "refresh", "locale"]),
    LogsRedactedExport => ("logs-redacted-export", "runtime", "logs", "", ["success", "cancel", "error", "retry", "stale", "pending", "unsupported", "filtered"]),
    LogsSearchHighlight => ("logs-search-highlight", "runtime", "logs", "", ["success", "empty", "error", "clear", "refresh", "locale", "stale"]),
    SyncConflictResolver => ("sync-conflict-resolver", "sync", "sync", "sync", ["success", "cancel", "error"]),
    SyncRestoreConfirm => ("sync-restore-confirm", "sync", "sync", "sync", ["success", "cancel", "error"]),
    DoctorFailureRecovery => ("doctor-failure-recovery", "doctor", "doctor", "doctor", ["success", "error", "retry"]),
    ShellCommandPalette => ("shell-command-palette", "overview", "overview", "", ["success", "cancel", "empty", "full-list", "locale", "refresh"]),
    ShellRoutingIntentControl => ("shell-routing-intent-control", "overview", "overview", "", ["success", "error", "retry", "close", "stale", "refresh", "pending", "unsupported", "guide"]),
    ShellRoutingIntentAuthentication => ("shell-routing-intent-authentication", "overview", "overview", "", ["error", "guide", "close"]),
    ShellKeyboardFocus => ("shell-keyboard-focus", "overview", "overview", "", ["success"]),
}

#[cfg(test)]
#[path = "parity_test.rs"]
mod parity_test;

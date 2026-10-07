//! Peer-product interaction identities and required state-machine paths.
//! UI toolkits and platform types never enter this registry.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct FeatureSpec {
    pub id: &'static str,
    pub iced_page: &'static str,
    pub bevy_page: &'static str,
    pub required_paths: &'static [&'static str],
}

macro_rules! features {
    ($( $variant:ident => ($id:literal, $iced:literal, $bevy:literal, [$($path:literal),+]) ),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum FeatureId { $( $variant ),+ }
        impl FeatureId {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub const fn spec(self) -> FeatureSpec {
                match self { $( Self::$variant => FeatureSpec {
                    id: $id, iced_page: $iced, bevy_page: $bevy, required_paths: &[$($path),+],
                } ),+ }
            }
        }
    };
}

features! {
    LifecycleRecoveryPanel => ("lifecycle-recovery-panel", "doctor", "doctor", ["success", "error", "retry"]),
    CoreVersionRollbackConfirm => ("core-version-rollback-confirm", "settings", "settings", ["success", "cancel", "error"]),
    SettingsPermissionPanel => ("settings-permission-panel", "settings", "settings", ["success", "error", "unsupported"]),
    SettingsPortConflictRepair => ("settings-port-conflict-repair", "settings", "settings", ["success", "error"]),
    SettingsTunPermission => ("settings-tun-permission", "settings", "settings", ["success", "cancel", "error", "unsupported"]),
    SettingsLanSecurity => ("settings-lan-security", "settings", "settings", ["success", "error"]),
    SettingsLanguageChoice => ("settings-language-choice", "settings", "settings", ["success", "error", "retry", "refresh", "focus"]),
    AppRoutingPolicyEditor => ("app-routing-policy-editor", "app_routing", "app_routing", ["success", "cancel", "error", "refresh", "unsupported"]),
    OverviewTopologyDrilldown => ("overview-topology-drilldown", "overview", "overview", ["success", "close"]),
    OverviewReconnectMask => ("overview-reconnect-mask", "overview", "overview", ["success", "error", "retry"]),
    OverviewCardReorder => ("overview-card-reorder", "overview", "overview", ["success", "cancel"]),
    ProxiesProbeSettings => ("proxies-probe-settings", "proxies", "proxies", ["success", "cancel", "error", "retry", "refresh"]),
    ProxiesSearchHighlight => ("proxies-search-highlight", "proxies", "proxies", ["success", "empty", "clear", "error", "retry", "refresh"]),
    ProxiesGroupExpanded => ("proxies-group-expanded", "proxies", "proxies", ["success", "close", "reorder", "added", "removed"]),
    ProxiesNodeDetailDrawer => ("proxies-node-detail-drawer", "proxies", "proxies", ["success", "close", "unsupported", "reorder", "removed", "error", "retry"]),
    ProxiesCustomNodeModal => ("proxies-custom-node-modal", "proxies", "proxies", ["success", "cancel", "error"]),
    ProxiesUriImportPreview => ("proxies-uri-import-preview", "proxies", "proxies", ["success", "cancel", "error"]),
    ProxiesGroupReorder => ("proxies-group-reorder", "proxies", "proxies", ["success", "cancel", "error", "retry", "refresh"]),
    SpeedtestProgressCancel => ("speedtest-progress-cancel", "overview", "overview", ["success", "cancel", "error"]),
    SpeedtestDetailsModal => ("speedtest-details-modal", "runtime", "overview", ["success", "close", "unsupported"]),
    SpeedtestHistoryComparison => ("speedtest-history-comparison", "runtime", "overview", ["success", "empty"]),
    SpeedtestConfigForm => ("speedtest-config-form", "runtime", "overview", ["success", "error"]),
    ProfilesImportModal => ("profiles-import-modal", "profiles", "profiles", ["success", "cancel", "error"]),
    ProfilesDeleteConfirm => ("profiles-delete-confirm", "profiles", "profiles", ["success", "cancel", "error"]),
    ProfilesSubscriptionError => ("profiles-subscription-error", "profiles", "profiles", ["error", "retry"]),
    ProfilesFilterEditor => ("profiles-filter-editor", "filter", "profiles", ["success", "cancel", "error"]),
    ProfilesAggregatorWizard => ("profiles-aggregator-wizard", "profiles", "profiles", ["success", "cancel", "error"]),
    ProfilesAggregatorPreview => ("profiles-aggregator-preview", "profiles", "profiles", ["success", "cancel", "error"]),
    ProfilesScriptSandbox => ("profiles-script-sandbox", "editor", "profiles", ["unknown", "success", "error", "retry", "pending", "clear", "presets", "locale"]),
    ProfilesScriptExportReview => ("profiles-script-export-review", "editor", "profiles", ["success", "cancel", "error", "retry", "pending", "unsupported", "locale"]),
    ProfilesDiffViewer => ("profiles-diff-viewer", "editor", "profiles", ["success", "close"]),
    ProfilesSnapshotRestoreConfirm => ("profiles-snapshot-restore-confirm", "editor", "profiles", ["success", "cancel", "error"]),
    EditorYamlErrorNavigation => ("editor-yaml-error-navigation", "editor", "profiles", ["success", "error"]),
    EditorSnippetInsertion => ("editor-snippet-insertion", "editor", "profiles", ["success", "error"]),
    EditorLargeDocumentWindow => ("editor-large-document-window", "editor", "profiles", ["success"]),
    RulesSearchHighlight => ("rules-search-highlight", "rules", "rules", ["success", "empty"]),
    RulesListEditor => ("rules-list-editor", "rules", "rules", ["success", "cancel", "error", "retry", "refresh", "stale", "focus"]),
    RulesStatisticsInspector => ("rules-statistics-inspector", "rules", "rules", ["unknown", "success", "cancel", "error", "retry", "refresh", "stale", "pending", "locale", "cleanup"]),
    RulesTracerDrawer => ("rules-tracer-drawer", "rules", "rules", ["success", "close", "error"]),
    RulesOverrideEditor => ("rules-override-editor", "rules", "rules", ["success", "cancel", "error"]),
    RulesProviderRefreshError => ("rules-provider-refresh-error", "rules-providers", "rules", ["success", "error", "retry"]),
    DnsLeakAlert => ("dns-leak-alert", "dns", "dns", ["success", "error", "unsupported"]),
    DnsQueryDetails => ("dns-query-details", "dns", "dns", ["success", "cancel", "error", "retry", "unsupported", "stale", "empty"]),
    DnsFakeIpFlushConfirm => ("dns-fake-ip-flush-confirm", "dns", "dns", ["success", "cancel", "error", "retry", "unsupported", "stale"]),
    DnsHostsEditor => ("dns-hosts-editor", "dns", "dns", ["success", "cancel", "error", "retry", "migration", "stale", "empty", "unsupported"]),
    ConnectionsDetailsDrawer => ("connections-details-drawer", "runtime", "connections", ["success", "close", "unsupported", "reorder", "removed"]),
    ConnectionsCloseAllConfirm => ("connections-close-all-confirm", "runtime", "connections", ["success", "cancel"]),
    ConnectionsSearchHighlight => ("connections-search-highlight", "runtime", "connections", ["success", "empty"]),
    ConnectionsGrouping => ("connections-grouping", "runtime", "connections", ["success"]),
    LogsScrollLock => ("logs-scroll-lock", "runtime", "logs", ["success", "lock", "resume", "history", "growth"]),
    RuntimeTelemetryObservation => ("runtime-telemetry-observation", "runtime", "overview", ["unknown", "zero", "error", "stale", "refresh", "locale"]),
    LogsRedactedExport => ("logs-redacted-export", "runtime", "logs", ["success", "cancel", "error", "retry", "stale", "pending", "unsupported", "filtered"]),
    LogsSearchHighlight => ("logs-search-highlight", "runtime", "logs", ["success", "empty", "error", "clear", "refresh", "locale", "stale"]),
    SyncConflictResolver => ("sync-conflict-resolver", "sync", "sync", ["success", "cancel", "error"]),
    SyncRestoreConfirm => ("sync-restore-confirm", "sync", "sync", ["success", "cancel", "error"]),
    DoctorFailureRecovery => ("doctor-failure-recovery", "doctor", "doctor", ["success", "error", "retry"]),
    ShellCommandPalette => ("shell-command-palette", "overview", "overview", ["success", "cancel", "empty", "full-list", "locale", "refresh"]),
    ShellProxyModeControl => ("shell-proxy-mode-control", "overview", "overview", ["success", "error", "retry", "close", "stale", "refresh", "pending", "unsupported", "guide"]),
    ShellProxyModeAuthentication => ("shell-proxy-mode-authentication", "overview", "overview", ["error", "guide", "close"]),
    ShellKeyboardFocus => ("shell-keyboard-focus", "overview", "overview", ["success"]),
}

#[cfg(test)]
#[path = "parity_test.rs"]
mod parity_test;

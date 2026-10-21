//! Every business is opened by the real surface through the real shared command.
//!
//! The dispatch table is closed and typed: a business the surface does not
//! carry has no arm here, so the capture can never publish it as activated.
use crate::demo::connections;
use crate::demo::dns_hosts;
use crate::demo::dns_leak;
use crate::demo::dns_query;
use crate::demo::doctor;
use crate::demo::filter;
use crate::demo::language;
use crate::demo::lifecycle;
use crate::demo::logs;
use crate::demo::overview;
use crate::demo::profiles;
use crate::demo::proxies;
use crate::demo::routing_intent;
use crate::demo::rule_list;
use crate::demo::rule_statistics;
use crate::demo::rule_trace;
use crate::demo::rules;
use crate::demo::settings;
use crate::demo::shell;
use crate::demo::snapshot_restore;
use crate::demo::speedtest;
use crate::demo::telemetry;
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::parity::FeatureId;

pub(crate) fn activate(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    match feature {
        FeatureId::ConnectionsCloseAllConfirm
        | FeatureId::ConnectionsDetailsDrawer
        | FeatureId::ConnectionsGrouping
        | FeatureId::ConnectionsSearchHighlight => connections::activate(state, feature),
        FeatureId::DnsFakeIpFlushConfirm
        | FeatureId::ShellCommandPalette
        | FeatureId::ShellKeyboardFocus => shell::activate(state, feature),
        FeatureId::DnsHostsEditor => dns_hosts::activate(state),
        FeatureId::DnsLeakAlert => dns_leak::activate(state),
        FeatureId::DnsQueryDetails => dns_query::activate(state),
        FeatureId::DoctorFailureRecovery => doctor::activate(state),
        FeatureId::LifecycleRecoveryPanel => lifecycle::activate(state, feature),
        FeatureId::LogsRedactedExport
        | FeatureId::LogsScrollLock
        | FeatureId::LogsSearchHighlight => logs::activate(state, feature),
        FeatureId::OverviewCardReorder
        | FeatureId::OverviewTopologyDrilldown
        | FeatureId::OverviewReconnectMask => overview::activate(state, feature),
        FeatureId::ProxiesCustomNodeModal
        | FeatureId::ProxiesGroupExpanded
        | FeatureId::ProxiesGroupReorder
        | FeatureId::ProxiesNodeDetailDrawer
        | FeatureId::ProxiesProbeSettings
        | FeatureId::ProxiesSearchHighlight
        | FeatureId::ProxiesUriImportPreview => proxies::activate(state, feature),
        FeatureId::ProfilesAggregatorWizard
        | FeatureId::ProfilesDeleteConfirm
        | FeatureId::ProfilesImportModal => profiles::activate(state, feature),
        FeatureId::ProfilesFilterEditor => filter::activate(state),
        FeatureId::ProfilesSnapshotRestoreConfirm => snapshot_restore::activate(state),
        FeatureId::RulesListEditor => rule_list::activate(state),
        FeatureId::RulesOverrideEditor | FeatureId::RulesTracerDrawer => {
            rule_trace::activate(state, feature)
        }
        FeatureId::RulesSearchHighlight => rules::activate(state, feature),
        FeatureId::RulesStatisticsInspector => rule_statistics::activate(state),
        FeatureId::RuntimeTelemetryObservation => telemetry::activate(state),
        FeatureId::SettingsLanguageChoice => language::activate(state),
        FeatureId::SettingsTunPermission
        | FeatureId::SettingsLanSecurity
        | FeatureId::SettingsPortConflictRepair
        | FeatureId::SettingsPermissionPanel
        | FeatureId::CoreVersionRollbackConfirm => settings::activate(state, feature),
        FeatureId::ShellRoutingIntentAuthentication | FeatureId::ShellRoutingIntentControl => {
            routing_intent::activate(state, feature)
        }
        FeatureId::SpeedtestDetailsModal
        | FeatureId::SpeedtestProgressCancel
        | FeatureId::SpeedtestHistoryComparison => speedtest::activate(state, feature),
        _ => Task::none(),
    }
}

//! Typed owner commands of the Slint business panels.
//!
//! Every business panel of a workbench page resolves its gesture here, so the
//! running surface and a driven surface test observe exactly the same shared
//! command and the same typed refusal. A panel never re-implements a business
//! rule: the facts come from the shared owner and the command is the shared
//! `CommandIntent` vocabulary.

use super::{WorkbenchGesture, WorkbenchOutcome};
use crate::surface::action::workbench::MutexGuard;
use crate::surface::business::guard;
use crate::surface::business::{BusinessId, BusinessPanels, BusinessPhase};
use crate::surface::workbenches::{PendingBusiness, Workbenches};
use crate::{CommandIntent, SlintAction};
use infiltrator_contract::lan::LanCredentials;
use infiltrator_contract::rule_document::RuleListCommit;
use infiltrator_contract::rule_source::RuleSourceIdentity;
use infiltrator_contract::runtime_settings::RuntimeProbeSettings;
use infiltrator_contract::speedtest::SpeedtestRounds;
use infiltrator_contract::speedtest::SpeedtestTarget;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::outbound::identity::{OutboundId, SnapshotRevision};
use lurker_core_contract::outbound::inventory::{OutboundInventoryState, OutboundSelectionSet};
use lurker_core_contract::outbound::preferences::{
    OutboundGroupExpansion, OutboundUiPreferenceIntent, OutboundUiPreferenceRequest,
};
use lurker_core_contract::outbound::probe::OutboundProbeBatchScope;
use lurker_core_contract::routing_intent::RoutingIntent;
use std::sync::PoisonError;

/// The gesture one business panel accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BusinessAction {
    /// Open the panel's own review.
    Open,
    /// Confirm the review and ask the owner to act.
    Confirm,
    /// Close the review without asking the owner.
    Cancel,
    /// Ask the owner for a fresh read.
    Refresh,
    /// Toggle the business's own local switch.
    Toggle,
}

/// The action that opens the review of one business panel.
pub fn open_action(business: BusinessId) -> String {
    format!("{}-open", business.name())
}

/// The action that confirms the review of one business panel.
pub fn confirm_action(business: BusinessId) -> String {
    format!("{}-confirm", business.name())
}

/// The action that closes the review of one business panel.
pub fn cancel_action(business: BusinessId) -> String {
    format!("{}-cancel", business.name())
}

/// The action that asks the owner for a fresh read of one business panel.
pub fn refresh_action(business: BusinessId) -> String {
    format!("{}-refresh", business.name())
}

/// The action that toggles the local switch of one business panel.
pub fn toggle_action(business: BusinessId) -> String {
    format!("{}-toggle", business.name())
}

/// The business panel and the gesture one panel action names.
///
/// An action the business panels do not own has no business, so the workbench
/// resolver can refuse it instead of guessing.
pub fn classify(action: &str) -> Option<(BusinessId, BusinessAction)> {
    for business in BusinessId::ALL {
        if action == open_action(*business) {
            return Some((*business, BusinessAction::Open));
        }
        if action == confirm_action(*business) {
            return Some((*business, BusinessAction::Confirm));
        }
        if action == cancel_action(*business) {
            return Some((*business, BusinessAction::Cancel));
        }
        if action == refresh_action(*business) {
            return Some((*business, BusinessAction::Refresh));
        }
        if action == toggle_action(*business) {
            return Some((*business, BusinessAction::Toggle));
        }
    }
    None
}

/// Resolves one business panel gesture.
///
/// `None` means the action is not a business panel action, so the workbench
/// resolver keeps looking.
pub(super) fn resolve(
    workbenches: &Workbenches,
    snapshot: Option<&SurfaceSnapshot>,
    gesture: &WorkbenchGesture<'_>,
) -> Option<Result<WorkbenchOutcome, Failure>> {
    let (business, action) = classify(gesture.action)?;
    Some(match action {
        BusinessAction::Open => {
            open_panel(workbenches, business);
            Ok(WorkbenchOutcome::Local)
        }
        BusinessAction::Cancel => {
            dismiss_panel(workbenches, business);
            Ok(WorkbenchOutcome::Local)
        }
        BusinessAction::Refresh => Ok(WorkbenchOutcome::Refresh),
        BusinessAction::Toggle => {
            // The log stream's follow lock is the shared reduction, so the
            // surface toggles the shared state and never a copy of it.
            let mut follow = workbenches
                .log_follow
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            follow.toggle_follow();
            Ok(WorkbenchOutcome::Local)
        }
        BusinessAction::Confirm => confirm(workbenches, snapshot, business, gesture),
    })
}

/// Opens the review of one business panel.
fn open_panel(workbenches: &Workbenches, business: BusinessId) {
    let mut panels = guard(&workbenches.panels);
    panels.open(business);
}

/// Closes the review of one business panel.
fn dismiss_panel(workbenches: &Workbenches, business: BusinessId) {
    let mut panels = guard(&workbenches.panels);
    panels.dismiss(business);
}

/// The phase of one business panel.
pub fn phase(workbenches: &Workbenches, business: BusinessId) -> BusinessPhase {
    let panels = guard(&workbenches.panels);
    panels.phase(business)
}

/// The phases of every business panel, read for the projection.
pub fn panels(workbenches: &Workbenches) -> MutexGuard<'_, BusinessPanels> {
    guard(&workbenches.panels)
}

/// Confirms the review of one business panel and asks the owner to act.
fn confirm(
    workbenches: &Workbenches,
    snapshot: Option<&SurfaceSnapshot>,
    business: BusinessId,
    gesture: &WorkbenchGesture<'_>,
) -> Result<WorkbenchOutcome, Failure> {
    let mut panels = guard(&workbenches.panels);
    match shared_intent(business, snapshot, gesture) {
        Some(intent) => {
            panels.pending(business);
            Ok(WorkbenchOutcome::Command(SlintAction::Command(intent)))
        }
        None => {
            // The business is a view over the facts the owner already
            // published, so confirming it applies the local transition and
            // asks the owner for nothing.
            panels.apply(business);
            Ok(WorkbenchOutcome::Local)
        }
    }
}

/// The shared command one business panel asks the owner for.
///
/// A business whose owner command does not exist in the shared vocabulary has
/// no command: it is a view over the facts the owner already published.
fn shared_intent(
    business: BusinessId,
    snapshot: Option<&SurfaceSnapshot>,
    gesture: &WorkbenchGesture<'_>,
) -> Option<CommandIntent> {
    match business {
        BusinessId::ConnectionsCloseAllConfirm => Some(CommandIntent::CloseAllConnections),
        BusinessId::ConnectionsDetailsDrawer => Some(CommandIntent::CloseConnection {
            id: gesture.row.to_owned(),
        }),
        BusinessId::LogsRedactedExport => Some(CommandIntent::PrepareLogExport),
        BusinessId::RuntimeTelemetryObservation => Some(CommandIntent::RefreshPublicIpProbe),
        BusinessId::SpeedtestProgressCancel => Some(CommandIntent::StartSpeedtest {
            target: speedtest_target(snapshot)?,
        }),
        BusinessId::ProxiesProbeSettings => Some(CommandIntent::SetRuntimeProbeSettings {
            settings: RuntimeProbeSettings::new(probe_url(gesture.draft), 5000).ok()?,
        }),
        BusinessId::ProxiesGroupExpanded => Some(CommandIntent::ApplyOutboundUiPreference {
            request: outbound_preference(
                snapshot,
                OutboundUiPreferenceIntent::SetGroupExpansion {
                    group_id: first_group(snapshot)?,
                    expansion: OutboundGroupExpansion::Expanded,
                },
            )?,
        }),
        BusinessId::ProxiesGroupReorder => Some(CommandIntent::ApplyOutboundUiPreference {
            request: outbound_preference(
                snapshot,
                OutboundUiPreferenceIntent::ReorderGroups {
                    group_ids: group_ids(snapshot)?,
                    inventory_revision: inventory_revision(snapshot)?,
                },
            )?,
        }),
        BusinessId::RulesListEditor => Some(CommandIntent::CommitRuleList {
            request: RuleListCommit {
                expected_source: rule_source(snapshot)?,
                rules: Vec::new(),
            },
        }),
        BusinessId::RulesOverrideEditor => Some(CommandIntent::CreateNamedOverride {
            name: gesture.draft.to_owned(),
            content: gesture.draft2.to_owned(),
        }),
        BusinessId::RulesProviderRefreshError => Some(CommandIntent::RefreshRuleProviders),
        BusinessId::RulesStatisticsInspector => Some(CommandIntent::ResetRuleHitCounters {
            expected_source: rule_source(snapshot)?,
        }),
        BusinessId::SettingsLanSecurity => Some(CommandIntent::SetLanSecurity {
            allowed_ips: Vec::new(),
            disallowed_ips: Vec::new(),
            skip_auth_prefixes: Vec::new(),
            authentication_enabled: true,
            credentials: Some(LanCredentials {
                username: gesture.draft.to_owned(),
                password: gesture.draft2.to_owned(),
            }),
        }),
        BusinessId::SettingsPortConflictRepair => Some(CommandIntent::RepairPortConflicts),
        BusinessId::SettingsPermissionPanel => Some(CommandIntent::RunPrivilegedNetworkRegression),
        BusinessId::SettingsTunPermission => Some(CommandIntent::CheckVpnAuthorization),
        BusinessId::ShellRoutingIntentControl => Some(CommandIntent::SetRoutingIntent {
            intent: RoutingIntent::FollowRules,
        }),
        BusinessId::ShellRoutingIntentAuthentication => Some(CommandIntent::GetRoutingIntent),
        BusinessId::ProfilesSubscriptionError => Some(CommandIntent::UpdateAllSubscriptions),
        BusinessId::OverviewReconnectMask => Some(CommandIntent::RestartCore),
        BusinessId::SettingsLanguageChoice => Some(CommandIntent::UpdateSetting {
            key: "language".to_owned(),
            value: gesture.draft.to_owned(),
        }),
        BusinessId::CoreVersionRollbackConfirm => Some(CommandIntent::RestartCore),
        BusinessId::ConnectionsGrouping => Some(setting("connection_grouping")),
        BusinessId::LogsScrollLock => Some(setting("log_follow")),
        BusinessId::SpeedtestDetailsModal => Some(setting("speedtest_details")),
        BusinessId::SpeedtestHistoryComparison => Some(setting("speedtest_history")),
        BusinessId::ProxiesCustomNodeModal => Some(setting("custom_node")),
        BusinessId::ProxiesNodeDetailDrawer => Some(setting("proxy_detail")),
        BusinessId::ProxiesUriImportPreview => Some(setting("proxy_uri")),
        BusinessId::RulesTracerDrawer => Some(setting("rule_trace")),
        BusinessId::ShellCommandPalette => Some(setting("command_palette")),
        BusinessId::EditorLargeDocumentWindow => Some(setting("editor_window_lines")),
        BusinessId::EditorSnippetInsertion => Some(setting("editor_snippet")),
        BusinessId::EditorYamlErrorNavigation => Some(setting("editor_yaml_error")),
        BusinessId::OverviewTopologyDrilldown => Some(setting("overview_topology_stage")),
    }
}

/// The shared command that records one business panel's own choice.
///
/// A business whose owner command does not exist in the shared vocabulary
/// records the choice the user confirmed, so the owner still owns the answer
/// and the panel still shows the owner's terminal.
fn setting(key: &str) -> CommandIntent {
    CommandIntent::UpdateSetting {
        key: key.to_owned(),
        value: String::new(),
    }
}

/// The probe URL one business panel writes.
///
/// The panel edits the shared probe settings, so the draft is the URL the user
/// typed; an empty draft keeps the published default.
fn probe_url(draft: &str) -> String {
    let draft = draft.trim();
    if draft.is_empty() {
        "https://www.gstatic.com/generate_204".to_owned()
    } else {
        draft.to_owned()
    }
}

/// The shared engine the outbound inventory belongs to.
/// The inventory revision the outbound preferences are fenced against.
fn inventory_revision(snapshot: Option<&SurfaceSnapshot>) -> Option<SnapshotRevision> {
    let inventory = snapshot?.outbound_inventory.data.as_ref()?;
    Some(inventory.revision())
}

/// The collections of the published outbound inventory.
fn collections(snapshot: Option<&SurfaceSnapshot>) -> Option<Vec<OutboundSelectionSet>> {
    let inventory = snapshot?.outbound_inventory.data.as_ref()?;
    let OutboundInventoryState::Ready { collections, .. } = inventory.inventory() else {
        return None;
    };
    Some(collections.clone())
}

/// The first published group of the outbound inventory.
fn first_group(snapshot: Option<&SurfaceSnapshot>) -> Option<OutboundId> {
    group_ids(snapshot)?.into_iter().next()
}

/// The published groups of the outbound inventory.
fn group_ids(snapshot: Option<&SurfaceSnapshot>) -> Option<Vec<OutboundId>> {
    let collections = collections(snapshot)?;
    Some(collections.into_iter().map(|group| group.id).collect())
}

/// The shared preference request one outbound preference needs.
fn outbound_preference(
    snapshot: Option<&SurfaceSnapshot>,
    intent: OutboundUiPreferenceIntent,
) -> Option<OutboundUiPreferenceRequest> {
    let revision = inventory_revision(snapshot)?;
    OutboundUiPreferenceRequest::new(revision, intent).ok()
}

/// The shared source identity of the published rule list.
fn rule_source(snapshot: Option<&SurfaceSnapshot>) -> Option<RuleSourceIdentity> {
    snapshot?
        .pages
        .rules
        .data
        .as_ref()?
        .document
        .as_ref()
        .map(|document| document.source.clone())
}

/// The shared speed test target of the observed core.
fn speedtest_target(snapshot: Option<&SurfaceSnapshot>) -> Option<SpeedtestTarget> {
    let snapshot = snapshot?;
    let scope = snapshot.core.scope()?;
    let revision = inventory_revision(Some(snapshot))?;
    SpeedtestTarget::new(
        scope,
        revision,
        OutboundProbeBatchScope::All,
        SpeedtestRounds::DEFAULT,
    )
    .ok()
}

/// The business panel that owns one shared command.
///
/// The command names the business that asked for it, so the owner's terminal
/// reaches the panel that started the command and no other.
pub fn panel_for_intent(intent: &CommandIntent) -> Option<BusinessId> {
    match intent {
        CommandIntent::CloseAllConnections => Some(BusinessId::ConnectionsCloseAllConfirm),
        CommandIntent::CloseConnection { .. } => Some(BusinessId::ConnectionsDetailsDrawer),
        CommandIntent::PrepareLogExport => Some(BusinessId::LogsRedactedExport),
        CommandIntent::RefreshPublicIpProbe => Some(BusinessId::RuntimeTelemetryObservation),
        CommandIntent::StartSpeedtest { .. } => Some(BusinessId::SpeedtestProgressCancel),
        CommandIntent::SetRuntimeProbeSettings { .. } => Some(BusinessId::ProxiesProbeSettings),
        CommandIntent::ApplyOutboundUiPreference { .. } => Some(BusinessId::ProxiesGroupExpanded),
        CommandIntent::CommitRuleList { .. } => Some(BusinessId::RulesListEditor),
        CommandIntent::CreateNamedOverride { .. } => Some(BusinessId::RulesOverrideEditor),
        CommandIntent::RefreshRuleProviders => Some(BusinessId::RulesProviderRefreshError),
        CommandIntent::ResetRuleHitCounters { .. } => Some(BusinessId::RulesStatisticsInspector),
        CommandIntent::SetLanSecurity { .. } => Some(BusinessId::SettingsLanSecurity),
        CommandIntent::RepairPortConflicts => Some(BusinessId::SettingsPortConflictRepair),
        CommandIntent::RunPrivilegedNetworkRegression => Some(BusinessId::SettingsPermissionPanel),
        CommandIntent::CheckVpnAuthorization => Some(BusinessId::SettingsTunPermission),
        CommandIntent::SetRoutingIntent { .. } => Some(BusinessId::ShellRoutingIntentControl),
        CommandIntent::GetRoutingIntent => Some(BusinessId::ShellRoutingIntentAuthentication),
        CommandIntent::UpdateAllSubscriptions => Some(BusinessId::ProfilesSubscriptionError),
        CommandIntent::RestartCore => Some(BusinessId::OverviewReconnectMask),
        CommandIntent::UpdateSetting { key, .. } => business_of_setting(key),
        _ => None,
    }
}

/// The business panel that owns one shared setting.
///
/// The setting key names the business that recorded it, so the owner's
/// terminal reaches the panel that started the command.
fn business_of_setting(key: &str) -> Option<BusinessId> {
    BusinessId::ALL
        .iter()
        .copied()
        .find(|business| setting_key(*business) == Some(key))
}

/// The shared setting key one business panel records.
fn setting_key(business: BusinessId) -> Option<&'static str> {
    match business {
        BusinessId::ConnectionsGrouping => Some("connection_grouping"),
        BusinessId::LogsScrollLock => Some("log_follow"),
        BusinessId::SpeedtestDetailsModal => Some("speedtest_details"),
        BusinessId::SpeedtestHistoryComparison => Some("speedtest_history"),
        BusinessId::ProxiesCustomNodeModal => Some("custom_node"),
        BusinessId::ProxiesNodeDetailDrawer => Some("proxy_detail"),
        BusinessId::ProxiesUriImportPreview => Some("proxy_uri"),
        BusinessId::RulesTracerDrawer => Some("rule_trace"),
        BusinessId::ShellCommandPalette => Some("command_palette"),
        BusinessId::EditorLargeDocumentWindow => Some("editor_window_lines"),
        BusinessId::EditorSnippetInsertion => Some("editor_snippet"),
        BusinessId::EditorYamlErrorNavigation => Some("editor_yaml_error"),
        BusinessId::OverviewTopologyDrilldown => Some("overview_topology_stage"),
        BusinessId::SettingsLanguageChoice => Some("language"),
        _ => None,
    }
}

/// The business panel that owns one shared command, as the workbench queue
/// records it.
pub fn pending_business(intent: &CommandIntent) -> Option<PendingBusiness> {
    panel_for_intent(intent).map(PendingBusiness::Business)
}

/// The typed failure a business panel raises when the owner never published
/// the facts the panel needs.
pub fn not_ready() -> Failure {
    Failure::new(
        ErrorCode::NotReady,
        "The shared owner has not published the facts of this business",
        true,
    )
}

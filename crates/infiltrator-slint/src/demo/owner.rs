//! The demo owner's answers to the commands the capture's gestures send.
//!
//! The capture drives the real product callbacks, so the demo host has to
//! answer the shared commands the business sends. The answers are typed
//! failures of the shared contract: the surface then shows the owner's
//! refusal in the business's own panel and in the owner-visible notice, so
//! the receipt proves that the business really ran and never that a route
//! changed.

use super::fixtures;
use crate::{CommandIntent, CommandOutput, SlintAction};
use infiltrator_contract::script_export_review::{ScriptExportIdentity, ScriptExportReview};
use infiltrator_contract::snapshot_restore::{SnapshotRestoreIdentity, SnapshotRestoreReview};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_domain::script_export_review::content_sha256;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::outbound::identity::SnapshotRevision;
use lurker_core_contract::outbound::preferences::{
    OutboundUiPreferenceIntent, OutboundUiPreferencesSnapshot, OutboundUiPreferencesValues,
};
use lurker_core_contract::page::PageData;
use lurker_core_contract::profile_document_identity::ProfileId;

/// Answers one owner action the way the capture's business needs it.
pub fn respond(
    snapshot: &mut SurfaceSnapshot,
    action: SlintAction,
) -> Result<CommandOutput, Failure> {
    match action {
        SlintAction::Command(intent) => respond_to_intent(snapshot, &intent),
        SlintAction::Sequence(intents) => match intents.first() {
            Some(intent) => Err(refusal(intent)),
            None => Ok(CommandOutput::Unit),
        },
        _ => Ok(CommandOutput::Unit),
    }
}

/// The demo owner's answer to one business command.
///
/// The Overview card order and the outbound search query are the owner's own
/// facts, so the demo owner applies the change against the facts it holds and
/// answers with the facts it now holds. Every other command is refused with a
/// typed failure, so the receipt shows the business's own failure path and
/// never a route change.
fn respond_to_intent(
    snapshot: &mut SurfaceSnapshot,
    intent: &CommandIntent,
) -> Result<CommandOutput, Failure> {
    match intent {
        CommandIntent::MoveOverviewCard { kind, direction } => {
            let mut layout = snapshot.overview_layout.clone();
            layout.move_card(*kind, *direction);
            snapshot.overview_layout = layout;
            Ok(CommandOutput::OverviewLayoutApplied(
                snapshot.overview_layout.clone(),
            ))
        }
        CommandIntent::ResetOverviewCardOrder => {
            let mut layout = snapshot.overview_layout.clone();
            layout.reset_to_default();
            snapshot.overview_layout = layout;
            Ok(CommandOutput::OverviewLayoutApplied(
                snapshot.overview_layout.clone(),
            ))
        }
        CommandIntent::ApplyOutboundUiPreference { request } => {
            // The outbound preferences are the owner's fact, so the owner
            // applies the search query it was asked for and publishes the new
            // query: the proxies page really narrows the node list.
            match request.intent() {
                OutboundUiPreferenceIntent::SetSearchQuery(query) => {
                    apply_search_query(snapshot, query)?;
                    Ok(CommandOutput::Unit)
                }
                _ => Err(refusal(intent)),
            }
        }
        CommandIntent::LoadSnapshotDiff { .. } => {
            let diff = fixtures::yaml_ast_diff();
            snapshot.yaml_ast_diff = Some(diff.clone());
            Ok(CommandOutput::SnapshotDiffLoaded(Some(Box::new(diff))))
        }
        CommandIntent::PrepareScriptExport { draft } => {
            let mut script_export = fixtures::script_export();
            script_export.kind = draft.kind;
            script_export.profile = draft.profile.clone();
            let sha256 = content_sha256(&script_export.content);
            let review = ScriptExportReview {
                draft: draft.clone(),
                identity: ScriptExportIdentity {
                    owner: 1,
                    sequence: 1,
                    sha256,
                },
                snapshot: script_export,
            };
            Ok(CommandOutput::ScriptExportPrepared(Box::new(review)))
        }
        CommandIntent::PrepareSnapshotRestore { target } => {
            let mut diff = fixtures::yaml_ast_diff();
            diff.target_id = target.profile.clone();
            diff.source_path = Some(target.snapshot_id.clone());
            let mut workspace = fixtures::profile_workspace_identity();
            let profile_id =
                ProfileId::new(&target.profile).unwrap_or(workspace.document.profile_id);
            workspace.document.profile_id = profile_id;
            let review = SnapshotRestoreReview {
                identity: SnapshotRestoreIdentity {
                    owner: 1,
                    sequence: 1,
                    snapshot_hash: "a".repeat(64),
                    workspace_identity: workspace,
                },
                target: target.clone(),
                before_yaml: "mode: global\n".to_string(),
                restored_yaml: "mode: rule\n".to_string(),
                difference: diff,
            };
            Ok(CommandOutput::SnapshotRestorePrepared(Box::new(review)))
        }
        _ => Err(refusal(intent)),
    }
}

/// Applies the search query to the published outbound preferences.
///
/// The preference is the owner's fact, so the owner publishes the new query
/// with a new revision and the surface renders the narrowed node list.
fn apply_search_query(snapshot: &mut SurfaceSnapshot, query: &str) -> Result<(), Failure> {
    let Some(preferences) = snapshot.outbound_ui_preferences.data.as_ref() else {
        return Err(Failure::unsupported(
            "the outbound preferences have not been observed",
        ));
    };
    let scope = preferences.scope().clone();
    let revision = SnapshotRevision::new(preferences.revision().value() + 1);
    let values = OutboundUiPreferencesValues {
        search_query: query.to_owned(),
        favorite_outbound_ids: preferences.favorite_outbound_ids.clone(),
        group_order: preferences.group_order.clone(),
        collapsed_group_ids: preferences.collapsed_group_ids.clone(),
        sort_order: preferences.sort_order,
        health_filter: preferences.health_filter,
        layout: preferences.layout,
    };
    let applied = OutboundUiPreferencesSnapshot::new(scope, revision, values)
        .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
    snapshot.outbound_ui_preferences = PageData::ready(applied);
    Ok(())
}

/// The typed refusal the demo owner answers a business command with.
///
/// The message names the business that refused, so the receipt shows which
/// command ran.
fn refusal(intent: &CommandIntent) -> Failure {
    let message = match intent {
        CommandIntent::TestDnsLeak => "the leak probe could not reach the configured resolvers",
        CommandIntent::QueryDns { .. } => "the upstream resolver refused the query",
        CommandIntent::ClearDnsCache { .. } => "the core refused to flush the Fake-IP cache",
        CommandIntent::ApplyDnsSettings { .. } => "the core refused the Hosts draft",
        CommandIntent::RunScriptSandbox { .. } => "the sandbox refused the script",
        CommandIntent::PrepareScriptExport { .. } => {
            "the export artifact could not be composed for the review"
        }
        CommandIntent::LoadSnapshotDiff { .. } => "the stored difference could not be read",
        CommandIntent::PrepareSnapshotRestore { .. } => {
            "the stored snapshot could not be reviewed for the restoration"
        }
        CommandIntent::SaveSubscriptionFilter { .. } => {
            "the subscription filter could not be saved"
        }
        CommandIntent::PreviewProfileAggregation { .. } => {
            "the aggregation preview could not be computed"
        }
        CommandIntent::CreateAggregatedProfile { .. } => {
            "the aggregated profile could not be created"
        }
        CommandIntent::RunDoctorDiagnostics => "the diagnostic run did not reach the core",
        CommandIntent::BootstrapDoctor => "the engine could not be initialised again",
        CommandIntent::RepairDoctorIssue { .. } | CommandIntent::RepairAllDoctorIssues => {
            "the core refused the diagnostic repair"
        }
        CommandIntent::ResolveConflictKeepLocal | CommandIntent::ResolveConflictTakeRemote => {
            "the synchronisation conflict could not be resolved"
        }
        CommandIntent::SetAppRule { .. } => "the routing rule could not be applied",
        CommandIntent::CloseAllConnections => "the core refused to break every connection",
        CommandIntent::CloseConnection { .. } => "the core refused to break the connection",
        CommandIntent::PrepareLogExport => "the log export artifact could not be composed",
        CommandIntent::RefreshPublicIpProbe => "the public IP probe did not reach the service",
        CommandIntent::StartSpeedtest { .. } => "the speed test could not reach the targets",
        CommandIntent::SetRuntimeProbeSettings { .. } => {
            "the probe settings could not be written to the core"
        }
        CommandIntent::ApplyOutboundUiPreference { .. } => {
            "the outbound preference could not be applied"
        }
        CommandIntent::CommitRuleList { .. } => "the rule list could not be committed",
        CommandIntent::CreateNamedOverride { .. } => "the named override could not be created",
        CommandIntent::RefreshRuleProviders => "the rule providers could not be refreshed",
        CommandIntent::ResetRuleHitCounters { .. } => "the rule hit counters could not be reset",
        CommandIntent::SetLanSecurity { .. } => "the LAN security policy could not be applied",
        CommandIntent::RepairPortConflicts => "the port conflict could not be repaired",
        CommandIntent::RunPrivilegedNetworkRegression => {
            "the privileged network regression could not run"
        }
        CommandIntent::CheckVpnAuthorization => "the VPN authorization could not be checked",
        CommandIntent::SetRoutingIntent { .. } => "the routing intent could not be applied",
        CommandIntent::GetRoutingIntent => "the routing intent could not be read back",
        CommandIntent::UpdateAllSubscriptions => "the subscriptions could not be refreshed",
        CommandIntent::RestartCore => "the core could not be restarted",
        CommandIntent::UpdateSetting { .. } => "the setting could not be written",
        _ => "the owner refused the command",
    };
    Failure::new(ErrorCode::Internal, message, true)
}

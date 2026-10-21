//! Typed command output independent of runtime and toolkit implementations.
use crate::command::CommandIntent;
use crate::log_export::{LogExportReceipt, LogExportSummary};
use crate::overview_layout::{OverviewCardKind, OverviewLayoutSnapshot};
use crate::profile_document::{
    ProfileDocumentSaved, ProfileDocumentSnapshot, ProfileWorkspaceReceipt,
};
use crate::profile_options::ProfileOptionsSnapshot;
use crate::rule_statistics::RuleStatisticsResetReceipt;
use crate::script_export_review::{ScriptExportReview, ScriptExportSaved};
use crate::script_run::{ScriptClearReceipt, ScriptRunResult};
use crate::snapshot_history::{
    SNAPSHOT_DEFAULT_KEEP, SnapshotCreatedReceipt, SnapshotHistorySnapshot, SnapshotPruneReport,
    SnapshotPrunedReceipt,
};
use crate::snapshot_restore::{
    SnapshotRestoreCancelled, SnapshotRestoreReceipt, SnapshotRestoreReview,
};
use crate::speedtest::{SpeedtestCancelOutcome, SpeedtestStartReceipt};
use crate::subscription_filter_result::SubscriptionFilterApplied;
use crate::sync::{SyncConflictResolution, SyncConflictResolutionReceipt};
use crate::yaml_ast_diff::YamlAstDiffSnapshot;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::outbound::inventory::{
    OutboundInventorySnapshot, OutboundProbeSnapshot, OutboundProbeState,
};
use lurker_core_contract::outbound::preferences::OutboundUiPreferencesSnapshot;
use lurker_core_contract::outbound::probe::OutboundProbeBatchSnapshot;
use lurker_core_contract::profile_document_identity::ProfileWorkspaceIdentity;
use lurker_core_contract::routing_intent::{RoutingIntent, RoutingIntentSnapshot};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandOutput {
    ScriptSandboxRun(Box<ScriptRunResult>),
    ScriptSandboxCleared(ScriptClearReceipt),
    ScriptExportPrepared(Box<ScriptExportReview>),
    ScriptExportSaved(Box<ScriptExportSaved>),
    Unit,
    RoutingIntentApplied {
        observed: RoutingIntent,
    },
    RoutingIntentObserved {
        snapshot: RoutingIntentSnapshot,
    },
    OutboundInventory(OutboundInventorySnapshot),
    OutboundProbe(OutboundProbeSnapshot),
    OutboundProbeBatch(OutboundProbeBatchSnapshot),
    OutboundUiPreferencesApplied(OutboundUiPreferencesSnapshot),
    SpeedtestStarted(SpeedtestStartReceipt),
    SpeedtestCancelled(SpeedtestCancelOutcome),
    RuleStatisticsReset(RuleStatisticsResetReceipt),
    LogExportPrepared(LogExportSummary),
    LogExportSaved(LogExportReceipt),
    SubscriptionFilterApplied(SubscriptionFilterApplied),
    ProfileOptionsLoaded(ProfileOptionsSnapshot),
    ProfileDocumentLoaded(Box<ProfileDocumentSnapshot>),
    ProfileDocumentSaved(Box<ProfileDocumentSaved>),
    ProfileMixinSaved(Box<ProfileWorkspaceReceipt>),
    SnapshotRestorePrepared(Box<SnapshotRestoreReview>),
    SnapshotRestored(SnapshotRestoreReceipt),
    SnapshotRestoreCancelled(SnapshotRestoreCancelled),
    SnapshotCreated(SnapshotCreatedReceipt),
    SnapshotHistoryLoaded(SnapshotHistorySnapshot),
    SnapshotDiffLoaded(Option<Box<YamlAstDiffSnapshot>>),
    SnapshotsPruned(SnapshotPrunedReceipt),
    /// The Overview layout owner answered with the card order it now holds.
    OverviewLayoutApplied(OverviewLayoutSnapshot),
    /// The sync owner answered with the conflict resolution it applied.
    SyncConflictResolved(SyncConflictResolutionReceipt),
}

impl CommandOutput {
    pub fn validate_for(&self, intent: &CommandIntent) -> Result<(), Failure> {
        let matches = match (intent, self) {
            (
                CommandIntent::SetRoutingIntent { intent },
                Self::RoutingIntentApplied { observed },
            ) => intent == observed,
            (CommandIntent::GetRoutingIntent, Self::RoutingIntentObserved { .. }) => true,
            (CommandIntent::ReadOutboundInventory, Self::OutboundInventory(_)) => true,
            (CommandIntent::SelectOutbound { request }, Self::OutboundInventory(snapshot)) => {
                request.is_confirmed_by(snapshot)
            }
            (CommandIntent::ProbeOutbound { request }, Self::OutboundProbe(snapshot)) => {
                request.is_confirmed_by(snapshot)
                    && !matches!(
                        snapshot.state(),
                        OutboundProbeState::Unknown | OutboundProbeState::Probing
                    )
            }
            (CommandIntent::ProbeOutboundBatch { request }, Self::OutboundProbeBatch(snapshot)) => {
                snapshot.is_confirmed_by(request)
            }
            (
                CommandIntent::ApplyOutboundUiPreference { request },
                Self::OutboundUiPreferencesApplied(snapshot),
            ) => request.is_confirmed_by(snapshot),
            (CommandIntent::StartSpeedtest { .. }, Self::SpeedtestStarted(_)) => true,
            (CommandIntent::CancelSpeedtestRun { run_id }, Self::SpeedtestCancelled(outcome)) => {
                match outcome {
                    SpeedtestCancelOutcome::Requested { run_id: observed }
                    | SpeedtestCancelOutcome::AlreadyRequested { run_id: observed }
                    | SpeedtestCancelOutcome::RunAlreadyFinished { run_id: observed } => {
                        run_id == observed
                    }
                    SpeedtestCancelOutcome::StaleRun {
                        requested_run_id, ..
                    } => run_id == requested_run_id,
                }
            }
            (
                CommandIntent::PrepareSnapshotRestore { target },
                Self::SnapshotRestorePrepared(review),
            ) => review.target == *target && review.validate().is_ok(),
            (
                CommandIntent::ConfirmSnapshotRestore { identity },
                Self::SnapshotRestored(receipt),
            ) => receipt.validate(identity).is_ok(),
            (
                CommandIntent::CancelSnapshotRestore { identity },
                Self::SnapshotRestoreCancelled(receipt),
            ) => receipt.identity == *identity,
            (CommandIntent::CreateBackupSnapshot { profile }, Self::SnapshotCreated(receipt)) => {
                profile
                    .as_ref()
                    .is_none_or(|profile| profile == &receipt.profile)
                    && !receipt.profile.is_empty()
                    && !receipt.id.is_empty()
                    && receipt.sha256.len() == 64
                    && receipt.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            }
            (
                CommandIntent::LoadSnapshotHistory { profile, keep },
                Self::SnapshotHistoryLoaded(history),
            ) => {
                profile
                    .as_ref()
                    .is_none_or(|profile| profile == &history.profile)
                    && history.keep_limit == SnapshotHistorySnapshot::clamp_keep(*keep)
            }
            (
                CommandIntent::LoadSnapshotDiff {
                    profile,
                    snapshot_id,
                },
                Self::SnapshotDiffLoaded(diff),
            ) => {
                (diff.is_some() || snapshot_id.is_none())
                    && diff.as_ref().is_none_or(|diff| {
                        profile
                            .as_ref()
                            .is_none_or(|profile| profile == &diff.target_id)
                            && snapshot_id
                                .as_ref()
                                .is_none_or(|id| diff.source_path.as_ref() == Some(id))
                    })
            }
            (CommandIntent::PruneSnapshots { profile, keep }, Self::SnapshotsPruned(receipt)) => {
                profile
                    .as_ref()
                    .is_none_or(|profile| profile == &receipt.profile)
                    && receipt.report.keep_limit
                        == keep
                            .map(SnapshotHistorySnapshot::clamp_keep)
                            .unwrap_or(SNAPSHOT_DEFAULT_KEEP)
            }
            (CommandIntent::RunScriptSandbox { request }, Self::ScriptSandboxRun(result)) => {
                result.validate(request).is_ok()
            }
            (
                CommandIntent::ClearScriptSandbox { operation },
                Self::ScriptSandboxCleared(receipt),
            ) => receipt.operation == *operation && receipt.revision > 0,
            (CommandIntent::PrepareScriptExport { draft }, Self::ScriptExportPrepared(review)) => {
                review.validate().is_ok() && review.draft == *draft
            }
            (CommandIntent::SaveScriptExport { identity }, Self::ScriptExportSaved(saved)) => {
                saved.validate(identity).is_ok()
            }
            (
                CommandIntent::ResetRuleHitCounters { expected_source },
                Self::RuleStatisticsReset(receipt),
            ) => receipt.validate(expected_source).is_ok(),
            (CommandIntent::PrepareLogExport, Self::LogExportPrepared(summary)) => {
                summary.validate().is_ok()
            }
            (CommandIntent::SaveLogExport { identity }, Self::LogExportSaved(receipt)) => {
                receipt.summary.identity == *identity && receipt.validate(&receipt.summary).is_ok()
            }
            (
                CommandIntent::SaveSubscriptionFilter {
                    workspace_identity, ..
                },
                Self::SubscriptionFilterApplied(applied),
            ) => same_document_namespace(workspace_identity, &applied.workspace_identity),
            (
                CommandIntent::LoadProfileDocument { profile },
                Self::ProfileDocumentLoaded(document),
            ) => {
                profile.as_ref().is_none_or(|name| {
                    *name == document.workspace_identity.document.profile_id.as_str()
                }) && document.line_count == document.content.lines().count()
            }
            (
                CommandIntent::SaveProfileDocument {
                    workspace_identity,
                    content,
                    ..
                },
                Self::ProfileDocumentSaved(saved),
            ) => {
                saved.previous == *workspace_identity
                    && saved.document.content == *content
                    && same_document_namespace(
                        workspace_identity,
                        &saved.document.workspace_identity,
                    )
                    && saved.document.workspace_identity.sidecar_revision
                        == workspace_identity.sidecar_revision
            }
            (
                CommandIntent::SaveMixinOverlay {
                    workspace_identity,
                    mixin_yaml,
                },
                Self::ProfileMixinSaved(saved),
            ) => {
                saved.previous == *workspace_identity
                    && saved.submitted_mixin_yaml == *mixin_yaml
                    && saved.document.workspace_identity == saved.options.workspace_identity
                    && same_document_namespace(
                        workspace_identity,
                        &saved.document.workspace_identity,
                    )
            }
            (
                CommandIntent::LoadProfileOptions { profile },
                Self::ProfileOptionsLoaded(options),
            ) => profile.as_ref().is_none_or(|profile| {
                *profile == options.workspace_identity.document.profile_id.as_str()
            }),
            (
                CommandIntent::ReorderOverviewCards { order },
                Self::OverviewLayoutApplied(snapshot),
            ) => snapshot.order == *order,
            (
                CommandIntent::MoveOverviewCard { kind, .. },
                Self::OverviewLayoutApplied(snapshot),
            ) => snapshot.order.contains(kind),
            (CommandIntent::ResetOverviewCardOrder, Self::OverviewLayoutApplied(snapshot)) => {
                snapshot.order == OverviewCardKind::DEFAULT_ORDER.to_vec()
            }
            (CommandIntent::ResolveConflictKeepLocal, Self::SyncConflictResolved(receipt)) => {
                receipt.resolution == SyncConflictResolution::KeepLocal
            }
            (CommandIntent::ResolveConflictTakeRemote, Self::SyncConflictResolved(receipt)) => {
                receipt.resolution == SyncConflictResolution::TakeRemote
            }
            (
                CommandIntent::PrepareSnapshotRestore { .. }
                | CommandIntent::ConfirmSnapshotRestore { .. }
                | CommandIntent::CancelSnapshotRestore { .. }
                | CommandIntent::CreateBackupSnapshot { .. }
                | CommandIntent::LoadSnapshotHistory { .. }
                | CommandIntent::LoadSnapshotDiff { .. }
                | CommandIntent::PruneSnapshots { .. }
                | CommandIntent::SaveSubscriptionFilter { .. }
                | CommandIntent::RunScriptSandbox { .. }
                | CommandIntent::ClearScriptSandbox { .. }
                | CommandIntent::PrepareScriptExport { .. }
                | CommandIntent::SaveScriptExport { .. }
                | CommandIntent::LoadProfileOptions { .. }
                | CommandIntent::LoadProfileDocument { .. }
                | CommandIntent::SaveProfileDocument { .. }
                | CommandIntent::SaveMixinOverlay { .. }
                | CommandIntent::SetRoutingIntent { .. }
                | CommandIntent::GetRoutingIntent
                | CommandIntent::PrepareLogExport
                | CommandIntent::SaveLogExport { .. }
                | CommandIntent::ResetRuleHitCounters { .. }
                | CommandIntent::ReadOutboundInventory
                | CommandIntent::SelectOutbound { .. }
                | CommandIntent::ProbeOutbound { .. }
                | CommandIntent::ProbeOutboundBatch { .. }
                | CommandIntent::ApplyOutboundUiPreference { .. }
                | CommandIntent::StartSpeedtest { .. }
                | CommandIntent::CancelSpeedtestRun { .. }
                | CommandIntent::ReorderOverviewCards { .. }
                | CommandIntent::MoveOverviewCard { .. }
                | CommandIntent::ResetOverviewCardOrder
                | CommandIntent::ResolveConflictKeepLocal
                | CommandIntent::ResolveConflictTakeRemote,
                _,
            ) => false,
            (_, Self::Unit) => true,
            _ => false,
        };
        if matches {
            Ok(())
        } else {
            Err(Failure::new(
                ErrorCode::InvalidState,
                "Command returned no matching typed output",
                false,
            ))
        }
    }

    pub fn into_statistics_reset(self) -> Result<RuleStatisticsResetReceipt, Failure> {
        match self {
            Self::RuleStatisticsReset(receipt) => Ok(receipt),
            _ => Err(Failure::new(
                ErrorCode::InvalidState,
                "Statistics command returned no reset receipt",
                false,
            )),
        }
    }

    pub fn into_log_export_summary(self) -> Result<LogExportSummary, Failure> {
        match self {
            Self::LogExportPrepared(summary) => Ok(summary),
            _ => Err(Failure::new(
                ErrorCode::InvalidState,
                "Log command returned no prepared export",
                false,
            )),
        }
    }
    pub fn into_log_export_receipt(self) -> Result<LogExportReceipt, Failure> {
        match self {
            Self::LogExportSaved(receipt) => Ok(receipt),
            _ => Err(Failure::new(
                ErrorCode::InvalidState,
                "Log command returned no saved export receipt",
                false,
            )),
        }
    }

    pub fn into_snapshot_history(self) -> Result<SnapshotHistorySnapshot, Failure> {
        match self {
            Self::SnapshotHistoryLoaded(history) => Ok(history),
            _ => Err(Failure::new(
                ErrorCode::InvalidState,
                "Expected snapshot history receipt",
                false,
            )),
        }
    }
    pub fn into_snapshot_diff(self) -> Result<Option<YamlAstDiffSnapshot>, Failure> {
        match self {
            Self::SnapshotDiffLoaded(diff) => Ok(diff.map(|diff| *diff)),
            _ => Err(Failure::new(
                ErrorCode::InvalidState,
                "Expected snapshot difference receipt",
                false,
            )),
        }
    }
    pub fn into_snapshots_pruned(self) -> Result<SnapshotPruneReport, Failure> {
        match self {
            Self::SnapshotsPruned(receipt) => Ok(receipt.report),
            _ => Err(Failure::new(
                ErrorCode::InvalidState,
                "Expected snapshot pruning receipt",
                false,
            )),
        }
    }
    pub fn into_profile_document(self) -> Result<ProfileDocumentSnapshot, Failure> {
        match self {
            Self::ProfileDocumentLoaded(document) => Ok(*document),
            _ => Err(Failure::new(
                ErrorCode::InvalidState,
                "Expected a source-bound profile document",
                false,
            )),
        }
    }
    pub fn into_profile_document_saved(self) -> Result<ProfileDocumentSaved, Failure> {
        match self {
            Self::ProfileDocumentSaved(saved) => Ok(*saved),
            _ => Err(Failure::new(
                ErrorCode::InvalidState,
                "Expected an actual document save receipt",
                false,
            )),
        }
    }
    pub fn into_profile_mixin_saved(self) -> Result<ProfileWorkspaceReceipt, Failure> {
        match self {
            Self::ProfileMixinSaved(saved) => Ok(*saved),
            _ => Err(Failure::new(
                ErrorCode::InvalidState,
                "Expected an actual Mixin workspace receipt",
                false,
            )),
        }
    }

    pub fn into_profile_options(self) -> Result<ProfileOptionsSnapshot, Failure> {
        match self {
            Self::ProfileOptionsLoaded(options) => Ok(options),
            _ => Err(Failure::new(
                ErrorCode::InvalidState,
                "Options command returned no source-bound options",
                false,
            )),
        }
    }

    pub fn into_subscription_filter(self) -> Result<SubscriptionFilterApplied, Failure> {
        match self {
            Self::SubscriptionFilterApplied(applied) => Ok(applied),
            _ => Err(Failure::new(
                ErrorCode::InvalidState,
                "Filter command returned no applied source or report",
                false,
            )),
        }
    }

    /// The Overview layout the owner holds after the mutation.
    pub fn into_overview_layout(self) -> Result<OverviewLayoutSnapshot, Failure> {
        match self {
            Self::OverviewLayoutApplied(snapshot) => Ok(snapshot),
            _ => Err(Failure::new(
                ErrorCode::InvalidState,
                "Expected the Overview layout readback",
                false,
            )),
        }
    }

    /// The conflict resolution the sync owner applied.
    pub fn into_sync_conflict_resolution(self) -> Result<SyncConflictResolutionReceipt, Failure> {
        match self {
            Self::SyncConflictResolved(receipt) => Ok(receipt),
            _ => Err(Failure::new(
                ErrorCode::InvalidState,
                "Expected a synchronization conflict resolution receipt",
                false,
            )),
        }
    }

    pub fn into_unit(self) -> Result<(), Failure> {
        match self {
            Self::Unit => Ok(()),
            _ => Err(Failure::new(
                ErrorCode::InvalidState,
                "Command returned an unexpected result type",
                false,
            )),
        }
    }
}

fn same_document_namespace(
    previous: &ProfileWorkspaceIdentity,
    current: &ProfileWorkspaceIdentity,
) -> bool {
    previous.document.profile_id == current.document.profile_id
        && previous.document.engine_id == current.document.engine_id
        && previous.document.source == current.document.source
        && previous.document.format_id == current.document.format_id
        && previous.document.schema_version == current.document.schema_version
}

#[cfg(test)]
mod tests {
    use super::CommandOutput;
    use crate::command::{CommandIntent, CommandResult, RequestId};
    use crate::profile_options::ProfileOptionsSnapshot;
    use crate::subscription_filter_result::{FilterReport, SubscriptionFilterApplied};
    use lurker_core_contract::error::{ErrorCode, Failure};
    use lurker_core_contract::identity::EngineId;
    use lurker_core_contract::profile_document_identity::{
        ProfileDocumentIdentity, ProfileDocumentRevision, ProfileDocumentSourceIdentity,
        ProfileDocumentSourceKind, ProfileId, ProfileSidecarRevision, ProfileSourceId,
        ProfileWorkspaceIdentity,
    };
    use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
    use lurker_core_contract::routing_intent::RoutingIntent;

    #[test]
    fn routing_intent_command_requires_matching_typed_readback() {
        let request = CommandIntent::SetRoutingIntent {
            intent: RoutingIntent::RouteAllThroughProxy,
        };
        let matching = CommandOutput::RoutingIntentApplied {
            observed: RoutingIntent::RouteAllThroughProxy,
        };
        let mismatched = CommandOutput::RoutingIntentApplied {
            observed: RoutingIntent::BypassProxy,
        };

        assert!(matching.validate_for(&request).is_ok());
        assert_eq!(
            mismatched.validate_for(&request).unwrap_err().code,
            ErrorCode::InvalidState
        );
        assert_eq!(
            CommandOutput::Unit.validate_for(&request).unwrap_err().code,
            ErrorCode::InvalidState
        );
    }

    #[test]
    fn command_result_preserves_actual_filter_output_and_rejects_incomplete_receipts() {
        let request_id = RequestId::new(19);
        let applied = SubscriptionFilterApplied {
            workspace_identity: workspace_identity(
                "alpha",
                "applied-document",
                ProfileSidecarRevision::Present(
                    ProfileDocumentRevision::new("applied-options").expect("valid revision"),
                ),
            ),
            report: FilterReport {
                total_input: 3,
                passed: 1,
                ..Default::default()
            },
        };
        let output = CommandResult::Produced {
            request_id,
            output: Box::new(CommandOutput::SubscriptionFilterApplied(applied.clone())),
        };
        assert_eq!(
            output
                .clone()
                .into_output()
                .unwrap()
                .into_subscription_filter(),
            Ok(applied)
        );
        assert_eq!(
            output.into_unit().unwrap_err().code,
            ErrorCode::InvalidState
        );
        assert_eq!(
            CommandResult::Completed { request_id }
                .into_output()
                .unwrap()
                .into_subscription_filter()
                .unwrap_err()
                .code,
            ErrorCode::InvalidState
        );
        assert_eq!(
            CommandResult::Accepted { request_id }
                .into_output()
                .unwrap_err()
                .code,
            ErrorCode::NotReady
        );
        let failure = Failure::new(ErrorCode::Permission, "source denied", false);
        assert_eq!(
            CommandResult::Rejected {
                request_id,
                failure: failure.clone()
            }
            .into_output(),
            Err(failure)
        );
    }

    #[test]
    fn typed_profile_options_are_bound_to_the_requested_profile_and_command_kind() {
        let options = ProfileOptionsSnapshot::new(
            workspace_identity("alpha", "observed-document", ProfileSidecarRevision::Absent),
            "{}\n",
            Default::default(),
        );
        let output = CommandOutput::ProfileOptionsLoaded(options.clone());
        let matching = CommandIntent::LoadProfileOptions {
            profile: Some("alpha".into()),
        };
        assert!(output.validate_for(&matching).is_ok());
        assert_eq!(output.clone().into_profile_options(), Ok(options));
        for wrong in [
            CommandIntent::LoadProfileOptions {
                profile: Some("beta".into()),
            },
            CommandIntent::CheckUpdates,
        ] {
            assert_eq!(
                output.validate_for(&wrong).unwrap_err().code,
                ErrorCode::InvalidState
            );
        }
        assert_eq!(
            CommandOutput::Unit
                .validate_for(&matching)
                .unwrap_err()
                .code,
            ErrorCode::InvalidState
        );
        assert_eq!(
            output.clone().into_subscription_filter().unwrap_err().code,
            ErrorCode::InvalidState
        );
        assert_eq!(
            output.into_unit().unwrap_err().code,
            ErrorCode::InvalidState
        );
    }

    fn workspace_identity(
        profile: &str,
        revision: &str,
        sidecar_revision: ProfileSidecarRevision,
    ) -> ProfileWorkspaceIdentity {
        let document = ProfileDocumentIdentity::new(
            ProfileId::new(profile).expect("valid profile"),
            EngineId::new("mihomo").expect("valid engine"),
            ProfileDocumentSourceIdentity::new(
                ProfileDocumentSourceKind::Local,
                ProfileSourceId::new(format!("local-{profile}")).expect("valid source"),
            ),
            ProfileDocumentRevision::new(revision).expect("valid revision"),
            ProfileFormatId::new("mihomo-yaml").expect("valid format"),
            ProfileSchemaVersion::new(1).expect("valid schema"),
        );
        ProfileWorkspaceIdentity::new(document, sidecar_revision)
    }
}

#[cfg(test)]
mod snapshot_receipt_tests {
    use super::*;
    use crate::snapshot_history::{SnapshotPruneReport, SnapshotPruneSource};

    #[test]
    fn snapshot_outputs_reject_unit_wrong_profile_wrong_retention_and_wrong_storage_identity() {
        let history_request = CommandIntent::LoadSnapshotHistory {
            profile: Some("main".into()),
            keep: 5,
        };
        let mut history = SnapshotHistorySnapshot::empty("other");
        history.keep_limit = 5;
        assert!(CommandOutput::Unit.validate_for(&history_request).is_err());
        assert!(
            CommandOutput::SnapshotHistoryLoaded(history.clone())
                .validate_for(&history_request)
                .is_err()
        );
        history.profile = "main".into();
        CommandOutput::SnapshotHistoryLoaded(history.clone())
            .validate_for(&history_request)
            .unwrap();
        history.keep_limit = 20;
        assert!(
            CommandOutput::SnapshotHistoryLoaded(history)
                .validate_for(&history_request)
                .is_err()
        );
        let diff_request = CommandIntent::LoadSnapshotDiff {
            profile: Some("main".into()),
            snapshot_id: Some("/snapshot/one".into()),
        };
        assert!(
            CommandOutput::SnapshotDiffLoaded(None)
                .validate_for(&diff_request)
                .is_err()
        );
        let mut diff = YamlAstDiffSnapshot {
            target_id: "main".into(),
            source_path: Some("/snapshot/two".into()),
            ..Default::default()
        };
        assert!(
            CommandOutput::SnapshotDiffLoaded(Some(Box::new(diff.clone())))
                .validate_for(&diff_request)
                .is_err()
        );
        diff.source_path = Some("/snapshot/one".into());
        CommandOutput::SnapshotDiffLoaded(Some(Box::new(diff)))
            .validate_for(&diff_request)
            .unwrap();
        let prune_request = CommandIntent::PruneSnapshots {
            profile: Some("main".into()),
            keep: Some(5),
        };
        let receipt = SnapshotPrunedReceipt {
            profile: "other".into(),
            report: SnapshotPruneReport {
                removed: 2,
                keep_limit: 5,
                source: SnapshotPruneSource::Manual,
            },
        };
        assert!(
            CommandOutput::SnapshotsPruned(receipt)
                .validate_for(&prune_request)
                .is_err()
        );
        let create_request = CommandIntent::CreateBackupSnapshot {
            profile: Some("main".into()),
        };
        let receipt = SnapshotCreatedReceipt {
            profile: "main".into(),
            id: "/snapshot/one".into(),
            sha256: "not-a-content-hash".into(),
        };
        assert!(
            CommandOutput::SnapshotCreated(receipt)
                .validate_for(&create_request)
                .is_err()
        );
    }
}

//! Typed owner commands produced by the Slint workbench pages.
//!
//! Every workbench gesture resolves in this module, so the running surface and
//! a driven surface test observe exactly the same shared command and the same
//! typed refusal. No workbench rule is implemented twice.

use crate::surface::workbenches::{PendingBusiness, Workbenches};
use crate::{CommandIntent, SlintAction};
use infiltrator_contract::aggregator::AggregationDraft;
use infiltrator_contract::app_routing::{AppRoutingMode, AppRoutingRule};
use infiltrator_contract::dns_hosts::parse_hosts_editor;
use infiltrator_contract::doctor::DoctorAction;
use infiltrator_contract::script_export::ScriptExportKind;
use infiltrator_contract::script_export_review::{ScriptExportDraft, ScriptExportValidationScope};
use infiltrator_contract::snapshot_restore::SnapshotRestoreTarget;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::page::PageStatus;
use std::sync::{Mutex, MutexGuard};

/// The DNS cross-source leak probe of the DNS page.
pub const DNS_LEAK_PROBE: &str = "dns-leak-probe";
/// Submit the DNS query draft.
pub const DNS_QUERY_RUN: &str = "dns-query-run";
/// Re-submit the DNS query draft after a failure.
pub const DNS_QUERY_RETRY: &str = "dns-query-retry";
/// Close the DNS query panel without asking the owner.
pub const DNS_QUERY_CANCEL: &str = "dns-query-cancel";
/// Confirm the Fake-IP cache flush.
pub const DNS_CACHE_CONFIRM: &str = "dns-cache-confirm";
/// Re-run the Fake-IP cache flush after a failure.
pub const DNS_CACHE_RETRY: &str = "dns-cache-retry";
/// Close the Fake-IP cache confirmation without asking the owner.
pub const DNS_CACHE_CANCEL: &str = "dns-cache-cancel";
/// Add the drafted Hosts row to the local draft.
pub const DNS_HOSTS_ADD: &str = "dns-hosts-add";
/// Remove one Hosts row from the local draft.
pub const DNS_HOSTS_REMOVE: &str = "dns-hosts-remove";
/// Import the preserved legacy Hosts rows into the local draft.
pub const DNS_HOSTS_IMPORT: &str = "dns-hosts-import";
/// Commit the Hosts draft through the shared guarded write.
pub const DNS_HOSTS_SAVE: &str = "dns-hosts-save";
/// Discard the Hosts draft without asking the owner.
pub const DNS_HOSTS_CANCEL: &str = "dns-hosts-cancel";
/// Run the drafted script in the shared sandbox.
pub const SCRIPT_RUN: &str = "script-run";
/// Clear the shared sandbox observation.
pub const SCRIPT_CLEAR: &str = "script-clear";
/// Re-run the last refused sandbox operation.
pub const SCRIPT_RETRY: &str = "script-retry";
/// Load one built-in script preset into the sandbox draft.
pub const SCRIPT_PRESET: &str = "script-preset";
/// Compose the export artifact the owner has to review.
pub const SCRIPT_EXPORT_PREPARE: &str = "script-export-prepare";
/// Persist the reviewed export artifact.
pub const SCRIPT_EXPORT_SAVE: &str = "script-export-save";
/// Discard the reviewed export artifact.
pub const SCRIPT_EXPORT_CANCEL: &str = "script-export-cancel";
/// Load the AST difference between the current document and a stored snapshot.
pub const DIFF_LOAD: &str = "diff-load";
/// Close the difference view without asking the owner.
pub const DIFF_CLOSE: &str = "diff-close";
/// Ask the owner to compose a snapshot restoration review.
pub const RESTORE_PREPARE: &str = "restore-prepare";
/// Apply the reviewed snapshot restoration.
pub const RESTORE_CONFIRM: &str = "restore-confirm";
/// Discard the reviewed snapshot restoration.
pub const RESTORE_CANCEL: &str = "restore-cancel";
/// Commit the subscription filter draft through the shared pipeline.
pub const FILTER_SAVE: &str = "filter-save";
/// Discard the subscription filter draft without asking the owner.
pub const FILTER_CANCEL: &str = "filter-cancel";
/// Recompute the aggregation preview from the real source contents.
pub const AGGREGATE_PREVIEW: &str = "aggregate-preview";
/// Materialise the aggregation draft into a new profile.
pub const AGGREGATE_CREATE: &str = "aggregate-create";
/// Discard the aggregation draft without asking the owner.
pub const AGGREGATE_CANCEL: &str = "aggregate-cancel";
/// Run the shared diagnostic report.
pub const DOCTOR_DIAGNOSE: &str = "doctor-diagnose";
/// Repair one diagnosed issue.
pub const DOCTOR_REPAIR: &str = "doctor-repair";
/// Repair every diagnosed issue.
pub const DOCTOR_REPAIR_ALL: &str = "doctor-repair-all";
/// Bootstrap the shared diagnostic report.
pub const DOCTOR_BOOTSTRAP: &str = "doctor-bootstrap";
/// Re-run the refused diagnostic operation.
pub const DOCTOR_RETRY: &str = "doctor-retry";
/// Synchronise with the configured remote.
pub const SYNC_NOW: &str = "sync-now";
/// Keep the local copy of a synchronisation conflict.
pub const SYNC_KEEP_LOCAL: &str = "sync-keep-local";
/// Take the remote copy of a synchronisation conflict.
pub const SYNC_TAKE_REMOTE: &str = "sync-take-remote";
/// Defer the resolution of a synchronisation conflict.
pub const SYNC_CONFLICT_CANCEL: &str = "sync-conflict-cancel";
/// Create a local backup snapshot of the active profile.
pub const SYNC_BACKUP: &str = "sync-backup";
/// Ask the owner to compose the restoration review of one synced snapshot.
pub const SYNC_RESTORE: &str = "sync-restore";
/// Change the application routing mode to proxy every application.
pub const APP_ROUTING_MODE_ALL: &str = "app-routing-mode-all";
/// Change the application routing mode to proxy the selected applications.
pub const APP_ROUTING_MODE_PROXY: &str = "app-routing-mode-proxy";
/// Change the application routing mode to bypass the selected applications.
pub const APP_ROUTING_MODE_BYPASS: &str = "app-routing-mode-bypass";
/// Change the rule of one routed application.
pub const APP_ROUTING_RULE: &str = "app-routing-rule";
/// Discard the routing rule draft without asking the owner.
pub const APP_ROUTING_CANCEL: &str = "app-routing-cancel";
/// Ask the owner for a fresh routing policy.
pub const APP_ROUTING_REFRESH: &str = "app-routing-refresh";

/// One workbench gesture.
///
/// The panel action names the business, the two draft fields carry the
/// values the panel edits, and the row identifies the row the action
/// addresses.
pub struct WorkbenchGesture<'a> {
    /// The panel action identity.
    pub action: &'a str,
    /// The value of the panel's primary draft field.
    pub draft: &'a str,
    /// The value of the panel's secondary draft field.
    pub draft2: &'a str,
    /// The identity of the addressed row, empty when the action is not a row
    /// action.
    pub row: &'a str,
}

/// What the surface did with one workbench gesture.
#[derive(Debug)]
pub enum WorkbenchOutcome {
    /// The gesture asked the shared owner for a command.
    Command(SlintAction),
    /// The gesture asked the owner for a fresh snapshot.
    Refresh,
    /// The gesture changed only the surface's own presentation.
    Local,
}

/// Resolves one workbench gesture.
///
/// The outcome is the shared owner command, a request for a fresh owner
/// snapshot, or a local surface transition; a typed failure is the refusal
/// the surface shows.
pub fn dispatch(
    workbenches: &Workbenches,
    snapshot: Option<&SurfaceSnapshot>,
    gesture: &WorkbenchGesture<'_>,
) -> Result<WorkbenchOutcome, Failure> {
    // The refresh gesture asks the owner for its next read instead of for a
    // command, so it never reaches the command resolution below.
    if gesture.action == APP_ROUTING_REFRESH {
        return Ok(WorkbenchOutcome::Refresh);
    }
    match resolve(workbenches, snapshot, gesture)? {
        Some(action) => Ok(WorkbenchOutcome::Command(action)),
        None => Ok(WorkbenchOutcome::Local),
    }
}

/// The shared reducer that owns an owner command.
///
/// The mapping is the surface's routing table between the shared command
/// vocabulary and the reducer that correlates it; the reducer keeps the
/// command identity and the phase, so the surface keeps no second copy.
pub fn owner_business(intent: &CommandIntent) -> Option<PendingBusiness> {
    match intent {
        CommandIntent::QueryDns { .. } => Some(PendingBusiness::DnsQuery),
        CommandIntent::ClearDnsCache { .. } => Some(PendingBusiness::DnsCache),
        CommandIntent::ApplyDnsSettings { .. } => Some(PendingBusiness::DnsHosts),
        CommandIntent::RunDoctorDiagnostics
        | CommandIntent::BootstrapDoctor
        | CommandIntent::RepairDoctorIssue { .. }
        | CommandIntent::RepairAllDoctorIssues => Some(PendingBusiness::Doctor),
        CommandIntent::RunScriptSandbox { .. }
        | CommandIntent::ClearScriptSandbox { .. }
        | CommandIntent::PrepareScriptExport { .. }
        | CommandIntent::SaveScriptExport { .. }
        | CommandIntent::CancelScriptExport { .. } => Some(PendingBusiness::Script),
        CommandIntent::PrepareSnapshotRestore { .. }
        | CommandIntent::ConfirmSnapshotRestore { .. }
        | CommandIntent::CancelSnapshotRestore { .. } => Some(PendingBusiness::Restore),
        CommandIntent::SaveSubscriptionFilter { .. } => Some(PendingBusiness::Filter),
        _ => None,
    }
}

/// Resolves one workbench gesture into an owner command.
///
/// `Ok(Some(action))` dispatches a shared owner command and `Ok(None)` is a
/// purely local surface transition; `Err(failure)` is the typed refusal the
/// surface shows.
fn resolve(
    workbenches: &Workbenches,
    snapshot: Option<&SurfaceSnapshot>,
    gesture: &WorkbenchGesture<'_>,
) -> Result<Option<SlintAction>, Failure> {
    match gesture.action {
        DNS_LEAK_PROBE => Ok(Some(SlintAction::Command(CommandIntent::TestDnsLeak))),
        DNS_QUERY_RUN | DNS_QUERY_RETRY => {
            let mut query = lock(&workbenches.dns_query)?;
            if gesture.action == DNS_QUERY_RETRY && !query.can_retry() {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "The DNS query has no retryable failure",
                    false,
                ));
            }
            query.show();
            query.set_name(gesture.draft.to_owned());
            let (operation, request) = query.begin()?;
            Ok(Some(SlintAction::Command(CommandIntent::QueryDns {
                operation,
                request,
            })))
        }
        DNS_QUERY_CANCEL => {
            let mut query = lock(&workbenches.dns_query)?;
            if !query.cancel() {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "The DNS query panel cannot be closed right now",
                    false,
                ));
            }
            Ok(None)
        }
        DNS_CACHE_CONFIRM => {
            let mut cache = lock(&workbenches.dns_cache)?;
            cache.show();
            let operation = cache.confirm()?;
            Ok(Some(SlintAction::Command(CommandIntent::ClearDnsCache {
                operation,
            })))
        }
        DNS_CACHE_RETRY => {
            let mut cache = lock(&workbenches.dns_cache)?;
            let operation = cache.retry()?;
            Ok(Some(SlintAction::Command(CommandIntent::ClearDnsCache {
                operation,
            })))
        }
        DNS_CACHE_CANCEL => {
            let mut cache = lock(&workbenches.dns_cache)?;
            if !cache.cancel() {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "The DNS cache confirmation cannot be closed right now",
                    false,
                ));
            }
            Ok(None)
        }
        DNS_HOSTS_ADD => {
            let row = {
                let drafts = lock(&workbenches.drafts)?;
                drafts.hosts_row().to_owned()
            };
            let entries = parse_hosts_editor(&row).map_err(|_| {
                Failure::new(
                    ErrorCode::InvalidInput,
                    "The Hosts row is not valid yet",
                    false,
                )
            })?;
            let entry = entries.first().ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidInput,
                    "The Hosts row is not valid yet",
                    false,
                )
            })?;
            let mut hosts = lock(&workbenches.dns_hosts)?;
            hosts.edit_address(entry.address.clone());
            hosts.edit_domain(entry.domain.clone());
            if !hosts.commit_row() {
                return Err(Failure::new(
                    ErrorCode::InvalidInput,
                    "The Hosts row is not valid yet",
                    false,
                ));
            }
            let mut drafts = lock(&workbenches.drafts)?;
            drafts.clear_hosts_row();
            Ok(None)
        }
        DNS_HOSTS_REMOVE => {
            let mut hosts = lock(&workbenches.dns_hosts)?;
            let id = gesture.row.parse::<u64>().map_err(|_| {
                Failure::new(ErrorCode::InvalidInput, "Invalid Hosts row identity", false)
            })?;
            hosts.remove(id);
            Ok(None)
        }
        DNS_HOSTS_IMPORT => {
            let mut hosts = lock(&workbenches.dns_hosts)?;
            hosts.import_legacy();
            Ok(None)
        }
        DNS_HOSTS_SAVE => {
            let mut hosts = lock(&workbenches.dns_hosts)?;
            let pending = hosts.begin()?;
            Ok(Some(SlintAction::Command(
                CommandIntent::ApplyDnsSettings {
                    patch: pending.patch,
                },
            )))
        }
        DNS_HOSTS_CANCEL => {
            let mut hosts = lock(&workbenches.dns_hosts)?;
            if !hosts.cancel() {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "The Hosts draft cannot be discarded right now",
                    false,
                ));
            }
            Ok(None)
        }
        SCRIPT_RUN => {
            let mut script = lock(&workbenches.script)?;
            script.edit_script(gesture.draft.to_owned());
            let pending = script.run().ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidState,
                    "The script sandbox is busy or the export review is open",
                    true,
                )
            })?;
            Ok(Some(SlintAction::Command(pending.intent)))
        }
        SCRIPT_CLEAR => {
            let mut script = lock(&workbenches.script)?;
            let pending = script.clear().ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidState,
                    "The script sandbox is busy or the export review is open",
                    true,
                )
            })?;
            Ok(Some(SlintAction::Command(pending.intent)))
        }
        SCRIPT_RETRY => {
            let mut script = lock(&workbenches.script)?;
            let pending = script.retry().ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidState,
                    "The script sandbox has no retryable failure",
                    false,
                )
            })?;
            Ok(Some(SlintAction::Command(pending.intent)))
        }
        SCRIPT_PRESET => {
            let mut script = lock(&workbenches.script)?;
            script.select_preset(gesture.row);
            if script.selected_preset() != Some(gesture.row) {
                return Err(Failure::new(
                    ErrorCode::InvalidInput,
                    "Unknown script preset",
                    false,
                ));
            }
            Ok(None)
        }
        SCRIPT_EXPORT_PREPARE => {
            let mut script = lock(&workbenches.script)?;
            let draft = ScriptExportDraft {
                kind: ScriptExportKind::DirectiveDslScript,
                validation_scope: ScriptExportValidationScope::Unscoped,
                profile: None,
                base_yaml: String::new(),
                mixin_yaml: String::new(),
                script_code: script.script_code().to_owned(),
                preset: script.selected_preset().map(str::to_owned),
            };
            let pending = script.prepare_export(draft).ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidState,
                    "The script export review is already open",
                    true,
                )
            })?;
            Ok(Some(SlintAction::Command(pending.intent)))
        }
        SCRIPT_EXPORT_SAVE => {
            let mut script = lock(&workbenches.script)?;
            let pending = script.confirm_export().ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidState,
                    "The script export has no review to persist",
                    false,
                )
            })?;
            Ok(Some(SlintAction::Command(pending.intent)))
        }
        SCRIPT_EXPORT_CANCEL => {
            let mut script = lock(&workbenches.script)?;
            let pending = script.cancel_export().ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidState,
                    "The script export has no review to discard",
                    false,
                )
            })?;
            Ok(Some(SlintAction::Command(pending.intent)))
        }
        DIFF_LOAD => {
            workbenches.open_difference_view();
            Ok(Some(SlintAction::Command(
                CommandIntent::LoadSnapshotDiff {
                    profile: None,
                    snapshot_id: None,
                },
            )))
        }
        DIFF_CLOSE => {
            let source_id = snapshot
                .and_then(|snapshot| snapshot.yaml_ast_diff.as_ref())
                .map(|diff| diff.source_id.clone())
                .unwrap_or_default();
            workbenches.close_difference_view_of(&source_id);
            Ok(None)
        }
        RESTORE_PREPARE => {
            let target = restore_target(snapshot)?;
            let mut restore = lock(&workbenches.restore)?;
            let pending = restore.open(target).ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidState,
                    "The snapshot restoration review is already open",
                    false,
                )
            })?;
            Ok(Some(SlintAction::Command(pending.intent)))
        }
        RESTORE_CONFIRM => {
            let mut restore = lock(&workbenches.restore)?;
            let pending = restore.confirm().ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidState,
                    "The snapshot restoration review has no review to confirm",
                    false,
                )
            })?;
            Ok(Some(SlintAction::Command(pending.intent)))
        }
        RESTORE_CANCEL => {
            let mut restore = lock(&workbenches.restore)?;
            let visible = restore.is_visible();
            match restore.cancel() {
                Some(pending) => Ok(Some(SlintAction::Command(pending.intent))),
                None if visible && !restore.is_visible() => Ok(None),
                None => Err(Failure::new(
                    ErrorCode::InvalidState,
                    "The snapshot restoration review cannot be dismissed right now",
                    false,
                )),
            }
        }
        FILTER_SAVE => {
            let mut filter = lock(&workbenches.filter)?;
            let pending = filter.begin()?;
            Ok(Some(SlintAction::Command(
                CommandIntent::SaveSubscriptionFilter {
                    workspace_identity: pending.source.workspace_identity,
                    filter: pending.draft,
                },
            )))
        }
        FILTER_CANCEL => {
            let mut filter = lock(&workbenches.filter)?;
            if !filter.cancel() {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "The filter draft cannot be discarded right now",
                    false,
                ));
            }
            Ok(None)
        }
        AGGREGATE_PREVIEW | AGGREGATE_CREATE => {
            let target_name = {
                let drafts = lock(&workbenches.drafts)?;
                drafts.aggregation_target().to_owned()
            };
            let draft = aggregation_draft(snapshot, &target_name)?;
            let intent = if gesture.action == AGGREGATE_PREVIEW {
                CommandIntent::PreviewProfileAggregation { draft }
            } else {
                CommandIntent::CreateAggregatedProfile { draft }
            };
            Ok(Some(SlintAction::Command(intent)))
        }
        AGGREGATE_CANCEL => {
            let mut drafts = lock(&workbenches.drafts)?;
            drafts.clear_aggregation_target();
            Ok(None)
        }
        DOCTOR_DIAGNOSE => {
            let mut doctor = lock(&workbenches.doctor)?;
            let pending = doctor.begin(DoctorAction::Diagnose)?;
            Ok(Some(SlintAction::Command(pending.action.intent())))
        }
        DOCTOR_REPAIR => {
            let mut doctor = lock(&workbenches.doctor)?;
            let pending = doctor.begin(DoctorAction::RepairOne(gesture.row.to_owned()))?;
            Ok(Some(SlintAction::Command(pending.action.intent())))
        }
        DOCTOR_REPAIR_ALL => {
            let mut doctor = lock(&workbenches.doctor)?;
            let pending = doctor.begin(DoctorAction::RepairAll)?;
            Ok(Some(SlintAction::Command(pending.action.intent())))
        }
        DOCTOR_BOOTSTRAP => {
            let mut doctor = lock(&workbenches.doctor)?;
            let pending = doctor.begin(DoctorAction::Bootstrap)?;
            Ok(Some(SlintAction::Command(pending.action.intent())))
        }
        DOCTOR_RETRY => {
            let mut doctor = lock(&workbenches.doctor)?;
            let pending = doctor.begin_retry()?;
            Ok(Some(SlintAction::Command(pending.action.intent())))
        }
        SYNC_NOW => Ok(Some(SlintAction::Command(CommandIntent::SyncNow))),
        SYNC_KEEP_LOCAL => Ok(Some(SlintAction::Command(
            CommandIntent::ResolveConflictKeepLocal,
        ))),
        SYNC_TAKE_REMOTE => Ok(Some(SlintAction::Command(
            CommandIntent::ResolveConflictTakeRemote,
        ))),
        SYNC_CONFLICT_CANCEL => {
            let conflict_time = sync_conflict_time(snapshot);
            workbenches.close_conflict_resolver(&conflict_time);
            Ok(None)
        }
        SYNC_BACKUP => Ok(Some(SlintAction::Command(
            CommandIntent::CreateBackupSnapshot { profile: None },
        ))),
        SYNC_RESTORE => {
            let target = sync_restore_target(snapshot, gesture.row)?;
            let mut restore = lock(&workbenches.restore)?;
            let pending = restore.open(target).ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidState,
                    "The snapshot restoration review is already open",
                    false,
                )
            })?;
            Ok(Some(SlintAction::Command(pending.intent)))
        }
        APP_ROUTING_MODE_ALL => {
            require_routing_policy(snapshot)?;
            Ok(Some(SlintAction::Command(
                CommandIntent::SetAppRoutingMode {
                    mode: AppRoutingMode::ProxyAll,
                },
            )))
        }
        APP_ROUTING_MODE_PROXY => {
            require_routing_policy(snapshot)?;
            Ok(Some(SlintAction::Command(
                CommandIntent::SetAppRoutingMode {
                    mode: AppRoutingMode::ProxySelected,
                },
            )))
        }
        APP_ROUTING_MODE_BYPASS => {
            require_routing_policy(snapshot)?;
            Ok(Some(SlintAction::Command(
                CommandIntent::SetAppRoutingMode {
                    mode: AppRoutingMode::BypassSelected,
                },
            )))
        }
        APP_ROUTING_RULE => {
            require_routing_policy(snapshot)?;
            let rule = {
                let drafts = lock(&workbenches.drafts)?;
                app_routing_rule(drafts.routing_rule())?
            };
            Ok(Some(SlintAction::Command(CommandIntent::SetAppRule {
                app_id: gesture.row.to_owned(),
                rule,
            })))
        }
        APP_ROUTING_CANCEL => {
            let mut drafts = lock(&workbenches.drafts)?;
            drafts.clear_routing_rule();
            Ok(None)
        }
        _ => Err(Failure::new(
            ErrorCode::InvalidInput,
            "Unknown workbench action",
            false,
        )),
    }
}

/// The typed failure a locked reducer reports when the surface state is
/// poisoned.
fn lock<T>(mutex: &Mutex<T>) -> Result<MutexGuard<'_, T>, Failure> {
    mutex.lock().map_err(|_| {
        Failure::new(
            ErrorCode::Internal,
            "The workbench state is poisoned",
            false,
        )
    })
}

/// The failure the surface raises before the owner published a snapshot.
fn not_ready() -> Failure {
    Failure::new(
        ErrorCode::NotReady,
        "The shared owner has not published its first snapshot",
        true,
    )
}

/// The newest stored snapshot of the active profile.
fn restore_target(snapshot: Option<&SurfaceSnapshot>) -> Result<SnapshotRestoreTarget, Failure> {
    let snapshot = snapshot.ok_or_else(not_ready)?;
    let history = snapshot
        .pages
        .profiles
        .data
        .as_ref()
        .and_then(|page| page.snapshot_history.as_ref())
        .ok_or_else(|| {
            Failure::new(
                ErrorCode::NotReady,
                "The profile snapshot history has not been observed",
                true,
            )
        })?;
    let entry = history
        .entries
        .iter()
        .find(|entry| entry.is_newest)
        .or_else(|| history.entries.first())
        .ok_or_else(|| {
            Failure::new(
                ErrorCode::NotReady,
                "The profile has no stored snapshot to restore",
                true,
            )
        })?;
    Ok(SnapshotRestoreTarget {
        profile: history.profile.clone(),
        snapshot_id: entry.id.clone(),
    })
}

/// The restoration target of one synchronised snapshot.
fn sync_restore_target(
    snapshot: Option<&SurfaceSnapshot>,
    snapshot_id: &str,
) -> Result<SnapshotRestoreTarget, Failure> {
    let snapshot = snapshot.ok_or_else(not_ready)?;
    let page = snapshot.pages.sync.data.as_ref().ok_or_else(not_ready)?;
    let entry = page
        .snapshots
        .iter()
        .find(|entry| entry.id == snapshot_id)
        .ok_or_else(|| {
            Failure::new(
                ErrorCode::NotReady,
                "The synchronised snapshot is not published",
                true,
            )
        })?;
    Ok(SnapshotRestoreTarget {
        profile: entry.profile.clone(),
        snapshot_id: entry.id.clone(),
    })
}

/// The identity of the published synchronisation conflict.
fn sync_conflict_time(snapshot: Option<&SurfaceSnapshot>) -> String {
    snapshot
        .and_then(|snapshot| snapshot.pages.sync.data.as_ref())
        .and_then(|page| page.conflict.as_ref())
        .map(|conflict| conflict.conflict_time.clone())
        .unwrap_or_default()
}

/// The aggregation draft the wizard panel composes.
///
/// The wizard merges every profile of the catalog; the user names the profile
/// the merge produces.
fn aggregation_draft(
    snapshot: Option<&SurfaceSnapshot>,
    target_name: &str,
) -> Result<AggregationDraft, Failure> {
    let snapshot = snapshot.ok_or_else(not_ready)?;
    let page = snapshot
        .pages
        .profiles
        .data
        .as_ref()
        .ok_or_else(not_ready)?;
    let sources: Vec<String> = page
        .profiles
        .iter()
        .map(|profile| profile.id.clone())
        .collect::<Vec<_>>();
    if sources.is_empty() {
        return Err(Failure::new(
            ErrorCode::InvalidInput,
            "The aggregation wizard needs at least one source profile",
            false,
        ));
    }
    let target_name = target_name.trim();
    if target_name.is_empty() {
        return Err(Failure::new(
            ErrorCode::InvalidInput,
            "The aggregation wizard needs a target profile name",
            false,
        ));
    }
    Ok(AggregationDraft {
        source_profiles: sources,
        target_name: target_name.to_owned(),
        deduplicate: true,
        deduplicate_names: true,
        geo_cluster: true,
        generate_groups: true,
        remove_emojis: false,
        rename_rules: Vec::new(),
        custom_groups: Vec::new(),
        availability_precheck: true,
        activate_after_create: false,
    })
}

/// Refuses a routing change the owner never published.
///
/// A host without per-application routing publishes the typed reason, and the
/// surface never writes a policy it has not observed.
fn require_routing_policy(snapshot: Option<&SurfaceSnapshot>) -> Result<(), Failure> {
    let snapshot = snapshot.ok_or_else(not_ready)?;
    match &snapshot.pages.app_routing.status {
        PageStatus::Ready | PageStatus::Empty => Ok(()),
        PageStatus::Loading => Err(Failure::new(
            ErrorCode::NotReady,
            "The application routing policy has not been observed yet",
            true,
        )),
        PageStatus::Unavailable { failure } | PageStatus::Failed { failure } => {
            Err(failure.clone())
        }
    }
}

/// Decodes the shared wire value of the application routing rule.
fn app_routing_rule(value: &str) -> Result<AppRoutingRule, Failure> {
    match value.trim() {
        "proxy" => Ok(AppRoutingRule::Proxy),
        "direct" => Ok(AppRoutingRule::Direct),
        "block" => Ok(AppRoutingRule::Block),
        _ => Err(Failure::new(
            ErrorCode::InvalidInput,
            "Unknown application routing rule",
            false,
        )),
    }
}

//! Debug rendering for wave-2 UI panels: DNS leak, custom node, aggregator, snapshot diff and hotkeys.
//!
//! Split out of the single `Debug` impl along the message-family seam; the
//! parent `debug` module dispatches to these sections in turn.

use crate::types::message::Message;
use std::fmt;

pub(super) fn fmt(m: &Message, f: &mut fmt::Formatter<'_>) -> Option<fmt::Result> {
    Some(match m {
        // ui-wave2-p
        Message::ToggleProxyGroupExpanded(name) => {
            write!(f, "ToggleProxyGroupExpanded({})", name)
        }
        Message::SetEditorPane(pane) => write!(f, "SetEditorPane({:?})", pane),
        Message::EditProfileAs(path, pane) => {
            write!(f, "EditProfileAs({},{:?})", path.display(), pane)
        }
        Message::MixinEditorAction(_) => write!(f, "MixinEditorAction"),
        Message::MixinLoaded(Ok(_)) => write!(f, "MixinLoaded(Ok)"),
        Message::MixinLoaded(Err(e)) => write!(f, "MixinLoaded(Err({:?}))", e),
        Message::SaveMixin => write!(f, "SaveMixin"),
        Message::MixinSaved(Ok(_)) => write!(f, "MixinSaved(Ok)"),
        Message::MixinSaved(Err(e)) => write!(f, "MixinSaved(Err({:?}))", e),
        Message::ToggleMixinPreset(id, enabled) => {
            write!(f, "ToggleMixinPreset({id}, {enabled})")
        }
        Message::LoadProfileFilter => write!(f, "LoadProfileFilter"),
        Message::ProfileFilterLoaded(Ok(_)) => write!(f, "ProfileFilterLoaded(Ok)"),
        Message::ProfileFilterLoaded(Err(e)) => write!(f, "ProfileFilterLoaded(Err({:?}))", e),
        Message::UpdateFilterInclude(v) => write!(f, "UpdateFilterInclude({})", v),
        Message::UpdateFilterExclude(v) => write!(f, "UpdateFilterExclude({})", v),
        Message::UpdateFilterExcludeTypes(v) => write!(f, "UpdateFilterExcludeTypes({})", v),
        Message::UpdateFilterRenames(v) => write!(f, "UpdateFilterRenames({})", v),
        Message::UpdateFilterDedup(i) => write!(f, "UpdateFilterDedup({})", i),
        Message::SaveProfileFilter => write!(f, "SaveProfileFilter"),
        Message::ProfileFilterSaved(Ok(report)) => {
            write!(f, "ProfileFilterSaved(Ok(passed={}))", report.passed)
        }
        Message::ProfileFilterSaved(Err(e)) => write!(f, "ProfileFilterSaved(Err({:?}))", e),
        Message::ScanMrsProviders => write!(f, "ScanMrsProviders"),
        Message::MrsDetailsReady(Ok(details)) => {
            write!(f, "MrsDetailsReady(Ok({} providers))", details.len())
        }
        Message::MrsDetailsReady(Err(e)) => write!(f, "MrsDetailsReady(Err({:?}))", e),
        Message::LoadSyncDiff(profile) => write!(f, "LoadSyncDiff({})", profile),
        Message::SyncDiffLoaded(Ok(bundle)) => {
            write!(f, "SyncDiffLoaded(Ok({}))", bundle.profile)
        }
        Message::SyncDiffLoaded(Err(e)) => write!(f, "SyncDiffLoaded(Err({:?}))", e),
        Message::PickSyncDiffKey(key, take_remote) => {
            write!(f, "PickSyncDiffKey({}, {})", key, take_remote)
        }
        Message::SetSyncDiffPicks(take_remote) => {
            write!(f, "SetSyncDiffPicks({})", take_remote)
        }
        Message::ApplySyncDiffMerge => write!(f, "ApplySyncDiffMerge"),
        Message::SyncDiffMerged(Ok(profile)) => write!(f, "SyncDiffMerged(Ok({}))", profile),
        Message::SyncDiffMerged(Err(e)) => write!(f, "SyncDiffMerged(Err({:?}))", e),
        Message::CloseSyncDiff => write!(f, "CloseSyncDiff"),
        // Doctor 体检面板
        Message::RunDoctor => write!(f, "RunDoctor"),
        Message::DoctorReportReady(Ok(report)) => {
            write!(f, "DoctorReportReady(Ok({} checks))", report.checks.len())
        }
        Message::DoctorReportReady(Err(e)) => write!(f, "DoctorReportReady(Err({:?}))", e),
        Message::RunDoctorFix => write!(f, "RunDoctorFix"),
        Message::DoctorFixApplied(Ok(report)) => {
            write!(f, "DoctorFixApplied(Ok({} actions))", report.actions.len())
        }
        Message::DoctorFixApplied(Err(e)) => write!(f, "DoctorFixApplied(Err({:?}))", e),
        Message::RunBootstrap => write!(f, "RunBootstrap"),
        Message::BootstrapFinished(Ok(report)) => {
            write!(f, "BootstrapFinished(Ok({} steps))", report.steps.len())
        }
        Message::BootstrapFinished(Err(e)) => write!(f, "BootstrapFinished(Err({:?}))", e),
        // Wave 1: Command Palette, Connection Drawer, Editor formatting
        Message::ToggleCommandPalette => write!(f, "ToggleCommandPalette"),
        Message::OpenCommandPalette => write!(f, "OpenCommandPalette"),
        Message::CloseCommandPalette => write!(f, "CloseCommandPalette"),
        Message::SetCommandQuery(q) => write!(f, "SetCommandQuery({q})"),
        Message::SelectNextCommand => write!(f, "SelectNextCommand"),
        Message::SelectPrevCommand => write!(f, "SelectPrevCommand"),
        Message::ExecuteCommand(action) => write!(f, "ExecuteCommand({action:?})"),
        Message::InspectConnection(id) => write!(f, "InspectConnection({id:?})"),
        Message::CloseSingleConnection(id) => write!(f, "CloseSingleConnection({id})"),
        Message::InsertYamlSnippet(snip) => write!(f, "InsertYamlSnippet({snip})"),
        Message::FormatYamlEditor => write!(f, "FormatYamlEditor"),
        Message::BackupProfileSnapshot => write!(f, "BackupProfileSnapshot"),
        Message::ProfileSnapshotBackedUp(Ok(())) => {
            write!(f, "ProfileSnapshotBackedUp(Ok)")
        }
        Message::ProfileSnapshotBackedUp(Err(error)) => {
            write!(f, "ProfileSnapshotBackedUp(Err({error:?}))")
        }
        Message::SetSnapshotPruneKeep(keep) => write!(f, "SetSnapshotPruneKeep({keep})"),
        Message::PruneProfileSnapshots => write!(f, "PruneProfileSnapshots"),
        Message::ProfileSnapshotsPruned(Ok(report)) => write!(
            f,
            "ProfileSnapshotsPruned(Ok(removed={}, keep={}))",
            report.removed, report.keep_limit
        ),
        Message::ProfileSnapshotsPruned(Err(error)) => {
            write!(f, "ProfileSnapshotsPruned(Err({error:?}))")
        }
        Message::RefreshAppRoutingProcesses => write!(f, "RefreshAppRoutingProcesses"),
        Message::AppRoutingProcessesLoaded(p) => {
            write!(f, "AppRoutingProcessesLoaded({} procs)", p.len())
        }
        Message::AppRoutingConfigLoaded(Ok(config)) => write!(
            f,
            "AppRoutingConfigLoaded({} packages, {} rules)",
            config.packages.len(),
            config.rules.len()
        ),
        Message::AppRoutingConfigLoaded(Err(e)) => {
            write!(f, "AppRoutingConfigLoaded(Err({e:?}))")
        }
        Message::AppRoutingPersisted(Ok(())) => write!(f, "AppRoutingPersisted(Ok)"),
        Message::AppRoutingPersisted(Err(e)) => write!(f, "AppRoutingPersisted(Err({e:?}))"),
        Message::SetAppRoutingFilter(q) => write!(f, "SetAppRoutingFilter({q})"),
        Message::SetAppRoutingMode(m) => write!(f, "SetAppRoutingMode({m:?})"),
        Message::SetAppRouteRule { process, rule } => {
            write!(f, "SetAppRouteRule({process}: {rule:?})")
        }
        Message::SetAppRoutingCategory(c) => write!(f, "SetAppRoutingCategory({c:?})"),
        Message::MoveProxyGroupUp(g) => write!(f, "MoveProxyGroupUp({g})"),
        Message::MoveProxyGroupDown(g) => write!(f, "MoveProxyGroupDown({g})"),
        Message::ResetProxyGroupOrder => write!(f, "ResetProxyGroupOrder"),
        Message::ToggleMiniHudMode => write!(f, "ToggleMiniHudMode"),
        Message::SetAlwaysOnTop(v) => write!(f, "SetAlwaysOnTop({v})"),
        Message::MiniHudMoved { x, y } => write!(f, "MiniHudMoved({x}, {y})"),
        Message::MiniHudDragReleased => write!(f, "MiniHudDragReleased"),
        Message::MiniHudPlacementUpdated(result) => match result {
            Ok(placement) => write!(
                f,
                "MiniHudPlacementUpdated({}, {}, pinned={})",
                placement.x, placement.y, placement.pinned
            ),
            Err(error) => write!(f, "MiniHudPlacementUpdated(Err({error}))"),
        },
        Message::MiniHudDisplayKnown(size) => match size {
            Some(size) => write!(f, "MiniHudDisplayKnown({}x{})", size.width, size.height),
            None => write!(f, "MiniHudDisplayKnown(None)"),
        },
        Message::WindowIdResolved(id) => write!(f, "WindowIdResolved({id:?})"),
        Message::WindowChromeDragRequested => write!(f, "WindowChromeDragRequested"),
        Message::WindowChromeToggleMaximize => write!(f, "WindowChromeToggleMaximize"),
        Message::WindowChromeMinimize => write!(f, "WindowChromeMinimize"),
        Message::WindowChromeClose => write!(f, "WindowChromeClose"),
        Message::RunScriptSandboxTest => write!(f, "RunScriptSandboxTest"),
        Message::SelectScriptPreset(p) => write!(f, "SelectScriptPreset({p})"),
        Message::UpdateScriptSandboxCode(c) => {
            write!(f, "UpdateScriptSandboxCode({} chars)", c.len())
        }
        Message::UpdateScriptSandboxInputYaml(y) => {
            write!(f, "UpdateScriptSandboxInputYaml({} chars)", y.len())
        }
        Message::ClearScriptSandbox => write!(f, "ClearScriptSandbox"),
        Message::ExportScriptDraft(kind) => write!(f, "ExportScriptDraft({kind:?})"),
        Message::ScriptExportFinished(Ok(snapshot)) => write!(
            f,
            "ScriptExportFinished(Ok({} {} bytes))",
            snapshot.file_name,
            snapshot.byte_len()
        ),
        Message::ScriptExportFinished(Err(e)) => {
            write!(f, "ScriptExportFinished(Err({:?}))", e)
        }
        _ => return None,
    })
}

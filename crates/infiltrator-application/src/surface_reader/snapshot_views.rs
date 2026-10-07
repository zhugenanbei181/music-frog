//! Snapshot observations belong to the injected product engine and actual active document.
use super::ApplicationSurfaceReader;
use infiltrator_contract::error::Failure;
use infiltrator_contract::snapshot_history::SnapshotHistorySnapshot;
use infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot;
impl ApplicationSurfaceReader {
    pub(super) async fn read_snapshot_views(
        &self,
    ) -> Result<(Option<SnapshotHistorySnapshot>, Option<YamlAstDiffSnapshot>), Failure> {
        let Some(snapshots) = &self.snapshots else {
            return Ok((None, None));
        };
        let Some(profiles) = &self.profiles else {
            return Ok((None, None));
        };
        let profile = profiles.current_profile().await?;
        let history = snapshots.history_observation(&profile);
        let detail = profiles.load_profile_detail(&profile).await?;
        let diff = snapshots.diff_observation(&profile, &detail.content);
        Ok((history, diff))
    }
}

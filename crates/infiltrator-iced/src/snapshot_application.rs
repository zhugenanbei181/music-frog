//! Iced host access to the shared snapshot application.

use infiltrator_application::snapshot_application::SnapshotApplication;
use infiltrator_contract::error::InfiltratorError;

pub async fn application() -> Result<SnapshotApplication, InfiltratorError> {
    let profiles = infiltrator_desktop::storage::profile_store()
        .await
        .map_err(|error| InfiltratorError::Config(error.to_string()))?;
    let snapshots = infiltrator_desktop::storage::snapshot_store()
        .await
        .map_err(|error| InfiltratorError::Config(error.to_string()))?;
    Ok(SnapshotApplication::new(profiles, std::sync::Arc::new(snapshots)))
}

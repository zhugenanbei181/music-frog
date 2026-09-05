//! Iced host access to the shared snapshot application.

use infiltrator_application::snapshot_application::SnapshotApplication;
use infiltrator_contract::error::InfiltratorError;

pub async fn application() -> Result<SnapshotApplication, InfiltratorError> {
    let profiles = crate::host::storage::profile_store()
        .await
        .map_err(|error| InfiltratorError::Config(error.to_string()))?;
    let snapshots = crate::host::storage::snapshot_store()
        .await
        .map_err(|error| InfiltratorError::Config(error.to_string()))?;
    Ok(SnapshotApplication::new(profiles, std::sync::Arc::new(snapshots)))
}

use super::asset::LockedSingBoxRelease;
use super::{SingBoxBinaryTarget, VERSION_STATE_SCHEMA, engine_id, invalid_journal};
use lurker_core_contract::identity::CoreVersion;
use lurker_core_ports::error::PortError;
use serde::{Deserialize, Serialize};
use singbox_driver::version::validate_version;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs;
use tokio::io::AsyncWriteExt;

static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct InstalledSingBoxReceipt {
    pub(super) schema_version: u32,
    pub(super) engine_id: String,
    pub(super) version: String,
    pub(super) target: String,
    pub(super) adapter_revision: u32,
    pub(super) archive: String,
    pub(super) archive_sha256: String,
    pub(super) binary_sha256: String,
}

#[derive(Serialize, Deserialize)]
pub(super) struct VersionSelectionJournal {
    pub(super) schema_version: u32,
    pub(super) engine_id: String,
    pub(super) target: String,
    pub(super) adapter_revision: u32,
    pub(super) revision: u64,
    pub(super) state: VersionSelectionState,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(super) enum VersionSelectionState {
    Unselected,
    Selected {
        version: String,
        previous: Option<String>,
        history: Vec<String>,
    },
}

impl VersionSelectionJournal {
    pub(super) fn empty(target: SingBoxBinaryTarget) -> Self {
        Self {
            schema_version: VERSION_STATE_SCHEMA,
            engine_id: engine_id().as_str().to_owned(),
            target: target.as_triple().to_owned(),
            adapter_revision: 0,
            revision: 0,
            state: VersionSelectionState::Unselected,
        }
    }

    pub(super) fn validate(
        &self,
        target: SingBoxBinaryTarget,
        release: &LockedSingBoxRelease,
    ) -> Result<(), PortError> {
        if self.schema_version != VERSION_STATE_SCHEMA
            || self.engine_id != engine_id().as_str()
            || self.target != target.as_triple()
            || self.revision == 0
        {
            return Err(invalid_journal(
                "sing-box version selection is not bound to this engine, target, schema, and revision",
            ));
        }
        match &self.state {
            VersionSelectionState::Unselected if self.adapter_revision == 0 => {}
            VersionSelectionState::Unselected => {
                return Err(invalid_journal(
                    "unselected sing-box version journal carries an adapter revision",
                ));
            }
            VersionSelectionState::Selected {
                version,
                previous,
                history,
            } => {
                validate_version(version)?;
                let asset = release.asset(target, version)?;
                if self.adapter_revision != asset.adapter_revision {
                    return Err(invalid_journal(
                        "sing-box selection adapter revision differs from its target-bound asset",
                    ));
                }
                if let Some(previous) = previous {
                    validate_version(previous)?;
                }
                for version in history {
                    validate_version(version)?;
                }
            }
        }
        Ok(())
    }
}

impl VersionSelectionState {
    pub(super) fn selected_version(&self) -> Option<&str> {
        match self {
            Self::Unselected => None,
            Self::Selected { version, .. } => Some(version),
        }
    }

    pub(super) fn history(&self) -> Vec<String> {
        match self {
            Self::Unselected => Vec::new(),
            Self::Selected { history, .. } => history.clone(),
        }
    }
}

pub(super) struct VerifiedInstalledBinary {
    pub(super) version: CoreVersion,
    pub(super) binary_path: PathBuf,
    pub(super) receipt: InstalledSingBoxReceipt,
}

pub(super) async fn write_json_atomically<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), PortError> {
    let parent = path.parent().ok_or_else(|| {
        PortError::Io("sing-box version state path has no parent directory".to_owned())
    })?;
    fs::create_dir_all(parent)
        .await
        .map_err(|error| PortError::Io(error.to_string()))?;
    let temp = unique_path(parent, ".sing-box-version-state", "tmp");
    let bytes = serde_json::to_vec(value)
        .map_err(|error| PortError::Failed(format!("encode sing-box version state: {error}")))?;
    let mut file = fs::File::create(&temp)
        .await
        .map_err(|error| PortError::Io(error.to_string()))?;
    if let Err(error) = file.write_all(&bytes).await {
        let _ = fs::remove_file(&temp).await;
        return Err(PortError::Io(error.to_string()));
    }
    if let Err(error) = file.sync_all().await {
        let _ = fs::remove_file(&temp).await;
        return Err(PortError::Io(error.to_string()));
    }
    drop(file);
    if let Err(error) = fs::rename(&temp, path).await {
        let _ = fs::remove_file(&temp).await;
        return Err(PortError::Io(error.to_string()));
    }
    Ok(())
}

pub(super) fn unique_path(parent: &Path, prefix: &str, suffix: &str) -> PathBuf {
    let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    parent.join(format!("{prefix}-{}-{sequence}.{suffix}", process::id()))
}

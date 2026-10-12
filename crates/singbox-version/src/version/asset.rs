use super::{
    EMBEDDED_ASSET_LOCK, SingBoxBinaryTarget, invalid_lock, valid_digest, validate_version,
};
use infiltrator_contract::capability::Capability;
use infiltrator_ports::error::PortError;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::Path;
use url::Url;

#[derive(Clone, Debug)]
pub(super) struct LockedSingBoxRelease {
    pub(super) version: String,
    pub(super) published_at: String,
    pub(super) assets: Vec<LockedSingBoxAsset>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LockedSingBoxAsset {
    pub(super) version: String,
    pub(super) target: String,
    pub(super) adapter_revision: u32,
    pub(super) archive: String,
    pub(super) archive_sha256: String,
    pub(super) download_url: String,
}

#[derive(Deserialize)]
struct AssetLockDocument {
    schema_version: u32,
    engine: String,
    release: ReleaseRecord,
    assets: Vec<AssetRecord>,
}

#[derive(Deserialize)]
struct ReleaseRecord {
    version: String,
    published_at: String,
    immutable: bool,
    prerelease: bool,
}

#[derive(Deserialize)]
struct AssetRecord {
    version: String,
    target: String,
    binary_target: String,
    adapter_revision: u32,
    archive: String,
    archive_sha256: String,
    download_url: String,
}

impl LockedSingBoxRelease {
    pub(super) fn embedded() -> Result<Self, PortError> {
        Self::from_json(EMBEDDED_ASSET_LOCK)
    }

    pub(super) fn from_json(json: &str) -> Result<Self, PortError> {
        let document: AssetLockDocument = serde_json::from_str(json)
            .map_err(|error| invalid_lock(format!("cannot decode sing-box asset lock: {error}")))?;
        if document.schema_version != 2
            || document.engine != "sing-box"
            || !document.release.immutable
            || document.release.prerelease
        {
            return Err(invalid_lock(
                "sing-box asset lock must be immutable schema-v2 release evidence",
            ));
        }
        validate_version(&document.release.version)?;
        if document.release.published_at.trim().is_empty() {
            return Err(invalid_lock("sing-box release date is missing"));
        }

        let mut seen = BTreeSet::new();
        let mut assets = Vec::with_capacity(document.assets.len());
        for record in document.assets {
            validate_version(&record.version)?;
            if record.binary_target != record.target
                || SingBoxBinaryTarget::from_triple(&record.target).is_none()
                || record.adapter_revision == 0
                || !valid_digest(&record.archive_sha256)
                || Path::new(&record.archive)
                    .file_name()
                    .and_then(|name| name.to_str())
                    != Some(record.archive.as_str())
            {
                return Err(invalid_lock(format!(
                    "sing-box asset record for {} has an invalid target, revision, archive, or digest",
                    record.version
                )));
            }
            let url = Url::parse(&record.download_url)
                .map_err(|error| invalid_lock(format!("invalid sing-box asset URL: {error}")))?;
            let expected_path = format!(
                "/SagerNet/sing-box/releases/download/{}/{}",
                record.version, record.archive
            );
            if url.scheme() != "https"
                || url.host_str() != Some("github.com")
                || url.path() != expected_path
            {
                return Err(invalid_lock(
                    "sing-box locked asset URL must identify its exact official versioned release archive",
                ));
            }
            let identity = (record.version.clone(), record.target.clone());
            if !seen.insert(identity) {
                return Err(invalid_lock(
                    "sing-box asset lock contains a duplicate version/target record",
                ));
            }
            assets.push(LockedSingBoxAsset {
                version: record.version,
                target: record.target,
                adapter_revision: record.adapter_revision,
                archive: record.archive,
                archive_sha256: record.archive_sha256,
                download_url: record.download_url,
            });
        }
        if assets.is_empty()
            || !assets
                .iter()
                .any(|asset| asset.version == document.release.version)
        {
            return Err(invalid_lock(
                "sing-box asset lock has no assets for its declared latest release",
            ));
        }
        Ok(Self {
            version: document.release.version,
            published_at: document.release.published_at,
            assets,
        })
    }

    pub(super) fn asset(
        &self,
        target: SingBoxBinaryTarget,
        version: &str,
    ) -> Result<LockedSingBoxAsset, PortError> {
        self.assets
            .iter()
            .find(|asset| asset.target == target.as_triple() && asset.version == version)
            .cloned()
            .ok_or_else(|| {
                PortError::unsupported(
                    Capability::CoreVersionInstall,
                    format!(
                        "sing-box version {version} has no locked asset for {}",
                        target.as_triple()
                    ),
                )
            })
    }

    pub(super) fn target_assets(&self, target: SingBoxBinaryTarget) -> Vec<LockedSingBoxAsset> {
        self.assets
            .iter()
            .filter(|asset| asset.target == target.as_triple())
            .cloned()
            .collect()
    }
}

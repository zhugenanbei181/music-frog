//! Engine-specific executable selection backed by `VersionManager`.

use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_capabilities::EngineBinaryTarget;
use infiltrator_contract::core_identity::{CoreVersion, EngineId};
use infiltrator_contract::version::CoreArtifactVerification;
use infiltrator_ports::core_binary_selection::{
    CoreBinarySelection, CoreBinarySelectionPolicy, CoreBinarySelectionPort,
    CoreBinarySelectionRequest, CoreBinarySelectionSource,
};
use infiltrator_ports::error::PortError;
use mihomo_api::error::MihomoError;
use mihomo_version::bundled_kernel::{bundled_binary_sha256, copy_bundled_binary};
use mihomo_version::engine;
use mihomo_version::manager::{VersionInfo, VersionManager};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::process::Command;

pub struct MihomoBinarySelectionAdapter {
    engine_id: EngineId,
    manager: VersionManager,
    verification: Arc<Mutex<CoreArtifactVerification>>,
}

impl MihomoBinarySelectionAdapter {
    pub fn new(
        engine_id: EngineId,
        manager: VersionManager,
        verification: Arc<Mutex<CoreArtifactVerification>>,
    ) -> Result<Self, PortError> {
        if engine_id != engine::engine_id() || manager.engine_id() != &engine_id {
            return Err(unsupported_engine(&engine_id));
        }
        Ok(Self {
            engine_id,
            manager,
            verification,
        })
    }

    async fn managed_selection(&self) -> Result<CoreBinarySelection, PortError> {
        let mut installed = self.manager.list_installed().await.map_err(version_error)?;
        if installed.is_empty() {
            return Err(PortError::NotFound(
                "no installed Mihomo version is available".to_owned(),
            ));
        }
        sort_versions_desc(&mut installed);
        let selected = installed
            .iter()
            .find(|version| version.is_default)
            .or_else(|| installed.first())
            .ok_or_else(|| {
                PortError::NotFound("no installed Mihomo version is available".into())
            })?;
        if !selected.is_default {
            self.manager
                .set_default(&selected.version)
                .await
                .map_err(version_error)?;
        }
        let path = self
            .manager
            .get_binary_path(Some(&selected.version))
            .await
            .map_err(version_error)?;
        let version = read_binary_version(&path).await?;
        if version.raw() != selected.version {
            return Err(PortError::Failed(format!(
                "managed Mihomo version label {} does not match binary-reported {}",
                selected.version,
                version.raw()
            )));
        }
        Ok(CoreBinarySelection {
            engine_id: self.engine_id.clone(),
            binary_target: EngineBinaryTarget::current(),
            verification: matching_verification(&self.verification, &self.engine_id, &version),
            version,
            binary_path: path,
            related_asset_candidates: Vec::new(),
            source: CoreBinarySelectionSource::ManagedVersion,
        })
    }

    async fn bundled_selection(
        &self,
        candidates: &[PathBuf],
        data_directory: &Path,
    ) -> Result<Option<CoreBinarySelection>, PortError> {
        let path = copy_bundled_binary(candidates, data_directory)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let Some(path) = path else {
            return Ok(None);
        };
        let version = read_binary_version(&path).await?;
        let content_sha256 = bundled_binary_sha256(&path)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        Ok(Some(CoreBinarySelection {
            engine_id: self.engine_id.clone(),
            binary_target: EngineBinaryTarget::current(),
            version,
            binary_path: path,
            related_asset_candidates: candidates.to_vec(),
            source: CoreBinarySelectionSource::BundledPackage { content_sha256 },
            // A content-addressed copy proves byte stability, not provenance
            // against the official release lock for this target.
            verification: CoreArtifactVerification::Unknown,
        }))
    }
}

#[async_trait::async_trait]
impl CoreBinarySelectionPort for MihomoBinarySelectionAdapter {
    fn engine_id(&self) -> EngineId {
        self.engine_id.clone()
    }

    async fn select(
        &self,
        request: CoreBinarySelectionRequest,
    ) -> Result<CoreBinarySelection, PortError> {
        if request.engine_id != self.engine_id {
            return Err(unsupported_engine(&request.engine_id));
        }
        match request.policy {
            CoreBinarySelectionPolicy::ManagedOnly => self.managed_selection().await,
            CoreBinarySelectionPolicy::ManagedWithBundledFallback(source) => {
                match self.managed_selection().await {
                    Ok(mut selection) => {
                        selection.related_asset_candidates = source.candidates;
                        Ok(selection)
                    }
                    Err(PortError::NotFound(_)) => self
                        .bundled_selection(&source.candidates, &source.data_directory)
                        .await?
                        .ok_or_else(|| {
                            PortError::NotFound(
                                "no installed Mihomo version or bundled binary is available"
                                    .to_owned(),
                            )
                        }),
                    Err(error) => Err(error),
                }
            }
            CoreBinarySelectionPolicy::PreferBundled(source) => {
                if let Some(selection) = self
                    .bundled_selection(&source.candidates, &source.data_directory)
                    .await?
                {
                    Ok(selection)
                } else {
                    let mut selection = self.managed_selection().await?;
                    selection.related_asset_candidates = source.candidates;
                    Ok(selection)
                }
            }
            CoreBinarySelectionPolicy::RequireBundled(source) => self
                .bundled_selection(&source.candidates, &source.data_directory)
                .await?
                .ok_or_else(|| {
                    PortError::NotFound("required bundled Mihomo binary was not found".to_owned())
                }),
        }
    }
}

async fn read_binary_version(path: &Path) -> Result<CoreVersion, PortError> {
    let output = Command::new(path)
        .arg("-v")
        .output()
        .await
        .map_err(|error| PortError::Failed(format!("run Mihomo version probe: {error}")))?;
    if !output.status.success() {
        return Err(PortError::Failed(format!(
            "Mihomo version probe exited with {}",
            output.status
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let version = stdout
        .split_whitespace()
        .find_map(version_token)
        .ok_or_else(|| {
            PortError::Failed("Mihomo `-v` output did not contain a version token".to_owned())
        })?;
    CoreVersion::new(version).map_err(|error| PortError::Failed(error.to_string()))
}

fn version_token(token: &str) -> Option<String> {
    let token = token.trim_matches(|character: char| {
        !character.is_ascii_alphanumeric() && !matches!(character, '.' | '-' | '+')
    });
    let version = token.strip_prefix('v').unwrap_or(token);
    let numeric = version.split(['-', '+']).next()?;
    let mut parts = numeric.split('.');
    let first = parts.next()?;
    let second = parts.next()?;
    if first.is_empty()
        || second.is_empty()
        || !first.bytes().all(|byte| byte.is_ascii_digit())
        || !second.bytes().all(|byte| byte.is_ascii_digit())
        || parts.any(|part| !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return None;
    }
    Some(token.to_owned())
}

fn sort_versions_desc(versions: &mut [VersionInfo]) {
    versions.sort_by(|left, right| compare_versions_desc(&left.version, &right.version));
}

fn compare_versions_desc(left: &str, right: &str) -> Ordering {
    let left_parts = version_parts(left);
    let right_parts = version_parts(right);
    match (left_parts, right_parts) {
        (Some(left), Some(right)) => right.cmp(&left),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => right.cmp(left),
    }
}

fn version_parts(value: &str) -> Option<(u64, u64, u64)> {
    let version = value.trim().trim_start_matches('v').split('-').next()?;
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

fn matching_verification(
    verification: &Arc<Mutex<CoreArtifactVerification>>,
    engine_id: &EngineId,
    selected_version: &CoreVersion,
) -> CoreArtifactVerification {
    let Ok(verification) = verification.lock() else {
        return CoreArtifactVerification::Unknown;
    };
    match &*verification {
        CoreArtifactVerification::Verified {
            engine_id: verified_engine,
            version,
        } if verified_engine == engine_id && version == selected_version.raw() => {
            verification.clone()
        }
        CoreArtifactVerification::Rejected {
            engine_id: verified_engine,
            version,
            failure,
        } if verified_engine == engine_id && version == selected_version.raw() => {
            CoreArtifactVerification::Rejected {
                engine_id: verified_engine.clone(),
                version: version.clone(),
                failure: failure.clone(),
            }
        }
        _ => CoreArtifactVerification::Unknown,
    }
}

fn unsupported_engine(engine_id: &EngineId) -> PortError {
    PortError::unsupported(
        Capability::CoreVersionInstall,
        format!(
            "Mihomo binary selection does not support {}",
            engine_id.as_str()
        ),
    )
}

fn version_error(error: MihomoError) -> PortError {
    PortError::Failed(error.to_string())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use infiltrator_contract::version::CoreArtifactVerification;
    use infiltrator_ports::core_binary_selection::BundledBinarySource;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;
    use tokio::fs;

    fn engine() -> EngineId {
        engine::engine_id()
    }

    async fn write_version_script(path: &Path, output: &str) {
        fs::write(path, format!("#!/bin/sh\nprintf '%s\\n' '{output}'\n"))
            .await
            .expect("write fake Mihomo executable");
        let mut permissions = fs::metadata(path).await.unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).await.unwrap();
    }

    #[test]
    fn version_parser_reads_engine_reported_tag_without_guessing_from_filename() {
        assert_eq!(version_token("v1.19.32"), Some("v1.19.32".to_owned()));
        assert_eq!(version_token("1.19.32"), Some("1.19.32".to_owned()));
        assert_eq!(version_token("go1.24.0"), None);
        assert_eq!(version_token("linux/amd64"), None);
    }

    #[tokio::test]
    async fn required_bundled_binary_returns_engine_version_digest_and_open_verification() {
        let home = tempdir().unwrap();
        let source = home.path().join("bundled-mihomo");
        write_version_script(&source, "Mihomo Meta v1.19.32 linux amd64 with go1.24.0").await;
        let manager = VersionManager::for_engine(home.path().into(), engine()).unwrap();
        let adapter = MihomoBinarySelectionAdapter::new(
            engine(),
            manager,
            Arc::new(Mutex::new(CoreArtifactVerification::Unknown)),
        )
        .unwrap();

        let selection = adapter
            .select(CoreBinarySelectionRequest {
                engine_id: engine(),
                policy: CoreBinarySelectionPolicy::RequireBundled(BundledBinarySource {
                    candidates: vec![source.clone()],
                    data_directory: home.path().join("runtime"),
                }),
            })
            .await
            .unwrap();

        assert_eq!(selection.engine_id, engine());
        assert_eq!(selection.version.raw(), "v1.19.32");
        assert_eq!(selection.binary_path.file_name().unwrap(), "mihomo");
        assert!(matches!(
            selection.source,
            CoreBinarySelectionSource::BundledPackage { .. }
        ));
        assert_eq!(selection.verification, CoreArtifactVerification::Unknown);
        assert_eq!(
            fs::read(&selection.binary_path).await.unwrap(),
            fs::read(source).await.unwrap()
        );
    }

    #[tokio::test]
    async fn managed_only_uses_the_version_manager_default_and_binds_its_version() {
        let home = tempdir().unwrap();
        let version_dir = home.path().join("engines/mihomo/versions/v1.19.32");
        fs::create_dir_all(&version_dir).await.unwrap();
        write_version_script(
            &version_dir.join("mihomo"),
            "Mihomo Meta v1.19.32 linux amd64 with go1.24.0",
        )
        .await;
        let manager = VersionManager::for_engine(home.path().into(), engine()).unwrap();
        let adapter = MihomoBinarySelectionAdapter::new(
            engine(),
            manager,
            Arc::new(Mutex::new(CoreArtifactVerification::Unknown)),
        )
        .unwrap();

        let selection = adapter
            .select(CoreBinarySelectionRequest {
                engine_id: engine(),
                policy: CoreBinarySelectionPolicy::ManagedOnly,
            })
            .await
            .unwrap();

        assert_eq!(selection.version.raw(), "v1.19.32");
        assert_eq!(selection.binary_path, version_dir.join("mihomo"));
        assert_eq!(selection.source, CoreBinarySelectionSource::ManagedVersion);
    }

    #[tokio::test]
    async fn prefer_bundled_uses_bundled_asset_even_when_a_managed_version_exists() {
        let home = tempdir().unwrap();
        let version_dir = home.path().join("engines/mihomo/versions/v1.18.0");
        fs::create_dir_all(&version_dir).await.unwrap();
        write_version_script(&version_dir.join("mihomo"), "Mihomo Meta v1.18.0").await;
        let source = home.path().join("bundled-mihomo");
        write_version_script(&source, "Mihomo Meta v1.19.32").await;
        let manager = VersionManager::for_engine(home.path().into(), engine()).unwrap();
        let adapter = MihomoBinarySelectionAdapter::new(
            engine(),
            manager,
            Arc::new(Mutex::new(CoreArtifactVerification::Unknown)),
        )
        .unwrap();

        let selection = adapter
            .select(CoreBinarySelectionRequest {
                engine_id: engine(),
                policy: CoreBinarySelectionPolicy::PreferBundled(BundledBinarySource {
                    candidates: vec![source],
                    data_directory: home.path().join("runtime"),
                }),
            })
            .await
            .unwrap();

        assert_eq!(selection.version.raw(), "v1.19.32");
        assert!(matches!(
            selection.source,
            CoreBinarySelectionSource::BundledPackage { .. }
        ));
    }

    #[tokio::test]
    async fn managed_with_bundled_fallback_uses_bundle_when_no_managed_version_exists() {
        let home = tempdir().unwrap();
        let source = home.path().join("bundled-mihomo");
        write_version_script(&source, "Mihomo Meta v1.19.32").await;
        let manager = VersionManager::for_engine(home.path().into(), engine()).unwrap();
        let adapter = MihomoBinarySelectionAdapter::new(
            engine(),
            manager,
            Arc::new(Mutex::new(CoreArtifactVerification::Unknown)),
        )
        .unwrap();

        let selection = adapter
            .select(CoreBinarySelectionRequest {
                engine_id: engine(),
                policy: CoreBinarySelectionPolicy::ManagedWithBundledFallback(
                    BundledBinarySource {
                        candidates: vec![source],
                        data_directory: home.path().join("runtime"),
                    },
                ),
            })
            .await
            .unwrap();

        assert_eq!(selection.version.raw(), "v1.19.32");
        assert!(matches!(
            selection.source,
            CoreBinarySelectionSource::BundledPackage { .. }
        ));
    }

    #[tokio::test]
    async fn managed_with_bundle_fallback_preserves_package_sidecar_search_roots() {
        let home = tempdir().unwrap();
        let version_dir = home.path().join("engines/mihomo/versions/v1.19.32");
        fs::create_dir_all(&version_dir).await.unwrap();
        write_version_script(&version_dir.join("mihomo"), "Mihomo Meta v1.19.32").await;
        let package_sidecar_root = home.path().join("package-assets");
        let manager = VersionManager::for_engine(home.path().into(), engine()).unwrap();
        let adapter = MihomoBinarySelectionAdapter::new(
            engine(),
            manager,
            Arc::new(Mutex::new(CoreArtifactVerification::Unknown)),
        )
        .unwrap();

        let selection = adapter
            .select(CoreBinarySelectionRequest {
                engine_id: engine(),
                policy: CoreBinarySelectionPolicy::ManagedWithBundledFallback(
                    BundledBinarySource {
                        candidates: vec![package_sidecar_root.clone()],
                        data_directory: home.path().join("runtime"),
                    },
                ),
            })
            .await
            .unwrap();

        assert_eq!(selection.source, CoreBinarySelectionSource::ManagedVersion);
        assert_eq!(
            selection.related_asset_candidates,
            vec![package_sidecar_root]
        );
    }

    #[tokio::test]
    async fn required_bundle_does_not_fall_back_to_an_installed_version() {
        let home = tempdir().unwrap();
        let version_dir = home.path().join("engines/mihomo/versions/v1.19.32");
        fs::create_dir_all(&version_dir).await.unwrap();
        write_version_script(&version_dir.join("mihomo"), "Mihomo Meta v1.19.32").await;
        let manager = VersionManager::for_engine(home.path().into(), engine()).unwrap();
        let adapter = MihomoBinarySelectionAdapter::new(
            engine(),
            manager,
            Arc::new(Mutex::new(CoreArtifactVerification::Unknown)),
        )
        .unwrap();

        let result = adapter
            .select(CoreBinarySelectionRequest {
                engine_id: engine(),
                policy: CoreBinarySelectionPolicy::RequireBundled(BundledBinarySource {
                    candidates: vec![home.path().join("missing")],
                    data_directory: home.path().join("runtime"),
                }),
            })
            .await;

        assert!(matches!(result, Err(PortError::NotFound(_))));
    }

    #[tokio::test]
    async fn mismatched_engine_request_is_unsupported_before_candidate_or_manager_io() {
        let home = tempdir().unwrap();
        let manager = VersionManager::for_engine(home.path().into(), engine()).unwrap();
        let adapter = MihomoBinarySelectionAdapter::new(
            engine(),
            manager,
            Arc::new(Mutex::new(CoreArtifactVerification::Unknown)),
        )
        .unwrap();
        let bundle_dir = home.path().join("runtime");

        let result = adapter
            .select(CoreBinarySelectionRequest {
                engine_id: EngineId::new("sing-box").unwrap(),
                policy: CoreBinarySelectionPolicy::RequireBundled(BundledBinarySource {
                    candidates: vec![home.path().join("missing")],
                    data_directory: bundle_dir.clone(),
                }),
            })
            .await;

        assert!(matches!(
            result,
            Err(PortError::Unsupported {
                capability: Capability::CoreVersionInstall,
                ..
            })
        ));
        assert!(!bundle_dir.exists());
        assert!(!home.path().join("engines/mihomo/versions").exists());
    }
}

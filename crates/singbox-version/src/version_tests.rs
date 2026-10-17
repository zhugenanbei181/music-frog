//! test-intent: behavior

use super::asset::LockedSingBoxAsset;
use super::engine_id;
use super::{
    CoreBinarySelectionPolicy, CoreBinarySelectionPort, CoreBinarySelectionRequest,
    EngineVersionBackendFactory, ErrorCode, LockedSingBoxRelease, SingBoxArchiveDownloader,
    SingBoxBinaryTarget, SingBoxEngineVersionBackendFactory, SingBoxVersionBackend,
    VersionActivationRequest, VersionInstallRequest, VersionPort, VersionProgressSink,
    VersionRollbackRequest, VersionUninstallRequest, sha256_hex,
};
use async_trait::async_trait;
use flate2::Compression;
use flate2::write::GzEncoder;
use infiltrator_contract::core_capabilities::EngineBinaryTarget;
use infiltrator_contract::version::{CoreArtifactVerification, VersionDownloadProgress};
use infiltrator_ports::core_binary_selection::{BundledBinarySource, CoreBinarySelectionSource};
use lurker_core_contract::identity::CoreVersion;
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::error::PortError;
use singbox_driver::version::SingBoxBinaryInspector;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::from_utf8;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tar::{Builder, Header};
use tempfile::TempDir;
use tokio::fs;

#[derive(Clone, Copy)]
enum CancelAt {
    Never,
    Check(usize),
}

struct TestProgress {
    cancel_at: CancelAt,
    checks: AtomicUsize,
    updates: Mutex<Vec<VersionDownloadProgress>>,
}

impl TestProgress {
    fn new(cancel_at: CancelAt) -> Self {
        Self {
            cancel_at,
            checks: AtomicUsize::new(0),
            updates: Mutex::new(Vec::new()),
        }
    }
}

impl VersionProgressSink for TestProgress {
    fn progress(&self, progress: VersionDownloadProgress) {
        self.updates.lock().unwrap().push(progress);
    }

    fn is_cancelled(&self) -> bool {
        let check = self.checks.fetch_add(1, Ordering::SeqCst) + 1;
        matches!(self.cancel_at, CancelAt::Check(cancel_at) if check >= cancel_at)
    }
}

struct FakeArchiveDownloader {
    archives: HashMap<String, Vec<u8>>,
    calls: AtomicUsize,
}

impl FakeArchiveDownloader {
    fn new(archives: HashMap<String, Vec<u8>>) -> Self {
        Self {
            archives,
            calls: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl SingBoxArchiveDownloader for FakeArchiveDownloader {
    async fn download(
        &self,
        asset: &LockedSingBoxAsset,
        progress: &dyn VersionProgressSink,
    ) -> Result<Vec<u8>, PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let bytes = self
            .archives
            .get(&asset.version)
            .cloned()
            .ok_or_else(|| PortError::NotFound(format!("no fixture for {}", asset.version)))?;
        progress.progress(VersionDownloadProgress {
            downloaded: bytes.len() as u64,
            total: Some(bytes.len() as u64),
        });
        Ok(bytes)
    }
}

struct MarkerBinaryInspector;

#[async_trait]
impl SingBoxBinaryInspector for MarkerBinaryInspector {
    async fn version(&self, path: &Path) -> Result<CoreVersion, PortError> {
        let bytes = fs::read(path)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let marker = bytes
            .strip_prefix(b"\x7fELF")
            .ok_or_else(|| PortError::Failed("fixture is not an ELF binary".to_owned()))?;
        let version = from_utf8(marker).map_err(|error| PortError::Failed(error.to_string()))?;
        CoreVersion::new(version.to_owned()).map_err(|error| PortError::Failed(error.to_string()))
    }
}

fn archive_with_binary(version: &str) -> Vec<u8> {
    let binary = [b"\x7fELF".as_slice(), version.as_bytes()].concat();
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut archive = Builder::new(encoder);
    let mut header = Header::new_gnu();
    header.set_size(binary.len() as u64);
    header.set_mode(0o755);
    header.set_cksum();
    archive
        .append_data(&mut header, "sing-box-release/sing-box", binary.as_slice())
        .unwrap();
    archive.into_inner().unwrap().finish().unwrap()
}

fn target_asset(version: &str, target: SingBoxBinaryTarget, archive: &[u8]) -> LockedSingBoxAsset {
    LockedSingBoxAsset {
        version: version.to_owned(),
        target: target.as_triple().to_owned(),
        adapter_revision: 1,
        archive: format!("sing-box-{version}-fixture.tar.gz"),
        archive_sha256: sha256_hex(archive),
        download_url: format!(
            "https://github.com/SagerNet/sing-box/releases/download/{version}/fixture.tar.gz"
        ),
    }
}

fn lock(
    latest: &str,
    target: SingBoxBinaryTarget,
    assets: Vec<LockedSingBoxAsset>,
) -> LockedSingBoxRelease {
    LockedSingBoxRelease {
        version: latest.to_owned(),
        published_at: "2026-10-09T05:35:00Z".to_owned(),
        assets: assets
            .into_iter()
            .map(|asset| {
                assert_eq!(asset.target, target.as_triple());
                asset
            })
            .collect(),
    }
}

fn backend(
    directory: &TempDir,
    release: LockedSingBoxRelease,
    archives: HashMap<String, Vec<u8>>,
) -> (SingBoxVersionBackend, Arc<FakeArchiveDownloader>) {
    let downloader = Arc::new(FakeArchiveDownloader::new(archives));
    let backend = SingBoxVersionBackend::new(
        directory.path().join("engines/sing-box"),
        SingBoxBinaryTarget::LinuxX86_64Gnu,
        Arc::new(release),
        downloader.clone(),
        Arc::new(MarkerBinaryInspector),
    );
    (backend, downloader)
}

fn install_request(version: &str, progress: Arc<TestProgress>) -> VersionInstallRequest {
    VersionInstallRequest {
        engine_id: engine_id(),
        version: version.to_owned(),
        progress,
    }
}

#[tokio::test]
async fn locked_target_install_activation_selection_and_rollback_are_engine_scoped() {
    let directory = TempDir::new().unwrap();
    let target = SingBoxBinaryTarget::LinuxX86_64Gnu;
    let latest_archive = archive_with_binary("v1.14.3");
    let prior_archive = archive_with_binary("v1.14.2");
    let release = lock(
        "v1.14.3",
        target,
        vec![
            target_asset("v1.14.3", target, &latest_archive),
            target_asset("v1.14.2", target, &prior_archive),
        ],
    );
    let mut archives = HashMap::new();
    archives.insert("v1.14.3".to_owned(), latest_archive);
    archives.insert("v1.14.2".to_owned(), prior_archive);
    let (backend, downloader) = backend(&directory, release, archives);

    for version in ["v1.14.2", "v1.14.3"] {
        VersionPort::install(
            &backend,
            install_request(version, Arc::new(TestProgress::new(CancelAt::Never))),
        )
        .await
        .unwrap();
    }
    assert_eq!(downloader.calls.load(Ordering::SeqCst), 2);

    VersionPort::activate(
        &backend,
        VersionActivationRequest {
            engine_id: engine_id(),
            version: "v1.14.2".to_owned(),
        },
    )
    .await
    .unwrap();
    VersionPort::activate(
        &backend,
        VersionActivationRequest {
            engine_id: engine_id(),
            version: "v1.14.3".to_owned(),
        },
    )
    .await
    .unwrap();

    let rollback = VersionPort::rollback_snapshot(&backend, &engine_id())
        .await
        .unwrap();
    assert_eq!(rollback.current.as_deref(), Some("v1.14.3"));
    assert_eq!(rollback.target.as_deref(), Some("v1.14.2"));
    assert_eq!(
        VersionPort::rollback(
            &backend,
            VersionRollbackRequest {
                engine_id: engine_id(),
            }
        )
        .await
        .unwrap(),
        "v1.14.2"
    );

    let selected = CoreBinarySelectionPort::select(
        &backend,
        CoreBinarySelectionRequest {
            engine_id: engine_id(),
            policy: CoreBinarySelectionPolicy::ManagedOnly,
        },
    )
    .await
    .unwrap();
    assert_eq!(selected.version.raw(), "v1.14.2");
    assert_eq!(
        selected.binary_target,
        EngineBinaryTarget::from_target_triple(target.as_triple())
            .expect("locked target is registered"),
    );
    assert!(matches!(
        selected.source,
        CoreBinarySelectionSource::ManagedVersion
    ));
    assert_eq!(
        selected.verification,
        CoreArtifactVerification::Verified {
            engine_id: engine_id(),
            version: "v1.14.2".to_owned(),
        }
    );
    assert!(Path::new(&selected.binary_path).exists());

    let installed = VersionPort::list_installed(&backend, &engine_id())
        .await
        .unwrap();
    assert_eq!(installed.len(), 2);
    assert!(
        installed
            .iter()
            .any(|version| version.version == "v1.14.2" && version.is_default)
    );
    assert!(
        installed
            .iter()
            .any(|version| version.version == "v1.14.3" && !version.is_default)
    );
    let active_uninstall = VersionPort::uninstall(
        &backend,
        VersionUninstallRequest {
            engine_id: engine_id(),
            version: "v1.14.2".to_owned(),
        },
    )
    .await;
    assert!(matches!(
        active_uninstall,
        Err(PortError::Rejected(ref failure)) if failure.code == ErrorCode::InvalidState
    ));
}

#[tokio::test]
async fn archive_digest_mismatch_rejects_before_installation_publication() {
    let directory = TempDir::new().unwrap();
    let target = SingBoxBinaryTarget::LinuxX86_64Gnu;
    let archive = archive_with_binary("v1.14.3");
    let mut asset = target_asset("v1.14.3", target, &archive);
    asset.archive_sha256 = "0".repeat(64);
    let (backend, _) = backend(
        &directory,
        lock("v1.14.3", target, vec![asset]),
        HashMap::from([("v1.14.3".to_owned(), archive)]),
    );

    let failure = VersionPort::install(
        &backend,
        install_request("v1.14.3", Arc::new(TestProgress::new(CancelAt::Never))),
    )
    .await
    .unwrap_err();
    assert!(matches!(
        failure,
        PortError::Rejected(ref error) if error.code == ErrorCode::Configuration
    ));
    assert!(
        !directory
            .path()
            .join("engines/sing-box/targets")
            .join(target.as_triple())
            .join("versions")
            .exists()
    );
    assert!(matches!(
        VersionPort::verification(&backend, &engine_id()),
        CoreArtifactVerification::Rejected { .. }
    ));
}

#[tokio::test]
async fn binary_version_mismatch_fails_before_publication() {
    let directory = TempDir::new().unwrap();
    let target = SingBoxBinaryTarget::LinuxX86_64Gnu;
    let archive = archive_with_binary("v9.9.9");
    let asset = target_asset("v1.14.3", target, &archive);
    let (backend, _) = backend(
        &directory,
        lock("v1.14.3", target, vec![asset]),
        HashMap::from([("v1.14.3".to_owned(), archive)]),
    );

    let result = VersionPort::install(
        &backend,
        install_request("v1.14.3", Arc::new(TestProgress::new(CancelAt::Never))),
    )
    .await;
    assert!(matches!(result, Err(PortError::Rejected(_))));
    assert!(
        !directory
            .path()
            .join("engines/sing-box/targets")
            .join(target.as_triple())
            .join("versions/v1.14.3")
            .exists()
    );
}

#[tokio::test]
async fn cancellation_after_staging_cleans_files_and_allows_retry() {
    let directory = TempDir::new().unwrap();
    let target = SingBoxBinaryTarget::LinuxX86_64Gnu;
    let archive = archive_with_binary("v1.14.3");
    let asset = target_asset("v1.14.3", target, &archive);
    let (backend, downloader) = backend(
        &directory,
        lock("v1.14.3", target, vec![asset]),
        HashMap::from([("v1.14.3".to_owned(), archive)]),
    );

    let canceled = VersionPort::install(
        &backend,
        install_request("v1.14.3", Arc::new(TestProgress::new(CancelAt::Check(3)))),
    )
    .await;
    assert!(matches!(
        canceled,
        Err(PortError::Rejected(ref failure)) if failure.code == ErrorCode::Canceled
    ));
    let versions = directory
        .path()
        .join("engines/sing-box/targets")
        .join(target.as_triple())
        .join("versions");
    let mut entries = fs::read_dir(&versions).await.unwrap();
    assert!(entries.next_entry().await.unwrap().is_none());

    VersionPort::install(
        &backend,
        install_request("v1.14.3", Arc::new(TestProgress::new(CancelAt::Never))),
    )
    .await
    .unwrap();
    assert_eq!(downloader.calls.load(Ordering::SeqCst), 2);
    assert!(versions.join("v1.14.3/sing-box").exists());
}

#[test]
fn unknown_target_missing_digest_and_foreign_factory_requests_fail_closed() {
    assert_eq!(
        SingBoxBinaryTarget::from_triple("x86_64-apple-darwin"),
        None
    );
    let missing_digest = r#"{
      "schema_version": 2,
      "engine": "sing-box",
      "release": {
        "version": "v1.14.3",
        "published_at": "2026-10-09T05:35:00Z",
        "immutable": true,
        "prerelease": false
      },
      "assets": [{
        "version": "v1.14.3",
        "target": "x86_64-unknown-linux-gnu",
        "binary_target": "x86_64-unknown-linux-gnu",
        "adapter_revision": 1,
        "archive": "sing-box.tar.gz",
        "download_url": "https://github.com/SagerNet/sing-box/releases/download/v1.14.3/sing-box.tar.gz"
      }]
    }"#;
    let failure = LockedSingBoxRelease::from_json(missing_digest).unwrap_err();
    assert_eq!(failure.error_code(), ErrorCode::Configuration);
}

#[test]
fn embedded_release_lock_is_target_and_adapter_revision_bound() {
    let release = LockedSingBoxRelease::embedded().unwrap();
    assert_eq!(release.version, "v1.14.3");
    for target in [
        SingBoxBinaryTarget::LinuxX86_64Gnu,
        SingBoxBinaryTarget::LinuxAarch64Gnu,
        SingBoxBinaryTarget::AndroidAarch64,
        SingBoxBinaryTarget::AndroidX86_64,
    ] {
        let asset = release.asset(target, "v1.14.3").unwrap();
        assert_eq!(asset.target, target.as_triple());
        assert_eq!(asset.adapter_revision, 1);
        assert_eq!(asset.archive_sha256.len(), 64);
    }
}

#[tokio::test]
async fn foreign_engine_and_bundled_policies_do_not_touch_files_or_download() {
    let directory = TempDir::new().unwrap();
    let target = SingBoxBinaryTarget::LinuxX86_64Gnu;
    let archive = archive_with_binary("v1.14.3");
    let asset = target_asset("v1.14.3", target, &archive);
    let (backend, downloader) = backend(
        &directory,
        lock("v1.14.3", target, vec![asset]),
        HashMap::from([("v1.14.3".to_owned(), archive)]),
    );

    let wrong_engine = EngineId::new("mihomo").unwrap();
    assert!(matches!(
        VersionPort::list_installed(&backend, &wrong_engine).await,
        Err(PortError::Unsupported { .. })
    ));
    assert!(matches!(
        CoreBinarySelectionPort::select(
            &backend,
            CoreBinarySelectionRequest {
                engine_id: engine_id(),
                policy: CoreBinarySelectionPolicy::RequireBundled(BundledBinarySource {
                    candidates: vec![PathBuf::from("/missing")],
                    data_directory: directory.path().to_path_buf(),
                }),
            },
        )
        .await,
        Err(PortError::Unsupported { .. })
    ));
    assert_eq!(downloader.calls.load(Ordering::SeqCst), 0);
    assert!(!directory.path().join("engines/sing-box").exists());

    let factory = SingBoxEngineVersionBackendFactory {
        backend: Arc::new(backend),
    };
    assert_eq!(
        EngineVersionBackendFactory::version_port(&factory, &engine_id())
            .unwrap()
            .engine_id(),
        engine_id()
    );
    assert_eq!(
        EngineVersionBackendFactory::binary_selection_port(&factory, &engine_id())
            .unwrap()
            .engine_id(),
        engine_id()
    );
    assert!(matches!(
        EngineVersionBackendFactory::version_port(&factory, &wrong_engine),
        Err(PortError::Unsupported { .. })
    ));
}

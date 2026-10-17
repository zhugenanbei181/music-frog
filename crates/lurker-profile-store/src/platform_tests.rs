//! Platform substitution tests: the store must use the injected
//! [`StorePlatform`] for every byte and every fence, and must fail with the
//! platform's typed error instead of silently degrading.

use super::{NativeStorePlatform, StoreDirEntry, StorePlatform};
use crate::document_store::write_document;
use crate::durable_replace::DurableReplaceError;
use crate::file_lock::ProfileWriteFence;
use crate::instance_lock::InstanceLockFence;
#[cfg(unix)]
use crate::instance_lock::try_acquire_instance_lock;
use crate::layout::ProfileStoreLayout;
use crate::platform::StoreEntryKind;
use crate::store::ProfileDocumentExtension;
use crate::test_core::TestCore;
use lurker_core_contract::profile_document_identity::ProfileId;
use lurker_core_ports::error::PortError;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

/// Call counters proving which implementation served each operation.
#[derive(Default)]
struct RecordedCalls {
    reads: AtomicUsize,
    atomic_writes: AtomicUsize,
    atomic_replacements: AtomicUsize,
    profile_fences: AtomicUsize,
    instance_locks: AtomicUsize,
}

/// A platform that records every call and can inject a replacement failure.
/// Everything not intercepted delegates to the native platform, so the test
/// exercises the real durable primitive behind the substitution boundary.
#[derive(Default)]
struct RecordingPlatform {
    calls: RecordedCalls,
    fail_replacement: AtomicBool,
    temporaries: AtomicU64,
}

impl RecordingPlatform {
    fn failing_replacement() -> Self {
        Self {
            fail_replacement: AtomicBool::new(true),
            ..Self::default()
        }
    }

    fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

#[async_trait::async_trait]
impl StorePlatform for RecordingPlatform {
    fn read(&self, path: &Path) -> Result<Option<Vec<u8>>, PortError> {
        self.calls.reads.fetch_add(1, Ordering::Relaxed);
        NativeStorePlatform.read(path)
    }

    fn write_atomic(&self, path: &Path, bytes: &[u8]) -> Result<(), DurableReplaceError> {
        self.calls.atomic_writes.fetch_add(1, Ordering::Relaxed);
        let parent = path.parent().ok_or_else(|| {
            DurableReplaceError::NotCommitted(io::Error::new(
                io::ErrorKind::InvalidInput,
                "document path has no parent",
            ))
        })?;
        fs::create_dir_all(parent).map_err(DurableReplaceError::NotCommitted)?;
        let sequence = self.temporaries.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(".recording-{sequence}.tmp"));
        let staged = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .and_then(|mut file| {
                file.write_all(bytes)?;
                file.sync_all()
            });
        if let Err(error) = staged {
            let _ = fs::remove_file(&temporary);
            return Err(DurableReplaceError::NotCommitted(error));
        }
        match self.replace_atomic(&temporary, path) {
            Ok(()) => Ok(()),
            Err(error) => {
                let _ = fs::remove_file(&temporary);
                Err(error)
            }
        }
    }

    fn replace_atomic(&self, source: &Path, target: &Path) -> Result<(), DurableReplaceError> {
        self.calls
            .atomic_replacements
            .fetch_add(1, Ordering::Relaxed);
        if self.fail_replacement.load(Ordering::Relaxed) {
            return Err(DurableReplaceError::NotCommitted(io::Error::other(
                "injected replacement failure",
            )));
        }
        NativeStorePlatform.replace_atomic(source, target)
    }

    fn remove_file(&self, path: &Path) -> Result<(), PortError> {
        NativeStorePlatform.remove_file(path)
    }

    fn remove_file_durable(&self, path: &Path) -> Result<(), DurableReplaceError> {
        NativeStorePlatform.remove_file_durable(path)
    }

    fn rename(&self, source: &Path, target: &Path) -> Result<(), PortError> {
        NativeStorePlatform.rename(source, target)
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), PortError> {
        NativeStorePlatform.create_dir_all(path)
    }

    fn remove_dir(&self, path: &Path) -> Result<(), PortError> {
        NativeStorePlatform.remove_dir(path)
    }

    fn exists(&self, path: &Path) -> Result<bool, PortError> {
        NativeStorePlatform.exists(path)
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf, PortError> {
        NativeStorePlatform.canonicalize(path)
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<StoreDirEntry>, PortError> {
        NativeStorePlatform.read_dir(path)
    }

    async fn lock_profile_writes(
        &self,
        layout: &ProfileStoreLayout,
        config_dir: &Path,
    ) -> Result<Box<dyn ProfileWriteFence>, PortError> {
        self.calls.profile_fences.fetch_add(1, Ordering::Relaxed);
        NativeStorePlatform
            .lock_profile_writes(layout, config_dir)
            .await
    }

    fn try_acquire_instance_lock(
        &self,
        path: &Path,
    ) -> Result<Option<Box<dyn InstanceLockFence>>, PortError> {
        self.calls.instance_locks.fetch_add(1, Ordering::Relaxed);
        NativeStorePlatform.try_acquire_instance_lock(path)
    }
}

fn fixture(platform: &Arc<RecordingPlatform>) -> TestCore {
    let injected: Arc<dyn StorePlatform> = Arc::clone(platform) as Arc<dyn StorePlatform>;
    TestCore::with_platform(
        ProfileDocumentExtension::Json,
        ProfileStoreLayout::default(),
        injected,
    )
}

#[tokio::test]
async fn document_writes_and_profile_fences_use_the_injected_platform() {
    let platform = RecordingPlatform::shared();
    let fixture = fixture(&platform);
    let profile = ProfileId::new("primary").expect("valid profile id");

    let settings: toml::Value = toml::from_str("schema_version = 1\n").expect("settings");
    fixture
        .core()
        .write_settings_value(&settings)
        .await
        .expect("settings write through the injected platform");
    fixture
        .core()
        .write_profile_document_locked(&profile, br#"{"revision":1}"#)
        .await
        .expect("document write through the injected platform");

    assert!(
        platform.calls.atomic_writes.load(Ordering::Relaxed) >= 2,
        "settings and profile documents must both pass through write_atomic"
    );
    assert_eq!(
        platform.calls.profile_fences.load(Ordering::Relaxed),
        0,
        "no fence has been requested yet"
    );

    let guard = fixture
        .core()
        .lock_profile_writes()
        .await
        .expect("profile fence through the injected platform");
    assert_eq!(
        platform.calls.profile_fences.load(Ordering::Relaxed),
        1,
        "lock_profile_writes must be dispatched to the injected platform"
    );
    drop(guard);

    let document = fixture
        .core()
        .read_profile_document(&profile)
        .await
        .expect("document read through the injected platform");
    assert_eq!(document, r#"{"revision":1}"#);
    assert!(
        platform.calls.reads.load(Ordering::Relaxed) >= 1,
        "document reads must use the injected platform"
    );
    let settings_file = fixture.core().settings_file().to_path_buf();
    let stored = fs::read_to_string(&settings_file).expect("settings file exists");
    assert!(stored.contains("schema_version"));
}

#[tokio::test]
async fn replacement_failure_preserves_the_previous_document_and_temp_free_directory() {
    let platform = Arc::new(RecordingPlatform::failing_replacement());
    let fixture = fixture(&platform);
    let settings_file = fixture.core().settings_file().to_path_buf();
    write_document(&settings_file, "schema_version = 1\n")
        .await
        .expect("seed previous settings with the native writer");

    let next: toml::Value = toml::from_str("schema_version = 2\n").expect("settings");
    let error = fixture
        .core()
        .write_settings_value(&next)
        .await
        .expect_err("the injected platform must reject the replacement");
    assert!(
        matches!(&error, PortError::Io(message) if message.contains("injected replacement failure")),
        "unexpected error: {error:?}"
    );
    assert_eq!(
        platform.calls.atomic_replacements.load(Ordering::Relaxed),
        1,
        "the injected replace_atomic must be the failing step"
    );

    let previous = fs::read_to_string(&settings_file).expect("previous document survives");
    assert_eq!(previous, "schema_version = 1\n");
    let entries: Vec<_> = fs::read_dir(
        settings_file
            .parent()
            .expect("settings file has a parent directory"),
    )
    .expect("read config dir")
    .map(|entry| entry.expect("entry").file_name())
    .collect();
    assert_eq!(
        entries,
        vec![
            settings_file
                .file_name()
                .expect("settings file name")
                .to_os_string()
        ],
        "a rejected replacement must not leave staged temporaries behind"
    );
}

#[cfg(unix)]
#[test]
fn native_instance_lock_file_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("instance.lock");
    let _guard = try_acquire_instance_lock(&path)
        .expect("native instance lock")
        .expect("first acquisition");
    let mode = fs::metadata(&path)
        .expect("lock file metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "lock file must be owner-only");
}

#[test]
fn directory_entries_keep_their_typed_kind() {
    let directory = tempfile::tempdir().expect("temporary directory");
    fs::create_dir(directory.path().join("nested")).expect("nested directory");
    fs::write(directory.path().join("document.json"), b"{}").expect("document");
    let entries = NativeStorePlatform
        .read_dir(directory.path())
        .expect("platform read_dir");
    assert!(entries.iter().any(|entry| {
        entry.path.ends_with("nested") && entry.kind == StoreEntryKind::Directory
    }));
    assert!(entries.iter().any(|entry| {
        entry.path.ends_with("document.json") && entry.kind == StoreEntryKind::File
    }));
}

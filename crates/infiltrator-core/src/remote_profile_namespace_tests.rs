use super::*;
use async_trait::async_trait;
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FailurePoint {
    Copy,
    Readback,
    Delete,
}

#[derive(Clone, Default)]
struct FakeWebDav(Arc<Mutex<FakeState>>);

#[derive(Default)]
struct FakeState {
    files: HashMap<String, FakeFile>,
    directories: HashSet<String>,
    next_etag: u64,
    copied_paths: HashSet<String>,
    failure: Option<FailurePoint>,
}

#[derive(Clone)]
struct FakeFile {
    bytes: Vec<u8>,
    etag: String,
}

impl FakeWebDav {
    fn file(&self, path: &str, bytes: &[u8]) {
        let path = normalized_path(path);
        let mut state = self.0.lock().expect("fake WebDAV state");
        let mut parent = path.rsplit_once('/').map(|(parent, _)| parent.to_owned());
        while let Some(directory) = parent {
            if directory.is_empty() || !state.directories.insert(directory.clone()) {
                break;
            }
            parent = directory
                .rsplit_once('/')
                .map(|(parent, _)| parent.to_owned());
        }
        state.next_etag += 1;
        let etag = format!("etag-{}", state.next_etag);
        state.files.insert(
            path,
            FakeFile {
                bytes: bytes.to_vec(),
                etag,
            },
        );
    }

    fn fail_once(&self, failure: FailurePoint) {
        self.0.lock().expect("fake WebDAV state").failure = Some(failure);
    }

    fn clear_etag(&self, path: &str) {
        if let Some(file) = self
            .0
            .lock()
            .expect("fake WebDAV state")
            .files
            .get_mut(&normalized_path(path))
        {
            file.etag.clear();
        }
    }

    fn bytes(&self, path: &str) -> Option<Vec<u8>> {
        self.0
            .lock()
            .expect("fake WebDAV state")
            .files
            .get(&normalized_path(path))
            .map(|file| file.bytes.clone())
    }

    fn has_file(&self, path: &str) -> bool {
        self.bytes(path).is_some()
    }

    fn marker_state(&self, engine: &EngineId) -> Option<RemoteNamespaceState> {
        if let Some(bytes) = self.bytes(&marker_path(engine, RemoteNamespaceState::EngineScopedV1)?)
        {
            if RemoteNamespaceState::parse_marker(
                &bytes,
                engine,
                RemoteNamespaceState::EngineScopedV1,
            )
            .is_ok()
            {
                return Some(RemoteNamespaceState::EngineScopedV1);
            }
        }
        let bytes = self.bytes(&marker_path(engine, RemoteNamespaceState::CopyVerifiedV1)?)?;
        RemoteNamespaceState::parse_marker(&bytes, engine, RemoteNamespaceState::CopyVerifiedV1)
            .ok()
            .map(|()| RemoteNamespaceState::CopyVerifiedV1)
    }

    fn trip(state: &mut FakeState, point: FailurePoint) -> Result<(), anyhow::Error> {
        if state.failure == Some(point) {
            state.failure = None;
            return Err(anyhow::anyhow!("injected {point:?} failure"));
        }
        Ok(())
    }

    fn entry(path: String, etag: String, is_dir: bool, size: u64) -> RemoteEntry {
        RemoteEntry {
            path,
            etag,
            last_modified: Utc::now(),
            is_dir,
            size,
        }
    }
}

#[async_trait]
impl DavClient for FakeWebDav {
    async fn list(&self, path: &str) -> Result<Vec<RemoteEntry>, anyhow::Error> {
        let path = normalized_path(path);
        let state = self.0.lock().expect("fake WebDAV state");
        if !path.is_empty() && !state.directories.contains(&path) {
            return Err(anyhow::anyhow!("missing directory {path}"));
        }
        let mut entries = vec![Self::entry(
            if path.is_empty() {
                "/".into()
            } else {
                format!("/{path}/")
            },
            String::new(),
            true,
            0,
        )];
        for directory in &state.directories {
            if directory
                .rsplit_once('/')
                .map(|(parent, _)| parent)
                .unwrap_or("")
                == path
                && directory != &path
            {
                entries.push(Self::entry(
                    format!("/{directory}/"),
                    String::new(),
                    true,
                    0,
                ));
            }
        }
        for (file_path, file) in &state.files {
            if file_path
                .rsplit_once('/')
                .map(|(parent, _)| parent)
                .unwrap_or("")
                == path
            {
                entries.push(Self::entry(
                    format!("/{file_path}"),
                    file.etag.clone(),
                    false,
                    file.bytes.len() as u64,
                ));
            }
        }
        Ok(entries)
    }

    async fn get(&self, path: &str) -> Result<Vec<u8>, anyhow::Error> {
        let path = normalized_path(path);
        let mut state = self.0.lock().expect("fake WebDAV state");
        if state.copied_paths.contains(&path) {
            Self::trip(&mut state, FailurePoint::Readback)?;
        }
        state
            .files
            .get(&path)
            .map(|file| file.bytes.clone())
            .ok_or_else(|| anyhow::anyhow!("missing file {path}"))
    }

    async fn put(
        &self,
        path: &str,
        data: &[u8],
        if_match: Option<&str>,
    ) -> Result<String, anyhow::Error> {
        let path = normalized_path(path);
        let mut state = self.0.lock().expect("fake WebDAV state");
        if let Some(expected) = if_match {
            let current = state
                .files
                .get(&path)
                .ok_or_else(|| anyhow::anyhow!("missing file {path}"))?;
            if current.etag != expected {
                return Err(anyhow::anyhow!("ETag changed for {path}"));
            }
        }
        state.next_etag += 1;
        let etag = format!("etag-{}", state.next_etag);
        state.files.insert(
            path,
            FakeFile {
                bytes: data.to_vec(),
                etag: etag.clone(),
            },
        );
        Ok(etag)
    }

    async fn put_if_absent(&self, path: &str, data: &[u8]) -> Result<String, anyhow::Error> {
        let path = normalized_path(path);
        let mut state = self.0.lock().expect("fake WebDAV state");
        Self::trip(&mut state, FailurePoint::Copy)?;
        if state.files.contains_key(&path) {
            return Err(anyhow::anyhow!("destination exists: {path}"));
        }
        state.next_etag += 1;
        let etag = format!("etag-{}", state.next_etag);
        state.files.insert(
            path.clone(),
            FakeFile {
                bytes: data.to_vec(),
                etag: etag.clone(),
            },
        );
        state.copied_paths.insert(path);
        Ok(etag)
    }

    async fn delete(&self, path: &str) -> Result<(), anyhow::Error> {
        self.0
            .lock()
            .expect("fake WebDAV state")
            .files
            .remove(&normalized_path(path));
        Ok(())
    }

    async fn delete_if_match(&self, path: &str, etag: &str) -> Result<(), anyhow::Error> {
        let path = normalized_path(path);
        let mut state = self.0.lock().expect("fake WebDAV state");
        Self::trip(&mut state, FailurePoint::Delete)?;
        match state.files.get(&path).map(|file| file.etag == etag) {
            None => Ok(()),
            Some(true) => {
                state.files.remove(&path);
                Ok(())
            }
            Some(false) => Err(anyhow::anyhow!("ETag changed for {path}")),
        }
    }

    async fn move_item(&self, from: &str, to: &str) -> Result<(), anyhow::Error> {
        let mut state = self.0.lock().expect("fake WebDAV state");
        let from = normalized_path(from);
        let to = normalized_path(to);
        let file = state
            .files
            .remove(&from)
            .ok_or_else(|| anyhow::anyhow!("missing file {from}"))?;
        state.files.insert(to, file);
        Ok(())
    }

    async fn mkdir(&self, path: &str) -> Result<(), anyhow::Error> {
        let mut state = self.0.lock().expect("fake WebDAV state");
        state.directories.insert(normalized_path(path));
        Ok(())
    }
}

fn engine(name: &str) -> EngineId {
    EngineId::new(name).expect("valid engine ID")
}

#[tokio::test]
async fn legacy_profiles_are_copy_verified_marked_then_retired_idempotently() {
    let dav = FakeWebDav::default();
    let mihomo = engine("mihomo");
    dav.file("main.yaml", b"# exact\r\nmode: rule\r\n");

    migrate_remote_profile_namespace(&dav, &mihomo)
        .await
        .unwrap();

    assert!(!dav.has_file("main.yaml"));
    assert_eq!(
        dav.bytes("mihomo/main.yaml").as_deref(),
        Some(b"# exact\r\nmode: rule\r\n".as_slice())
    );
    assert_eq!(
        dav.marker_state(&mihomo),
        Some(RemoteNamespaceState::EngineScopedV1)
    );

    migrate_remote_profile_namespace(&dav, &mihomo)
        .await
        .unwrap();
    assert_eq!(
        dav.bytes("mihomo/main.yaml").as_deref(),
        Some(b"# exact\r\nmode: rule\r\n".as_slice())
    );
}

#[tokio::test]
async fn terminal_marker_does_not_hide_a_later_legacy_profile() {
    let dav = FakeWebDav::default();
    let mihomo = engine("mihomo");
    migrate_remote_profile_namespace(&dav, &mihomo)
        .await
        .unwrap();
    dav.file("late.yaml", b"late legacy bytes");

    assert!(
        migrate_remote_profile_namespace(&dav, &mihomo)
            .await
            .is_err()
    );
    assert_eq!(
        dav.bytes("late.yaml").as_deref(),
        Some(b"late legacy bytes".as_slice())
    );
    assert_eq!(
        dav.marker_state(&mihomo),
        Some(RemoteNamespaceState::EngineScopedV1)
    );
}

#[tokio::test]
async fn same_profile_name_in_multiple_namespaces_remains_disjoint() {
    let dav = FakeWebDav::default();
    let mihomo = engine("mihomo");
    let sing_box = engine("sing-box");
    dav.file("main.yaml", b"legacy mihomo document");
    dav.file("sing-box/main.yaml", b"sing-box document");

    migrate_remote_profile_namespace(&dav, &sing_box)
        .await
        .unwrap();
    assert!(
        dav.has_file("main.yaml"),
        "sing-box must not claim legacy Mihomo data"
    );
    migrate_remote_profile_namespace(&dav, &mihomo)
        .await
        .unwrap();

    assert_eq!(dav.bytes("main.yaml"), None);
    assert_eq!(
        dav.bytes("mihomo/main.yaml").as_deref(),
        Some(b"legacy mihomo document".as_slice())
    );
    assert_eq!(
        dav.bytes("sing-box/main.yaml").as_deref(),
        Some(b"sing-box document".as_slice())
    );
    assert_eq!(
        dav.marker_state(&mihomo),
        Some(RemoteNamespaceState::EngineScopedV1)
    );
    assert_eq!(
        dav.marker_state(&sing_box),
        Some(RemoteNamespaceState::EngineScopedV1)
    );
}

#[tokio::test]
async fn destination_conflict_preserves_the_legacy_source() {
    let dav = FakeWebDav::default();
    let mihomo = engine("mihomo");
    dav.file("main.yaml", b"legacy bytes");
    dav.file("mihomo/main.yaml", b"different scoped bytes");

    assert!(
        migrate_remote_profile_namespace(&dav, &mihomo)
            .await
            .is_err()
    );

    assert_eq!(
        dav.bytes("main.yaml").as_deref(),
        Some(b"legacy bytes".as_slice())
    );
    assert_eq!(
        dav.bytes("mihomo/main.yaml").as_deref(),
        Some(b"different scoped bytes".as_slice())
    );
    assert_eq!(dav.marker_state(&mihomo), None);
}

#[tokio::test]
async fn unknown_remote_marker_fails_closed_without_touching_legacy_data() {
    let dav = FakeWebDav::default();
    let mihomo = engine("mihomo");
    dav.file("main.yaml", b"legacy bytes");
    dav.file(
        &marker_path(&mihomo, RemoteNamespaceState::CopyVerifiedV1).unwrap(),
        b"unknown namespace marker",
    );

    assert!(
        migrate_remote_profile_namespace(&dav, &mihomo)
            .await
            .is_err()
    );

    assert_eq!(
        dav.bytes("main.yaml").as_deref(),
        Some(b"legacy bytes".as_slice())
    );
    assert_eq!(dav.bytes("mihomo/main.yaml"), None);
}

#[tokio::test]
async fn missing_strong_etag_retains_both_legacy_and_verified_copies() {
    let dav = FakeWebDav::default();
    let mihomo = engine("mihomo");
    dav.file("main.yaml", b"legacy bytes");
    dav.clear_etag("main.yaml");

    assert!(
        migrate_remote_profile_namespace(&dav, &mihomo)
            .await
            .is_err()
    );

    assert_eq!(
        dav.bytes("main.yaml").as_deref(),
        Some(b"legacy bytes".as_slice())
    );
    assert_eq!(
        dav.bytes("mihomo/main.yaml").as_deref(),
        Some(b"legacy bytes".as_slice())
    );
    assert_eq!(
        dav.marker_state(&mihomo),
        Some(RemoteNamespaceState::CopyVerifiedV1)
    );
}

async fn failure_then_retry(failure: FailurePoint) {
    let dav = FakeWebDav::default();
    let mihomo = engine("mihomo");
    dav.file("main.yaml", b"# retain\r\nopaque\x00bytes");
    dav.fail_once(failure);

    assert!(
        migrate_remote_profile_namespace(&dav, &mihomo)
            .await
            .is_err()
    );
    assert!(dav.has_file("main.yaml"), "source must survive {failure:?}");
    if failure != FailurePoint::Copy {
        assert_eq!(
            dav.bytes("mihomo/main.yaml").as_deref(),
            Some(b"# retain\r\nopaque\x00bytes".as_slice()),
            "copied target remains recoverable after {failure:?}"
        );
    }
    if failure == FailurePoint::Delete {
        assert_eq!(
            dav.marker_state(&mihomo),
            Some(RemoteNamespaceState::CopyVerifiedV1)
        );
    } else {
        assert_eq!(dav.marker_state(&mihomo), None);
    }

    migrate_remote_profile_namespace(&dav, &mihomo)
        .await
        .unwrap();
    assert!(!dav.has_file("main.yaml"));
    assert_eq!(
        dav.bytes("mihomo/main.yaml").as_deref(),
        Some(b"# retain\r\nopaque\x00bytes".as_slice())
    );
    assert_eq!(
        dav.marker_state(&mihomo),
        Some(RemoteNamespaceState::EngineScopedV1)
    );
}

#[tokio::test]
async fn copy_failure_retains_source_and_retry_completes() {
    failure_then_retry(FailurePoint::Copy).await;
}

#[tokio::test]
async fn readback_failure_retains_source_and_retry_completes() {
    failure_then_retry(FailurePoint::Readback).await;
}

#[tokio::test]
async fn delete_failure_retains_source_and_retry_completes() {
    failure_then_retry(FailurePoint::Delete).await;
}

//! Explicit migration of the former unscoped WebDAV profile directory.
//!
//! A root-level profile is copied to the Mihomo namespace only after a
//! conditional create and exact-byte readback. A typed `CopyVerifiedV1`
//! marker is published before any legacy source is conditionally deleted;
//! failures leave enough source or destination data to retry safely.

use dav_client::{DavClient, RemoteEntry};
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_domain::profiles::validate_profile_name;
use infiltrator_ports::error::PortError;
use std::collections::BTreeMap;

const MARKER_PREFIX: &str = "musicfrog-profile-namespace-v1-";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RemoteNamespaceState {
    LegacyShared,
    CopyVerifiedV1,
    EngineScopedV1,
}

impl RemoteNamespaceState {
    fn marker_value(self, engine: &EngineId) -> Vec<u8> {
        let state = match self {
            Self::LegacyShared => "legacy_shared",
            Self::CopyVerifiedV1 => "copy_verified_v1",
            Self::EngineScopedV1 => "engine_scoped_v1",
        };
        format!(
            "musicfrog-profile-namespace-v1\nengine={}\nstate={state}\n",
            engine.as_str()
        )
        .into_bytes()
    }

    fn parse_marker(bytes: &[u8], engine: &EngineId, expected: Self) -> Result<(), PortError> {
        if bytes == expected.marker_value(engine).as_slice() {
            return Ok(());
        }
        Err(PortError::Failed(format!(
            "remote profile namespace marker for `{}` is not `{expected:?}`",
            engine.as_str(),
        )))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LegacyRemoteProfile {
    profile: String,
    source_path: String,
    source_etag: String,
}

#[derive(Clone, Debug)]
struct MarkerObservation {
    state: RemoteNamespaceState,
}

/// Migrate legacy root-level WebDAV profiles into one engine namespace.
///
/// The historical root directory belonged to Mihomo. Other engines receive
/// their own terminal marker without inspecting or claiming those files.
pub(crate) async fn migrate_remote_profile_namespace(
    dav: &dyn DavClient,
    engine: &EngineId,
) -> Result<(), PortError> {
    dav.mkdir(engine.as_str())
        .await
        .map_err(|error| network_error("create engine namespace", error))?;

    let marker = read_marker(dav, engine).await?;
    if marker.state == RemoteNamespaceState::EngineScopedV1 {
        if !collect_legacy_profiles(dav, engine).await?.is_empty() {
            return Err(PortError::Failed(format!(
                "terminal namespace marker for `{}` exists while legacy WebDAV profiles remain; refusing to ignore them",
                engine.as_str()
            )));
        }
        return Ok(());
    }

    let mut legacy_profiles = collect_legacy_profiles(dav, engine).await?;
    copy_and_verify_all(dav, engine, &legacy_profiles).await?;

    let marker = match marker.state {
        RemoteNamespaceState::LegacyShared => ensure_copy_verified_marker(dav, engine).await?,
        RemoteNamespaceState::CopyVerifiedV1 => marker,
        RemoteNamespaceState::EngineScopedV1 => return Ok(()),
    };
    if marker.state == RemoteNamespaceState::EngineScopedV1 {
        return Ok(());
    }
    // Refresh the source inventory and verify every current source again
    // before beginning any delete. This also makes retries safe if an older
    // client wrote another root-level profile while the copy phase ran.
    legacy_profiles = collect_legacy_profiles(dav, engine).await?;
    copy_and_verify_all(dav, engine, &legacy_profiles).await?;
    delete_verified_sources(dav, engine, &legacy_profiles).await?;

    if !collect_legacy_profiles(dav, engine).await?.is_empty() {
        return Err(PortError::Failed(format!(
            "legacy WebDAV profiles appeared during `{}` namespace cleanup; retry migration",
            engine.as_str()
        )));
    }

    publish_engine_scoped_marker(dav, engine).await
}

fn marker_path(engine: &EngineId, state: RemoteNamespaceState) -> Option<String> {
    match state {
        RemoteNamespaceState::LegacyShared => None,
        RemoteNamespaceState::CopyVerifiedV1 => Some(format!(
            "{MARKER_PREFIX}{}-copy-verified.marker",
            engine.as_str()
        )),
        RemoteNamespaceState::EngineScopedV1 => Some(format!(
            "{MARKER_PREFIX}{}-engine-scoped.marker",
            engine.as_str()
        )),
    }
}

async fn read_marker(
    dav: &dyn DavClient,
    engine: &EngineId,
) -> Result<MarkerObservation, PortError> {
    let entries = dav
        .list("")
        .await
        .map_err(|error| network_error("read namespace marker listing", error))?;
    let copy_path = marker_path(engine, RemoteNamespaceState::CopyVerifiedV1)
        .expect("copy-verified marker path exists");
    let final_path = marker_path(engine, RemoteNamespaceState::EngineScopedV1)
        .expect("terminal marker path exists");
    let copy_entry = find_entry(&entries, &copy_path)?;
    let final_entry = find_entry(&entries, &final_path)?;
    if let Some(entry) = copy_entry {
        validate_marker_entry(
            dav,
            engine,
            &copy_path,
            entry,
            RemoteNamespaceState::CopyVerifiedV1,
        )
        .await?;
    }
    if let Some(entry) = final_entry {
        validate_marker_entry(
            dav,
            engine,
            &final_path,
            entry,
            RemoteNamespaceState::EngineScopedV1,
        )
        .await?;
    }
    let state = match (copy_entry.is_some(), final_entry.is_some()) {
        (_, true) => RemoteNamespaceState::EngineScopedV1,
        (true, false) => RemoteNamespaceState::CopyVerifiedV1,
        (false, false) => RemoteNamespaceState::LegacyShared,
    };
    Ok(MarkerObservation { state })
}

async fn validate_marker_entry(
    dav: &dyn DavClient,
    engine: &EngineId,
    path: &str,
    entry: &RemoteEntry,
    state: RemoteNamespaceState,
) -> Result<(), PortError> {
    if entry.is_dir {
        return Err(PortError::Failed(format!(
            "remote namespace marker `{path}` is a directory"
        )));
    }
    let bytes = dav
        .get(path)
        .await
        .map_err(|error| network_error("read namespace marker", error))?;
    RemoteNamespaceState::parse_marker(&bytes, engine, state)
}

async fn collect_legacy_profiles(
    dav: &dyn DavClient,
    engine: &EngineId,
) -> Result<Vec<LegacyRemoteProfile>, PortError> {
    // The old unscoped collection was the Mihomo product's WebDAV format.
    // A future engine must never claim or delete it.
    if engine != &mihomo_config::engine_id() {
        return Ok(Vec::new());
    }

    let entries = dav
        .list("")
        .await
        .map_err(|error| network_error("list legacy profiles", error))?;
    let mut by_profile = BTreeMap::new();
    for entry in entries {
        if entry.is_dir {
            continue;
        }
        let path = normalized_path(&entry.path);
        if path.contains('/') {
            continue;
        }
        let Some(profile) = legacy_profile_name(&path)? else {
            continue;
        };
        let legacy = LegacyRemoteProfile {
            profile: profile.clone(),
            source_path: path,
            source_etag: entry.etag,
        };
        if by_profile.insert(profile.clone(), legacy).is_some() {
            return Err(PortError::Failed(format!(
                "legacy WebDAV contains multiple files for profile `{profile}`"
            )));
        }
    }
    Ok(by_profile.into_values().collect())
}

fn legacy_profile_name(path: &str) -> Result<Option<String>, PortError> {
    let Some((stem, extension)) = path.rsplit_once('.') else {
        return Ok(None);
    };
    if !matches!(extension, "yaml" | "yml") {
        return Ok(None);
    }
    validate_profile_name(stem).map_err(|error| {
        PortError::Failed(format!("unsafe legacy profile path `{path}`: {error}"))
    })?;
    ProfileId::new(stem.to_owned())
        .map(|profile| Some(profile.as_str().to_owned()))
        .map_err(|error| PortError::Failed(format!("unsafe legacy profile path `{path}`: {error}")))
}

fn normalized_path(path: &str) -> String {
    path.trim_matches('/').to_owned()
}

async fn copy_and_verify_all(
    dav: &dyn DavClient,
    engine: &EngineId,
    profiles: &[LegacyRemoteProfile],
) -> Result<(), PortError> {
    for profile in profiles {
        let bytes = dav
            .get(&profile.source_path)
            .await
            .map_err(|error| network_error("read legacy profile", error))?;
        ensure_target_exact(dav, engine, profile, &bytes).await?;
    }
    Ok(())
}

async fn ensure_target_exact(
    dav: &dyn DavClient,
    engine: &EngineId,
    profile: &LegacyRemoteProfile,
    source: &[u8],
) -> Result<(), PortError> {
    let destination = remote_profile_path(engine, &profile.profile);
    let entries = dav
        .list(engine.as_str())
        .await
        .map_err(|error| network_error("list engine-scoped profiles", error))?;
    if let Some(entry) = find_entry(&entries, &destination)? {
        if entry.is_dir {
            return Err(PortError::Failed(format!(
                "engine-scoped profile destination `{destination}` is a directory"
            )));
        }
        let current = dav
            .get(&destination)
            .await
            .map_err(|error| network_error("read engine-scoped profile", error))?;
        return exact_bytes(&destination, source, &current);
    }

    match dav.put_if_absent(&destination, source).await {
        Ok(_) => {}
        Err(create_error) => {
            // A concurrent migrator may have created the exact destination;
            // only that case is accepted. A missing or different copy remains
            // a hard failure and the legacy source is never removed.
            let refreshed = dav
                .list(engine.as_str())
                .await
                .map_err(|error| network_error("verify concurrent engine-scoped create", error))?;
            let Some(entry) = find_entry(&refreshed, &destination)? else {
                return Err(network_error("create engine-scoped profile", create_error));
            };
            if entry.is_dir {
                return Err(PortError::Failed(format!(
                    "engine-scoped profile destination `{destination}` is a directory"
                )));
            }
            let current = dav
                .get(&destination)
                .await
                .map_err(|error| network_error("read concurrent engine-scoped copy", error))?;
            exact_bytes(&destination, source, &current)?;
        }
    }

    let readback = dav
        .get(&destination)
        .await
        .map_err(|error| network_error("read back engine-scoped profile", error))?;
    exact_bytes(&destination, source, &readback)
}

fn exact_bytes(path: &str, expected: &[u8], actual: &[u8]) -> Result<(), PortError> {
    if expected == actual {
        return Ok(());
    }
    Err(PortError::Failed(format!(
        "remote profile namespace conflict at `{path}`; source bytes were retained"
    )))
}

async fn ensure_copy_verified_marker(
    dav: &dyn DavClient,
    engine: &EngineId,
) -> Result<MarkerObservation, PortError> {
    let path = marker_path(engine, RemoteNamespaceState::CopyVerifiedV1)
        .expect("copy-verified marker path exists");
    match dav
        .put_if_absent(
            &path,
            &RemoteNamespaceState::CopyVerifiedV1.marker_value(engine),
        )
        .await
    {
        Ok(_) => {}
        Err(create_error) => {
            let existing = read_marker(dav, engine).await?;
            match existing.state {
                RemoteNamespaceState::CopyVerifiedV1 | RemoteNamespaceState::EngineScopedV1 => {
                    return Ok(existing);
                }
                RemoteNamespaceState::LegacyShared => {
                    return Err(network_error("publish copy-verified marker", create_error));
                }
            }
        }
    }
    let marker = read_marker(dav, engine).await?;
    match marker.state {
        RemoteNamespaceState::CopyVerifiedV1 | RemoteNamespaceState::EngineScopedV1 => Ok(marker),
        RemoteNamespaceState::LegacyShared => Err(PortError::Failed(format!(
            "copy-verified marker `{path}` was not readable after publication"
        ))),
    }
}

async fn delete_verified_sources(
    dav: &dyn DavClient,
    engine: &EngineId,
    profiles: &[LegacyRemoteProfile],
) -> Result<(), PortError> {
    for profile in profiles {
        require_strong_etag(
            &profile.source_etag,
            &format!("legacy profile `{}`", profile.source_path),
        )?;
    }
    for profile in profiles {
        let source = dav
            .get(&profile.source_path)
            .await
            .map_err(|error| network_error("recheck legacy profile before deletion", error))?;
        let destination = remote_profile_path(engine, &profile.profile);
        let target = dav
            .get(&destination)
            .await
            .map_err(|error| network_error("verify profile before source deletion", error))?;
        exact_bytes(&destination, &source, &target)?;
        dav.delete_if_match(&profile.source_path, &profile.source_etag)
            .await
            .map_err(|error| network_error("delete verified legacy profile", error))?;
    }
    Ok(())
}

async fn publish_engine_scoped_marker(
    dav: &dyn DavClient,
    engine: &EngineId,
) -> Result<(), PortError> {
    let path = marker_path(engine, RemoteNamespaceState::EngineScopedV1)
        .expect("terminal marker path exists");
    match dav
        .put_if_absent(
            &path,
            &RemoteNamespaceState::EngineScopedV1.marker_value(engine),
        )
        .await
    {
        Ok(_) => {}
        Err(update_error) => {
            let marker = read_marker(dav, engine).await?;
            if marker.state == RemoteNamespaceState::EngineScopedV1 {
                return Ok(());
            }
            return Err(network_error(
                "publish terminal namespace marker",
                update_error,
            ));
        }
    }
    let marker = read_marker(dav, engine).await?;
    if marker.state == RemoteNamespaceState::EngineScopedV1 {
        Ok(())
    } else {
        Err(PortError::Failed(format!(
            "terminal namespace marker `{path}` did not read back as engine scoped"
        )))
    }
}

fn require_strong_etag(etag: &str, resource: &str) -> Result<(), PortError> {
    if etag.trim().is_empty() || etag.starts_with("W/") || etag.chars().any(char::is_control) {
        return Err(PortError::Failed(format!(
            "{resource} has no usable strong ETag; legacy files were retained"
        )));
    }
    Ok(())
}

fn find_entry<'a>(
    entries: &'a [RemoteEntry],
    path: &str,
) -> Result<Option<&'a RemoteEntry>, PortError> {
    let path = normalized_path(path);
    let mut matches = entries
        .iter()
        .filter(|entry| normalized_path(&entry.path) == path);
    let first = matches.next();
    if matches.next().is_some() {
        return Err(PortError::Failed(format!(
            "remote WebDAV listing returned duplicate path `{path}`"
        )));
    }
    Ok(first)
}

pub(crate) fn remote_profile_path(engine: &EngineId, profile: &str) -> String {
    format!("{}/{}.yaml", engine.as_str(), profile)
}

fn network_error(operation: &str, error: impl std::fmt::Display) -> PortError {
    PortError::Network(format!("remote profile namespace {operation}: {error}"))
}

#[cfg(test)]
mod tests {
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
            if let Some(bytes) =
                self.bytes(&marker_path(engine, RemoteNamespaceState::EngineScopedV1)?)
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
}

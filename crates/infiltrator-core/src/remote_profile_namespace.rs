//! Explicit migration of the former unscoped WebDAV profile directory.
//!
//! A root-level profile is copied to the Mihomo namespace only after a
//! conditional create and exact-byte readback. A typed `CopyVerifiedV1`
//! marker is published before any legacy source is conditionally deleted;
//! failures leave enough source or destination data to retry safely.

use dav_client::{DavClient, RemoteEntry};
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::profile_document_identity::ProfileId;
use lurker_core_contract::profile_name::validate_profile_name;
use lurker_core_ports::error::PortError;
use std::collections::BTreeMap;
use std::fmt::Display;

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

fn network_error(operation: &str, error: impl Display) -> PortError {
    PortError::Network(format!("remote profile namespace {operation}: {error}"))
}

#[cfg(test)]
#[path = "remote_profile_namespace_tests.rs"]
mod tests;

//! Engine-neutral profile namespace schema, state recovery and state merging.
//!
//! One engine namespace owns a single profile-state TOML document. This module
//! owns its schema marker, its durable backup recovery and the pure merge rules
//! used when a product migrates its legacy shared state. Product-specific
//! credential migration and legacy file discovery entry points are composed by
//! the product crate on top of these primitives.
//!
//! All filesystem access is delegated to the store's [`StorePlatform`]; the
//! free functions are the native default.

use crate::document_store::{
    read_optional_bytes_with_platform, write_document_bytes_durable_with_platform,
};
use crate::layout::ProfileStoreLayout;
use crate::platform::{StoreEntryKind, StorePlatform, native_platform, run_platform_io};
use crate::store::ProfileDocumentExtension;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_ports::error::PortError;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use toml::map::Map;

pub const PROFILE_NAMESPACE_SCHEMA_KEY: &str = "profile_namespace_schema";
const ENGINE_SCOPED_SCHEMA_V1: &str = "engine_scoped_v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileNamespaceSchema {
    LegacyShared,
    EngineScopedV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileNamespaceInitialization {
    EngineScoped,
    LegacyV1(LegacyProfileNamespaceMigrationV1),
}

/// Product-supplied description of the legacy shared namespace to migrate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyProfileNamespaceMigrationV1 {
    settings_file: PathBuf,
    sync_index_file_name: Option<String>,
    directories: Vec<String>,
}

impl LegacyProfileNamespaceMigrationV1 {
    pub fn new(
        settings_file: impl Into<PathBuf>,
        sync_index_file_name: Option<String>,
        directories: Vec<String>,
    ) -> Self {
        Self {
            settings_file: settings_file.into(),
            sync_index_file_name,
            directories,
        }
    }

    pub fn settings_file(&self) -> &Path {
        &self.settings_file
    }

    pub fn sync_index_file_name(&self) -> Option<&str> {
        self.sync_index_file_name.as_deref()
    }

    pub fn directories(&self) -> &[String] {
        &self.directories
    }

    pub fn validate(&self) -> Result<(), PortError> {
        if self
            .sync_index_file_name
            .as_deref()
            .is_some_and(|name| !is_single_relative_component(name))
            || self
                .directories
                .iter()
                .any(|directory| !is_single_relative_component(directory))
        {
            return Err(namespace_configuration_error(
                "legacy namespace migration paths must be single relative names",
            ));
        }
        Ok(())
    }
}

impl ProfileNamespaceSchema {
    pub fn parse(value: Option<&toml::Value>) -> Result<Self, PortError> {
        match value {
            None => Ok(Self::LegacyShared),
            Some(toml::Value::String(value)) if value == ENGINE_SCOPED_SCHEMA_V1 => {
                Ok(Self::EngineScopedV1)
            }
            Some(toml::Value::String(other)) => Err(namespace_configuration_error(format!(
                "unsupported profile namespace schema `{other}`"
            ))),
            Some(_) => Err(namespace_configuration_error(
                "profile namespace schema marker must be a string",
            )),
        }
    }

    pub fn write_engine_scoped_v1(state: &mut toml::Value) -> Result<(), PortError> {
        let table = state
            .as_table_mut()
            .ok_or_else(|| namespace_configuration_error("profile state must be a TOML table"))?;
        table.insert(
            PROFILE_NAMESPACE_SCHEMA_KEY.into(),
            toml::Value::String(ENGINE_SCOPED_SCHEMA_V1.into()),
        );
        Ok(())
    }
}

/// Initialize only the generic engine-scoped schema. The caller must hold the
/// product reset fence and the profile write lock.
pub async fn initialize_engine_scoped_namespace_unlocked(
    layout: &ProfileStoreLayout,
    config_dir: &Path,
    settings_file: &Path,
) -> Result<(), PortError> {
    let platform = native_platform();
    initialize_engine_scoped_namespace_unlocked_with_platform(
        &platform,
        layout,
        config_dir,
        settings_file,
    )
    .await
}

/// [`initialize_engine_scoped_namespace_unlocked`] through an injected platform.
pub async fn initialize_engine_scoped_namespace_unlocked_with_platform(
    platform: &Arc<dyn StorePlatform>,
    layout: &ProfileStoreLayout,
    config_dir: &Path,
    settings_file: &Path,
) -> Result<(), PortError> {
    let config_dir_owned = config_dir.to_path_buf();
    run_platform_io(platform, move |platform| {
        platform.create_dir_all(&config_dir_owned)
    })
    .await?;
    let mut state = read_profile_state_with_platform(platform, layout, settings_file).await?;
    if matches!(
        ProfileNamespaceSchema::parse(state.get(PROFILE_NAMESPACE_SCHEMA_KEY))?,
        ProfileNamespaceSchema::EngineScopedV1
    ) {
        return Ok(());
    }
    ProfileNamespaceSchema::write_engine_scoped_v1(&mut state)?;
    write_profile_state_atomically_with_platform(platform, layout, settings_file, &state).await
}

pub async fn read_profile_state(
    layout: &ProfileStoreLayout,
    path: &Path,
) -> Result<toml::Value, PortError> {
    let platform = native_platform();
    read_profile_state_with_platform(&platform, layout, path).await
}

/// [`read_profile_state`] through an injected platform.
pub async fn read_profile_state_with_platform(
    platform: &Arc<dyn StorePlatform>,
    layout: &ProfileStoreLayout,
    path: &Path,
) -> Result<toml::Value, PortError> {
    recover_profile_state_backup_with_platform(platform, layout, path).await?;
    let Some(bytes) = read_optional_bytes_with_platform(platform, path).await? else {
        return Ok(toml::Value::Table(Map::new()));
    };
    let text = String::from_utf8(bytes).map_err(|error| PortError::Io(error.to_string()))?;
    toml::from_str(&text).map_err(|error| namespace_configuration_error(error.to_string()))
}

pub async fn write_profile_state_atomically(
    layout: &ProfileStoreLayout,
    path: &Path,
    state: &toml::Value,
) -> Result<(), PortError> {
    let platform = native_platform();
    write_profile_state_atomically_with_platform(&platform, layout, path, state).await
}

/// [`write_profile_state_atomically`] through an injected platform.
pub async fn write_profile_state_atomically_with_platform(
    platform: &Arc<dyn StorePlatform>,
    layout: &ProfileStoreLayout,
    path: &Path,
    state: &toml::Value,
) -> Result<(), PortError> {
    recover_profile_state_backup_with_platform(platform, layout, path).await?;
    let content = toml::to_string_pretty(state)
        .map_err(|error| namespace_configuration_error(error.to_string()))?;
    write_document_bytes_durable_with_platform(platform, path, content.as_bytes()).await
}

/// A crash between the state write and its backup cleanup is repaired here:
/// a surviving backup with no live document becomes the document.
pub async fn recover_profile_state_backup(
    layout: &ProfileStoreLayout,
    path: &Path,
) -> Result<(), PortError> {
    let platform = native_platform();
    recover_profile_state_backup_with_platform(&platform, layout, path).await
}

/// [`recover_profile_state_backup`] through an injected platform.
pub async fn recover_profile_state_backup_with_platform(
    platform: &Arc<dyn StorePlatform>,
    layout: &ProfileStoreLayout,
    path: &Path,
) -> Result<(), PortError> {
    let backup = layout.profile_state_backup_path(path);
    let live_path = path.to_path_buf();
    let backup_path = backup.clone();
    let (live, back_up) = run_platform_io(platform, move |platform| {
        Ok((platform.exists(&live_path)?, platform.exists(&backup_path)?))
    })
    .await?;
    match (live, back_up) {
        (false, true) => {
            let live_path = path.to_path_buf();
            run_platform_io(platform, move |platform| {
                platform.rename(&backup, &live_path)
            })
            .await
        }
        (true, true) => {
            run_platform_io(platform, move |platform| platform.remove_file(&backup)).await
        }
        (_, false) => Ok(()),
    }
}

/// Merge one legacy shared state document into an engine-scoped state table.
///
/// Conflicting `[default]` or `[profiles]` entries are rejected instead of
/// silently picking a winner.
pub fn merge_profile_state(
    scoped: &mut toml::Value,
    legacy: &mut toml::Value,
) -> Result<(), PortError> {
    let scoped = scoped
        .as_table_mut()
        .ok_or_else(|| namespace_configuration_error("profile state must be a TOML table"))?;
    let legacy = legacy.as_table_mut().ok_or_else(|| {
        namespace_configuration_error("legacy profile state must be a TOML table")
    })?;

    if let Some(default) = legacy.get_mut("default") {
        let legacy_default = default.as_table_mut().ok_or_else(|| {
            namespace_configuration_error("legacy `[default]` profile state must be a table")
        })?;
        if let Some(profile) = legacy_default.remove("profile") {
            if profile.as_str().is_none() {
                return Err(namespace_configuration_error(
                    "legacy active profile pointer must be a string",
                ));
            }
            merge_value(scoped, "default", "profile", profile)?;
        }
        if legacy_default.is_empty() {
            legacy.remove("default");
        }
    }

    if let Some(legacy_profiles) = legacy.remove("profiles") {
        let toml::Value::Table(legacy_profiles) = legacy_profiles else {
            return Err(namespace_configuration_error(
                "legacy `[profiles]` profile state must be a table",
            ));
        };
        let scoped_profiles = scoped
            .entry("profiles")
            .or_insert_with(|| toml::Value::Table(Map::new()))
            .as_table_mut()
            .ok_or_else(|| {
                namespace_configuration_error("profile metadata must be a TOML table")
            })?;
        for (profile, metadata) in legacy_profiles {
            match scoped_profiles.get(&profile) {
                Some(existing) if existing == &metadata => {}
                Some(_) => {
                    return Err(namespace_configuration_error(format!(
                        "profile namespace migration found conflicting metadata for `{profile}`"
                    )));
                }
                None => {
                    scoped_profiles.insert(profile, metadata);
                }
            }
        }
    }
    Ok(())
}

/// Move every legacy document and legacy directory into the engine namespace.
///
/// Existing destinations must be byte-identical, otherwise the migration fails
/// without deleting either copy.
pub async fn migrate_legacy_profile_files(
    config_root: &Path,
    config_dir: &Path,
    document_extension: ProfileDocumentExtension,
    migration: &LegacyProfileNamespaceMigrationV1,
) -> Result<(), PortError> {
    let platform = native_platform();
    migrate_legacy_profile_files_with_platform(
        &platform,
        config_root,
        config_dir,
        document_extension,
        migration,
    )
    .await
}

/// [`migrate_legacy_profile_files`] through an injected platform.
pub async fn migrate_legacy_profile_files_with_platform(
    platform: &Arc<dyn StorePlatform>,
    config_root: &Path,
    config_dir: &Path,
    document_extension: ProfileDocumentExtension,
    migration: &LegacyProfileNamespaceMigrationV1,
) -> Result<(), PortError> {
    let extension = document_extension.as_str();
    let root = config_root.to_path_buf();
    let entries = match run_platform_io(platform, move |platform| platform.read_dir(&root)).await {
        Ok(entries) => entries,
        Err(PortError::NotFound(_)) => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let name = entry
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        if entry.kind == StoreEntryKind::Directory
            && migration.directories.iter().any(|dir| dir == &name)
        {
            move_directory_contents(platform, &entry.path, &config_dir.join(&name)).await?;
        } else if entry.kind == StoreEntryKind::File
            && (name.ends_with(&format!(".{extension}"))
                || name.ends_with(&format!(".{extension}.bak")))
        {
            move_file(platform, &entry.path, &config_dir.join(&name)).await?;
        }
    }
    Ok(())
}

/// Whether the legacy shared namespace still holds profile data.
pub async fn has_legacy_profile_data(
    layout: &ProfileStoreLayout,
    config_root: &Path,
    settings_file: &Path,
    document_extension: ProfileDocumentExtension,
    migration: &LegacyProfileNamespaceMigrationV1,
) -> Result<bool, PortError> {
    let platform = native_platform();
    has_legacy_profile_data_with_platform(
        &platform,
        layout,
        config_root,
        settings_file,
        document_extension,
        migration,
    )
    .await
}

/// [`has_legacy_profile_data`] through an injected platform.
pub async fn has_legacy_profile_data_with_platform(
    platform: &Arc<dyn StorePlatform>,
    layout: &ProfileStoreLayout,
    config_root: &Path,
    settings_file: &Path,
    document_extension: ProfileDocumentExtension,
    migration: &LegacyProfileNamespaceMigrationV1,
) -> Result<bool, PortError> {
    let settings_file = settings_file.to_path_buf();
    if run_platform_io(platform, move |platform| platform.exists(&settings_file)).await? {
        return Ok(true);
    }

    let legacy_settings = migration.settings_file().to_path_buf();
    if run_platform_io(platform, move |platform| platform.exists(&legacy_settings)).await? {
        let legacy =
            read_profile_state_with_platform(platform, layout, migration.settings_file()).await?;
        if legacy
            .get("default")
            .and_then(|value| value.get("profile"))
            .is_some()
            || legacy.get("profiles").is_some()
        {
            return Ok(true);
        }
    }
    if let Some(sync_index_name) = migration.sync_index_file_name() {
        let sync_index = config_root.join(sync_index_name);
        if run_platform_io(platform, move |platform| platform.exists(&sync_index)).await? {
            return Ok(true);
        }
    }

    let extension = document_extension.as_str();
    let root = config_root.to_path_buf();
    let entries = match run_platform_io(platform, move |platform| platform.read_dir(&root)).await {
        Ok(entries) => entries,
        Err(PortError::NotFound(_)) => return Ok(false),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let name = entry
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let legacy_document = name.ends_with(&format!(".{extension}"))
            || name.ends_with(&format!(".{extension}.bak"));
        let legacy_directory = entry.kind == StoreEntryKind::Directory
            && migration.directories.iter().any(|dir| dir == &name);
        if (entry.kind == StoreEntryKind::File && legacy_document) || legacy_directory {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Remove the legacy sync index after its data has been adopted.
pub async fn retire_legacy_sync_index(
    config_root: &Path,
    migration: &LegacyProfileNamespaceMigrationV1,
) -> Result<(), PortError> {
    let platform = native_platform();
    retire_legacy_sync_index_with_platform(&platform, config_root, migration).await
}

/// [`retire_legacy_sync_index`] through an injected platform.
pub async fn retire_legacy_sync_index_with_platform(
    platform: &Arc<dyn StorePlatform>,
    config_root: &Path,
    migration: &LegacyProfileNamespaceMigrationV1,
) -> Result<(), PortError> {
    let Some(path) = migration.sync_index_file_name() else {
        return Ok(());
    };
    let path = config_root.join(path);
    run_platform_io(platform, move |platform| platform.remove_file(&path)).await
}

/// Rewrite (or delete, when empty) the legacy shared settings after a merge.
pub async fn write_legacy_settings(
    layout: &ProfileStoreLayout,
    path: &Path,
    state: &toml::Value,
) -> Result<(), PortError> {
    let platform = native_platform();
    write_legacy_settings_with_platform(&platform, layout, path, state).await
}

/// [`write_legacy_settings`] through an injected platform.
pub async fn write_legacy_settings_with_platform(
    platform: &Arc<dyn StorePlatform>,
    layout: &ProfileStoreLayout,
    path: &Path,
    state: &toml::Value,
) -> Result<(), PortError> {
    let Some(table) = state.as_table() else {
        return Err(namespace_configuration_error(
            "legacy settings must be a TOML table",
        ));
    };
    if table.is_empty() {
        let path = path.to_path_buf();
        return run_platform_io(platform, move |platform| platform.remove_file(&path)).await;
    }
    write_profile_state_atomically_with_platform(platform, layout, path, state).await
}

fn merge_value(
    state: &mut Map<String, toml::Value>,
    table_name: &str,
    key: &str,
    value: toml::Value,
) -> Result<(), PortError> {
    let table = state
        .entry(table_name.to_owned())
        .or_insert_with(|| toml::Value::Table(Map::new()))
        .as_table_mut()
        .ok_or_else(|| {
            namespace_configuration_error(format!("`{table_name}` must be a TOML table"))
        })?;
    match table.get(key) {
        Some(existing) if existing == &value => Ok(()),
        Some(_) => Err(namespace_configuration_error(format!(
            "profile namespace migration found conflicting `{table_name}.{key}` values"
        ))),
        None => {
            table.insert(key.into(), value);
            Ok(())
        }
    }
}

async fn move_directory_contents(
    platform: &Arc<dyn StorePlatform>,
    source: &Path,
    destination: &Path,
) -> Result<(), PortError> {
    enum DirectoryMove {
        Visit(PathBuf, PathBuf),
        Remove(PathBuf),
    }
    let mut pending = vec![DirectoryMove::Visit(
        source.to_path_buf(),
        destination.to_path_buf(),
    )];
    while let Some(move_step) = pending.pop() {
        match move_step {
            DirectoryMove::Visit(source, destination) => {
                let destination_dir = destination.clone();
                run_platform_io(platform, move |platform| {
                    platform.create_dir_all(&destination_dir)
                })
                .await?;
                let source_dir = source.clone();
                let entries =
                    run_platform_io(platform, move |platform| platform.read_dir(&source_dir))
                        .await?;
                pending.push(DirectoryMove::Remove(source));
                for entry in entries {
                    let destination_path = entry
                        .path
                        .file_name()
                        .map(|name| destination.join(name))
                        .unwrap_or_else(|| destination.clone());
                    match entry.kind {
                        StoreEntryKind::Directory => {
                            pending.push(DirectoryMove::Visit(entry.path, destination_path));
                        }
                        StoreEntryKind::File => {
                            move_file(platform, &entry.path, &destination_path).await?;
                        }
                        StoreEntryKind::Other => {
                            return Err(namespace_configuration_error(format!(
                                "profile namespace migration refuses non-file entry `{}`",
                                entry.path.display()
                            )));
                        }
                    }
                }
            }
            DirectoryMove::Remove(source) => {
                run_platform_io(platform, move |platform| platform.remove_dir(&source)).await?;
            }
        }
    }
    Ok(())
}

async fn move_file(
    platform: &Arc<dyn StorePlatform>,
    source: &Path,
    destination: &Path,
) -> Result<(), PortError> {
    let destination_path = destination.to_path_buf();
    let exists =
        run_platform_io(platform, move |platform| platform.exists(&destination_path)).await?;
    if exists {
        let source_path = source.to_path_buf();
        let source_bytes = run_platform_io(platform, move |platform| platform.read(&source_path))
            .await?
            .ok_or_else(|| {
                PortError::Io("legacy source file disappeared during migration".into())
            })?;
        let destination_path = destination.to_path_buf();
        let destination_bytes =
            run_platform_io(platform, move |platform| platform.read(&destination_path))
                .await?
                .ok_or_else(|| {
                    namespace_configuration_error("legacy destination file is missing")
                })?;
        if source_bytes != destination_bytes {
            return Err(namespace_configuration_error(format!(
                "profile namespace migration found conflicting files `{}` and `{}`",
                source.display(),
                destination.display()
            )));
        }
        let source_path = source.to_path_buf();
        return run_platform_io(platform, move |platform| platform.remove_file(&source_path)).await;
    }
    if let Some(parent) = destination.parent() {
        let parent = parent.to_path_buf();
        run_platform_io(platform, move |platform| platform.create_dir_all(&parent)).await?;
    }
    let source_path = source.to_path_buf();
    let destination_path = destination.to_path_buf();
    run_platform_io(platform, move |platform| {
        platform.rename(&source_path, &destination_path)
    })
    .await
}

fn namespace_configuration_error(message: impl Into<String>) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::Configuration, message, false))
}

pub fn is_single_relative_component(value: &str) -> bool {
    let mut components = Path::new(value).components();
    matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(_)), None)
    )
}

#[cfg(test)]
mod tests {
    use super::{LegacyProfileNamespaceMigrationV1, ProfileNamespaceSchema, merge_profile_state};
    use std::path::PathBuf;

    #[test]
    fn legacy_migration_paths_cannot_escape_the_shared_root() {
        for invalid_name in ["../outside", "/absolute", "nested/path", ".", ".."] {
            let migration = LegacyProfileNamespaceMigrationV1::new(
                PathBuf::from("/home/app/config.toml"),
                Some(invalid_name.into()),
                vec!["options".into()],
            );
            assert!(migration.validate().is_err(), "accepted {invalid_name:?}");

            let migration = LegacyProfileNamespaceMigrationV1::new(
                PathBuf::from("/home/app/config.toml"),
                Some("sync_state.db".into()),
                vec![invalid_name.into()],
            );
            assert!(migration.validate().is_err(), "accepted {invalid_name:?}");
        }

        let migration = LegacyProfileNamespaceMigrationV1::new(
            PathBuf::from("/home/app/config.toml"),
            Some("sync_state.db".into()),
            vec!["options".into(), "snapshots".into()],
        );
        assert!(migration.validate().is_ok());
    }

    #[test]
    fn schema_marker_is_typed_and_versioned() {
        assert_eq!(
            ProfileNamespaceSchema::parse(None).unwrap(),
            ProfileNamespaceSchema::LegacyShared
        );
        assert!(ProfileNamespaceSchema::parse(Some(&toml::Value::String("nope".into()))).is_err());
        let mut state = toml::Value::Table(Default::default());
        ProfileNamespaceSchema::write_engine_scoped_v1(&mut state).unwrap();
        assert_eq!(
            ProfileNamespaceSchema::parse(state.get("profile_namespace_schema")).unwrap(),
            ProfileNamespaceSchema::EngineScopedV1
        );
    }

    #[test]
    fn conflicting_legacy_rows_are_rejected() {
        let mut scoped: toml::Value =
            toml::from_str("[profiles.a]\nsubscription_url = \"x\"\n").unwrap();
        let mut legacy: toml::Value =
            toml::from_str("[profiles.a]\nsubscription_url = \"y\"\n").unwrap();
        assert!(merge_profile_state(&mut scoped, &mut legacy).is_err());
    }

    #[test]
    fn identical_legacy_rows_merge_without_duplication() {
        let mut scoped: toml::Value = toml::from_str("[default]\nprofile = \"a\"\n").unwrap();
        let mut legacy: toml::Value =
            toml::from_str("[default]\nprofile = \"a\"\n[profiles.a]\nx = 1\n").unwrap();
        merge_profile_state(&mut scoped, &mut legacy).unwrap();
        assert!(scoped.get("profiles").and_then(|p| p.get("a")).is_some());
        assert!(legacy.as_table().unwrap().is_empty());
    }
}

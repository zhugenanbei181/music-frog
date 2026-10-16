//! Engine-scoped profile namespace schema and durable settings I/O.

use crate::document_store::write_document_bytes_durable;
use crate::metadata::{SUBSCRIPTION_SERVICE, legacy_subscription_key, subscription_key};
use crate::store::EngineProfileStore;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_domain::profiles::validate_profile_name;
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use std::future::Future;
use std::io::{self, ErrorKind};
use std::path::{Component, Path, PathBuf};
use tokio::fs;
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

    fn validate(&self) -> Result<(), PortError> {
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

/// Initialize only the generic engine-scoped schema. The caller must hold
/// the product reset fence and the profile write lock.
pub async fn initialize_engine_scoped_namespace_unlocked(
    config_dir: &Path,
    settings_file: &Path,
) -> Result<(), PortError> {
    fs::create_dir_all(config_dir).await.map_err(map_io_error)?;
    let mut state = read_profile_state(settings_file).await?;
    if matches!(
        ProfileNamespaceSchema::parse(state.get(PROFILE_NAMESPACE_SCHEMA_KEY))?,
        ProfileNamespaceSchema::EngineScopedV1
    ) {
        return Ok(());
    }
    ProfileNamespaceSchema::write_engine_scoped_v1(&mut state)?;
    write_profile_state_atomically(settings_file, &state).await
}

pub async fn read_profile_state(path: &Path) -> Result<toml::Value, PortError> {
    recover_profile_state_backup(path).await?;
    let text = match fs::read_to_string(path).await {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(toml::Value::Table(Map::new()));
        }
        Err(error) => return Err(map_io_error(error)),
    };
    toml::from_str(&text).map_err(|error| namespace_configuration_error(error.to_string()))
}

pub async fn write_profile_state_atomically(
    path: &Path,
    state: &toml::Value,
) -> Result<(), PortError> {
    recover_profile_state_backup(path).await?;
    let content = toml::to_string_pretty(state)
        .map_err(|error| namespace_configuration_error(error.to_string()))?;
    write_document_bytes_durable(path, content.as_bytes()).await
}

pub async fn recover_profile_state_backup(path: &Path) -> Result<(), PortError> {
    let backup = profile_state_backup_path(path);
    match (
        fs::try_exists(path).await.map_err(map_io_error)?,
        fs::try_exists(&backup).await.map_err(map_io_error)?,
    ) {
        (false, true) => fs::rename(backup, path).await.map_err(map_io_error),
        (true, true) => fs::remove_file(backup).await.map_err(map_io_error),
        (_, false) => Ok(()),
    }
}

pub fn profile_state_backup_path(path: &Path) -> PathBuf {
    path.with_extension("toml.namespace-backup")
}

impl<S: SecureStore> EngineProfileStore<S> {
    /// Initialize this store's typed namespace policy once, under a
    /// caller-provided product mutation fence.
    pub async fn initialize_profile_namespace_with<G, F, Fut>(
        &self,
        acquire_fence: F,
    ) -> Result<(), PortError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<G, PortError>>,
    {
        self.initialize_namespace_with(|| async {
            let _fence = acquire_fence().await?;
            match self.namespace_initialization() {
                ProfileNamespaceInitialization::EngineScoped => {
                    initialize_engine_scoped_namespace_unlocked(
                        self.config_dir(),
                        self.settings_file(),
                    )
                    .await
                }
                ProfileNamespaceInitialization::LegacyV1(migration) => {
                    initialize_legacy_namespace_v1_unlocked(self, migration).await
                }
            }
        })
        .await
    }

    pub async fn reset_engine_scoped_namespace_marker_unlocked(&self) -> Result<(), PortError> {
        let mut state = toml::Value::Table(Map::new());
        ProfileNamespaceSchema::write_engine_scoped_v1(&mut state)?;
        write_profile_state_atomically(self.settings_file(), &state).await
    }
}

async fn initialize_legacy_namespace_v1_unlocked<S: SecureStore>(
    store: &EngineProfileStore<S>,
    migration: &LegacyProfileNamespaceMigrationV1,
) -> Result<(), PortError> {
    migration.validate()?;
    let mut scoped_state = read_profile_state(store.settings_file()).await?;
    let schema = ProfileNamespaceSchema::parse(scoped_state.get(PROFILE_NAMESPACE_SCHEMA_KEY))?;
    match schema {
        ProfileNamespaceSchema::EngineScopedV1 => {
            migrate_legacy_profile_files(store, migration).await?;
            let mut legacy_state = read_profile_state(&migration.settings_file).await?;
            let previous_legacy_state = legacy_state.clone();
            let mut retired = toml::Value::Table(Map::new());
            merge_profile_state(&mut retired, &mut legacy_state)?;
            if legacy_state != previous_legacy_state {
                write_legacy_settings(&migration.settings_file, &legacy_state).await?;
            }
            retire_legacy_sync_index(store, migration).await?;
            return Ok(());
        }
        ProfileNamespaceSchema::LegacyShared => {}
    }

    if !has_legacy_profile_data(store, migration).await? {
        return Ok(());
    }
    fs::create_dir_all(store.config_dir())
        .await
        .map_err(map_io_error)?;
    migrate_legacy_profile_files(store, migration).await?;
    let mut legacy_state = read_profile_state(&migration.settings_file).await?;
    let previous_legacy_state = legacy_state.clone();
    merge_profile_state(&mut scoped_state, &mut legacy_state)?;
    migrate_legacy_credentials(store, &mut scoped_state).await?;
    ProfileNamespaceSchema::write_engine_scoped_v1(&mut scoped_state)?;
    write_profile_state_atomically(store.settings_file(), &scoped_state).await?;
    if legacy_state != previous_legacy_state {
        write_legacy_settings(&migration.settings_file, &legacy_state).await?;
    }
    retire_legacy_sync_index(store, migration).await
}

async fn migrate_legacy_profile_files<S: SecureStore>(
    store: &EngineProfileStore<S>,
    migration: &LegacyProfileNamespaceMigrationV1,
) -> Result<(), PortError> {
    let mut entries = match fs::read_dir(store.config_root()).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(map_io_error(error)),
    };
    while let Some(entry) = entries.next_entry().await.map_err(map_io_error)? {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let source = entry.path();
        let file_type = entry.file_type().await.map_err(map_io_error)?;
        if file_type.is_dir() && migration.directories.iter().any(|dir| dir == name.as_ref()) {
            move_directory_contents(&source, &store.config_dir().join(name.as_ref())).await?;
        } else if file_type.is_file()
            && (name.ends_with(&format!(".{}", store.document_extension().as_str()))
                || name.ends_with(&format!(".{}.bak", store.document_extension().as_str())))
        {
            move_file(&source, &store.config_dir().join(name.as_ref())).await?;
        }
    }
    Ok(())
}

async fn has_legacy_profile_data<S: SecureStore>(
    store: &EngineProfileStore<S>,
    migration: &LegacyProfileNamespaceMigrationV1,
) -> Result<bool, PortError> {
    if fs::try_exists(store.settings_file())
        .await
        .map_err(map_io_error)?
    {
        return Ok(true);
    }

    if fs::try_exists(&migration.settings_file)
        .await
        .map_err(map_io_error)?
    {
        let legacy = read_profile_state(&migration.settings_file).await?;
        if legacy
            .get("default")
            .and_then(|value| value.get("profile"))
            .is_some()
            || legacy.get("profiles").is_some()
        {
            return Ok(true);
        }
    }
    if let Some(sync_index_name) = &migration.sync_index_file_name {
        let sync_index = store.config_root().join(sync_index_name);
        if fs::try_exists(sync_index).await.map_err(map_io_error)? {
            return Ok(true);
        }
    }

    let mut entries = match fs::read_dir(store.config_root()).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(map_io_error(error)),
    };
    while let Some(entry) = entries.next_entry().await.map_err(map_io_error)? {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let file_type = entry.file_type().await.map_err(map_io_error)?;
        let legacy_document = name.ends_with(&format!(".{}", store.document_extension().as_str()))
            || name.ends_with(&format!(".{}.bak", store.document_extension().as_str()));
        let legacy_directory =
            file_type.is_dir() && migration.directories.iter().any(|dir| dir == name.as_ref());
        if (file_type.is_file() && legacy_document) || legacy_directory {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn retire_legacy_sync_index<S: SecureStore>(
    store: &EngineProfileStore<S>,
    migration: &LegacyProfileNamespaceMigrationV1,
) -> Result<(), PortError> {
    let Some(path) = &migration.sync_index_file_name else {
        return Ok(());
    };
    match fs::remove_file(store.config_root().join(path)).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(map_io_error(error)),
    }
}

async fn migrate_legacy_credentials<S: SecureStore>(
    store: &EngineProfileStore<S>,
    state: &mut toml::Value,
) -> Result<(), PortError> {
    let engine = store.engine_id();
    let Some(profiles) = state
        .as_table_mut()
        .and_then(|table| table.get_mut("profiles"))
        .and_then(toml::Value::as_table_mut)
    else {
        return Ok(());
    };

    for (profile, metadata) in profiles.iter_mut() {
        validate_profile_name(profile).map_err(|error| {
            namespace_configuration_error(format!(
                "legacy profile ID `{profile}` is invalid: {error}"
            ))
        })?;
        let metadata = metadata.as_table_mut().ok_or_else(|| {
            namespace_configuration_error(format!(
                "profile namespace migration found invalid metadata for `{profile}`"
            ))
        })?;
        let old_key = metadata
            .get("subscription_url_key")
            .and_then(toml::Value::as_str)
            .map(str::to_owned)
            .or_else(|| {
                metadata
                    .get("subscription_url")
                    .and_then(toml::Value::as_str)
                    .filter(|url| !url.trim().is_empty())
                    .map(|_| legacy_subscription_key(profile))
            });
        let Some(old_key) = old_key else {
            continue;
        };

        let new_key = subscription_key(&engine, profile);
        if old_key != new_key {
            let mut value = store
                .credential_store()
                .get(SUBSCRIPTION_SERVICE, &new_key)
                .await
                .map_err(credential_migration_error)?;
            if value.is_none() {
                value = store
                    .credential_store()
                    .get(SUBSCRIPTION_SERVICE, &old_key)
                    .await
                    .map_err(credential_migration_error)?;
                if value.is_none() {
                    value = metadata
                        .get("subscription_url")
                        .and_then(toml::Value::as_str)
                        .map(str::to_owned);
                }
                if let Some(value) = value.as_deref() {
                    store
                        .credential_store()
                        .set(SUBSCRIPTION_SERVICE, &new_key, value)
                        .await
                        .map_err(credential_migration_error)?;
                }
            }
            if value.is_some() {
                store
                    .credential_store()
                    .delete(SUBSCRIPTION_SERVICE, &old_key)
                    .await
                    .map_err(credential_migration_error)?;
            }
        }
        metadata.insert("subscription_url_key".into(), toml::Value::String(new_key));
    }
    Ok(())
}

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

async fn write_legacy_settings(path: &Path, state: &toml::Value) -> Result<(), PortError> {
    let Some(table) = state.as_table() else {
        return Err(namespace_configuration_error(
            "legacy settings must be a TOML table",
        ));
    };
    if table.is_empty() {
        match fs::remove_file(path).await {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(map_io_error(error)),
        }
    }
    write_profile_state_atomically(path, state).await
}

async fn move_directory_contents(source: &Path, destination: &Path) -> Result<(), PortError> {
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
                fs::create_dir_all(&destination)
                    .await
                    .map_err(map_io_error)?;
                let mut entries = fs::read_dir(&source).await.map_err(map_io_error)?;
                let mut children = Vec::new();
                while let Some(entry) = entries.next_entry().await.map_err(map_io_error)? {
                    children.push((
                        entry.path(),
                        destination.join(entry.file_name()),
                        entry.file_type().await.map_err(map_io_error)?,
                    ));
                }
                pending.push(DirectoryMove::Remove(source));
                for (source_path, destination_path, file_type) in children {
                    if file_type.is_dir() {
                        pending.push(DirectoryMove::Visit(source_path, destination_path));
                    } else if file_type.is_file() {
                        move_file(&source_path, &destination_path).await?;
                    } else {
                        return Err(namespace_configuration_error(format!(
                            "profile namespace migration refuses non-file entry `{}`",
                            source_path.display()
                        )));
                    }
                }
            }
            DirectoryMove::Remove(source) => {
                fs::remove_dir(source).await.map_err(map_io_error)?;
            }
        }
    }
    Ok(())
}

async fn move_file(source: &Path, destination: &Path) -> Result<(), PortError> {
    if fs::try_exists(destination).await.map_err(map_io_error)? {
        let source_bytes = fs::read(source).await.map_err(map_io_error)?;
        let destination_bytes = fs::read(destination).await.map_err(map_io_error)?;
        if source_bytes != destination_bytes {
            return Err(namespace_configuration_error(format!(
                "profile namespace migration found conflicting files `{}` and `{}`",
                source.display(),
                destination.display()
            )));
        }
        fs::remove_file(source).await.map_err(map_io_error)?;
        return Ok(());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).await.map_err(map_io_error)?;
    }
    fs::rename(source, destination).await.map_err(map_io_error)
}

fn namespace_configuration_error(message: impl Into<String>) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::Configuration, message, false))
}

fn credential_migration_error(error: PortError) -> PortError {
    namespace_configuration_error(format!(
        "profile credential namespace migration failed: {error}"
    ))
}

fn is_single_relative_component(value: &str) -> bool {
    let mut components = Path::new(value).components();
    matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(_)), None)
    )
}

#[cfg(test)]
mod tests {
    use super::LegacyProfileNamespaceMigrationV1;
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
}

fn map_io_error(error: io::Error) -> PortError {
    if error.kind() == ErrorKind::PermissionDenied {
        PortError::PermissionDenied(error.to_string())
    } else {
        PortError::Io(error.to_string())
    }
}

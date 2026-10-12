//! One-time Mihomo migration from former shared ProfileId paths. Other engine
//! stores initialize only their own namespace and never consume legacy files.

use super::paths::profile_id;
use super::{ConfigManager, ProfileNamespaceInitialization};
use infiltrator_contract::error::ErrorCode;
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use infiltrator_profile_store::metadata::{
    SUBSCRIPTION_SERVICE, legacy_subscription_key, subscription_key,
};
#[cfg(test)]
use infiltrator_profile_store::namespace::profile_state_backup_path as namespace_backup_path;
use infiltrator_profile_store::namespace::{
    PROFILE_NAMESPACE_SCHEMA_KEY as NAMESPACE_SCHEMA_KEY,
    ProfileNamespaceSchema as NamespaceSchema, read_profile_state, write_profile_state_atomically,
};
use mihomo_api::error::{MihomoError, Result};
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use tokio::fs;

fn profile_namespace_fence_error(error: PortError) -> MihomoError {
    match error {
        PortError::Rejected(failure) if failure.code == ErrorCode::Configuration => {
            MihomoError::Config(failure.message)
        }
        PortError::PermissionDenied(message) => {
            MihomoError::Io(Error::new(ErrorKind::PermissionDenied, message))
        }
        PortError::NotFound(message) => MihomoError::Io(Error::new(ErrorKind::NotFound, message)),
        PortError::Io(message) => MihomoError::Io(Error::new(ErrorKind::Other, message)),
        error => MihomoError::Config(error.to_string()),
    }
}

impl<S: SecureStore> ConfigManager<S> {
    /// Initialize this engine namespace before any production consumer is
    /// handed the store. The manager's typed policy decides whether a legacy
    /// Mihomo migration is authorized for this instance.
    pub async fn initialize_profile_namespace(&self) -> Result<()> {
        match self.namespace_initialization {
            ProfileNamespaceInitialization::MigrateLegacyMihomo => {
                self.profile_store
                    .initialize_namespace_with(|| async {
                        self.initialize_mihomo_profile_namespace_once().await
                    })
                    .await
            }
            ProfileNamespaceInitialization::EngineScoped => self
                .profile_store
                .initialize_engine_scoped_namespace_with(|| async {
                    crate::manager::lock_reset_fenced_profile_mutation_for_dir(self.config_dir())
                        .await
                })
                .await
                .map_err(profile_namespace_fence_error),
        }
    }

    /// Replace profile settings with the minimal engine-scope marker after a
    /// committed profile reset. The caller holds the cross-process profile
    /// write lock; this helper deliberately does not reacquire it.
    pub(crate) async fn reset_profile_namespace_marker_unlocked(&self) -> Result<()> {
        self.profile_store
            .reset_engine_scoped_namespace_marker_unlocked()
            .await
            .map_err(profile_namespace_fence_error)
    }

    async fn initialize_mihomo_profile_namespace_once(&self) -> Result<()> {
        let _guard = crate::manager::lock_reset_fenced_profile_mutation_for_dir(self.config_dir())
            .await
            .map_err(profile_namespace_fence_error)?;

        let mut scoped_state = read_toml_table(self.settings_file()).await?;
        let schema = NamespaceSchema::parse(scoped_state.get(NAMESPACE_SCHEMA_KEY))
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        match schema {
            NamespaceSchema::EngineScopedV1 => {
                self.migrate_legacy_profile_files().await?;
                let mut legacy_state = read_toml_table(&self.legacy_settings_file).await?;
                let previous_legacy_state = legacy_state.clone();
                let mut retired = toml::Value::Table(toml::map::Map::new());
                merge_profile_state(&mut retired, &mut legacy_state)?;
                if legacy_state != previous_legacy_state {
                    write_legacy_settings(&self.legacy_settings_file, &legacy_state).await?;
                }
                self.retire_legacy_sync_index().await?;
                return Ok(());
            }
            NamespaceSchema::LegacyShared => {}
        }

        if !self.has_legacy_profile_data().await? {
            return Ok(());
        }
        fs::create_dir_all(self.config_dir()).await?;
        self.migrate_legacy_profile_files().await?;
        let mut legacy_state = read_toml_table(&self.legacy_settings_file).await?;
        let previous_legacy_state = legacy_state.clone();
        merge_profile_state(&mut scoped_state, &mut legacy_state)?;
        self.migrate_legacy_credentials(&mut scoped_state).await?;

        NamespaceSchema::write_engine_scoped_v1(&mut scoped_state)
            .map_err(profile_namespace_fence_error)?;
        write_toml_atomically(self.settings_file(), &scoped_state).await?;
        if legacy_state != previous_legacy_state {
            write_legacy_settings(&self.legacy_settings_file, &legacy_state).await?;
        }
        self.retire_legacy_sync_index().await?;
        Ok(())
    }

    async fn migrate_legacy_profile_files(&self) -> Result<()> {
        let mut entries = match fs::read_dir(self.config_root()).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let source = entry.path();
            let file_type = entry.file_type().await?;
            if file_type.is_dir() && matches!(name.as_ref(), "options" | "snapshots") {
                move_directory_contents(&source, &self.config_dir().join(name.as_ref())).await?;
            } else if file_type.is_file()
                && (name.ends_with(".yaml") || name.ends_with(".yaml.bak"))
            {
                move_file(&source, &self.config_dir().join(name.as_ref())).await?;
            }
        }
        Ok(())
    }

    async fn has_legacy_profile_data(&self) -> Result<bool> {
        if fs::try_exists(self.settings_file()).await? {
            return Ok(true);
        }

        if fs::try_exists(&self.legacy_settings_file).await? {
            let legacy = read_toml_table(&self.legacy_settings_file).await?;
            if legacy
                .get("default")
                .and_then(|value| value.get("profile"))
                .is_some()
                || legacy.get("profiles").is_some()
            {
                return Ok(true);
            }
        }
        if fs::try_exists(self.legacy_sync_index_path()).await? {
            return Ok(true);
        }

        let mut entries = match fs::read_dir(self.config_root()).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let file_type = entry.file_type().await?;
            if (file_type.is_file() && (name.ends_with(".yaml") || name.ends_with(".yaml.bak")))
                || (file_type.is_dir() && matches!(name.as_ref(), "options" | "snapshots"))
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn retire_legacy_sync_index(&self) -> Result<()> {
        let path = self.legacy_sync_index_path();
        match fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    fn legacy_sync_index_path(&self) -> PathBuf {
        self.config_root().join("sync_state.db")
    }

    async fn migrate_legacy_credentials(&self, state: &mut toml::Value) -> Result<()> {
        let engine = self.engine_id();
        let Some(profiles) = state
            .as_table_mut()
            .and_then(|table| table.get_mut("profiles"))
            .and_then(toml::Value::as_table_mut)
        else {
            return Ok(());
        };

        for (profile, metadata) in profiles.iter_mut() {
            profile_id(profile).map_err(|error| MihomoError::Config(error.to_string()))?;
            let metadata = metadata.as_table_mut().ok_or_else(|| {
                MihomoError::Config(format!(
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
                let mut value = self
                    .credential_store()
                    .get(SUBSCRIPTION_SERVICE, &new_key)
                    .await
                    .map_err(|error| credential_migration_error(error.to_string()))?;
                if value.is_none() {
                    value = self
                        .credential_store()
                        .get(SUBSCRIPTION_SERVICE, &old_key)
                        .await
                        .map_err(|error| credential_migration_error(error.to_string()))?;
                    if value.is_none() {
                        value = metadata
                            .get("subscription_url")
                            .and_then(toml::Value::as_str)
                            .map(str::to_owned);
                    }
                    if let Some(value) = value.as_deref() {
                        self.credential_store()
                            .set(SUBSCRIPTION_SERVICE, &new_key, value)
                            .await
                            .map_err(|error| credential_migration_error(error.to_string()))?;
                    }
                }
                if value.is_some() {
                    self.credential_store()
                        .delete(SUBSCRIPTION_SERVICE, &old_key)
                        .await
                        .map_err(|error| credential_migration_error(error.to_string()))?;
                }
            }
            metadata.insert("subscription_url_key".into(), toml::Value::String(new_key));
        }
        Ok(())
    }
}

async fn read_toml_table(path: &Path) -> Result<toml::Value> {
    read_profile_state(path)
        .await
        .map_err(profile_namespace_fence_error)
}

fn merge_profile_state(scoped: &mut toml::Value, legacy: &mut toml::Value) -> Result<()> {
    let scoped = scoped
        .as_table_mut()
        .ok_or_else(|| MihomoError::Config("profile state must be a TOML table".into()))?;
    let legacy = legacy
        .as_table_mut()
        .ok_or_else(|| MihomoError::Config("legacy profile state must be a TOML table".into()))?;

    if let Some(default) = legacy.get_mut("default") {
        let legacy_default = default.as_table_mut().ok_or_else(|| {
            MihomoError::Config("legacy `[default]` profile state must be a table".into())
        })?;
        if let Some(profile) = legacy_default.remove("profile") {
            if profile.as_str().is_none() {
                return Err(MihomoError::Config(
                    "legacy active profile pointer must be a string".into(),
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
            return Err(MihomoError::Config(
                "legacy `[profiles]` profile state must be a table".into(),
            ));
        };
        let scoped_profiles = scoped
            .entry("profiles")
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
            .as_table_mut()
            .ok_or_else(|| MihomoError::Config("profile metadata must be a TOML table".into()))?;
        for (profile, metadata) in legacy_profiles {
            match scoped_profiles.get(&profile) {
                Some(existing) if existing == &metadata => {}
                Some(_) => {
                    return Err(MihomoError::Config(format!(
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
    state: &mut toml::map::Map<String, toml::Value>,
    table_name: &str,
    key: &str,
    value: toml::Value,
) -> Result<()> {
    let table = state
        .entry(table_name)
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .ok_or_else(|| MihomoError::Config(format!("`{table_name}` must be a TOML table")))?;
    match table.get(key) {
        Some(existing) if existing == &value => Ok(()),
        Some(_) => Err(MihomoError::Config(format!(
            "profile namespace migration found conflicting `{table_name}.{key}` values"
        ))),
        None => {
            table.insert(key.into(), value);
            Ok(())
        }
    }
}

async fn write_legacy_settings(path: &Path, state: &toml::Value) -> Result<()> {
    let Some(table) = state.as_table() else {
        return Err(MihomoError::Config(
            "legacy settings must be a TOML table".into(),
        ));
    };
    if table.is_empty() {
        match fs::remove_file(path).await {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        }
    }
    write_toml_atomically(path, state).await
}

async fn write_toml_atomically(path: &Path, state: &toml::Value) -> Result<()> {
    write_profile_state_atomically(path, state)
        .await
        .map_err(profile_namespace_fence_error)
}

async fn move_directory_contents(source: &Path, destination: &Path) -> Result<()> {
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
                fs::create_dir_all(&destination).await?;
                let mut entries = fs::read_dir(&source).await?;
                let mut children = Vec::new();
                while let Some(entry) = entries.next_entry().await? {
                    children.push((
                        entry.path(),
                        destination.join(entry.file_name()),
                        entry.file_type().await?,
                    ));
                }
                pending.push(DirectoryMove::Remove(source));
                for (source_path, destination_path, file_type) in children {
                    if file_type.is_dir() {
                        pending.push(DirectoryMove::Visit(source_path, destination_path));
                    } else if file_type.is_file() {
                        move_file(&source_path, &destination_path).await?;
                    } else {
                        return Err(MihomoError::Config(format!(
                            "profile namespace migration refuses non-file entry `{}`",
                            source_path.display()
                        )));
                    }
                }
            }
            DirectoryMove::Remove(source) => {
                fs::remove_dir(source).await?;
            }
        }
    }
    Ok(())
}

async fn move_file(source: &Path, destination: &Path) -> Result<()> {
    if fs::try_exists(destination).await? {
        let source_bytes = fs::read(source).await?;
        let destination_bytes = fs::read(destination).await?;
        if source_bytes != destination_bytes {
            return Err(MihomoError::Config(format!(
                "profile namespace migration found conflicting files `{}` and `{}`",
                source.display(),
                destination.display()
            )));
        }
        fs::remove_file(source).await?;
        return Ok(());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::rename(source, destination).await?;
    Ok(())
}

fn credential_migration_error(detail: String) -> MihomoError {
    MihomoError::Config(format!(
        "profile credential namespace migration failed: {detail}"
    ))
}

#[cfg(test)]
mod tests {
    use super::{NamespaceSchema, merge_profile_state, namespace_backup_path, read_toml_table};
    use crate::manager::ConfigManager;
    use crate::manager::paths::engine_config_dir;
    use async_trait::async_trait;
    use infiltrator_contract::core_identity::EngineId;
    use infiltrator_ports::error::PortError;
    use infiltrator_ports::secure_store::SecureStore;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use tokio::fs;
    use toml::map::Map;

    #[derive(Clone, Default)]
    struct MemorySecureStore(Arc<Mutex<HashMap<(String, String), String>>>);

    #[async_trait]
    impl SecureStore for MemorySecureStore {
        async fn get(&self, service: &str, key: &str) -> Result<Option<String>, PortError> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .get(&(service.to_owned(), key.to_owned()))
                .cloned())
        }

        async fn set(&self, service: &str, key: &str, value: &str) -> Result<(), PortError> {
            self.0
                .lock()
                .unwrap()
                .insert((service.to_owned(), key.to_owned()), value.to_owned());
            Ok(())
        }

        async fn delete(&self, service: &str, key: &str) -> Result<(), PortError> {
            self.0
                .lock()
                .unwrap()
                .remove(&(service.to_owned(), key.to_owned()));
            Ok(())
        }
    }

    #[test]
    fn namespace_schema_uses_exhaustive_legacy_and_current_states() {
        assert_eq!(
            NamespaceSchema::parse(None).unwrap(),
            NamespaceSchema::LegacyShared
        );
        assert_eq!(
            NamespaceSchema::parse(Some(&toml::Value::String("engine_scoped_v1".into()))).unwrap(),
            NamespaceSchema::EngineScopedV1
        );
        assert!(NamespaceSchema::parse(Some(&toml::Value::String("unknown".into()))).is_err());
        assert!(NamespaceSchema::parse(Some(&toml::Value::Integer(1))).is_err());
    }

    #[tokio::test]
    async fn interrupted_state_replace_restores_backup_before_reading() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("profile-state.toml");
        let backup = namespace_backup_path(&path);
        let previous = "[default]\nprofile = \"main\"\n";
        fs::write(&backup, previous).await.unwrap();

        let state = read_toml_table(&path).await.unwrap();

        assert_eq!(state["default"]["profile"].as_str(), Some("main"));
        assert_eq!(fs::read_to_string(&path).await.unwrap(), previous);
        assert!(!backup.exists());
    }

    #[test]
    fn profile_state_merge_moves_only_active_pointer_and_profile_metadata() {
        let mut scoped = toml::Value::Table(Map::new());
        let mut legacy = toml::Value::try_from(toml::toml! {
            [default]
            profile = "main"
            [profiles.main]
            subscription_url = "https://example.test/sub"
            [application]
            keep = "global"
        })
        .unwrap();

        merge_profile_state(&mut scoped, &mut legacy).unwrap();

        assert_eq!(scoped["default"]["profile"].as_str(), Some("main"));
        assert_eq!(
            scoped["profiles"]["main"]["subscription_url"].as_str(),
            Some("https://example.test/sub")
        );
        assert!(legacy.get("default").is_none());
        assert!(legacy.get("profiles").is_none());
        assert_eq!(legacy["application"]["keep"].as_str(), Some("global"));
    }

    #[tokio::test]
    async fn initialization_moves_legacy_profile_data_and_credentials_into_engine_namespace() {
        let home = tempfile::tempdir().unwrap();
        let config_root = home.path().join("configs");
        fs::create_dir_all(config_root.join("options"))
            .await
            .unwrap();
        fs::create_dir_all(config_root.join("snapshots/main"))
            .await
            .unwrap();
        fs::write(
            config_root.join("main.yaml"),
            b"# profile\r\nmode: rule\r\n",
        )
        .await
        .unwrap();
        fs::write(config_root.join("main.yaml.bak"), b"# backup\r\n")
            .await
            .unwrap();
        fs::write(config_root.join("options/main.yaml"), b"\xff\x00sidecar")
            .await
            .unwrap();
        fs::write(
            config_root.join("snapshots/main/1-deadbeef.yaml"),
            b"# snapshot\r\n",
        )
        .await
        .unwrap();
        fs::write(config_root.join("sync_state.db"), b"legacy index")
            .await
            .unwrap();
        fs::write(
            home.path().join("config.toml"),
            r#"[default]
profile = "main"

[profiles.main]
subscription_url = "https://example.test/sub"
subscription_url_key = "subscription:main"

[application]
keep = "value"
"#,
        )
        .await
        .unwrap();

        let credentials = MemorySecureStore::default();
        credentials
            .set(
                super::SUBSCRIPTION_SERVICE,
                "subscription:main",
                "https://example.test/secret-subscription",
            )
            .await
            .unwrap();
        let manager = ConfigManager::with_home_configs_dir_and_store(
            home.path().to_path_buf(),
            Some(config_root.to_str().unwrap()),
            credentials.clone(),
        )
        .unwrap();

        manager.initialize_profile_namespace().await.unwrap();
        manager.initialize_profile_namespace().await.unwrap();

        let scoped = manager.config_dir();
        assert_eq!(
            fs::read(scoped.join("main.yaml")).await.unwrap(),
            b"# profile\r\nmode: rule\r\n"
        );
        assert_eq!(
            fs::read(scoped.join("main.yaml.bak")).await.unwrap(),
            b"# backup\r\n"
        );
        assert_eq!(
            fs::read(scoped.join("options/main.yaml")).await.unwrap(),
            b"\xff\x00sidecar"
        );
        assert_eq!(
            fs::read(scoped.join("snapshots/main/1-deadbeef.yaml"))
                .await
                .unwrap(),
            b"# snapshot\r\n"
        );
        assert!(!config_root.join("main.yaml").exists());
        assert!(!config_root.join("main.yaml.bak").exists());
        assert!(!config_root.join("options").exists());
        assert!(!config_root.join("snapshots").exists());
        assert!(!config_root.join("sync_state.db").exists());

        let profile_state = fs::read_to_string(scoped.join("profile-state.toml"))
            .await
            .unwrap();
        let parsed: toml::Value = toml::from_str(&profile_state).unwrap();
        assert_eq!(parsed["default"]["profile"].as_str(), Some("main"));
        assert_eq!(
            parsed["profiles"]["main"]["subscription_url_key"].as_str(),
            Some("subscription:mihomo:main")
        );
        assert_eq!(
            parsed["profile_namespace_schema"].as_str(),
            Some("engine_scoped_v1")
        );
        assert_eq!(
            credentials
                .get(super::SUBSCRIPTION_SERVICE, "subscription:mihomo:main")
                .await
                .unwrap()
                .as_deref(),
            Some("https://example.test/secret-subscription")
        );
        assert_eq!(
            credentials
                .get(super::SUBSCRIPTION_SERVICE, "subscription:main")
                .await
                .unwrap(),
            None
        );

        let legacy_settings = fs::read_to_string(home.path().join("config.toml"))
            .await
            .unwrap();
        let legacy: toml::Value = toml::from_str(&legacy_settings).unwrap();
        assert!(legacy.get("default").is_none());
        assert!(legacy.get("profiles").is_none());
        assert_eq!(legacy["application"]["keep"].as_str(), Some("value"));
    }

    #[test]
    fn engine_ids_produce_disjoint_roots_for_identical_profile_ids() {
        let root = PathBuf::from("/shared/configs");
        let mihomo = EngineId::new("mihomo").unwrap();
        let sing_box = EngineId::new("sing-box").unwrap();
        let mihomo_path = engine_config_dir(&root, &mihomo).join("main.yaml");
        let sing_box_path = engine_config_dir(&root, &sing_box).join("main.yaml");

        assert_ne!(mihomo_path, sing_box_path);
        assert!(mihomo_path.ends_with("engines/mihomo/main.yaml"));
        assert!(sing_box_path.ends_with("engines/sing-box/main.yaml"));
    }
}

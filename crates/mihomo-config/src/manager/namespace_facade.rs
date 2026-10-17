//! Mihomo facade for the shared engine-scoped namespace owner.

use super::{ConfigManager, lock_reset_fenced_profile_mutation_for_dir};
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::error::ErrorCode;
use lurker_core_ports::error::PortError;
use mihomo_driver::error::{MihomoError, Result};
use std::io::{Error, ErrorKind};

fn namespace_error(error: PortError) -> MihomoError {
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
    /// The shared store owns engine-scoped bootstrap and explicit legacy
    /// namespace migration; this facade supplies only the product reset fence.
    pub async fn initialize_profile_namespace(&self) -> Result<()> {
        self.profile_store
            .initialize_profile_namespace_with(|| async {
                lock_reset_fenced_profile_mutation_for_dir(self.config_dir()).await
            })
            .await
            .map_err(namespace_error)
    }
}

#[cfg(test)]
mod tests {
    use crate::manager::ConfigManager;
    use crate::manager::paths::engine_config_dir;
    use async_trait::async_trait;
    use infiltrator_ports::secure_store::SecureStore;
    use infiltrator_profile_store::layout::product_layout;
    use infiltrator_profile_store::metadata::SUBSCRIPTION_SERVICE;
    use lurker_core_contract::identity::EngineId;
    use lurker_core_ports::error::PortError;
    use lurker_profile_store::namespace::{
        ProfileNamespaceSchema, merge_profile_state, read_profile_state,
    };
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
            ProfileNamespaceSchema::parse(None).unwrap(),
            ProfileNamespaceSchema::LegacyShared
        );
        assert_eq!(
            ProfileNamespaceSchema::parse(Some(&toml::Value::String("engine_scoped_v1".into())))
                .unwrap(),
            ProfileNamespaceSchema::EngineScopedV1
        );
        assert!(
            ProfileNamespaceSchema::parse(Some(&toml::Value::String("unknown".into()))).is_err()
        );
        assert!(ProfileNamespaceSchema::parse(Some(&toml::Value::Integer(1))).is_err());
    }

    #[tokio::test]
    async fn interrupted_state_replace_restores_backup_before_reading() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("profile-state.toml");
        let layout = product_layout();
        let backup = layout.profile_state_backup_path(&path);
        let previous = "[default]\nprofile = \"main\"\n";
        fs::write(&backup, previous).await.unwrap();

        let state = read_profile_state(layout, &path).await.unwrap();

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
                SUBSCRIPTION_SERVICE,
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
                .get(SUBSCRIPTION_SERVICE, "subscription:mihomo:main")
                .await
                .unwrap()
                .as_deref(),
            Some("https://example.test/secret-subscription")
        );
        assert_eq!(
            credentials
                .get(SUBSCRIPTION_SERVICE, "subscription:main")
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

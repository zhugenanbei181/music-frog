use super::{profile::Profile, yaml};
use crate::port::{find_available_port, is_port_available, parse_port_from_addr};
use mihomo_api::{MihomoError, Result};
use mihomo_platform::{get_home_dir, CredentialStore, DefaultCredentialStore};
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use tokio::fs;

pub struct ConfigManager<S: CredentialStore = DefaultCredentialStore> {
    config_dir: PathBuf,
    settings_file: PathBuf,
    credential_store: S,
}

impl<S: CredentialStore> ConfigManager<S> {
    pub fn new_with_store(credential_store: S) -> Result<Self> {
        let home = get_home_dir()?;
        Self::with_home_and_store(home, credential_store)
    }

    pub fn with_home_and_store(home: PathBuf, credential_store: S) -> Result<Self> {
        let config_dir = home.join("configs");
        let settings_file = home.join("config.toml");

        Ok(Self {
            config_dir,
            settings_file,
            credential_store,
        })
    }
    pub async fn load(&self, profile: &str) -> Result<String> {
        let path = self.config_dir.join(format!("{}.yaml", profile));
        if !path.exists() {
            return Err(MihomoError::NotFound(format!(
                "Profile '{}' not found",
                profile
            )));
        }

        let content = fs::read_to_string(&path).await?;
        Ok(content)
    }

    pub async fn save(&self, profile: &str, content: &str) -> Result<()> {
        fs::create_dir_all(&self.config_dir).await?;

        yaml::validate(content)?;

        let path = self.config_dir.join(format!("{}.yaml", profile));
        fs::write(&path, content).await?;

        Ok(())
    }

    pub async fn list_profiles(&self) -> Result<Vec<Profile>> {
        if !self.config_dir.exists() {
            return Ok(vec![]);
        }

        let current = self.get_current().await.ok();
        let settings = self.read_settings_value().await.ok();
        let metadata_table = settings
            .as_ref()
            .and_then(|value| value.get("profiles"))
            .and_then(|value| value.as_table());
        let mut profiles = vec![];

        let mut entries = fs::read_dir(&self.config_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                let active = current.as_ref() == Some(&name);
                let mut profile = Profile::new(name.clone(), path, active);
                if let Some(table) = metadata_table.and_then(|table| table.get(&name))
                    && let Some(profile_table) = table.as_table() {
                        apply_profile_metadata(
                            &self.credential_store,
                            &mut profile,
                            profile_table,
                        )
                        .await;
                    }
                profiles.push(profile);
            }
        }

        profiles.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(profiles)
    }

    pub async fn delete_profile(&self, profile: &str) -> Result<()> {
        let path = self.config_dir.join(format!("{}.yaml", profile));
        if !path.exists() {
            return Err(MihomoError::NotFound(format!(
                "Profile '{}' not found",
                profile
            )));
        }

        let current = self.get_current().await.ok();
        if current.as_ref() == Some(&profile.to_string()) {
            return Err(MihomoError::Config(
                "Cannot delete the active profile".to_string(),
            ));
        }

        fs::remove_file(path).await?;
        if let Err(err) = delete_subscription_url(&self.credential_store, profile).await {
            log::warn!("failed to delete subscription entry: {err}");
        }
        self.remove_profile_metadata(profile).await?;
        Ok(())
    }

    pub async fn get_profile_metadata(&self, profile: &str) -> Result<Profile> {
        let mut profile_info = Profile::new(profile.to_string(), PathBuf::new(), false);
        let settings = self.read_settings_value().await?;
        if let Some(table) = settings
            .get("profiles")
            .and_then(|value| value.as_table())
            .and_then(|table| table.get(profile))
            .and_then(|value| value.as_table())
        {
            apply_profile_metadata(&self.credential_store, &mut profile_info, table).await;
        }
        Ok(profile_info)
    }

    pub async fn update_profile_metadata(
        &self,
        profile: &str,
        metadata: &Profile,
    ) -> Result<()> {
        let mut settings = self.read_settings_value().await?;
        let root_table = ensure_table(&mut settings)?;
        let profiles_value = root_table
            .entry("profiles".to_string())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
        let profiles_table = ensure_table(profiles_value)?;
        let profile_value = profiles_table
            .entry(profile.to_string())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
        let profile_table = ensure_table(profile_value)?;

        let mut subscription_key = None;
        let subscription_fallback = metadata.subscription_url.clone();
        if let Some(url) = metadata.subscription_url.as_deref() {
            match store_subscription_url(&self.credential_store, profile, url).await {
                Ok(key) => {
                    subscription_key = Some(key);
                }
                Err(err) => {
                    log::warn!("failed to store subscription url securely: {err}");
                }
            }
        } else if let Err(err) = delete_subscription_url(&self.credential_store, profile).await {
            log::warn!("failed to delete subscription url: {err}");
        }
        set_optional_string(profile_table, "subscription_url_key", subscription_key);
        set_optional_string(profile_table, "subscription_url", subscription_fallback);
        set_bool(
            profile_table,
            "auto_update_enabled",
            metadata.auto_update_enabled,
        );
        set_optional_u32(
            profile_table,
            "update_interval_hours",
            metadata.update_interval_hours,
        );
        set_optional_datetime(profile_table, "last_updated", metadata.last_updated);
        set_optional_datetime(profile_table, "next_update", metadata.next_update);

        let content = toml::to_string(&settings)
            .map_err(|e| MihomoError::Config(format!("Failed to serialize config: {}", e)))?;
        if let Some(parent) = self.settings_file.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(&self.settings_file, content).await?;
        Ok(())
    }

    pub async fn set_current(&self, profile: &str) -> Result<()> {
        let path = self.config_dir.join(format!("{}.yaml", profile));
        if !path.exists() {
            return Err(MihomoError::NotFound(format!(
                "Profile '{}' not found",
                profile
            )));
        }

        if let Some(parent) = self.settings_file.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut config = if self.settings_file.exists() {
            let content = fs::read_to_string(&self.settings_file).await?;
            toml::from_str(&content).unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()))
        } else {
            toml::Value::Table(toml::map::Map::new())
        };

        if let toml::Value::Table(ref mut table) = config {
            let default_table = table
                .entry("default".to_string())
                .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));

            if let toml::Value::Table(default) = default_table {
                default.insert(
                    "profile".to_string(),
                    toml::Value::String(profile.to_string()),
                );
            }
        }

        let content = toml::to_string(&config)
            .map_err(|e| MihomoError::Config(format!("Failed to serialize config: {}", e)))?;
        fs::write(&self.settings_file, content).await?;

        Ok(())
    }

    async fn read_settings_value(&self) -> Result<toml::Value> {
        if !self.settings_file.exists() {
            return Ok(toml::Value::Table(toml::map::Map::new()));
        }
        let content = fs::read_to_string(&self.settings_file).await?;
        toml::from_str(&content)
            .map_err(|e| MihomoError::Config(format!("Invalid config: {}", e)))
    }

    async fn remove_profile_metadata(&self, profile: &str) -> Result<()> {
        if !self.settings_file.exists() {
            return Ok(());
        }
        let mut settings = self.read_settings_value().await?;
        if let toml::Value::Table(ref mut root) = settings
            && let Some(toml::Value::Table(profiles)) = root.get_mut("profiles") {
                profiles.remove(profile);
            }
        let content = toml::to_string(&settings)
            .map_err(|e| MihomoError::Config(format!("Failed to serialize config: {}", e)))?;
        fs::write(&self.settings_file, content).await?;
        Ok(())
    }

    pub async fn get_current(&self) -> Result<String> {
        if !self.settings_file.exists() {
            return Ok("default".to_string());
        }

        let content = fs::read_to_string(&self.settings_file).await?;
        let config: toml::Value = toml::from_str(&content)
            .map_err(|e| MihomoError::Config(format!("Invalid config: {}", e)))?;

        Ok(config
            .get("default")
            .and_then(|d| d.get("profile"))
            .and_then(|p| p.as_str())
            .unwrap_or("default")
            .to_string())
    }

    pub async fn get_current_path(&self) -> Result<PathBuf> {
        let profile = self.get_current().await?;
        Ok(self.config_dir.join(format!("{}.yaml", profile)))
    }

    /// Ensure a default config file exists, create one if it doesn't
    pub async fn ensure_default_config(&self) -> Result<()> {
        let profile = self.get_current().await?;
        let path = self.config_dir.join(format!("{}.yaml", profile));

        if !path.exists() {
            log::info!("Default config '{}' not found, creating...", profile);

            let port = find_available_port(9090).ok_or_else(|| {
                MihomoError::Config("No available ports found in range 9090-9190".to_string())
            })?;

            let default_config = format!(
                r#"# mihomo configuration
port: 7890
socks-port: 7891
allow-lan: false
mode: rule
log-level: info
external-controller: 127.0.0.1:{}
"#,
                port
            );

            self.save(&profile, &default_config).await?;
            log::info!("Created default config at: {}", path.display());
        }

        Ok(())
    }

    pub async fn ensure_proxy_ports(&self) -> Result<()> {
        let profile = self.get_current().await?;
        let content = self.load(&profile).await?;
        let mut doc = yaml::load_yaml(&content)?;
        let mut changed = false;

        for key in ["mixed-port", "port", "socks-port"] {
            let port = match yaml::get_u16(&doc, key) {
                Some(port) => port,
                None => continue,
            };
            if port == 0 {
                continue;
            }
            if !is_port_available(port) {
                let fallback = find_available_port(port).ok_or_else(|| {
                    MihomoError::Config(format!("No available port found for {key}"))
                })?;
                if fallback != port {
                    yaml::set_u16(&mut doc, key, fallback)?;
                    log::warn!("{} {} is in use, switched to {}", key, port, fallback);
                    changed = true;
                }
            }
        }

        if changed {
            let updated = yaml::to_string(&doc)?;
            self.save(&profile, &updated).await?;
        }

        Ok(())
    }

    pub async fn get_external_controller(&self) -> Result<String> {
        let profile = self.get_current().await?;
        log::debug!("Reading external-controller from profile: {}", profile);

        let content = self.load(&profile).await?;
        let config = yaml::load_yaml(&content)?;

        let controller = yaml::get_str(&config, "external-controller")
            .unwrap_or_else(|| "127.0.0.1:9090".to_string());

        let url = if controller.starts_with(':') {
            format!("http://127.0.0.1{}", controller)
        } else if controller.starts_with("http://") || controller.starts_with("https://") {
            controller.to_string()
        } else {
            format!("http://{}", controller)
        };

        log::debug!("External controller URL: {}", url);
        Ok(url)
    }

    /// Ensure external-controller is configured in the current profile
    /// If not present or port is occupied, add/update it with an available port
    pub async fn ensure_external_controller(&self) -> Result<String> {
        let profile = self.get_current().await?;
        let content = self.load(&profile).await?;
        let mut config = yaml::load_yaml(&content)?;

        let needs_update = match yaml::get_str(&config, "external-controller") {
            Some(controller) => {
                // Parse the port from the controller address
                let addr = if controller.starts_with(':') {
                    format!("127.0.0.1{}", controller)
                } else {
                    controller.to_string()
                };

                match parse_port_from_addr(&addr) {
                    Some(port) => {
                        if !is_port_available(port) {
                            log::warn!("Port {} is occupied, finding alternative", port);
                            true
                        } else {
                            false
                        }
                    }
                    None => {
                        log::warn!("Invalid external-controller format: {}", controller);
                        true
                    }
                }
            }
            None => {
                log::info!("external-controller not found in config, adding default");
                true
            }
        };

        if needs_update {
            let port = find_available_port(9090).ok_or_else(|| {
                MihomoError::Config("No available ports found in range 9090-9190".to_string())
            })?;

            let controller_addr = format!("127.0.0.1:{}", port);
            log::info!("Setting external-controller to {}", controller_addr);

            yaml::set_str(&mut config, "external-controller", &controller_addr)?;
            let updated_content = yaml::to_string(&config)?;
            self.save(&profile, &updated_content).await?;

            Ok(format!("http://{}", controller_addr))
        } else {
            self.get_external_controller().await
        }
    }

    /// Forcefully rotate the external-controller port to a new available one.
    /// Used when the service fails to start despite the port appearing available initially.
    pub async fn rotate_external_controller(&self) -> Result<String> {
        let profile = self.get_current().await?;
        let content = self.load(&profile).await?;
        let mut config = yaml::load_yaml(&content)?;

        let current_port = yaml::get_str(&config, "external-controller")
            .and_then(|s| parse_port_from_addr(&s))
            .unwrap_or(9090);

        // Start searching from current_port + 1
        let new_port = find_available_port(current_port + 1).ok_or_else(|| {
            MihomoError::Config("No available ports found for rotation".to_string())
        })?;

        let controller_addr = format!("127.0.0.1:{}", new_port);
        log::info!(
            "Rotating external-controller from {} to {}",
            current_port,
            new_port
        );

        yaml::set_str(&mut config, "external-controller", &controller_addr)?;
        let updated_content = yaml::to_string(&config)?;
        self.save(&profile, &updated_content).await?;

        Ok(format!("http://{}", controller_addr))
    }
}

impl ConfigManager<DefaultCredentialStore> {
    pub fn new() -> Result<Self> {
        Self::new_with_store(DefaultCredentialStore::default())
    }

    pub fn with_home(home: PathBuf) -> Result<Self> {
        Self::with_home_and_store(home, DefaultCredentialStore::default())
    }
}

fn ensure_table(
    value: &mut toml::Value,
) -> Result<&mut toml::map::Map<String, toml::Value>> {
    if !matches!(value, toml::Value::Table(_)) {
        *value = toml::Value::Table(toml::map::Map::new());
    }
    match value {
        toml::Value::Table(table) => Ok(table),
        _ => Err(MihomoError::Config(
            "Invalid settings table".to_string(),
        )),
    }
}

async fn apply_profile_metadata<S: CredentialStore>(
    credential_store: &S,
    profile: &mut Profile,
    table: &toml::map::Map<String, toml::Value>,
) {
    let fallback_url = table
        .get("subscription_url")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    let mut key = table
        .get("subscription_url_key")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    if key.is_none() && fallback_url.is_some() {
        key = Some(subscription_key(&profile.name));
    }
    let mut resolved =
        load_subscription_url(credential_store, &profile.name, key.as_deref()).await;
    if resolved.is_none()
        && let Some(url) = fallback_url.as_ref() {
            if let Err(err) =
                store_subscription_url(credential_store, &profile.name, url).await
            {
                log::warn!("failed to restore subscription url to store: {err}");
            } else {
                resolved = Some(url.clone());
            }
        }
    profile.subscription_url = resolved.or(fallback_url);
    profile.auto_update_enabled = table
        .get("auto_update_enabled")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    profile.update_interval_hours = table
        .get("update_interval_hours")
        .and_then(|value| value.as_integer())
        .and_then(|value| {
            if value >= 0 && value <= u32::MAX as i64 {
                Some(value as u32)
            } else {
                None
            }
        });
    profile.last_updated = parse_datetime(table.get("last_updated"));
    profile.next_update = parse_datetime(table.get("next_update"));
}

fn parse_datetime(value: Option<&toml::Value>) -> Option<DateTime<Utc>> {
    value
        .and_then(|value| value.as_str())
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|parsed| parsed.with_timezone(&Utc))
}

const SUBSCRIPTION_SERVICE: &str = "MusicFrog-Despicable-Infiltrator";
const SUBSCRIPTION_KEY_PREFIX: &str = "subscription";

fn subscription_key(profile: &str) -> String {
    format!("{SUBSCRIPTION_KEY_PREFIX}:{profile}")
}

async fn store_subscription_url<S: CredentialStore>(
    credential_store: &S,
    profile: &str,
    url: &str,
) -> Result<String> {
    let key = subscription_key(profile);
    credential_store
        .set(SUBSCRIPTION_SERVICE, &key, url)
        .await?;
    Ok(key)
}

async fn load_subscription_url<S: CredentialStore>(
    credential_store: &S,
    profile: &str,
    key: Option<&str>,
) -> Option<String> {
    let key = match key {
        Some(key) if !key.trim().is_empty() => key.to_string(),
        _ => return None,
    };
    match credential_store.get(SUBSCRIPTION_SERVICE, &key).await {
        Ok(value) => value,
        Err(err) => {
            log::warn!("subscription get failed for profile {}: {err}", profile);
            None
        }
    }
}

async fn delete_subscription_url<S: CredentialStore>(
    credential_store: &S,
    profile: &str,
) -> Result<()> {
    let key = subscription_key(profile);
    credential_store
        .delete(SUBSCRIPTION_SERVICE, &key)
        .await?;
    Ok(())
}

fn set_optional_string(
    table: &mut toml::map::Map<String, toml::Value>,
    key: &str,
    value: Option<String>,
) {
    match value {
        Some(value) => {
            table.insert(key.to_string(), toml::Value::String(value));
        }
        None => {
            table.remove(key);
        }
    }
}

fn set_optional_u32(
    table: &mut toml::map::Map<String, toml::Value>,
    key: &str,
    value: Option<u32>,
) {
    match value {
        Some(value) => {
            table.insert(key.to_string(), toml::Value::Integer(value as i64));
        }
        None => {
            table.remove(key);
        }
    }
}

fn set_bool(table: &mut toml::map::Map<String, toml::Value>, key: &str, value: bool) {
    table.insert(key.to_string(), toml::Value::Boolean(value));
}

fn set_optional_datetime(
    table: &mut toml::map::Map<String, toml::Value>,
    key: &str,
    value: Option<DateTime<Utc>>,
) {
    match value {
        Some(value) => {
            table.insert(key.to_string(), toml::Value::String(value.to_rfc3339()));
        }
        None => {
            table.remove(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    async fn setup_test_manager(temp_dir: &TempDir) -> ConfigManager {
        let home = temp_dir.path().to_path_buf();
        ConfigManager::with_home(home).unwrap()
    }

    #[tokio::test]
    async fn test_manager_new() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        assert_eq!(manager.config_dir, temp_dir.path().join("configs"));
        assert_eq!(manager.settings_file, temp_dir.path().join("config.toml"));
    }

    #[tokio::test]
    async fn test_save_profile_success() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let profile_content = "port: 7890\nsocks-port: 7891\nmode: rule";
        let result = manager.save("test-profile", profile_content).await;

        assert!(result.is_ok());
        let profile_path = manager.config_dir.join("test-profile.yaml");
        assert!(profile_path.exists());

        let content = fs::read_to_string(&profile_path).await.unwrap();
        assert!(content.contains("port: 7890"));
    }

    #[tokio::test]
    async fn test_save_invalid_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let profile_content = "invalid: yaml: content: [";
        let result = manager.save("test-profile", profile_content).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_load_profile_success() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let profile_content = "port: 7890\nsocks-port: 7891\nmode: rule";
        manager.save("test-profile", profile_content).await.unwrap();

        let result = manager.load("test-profile").await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("port: 7890"));
    }

    #[tokio::test]
    async fn test_load_nonexistent_profile() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let result = manager.load("nonexistent").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_list_profiles_empty() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let result = manager.list_profiles().await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_profiles_with_files() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        manager.save("profile1", "port: 7890").await.unwrap();
        manager.save("profile2", "port: 7891").await.unwrap();

        let result = manager.list_profiles().await;
        assert!(result.is_ok());
        let profiles = result.unwrap();
        assert_eq!(profiles.len(), 2);
        assert!(profiles.iter().any(|p| p.name == "profile1"));
        assert!(profiles.iter().any(|p| p.name == "profile2"));
    }

    #[tokio::test]
    async fn test_list_profiles_sorted() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        manager.save("z-profile", "port: 7890").await.unwrap();
        manager.save("a-profile", "port: 7891").await.unwrap();
        manager.save("m-profile", "port: 7892").await.unwrap();

        let result = manager.list_profiles().await;
        assert!(result.is_ok());
        let profiles = result.unwrap();
        assert_eq!(profiles[0].name, "a-profile");
        assert_eq!(profiles[1].name, "m-profile");
        assert_eq!(profiles[2].name, "z-profile");
    }

    #[tokio::test]
    async fn test_delete_profile_success() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        manager.save("test-profile", "port: 7890").await.unwrap();
        assert!(manager.config_dir.join("test-profile.yaml").exists());

        let result = manager.delete_profile("test-profile").await;
        assert!(result.is_ok());
        assert!(!manager.config_dir.join("test-profile.yaml").exists());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_profile() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let result = manager.delete_profile("nonexistent").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_delete_active_profile() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        manager.save("active-profile", "port: 7890").await.unwrap();
        manager.set_current("active-profile").await.unwrap();

        let result = manager.delete_profile("active-profile").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Cannot delete the active profile"));
    }

    #[tokio::test]
    async fn test_set_current_profile() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        manager.save("test-profile", "port: 7890").await.unwrap();

        let result = manager.set_current("test-profile").await;
        assert!(result.is_ok());

        let current = manager.get_current().await.unwrap();
        assert_eq!(current, "test-profile");
    }

    #[tokio::test]
    async fn test_set_nonexistent_current_profile() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let result = manager.set_current("nonexistent").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_get_current_default() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let current = manager.get_current().await.unwrap();
        assert_eq!(current, "default");
    }

    #[tokio::test]
    async fn test_get_current_path() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        manager.save("test-profile", "port: 7890").await.unwrap();
        manager.set_current("test-profile").await.unwrap();

        let path = manager.get_current_path().await.unwrap();
        assert_eq!(path, manager.config_dir.join("test-profile.yaml"));
    }

    #[tokio::test]
    async fn test_ensure_default_config_creates() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let result = manager.ensure_default_config().await;
        assert!(result.is_ok());

        let current = manager.get_current().await.unwrap();
        assert!(manager.config_dir.join(format!("{}.yaml", current)).exists());
    }

    #[tokio::test]
    async fn test_ensure_default_config_exists() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        manager.save("default", "port: 7890").await.unwrap();

        let result = manager.ensure_default_config().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_external_controller() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let config = r#"
port: 7890
external-controller: 127.0.0.1:9090
"#;
        manager.save("default", config).await.unwrap();

        let result = manager.get_external_controller().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "http://127.0.0.1:9090");
    }

    #[tokio::test]
    async fn test_get_external_controller_default() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let config = r#"
port: 7890
"#;
        manager.save("default", config).await.unwrap();

        let result = manager.get_external_controller().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "http://127.0.0.1:9090");
    }

    #[tokio::test]
    async fn test_get_external_controller_with_colon() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let config = r#"
port: 7890
external-controller: ":9090"
"#;
        manager.save("default", config).await.unwrap();

        let result = manager.get_external_controller().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "http://127.0.0.1:9090");
    }

    #[tokio::test]
    async fn test_get_external_controller_with_http() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let config = r#"
port: 7890
external-controller: http://127.0.0.1:9090
"#;
        manager.save("default", config).await.unwrap();

        let result = manager.get_external_controller().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "http://127.0.0.1:9090");
    }

    #[tokio::test]
    async fn test_update_profile_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let metadata = Profile {
            name: "test".to_string(),
            path: PathBuf::new(),
            active: false,
            subscription_url: Some("http://example.com".to_string()),
            auto_update_enabled: true,
            update_interval_hours: Some(24),
            last_updated: None,
            next_update: None,
        };

        let result = manager.update_profile_metadata("test-profile", &metadata).await;
        assert!(result.is_ok());

        let retrieved = manager.get_profile_metadata("test-profile").await.unwrap();
        assert!(retrieved.auto_update_enabled);
        assert_eq!(retrieved.update_interval_hours, Some(24));
    }

    #[tokio::test]
    async fn test_get_profile_metadata_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await;

        let result = manager.get_profile_metadata("nonexistent").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "nonexistent");
    }

    #[tokio::test]
    async fn test_secured_storage_integration() {
        use std::collections::HashMap;
        use std::sync::{Arc, Mutex};
        use async_trait::async_trait;

        #[derive(Default, Clone)]
        struct MockStore {
            data: Arc<Mutex<HashMap<String, String>>>,
        }

        #[async_trait]
        impl CredentialStore for MockStore {
            async fn get(&self, _svc: &str, key: &str) -> mihomo_api::Result<Option<String>> {
                Ok(self.data.lock().unwrap().get(key).cloned())
            }
            async fn set(&self, _svc: &str, key: &str, val: &str) -> mihomo_api::Result<()> {
                self.data.lock().unwrap().insert(key.to_string(), val.to_string());
                Ok(())
            }
            async fn delete(&self, _svc: &str, key: &str) -> mihomo_api::Result<()> {
                self.data.lock().unwrap().remove(key);
                Ok(())
            }
        }

        let temp_dir = TempDir::new().unwrap();
        let store = MockStore::default();
        let manager = ConfigManager::with_home_and_store(temp_dir.path().to_path_buf(), store.clone()).unwrap();

        let mut metadata = Profile::new("test".to_string(), PathBuf::new(), false);
        metadata.subscription_url = Some("https://secret.url/sub".to_string());
        
        // 1. Save metadata
        manager.update_profile_metadata("test", &metadata).await.unwrap();

        // 2. Verify store has the secret
        let key = format!("subscription:test");
        assert_eq!(store.data.lock().unwrap().get(&key).unwrap(), "https://secret.url/sub");

        // 3. Load metadata and verify url is recovered
        let loaded = manager.get_profile_metadata("test").await.unwrap();
        assert_eq!(loaded.subscription_url, Some("https://secret.url/sub".to_string()));
    }
}

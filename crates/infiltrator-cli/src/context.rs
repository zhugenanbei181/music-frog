use std::path::PathBuf;
use std::sync::Arc;

use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_core::settings_store::FileSettingsStore;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::sync_application::SyncApplication;
use infiltrator_domain::settings::AppSettings;
use infiltrator_ports::endpoint::EndpointSource as _;
use infiltrator_ports::subscription_source::SubscriptionSource;
use mihomo_api::client::MihomoClient;
use mihomo_config::endpoint::ProfileEndpointSource;
use mihomo_config::manager::ConfigManager;
use mihomo_platform::defaults::DefaultCredentialStore;
use mihomo_platform::paths::get_home_dir;
use mihomo_version::manager::VersionManager;

/// Per-invocation runtime context: the resolved home directory plus the
/// loaded [`AppSettings`]. Every handler builds its managers through this
/// type so the configs-directory resolution chain (env > settings > default)
/// stays identical across the whole CLI.
pub struct Runtime {
    home: PathBuf,
    pub settings: AppSettings,
}

impl Runtime {
    /// Context for the real installation.
    pub async fn detect() -> anyhow::Result<Self> {
        let home = get_home_dir()?;
        Self::with_home(home).await
    }

    /// Context rooted at an explicit home (tests inject a temp dir).
    pub async fn with_home(home: PathBuf) -> anyhow::Result<Self> {
        let settings = SettingsApplication::new(Arc::new(FileSettingsStore::for_home(
            home.clone(),
        )))
        .load()
        .await
        .map_err(|failure| anyhow::anyhow!(failure.message))?;
        Ok(Self { home, settings })
    }

    pub fn home(&self) -> &std::path::Path {
        &self.home
    }

    #[cfg(test)]
    pub fn settings_file(&self) -> anyhow::Result<PathBuf> {
        Ok(self.home.join("settings.toml"))
    }

    pub fn settings_application(&self) -> SettingsApplication {
        SettingsApplication::new(Arc::new(FileSettingsStore::for_home(self.home.clone())))
    }

    /// ConfigManager following the full configs-directory resolution chain:
    /// `INFILTRATOR_CONFIGS_DIR` env > `settings.configs_dir` >
    /// `<home>/configs`.
    pub fn config_manager(
        &self,
    ) -> mihomo_api::error::Result<ConfigManager<DefaultCredentialStore>> {
        ConfigManager::with_home_configs_dir_and_store(
            self.home.clone(),
            self.settings.configs_dir.as_deref(),
            DefaultCredentialStore::default(),
        )
    }

    pub fn version_manager(&self) -> mihomo_api::error::Result<VersionManager> {
        VersionManager::with_home(self.home.clone())
    }

    /// Resolved configs directory (same chain as [`Self::config_manager`]).
    /// Uses the home-bound resolution so an injected test home works exactly
    /// like the detected one.
    pub fn configs_dir(&self) -> anyhow::Result<PathBuf> {
        Ok(mihomo_config::manager::paths::resolve_configs_dir_in(
            self.settings.configs_dir.as_deref(),
            &self.home,
        )?)
    }

    pub fn profile_application(&self) -> mihomo_api::error::Result<ProfileApplication> {
        Ok(ProfileApplication::new(Arc::new(self.config_manager()?)))
    }

    pub fn subscription_source(&self) -> impl SubscriptionSource {
        infiltrator_core::subscription_io::HttpSubscriptionSource::with_default_clients()
    }

    pub fn sync_application(&self) -> anyhow::Result<SyncApplication> {
        let sync = infiltrator_core::sync_port::FileWebDavSync::new(
            self.home.clone(),
            DefaultCredentialStore::default(),
        );
        Ok(SyncApplication::new(Arc::new(sync)))
    }

    /// Controller API client for the current profile. Endpoint URL and secret
    /// resolution reuse `mihomo_config::endpoint::ProfileEndpointSource`,
    /// the same construction path every frontend takes.
    pub async fn api_client(&self) -> anyhow::Result<MihomoClient> {
        let source = ProfileEndpointSource::new(Arc::new(self.config_manager()?));
        let endpoint = source.resolve().await?;
        Ok(MihomoClient::new(&endpoint.url, endpoint.secret)?)
    }

    /// Apply `update` to the in-memory settings and persist them to the
    /// settings file. The caller's [`Runtime`] keeps its stale snapshot; the
    /// CLI is single-shot so that never matters.
    pub async fn update_settings<F>(&self, update: F) -> anyhow::Result<()>
    where
        F: FnOnce(&mut AppSettings),
    {
        self.settings_application()
            .update(update)
            .await
            .map_err(|failure| anyhow::anyhow!(failure.message))
    }
}

#[cfg(test)]
mod tests {
    use super::Runtime;

    #[tokio::test]
    async fn default_settings_resolve_configs_under_home() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = Runtime::with_home(temp.path().to_path_buf()).await.unwrap();
        assert_eq!(runtime.configs_dir().unwrap(), temp.path().join("configs"));
        assert_eq!(
            runtime.settings_file().unwrap(),
            temp.path().join("settings.toml")
        );
        assert!(runtime.settings.configs_dir.is_none());
    }

    #[tokio::test]
    async fn settings_configs_dir_redirects_the_configs_directory() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = Runtime::with_home(temp.path().to_path_buf()).await.unwrap();
        runtime
            .update_settings(|settings| settings.configs_dir = Some("cloud/profiles".to_string()))
            .await
            .unwrap();

        let reloaded = Runtime::with_home(temp.path().to_path_buf()).await.unwrap();
        assert_eq!(
            reloaded.settings.configs_dir.as_deref(),
            Some("cloud/profiles")
        );
        assert_eq!(
            reloaded.configs_dir().unwrap(),
            temp.path().join("cloud/profiles")
        );
    }
}

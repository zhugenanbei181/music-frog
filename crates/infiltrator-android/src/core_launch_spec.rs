use crate::runtime::CoreLaunchSpecSource;
use async_trait::async_trait;
use infiltrator_contract::android_control::{
    CoreLaunchArguments, CoreLaunchBinary, CoreLaunchConfigurationIdentity, CoreLaunchPath,
    CoreLaunchSpec, CoreLaunchWorkingDirectory,
};
use infiltrator_contract::core_identity::CoreVersion;
use infiltrator_ports::core_process::CoreReadiness;
use infiltrator_ports::endpoint::EndpointSource;
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::readiness::ControllerReadiness;
use mihomo_config::endpoint::ProfileEndpointSource;
use mihomo_config::manager::ConfigManager;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod singbox;

pub(crate) use singbox::AndroidSingBoxProfileLaunchSpecProvider;

pub(crate) struct AndroidMihomoProfileLaunchSpecProvider<S: SecureStore> {
    config: Arc<ConfigManager<S>>,
    working_directory: PathBuf,
}

impl<S: SecureStore> AndroidMihomoProfileLaunchSpecProvider<S> {
    pub(crate) fn new(config: Arc<ConfigManager<S>>, working_directory: PathBuf) -> Self {
        Self {
            config,
            working_directory,
        }
    }
}

#[async_trait]
impl<S: SecureStore> CoreLaunchSpecSource for AndroidMihomoProfileLaunchSpecProvider<S> {
    async fn resolve(&self) -> Result<CoreLaunchSpec, PortError> {
        let engine_id = self.config.engine_id();
        if engine_id.as_str() != "mihomo" {
            return Err(PortError::Failed(
                "Mihomo launch provider received a non-Mihomo profile store".to_owned(),
            ));
        }
        let working_directory = CoreLaunchWorkingDirectory::new(self.working_directory.clone())
            .map_err(config_failure)?;

        self.config
            .migrate_legacy_mihomo_selection()
            .await
            .map_err(config_failure)?;
        self.config
            .ensure_external_controller()
            .await
            .map_err(config_failure)?;
        self.config
            .validate_current_profile()
            .await
            .map_err(config_failure)?;

        let workspace = infiltrator_ports::profile_store::ProfileStore::load_active_workspace(
            self.config.engine_profile_store(),
        )
        .await?;
        let document_identity = workspace.document.identity;
        if document_identity.engine_id != engine_id {
            return Err(PortError::Failed(format!(
                "Android Core launch profile belongs to engine `{}` instead of `{}`",
                document_identity.engine_id.as_str(),
                engine_id.as_str(),
            )));
        }

        let configuration_path = self
            .config
            .existing_profile_document_path(document_identity.profile_id.as_str())
            .await
            .map_err(config_failure)?;
        let launch_arguments = CoreLaunchArguments::Mihomo {
            configuration_path: CoreLaunchPath::new(path_argument(&configuration_path)?)
                .map_err(config_failure)?,
        };
        CoreLaunchSpec::new(
            CoreLaunchConfigurationIdentity {
                engine_id,
                profile_id: document_identity.profile_id,
                document_revision: document_identity.revision,
            },
            CoreLaunchBinary::MihomoBundled,
            working_directory.as_path().to_path_buf(),
            launch_arguments,
        )
        .map_err(config_failure)
    }
}

fn config_failure(error: impl fmt::Display) -> PortError {
    PortError::Failed(format!(
        "Android Core launch configuration is unavailable: {error}"
    ))
}

fn path_argument(path: &Path) -> Result<String, PortError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| PortError::Failed("Android Core launch path is not valid UTF-8".to_owned()))
}

pub(crate) struct AndroidProfileReadiness<S: SecureStore> {
    endpoints: Arc<ProfileEndpointSource<S>>,
}

impl<S: SecureStore> AndroidProfileReadiness<S> {
    pub(crate) fn new(endpoints: Arc<ProfileEndpointSource<S>>) -> Self {
        Self { endpoints }
    }

    async fn current_probe(&self) -> Result<ControllerReadiness, PortError> {
        let endpoint = self.endpoints.resolve().await?;
        Ok(ControllerReadiness::new(endpoint.url, endpoint.secret))
    }
}

#[async_trait]
impl<S: SecureStore> CoreReadiness for AndroidProfileReadiness<S> {
    async fn probe(&self) -> Result<String, PortError> {
        self.current_probe().await?.probe().await
    }

    async fn active_core_version(&self) -> Result<Option<CoreVersion>, PortError> {
        self.current_probe().await?.active_core_version().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestCredentialStore;

    #[async_trait]
    impl SecureStore for TestCredentialStore {
        async fn get(&self, _namespace: &str, _key: &str) -> Result<Option<String>, PortError> {
            Ok(None)
        }

        async fn set(&self, _namespace: &str, _key: &str, _value: &str) -> Result<(), PortError> {
            Ok(())
        }

        async fn delete(&self, _namespace: &str, _key: &str) -> Result<(), PortError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn owner_generates_launch_spec_for_active_profile_and_bootstraps_default_in_rust() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock is after the Unix epoch")
            .as_nanos();
        let home = temp_dir().join(format!("android-launch-spec-{stamp}"));
        fs::create_dir_all(&home).expect("test home is created");
        let config = Arc::new(
            ConfigManager::with_home_and_store(home.clone(), TestCredentialStore)
                .expect("test config manager is created"),
        );
        let working_directory = home.join("android data");
        let provider = AndroidMihomoProfileLaunchSpecProvider::new(
            Arc::clone(&config),
            working_directory.clone(),
        );

        let spec = provider
            .resolve()
            .await
            .expect("owner resolves launch spec");
        let current_profile = config.get_current().await.expect("active profile");
        let config_path = config
            .existing_profile_document_path(&current_profile)
            .await
            .expect("Rust-created default profile exists");
        let workspace = infiltrator_ports::profile_store::ProfileStore::load_active_workspace(
            config.engine_profile_store(),
        )
        .await
        .expect("active workspace readback");

        assert_eq!(spec.configuration_identity().engine_id, config.engine_id());
        assert_eq!(
            spec.configuration_identity().profile_id.as_str(),
            current_profile
        );
        assert_eq!(
            spec.configuration_identity().document_revision,
            workspace.document.identity.revision
        );
        assert_eq!(spec.working_directory().as_path(), working_directory);
        assert_eq!(
            spec.launch_arguments().configuration_path().as_str(),
            config_path.to_string_lossy()
        );
        let generated_config = fs::read_to_string(&config_path)
            .expect("Rust owner writes the selected default profile");
        assert!(generated_config.contains("external-controller:"));

        fs::remove_dir_all(home).expect("test home is removed");
    }

    #[tokio::test]
    async fn owner_rejects_relative_working_directory_before_default_config_bootstrap() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock is after the Unix epoch")
            .as_nanos();
        let home = temp_dir().join(format!("android-launch-invalid-{stamp}"));
        fs::create_dir_all(&home).expect("test home is created");
        let config = Arc::new(
            ConfigManager::with_home_and_store(home.clone(), TestCredentialStore)
                .expect("test config manager is created"),
        );
        let provider = AndroidMihomoProfileLaunchSpecProvider::new(
            Arc::clone(&config),
            PathBuf::from("relative-owner-directory"),
        );

        let default_path = config.config_dir().join("default.yaml");
        assert!(!default_path.exists());
        assert!(provider.resolve().await.is_err());
        assert!(!default_path.exists());

        fs::remove_dir_all(home).expect("test home is removed");
    }
}

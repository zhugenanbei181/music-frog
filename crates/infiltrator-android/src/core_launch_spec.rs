use crate::runtime::CoreLaunchSpecSource;
use async_trait::async_trait;
use infiltrator_contract::android_control::{CoreLaunchConfigurationIdentity, CoreLaunchSpec};
use infiltrator_contract::core_identity::CoreVersion;
use infiltrator_ports::core_process::CoreReadiness;
use infiltrator_ports::endpoint::EndpointSource;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::readiness::ControllerReadiness;
use mihomo_config::endpoint::ProfileEndpointSource;
use mihomo_config::manager::ConfigManager;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(crate) struct AndroidProfileLaunchSpecProvider<S: SecureStore> {
    config: Arc<ConfigManager<S>>,
    working_directory: PathBuf,
}

impl<S: SecureStore> AndroidProfileLaunchSpecProvider<S> {
    pub(crate) fn new(config: Arc<ConfigManager<S>>, working_directory: PathBuf) -> Self {
        Self {
            config,
            working_directory,
        }
    }
}

#[async_trait]
impl<S: SecureStore> CoreLaunchSpecSource for AndroidProfileLaunchSpecProvider<S> {
    async fn resolve(&self) -> Result<CoreLaunchSpec, PortError> {
        if !self.working_directory.is_absolute() {
            return Err(PortError::Failed(
                "Android Core working directory must be an absolute host path".to_owned(),
            ));
        }

        self.config
            .ensure_default_config()
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

        let workspace = self.config.load_active_workspace().await?;
        let document_identity = workspace.document.identity;
        let engine_id = self.config.engine_id();
        if document_identity.engine_id != engine_id {
            return Err(PortError::Failed(format!(
                "Android Core launch profile belongs to engine `{}` instead of `{}`",
                document_identity.engine_id.as_str(),
                engine_id.as_str(),
            )));
        }

        let configuration_path = self
            .config
            .existing_profile_yaml_path(document_identity.profile_id.as_str())
            .await
            .map_err(config_failure)?;
        let working_directory = path_argument(&self.working_directory)?;
        let configuration_path = path_argument(&configuration_path)?;
        Ok(CoreLaunchSpec {
            configuration_identity: CoreLaunchConfigurationIdentity {
                engine_id,
                profile_id: document_identity.profile_id,
                document_revision: document_identity.revision,
            },
            working_directory: self.working_directory.clone(),
            launch_arguments: vec![
                "-d".to_owned(),
                working_directory,
                "-f".to_owned(),
                configuration_path,
            ],
        })
    }
}

fn config_failure(error: impl std::fmt::Display) -> PortError {
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
        let home = std::env::temp_dir().join(format!("android-launch-spec-{stamp}"));
        fs::create_dir_all(&home).expect("test home is created");
        let config = Arc::new(
            ConfigManager::with_home_and_store(home.clone(), TestCredentialStore)
                .expect("test config manager is created"),
        );
        let working_directory = home.join("android-data");
        let provider =
            AndroidProfileLaunchSpecProvider::new(Arc::clone(&config), working_directory.clone());

        let spec = provider
            .resolve()
            .await
            .expect("owner resolves launch spec");
        let current_profile = config.get_current().await.expect("active profile");
        let config_path = config
            .existing_profile_yaml_path(&current_profile)
            .await
            .expect("Rust-created default profile exists");
        let workspace = config
            .load_active_workspace()
            .await
            .expect("active workspace readback");

        assert_eq!(spec.configuration_identity.engine_id, config.engine_id());
        assert_eq!(
            spec.configuration_identity.profile_id.as_str(),
            current_profile
        );
        assert_eq!(
            spec.configuration_identity.document_revision,
            workspace.document.identity.revision
        );
        assert_eq!(spec.working_directory, working_directory);
        assert_eq!(
            spec.launch_arguments,
            vec![
                "-d".to_owned(),
                working_directory.to_string_lossy().into_owned(),
                "-f".to_owned(),
                config_path.to_string_lossy().into_owned(),
            ],
        );
        let generated_config = fs::read_to_string(&config_path)
            .expect("Rust owner writes the selected default profile");
        assert!(generated_config.contains("external-controller:"));

        fs::remove_dir_all(home).expect("test home is removed");
    }
}

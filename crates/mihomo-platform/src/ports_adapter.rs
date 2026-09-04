//! Implementations of the 0.30 runtime-neutral ports.
//!
//! The legacy `traits` module remains available during the migration, but new
//! application code must depend on the ports in `infiltrator-ports` instead.

use async_trait::async_trait;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::core_process::CoreProcess;
use infiltrator_ports::data_dir::DataDirProvider;
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::error::MihomoError;

fn map_mihomo_error(error: MihomoError) -> PortError {
    match error {
        MihomoError::Io(error) => PortError::Io(error.to_string()),
        MihomoError::Http(error) => PortError::Network(error.to_string()),
        MihomoError::WebSocket(error) => PortError::Network(error.to_string()),
        MihomoError::NotFound(message) => PortError::NotFound(message),
        MihomoError::Config(message) | MihomoError::YamlEmit(message) => PortError::Failed(message),
        other => PortError::Failed(other.to_string()),
    }
}

#[cfg(not(target_os = "android"))]
#[async_trait]
impl CoreProcess for crate::desktop::ProcessCoreController {
    async fn start(&self) -> Result<(), PortError> {
        <Self as crate::traits::CoreController>::start(self)
            .await
            .map_err(map_mihomo_error)
    }

    async fn stop(&self) -> Result<(), PortError> {
        <Self as crate::traits::CoreController>::stop(self)
            .await
            .map_err(map_mihomo_error)
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        if <Self as crate::traits::CoreController>::is_running(self).await {
            Ok(CoreLifecycle::Running)
        } else {
            Ok(CoreLifecycle::Stopped)
        }
    }

    fn controller_endpoint(&self) -> Option<String> {
        <Self as crate::traits::CoreController>::controller_url(self)
    }
}

#[cfg(target_os = "android")]
#[async_trait]
impl CoreProcess for crate::android::AndroidCoreController {
    async fn start(&self) -> Result<(), PortError> {
        <Self as crate::traits::CoreController>::start(self)
            .await
            .map_err(map_mihomo_error)
    }

    async fn stop(&self) -> Result<(), PortError> {
        <Self as crate::traits::CoreController>::stop(self)
            .await
            .map_err(map_mihomo_error)
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        if <Self as crate::traits::CoreController>::is_running(self).await {
            Ok(CoreLifecycle::Running)
        } else {
            Ok(CoreLifecycle::Stopped)
        }
    }

    fn controller_endpoint(&self) -> Option<String> {
        <Self as crate::traits::CoreController>::controller_url(self)
    }
}

#[cfg(not(target_os = "android"))]
#[async_trait]
impl SecureStore for crate::desktop::KeyringCredentialStore {
    async fn get(&self, namespace: &str, key: &str) -> Result<Option<String>, PortError> {
        <Self as crate::traits::CredentialStore>::get(self, namespace, key)
            .await
            .map_err(map_mihomo_error)
    }

    async fn set(&self, namespace: &str, key: &str, value: &str) -> Result<(), PortError> {
        <Self as crate::traits::CredentialStore>::set(self, namespace, key, value)
            .await
            .map_err(map_mihomo_error)
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<(), PortError> {
        <Self as crate::traits::CredentialStore>::delete(self, namespace, key)
            .await
            .map_err(map_mihomo_error)
    }
}

#[cfg(target_os = "android")]
#[async_trait]
impl SecureStore for crate::android::AndroidCredentialStore {
    async fn get(&self, namespace: &str, key: &str) -> Result<Option<String>, PortError> {
        <Self as crate::traits::CredentialStore>::get(self, namespace, key)
            .await
            .map_err(map_mihomo_error)
    }

    async fn set(&self, namespace: &str, key: &str, value: &str) -> Result<(), PortError> {
        <Self as crate::traits::CredentialStore>::set(self, namespace, key, value)
            .await
            .map_err(map_mihomo_error)
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<(), PortError> {
        <Self as crate::traits::CredentialStore>::delete(self, namespace, key)
            .await
            .map_err(map_mihomo_error)
    }
}

#[cfg(not(target_os = "android"))]
impl DataDirProvider for crate::desktop::DesktopDataDirProvider {
    fn data_dir(&self) -> Option<std::path::PathBuf> {
        <Self as crate::traits::DataDirProvider>::data_dir(self)
    }
}

#[cfg(target_os = "android")]
impl DataDirProvider for crate::android::AndroidDataDirProvider {
    fn data_dir(&self) -> Option<std::path::PathBuf> {
        <Self as crate::traits::DataDirProvider>::data_dir(self)
    }

    fn cache_dir(&self) -> Option<std::path::PathBuf> {
        <Self as crate::traits::DataDirProvider>::cache_dir(self)
    }
}

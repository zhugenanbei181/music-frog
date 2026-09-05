//! Desktop local-only startup validation.

use async_trait::async_trait;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::offline_startup::{
    LocalAssetStatus, OfflineStartupSnapshot, StartupNetworkPolicy,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::offline_startup::OfflineStartupPort;
use std::path::{Path, PathBuf};

const GEOIP_MIN_SIZE: u64 = 1024 * 1024;

/// Validates only files already owned by the desktop host.  No HTTP client is
/// present in this adapter by design, which makes its offline guarantee
/// structural instead of a best-effort runtime flag.
pub struct DesktopOfflineStartup {
    config_path: PathBuf,
    binary_path: PathBuf,
}

impl DesktopOfflineStartup {
    pub fn new(config_path: PathBuf, binary_path: PathBuf) -> Self {
        Self {
            config_path,
            binary_path,
        }
    }

    async fn binary_available(&self) -> bool {
        tokio::fs::metadata(&self.binary_path)
            .await
            .is_ok_and(|metadata| metadata.is_file())
    }

    async fn geoip_status(&self, config: &str) -> LocalAssetStatus {
        if !config.to_ascii_uppercase().contains("GEOIP") {
            return LocalAssetStatus::NotRequired;
        }

        let mut candidates = Vec::with_capacity(2);
        if let Some(parent) = self.config_path.parent() {
            candidates.push(parent.join("geoip.metadb"));
        }
        if let Some(parent) = self.binary_path.parent() {
            let candidate = parent.join("geoip.metadb");
            if !candidates.contains(&candidate) {
                candidates.push(candidate);
            }
        }

        for candidate in candidates {
            if tokio::fs::metadata(candidate)
                .await
                .is_ok_and(|metadata| metadata.len() >= GEOIP_MIN_SIZE)
            {
                return LocalAssetStatus::Available;
            }
        }
        LocalAssetStatus::Missing
    }

    async fn read_config(&self) -> Result<String, PortError> {
        tokio::fs::read_to_string(&self.config_path)
            .await
            .map_err(|error| {
                PortError::Io(format!(
                    "read local Mihomo profile {}: {error}",
                    self.config_path.display()
                ))
            })
    }
}

#[async_trait]
impl OfflineStartupPort for DesktopOfflineStartup {
    async fn validate_offline_startup(&self) -> Result<OfflineStartupSnapshot, PortError> {
        let binary_available = self.binary_available().await;
        let config = self.read_config().await?;
        if let Err(error) = mihomo_config::yaml::validate(&config) {
            return Ok(OfflineStartupSnapshot {
                policy: StartupNetworkPolicy::OfflineFirst,
                state: infiltrator_contract::offline_startup::OfflineStartupState::Blocked,
                config_valid: false,
                binary_available,
                geoip: LocalAssetStatus::NotRequired,
                remote_dependency:
                    infiltrator_contract::offline_startup::StartupRemoteDependency::Optional,
                failure: Some(Failure::new(
                    ErrorCode::Configuration,
                    format!("local Mihomo profile is invalid: {error}"),
                    false,
                )),
            });
        }

        if !binary_available {
            return Ok(OfflineStartupSnapshot {
                policy: StartupNetworkPolicy::OfflineFirst,
                state: infiltrator_contract::offline_startup::OfflineStartupState::Blocked,
                config_valid: true,
                binary_available: false,
                geoip: LocalAssetStatus::NotRequired,
                remote_dependency:
                    infiltrator_contract::offline_startup::StartupRemoteDependency::Optional,
                failure: Some(Failure::new(
                    ErrorCode::Storage,
                    format!(
                        "local Mihomo core is unavailable: {}",
                        self.binary_path.display()
                    ),
                    false,
                )),
            });
        }

        Ok(OfflineStartupSnapshot::ready(
            self.geoip_status(&config).await,
        ))
    }
}

/// Small constructor used by the desktop surface composition.
pub fn offline_startup_port(
    config_path: impl AsRef<Path>,
    binary_path: impl AsRef<Path>,
) -> DesktopOfflineStartup {
    DesktopOfflineStartup::new(
        config_path.as_ref().to_path_buf(),
        binary_path.as_ref().to_path_buf(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::offline_startup::{OfflineStartupState, StartupRemoteDependency};
    use tempfile::TempDir;

    async fn fixture(binary: bool, config: &str) -> (TempDir, DesktopOfflineStartup) {
        let dir = TempDir::new().expect("temp dir");
        let config_path = dir.path().join("default.yaml");
        tokio::fs::write(&config_path, config)
            .await
            .expect("config");
        let binary_path = dir.path().join(if binary { "mihomo" } else { "missing" });
        if binary {
            tokio::fs::write(&binary_path, b"local core")
                .await
                .expect("binary");
        }
        (dir, DesktopOfflineStartup::new(config_path, binary_path))
    }

    #[tokio::test]
    async fn valid_local_profile_and_binary_are_offline_startable() {
        let (_dir, adapter) = fixture(true, "mixed-port: 7890\nmode: rule\n").await;
        let snapshot = adapter
            .validate_offline_startup()
            .await
            .expect("local validation");
        assert_eq!(snapshot.state, OfflineStartupState::Ready);
        assert!(snapshot.is_offline_startable());
        assert_eq!(
            snapshot.remote_dependency,
            StartupRemoteDependency::Optional
        );
    }

    #[tokio::test]
    async fn missing_geoip_is_visible_but_does_not_block_offline_boot() {
        let (_dir, adapter) = fixture(true, "geoip: true\nmode: rule\n").await;
        let snapshot = adapter.validate_offline_startup().await.unwrap();
        assert_eq!(snapshot.geoip, LocalAssetStatus::Missing);
        assert_eq!(snapshot.state, OfflineStartupState::Degraded);
        assert!(snapshot.is_offline_startable());
    }

    #[tokio::test]
    async fn malformed_profile_fails_closed_without_network_fallback() {
        let (_dir, adapter) = fixture(true, "invalid: yaml: [").await;
        let snapshot = adapter.validate_offline_startup().await.unwrap();
        assert_eq!(snapshot.state, OfflineStartupState::Blocked);
        assert!(!snapshot.config_valid);
        assert!(!snapshot.is_offline_startable());
        assert_eq!(
            snapshot.remote_dependency,
            StartupRemoteDependency::Optional
        );
    }

    #[tokio::test]
    async fn missing_binary_blocks_even_when_profile_is_valid() {
        let (_dir, adapter) = fixture(false, "mode: rule\n").await;
        let snapshot = adapter.validate_offline_startup().await.unwrap();
        assert_eq!(snapshot.state, OfflineStartupState::Blocked);
        assert!(snapshot.config_valid);
        assert!(!snapshot.binary_available);
    }
}

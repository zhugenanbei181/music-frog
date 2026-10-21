//! WebDAV synchronization use-cases over a host-provided port.

use infiltrator_contract::sync::{SyncReport, SyncTransferReport};
use infiltrator_domain::settings::WebDavConfig;
use infiltrator_ports::sync::{
    SyncConflictSnapshot, SyncPort, SyncProgressSink, SyncRequest, SyncTransferRequest,
};
use lurker_core_contract::error::{ErrorCode, Failure};
use std::sync::Arc;

#[derive(Clone)]
pub struct SyncApplication {
    port: Arc<dyn SyncPort>,
}

impl SyncApplication {
    pub fn new(port: Arc<dyn SyncPort>) -> Self {
        Self { port }
    }

    pub async fn test(&self, config: WebDavConfig) -> Result<usize, Failure> {
        if config.url.trim().is_empty() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "WebDAV URL is empty",
                false,
            ));
        }
        self.port.test(config).await.map_err(Failure::from)
    }

    pub async fn sync(
        &self,
        config: WebDavConfig,
        configs_dir: Option<String>,
    ) -> Result<SyncReport, Failure> {
        if config.enabled && config.url.trim().is_empty() {
            return Err(Failure::new(
                ErrorCode::Configuration,
                "WebDAV URL is empty",
                false,
            ));
        }
        self.port
            .sync(SyncRequest {
                config,
                configs_dir,
            })
            .await
            .map_err(Failure::from)
    }

    pub async fn upload(
        &self,
        config: WebDavConfig,
        configs_dir: Option<String>,
        observer: Arc<dyn SyncProgressSink>,
    ) -> Result<SyncTransferReport, Failure> {
        self.port
            .upload(SyncTransferRequest {
                config,
                configs_dir,
                observer,
            })
            .await
            .map_err(Failure::from)
    }

    pub async fn download(
        &self,
        config: WebDavConfig,
        configs_dir: Option<String>,
        observer: Arc<dyn SyncProgressSink>,
    ) -> Result<SyncTransferReport, Failure> {
        self.port
            .download(SyncTransferRequest {
                config,
                configs_dir,
                observer,
            })
            .await
            .map_err(Failure::from)
    }

    pub async fn read_conflict(
        &self,
        configs_dir: String,
        remote_path: String,
    ) -> Result<String, Failure> {
        self.port
            .read_conflict(configs_dir, remote_path)
            .await
            .map_err(Failure::from)
    }

    pub async fn delete_conflict(
        &self,
        configs_dir: String,
        remote_path: String,
    ) -> Result<(), Failure> {
        self.port
            .delete_conflict(configs_dir, remote_path)
            .await
            .map_err(Failure::from)
    }

    /// The conflict the owner currently holds, if any.
    ///
    /// The sync owner is the only source of this fact: an adapter that cannot
    /// report its pending conflict answers with a typed failure instead of
    /// claiming that no conflict exists.
    pub async fn current_conflict(&self) -> Result<Option<SyncConflictSnapshot>, Failure> {
        self.port.current_conflict().await.map_err(Failure::from)
    }

    /// Read the remote copy of the owner's current conflict.
    pub async fn read_conflict_content(
        &self,
        configs_dir: Option<&str>,
        conflict: &SyncConflictSnapshot,
    ) -> Result<String, Failure> {
        let configs_dir = conflict_configs_dir(configs_dir)?;
        self.port
            .read_conflict(configs_dir, conflict.conflict.remote_path.clone())
            .await
            .map_err(Failure::from)
    }

    /// Discard the remote copy of the owner's current conflict.
    pub async fn discard_conflict(
        &self,
        configs_dir: Option<&str>,
        conflict: &SyncConflictSnapshot,
    ) -> Result<(), Failure> {
        let configs_dir = conflict_configs_dir(configs_dir)?;
        self.port
            .delete_conflict(configs_dir, conflict.conflict.remote_path.clone())
            .await
            .map_err(Failure::from)
    }
}

/// The configuration root the owner's conflict copies live in.
///
/// A host that does not expose one cannot address a conflict copy, so the
/// resolution is refused with a typed failure instead of guessing a path.
fn conflict_configs_dir(configs_dir: Option<&str>) -> Result<String, Failure> {
    let configs_dir = configs_dir
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            Failure::new(
                ErrorCode::NotReady,
                "the configuration directory is unknown, so no conflict copy can be addressed",
                false,
            )
        })?;
    Ok(configs_dir.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::sync::SyncReport;
    use lurker_core_ports::error::PortError;

    struct FakeSync;

    #[async_trait]
    impl SyncPort for FakeSync {
        async fn test(&self, _config: WebDavConfig) -> Result<usize, PortError> {
            Ok(3)
        }

        async fn sync(&self, _request: SyncRequest) -> Result<SyncReport, PortError> {
            Ok(SyncReport {
                success_count: 2,
                total_actions: 2,
                ..SyncReport::default()
            })
        }

        async fn upload(
            &self,
            _request: SyncTransferRequest,
        ) -> Result<SyncTransferReport, PortError> {
            Ok(SyncTransferReport::default())
        }

        async fn download(
            &self,
            _request: SyncTransferRequest,
        ) -> Result<SyncTransferReport, PortError> {
            Ok(SyncTransferReport::default())
        }

        async fn read_conflict(
            &self,
            _configs_dir: String,
            _remote_path: String,
        ) -> Result<String, PortError> {
            Ok("mode: rule\n".to_string())
        }

        async fn delete_conflict(
            &self,
            _configs_dir: String,
            _remote_path: String,
        ) -> Result<(), PortError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn forwards_test_and_sync_through_the_port() {
        let application = SyncApplication::new(Arc::new(FakeSync));
        let config = WebDavConfig {
            url: "http://example.com".to_string(),
            ..WebDavConfig::default()
        };
        assert_eq!(application.test(config).await.unwrap(), 3);
        let report = application
            .sync(WebDavConfig::default(), None)
            .await
            .expect("sync");
        assert_eq!(report.success_count, 2);
        assert_eq!(report.total_actions, 2);
    }
}

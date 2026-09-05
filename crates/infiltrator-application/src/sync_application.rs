//! WebDAV synchronization use-cases over a host-provided port.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::sync::SyncReport;
use infiltrator_domain::settings::WebDavConfig;
use infiltrator_contract::sync::SyncTransferReport;
use infiltrator_ports::sync::{SyncPort, SyncProgressSink, SyncRequest, SyncTransferRequest};
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
                runtime_present: false,
                observer,
            })
            .await
            .map_err(Failure::from)
    }

    pub async fn download(
        &self,
        config: WebDavConfig,
        configs_dir: Option<String>,
        runtime_present: bool,
        observer: Arc<dyn SyncProgressSink>,
    ) -> Result<SyncTransferReport, Failure> {
        self.port
            .download(SyncTransferRequest {
                config,
                configs_dir,
                runtime_present,
                observer,
            })
            .await
            .map_err(Failure::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::sync::SyncReport;
    use infiltrator_ports::error::PortError;

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

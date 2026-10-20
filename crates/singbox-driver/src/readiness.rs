//! Loopback-only readiness and live version observation for sing-box.

use crate::api::clash_api::{SingBoxClashApiClient, SingBoxCoreVersionProvider};
use crate::api::controller_configuration::SingBoxControllerConfiguration;
use crate::api::http::{ReqwestSingBoxClashHttpPort, SingBoxClashHttpPort};
use async_trait::async_trait;
use lurker_core_contract::identity::CoreVersion;
use lurker_core_ports::core_process::CoreReadiness;
use lurker_core_ports::error::PortError;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{Instant, sleep};

const RETRY_INTERVAL: Duration = Duration::from_millis(100);

enum ReadinessWaitState {
    Probe,
    Retry(PortError),
    Ready,
}

/// The loopback readiness probe of the sing-box Clash API.
pub struct SingBoxControllerReadiness {
    api: SingBoxClashApiClient,
    version_provider: SingBoxCoreVersionProvider,
}

impl SingBoxControllerReadiness {
    /// Creates the readiness probe with the production HTTP boundary.
    pub fn with_reqwest(controller: SingBoxControllerConfiguration) -> Result<Self, PortError> {
        Self::new(controller, Arc::new(ReqwestSingBoxClashHttpPort))
    }

    /// Creates the readiness probe with an injected HTTP boundary.
    pub fn new(
        controller: SingBoxControllerConfiguration,
        http: Arc<dyn SingBoxClashHttpPort>,
    ) -> Result<Self, PortError> {
        let api = SingBoxClashApiClient::new(controller, http)?;
        let version_provider = SingBoxCoreVersionProvider::new(api.clone());
        Ok(Self {
            api,
            version_provider,
        })
    }

    /// Waits until the controller answers, or the timeout elapses.
    ///
    /// The last failure is returned when the bound elapses; the probe never
    /// reports readiness it did not observe.
    pub async fn wait_until_ready(&self, timeout: Duration) -> Result<(), PortError> {
        let deadline = Instant::now() + timeout;
        let mut state = ReadinessWaitState::Probe;
        loop {
            state = match state {
                ReadinessWaitState::Probe => match self.probe().await {
                    Ok(_) => ReadinessWaitState::Ready,
                    Err(error) => ReadinessWaitState::Retry(error),
                },
                ReadinessWaitState::Retry(error) => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return Err(error);
                    }
                    sleep(remaining.min(RETRY_INTERVAL)).await;
                    ReadinessWaitState::Probe
                }
                ReadinessWaitState::Ready => return Ok(()),
            };
        }
    }
}

#[async_trait]
impl CoreReadiness for SingBoxControllerReadiness {
    async fn probe(&self) -> Result<String, PortError> {
        let _ = self.version_provider.observe().await?;
        Ok(self
            .api
            .endpoint()
            .as_str()
            .trim_end_matches('/')
            .to_owned())
    }

    async fn active_core_version(&self) -> Result<Option<CoreVersion>, PortError> {
        self.version_provider.observe().await.map(Some)
    }
}

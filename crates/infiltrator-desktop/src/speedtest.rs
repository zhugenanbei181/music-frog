//! Desktop adapter exposing the shared speedtest engine through the
//! contract-only [`SpeedtestPort`] seam.
//!
//! Inbound UI code uses this port to trigger probes and read the canonical
//! snapshot; it never constructs a Mihomo client or fabricates metrics.

use async_trait::async_trait;
use infiltrator_application::speedtest_application::SpeedtestApplication;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::error::Failure;
use infiltrator_contract::speedtest::{SpeedtestScope, SpeedtestSnapshot};
use infiltrator_ports::error::PortError;
use infiltrator_ports::speedtest::SpeedtestPort;

/// Wraps the application-owned engine so a UI surface can drive it.
#[derive(Clone)]
pub struct DesktopSpeedtestPort {
    application: SpeedtestApplication,
}

impl DesktopSpeedtestPort {
    pub fn new(application: SpeedtestApplication) -> Self {
        Self { application }
    }
}

fn map_failure(error: Failure) -> PortError {
    match error.code {
        infiltrator_contract::error::ErrorCode::Unsupported => {
            PortError::unsupported(Capability::Speedtest, error.message)
        }
        infiltrator_contract::error::ErrorCode::Permission => {
            PortError::PermissionDenied(error.message)
        }
        infiltrator_contract::error::ErrorCode::Storage => PortError::NotFound(error.message),
        infiltrator_contract::error::ErrorCode::Network => PortError::Network(error.message),
        _ => PortError::Failed(error.message),
    }
}

#[async_trait]
impl SpeedtestPort for DesktopSpeedtestPort {
    fn snapshot(&self) -> SpeedtestSnapshot {
        self.application.snapshot()
    }

    async fn run_scope(
        &self,
        scope: SpeedtestScope,
        url: Option<String>,
        timeout_ms: Option<u32>,
    ) -> Result<SpeedtestSnapshot, PortError> {
        self.application
            .test_delays(scope, url, timeout_ms)
            .await
            .map_err(map_failure)
    }

    async fn probe_node(
        &self,
        node: &str,
        rounds: usize,
        url: Option<String>,
        timeout_ms: Option<u32>,
    ) -> Result<SpeedtestSnapshot, PortError> {
        // The engine records the jitter probe into its snapshot; return the
        // snapshot so callers read one canonical read model.
        self.application
            .probe_node_jitter(node, rounds, url, timeout_ms)
            .await
            .map(|_| self.application.snapshot())
            .map_err(map_failure)
    }

    fn cancel(&self) -> bool {
        self.application.cancel()
    }
}

/// Typed unsupported adapter for hosts without a speedtest engine.
pub struct UnsupportedSpeedtestPort;

#[async_trait]
impl SpeedtestPort for UnsupportedSpeedtestPort {
    fn snapshot(&self) -> SpeedtestSnapshot {
        SpeedtestSnapshot::default()
    }

    async fn run_scope(
        &self,
        _scope: SpeedtestScope,
        _url: Option<String>,
        _timeout_ms: Option<u32>,
    ) -> Result<SpeedtestSnapshot, PortError> {
        Err(PortError::unsupported(
            Capability::Speedtest,
            "speedtest engine is not available on this host",
        ))
    }

    async fn probe_node(
        &self,
        _node: &str,
        _rounds: usize,
        _url: Option<String>,
        _timeout_ms: Option<u32>,
    ) -> Result<SpeedtestSnapshot, PortError> {
        Err(PortError::unsupported(
            Capability::Speedtest,
            "speedtest engine is not available on this host",
        ))
    }

    fn cancel(&self) -> bool {
        false
    }
}

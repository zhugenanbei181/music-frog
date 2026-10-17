//! Isolated controller process used by deterministic log and telemetry tests.
use async_trait::async_trait;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::identity::{CoreIdentity, CoreInstanceId, CoreVersion, EngineId};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use lurker_core_ports::error::PortError;
use std::sync::RwLock;

pub const LOG_QUERY: &str = "api\\.example|timeout";
pub const LOG_RECORDS: [&str; 3] = [
    "WARN[12:00:01] [TCP] API.EXAMPLE dial TIMEOUT",
    "INFO[12:00:02] [TCP] connected api.example safely",
    "DEBUG[12:00:03] resolver healthy",
];

pub struct LogCaptureProcess(RwLock<CoreLifecycle>);

impl Default for LogCaptureProcess {
    fn default() -> Self {
        Self(RwLock::new(CoreLifecycle::Stopped))
    }
}

#[async_trait]
impl CoreProcess for LogCaptureProcess {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            EngineId::new("log-capture").expect("valid test engine"),
            CoreInstanceId::new("isolated").expect("valid test instance"),
            Some(CoreVersion::new("1.19.12").expect("valid test version")),
        ))
    }

    async fn start(&self, _context: CoreProcessStartContext) -> Result<(), PortError> {
        *self.0.write().expect("log capture lifecycle lock") = CoreLifecycle::Running;
        Ok(())
    }

    async fn stop(&self) -> Result<(), PortError> {
        *self.0.write().expect("log capture lifecycle lock") = CoreLifecycle::Stopped;
        Ok(())
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(self.0.read().expect("log capture lifecycle lock").clone())
    }

    fn controller_endpoint(&self) -> Option<String> {
        Some("http://isolated-controller.invalid".into())
    }
}

#[async_trait]
impl CoreReadiness for LogCaptureProcess {
    async fn probe(&self) -> Result<String, PortError> {
        match self.0.read().expect("log capture lifecycle lock").clone() {
            CoreLifecycle::Running => Ok("isolated-log-controller".into()),
            lifecycle => Err(PortError::Failed(format!(
                "isolated controller is not ready: {lifecycle:?}"
            ))),
        }
    }

    async fn active_core_version(&self) -> Result<Option<CoreVersion>, PortError> {
        Ok(Some(
            CoreVersion::new("1.19.12").expect("valid fixture controller version"),
        ))
    }
}

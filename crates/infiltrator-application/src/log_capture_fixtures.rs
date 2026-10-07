//! Isolated controller records pass through the actual application lifecycle and buffer.
use crate::core_application::CoreApplication;
use async_trait::async_trait;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::logs::LogSession;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::error::PortError;
use infiltrator_ports::runtime_gateway::RuntimeStreamEvent;
use std::sync::atomic::{AtomicBool, Ordering};

pub const LOG_QUERY: &str = "api\\.example|timeout";
pub const LOG_RECORDS: [&str; 3] = [
    "WARN[12:00:01] [TCP] API.EXAMPLE dial TIMEOUT",
    "INFO[12:00:02] [TCP] connected api.example safely",
    "DEBUG[12:00:03] resolver healthy",
];
#[derive(Default)]
pub struct LogCaptureProcess(AtomicBool);
#[async_trait]
impl CoreProcess for LogCaptureProcess {
    async fn start(&self) -> Result<(), PortError> {
        self.0.store(true, Ordering::Release);
        Ok(())
    }
    async fn stop(&self) -> Result<(), PortError> {
        self.0.store(false, Ordering::Release);
        Ok(())
    }
    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(if self.0.load(Ordering::Acquire) {
            CoreLifecycle::Running
        } else {
            CoreLifecycle::Stopped
        })
    }
    fn controller_endpoint(&self) -> Option<String> {
        Some("http://isolated-controller.invalid".into())
    }
}
#[async_trait]
impl CoreReadiness for LogCaptureProcess {
    async fn probe(&self) -> Result<String, PortError> {
        if self.0.load(Ordering::Acquire) {
            Ok("isolated-log-controller".into())
        } else {
            Err(PortError::Failed("isolated controller is stopped".into()))
        }
    }
}
pub async fn populate_logs(core: &CoreApplication) -> Result<(), Failure> {
    core.execute(CommandIntent::StartCore).await.into_unit()?;
    let snapshot = core.snapshot();
    let token = snapshot.session_token.ok_or_else(|| {
        Failure::new(
            ErrorCode::InvalidState,
            "isolated log controller has no session",
            false,
        )
    })?;
    let scope = LogSession {
        generation: snapshot.generation,
        token,
    };
    let logs = core.log_application();
    logs.bind(Some(scope));
    logs.ingest(scope, RuntimeStreamEvent::Connected);
    for record in LOG_RECORDS {
        logs.ingest(scope, RuntimeStreamEvent::Item(record.into()));
    }
    Ok(())
}

pub fn append_follow_records(core: &CoreApplication) -> Result<(), Failure> {
    let snapshot = core.snapshot();
    let scope = LogSession {
        generation: snapshot.generation,
        token: snapshot
            .session_token
            .ok_or_else(|| Failure::new(ErrorCode::NotReady, "no isolated log session", false))?,
    };
    for index in 4..=35 {
        if !core.log_application().ingest(
            scope,
            RuntimeStreamEvent::Item(format!(
                "INFO[12:01:{index:02}] continued controller record {index}"
            )),
        ) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "isolated log session changed",
                false,
            ));
        }
    }
    Ok(())
}

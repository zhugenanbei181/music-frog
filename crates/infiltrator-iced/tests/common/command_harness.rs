//! test-intent: behavior
//! Hostless command application shared by UI behavior tests.
use infiltrator_application::command_application::{CommandFuture, CommandHandler};
use infiltrator_application::core_application::CoreApplication;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::error::PortError;
use std::sync::{Arc, Mutex};

pub struct HostlessProcess;

#[async_trait::async_trait]
impl CoreProcess for HostlessProcess {
    async fn start(&self) -> Result<(), PortError> {
        Err(PortError::Failed("unexpected lifecycle command".into()))
    }
    async fn stop(&self) -> Result<(), PortError> {
        Err(PortError::Failed("unexpected lifecycle command".into()))
    }
    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(CoreLifecycle::Stopped)
    }
    fn controller_endpoint(&self) -> Option<String> {
        None
    }
}

#[async_trait::async_trait]
impl CoreReadiness for HostlessProcess {
    async fn probe(&self) -> Result<String, PortError> {
        Err(PortError::Failed("unexpected readiness probe".into()))
    }
}

#[derive(Default)]
pub struct RecordingHandler(pub Mutex<Vec<CommandIntent>>, pub Mutex<Option<Failure>>);

impl CommandHandler for RecordingHandler {
    fn handle(&self, intent: CommandIntent) -> CommandFuture {
        self.0.lock().unwrap().push(intent);
        let failure = self.1.lock().unwrap().clone();
        Box::pin(async move {
            match failure {
                Some(failure) => Err(failure),
                None => Ok(()),
            }
        })
    }
}

pub fn recording_application() -> (CoreApplication, Arc<RecordingHandler>) {
    let handler = Arc::new(RecordingHandler::default());
    let application = CoreApplication::new(
        Arc::new(HostlessProcess),
        Arc::new(HostlessProcess),
        infiltrator_composition::tokio_application_runtime().unwrap(),
    );
    application.install_command_handler(handler.clone());
    (application, handler)
}

pub fn rejecting_application() -> (CoreApplication, Arc<RecordingHandler>) {
    let (application, handler) = recording_application();
    *handler.1.lock().unwrap() = Some(Failure::new(
        ErrorCode::InvalidInput,
        "profile is read-only",
        false,
    ));
    (application, handler)
}

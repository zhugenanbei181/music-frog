//! Identified, hostless lifecycle fixture for capability-gated UI tests.
use async_trait::async_trait;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_test_support::language_choice_fixtures::LanguageCaptureProcess;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::identity::{CoreIdentity, CoreInstanceId, CoreVersion, EngineId};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use lurker_core_ports::error::PortError;
use std::sync::Arc;

struct IdentifiedLanguageCaptureProcess;

#[async_trait]
impl CoreProcess for IdentifiedLanguageCaptureProcess {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            EngineId::new("mihomo").expect("valid Mihomo test engine"),
            CoreInstanceId::new("iced-test-core").expect("valid test core instance"),
            Some(CoreVersion::new("1.19.12").expect("valid test core version")),
        ))
    }

    async fn start(&self, context: CoreProcessStartContext) -> Result<(), PortError> {
        LanguageCaptureProcess.start(context).await
    }

    async fn stop(&self) -> Result<(), PortError> {
        LanguageCaptureProcess.stop().await
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        LanguageCaptureProcess.status().await
    }

    fn controller_endpoint(&self) -> Option<String> {
        LanguageCaptureProcess.controller_endpoint()
    }
}

#[async_trait]
impl CoreReadiness for IdentifiedLanguageCaptureProcess {
    async fn probe(&self) -> Result<String, PortError> {
        LanguageCaptureProcess.probe().await
    }
}

pub(crate) fn identified_core_application() -> CoreApplication {
    let process = Arc::new(IdentifiedLanguageCaptureProcess);
    CoreApplication::new(
        process.clone(),
        process,
        infiltrator_composition::tokio_application_runtime().unwrap(),
    )
}

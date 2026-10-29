//! Isolated core process and readiness fixtures for lifecycle captures.
//!
//! The fixture never launches a process. It publishes the neutral lifecycle
//! vocabulary a surface consumes and it fails closed: a core that never
//! became ready answers the readiness probe with a typed failure instead of a
//! fabricated success.

use async_trait::async_trait;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreIdentity, CoreInstanceId, CoreVersion, EngineId};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use lurker_core_ports::error::PortError;
use std::sync::Mutex;

/// How the isolated core behaves when a surface drives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreCaptureBehaviour {
    /// The core starts, answers readiness probes and serves traffic.
    Ready,
    /// The core never leaves `Starting`; every readiness probe keeps failing.
    StalledOnStart,
    /// The launch itself fails; the core never runs.
    FailedLaunch,
}

/// The engine the isolated core pretends to serve.
pub const CAPTURE_ENGINE: &str = "mihomo";
/// The core instance the isolated core answers for.
pub const CAPTURE_INSTANCE: &str = "capture-core";
/// The core version the isolated core reports once it is ready.
pub const CAPTURE_CORE_VERSION: &str = "1.19.30";

/// An isolated core process that publishes lifecycle and readiness facts.
pub struct CoreCaptureProcess {
    identity: CoreIdentity,
    behaviour: CoreCaptureBehaviour,
    lifecycle: Mutex<CoreLifecycle>,
    starts: Mutex<usize>,
}

impl Default for CoreCaptureProcess {
    fn default() -> Self {
        Self::new(CAPTURE_ENGINE, CAPTURE_INSTANCE)
    }
}

impl CoreCaptureProcess {
    pub fn new(engine: &str, instance: &str) -> Self {
        Self {
            identity: CoreIdentity::new(
                EngineId::new(engine).expect("valid capture engine ID"),
                CoreInstanceId::new(instance).expect("valid capture instance ID"),
                Some(CoreVersion::new(CAPTURE_CORE_VERSION).expect("valid capture core version")),
            ),
            behaviour: CoreCaptureBehaviour::Ready,
            lifecycle: Mutex::new(CoreLifecycle::Stopped),
            starts: Mutex::new(0),
        }
    }

    /// Constructs an isolated core process that is already in the `Running` state.
    pub fn running(engine: &str, instance: &str) -> Self {
        Self {
            identity: CoreIdentity::new(
                EngineId::new(engine).expect("valid capture engine ID"),
                CoreInstanceId::new(instance).expect("valid capture instance ID"),
                Some(CoreVersion::new(CAPTURE_CORE_VERSION).expect("valid capture core version")),
            ),
            behaviour: CoreCaptureBehaviour::Ready,
            lifecycle: Mutex::new(CoreLifecycle::Running),
            starts: Mutex::new(1),
        }
    }

    /// Selects the behaviour the capture needs.
    pub fn with_behaviour(mut self, behaviour: CoreCaptureBehaviour) -> Self {
        self.behaviour = behaviour;
        self
    }

    /// The lifecycle the isolated core currently publishes.
    pub fn lifecycle(&self) -> CoreLifecycle {
        self.lifecycle
            .lock()
            .expect("capture core lifecycle")
            .clone()
    }

    /// How many launches the surface requested.
    pub fn start_count(&self) -> usize {
        *self.starts.lock().expect("capture core start counter")
    }

    fn set_lifecycle(&self, lifecycle: CoreLifecycle) {
        *self.lifecycle.lock().expect("capture core lifecycle") = lifecycle;
    }
}

#[async_trait]
impl CoreProcess for CoreCaptureProcess {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(self.identity.clone())
    }

    async fn start(&self, context: CoreProcessStartContext) -> Result<(), PortError> {
        if context.identity().engine != self.identity.engine {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                format!(
                    "capture core owns engine {} and cannot serve {}",
                    self.identity.engine.as_str(),
                    context.identity().engine.as_str()
                ),
                false,
            )));
        }
        *self.starts.lock().expect("capture core start counter") += 1;
        match self.behaviour {
            CoreCaptureBehaviour::Ready => {
                self.set_lifecycle(CoreLifecycle::Running);
                Ok(())
            }
            CoreCaptureBehaviour::StalledOnStart => {
                self.set_lifecycle(CoreLifecycle::Starting);
                Ok(())
            }
            CoreCaptureBehaviour::FailedLaunch => Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "capture core failed to launch",
                false,
            ))),
        }
    }

    async fn stop(&self) -> Result<(), PortError> {
        self.set_lifecycle(CoreLifecycle::Stopped);
        Ok(())
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(self.lifecycle())
    }

    fn controller_endpoint(&self) -> Option<String> {
        match self.lifecycle() {
            CoreLifecycle::Running | CoreLifecycle::Ready => {
                Some("http://isolated-core.invalid".to_owned())
            }
            CoreLifecycle::Stopped
            | CoreLifecycle::Starting
            | CoreLifecycle::Stopping
            | CoreLifecycle::Failed => None,
        }
    }

    async fn pid(&self) -> Option<u32> {
        match self.lifecycle() {
            CoreLifecycle::Running | CoreLifecycle::Ready => Some(4242),
            CoreLifecycle::Stopped
            | CoreLifecycle::Starting
            | CoreLifecycle::Stopping
            | CoreLifecycle::Failed => None,
        }
    }
}

#[async_trait]
impl CoreReadiness for CoreCaptureProcess {
    async fn probe(&self) -> Result<String, PortError> {
        match self.lifecycle() {
            CoreLifecycle::Running | CoreLifecycle::Ready => {
                Ok("isolated-core-controller".to_owned())
            }
            CoreLifecycle::Starting => Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "capture core never left the starting state",
                true,
            ))),
            CoreLifecycle::Stopping => Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "capture core is stopping",
                true,
            ))),
            CoreLifecycle::Stopped => Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "capture core is stopped",
                true,
            ))),
            CoreLifecycle::Failed => Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "capture core failed to start",
                false,
            ))),
        }
    }

    async fn active_core_version(&self) -> Result<Option<CoreVersion>, PortError> {
        Ok(self.identity.version.clone())
    }
}

//! Recording switch host used by the neutral switch conformance flow.

use super::super::readiness::wait_for_readiness;
use crate::switch::host::EngineSwitchHost;
use async_trait::async_trait;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::engine_switch::EngineSwitchTarget;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, EngineId};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_ports::core_process::CoreProcess;
use lurker_core_ports::engine_backend::EngineBackendPorts;
use lurker_core_ports::error::PortError;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SwitchHostEvent {
    ValidateTarget,
    PrepareCandidate,
    OldOwnerStopRequested,
    OldOwnerTerminalObserved,
    NewOwnerStartRequested,
    NewOwnerReady,
    CloseCandidate,
}

pub(super) struct ConformanceSwitchHost {
    old_engine: EngineId,
    old_process: Arc<dyn CoreProcess>,
    old_identity: CoreIdentity,
    old_generation: CoreGeneration,
    readiness_timeout: Duration,
    trace: Arc<Mutex<Vec<SwitchHostEvent>>>,
}

impl ConformanceSwitchHost {
    pub(super) fn new(
        old_engine: EngineId,
        old_process: Arc<dyn CoreProcess>,
        old_identity: CoreIdentity,
        old_generation: CoreGeneration,
        readiness_timeout: Duration,
        trace: Arc<Mutex<Vec<SwitchHostEvent>>>,
    ) -> Self {
        Self {
            old_engine,
            old_process,
            old_identity,
            old_generation,
            readiness_timeout,
            trace,
        }
    }

    fn record(&self, event: SwitchHostEvent) {
        self.trace
            .lock()
            .expect("switch conformance trace lock")
            .push(event);
    }

    fn old_owner_is_terminal(&self) -> bool {
        self.trace
            .lock()
            .expect("switch conformance trace lock")
            .contains(&SwitchHostEvent::OldOwnerTerminalObserved)
    }
}

#[async_trait]
impl EngineSwitchHost<EngineBackendPorts, EngineId> for ConformanceSwitchHost {
    async fn validate_target(
        &self,
        target: &EngineSwitchTarget<EngineId>,
    ) -> Result<(), PortError> {
        self.record(SwitchHostEvent::ValidateTarget);
        match target {
            EngineSwitchTarget::Selected {
                engine_id,
                selection,
            } if selection == engine_id => Ok(()),
            EngineSwitchTarget::Selected { .. } => Err(not_ready(
                "the engine switch selection must equal the target engine",
            )),
            EngineSwitchTarget::Unselected => {
                Err(not_ready("the engine switch requires a selected target"))
            }
        }
    }

    async fn prepare_candidate(
        &self,
        engine_id: &EngineId,
        selection: &EngineId,
        backend: &EngineBackendPorts,
    ) -> Result<CoreIdentity, PortError> {
        self.record(SwitchHostEvent::PrepareCandidate);
        if selection != engine_id || backend.engine_id() != engine_id {
            return Err(not_ready(
                "the candidate bundle does not match the requested engine",
            ));
        }
        let identity = backend.process().core_identity().ok_or_else(|| {
            PortError::Failed(format!(
                "{} candidate backend has no Core identity",
                engine_id.as_str()
            ))
        })?;
        if identity.engine != *engine_id {
            return Err(PortError::Failed(format!(
                "{} candidate backend reports identity for {}",
                engine_id.as_str(),
                identity.engine.as_str()
            )));
        }
        Ok(identity)
    }

    async fn terminate_old_owner(
        &self,
        identity: &CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<CoreGeneration, PortError> {
        self.record(SwitchHostEvent::OldOwnerStopRequested);
        if identity.engine != self.old_engine || !identity.same_instance(&self.old_identity) {
            return Err(PortError::Failed(
                "the switch host was asked to stop another owner".to_owned(),
            ));
        }
        if generation != self.old_generation {
            return Err(PortError::Failed(
                "the switch host received another owner generation".to_owned(),
            ));
        }
        match self.old_process.status().await {
            Ok(CoreLifecycle::Running) => {}
            Ok(other) => {
                return Err(PortError::Failed(format!(
                    "the old owner was {other:?} instead of Running before stop"
                )));
            }
            Err(error) => return Err(error),
        }
        self.old_process.stop().await?;
        match self.old_process.status().await {
            Ok(CoreLifecycle::Stopped) => {}
            Ok(other) => {
                return Err(PortError::Failed(format!(
                    "the old owner stopped at {other:?} instead of the exact terminal"
                )));
            }
            Err(error) => return Err(error),
        }
        match self.old_process.pid().await {
            None => {}
            Some(pid) => {
                return Err(PortError::Failed(format!(
                    "the old owner still reports pid {pid} after stop"
                )));
            }
        }
        self.record(SwitchHostEvent::OldOwnerTerminalObserved);
        Ok(generation)
    }

    async fn apply_candidate(
        &self,
        backend: EngineBackendPorts,
        selection: &EngineId,
    ) -> Result<(CoreIdentity, CoreGeneration), PortError> {
        self.record(SwitchHostEvent::NewOwnerStartRequested);
        if backend.engine_id() != selection {
            return Err(PortError::Failed(format!(
                "the applied backend does not match {}",
                selection.as_str()
            )));
        }
        if !self.old_owner_is_terminal() {
            return Err(PortError::Failed(
                "the candidate start was requested before the old owner terminal".to_owned(),
            ));
        }
        match self.old_process.status().await {
            Ok(CoreLifecycle::Stopped) => {}
            Ok(other) => {
                return Err(PortError::Failed(format!(
                    "the old owner is {other:?} while the candidate starts"
                )));
            }
            Err(error) => return Err(error),
        }
        let process = backend.process();
        let identity = process.core_identity().ok_or_else(|| {
            PortError::Failed(format!(
                "{} candidate backend has no Core identity",
                selection.as_str()
            ))
        })?;
        if identity.engine != *selection {
            return Err(PortError::Failed(format!(
                "{} candidate backend reports identity for {}",
                selection.as_str(),
                identity.engine.as_str()
            )));
        }
        process
            .start(CoreProcessStartContext::standalone(identity.clone()))
            .await?;
        let version =
            wait_for_readiness(backend.readiness().as_ref(), self.readiness_timeout).await?;
        if version.is_none() {
            return Err(PortError::Failed(
                "the candidate did not report an active version".to_owned(),
            ));
        }
        match process.status().await {
            Ok(CoreLifecycle::Running) => {}
            Ok(other) => {
                return Err(PortError::Failed(format!(
                    "the candidate reported {other:?} after readiness"
                )));
            }
            Err(error) => return Err(error),
        }
        match process.pid().await {
            Some(_) => {}
            None => {
                return Err(PortError::Failed(
                    "the running candidate has no process id".to_owned(),
                ));
            }
        }
        self.record(SwitchHostEvent::NewOwnerReady);
        Ok((identity, CoreGeneration::new(1)))
    }

    async fn close_candidate(&self, backend: EngineBackendPorts) -> Result<(), PortError> {
        self.record(SwitchHostEvent::CloseCandidate);
        let process = backend.process();
        match process.status().await {
            Ok(CoreLifecycle::Running) => process.stop().await,
            Ok(_) => Ok(()),
            Err(error) => Err(error),
        }
    }
}

fn not_ready(message: &'static str) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::NotReady, message, false))
}

//! The chaos adapters of the headless sandbox.
//!
//! The adapters implement the neutral ports of `lurker-core-ports` and inject
//! the deterministic faults of [`crate::fault`]. They never re-implement a
//! business rule: the lifecycle, the failure vocabulary and the owner fence
//! stay owned by the neutral crates, and the sandbox only decides *when* a
//! fault fires.

use crate::fault::{Fault, FaultPlan};
use async_trait::async_trait;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreIdentity, CoreVersion, EngineId};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::profile_document::DocumentDiagnosticSnapshot;
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use lurker_core_ports::engine_backend::{
    EngineBackendPorts, EngineBackendProvider, EngineOperationPorts,
};
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use std::sync::{Arc, Mutex, MutexGuard};

/// The observable phase of the chaos process.
///
/// The phase is one typed enum: the sandbox never encodes it with a flag or
/// with an optional field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChaosPhase {
    /// The Core process is not running.
    Stopped,
    /// The Core process is running.
    Running,
    /// The Core process died.
    Died,
}

/// The shared state of the chaos adapters.
#[derive(Debug)]
struct ChaosState {
    identity: CoreIdentity,
    version: CoreVersion,
    phase: ChaosPhase,
    plan: FaultPlan,
    starts: u32,
    stops: u32,
}

/// The process adapter of the sandbox.
///
/// The adapter owns the injected faults and the resource ledger: every
/// successful start must be matched by exactly one stop, so a scenario can
/// prove that no process is left behind.
pub struct ChaosProcess {
    state: Arc<Mutex<ChaosState>>,
}

impl ChaosProcess {
    /// Creates a stopped chaos process for one core identity.
    pub fn new(identity: CoreIdentity, version: CoreVersion) -> Self {
        Self {
            state: Arc::new(Mutex::new(ChaosState {
                identity,
                version,
                phase: ChaosPhase::Stopped,
                plan: FaultPlan::new(Fault::NetworkFlap, 0),
                starts: 0,
                stops: 0,
            })),
        }
    }

    /// The observable phase of the process.
    pub fn phase(&self) -> ChaosPhase {
        self.state().phase
    }

    /// The number of successful starts.
    pub fn starts(&self) -> u32 {
        self.state().starts
    }

    /// The number of stops.
    pub fn stops(&self) -> u32 {
        self.state().stops
    }

    /// Injects a fault plan.
    pub fn inject(&self, plan: FaultPlan) {
        self.state_mut().plan = plan;
    }

    /// Removes every injected fault.
    pub fn clear_faults(&self) {
        self.state_mut().plan = FaultPlan::new(Fault::NetworkFlap, 0);
    }

    /// The readiness adapter that follows this process.
    pub fn readiness(&self) -> ChaosReadiness {
        ChaosReadiness {
            state: Arc::clone(&self.state),
        }
    }

    fn state(&self) -> ChaosGuard<'_> {
        self.state.lock().expect("the chaos state lock is not poisoned")
    }

    fn state_mut(&self) -> ChaosGuard<'_> {
        self.state.lock().expect("the chaos state lock is not poisoned")
    }
}

/// The guard of the chaos state.
type ChaosGuard<'a> = MutexGuard<'a, ChaosState>;

#[async_trait]
impl CoreProcess for ChaosProcess {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(self.state().identity.clone())
    }

    async fn start(&self, _context: CoreProcessStartContext) -> Result<(), PortError> {
        let mut state = self.state_mut();
        if state.phase == ChaosPhase::Running {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "the chaos process is already running",
                false,
            )));
        }
        if state.plan.fire(Fault::KernelProcessDeath) {
            state.phase = ChaosPhase::Died;
            return Err(PortError::Failed(
                "the kernel process died while the runtime prepared the start".to_owned(),
            ));
        }
        state.phase = ChaosPhase::Running;
        state.starts += 1;
        Ok(())
    }

    async fn stop(&self) -> Result<(), PortError> {
        let mut state = self.state_mut();
        state.phase = ChaosPhase::Stopped;
        state.stops += 1;
        Ok(())
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(match self.state().phase {
            ChaosPhase::Stopped => CoreLifecycle::Stopped,
            ChaosPhase::Running => CoreLifecycle::Running,
            ChaosPhase::Died => CoreLifecycle::Failed,
        })
    }

    fn controller_endpoint(&self) -> Option<String> {
        Some("http://127.0.0.1:0".to_owned())
    }
}

/// The readiness adapter of the sandbox: the network boundary of the Core.
pub struct ChaosReadiness {
    state: Arc<Mutex<ChaosState>>,
}

#[async_trait]
impl CoreReadiness for ChaosReadiness {
    async fn probe(&self) -> Result<String, PortError> {
        let mut state = self
            .state
            .lock()
            .expect("the chaos state lock is not poisoned");
        if state.plan.fire(Fault::NetworkFlap) {
            return Err(PortError::Network(
                "the controller endpoint did not answer the readiness probe".to_owned(),
            ));
        }
        match state.phase {
            ChaosPhase::Running => Ok("chaos-ready".to_owned()),
            ChaosPhase::Stopped => {
                Err(PortError::Network("the chaos process is stopped".to_owned()))
            }
            ChaosPhase::Died => Err(PortError::Failed(
                "the kernel process died before the readiness probe".to_owned(),
            )),
        }
    }

    async fn active_core_version(&self) -> Result<Option<CoreVersion>, PortError> {
        let state = self
            .state
            .lock()
            .expect("the chaos state lock is not poisoned");
        Ok(match state.phase {
            ChaosPhase::Running => Some(state.version.clone()),
            ChaosPhase::Stopped | ChaosPhase::Died => None,
        })
    }
}

/// The document codec of the sandbox: one valid document, everything else is
/// a typed corruption.
pub struct ChaosCodec {
    scope: ProfileDocumentCodecScope,
    valid: Vec<u8>,
}

impl ChaosCodec {
    /// Creates a codec that accepts exactly one document.
    pub fn new(scope: ProfileDocumentCodecScope, valid: Vec<u8>) -> Self {
        Self { scope, valid }
    }

    /// The scope of the codec.
    pub fn scope(&self) -> ProfileDocumentCodecScope {
        self.scope.clone()
    }
}

impl ProfileDocumentCodecPort for ChaosCodec {
    fn scope(&self) -> ProfileDocumentCodecScope {
        self.scope.clone()
    }

    fn inspect(&self, raw_bytes: &[u8]) -> Result<DocumentValidation, PortError> {
        if raw_bytes == self.valid.as_slice() {
            return Ok(DocumentValidation::Valid);
        }
        Ok(DocumentValidation::Invalid {
            diagnostics: vec![DocumentDiagnosticSnapshot {
                line: Some(0),
                column: Some(0),
                message: "the profile document is corrupted".to_owned(),
            }],
        })
    }
}

/// The backend provider of the sandbox: one engine, one chaos process.
pub struct ChaosProvider {
    engine: EngineId,
    process: Arc<ChaosProcess>,
}

impl ChaosProvider {
    /// Creates a provider for one engine and one chaos process.
    pub fn new(engine: EngineId, process: Arc<ChaosProcess>) -> Self {
        Self { engine, process }
    }

    /// The codec scope of the sandbox engine.
    pub fn codec_scope(engine: &EngineId) -> ProfileDocumentCodecScope {
        ProfileDocumentCodecScope {
            engine_id: engine.clone(),
            format_id: ProfileFormatId::new("sandbox-yaml").expect("the sandbox format is valid"),
            schema_version: ProfileSchemaVersion::new(1)
                .expect("the sandbox schema version is valid"),
        }
    }

    /// The document the sandbox codec accepts.
    pub fn valid_document() -> Vec<u8> {
        b"proxies: []\n".to_vec()
    }
}

impl EngineBackendProvider<EngineBackendPorts> for ChaosProvider {
    fn engine_id(&self) -> EngineId {
        self.engine.clone()
    }

    fn create_backend(&self) -> Result<EngineBackendPorts, PortError> {
        EngineBackendPorts::new(
            self.engine.clone(),
            Arc::clone(&self.process),
            Arc::new(self.process.readiness()),
            Arc::new(ChaosCodec::new(
                Self::codec_scope(&self.engine),
                Self::valid_document(),
            )),
            EngineOperationPorts::unsupported("the sandbox composes no runtime operation"),
        )
    }
}

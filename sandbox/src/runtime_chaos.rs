//! The runtime harness of the headless sandbox.
//!
//! The harness owns one chaos process and the neutral runtime over it. Every
//! scenario runs headless: no engine binary, no network, no display and no
//! wall-clock decision.

use crate::fault::{Fault, FaultPlan};
use crate::ports::{ChaosProcess, ChaosProvider};
use lurker_core_contract::identity::{CoreIdentity, CoreInstanceId, CoreVersion, EngineId};
use lurker_core_ports::engine_backend::{EngineBackendPorts, EngineBackendProvider};
use lurker_core_ports::error::PortError;
use lurker_engine_runtime::facade::{LurkerRuntime, LurkerRuntimeBuilder, RuntimeBuildError};
use std::sync::Arc;
use std::time::Duration;

/// The bound of the readiness wait of the sandbox.
///
/// The bound is a safety net, not an assertion: every scenario decides its
/// outcome from a typed failure, never from the expiry of this bound.
pub const READINESS_TIMEOUT: Duration = Duration::from_secs(1);

/// The identity of the sandbox Core instance.
pub fn sandbox_identity(engine: &EngineId) -> CoreIdentity {
    CoreIdentity::new(
        engine.clone(),
        CoreInstanceId::new("sandbox-instance").expect("the sandbox instance identifier is valid"),
        None,
    )
}

/// The version the sandbox Core reports.
pub fn sandbox_version() -> CoreVersion {
    CoreVersion::new("sandbox-1.0.0").expect("the sandbox version is valid")
}

/// The headless sandbox: one chaos process and the runtime over it.
pub struct Sandbox {
    engine: EngineId,
    process: Arc<ChaosProcess>,
}

impl Sandbox {
    /// Creates a sandbox for one engine.
    pub fn new(engine: EngineId) -> Self {
        let process = Arc::new(ChaosProcess::new(
            sandbox_identity(&engine),
            sandbox_version(),
        ));
        Self { engine, process }
    }

    /// The chaos process of the sandbox.
    pub fn process(&self) -> Arc<ChaosProcess> {
        Arc::clone(&self.process)
    }

    /// Injects one fault into the sandbox.
    pub fn inject(&self, fault: Fault, remaining: u32) {
        self.process.inject(FaultPlan::new(fault, remaining));
    }

    /// Removes every injected fault.
    pub fn clear_faults(&self) {
        self.process.clear_faults();
    }

    /// Builds a runtime over the chaos process.
    pub fn runtime(&self) -> Result<LurkerRuntime, RuntimeBuildError> {
        let provider = ChaosProvider::new(self.engine.clone(), Arc::clone(&self.process));
        let builder = LurkerRuntimeBuilder::new().register_engine(Arc::new(provider))?;
        builder
            .select_engine(self.engine.clone())
            .with_readiness_timeout(READINESS_TIMEOUT)
            .build()
    }

    /// Builds the backend bundle of the sandbox engine without starting it.
    pub fn backend(&self) -> Result<EngineBackendPorts, PortError> {
        ChaosProvider::new(self.engine.clone(), Arc::clone(&self.process)).create_backend()
    }
}

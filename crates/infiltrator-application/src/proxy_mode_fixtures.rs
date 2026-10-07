//! Isolated controller for native mode scenarios; commands execute the real application path.
use async_trait::async_trait;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::runtime_control::{RuntimeControlSnapshot, RuntimeControlStatus};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{SurfaceOrigin, SurfaceSnapshot};
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::error::PortError;
use infiltrator_ports::overview::{OverviewReader, OverviewSample};
use std::sync::Mutex;

pub struct ModeScenarioController {
    state: Mutex<ControllerState>,
}
struct ControllerState {
    mode: ProxyMode,
    requests: Vec<ProxyMode>,
    failure: Option<Failure>,
}
impl Default for ModeScenarioController {
    fn default() -> Self {
        Self {
            state: Mutex::new(ControllerState {
                mode: ProxyMode::Rule,
                requests: Vec::new(),
                failure: Some(Failure::new(
                    ErrorCode::Network,
                    "Controller temporarily refused mode change",
                    true,
                )),
            }),
        }
    }
}
impl ModeScenarioController {
    pub fn reject_with(&self, failure: Option<Failure>) {
        self.state.lock().expect("mode fixture").failure = failure;
    }
    pub fn requests(&self) -> Vec<ProxyMode> {
        self.state.lock().expect("mode fixture").requests.clone()
    }
    pub fn snapshot(&self, core: CoreSnapshot, surface: SurfaceKind) -> SurfaceSnapshot {
        let mode = self.state.lock().expect("mode fixture").mode;
        let mut snapshot = SurfaceSnapshot::unavailable(
            surface,
            HostKind::Desktop,
            Failure::unsupported("Unrelated capture page is not composed"),
        );
        snapshot.generation = core.generation;
        snapshot.origin = SurfaceOrigin::Demo;
        snapshot.failure = None;
        snapshot.revision = 1;
        snapshot.core = core;
        snapshot.core.proxy_mode = Some(mode);
        snapshot.runtime_control = RuntimeControlSnapshot {
            status: RuntimeControlStatus::Ready,
            mode: Some(mode),
            script_available: Some(false),
            ..RuntimeControlSnapshot::default()
        };
        snapshot
    }
}
#[async_trait]
impl CoreProcess for ModeScenarioController {
    async fn start(&self) -> Result<(), PortError> {
        Err(PortError::Failed(
            "Mode scenario refuses lifecycle effects".into(),
        ))
    }
    async fn stop(&self) -> Result<(), PortError> {
        Err(PortError::Failed(
            "Mode scenario refuses lifecycle effects".into(),
        ))
    }
    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(CoreLifecycle::Running)
    }
    fn controller_endpoint(&self) -> Option<String> {
        None
    }
}
#[async_trait]
impl CoreReadiness for ModeScenarioController {
    async fn probe(&self) -> Result<String, PortError> {
        Ok("mode-scenario-controller".into())
    }
}
#[async_trait]
impl OverviewReader for ModeScenarioController {
    async fn sample(&self) -> Result<OverviewSample, PortError> {
        Ok(OverviewSample {
            lifecycle: CoreLifecycle::Running,
            mode: Some(self.state.lock().expect("mode fixture").mode),
            upload_total: 0,
            download_total: 0,
            active_connections: 0,
            memory_bytes: None,
            core_version: Some("mode-scenario-controller".into()),
            sampled_at_epoch_ms: None,
        })
    }
    async fn set_mode(&self, mode: ProxyMode) -> Result<ProxyMode, PortError> {
        let mut state = self.state.lock().expect("mode fixture");
        state.requests.push(mode);
        if let Some(failure) = &state.failure {
            return Err(PortError::Rejected(failure.clone()));
        }
        state.mode = mode;
        Ok(state.mode)
    }
}

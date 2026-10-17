//! Isolated controller fixture for routing-intent command scenarios.

use async_trait::async_trait;
use infiltrator_contract::snapshot::CoreSnapshot;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{SurfaceOrigin, SurfaceSnapshot};
use infiltrator_ports::overview::{OverviewReader, OverviewSample};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{
    CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::routing_intent::{
    RoutingIntent, RoutingIntentSnapshot, RoutingIntentStatus,
};
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use lurker_core_ports::error::PortError;
use lurker_core_ports::routing_intent::RoutingIntentPort;
use std::sync::Mutex;

pub struct RoutingIntentScenarioController {
    state: Mutex<ControllerState>,
}

struct ControllerState {
    intent: RoutingIntent,
    requests: Vec<RoutingIntent>,
    disposition: RoutingIntentScenarioDisposition,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoutingIntentScenarioDisposition {
    Accept,
    Reject(Failure),
}

impl Default for RoutingIntentScenarioController {
    fn default() -> Self {
        Self {
            state: Mutex::new(ControllerState {
                intent: RoutingIntent::FollowRules,
                requests: Vec::new(),
                disposition: RoutingIntentScenarioDisposition::Reject(Failure::new(
                    ErrorCode::Network,
                    "Controller temporarily refused routing change",
                    true,
                )),
            }),
        }
    }
}

impl RoutingIntentScenarioController {
    pub fn accept_requests(&self) {
        self.state
            .lock()
            .expect("routing intent fixture")
            .disposition = RoutingIntentScenarioDisposition::Accept;
    }

    pub fn reject_requests(&self, failure: Failure) {
        self.state
            .lock()
            .expect("routing intent fixture")
            .disposition = RoutingIntentScenarioDisposition::Reject(failure);
    }

    pub fn requests(&self) -> Vec<RoutingIntent> {
        self.state
            .lock()
            .expect("routing intent fixture")
            .requests
            .clone()
    }

    pub fn snapshot(&self, mut core: CoreSnapshot, surface: SurfaceKind) -> SurfaceSnapshot {
        let current = self
            .state
            .lock()
            .expect("routing intent fixture")
            .intent
            .clone();
        let routing_intent = RoutingIntentSnapshot {
            current: Some(current.clone()),
            status: RoutingIntentStatus::Ready,
            failure: None,
        };
        core.routing_intent = routing_intent;
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
        snapshot
    }
}

#[async_trait]
impl CoreProcess for RoutingIntentScenarioController {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            EngineId::new("mihomo").ok()?,
            CoreInstanceId::new("routing-intent-scenario").ok()?,
            None,
        ))
    }

    async fn start(&self, _context: CoreProcessStartContext) -> Result<(), PortError> {
        Err(PortError::Failed(
            "Routing intent scenario refuses lifecycle effects".into(),
        ))
    }

    async fn stop(&self) -> Result<(), PortError> {
        Err(PortError::Failed(
            "Routing intent scenario refuses lifecycle effects".into(),
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
impl CoreReadiness for RoutingIntentScenarioController {
    async fn probe(&self) -> Result<String, PortError> {
        Ok("routing-intent-scenario-controller".into())
    }

    async fn active_core_version(&self) -> Result<Option<CoreVersion>, PortError> {
        Ok(Some(
            CoreVersion::new("1.19.12").expect("valid routing intent scenario version"),
        ))
    }
}

#[async_trait]
impl OverviewReader for RoutingIntentScenarioController {
    async fn sample(&self) -> Result<OverviewSample, PortError> {
        Ok(OverviewSample {
            lifecycle: CoreLifecycle::Running,
            upload_total: 0,
            download_total: 0,
            active_connections: 0,
            core_version: Some("routing-intent-scenario-controller".into()),
            sampled_at_epoch_ms: None,
        })
    }
}

#[async_trait]
impl RoutingIntentPort for RoutingIntentScenarioController {
    async fn observe(&self, scope: &CoreScope) -> Result<RoutingIntent, PortError> {
        if scope.identity.engine.as_str() != "mihomo" {
            return Err(PortError::unsupported(
                Capability::RoutingIntent,
                "routing-intent scenario only accepts its Mihomo test scope",
            ));
        }
        Ok(self
            .state
            .lock()
            .expect("routing intent fixture")
            .intent
            .clone())
    }

    async fn apply(
        &self,
        scope: &CoreScope,
        intent: RoutingIntent,
    ) -> Result<RoutingIntent, PortError> {
        if scope.identity.engine.as_str() != "mihomo" {
            return Err(PortError::unsupported(
                Capability::RoutingIntent,
                "routing-intent scenario only accepts its Mihomo test scope",
            ));
        }
        let mut state = self.state.lock().expect("routing intent fixture");
        state.requests.push(intent.clone());
        match &state.disposition {
            RoutingIntentScenarioDisposition::Accept => {}
            RoutingIntentScenarioDisposition::Reject(failure) => {
                return Err(PortError::Rejected(failure.clone()));
            }
        }
        if matches!(intent, RoutingIntent::EngineDefined(_)) {
            return Err(PortError::unsupported(
                Capability::RoutingIntent,
                "engine-defined routing intents are unsupported by this scenario",
            ));
        }
        state.intent = intent;
        Ok(state.intent.clone())
    }
}

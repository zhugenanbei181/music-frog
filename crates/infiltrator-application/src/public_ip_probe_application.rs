//! Core-first owner of one public-egress observation.
//!
//! The owner stamps a refresh with the live Core route, publishes `Probing`,
//! asks exactly one host port, then admits only the answer carrying that same
//! stamp. Internal errors become typed failures and terminal readback; no
//! command falls through to a generic `unsupported`, and no UI keeps a
//! private loading boolean.

use infiltrator_contract::public_ip::PublicIpProbeSnapshot;
use infiltrator_contract::public_ip_probe::{
    PublicIpProbeAnswer, PublicIpProbeError, PublicIpProbeRequest, PublicIpProbeRouteStamp,
    PublicIpProbeStamp, PublicIpProbeState,
};
use infiltrator_ports::core_snapshot_source::CoreSnapshotSource;
use infiltrator_ports::public_ip_probe::PublicIpProbePort;
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::routing_intent::RoutingIntent;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub const NO_PROBE_PORT_REASON: &str = "public IP probe port is not composed";

/// Route authored by the Core owner from the live snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicIpRefreshRoute {
    Direct,
    RuntimeHttpProxy,
}

impl PublicIpRefreshRoute {
    fn stamp(self) -> PublicIpProbeRouteStamp {
        match self {
            Self::Direct => PublicIpProbeRouteStamp::Direct,
            Self::RuntimeHttpProxy => PublicIpProbeRouteStamp::ThroughRuntimeHttpProxy,
        }
    }
}

#[derive(Clone)]
pub struct PublicIpProbeApplication {
    port: Option<Arc<dyn PublicIpProbePort>>,
    core: Option<Arc<dyn CoreSnapshotSource>>,
    state: Arc<Mutex<PublicIpProbeState>>,
    next_request_id: Arc<AtomicU64>,
}

impl Default for PublicIpProbeApplication {
    fn default() -> Self {
        Self::unconfigured()
    }
}

impl PublicIpProbeApplication {
    pub fn new(port: Option<Arc<dyn PublicIpProbePort>>) -> Self {
        Self {
            port,
            core: None,
            state: Arc::new(Mutex::new(PublicIpProbeState::Idle)),
            next_request_id: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Install the Core application as the only author of generation,
    /// revision, lifecycle and routing facts.
    pub fn with_core_snapshot_source(mut self, core: Arc<dyn CoreSnapshotSource>) -> Self {
        self.core = Some(core);
        self
    }

    pub fn unconfigured() -> Self {
        Self::new(None)
    }

    pub fn has_port(&self) -> bool {
        self.port.is_some()
    }

    /// Publish the owner's authoritative readback.
    pub fn snapshot(&self) -> PublicIpProbeSnapshot {
        let state = self
            .state
            .lock()
            .expect("public IP probe state lock")
            .clone();
        PublicIpProbeSnapshot::from_state(&state)
    }

    /// Drive one explicit refresh to a terminal state.
    pub async fn refresh(&self, route: PublicIpRefreshRoute) {
        let Some(core_source) = self.core.as_ref() else {
            self.publish_failure(
                PublicIpProbeStamp {
                    request_id: self.next_request_id(),
                    core_generation: 0,
                    revision: 0,
                    route: route.stamp(),
                },
                "Core snapshot source is not composed",
            );
            return;
        };
        let core = core_source.core_snapshot();
        let stamp = PublicIpProbeStamp {
            request_id: self.next_request_id(),
            core_generation: core.generation,
            revision: core.revision.max(1),
            route: route.stamp(),
        };

        let started = {
            let mut guard = self.state.lock().expect("public IP probe state lock");
            match guard.begin(stamp) {
                Ok(state) => {
                    *guard = state;
                    true
                }
                Err(PublicIpProbeError::AlreadyProbing) => false,
                Err(_) => unreachable!("typed begin has exactly one refusal"),
            }
        };
        if !started {
            return;
        }

        let answer = self.probe(stamp).await;
        let mut guard = self.state.lock().expect("public IP probe state lock");
        let current = guard.clone();
        match current.accept(answer.clone()) {
            Ok(state) => *guard = state,
            Err(PublicIpProbeError::StaleAnswer) => {
                *guard = PublicIpProbeState::Stale { stamp };
            }
            Err(error) => {
                *guard = PublicIpProbeState::Failed {
                    stamp,
                    reason: format!("public IP probe owner rejected answer: {error}"),
                };
            }
        }
    }

    fn next_request_id(&self) -> u64 {
        self.next_request_id.fetch_add(1, Ordering::SeqCst).max(1)
    }

    fn publish_failure(&self, stamp: PublicIpProbeStamp, reason: &str) {
        *self.state.lock().expect("public IP probe state lock") = PublicIpProbeState::Failed {
            stamp,
            reason: reason.to_owned(),
        };
    }

    /// The route is a Core-authored fact, not a UI preference. A proxy policy
    /// uses the runtime HTTP proxy; direct or unobserved policies use Direct.
    pub fn refresh_route(&self) -> PublicIpRefreshRoute {
        let Some(core_source) = self.core.as_ref() else {
            return PublicIpRefreshRoute::Direct;
        };
        match core_source.core_snapshot().routing_intent.current {
            Some(RoutingIntent::RouteAllThroughProxy) => PublicIpRefreshRoute::RuntimeHttpProxy,
            _ => PublicIpRefreshRoute::Direct,
        }
    }

    async fn probe(&self, stamp: PublicIpProbeStamp) -> PublicIpProbeAnswer {
        let Some(port) = self.port.as_ref() else {
            return PublicIpProbeAnswer::Failed {
                stamp,
                reason: NO_PROBE_PORT_REASON.to_owned(),
            };
        };
        let core = self
            .core
            .as_ref()
            .expect("refresh stamps only through an installed Core source")
            .core_snapshot();
        if !matches!(
            core.lifecycle,
            CoreLifecycle::Running | CoreLifecycle::Ready
        ) {
            return PublicIpProbeAnswer::Failed {
                stamp,
                reason: "core is not running; public IP is not probed".to_owned(),
            };
        }
        port.observe(PublicIpProbeRequest::new(stamp)).await
    }
}

#[cfg(test)]
#[path = "public_ip_probe_application_tests.rs"]
mod tests;

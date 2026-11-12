//! Behavior test for the shared closed-connection owner readback.

use super::*;
use async_trait::async_trait;
use infiltrator_domain::runtime::{Connection, ConnectionMetadata, ConnectionSnapshot};
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationSleep,
};
use infiltrator_ports::connection::{
    ConnectionControlReceipt, ConnectionControlRequest, ConnectionObservationSnapshot,
    ConnectionPort,
};
use infiltrator_ports::runtime_stream::ScopedRuntimeStream;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::identity::{CoreIdentity, CoreInstanceId, EngineId};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::outbound::identity::SnapshotRevision;
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use std::sync::Mutex;
use std::time::Duration;
use tokio::runtime::Builder;
use tokio::time::sleep;

struct SequencedConnectionPort {
    observations: Mutex<Vec<ConnectionObservationSnapshot>>,
}

#[async_trait]
impl ConnectionPort for SequencedConnectionPort {
    async fn observe(&self, _scope: CoreScope) -> Result<ConnectionObservationSnapshot, PortError> {
        let mut observations = self.observations.lock().expect("connection sequence lock");
        if observations.is_empty() {
            return Err(PortError::unsupported(
                Capability::Connections,
                "closed-history surface test has no more observations",
            ));
        }
        Ok(observations.remove(0))
    }

    async fn close(
        &self,
        _request: ConnectionControlRequest,
    ) -> Result<ConnectionControlReceipt, PortError> {
        Err(PortError::unsupported(
            Capability::Connections,
            "closed-history surface test does not close connections",
        ))
    }

    async fn stream(
        &self,
        _scope: CoreScope,
    ) -> Result<ScopedRuntimeStream<ConnectionObservationSnapshot>, PortError> {
        Err(PortError::unsupported(
            Capability::Connections,
            "closed-history surface test does not stream connections",
        ))
    }
}

fn observation(
    scope: CoreScope,
    revision: u64,
    connections: Vec<Connection>,
) -> ConnectionObservationSnapshot {
    ConnectionObservationSnapshot::new(
        scope,
        SnapshotRevision::new(revision),
        ConnectionSnapshot {
            connections,
            ..Default::default()
        },
    )
    .expect("valid connection observation")
}

struct ClosedHistoryRuntime;

impl ApplicationRuntime for ClosedHistoryRuntime {
    fn block_on(&self, future: ApplicationFuture) {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
            .block_on(future);
    }

    fn sleep(&self, duration: Duration) -> ApplicationSleep<'_> {
        Box::pin(sleep(duration))
    }
}

struct RunningProcess;

#[async_trait]
impl CoreProcess for RunningProcess {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            EngineId::new("mihomo").expect("engine ID"),
            CoreInstanceId::new("closed-history-test").expect("instance ID"),
            None,
        ))
    }

    async fn start(&self, _context: CoreProcessStartContext) -> Result<(), PortError> {
        Ok(())
    }

    async fn stop(&self) -> Result<(), PortError> {
        Ok(())
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(CoreLifecycle::Running)
    }

    fn controller_endpoint(&self) -> Option<String> {
        Some("http://127.0.0.1:9090".to_owned())
    }
}

struct ReadyCore;

#[async_trait]
impl CoreReadiness for ReadyCore {
    async fn probe(&self) -> Result<String, PortError> {
        Ok("http://127.0.0.1:9090".to_owned())
    }
}

#[tokio::test]
async fn surface_reader_feeds_the_one_closed_connection_owner() {
    let core = Arc::new(CoreApplication::new(
        Arc::new(RunningProcess),
        Arc::new(ReadyCore),
        Arc::new(ClosedHistoryRuntime),
    ));
    let scope = core.snapshot().scope().expect("running test core scope");
    let first = observation(
        scope.clone(),
        1,
        vec![Connection {
            id: "c-1".to_owned(),
            metadata: ConnectionMetadata {
                host: "example.com".to_owned(),
                ..Default::default()
            },
            start: "2026-11-12T05:00:00+00:00".to_owned(),
            ..Default::default()
        }],
    );
    let second = observation(scope, 2, Vec::new());
    let port = Arc::new(SequencedConnectionPort {
        observations: Mutex::new(vec![first, second]),
    });
    let closed_connections = core.closed_connections_application();
    let reader =
        ProductSurfaceReader::new(core.clone(), SurfaceKind::BevyDesktop, HostKind::Desktop)
            .with_connection_port(port);

    let baseline = reader.read().await.expect("first surface read");
    assert_eq!(baseline.closed_connections.total_closed, 0);
    let closed = reader.read().await.expect("second surface read");
    assert_eq!(closed.closed_connections.total_closed, 1);
    assert_eq!(closed.closed_connections.records[0].id, "c-1");
    assert_eq!(closed_connections.readback(), closed.closed_connections);

    assert_eq!(closed_connections.clear(), Default::default());
    let cleared = reader.read().await.expect("surface read after clear");
    assert_eq!(cleared.closed_connections, Default::default());
}

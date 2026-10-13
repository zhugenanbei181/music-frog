//! Active-connection use-cases over the scoped connection port.

use infiltrator_contract::connection::ConnectionId;
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_domain::connection_activity::ConnectionActivityTracker;
use infiltrator_domain::runtime::Connection;
use infiltrator_ports::connection::{
    ConnectionControlReceipt, ConnectionControlRequest, ConnectionControlTarget,
    ConnectionObservationSnapshot, ConnectionPort,
};
use infiltrator_ports::core_scope::CoreScopeSource;
use infiltrator_ports::runtime_stream::ScopedRuntimeStream;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Outcome of one idle-connection sweep (DUAL-13-11).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdleSweepReport {
    /// Connections inspected in the live snapshot.
    pub inspected: usize,
    /// Ids whose counters had not moved for the configured timeout.
    pub idle: Vec<String>,
    /// Ids actually terminated by this sweep.
    pub closed: usize,
}

#[derive(Clone)]
pub struct ConnectionApplication {
    scope_source: Arc<dyn CoreScopeSource>,
    port: Arc<dyn ConnectionPort>,
    activity: Arc<Mutex<ConnectionActivityTracker>>,
}

impl ConnectionApplication {
    pub fn new(scope_source: Arc<dyn CoreScopeSource>, port: Arc<dyn ConnectionPort>) -> Self {
        Self {
            scope_source,
            port,
            activity: Arc::new(Mutex::new(ConnectionActivityTracker::new())),
        }
    }

    pub async fn snapshot(&self) -> Result<ConnectionObservationSnapshot, Failure> {
        let scope = crate::runtime_scope::active_scope(self.scope_source.as_ref())?;
        self.snapshot_for_scope(scope).await
    }

    pub async fn close(&self, id: &str) -> Result<(), Failure> {
        let scope = crate::runtime_scope::active_scope(self.scope_source.as_ref())?;
        self.close_for_scope(scope, id).await
    }

    pub async fn close_all(&self) -> Result<(), Failure> {
        let scope = crate::runtime_scope::active_scope(self.scope_source.as_ref())?;
        self.close_all_for_scope(scope).await
    }

    pub async fn snapshot_for_scope(
        &self,
        scope: CoreScope,
    ) -> Result<ConnectionObservationSnapshot, Failure> {
        let observed = self
            .port
            .observe(scope.clone())
            .await
            .map_err(Failure::from)?;
        if observed.scope() != &scope {
            return Err(stale_scope_failure());
        }
        Ok(observed)
    }

    pub async fn close_for_scope(&self, scope: CoreScope, raw_id: &str) -> Result<(), Failure> {
        let connection_id = ConnectionId::new(scope.identity.engine.clone(), raw_id)
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let observed = self.snapshot_for_scope(scope.clone()).await?;
        if !observed.connection_ids().contains(&connection_id) {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "connection ID was not present in the accepted Core observation",
                false,
            ));
        }
        let request = ConnectionControlRequest::new(
            scope.clone(),
            observed.revision(),
            ConnectionControlTarget::One(connection_id.clone()),
        )
        .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let receipt = self.port.close(request).await.map_err(Failure::from)?;
        validate_receipt(&scope, observed.revision(), &receipt, &[connection_id])
    }

    pub async fn close_all_for_scope(&self, scope: CoreScope) -> Result<(), Failure> {
        let observed = self.snapshot_for_scope(scope.clone()).await?;
        let expected_ids = observed.connection_ids().to_vec();
        let request = ConnectionControlRequest::new(
            scope.clone(),
            observed.revision(),
            ConnectionControlTarget::All,
        )
        .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let receipt = self.port.close(request).await.map_err(Failure::from)?;
        validate_receipt(&scope, observed.revision(), &receipt, &expected_ids)
    }

    pub async fn close_by_host(&self, host: &str) -> Result<usize, Failure> {
        let observed = self.snapshot().await?;
        self.close_matching(
            observed.scope().clone(),
            observed.snapshot().connections.clone(),
            |connection| connection.metadata.host.contains(host),
        )
        .await
    }

    pub async fn close_by_process(&self, process: &str) -> Result<usize, Failure> {
        let observed = self.snapshot().await?;
        self.close_matching(
            observed.scope().clone(),
            observed.snapshot().connections.clone(),
            |connection| connection.metadata.process_path.contains(process),
        )
        .await
    }

    pub async fn stream(
        &self,
    ) -> Result<ScopedRuntimeStream<ConnectionObservationSnapshot>, Failure> {
        let scope = crate::runtime_scope::active_scope(self.scope_source.as_ref())?;
        let stream = self
            .port
            .stream(scope.clone())
            .await
            .map_err(Failure::from)?;
        Ok(crate::runtime_stream_application::scope_checked(
            self.scope_source.clone(),
            scope,
            stream,
        ))
    }

    /// Record the current connection slice so idle detection can compare
    /// cumulative counters across observations (DUAL-13-11).
    pub fn observe_activity(&self, connections: &[Connection], now_secs: u64) {
        if let Ok(mut tracker) = self.activity.lock() {
            tracker.observe(connections, now_secs);
        }
    }

    /// Ids that have not moved for more than `timeout_secs` at `now_secs`,
    /// after recording this slice.
    pub fn idle_connections(
        &self,
        connections: &[Connection],
        now_secs: u64,
        timeout_secs: u64,
    ) -> Vec<String> {
        match self.activity.lock() {
            Ok(mut tracker) => {
                tracker.observe(connections, now_secs);
                tracker.idle_ids(now_secs, timeout_secs)
            }
            Err(_) => Vec::new(),
        }
    }

    /// Sweep idle connections through the shared tracker and terminate each
    /// identified ID through the scoped connection port (DUAL-13-11). The report is honest: it
    /// counts only the ids actually closed.
    pub async fn sweep_idle(&self, timeout_secs: u64) -> Result<IdleSweepReport, Failure> {
        let snapshot = self.snapshot().await?;
        let now = current_unix_secs();
        let idle = self.idle_connections(&snapshot.snapshot().connections, now, timeout_secs);
        let mut closed = 0usize;
        for id in &idle {
            self.close_for_scope(snapshot.scope().clone(), id).await?;
            closed += 1;
        }
        Ok(IdleSweepReport {
            inspected: snapshot.snapshot().connections.len(),
            idle,
            closed,
        })
    }

    async fn close_matching(
        &self,
        scope: CoreScope,
        connections: Vec<Connection>,
        matches: impl Fn(&Connection) -> bool,
    ) -> Result<usize, Failure> {
        let ids = connections
            .iter()
            .filter(|connection| matches(connection))
            .map(|connection| connection.id.clone())
            .collect::<Vec<_>>();
        for id in &ids {
            self.close_for_scope(scope.clone(), id).await?;
        }
        Ok(ids.len())
    }
}

fn validate_receipt(
    scope: &CoreScope,
    expected_revision: infiltrator_contract::outbound::identity::SnapshotRevision,
    receipt: &ConnectionControlReceipt,
    expected_ids: &[ConnectionId],
) -> Result<(), Failure> {
    if receipt.scope() != scope
        || receipt.observed_revision() < expected_revision
        || receipt.closed().len() != expected_ids.len()
        || expected_ids.iter().any(|id| !receipt.closed().contains(id))
    {
        return Err(stale_scope_failure());
    }
    Ok(())
}

fn stale_scope_failure() -> Failure {
    Failure::new(
        ErrorCode::InvalidState,
        "connection observation or close receipt belongs to another Core scope",
        false,
    )
}

/// Wall-clock seconds, the observation instant shared by both surfaces'
/// activity trackers. Zero on a pre-epoch clock.
fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct UnavailableScope;

    impl CoreScopeSource for UnavailableScope {
        fn scope_observation(&self) -> CoreScopeObservation {
            CoreScopeObservation::Unavailable
        }
    }

    struct CountingConnectionPort(AtomicUsize);

    #[async_trait]
    impl ConnectionPort for CountingConnectionPort {
        async fn observe(
            &self,
            _scope: CoreScope,
        ) -> Result<ConnectionObservationSnapshot, infiltrator_ports::error::PortError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(infiltrator_ports::error::PortError::Failed(
                "unexpected observation".to_owned(),
            ))
        }

        async fn close(
            &self,
            _request: ConnectionControlRequest,
        ) -> Result<ConnectionControlReceipt, infiltrator_ports::error::PortError> {
            panic!("unavailable owner must fail before close")
        }

        async fn stream(
            &self,
            _scope: CoreScope,
        ) -> Result<
            ScopedRuntimeStream<ConnectionObservationSnapshot>,
            infiltrator_ports::error::PortError,
        > {
            panic!("unavailable owner must fail before stream")
        }
    }

    #[tokio::test]
    async fn unavailable_owner_scope_rejects_connection_read_before_adapter_call() {
        let port = Arc::new(CountingConnectionPort(AtomicUsize::new(0)));
        let application = ConnectionApplication::new(Arc::new(UnavailableScope), port.clone());

        let failure = application
            .snapshot()
            .await
            .expect_err("inactive Core cannot provide a connection snapshot");

        assert_eq!(failure.code, ErrorCode::NotReady);
        assert_eq!(port.0.load(Ordering::SeqCst), 0);
    }
}

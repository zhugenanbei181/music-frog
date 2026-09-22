//! Active-connection use-cases over the runtime gateway.

use infiltrator_contract::error::Failure;
use infiltrator_domain::connection_activity::ConnectionActivityTracker;
use infiltrator_domain::runtime::{Connection, ConnectionSnapshot};
use infiltrator_ports::runtime_gateway::{RuntimeGateway, RuntimeStream};
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
    gateway: Arc<dyn RuntimeGateway>,
    activity: Arc<Mutex<ConnectionActivityTracker>>,
}

impl ConnectionApplication {
    pub fn new(gateway: Arc<dyn RuntimeGateway>) -> Self {
        Self {
            gateway,
            activity: Arc::new(Mutex::new(ConnectionActivityTracker::new())),
        }
    }

    pub async fn snapshot(&self) -> Result<ConnectionSnapshot, Failure> {
        self.gateway.get_connections().await.map_err(Failure::from)
    }

    pub async fn close(&self, id: &str) -> Result<(), Failure> {
        self.gateway
            .close_connection(id)
            .await
            .map_err(Failure::from)
    }

    pub async fn close_all(&self) -> Result<(), Failure> {
        self.gateway
            .close_all_connections()
            .await
            .map_err(Failure::from)
    }

    pub async fn close_by_host(&self, host: &str) -> Result<usize, Failure> {
        let connections = self.snapshot().await?.connections;
        self.close_matching(connections, |connection| {
            connection.metadata.host.contains(host)
        })
        .await
    }

    pub async fn close_by_process(&self, process: &str) -> Result<usize, Failure> {
        let connections = self.snapshot().await?.connections;
        self.close_matching(connections, |connection| {
            connection.metadata.process_path.contains(process)
        })
        .await
    }

    pub async fn stream(&self) -> Result<RuntimeStream<ConnectionSnapshot>, Failure> {
        self.gateway
            .stream_connections()
            .await
            .map_err(Failure::from)
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
    /// identified id over the gateway (DUAL-13-11). The report is honest: it
    /// counts only the ids actually closed.
    pub async fn sweep_idle(&self, timeout_secs: u64) -> Result<IdleSweepReport, Failure> {
        let snapshot = self.snapshot().await?;
        let now = current_unix_secs();
        let idle = self.idle_connections(&snapshot.connections, now, timeout_secs);
        let mut closed = 0usize;
        for id in &idle {
            self.close(id).await?;
            closed += 1;
        }
        Ok(IdleSweepReport {
            inspected: snapshot.connections.len(),
            idle,
            closed,
        })
    }

    async fn close_matching(
        &self,
        connections: Vec<Connection>,
        matches: impl Fn(&Connection) -> bool,
    ) -> Result<usize, Failure> {
        let ids = connections
            .iter()
            .filter(|connection| matches(connection))
            .map(|connection| connection.id.clone())
            .collect::<Vec<_>>();
        for id in &ids {
            self.close(id).await?;
        }
        Ok(ids.len())
    }
}

/// Wall-clock seconds, the observation instant shared by both surfaces'
/// activity trackers. Zero on a pre-epoch clock.
fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

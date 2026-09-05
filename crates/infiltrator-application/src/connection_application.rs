//! Active-connection use-cases over the runtime gateway.

use infiltrator_contract::error::Failure;
use infiltrator_domain::runtime::{Connection, ConnectionSnapshot};
use infiltrator_ports::runtime_gateway::{RuntimeGateway, RuntimeStream};
use std::sync::Arc;

#[derive(Clone)]
pub struct ConnectionApplication {
    gateway: Arc<dyn RuntimeGateway>,
}

impl ConnectionApplication {
    pub fn new(gateway: Arc<dyn RuntimeGateway>) -> Self {
        Self { gateway }
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

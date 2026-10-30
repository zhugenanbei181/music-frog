//! Outbound port definition for DNS query history streams and clearing (FLPARITY-004).

use async_trait::async_trait;
use infiltrator_contract::dns_query_history::DnsQueryHistoryRecord;
use lurker_core_ports::error::PortError;

/// Port abstraction for observing and managing historic DNS queries from the running engine.
#[async_trait]
pub trait DnsQueryHistoryPort: Send + Sync {
    /// Poll or fetch latest historic query records observed by the controller.
    async fn fetch_recent_queries(&self) -> Result<Vec<DnsQueryHistoryRecord>, PortError>;

    /// Clear the controller's internal DNS query stream buffer.
    async fn clear_query_stream(&self) -> Result<(), PortError>;
}

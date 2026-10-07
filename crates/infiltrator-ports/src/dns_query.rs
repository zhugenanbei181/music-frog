//! Only the running controller's DNS query; no UI or full host handle.
use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::dns_query::{DnsQueryRequest, DnsQueryResponse};

#[async_trait]
pub trait DnsQueryPort: Send + Sync {
    async fn query(&self, request: &DnsQueryRequest) -> Result<DnsQueryResponse, PortError>;
}

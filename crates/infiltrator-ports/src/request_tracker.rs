//! Outbound port definition for real-time and historic network requests (FLPARITY-003).

use async_trait::async_trait;
use infiltrator_contract::request_tracker::RequestRecord;
use lurker_core_ports::error::PortError;

/// Port abstraction for observing and managing network request events from the running engine.
#[async_trait]
pub trait RequestTrackerPort: Send + Sync {
    /// Poll or fetch latest request records observed by the controller.
    async fn fetch_recent_requests(&self) -> Result<Vec<RequestRecord>, PortError>;

    /// Clear the controller's internal request / connection tracking stream buffer.
    async fn clear_request_stream(&self) -> Result<(), PortError>;
}

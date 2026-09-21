//! Host-owned persistence seam for the bounded speedtest run history.
//!
//! The shared `SpeedtestApplication` owns the canonical snapshot; this port is
//! the zero-toolkit seam a host uses to survive a process restart without the
//! application layer depending on a filesystem or async runtime.

use infiltrator_contract::speedtest::HistoricalSpeedtestRecord;

use crate::error::PortError;

/// Durable store for the most recent speedtest run summaries.
pub trait SpeedtestHistoryStore: Send + Sync {
    /// Load the persisted records, oldest first. A missing store is empty.
    fn load(&self) -> Result<Vec<HistoricalSpeedtestRecord>, PortError>;

    /// Persist the bounded records, oldest first.
    fn save(&self, records: &[HistoricalSpeedtestRecord]) -> Result<(), PortError>;
}

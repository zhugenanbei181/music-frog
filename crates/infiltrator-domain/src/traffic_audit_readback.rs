//! The shared traffic-audit read model.
//!
//! The accumulator answers one `(dimension, window)` ask with these facts;
//! every surface consumes the same typed answer instead of importing an
//! application-private DTO or folding a second ranking.

use crate::traffic_audit::{TrafficDimension, TrafficStats, TrafficWindow};

/// Entity key for traffic whose process the core never named.
pub const UNATTRIBUTED_PROCESS_ENTITY: &str = "[System / Unknown]";

/// One ranked entity of a window.
#[derive(Clone, Debug, PartialEq)]
pub struct TrafficAuditRow {
    pub entity: String,
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub total_bytes: u64,
    /// Share of the window's whole traffic, in percent.
    pub share_percent: f64,
}

/// One bucket of the whole-traffic trend a window is drawn from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrafficAuditPoint {
    pub bucket_secs: u64,
    pub upload_bytes: u64,
    pub download_bytes: u64,
}

impl TrafficAuditPoint {
    pub fn total_bytes(self) -> u64 {
        self.upload_bytes.saturating_add(self.download_bytes)
    }
}

/// Everything one `(dimension, window)` ask answers with.
#[derive(Clone, Debug, PartialEq)]
pub struct TrafficAuditReadback {
    /// The ask this readback answers, so a reader cannot mistake one window's
    /// rows for another's.
    pub dimension: TrafficDimension,
    pub window: TrafficWindow,
    pub rows: Vec<TrafficAuditRow>,
    pub trend: Vec<TrafficAuditPoint>,
    pub totals: TrafficStats,
}

impl TrafficAuditReadback {
    /// The largest total across the rows, used to scale a row's bar.
    pub fn peak_row_total(&self) -> u64 {
        self.rows.first().map(|row| row.total_bytes).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trend_point_sums_both_directions() {
        let point = TrafficAuditPoint {
            bucket_secs: 300,
            upload_bytes: 7,
            download_bytes: 11,
        };
        assert_eq!(point.total_bytes(), 18);
    }

    #[test]
    fn an_empty_readback_has_no_peak() {
        let readback = TrafficAuditReadback {
            dimension: TrafficDimension::Domain,
            window: TrafficWindow::LastDay,
            rows: Vec::new(),
            trend: Vec::new(),
            totals: TrafficStats::default(),
        };
        assert_eq!(readback.peak_row_total(), 0);
    }
}

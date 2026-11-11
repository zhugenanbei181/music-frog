//! Application-facing traffic audit (TRAFFICAUDIT-001).
//!
//! The domain accumulator owns every bucket; this module owns the two things
//! above it:
//!
//! * the one mapping from a connection observation to a recorded flow - which
//!   entity a connection belongs to in each dimension, and which route it left
//!   through - so a surface never folds a ranking again, and
//! * the readback for one `(dimension, window)` ask: the ranked rows, the
//!   whole-traffic buckets the range is drawn from, and the window totals.
//!
//! A host shares one instance across its surface and its readers (the same way
//! it shares the connection rate book), and the instance is cheap to clone.
//! Nothing here reads a clock: observations and readbacks carry the instant.

use infiltrator_domain::connection_rate::ConnectionRates;
use infiltrator_domain::connection_view::{ConnectionView, connection_host};
use infiltrator_domain::traffic_audit::{
    AuditRouteType, FlowEvent, TrafficAuditAccumulator, TrafficDimension, TrafficStats,
    TrafficWindow,
};
use std::sync::{Arc, Mutex};

/// How long the daily tier is kept: the month window is its longest reader.
pub const DAILY_RETENTION_SECS: u64 = 31 * 86_400;

/// How long the fine tier is kept: the hour window is its only reader.
pub const RECENT_RETENTION_SECS: u64 = 3_600;

/// Entity key for traffic whose process the core never named.
///
/// It is a bucket, not a decoration: the traffic happened, so it belongs to a
/// row the reader can see, and the row says plainly that the process is
/// unknown. Naming it here keeps the string in one place instead of once per
/// surface.
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

/// The recorded buckets and the newest instant they were observed at.
///
/// Both are written by the same observation, so they share one lock: a reader
/// can never see a window cut against an instant the buckets never saw.
#[derive(Default)]
struct AuditInner {
    accumulator: TrafficAuditAccumulator,
    newest_observed_secs: Option<u64>,
}

/// The shared traffic audit: one accumulator behind a lock, cloned by handle.
#[derive(Clone, Default)]
pub struct TrafficAuditApplication {
    inner: Arc<Mutex<AuditInner>>,
}

impl TrafficAuditApplication {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record the byte change one observation measured.
    ///
    /// The rate book derived each connection's delta from the same pair of
    /// observations, so the audit consumes that number instead of diffing the
    /// counters a second time: a connection with no measured change records
    /// nothing, and a connection the core never named in a dimension is simply
    /// not attributed to it.
    pub fn observe<C: ConnectionView>(&self, now_secs: u64, conns: &[C], rates: &ConnectionRates) {
        let mut inner = self.inner.lock().expect("traffic audit accumulator lock");
        for conn in conns {
            let rate = rates.get(conn.view_id());
            if !rate.has_delta() {
                continue;
            }
            let outbound = conn.view_outbound_name();
            let process = conn.view_process_name();
            let event = FlowEvent::new(
                AuditRouteType::from_outbound_name(&outbound),
                rate.upload_delta_bytes,
                rate.download_delta_bytes,
            )
            .with_node(outbound)
            .with_process(if process.is_empty() {
                UNATTRIBUTED_PROCESS_ENTITY
            } else {
                &process
            })
            .with_domain(connection_host(conn))
            .with_device(conn.view_source_ip())
            .with_timestamp(now_secs);
            inner.accumulator.record_flow(&event);
        }
        // Pruning follows the newest observation, never a sample that arrived
        // late: an out-of-order observation must not shorten the windows that
        // already hold newer traffic.
        let newest = inner
            .newest_observed_secs
            .map_or(now_secs, |previous| previous.max(now_secs));
        inner.newest_observed_secs = Some(newest);
        inner
            .accumulator
            .prune_recent_before(newest.saturating_sub(RECENT_RETENTION_SECS));
        inner
            .accumulator
            .prune_older_than(newest.saturating_sub(DAILY_RETENTION_SECS));
    }

    /// Rank one dimension inside one window, with the window's trend and
    /// totals.
    pub fn readback(
        &self,
        dimension: TrafficDimension,
        window: TrafficWindow,
        now_secs: u64,
        limit: usize,
    ) -> TrafficAuditReadback {
        let inner = self.inner.lock().expect("traffic audit accumulator lock");
        let accumulator = &inner.accumulator;
        let totals = accumulator.window_totals(window, now_secs);
        let grand_total = totals.total_bytes();
        let rows = accumulator
            .window_rankings(dimension, window, now_secs, limit)
            .into_iter()
            .map(|(entity, stats)| TrafficAuditRow {
                entity,
                upload_bytes: stats.upload_bytes,
                download_bytes: stats.download_bytes,
                total_bytes: stats.total_bytes(),
                share_percent: if grand_total > 0 {
                    stats.total_bytes() as f64 * 100.0 / grand_total as f64
                } else {
                    0.0
                },
            })
            .collect();
        let trend = accumulator
            .window_series(window, now_secs)
            .into_iter()
            .map(|(bucket_secs, stats)| TrafficAuditPoint {
                bucket_secs,
                upload_bytes: stats.upload_bytes,
                download_bytes: stats.download_bytes,
            })
            .collect();
        TrafficAuditReadback {
            dimension,
            window,
            rows,
            trend,
            totals,
        }
    }

    /// Forget every recorded bucket.
    pub fn clear(&self) {
        let mut inner = self.inner.lock().expect("traffic audit accumulator lock");
        inner.accumulator.clear();
    }
}

#[cfg(test)]
#[path = "traffic_audit_application_test.rs"]
mod traffic_audit_application_test;

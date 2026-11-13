//! Retention owner for connections that disappeared from a live observation.
//!
//! The controller reports active connections, not close events. The only
//! honest local history is therefore the difference between two consecutive
//! observations of the same Core scope: the first slice is a baseline, and an
//! id missing from the next slice is recorded with the last facts it published.
//! A scope change is not a close event, so it starts a new baseline.
//!
//! Wall-clock facts and the retention cap are caller inputs. The history never
//! reads a clock or invents a closed connection that was never observed.

use crate::connection_observation::parse_connection_start;
use crate::connection_view::{ConnectionView, RouteChain};
use chrono::{DateTime, Utc};
use infiltrator_contract::traffic_history::{ClosedConnectionSnapshot, ClosedConnectionsSnapshot};
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::outbound::identity::SnapshotRevision;
use std::collections::HashMap;

/// The most recent retained closures. The total count survives eviction.
pub const DEFAULT_CLOSED_CONNECTION_CAP: usize = 256;

/// Resolve a controller start timestamp against one shared observation clock.
/// Mihomo reports RFC3339; the legacy `%Y-%m-%d %H:%M:%S` form is also read.
/// An unparseable or future start is an unknown duration, never an invented one.
pub fn duration_since_connection_start(start: &str, observed_at: DateTime<Utc>) -> u64 {
    let Some(started_at) = parse_connection_start(start) else {
        return 0;
    };
    observed_at
        .signed_duration_since(started_at)
        .num_milliseconds()
        .try_into()
        .unwrap_or(0)
}

/// The mutually exclusive phases of a scoped observation stream.
#[derive(Clone, Debug, PartialEq, Eq)]
enum HistoryPhase {
    NoBaseline,
    Tracking {
        scope: CoreScope,
        revision: SnapshotRevision,
    },
}

/// The caller-owned facts of one observation.
#[derive(Clone, Copy, Debug)]
pub struct ClosedConnectionObservation {
    pub observed_at: DateTime<Utc>,
    pub duration_of: fn(&str, DateTime<Utc>) -> u64,
}

impl ClosedConnectionObservation {
    pub const fn new(
        observed_at: DateTime<Utc>,
        duration_of: fn(&str, DateTime<Utc>) -> u64,
    ) -> Self {
        Self {
            observed_at,
            duration_of,
        }
    }

    fn closed_at(self) -> String {
        self.observed_at.to_rfc3339()
    }

    fn duration(self, start: &str) -> u64 {
        (self.duration_of)(start, self.observed_at)
    }
}

/// The most recent retained closures, with one typed stream phase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosedConnectionHistory {
    phase: HistoryPhase,
    active: HashMap<String, ClosedConnectionSnapshot>,
    records: Vec<ClosedConnectionSnapshot>,
    total_closed: u64,
    cap: usize,
}

impl Default for ClosedConnectionHistory {
    fn default() -> Self {
        Self {
            phase: HistoryPhase::NoBaseline,
            active: HashMap::new(),
            records: Vec::new(),
            total_closed: 0,
            cap: DEFAULT_CLOSED_CONNECTION_CAP,
        }
    }
}

impl ClosedConnectionHistory {
    pub fn new(cap: usize) -> Self {
        Self {
            cap: cap.max(1),
            ..Self::default()
        }
    }

    /// Fold one live observation into the history.
    ///
    /// `scope` fences the stream: a foreign Core identity starts a new
    /// baseline and discards the accumulated history. `observation` carries
    /// the shared instant and start-time conversion, so every closure in a
    /// slice is stamped against the same clock.
    pub fn observe<C: ConnectionView>(
        &mut self,
        scope: &CoreScope,
        revision: SnapshotRevision,
        connections: &[C],
        observation: ClosedConnectionObservation,
    ) {
        match &mut self.phase {
            HistoryPhase::Tracking {
                scope: tracking_scope,
                revision: tracking_revision,
            } if tracking_scope == scope && revision < *tracking_revision => return,
            HistoryPhase::Tracking {
                scope: tracking_scope,
                revision: tracking_revision,
            } if tracking_scope == scope => {
                *tracking_revision = revision;
            }
            phase => {
                *phase = HistoryPhase::Tracking {
                    scope: scope.clone(),
                    revision,
                };
                self.active.clear();
                self.records.clear();
                self.total_closed = 0;
            }
        }
        let closed_at = observation.closed_at();
        let mut next: HashMap<String, ClosedConnectionSnapshot> =
            HashMap::with_capacity(connections.len());
        for connection in connections {
            next.insert(
                connection.view_id().to_owned(),
                ClosedConnectionSnapshot {
                    id: connection.view_id().to_owned(),
                    destination: connection.view_host().to_owned(),
                    duration_ms: observation.duration(connection.view_start()),
                    upload_bytes: connection.view_upload_total(),
                    download_bytes: connection.view_download_total(),
                    rule: connection.view_rule().to_owned(),
                    outbound: RouteChain::from_hops(connection.view_chain()).display(" → "),
                    closed_at: closed_at.clone(),
                },
            );
        }
        let mut closed: Vec<ClosedConnectionSnapshot> = self
            .active
            .iter()
            .filter(|(id, _)| !next.contains_key(*id))
            .map(|(_, record)| record.clone())
            .collect();
        closed.sort_by(|left, right| left.id.cmp(&right.id));
        self.total_closed = self.total_closed.saturating_add(closed.len() as u64);
        if !closed.is_empty() {
            self.records.extend(closed);
            let overflow = self.records.len().saturating_sub(self.cap);
            if overflow > 0 {
                self.records.drain(..overflow);
            }
        }
        self.active = next;
    }

    /// Authoritative readback: the retained rows and the lifetime total.
    pub fn readback(&self) -> ClosedConnectionsSnapshot {
        ClosedConnectionsSnapshot {
            records: self.records.clone(),
            total_closed: self.total_closed,
        }
    }

    /// Clear the retained rows. Active connections stay as the baseline, so
    /// the next observation cannot re-report the same closures.
    pub fn clear(&mut self) -> ClosedConnectionsSnapshot {
        self.records.clear();
        self.total_closed = 0;
        self.readback()
    }
}

#[cfg(test)]
#[path = "closed_connection_history_test.rs"]
mod tests;

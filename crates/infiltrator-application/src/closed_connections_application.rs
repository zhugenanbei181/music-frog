//! The single closed-connection history owner (CPPARITY-CLOSED-001).
//!
//! The controller publishes only live connections, so a closure is observable
//! as the difference between two consecutive observations of one Core scope.
//! This type owns that difference: exactly one instance lives in
//! `CoreApplication`, the surface reader records into it and answers its
//! readback, and `ClearClosedConnections` resets the same instance. Hosts and
//! surfaces never keep a local closure list.
//!
//! Like the traffic waveform owner, it is cheap to clone (one shared inner
//! state) and never reads a clock outside an observation it was handed.

use crate::observation_clock::accepted_at;
use chrono::{DateTime, Utc};
use infiltrator_contract::traffic_history::ClosedConnectionsSnapshot;
use infiltrator_domain::closed_connection_history::{
    ClosedConnectionHistory, ClosedConnectionObservation, duration_since_connection_start,
};
use infiltrator_ports::connection::ConnectionObservationSnapshot;
use std::sync::{Arc, Mutex};

/// The retained closures, shared by every clone of this handle.
#[derive(Clone, Default)]
pub struct ClosedConnectionsApplication {
    history: Arc<Mutex<ClosedConnectionHistory>>,
}

impl ClosedConnectionsApplication {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one live observation at an injected instant. Every row in the
    /// slice is stamped against that single instant.
    pub fn observe_at(&self, observed: &ConnectionObservationSnapshot, observed_at: DateTime<Utc>) {
        let observation =
            ClosedConnectionObservation::new(observed_at, duration_since_connection_start);
        self.history
            .lock()
            .expect("closed connection history lock")
            .observe(
                observed.scope(),
                observed.revision(),
                observed.snapshot().connections.as_slice(),
                observation,
            );
    }

    /// Record one live observation against the current wall clock. The
    /// surface reader calls this once per accepted scoped observation.
    pub fn observe(&self, observed: &ConnectionObservationSnapshot) {
        self.observe_at(observed, accepted_at());
    }

    /// The authoritative readback published with every surface snapshot.
    pub fn readback(&self) -> ClosedConnectionsSnapshot {
        self.history
            .lock()
            .expect("closed connection history lock")
            .readback()
    }

    /// Forget the retained rows, returning the readback after the reset.
    pub fn clear(&self) -> ClosedConnectionsSnapshot {
        self.history
            .lock()
            .expect("closed connection history lock")
            .clear()
    }
}

#[cfg(test)]
#[path = "closed_connections_application_test.rs"]
mod tests;

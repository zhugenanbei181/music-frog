//! Application-facing instantaneous connection rates (DUAL-13-10 / 13-12).
//!
//! The derivation is the shared [`ConnectionRateDiffer`] of the domain crate.
//! This module owns the two application-side pieces:
//!
//! * the process-wide rate window a composition root shares across clones (the
//!   surface reader observes every runtime snapshot through it), and
//! * the one pure mapping from the runtime connection snapshot plus its rate
//!   book into the shared connections read model, so the surface contract is
//!   filled in exactly one place.
//!
//! A host without two successive observations publishes honest zeros; the
//! mapper never invents a rate.

use crate::connection_search::display_endpoint;
use infiltrator_contract::surface_snapshot;
use infiltrator_domain::connection_rate::{ConnectionRate, ConnectionRateDiffer, ConnectionRates};
use infiltrator_domain::connection_view::ConnectionView;
use infiltrator_domain::runtime::{Connection, ConnectionSnapshot};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Shared per-connection rate window. Cheap to clone; every clone observes the
/// same window and therefore derives identical rates.
#[derive(Clone, Default)]
pub struct ConnectionRateApplication {
    differ: Arc<Mutex<ConnectionRateDiffer>>,
}

impl ConnectionRateApplication {
    pub fn new() -> Self {
        Self::default()
    }

    /// Observe one connection snapshot at `now` and return the rate book
    /// valid at that instant.
    pub fn observe_at<C: ConnectionView>(&self, now: Instant, conns: &[C]) -> ConnectionRates {
        self.differ
            .lock()
            .expect("connection rate differ lock")
            .observe(now, conns)
    }

    /// Forget the current window; the next observation reports zero rates.
    pub fn reset(&self) {
        self.differ
            .lock()
            .expect("connection rate differ lock")
            .reset();
    }
}

/// Publish the runtime connections and their derived rates as the shared
/// connections page read model (DUAL-13-10/12 fill `upload_bps` /
/// `download_bps`, which the runtime DTO does not carry).
pub fn connections_page_snapshot(
    connections: &ConnectionSnapshot,
    rates: &ConnectionRates,
) -> surface_snapshot::ConnectionsPageSnapshot {
    surface_snapshot::ConnectionsPageSnapshot {
        total_connections: connections.connections.len(),
        total_upload_bytes: connections.upload_total,
        total_download_bytes: connections.download_total,
        connections: connections
            .connections
            .iter()
            .map(|connection| project_connection(connection, rates.get(&connection.id)))
            .collect(),
    }
}

/// Fold endpoint identity once; display hosts and copy targets are distinct facts.
pub fn project_connection(
    connection: &Connection,
    rate: ConnectionRate,
) -> surface_snapshot::ConnectionSnapshot {
    let metadata = &connection.metadata;
    let destination_host = if metadata.host.is_empty() {
        &metadata.destination_ip
    } else {
        &metadata.host
    };
    let host = display_endpoint(destination_host, &metadata.destination_port);
    surface_snapshot::ConnectionSnapshot {
        id: connection.id.clone(),
        start: connection.start.clone(),
        destination_host: destination_host.clone(),
        host,
        process: metadata.process_path.clone(),
        rule: connection.rule.clone(),
        rule_payload: connection.rule_payload.clone(),
        chain: connection.chains.join(" -> "),
        chains: connection.chains.clone(),
        network: metadata.network.clone(),
        source_ip: metadata.source_ip.clone(),
        source_port: metadata.source_port.clone(),
        destination_ip: metadata.destination_ip.clone(),
        destination_port: metadata.destination_port.clone(),
        destination_geo_ip: metadata.destination_geo_ip.clone(),
        destination_ip_asn: metadata.destination_ip_asn.clone(),
        rate_observed: rate.observed,
        upload_bps: rate.upload_bps,
        download_bps: rate.download_bps,
        upload_total: connection.upload,
        download_total: connection.download,
    }
}

#[cfg(test)]
#[path = "connection_rate_application_test.rs"]
mod tests;

//! The dashboard's live-connection table, one row per active connection.
//!
//! The reference dashboard renders the live connections as a real table:
//! source, destination, protocol, outbound, download, upload and duration.
//! Every one of those columns is a read of a fact the owner already
//! publishes, formatted once here so no surface re-derives it. The outbound
//! is the last hop of the owner's typed route chain — the contract defines
//! that chain as running from the matched inbound rule through each policy
//! group to the outbound, so reading the final element is a projection, not
//! an inference.
//!
//! The duration column stays the honest unknown: `ConnectionSnapshot.start`
//! is published, but no snapshot carries the moment it was taken, so an
//! elapsed time computed here would measure against the wrong clock. A
//! duration arrives only when the contract carries one (or the snapshot
//! carries its observation time); until then the column renders `—`, the
//! same honesty the dashboard's own runtime duration already keeps.

use infiltrator_contract::surface_snapshot::{ConnectionSnapshot, ConnectionsPageSnapshot};
use infiltrator_domain::connection_view::ConnectionView;
use infiltrator_shared::byte_units::format_bytes;

/// One formatted table row of the dashboard's live connection table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionTableRow {
    pub source: String,
    pub destination: String,
    pub protocol: String,
    pub outbound: String,
    pub download: String,
    pub upload: String,
    pub duration: String,
}

/// The honest value every column keeps when the owner published nothing.
const UNKNOWN: &str = "—";

/// The outbound hop of one connection's typed route chain.
fn chain_outbound(connection: &ConnectionSnapshot) -> String {
    let chain = connection.view_chain();
    if let Some(last) = chain.last() {
        return last.clone();
    }
    let joined = connection.view_joined_chain();
    if joined.is_empty() {
        UNKNOWN.to_owned()
    } else {
        joined.to_owned()
    }
}

/// The local endpoint the connection came from, as the owner reported it.
fn connection_source(connection: &ConnectionSnapshot) -> String {
    // The dashboard overview displays the client endpoint without the
    // ephemeral local source port; in the reference this is just the client IP
    // (e.g. `127.0.0.1`) or `—` when unobserved.
    if !connection.source_ip.is_empty() {
        connection.source_ip.clone()
    } else {
        UNKNOWN.to_owned()
    }
}

/// Format byte amounts for compact tables: strip redundant `.00` when exact.
fn format_table_bytes(bytes: u64) -> String {
    let raw = format_bytes(bytes);
    raw.replace(".00 ", " ")
}

/// The formatted rows of the live connection table, newest page order first.
pub fn connection_table_rows(
    page: &ConnectionsPageSnapshot,
    limit: usize,
) -> Vec<ConnectionTableRow> {
    page.connections
        .iter()
        .take(limit)
        .map(|connection| ConnectionTableRow {
            source: connection_source(connection),
            destination: connection.view_host().to_owned(),
            protocol: if connection.network.is_empty() {
                UNKNOWN.to_owned()
            } else {
                connection.network.to_ascii_uppercase()
            },
            outbound: chain_outbound(connection),
            download: format_table_bytes(connection.view_download_total()),
            upload: format_table_bytes(connection.view_upload_total()),
            // No snapshot carries its own observation time, so an elapsed
            // value would be measured against the wrong clock. The column
            // stays the honest unknown until the contract carries a duration
            // or an observation timestamp.
            duration: UNKNOWN.to_owned(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connection() -> ConnectionSnapshot {
        ConnectionSnapshot {
            id: "conn-1".to_owned(),
            start: "2026-11-05T00:00:00Z".to_owned(),
            destination_host: "api.github.com:443".to_owned(),
            host: "api.github.com".to_owned(),
            process: "git".to_owned(),
            rule: "DomainSuffix".to_owned(),
            rule_payload: "github.com".to_owned(),
            chain: "PROXIES -> HK-01".to_owned(),
            chains: vec!["PROXIES".to_owned(), "HK-01".to_owned()],
            network: "tcp".to_owned(),
            source_ip: "127.0.0.1".to_owned(),
            source_port: "54320".to_owned(),
            destination_ip: "20.205.243.166".to_owned(),
            destination_port: "443".to_owned(),
            destination_geo_ip: None,
            destination_ip_asn: String::new(),
            rate_observation: Default::default(),
            upload_bps: 0.0,
            download_bps: 0.0,
            upload_total: 2 * 1024 * 1024,
            download_total: 48 * 1024 * 1024,
        }
    }

    fn page(connections: Vec<ConnectionSnapshot>) -> ConnectionsPageSnapshot {
        ConnectionsPageSnapshot {
            total_connections: connections.len(),
            total_upload_bytes: 0,
            total_download_bytes: 0,
            connections,
        }
    }

    #[test]
    fn every_column_is_an_owner_read_not_an_invention() {
        let rows = connection_table_rows(&page(vec![connection()]), 7);
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.source, "127.0.0.1");
        assert_eq!(row.destination, "api.github.com:443");
        assert_eq!(row.protocol, "TCP");
        assert_eq!(row.outbound, "HK-01");
        assert_eq!(row.download, "48 MB");
        assert_eq!(row.upload, "2 MB");
    }

    #[test]
    fn the_outbound_is_the_last_hop_of_the_typed_chain() {
        let mut connection = connection();
        connection.chains = vec!["MATCH".to_owned(), "节点选择".to_owned(), "ZGO".to_owned()];
        let rows = connection_table_rows(&page(vec![connection]), 7);
        assert_eq!(rows[0].outbound, "ZGO");
    }

    #[test]
    fn a_connection_without_a_chain_reads_the_joined_fallback() {
        let mut connection = connection();
        connection.chains = Vec::new();
        connection.chain = "DIRECT".to_owned();
        let rows = connection_table_rows(&page(vec![connection]), 7);
        assert_eq!(rows[0].outbound, "DIRECT");
    }

    #[test]
    fn unpublished_facts_stay_the_honest_unknown() {
        let mut connection = connection();
        connection.source_ip = String::new();
        connection.source_port = String::new();
        connection.network = String::new();
        connection.chains = Vec::new();
        connection.chain = String::new();
        let rows = connection_table_rows(&page(vec![connection]), 7);
        assert_eq!(rows[0].source, UNKNOWN);
        assert_eq!(rows[0].protocol, UNKNOWN);
        assert_eq!(rows[0].outbound, UNKNOWN);
        // The duration stays unknown until the contract carries a time base.
        assert_eq!(rows[0].duration, UNKNOWN);
    }

    #[test]
    fn the_limit_caps_the_rows_without_touching_the_page() {
        let connections = vec![connection(), connection(), connection()];
        let rows = connection_table_rows(&page(connections), 2);
        assert_eq!(rows.len(), 2);
    }
}

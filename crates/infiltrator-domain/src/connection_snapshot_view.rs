//! Shared connection snapshot implements the domain reduction seam.
use crate::connection_view::{ConnectionView, bare_host};
use infiltrator_contract::surface_snapshot::ConnectionSnapshot;
use infiltrator_contract::traffic_history::ClosedConnectionSnapshot;

impl ConnectionView for ConnectionSnapshot {
    fn view_id(&self) -> &str {
        &self.id
    }

    fn view_host(&self) -> &str {
        if self.destination_host.is_empty() {
            bare_host(&self.host)
        } else {
            &self.destination_host
        }
    }

    fn view_destination_port(&self) -> &str {
        &self.destination_port
    }
    fn view_rule(&self) -> &str {
        &self.rule
    }

    fn view_start(&self) -> &str {
        &self.start
    }

    fn view_destination_ip(&self) -> &str {
        &self.destination_ip
    }

    fn view_process_path(&self) -> &str {
        &self.process
    }

    fn view_upload_total(&self) -> u64 {
        self.upload_total
    }

    fn view_download_total(&self) -> u64 {
        self.download_total
    }

    fn view_upload_rate_bps(&self) -> f64 {
        self.upload_bps
    }

    fn view_download_rate_bps(&self) -> f64 {
        self.download_bps
    }

    fn view_chain(&self) -> &[String] {
        &self.chains
    }

    fn view_joined_chain(&self) -> &str {
        &self.chain
    }

    fn view_search_terms(&self) -> Vec<&str> {
        let mut terms = vec![
            self.id.as_str(),
            self.view_host(),
            self.process.as_str(),
            self.source_ip.as_str(),
            self.destination_ip.as_str(),
            self.source_port.as_str(),
            self.destination_port.as_str(),
            self.network.as_str(),
            self.rule.as_str(),
            self.rule_payload.as_str(),
        ];
        terms.extend(self.chains.iter().map(String::as_str));
        terms
    }
}

/// The closed connection history is a connection list too.
///
/// A closed record carries fewer facts than a live one — it has no process
/// path and no endpoint — so the accessors answer the honest empty value
/// instead of inventing one. Every fact it does carry (the destination, the
/// rule, the outbound, the totals and the close time) is exposed, so the
/// shared reductions project the closed history exactly like the open one.
impl ConnectionView for ClosedConnectionSnapshot {
    fn view_id(&self) -> &str {
        &self.id
    }

    fn view_host(&self) -> &str {
        &self.destination
    }

    fn view_process_path(&self) -> &str {
        ""
    }

    fn view_upload_total(&self) -> u64 {
        self.upload_bytes
    }

    fn view_download_total(&self) -> u64 {
        self.download_bytes
    }

    fn view_rule(&self) -> &str {
        &self.rule
    }

    fn view_start(&self) -> &str {
        &self.closed_at
    }

    fn view_joined_chain(&self) -> &str {
        &self.outbound
    }

    fn view_search_terms(&self) -> Vec<&str> {
        vec![
            self.id.as_str(),
            self.destination.as_str(),
            self.rule.as_str(),
            self.outbound.as_str(),
            self.closed_at.as_str(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection_view::{
        ConnectionGroupingMode, ConnectionSortKey, aggregate_connections, matches_search,
        sort_connections,
    };

    fn open_connection(
        id: &str,
        destination: &str,
        rule: &str,
        upload: u64,
        download: u64,
    ) -> ConnectionSnapshot {
        ConnectionSnapshot {
            id: id.to_owned(),
            start: "2026-09-22T00:00:00Z".to_owned(),
            destination_host: destination.to_owned(),
            host: destination.to_owned(),
            process: String::new(),
            rule: rule.to_owned(),
            rule_payload: String::new(),
            chain: "PROXY".to_owned(),
            chains: vec!["PROXY".to_owned()],
            network: "tcp".to_owned(),
            source_ip: "192.168.1.5".to_owned(),
            source_port: "50000".to_owned(),
            destination_ip: "1.1.1.1".to_owned(),
            destination_port: "443".to_owned(),
            destination_geo_ip: None,
            destination_ip_asn: String::new(),
            rate_observation: Default::default(),
            upload_bps: 0.0,
            download_bps: 0.0,
            upload_total: upload,
            download_total: download,
        }
    }

    fn closed_connection(
        id: &str,
        destination: &str,
        rule: &str,
        upload: u64,
        download: u64,
    ) -> ClosedConnectionSnapshot {
        ClosedConnectionSnapshot {
            id: id.to_owned(),
            destination: destination.to_owned(),
            duration_ms: 1_000,
            upload_bytes: upload,
            download_bytes: download,
            rule: rule.to_owned(),
            outbound: "PROXY".to_owned(),
            closed_at: "2026-09-22T00:00:00Z".to_owned(),
        }
    }

    #[test]
    fn the_closed_history_projects_like_the_open_one() {
        let open = vec![
            open_connection("c1", "api.github.com", "DomainSuffix", 100, 500),
            open_connection("c2", "cdn.example.org", "GeoIP", 900, 200),
        ];
        let closed = vec![
            closed_connection("c1", "api.github.com", "DomainSuffix", 100, 500),
            closed_connection("c2", "cdn.example.org", "GeoIP", 900, 200),
        ];

        // The keyword search answers the same for both histories.
        for query in ["github", "GEOIP", "example"] {
            assert_eq!(
                matches_search(&open[0], query),
                matches_search(&closed[0], query),
                "the closed history answers the query {query} like the open one"
            );
        }
        assert!(matches_search(&closed[0], "github"));
        assert!(!matches_search(&closed[1], "github"));

        // The sort order is the same.
        let mut open_sorted = open.clone();
        let mut closed_sorted = closed.clone();
        sort_connections(&mut open_sorted, ConnectionSortKey::DownloadDesc);
        sort_connections(&mut closed_sorted, ConnectionSortKey::DownloadDesc);
        assert_eq!(
            open_sorted
                .iter()
                .map(|row| row.view_id())
                .collect::<Vec<_>>(),
            closed_sorted
                .iter()
                .map(|row| row.view_id())
                .collect::<Vec<_>>()
        );
        assert_eq!(closed_sorted[0].view_id(), "c1");

        // The aggregation buckets are the same.
        let open_buckets = aggregate_connections(&open, ConnectionGroupingMode::ByHost);
        let closed_buckets = aggregate_connections(&closed, ConnectionGroupingMode::ByHost);
        assert_eq!(open_buckets.len(), closed_buckets.len());
        assert_eq!(open_buckets[0].key, closed_buckets[0].key);
        assert_eq!(
            open_buckets[0].download_total,
            closed_buckets[0].download_total
        );
    }

    #[test]
    fn a_closed_record_never_invents_the_facts_it_does_not_carry() {
        let closed = closed_connection("c1", "api.github.com", "DomainSuffix", 100, 500);
        assert_eq!(closed.view_host(), "api.github.com");
        assert_eq!(closed.view_upload_total(), 100);
        assert_eq!(closed.view_download_total(), 500);
        assert_eq!(closed.view_rule(), "DomainSuffix");
        assert_eq!(closed.view_joined_chain(), "PROXY");
        assert_eq!(closed.view_start(), "2026-09-22T00:00:00Z");
        // The record carries no process path and no endpoint.
        assert_eq!(closed.view_process_path(), "");
        assert_eq!(closed.view_destination_ip(), "");
        assert_eq!(closed.view_destination_port(), "");
        assert_eq!(closed.view_upload_rate_bps(), 0.0);
        assert_eq!(closed.view_download_rate_bps(), 0.0);
    }
}

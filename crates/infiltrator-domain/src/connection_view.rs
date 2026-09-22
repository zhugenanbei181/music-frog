//! Shared connection-view reduction for both UI surfaces (DUAL-13).
//!
//! Iced reads live connections from the runtime stream while Bevy reads the
//! contract projection, but both present the same list. Keyword search
//! (DUAL-13-13), multi-dimensional aggregation (DUAL-13-02), sorting and the
//! reverse rule draft (DUAL-13-09) are pure reductions on that list, so they
//! live here once and both surfaces call them: the two views cannot drift.
//!
//! [`ConnectionView`] is the narrow accessor seam. It is implemented for the
//! domain [`Connection`] (Iced's live type) and, in the Bevy crate, for its
//! local projection row, so neither surface has to copy its list into a
//! second representation to reuse the reduction.

use crate::runtime::Connection;
use std::collections::HashMap;

/// Multi-dimensional aggregation dimension for the connections view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ConnectionGroupingMode {
    /// Flat stream: every connection rendered on its own row.
    #[default]
    Flat,
    /// Aggregate rows by application process (DUAL-13-02).
    ByProcess,
    /// Aggregate rows by destination host (DUAL-13-02).
    ByHost,
}

impl ConnectionGroupingMode {
    /// The canonical [`Self::Flat`] -> [`Self::ByHost`] order used by both
    /// surfaces' segmented controls.
    pub const ALL: [Self; 3] = [Self::Flat, Self::ByProcess, Self::ByHost];

    /// Stable wire/persistence identifier.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Flat => "flat",
            Self::ByProcess => "by_process",
            Self::ByHost => "by_host",
        }
    }

    /// Parse an identifier produced by [`Self::as_str`], defaulting to flat.
    pub fn from_identifier(value: &str) -> Self {
        match value {
            "by_process" => Self::ByProcess,
            "by_host" => Self::ByHost,
            _ => Self::Flat,
        }
    }

    /// Whether this mode renders individual connection rows.
    pub fn is_flat(self) -> bool {
        matches!(self, Self::Flat)
    }
}

/// Sort dimension for the connections list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ConnectionSortKey {
    /// Highest downloaded total first (default).
    #[default]
    DownloadDesc,
    /// Highest uploaded total first.
    UploadDesc,
    /// Newest connection start first.
    LatestDesc,
    /// Destination host ascending.
    HostAsc,
}

impl ConnectionSortKey {
    /// Stable wire/persistence identifier.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DownloadDesc => "download_desc",
            Self::UploadDesc => "upload_desc",
            Self::LatestDesc => "latest_desc",
            Self::HostAsc => "host_asc",
        }
    }

    /// Parse an identifier produced by [`Self::as_str`], defaulting to
    /// download-descending.
    pub fn from_identifier(value: &str) -> Self {
        match value {
            "upload_desc" => Self::UploadDesc,
            "latest_desc" => Self::LatestDesc,
            "host_asc" => Self::HostAsc,
            _ => Self::DownloadDesc,
        }
    }
}

/// One aggregated bucket produced by [`aggregate_connections`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionAggregate {
    /// Bucket label: process name or destination host.
    pub key: String,
    /// Number of member connections.
    pub count: usize,
    /// Summed uploaded bytes across members.
    pub upload_total: u64,
    /// Summed downloaded bytes across members.
    pub download_total: u64,
}

/// Reverse routing-rule draft derived from a connection (DUAL-13-09).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionRuleSpec {
    /// Rule pattern, e.g. `DOMAIN-SUFFIX,github.com` or `IP-CIDR,1.1.1.1/32`.
    pub pattern: String,
    /// Outbound target for the new rule, e.g. `DIRECT`.
    pub target: String,
}

impl ConnectionRuleSpec {
    /// The full comma-joined rule line a rules editor appends.
    pub fn rule_line(&self) -> String {
        format!("{},{}", self.pattern, self.target)
    }

    /// Whether the source connection carried enough of a destination to build
    /// a rule; an empty pattern must never be reported as a successful draft.
    pub fn is_draftable(&self) -> bool {
        !self.pattern.is_empty()
    }
}

/// Default outbound target for a reverse-drafted rule.
pub const DEFAULT_QUICK_RULE_TARGET: &str = "DIRECT";

/// Accessor seam over a connection row so both surfaces reuse the reductions.
pub trait ConnectionView {
    /// Stable connection identifier.
    fn view_id(&self) -> &str;
    /// Destination host without the port, possibly empty when only an IP is known.
    fn view_host(&self) -> &str;
    /// Full process path, possibly empty.
    fn view_process_path(&self) -> &str;
    /// Cumulative uploaded bytes.
    fn view_upload_total(&self) -> u64;
    /// Cumulative downloaded bytes.
    fn view_download_total(&self) -> u64;
    /// Destination IP, used for rule drafting and host fallback.
    fn view_destination_ip(&self) -> &str {
        ""
    }
    /// Connection start timestamp (string sortable), used by latest-first.
    fn view_start(&self) -> &str {
        ""
    }
    /// All strings a keyword query may match against. Override to widen search.
    fn view_search_terms(&self) -> Vec<&str> {
        vec![self.view_id(), self.view_host(), self.view_process_path()]
    }
}

impl ConnectionView for Connection {
    fn view_id(&self) -> &str {
        &self.id
    }

    fn view_host(&self) -> &str {
        &self.metadata.host
    }

    fn view_process_path(&self) -> &str {
        &self.metadata.process_path
    }

    fn view_upload_total(&self) -> u64 {
        self.upload
    }

    fn view_download_total(&self) -> u64 {
        self.download
    }

    fn view_destination_ip(&self) -> &str {
        &self.metadata.destination_ip
    }

    fn view_start(&self) -> &str {
        &self.start
    }

    fn view_search_terms(&self) -> Vec<&str> {
        let meta = &self.metadata;
        let mut terms = vec![
            self.id.as_str(),
            meta.host.as_str(),
            meta.process_path.as_str(),
            meta.source_ip.as_str(),
            meta.destination_ip.as_str(),
            meta.source_port.as_str(),
            meta.destination_port.as_str(),
            meta.network.as_str(),
            self.rule.as_str(),
            self.rule_payload.as_str(),
        ];
        terms.extend(self.chains.iter().map(String::as_str));
        terms
    }
}

/// Extract a clean executable name from a system process path.
pub fn process_display_name(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let filename = trimmed.rsplit(['/', '\\']).next().unwrap_or(trimmed);
    filename
        .strip_suffix(".exe")
        .unwrap_or(filename)
        .to_string()
}

/// Destination display key: the host when known, otherwise the destination IP.
pub fn connection_host<C: ConnectionView>(conn: &C) -> &str {
    let host = conn.view_host();
    if host.is_empty() {
        conn.view_destination_ip()
    } else {
        host
    }
}

/// Case-insensitive keyword match over the connection's searchable terms
/// (domain, IP, process, rule, chain). An empty query matches everything.
pub fn matches_search<C: ConnectionView>(conn: &C, query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return true;
    }
    let needle = query.to_lowercase();
    conn.view_search_terms()
        .into_iter()
        .any(|term| term.to_lowercase().contains(&needle))
}

/// Sort the connection slice in place using the shared sort key, with stable
/// id tie-breaking so both surfaces render identical order.
pub fn sort_connections<C: ConnectionView>(conns: &mut [C], key: ConnectionSortKey) {
    conns.sort_by(|a, b| {
        let ordering = match key {
            ConnectionSortKey::DownloadDesc => {
                b.view_download_total().cmp(&a.view_download_total())
            }
            ConnectionSortKey::UploadDesc => b.view_upload_total().cmp(&a.view_upload_total()),
            ConnectionSortKey::LatestDesc => b.view_start().cmp(a.view_start()),
            ConnectionSortKey::HostAsc => connection_host(a).cmp(connection_host(b)),
        };
        ordering.then_with(|| a.view_id().cmp(b.view_id()))
    });
}

/// Aggregate the connection slice into sorted buckets for the requested mode.
/// Returns an empty list for [`ConnectionGroupingMode::Flat`].
pub fn aggregate_connections<C: ConnectionView>(
    conns: &[C],
    mode: ConnectionGroupingMode,
) -> Vec<ConnectionAggregate> {
    if mode.is_flat() {
        return Vec::new();
    }
    let mut buckets: HashMap<String, ConnectionAggregate> = HashMap::new();
    for conn in conns {
        let key = match mode {
            ConnectionGroupingMode::ByProcess => {
                let name = process_display_name(conn.view_process_path());
                if name.is_empty() {
                    "unknown".to_string()
                } else {
                    name
                }
            }
            ConnectionGroupingMode::ByHost => connection_host(conn).to_string(),
            ConnectionGroupingMode::Flat => continue,
        };
        let bucket = buckets
            .entry(key.clone())
            .or_insert_with(|| ConnectionAggregate {
                key,
                count: 0,
                upload_total: 0,
                download_total: 0,
            });
        bucket.count += 1;
        bucket.upload_total = bucket.upload_total.saturating_add(conn.view_upload_total());
        bucket.download_total = bucket
            .download_total
            .saturating_add(conn.view_download_total());
    }
    let mut aggregated: Vec<ConnectionAggregate> = buckets.into_values().collect();
    aggregated.sort_by(|a, b| {
        b.upload_total
            .cmp(&a.upload_total)
            .then_with(|| a.key.cmp(&b.key))
    });
    aggregated
}

/// Build a reverse routing-rule draft from a connection (DUAL-13-09). Prefers
/// the domain suffix; falls back to a host IP-CIDR. An empty pattern means the
/// connection carried no usable destination and must not be reported as added.
pub fn quick_rule_spec<C: ConnectionView>(conn: &C, target: &str) -> ConnectionRuleSpec {
    let host = conn.view_host().trim();
    let pattern = if !host.is_empty() {
        format!("DOMAIN-SUFFIX,{host}")
    } else {
        let ip = conn.view_destination_ip().trim();
        if ip.is_empty() {
            String::new()
        } else {
            format!("IP-CIDR,{ip}/32")
        }
    };
    ConnectionRuleSpec {
        pattern,
        target: target.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::ConnectionMetadata;

    fn connection(id: &str, host: &str, ip: &str, process: &str, up: u64, down: u64) -> Connection {
        Connection {
            id: id.to_string(),
            metadata: ConnectionMetadata {
                network: "tcp".to_string(),
                connection_type: "HTTP".to_string(),
                source_ip: "192.168.1.5".to_string(),
                destination_ip: ip.to_string(),
                source_port: "50000".to_string(),
                destination_port: "443".to_string(),
                host: host.to_string(),
                dns_mode: "normal".to_string(),
                process_path: process.to_string(),
                special_proxy: String::new(),
            },
            upload: up,
            download: down,
            start: "2026-09-22T00:00:00Z".to_string(),
            rule: "DomainSuffix".to_string(),
            rule_payload: host.to_string(),
            chains: vec!["PROXY".to_string()],
        }
    }

    #[test]
    fn grouping_mode_round_trips_its_identifier() {
        for mode in ConnectionGroupingMode::ALL {
            assert_eq!(ConnectionGroupingMode::from_identifier(mode.as_str()), mode);
        }
        assert_eq!(
            ConnectionGroupingMode::from_identifier("unknown"),
            ConnectionGroupingMode::Flat
        );
        assert!(ConnectionGroupingMode::Flat.is_flat());
    }

    #[test]
    fn sort_key_round_trips_its_identifier() {
        for key in [
            ConnectionSortKey::DownloadDesc,
            ConnectionSortKey::UploadDesc,
            ConnectionSortKey::LatestDesc,
            ConnectionSortKey::HostAsc,
        ] {
            assert_eq!(ConnectionSortKey::from_identifier(key.as_str()), key);
        }
        assert_eq!(
            ConnectionSortKey::from_identifier("bogus"),
            ConnectionSortKey::DownloadDesc
        );
    }

    #[test]
    fn process_display_name_strips_path_and_extension() {
        assert_eq!(process_display_name("/usr/bin/firefox"), "firefox");
        assert_eq!(
            process_display_name("C:\\Program Files\\Zed\\zed-editor.exe"),
            "zed-editor"
        );
        assert_eq!(process_display_name("   "), "");
    }

    #[test]
    fn search_matches_domain_ip_and_process() {
        let conn = connection(
            "c1",
            "api.github.com",
            "140.82.121.5",
            "/usr/bin/git",
            100,
            200,
        );
        assert!(matches_search(&conn, ""));
        assert!(matches_search(&conn, "github"));
        assert!(matches_search(&conn, "140.82"));
        assert!(matches_search(&conn, "GIT"));
        assert!(!matches_search(&conn, "cloudflare"));
    }

    #[test]
    fn search_falls_back_to_destination_ip_without_host() {
        let conn = connection("c2", "", "119.29.29.29", "/usr/bin/curl", 0, 0);
        assert!(matches_search(&conn, "119.29"));
        assert_eq!(connection_host(&conn), "119.29.29.29");
    }

    #[test]
    fn aggregation_buckets_by_process_then_upload() {
        let conns = vec![
            connection("c1", "a.com", "1.1.1.1", "/usr/bin/git", 10, 100),
            connection("c2", "b.com", "2.2.2.2", "/usr/bin/git", 30, 200),
            connection("c3", "c.com", "3.3.3.3", "/usr/bin/curl", 25, 50),
        ];
        let by_process = aggregate_connections(&conns, ConnectionGroupingMode::ByProcess);
        assert_eq!(by_process.len(), 2);
        assert_eq!(by_process[0].key, "git");
        assert_eq!(by_process[0].count, 2);
        assert_eq!(by_process[0].upload_total, 40);
        assert_eq!(by_process[0].download_total, 300);
        assert_eq!(by_process[1].key, "curl");

        let by_host = aggregate_connections(&conns, ConnectionGroupingMode::ByHost);
        assert_eq!(by_host.len(), 3);
        assert_eq!(by_host[0].key, "b.com");
        assert_eq!(by_host[0].upload_total, 30);

        assert!(aggregate_connections(&conns, ConnectionGroupingMode::Flat).is_empty());
    }

    #[test]
    fn sort_orders_by_download_upload_and_host() {
        let mut conns = vec![
            connection("c1", "b.com", "1.1.1.1", "/bin/a", 100, 500),
            connection("c2", "a.com", "2.2.2.2", "/bin/b", 900, 200),
            connection("c3", "c.com", "3.3.3.3", "/bin/c", 500, 800),
        ];
        sort_connections(&mut conns, ConnectionSortKey::DownloadDesc);
        assert_eq!(
            conns.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            vec!["c3", "c1", "c2"]
        );
        sort_connections(&mut conns, ConnectionSortKey::UploadDesc);
        assert_eq!(conns[0].id, "c2");
        sort_connections(&mut conns, ConnectionSortKey::HostAsc);
        assert_eq!(
            conns.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            vec!["c2", "c1", "c3"]
        );
    }

    #[test]
    fn quick_rule_spec_prefers_domain_then_ip() {
        let domain = connection("c1", "github.com", "1.1.1.1", "/bin/git", 0, 0);
        let spec = quick_rule_spec(&domain, DEFAULT_QUICK_RULE_TARGET);
        assert_eq!(spec.pattern, "DOMAIN-SUFFIX,github.com");
        assert_eq!(spec.rule_line(), "DOMAIN-SUFFIX,github.com,DIRECT");
        assert!(spec.is_draftable());

        let ip_only = connection("c2", "", "8.8.8.8", "/bin/dig", 0, 0);
        assert_eq!(
            quick_rule_spec(&ip_only, "PROXY").rule_line(),
            "IP-CIDR,8.8.8.8/32,PROXY"
        );

        let empty = connection("c3", "", "", "/bin/x", 0, 0);
        assert!(!quick_rule_spec(&empty, DEFAULT_QUICK_RULE_TARGET).is_draftable());
    }
}

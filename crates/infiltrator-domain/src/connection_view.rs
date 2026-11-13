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

use crate::connection_rate::ConnectionRate;
use crate::rules::RuleEntry;
use crate::runtime::Connection;
use infiltrator_contract::rule_override::CreateRuleOverrideIntent;
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
    /// Highest instantaneous download rate first (DUAL-13-12).
    DownloadRateDesc,
    /// Highest instantaneous upload rate first (DUAL-13-12).
    UploadRateDesc,
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
            Self::DownloadRateDesc => "download_rate_desc",
            Self::UploadRateDesc => "upload_rate_desc",
            Self::LatestDesc => "latest_desc",
            Self::HostAsc => "host_asc",
        }
    }

    /// Parse an identifier produced by [`Self::as_str`], defaulting to
    /// download-descending.
    pub fn from_identifier(value: &str) -> Self {
        match value {
            "upload_desc" => Self::UploadDesc,
            "download_rate_desc" => Self::DownloadRateDesc,
            "upload_rate_desc" => Self::UploadRateDesc,
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

/// Parsed route chain for one connection (DUAL-13-06): the ordered hops from
/// the matched inbound rule through each policy group to the outbound node.
///
/// The core reports the chain as an ordered list in the domain [`Connection`]
/// and as a joined string in the surface snapshot, so both surfaces parse and
/// render hops through this one model instead of re-splitting text locally.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RouteChain {
    hops: Vec<String>,
}

impl RouteChain {
    /// Normalize an ordered hop list: trim, drop blanks, and collapse a hop
    /// repeated back-to-back. Pure.
    pub fn from_hops(hops: &[String]) -> Self {
        let mut normalized: Vec<String> = Vec::with_capacity(hops.len());
        for hop in hops {
            let trimmed = hop.trim();
            if trimmed.is_empty() || normalized.last().map(String::as_str) == Some(trimmed) {
                continue;
            }
            normalized.push(trimmed.to_string());
        }
        Self { hops: normalized }
    }

    /// Parse the joined fallback form (`A -> B -> C`, `A → B`) into hops.
    pub fn from_joined(joined: &str) -> Self {
        let split: Vec<String> = joined
            .split(['→', '➔'])
            .flat_map(|part| part.split("->"))
            .map(str::to_string)
            .collect();
        Self::from_hops(&split)
    }

    /// The normalized hops in routing order.
    pub fn hops(&self) -> &[String] {
        &self.hops
    }

    /// Number of hops after normalization.
    pub fn len(&self) -> usize {
        self.hops.len()
    }

    /// Whether the chain carries no hop at all.
    pub fn is_empty(&self) -> bool {
        self.hops.is_empty()
    }

    /// First hop, the matched inbound stage, when present.
    pub fn first(&self) -> Option<&str> {
        self.hops.first().map(String::as_str)
    }

    /// Render the hops with a caller-chosen separator.
    pub fn display(&self, separator: &str) -> String {
        self.hops.join(separator)
    }
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

    /// Convert to a typed `CreateRuleOverrideIntent` for direct application dispatch (FLPARITY-028).
    pub fn to_override_intent(&self) -> Option<CreateRuleOverrideIntent> {
        if !self.is_draftable() {
            return None;
        }
        let (rule_type, payload) = self.pattern.split_once(',')?;
        Some(CreateRuleOverrideIntent::new(
            rule_type,
            payload,
            &self.target,
        ))
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
    /// Instantaneous upload rate in bytes per second (DUAL-13-12). Rows that
    /// only carry cumulative counters keep the honest zero.
    fn view_upload_rate_bps(&self) -> f64 {
        0.0
    }
    /// Instantaneous download rate in bytes per second (DUAL-13-12).
    fn view_download_rate_bps(&self) -> f64 {
        0.0
    }
    /// Destination IP, used for rule drafting and host fallback.
    fn view_destination_ip(&self) -> &str {
        ""
    }
    /// Source address the connection came from, used to attribute traffic to
    /// the device it originated on. Rows that do not carry one keep "".
    fn view_source_ip(&self) -> &str {
        ""
    }
    /// Raw payload of the rule that matched the connection, "" when the core
    /// reported none.
    fn view_rule_payload(&self) -> &str {
        ""
    }
    /// Executable name of the process that opened the connection, "" when the
    /// core reported no path for it.
    ///
    /// The rule lives here because every reader that names a process (the
    /// traffic audit, the connection row, the admin audit) must name it the
    /// same way; a second implementation of "strip the directory and the
    /// extension" is a second answer to one question.
    fn view_process_name(&self) -> String {
        process_display_name(self.view_process_path())
    }
    /// The outbound the connection left through: the head of its chain, else
    /// the matched rule's payload or name, else `DIRECT`.
    fn view_outbound_name(&self) -> String {
        self.view_chain()
            .iter()
            .map(|hop| hop.trim())
            .find(|hop| !hop.is_empty())
            .map(str::to_string)
            .or_else(|| {
                let payload = self.view_rule_payload().trim();
                (!payload.is_empty()).then(|| payload.to_string())
            })
            .or_else(|| {
                let rule = self.view_rule().trim();
                (!rule.is_empty()).then(|| rule.to_string())
            })
            .unwrap_or_else(|| "DIRECT".to_string())
    }
    /// Controller-reported remote port for native endpoint display.
    fn view_destination_port(&self) -> &str {
        ""
    }
    /// The transport protocol the kernel reported ("tcp", "udp", ...).
    fn view_network(&self) -> &str {
        ""
    }
    /// The local port the connection came from.
    fn view_source_port(&self) -> &str {
        ""
    }
    /// The DNS resolution mode the kernel reported for this connection.
    fn view_dns_mode(&self) -> &str {
        ""
    }
    /// The remote endpoint the connection is aimed at.
    ///
    /// The host when the kernel named one, else the destination address; a
    /// connection that reported neither keeps the honest empty.
    fn view_remote_destination(&self) -> String {
        let host = self.view_host().trim();
        if !host.is_empty() {
            return host.to_owned();
        }
        self.view_destination_ip().trim().to_owned()
    }
    /// Matched rule type; empty when it was not reported.
    fn view_rule(&self) -> &str {
        ""
    }
    /// Connection start timestamp (string sortable), used by latest-first.
    fn view_start(&self) -> &str {
        ""
    }
    /// Parsed route-chain hops in routing order (DUAL-13-06).
    fn view_chain(&self) -> &[String] {
        &[]
    }
    /// Joined route-chain fallback for rows that only carry the snapshot form.
    fn view_joined_chain(&self) -> &str {
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

    fn view_source_ip(&self) -> &str {
        &self.metadata.source_ip
    }

    fn view_destination_port(&self) -> &str {
        &self.metadata.destination_port
    }
    fn view_network(&self) -> &str {
        &self.metadata.network
    }
    fn view_source_port(&self) -> &str {
        &self.metadata.source_port
    }
    fn view_dns_mode(&self) -> &str {
        &self.metadata.dns_mode
    }
    fn view_rule(&self) -> &str {
        &self.rule
    }

    fn view_rule_payload(&self) -> &str {
        &self.rule_payload
    }
    fn view_start(&self) -> &str {
        &self.start
    }

    fn view_chain(&self) -> &[String] {
        &self.chains
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

/// A connection row paired with the instantaneous rates of the shared rate
/// book (DUAL-13-10/12). Surfaces that hold domain rows and a separate
/// [`crate::connection_rate::ConnectionRates`] book wrap each row in this view
/// so the one shared sort/threshold reduction sees the same rates as surfaces
/// whose rows already carry them.
pub struct RatedConnection<'a, C: ConnectionView> {
    pub row: &'a C,
    pub rate: ConnectionRate,
}

impl<'a, C: ConnectionView> RatedConnection<'a, C> {
    pub fn new(row: &'a C, rate: ConnectionRate) -> Self {
        Self { row, rate }
    }
}

impl<C: ConnectionView> ConnectionView for RatedConnection<'_, C> {
    fn view_id(&self) -> &str {
        self.row.view_id()
    }

    fn view_host(&self) -> &str {
        self.row.view_host()
    }

    fn view_process_path(&self) -> &str {
        self.row.view_process_path()
    }

    fn view_upload_total(&self) -> u64 {
        self.row.view_upload_total()
    }

    fn view_download_total(&self) -> u64 {
        self.row.view_download_total()
    }

    fn view_upload_rate_bps(&self) -> f64 {
        self.rate.upload_bps
    }

    fn view_download_rate_bps(&self) -> f64 {
        self.rate.download_bps
    }

    fn view_destination_ip(&self) -> &str {
        self.row.view_destination_ip()
    }

    fn view_source_ip(&self) -> &str {
        self.row.view_source_ip()
    }

    fn view_destination_port(&self) -> &str {
        self.row.view_destination_port()
    }

    fn view_source_port(&self) -> &str {
        self.row.view_source_port()
    }

    fn view_network(&self) -> &str {
        self.row.view_network()
    }

    fn view_dns_mode(&self) -> &str {
        self.row.view_dns_mode()
    }

    fn view_rule_payload(&self) -> &str {
        self.row.view_rule_payload()
    }

    fn view_rule(&self) -> &str {
        self.row.view_rule()
    }

    fn view_start(&self) -> &str {
        self.row.view_start()
    }

    fn view_chain(&self) -> &[String] {
        self.row.view_chain()
    }

    fn view_joined_chain(&self) -> &str {
        self.row.view_joined_chain()
    }

    fn view_search_terms(&self) -> Vec<&str> {
        self.row.view_search_terms()
    }
}

/// Destination display key: the host when known, otherwise the destination IP.
///
/// Both halves are trimmed: a host the core reported with padding is still a
/// host, and a padded address must not become a different entity name than the
/// same address reported cleanly.
pub fn connection_host<C: ConnectionView>(conn: &C) -> &str {
    let host = conn.view_host().trim();
    if host.is_empty() {
        conn.view_destination_ip().trim()
    } else {
        host
    }
}

/// Executable name of a process path: the last path component, without a
/// Windows `.exe` suffix. An empty or separator-only path has no name.
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

/// Format a destination host and port, adding IPv6 brackets only when needed.
pub fn display_endpoint(host: &str, port: &str) -> String {
    if port.is_empty() {
        host.into()
    } else if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// Parse a connection's route chain into the shared hop model (DUAL-13-06).
/// Falls back to parsing the joined snapshot form when no structured hops are
/// available, so both surfaces render identical stages.
pub fn route_chain<C: ConnectionView>(conn: &C) -> RouteChain {
    let hops = conn.view_chain();
    if hops.is_empty() {
        RouteChain::from_joined(conn.view_joined_chain())
    } else {
        RouteChain::from_hops(hops)
    }
}

/// Case-insensitive keyword match over the connection's searchable terms
/// (domain, IP, process, rule, chain). An empty query matches everything.
pub fn matching_term<'a, C: ConnectionView>(conn: &'a C, query: &str) -> Option<&'a str> {
    let query = query.trim();
    if query.is_empty() {
        return None;
    }
    let needle = query.to_lowercase();
    conn.view_search_terms()
        .into_iter()
        .find(|term| term.to_lowercase().contains(&needle))
}
pub fn matches_search<C: ConnectionView>(conn: &C, query: &str) -> bool {
    query.trim().is_empty() || matching_term(conn, query).is_some()
}

/// Sort the connection slice in place using the shared sort key, with stable
/// id tie-breaking so both surfaces render identical order. The rate keys
/// (DUAL-13-12) rank by the instantaneous accessors, which read zero for rows
/// that carry only cumulative counters.
pub fn sort_connections<C: ConnectionView>(conns: &mut [C], key: ConnectionSortKey) {
    conns.sort_by(|a, b| {
        let ordering = match key {
            ConnectionSortKey::DownloadDesc => {
                b.view_download_total().cmp(&a.view_download_total())
            }
            ConnectionSortKey::UploadDesc => b.view_upload_total().cmp(&a.view_upload_total()),
            ConnectionSortKey::DownloadRateDesc => b
                .view_download_rate_bps()
                .total_cmp(&a.view_download_rate_bps()),
            ConnectionSortKey::UploadRateDesc => b
                .view_upload_rate_bps()
                .total_cmp(&a.view_upload_rate_bps()),
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

/// Strip a trailing `:port` from a `host:port` display key, leaving a bare
/// domain or IPv4 host. IPv6 hosts (which contain multiple colons) are
/// returned unchanged. Pure.
pub fn bare_host(host: &str) -> &str {
    match host.rsplit_once(':') {
        Some((prefix, port))
            if !prefix.is_empty()
                && !prefix.contains(':')
                && !port.is_empty()
                && port.chars().all(|c| c.is_ascii_digit()) =>
        {
            prefix
        }
        _ => host,
    }
}

/// Build a reverse routing-rule draft from a connection (DUAL-13-09). Prefers
/// the domain suffix; falls back to a host IP-CIDR. An empty pattern means the
/// connection carried no usable destination and must not be reported as added.
pub fn quick_rule_spec<C: ConnectionView>(conn: &C, target: &str) -> ConnectionRuleSpec {
    let host = bare_host(conn.view_host().trim()).trim();
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

/// Build a rules-editor entry from a reverse-drafted spec (DUAL-13-09). An
/// un-draftable spec (no usable destination) yields `None` so an empty pattern
/// is never reported as an added rule.
pub fn draft_rule_entry(spec: &ConnectionRuleSpec) -> Option<RuleEntry> {
    spec.is_draftable().then(|| RuleEntry {
        rule: spec.rule_line(),
        enabled: true,
    })
}

/// Append a reverse-drafted rule to an editor draft list, de-duplicating by
/// rule line. Returns the appended entry, or `None` when the spec was empty or
/// the identical rule line already existed. Both surfaces call this one seam.
pub fn append_draft_rule(
    rules: &mut Vec<RuleEntry>,
    spec: &ConnectionRuleSpec,
) -> Option<RuleEntry> {
    let entry = draft_rule_entry(spec)?;
    if rules.iter().any(|existing| existing.rule == entry.rule) {
        return None;
    }
    rules.push(entry.clone());
    Some(entry)
}

/// DUAL-13-05: what the kernel's `/connections` `destinationGeoIP` field
/// really says about this connection's target IP.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DestinationGeoFact {
    /// The kernel never evaluated a GEOIP rule for this connection (the field
    /// was `null`).
    NotEvaluated,
    /// The kernel evaluated a GEOIP rule and had no record for the IP (the
    /// field was `[]`).
    NoResult,
    /// The country codes the kernel resolved, in kernel order.
    Codes(Vec<String>),
}

/// DUAL-13-05: what the kernel's `/connections` `destinationIPASN` field
/// really says about this connection's target IP.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DestinationAsnFact {
    /// No IP-ASN rule ran for this connection (the field was empty).
    NotEvaluated,
    /// An IP-ASN rule ran and the kernel's ASN database had no record for the
    /// IP (the field was whitespace-only).
    NoResult,
    /// The kernel's raw value, e.g. `15169 Google LLC`. The client adds no
    /// `AS` prefix and substitutes nothing.
    Reported(String),
}

/// Classify the kernel's raw `destinationGeoIP` value. `None` is the kernel's
/// `null`; `Some(vec![])` is its empty slice.
pub fn destination_geo_fact(codes: Option<&[String]>) -> DestinationGeoFact {
    match codes {
        None => DestinationGeoFact::NotEvaluated,
        Some([]) => DestinationGeoFact::NoResult,
        Some(codes) => DestinationGeoFact::Codes(codes.to_vec()),
    }
}

/// Classify the kernel's raw `destinationIPASN` value. mihomo writes `""` when
/// no IP-ASN rule ran and `" "` (number plus space plus organization) when it
/// looked up the IP without a record.
pub fn destination_asn_fact(raw: &str) -> DestinationAsnFact {
    let trimmed = raw.trim();
    if raw.is_empty() {
        DestinationAsnFact::NotEvaluated
    } else if trimmed.is_empty() {
        DestinationAsnFact::NoResult
    } else {
        DestinationAsnFact::Reported(trimmed.to_owned())
    }
}

#[cfg(test)]
#[path = "connection_view_test.rs"]
mod tests;

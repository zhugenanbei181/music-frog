//! Request tracking, real-time requests stream, and historic request contract (FLPARITY-003).
//!
//! Defines domain models for requests, tracking execution states, ring buffer capacity,
//! multi-dimensional filtering, and surface snapshots matching FlClash v0.9.2 parity.

use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};

/// Default maximum number of historic requests retained in memory.
pub const DEFAULT_REQUEST_HISTORY_CAPACITY: usize = 1000;

/// Recording state for the request tracker buffer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestTrackingState {
    /// Actively recording newly observed network requests.
    #[default]
    Active,
    /// Request recording is temporarily paused.
    Paused,
}

impl RequestTrackingState {
    pub const fn is_paused(&self) -> bool {
        matches!(self, Self::Paused)
    }

    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
}

/// Lifecycle status of an individual request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestStatus {
    /// Request connection is open and active.
    #[default]
    Active,
    /// Request closed normally.
    Closed,
    /// Request rejected or dropped by firewall or rule.
    Rejected,
    /// Request failed due to network error or timeout.
    Failed,
}

impl RequestStatus {
    /// Localized Chinese description.
    pub fn label_zh(&self) -> &'static str {
        match self {
            Self::Active => "活跃中",
            Self::Closed => "已关闭",
            Self::Rejected => "已拦截",
            Self::Failed => "失败",
        }
    }

    /// User-facing English label.
    pub fn label_en(&self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Closed => "Closed",
            Self::Rejected => "Rejected",
            Self::Failed => "Failed",
        }
    }

    /// Semantic badge color role.
    pub const fn badge_color_role(&self) -> &'static str {
        match self {
            Self::Active => "success",
            Self::Closed => "dim",
            Self::Rejected => "warning",
            Self::Failed => "danger",
        }
    }

    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
}

impl Display for RequestStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label_en())
    }
}

/// Transport network protocol of the request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestNetwork {
    #[default]
    Tcp,
    Udp,
    Other,
}

impl RequestNetwork {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Tcp => "TCP",
            Self::Udp => "UDP",
            Self::Other => "OTHER",
        }
    }
}

impl Display for RequestNetwork {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Single historic or live network request record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestRecord {
    /// Unique request or connection identifier.
    pub id: String,
    /// Occurrence epoch timestamp in milliseconds.
    pub timestamp_epoch_ms: u64,
    /// Destination host or target address (e.g. "api.github.com:443").
    pub host: String,
    /// Transport network.
    pub network: RequestNetwork,
    /// Protocol type (e.g. "HTTPS", "HTTP", "DNS", "SOCKS5").
    pub type_name: String,
    /// Source IP and port (e.g. "127.0.0.1:54321").
    pub source_endpoint: String,
    /// Destination IP and port (e.g. "140.82.121.4:443").
    pub destination_endpoint: String,
    /// Inbound name (e.g. "mixed-in", "tun-in").
    pub inbound: String,
    /// Matched rule name (e.g. "DomainSuffix", "Match", "GeoIP").
    pub rule: String,
    /// Matched rule payload (e.g. "github.com").
    pub rule_payload: String,
    /// Outbound node or proxy group name.
    pub outbound: String,
    /// Complete proxy chain traversal list.
    pub outbound_chain: Vec<String>,
    /// Accumulated upload bytes.
    pub upload_bytes: u64,
    /// Accumulated download bytes.
    pub download_bytes: u64,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
    /// Client process name if available.
    pub process: Option<String>,
    /// Absolute process path if available.
    pub process_path: Option<String>,
    /// Outcome status.
    pub status: RequestStatus,
}

impl RequestRecord {
    pub fn new(
        id: impl Into<String>,
        timestamp_epoch_ms: u64,
        host: impl Into<String>,
        network: RequestNetwork,
        outbound: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            timestamp_epoch_ms,
            host: host.into(),
            network,
            type_name: "TCP".to_owned(),
            source_endpoint: String::new(),
            destination_endpoint: String::new(),
            inbound: "mixed-in".to_owned(),
            rule: "Match".to_owned(),
            rule_payload: String::new(),
            outbound: outbound.into(),
            outbound_chain: Vec::new(),
            upload_bytes: 0,
            download_bytes: 0,
            duration_ms: 0,
            process: None,
            process_path: None,
            status: RequestStatus::Active,
        }
    }

    pub fn with_type_name(mut self, type_name: impl Into<String>) -> Self {
        self.type_name = type_name.into();
        self
    }

    pub fn with_endpoints(
        mut self,
        source: impl Into<String>,
        destination: impl Into<String>,
    ) -> Self {
        self.source_endpoint = source.into();
        self.destination_endpoint = destination.into();
        self
    }

    pub fn with_rule(mut self, rule: impl Into<String>, payload: impl Into<String>) -> Self {
        self.rule = rule.into();
        self.rule_payload = payload.into();
        self
    }

    pub fn with_chain(mut self, chain: Vec<String>) -> Self {
        self.outbound_chain = chain;
        self
    }

    pub fn with_traffic(mut self, upload: u64, download: u64, duration_ms: u64) -> Self {
        self.upload_bytes = upload;
        self.download_bytes = download;
        self.duration_ms = duration_ms;
        self
    }

    pub fn with_process(mut self, name: impl Into<String>, path: Option<String>) -> Self {
        self.process = Some(name.into());
        self.process_path = path;
        self
    }

    pub fn with_status(mut self, status: RequestStatus) -> Self {
        self.status = status;
        self
    }
}

/// Status filtering category for request history.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestStatusFilter {
    /// Return all request records.
    #[default]
    All,
    /// Return only active open connections.
    ActiveOnly,
    /// Return only closed connections.
    ClosedOnly,
    /// Return only rejected or failed requests.
    ErrorsOnly,
}

impl RequestStatusFilter {
    pub fn matches(&self, status: RequestStatus) -> bool {
        match self {
            Self::All => true,
            Self::ActiveOnly => status == RequestStatus::Active,
            Self::ClosedOnly => status == RequestStatus::Closed,
            Self::ErrorsOnly => matches!(status, RequestStatus::Rejected | RequestStatus::Failed),
        }
    }
}

/// Search and filter criteria for request history entries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RequestHistoryFilter {
    /// Optional substring match on host, process, or rule payload.
    pub keyword: Option<String>,
    /// Optional exact transport network match.
    pub network: Option<RequestNetwork>,
    /// Optional exact outbound node / group name match.
    pub outbound: Option<String>,
    /// Status filter category.
    pub status_filter: RequestStatusFilter,
}

impl RequestHistoryFilter {
    pub fn matches(&self, record: &RequestRecord) -> bool {
        if let Some(kw) = &self.keyword {
            let kw_lower = kw.trim().to_ascii_lowercase();
            if !kw_lower.is_empty() {
                let matches_host = record.host.to_ascii_lowercase().contains(&kw_lower);
                let matches_process = record
                    .process
                    .as_deref()
                    .is_some_and(|p| p.to_ascii_lowercase().contains(&kw_lower));
                let matches_payload = record.rule_payload.to_ascii_lowercase().contains(&kw_lower);
                if !matches_host && !matches_process && !matches_payload {
                    return false;
                }
            }
        }
        if self.network.is_some_and(|net| record.network != net) {
            return false;
        }
        if let Some(ob) = &self.outbound {
            let ob_lower = ob.trim().to_ascii_lowercase();
            if !ob_lower.is_empty() && !record.outbound.to_ascii_lowercase().contains(&ob_lower) {
                return false;
            }
        }
        if !self.status_filter.matches(record.status) {
            return false;
        }
        true
    }
}

/// Canonical surface snapshot for requests tracker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestTrackerSnapshot {
    pub generation: u64,
    pub revision: u64,
    pub records: Vec<RequestRecord>,
    pub capacity: usize,
    pub total_requests_observed: u64,
    pub active_count: usize,
    pub tracking_state: RequestTrackingState,
}

impl Default for RequestTrackerSnapshot {
    fn default() -> Self {
        Self {
            generation: 1,
            revision: 1,
            records: Vec::new(),
            capacity: DEFAULT_REQUEST_HISTORY_CAPACITY,
            total_requests_observed: 0,
            active_count: 0,
            tracking_state: RequestTrackingState::Active,
        }
    }
}

impl RequestTrackerSnapshot {
    pub fn demo_fixture() -> Self {
        let records = vec![
            RequestRecord::new(
                "req-1",
                1741300000000,
                "api.github.com:443",
                RequestNetwork::Tcp,
                "HongKong-01",
            )
            .with_type_name("HTTPS")
            .with_rule("DomainSuffix", "github.com")
            .with_traffic(1024, 8192, 450)
            .with_process("git", Some("/usr/bin/git".to_owned()))
            .with_status(RequestStatus::Closed),
            RequestRecord::new(
                "req-2",
                1741300001000,
                "chatgpt.com:443",
                RequestNetwork::Tcp,
                "US-SiliconValley",
            )
            .with_type_name("HTTPS")
            .with_rule("DomainSuffix", "chatgpt.com")
            .with_traffic(4096, 65536, 1200)
            .with_process("chrome", None)
            .with_status(RequestStatus::Active),
            RequestRecord::new(
                "req-3",
                1741300002000,
                "blocked-ads.tracking.com:80",
                RequestNetwork::Tcp,
                "REJECT",
            )
            .with_type_name("HTTP")
            .with_rule("RuleSet", "reject-ads")
            .with_status(RequestStatus::Rejected),
        ];

        Self {
            generation: 1,
            revision: 3,
            records,
            capacity: DEFAULT_REQUEST_HISTORY_CAPACITY,
            total_requests_observed: 3,
            active_count: 1,
            tracking_state: RequestTrackingState::Active,
        }
    }

    pub const fn is_tracking_paused(&self) -> bool {
        self.tracking_state.is_paused()
    }

    pub fn filter<'a>(&'a self, filter: &RequestHistoryFilter) -> Vec<&'a RequestRecord> {
        self.records.iter().filter(|r| filter.matches(r)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_status_labels_and_colors() {
        assert_eq!(RequestStatus::Active.label_zh(), "活跃中");
        assert_eq!(RequestStatus::Active.badge_color_role(), "success");
        assert!(RequestStatus::Active.is_active());

        assert_eq!(RequestStatus::Closed.label_zh(), "已关闭");
        assert_eq!(RequestStatus::Closed.badge_color_role(), "dim");
        assert!(!RequestStatus::Closed.is_active());

        assert_eq!(RequestStatus::Rejected.label_zh(), "已拦截");
        assert_eq!(RequestStatus::Rejected.badge_color_role(), "warning");

        assert_eq!(RequestStatus::Failed.label_zh(), "失败");
        assert_eq!(RequestStatus::Failed.badge_color_role(), "danger");
    }

    #[test]
    fn filter_matching() {
        let snap = RequestTrackerSnapshot::demo_fixture();

        let kw_filter = RequestHistoryFilter {
            keyword: Some("github".to_owned()),
            ..Default::default()
        };
        let matches = snap.filter(&kw_filter);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, "req-1");

        let active_filter = RequestHistoryFilter {
            status_filter: RequestStatusFilter::ActiveOnly,
            ..Default::default()
        };
        let active = snap.filter(&active_filter);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, "req-2");

        let err_filter = RequestHistoryFilter {
            status_filter: RequestStatusFilter::ErrorsOnly,
            ..Default::default()
        };
        let errs = snap.filter(&err_filter);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].id, "req-3");
    }

    #[test]
    fn snapshot_demo_fixture() {
        let snap = RequestTrackerSnapshot::demo_fixture();
        assert_eq!(snap.records.len(), 3);
        assert_eq!(snap.total_requests_observed, 3);
        assert_eq!(snap.active_count, 1);
        assert!(!snap.is_tracking_paused());
    }
}

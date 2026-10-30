//! DNS query history, response observation, and live tracker contract (FLPARITY-004).
//!
//! Defines domain models for DNS query records, execution statuses, ring buffer
//! capacity, search filtering, and surface snapshots matching FlClash v0.9.2 parity.

use crate::dns_query::DnsRecordType;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};

/// Default maximum number of DNS query records retained in memory.
pub const DEFAULT_DNS_HISTORY_CAPACITY: usize = 500;

/// Recording state for the DNS query history buffer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsHistoryRecordingState {
    /// Actively recording newly observed DNS queries.
    #[default]
    Active,
    /// Query recording is temporarily paused.
    Paused,
}

impl DnsHistoryRecordingState {
    pub const fn is_paused(&self) -> bool {
        matches!(self, Self::Paused)
    }

    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
}

/// Status filtering category for DNS query history queries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsQueryStatusFilter {
    /// Return all query outcomes.
    #[default]
    All,
    /// Return only non-successful or error queries.
    ErrorsOnly,
    /// Return only cache hit queries.
    CacheHitsOnly,
}

impl DnsQueryStatusFilter {
    pub fn matches(&self, record: &DnsQueryHistoryRecord) -> bool {
        match self {
            Self::All => true,
            Self::ErrorsOnly => !record.status.is_successful(),
            Self::CacheHitsOnly => record.is_cache_hit(),
        }
    }
}

/// Outcome status of a resolved DNS query.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsQueryStatus {
    /// Successfully resolved from upstream with answer records.
    Success,
    /// Served from local Fake-IP or DNS controller cache.
    CacheHit,
    /// Non-existent domain (rcode 3, NXDOMAIN).
    NxDomain,
    /// Query refused by upstream resolver (rcode 5, REFUSED).
    Refused,
    /// Upstream server failure (rcode 2, SERVFAIL).
    ServerFailure,
    /// Query timed out waiting for upstream reply.
    Timeout,
    /// Network connection or protocol error.
    NetworkError(String),
}

impl DnsQueryStatus {
    /// Localized Chinese description.
    pub fn label_zh(&self) -> &'static str {
        match self {
            Self::Success => "解析成功",
            Self::CacheHit => "命中缓存",
            Self::NxDomain => "域名不存在",
            Self::Refused => "请求被拒绝",
            Self::ServerFailure => "服务器故障",
            Self::Timeout => "查询超时",
            Self::NetworkError(_) => "网络错误",
        }
    }

    /// User-facing English label.
    pub fn label_en(&self) -> &'static str {
        match self {
            Self::Success => "Success",
            Self::CacheHit => "Cache Hit",
            Self::NxDomain => "NXDOMAIN",
            Self::Refused => "Refused",
            Self::ServerFailure => "Server Failure",
            Self::Timeout => "Timeout",
            Self::NetworkError(_) => "Error",
        }
    }

    /// Semantic badge color role.
    pub const fn badge_color_role(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::CacheHit => "accent",
            Self::NxDomain => "warning",
            Self::Refused | Self::ServerFailure => "danger",
            Self::Timeout | Self::NetworkError(_) => "dim",
        }
    }

    /// Whether this query resolved successfully or from cache.
    pub const fn is_successful(&self) -> bool {
        matches!(self, Self::Success | Self::CacheHit)
    }
}

impl Display for DnsQueryStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label_en())
    }
}

/// Single historic DNS query record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsQueryHistoryRecord {
    /// Monotonically increasing record sequence identifier.
    pub id: u64,
    /// Query occurrence epoch timestamp in milliseconds.
    pub timestamp_epoch_ms: u64,
    /// Queried domain name (e.g. "api.github.com").
    pub name: String,
    /// Queried DNS record type (A, AAAA, CNAME, etc.).
    pub record_type: DnsRecordType,
    /// Outcome status.
    pub status: DnsQueryStatus,
    /// DNS protocol response code (0 = NOERROR, 2 = SERVFAIL, 3 = NXDOMAIN, etc.).
    pub rcode: u16,
    /// Round-trip latency in milliseconds.
    pub latency_ms: u32,
    /// Resolved answer records or IP addresses.
    pub answers: Vec<String>,
    /// Upstream DNS server identifier or endpoint (e.g. "tls://1.1.1.1:853").
    pub server: Option<String>,
}

impl DnsQueryHistoryRecord {
    pub fn new(
        id: u64,
        timestamp_epoch_ms: u64,
        name: impl Into<String>,
        record_type: DnsRecordType,
        status: DnsQueryStatus,
        latency_ms: u32,
    ) -> Self {
        let rcode = match &status {
            DnsQueryStatus::Success | DnsQueryStatus::CacheHit => 0,
            DnsQueryStatus::ServerFailure => 2,
            DnsQueryStatus::NxDomain => 3,
            DnsQueryStatus::Refused => 5,
            _ => 1, // Format error or generic failure
        };
        Self {
            id,
            timestamp_epoch_ms,
            name: name.into(),
            record_type,
            status,
            rcode,
            latency_ms,
            answers: Vec::new(),
            server: None,
        }
    }

    pub fn with_answers(mut self, answers: Vec<String>) -> Self {
        self.answers = answers;
        self
    }

    pub fn with_server(mut self, server: impl Into<String>) -> Self {
        self.server = Some(server.into());
        self
    }

    /// Whether this query answer was served from cache.
    pub const fn is_cache_hit(&self) -> bool {
        matches!(self.status, DnsQueryStatus::CacheHit)
    }
}

/// Filter criteria for querying DNS history entries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DnsQueryHistoryFilter {
    /// Optional substring match on queried domain name.
    pub keyword: Option<String>,
    /// Optional exact record type match.
    pub record_type: Option<DnsRecordType>,
    /// Filter category by status (all, errors only, cache hits only).
    pub status_filter: DnsQueryStatusFilter,
}

impl DnsQueryHistoryFilter {
    pub fn matches(&self, record: &DnsQueryHistoryRecord) -> bool {
        if let Some(kw) = &self.keyword {
            let kw_lower = kw.trim().to_ascii_lowercase();
            if !kw_lower.is_empty() && !record.name.to_ascii_lowercase().contains(&kw_lower) {
                return false;
            }
        }
        if self.record_type.is_some_and(|rt| record.record_type != rt) {
            return false;
        }
        if !self.status_filter.matches(record) {
            return false;
        }
        true
    }
}

/// Canonical surface snapshot for DNS query history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsQueryHistorySnapshot {
    pub generation: u64,
    pub revision: u64,
    pub records: Vec<DnsQueryHistoryRecord>,
    pub capacity: usize,
    pub total_queries_observed: u64,
    pub recording_state: DnsHistoryRecordingState,
}

impl Default for DnsQueryHistorySnapshot {
    fn default() -> Self {
        Self {
            generation: 1,
            revision: 1,
            records: Vec::new(),
            capacity: DEFAULT_DNS_HISTORY_CAPACITY,
            total_queries_observed: 0,
            recording_state: DnsHistoryRecordingState::Active,
        }
    }
}

impl DnsQueryHistorySnapshot {
    pub fn demo_fixture() -> Self {
        let records = vec![
            DnsQueryHistoryRecord::new(
                1,
                1741300000000,
                "api.github.com",
                DnsRecordType::A,
                DnsQueryStatus::Success,
                18,
            )
            .with_answers(vec!["140.82.121.4".to_owned()])
            .with_server("tls://1.1.1.1:853"),
            DnsQueryHistoryRecord::new(
                2,
                1741300001000,
                "chatgpt.com",
                DnsRecordType::Https,
                DnsQueryStatus::CacheHit,
                1,
            )
            .with_answers(vec!["104.18.3.161".to_owned()]),
            DnsQueryHistoryRecord::new(
                3,
                1741300002000,
                "nonexistent-test-host.local",
                DnsRecordType::A,
                DnsQueryStatus::NxDomain,
                45,
            )
            .with_server("8.8.8.8"),
        ];

        Self {
            generation: 1,
            revision: 3,
            records,
            capacity: DEFAULT_DNS_HISTORY_CAPACITY,
            total_queries_observed: 3,
            recording_state: DnsHistoryRecordingState::Active,
        }
    }

    pub const fn is_recording_paused(&self) -> bool {
        self.recording_state.is_paused()
    }

    pub fn filter<'a>(&'a self, filter: &DnsQueryHistoryFilter) -> Vec<&'a DnsQueryHistoryRecord> {
        self.records.iter().filter(|r| filter.matches(r)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_status_labels_and_colors() {
        assert_eq!(DnsQueryStatus::Success.label_zh(), "解析成功");
        assert_eq!(DnsQueryStatus::Success.badge_color_role(), "success");
        assert!(DnsQueryStatus::Success.is_successful());

        assert_eq!(DnsQueryStatus::CacheHit.label_zh(), "命中缓存");
        assert_eq!(DnsQueryStatus::CacheHit.badge_color_role(), "accent");
        assert!(DnsQueryStatus::CacheHit.is_successful());

        assert_eq!(DnsQueryStatus::NxDomain.label_zh(), "域名不存在");
        assert_eq!(DnsQueryStatus::NxDomain.badge_color_role(), "warning");
        assert!(!DnsQueryStatus::NxDomain.is_successful());
    }

    #[test]
    fn filter_matching() {
        let rec1 = DnsQueryHistoryRecord::new(
            1,
            1000,
            "api.github.com",
            DnsRecordType::A,
            DnsQueryStatus::Success,
            20,
        );
        let rec2 = DnsQueryHistoryRecord::new(
            2,
            2000,
            "bad.domain.invalid",
            DnsRecordType::A,
            DnsQueryStatus::NxDomain,
            50,
        );

        let kw_filter = DnsQueryHistoryFilter {
            keyword: Some("github".to_owned()),
            ..Default::default()
        };
        assert!(kw_filter.matches(&rec1));
        assert!(!kw_filter.matches(&rec2));

        let err_filter = DnsQueryHistoryFilter {
            status_filter: DnsQueryStatusFilter::ErrorsOnly,
            ..Default::default()
        };
        assert!(!err_filter.matches(&rec1));
        assert!(err_filter.matches(&rec2));

        let cache_filter = DnsQueryHistoryFilter {
            status_filter: DnsQueryStatusFilter::CacheHitsOnly,
            ..Default::default()
        };
        assert!(!cache_filter.matches(&rec1));
        assert!(!cache_filter.matches(&rec2));
    }

    #[test]
    fn snapshot_demo_fixture() {
        let snap = DnsQueryHistorySnapshot::demo_fixture();
        assert_eq!(snap.records.len(), 3);
        assert_eq!(snap.total_queries_observed, 3);
        assert!(!snap.is_recording_paused());

        let errors = snap.filter(&DnsQueryHistoryFilter {
            status_filter: DnsQueryStatusFilter::ErrorsOnly,
            ..Default::default()
        });
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].name, "nonexistent-test-host.local");
    }
}

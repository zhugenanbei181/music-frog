//! IP quality, fraud scoring, and usage type detection contract (FLPARITY-001).
//!
//! Provides canonical typed representations for IP quality facts (usage type,
//! risk score, fraud flags, ASN, geolocation) matching FlClash v0.9.2 parity.

use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};
use std::time::Duration;

/// Cache TTL for successful IP quality records (3 days, matching FlClash v0.9.2).
pub const IP_QUALITY_CACHE_TTL: Duration = Duration::from_secs(3 * 86400);

/// Cooldown period after a probe error before retrying (60 seconds).
pub const IP_QUALITY_ERROR_COOLDOWN: Duration = Duration::from_secs(60);

/// Network usage and classification type of an IP address.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IpUsageType {
    /// Residential broadband / ISP connection (家庭宽带 / 原生住宅).
    Residential,
    /// Mobile cellular carrier network (移动网络 / 蜂窝网络).
    Cellular,
    /// Business enterprise or corporate leased line (企业专线 / 商业专线).
    Business,
    /// Hosting, cloud provider, or datacenter (机房托管 / 数据中心).
    Hosting,
    /// Educational institution or campus network (教育网 / 校园网络).
    Education,
    /// Unknown or unclassified connection type.
    #[default]
    Unknown,
}

impl IpUsageType {
    /// Canonical short identifier.
    pub const fn id(&self) -> &'static str {
        match self {
            Self::Residential => "residential",
            Self::Cellular => "cellular",
            Self::Business => "business",
            Self::Hosting => "hosting",
            Self::Education => "education",
            Self::Unknown => "unknown",
        }
    }

    /// User-facing localized Chinese label.
    pub const fn label_zh(&self) -> &'static str {
        match self {
            Self::Residential => "住宅宽带",
            Self::Cellular => "移动蜂窝",
            Self::Business => "商业专线",
            Self::Hosting => "数据中心",
            Self::Education => "教育网",
            Self::Unknown => "未知类型",
        }
    }

    /// User-facing English label.
    pub const fn label_en(&self) -> &'static str {
        match self {
            Self::Residential => "Residential",
            Self::Cellular => "Cellular",
            Self::Business => "Business",
            Self::Hosting => "Hosting",
            Self::Education => "Education",
            Self::Unknown => "Unknown",
        }
    }

    /// Semantic visual badge color role token.
    pub const fn badge_color_role(&self) -> &'static str {
        match self {
            Self::Residential => "success",
            Self::Cellular => "accent",
            Self::Business => "info",
            Self::Hosting => "warning",
            Self::Education => "accent",
            Self::Unknown => "dim",
        }
    }

    /// Returns true if this is clean residential / consumer broadband.
    pub const fn is_residential(&self) -> bool {
        matches!(self, Self::Residential)
    }
}

impl Display for IpUsageType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label_en())
    }
}

/// Fraud and abuse risk classification level.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum IpRiskLevel {
    /// Clean, very low risk (0 - 15 fraud score).
    #[default]
    Clean,
    /// Low risk (16 - 40 fraud score).
    Low,
    /// Moderate or medium risk (41 - 75 fraud score).
    Medium,
    /// High or critical risk (76 - 100 fraud score).
    High,
}

impl IpRiskLevel {
    /// Canonical short identifier.
    pub const fn id(&self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    /// User-facing localized Chinese label.
    pub const fn label_zh(&self) -> &'static str {
        match self {
            Self::Clean => "安全洁净",
            Self::Low => "低风险",
            Self::Medium => "中度风险",
            Self::High => "高度风险",
        }
    }

    /// User-facing English label.
    pub const fn label_en(&self) -> &'static str {
        match self {
            Self::Clean => "Clean",
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }

    /// Semantic badge color role.
    pub const fn badge_color_role(&self) -> &'static str {
        match self {
            Self::Clean => "success",
            Self::Low => "accent",
            Self::Medium => "warning",
            Self::High => "danger",
        }
    }

    /// Derive risk level from a 0..=100 fraud score.
    pub const fn from_fraud_score(score: u8) -> Self {
        match score {
            0..=15 => Self::Clean,
            16..=40 => Self::Low,
            41..=75 => Self::Medium,
            _ => Self::High,
        }
    }
}

impl Display for IpRiskLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label_en())
    }
}

/// Query lifecycle phase for IP quality queries.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IpQualityStatus {
    /// Unobserved or not yet requested.
    #[default]
    Unknown,
    /// Currently probing.
    Probing,
    /// Probe succeeded with valid record.
    Ready,
    /// In error cooldown window.
    Cooldown,
    /// Upstream rate limit reached.
    RateLimited,
    /// Probe failed with diagnostic reason.
    Failed(String),
    /// Platform or host does not support IP quality probing.
    Unsupported,
}

/// Detailed evaluation record for an IP address.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpQualityRecord {
    /// The probed IP address.
    pub ip: String,
    /// Connection usage classification.
    pub usage_type: IpUsageType,
    /// Fraud / abuse risk level.
    pub risk_level: IpRiskLevel,
    /// Numeric fraud score (0 to 100, if provided by upstream).
    pub fraud_score: Option<u8>,
    /// Whether upstream classifies this IP as a proxy.
    pub is_proxy: bool,
    /// Whether upstream classifies this IP as a VPN endpoint.
    pub is_vpn: bool,
    /// Whether upstream classifies this IP as a Tor exit node.
    pub is_tor: bool,
    /// Whether upstream classifies this IP as datacenter-hosted.
    pub is_datacenter: bool,
    /// Whether this IP has known abuse / malicious activity history.
    pub is_abuser: bool,
    /// Autonomous System Number.
    pub asn: Option<u32>,
    /// Autonomous System Organization name.
    pub as_organization: Option<String>,
    /// ISO 3166-1 alpha-2 country code.
    pub country_code: Option<String>,
    /// City name.
    pub city: Option<String>,
    /// Upstream intelligence provider identifier.
    pub provider: Option<String>,
    /// Probe evaluation epoch timestamp in milliseconds.
    pub checked_at_epoch_ms: u64,
}

impl IpQualityRecord {
    pub fn new(
        ip: impl Into<String>,
        usage_type: IpUsageType,
        risk_level: IpRiskLevel,
        checked_at_epoch_ms: u64,
    ) -> Self {
        let is_datacenter = usage_type == IpUsageType::Hosting;
        Self {
            ip: ip.into(),
            usage_type,
            risk_level,
            fraud_score: None,
            is_proxy: false,
            is_vpn: false,
            is_tor: false,
            is_datacenter,
            is_abuser: false,
            asn: None,
            as_organization: None,
            country_code: None,
            city: None,
            provider: None,
            checked_at_epoch_ms,
        }
    }
}

/// Shared surface snapshot for IP quality presentation across all surfaces.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct IpQualitySnapshot {
    pub generation: u64,
    pub revision: u64,
    pub status: IpQualityStatus,
    pub record: Option<IpQualityRecord>,
    pub failure: Option<String>,
    pub is_cached: bool,
    pub cache_age_seconds: Option<u64>,
}

impl IpQualitySnapshot {
    pub fn demo_fixture() -> Self {
        let record = IpQualityRecord {
            ip: "203.0.113.7".to_owned(),
            usage_type: IpUsageType::Residential,
            risk_level: IpRiskLevel::Clean,
            fraud_score: Some(5),
            is_proxy: false,
            is_vpn: false,
            is_tor: false,
            is_datacenter: false,
            is_abuser: false,
            asn: Some(13335),
            as_organization: Some("Cloudflare / APNIC".to_owned()),
            country_code: Some("HK".to_owned()),
            city: Some("Hong Kong".to_owned()),
            provider: Some("ipapi.is".to_owned()),
            checked_at_epoch_ms: 1741300000000,
        };
        Self {
            generation: 1,
            revision: 1,
            status: IpQualityStatus::Ready,
            record: Some(record),
            failure: None,
            is_cached: true,
            cache_age_seconds: Some(3600),
        }
    }

    pub fn ready(
        generation: u64,
        revision: u64,
        record: IpQualityRecord,
        is_cached: bool,
        cache_age_seconds: Option<u64>,
    ) -> Self {
        Self {
            generation,
            revision,
            status: IpQualityStatus::Ready,
            record: Some(record),
            failure: None,
            is_cached,
            cache_age_seconds,
        }
    }

    pub fn probing(generation: u64, revision: u64) -> Self {
        Self {
            generation,
            revision,
            status: IpQualityStatus::Probing,
            record: None,
            failure: None,
            is_cached: false,
            cache_age_seconds: None,
        }
    }

    pub fn cooldown(
        generation: u64,
        revision: u64,
        cached: Option<IpQualityRecord>,
        age: Option<u64>,
    ) -> Self {
        Self {
            generation,
            revision,
            status: IpQualityStatus::Cooldown,
            record: cached,
            failure: Some("IP probe in cooldown period".to_owned()),
            is_cached: true,
            cache_age_seconds: age,
        }
    }

    pub fn rate_limited(generation: u64, revision: u64, reason: impl Into<String>) -> Self {
        Self {
            generation,
            revision,
            status: IpQualityStatus::RateLimited,
            record: None,
            failure: Some(reason.into()),
            is_cached: false,
            cache_age_seconds: None,
        }
    }

    pub fn failed(generation: u64, revision: u64, reason: impl Into<String>) -> Self {
        let err = reason.into();
        Self {
            generation,
            revision,
            status: IpQualityStatus::Failed(err.clone()),
            record: None,
            failure: Some(err),
            is_cached: false,
            cache_age_seconds: None,
        }
    }

    pub fn unsupported(generation: u64, revision: u64, reason: impl Into<String>) -> Self {
        Self {
            generation,
            revision,
            status: IpQualityStatus::Unsupported,
            record: None,
            failure: Some(reason.into()),
            is_cached: false,
            cache_age_seconds: None,
        }
    }

    pub fn unavailable(generation: u64, revision: u64, reason: impl Into<String>) -> Self {
        Self {
            generation,
            revision,
            status: IpQualityStatus::Unknown,
            record: None,
            failure: Some(reason.into()),
            is_cached: false,
            cache_age_seconds: None,
        }
    }

    pub fn is_drawable(&self) -> bool {
        self.status == IpQualityStatus::Ready && self.record.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_type_labels_and_colors() {
        assert_eq!(IpUsageType::Residential.label_zh(), "住宅宽带");
        assert_eq!(IpUsageType::Residential.badge_color_role(), "success");
        assert!(IpUsageType::Residential.is_residential());

        assert_eq!(IpUsageType::Hosting.label_zh(), "数据中心");
        assert_eq!(IpUsageType::Hosting.badge_color_role(), "warning");
        assert!(!IpUsageType::Hosting.is_residential());
    }

    #[test]
    fn risk_level_derivation() {
        assert_eq!(IpRiskLevel::from_fraud_score(0), IpRiskLevel::Clean);
        assert_eq!(IpRiskLevel::from_fraud_score(15), IpRiskLevel::Clean);
        assert_eq!(IpRiskLevel::from_fraud_score(16), IpRiskLevel::Low);
        assert_eq!(IpRiskLevel::from_fraud_score(40), IpRiskLevel::Low);
        assert_eq!(IpRiskLevel::from_fraud_score(41), IpRiskLevel::Medium);
        assert_eq!(IpRiskLevel::from_fraud_score(75), IpRiskLevel::Medium);
        assert_eq!(IpRiskLevel::from_fraud_score(76), IpRiskLevel::High);
        assert_eq!(IpRiskLevel::from_fraud_score(100), IpRiskLevel::High);
    }

    #[test]
    fn snapshot_demo_fixture_drawable() {
        let snap = IpQualitySnapshot::demo_fixture();
        assert!(snap.is_drawable());
        assert_eq!(snap.status, IpQualityStatus::Ready);
        let rec = snap.record.unwrap();
        assert_eq!(rec.ip, "203.0.113.7");
        assert_eq!(rec.usage_type, IpUsageType::Residential);
        assert_eq!(rec.risk_level, IpRiskLevel::Clean);
        assert_eq!(rec.fraud_score, Some(5));
    }

    #[test]
    fn failed_snapshot_not_drawable() {
        let snap = IpQualitySnapshot::failed(1, 1, "timeout");
        assert!(!snap.is_drawable());
        assert_eq!(snap.status, IpQualityStatus::Failed("timeout".to_owned()));
    }
}

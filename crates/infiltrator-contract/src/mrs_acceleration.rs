//! Shared read model for MRS (Mihomo Rule Set) binary acceleration and provider governance (MRS 二进制规则集治理).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MrsAccelerationStatus {
    #[default]
    Unknown,
    Ready,
    Empty,
    Unsupported,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MrsBehaviorKind {
    Domain,
    IpCidr,
    Classical,
    Unknown,
}

impl MrsBehaviorKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Domain => "domain",
            Self::IpCidr => "ipcidr",
            Self::Classical => "classical",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "domain" => Self::Domain,
            "ipcidr" | "ip-cidr" | "cidr" => Self::IpCidr,
            "classical" => Self::Classical,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MrsCompressionKind {
    #[default]
    None,
    Zstd,
    Gzip,
    Unknown,
}

impl MrsCompressionKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Zstd => "zstd",
            Self::Gzip => "gzip",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "zstd" => Self::Zstd,
            "gzip" => Self::Gzip,
            "none" | "" => Self::None,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MrsItemSnapshot {
    pub name: String,
    pub behavior: MrsBehaviorKind,
    pub format_version: u8,
    pub compression: MrsCompressionKind,
    pub rule_count: u32,
    pub payload_size_bytes: u32,
    pub file_size_bytes: u64,
    pub sha256_digest: Option<String>,
    pub crc32_checksum: Option<u32>,
    pub is_mmap_accelerated: bool,
    pub is_valid: bool,
    pub description: String,
    pub updated_at: String,
    pub source_url: Option<String>,
    pub unpack_supported: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MrsAccelerationSnapshot {
    pub generation: u64,
    pub revision: u64,
    pub status: MrsAccelerationStatus,
    pub failure: Option<String>,
    pub total_providers: usize,
    pub total_accelerated_rules: u64,
    pub total_memory_saved_bytes: u64,
    pub mmap_acceleration_active: bool,
    pub items: Vec<MrsItemSnapshot>,
}

impl MrsAccelerationSnapshot {
    /// Explicit high-fidelity fixture for demo and testing surfaces.
    pub fn demo_fixture() -> Self {
        let items = vec![
            MrsItemSnapshot {
                name: "geoip-cn.mrs".to_owned(),
                behavior: MrsBehaviorKind::IpCidr,
                format_version: 1,
                compression: MrsCompressionKind::None,
                rule_count: 8500,
                payload_size_bytes: 142_800,
                file_size_bytes: 142_864,
                sha256_digest: Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_owned()),
                crc32_checksum: Some(0x9A4C_82F1),
                is_mmap_accelerated: true,
                is_valid: true,
                description: "中国大陆 IPv4/IPv6 地址网段 · mmap 零拷贝高速匹配".to_owned(),
                updated_at: "2026-09-06 12:00".to_owned(),
                source_url: Some("https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geoip/cn.mrs".to_owned()),
                unpack_supported: true,
            },
            MrsItemSnapshot {
                name: "geosite-geolocation-!cn.mrs".to_owned(),
                behavior: MrsBehaviorKind::Domain,
                format_version: 1,
                compression: MrsCompressionKind::Zstd,
                rule_count: 28_400,
                payload_size_bytes: 384_200,
                file_size_bytes: 128_500,
                sha256_digest: Some("cbf529a4d5d4970994f10ade8256247831fed05779099da3e4d00f329b52b323".to_owned()),
                crc32_checksum: Some(0x73B1_2A09),
                is_mmap_accelerated: true,
                is_valid: true,
                description: "非中国大陆主流常用域名列表 · 前缀字典树压缩".to_owned(),
                updated_at: "2026-09-06 06:30".to_owned(),
                source_url: Some("https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/geolocation-!cn.mrs".to_owned()),
                unpack_supported: true,
            },
            MrsItemSnapshot {
                name: "custom-reject-ads.mrs".to_owned(),
                behavior: MrsBehaviorKind::Classical,
                format_version: 1,
                compression: MrsCompressionKind::None,
                rule_count: 572,
                payload_size_bytes: 18_400,
                file_size_bytes: 18_450,
                sha256_digest: Some("a1b2c3d4e5f60718293a4b5c6d7e8f90123456789abcdef0123456789abcdef0".to_owned()),
                crc32_checksum: Some(0x38F2_110E),
                is_mmap_accelerated: true,
                is_valid: true,
                description: "全协议经典规则集 · 广告与隐私追踪拦截".to_owned(),
                updated_at: "2026-09-05 18:00".to_owned(),
                source_url: None,
                unpack_supported: true,
            },
        ];

        let total_rules: u64 = items.iter().map(|i| i.rule_count as u64).sum();
        let total_bytes: u64 = items.iter().map(|i| i.file_size_bytes).sum();
        // Estimated memory savings compared to uncompressed in-memory AST strings (~5x)
        let memory_saved = total_bytes.saturating_mul(4);

        Self {
            generation: 1,
            revision: 1,
            status: MrsAccelerationStatus::Ready,
            failure: None,
            total_providers: items.len(),
            total_accelerated_rules: total_rules,
            total_memory_saved_bytes: memory_saved,
            mmap_acceleration_active: true,
            items,
        }
    }

    pub fn ready(
        generation: u64,
        revision: u64,
        items: Vec<MrsItemSnapshot>,
        mmap_active: bool,
    ) -> Self {
        let total_rules: u64 = items.iter().map(|i| i.rule_count as u64).sum();
        let total_bytes: u64 = items.iter().map(|i| i.file_size_bytes).sum();
        let total_memory_saved_bytes = total_bytes.saturating_mul(4);
        let total_providers = items.len();

        let status = if items.is_empty() {
            MrsAccelerationStatus::Empty
        } else {
            MrsAccelerationStatus::Ready
        };

        Self {
            generation,
            revision,
            status,
            failure: None,
            total_providers,
            total_accelerated_rules: total_rules,
            total_memory_saved_bytes,
            mmap_acceleration_active: mmap_active,
            items,
        }
    }

    pub fn empty(generation: u64, revision: u64) -> Self {
        Self {
            generation,
            revision,
            status: MrsAccelerationStatus::Empty,
            failure: None,
            total_providers: 0,
            total_accelerated_rules: 0,
            total_memory_saved_bytes: 0,
            mmap_acceleration_active: false,
            items: Vec::new(),
        }
    }

    pub fn unsupported(generation: u64, revision: u64, reason: impl Into<String>) -> Self {
        Self {
            generation,
            revision,
            status: MrsAccelerationStatus::Unsupported,
            failure: Some(reason.into()),
            ..Self::default()
        }
    }

    pub fn failed(generation: u64, revision: u64, reason: impl Into<String>) -> Self {
        Self {
            generation,
            revision,
            status: MrsAccelerationStatus::Failed,
            failure: Some(reason.into()),
            ..Self::default()
        }
    }

    pub fn unavailable(generation: u64, revision: u64, reason: impl Into<String>) -> Self {
        Self {
            generation,
            revision,
            status: MrsAccelerationStatus::Unknown,
            failure: Some(reason.into()),
            ..Self::default()
        }
    }

    pub fn is_drawable(&self) -> bool {
        self.status == MrsAccelerationStatus::Ready && !self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_fixture_has_valid_mrs_items_and_mmap() {
        let snapshot = MrsAccelerationSnapshot::demo_fixture();
        assert!(snapshot.is_drawable());
        assert_eq!(snapshot.status, MrsAccelerationStatus::Ready);
        assert_eq!(snapshot.total_providers, 3);
        assert!(snapshot.total_accelerated_rules > 30000);
        assert!(snapshot.mmap_acceleration_active);
        assert!(snapshot.items[0].is_mmap_accelerated);
        assert_eq!(snapshot.items[0].behavior, MrsBehaviorKind::IpCidr);
        assert_eq!(snapshot.items[1].behavior, MrsBehaviorKind::Domain);
    }

    #[test]
    fn empty_snapshot_reports_empty_status() {
        let snapshot = MrsAccelerationSnapshot::empty(1, 1);
        assert!(!snapshot.is_drawable());
        assert_eq!(snapshot.status, MrsAccelerationStatus::Empty);
        assert_eq!(snapshot.total_providers, 0);
    }
}

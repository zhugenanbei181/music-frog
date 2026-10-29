//! Streaming media and modern AI service availability detection contract (FLPARITY-020).
//!
//! Provides canonical typed representations for media / AI unlock capabilities
//! (Netflix, YouTube, Disney+, OpenAI/ChatGPT, Claude, Spotify) across proxy
//! nodes and outbound endpoints, including region identification, probe status,
//! and badge color classifications.

use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};

/// Supported media and modern AI service classifications for unlock detection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnlockServiceKind {
    /// Netflix streaming availability.
    Netflix,
    /// YouTube Premium / CDN caching region.
    YouTube,
    /// Disney+ streaming availability.
    DisneyPlus,
    /// OpenAI / ChatGPT web and API access.
    OpenAi,
    /// Anthropic Claude web and API access.
    Claude,
    /// Spotify music streaming.
    Spotify,
}

impl UnlockServiceKind {
    /// Canonical short service identifier.
    pub const fn id(&self) -> &'static str {
        match self {
            Self::Netflix => "netflix",
            Self::YouTube => "youtube",
            Self::DisneyPlus => "disney_plus",
            Self::OpenAi => "openai",
            Self::Claude => "claude",
            Self::Spotify => "spotify",
        }
    }

    /// User-facing display name.
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Netflix => "Netflix",
            Self::YouTube => "YouTube",
            Self::DisneyPlus => "Disney+",
            Self::OpenAi => "ChatGPT",
            Self::Claude => "Claude",
            Self::Spotify => "Spotify",
        }
    }

    /// Default probe endpoint URL for this service.
    pub const fn probe_url(&self) -> &'static str {
        match self {
            Self::Netflix => "https://www.netflix.com/title/81280792",
            Self::YouTube => "https://www.youtube.com/premium",
            Self::DisneyPlus => "https://www.disneyplus.com",
            Self::OpenAi => "https://chatgpt.com",
            Self::Claude => "https://claude.ai",
            Self::Spotify => "https://spclient.wg.spotify.com",
        }
    }

    /// All canonical probe services in presentation order.
    pub const ALL: [Self; 6] = [
        Self::Netflix,
        Self::YouTube,
        Self::DisneyPlus,
        Self::OpenAi,
        Self::Claude,
        Self::Spotify,
    ];
}

impl Display for UnlockServiceKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Detailed unlock status for a specific service.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnlockStatus {
    /// Native full library unlock (原生解锁).
    Native,
    /// DNS or SmartDNS based proxy unlock (DNS解锁).
    DnsUnlock,
    /// Original self-produced content only (仅自制剧).
    OriginalOnly,
    /// Service is blocked or restricted in node region (地区阻断).
    Blocked,
    /// Blocked by Cloudflare, WAF, or bot challenge/captcha (人机验证).
    Captcha,
    /// Probe request timed out (探测超时).
    Timeout,
    /// Network connection or protocol error (连接失败).
    Failed(String),
}

impl UnlockStatus {
    /// Localized Chinese description.
    pub fn label_zh(&self) -> &'static str {
        match self {
            Self::Native => "原生解锁",
            Self::DnsUnlock => "DNS解锁",
            Self::OriginalOnly => "仅自制剧",
            Self::Blocked => "服务阻断",
            Self::Captcha => "人机验证",
            Self::Timeout => "检测超时",
            Self::Failed(_) => "检测失败",
        }
    }

    /// English description.
    pub fn label_en(&self) -> &'static str {
        match self {
            Self::Native => "Native",
            Self::DnsUnlock => "DNS Unlock",
            Self::OriginalOnly => "Originals Only",
            Self::Blocked => "Blocked",
            Self::Captcha => "Captcha",
            Self::Timeout => "Timeout",
            Self::Failed(_) => "Failed",
        }
    }

    /// Whether this status represents a functional usable unlock.
    pub const fn is_unlocked(&self) -> bool {
        matches!(self, Self::Native | Self::DnsUnlock | Self::OriginalOnly)
    }

    /// Visual badge color classification token.
    pub const fn badge_color_role(&self) -> &'static str {
        match self {
            Self::Native => "success",
            Self::DnsUnlock => "accent",
            Self::OriginalOnly => "warning",
            Self::Blocked | Self::Captcha => "danger",
            Self::Timeout | Self::Failed(_) => "dim",
        }
    }
}

/// Result of probing a single service for a node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceUnlockResult {
    /// Target service.
    pub service: UnlockServiceKind,
    /// Evaluated unlock status.
    pub status: UnlockStatus,
    /// Detected ISO 3166-1 alpha-2 region code, e.g. "US", "JP", "HK".
    pub region_code: Option<String>,
    /// Round-trip latency in milliseconds, if successful.
    pub latency_ms: Option<u32>,
}

impl ServiceUnlockResult {
    pub fn new(service: UnlockServiceKind, status: UnlockStatus) -> Self {
        Self {
            service,
            status,
            region_code: None,
            latency_ms: None,
        }
    }

    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region_code = Some(region.into().trim().to_ascii_uppercase());
        self
    }

    pub fn with_latency(mut self, latency_ms: u32) -> Self {
        self.latency_ms = Some(latency_ms);
        self
    }

    /// User-facing short badge text (e.g. "NF: US", "AI: OK", "D+: Blocked").
    pub fn badge_text(&self) -> String {
        let prefix = match self.service {
            UnlockServiceKind::Netflix => "NF",
            UnlockServiceKind::YouTube => "YT",
            UnlockServiceKind::DisneyPlus => "D+",
            UnlockServiceKind::OpenAi => "AI",
            UnlockServiceKind::Claude => "CD",
            UnlockServiceKind::Spotify => "SP",
        };
        match (&self.status, &self.region_code) {
            (UnlockStatus::Native, Some(reg)) => format!("{prefix}: {reg}"),
            (UnlockStatus::Native, None) => format!("{prefix}: OK"),
            (UnlockStatus::DnsUnlock, Some(reg)) => format!("{prefix}: DNS·{reg}"),
            (UnlockStatus::DnsUnlock, None) => format!("{prefix}: DNS"),
            (UnlockStatus::OriginalOnly, _) => format!("{prefix}: 自制"),
            (UnlockStatus::Blocked, _) => format!("{prefix}: ×"),
            (UnlockStatus::Captcha, _) => format!("{prefix}: 验证"),
            (UnlockStatus::Timeout, _) => format!("{prefix}: 超时"),
            (UnlockStatus::Failed(_), _) => format!("{prefix}: !"),
        }
    }
}

/// Comprehensive unlock report for a proxy node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeUnlockReport {
    /// Proxy node identifier or name.
    pub node_name: String,
    /// Probe execution timestamp (ISO 8601).
    pub probed_at: String,
    /// Probed service results.
    pub services: Vec<ServiceUnlockResult>,
}

impl NodeUnlockReport {
    pub fn new(node_name: impl Into<String>, probed_at: impl Into<String>) -> Self {
        Self {
            node_name: node_name.into(),
            probed_at: probed_at.into(),
            services: Vec::new(),
        }
    }

    pub fn with_service(mut self, result: ServiceUnlockResult) -> Self {
        self.services.push(result);
        self
    }

    /// Query the probe result for a specific service.
    pub fn get(&self, service: UnlockServiceKind) -> Option<&ServiceUnlockResult> {
        self.services.iter().find(|s| s.service == service)
    }

    /// Total count of successfully unlocked services.
    pub fn unlocked_count(&self) -> usize {
        self.services
            .iter()
            .filter(|s| s.status.is_unlocked())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlock_service_kinds_and_badge_text() {
        let nf_us = ServiceUnlockResult::new(UnlockServiceKind::Netflix, UnlockStatus::Native)
            .with_region("us")
            .with_latency(120);
        assert_eq!(nf_us.badge_text(), "NF: US");
        assert_eq!(nf_us.status.badge_color_role(), "success");
        assert!(nf_us.status.is_unlocked());

        let ai_ok = ServiceUnlockResult::new(UnlockServiceKind::OpenAi, UnlockStatus::Native);
        assert_eq!(ai_ok.badge_text(), "AI: OK");

        let claude_blocked =
            ServiceUnlockResult::new(UnlockServiceKind::Claude, UnlockStatus::Blocked);
        assert_eq!(claude_blocked.badge_text(), "CD: ×");
        assert_eq!(claude_blocked.status.badge_color_role(), "danger");
        assert!(!claude_blocked.status.is_unlocked());
    }

    #[test]
    fn node_unlock_report_aggregation() {
        let report = NodeUnlockReport::new("US-SiliconValley-01", "2026-10-29T10:00:00Z")
            .with_service(
                ServiceUnlockResult::new(UnlockServiceKind::Netflix, UnlockStatus::Native)
                    .with_region("US"),
            )
            .with_service(ServiceUnlockResult::new(
                UnlockServiceKind::OpenAi,
                UnlockStatus::Native,
            ))
            .with_service(ServiceUnlockResult::new(
                UnlockServiceKind::DisneyPlus,
                UnlockStatus::Blocked,
            ));

        assert_eq!(report.unlocked_count(), 2);
        assert_eq!(
            report
                .get(UnlockServiceKind::Netflix)
                .unwrap()
                .region_code
                .as_deref(),
            Some("US")
        );
        assert_eq!(
            report.get(UnlockServiceKind::DisneyPlus).unwrap().status,
            UnlockStatus::Blocked
        );
        assert!(report.get(UnlockServiceKind::Spotify).is_none());
    }
}

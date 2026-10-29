//! Shared contract types for proxy strategy groups, leaf node selection,
//! alive-only filtering, and four-dimensional sorting.
//!
//! Track 2 / Group 04: Proxies & Sorting specifications.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::fmt::{Display, Formatter};

/// Five canonical proxy group classifications supported across all Mihomo kernels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyGroupClassification {
    /// Manual selection group (用户手动挑选目标节点).
    Selector,
    /// Automated latency benchmark group (定期测速并自动选最低延迟节点).
    #[serde(alias = "url_test", alias = "url-test", alias = "URLTest")]
    UrlTest,
    /// Fallback failover group (依序健康检查并故障自动转移).
    #[serde(alias = "fallback", alias = "Fallback")]
    Fallback,
    /// Load balance group (负载均衡，散列或轮询流量).
    #[serde(alias = "load_balance", alias = "load-balance", alias = "LoadBalance")]
    LoadBalance,
    /// Chained relay group (多跳链式中继代理).
    #[serde(alias = "relay", alias = "Relay")]
    Relay,
}

impl ProxyGroupClassification {
    /// Parse from wire string leniently, supporting Mihomo PascalCase,
    /// kebab-case, and snake_case variants.
    pub fn from_str_loose(s: &str) -> Option<Self> {
        let normalized = s.trim().to_ascii_lowercase().replace(['-', '_'], "");
        match normalized.as_str() {
            "selector" => Some(Self::Selector),
            "urltest" | "auto" => Some(Self::UrlTest),
            "fallback" => Some(Self::Fallback),
            "loadbalance" | "lb" => Some(Self::LoadBalance),
            "relay" => Some(Self::Relay),
            _ => None,
        }
    }

    /// Canonical kebab/wire string used by Mihomo configs and REST responses.
    pub fn mihomo_wire_type(&self) -> &'static str {
        match self {
            Self::Selector => "Selector",
            Self::UrlTest => "URLTest",
            Self::Fallback => "Fallback",
            Self::LoadBalance => "LoadBalance",
            Self::Relay => "Relay",
        }
    }

    /// Machine-readable snake_case identifier.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Selector => "selector",
            Self::UrlTest => "url_test",
            Self::Fallback => "fallback",
            Self::LoadBalance => "load_balance",
            Self::Relay => "relay",
        }
    }

    /// Localized Chinese label for GUI displays.
    pub fn label_zh(&self) -> &'static str {
        match self {
            Self::Selector => "手动选择",
            Self::UrlTest => "自动测速",
            Self::Fallback => "故障转移",
            Self::LoadBalance => "负载均衡",
            Self::Relay => "链式中继",
        }
    }

    /// Localized English label for GUI displays.
    pub fn label_en(&self) -> &'static str {
        match self {
            Self::Selector => "Selector",
            Self::UrlTest => "URL-Test",
            Self::Fallback => "Fallback",
            Self::LoadBalance => "Load-Balance",
            Self::Relay => "Relay",
        }
    }

    /// Whether this strategy group supports user interactive node switching (`PUT /proxies/{group}`).
    /// In Mihomo, only Selector groups accept manual node switching; automated groups manage
    /// selected node internal to the core or relay in sequential order.
    pub fn is_manual_selectable(&self) -> bool {
        matches!(self, Self::Selector)
    }

    /// Whether this strategy group runs automated node scheduling.
    pub fn is_automated(&self) -> bool {
        !self.is_manual_selectable()
    }

    /// Whether this group requires background periodic healthcheck benchmarking.
    pub fn requires_healthcheck(&self) -> bool {
        matches!(self, Self::UrlTest | Self::Fallback)
    }
}

impl Display for ProxyGroupClassification {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Latency tier for multi-color scale rendering (5 color tiers).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LatencyTier {
    /// < 100ms: Emerald green / 翡翠绿.
    Fast,
    /// 100 - 200ms: Grass green / 青草绿.
    Normal,
    /// 200 - 300ms: Warm amber / 暖黄.
    Medium,
    /// > 300ms: Alert red / 警戒红.
    Slow,
    /// Timeout (0ms) or untested (None): Muted grey / 灰色.
    Timeout,
}

impl LatencyTier {
    pub fn classify(delay_ms: Option<u32>) -> Self {
        match delay_ms {
            Some(0) => Self::Timeout,
            Some(ms) if ms < 100 => Self::Fast,
            Some(ms) if ms < 200 => Self::Normal,
            Some(ms) if ms < 300 => Self::Medium,
            Some(_) => Self::Slow,
            None => Self::Timeout,
        }
    }
}

/// Standardized human label and tier for node latency rendering.
pub fn format_latency_standard(delay_ms: Option<u32>) -> (String, LatencyTier) {
    let tier = LatencyTier::classify(delay_ms);
    let label = match delay_ms {
        Some(0) => "超时".to_owned(),
        Some(ms) => format!("{ms} ms"),
        None => "未测速".to_owned(),
    };
    (label, tier)
}

/// Canonical protocol badge representation for proxy node cards across all surfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyProtocolBadge {
    Shadowsocks,
    Vless,
    Vmess,
    Trojan,
    Hysteria2,
    WireGuard,
    Tuic,
    Ssh,
    Http,
    Socks5,
    Snell,
    Direct,
    Reject,
    Other,
}

impl ProxyProtocolBadge {
    pub fn from_str_loose(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "shadowsocks" | "ss" => Self::Shadowsocks,
            "vless" => Self::Vless,
            "vmess" => Self::Vmess,
            "trojan" | "trojan-go" => Self::Trojan,
            "hysteria2" | "hy2" | "hysteria" => Self::Hysteria2,
            "wireguard" | "wg" | "awg" | "amnezia-wg" => Self::WireGuard,
            "tuic" => Self::Tuic,
            "ssh" => Self::Ssh,
            "http" | "https" => Self::Http,
            "socks5" | "socks" => Self::Socks5,
            "snell" => Self::Snell,
            "direct" => Self::Direct,
            "reject" => Self::Reject,
            _ => Self::Other,
        }
    }

    pub const fn badge_text(self) -> &'static str {
        match self {
            Self::Shadowsocks => "SS",
            Self::Vless => "VLESS",
            Self::Vmess => "VMess",
            Self::Trojan => "Trojan",
            Self::Hysteria2 => "Hy2",
            Self::WireGuard => "WG",
            Self::Tuic => "TUIC",
            Self::Ssh => "SSH",
            Self::Http => "HTTP",
            Self::Socks5 => "SOCKS5",
            Self::Snell => "Snell",
            Self::Direct => "Direct",
            Self::Reject => "Reject",
            Self::Other => "Proxy",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Shadowsocks => "Shadowsocks",
            Self::Vless => "Vless",
            Self::Vmess => "VMess",
            Self::Trojan => "Trojan",
            Self::Hysteria2 => "Hysteria2",
            Self::WireGuard => "WireGuard",
            Self::Tuic => "Tuic",
            Self::Ssh => "SSH",
            Self::Http => "HTTP",
            Self::Socks5 => "SOCKS5",
            Self::Snell => "Snell",
            Self::Direct => "Direct",
            Self::Reject => "Reject",
            Self::Other => "Proxy",
        }
    }

    pub const fn badge_color_hex(self) -> &'static str {
        match self {
            Self::Shadowsocks => "#007AFF",
            Self::Vless => "#34C759",
            Self::Vmess => "#AF52DE",
            Self::Trojan => "#FF9500",
            Self::Hysteria2 => "#00C7BE",
            Self::WireGuard => "#E056FD",
            Self::Tuic => "#FF2D55",
            Self::Ssh => "#A4B0BE",
            Self::Http => "#5352ED",
            Self::Socks5 => "#3742FA",
            Self::Snell => "#2ED573",
            Self::Direct => "#747D8C",
            Self::Reject => "#FF4757",
            Self::Other => "#016888",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_group_classifications_coverage() {
        let cases = [
            ("Selector", ProxyGroupClassification::Selector, true),
            ("selector", ProxyGroupClassification::Selector, true),
            ("URLTest", ProxyGroupClassification::UrlTest, false),
            ("url-test", ProxyGroupClassification::UrlTest, false),
            ("url_test", ProxyGroupClassification::UrlTest, false),
            ("auto", ProxyGroupClassification::UrlTest, false),
            ("Fallback", ProxyGroupClassification::Fallback, false),
            ("fallback", ProxyGroupClassification::Fallback, false),
            ("LoadBalance", ProxyGroupClassification::LoadBalance, false),
            ("load-balance", ProxyGroupClassification::LoadBalance, false),
            ("lb", ProxyGroupClassification::LoadBalance, false),
            ("Relay", ProxyGroupClassification::Relay, false),
            ("relay", ProxyGroupClassification::Relay, false),
        ];

        for (input, expected, selectable) in cases {
            let parsed = ProxyGroupClassification::from_str_loose(input)
                .unwrap_or_else(|| panic!("failed to parse {input}"));
            assert_eq!(parsed, expected);
            assert_eq!(parsed.is_manual_selectable(), selectable);
            assert_eq!(parsed.is_automated(), !selectable);
        }

        assert_eq!(ProxyGroupClassification::from_str_loose("unknown"), None);
    }

    #[test]
    fn latency_tier_color_classification() {
        assert_eq!(LatencyTier::classify(Some(45)), LatencyTier::Fast);
        assert_eq!(LatencyTier::classify(Some(120)), LatencyTier::Normal);
        assert_eq!(LatencyTier::classify(Some(250)), LatencyTier::Medium);
        assert_eq!(LatencyTier::classify(Some(350)), LatencyTier::Slow);
        assert_eq!(LatencyTier::classify(Some(0)), LatencyTier::Timeout);
        assert_eq!(LatencyTier::classify(None), LatencyTier::Timeout);

        let (label, tier) = format_latency_standard(Some(88));
        assert_eq!(label, "88 ms");
        assert_eq!(tier, LatencyTier::Fast);

        let (label, tier) = format_latency_standard(Some(0));
        assert_eq!(label, "超时");
        assert_eq!(tier, LatencyTier::Timeout);
    }

    #[test]
    fn proxy_protocol_badge_classification() {
        assert_eq!(
            ProxyProtocolBadge::from_str_loose("ss"),
            ProxyProtocolBadge::Shadowsocks
        );
        assert_eq!(
            ProxyProtocolBadge::from_str_loose("Shadowsocks").badge_text(),
            "SS"
        );
        assert_eq!(
            ProxyProtocolBadge::from_str_loose("hy2"),
            ProxyProtocolBadge::Hysteria2
        );
        assert_eq!(
            ProxyProtocolBadge::from_str_loose("Hysteria2").badge_text(),
            "Hy2"
        );
        assert_eq!(
            ProxyProtocolBadge::from_str_loose("vless").badge_text(),
            "VLESS"
        );
        assert_eq!(
            ProxyProtocolBadge::from_str_loose("vmess").badge_text(),
            "VMess"
        );
        assert_eq!(
            ProxyProtocolBadge::from_str_loose("trojan").badge_text(),
            "Trojan"
        );
        assert_eq!(
            ProxyProtocolBadge::from_str_loose("unknown").badge_text(),
            "Proxy"
        );
        assert_eq!(
            ProxyProtocolBadge::from_str_loose("ss").badge_color_hex(),
            "#007AFF"
        );
    }
}

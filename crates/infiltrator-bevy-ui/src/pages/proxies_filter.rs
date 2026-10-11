//! Filter, search, and region flag utilities for the Proxies domain.
//!
//! Subtree providing multi-mode fuzzy search, pinyin matching,
//! latency tier bounds, and emoji flag matching for proxy nodes.

use crate::pages::proxies::ProxyNode;
use infiltrator_shared::fuzzy_search::pinyin_fuzzy_match;
use zhuge_ui_core::text::{CaseSensitivity, SearchTextRun, project_text_runs};

/// Multi-mode fuzzy search and pinyin/abbreviation filter (BEVY-GAP-032).
pub fn matches_proxy_filter(node: &ProxyNode, query: &str) -> bool {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return true;
    }
    let name = node.name.to_ascii_lowercase();
    if name.contains(&query)
        || node
            .outbound_id
            .as_ref()
            .is_some_and(|id| id.value().to_ascii_lowercase().contains(&query))
    {
        return true;
    }
    if let Some(limit) = query
        .strip_prefix('<')
        .or_else(|| query.strip_prefix('>'))
        .and_then(|value| value.trim().parse::<u32>().ok())
    {
        return node
            .delay_ms
            .filter(|delay| *delay > 0)
            .is_some_and(|delay| {
                if query.starts_with('<') {
                    delay < limit
                } else {
                    delay > limit
                }
            });
    }
    let protocol = node.node_type.to_ascii_lowercase();
    let metadata_match = protocol.contains(&query)
        || (query == "ss" && protocol == "shadowsocks")
        || (matches!(query.as_str(), "hy" | "hy2") && protocol == "hysteria2")
        || node
            .features
            .iter()
            .any(|tag| tag.to_ascii_lowercase().contains(&query));
    metadata_match || pinyin_fuzzy_match(&node.name, &query)
}

pub fn name_runs(node: &ProxyNode, query: &str) -> Vec<SearchTextRun> {
    let mut runs = project_text_runs(&node.name, query, CaseSensitivity::AsciiInsensitive);
    if !query.trim().is_empty()
        && !runs.iter().any(|run| run.highlighted)
        && pinyin_fuzzy_match(&node.name, &query.trim().to_ascii_lowercase())
    {
        for run in &mut runs {
            run.highlighted = true;
        }
    }
    runs
}

/// Canonical display name for proxy protocols (Shadowsocks, Vless, VMess, Trojan, Hysteria2).
pub fn format_protocol_chip(raw_type: &str) -> String {
    match raw_type.to_ascii_lowercase().as_str() {
        "shadowsocks" | "ss" => "Shadowsocks".to_string(),
        "vless" => "Vless".to_string(),
        "vmess" => "VMess".to_string(),
        "trojan" => "Trojan".to_string(),
        "hysteria2" | "hy2" => "Hysteria2".to_string(),
        "wireguard" => "WireGuard".to_string(),
        "tuic" => "Tuic".to_string(),
        "http" => "HTTP".to_string(),
        "socks5" => "SOCKS5".to_string(),
        "snell" => "Snell".to_string(),
        "direct" => "Direct".to_string(),
        "reject" => "Reject".to_string(),
        _ if !raw_type.is_empty() => raw_type.to_string(),
        _ => "Proxy".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::outbound::details::OutboundHealthObservation;
    use infiltrator_shared::country_flags::node_flag_emoji;

    #[test]
    fn test_node_flag_extraction() {
        assert_eq!(node_flag_emoji("香港 01"), "🇭🇰");
        assert_eq!(node_flag_emoji("Tokyo VIP"), "🇯🇵");
        assert_eq!(node_flag_emoji("Singapore SG 02"), "🇸🇬");
        assert_eq!(node_flag_emoji("US Silicon Valley"), "🇺🇸");
        assert_eq!(node_flag_emoji("Unknown Server"), "🌐");
    }

    #[test]
    fn test_matches_proxy_filter() {
        let node = ProxyNode {
            outbound_id: None,
            name: "🇭🇰 香港 01 · BGP 专线".to_owned(),
            node_type: "VLESS".to_owned(),
            delay_ms: Some(45),
            health: OutboundHealthObservation::Unknown,
            selected: true,
            favorite: true,
            features: vec!["Reality".to_owned(), "Vision".to_owned()],
        };

        assert!(matches_proxy_filter(&node, ""));
        assert!(matches_proxy_filter(&node, "香港"));
        assert!(matches_proxy_filter(&node, "xg"));
        assert!(matches_proxy_filter(&node, "hk"));
        assert!(matches_proxy_filter(&node, "vless"));
        assert!(matches_proxy_filter(&node, "reality"));
        assert!(matches_proxy_filter(&node, "<100"));
        assert!(!matches_proxy_filter(&node, ">100"));
        assert!(!matches_proxy_filter(&node, "日本"));
    }

    #[test]
    fn test_format_protocol_chip() {
        assert_eq!(format_protocol_chip("Shadowsocks"), "Shadowsocks");
        assert_eq!(format_protocol_chip("ss"), "Shadowsocks");
        assert_eq!(format_protocol_chip("vless"), "Vless");
        assert_eq!(format_protocol_chip("vmess"), "VMess");
        assert_eq!(format_protocol_chip("trojan"), "Trojan");
        assert_eq!(format_protocol_chip("hy2"), "Hysteria2");
        assert_eq!(format_protocol_chip("wireguard"), "WireGuard");
        assert_eq!(format_protocol_chip(""), "Proxy");
    }
}

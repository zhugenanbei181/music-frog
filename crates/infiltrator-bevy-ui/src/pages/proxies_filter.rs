//! Filter, search, and region flag utilities for the Proxies domain.
//!
//! Subtree providing multi-mode fuzzy search, pinyin matching,
//! latency tier bounds, and emoji flag matching for proxy nodes.

use crate::pages::proxies::ProxyNode;
use infiltrator_application::proxy_search_projection::search_node;
use infiltrator_contract::surface_snapshot::ProxyNodeSnapshot;

/// Multi-mode fuzzy search and pinyin/abbreviation filter (BEVY-GAP-032).
pub fn matches_proxy_filter(node: &ProxyNode, query: &str) -> bool {
    search_node(
        &ProxyNodeSnapshot {
            name: node.name.clone(),
            node_type: node.node_type.clone(),
            delay_ms: node.delay_ms,
            alive: None,
            selected: node.selected,
            favorite: node.favorite,
            features: node.features.clone(),
        },
        query,
    )
    .matches
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
            name: "🇭🇰 香港 01 · BGP 专线".to_owned(),
            node_type: "VLESS".to_owned(),
            delay_ms: Some(45),
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

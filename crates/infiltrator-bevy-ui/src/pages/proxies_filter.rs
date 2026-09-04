//! Filter, search, and region flag utilities for the Proxies domain.
//!
//! Subtree providing multi-mode fuzzy search, pinyin matching,
//! latency tier bounds, and emoji flag matching for proxy nodes.

use crate::pages::proxies::ProxyNode;

pub fn has_word_or_code(text: &str, code: &str) -> bool {
    let bytes = text.as_bytes();
    let code_bytes = code.as_bytes();
    if code_bytes.len() > bytes.len() {
        return false;
    }
    for (i, window) in bytes.windows(code_bytes.len()).enumerate() {
        if window.eq_ignore_ascii_case(code_bytes) {
            let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
            let after_idx = i + code_bytes.len();
            let after_ok = after_idx == bytes.len() || !bytes[after_idx].is_ascii_alphanumeric();
            if before_ok && after_ok {
                return true;
            }
        }
    }
    false
}

/// Convenience helper to return the flag emoji for a node name, or 🌐 if unrecognized.
pub fn node_flag(name: &str) -> &'static str {
    let trimmed = name.trim();
    for flag in [
        "🇭🇰", "🇯🇵", "🇸🇬", "🇺🇸", "🇹🇼", "🇰🇷", "🇬🇧", "🇩🇪", "🇫🇷", "🇨🇦", "🇦🇺", "🇷🇺", "🇮🇳", "🇳🇱", "🇧🇷",
        "🇹🇷", "🇦🇷", "🇵🇭", "🇹🇭", "🇲🇾", "🇻🇳", "🇦🇪", "🇨🇳",
    ] {
        if trimmed.starts_with(flag) {
            return flag;
        }
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("香港")
        || lower.contains("hong kong")
        || lower.contains("hongkong")
        || has_word_or_code(trimmed, "HK")
    {
        "🇭🇰"
    } else if lower.contains("日本")
        || lower.contains("东京")
        || lower.contains("osaka")
        || lower.contains("tokyo")
        || lower.contains("japan")
        || has_word_or_code(trimmed, "JP")
    {
        "🇯🇵"
    } else if lower.contains("新加坡")
        || lower.contains("狮城")
        || lower.contains("singapore")
        || has_word_or_code(trimmed, "SG")
    {
        "🇸🇬"
    } else if lower.contains("美国")
        || lower.contains("硅谷")
        || lower.contains("united states")
        || lower.contains("america")
        || has_word_or_code(trimmed, "US")
    {
        "🇺🇸"
    } else if lower.contains("台湾")
        || lower.contains("taiwan")
        || lower.contains("taipei")
        || has_word_or_code(trimmed, "TW")
    {
        "🇹🇼"
    } else if lower.contains("韩国")
        || lower.contains("首尔")
        || lower.contains("korea")
        || lower.contains("seoul")
        || has_word_or_code(trimmed, "KR")
    {
        "🇰🇷"
    } else if lower.contains("英国")
        || lower.contains("伦敦")
        || lower.contains("united kingdom")
        || lower.contains("london")
        || has_word_or_code(trimmed, "UK")
        || has_word_or_code(trimmed, "GB")
    {
        "🇬🇧"
    } else if lower.contains("德国")
        || lower.contains("germany")
        || lower.contains("frankfurt")
        || has_word_or_code(trimmed, "DE")
    {
        "🇩🇪"
    } else if lower.contains("法国")
        || lower.contains("france")
        || lower.contains("paris")
        || has_word_or_code(trimmed, "FR")
    {
        "🇫🇷"
    } else if lower.contains("加拿大")
        || lower.contains("canada")
        || lower.contains("toronto")
        || has_word_or_code(trimmed, "CA")
    {
        "🇨🇦"
    } else if lower.contains("澳大利亚")
        || lower.contains("澳洲")
        || lower.contains("australia")
        || lower.contains("sydney")
        || has_word_or_code(trimmed, "AU")
    {
        "🇦🇺"
    } else if lower.contains("中国") || lower.contains("china") || has_word_or_code(trimmed, "CN")
    {
        "🇨🇳"
    } else {
        "🌐"
    }
}

/// Multi-mode fuzzy search and pinyin/abbreviation filter (BEVY-GAP-032).
pub fn matches_proxy_filter(node: &ProxyNode, query: &str) -> bool {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return true;
    }
    // Match node name
    if node.name.to_ascii_lowercase().contains(&q) {
        return true;
    }
    // Match protocol or protocol abbreviation
    let proto = node.node_type.to_ascii_lowercase();
    if proto.contains(&q) {
        return true;
    }
    if q == "ss" && (proto.contains("shadowsocks") || proto == "ss") {
        return true;
    }
    if (q == "hy2" || q == "hy") && (proto.contains("hysteria") || proto.contains("hy2")) {
        return true;
    }
    // Match feature tags (e.g. "reality", "vision", "udp")
    if node
        .features
        .iter()
        .any(|f| f.to_ascii_lowercase().contains(&q))
    {
        return true;
    }
    // Match latency filter (e.g. "<100" or ">200")
    if let Some(rest) = q.strip_prefix('<')
        && let Ok(limit) = rest.trim().parse::<u32>()
    {
        return node.delay_ms.is_some_and(|ms| ms < limit);
    }
    if let Some(rest) = q.strip_prefix('>')
        && let Ok(limit) = rest.trim().parse::<u32>()
    {
        return node.delay_ms.is_some_and(|ms| ms > limit);
    }
    // Pinyin initials and country code abbreviations
    if q == "hk" || q == "xg" {
        return node.name.contains("香港") || node.name.to_ascii_lowercase().contains("hk");
    }
    if q == "jp" || q == "rb" {
        return node.name.contains("日本") || node.name.to_ascii_lowercase().contains("jp");
    }
    if q == "sg" || q == "xjp" {
        return node.name.contains("新加坡") || node.name.to_ascii_lowercase().contains("sg");
    }
    if q == "us" || q == "mg" {
        return node.name.contains("美国") || node.name.to_ascii_lowercase().contains("us");
    }
    if q == "tw" {
        return node.name.contains("台湾") || node.name.to_ascii_lowercase().contains("tw");
    }
    false
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

    #[test]
    fn test_node_flag_extraction() {
        assert_eq!(node_flag("香港 01"), "🇭🇰");
        assert_eq!(node_flag("Tokyo VIP"), "🇯🇵");
        assert_eq!(node_flag("Singapore SG 02"), "🇸🇬");
        assert_eq!(node_flag("US Silicon Valley"), "🇺🇸");
        assert_eq!(node_flag("Unknown Server"), "🌐");
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
